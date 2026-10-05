//! Errors of the composition root and the mapping of every crate error to the IPC envelope.
//!
//! [`AppError`] covers what only the app layer knows: an unknown command, a request that does not
//! deserialise (with the path of the offending field), job registry failures. [`BootError`] is what
//! stops the application from starting. The mapping functions turn a [`ManagerError`] or a
//! [`ToolkitError`] into an [`ApiError`] whose code is always one of the registered codes of
//! `rimstudio-ipc-types` (the original code is kept in `details.sourceCode` when it had to be
//! translated), whose message has been through the redactor, and whose id is fresh.

use std::any::Any;

use rimstudio_core::redact::Redactor;
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_manager::ManagerError;
use rimstudio_toolkit::error::ToolkitError;
use thiserror::Error;

/// What only the app layer can get wrong.
#[derive(Debug, Error)]
pub enum AppError {
    /// No registry row has this name.
    #[error("unknown command {name:?}")]
    UnknownCommand {
        /// The name that was asked for.
        name: String,
    },
    /// The request JSON does not have the shape of the request type.
    #[error("invalid request for {command}: {message} (at {field})")]
    InvalidRequest {
        /// The command.
        command: String,
        /// The path of the offending field, `.` when the whole request is wrong.
        field: String,
        /// What is wrong.
        message: String,
    },
    /// A job with this id is already registered (running or finished less than a minute ago).
    #[error("job id {id:?} is already in use")]
    JobDuplicateId {
        /// The id.
        id: String,
    },
    /// The job registry does not know this id.
    #[error("job {id:?} is not known")]
    JobNotFound {
        /// The id.
        id: String,
    },
    /// Something broke that is nobody's fault: a panic in a handler, a value that cannot be encoded.
    #[error("internal error: {message}")]
    Internal {
        /// What happened.
        message: String,
    },
}

impl AppError {
    /// The stable code, one of the registered IPC codes.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownCommand { .. } => codes::IPC_UNKNOWN_COMMAND,
            Self::InvalidRequest { .. } => codes::IPC_INVALID_REQUEST,
            Self::JobDuplicateId { .. } => codes::JOB_DUPLICATE_ID,
            Self::JobNotFound { .. } => codes::JOB_NOT_FOUND,
            Self::Internal { .. } => codes::APP_INTERNAL_ERROR,
        }
    }

    /// The envelope, with the structured details the code needs.
    #[must_use]
    pub fn to_api(&self) -> ApiError {
        let api = ApiError::new(self.code(), self.to_string());
        match self {
            Self::UnknownCommand { name } => api.detail("command", name.as_str()),
            Self::InvalidRequest { command, field, .. } => api
                .detail("command", command.as_str())
                .detail("field", field.as_str()),
            Self::JobDuplicateId { id } | Self::JobNotFound { id } => {
                api.detail("jobId", id.as_str())
            }
            Self::Internal { .. } => api,
        }
    }

    /// An [`AppError::InvalidRequest`] about one field.
    #[must_use]
    pub fn invalid(command: &str, field: &str, message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            command: command.to_owned(),
            field: field.to_owned(),
            message: message.into(),
        }
    }

    /// An [`AppError::Internal`].
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        e.to_api()
    }
}

