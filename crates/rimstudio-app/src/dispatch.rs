//! Dispatch: the one door every client uses to run a command by name.
//!
//! [`dispatch`] takes a command name and a JSON request and returns a JSON response or an
//! [`ApiError`]. It is what the CLI, the tests and (through the typed wrappers of the registry) the
//! shell run, so all of them execute the same handlers. Request shape errors name the offending field,
//! a panic in a handler is caught per call and becomes `app.internal-error`, and every failure is
//! logged once, here, with its error id.
//!
//! A `job` command returns a job handle at once and runs on the [`crate::jobs::JobRunner`]:
//! [`dispatch_job`] takes the caller's job id and an event sink, [`dispatch`] mints an id and keeps the
//! outcome in the job registry (`job_status` reads it), and [`dispatch_blocking`] waits for the
//! terminal event and returns the result, which is what the CLI and the tests want.
//! [`dispatch_to_file`] runs a command and writes its result through `rimstudio-io`.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use rimstudio_core::redact::Redactor;
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_ipc_types::jobs::{JobEvent, JobHandleDto};
use serde_json::Value;

use crate::context::AppContext;
use crate::error::{AppError, api_from_store, panic_message};
use crate::jobs::{JobSink, JobSpec, NullSink, channel};
use crate::registry::{self, Route, Runner};

/// What [`dispatch_to_file`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    /// The file that was written.
    pub path: Utf8PathBuf,
    /// Its size in bytes.
    pub bytes: usize,
}

fn log_failure(redactor: &Redactor, command: &str, error: &ApiError) {
    if error.is_cancelled() {
        tracing::info!(command, "command cancelled");
        return;
    }
    let message = redactor.redact(&error.message);
    tracing::error!(command, code = %error.code, error_id = %error.error_id, "command failed: {message}");
}

/// Runs a handler body, turning a panic into `app.internal-error` and logging every failure once.
///
/// The envelope of a panic carries the redacted panic text; the original payload stays in the log
/// line only.
pub fn catch_handler(
    redactor: &Redactor,
    command: &str,
    body: impl FnOnce() -> Result<Value, ApiError>,
) -> Result<Value, ApiError> {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            log_failure(redactor, command, &error);
            Err(error)
        }
        Err(payload) => {
            let message = redactor.redact(&panic_message(payload.as_ref()));
            let error = ApiError::new(
                codes::APP_INTERNAL_ERROR,
                format!("the handler of {command} panicked: {message}"),
            );
            log_failure(redactor, command, &error);
            Err(error)
        }
    }
}

fn route_of(ctx: &AppContext, name: &str) -> Result<&'static Route, ApiError> {
    registry::find(name).ok_or_else(|| {
        let error = AppError::UnknownCommand {
            name: name.to_owned(),
        }
        .to_api();
        log_failure(&ctx.redactor, name, &error);
        error
    })
}

fn run_sync(
    ctx: &AppContext,
    route: &Route,
    run: fn(&AppContext, Value) -> Result<Value, ApiError>,
    json: Value,
) -> Result<Value, ApiError> {
    let span = tracing::info_span!("cmd", name = route.name);
    let _entered = span.enter();
    catch_handler(&ctx.redactor, route.name, || run(ctx, json))
}

fn take_job_id(json: &mut Value) -> Option<String> {
    json.as_object_mut()
        .and_then(|o| o.remove("jobId"))
        .and_then(|v| v.as_str().map(str::to_owned))
}

/// Starts a job command under the caller's job id and sends its events to `sink`.
///
/// The request is decoded before the job is registered, so a bad request fails here and no job exists.
/// When `job_id` is `None`, a `jobId` member of the request object is used, else an id is minted.
///
/// # Errors
/// `ipc.unknown-command`, `ipc.invalid-request` (also for a command that is not a job), and
/// `job.duplicate-id`.
pub fn dispatch_job(
    ctx: &AppContext,
    name: &str,
    mut json: Value,
    job_id: Option<String>,
    sink: Arc<dyn JobSink>,
) -> Result<JobHandleDto, ApiError> {
    let route = route_of(ctx, name)?;
    let Runner::Job(prepare) = route.runner else {
        let error = AppError::invalid(name, "command", "the command is not a job").to_api();
        log_failure(&ctx.redactor, name, &error);
        return Err(error);
    };
    let embedded = take_job_id(&mut json);
    let id = job_id.or(embedded).unwrap_or_else(|| ctx.jobs.mint_id());
    let work = prepare(ctx, json).inspect_err(|e| log_failure(&ctx.redactor, name, e))?;
    let span = tracing::info_span!("job", id = %id, kind = route.name);
    let work: crate::jobs::JobWork = Box::new(move |job| {
        let _entered = span.enter();
        work(job)
    });
    let spec = JobSpec::new(id, format!("job.{}", route.name));
    ctx.jobs
        .spawn(spec, sink, work)
        .inspect_err(|e| log_failure(&ctx.redactor, name, e))
}

