//! The Tauri application: builder, command wrappers and run loop.
//!
//! The wrappers do four things and nothing else (IPC and state, section 2.3): take the managed
//! [`Shell`], run the call on the blocking pool, forward job events to the webview and pass the
//! [`ApiError`] envelope through unchanged.

use std::process::ExitCode;
use std::time::Duration;

use rimstudio_ipc_types::error::{ApiError, codes};
use serde_json::Value;
use tauri::{AppHandle, Emitter as _, RunEvent, State, WebviewWindowBuilder};

use crate::boot::boot_system;
use crate::commands::{self, Shell};
use crate::events::JOB_EVENT;
use crate::nav::navigation_allowed;

/// The label of the one window.
pub const MAIN_WINDOW: &str = "main";

/// How long the exit waits for running jobs to stop.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

fn join_failure(error: &tauri::Error) -> ApiError {
    ApiError::new(
        codes::APP_INTERNAL_ERROR,
        format!("the command could not finish: {error}"),
    )
}

/// `rs_call(name, request, jobId?)`: runs a registry command. A job command resolves when the job
/// ends; its progress arrives as [`JOB_EVENT`] events meanwhile.
#[tauri::command]
async fn rs_call(
    app: AppHandle,
    shell: State<'_, Shell>,
    name: String,
    request: Option<Value>,
    job_id: Option<String>,
) -> Result<Value, ApiError> {
    let ctx = shell.app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        commands::call(
            &ctx,
            &name,
            request.unwrap_or(Value::Null),
            job_id.as_deref(),
            &mut |event| {
                // a closed webview is not an error for the job
                let _ = app.emit(JOB_EVENT, event);
            },
        )
    })
    .await
    .map_err(|e| join_failure(&e))?
}

/// `rs_cancel(jobId)`: cancels a running job; idempotent.
#[tauri::command]
async fn rs_cancel(shell: State<'_, Shell>, job_id: String) -> Result<Value, ApiError> {
    let ctx = shell.app.clone();
    tauri::async_runtime::spawn_blocking(move || commands::cancel(&ctx, &job_id))
        .await
        .map_err(|e| join_failure(&e))?
}

/// `rs_info()`: platform, data folder, command count and contract hash.
#[tauri::command]
fn rs_info(shell: State<'_, Shell>) -> Value {
    commands::info(&shell.app)
}

/// `rs_commands()`: every registry row with its kind.
#[tauri::command]
fn rs_commands() -> Value {
    commands::commands()
}

/// Creates the main window from the configuration with the navigation policy attached.
fn open_main_window(app: &tauri::App) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_WINDOW)
        .cloned()
        .ok_or_else(|| tauri::Error::WindowNotFound)?;
    let dev_origin = app.config().build.dev_url.clone();
    WebviewWindowBuilder::from_config(app.handle(), &config)?
        .on_navigation(move |url| {
            navigation_allowed(url, dev_origin.as_ref(), cfg!(debug_assertions))
        })
        .on_new_window(|_url, _features| tauri::webview::NewWindowResponse::Deny)
        .build()?;
    Ok(())
}

fn launch() -> Result<(), String> {
    let ctx = boot_system().map_err(|e| format!("the application could not start: {e}"))?;
    let shell = Shell { app: ctx.clone() };
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(shell)
        .invoke_handler(tauri::generate_handler![
            rs_call,
            rs_cancel,
            rs_info,
            rs_commands
        ])
        .setup(|app| Ok(open_main_window(app)?))
        .build(tauri::generate_context!())
        .map_err(|e| format!("the window system could not start: {e}"))?;
    built.run(move |_app, event| {
        if matches!(event, RunEvent::Exit) {
            ctx.shutdown(SHUTDOWN_WAIT);
        }
    });
    Ok(())
}

/// Runs the application, or the headless self test when the arguments contain `--smoke`.
#[must_use]
#[allow(clippy::print_stderr)]
pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    if args.any(|a| a == "--smoke") {
        return crate::smoke::run_cli();
    }
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("rimstudio: {message}");
            ExitCode::FAILURE
        }
    }
}
