//! Guarded file copies for imported assets (textures and sound clips).
//!
//! The designer copies a file the user points at into the project. The copy is deliberately narrow:
//!
//! - the source must be a regular file; a link is never accepted (reading through it could leak a file the
//!   user did not mean to import) and so is nothing else, such as a folder or a device;
//! - the source is read whole under a byte limit and hashed with SHA-256 ([`crate::sha256`]); the caller
//!   passes the hash it planned with and a source that changed since is refused;
//! - the bytes go to the target through [`crate::atomic::atomic_write`] and the target is read back and
//!   compared with the hash, so a copy is either complete and verified or not there at all.
//!
//! Where the target lives (inside a project, outside protected folders) is the caller's business: the
//! toolkit validates the path with its guarded writer before it calls [`copy_verified`]. Backups of a replaced
//! target are made by the caller too.

use std::io::Read as _;

use camino::{Utf8Path, Utf8PathBuf};
use thiserror::Error;

use crate::atomic::atomic_write;
use crate::error::StoreError;
use crate::sha256::sha256_hex;

/// Why a copy or a read of a source file failed.
#[derive(Debug, Error)]
pub enum CopyError {
    /// The source does not exist.
    #[error("the file {path} does not exist")]
    SourceMissing {
        /// The path.
        path: Utf8PathBuf,
    },
    /// The source is a link, a folder or another kind of entry that is not a regular file.
    #[error("{path} is not accepted: {reason}")]
    SourceNotRegular {
        /// The path.
        path: Utf8PathBuf,
        /// What it is.
        reason: &'static str,
    },
    /// The source is larger than the limit.
    #[error("{path} is {bytes} bytes, more than the limit of {max} bytes")]
    SourceTooLarge {
        /// The path.
        path: Utf8PathBuf,
        /// The size found (at least).
        bytes: u64,
        /// The limit.
        max: u64,
    },
    /// The source holds other bytes than the plan was made from.
    #[error("{path} changed since the plan was made (expected {expected}, found {found})")]
    SourceChanged {
        /// The path.
        path: Utf8PathBuf,
        /// The hash the plan holds.
        expected: String,
        /// The hash of the bytes read now.
        found: String,
    },
    /// The target does not hold the copied bytes after the write.
    #[error("the copy to {path} could not be verified")]
    VerifyFailed {
        /// The target.
        path: Utf8PathBuf,
    },
    /// A file system call failed.
    #[error(transparent)]
    Io(#[from] StoreError),
}

impl CopyError {
    /// The stable code of the error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::SourceMissing { .. } => "copy.source-missing",
            Self::SourceNotRegular { .. } => "copy.source-not-regular",
            Self::SourceTooLarge { .. } => "copy.source-too-large",
            Self::SourceChanged { .. } => "copy.source-changed",
            Self::VerifyFailed { .. } => "copy.verify-failed",
            Self::Io(e) => e.code(),
        }
    }
}

/// The bytes and the hash of a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBytes {
    /// The whole content.
    pub bytes: Vec<u8>,
    /// SHA-256 of the content as 64 lower case hexadecimal characters.
    pub sha256: String,
}

/// What a copy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyReport {
    /// The target that was written.
    pub target: Utf8PathBuf,
    /// Bytes written.
    pub bytes: u64,
    /// SHA-256 of the written bytes.
    pub sha256: String,
}

/// Reads a regular file under a limit and hashes it.
///
/// A link is refused even when it points at a regular file. The size is checked from the metadata first and
/// again while reading (a file that grows while it is read is refused when it passes the limit).
///
/// # Errors
///
/// [`CopyError::SourceMissing`], [`CopyError::SourceNotRegular`], [`CopyError::SourceTooLarge`] or
/// [`CopyError::Io`].
pub fn read_source(path: &Utf8Path, max_bytes: u64) -> Result<SourceBytes, CopyError> {
    let meta = match std::fs::symlink_metadata(path.as_std_path()) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CopyError::SourceMissing {
                path: path.to_path_buf(),
            });
        }
        Err(e) => return Err(StoreError::io("stat", path, e).into()),
    };
    if meta.file_type().is_symlink() {
        return Err(CopyError::SourceNotRegular {
            path: path.to_path_buf(),
            reason: "it is a link; point at the file itself",
        });
    }
    if !meta.is_file() {
        return Err(CopyError::SourceNotRegular {
            path: path.to_path_buf(),
            reason: "it is not a regular file",
        });
    }
    if meta.len() > max_bytes {
        return Err(CopyError::SourceTooLarge {
            path: path.to_path_buf(),
            bytes: meta.len(),
            max: max_bytes,
        });
    }
    let file =
        std::fs::File::open(path.as_std_path()).map_err(|e| StoreError::io("open", path, e))?;
    // the entry may have been swapped for something else between the check and the open
    let opened = file
        .metadata()
        .map_err(|e| StoreError::io("stat", path, e))?;
    if !opened.is_file() {
        return Err(CopyError::SourceNotRegular {
            path: path.to_path_buf(),
            reason: "it is not a regular file",
        });
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| StoreError::io("read", path, e))?;
    let len = bytes.len() as u64;
    if len > max_bytes {
        return Err(CopyError::SourceTooLarge {
            path: path.to_path_buf(),
            bytes: len,
            max: max_bytes,
        });
    }
    let sha256 = sha256_hex(&bytes);
    Ok(SourceBytes { bytes, sha256 })
}

