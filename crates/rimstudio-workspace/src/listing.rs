//! A [`Listing`] over the real file system.
//!
//! The pure load plan code in `rimstudio-core` asks a [`Listing`] which files a folder holds. This
//! module is the real implementation used by the reference set and the session. It never writes.

use std::fs;
use std::path::Path;

use camino::Utf8Path;
use rimstudio_core::load_plan::Listing;

/// The deepest folder nesting a listing follows (a guard against link loops).
const MAX_DEPTH: usize = 32;

/// The real file system as a [`Listing`]: links to folders and files are followed, hidden names are
/// returned (the load plan skips them), unreadable folders are treated as empty.
#[derive(Debug, Default, Clone, Copy)]
pub struct DiskListing;

impl DiskListing {
    /// The names of the immediate sub folders of `dir`, sorted. Empty when `dir` is unreadable.
    #[must_use]
    pub fn child_dirs(dir: &Utf8Path) -> Vec<String> {
        let Ok(read) = fs::read_dir(dir.as_std_path()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = read
            .flatten()
            .filter(|e| is_dir(&e.path()))
            .filter_map(|e| e.file_name().to_str().map(str::to_owned))
            .collect();
        names.sort();
        names
    }
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_dir())
}

fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(m) if m.is_dir() => walk(&path, &rel, depth + 1, out),
            Ok(m) if m.is_file() => out.push(rel),
            _ => {}
        }
    }
}

impl Listing for DiskListing {
    fn list_files(&self, dir: &Utf8Path) -> Vec<String> {
        let mut out = Vec::new();
        walk(dir.as_std_path(), "", 0, &mut out);
        out
    }

    fn dir_exists(&self, dir: &Utf8Path) -> bool {
        is_dir(dir.as_std_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn lists_nested_files_with_forward_slashes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        fs::create_dir_all(root.join("Defs/Sub")).unwrap();
        fs::write(root.join("Defs/a.xml"), "x").unwrap();
        fs::write(root.join("Defs/Sub/b.xml"), "x").unwrap();
        let mut files = DiskListing.list_files(&root.join("Defs"));
        files.sort();
        assert_eq!(files, vec!["Sub/b.xml".to_owned(), "a.xml".to_owned()]);
        assert!(DiskListing.dir_exists(&root.join("Defs")));
        assert!(!DiskListing.dir_exists(&root.join("Nope")));
        assert!(DiskListing.list_files(&root.join("Nope")).is_empty());
        assert_eq!(DiskListing::child_dirs(&root), vec!["Defs".to_owned()]);
    }
}
