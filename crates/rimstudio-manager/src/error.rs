//! Manager error type with stable codes.
//!
//! Every use case of this crate returns [`ManagerError`]. The variants wrap the typed errors of the
//! crates below (store, core, library) or describe a rejected request. [`ManagerError::code`] gives
//! a stable machine readable string that `rimstudio-app` maps into the IPC error envelope; the
//! `Display` text is for developers and logs, never for end users.

use camino::Utf8PathBuf;
use rimstudio_core::diag::Diagnostic;
use rimstudio_core::error::CoreError;
use rimstudio_core::settings::SettingProblem;
use rimstudio_io::error::StoreError;
use rimstudio_library::error::{CacheError, LibraryError};

/// The error of every manager use case.
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    /// Reading or writing a settings file, the report cache or the manifest failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The scan cache could not be written.
    #[error(transparent)]
    Library(#[from] LibraryError),
    /// The manifest cache could not be written.
    #[error(transparent)]
    Cache(#[from] CacheError),
    /// A path text could not be normalised.
    #[error(transparent)]
    Path(#[from] CoreError),
    /// The settings after a change would not be valid; nothing was written.
    #[error("the settings are not valid: {}", .problems.iter().map(|p| format!("{}: {}", p.field, p.reason)).collect::<Vec<_>>().join("; "))]
    SettingsInvalid {
        /// Every problem found.
        problems: Vec<SettingProblem>,
    },
    /// A section patch named a section that does not exist.
    #[error("unknown settings section {name:?}")]
    UnknownSection {
        /// The rejected section name.
        name: String,
    },
    /// A patch used keys that the section does not have.
    #[error("unknown settings keys: {}", .keys.join(", "))]
    UnknownKeys {
        /// Dotted key paths that were not recognised.
        keys: Vec<String>,
    },
    /// The file on disk must not be overwritten (syntax error, newer version, unreadable).
    #[error("{path} is read only: {reason}")]
    ReadOnly {
        /// The file.
        path: Utf8PathBuf,
        /// Why saving is refused.
        reason: String,
    },
    /// A path override failed validation.
    #[error("invalid override for {field}: {reason}")]
    OverrideInvalid {
        /// The override field, for example `gameInstall`.
        field: &'static str,
        /// What is wrong.
        reason: String,
    },
    /// A folder cannot be added because of hard errors (missing, not a folder, overlap).
    #[error("the folder {path} cannot be added")]
    FolderRejected {
        /// The normalised folder path.
        path: Utf8PathBuf,
        /// The diagnostics that explain the refusal.
        diagnostics: Vec<Diagnostic>,
    },
    /// No custom folder or source has this id.
    #[error("no source with id {id:?}")]
    SourceNotFound {
        /// The id that was asked for.
        id: String,
    },
    /// A request field is out of range or not allowed for this source.
    #[error("invalid request field {field}: {reason}")]
    InvalidRequest {
        /// The field name.
        field: &'static str,
        /// What is wrong.
        reason: String,
    },
    /// There is nothing to scan: no install was detected and no custom folder is enabled.
    #[error("there are no sources to scan")]
    NoSources,
    /// A file system operation of this crate failed.
    #[error("{op} failed for {path}: {message}")]
    Io {
        /// The operation.
        op: &'static str,
        /// The path.
        path: Utf8PathBuf,
        /// The OS message.
        message: String,
    },
}

impl ManagerError {
    /// A stable machine readable code.
    pub fn code(&self) -> &'static str {
        match self {
            ManagerError::Store(e) => e.code(),
            ManagerError::Library(e) => e.code(),
            ManagerError::Cache(e) => e.code(),
            ManagerError::Path(e) => e.code(),
            ManagerError::SettingsInvalid { .. } => "manager.settings-invalid",
            ManagerError::UnknownSection { .. } => "manager.unknown-section",
            ManagerError::UnknownKeys { .. } => "manager.unknown-keys",
            ManagerError::ReadOnly { .. } => "manager.read-only",
            ManagerError::OverrideInvalid { .. } => "manager.override-invalid",
            ManagerError::FolderRejected { .. } => "manager.folder-rejected",
            ManagerError::SourceNotFound { .. } => "manager.source-not-found",
            ManagerError::InvalidRequest { .. } => "manager.invalid-request",
            ManagerError::NoSources => "manager.no-sources",
            ManagerError::Io { .. } => "manager.io",
        }
    }

    pub(crate) fn io(op: &'static str, path: &camino::Utf8Path, e: &std::io::Error) -> Self {
        ManagerError::Io {
            op,
            path: path.to_owned(),
            message: e.to_string(),
        }
    }
}

/// The result type of the manager use cases.
pub type ManagerResult<T> = Result<T, ManagerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_and_namespaced() {
        let e = ManagerError::NoSources;
        assert_eq!(e.code(), "manager.no-sources");
        let e = ManagerError::UnknownSection { name: "x".into() };
        assert_eq!(e.code(), "manager.unknown-section");
        assert!(e.to_string().contains("\"x\""));
    }

    #[test]
    fn store_errors_keep_their_own_code() {
        let e: ManagerError = StoreError::Edit {
            path: Utf8PathBuf::from("/x"),
            message: "m".into(),
        }
        .into();
        assert_eq!(e.code(), "store.edit");
    }
}
