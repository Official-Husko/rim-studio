//! The error type of `rimstudio-platform` and conversions from `std::io::Error`.

use rimstudio_core::ports::{PortError, PortErrorKind};
use thiserror::Error;

/// Errors raised by the pure helpers of this crate. Port implementations return
/// [`PortError`] instead; [`PlatformError`] converts into it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlatformError {
    /// A request was malformed (an empty program, a URL that is not allowed, a bad variable name).
    #[error("platform.invalid: {field}: {reason}")]
    Invalid {
        /// The field or argument that is wrong.
        field: &'static str,
        /// What is wrong with it.
        reason: String,
    },
    /// A port level failure.
    #[error("{0}")]
    Port(#[from] PortError),
}

/// Result alias for this crate.
pub type PlatformResult<T> = Result<T, PlatformError>;

impl PlatformError {
    /// Builds an [`PlatformError::Invalid`].
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        PlatformError::Invalid {
            field,
            reason: reason.into(),
        }
    }

    /// The stable code: `platform.invalid` or the `port.*` code of the wrapped error.
    pub fn code(&self) -> &'static str {
        match self {
            PlatformError::Invalid { .. } => "platform.invalid",
            PlatformError::Port(e) => e.code(),
        }
    }
}

impl From<PlatformError> for PortError {
    fn from(e: PlatformError) -> Self {
        match e {
            PlatformError::Port(p) => p,
            PlatformError::Invalid { field, reason } => {
                PortError::new(PortErrorKind::Io, format!("invalid {field}: {reason}"))
            }
        }
    }
}

/// Maps an IO error to a [`PortError`], keeping the class and a short message.
pub(crate) fn io_error(context: &str, e: &std::io::Error) -> PortError {
    use std::io::ErrorKind;
    let kind = match e.kind() {
        ErrorKind::NotFound => PortErrorKind::NotFound,
        ErrorKind::PermissionDenied => PortErrorKind::PermissionDenied,
        ErrorKind::TimedOut => PortErrorKind::TimedOut,
        ErrorKind::Unsupported => PortErrorKind::Unsupported,
        _ => PortErrorKind::Io,
    };
    PortError::new(kind, format!("{context}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(
            PlatformError::invalid("url", "x").code(),
            "platform.invalid"
        );
        let p = PortError::new(PortErrorKind::NotFound, "n");
        assert_eq!(PlatformError::from(p).code(), "port.not-found");
    }

    #[test]
    fn io_errors_keep_their_class() {
        let e = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(io_error("x", &e).kind, PortErrorKind::PermissionDenied);
        let e = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(io_error("x", &e).kind, PortErrorKind::NotFound);
        let e = std::io::Error::other("boom");
        assert_eq!(io_error("x", &e).kind, PortErrorKind::Io);
    }

    #[test]
    fn invalid_converts_to_a_port_error() {
        let p: PortError = PlatformError::invalid("program", "empty").into();
        assert_eq!(p.kind, PortErrorKind::Io);
        assert!(p.message.contains("program"));
    }
}
