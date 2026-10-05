//! A guarded move of a file or a folder inside the roots of a [`RootGuard`].
//!
//! [`move_path`] is the one way the app moves existing content around inside a mod folder. It never
//! overwrites: a destination that exists (also one that differs only by letter case) is refused, and
//! nothing has been changed when it is. Both paths are checked by the guard (inside the roots, no `..`,
//! no link that leaves them) before the move and the destination again after its parent folders exist.
//! A link is never moved or followed: a source that is a link is refused, and a folder that holds a link
//! is refused by the copy fallback.
//!
//! The move is a rename. A file is first hard linked to its new name (which fails when the name exists, so
//! two writers cannot overwrite each other) and the old name is removed; where the file system has no
//! hard links the plain rename is used after the existence check. When the rename crosses a volume
//! boundary the content is copied, read back and compared byte for byte, and only then is the source
//! removed; a failed copy removes the partial destination and leaves the source alone.
//!
//! A change of letter case only (`defs` to `Defs`) is allowed for the same entry: it is how a folder name
//! is corrected on every platform.

use std::io::ErrorKind;

use camino::Utf8Path;

use crate::error::{IoError, StoreError};
use crate::guard::RootGuard;

/// What a move did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveOutcome {
    /// True when the content was copied, verified and the source removed (the move crossed a volume
    /// boundary), false for a plain rename.
    pub copied: bool,
}

fn blocked(path: &Utf8Path, reason: impl Into<String>) -> IoError {
    StoreError::WriteBlocked {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
    .into()
}

fn io_err(op: &'static str, path: &Utf8Path, e: std::io::Error) -> IoError {
    StoreError::io(op, path, e).into()
}

/// Moves `from` to `to` (a file or a folder), both inside the guard's roots, without overwriting.
///
/// # Errors
///
/// [`GuardError`] when a path is outside the roots; [`StoreError::WriteBlocked`] when the source does not
/// exist, is a link, or the destination exists; [`StoreError::Io`] for an operating system failure. The
/// source is untouched on every error except a failure to remove it after a verified copy (then both
/// copies exist and the error says so).
pub fn move_path(
    guard: &RootGuard,
    from: &Utf8Path,
    to: &Utf8Path,
) -> Result<MoveOutcome, IoError> {
    move_path_with(guard, from, to, &|a, b| fs_err::rename(a, b))
}

/// [`move_path`] with the rename step injected, so a test can make it fail like a cross volume rename.
///
/// # Errors
///
/// See [`move_path`].
pub fn move_path_with(
    guard: &RootGuard,
    from: &Utf8Path,
    to: &Utf8Path,
    rename: &dyn Fn(&Utf8Path, &Utf8Path) -> std::io::Result<()>,
) -> Result<MoveOutcome, IoError> {
    let from_checked = guard.validate(from)?.path;
    let to_checked = guard.validate(to)?.path;
    let meta = match fs_err::symlink_metadata(&from_checked) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(blocked(&from_checked, "the source does not exist"));
        }
        Err(e) => return Err(io_err("move-source", &from_checked, e)),
    };
    if meta.file_type().is_symlink() {
        return Err(blocked(&from_checked, "the source is a link"));
    }
    if to_checked.starts_with(&from_checked) && to_checked != from_checked {
        return Err(blocked(&to_checked, "a folder cannot be moved into itself"));
    }
    if to_checked == from_checked {
        return Err(blocked(
            &to_checked,
            "the source and the destination are the same",
        ));
    }
    let case_only = is_case_only_rename(&from_checked, &to_checked);
    check_destination_free(&to_checked, case_only)?;
    if let Some(parent) = to_checked.parent() {
        fs_err::create_dir_all(parent).map_err(|e| io_err("create-dir", parent, e))?;
    }
    // a link planted while the parent folders were made is seen here
    let again = guard.validate(to)?.path;
    if again != to_checked {
        return Err(blocked(
            &again,
            "the destination changed while it was prepared",
        ));
    }
    check_destination_free(&to_checked, case_only)?;
    if !meta.is_dir() && !case_only {
        match fs_err::hard_link(&from_checked, &to_checked) {
            Ok(()) => {
                if let Err(e) = fs_err::remove_file(&from_checked) {
                    let _ = fs_err::remove_file(&to_checked);
                    return Err(io_err("move-remove-source", &from_checked, e));
                }
                return Ok(MoveOutcome { copied: false });
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                return Err(blocked(&to_checked, "the destination exists"));
            }
            // no hard links here (some file systems): the plain rename below
            Err(_) => {}
        }
    }
    match rename(&from_checked, &to_checked) {
        Ok(()) => Ok(MoveOutcome { copied: false }),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            copy_verify_remove(&from_checked, &to_checked)?;
            Ok(MoveOutcome { copied: true })
        }
        Err(e) => Err(io_err("move", &from_checked, e)),
    }
}

