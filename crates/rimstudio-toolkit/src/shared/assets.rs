//! Reading the source files of imported assets (textures and sound clips), without decoding them.
//!
//! The designer lets a person point at a PNG or at WAV and OGG clips on their machine. This module turns a
//! path into the facts the planner works from ([`AssetFacts`]): it resolves the path, refuses a link or a
//! non regular file, reads the file under a size limit, hashes it with SHA-256 and recognises the format by
//! its signature and header ([`rimstudio_design::assets::detect`]). Nothing is decoded and nothing is
//! written. The webview never sees a file: a thumbnail for a small PNG travels as a data URL built here.
//!
//! The same module holds the small in house base64 encoder of that data URL.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_design::assets::{
    AssetFacts, AssetInfo, MAX_CLIP_BYTES, MAX_TEXTURE_BYTES, PREVIEW_MAX_BYTES, SourceState,
    detect,
};
use rimstudio_design::model::DesignSpec;
use rimstudio_io::copy::{CopyError, read_source};

/// What an import is for; it decides the size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetRole {
    /// A texture (PNG).
    Texture,
    /// A sound clip (WAV or Ogg).
    Clip,
}

impl AssetRole {
    /// The largest file of this role the designer reads and copies.
    #[must_use]
    pub fn max_bytes(self) -> u64 {
        match self {
            Self::Texture => MAX_TEXTURE_BYTES,
            Self::Clip => MAX_CLIP_BYTES,
        }
    }
}

/// Resolves a source path as written in a spec: an absolute path is used as it is, a relative path is read
/// against the project root.
///
/// # Errors
///
/// A plain sentence when the path is empty, holds a control character, or is relative and cannot be
/// resolved (no project root given, or the path climbs out with `..`).
pub fn resolve_source(path: &str, root: Option<&Utf8Path>) -> Result<Utf8PathBuf, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("the path is empty".to_owned());
    }
    if path.chars().any(char::is_control) {
        return Err("the path holds a control character".to_owned());
    }
    let candidate = Utf8Path::new(path);
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    let Some(root) = root else {
        return Err("a relative path needs an open project".to_owned());
    };
    if !rimstudio_design::plan::is_safe_relative_path(&path.replace('\\', "/")) {
        return Err("a relative path may not climb out of the project".to_owned());
    }
    Ok(root.join(path))
}

/// Reads a source file and describes it. A path that cannot be resolved, a link, a folder and an unreadable
/// file are [`SourceState::Refused`]; a missing file is [`SourceState::Missing`]; a file above the limit of
/// its role is [`SourceState::TooLarge`] and is not read.
#[must_use]
pub fn probe(path: &str, root: Option<&Utf8Path>, role: AssetRole) -> SourceState {
    if path.trim().is_empty() {
        return SourceState::Missing;
    }
    let resolved = match resolve_source(path, root) {
        Ok(p) => p,
        Err(reason) => return SourceState::Refused(reason),
    };
    match read_source(&resolved, role.max_bytes()) {
        Ok(read) => SourceState::Read(AssetInfo {
            bytes: read.bytes.len() as u64,
            sha256: read.sha256,
            detected: detect(&read.bytes),
        }),
        Err(CopyError::SourceMissing { .. }) => SourceState::Missing,
        Err(CopyError::SourceTooLarge { bytes, .. }) => SourceState::TooLarge(bytes),
        Err(CopyError::SourceNotRegular { reason, .. }) => SourceState::Refused(reason.to_owned()),
        Err(e) => SourceState::Refused(e.to_string()),
    }
}

/// The source paths a spec imports with the role of each, in a stable order and without repeats.
#[must_use]
pub fn sources_of(spec: &DesignSpec) -> Vec<(String, AssetRole)> {
    let mut out: Vec<(String, AssetRole)> = Vec::new();
    let mut add = |path: &str, role: AssetRole| {
        let path = path.trim().to_owned();
        if !out.iter().any(|(p, _)| *p == path) {
            out.push((path, role));
        }
    };
    if let Some(p) = &spec.assets.texture {
        add(p, AssetRole::Texture);
    }
    if let Some(p) = &spec.assets.projectile_texture {
        add(p, AssetRole::Texture);
    }
    if let Some(sound) = &spec.sounds.shot {
        for clip in &sound.clips {
            add(clip, AssetRole::Clip);
        }
    }
    out
}

/// Reads every source file a spec imports. A spec without imports reads nothing.
#[must_use]
pub fn facts_of(spec: &DesignSpec, root: &Utf8Path) -> AssetFacts {
    let mut facts = AssetFacts::new();
    for (path, role) in sources_of(spec) {
        let state = probe(&path, Some(root), role);
        facts.insert(path, state);
    }
    facts
}

