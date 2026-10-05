//! The error type of the crate.
//!
//! Detection itself never fails: problems become [`crate::report::Warning`] values in the report.
//! [`SteamError`] covers the places where a caller asks for one specific thing (parse a file, read a
//! typed view) and a failure is the answer.

use rimstudio_core::ports::PortError;
use thiserror::Error;

use crate::vdf::VdfError;

/// Result alias of the crate.
pub type SteamResult<T> = Result<T, SteamError>;

/// An error of the VDF reader, the ACF views or a probe.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SteamError {
    /// The text is not valid KeyValues1.
    #[error(transparent)]
    Vdf(#[from] VdfError),
    /// The text parsed but does not have the expected shape.
    #[error("unexpected {file} shape: {reason}")]
    Shape {
        /// Which kind of file was expected, for example `libraryfolders`.
        file: &'static str,
        /// A short developer facing reason.
        reason: String,
    },
    /// A file system probe failed.
    #[error(transparent)]
    Port(#[from] PortError),
}

impl SteamError {
    /// Builds a [`SteamError::Shape`].
    pub fn shape(file: &'static str, reason: impl Into<String>) -> Self {
        SteamError::Shape {
            file,
            reason: reason.into(),
        }
    }

    /// The stable code of the error: `steam.vdf-<kind>`, `steam.shape` or a `port.*` code.
    pub fn code(&self) -> String {
        match self {
            SteamError::Vdf(e) => format!("steam.vdf-{}", e.kind.name()),
            SteamError::Shape { .. } => "steam.shape".to_owned(),
            SteamError::Port(e) => e.code().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ports::PortErrorKind;

    #[test]
    fn codes_are_stable() {
        assert_eq!(SteamError::shape("x", "y").code(), "steam.shape");
        let port = SteamError::from(PortError::new(PortErrorKind::NotFound, "gone"));
        assert_eq!(port.code(), "port.not-found");
    }
}
