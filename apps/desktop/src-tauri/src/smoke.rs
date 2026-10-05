//! The headless self test behind `rimstudio --smoke`.
//!
//! It boots the application on the real platform roots (or under `RIMSTUDIO_DATA_BASE`) without
//! creating a window, runs a few registry commands and reports one line per step. The exit code is 0
//! when every step passed and 1 otherwise, so CI can run it on a machine without a display.

use std::process::ExitCode;
use std::time::Duration;

use rimstudio_app::AppContext;
use serde_json::{Value, json};

use crate::boot::boot_system;
use crate::commands::{call, commands};

/// The result of one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What was checked.
    pub name: &'static str,
    /// True when it passed.
    pub ok: bool,
    /// A short detail: the answer, or the reason it failed.
    pub detail: String,
}

fn step(name: &'static str, result: Result<String, String>) -> Step {
    match result {
        Ok(detail) => Step {
            name,
            ok: true,
            detail,
        },
        Err(detail) => Step {
            name,
            ok: false,
            detail,
        },
    }
}

fn query(ctx: &AppContext, name: &str, request: Value) -> Result<Value, String> {
    call(ctx, name, request, None, &mut |_| {})
        .map_err(|e| format!("{} ({}): {}", name, e.code, e.message))
}

/// Runs the steps against a booted context.
#[must_use]
pub fn run_steps(ctx: &AppContext) -> Vec<Step> {
    let ping = step(
        "app_ping",
        query(ctx, "app_ping", json!({"echo": "smoke"})).and_then(|v| {
            (v["echo"] == "smoke")
                .then(|| "echo returned".to_owned())
                .ok_or_else(|| format!("unexpected answer {v}"))
        }),
    );
    let report = step(
        "detect_get_report",
        query(ctx, "detect_get_report", json!({})).map(|v| {
            if v.get("report").is_some_and(|r| !r.is_null()) {
                "a detection report exists".to_owned()
            } else {
                "no detection report yet".to_owned()
            }
        }),
    );
    let list = commands();
    let rows = list.as_array().map_or(0, Vec::len);
    let has_ping = list
        .as_array()
        .is_some_and(|r| r.iter().any(|row| row["name"] == "app_ping"));
    let registry = step(
        "command list",
        if rows > 0 && has_ping {
            Ok(format!("{rows} commands"))
        } else {
            Err(format!("{rows} commands, app_ping listed: {has_ping}"))
        },
    );
    vec![
        Step {
            name: "boot",
            ok: true,
            detail: format!("data root {}", ctx.roots.data),
        },
        ping,
        report,
        registry,
    ]
}

/// Boots, runs the steps, prints one line each and returns the exit code.
#[must_use]
#[allow(clippy::print_stdout, clippy::print_stderr)]
pub fn run_cli() -> ExitCode {
    let ctx = match boot_system() {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("smoke: FAIL boot: {e}");
            return ExitCode::FAILURE;
        }
    };
    let steps = run_steps(&ctx);
    for s in &steps {
        println!(
            "smoke: {} {}: {}",
            if s.ok { "ok  " } else { "FAIL" },
            s.name,
            s.detail
        );
    }
    ctx.shutdown(Duration::from_secs(5));
    if steps.iter().all(|s| s.ok) {
        println!("smoke: passed");
        ExitCode::SUCCESS
    } else {
        println!("smoke: failed");
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use rimstudio_app::logging::LogConfig;
    use rimstudio_app::{BootInput, Platform, boot};

    use super::*;

    #[test]
    fn every_step_passes_on_a_fresh_data_base() {
        let dir = tempfile::tempdir().unwrap();
        let input = BootInput::new(Platform::system())
            .with_data_base(dir.path().to_str().unwrap())
            .with_log(LogConfig::off());
        let ctx = boot(input).unwrap();
        let steps = run_steps(&ctx);
        assert_eq!(steps.len(), 4);
        for s in &steps {
            assert!(s.ok, "{s:?}");
        }
        assert_eq!(steps[2].detail, "no detection report yet");
    }
}
