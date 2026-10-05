//! Typed errors of the def engine.
//!
//! Content problems (a patch that fails, an inheritance cycle, an unknown type) are never errors
//! of this module: they are [`rimstudio_core::diag::Diagnostic`]s with the stable codes of
//! [`crate::diag_codes`]. [`DefsError`] covers API misuse, and [`PatchError`] is the exception a
//! patch operation raises, which the engine turns into a diagnostic.

use rimstudio_core::tree::TreeError;
use rimstudio_xpath::XPathError;

/// A failed call into the def engine (API misuse or a bad type table).
///
/// Every variant has a stable [`DefsError::code`] string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DefsError {
    /// The type table JSON is malformed or inconsistent.
    #[error("invalid type table: {message}")]
    InvalidTypeTable {
        /// What is wrong with the table.
        message: String,
    },
    /// A database was requested for a type name the table does not know.
    #[error("unknown def type {name:?}")]
    UnknownType {
        /// The name as given.
        name: String,
    },
    /// A database was requested for the root def type, which the game never builds.
    #[error("the game never builds a database for the root def type {name}")]
    RootType {
        /// The full name of the root type.
        name: String,
    },
    /// A node tree operation failed.
    #[error("node tree error: {0}")]
    Tree(#[from] TreeError),
}

impl DefsError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidTypeTable { .. } => "defs.type-table-invalid",
            Self::UnknownType { .. } => "defs.database-unknown-type",
            Self::RootType { .. } => "defs.database-root-type",
            Self::Tree(_) => "defs.tree",
        }
    }
}

/// An exception raised while applying a patch operation.
///
/// The game catches these at the top level, logs "Error in patch.Apply()" and counts the operation
/// as failed; a nested operation that raises aborts the enclosing sequence, conditional or
/// mod search. The engine does the same and reports `defs.patch-exception`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PatchError {
    /// An exception the game's runtime would raise (missing value, wrong node kind, bad XPath).
    #[error("{message}")]
    Exception {
        /// Human readable description.
        message: String,
    },
    /// The node tree rejected an edit (invalid name, stale node, forbidden structure).
    #[error("node tree error: {0}")]
    Tree(#[from] TreeError),
    /// The XPath expression failed to parse or evaluate.
    #[error("XPath error: {0}")]
    XPath(#[from] XPathError),
}

impl PatchError {
    /// An exception with the given message.
    pub fn exception(message: impl Into<String>) -> Self {
        Self::Exception {
            message: message.into(),
        }
    }

    /// The stable code of this error (every variant is reported as a patch exception).
    #[must_use]
    pub fn code(&self) -> &'static str {
        "defs.patch-exception"
    }
}

/// Result alias for engine calls that can fail on API misuse.
pub type DefsResult<T> = Result<T, DefsError>;