/// Runs a command by name. A query or action returns its response; a job returns its handle
/// (`{jobId, startedAtMs}`) and its outcome stays in the job registry.
///
/// # Errors
/// The envelope of whatever stopped the command.
pub fn dispatch(ctx: &AppContext, name: &str, json: Value) -> Result<Value, ApiError> {
    let route = route_of(ctx, name)?;
    match route.runner {
        Runner::Sync(run) => run_sync(ctx, route, run, json),
        Runner::Job(_) => {
            let handle = dispatch_job(ctx, name, json, None, Arc::new(NullSink))?;
            registry::encode_response(name, &handle)
        }
    }
}

/// Runs a command and waits for its result; a job is awaited until its terminal event.
///
/// `observer` sees every job event (started, progress, terminal) as it arrives, which is how the CLI
/// draws its progress line. A cancelled job answers `job.cancelled`.
///
/// # Errors
/// The envelope of whatever stopped the command.
pub fn dispatch_blocking_with(
    ctx: &AppContext,
    name: &str,
    json: Value,
    observer: &mut dyn FnMut(&JobEvent<Value>),
) -> Result<Value, ApiError> {
    let route = route_of(ctx, name)?;
    match route.runner {
        Runner::Sync(_) => dispatch(ctx, name, json),
        Runner::Job(_) => {
            let (sink, rx) = channel();
            let handle = dispatch_job(ctx, name, json, None, sink)?;
            loop {
                let Ok(event) = rx.recv() else {
                    return Err(
                        AppError::internal("the job ended without a terminal event").to_api()
                    );
                };
                observer(&event);
                match event {
                    JobEvent::Finished { result, .. } => return Ok(result),
                    JobEvent::Failed { error, .. } => return Err(error),
                    JobEvent::Cancelled { .. } => {
                        return Err(ApiError::new(codes::JOB_CANCELLED, "the job was cancelled")
                            .detail("jobId", handle.job_id.as_str()));
                    }
                    JobEvent::Started { .. } | JobEvent::Progress { .. } => {}
                }
            }
        }
    }
}

/// [`dispatch_blocking_with`] without an observer.
///
/// # Errors
/// The envelope of whatever stopped the command.
pub fn dispatch_blocking(ctx: &AppContext, name: &str, json: Value) -> Result<Value, ApiError> {
    dispatch_blocking_with(ctx, name, json, &mut |_| {})
}

fn check_target(ctx: &AppContext, out: &Utf8Path) -> Result<(), ApiError> {
    let refuse = |reason: &str| Err(AppError::invalid("dispatch_to_file", "out", reason).to_api());
    if !out.is_absolute() {
        return refuse("the output path must be absolute");
    }
    if out
        .components()
        .any(|c| matches!(c, Utf8Component::ParentDir))
    {
        return refuse("the output path must not contain ..");
    }
    if out.file_name().is_none() {
        return refuse("the output path must name a file");
    }
    if ctx.roots.kind_of(out).is_some() {
        return refuse("the application's own folders are not an output target");
    }
    if let Ok(protected) = ctx.workspace.protected_paths(ctx)
        && protected.iter().any(|p| out.starts_with(p))
    {
        return refuse("the game install, Workshop and user data folders are not an output target");
    }
    Ok(())
}

/// Runs a command (waiting for a job) and writes its JSON result to `out` through
/// `rimstudio-io::atomic_write`: temp file, flush, rename.
///
/// `out` must be an absolute path to a file outside the application's own folders, the game install
/// and the Workshop and user data folders.
///
/// # Errors
/// `ipc.invalid-request` for a refused target, the envelope of the command, `io.write-failed` when the
/// write fails.
pub fn dispatch_to_file(
    ctx: &AppContext,
    name: &str,
    json: Value,
    out: &Utf8Path,
) -> Result<FileReport, ApiError> {
    check_target(ctx, out)?;
    let value = dispatch_blocking(ctx, name, json)?;
    let mut bytes = serde_json::to_vec_pretty(&value)
        .map_err(|e| AppError::internal(format!("the result cannot be encoded: {e}")).to_api())?;
    bytes.push(b'\n');
    rimstudio_io::atomic_write(out, &bytes).map_err(|e| {
        let error = api_from_store(&e, &ctx.redactor);
        log_failure(&ctx.redactor, name, &error);
        error
    })?;
    Ok(FileReport {
        path: out.to_owned(),
        bytes: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn redactor() -> Redactor {
        Redactor::new().with_home("/home/rs_person")
    }

    #[test]
    fn a_panic_in_a_handler_becomes_an_internal_error_with_a_redacted_message() {
        let error = catch_handler(&redactor(), "rs_command", || {
            panic!("broke while reading /home/rs_person/secret")
        })
        .unwrap_err();
        assert_eq!(error.code, "app.internal-error");
        assert!(error.message.contains("rs_command"));
        assert!(!error.message.contains("rs_person"), "{}", error.message);
        assert!(error.error_id.starts_with("e-"));
    }

    #[test]
    fn results_and_errors_pass_through_unchanged() {
        let ok = catch_handler(&redactor(), "c", || Ok(json!({"a": 1})));
        assert_eq!(ok.unwrap(), json!({"a": 1}));
        let err = catch_handler(&redactor(), "c", || {
            Err(ApiError::with_id("io.not-found", "x", "e-00000001"))
        })
        .unwrap_err();
        assert_eq!(err.error_id, "e-00000001");
    }
}
