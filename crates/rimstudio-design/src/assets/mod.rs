//! Imported assets: what the designer knows about a texture or a sound clip before it copies the file.
//!
//! This crate does no file access. The toolkit reads a source file under a size limit, hashes it and hands
//! the bytes to [`detect`], which recognises the format by its signature and reads the header in house
//! ([`png`], [`wav`], [`ogg`]); the result is an [`AssetInfo`], stored in an [`AssetFacts`] map by source
//! path. The validation and the planner of this crate work from those facts, so they stay pure and testable
//! with byte arrays built in code. No file is ever decoded.
//!
//! The limits are part of the contract (specification section 8.6 of the items toolkit): a texture is at most
//! [`MAX_TEXTURE_BYTES`] and [`MAX_TEXTURE_DIMENSION`] pixels on a side, a sound clip at most
//! [`MAX_CLIP_BYTES`].

use std::collections::BTreeMap;

pub mod ogg;
pub mod png;
pub mod wav;

pub use ogg::{OggCodec, OggInfo, OggProblem};
pub use png::{PngInfo, PngProblem};
pub use wav::{WavInfo, WavProblem};

/// The largest texture file the designer copies: 8 MiB.
pub const MAX_TEXTURE_BYTES: u64 = 8 * 1024 * 1024;
/// The largest width or height of an imported texture, in pixels.
pub const MAX_TEXTURE_DIMENSION: u32 = 4096;
/// A texture above this many pixels on a side gets a warning (the game's own weapon art is far smaller).
pub const LARGE_TEXTURE_DIMENSION: u32 = 1024;
/// A texture file above this many bytes gets a warning.
pub const LARGE_TEXTURE_BYTES: u64 = 2 * 1024 * 1024;
/// The largest sound clip the designer copies: 20 MiB.
pub const MAX_CLIP_BYTES: u64 = 20 * 1024 * 1024;
/// A texture up to this many bytes gets a preview thumbnail in `designer_asset_info`: 256 KiB.
pub const PREVIEW_MAX_BYTES: u64 = 256 * 1024;

/// The file formats the designer recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssetFormat {
    /// A PNG image.
    Png,
    /// A RIFF WAVE sound.
    Wav,
    /// An Ogg stream (Vorbis or Opus).
    Ogg,
}

impl AssetFormat {
    /// The file extension the copy gets (lower case, no dot).
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }
}

/// What the signature and the header of a file say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detected {
    /// A PNG with a valid header.
    Png(PngInfo),
    /// A WAV with a valid format and data chunk.
    Wav(WavInfo),
    /// An Ogg stream with a valid first page.
    Ogg(OggInfo),
    /// The signature of a known format with a header that is not usable.
    Broken {
        /// The format the signature names.
        format: AssetFormat,
        /// A plain sentence fragment saying what is wrong.
        reason: &'static str,
    },
    /// Neither a PNG, a WAV nor an Ogg stream.
    Unknown,
}

impl Detected {
    /// The format, `None` for [`Detected::Unknown`].
    #[must_use]
    pub fn format(&self) -> Option<AssetFormat> {
        match self {
            Self::Png(_) => Some(AssetFormat::Png),
            Self::Wav(_) => Some(AssetFormat::Wav),
            Self::Ogg(_) => Some(AssetFormat::Ogg),
            Self::Broken { format, .. } => Some(*format),
            Self::Unknown => None,
        }
    }

    /// The channel count of a sound, when known.
    #[must_use]
    pub fn channels(&self) -> Option<u16> {
        match self {
            Self::Wav(w) => Some(w.channels),
            Self::Ogg(o) => o.channels.map(u16::from),
            _ => None,
        }
    }
}

/// Recognises a file by its signature and reads its header.
#[must_use]
pub fn detect(bytes: &[u8]) -> Detected {
    if png::has_signature(bytes) {
        return match png::parse_png(bytes) {
            Ok(info) => Detected::Png(info),
            Err(p) => Detected::Broken {
                format: AssetFormat::Png,
                reason: p.reason(),
            },
        };
    }
    if wav::has_signature(bytes) {
        return match wav::parse_wav(bytes) {
            Ok(info) => Detected::Wav(info),
            Err(p) => Detected::Broken {
                format: AssetFormat::Wav,
                reason: p.reason(),
            },
        };
    }
    if ogg::has_signature(bytes) {
        return match ogg::parse_ogg(bytes) {
            Ok(info) => Detected::Ogg(info),
            Err(p) => Detected::Broken {
                format: AssetFormat::Ogg,
                reason: p.reason(),
            },
        };
    }
    Detected::Unknown
}

/// A source file the toolkit read: its size, its hash and what its header says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfo {
    /// Size in bytes.
    pub bytes: u64,
    /// SHA-256 of the content, 64 lower case hexadecimal characters.
    pub sha256: String,
    /// What the signature and the header say.
    pub detected: Detected,
}

impl AssetInfo {
    /// Width and height of an image, `None` for anything else.
    #[must_use]
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match &self.detected {
            Detected::Png(p) => Some((p.width, p.height)),
            _ => None,
        }
    }
}

/// What the toolkit found at a source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceState {
    /// Nothing is there (or the path is empty).
    Missing,
    /// Something is there that is not accepted: a link, a folder, an unreadable file. The text says why.
    Refused(String),
    /// A regular file above the size limit that was not read; the size is given.
    TooLarge(u64),
    /// A regular file that was read.
    Read(AssetInfo),
}

/// The source files of a design as the toolkit read them, by source path as written in the spec.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFacts {
    sources: BTreeMap<String, SourceState>,
}

impl AssetFacts {
    /// No file inspected.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what was found at a source path.
    pub fn insert(&mut self, path: impl Into<String>, state: SourceState) {
        self.sources.insert(path.into(), state);
    }

    /// What was found at a source path, `None` when the path was not inspected.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&SourceState> {
        self.sources.get(path)
    }

    /// True when nothing was inspected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_follows_the_signature_not_the_name() {
        assert!(matches!(detect(&png::build_png(4, 4)), Detected::Png(_)));
        assert!(matches!(
            detect(&wav::build_wav(1, 8000, 4)),
            Detected::Wav(_)
        ));
        assert!(matches!(
            detect(&ogg::build_ogg_vorbis(1, 8000)),
            Detected::Ogg(_)
        ));
        assert_eq!(detect(b"<html></html>"), Detected::Unknown);
        assert_eq!(detect(b""), Detected::Unknown);
        assert!(matches!(
            detect(&png::PNG_SIGNATURE),
            Detected::Broken {
                format: AssetFormat::Png,
                ..
            }
        ));
    }

    #[test]
    fn channels_come_from_the_header() {
        assert_eq!(detect(&wav::build_wav(2, 8000, 4)).channels(), Some(2));
        assert_eq!(detect(&ogg::build_ogg_vorbis(1, 8000)).channels(), Some(1));
        assert_eq!(detect(&png::build_png(2, 2)).channels(), None);
    }
}
