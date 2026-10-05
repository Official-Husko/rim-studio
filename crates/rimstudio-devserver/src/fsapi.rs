//! The folder picker support of the bridge: `GET /dev/fs/list` and `GET /dev/fs/home`.
//!
//! The browser cannot read the disk, so the development UI browses folders through the bridge. The
//! listing is read only, takes absolute paths only (a path with `..` or a relative path is refused),
//! hides dotfiles, lists directories first and returns at most [`MAX_ENTRIES`] entries. A symbolic link
//! is listed with the kind of its target but is never looked into: it is reported as not a mod folder.

use std::fs;
use std::path::{Component, Path, PathBuf};

use rimstudio_app::AppContext;
use rimstudio_app::dispatch::dispatch;
use serde_json::{Value, json};

use crate::codes;
use crate::http::Reject;

/// The most entries one listing returns.
pub const MAX_ENTRIES: usize = 2000;
/// The most directory entries read before the listing is cut off, so a huge folder stays cheap.
const MAX_SCANNED: usize = 20_000;
/// The most entries looked at when searching a folder for `About`.
const MAX_ABOUT_SCAN: usize = 5000;
/// The longest path accepted.
const MAX_PATH_BYTES: usize = 4096;

/// One entry of a listing before it is turned into JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// File name.
    pub name: String,
    /// True for a directory (also a link to one).
    pub is_dir: bool,
    /// True for a symbolic link.
    pub symlink: bool,
    /// `About/About.xml` exists.
    pub is_mod_folder: bool,
    /// An `About` folder exists.
    pub has_about: bool,
}

/// The result of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The cleaned absolute path that was listed.
    pub path: String,
    /// Its parent, `None` for a root.
    pub parent: Option<String>,
    /// The entries, directories first, then by name without case.
    pub entries: Vec<Entry>,
    /// True when entries were left out because of a limit.
    pub truncated: bool,
}

impl Listing {
    /// The JSON of the response.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let entries: Vec<Value> = self
            .entries
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "kind": if e.is_dir { "dir" } else { "file" },
                    "isModFolder": e.is_mod_folder,
                    "hasAbout": e.has_about,
                    "symlink": e.symlink,
                })
            })
            .collect();
        json!({
            "path": self.path,
            "parent": self.parent,
            "entries": entries,
            "truncated": self.truncated,
        })
    }
}

/// Checks that a path is absolute and has no parent step, and returns it cleaned (no `.` parts, no
/// repeated separators).
///
/// # Errors
/// A 400 for an empty, relative, too long or `..` path or one with a NUL byte.
pub fn clean_absolute(path: &str) -> Result<PathBuf, Reject> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains('\0') {
        return Err(Reject::bad(
            "bad-path",
            "the path is empty, too long or invalid",
        ));
    }
    let given = Path::new(path);
    if !given.is_absolute() {
        return Err(Reject::bad("relative-path", "the path must be absolute"));
    }
    let mut clean = PathBuf::new();
    for part in given.components() {
        match part {
            Component::ParentDir => {
                return Err(Reject::bad("parent-step", "the path must not contain .."));
            }
            Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    Ok(clean)
}

fn io_reject(error: &std::io::Error) -> Reject {
    match error.kind() {
        std::io::ErrorKind::NotFound => Reject::new(
            404,
            codes::IO_NOT_FOUND,
            "not-found",
            "the folder does not exist",
        ),
        std::io::ErrorKind::PermissionDenied => Reject::new(
            403,
            codes::IO_PERMISSION_DENIED,
            "permission-denied",
            "the folder cannot be read",
        ),
        _ => Reject::new(
            500,
            codes::APP_INTERNAL_ERROR,
            "io",
            "the folder cannot be listed",
        ),
    }
}

/// The child of `dir` with this name (any case) that is a folder (or a file when `want_dir` is false).
fn find_child(dir: &Path, wanted: &str, want_dir: bool) -> Option<PathBuf> {
    let exact = dir.join(wanted);
    if fs::metadata(&exact).is_ok_and(|m| m.is_dir() == want_dir) {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .take(MAX_ABOUT_SCAN)
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
                && fs::metadata(entry.path()).is_ok_and(|m| m.is_dir() == want_dir)
        })
        .map(|entry| entry.path())
}

/// `(isModFolder, hasAbout)` of a folder: an `About` folder, and `About.xml` inside it, both found
/// without regard to case.
fn about_flags(dir: &Path) -> (bool, bool) {
    match find_child(dir, "About", true) {
        None => (false, false),
        Some(about) => (find_child(&about, "About.xml", false).is_some(), true),
    }
}

