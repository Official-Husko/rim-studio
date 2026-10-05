//! Path validation against allowed roots.
//!
//! [`RootGuard`] answers one question: is this path inside one of the allowed roots, after
//! normalisation, without traversal and without a link that leaves the roots? The steps follow the
//! security design: reject empty text and NUL, reject relative paths, reject `..` components, clean
//! `.`, canonicalise the longest existing prefix (resolving links), and compare by path components,
//! never by string prefix. A path whose text lies inside a root but whose resolved location does not
//! is reported as a link escape. Case folding is not applied: the guard compares exactly, which is
//! the safe direction on case insensitive volumes (it can refuse, never wrongly allow).
//!
//! The module also provides [`sanitize_name`] for single path components (file names, document
//! names) and [`resolve_path`], the shared normaliser used by the write fence.

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

use crate::error::GuardError;

fn truncated(text: &str) -> String {
    text.chars().take(200).collect()
}

const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn is_reserved_stem(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let lower = stem.to_lowercase();
    if WINDOWS_RESERVED.contains(&lower.as_str()) || matches!(lower.as_str(), "conin$" | "conout$")
    {
        return true;
    }
    // Windows also treats COM and LPT followed by a superscript digit as device names.
    ["com", "lpt"].iter().any(|prefix| {
        lower
            .strip_prefix(prefix)
            .is_some_and(|rest| matches!(rest, "\u{b9}" | "\u{b2}" | "\u{b3}"))
    })
}

/// Deny lists (protected folders) compare with letter case folded on every platform, so a case only
/// difference cannot slip past them on Windows and macOS volumes. The cost is a refusal on a case
/// sensitive disk that holds two folders differing only by case, which is the safe direction. Allow
/// lists (roots) compare exactly, which can only refuse more.
pub const FOLD_CASE_PATHS: bool = true;

/// Component wise `starts_with`, optionally ignoring letter case (Unicode lower casing per
/// component). Never a string prefix comparison.
#[must_use]
pub fn path_starts_with(path: &Utf8Path, base: &Utf8Path, fold_case: bool) -> bool {
    if !fold_case {
        return path.starts_with(base);
    }
    let mut own = path.components();
    base.components().all(|b| {
        own.next()
            .is_some_and(|c| c.as_str().to_lowercase() == b.as_str().to_lowercase())
    })
}

/// Validates a single path component such as a file or folder name and returns it. Rejects empty
/// names, `.` and `..`, separators, NUL and control characters, `:`, `<>"|?*`, trailing dots or
/// spaces, Windows reserved device names, and names over 255 bytes.
pub fn sanitize_name(name: &str) -> Result<String, GuardError> {
    let bad = |reason: &'static str| GuardError::Invalid {
        path: truncated(name),
        reason,
    };
    if name.is_empty() {
        return Err(bad("empty name"));
    }
    if name.len() > 255 {
        return Err(bad("name too long"));
    }
    if name == "." || name == ".." {
        return Err(bad("dot name"));
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
    }) {
        return Err(bad("forbidden character"));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(bad("trailing dot or space"));
    }
    if is_reserved_stem(name) {
        return Err(bad("reserved device name"));
    }
    Ok(name.to_owned())
}

/// A lenient variant of [`sanitize_name`] for names typed by a person: forbidden characters become
/// `_`, trailing dots and spaces are trimmed, reserved device names get a leading `_`, and the result is
/// capped at 120 characters and 200 bytes. An empty result becomes `_`.
pub fn to_safe_component(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut capped = String::new();
    for c in mapped.chars().take(120) {
        if capped.len() + c.len_utf8() > 200 {
            break;
        }
        capped.push(c);
    }
    let mut s: String = capped
        .trim_end_matches(['.', ' '])
        .trim_start_matches(' ')
        .to_owned();
    if s.is_empty() || s == "." || s == ".." {
        s = "_".to_owned();
    }
    if is_reserved_stem(&s) {
        s.insert(0, '_');
    }
    s
}

/// Cleans `.` components and rejects `..`, NUL and relative paths. Purely lexical.
pub fn lexical_clean(path: &Utf8Path) -> Result<Utf8PathBuf, GuardError> {
    let text = path.as_str();
    if text.is_empty() {
        return Err(GuardError::Invalid {
            path: String::new(),
            reason: "empty path",
        });
    }
    if text.contains('\0') {
        return Err(GuardError::Invalid {
            path: truncated(text),
            reason: "contains NUL",
        });
    }
    if !path.is_absolute() {
        return Err(GuardError::Relative {
            path: truncated(text),
        });
    }
    let mut out = Utf8PathBuf::new();
    for c in path.components() {
        match c {
            Utf8Component::ParentDir => {
                return Err(GuardError::Traversal {
                    path: truncated(text),
                });
            }
            Utf8Component::CurDir => {}
            other => out.push(other.as_str()),
        }
    }
    Ok(out)
}

