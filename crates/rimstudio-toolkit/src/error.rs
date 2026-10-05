//! The error type of the toolkit.
//!
//! Content problems (a value out of band, a missing field, a failed reference) are never errors: they are
//! diagnostics beside the result (invariant I-10). A [`ToolkitError`] means the request itself cannot be
//! served: a draft that cannot be read, a reference set that is not loaded, a store that failed, a job that
//! was cancelled. Every variant has a stable code from [`ToolkitError::code`]; the codes that exist in the
//! IPC registry reuse its constants, so the app layer can map an error to an `ApiError` without a table.

use rimstudio_design::error::DesignError;
use rimstudio_io::error::{GuardError, StoreError};
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_workspace::WorkspaceError;
use rimstudio_xml::XmlError;
use thiserror::Error;

/// Result alias of the toolkit.
pub type ToolkitResult<T> = Result<T, ToolkitError>;

/// Why a toolkit call could not be served.
#[derive(Debug, Error)]
pub enum ToolkitError {
    /// The draft or request cannot be read or is inconsistent (wrong shape, kind mismatch, bad id).
    #[error("invalid draft: {reason}")]
    InvalidDraft {
        /// What is wrong, in plain words.
        reason: String,
    },
    /// The stored or sent draft was written by a newer build.
    #[error("the draft has schema version {found}, this build supports up to {supported}")]
    DraftNewerSchema {
        /// The version found.
        found: u32,
        /// The newest version this build reads and writes.
        supported: u32,
    },
    /// No draft with this id exists in the project.
    #[error("draft {id} of project {project_id} does not exist")]
    DraftNotFound {
        /// The project id of the request.
        project_id: String,
        /// The draft id of the request.
        id: String,
    },
    /// The call needs a loaded reference set (a game install) and there is none, or it holds nothing usable.
    #[error("no reference data: {reason}")]
    ReferenceUnavailable {
        /// Why the data is missing.
        reason: String,
    },
    /// The reference pool is too small for calibration or the quiz.
    #[error("calibration is not available: {reason}")]
    CalibrationUnavailable {
        /// Why calibration cannot run.
        reason: String,
    },
    /// The quiz has no open question.
    #[error("the quiz has no open question")]
    QuizFinished,
    /// The answer does not belong to the open question, or names something the pool does not contain.
    #[error("the answer does not fit the open question: {detail}")]
    QuizWrongAnswer {
        /// What did not fit.
        detail: String,
    },
    /// A job was cancelled between units of work.
    #[error("cancelled")]
    Cancelled,
    /// The document store failed.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    /// The design engine rejected an input.
    #[error("design engine error: {0}")]
    Design(#[from] DesignError),
    /// An internal invariant was broken (a value that cannot be converted, for example).
    #[error("internal error: {what}")]
    Internal {
        /// What went wrong.
        what: String,
    },
    /// The plan that was reviewed is not the plan the same request builds now (a file changed on disk or
    /// the inputs differ), so nothing is written.
    #[error("the plan changed since it was reviewed (expected {expected}, now {found})")]
    PlanStale {
        /// The plan id the caller reviewed.
        expected: String,
        /// The plan id built now.
        found: String,
    },
    /// The plan cannot be applied (it has an error diagnostic, or the request is not applicable).
    #[error("the plan cannot be applied: {reason}")]
    ApplyRefused {
        /// Why.
        reason: String,
    },
    /// A write was refused: the path is outside the project root, inside the game install or config
    /// folders, or not a safe relative path.
    #[error("the path {path} is refused: {reason}")]
    PathRefused {
        /// The path as given.
        path: String,
        /// Why it is refused.
        reason: String,
    },
    /// No project with this id is registered.
    #[error("project {id} is not open")]
    ProjectNotOpen {
        /// The project id of the request.
        id: String,
    },
    /// The folder is not a mod project (no About.xml, or it cannot be read).
    #[error("{path} is not a mod project: {reason}")]
    ProjectInvalid {
        /// The folder.
        path: String,
        /// Why.
        reason: String,
    },
    /// The workspace layer failed (a path, a store, a cancelled rebuild).
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    /// The XML boundary failed on a file it had to read or edit.
    #[error("xml error: {0}")]
    Xml(#[from] XmlError),
    /// A path guard refused a path.
    #[error("path guard: {0}")]
    Guard(#[from] GuardError),
}

impl ToolkitError {
    /// The stable code. The designer codes are those of the IPC registry; store and design errors keep the
    /// code of the engine that raised them.
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::InvalidDraft { .. } => codes::DESIGNER_INVALID_DRAFT,
            Self::DraftNewerSchema { .. } => codes::DESIGNER_DRAFT_NEWER_SCHEMA,
            Self::DraftNotFound { .. } => codes::DESIGNER_DRAFT_NOT_FOUND,
            Self::ReferenceUnavailable { .. } => codes::DESIGNER_REFERENCE_UNAVAILABLE,
            Self::CalibrationUnavailable { .. } => codes::DESIGNER_CALIBRATION_UNAVAILABLE,
            Self::QuizFinished => codes::DESIGNER_QUIZ_FINISHED,
            Self::QuizWrongAnswer { .. } => codes::DESIGNER_QUIZ_WRONG_ANSWER,
            Self::Cancelled => codes::JOB_CANCELLED,
            Self::Store(e) => e.code(),
            Self::Design(e) => e.code(),
            Self::Internal { .. } => codes::APP_INTERNAL_ERROR,
            Self::PlanStale { .. } => codes::DESIGNER_PLAN_STALE,
            Self::ApplyRefused { .. } => codes::DESIGNER_APPLY_FAILED,
            Self::PathRefused { .. } => codes::PROJECT_PATH_OUTSIDE_ROOT,
            Self::ProjectNotOpen { .. } => codes::PROJECT_NOT_OPEN,
            Self::ProjectInvalid { .. } => codes::IO_NOT_FOUND,
            Self::Workspace(e) => e.code(),
            Self::Xml(e) => e.code(),
            Self::Guard(e) => e.code(),
        }
    }

    /// The error as an IPC envelope with a freshly minted error id.
    #[must_use]
    pub fn to_api_error(&self) -> ApiError {
        ApiError::new(self.code(), self.to_string())
    }

    /// An [`ToolkitError::InvalidDraft`] with the given reason.
    #[must_use]
    pub fn invalid_draft(reason: impl Into<String>) -> Self {
        Self::InvalidDraft {
            reason: reason.into(),
        }
    }

    /// An [`ToolkitError::Internal`] with the given description.
    #[must_use]
    pub fn internal(what: impl Into<String>) -> Self {
        Self::Internal { what: what.into() }
    }
}

impl From<&ToolkitError> for ApiError {
    fn from(e: &ToolkitError) -> Self {
        e.to_api_error()
    }
}

impl From<rimstudio_core::error::CoreError> for ToolkitError {
    fn from(e: rimstudio_core::error::CoreError) -> Self {
        // The only core error a toolkit job meets is the cancellation of a token check.
        if e.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Internal {
                what: e.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn designer_errors_use_registered_codes() {
        let all = [
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
        ];
        for e in &all {
            assert!(
                codes::lookup(e.code()).is_some(),
                "{} is not in the registry",
                e.code()
            );
        }
    }

    #[test]
    fn engine_errors_keep_their_own_codes() {
        let e: ToolkitError = DesignError::invalid("damage", "bad").into();
        assert_eq!(e.code(), "design.invalid-input");
        let e: ToolkitError = StoreError::IdInvalid {
            id: "x".into(),
            reason: "bad",
        }
        .into();
        assert_eq!(e.code(), "store.id-invalid");
    }

    #[test]
    fn api_error_carries_the_code_and_message() {
        let api = ToolkitError::QuizFinished.to_api_error();
        assert_eq!(api.code, "designer.quiz-finished");
        assert!(api.is_registered());
    }
}