/// Removes one regular file inside the guard's roots. A link or a folder is refused.
///
/// # Errors
///
/// [`GuardError`](crate::error::GuardError) outside the roots; [`StoreError::WriteBlocked`] for a link or a
/// folder; [`StoreError::Io`] otherwise.
pub fn remove_file(guard: &RootGuard, path: &Utf8Path) -> Result<(), IoError> {
    let checked = guard.validate(path)?.path;
    let meta = fs_err::symlink_metadata(&checked).map_err(|e| io_err("remove", &checked, e))?;
    if !meta.is_file() {
        return Err(blocked(&checked, "not a regular file"));
    }
    fs_err::remove_file(&checked).map_err(|e| io_err("remove", &checked, e))
}

/// Removes a folder inside the guard's roots when it is empty. `Ok(false)` means it holds something and
/// was left alone. A link or a file is refused.
///
/// # Errors
///
/// As [`remove_file`].
pub fn remove_empty_dir(guard: &RootGuard, path: &Utf8Path) -> Result<bool, IoError> {
    let checked = guard.validate(path)?.path;
    let meta = fs_err::symlink_metadata(&checked).map_err(|e| io_err("remove", &checked, e))?;
    if !meta.is_dir() {
        return Err(blocked(&checked, "not a folder"));
    }
    let empty = fs_err::read_dir(&checked)
        .map_err(|e| io_err("read-dir", &checked, e))?
        .next()
        .is_none();
    if !empty {
        return Ok(false);
    }
    fs_err::remove_dir(&checked).map_err(|e| io_err("remove", &checked, e))?;
    Ok(true)
}

fn is_case_only_rename(from: &Utf8Path, to: &Utf8Path) -> bool {
    from.parent() == to.parent()
        && from.file_name() != to.file_name()
        && from
            .file_name()
            .zip(to.file_name())
            .is_some_and(|(a, b)| a.to_lowercase() == b.to_lowercase())
}

/// The destination must not exist. For a case only rename the entry of the source itself is not a
/// conflict on a volume that ignores letter case, so the folder listing is used instead of a lookup.
fn check_destination_free(to: &Utf8Path, case_only: bool) -> Result<(), IoError> {
    if case_only {
        let (Some(parent), Some(name)) = (to.parent(), to.file_name()) else {
            return Err(blocked(to, "the destination has no name"));
        };
        let listing = fs_err::read_dir(parent).map_err(|e| io_err("read-dir", parent, e))?;
        for entry in listing.flatten() {
            if entry.file_name().to_str() == Some(name) {
                return Err(blocked(to, "the destination exists"));
            }
        }
        return Ok(());
    }
    match fs_err::symlink_metadata(to) {
        Ok(_) => Err(blocked(to, "the destination exists")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err("move-destination", to, e)),
    }
}

/// Copies `from` to `to`, compares every file with its copy and removes `from`. Used when a rename
/// crosses a volume boundary; a failure before the removal deletes the partial copy and leaves `from`
/// as it was.
///
/// # Errors
///
/// [`StoreError::Io`] and [`StoreError::WriteBlocked`] (a link inside a folder, a copy that differs).
pub fn copy_verify_remove(from: &Utf8Path, to: &Utf8Path) -> Result<(), IoError> {
    if let Err(e) = copy_tree(from, to) {
        remove_any(to);
        return Err(e);
    }
    if let Err(e) = verify_tree(from, to) {
        remove_any(to);
        return Err(e);
    }
    let removed = if fs_err::symlink_metadata(from).is_ok_and(|m| m.is_dir()) {
        fs_err::remove_dir_all(from)
    } else {
        fs_err::remove_file(from)
    };
    removed.map_err(|e| {
        io_err(
            "move-remove-source",
            from,
            std::io::Error::new(
                e.kind(),
                format!("the copy at {to} is complete but the original could not be removed: {e}"),
            ),
        )
    })
}

fn remove_any(path: &Utf8Path) {
    match fs_err::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            let _ = fs_err::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs_err::remove_file(path);
        }
        Err(_) => {}
    }
}

