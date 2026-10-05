//! How the shell boots the application context.
//!
//! The data roots are the real platform folders (`rimstudio-platform` directories, or the portable
//! folders beside the executable). `RIMSTUDIO_DATA_BASE` puts all four roots under one folder, which
//! is what tests, smoke runs and side by side development use. Logging goes to the rolling file in the
//! logs root; `RIMSTUDIO_LOG` additionally writes to standard error.

use rimstudio_app::logging::{FILTER_VAR, LogConfig};
use rimstudio_app::{AppContext, BootError, BootInput, Platform, boot};

/// The environment variable that moves the four data roots under one folder.
pub const DATA_BASE_VAR: &str = "RIMSTUDIO_DATA_BASE";

/// The boot input of the shell for a given platform and an optional data base folder.
///
/// A base that is empty or only whitespace is ignored, so an unset and an empty variable behave the
/// same.
#[must_use]
pub fn boot_input(platform: Platform, data_base: Option<&str>, log_to_stderr: bool) -> BootInput {
    let mut log = LogConfig::file_only();
    log.stderr = log_to_stderr;
    let mut input = BootInput::new(platform).with_log(log);
    if let Some(base) = data_base.map(str::trim).filter(|b| !b.is_empty()) {
        input = input.with_data_base(base);
    }
    input
}

/// Boots the application on the real platform, reading `RIMSTUDIO_DATA_BASE` and `RIMSTUDIO_LOG`.
///
/// # Errors
/// [`BootError`] when no data folder can be determined or written.
pub fn boot_system() -> Result<AppContext, BootError> {
    let base = std::env::var(DATA_BASE_VAR).ok();
    let log_to_stderr = std::env::var_os(FILTER_VAR).is_some();
    boot(boot_input(
        Platform::system(),
        base.as_deref(),
        log_to_stderr,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_base_fixes_the_roots() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_str().unwrap();
        let input = boot_input(Platform::system(), Some(base), false);
        let roots = input.roots.unwrap();
        assert!(roots.data.as_str().starts_with(base));
        assert!(roots.logs.as_str().starts_with(base));
    }

    #[test]
    fn an_empty_base_is_ignored() {
        assert!(
            boot_input(Platform::system(), Some("  "), false)
                .roots
                .is_none()
        );
        assert!(boot_input(Platform::system(), None, false).roots.is_none());
    }

    #[test]
    fn logging_is_the_file_layer_and_optionally_stderr() {
        let log = boot_input(Platform::system(), None, true).log.unwrap();
        assert!(log.file && log.stderr && log.panic_hook);
        let quiet = boot_input(Platform::system(), None, false).log.unwrap();
        assert!(quiet.file && !quiet.stderr);
    }
}
