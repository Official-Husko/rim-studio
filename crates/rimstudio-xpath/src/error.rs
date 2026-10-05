//! Typed errors of the XPath engine, each with a stable code string.
//!
//! Parse time problems carry a byte offset into the (trimmed) expression text. Evaluation
//! problems are type errors, unsupported features and resource limits. The strings returned by
//! [`XPathError::code`] are stable and safe to match on or to show in logs.

/// Every way parsing or evaluating an XPath expression can fail.
///
/// The set of expressions that parse is the set the game's runtime (an XPath 1.0 engine over a
/// .NET `XmlDocument`) accepts for `SelectNodes`: unknown functions, wrong argument counts,
/// namespace prefixes and variables are rejected at parse time, exactly as the game rejects them
/// while compiling the expression.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum XPathError {
    /// The text is not valid XPath 1.0 syntax.
    #[error("xpath syntax error at byte {pos}: {message}")]
    Parse {
        /// Byte offset into the trimmed expression text.
        pos: usize,
        /// Human readable description.
        message: String,
    },
    /// A function name that is not part of the XPath 1.0 core library.
    #[error("unknown function {name}() at byte {pos}")]
    UnknownFunction {
        /// Byte offset of the function name.
        pos: usize,
        /// The name as written.
        name: String,
    },
    /// A core function called with a wrong number of arguments.
    #[error("function {name}() takes {expected} arguments but {found} were given (byte {pos})")]
    Arity {
        /// Byte offset of the function name.
        pos: usize,
        /// The function name.
        name: String,
        /// A description of the accepted counts, for example `2 or 3`.
        expected: String,
        /// The number of arguments in the expression.
        found: usize,
    },
    /// A name test with a namespace prefix. The unified document has no namespaces and the game
    /// supplies no namespace resolver, so such an expression cannot be compiled.
    #[error("namespace prefix {prefix:?} is not defined (byte {pos})")]
    UnboundPrefix {
        /// Byte offset of the name.
        pos: usize,
        /// The prefix as written.
        prefix: String,
    },
    /// A variable reference. No variables are ever defined.
    #[error("variable ${name} is not defined (byte {pos})")]
    UnboundVariable {
        /// Byte offset of the reference.
        pos: usize,
        /// The variable name as written.
        name: String,
    },
    /// An operand had the wrong type, for example `count("x")` or `"x"/a`.
    #[error("type error: {message}")]
    Type {
        /// Human readable description.
        message: String,
    },
    /// The expression evaluates to a number, string or boolean where a node-set is required.
    #[error("expression must evaluate to a node-set but produced a {found}")]
    NotNodeSet {
        /// The type that was produced: `number`, `string` or `boolean`.
        found: &'static str,
    },
    /// The result contains attribute nodes, which a plain node handle cannot name.
    #[error("the result contains attribute nodes; use select_nodes")]
    AttributeResult,
    /// An axis this engine does not evaluate.
    #[error("the {axis} axis is not supported")]
    UnsupportedAxis {
        /// The axis name.
        axis: &'static str,
    },
    /// A construct the engine does not support (for example `lang()`).
    #[error("unsupported: {what}")]
    Unsupported {
        /// What is not supported.
        what: String,
    },
    /// The expression nests deeper than the documented limit.
    #[error("expression is nested deeper than {limit} levels")]
    TooDeep {
        /// The nesting limit.
        limit: usize,
    },
    /// The context node handle is stale or does not belong to the document.
    #[error("the context node is stale")]
    StaleContext,
}

impl XPathError {
    /// A stable machine readable code, `xpath.<kind>`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "xpath.parse",
            Self::UnknownFunction { .. } => "xpath.unknown-function",
            Self::Arity { .. } => "xpath.arity",
            Self::UnboundPrefix { .. } => "xpath.unbound-prefix",
            Self::UnboundVariable { .. } => "xpath.unbound-variable",
            Self::Type { .. } => "xpath.type",
            Self::NotNodeSet { .. } => "xpath.not-node-set",
            Self::AttributeResult => "xpath.attribute-result",
            Self::UnsupportedAxis { .. } => "xpath.unsupported-axis",
            Self::Unsupported { .. } => "xpath.unsupported",
            Self::TooDeep { .. } => "xpath.too-deep",
            Self::StaleContext => "xpath.stale-context",
        }
    }

    /// The byte offset for errors found while parsing, `None` for evaluation errors.
    #[must_use]
    pub fn position(&self) -> Option<usize> {
        match self {
            Self::Parse { pos, .. }
            | Self::UnknownFunction { pos, .. }
            | Self::Arity { pos, .. }
            | Self::UnboundPrefix { pos, .. }
            | Self::UnboundVariable { pos, .. } => Some(*pos),
            _ => None,
        }
    }

    pub(crate) fn parse(pos: usize, message: impl Into<String>) -> Self {
        Self::Parse {
            pos,
            message: message.into(),
        }
    }

    pub(crate) fn type_error(message: impl Into<String>) -> Self {
        Self::Type {
            message: message.into(),
        }
    }
}

/// Result alias used across the crate.
pub type XPathResult<T> = Result<T, XPathError>;
