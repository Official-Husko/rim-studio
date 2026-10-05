//! The header of a WAV file, read in house. The audio is never decoded.
//!
//! A WAV file is a `RIFF` container of type `WAVE` that holds a `fmt ` chunk (the encoding, the channel
//! count, the sample rate, the bit depth) and a `data` chunk. The reader walks the chunks with every offset
//! and size checked against the length of the file: a chunk that claims more bytes than the file has, a
//! missing `fmt ` or `data` chunk and a format the game cannot play are reported as a [`WavProblem`], never
//! as a panic. The size field of the `RIFF` header itself is ignored, because many writers get it wrong.

/// What the `fmt ` chunk and the `data` chunk say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    /// The encoding: 1 integer PCM, 2 ADPCM, 3 floating point, 0xFFFE extensible.
    pub format_tag: u16,
    /// Number of channels (1 mono, 2 stereo).
    pub channels: u16,
    /// Samples per second.
    pub sample_rate: u32,
    /// Bits per sample.
    pub bits_per_sample: u16,
    /// Bytes of audio in the `data` chunk.
    pub data_bytes: u32,
    /// Length of the clip in milliseconds, 0 when it cannot be worked out (compressed encodings).
    pub duration_ms: u64,
}

/// Why a file that starts like a WAV is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavProblem {
    /// The file ends inside the header or inside a chunk header.
    Truncated,
    /// A chunk claims more bytes than the file holds.
    ChunkSize,
    /// No `fmt ` chunk before the audio.
    NoFormat,
    /// No `data` chunk.
    NoData,
    /// The `fmt ` chunk is too short or holds impossible values (no channel, rate zero).
    BadFormat,
    /// An encoding the game does not play.
    Encoding,
}

impl WavProblem {
    /// A plain sentence fragment for messages.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Truncated => "the file ends inside the WAV header",
            Self::ChunkSize => "a chunk of the WAV file is larger than the file",
            Self::NoFormat => "the WAV file has no format chunk",
            Self::NoData => "the WAV file has no audio data",
            Self::BadFormat => "the WAV format chunk holds impossible values",
            Self::Encoding => "the WAV file uses an encoding the game does not play",
        }
    }
}

/// True when the bytes start like a WAV file (`RIFF`, a size, `WAVE`).
#[must_use]
pub fn has_signature(bytes: &[u8]) -> bool {
    bytes.get(..4) == Some(b"RIFF".as_slice()) && bytes.get(8..12) == Some(b"WAVE".as_slice())
}

fn le16(bytes: &[u8], at: usize) -> Option<u16> {
    let s = bytes.get(at..at.checked_add(2)?)?;
    let mut w = [0u8; 2];
    w.copy_from_slice(s);
    Some(u16::from_le_bytes(w))
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    let s = bytes.get(at..at.checked_add(4)?)?;
    let mut w = [0u8; 4];
    w.copy_from_slice(s);
    Some(u32::from_le_bytes(w))
}

/// Reads the format and the data size of a WAV file.
///
/// # Errors
///
/// A [`WavProblem`] naming what is wrong.
pub fn parse_wav(bytes: &[u8]) -> Result<WavInfo, WavProblem> {
    if !has_signature(bytes) {
        return Err(WavProblem::Truncated);
    }
    let mut pos = 12usize;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data_bytes: Option<u32> = None;
    // the number of chunks is bounded by the length of the file, so the loop ends
    while pos < bytes.len() && data_bytes.is_none() {
        let (Some(size), Some(id)) = (le32(bytes, pos + 4), bytes.get(pos..pos + 4)) else {
            return Err(WavProblem::Truncated);
        };
        let body = pos + 8;
        let size_usize = usize::try_from(size).map_err(|_| WavProblem::ChunkSize)?;
        let end = body.checked_add(size_usize).ok_or(WavProblem::ChunkSize)?;
        if id == b"data" {
            // a data chunk may be the last one and may run to the end of the file, never past it
            if end > bytes.len() {
                return Err(WavProblem::ChunkSize);
            }
            data_bytes = Some(size);
            break;
        }
        if end > bytes.len() {
            return Err(WavProblem::ChunkSize);
        }
        if id == b"fmt " {
            if size < 16 {
                return Err(WavProblem::BadFormat);
            }
            let (Some(tag), Some(channels), Some(rate), Some(bits)) = (
                le16(bytes, body),
                le16(bytes, body + 2),
                le32(bytes, body + 4),
                le16(bytes, body + 14),
            ) else {
                return Err(WavProblem::Truncated);
            };
            format = Some((tag, channels, rate, bits));
        }
        // chunks are padded to an even size
        pos = end.saturating_add(size_usize & 1);
    }
    let (format_tag, channels, sample_rate, bits_per_sample) =
        format.ok_or(WavProblem::NoFormat)?;
    let data_bytes = data_bytes.ok_or(WavProblem::NoData)?;
    if channels == 0 || sample_rate == 0 {
        return Err(WavProblem::BadFormat);
    }
    if !matches!(format_tag, 1..=3 | 0xfffe) {
        return Err(WavProblem::Encoding);
    }
    if matches!(format_tag, 1 | 3 | 0xfffe) && bits_per_sample == 0 {
        return Err(WavProblem::BadFormat);
    }
    let bytes_per_second =
        u64::from(sample_rate) * u64::from(channels) * u64::from(bits_per_sample) / 8;
    let duration_ms = if format_tag == 2 || bytes_per_second == 0 {
        0
    } else {
        u64::from(data_bytes) * 1000 / bytes_per_second
    };
    Ok(WavInfo {
        format_tag,
        channels,
        sample_rate,
        bits_per_sample,
        data_bytes,
        duration_ms,
    })
}

