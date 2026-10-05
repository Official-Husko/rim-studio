//! Atomic file replacement and crash injection.
//!
//! [`atomic_write`] replaces a file in one step through the `atomic-write-file` crate (temporary
//! file in the same directory, fsync, rename, directory fsync). [`atomic_write_with`] runs the same
//! algorithm over the [`FsOps`] trait so tests can fail any step and assert that the target holds
//! either the old or the new bytes. A leftover temporary file is harmless and removed by
//! [`cleanup_stale_temps`].
//!
//! When a rename over an existing file is refused (some network and removable file systems), the
//! writer falls back to renaming the target away, renaming the new file in and deleting the old
//! one, restoring the old file if the second rename fails, and logs the fallback.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::StoreError;

/// Marker inside the names of temporary files so they can be recognised and cleaned up.
pub const TEMP_MARKER: &str = ".rstmp-";

/// The file system steps used by [`atomic_write_with`].
pub trait FsOps: Send + Sync {
    /// Creates a directory and its parents.
    fn create_dir_all(&self, dir: &Utf8Path) -> std::io::Result<()>;
    /// Creates a new file (failing when it exists) and writes all bytes.
    fn write_temp(&self, tmp: &Utf8Path, bytes: &[u8]) -> std::io::Result<()>;
    /// Flushes a file to stable storage.
    fn sync_file(&self, path: &Utf8Path) -> std::io::Result<()>;
    /// Renames `from` over `to`.
    fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> std::io::Result<()>;
    /// Flushes a directory entry to stable storage.
    fn sync_dir(&self, dir: &Utf8Path) -> std::io::Result<()>;
    /// Removes a file.
    fn remove_file(&self, path: &Utf8Path) -> std::io::Result<()>;
}

/// The real file system. With `durable` false the fsync steps are skipped (tests and bulk loads).
#[derive(Debug, Clone, Copy)]
pub struct RealOps {
    /// Whether to fsync files and directories.
    pub durable: bool,
}

impl Default for RealOps {
    fn default() -> Self {
        RealOps { durable: true }
    }
}

impl FsOps for RealOps {
    fn create_dir_all(&self, dir: &Utf8Path) -> std::io::Result<()> {
        fs_err::create_dir_all(dir)
    }

    fn write_temp(&self, tmp: &Utf8Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut f = fs_err::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(tmp)?;
        f.write_all(bytes)?;
        f.flush()
    }

    fn sync_file(&self, path: &Utf8Path) -> std::io::Result<()> {
        if !self.durable {
            return Ok(());
        }
        fs_err::File::open(path)?.sync_all()
    }

    fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> std::io::Result<()> {
        fs_err::rename(from, to)
    }

    fn sync_dir(&self, dir: &Utf8Path) -> std::io::Result<()> {
        if !self.durable {
            return Ok(());
        }
        fs_err::File::open(dir)?.sync_all()
    }

    fn remove_file(&self, path: &Utf8Path) -> std::io::Result<()> {
        fs_err::remove_file(path)
    }
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_path_for(path: &Utf8Path) -> Utf8PathBuf {
    let name = path.file_name().unwrap_or("file");
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".{name}{TEMP_MARKER}{}-{seq}", std::process::id());
    match path.parent() {
        Some(dir) if !dir.as_str().is_empty() => dir.join(tmp_name),
        _ => Utf8PathBuf::from(tmp_name),
    }
}

fn dir_of(path: &Utf8Path) -> &Utf8Path {
    match path.parent() {
        Some(d) if !d.as_str().is_empty() => d,
        _ => Utf8Path::new("."),
    }
}

/// Replaces `path` with `bytes` atomically and durably. Parent folders are created.
pub fn atomic_write(path: &Utf8Path, bytes: &[u8]) -> Result<(), StoreError> {
    fs_err::create_dir_all(dir_of(path))
        .map_err(|e| StoreError::io("create-dir", dir_of(path), e))?;
    let mut file = atomic_write_file::AtomicWriteFile::open(path.as_std_path())
        .map_err(|e| StoreError::io("atomic-open", path, e))?;
    if let Err(e) = file.write_all(bytes) {
        let _ = file.discard();
        return Err(StoreError::io("atomic-write", path, e));
    }
    file.commit()
        .map_err(|e| StoreError::io("atomic-commit", path, e))
}

