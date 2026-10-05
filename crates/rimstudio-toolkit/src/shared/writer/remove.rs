//! Guarded removal of one file: used where a person asks for a file of the project to go (the preview image
//! of a mod). The file is copied into the backup folder first and the removal is refused when that copy
//! cannot be made, so a removed file can always be brought back from the app data folder.

use rimstudio_io::backup::rotate;
use rimstudio_io::remove::remove_regular_file;

use super::{GuardedWriter, refused};
use crate::error::{ToolkitError, ToolkitResult};
use rimstudio_io::error::StoreError;

/// What a removal did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveReport {
    /// The path relative to the project root, with `/` separators.
    pub rel: String,
    /// True when a file was removed (false when there was none).
    pub removed: bool,
    /// The backup copy of the removed file, when one was made. A file identical to the newest earlier backup
    /// is not copied again.
    pub backup: Option<camino::Utf8PathBuf>,
}

impl GuardedWriter {
    /// Removes the regular file at a project path after backing it up.
    ///
    /// # Errors
    ///
    /// A path refusal, [`ToolkitError::PathRefused`] when the entry is a folder or the file is read only, and
    /// [`ToolkitError::Store`] when the backup or the removal fails.
    pub fn remove_file(&self, rel: &str) -> ToolkitResult<RemoveReport> {
        let io = self.checked(rel)?.io;
        let meta = match std::fs::metadata(io.as_std_path()) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RemoveReport {
                    rel: rel.to_owned(),
                    removed: false,
                    backup: None,
                });
            }
            Err(e) => return Err(ToolkitError::from(StoreError::io("remove", &io, e))),
        };
        if !meta.is_file() {
            return Err(refused(rel, "not a regular file"));
        }
        if meta.permissions().readonly() {
            return Err(refused(
                rel,
                "the file is read only; clear the read only flag to let RimStudio remove it",
            ));
        }
        let report = rotate(&io, &self.backup_policy(rel), self.clock.as_ref())?;
        // last look before the file goes: the backup step took time
        let io = self.checked(rel)?.io;
        let removed = remove_regular_file(&io)?;
        Ok(RemoveReport {
            rel: rel.to_owned(),
            removed,
            backup: report.created,
        })
    }
}