/// Lists a folder.
///
/// # Errors
/// A [`Reject`] for a bad path (400), a missing folder (404), a path that is not a folder (400) and a
/// folder that cannot be read (403).
pub fn list(path: &str, with_files: bool) -> Result<Listing, Reject> {
    let clean = clean_absolute(path)?;
    let meta = fs::metadata(&clean).map_err(|e| io_reject(&e))?;
    if !meta.is_dir() {
        return Err(Reject::new(
            400,
            codes::IO_NOT_A_DIRECTORY,
            "not-a-directory",
            "the path is not a folder",
        ));
    }
    let read = fs::read_dir(&clean).map_err(|e| io_reject(&e))?;
    let mut raw: Vec<(String, bool, bool)> = Vec::new();
    let mut truncated = false;
    for (seen, entry) in read.flatten().enumerate() {
        if seen >= MAX_SCANNED {
            truncated = true;
            break;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let symlink = file_type.is_symlink();
        let is_dir = if symlink {
            fs::metadata(entry.path()).is_ok_and(|m| m.is_dir())
        } else {
            file_type.is_dir()
        };
        if !is_dir && !with_files {
            continue;
        }
        raw.push((name, is_dir, symlink));
    }
    raw.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
            .then_with(|| a.0.cmp(&b.0))
    });
    if raw.len() > MAX_ENTRIES {
        raw.truncate(MAX_ENTRIES);
        truncated = true;
    }
    let entries = raw
        .into_iter()
        .map(|(name, is_dir, symlink)| {
            let (is_mod_folder, has_about) = if is_dir && !symlink {
                about_flags(&clean.join(&name))
            } else {
                (false, false)
            };
            Entry {
                name,
                is_dir,
                symlink,
                is_mod_folder,
                has_about,
            }
        })
        .collect();
    Ok(Listing {
        path: clean.to_string_lossy().into_owned(),
        parent: clean.parent().map(|p| p.to_string_lossy().into_owned()),
        entries,
        truncated,
    })
}

fn add_place(places: &mut Vec<(String, String)>, label: &str, path: &str) {
    if path.is_empty() || places.iter().any(|(_, p)| p == path) {
        return;
    }
    if Path::new(path).is_absolute() && fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        places.push((label.to_owned(), path.to_owned()));
    }
}

fn source_label(kind: &str, label: &str) -> String {
    let base = match kind {
        "game-data" => "RimWorld Data",
        "game-mods" => "RimWorld Mods",
        "workshop" => "Steam Workshop",
        _ => "Mod folder",
    };
    if kind == "custom" && !label.is_empty() {
        format!("Mod folder: {label}")
    } else {
        base.to_owned()
    }
}

