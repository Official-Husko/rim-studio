//! The header of a PNG file, read in house. The image is never decoded.
//!
//! The reader checks the eight byte signature, the `IHDR` chunk (the first chunk, 13 bytes of data, a valid
//! combination of bit depth and colour type, dimensions above zero, a matching CRC) and whether the file ends
//! with the `IEND` chunk, which a cut off file does not. Everything is bounds checked; no input panics.

/// The signature every PNG file starts with.
pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The twelve bytes that end a complete PNG file: an empty `IEND` chunk with its CRC.
const IEND: [u8; 12] = [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82];

/// What the `IHDR` chunk says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bits per sample.
    pub bit_depth: u8,
    /// The PNG colour type (0 grey, 2 colour, 3 palette, 4 grey with alpha, 6 colour with alpha).
    pub color_type: u8,
    /// True when the file ends with the `IEND` chunk.
    pub complete: bool,
}

/// Why a file that starts like a PNG is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngProblem {
    /// Fewer bytes than the signature and the `IHDR` chunk need.
    Truncated,
    /// The first chunk is not a valid `IHDR`.
    BadHeader,
    /// The checksum of the `IHDR` chunk does not match.
    BadChecksum,
    /// A width or height of zero, or above 2^31 - 1.
    BadDimensions,
}

impl PngProblem {
    /// A plain sentence fragment for messages.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Truncated => "the file ends inside the PNG header",
            Self::BadHeader => "the first chunk is not a valid PNG header",
            Self::BadChecksum => "the PNG header checksum does not match",
            Self::BadDimensions => "the PNG header holds an impossible width or height",
        }
    }
}

/// True when the bytes start with the PNG signature.
#[must_use]
pub fn has_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&PNG_SIGNATURE)
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    let mut word = [0u8; 4];
    word.copy_from_slice(slice);
    Some(u32::from_be_bytes(word))
}

/// The CRC-32 the PNG format uses (polynomial 0xEDB88320, over the chunk type and data).
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn depth_allowed(color_type: u8, depth: u8) -> bool {
    match color_type {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        _ => false,
    }
}

/// Reads the header of a PNG file.
///
/// # Errors
///
/// A [`PngProblem`] when the bytes carry the signature but no valid header. A file without the signature is
/// not a PNG at all; check [`has_signature`] first.
pub fn parse_png(bytes: &[u8]) -> Result<PngInfo, PngProblem> {
    // signature (8) + length (4) + type (4) + data (13) + crc (4)
    if bytes.len() < 33 {
        return Err(PngProblem::Truncated);
    }
    if be32(bytes, 8) != Some(13) || bytes.get(12..16) != Some(b"IHDR".as_slice()) {
        return Err(PngProblem::BadHeader);
    }
    let data = bytes.get(16..29).ok_or(PngProblem::Truncated)?;
    let stored = be32(bytes, 29).ok_or(PngProblem::Truncated)?;
    let checked = bytes.get(12..29).ok_or(PngProblem::Truncated)?;
    if crc32(checked) != stored {
        return Err(PngProblem::BadChecksum);
    }
    let width = be32(data, 0).ok_or(PngProblem::Truncated)?;
    let height = be32(data, 4).ok_or(PngProblem::Truncated)?;
    let (Some(&bit_depth), Some(&color_type), Some(&compression), Some(&filter), Some(&interlace)) = (
        data.get(8),
        data.get(9),
        data.get(10),
        data.get(11),
        data.get(12),
    ) else {
        return Err(PngProblem::Truncated);
    };
    if width == 0 || height == 0 || width > 0x7fff_ffff || height > 0x7fff_ffff {
        return Err(PngProblem::BadDimensions);
    }
    if !depth_allowed(color_type, bit_depth) || compression != 0 || filter != 0 || interlace > 1 {
        return Err(PngProblem::BadHeader);
    }
    Ok(PngInfo {
        width,
        height,
        bit_depth,
        color_type,
        complete: bytes.ends_with(&IEND),
    })
}

/// A minimal valid PNG of the given size, built without an image library: one filtered scanline per row of
/// zero bytes, stored (uncompressed) in a zlib stream. For tests and fixtures; the rows are 8 bit grey.
#[must_use]
pub fn build_png(width: u32, height: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut out = PNG_SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    // raw image data: per row a filter byte and `width` samples, stored in deflate blocks of at most 65535
    let row = 1 + usize::try_from(width).unwrap_or(0);
    let raw_len = row
        .saturating_mul(usize::try_from(height).unwrap_or(0))
        .min(1 << 20);
    let raw = vec![0u8; raw_len];
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        let last = u8::from(blocks.peek().is_none());
        let len = u16::try_from(block.len()).unwrap_or(0);
        z.push(last);
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn crc_of_the_check_string() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }

    #[test]
    fn a_built_png_parses() {
        let png = build_png(64, 32);
        assert!(has_signature(&png));
        let info = parse_png(&png).unwrap();
        assert_eq!((info.width, info.height), (64, 32));
        assert!(info.complete);
        assert_eq!(info.color_type, 0);
    }

    #[test]
    fn a_cut_off_file_is_incomplete_or_truncated() {
        let png = build_png(16, 16);
        assert_eq!(parse_png(&png[..20]), Err(PngProblem::Truncated));
        let info = parse_png(&png[..png.len() - 5]).unwrap();
        assert!(!info.complete);
    }

    #[test]
    fn a_damaged_header_is_refused() {
        let mut png = build_png(16, 16);
        png[17] ^= 0xff; // width changes, the checksum no longer matches
        assert_eq!(parse_png(&png), Err(PngProblem::BadChecksum));
        let mut png = build_png(16, 16);
        png[12] = b'X';
        assert_eq!(parse_png(&png), Err(PngProblem::BadHeader));
    }

    #[test]
    fn zero_and_huge_dimensions_are_refused() {
        assert_eq!(parse_png(&build_png(0, 4)), Err(PngProblem::BadDimensions));
        // a header that claims a gigantic image is parsed, not decoded
        let big = parse_png(&build_png(100_000, 100_000)).unwrap();
        assert_eq!(big.width, 100_000);
    }

    proptest! {
        #[test]
        fn the_parser_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let _ = parse_png(&bytes);
            let _ = has_signature(&bytes);
        }

        #[test]
        fn any_prefix_of_a_png_never_panics(cut in 0usize..120) {
            let png = build_png(8, 8);
            let end = cut.min(png.len());
            let _ = parse_png(&png[..end]);
        }
    }
}