/// Why the application could not start.
#[derive(Debug, Error)]
pub enum BootError {
    /// No standard folder for the app data could be determined (no home folder).
    #[error("no data folder could be determined")]
    NoDataDir,
    /// A data root could not be created.
    #[error("the data folders could not be prepared: {0}")]
    Roots(#[from] rimstudio_io::error::StoreError),
    /// A data root exists but cannot be written.
    #[error("the {root} folder is not writable: {reason}")]
    NotWritable {
        /// Which root (`config`, `data`, `cache` or `logs`).
        root: &'static str,
        /// The operating system's reason.
        reason: String,
    },
    /// The settings could not be opened at all (a damaged file is not this: it loads read only).
    #[error("the settings could not be opened: {0}")]
    Settings(#[from] ManagerError),
    /// The toolkit stores could not be opened.
    #[error("the toolkit could not be prepared: {0}")]
    Toolkit(#[from] ToolkitError),
    /// The job runtime could not start.
    #[error("the job runtime could not start: {0}")]
    Runtime(String),
    /// Logging could not be set up.
    #[error("logging could not be set up: {0}")]
    Logging(String),
}

impl BootError {
    /// A stable code for logs and the CLI.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoDataDir => "boot.no-data-dir",
            Self::Roots(_) => "boot.roots-failed",
            Self::NotWritable { .. } => "boot.root-not-writable",
            Self::Settings(_) => "boot.settings-failed",
            Self::Toolkit(_) => "boot.toolkit-failed",
            Self::Runtime(_) => "boot.runtime-failed",
            Self::Logging(_) => "boot.logging-failed",
        }
    }
}

/// Translates the code of any crate error into a registered IPC code.
///
/// Exact codes come first, then prefixes of whole crates. A code the table does not know becomes
/// `app.internal-error`, so a new engine error can never put an unregistered code on the wire.
#[must_use]
pub fn registered_code(source: &str) -> &'static str {
    if let Some(info) = codes::lookup(source) {
        return info.code;
    }
    match source {
        "manager.settings-invalid"
        | "manager.unknown-section"
        | "manager.unknown-keys"
        | "manager.read-only"
        | "store.parse"
        | "store.shape"
        | "store.document-corrupt"
        | "store.write-blocked"
        | "store.edit"
        | "store.serialize"
        | "store.id-invalid"
        | "core.invalid-settings" => codes::SETTINGS_INVALID,
        "store.newer-than-app" => codes::SETTINGS_NEWER_SCHEMA,
        "manager.override-invalid" => codes::DETECT_FAILED,
        "manager.folder-rejected" => codes::SOURCES_INVALID_FOLDER,
        "manager.source-not-found" => codes::SOURCES_NOT_FOUND,
        "manager.invalid-request"
        | "core.invalid-id"
        | "core.invalid-version"
        | "core.invalid-load-folders"
        | "core.invalid-path"
        | "core.invalid-tree" => codes::IPC_INVALID_REQUEST,
        "manager.no-sources"
        | "scan.pool"
        | "scan.source-unreadable"
        | "cache.write"
        | "cache.serialize" => codes::LIBRARY_SCAN_FAILED,
        "manager.io"
        | "store.io"
        | "io.write-verify-failed"
        | "migrate.failed"
        | "migrate.no-path" => codes::IO_WRITE_FAILED,
        "port.not-found" => codes::IO_NOT_FOUND,
        "port.permission-denied" => codes::IO_PERMISSION_DENIED,
        "workspace.not-a-project" => codes::IO_NOT_FOUND,
        "workspace.invalid-path" => codes::PROJECT_PATH_OUTSIDE_ROOT,
        "workspace.project-not-found" => codes::PROJECT_NOT_OPEN,
        "workspace.io" | "workspace.store" => codes::IO_WRITE_FAILED,
        "workspace.cancelled" | "core.cancelled" => codes::JOB_CANCELLED,
        "workspace.type-table" => codes::DESIGNER_REFERENCE_UNAVAILABLE,
        "workspace.def-not-found" => codes::DEFS_INVALID_QUERY,
        "fence.refused" => codes::IO_PATH_OUTSIDE_ROOTS,
        _ => registered_by_prefix(source),
    }
}

fn registered_by_prefix(source: &str) -> &'static str {
    let area = source.split('.').next().unwrap_or("");
    match area {
        "guard" => codes::IO_PATH_OUTSIDE_ROOTS,
        "design" => codes::DESIGNER_INVALID_DRAFT,
        "xml" => codes::DESIGNER_APPLY_FAILED,
        "store" | "migrate" | "port" => codes::IO_WRITE_FAILED,
        "scan" | "cache" => codes::LIBRARY_SCAN_FAILED,
        _ => codes::APP_INTERNAL_ERROR,
    }
}

fn finish(source_code: &str, message: &str, redactor: &Redactor) -> ApiError {
    let code = registered_code(source_code);
    let api = ApiError::new(code, redactor.redact(message));
    if code == source_code {
        api
    } else {
        api.detail("sourceCode", source_code)
    }
}

/// Maps a manager error to the envelope.
#[must_use]
pub fn api_from_manager(e: &ManagerError, redactor: &Redactor) -> ApiError {
    let mut api = finish(e.code(), &e.to_string(), redactor);
    match e {
        ManagerError::FolderRejected { diagnostics, .. } => {
            let overlap = diagnostics.iter().any(|d| {
                d.code.as_str().contains("source-overlap")
                    || d.code.as_str().contains("source-inside")
            });
            if overlap {
                api.code = codes::SOURCES_OVERLAP.to_owned();
            }
            let reasons: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
            api = api.detail("reasons", reasons.join(","));
        }
        ManagerError::UnknownSection { name } => api = api.detail("section", name.as_str()),
        ManagerError::UnknownKeys { keys } => api = api.detail("keys", keys.join(",")),
        ManagerError::SettingsInvalid { problems } => {
            let fields: Vec<&str> = problems.iter().map(|p| p.field.as_str()).collect();
            api = api.detail("fields", fields.join(","));
        }
        ManagerError::InvalidRequest { field, .. } => api = api.detail("field", *field),
        ManagerError::OverrideInvalid { field, .. } => api = api.detail("field", *field),
        ManagerError::SourceNotFound { id } => api = api.detail("sourceId", id.as_str()),
        _ => {}
    }
    api
}

