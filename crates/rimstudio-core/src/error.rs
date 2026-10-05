//! The error type of `rimstudio-core`.
//!
//! `CoreError` reports misuse of the API (an id that does not parse, a version string that is not a
//! version, a path that cannot be normalised). Problems in user content are never errors: they are
//! [`crate::diag::Diagnostic`] values (invariant I-10). Every variant has a stable code, a lowercase
//! `<area>.<kebab-name>` string that callers can match on and that the UI translates.

use thiserror::Error;

/// Longest value echoed back inside an error, so a pathological input cannot bloat a log line.
const ECHO_LIMIT: usize = 80;

/// Shortens `value` to at most [`ECHO_LIMIT`] characters, marking the cut with three dots.
pub(crate) fn echo(value: &str) -> String {
    if value.chars().count() <= ECHO_LIMIT {
        return value.to_owned();
    }
    let mut out: String = value.chars().take(ECHO_LIMIT).collect();
    out.push_str("...");
    out
}

/// Why an identifier was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IdFault {
    /// The text is empty (after trimming where the id type trims).
    #[error("it is empty")]
    Empty,
    /// The text is longer than the id type allows.
    #[error("it is longer than {max} characters")]
    TooLong {
        /// The maximum accepted length in characters.
        max: usize,
    },
    /// The text contains a character the id type does not allow.
    #[error("it contains the character {ch:?} at position {index}")]
    BadChar {
        /// The offending character.
        ch: char,
        /// Zero based character position of the offending character.
        index: usize,
    },
    /// A required prefix or separator is missing.
    #[error("it does not have the required shape")]
    BadShape,
    /// The text should be a decimal number and is not.
    #[error("it is not a decimal number")]
    NotANumber,
    /// The number is zero or does not fit the integer type.
    #[error("the number is zero or out of range")]
    OutOfRange,
}

/// Why a version string was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VersionFault {
    /// The text is empty.
    #[error("it is empty")]
    Empty,
    /// Fewer than two dotted numeric components (a version needs at least major and minor).
    #[error("it needs at least a major and a minor number")]
    TooFewParts,
    /// More than four dotted numeric components.
    #[error("it has more than four numeric components")]
    TooManyParts,
    /// A component is not a non negative decimal number that fits in 32 bits.
    #[error("a component is not a non negative number")]
    BadNumber,
    /// Text after the numeric part that is not a revision marker such as `rev598`.
    #[error("unexpected text after the numeric part")]
    UnexpectedSuffix,
}

/// Why a path text was rejected by the pure path helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PathFault {
    /// The text is empty.
    #[error("it is empty")]
    Empty,
    /// The path is not absolute.
    #[error("it is not an absolute path")]
    Relative,
    /// The path contains a NUL or another control character.
    #[error("it contains a control character")]
    ControlChar,
    /// A `..` segment would climb above the root.
    #[error("it climbs above the root")]
    EscapesRoot,
}

/// Everything that can go wrong inside `rimstudio-core`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// An identifier did not parse or validate.
    #[error("invalid {kind} id {value:?}: {fault}")]
    InvalidId {
        /// The id type, for example `package`, `workshop`, `mod`, `source`.
        kind: &'static str,
        /// The rejected text (truncated).
        value: String,
        /// What is wrong with it.
        fault: IdFault,
    },
    /// A version string did not parse.
    #[error("invalid version {input:?}: {fault}")]
    InvalidVersion {
        /// The rejected text (truncated).
        input: String,
        /// What is wrong with it.
        fault: VersionFault,
    },
    /// A `LoadFolders` specification is structurally invalid.
    #[error("invalid load folders specification: {reason}")]
    InvalidLoadFolders {
        /// A short developer facing reason.
        reason: String,
    },
    /// A path text could not be normalised.
    #[error("invalid path {path:?}: {fault}")]
    InvalidPath {
        /// The rejected text (truncated).
        path: String,
        /// What is wrong with it.
        fault: PathFault,
    },
    /// A settings value is outside its allowed range or shape.
    #[error("invalid setting {field}: {reason}")]
    InvalidSettings {
        /// Dotted key path of the setting, for example `library.scanThreads`.
        field: String,
        /// A short developer facing reason.
        reason: String,
    },
    /// A node tree operation was invalid (used by `rimstudio_core::tree`).
    #[error("invalid node tree operation: {reason}")]
    InvalidTree {
        /// A short developer facing reason.
        reason: String,
    },
    /// An implementation of a port broke the documented contract of the trait.
    #[error("port contract violation in {port}: {reason}")]
    PortContract {
        /// The port trait name.
        port: &'static str,
        /// A short developer facing reason.
        reason: String,
    },
    /// The operation observed its cancel token and stopped. Callers treat this as a normal end.
    #[error("the operation was cancelled")]
    Cancelled,
}

