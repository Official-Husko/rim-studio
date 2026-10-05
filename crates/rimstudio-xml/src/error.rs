//! Typed errors of the XML boundary and the stable diagnostic codes it emits.
//!
//! Hard failures (a file the game would skip, an edit that cannot be applied) are [`XmlError`]
//! values with a stable [`XmlError::code`]. Problems in user content that do not stop the work are
//! reported as [`Diagnostic`]s whose codes are the constants in [`codes`].

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity, Span};

/// Every way an XML operation can fail. The [`XmlError::code`] strings are stable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum XmlError {
    /// The text is not well formed XML (the game would log a warning and skip the file).
    #[error("XML parse error at line {line}, column {column}: {message}")]
    Parse {
        /// What the parser objected to.
        message: String,
        /// One based line of the problem.
        line: u32,
        /// One based column of the problem.
        column: u32,
        /// Byte offset of the problem into the input (after any byte order mark).
        offset: usize,
    },
    /// The document has a document type declaration, which the game's loader refuses.
    #[error("a document type declaration is not allowed (line {line})")]
    DtdRejected {
        /// One based line of the declaration.
        line: u32,
    },
    /// The input holds no root element at all.
    #[error("the document has no root element")]
    NoRoot,
    /// The element nesting is deeper than the supported maximum.
    #[error("elements are nested deeper than {max} levels")]
    TooDeep {
        /// The limit.
        max: usize,
    },
    /// An edit path names no element of the document.
    #[error("no element matches the path {path}")]
    EditPathMissing {
        /// The path as given.
        path: String,
    },
    /// An edit path is not valid path syntax.
    #[error("invalid edit path {path:?}: {reason}")]
    EditPathInvalid {
        /// The path as given.
        path: String,
        /// Why it was rejected.
        reason: String,
    },
    /// The edit cannot be applied to the element it targets.
    #[error("unsupported edit at {path}: {reason}")]
    EditUnsupported {
        /// The path of the target.
        path: String,
        /// Why the edit does not apply.
        reason: String,
    },
    /// A value cannot be written (for example an attribute name that is not an XML name).
    #[error("invalid value for {what}: {reason}")]
    InvalidValue {
        /// What was being written.
        what: String,
        /// Why it was rejected.
        reason: String,
    },
    /// Reading from a stream failed.
    #[error("read error: {message}")]
    Io {
        /// The operating system message.
        message: String,
    },
}

impl XmlError {
    /// The stable, machine readable code of this error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            XmlError::Parse { .. } => "xml.parse",
            XmlError::DtdRejected { .. } => "xml.dtd-rejected",
            XmlError::NoRoot => "xml.no-root",
            XmlError::TooDeep { .. } => "xml.too-deep",
            XmlError::EditPathMissing { .. } => "xml.edit-path-missing",
            XmlError::EditPathInvalid { .. } => "xml.edit-path-invalid",
            XmlError::EditUnsupported { .. } => "xml.edit-unsupported",
            XmlError::InvalidValue { .. } => "xml.invalid-value",
            XmlError::Io { .. } => "xml.io",
        }
    }

    /// The error as a diagnostic (severity error), with a line and column span when known.
    #[must_use]
    pub fn to_diagnostic(&self) -> Diagnostic {
        let code = DiagCode::new(match self {
            XmlError::Parse { .. } => codes::PARSE_ERROR_STR,
            XmlError::DtdRejected { .. } => codes::DTD_STR,
            XmlError::NoRoot => codes::NO_ROOT_STR,
            _ => codes::OTHER_STR,
        });
        let mut d = Diagnostic::new(code, Severity::Error, self.to_string());
        match self {
            XmlError::Parse { line, column, .. } => {
                d = d.with_span(Span::LineCol {
                    line: *line,
                    column: *column,
                });
            }
            XmlError::DtdRejected { line } => {
                d = d.with_span(Span::LineCol {
                    line: *line,
                    column: 1,
                });
            }
            _ => {}
        }
        d
    }
}

/// Result alias of the boundary crate.
pub type XmlResult<T> = Result<T, XmlError>;

/// Stable diagnostic codes emitted by this crate (`xml.*` and `about.*` and `loadfolders.*`).
pub mod codes {
    use super::DiagCode;

    pub(super) const PARSE_ERROR_STR: &str = "xml.parse-error";
    pub(super) const DTD_STR: &str = "xml.dtd-rejected";
    pub(super) const NO_ROOT_STR: &str = "xml.no-root";
    pub(super) const OTHER_STR: &str = "xml.error";

    /// The file is not well formed; the tolerant reader stopped there and kept what it had.
    pub const PARSE_RECOVERED: DiagCode = DiagCode::new("xml.parse-recovered");
    /// The input is not valid UTF-8; invalid sequences were replaced, as the game does.
    pub const INVALID_UTF8: DiagCode = DiagCode::new("xml.invalid-utf8");
    /// A closing tag did not match the open element.
    pub const MISMATCHED_END: DiagCode = DiagCode::new("xml.mismatched-end");
    /// Elements were still open at the end of the input and were closed implicitly.
    pub const UNCLOSED: DiagCode = DiagCode::new("xml.unclosed");
    /// An entity reference that XML does not define (`&nbsp;`) was kept as literal text.
    pub const UNDEFINED_ENTITY: DiagCode = DiagCode::new("xml.undefined-entity");
    /// A document type declaration was ignored.
    pub const DTD_IGNORED: DiagCode = DiagCode::new("xml.dtd-ignored");
    /// An attribute was malformed or repeated and was skipped.
    pub const BAD_ATTRIBUTE: DiagCode = DiagCode::new("xml.bad-attribute");
    /// Content outside the root element was ignored.
    pub const EXTRA_CONTENT: DiagCode = DiagCode::new("xml.extra-content");
    /// A second root element was ignored.
    pub const EXTRA_ROOT: DiagCode = DiagCode::new("xml.extra-root");
    /// The root element of a Defs or Patch file has an unexpected name.
    pub const UNEXPECTED_ROOT: DiagCode = DiagCode::new("xml.unexpected-root");
    /// A name prefix (`p:name`) is used without a declaration.
    pub const UNDECLARED_PREFIX: DiagCode = DiagCode::new("xml.undeclared-prefix");

    /// An About element name differs from the field name only in letter case; the game ignores it.
    pub const ABOUT_TAG_CASE: DiagCode = DiagCode::new("about.tag-case");
    /// An About element is not a field the game reads.
    pub const ABOUT_UNKNOWN_TAG: DiagCode = DiagCode::new("about.unknown-tag");
    /// A text field holds a comment or child elements, so the game drops its value.
    pub const ABOUT_FIELD_DROPPED: DiagCode = DiagCode::new("about.field-dropped");
    /// The same About field appears twice.
    pub const ABOUT_DUPLICATE: DiagCode = DiagCode::new("about.duplicate-field");
    /// `supportedVersions` is missing.
    pub const ABOUT_NO_VERSIONS: DiagCode = DiagCode::new("about.no-supported-versions");
    /// A ByVersion block repeats a version key; the first one wins.
    pub const ABOUT_DUPLICATE_VERSION: DiagCode = DiagCode::new("about.duplicate-version-key");
    /// A dependency entry has no package id.
    pub const ABOUT_DEPENDENCY_NO_ID: DiagCode = DiagCode::new("about.dependency-no-id");

    /// A LoadFolders entry carries an attribute the game does not read.
    pub const LOADFOLDERS_IGNORED_ATTR: DiagCode = DiagCode::new("loadfolders.ignored-attribute");
}
