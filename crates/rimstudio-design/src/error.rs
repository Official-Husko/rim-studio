//! Typed errors of the design crate.
//!
//! Every error carries a stable machine readable code from [`DesignError::code`]. The pure math in this crate
//! never returns `NaN` or infinity: a non finite or out of range input is reported as
//! [`DesignError::InvalidInput`] instead, so callers can show the problem next to the offending field.

use thiserror::Error;

/// Result alias used by every fallible function of the crate.
pub type DesignResult<T> = Result<T, DesignError>;

/// Error produced by the design math.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DesignError {
    /// An input value is not finite, is negative where it must not be, or is otherwise out of range.
    #[error("invalid input `{field}`: {reason}")]
    InvalidInput {
        /// Name of the offending field, as written in the public API.
        field: &'static str,
        /// Short human readable reason.
        reason: String,
    },
    /// The inputs are valid on their own but describe a degenerate case that has no defined result
    /// (for example a weapon whose full cycle takes no time).
    #[error("degenerate input: {what}")]
    Degenerate {
        /// What is degenerate.
        what: &'static str,
    },
    /// A list that must not be empty is empty (no tools, no presets, no layers where one is required).
    #[error("empty input: {what}")]
    EmptyInput {
        /// Which list is empty.
        what: &'static str,
    },
}

impl DesignError {
    /// Stable code string for logs, diagnostics and the IPC error envelope.
    ///
    /// The codes are `design.invalid-input`, `design.degenerate` and `design.empty-input`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "design.invalid-input",
            Self::Degenerate { .. } => "design.degenerate",
            Self::EmptyInput { .. } => "design.empty-input",
        }
    }

    /// Builds an [`DesignError::InvalidInput`].
    #[must_use]
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            field,
            reason: reason.into(),
        }
    }
}

/// Returns `value` when it is finite, otherwise an invalid input error naming `field`.
pub(crate) fn finite(field: &'static str, value: f64) -> DesignResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(DesignError::invalid(field, "must be a finite number"))
    }
}

/// Returns `value` when it is finite and at least zero.
pub(crate) fn non_negative(field: &'static str, value: f64) -> DesignResult<f64> {
    let value = finite(field, value)?;
    if value < 0.0 {
        Err(DesignError::invalid(field, "must not be negative"))
    } else {
        Ok(value)
    }
}

/// Returns `value` when it is finite and strictly greater than zero.
pub(crate) fn positive(field: &'static str, value: f64) -> DesignResult<f64> {
    let value = finite(field, value)?;
    if value <= 0.0 {
        Err(DesignError::invalid(field, "must be greater than zero"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(DesignError::invalid("x", "bad"), "design.invalid-input")]
    #[case(DesignError::Degenerate { what: "cycle" }, "design.degenerate")]
    #[case(DesignError::EmptyInput { what: "tools" }, "design.empty-input")]
    fn codes_are_stable(#[case] error: DesignError, #[case] code: &str) {
        assert_eq!(error.code(), code);
    }

    #[test]
    fn messages_name_the_field() {
        let message = DesignError::invalid("damage", "must not be negative").to_string();
        assert!(message.contains("damage"));
    }

    #[rstest]
    #[case(f64::NAN)]
    #[case(f64::INFINITY)]
    #[case(f64::NEG_INFINITY)]
    fn finite_rejects_non_finite(#[case] value: f64) {
        assert!(finite("v", value).is_err());
        assert!(non_negative("v", value).is_err());
        assert!(positive("v", value).is_err());
    }

    #[test]
    fn sign_checks() {
        assert_eq!(non_negative("v", 0.0), Ok(0.0));
        assert!(non_negative("v", -0.1).is_err());
        assert!(positive("v", 0.0).is_err());
        assert_eq!(positive("v", 0.1), Ok(0.1));
    }
}
