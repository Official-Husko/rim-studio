//! Guarded binary copies: the way an imported texture or sound clip gets into a project.
//!
//! A copy goes through the same checks as a text write ([`GuardedWriter::path_of`]: a safe relative path,
//! inside the project root, no link out of it, outside the protected folders, no case twin, no link as the
//! target, not read only) and adds the rules of an import: the source is a regular file (never a link) read
//! under a size limit, it still holds the bytes the plan was made from (SHA-256), a different file already at
//! the target is backed up outside the project first, the bytes go down atomically and the target is read
//! back and compared with the hash. Nothing outside the project root is ever written.

use camino::Utf8Path;
use rimstudio_io::atomic::create_dir_all;
use rimstudio_io::backup::rotate;
use rimstudio_io::copy::{CopyError, copy_verified, hash_existing};
use rimstudio_io::error::StoreError;

use super::GuardedWriter;
use crate::error::{ToolkitError, ToolkitResult};

/// The largest file already at a copy target that is compared by hash (64 MiB).
pub(super) const MAX_EXISTING_BYTES: u64 = 64 * 1024 * 1024;

/// What one copy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyReport {
    /// The path relative to the project root, with `/` separators.
    pub rel: String,
    /// Bytes at the target after the copy.
    pub bytes: u64,
    /// SHA-256 of the copied bytes.
    pub sha256: String,
    /// True when a file with other content was replaced.
    pub replaced: bool,
    /// The backup of the replaced file, when one was made.
    pub backup: Option<camino::Utf8PathBuf>,
    /// True when the target was read back and holds the source's hash.
    pub verified: bool,
}

/// A copy error as a toolkit error: a source that is gone, changed, not a regular file or too large is a
/// refusal of the plan; an I/O failure keeps its store error.
fn refusal(e: CopyError) -> ToolkitError {
    match e {
        CopyError::Io(s) => ToolkitError::Store(s),
        CopyError::VerifyFailed { path } => {
            ToolkitError::Store(StoreError::WriteVerifyFailed { path })
        }
        other => ToolkitError::ApplyRefused {
            reason: format!("{other}; plan again"),
        },
    }
}

impl GuardedWriter {
    /// The size and the SHA-256 of the file at a project path, `None` when there is none.
    ///
    /// # Errors
    ///
    /// A path refusal, or [`ToolkitError::ApplyRefused`] when the entry is not a regular file or is larger
    /// than [`MAX_EXISTING_BYTES`].
    pub fn existing_file_hash(&self, rel: &str) -> ToolkitResult<Option<(u64, String)>> {
        let io = self.checked(rel)?.io;
        hash_existing(&io, MAX_EXISTING_BYTES).map_err(refusal)
    }

    fn check_existing_hash(
        &self,
        rel: &str,
        io: &Utf8Path,
        expected: Option<&str>,
    ) -> ToolkitResult<()> {
        let now = hash_existing(io, MAX_EXISTING_BYTES).map_err(refusal)?;
        let same = match (&now, expected) {
            (None, None) => true,
            (Some((_, hash)), Some(want)) => hash == want,
            _ => false,
        };
        if same {
            Ok(())
        } else {
            Err(ToolkitError::ApplyRefused {
                reason: format!("{rel} changed on disk since the plan was made; plan again"),
            })
        }
    }

    /// Checks everything [`GuardedWriter::copy_in`] checks before it writes, without writing: the path, the
    /// read only flag and that the target still holds the file the plan saw (`expected_existing` is its
    /// SHA-256, `None` means there must be no file).
    ///
    /// # Errors
    ///
    /// A path refusal, or [`ToolkitError::ApplyRefused`].
    pub fn preflight_copy(&self, rel: &str, expected_existing: Option<&str>) -> ToolkitResult<()> {
        let checked = self.checked(rel)?;
        self.check_writable(rel, &checked.io)?;
        self.check_existing_hash(rel, &checked.io, expected_existing)
    }

    /// Copies `source` (an absolute path of a regular file) to the project path `rel`.
    ///
    /// `expected_sha256` is the hash of the source the plan was made from; `expected_existing` the hash of
    /// the file the plan saw at the target (`None` for none). A target that already holds the source's bytes
    /// is left alone. Otherwise a replaced file is backed up first when `backup` is true.
    ///
    /// # Errors
    ///
    /// A path refusal, [`ToolkitError::ApplyRefused`] for a read only target, a target or a source that
    /// changed since the plan, a source that is not a regular file, is missing or too large, and
    /// [`ToolkitError::Store`] for a failed backup, write or verification.
    pub fn copy_in(
        &self,
        rel: &str,
        source: &Utf8Path,
        expected_sha256: &str,
        max_bytes: u64,
        backup: bool,
        expected_existing: Option<&str>,
    ) -> ToolkitResult<CopyReport> {
        let first = self.checked(rel)?;
        self.check_writable(rel, &first.io)?;
        self.check_existing_hash(rel, &first.io, expected_existing)?;
        if let Some(parent) = first.io.parent() {
            create_dir_all(parent)?;
        }
        // the folders exist now: check again, then act on the canonical path only
        let io = self.checked(rel)?.io;
        let existing = hash_existing(&io, MAX_EXISTING_BYTES).map_err(refusal)?;
        if let Some((bytes, hash)) = &existing
            && hash.eq_ignore_ascii_case(expected_sha256)
        {
            return Ok(CopyReport {
                rel: rel.to_owned(),
                bytes: *bytes,
                sha256: hash.clone(),
                replaced: false,
                backup: None,
                verified: true,
            });
        }
        let replaced = existing.is_some();
        let mut backup_path = None;
        if replaced && backup {
            let report = rotate(&io, &self.backup_policy(rel), self.clock.as_ref())?;
            backup_path = report.created;
        }
        // last look before the bytes go down: the backup step took time
        let io = self.checked(rel)?.io;
        self.check_existing_hash(rel, &io, expected_existing)?;
        let done = copy_verified(source, &io, expected_sha256, max_bytes).map_err(refusal)?;
        Ok(CopyReport {
            rel: rel.to_owned(),
            bytes: done.bytes,
            sha256: done.sha256,
            replaced,
            backup: backup_path,
            verified: true,
        })
    }
}