/// Maps a toolkit error to the envelope.
#[must_use]
pub fn api_from_toolkit(e: &ToolkitError, redactor: &Redactor) -> ApiError {
    let mut api = finish(e.code(), &e.to_string(), redactor);
    match e {
        ToolkitError::PlanStale { expected, found } => {
            api = api
                .detail("expected", expected.as_str())
                .detail("found", found.as_str());
        }
        ToolkitError::DraftNotFound { project_id, id } => {
            api = api
                .detail("projectId", project_id.as_str())
                .detail("draftId", id.as_str());
        }
        ToolkitError::ProjectNotOpen { id } => api = api.detail("projectId", id.as_str()),
        ToolkitError::DraftNewerSchema { found, supported } => {
            api = api.detail("found", *found).detail("supported", *supported);
        }
        _ => {}
    }
    api
}

/// Maps a store error (a failed read or write of an app file) to the envelope.
#[must_use]
pub fn api_from_store(e: &rimstudio_io::error::StoreError, redactor: &Redactor) -> ApiError {
    finish(e.code(), &e.to_string(), redactor)
}

/// Maps a workspace error to the envelope.
#[must_use]
pub fn api_from_workspace(
    e: &rimstudio_workspace::WorkspaceError,
    redactor: &Redactor,
) -> ApiError {
    finish(e.code(), &e.to_string(), redactor)
}

