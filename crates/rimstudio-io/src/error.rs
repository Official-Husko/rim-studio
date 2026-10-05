//! Error types of the storage layer.
//!
//! [`IoError`] groups the three families: [`StoreError`] (reading and writing app data),
//! [`MigrateError`] (schema migrations) and [`GuardError`] (path validation and the write fence).
//! Every variant has a stable [`code`](IoError::code) string for logs and the IPC error envelope.

use camino::Utf8PathBuf;
use thiserror::Error;

/// Any error raised by `rimstudio-io`.
#[derive(Debug, Error)]
pub enum IoError {
    /// Reading or writing app data failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A schema migration failed.
    #[error(transparent)]
    Migrate(#[from] MigrateError),
    /// A path was refused by the guard or the write fence.
    #[error(transparent)]
    Guard(#[from] GuardError),
}

impl IoError {
    /// The stable code of the underlying error, for example `store.parse`.
    pub fn code(&self) -> &'static str {
        match self {
            IoError::Store(e) => e.code(),
            IoError::Migrate(e) => e.code(),
            IoError::Guard(e) => e.code(),
        }
    }
}

/// Result alias using [`IoError`].
pub type IoResult<T> = Result<T, IoError>;

/// Failures while loading or saving app data (settings files, collections, backups).
#[derive(Debug, Error)]
pub enum StoreError {
    /// An operating system call failed.
    #[error("io failure during {op} on {path}: {source}")]
    Io {
        /// The step that failed, for example `read` or `rename`.
        op: &'static str,
        /// The path involved.
        path: Utf8PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The text is not valid JSON or JSONC.
    #[error("{path}: syntax error at line {line}, column {column}: {message}")]
    Parse {
        /// The file.
        path: Utf8PathBuf,
        /// What is wrong.
        message: String,
        /// One based line.
        line: usize,
        /// One based column.
        column: usize,
    },
    /// The JSON is valid but does not match the Rust type.
    #[error("{path}: does not match {kind}: {message}")]
    Shape {
        /// The file.
        path: Utf8PathBuf,
        /// The document kind.
        kind: &'static str,
        /// The message including the path to the offending field.
        message: String,
    },
    /// A collection document is unreadable; it was moved aside when `quarantined` is set.
    #[error("document {id} is corrupt: {reason}")]
    DocumentCorrupt {
        /// The document id.
        id: String,
        /// Why it was rejected.
        reason: String,
        /// Where the file was moved, when that succeeded.
        quarantined: Option<Utf8PathBuf>,
    },
    /// The file was written by a newer version of the app and is read only.
    #[error("{path}: written by a newer version (file v{found}, this app supports v{supported})")]
    NewerThanApp {
        /// The file.
        path: Utf8PathBuf,
        /// The document kind.
        kind: &'static str,
        /// The version found.
        found: u32,
        /// The version this app writes.
        supported: u32,
    },
    /// A document id is not allowed.
    #[error("invalid document id {id:?}: {reason}")]
    IdInvalid {
        /// The rejected id (truncated).
        id: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// Re-reading a written file did not give the bytes that were written.
    #[error("write verification failed for {path}")]
    WriteVerifyFailed {
        /// The file.
        path: Utf8PathBuf,
    },
    /// Saving is refused because the file on disk could not be read faithfully.
    #[error("{path} is not writable right now: {reason}")]
    WriteBlocked {
        /// The file.
        path: Utf8PathBuf,
        /// Why writes are blocked.
        reason: String,
    },
    /// A comment preserving edit could not be applied.
    #[error("{path}: edit failed: {message}")]
    Edit {
        /// The file.
        path: Utf8PathBuf,
        /// What went wrong.
        message: String,
    },
    /// Serialising a value failed.
    #[error("serialisation failed: {message}")]
    Serialize {
        /// What went wrong.
        message: String,
    },
    /// A migration failed.
    #[error(transparent)]
    Migrate(#[from] MigrateError),
}

impl StoreError {
    /// Builds an [`StoreError::Io`].
    pub fn io(op: &'static str, path: impl Into<Utf8PathBuf>, source: std::io::Error) -> Self {
        StoreError::Io {
            op,
            path: path.into(),
            source,
        }
    }

    /// The stable code of this error.
    pub fn code(&self) -> &'static str {
        match self {
            StoreError::Io { .. } => "store.io",
            StoreError::Parse { .. } => "store.parse",
            StoreError::Shape { .. } => "store.shape",
            StoreError::DocumentCorrupt { .. } => "store.document-corrupt",
            StoreError::NewerThanApp { .. } => "store.newer-than-app",
            StoreError::IdInvalid { .. } => "store.id-invalid",
            StoreError::WriteVerifyFailed { .. } => "io.write-verify-failed",
            StoreError::WriteBlocked { .. } => "store.write-blocked",
            StoreError::Edit { .. } => "store.edit",
            StoreError::Serialize { .. } => "store.serialize",
            StoreError::Migrate(e) => e.code(),
        }
    }
}

/// Failures of a schema migration.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MigrateError {
    /// No chain of steps leads from the stored version to the current one.
    #[error("no migration path for {kind} from v{from} to v{to}")]
    NoPath {
        /// The document kind.
        kind: String,
        /// The stored version.
        from: u32,
        /// The wanted version.
        to: u32,
    },
    /// A step reported a failure.
    #[error("migration of {kind} from v{from} failed: {reason}")]
    Failed {
        /// The document kind.
        kind: String,
        /// The version the failing step started from.
        from: u32,
        /// The step's explanation.
        reason: String,
    },
}

impl MigrateError {
    /// Convenience constructor for migration steps.
    pub fn failed(kind: impl Into<String>, from: u32, reason: impl Into<String>) -> Self {
        MigrateError::Failed {
            kind: kind.into(),
            from,
            reason: reason.into(),
        }
    }