fn children(dir: &Utf8Path) -> Result<Vec<(String, std::fs::FileType)>, IoError> {
    let mut out = Vec::new();
    let listing = fs_err::read_dir(dir).map_err(|e| io_err("read-dir", dir, e))?;
    for entry in listing {
        let entry = entry.map_err(|e| io_err("read-dir", dir, e))?;
        let kind = entry.file_type().map_err(|e| io_err("file-type", dir, e))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| blocked(dir, "a name in the folder is not UTF-8"))?;
        out.push((name, kind));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn copy_tree(from: &Utf8Path, to: &Utf8Path) -> Result<(), IoError> {
    let meta = fs_err::symlink_metadata(from).map_err(|e| io_err("copy", from, e))?;
    if meta.file_type().is_symlink() {
        return Err(blocked(from, "a link inside the moved content"));
    }
    if meta.is_dir() {
        fs_err::create_dir(to).map_err(|e| io_err("copy", to, e))?;
        for (name, _) in children(from)? {
            copy_tree(&from.join(&name), &to.join(&name))?;
        }
        return Ok(());
    }
    fs_err::copy(from, to).map_err(|e| io_err("copy", to, e))?;
    Ok(())
}

fn verify_tree(from: &Utf8Path, to: &Utf8Path) -> Result<(), IoError> {
    let meta = fs_err::symlink_metadata(from).map_err(|e| io_err("verify", from, e))?;
    if meta.is_dir() {
        let a = children(from)?;
        let b = children(to)?;
        let names = |v: &[(String, std::fs::FileType)]| -> Vec<String> {
            v.iter().map(|(n, _)| n.clone()).collect()
        };
        if names(&a) != names(&b) {
            return Err(blocked(to, "the copy does not hold the same entries"));
        }
        for (name, _) in a {
            verify_tree(&from.join(&name), &to.join(&name))?;
        }
        return Ok(());
    }
    let x = fs_err::read(from).map_err(|e| io_err("verify", from, e))?;
    let y = fs_err::read(to).map_err(|e| io_err("verify", to, e))?;
    if x == y {
        Ok(())
    } else {
        Err(IoError::Store(StoreError::WriteVerifyFailed {
            path: to.to_path_buf(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    fn temp() -> (tempfile::TempDir, Utf8PathBuf, RootGuard) {
        let t = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(t.path().canonicalize().unwrap()).unwrap();
        let guard = RootGuard::new([root.clone()]).unwrap();
        (t, root, guard)
    }

    fn put(path: &Utf8Path, text: &str) {
        fs_err::create_dir_all(path.parent().unwrap()).unwrap();
        fs_err::write(path, text).unwrap();
    }

    #[test]
    fn a_file_moves_and_the_parents_are_created() {
        let (_t, root, guard) = temp();
        put(&root.join("a/x.xml"), "one");
        let out = move_path(&guard, &root.join("a/x.xml"), &root.join("b/c/x.xml")).unwrap();
        assert!(!out.copied);
        assert!(!root.join("a/x.xml").exists());
        assert_eq!(
            fs_err::read_to_string(root.join("b/c/x.xml")).unwrap(),
            "one"
        );
    }

    #[test]
    fn a_folder_moves_with_its_content() {
        let (_t, root, guard) = temp();
        put(&root.join("CE/Patches/p.xml"), "p");
        move_path(
            &guard,
            &root.join("CE"),
            &root.join("Compat/CombatExtended"),
        )
        .unwrap();
        assert_eq!(
            fs_err::read_to_string(root.join("Compat/CombatExtended/Patches/p.xml")).unwrap(),
            "p"
        );
        assert!(!root.join("CE").exists());
    }

    #[test]
    fn an_existing_destination_is_never_overwritten() {
        let (_t, root, guard) = temp();
        put(&root.join("a.xml"), "mine");
        put(&root.join("b.xml"), "theirs");
        let e = move_path(&guard, &root.join("a.xml"), &root.join("b.xml")).unwrap_err();
        assert_eq!(e.code(), "store.write-blocked");
        assert_eq!(fs_err::read_to_string(root.join("a.xml")).unwrap(), "mine");
        assert_eq!(
            fs_err::read_to_string(root.join("b.xml")).unwrap(),
            "theirs"
        );
        put(&root.join("d1/f"), "1");
        fs_err::create_dir_all(root.join("d2")).unwrap();
        assert!(move_path(&guard, &root.join("d1"), &root.join("d2")).is_err());
        assert!(root.join("d1/f").exists());
    }

    #[test]
    fn removal_takes_files_and_empty_folders_only() {
        let (_t, root, guard) = temp();
        put(&root.join("a/x.txt"), "x");
        assert!(remove_file(&guard, &root.join("a")).is_err());
        assert!(!remove_empty_dir(&guard, &root.join("a")).unwrap());
        remove_file(&guard, &root.join("a/x.txt")).unwrap();
        assert!(remove_empty_dir(&guard, &root.join("a")).unwrap());
        assert!(!root.join("a").exists());
        assert!(remove_file(&guard, &root.join("a/x.txt")).is_err());
    }

    #[test]
    fn a_missing_source_and_a_move_into_itself_are_refused() {
        let (_t, root, guard) = temp();
        assert!(move_path(&guard, &root.join("none"), &root.join("x")).is_err());
        put(&root.join("d/f"), "1");
        assert!(move_path(&guard, &root.join("d"), &root.join("d/e")).is_err());
        assert!(move_path(&guard, &root.join("d"), &root.join("d")).is_err());
    }

    #[test]
    fn paths_outside_the_roots_are_refused() {
        let (_t, root, guard) = temp();
        put(&root.join("a.xml"), "x");
        let other = tempfile::tempdir().unwrap();
        let outside = Utf8PathBuf::from_path_buf(other.path().canonicalize().unwrap()).unwrap();
        assert!(move_path(&guard, &root.join("a.xml"), &outside.join("a.xml")).is_err());
        assert!(move_path(&guard, &outside, &root.join("o")).is_err());
        assert!(root.join("a.xml").exists());
    }

    #[test]
    fn a_letter_case_change_renames_the_same_entry() {
        let (_t, root, guard) = temp();
        put(&root.join("defs/a.xml"), "a");
        move_path(&guard, &root.join("defs"), &root.join("Defs")).unwrap();
        let names: Vec<String> = fs_err::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["Defs"]);
        assert!(root.join("Defs/a.xml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_case_twin_destination_is_refused() {
        let (_t, root, guard) = temp();
        put(&root.join("defs/a.xml"), "a");
        put(&root.join("Defs/b.xml"), "b");
        assert!(move_path(&guard, &root.join("defs"), &root.join("Defs")).is_err());
        assert!(root.join("defs/a.xml").exists() && root.join("Defs/b.xml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_moved_or_followed() {
        let (_t, root, guard) = temp();
        let other = tempfile::tempdir().unwrap();
        let outside = Utf8PathBuf::from_path_buf(other.path().canonicalize().unwrap()).unwrap();
        put(&outside.join("secret.txt"), "s");
        std::os::unix::fs::symlink(outside.as_std_path(), root.join("link").as_std_path()).unwrap();
        assert!(move_path(&guard, &root.join("link"), &root.join("moved")).is_err());
        put(&root.join("a.xml"), "x");
        assert!(move_path(&guard, &root.join("a.xml"), &root.join("link/a.xml")).is_err());
        assert!(!outside.join("a.xml").exists());
    }

    fn cross_volume(_: &Utf8Path, _: &Utf8Path) -> std::io::Result<()> {
        Err(std::io::Error::from(ErrorKind::CrossesDevices))
    }

    #[test]
    fn a_cross_volume_move_copies_verifies_and_removes() {
        let (_t, root, guard) = temp();
        put(&root.join("d/x/1.txt"), "one");
        put(&root.join("d/2.txt"), "two");
        // the hard link step does not apply to a folder, so the injected rename is used
        let out = move_path_with(&guard, &root.join("d"), &root.join("e"), &cross_volume).unwrap();
        assert!(out.copied);
        assert!(!root.join("d").exists());
        assert_eq!(
            fs_err::read_to_string(root.join("e/x/1.txt")).unwrap(),
            "one"
        );
        assert_eq!(fs_err::read_to_string(root.join("e/2.txt")).unwrap(), "two");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_copy_leaves_the_source_and_no_partial_destination() {
        let (_t, root, guard) = temp();
        put(&root.join("d/1.txt"), "one");
        std::os::unix::fs::symlink("1.txt", root.join("d/link").as_std_path()).unwrap();
        let e = move_path_with(&guard, &root.join("d"), &root.join("e"), &cross_volume);
        assert!(e.is_err());
        assert!(root.join("d/1.txt").exists());
        assert!(!root.join("e").exists());
    }
}
