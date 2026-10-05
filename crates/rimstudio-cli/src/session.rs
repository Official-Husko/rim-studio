//! The session: boots the application once and runs registry commands for the CLI.
//!
//! The CLI boots the same [`AppContext`] as the desktop shell and calls the same handlers through
//! [`rimstudio_app::dispatch`]. A job is awaited here, with a progress line on standard error when that
//! is a terminal. Output goes through [`Session::emit`], which prints a JSON document in `--json` mode
//! and the human text otherwise.

use std::cell::Cell;
use std::io::{IsTerminal, Write as _};
use std::time::Duration;

use rimstudio_app::context::AppOptions;
use rimstudio_app::dispatch::dispatch_blocking_with;
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{AppContext, BootInput, boot};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::jobs::JobEvent;
use serde_json::Value;

use crate::error::{CliError, CliResult};

/// The answer of a command: the JSON result and the diagnostics a job reported with it.
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    /// The response of the command.
    pub(crate) value: Value,
    /// Content problems reported by a job next to its result, as JSON.
    pub(crate) diagnostics: Vec<Value>,
}

/// A booted application and the output settings of this run.
pub(crate) struct Session {
    /// The application context.
    pub(crate) app: AppContext,
    /// Print JSON documents instead of text.
    pub(crate) json: bool,
    progress: bool,
    warned: Cell<bool>,
}

/// The environment variable that names a def type table file (developer and test override).
pub(crate) const TYPE_TABLE_VAR: &str = "RIMSTUDIO_TYPE_TABLE";

/// The directory a relative path argument is resolved against.
///
/// # Errors
/// A usage error when the working directory is unknown.
pub(crate) fn absolute(path: &str) -> CliResult<String> {
    let given = std::path::Path::new(path);
    let full = if given.is_absolute() {
        given.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| CliError::usage(format!("the working directory is unknown: {e}")))?
            .join(given)
    };
    let mut clean = std::path::PathBuf::new();
    for part in full.components() {
        match part {
            std::path::Component::ParentDir => {
                clean.pop();
            }
            std::path::Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    clean
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| CliError::usage(format!("the path is not valid text: {path}")))
}

impl Session {
    /// Boots the application over the real platform.
    ///
    /// # Errors
    /// [`CliError::Failed`] when boot fails (no data folder, a folder that cannot be written).
    pub(crate) fn open(json: bool, no_progress: bool) -> CliResult<Self> {
        let mut log = LogConfig::file_only();
        log.stderr = std::env::var_os("RIMSTUDIO_LOG").is_some();
        let mut input = BootInput::system().with_log(log);
        // Developer override: a def type table in JSON replaces reading the game assemblies.
        if let Some(path) = std::env::var_os(TYPE_TABLE_VAR) {
            let text = std::fs::read_to_string(&path).map_err(|e| {
                CliError::usage(format!(
                    "{TYPE_TABLE_VAR} names a file that cannot be read: {e}"
                ))
            })?;
            let options = AppOptions::default()
                .with_type_table_json(&text)
                .map_err(|e| CliError::usage(format!("{TYPE_TABLE_VAR}: {e}")))?;
            input = input.with_options(options);
        }
        let app = boot(input)
            .map_err(|e| CliError::failed(format!("the application could not start: {e}")))?;
        Ok(Self {
            app,
            json,
            progress: !no_progress && std::io::stderr().is_terminal(),
            warned: Cell::new(false),
        })
    }

    /// Records that the run completed with warnings (exit code 3).
    pub(crate) fn note_warning(&self) {
        self.warned.set(true);
    }

    /// True when a warning was recorded.
    #[must_use]
    pub(crate) fn warned(&self) -> bool {
        self.warned.get()
    }

    /// Marks the session as ended cleanly.
    pub(crate) fn close(&self) {
        self.app.shutdown(Duration::from_secs(5));
    }

    /// Runs a command of the registry and waits for it, drawing progress for a job.
    ///
    /// # Errors
    /// The error envelope of the command.
    pub(crate) fn call(&self, name: &str, request: Value) -> Result<Reply, ApiError> {
        let show = self.progress;
        let mut diagnostics: Vec<Value> = Vec::new();
        let mut line_len = 0_usize;
        let result = dispatch_blocking_with(&self.app, name, request, &mut |event| match event {
            JobEvent::Progress { progress, .. } if show => {
                let mut text = progress.phase.clone();
                match progress.total {
                    Some(total) => text.push_str(&format!(" {}/{}", progress.done, total)),
                    None => text.push_str(&format!(" {}", progress.done)),
                }
                if let Some(detail) = &progress.detail {
                    text.push(' ');
                    text.push_str(detail);
                }
                let text: String = text.chars().take(78).collect();
                let pad = line_len.saturating_sub(text.chars().count());
                let mut err = std::io::stderr();
                let _ = write!(err, "\r{text}{}", " ".repeat(pad));
                let _ = err.flush();
                line_len = text.chars().count();
            }
            JobEvent::Finished { diagnostics: d, .. } => {
                diagnostics = d
                    .iter()
                    .filter_map(|x| serde_json::to_value(x).ok())
                    .collect();
            }
            _ => {}
        });
        if show && line_len > 0 {
            let mut err = std::io::stderr();
            let _ = write!(err, "\r{}\r", " ".repeat(line_len));
            let _ = err.flush();
        }
        result.map(|value| Reply { value, diagnostics })
    }

    /// Prints the result of a command: `document` in JSON mode, `text` otherwise.
    pub(crate) fn emit(&self, document: &Value, text: impl FnOnce() -> String) {
        if self.json {
            self.print_json(document);
        } else {
            let text = text();
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(text.as_bytes());
            if !text.ends_with('\n') {
                let _ = out.write_all(b"\n");
            }
        }
    }

    /// Prints a JSON document, pretty, followed by a newline.
    pub(crate) fn print_json(&self, document: &Value) {
        let text = serde_json::to_string_pretty(document).unwrap_or_else(|_| "null".to_owned());
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.write_all(b"\n");
    }

    /// Prints a line of human text to standard error (never in the data stream).
    pub(crate) fn note(&self, text: &str) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{text}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_are_made_absolute_and_cleaned() {
        let a = absolute("/x/y/../z/./w").unwrap_or_default();
        assert_eq!(a, "/x/z/w");
        let b = absolute("some/dir").unwrap_or_default();
        assert!(std::path::Path::new(&b).is_absolute(), "{b}");
        assert!(b.ends_with("some/dir"));
    }
}