/// Standard base64 (RFC 4648, with padding).
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk.first().copied().unwrap_or(0));
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        let pick = |shift: u32| char::from(TABLE[((n >> shift) & 0x3f) as usize]);
        out.push(pick(18));
        out.push(pick(12));
        out.push(if chunk.len() > 1 { pick(6) } else { '=' });
        out.push(if chunk.len() > 2 { pick(0) } else { '=' });
    }
    out
}

/// A `data:image/png;base64,...` URL for a PNG file read from `path`, when the file is a valid complete PNG
/// of at most [`PREVIEW_MAX_BYTES`]. The file is read again here; this is only for the small preview of
/// `designer_asset_info`.
#[must_use]
pub fn png_preview(path: &Utf8Path) -> Option<String> {
    let read = read_source(path, PREVIEW_MAX_BYTES).ok()?;
    match detect(&read.bytes) {
        rimstudio_design::assets::Detected::Png(p) if p.complete => Some(format!(
            "data:image/png;base64,{}",
            base64_encode(&read.bytes)
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::assets::png::build_png;

    fn temp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        (t, p)
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), want);
        }
    }

    #[test]
    fn paths_resolve_against_the_project_only_when_relative_and_safe() {
        let root = Utf8Path::new("/proj");
        assert_eq!(
            resolve_source("/a/b.png", None).unwrap(),
            Utf8PathBuf::from("/a/b.png")
        );
        assert_eq!(
            resolve_source("Source/Art/a.png", Some(root)).unwrap(),
            Utf8PathBuf::from("/proj/Source/Art/a.png")
        );
        assert!(resolve_source("a.png", None).is_err());
        assert!(resolve_source("../a.png", Some(root)).is_err());
        assert!(resolve_source("  ", Some(root)).is_err());
        assert!(resolve_source("a\u{0}b", Some(root)).is_err());
    }

    #[test]
    fn probing_reads_hashes_and_detects() {
        let (_t, base) = temp();
        let file = base.join("a.png");
        let png = build_png(32, 16);
        std::fs::write(file.as_std_path(), &png).unwrap();
        let SourceState::Read(info) = probe(file.as_str(), None, AssetRole::Texture) else {
            panic!("not read");
        };
        assert_eq!(info.bytes, png.len() as u64);
        assert_eq!(info.dimensions(), Some((32, 16)));
        assert_eq!(info.sha256, rimstudio_io::sha256::sha256_hex(&png));
        assert_eq!(
            probe(base.join("none.png").as_str(), None, AssetRole::Texture),
            SourceState::Missing
        );
        assert!(matches!(
            probe(base.as_str(), None, AssetRole::Texture),
            SourceState::Refused(_)
        ));
        assert_eq!(probe("", None, AssetRole::Clip), SourceState::Missing);
    }

    #[test]
    fn a_file_over_the_limit_is_not_read() {
        let (_t, base) = temp();
        let file = base.join("big.png");
        let f = std::fs::File::create(file.as_std_path()).unwrap();
        f.set_len(MAX_TEXTURE_BYTES + 1).unwrap();
        assert_eq!(
            probe(file.as_str(), None, AssetRole::Texture),
            SourceState::TooLarge(MAX_TEXTURE_BYTES + 1)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_refused_even_to_a_good_file() {
        let (_t, base) = temp();
        let real = base.join("real.png");
        std::fs::write(real.as_std_path(), build_png(8, 8)).unwrap();
        let link = base.join("link.png");
        if std::os::unix::fs::symlink(real.as_std_path(), link.as_std_path()).is_err() {
            return;
        }
        assert!(matches!(
            probe(link.as_str(), None, AssetRole::Texture),
            SourceState::Refused(_)
        ));
    }

    #[test]
    fn a_small_png_gets_a_preview_and_a_large_or_odd_file_does_not() {
        let (_t, base) = temp();
        let file = base.join("a.png");
        std::fs::write(file.as_std_path(), build_png(8, 8)).unwrap();
        let url = png_preview(&file).unwrap();
        assert!(url.starts_with("data:image/png;base64,iVBORw0KGgo"));
        let text = base.join("t.png");
        std::fs::write(text.as_std_path(), b"not a png").unwrap();
        assert_eq!(png_preview(&text), None);
        assert_eq!(png_preview(&base.join("none.png")), None);
    }

    #[test]
    fn sources_are_listed_once_with_their_role() {
        let mut spec = DesignSpec::new_ranged("RS_A", "a");
        spec.assets.texture = Some(" /a.png ".into());
        spec.sounds.shot = Some(rimstudio_design::model::CustomSound {
            clips: vec!["/c.wav".into(), "/c.wav".into(), "/a.png".into()],
            ..Default::default()
        });
        let sources = sources_of(&spec);
        assert_eq!(
            sources,
            vec![
                ("/a.png".to_owned(), AssetRole::Texture),
                ("/c.wav".to_owned(), AssetRole::Clip)
            ]
        );
    }
}