/// Creates a folder and its missing parents. Succeeds when the folder already exists. Callers outside
/// this crate create folders only through this function (I-05).
pub fn create_dir_all(path: &Utf8Path) -> Result<(), StoreError> {
    fs_err::create_dir_all(path).map_err(|e| StoreError::io("create-dir", path, e))
}

/// The same algorithm as [`atomic_write`] over an [`FsOps`], used for crash injection tests and for
/// bulk writes without fsync. Steps: create folder, write temp, sync temp, rename, sync folder.
pub fn atomic_write_with(ops: &dyn FsOps, path: &Utf8Path, bytes: &[u8]) -> Result<(), StoreError> {
    let dir = dir_of(path);
    ops.create_dir_all(dir)
        .map_err(|e| StoreError::io("create-dir", dir, e))?;
    let tmp = temp_path_for(path);
    if let Err(e) = ops.write_temp(&tmp, bytes) {
        let _ = ops.remove_file(&tmp);
        return Err(StoreError::io("write-temp", &tmp, e));
    }
    if let Err(e) = ops.sync_file(&tmp) {
        let _ = ops.remove_file(&tmp);
        return Err(StoreError::io("sync-temp", &tmp, e));
    }
    if let Err(e) = ops.rename(&tmp, path) {
        if let Err(e2) = rename_via_aside(ops, &tmp, path) {
            let _ = ops.remove_file(&tmp);
            tracing::warn!(path = %path, first = %e, second = %e2, "atomic rename failed");
            return Err(StoreError::io("rename", path, e2));
        }
        tracing::warn!(path = %path, error = %e, "rename over target refused, used the aside fallback");
    }
    if let Err(e) = ops.sync_dir(dir) {
        tracing::debug!(dir = %dir, error = %e, "directory sync failed after rename");
    }
    Ok(())
}

fn rename_via_aside(ops: &dyn FsOps, tmp: &Utf8Path, path: &Utf8Path) -> std::io::Result<()> {
    let aside = Utf8PathBuf::from(format!("{tmp}.old"));
    ops.rename(path, &aside)?;
    match ops.rename(tmp, path) {
        Ok(()) => {
            let _ = ops.remove_file(&aside);
            Ok(())
        }
        Err(e) => {
            let _ = ops.rename(&aside, path);
            Err(e)
        }
    }
}

/// The name of the file a temporary or aside file belongs to: `.doc.json.rstmp-1-2.old` gives
/// `doc.json`.
fn target_name_of(temp_name: &str) -> Option<&str> {
    let rest = temp_name.strip_prefix('.')?;
    let at = rest.find(TEMP_MARKER)?;
    rest.get(..at).filter(|n| !n.is_empty())
}