/// A valid 16 bit PCM WAV file holding `frames` frames of silence. For tests and fixtures.
#[must_use]
pub fn build_wav(channels: u16, sample_rate: u32, frames: u32) -> Vec<u8> {
    let data_len = frames.saturating_mul(u32::from(channels)).saturating_mul(2);
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32.saturating_add(data_len)).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate
        .saturating_mul(u32::from(channels))
        .saturating_mul(2);
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&channels.saturating_mul(2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(out.len() + usize::try_from(data_len).unwrap_or(0), 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_built_wav_parses() {
        let wav = build_wav(1, 44_100, 22_050);
        let info = parse_wav(&wav).unwrap();
        assert_eq!(info.channels, 1);
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(info.data_bytes, 44_100);
        assert_eq!(info.duration_ms, 500);
        assert_eq!(parse_wav(&build_wav(2, 48_000, 10)).unwrap().channels, 2);
    }

    #[test]
    fn a_chunk_size_past_the_end_is_refused() {
        let mut wav = build_wav(1, 8000, 100);
        // the data chunk claims far more bytes than the file holds
        let at = wav.len() - 200 - 4;
        wav[at..at + 4].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
        assert_eq!(parse_wav(&wav), Err(WavProblem::ChunkSize));
        // a fmt chunk that claims to be larger than the file
        let mut wav = build_wav(1, 8000, 4);
        wav[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_wav(&wav), Err(WavProblem::ChunkSize));
    }

    #[test]
    fn a_truncated_data_chunk_is_refused() {
        let wav = build_wav(1, 8000, 100);
        assert_eq!(parse_wav(&wav[..wav.len() - 1]), Err(WavProblem::ChunkSize));
        assert_eq!(parse_wav(&wav[..30]), Err(WavProblem::ChunkSize));
    }

    #[test]
    fn missing_chunks_and_bad_formats_are_named() {
        let wav = build_wav(1, 8000, 4);
        // a header with only the RIFF part
        assert_eq!(parse_wav(&wav[..12]), Err(WavProblem::NoFormat));
        let mut zero = wav.clone();
        zero[22..24].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(parse_wav(&zero), Err(WavProblem::BadFormat));
        let mut mp3 = wav.clone();
        mp3[20..22].copy_from_slice(&0x55u16.to_le_bytes());
        assert_eq!(parse_wav(&mp3), Err(WavProblem::Encoding));
    }

    #[test]
    fn an_odd_sized_chunk_before_the_format_is_skipped_with_its_padding() {
        let base = build_wav(1, 8000, 4);
        let mut wav = base[..12].to_vec();
        wav.extend_from_slice(b"LIST");
        wav.extend_from_slice(&3u32.to_le_bytes());
        wav.extend_from_slice(&[1, 2, 3, 0]);
        wav.extend_from_slice(&base[12..]);
        assert_eq!(parse_wav(&wav).unwrap().sample_rate, 8000);
    }

    proptest! {
        #[test]
        fn the_parser_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            let _ = parse_wav(&bytes);
        }

        #[test]
        fn a_damaged_wav_never_panics(at in 0usize..60, value in any::<u8>()) {
            let mut wav = build_wav(2, 22_050, 8);
            if let Some(b) = wav.get_mut(at) { *b = value; }
            let _ = parse_wav(&wav);
            let end = at.min(wav.len());
            let _ = parse_wav(&wav[..end]);
        }
    }
}