impl CoreError {
    /// Every code this enum can produce, in a stable order. Used by the code mapping tests.
    pub const ALL_CODES: [&'static str; 8] = [
        "core.invalid-id",
        "core.invalid-version",
        "core.invalid-load-folders",
        "core.invalid-path",
        "core.invalid-settings",
        "core.invalid-tree",
        "core.port-contract",
        "job.cancelled",
    ];

    /// The stable machine readable code of this error, `<area>.<kebab-name>`.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::InvalidId { .. } => "core.invalid-id",
            CoreError::InvalidVersion { .. } => "core.invalid-version",
            CoreError::InvalidLoadFolders { .. } => "core.invalid-load-folders",
            CoreError::InvalidPath { .. } => "core.invalid-path",
            CoreError::InvalidSettings { .. } => "core.invalid-settings",
            CoreError::InvalidTree { .. } => "core.invalid-tree",
            CoreError::PortContract { .. } => "core.port-contract",
            CoreError::Cancelled => "job.cancelled",
        }
    }

    /// Builds an [`CoreError::InvalidId`], truncating the echoed value.
    pub fn invalid_id(kind: &'static str, value: &str, fault: IdFault) -> Self {
        CoreError::InvalidId {
            kind,
            value: echo(value),
            fault,
        }
    }

    /// Builds an [`CoreError::InvalidVersion`], truncating the echoed value.
    pub fn invalid_version(input: &str, fault: VersionFault) -> Self {
        CoreError::InvalidVersion {
            input: echo(input),
            fault,
        }
    }

    /// Builds an [`CoreError::InvalidPath`], truncating the echoed value.
    pub fn invalid_path(path: &str, fault: PathFault) -> Self {
        CoreError::InvalidPath {
            path: echo(path),
            fault,
        }
    }

    /// Builds an [`CoreError::InvalidSettings`].
    pub fn invalid_settings(field: impl Into<String>, reason: impl Into<String>) -> Self {
        CoreError::InvalidSettings {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// True when the error means "stopped on request" rather than a failure.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, CoreError::Cancelled)
    }
}

/// Result alias used across the crate.
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn samples() -> Vec<CoreError> {
        vec![
            CoreError::invalid_id("package", "", IdFault::Empty),
            CoreError::invalid_version("x", VersionFault::TooFewParts),
            CoreError::InvalidLoadFolders { reason: "r".into() },
            CoreError::invalid_path("rel", PathFault::Relative),
            CoreError::invalid_settings("a.b", "bad"),
            CoreError::InvalidTree { reason: "r".into() },
            CoreError::PortContract {
                port: "Clock",
                reason: "r".into(),
            },
            CoreError::Cancelled,
        ]
    }

    #[test]
    fn every_variant_has_its_listed_code_in_order() {
        let codes: Vec<&str> = samples().iter().map(CoreError::code).collect();
        assert_eq!(codes, CoreError::ALL_CODES.to_vec());
    }

    #[test]
    fn codes_follow_the_area_dot_kebab_grammar() {
        for code in CoreError::ALL_CODES {
            let (area, name) = code.split_once('.').expect("one dot");
            assert!(!area.is_empty() && !name.is_empty());
            assert!(
                code.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'-')
            );
        }
    }

    #[rstest]
    #[case("short", "short")]
    #[case(&"a".repeat(80), &"a".repeat(80))]
    fn echo_keeps_short_values(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(echo(input), expected);
    }

    #[test]
    fn echo_truncates_long_values_on_char_boundaries() {
        let long = "é".repeat(200);
        let out = echo(&long);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), ECHO_LIMIT + 3);
    }

    #[test]
    fn cancelled_is_detected() {
        assert!(CoreError::Cancelled.is_cancelled());
        assert!(!CoreError::invalid_settings("a", "b").is_cancelled());
    }
}