/// The size and the SHA-256 of an existing file, `None` when it does not exist. Used on the target of a
/// copy to decide between create, replace and unchanged.
///
/// # Errors
///
/// [`CopyError::SourceNotRegular`] when the entry is not a regular file, [`CopyError::SourceTooLarge`] above
/// the limit, [`CopyError::Io`] on a failed read.
pub fn hash_existing(path: &Utf8Path, max_bytes: u64) -> Result<Option<(u64, String)>, CopyError> {
    match read_source(path, max_bytes) {
        Ok(s) => Ok(Some((s.bytes.len() as u64, s.sha256))),
        Err(CopyError::SourceMissing { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Copies `source` to `target` atomically and verifies the result.
///
/// `expected_sha256` is the hash the plan was made from; a source that holds other bytes now is refused
/// before anything is written. Parent folders of the target are created.
///
/// # Errors
///
/// The errors of [`read_source`], [`CopyError::SourceChanged`], [`CopyError::VerifyFailed`] and
/// [`CopyError::Io`] for the write.
pub fn copy_verified(
    source: &Utf8Path,
    target: &Utf8Path,
    expected_sha256: &str,
    max_bytes: u64,
) -> Result<CopyReport, CopyError> {
    let read = read_source(source, max_bytes)?;
    if !read.sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(CopyError::SourceChanged {
            path: source.to_path_buf(),
            expected: expected_sha256.to_owned(),
            found: read.sha256,
        });
    }
    atomic_write(target, &read.bytes)?;
    let back =
        std::fs::read(target.as_std_path()).map_err(|e| StoreError::io("verify", target, e))?;
    if sha256_hex(&back) != read.sha256 {
        return Err(CopyError::VerifyFailed {
            path: target.to_path_buf(),
        });
    }
    Ok(CopyReport {
        target: target.to_path_buf(),
        bytes: read.bytes.len() as u64,
        sha256: read.sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().expect("temp dir");
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).expect("utf-8 temp path");
        (t, p)
    }

    #[test]
    fn copies_and_verifies() {
        let (_t, base) = temp();
        let src = base.join("a.bin");
        std::fs::write(src.as_std_path(), b"hello").expect("write");
        let hash = sha256_hex(b"hello");
        let dst = base.join("deep/er/b.bin");
        let r = copy_verified(&src, &dst, &hash, 100).expect("copy");
        assert_eq!(r.bytes, 5);
        assert_eq!(r.sha256, hash);
        assert_eq!(std::fs::read(dst.as_std_path()).expect("read"), b"hello");
    }

    #[test]
    fn a_changed_source_and_a_missing_one_are_refused() {
        let (_t, base) = temp();
        let src = base.join("a.bin");
        std::fs::write(src.as_std_path(), b"hello").expect("write");
        let dst = base.join("b.bin");
        let err = copy_verified(&src, &dst, &sha256_hex(b"other"), 100).expect_err("changed");
        assert_eq!(err.code(), "copy.source-changed");
        assert!(!dst.as_std_path().exists());
        let err = copy_verified(&base.join("none"), &dst, "x", 100).expect_err("missing");
        assert_eq!(err.code(), "copy.source-missing");
    }

    #[test]
    fn the_limit_and_non_files_are_refused() {
        let (_t, base) = temp();
        let src = base.join("a.bin");
        std::fs::write(src.as_std_path(), vec![0u8; 10]).expect("write");
        assert_eq!(
            read_source(&src, 9).expect_err("large").code(),
            "copy.source-too-large"
        );
        assert_eq!(
            read_source(&base, 100).expect_err("folder").code(),
            "copy.source-not-regular"
        );
        assert_eq!(read_source(&src, 10).expect("exact limit").bytes.len(), 10);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_a_source() {
        let (_t, base) = temp();
        let real = base.join("real.bin");
        std::fs::write(real.as_std_path(), b"x").expect("write");
        let link = base.join("link.bin");
        if std::os::unix::fs::symlink(real.as_std_path(), link.as_std_path()).is_err() {
            return;
        }
        assert_eq!(
            read_source(&link, 100).expect_err("link").code(),
            "copy.source-not-regular"
        );
    }

    #[test]
    fn hash_existing_reports_absence() {
        let (_t, base) = temp();
        assert_eq!(hash_existing(&base.join("none"), 10).expect("ok"), None);
        let f = base.join("f");
        std::fs::write(f.as_std_path(), b"abc").expect("write");
        let (len, hash) = hash_existing(&f, 10).expect("ok").expect("exists");
        assert_eq!(len, 3);
        assert_eq!(hash, sha256_hex(b"abc"));
    }
}