/// `GET /dev/fs/home`: the home folder and the places the app knows about.
#[must_use]
pub fn home(app: &AppContext) -> Value {
    let home = app
        .platform
        .env
        .home_dir()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_default();
    let mut places: Vec<(String, String)> = Vec::new();
    add_place(&mut places, "Home", &home);
    if let Ok(report) = dispatch(app, "detect_get_report", json!({})) {
        let libraries = report
            .pointer("/report/libraries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for lib in &libraries {
            let path = lib.get("path").and_then(Value::as_str).unwrap_or("");
            let label = lib.get("label").and_then(Value::as_str).unwrap_or("");
            let name = if label.is_empty() {
                "Steam library".to_owned()
            } else {
                format!("Steam library: {label}")
            };
            add_place(&mut places, &name, path);
        }
    }
    if let Ok(list) = dispatch(app, "sources_list", json!({})) {
        let sources = list
            .get("sources")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for source in &sources {
            let kind = source.get("kind").and_then(Value::as_str).unwrap_or("");
            let label = source.get("label").and_then(Value::as_str).unwrap_or("");
            let path = source.get("path").and_then(Value::as_str).unwrap_or("");
            if kind == "game-data"
                && let Some(parent) = Path::new(path).parent()
            {
                add_place(&mut places, "RimWorld", &parent.to_string_lossy());
            }
            add_place(&mut places, &source_label(kind, label), path);
        }
    }
    if std::path::MAIN_SEPARATOR == '\\' {
        for letter in 'A'..='Z' {
            let root = format!("{letter}:\\");
            add_place(&mut places, &format!("Drive {letter}:"), &root);
        }
    } else {
        add_place(&mut places, "Computer", "/");
    }
    let places: Vec<Value> = places
        .into_iter()
        .map(|(label, path)| json!({"label": label, "path": path}))
        .collect();
    json!({"home": home, "places": places})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for d in [
            "Zeta",
            "alpha",
            "Beta/About",
            "gamma/about",
            "delta/About",
            ".hidden",
            "epsilon",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join("Beta/About/About.xml"), "x").unwrap();
        fs::write(root.join("gamma/about/ABOUT.XML"), "x").unwrap();
        fs::write(root.join("file.txt"), "x").unwrap();
        fs::write(root.join(".dotfile"), "x").unwrap();
        dir
    }

    fn names(l: &Listing) -> Vec<&str> {
        l.entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn directories_come_first_sorted_without_case_and_dotfiles_are_hidden() {
        let dir = tree();
        let l = list(dir.path().to_str().unwrap(), true).unwrap();
        assert_eq!(
            names(&l),
            [
                "alpha", "Beta", "delta", "epsilon", "gamma", "Zeta", "file.txt"
            ]
        );
        let l = list(dir.path().to_str().unwrap(), false).unwrap();
        assert_eq!(
            names(&l),
            ["alpha", "Beta", "delta", "epsilon", "gamma", "Zeta"]
        );
        assert!(!l.truncated);
    }

    #[test]
    fn mod_folders_are_found_without_regard_to_case() {
        let dir = tree();
        let l = list(dir.path().to_str().unwrap(), false).unwrap();
        let flags = |n: &str| {
            let e = l.entries.iter().find(|e| e.name == n).unwrap();
            (e.is_mod_folder, e.has_about)
        };
        assert_eq!(flags("Beta"), (true, true));
        assert_eq!(flags("gamma"), (true, true));
        assert_eq!(flags("delta"), (false, true));
        assert_eq!(flags("alpha"), (false, false));
    }

    #[test]
    fn the_json_has_the_documented_fields() {
        let dir = tree();
        let l = list(dir.path().to_str().unwrap(), false).unwrap();
        let v = l.to_json();
        assert_eq!(v["parent"], dir.path().parent().unwrap().to_str().unwrap());
        let first = &v["entries"][0];
        assert_eq!(first["name"], "alpha");
        assert_eq!(first["kind"], "dir");
        assert_eq!(first["isModFolder"], false);
        assert_eq!(first["hasAbout"], false);
    }

    #[test]
    fn relative_and_parent_paths_are_refused() {
        for bad in ["", "relative/dir", ".", "../x", "/tmp/../etc", "/a\0b"] {
            let e = list(bad, false).unwrap_err();
            assert_eq!(e.status, 400, "{bad:?}");
        }
    }

    #[test]
    fn dots_and_repeated_separators_are_cleaned() {
        let dir = tree();
        let messy = format!("{}//./alpha/", dir.path().display());
        let l = list(&messy, false).unwrap();
        assert_eq!(l.path, dir.path().join("alpha").to_str().unwrap());
    }

    #[test]
    fn missing_and_non_folder_paths_have_their_own_status() {
        let dir = tree();
        let missing = dir.path().join("nope");
        assert_eq!(
            list(missing.to_str().unwrap(), false).unwrap_err().status,
            404
        );
        let file = dir.path().join("file.txt");
        let e = list(file.to_str().unwrap(), false).unwrap_err();
        assert_eq!((e.status, e.code), (400, "io.not-a-directory"));
    }

    #[test]
    fn a_long_listing_is_cut_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(MAX_ENTRIES + 5) {
            fs::create_dir(dir.path().join(format!("d{i:05}"))).unwrap();
        }
        let l = list(dir.path().to_str().unwrap(), false).unwrap();
        assert_eq!(l.entries.len(), MAX_ENTRIES);
        assert!(l.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_listed_but_not_looked_into() {
        let dir = tree();
        std::os::unix::fs::symlink(dir.path().join("Beta"), dir.path().join("link")).unwrap();
        let l = list(dir.path().to_str().unwrap(), false).unwrap();
        let link = l.entries.iter().find(|e| e.name == "link").unwrap();
        assert!(link.is_dir && link.symlink);
        assert!(!link.is_mod_folder && !link.has_about);
    }

    #[test]
    fn a_root_has_no_parent() {
        let root = if std::path::MAIN_SEPARATOR == '\\' {
            "C:\\"
        } else {
            "/"
        };
        if let Ok(l) = list(root, false) {
            assert_eq!(l.parent, None);
        }
    }
}