/// Normalises `path` and resolves links in its longest existing prefix. The result is absolute and
/// has no `.` or `..` components. A dangling link in the existing part is reported as a link escape
/// because its destination cannot be checked.
pub fn resolve_path(path: &Utf8Path) -> Result<Utf8PathBuf, GuardError> {
    let clean = lexical_clean(path)?;
    let mut existing = clean.clone();
    let mut tail: Vec<String> = Vec::new();
    loop {
        match dunce::canonicalize(existing.as_std_path()) {
            Ok(canon) => {
                let mut out =
                    Utf8PathBuf::from_path_buf(canon).map_err(|_| GuardError::Invalid {
                        path: truncated(path.as_str()),
                        reason: "resolved path is not UTF-8",
                    })?;
                for seg in tail.iter().rev() {
                    out.push(seg);
                }
                return Ok(out);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if fs_err::symlink_metadata(&existing).is_ok() {
                    return Err(GuardError::LinkEscape {
                        path: truncated(path.as_str()),
                    });
                }
                match (existing.file_name(), existing.parent()) {
                    (Some(name), Some(parent)) => {
                        tail.push(name.to_owned());
                        existing = parent.to_path_buf();
                    }
                    _ => return Ok(clean),
                }
            }
            Err(e) => {
                return Err(GuardError::Io {
                    op: "canonicalize",
                    path: truncated(existing.as_str()),
                    source: e,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RootEntry {
    lexical: Utf8PathBuf,
    canonical: Utf8PathBuf,
}

/// A set of allowed roots.
#[derive(Debug, Clone, Default)]
pub struct RootGuard {
    roots: Vec<RootEntry>,
}

/// A path that passed [`RootGuard::validate`], with the index of the root that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPath {
    /// The canonical path (links resolved in the existing part).
    pub path: Utf8PathBuf,
    /// The index of the root in the order the guard was built.
    pub root_index: usize,
}

impl RootGuard {
    /// Builds a guard. Every root must be absolute; roots that do not exist yet are accepted and
    /// compared by their normalised text.
    pub fn new<I>(roots: I) -> Result<Self, GuardError>
    where
        I: IntoIterator,
        I::Item: AsRef<Utf8Path>,
    {
        let mut out = Vec::new();
        for r in roots {
            let lexical = lexical_clean(r.as_ref())?;
            let canonical = resolve_path(&lexical)?;
            out.push(RootEntry { lexical, canonical });
        }
        Ok(RootGuard { roots: out })
    }

    /// The number of roots.
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// True when the guard allows nothing.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Checks `path` and returns it unchanged when it is inside an allowed root.
    pub fn check<'a>(&self, path: &'a Utf8Path) -> Result<&'a Utf8Path, GuardError> {
        self.validate(path).map(|_| path)
    }

    /// Checks `path` and returns the canonical form with the root index.
    pub fn validate(&self, path: &Utf8Path) -> Result<ValidatedPath, GuardError> {
        let lexical = lexical_clean(path)?;
        let resolved = resolve_path(&lexical)?;
        if let Some(i) = self
            .roots
            .iter()
            .position(|r| resolved.starts_with(&r.canonical))
        {
            return Ok(ValidatedPath {
                path: resolved,
                root_index: i,
            });
        }
        if self.roots.iter().any(|r| lexical.starts_with(&r.lexical)) {
            return Err(GuardError::LinkEscape {
                path: truncated(path.as_str()),
            });
        }
        Err(GuardError::OutsideRoots {
            path: truncated(path.as_str()),
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn utf8(p: &std::path::Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()
    }

    #[rstest]
    #[case("RS_Rifle.json", true)]
    #[case("my mod", true)]
    #[case("", false)]
    #[case(".", false)]
    #[case("..", false)]
    #[case("a/b", false)]
    #[case("a\\b", false)]
    #[case("nul", false)]
    #[case("COM1.txt", false)]
    #[case("name.", false)]
    #[case("name ", false)]
    #[case("a:b", false)]
    #[case("a\0b", false)]
    fn sanitize_name_table(#[case] name: &str, #[case] ok: bool) {
        assert_eq!(sanitize_name(name).is_ok(), ok, "{name:?}");
    }

    #[rstest]
    #[case("CON")]
    #[case("con.xml")]
    #[case("Nul.tar.gz")]
    #[case("com9")]
    #[case("LPT1.txt")]
    #[case("COM\u{b9}")]
    #[case("lpt\u{b3}.dat")]
    #[case("CONIN$")]
    #[case("con .txt")]
    fn windows_device_names_are_refused_with_any_extension(#[case] name: &str) {
        assert!(sanitize_name(name).is_err(), "{name:?}");
        assert!(sanitize_name(&to_safe_component(name)).is_ok(), "{name:?}");
    }

    #[test]
    fn names_near_the_limit_are_checked_in_bytes() {
        assert!(sanitize_name(&"a".repeat(255)).is_ok());
        assert!(sanitize_name(&"a".repeat(256)).is_err());
        assert!(sanitize_name(&"\u{e9}".repeat(128)).is_err());
    }

    #[test]
    fn folded_prefix_comparison_works_per_component() {
        let base = Utf8Path::new("/Games/Steam/steamapps");
        assert!(path_starts_with(
            Utf8Path::new("/games/STEAM/SteamApps/common/x"),
            base,
            true
        ));
        assert!(!path_starts_with(
            Utf8Path::new("/games/STEAM/SteamApps/common/x"),
            base,
            false
        ));
        assert!(!path_starts_with(
            Utf8Path::new("/games/steam/steamapps-evil/x"),
            base,
            true
        ));
        assert!(!path_starts_with(Utf8Path::new("/games/steam"), base, true));
    }

    #[test]
    fn lenient_names_always_validate() {
        for raw in ["a/b", "..", "", "con", "x.", "  ", "q?*:", &"z".repeat(500)] {
            let s = to_safe_component(raw);
            assert!(sanitize_name(&s).is_ok(), "{raw:?} -> {s:?}");
        }
    }

    #[test]
    fn traversal_relative_and_nul_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path());
        let g = RootGuard::new([&root]).unwrap();
        assert_eq!(
            g.check(&root.join("a/../b")).unwrap_err().code(),
            "guard.traversal"
        );
        assert_eq!(
            g.check(Utf8Path::new("rel/x")).unwrap_err().code(),
            "guard.relative-path"
        );
        assert_eq!(
            g.check(Utf8Path::new("")).unwrap_err().code(),
            "guard.invalid-path"
        );
        assert_eq!(
            g.check(Utf8Path::new("/tmp/a\0b")).unwrap_err().code(),
            "guard.invalid-path"
        );
    }

    #[test]
    fn inside_and_outside_use_component_comparison() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path()).join("proj");
        fs_err::create_dir_all(&root).unwrap();
        let sibling = utf8(t.path()).join("proj-evil");
        fs_err::create_dir_all(&sibling).unwrap();
        let g = RootGuard::new([&root]).unwrap();
        assert!(g.check(&root.join("sub/new.xml")).is_ok());
        assert!(g.check(&root).is_ok());
        assert_eq!(
            g.check(&sibling).unwrap_err().code(),
            "io.path-outside-roots"
        );
        assert_eq!(
            g.check(&utf8(t.path())).unwrap_err().code(),
            "io.path-outside-roots"
        );
    }

    #[test]
    fn dot_components_are_cleaned() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path());
        let g = RootGuard::new([&root]).unwrap();
        let v = g
            .validate(&Utf8PathBuf::from(format!("{root}/./a/./b")))
            .unwrap();
        assert!(v.path.ends_with("a/b"));
        assert_eq!(v.root_index, 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_detected() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path()).join("root");
        let outside = utf8(t.path()).join("outside");
        fs_err::create_dir_all(&root).unwrap();
        fs_err::create_dir_all(&outside).unwrap();
        let link = root.join("link");
        if std::os::unix::fs::symlink(&outside, &link).is_err() {
            return;
        }
        let g = RootGuard::new([&root]).unwrap();
        let err = g.check(&link.join("file.txt")).unwrap_err();
        assert_eq!(err.code(), "guard.link-escape");
        // A link that stays inside is fine.
        fs_err::create_dir_all(root.join("real")).unwrap();
        let inner = root.join("alias");
        std::os::unix::fs::symlink(root.join("real"), &inner).unwrap();
        assert!(g.check(&inner.join("x")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_link_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path());
        let link = root.join("dangling");
        if std::os::unix::fs::symlink(root.join("nowhere"), &link).is_err() {
            return;
        }
        let g = RootGuard::new([&root]).unwrap();
        assert_eq!(g.check(&link).unwrap_err().code(), "guard.link-escape");
    }

    #[test]
    fn traversal_strings_never_validate_outside() {
        let t = tempfile::tempdir().unwrap();
        let root = utf8(t.path()).join("r");
        fs_err::create_dir_all(&root).unwrap();
        let g = RootGuard::new([&root]).unwrap();
        for tail in [
            "../x",
            "a/../../x",
            "..",
            "a/b/../../../etc",
            "./../x",
            "a//../../..",
        ] {
            let p = Utf8PathBuf::from(format!("{root}/{tail}"));
            assert!(g.check(&p).is_err(), "{p}");
        }
    }
}
