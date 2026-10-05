//! The error type and the diagnostic codes of `rimstudio-workspace`.
//!
//! [`WorkspaceError`] reports misuse and failures of the operation itself (a folder that is not a
//! mod, an unwritable store, a cancelled open). Problems in user content (a broken DLL, an
//! unreadable Defs file, a missing mod of the active list) are never errors: they are
//! [`rimstudio_core::diag::Diagnostic`] values with the codes in [`codes`].

use rimstudio_io::StoreError;
use thiserror::Error;

/// Everything that can go wrong in `rimstudio-workspace`.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    /// The folder is not a mod: it has no `About/About.xml`.
    #[error("{path} is not a mod folder: it has no About/About.xml")]
    NotAProject {
        /// The folder.
        path: String,
    },
    /// A path was relative, empty, held a NUL character or climbed out of its root.
    #[error("invalid path {path}: {reason}")]
    InvalidPath {
        /// The rejected path (shortened).
        path: String,
        /// What is wrong with it.
        reason: String,
    },
    /// No project with this id is registered.
    #[error("project {id} is not registered")]
    ProjectNotFound {
        /// The project id.
        id: String,
    },
    /// A file system operation failed.
    #[error("{op} failed for {path}: {message}")]
    Io {
        /// The operation, for example `read-about`.
        op: &'static str,
        /// The path involved.
        path: String,
        /// The system message.
        message: String,
    },
    /// The document store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The operation was cancelled through its token.
    #[error("the operation was cancelled")]
    Cancelled,
    /// Not even an empty def type table could be made.
    #[error("the def type table is unusable: {message}")]
    TypeTable {
        /// What the table builder said.
        message: String,
    },
    /// A def reference does not resolve in the snapshot.
    #[error("no def {def_type}/{def_name} in the snapshot")]
    DefNotFound {
        /// The def type of the reference.
        def_type: String,
        /// The def name of the reference.
        def_name: String,
    },
}

/// A result with a [`WorkspaceError`].
pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

impl WorkspaceError {
    /// The stable code, `<area>.<kebab-name>`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            WorkspaceError::NotAProject { .. } => "workspace.not-a-project",
            WorkspaceError::InvalidPath { .. } => "workspace.invalid-path",
            WorkspaceError::ProjectNotFound { .. } => "workspace.project-not-found",
            WorkspaceError::Io { .. } => "workspace.io",
            WorkspaceError::Store(_) => "workspace.store",
            WorkspaceError::Cancelled => "workspace.cancelled",
            WorkspaceError::TypeTable { .. } => "workspace.type-table",
            WorkspaceError::DefNotFound { .. } => "workspace.def-not-found",
        }
    }

    /// An [`WorkspaceError::Io`] from a standard error.
    pub(crate) fn io(op: &'static str, path: &camino::Utf8Path, e: &std::io::Error) -> Self {
        WorkspaceError::Io {
            op,
            path: path.to_string(),
            message: e.to_string(),
        }
    }
}

/// Diagnostic codes produced by this crate.
pub mod codes {
    use rimstudio_core::diag::DiagCode;

    /// A DLL could not be read as a managed assembly and was skipped (damaged file).
    pub const ASSEMBLY_SKIPPED: DiagCode = DiagCode::new("workspace.assembly-skipped");
    /// A DLL is not a managed assembly (a native library) and was skipped.
    pub const ASSEMBLY_NOT_MANAGED: DiagCode = DiagCode::new("workspace.assembly-not-managed");
    /// The def type table could not be built; defs resolve as unknown types.
    pub const TYPE_TABLE_FAILED: DiagCode = DiagCode::new("workspace.type-table-failed");
    /// The type table cache could not be read or written.
    pub const TYPE_TABLE_CACHE: DiagCode = DiagCode::new("workspace.type-table-cache");
    /// No game assembly was found, so the base game def types are unknown.
    pub const NO_GAME_ASSEMBLIES: DiagCode = DiagCode::new("workspace.no-game-assemblies");
    /// A Defs or Patches file could not be read from disk.
    pub const FILE_UNREADABLE: DiagCode = DiagCode::new("workspace.file-unreadable");
    /// The project is also in the reference list; the project folder is used, last.
    pub const PROJECT_IN_LIST: DiagCode = DiagCode::new("workspace.project-in-list");
    /// A package id of the active list is not in the mod index and was left out.
    pub const MOD_NOT_FOUND: DiagCode = DiagCode::new("workspace.mod-not-found");
    /// The project's About.xml could not be read; defaults were used.
    pub const PROJECT_ABOUT_UNREADABLE: DiagCode =
        DiagCode::new("workspace.project-about-unreadable");
    /// A mod's load folders select nothing for this game version.
    pub const PACK_LOADS_NOTHING: DiagCode = DiagCode::new("workspace.pack-loads-nothing");
    /// The base game (Core) is not in the mod index.
    pub const CORE_MISSING: DiagCode = DiagCode::new("workspace.core-missing");
    /// A scaffold specification is invalid.
    pub const SCAFFOLD_INVALID: DiagCode = DiagCode::new("workspace.scaffold-invalid");

    /// Every code of this crate.
    pub const ALL: [DiagCode; 12] = [
        ASSEMBLY_SKIPPED,
        ASSEMBLY_NOT_MANAGED,
        TYPE_TABLE_FAILED,
        TYPE_TABLE_CACHE,
        NO_GAME_ASSEMBLIES,
        FILE_UNREADABLE,
        PROJECT_IN_LIST,
        MOD_NOT_FOUND,
        PROJECT_ABOUT_UNREADABLE,
        PACK_LOADS_NOTHING,
        CORE_MISSING,
        SCAFFOLD_INVALID,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_well_formed_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for code in codes::ALL {
            assert!(code.is_well_formed(), "{}", code.as_str());
            assert!(seen.insert(code.as_str().to_owned()));
        }
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(WorkspaceError::Cancelled.code(), "workspace.cancelled");
        let e = WorkspaceError::NotAProject { path: "x".into() };
        assert_eq!(e.code(), "workspace.not-a-project");
        assert!(e.to_string().contains("About.xml"));
    }
}