/// Removes temporary files left by an interrupted write in `dir` (not recursive) that are older
/// than `min_age`. Returns how many were removed.
///
/// A leftover is only deleted when the file it belongs to exists. The aside fallback of
/// [`atomic_write_with`] has a short window in which the target is missing and the old and new
/// bytes live only in the aside and temporary files; a crash there is repaired here by rolling
/// forward (the temporary file, which was synced before the rename, becomes the target) or, when
/// only the aside file is left, by moving the old bytes back.
pub fn cleanup_stale_temps(dir: &Utf8Path, min_age: Duration) -> usize {
    let Ok(rd) = fs_err::read_dir(dir) else {
        return 0;
    };
    let now = SystemTime::now();
    let mut removed = 0;
    for entry in rd.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let is_temp = name.starts_with('.') && (name.contains(TEMP_MARKER));
        if !is_temp {
            continue;
        }
        let old_enough = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age >= min_age);
        if !old_enough {
            continue;
        }
        let path = dir.join(name);
        let target = target_name_of(name).map(|t| dir.join(t));
        let target_missing = target.as_ref().is_some_and(|t| {
            fs_err::symlink_metadata(t).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        });
        if target_missing && let Some(target) = &target {
            if let Some(temp_name) = name.strip_suffix(".old") {
                // the aside file: the temporary file, when it is still there, is the one to keep
                if dir.join(temp_name).exists() {
                    continue;
                }
                if fs_err::rename(&path, target).is_ok() {
                    tracing::warn!(target = %target, "restored a file from its aside copy");
                }
                continue;
            }
            if fs_err::rename(&path, target).is_ok() {
                tracing::warn!(target = %target, "completed an interrupted write");
                let aside = dir.join(format!("{name}.old"));
                if aside.exists() && fs_err::remove_file(&aside).is_ok() {
                    removed += 1;
                }
            }
            continue;
        }
        if fs_err::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    /// Fails at the n-th call (0 based) and every call after it, like a process that died.
    struct CrashAt {
        inner: RealOps,
        at: usize,
        calls: AtomicUsize,
    }

    impl CrashAt {
        fn new(at: usize) -> Self {
            CrashAt {
                inner: RealOps { durable: false },
                at,
                calls: AtomicUsize::new(0),
            }
        }
        fn gate(&self) -> std::io::Result<()> {
            if self.calls.fetch_add(1, Ordering::SeqCst) >= self.at {
                Err(std::io::Error::other("injected crash"))
            } else {
                Ok(())
            }
        }
    }

    impl FsOps for CrashAt {
        fn create_dir_all(&self, d: &Utf8Path) -> std::io::Result<()> {
            self.gate()?;
            self.inner.create_dir_all(d)
        }
        fn write_temp(&self, t: &Utf8Path, b: &[u8]) -> std::io::Result<()> {
            self.gate()?;
            self.inner.write_temp(t, b)
        }
        fn sync_file(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.gate()?;
            self.inner.sync_file(p)
        }
        fn rename(&self, a: &Utf8Path, b: &Utf8Path) -> std::io::Result<()> {
            self.gate()?;
            self.inner.rename(a, b)
        }
        fn sync_dir(&self, d: &Utf8Path) -> std::io::Result<()> {
            self.gate()?;
            self.inner.sync_dir(d)
        }
        fn remove_file(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.gate()?;
            self.inner.remove_file(p)
        }
    }

    fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("doc.json")).unwrap();
        (t, p)
    }

    #[test]
    fn atomic_write_creates_and_replaces() {
        let (_t, p) = tmp();
        atomic_write(&p, b"one").unwrap();
        assert_eq!(fs_err::read(&p).unwrap(), b"one");
        atomic_write(&p, b"two").unwrap();
        assert_eq!(fs_err::read(&p).unwrap(), b"two");
    }

    #[test]
    fn atomic_write_creates_parent_folders() {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("a/b/c.json")).unwrap();
        atomic_write(&p, b"x").unwrap();
        assert_eq!(fs_err::read(&p).unwrap(), b"x");
    }

    #[test]
    fn crash_at_every_step_leaves_old_or_new_bytes() {
        // Steps of a clean run: create_dir, write_temp, sync_file, rename, sync_dir.
        for at in 0..6 {
            let (t, p) = tmp();
            atomic_write(&p, b"old").unwrap();
            let ops = CrashAt::new(at);
            let result = atomic_write_with(&ops, &p, b"new");
            let now = fs_err::read(&p).unwrap();
            assert!(
                now == b"old" || now == b"new",
                "step {at}: target holds {now:?}"
            );
            if at <= 3 {
                assert!(result.is_err(), "step {at} should fail");
                assert_eq!(now, b"old", "step {at}: target must be untouched");
            } else {
                // The directory sync is best effort; the data is already in place.
                assert!(result.is_ok(), "step {at}: {result:?}");
                assert_eq!(now, b"new");
            }
            let dir = Utf8Path::from_path(t.path()).unwrap();
            cleanup_stale_temps(dir, Duration::ZERO);
            let leftovers: Vec<_> = fs_err::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().contains(TEMP_MARKER))
                .collect();
            assert!(leftovers.is_empty(), "step {at}: temp files remain");
        }
    }

    #[test]
    fn crash_before_first_write_keeps_missing_target_missing() {
        let (_t, p) = tmp();
        let ops = CrashAt::new(2);
        assert!(atomic_write_with(&ops, &p, b"new").is_err());
        assert!(!p.exists());
    }

    /// Refuses rename over an existing target, allowing the aside fallback.
    struct NoRenameOver(RealOps);

    impl FsOps for NoRenameOver {
        fn create_dir_all(&self, d: &Utf8Path) -> std::io::Result<()> {
            self.0.create_dir_all(d)
        }
        fn write_temp(&self, t: &Utf8Path, b: &[u8]) -> std::io::Result<()> {
            self.0.write_temp(t, b)
        }
        fn sync_file(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.0.sync_file(p)
        }
        fn rename(&self, a: &Utf8Path, b: &Utf8Path) -> std::io::Result<()> {
            if b.exists() {
                return Err(std::io::Error::other("rename over existing refused"));
            }
            self.0.rename(a, b)
        }
        fn sync_dir(&self, d: &Utf8Path) -> std::io::Result<()> {
            self.0.sync_dir(d)
        }
        fn remove_file(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.0.remove_file(p)
        }
    }

    #[test]
    fn rename_over_refused_uses_the_aside_fallback() {
        let (t, p) = tmp();
        fs_err::write(&p, b"old").unwrap();
        atomic_write_with(&NoRenameOver(RealOps { durable: false }), &p, b"new").unwrap();
        assert_eq!(fs_err::read(&p).unwrap(), b"new");
        let names: Vec<String> = fs_err::read_dir(t.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["doc.json"], "no temp or aside file may remain");
    }

    #[test]
    fn a_crash_inside_the_aside_fallback_is_rolled_forward_and_never_loses_both_copies() {
        let (t, p) = tmp();
        let dir = Utf8Path::from_path(t.path()).unwrap();
        // the target was moved aside, the new bytes are in the temporary file, nothing is at the target
        fs_err::write(dir.join(".doc.json.rstmp-7-1"), b"new").unwrap();
        fs_err::write(dir.join(".doc.json.rstmp-7-1.old"), b"old").unwrap();
        cleanup_stale_temps(dir, Duration::ZERO);
        assert_eq!(fs_err::read(&p).unwrap(), b"new");
        let names: Vec<String> = fs_err::read_dir(t.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["doc.json"]);
    }

    #[test]
    fn an_aside_file_alone_is_moved_back_to_the_missing_target() {
        let (t, p) = tmp();
        let dir = Utf8Path::from_path(t.path()).unwrap();
        fs_err::write(dir.join(".doc.json.rstmp-7-2.old"), b"old").unwrap();
        cleanup_stale_temps(dir, Duration::ZERO);
        assert_eq!(fs_err::read(&p).unwrap(), b"old");
        assert_eq!(fs_err::read_dir(t.path()).unwrap().count(), 1);
    }

    #[test]
    fn leftovers_are_deleted_when_their_target_exists() {
        let (t, p) = tmp();
        let dir = Utf8Path::from_path(t.path()).unwrap();
        fs_err::write(&p, b"keep").unwrap();
        fs_err::write(dir.join(".doc.json.rstmp-7-3"), b"half").unwrap();
        fs_err::write(dir.join(".doc.json.rstmp-7-3.old"), b"older").unwrap();
        assert_eq!(cleanup_stale_temps(dir, Duration::ZERO), 2);
        assert_eq!(fs_err::read(&p).unwrap(), b"keep");
        assert_eq!(fs_err::read_dir(t.path()).unwrap().count(), 1);
    }

    #[test]
    fn cleanup_respects_min_age_and_only_touches_temps() {
        let (t, p) = tmp();
        let dir = Utf8Path::from_path(t.path()).unwrap();
        fs_err::write(dir.join(".doc.json.rstmp-1-1"), b"x").unwrap();
        fs_err::write(&p, b"keep").unwrap();
        assert_eq!(cleanup_stale_temps(dir, Duration::from_secs(3600)), 0);
        assert_eq!(cleanup_stale_temps(dir, Duration::ZERO), 1);
        assert!(p.exists());
    }
}
