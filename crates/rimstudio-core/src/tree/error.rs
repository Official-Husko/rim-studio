//! Typed errors of the node tree and the arena document, each with a stable code string.

use super::arena::NodeKind;

/// Every way a tree operation can fail.
///
/// Library code never panics on bad input; it returns one of these. The
/// [`TreeError::code`] strings are stable and safe to match on or to show in logs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TreeError {
    /// The JSON text or value is not a valid node tree. `code` is `tree.json` for syntax and
    /// shape problems, or the code of the specific validation failure found while decoding
    /// (`tree.invalid_name`, `tree.duplicate_attr`, `tree.invalid_char`).
    #[error("invalid node tree JSON: {message}")]
    Json {
        /// Stable error code.
        code: &'static str,
        /// Human readable description.
        message: String,
    },
    /// An element or attribute name is not a valid XML name.
    #[error("invalid name {name:?} at {path}: {reason}")]
    InvalidName {
        /// Location of the offending node, `Defs/ThingDef[2]/label` style (empty if unknown).
        path: String,
        /// The rejected name.
        name: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// An element carries two attributes with the same name.
    #[error("duplicate attribute {name:?} at {path}")]
    DuplicateAttr {
        /// Location of the element.
        path: String,
        /// The repeated attribute name.
        name: String,
    },
    /// Text or an attribute value contains a character that XML 1.0 cannot represent.
    #[error("character U+{ch:04X} is not allowed in XML content at {path}")]
    InvalidChar {
        /// Location of the text or the element carrying the attribute.
        path: String,
        /// The offending code point.
        ch: u32,
    },
    /// The tree is nested deeper than [`MAX_DEPTH`](super::MAX_DEPTH).
    #[error("tree is nested deeper than {max} levels at {path}")]
    TooDeep {
        /// Location where the limit was crossed.
        path: String,
        /// The limit.
        max: usize,
    },
    /// The node id refers to a node that was removed (or never existed in this arena).
    #[error("node id is stale or does not belong to this arena")]
    StaleId,
    /// The operation does not apply to this kind of node.
    #[error("operation {op} is not allowed on a {kind:?} node")]
    WrongKind {
        /// The kind of the node the operation was attempted on.
        kind: NodeKind,
        /// The operation name.
        op: &'static str,
    },
    /// The node being inserted still has a parent; detach it first.
    #[error("node already has a parent; detach it first")]
    AlreadyAttached,
    /// The reference node has no parent, so nothing can be inserted next to it.
    #[error("node has no parent")]
    NotAttached,
    /// The operation would make a node an ancestor of itself.
    #[error("operation would make a node its own ancestor")]
    Cycle,
    /// The document node cannot be detached, removed, replaced or cloned.
    #[error("the document node cannot be moved, removed or cloned")]
    DocumentRoot,
    /// The arena reached its maximum size.
    #[error("arena is full")]
    Capacity,
    /// Internal links are inconsistent (only reported by `ArenaDoc::check_integrity`).
    #[error("arena integrity violation: {0}")]
    Corrupt(String),
}

impl TreeError {
    /// The stable machine readable code of this error, for example `tree.invalid_name`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Json { code, .. } => code,
            Self::InvalidName { .. } => "tree.invalid_name",
            Self::DuplicateAttr { .. } => "tree.duplicate_attr",
            Self::InvalidChar { .. } => "tree.invalid_char",
            Self::TooDeep { .. } => "tree.too_deep",
            Self::StaleId => "tree.stale_id",
            Self::WrongKind { .. } => "tree.wrong_kind",
            Self::AlreadyAttached => "tree.attached",
            Self::NotAttached => "tree.detached",
            Self::Cycle => "tree.cycle",
            Self::DocumentRoot => "tree.document_root",
            Self::Capacity => "tree.capacity",
            Self::Corrupt(_) => "tree.corrupt",
        }
    }

    /// Converts a decoding error, recovering the validation code that the node deserialiser
    /// embeds in its messages as a leading `[code]`.
    pub(crate) fn from_json_error(err: &serde_json::Error) -> Self {
        let message = err.to_string();
        let code = KNOWN_JSON_CODES
            .iter()
            .find(|c| {
                message
                    .strip_prefix('[')
                    .is_some_and(|m| m.starts_with(**c))
            })
            .copied()
            .unwrap_or("tree.json");
        Self::Json { code, message }
    }
}

/// Validation codes that can surface from inside the deserialiser.
const KNOWN_JSON_CODES: [&str; 3] = [
    "tree.invalid_name",
    "tree.duplicate_attr",
    "tree.invalid_char",
];

/// Formats a deserialiser failure so that [`TreeError::from_json_error`] can recover its code.
pub(crate) fn coded_message(code: &str, detail: &str) -> String {
    format!("[{code}] {detail}")
}