/// The text of a panic payload, for the log and the envelope.
#[must_use]
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic without a message".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
    use rimstudio_core::settings::SettingProblem;
    use rimstudio_io::error::StoreError;
    use rimstudio_library::error::{CacheError, LibraryError, ScanError};
    use rstest::rstest;

    use super::*;

    fn redactor() -> Redactor {
        Redactor::new().with_home("/home/rs_user")
    }

    fn manager_samples() -> Vec<ManagerError> {
        vec![
            StoreError::Edit {
                path: Utf8PathBuf::from("/x"),
                message: "m".into(),
            }
            .into(),
            ManagerError::Library(LibraryError::Scan(ScanError::Pool("p".into()))),
            ManagerError::Cache(CacheError::Serialize("s".into())),
            ManagerError::Path(rimstudio_core::error::CoreError::invalid_path(
                "rel",
                rimstudio_core::error::PathFault::Relative,
            )),
            ManagerError::SettingsInvalid {
                problems: vec![SettingProblem {
                    field: "appearance.fontScale".into(),
                    reason: "too big".into(),
                }],
            },
            ManagerError::UnknownSection { name: "x".into() },
            ManagerError::UnknownKeys {
                keys: vec!["a.b".into()],
            },
            ManagerError::ReadOnly {
                path: Utf8PathBuf::from("/x"),
                reason: "newer".into(),
            },
            ManagerError::OverrideInvalid {
                field: "gameInstall",
                reason: "no".into(),
            },
            ManagerError::FolderRejected {
                path: Utf8PathBuf::from("/x"),
                diagnostics: vec![],
            },
            ManagerError::SourceNotFound { id: "x".into() },
            ManagerError::InvalidRequest {
                field: "label",
                reason: "empty".into(),
            },
            ManagerError::NoSources,
            ManagerError::Io {
                op: "create-settings",
                path: Utf8PathBuf::from("/x"),
                message: "denied".into(),
            },
        ]
    }

    fn toolkit_samples() -> Vec<ToolkitError> {
        vec![
            ToolkitError::invalid_draft("x"),
            ToolkitError::DraftNewerSchema {
                found: 2,
                supported: 1,
            },
            ToolkitError::DraftNotFound {
                project_id: "p-1".into(),
                id: "d-1".into(),
            },
            ToolkitError::ReferenceUnavailable { reason: "x".into() },
            ToolkitError::CalibrationUnavailable { reason: "x".into() },
            ToolkitError::QuizFinished,
            ToolkitError::QuizWrongAnswer { detail: "x".into() },
            ToolkitError::Cancelled,
            ToolkitError::internal("x"),
            ToolkitError::PlanStale {
                expected: "a".into(),
                found: "b".into(),
            },
            ToolkitError::ApplyRefused { reason: "x".into() },
            ToolkitError::PathRefused {
                path: "x".into(),
                reason: "y".into(),
            },
            ToolkitError::ProjectNotOpen { id: "p-1".into() },
            ToolkitError::ProjectInvalid {
                path: "x".into(),
                reason: "y".into(),
            },
            rimstudio_design::error::DesignError::invalid("damage", "bad").into(),
            StoreError::IdInvalid {
                id: "x".into(),
                reason: "bad",
            }
            .into(),
            rimstudio_workspace::WorkspaceError::Cancelled.into(),
            rimstudio_workspace::WorkspaceError::DefNotFound {
                def_type: "ThingDef".into(),
                def_name: "RS_X".into(),
            }
            .into(),
        ]
    }

    #[test]
    fn every_manager_error_maps_to_a_registered_code() {
        for e in manager_samples() {
            let api = api_from_manager(&e, &redactor());
            assert!(api.is_registered(), "{} -> {}", e.code(), api.code);
            assert!(api.error_id.starts_with("e-"));
        }
    }

    #[test]
    fn every_toolkit_error_maps_to_a_registered_code() {
        for e in toolkit_samples() {
            let api = api_from_toolkit(&e, &redactor());
            assert!(api.is_registered(), "{} -> {}", e.code(), api.code);
        }
    }

    #[rstest]
    #[case("manager.settings-invalid", "settings.invalid")]
    #[case("manager.unknown-keys", "settings.invalid")]
    #[case("store.newer-than-app", "settings.newer-schema")]
    #[case("store.io", "io.write-failed")]
    #[case("manager.source-not-found", "sources.not-found")]
    #[case("manager.folder-rejected", "sources.invalid-folder")]
    #[case("manager.no-sources", "library.scan-failed")]
    #[case("scan.pool", "library.scan-failed")]
    #[case("guard.traversal", "io.path-outside-roots")]
    #[case("design.invalid-input", "designer.invalid-draft")]
    #[case("workspace.cancelled", "job.cancelled")]
    #[case("workspace.def-not-found", "defs.invalid-query")]
    #[case("designer.plan-stale", "designer.plan-stale")]
    #[case("io.not-found", "io.not-found")]
    #[case("nothing.known", "app.internal-error")]
    fn codes_are_translated_by_table(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(registered_code(source), expected);
    }

    #[test]
    fn a_translated_code_keeps_the_source_code_in_the_details() {
        let e = ManagerError::NoSources;
        let api = api_from_manager(&e, &redactor());
        let source = api
            .details
            .as_ref()
            .and_then(|d| d.get("sourceCode"))
            .and_then(|v| v.as_str());
        assert_eq!(source, Some("manager.no-sources"));
    }

    #[test]
    fn a_registered_code_has_no_source_code_detail() {
        let api = api_from_toolkit(&ToolkitError::QuizFinished, &redactor());
        assert_eq!(api.code, "designer.quiz-finished");
        assert!(api.details.is_none());
    }

    #[test]
    fn overlap_diagnostics_choose_the_overlap_code() {
        let e = ManagerError::FolderRejected {
            path: Utf8PathBuf::from("/x"),
            diagnostics: vec![Diagnostic {
                code: DiagCode::new("deploy.source-overlap"),
                severity: Severity::Error,
                mod_idx: None,
                file: None,
                message: "overlap".into(),
                span: None,
                args: Default::default(),
            }],
        };
        assert_eq!(api_from_manager(&e, &redactor()).code, "sources.overlap");
    }

    #[test]
    fn messages_are_redacted() {
        let e = ManagerError::Io {
            op: "create-settings",
            path: Utf8PathBuf::from("/home/rs_user/secret/settings.jsonc"),
            message: "denied".into(),
        };
        let api = api_from_manager(&e, &redactor());
        assert!(!api.message.contains("rs_user"), "{}", api.message);
    }

    #[test]
    fn app_errors_use_registered_codes_and_details() {
        let all = [
            AppError::UnknownCommand { name: "x".into() },
            AppError::invalid("c", "a.b", "bad"),
            AppError::JobDuplicateId { id: "j".into() },
            AppError::JobNotFound { id: "j".into() },
            AppError::internal("boom"),
        ];
        for e in &all {
            let api = e.to_api();
            assert!(api.is_registered(), "{}", api.code);
        }
        let api = AppError::invalid("c", "a.b", "bad").to_api();
        let field = api
            .details
            .as_ref()
            .and_then(|d| d.get("field"))
            .and_then(|v| v.as_str());
        assert_eq!(field, Some("a.b"));
    }

    #[test]
    fn boot_error_codes_are_distinct() {
        let all = [
            BootError::NoDataDir,
            BootError::NotWritable {
                root: "data",
                reason: "x".into(),
            },
            BootError::Runtime("x".into()),
            BootError::Logging("x".into()),
        ];
        let mut codes: Vec<&str> = all.iter().map(BootError::code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
    }

    #[test]
    fn panic_payloads_become_text() {
        let a: Box<dyn Any + Send> = Box::new("literal");
        let b: Box<dyn Any + Send> = Box::new(String::from("owned"));
        let c: Box<dyn Any + Send> = Box::new(7_u32);
        assert_eq!(panic_message(a.as_ref()), "literal");
        assert_eq!(panic_message(b.as_ref()), "owned");
        assert!(panic_message(c.as_ref()).contains("without a message"));
    }
}
