//! Removing one regular file.
//!
//! The only removal the rest of the workspace uses: a caller that has already checked the path (the
//! guarded project writer) and made its backup asks for the file to go. A missing file is not an error, a
//! folder or a link is refused, and nothing is removed recursively.

use camino::Utf8Path;

use crate::error::StoreError;

/// Removes the regular file at `path`. Returns `true` when a file was removed and `false` when there was
/// none.
///
/// # Errors
/// [`StoreError::Io`] when the entry is a folder or a link (kind `InvalidInput`) or the removal fails.
pub fn remove_regular_file(path: &Utf8Path) -> Result<bool, StoreError> {
    let meta = match fs_err::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(StoreError::io("remove", path, e)),
    };
    if !meta.file_type().is_file() {
        return Err(StoreError::io(
            "remove",
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "not a regular file (a folder or a link is never removed)",
            ),
        ));
    }
    fs_err::remove_file(path).map_err(|e| StoreError::io("remove", path, e))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_goes_a_missing_file_is_fine_and_a_folder_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(remove_regular_file(&file).unwrap());
        assert!(!file.exists());
        assert!(!remove_regular_file(&file).unwrap());
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(remove_regular_file(&sub).is_err());
        assert!(sub.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_refused_and_its_target_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        let target = dir.join("t.txt");
        std::fs::write(&target, b"x").unwrap();
        let link = dir.join("l.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(remove_regular_file(&link).is_err());
        assert!(target.exists() && link.exists());
    }
}
