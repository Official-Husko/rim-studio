//! The error type of the command line front end and its mapping to exit codes.
//!
//! Library crates below return typed errors; the CLI is the top of the stack, so it keeps one small
//! enum that knows how to print itself for people and as a JSON envelope for scripts.

use std::fmt::Write as _;

use rimstudio_ipc_types::error::ApiError;
use serde_json::{Value, json};

/// Exit code of a successful run.
pub(crate) const EXIT_OK: u8 = 0;
/// Exit code of a failed run.
pub(crate) const EXIT_ERROR: u8 = 1;
/// Exit code of a usage error.
pub(crate) const EXIT_USAGE: u8 = 2;
/// Exit code of a run that completed but reported warnings.
pub(crate) const EXIT_WARNINGS: u8 = 3;

/// Why a command did not complete.
#[derive(Debug)]
pub(crate) enum CliError {
    /// A command of the registry answered with an error envelope.
    Api(ApiError),
    /// The arguments are wrong in a way clap cannot see; exit code 2.
    Usage(String),
    /// The command ran but its result is a failure (for example a plan with errors); exit code 1.
    Failed(String),
    /// The failure was already printed in full; only the exit code is left.
    Reported,
}

impl From<ApiError> for CliError {
    fn from(value: ApiError) -> Self {
        Self::Api(value)
    }
}

/// The result type of every command.
pub(crate) type CliResult<T = ()> = Result<T, CliError>;

impl CliError {
    /// A usage error.
    pub(crate) fn usage(text: impl Into<String>) -> Self {
        Self::Usage(text.into())
    }

    /// A failed run with a message.
    pub(crate) fn failed(text: impl Into<String>) -> Self {
        Self::Failed(text.into())
    }

    /// The process exit code of this error.
    #[must_use]
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Api(_) | Self::Failed(_) | Self::Reported => EXIT_ERROR,
        }
    }

    /// The stable code of this error (the registry code for an envelope, `cli.*` otherwise).
    #[must_use]
    pub(crate) fn code(&self) -> &str {
        match self {
            Self::Api(e) => &e.code,
            Self::Usage(_) => "cli.usage",
            Self::Failed(_) => "cli.failed",
            Self::Reported => "cli.reported",
        }
    }

    /// The JSON envelope printed in `--json` mode: `{"error": {code, message, ...}}`.
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Api(e) => json!({"error": serde_json::to_value(e).unwrap_or(Value::Null)}),
            Self::Usage(m) | Self::Failed(m) => {
                json!({"error": {"code": self.code(), "message": m}})
            }
            Self::Reported => {
                json!({"error": {"code": self.code(), "message": "see the output above"}})
            }
        }
    }

    /// The text printed for people. Empty for an error that was already reported.
    #[must_use]
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Api(e) => render_api(e),
            Self::Usage(m) => format!("usage error: {m}\n"),
            Self::Failed(m) => format!("error: {m}\n"),
            Self::Reported => String::new(),
        }
    }
}

fn render_api(e: &ApiError) -> String {
    let mut out = format!("error: {} ({}): {}\n", e.code, e.error_id, e.message);
    if let Some(details) = &e.details {
        for (key, value) in details.iter() {
            let text = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let _ = writeln!(out, "  {key}: {text}");
        }
    }
    if let Some(hint) = hint_for(&e.code) {
        let _ = writeln!(out, "hint: {hint}");
    }
    out
}

/// A one line suggestion for the codes users meet first.
fn hint_for(code: &str) -> Option<&'static str> {
    match code {
        "designer.reference-unavailable" => Some(
            "no game install is selected or the library has not been scanned; run `rimstudio-cli detect --install PATH` and `rimstudio-cli scan`",
        ),
        "library.scan-failed" => Some(
            "no mod source is known; run `rimstudio-cli detect` or add a folder with `rimstudio-cli sources add PATH`",
        ),
        "io.not-found" => {
            Some("check that the folder exists and is a mod folder (it has About/About.xml)")
        }
        "designer.plan-stale" => {
            Some("the project changed since the plan was made; run the command again")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_contract() {
        assert_eq!(CliError::usage("x").exit_code(), 2);
        assert_eq!(CliError::failed("x").exit_code(), 1);
        assert_eq!(CliError::Reported.exit_code(), 1);
        assert_eq!(
            CliError::Api(ApiError::new("io.not-found", "x")).exit_code(),
            1
        );
    }

    #[test]
    fn an_envelope_renders_code_id_message_and_hint() {
        let e = ApiError::with_id("designer.reference-unavailable", "no install", "e-00000001")
            .detail("why", "none");
        let text = CliError::Api(e).render();
        assert!(
            text.contains("designer.reference-unavailable (e-00000001): no install"),
            "{text}"
        );
        assert!(text.contains("why: none"));
        assert!(text.contains("hint:"));
    }

    #[test]
    fn the_json_envelope_always_has_a_code() {
        let v = CliError::usage("bad").to_json();
        assert_eq!(v["error"]["code"], json!("cli.usage"));
        let v = CliError::Api(ApiError::with_id("io.not-found", "m", "e-1")).to_json();
        assert_eq!(v["error"]["code"], json!("io.not-found"));
        assert_eq!(v["error"]["errorId"], json!("e-1"));
    }
}
