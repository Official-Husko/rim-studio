//! The first page of an Ogg file, read in house. The audio is never decoded.
//!
//! The reader checks the `OggS` capture pattern, the stream structure version, the beginning of stream flag,
//! the segment table and that the first page fits in the file. When the first packet is a Vorbis or Opus
//! identification header the channel count and the sample rate are read from it.

/// The codec of the first packet, when the reader recognises it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OggCodec {
    /// Ogg Vorbis, the format the game's own clips use.
    Vorbis,
    /// Ogg Opus.
    Opus,
}

/// What the first page of an Ogg file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OggInfo {
    /// The codec of the first packet, `None` for one the reader does not know.
    pub codec: Option<OggCodec>,
    /// Number of channels, when the codec header was read.
    pub channels: Option<u8>,
    /// Sample rate, when the codec header was read.
    pub sample_rate: Option<u32>,
}

/// Why a file that starts like an Ogg stream is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OggProblem {
    /// The file ends inside the first page header.
    Truncated,
    /// The stream structure version is not 0, or the first page is not marked as the beginning.
    BadHeader,
    /// The first page claims more bytes than the file holds.
    PageSize,
}

impl OggProblem {
    /// A plain sentence fragment for messages.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Truncated => "the file ends inside the Ogg header",
            Self::BadHeader => "the Ogg header is not valid",
            Self::PageSize => "the first Ogg page is larger than the file",
        }
    }
}

/// True when the bytes start with the Ogg capture pattern.
#[must_use]
pub fn has_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
}

/// Reads the first page of an Ogg file.
///
/// # Errors
///
/// An [`OggProblem`] naming what is wrong.
pub fn parse_ogg(bytes: &[u8]) -> Result<OggInfo, OggProblem> {
    if !has_signature(bytes) {
        return Err(OggProblem::BadHeader);
    }
    let (Some(&version), Some(&flags), Some(&segments)) =
        (bytes.get(4), bytes.get(5), bytes.get(26))
    else {
        return Err(OggProblem::Truncated);
    };
    if version != 0 || flags & 0x02 == 0 {
        return Err(OggProblem::BadHeader);
    }
    let table_end = 27 + usize::from(segments);
    let table = bytes.get(27..table_end).ok_or(OggProblem::Truncated)?;
    let body: usize = table.iter().map(|&s| usize::from(s)).sum();
    let page_end = table_end + body;
    let packet = bytes.get(table_end..page_end).ok_or(OggProblem::PageSize)?;
    let info = if packet.starts_with(b"\x01vorbis") {
        OggInfo {
            codec: Some(OggCodec::Vorbis),
            channels: packet.get(11).copied(),
            sample_rate: read_u32(packet, 12),
        }
    } else if packet.starts_with(b"OpusHead") {
        OggInfo {
            codec: Some(OggCodec::Opus),
            channels: packet.get(9).copied(),
            sample_rate: read_u32(packet, 12),
        }
    } else {
        OggInfo {
            codec: None,
            channels: None,
            sample_rate: None,
        }
    };
    Ok(info)
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let s = bytes.get(at..at.checked_add(4)?)?;
    let mut w = [0u8; 4];
    w.copy_from_slice(s);
    Some(u32::from_le_bytes(w))
}

/// A first Ogg page that holds a Vorbis identification header. For tests and fixtures; the file is not a
/// playable stream, only a valid beginning.
#[must_use]
pub fn build_ogg_vorbis(channels: u8, sample_rate: u32) -> Vec<u8> {
    let mut packet = b"\x01vorbis".to_vec();
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.push(channels);
    packet.extend_from_slice(&sample_rate.to_le_bytes());
    packet.extend_from_slice(&[0; 12]);
    packet.extend_from_slice(&[0xb8, 0x01]);
    let mut out = b"OggS".to_vec();
    out.extend_from_slice(&[0, 0x02]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.push(1);
    out.push(u8::try_from(packet.len()).unwrap_or(0));
    out.extend_from_slice(&packet);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_vorbis_header_is_read() {
        let ogg = build_ogg_vorbis(2, 44_100);
        let info = parse_ogg(&ogg).unwrap();
        assert_eq!(info.codec, Some(OggCodec::Vorbis));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.sample_rate, Some(44_100));
    }

    #[test]
    fn damaged_headers_are_named() {
        let ogg = build_ogg_vorbis(1, 22_050);
        assert_eq!(parse_ogg(&ogg[..20]), Err(OggProblem::Truncated));
        assert_eq!(parse_ogg(&ogg[..ogg.len() - 3]), Err(OggProblem::PageSize));
        let mut bad = ogg.clone();
        bad[4] = 1;
        assert_eq!(parse_ogg(&bad), Err(OggProblem::BadHeader));
        let mut not_first = ogg;
        not_first[5] = 0;
        assert_eq!(parse_ogg(&not_first), Err(OggProblem::BadHeader));
        assert_eq!(parse_ogg(b"RIFF"), Err(OggProblem::BadHeader));
    }

    proptest! {
        #[test]
        fn the_parser_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            let _ = parse_ogg(&bytes);
        }

        #[test]
        fn a_damaged_ogg_never_panics(at in 0usize..60, value in any::<u8>()) {
            let mut ogg = build_ogg_vorbis(2, 44_100);
            if let Some(b) = ogg.get_mut(at) { *b = value; }
            let _ = parse_ogg(&ogg);
        }
    }
}