    /// The stable code of this error.
    pub fn code(&self) -> &'static str {
        match self {
            MigrateError::NoPath { .. } => "migrate.no-path",
            MigrateError::Failed { .. } => "migrate.failed",
        }
    }
}

/// Failures of path validation and of the game write fence.
#[derive(Debug, Error)]
pub enum GuardError {
    /// The path is empty or contains a NUL byte or another forbidden character.
    #[error("invalid path {path:?}: {reason}")]
    Invalid {
        /// The rejected text (truncated).
        path: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// The path is not absolute.
    #[error("path {path:?} is not absolute")]
    Relative {
        /// The rejected text.
        path: String,
    },
    /// The path contains a parent directory component.
    #[error("path {path:?} contains a parent directory component")]
    Traversal {
        /// The rejected text.
        path: String,
    },
    /// The path is not inside any allowed root.
    #[error("path {path:?} is outside the allowed roots")]
    OutsideRoots {
        /// The rejected text.
        path: String,
    },
    /// The path looks inside a root but a link leads out of it.
    #[error("path {path:?} crosses a link that leaves the allowed roots")]
    LinkEscape {
        /// The rejected text.
        path: String,
    },
    /// The write fence refused an operation.
    #[error("write fence refused {operation}: {reason}")]
    FenceRefused {
        /// The operation name.
        operation: &'static str,
        /// Why.
        reason: String,
    },
    /// An operating system call failed while validating or acting.
    #[error("io failure during {op} on {path}: {source}")]
    Io {
        /// The step.
        op: &'static str,
        /// The path.
        path: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl GuardError {
    /// The stable code of this error. Outside-root violations use `io.path-outside-roots`.
    pub fn code(&self) -> &'static str {
        match self {
            GuardError::Invalid { .. } => "guard.invalid-path",
            GuardError::Relative { .. } => "guard.relative-path",
            GuardError::Traversal { .. } => "guard.traversal",
            GuardError::OutsideRoots { .. } => "io.path-outside-roots",
            GuardError::LinkEscape { .. } => "guard.link-escape",
            GuardError::FenceRefused { .. } => "fence.refused",
            GuardError::Io { .. } => "guard.io",
        }
    }
}
