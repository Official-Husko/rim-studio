//! The command layer: what the four shell commands do, without a window.
//!
//! Semantics are those of the development bridge (ADR 0040): a query or action answers its response, a
//! job command resolves when the job ends (its progress goes to the `emit` callback while it runs), and
//! a failure is the [`ApiError`] envelope of the registry.

use rimstudio_app::dispatch::dispatch_blocking_with;
use rimstudio_app::{AppContext, registry};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::jobs::CancelJobRequest;
use serde_json::{Map, Value, json};

use crate::events::job_event_json;

/// What the shell keeps in Tauri's managed state.
#[derive(Clone)]
pub struct Shell {
    /// The booted composition root.
    pub app: AppContext,
}

/// Runs one registry command by its snake case name.
///
/// A job command is awaited; `emit` receives the bridge shaped JSON of its progress and terminal
/// events while it runs. `job_id` is the caller's own job id for a job command (it lets the webview
/// cancel before the first event arrives); it is ignored for queries and actions.
///
/// # Errors
/// The envelope of whatever stopped the command, including `ipc.unknown-command` and
/// `ipc.invalid-request`.
pub fn call(
    ctx: &AppContext,
    name: &str,
    request: Value,
    job_id: Option<&str>,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, ApiError> {
    let request = with_job_id(name, request, job_id);
    dispatch_blocking_with(ctx, name, request, &mut |event| {
        if let Some(json) = job_event_json(name, event) {
            emit(json);
        }
    })
}

fn with_job_id(name: &str, request: Value, job_id: Option<&str>) -> Value {
    let is_job = registry::find(name).is_some_and(|r| r.kind.as_str() == "job");
    let (true, Some(id)) = (is_job, job_id) else {
        return request;
    };
    match request {
        Value::Object(mut map) => {
            map.insert("jobId".to_owned(), Value::String(id.to_owned()));
            Value::Object(map)
        }
        Value::Null => {
            let mut map = Map::new();
            map.insert("jobId".to_owned(), Value::String(id.to_owned()));
            Value::Object(map)
        }
        other => other,
    }
}

/// Cancels a job by id through the registry command `cancel_job`; idempotent.
///
/// # Errors
/// The envelope of `cancel_job` (it does not fail in 0.1.0).
pub fn cancel(ctx: &AppContext, job_id: &str) -> Result<Value, ApiError> {
    let request = serde_json::to_value(CancelJobRequest {
        job_id: job_id.to_owned(),
    })
    .unwrap_or_else(|_| json!({"jobId": job_id}));
    call(ctx, "cancel_job", request, None, &mut |_| {})
}

/// The facts about this run, with the keys of the bridge's `/dev/info`.
#[must_use]
pub fn info(ctx: &AppContext) -> Value {
    json!({
        "bridgeVersion": env!("CARGO_PKG_VERSION"),
        "shellVersion": env!("CARGO_PKG_VERSION"),
        "platform": format!("{:?}", ctx.platform.os).to_lowercase(),
        "home": ctx.platform.env.home_dir().map(|p| p.as_str().to_owned()).unwrap_or_default(),
        "dataDir": ctx.roots.data.as_str(),
        "commandCount": registry::ROUTES.len(),
        "contractHash": registry::contract_hash(),
        "roots": {
            "config": ctx.roots.config.as_str(),
            "data": ctx.roots.data.as_str(),
            "cache": ctx.roots.cache.as_str(),
            "logs": ctx.roots.logs.as_str(),
        },
    })
}

/// Every registry row (name, kind, request and response type names), like `/dev/commands`.
#[must_use]
pub fn commands() -> Value {
    Value::Array(
        registry::ROUTES
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "kind": r.kind.as_str(),
                    "request": r.request,
                    "response": r.response,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use rimstudio_app::logging::LogConfig;
    use rimstudio_app::{BootInput, Platform, boot};

    use super::*;

    /// A context over a temporary data base; the guard keeps the folder alive.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let dir = tempfile::tempdir().unwrap();
        let input = BootInput::new(Platform::system())
            .with_data_base(dir.path().to_str().unwrap())
            .with_log(LogConfig::off());
        let ctx = boot(input).unwrap();
        (dir, ctx)
    }

    #[test]
    fn a_query_answers_its_response() {
        let (_dir, ctx) = fixture();
        let out = call(&ctx, "app_ping", json!({}), None, &mut |_| {}).unwrap();
        assert!(out.is_object());
    }

    #[test]
    fn an_unknown_command_is_an_error_envelope() {
        let (_dir, ctx) = fixture();
        let err = call(&ctx, "no_such_command", json!({}), None, &mut |_| {}).unwrap_err();
        assert_eq!(err.code, "ipc.unknown-command");
        assert!(!err.error_id.is_empty());
    }

    #[test]
    fn a_bad_request_names_the_field() {
        let (_dir, ctx) = fixture();
        let err = call(&ctx, "cancel_job", json!({"nope": 1}), None, &mut |_| {}).unwrap_err();
        assert_eq!(err.code, "ipc.invalid-request");
    }

    #[test]
    fn a_job_resolves_when_it_ends_and_reports_progress() {
        let (_dir, ctx) = fixture();
        let mut events = Vec::new();
        let out = call(&ctx, "detect_run", json!({}), None, &mut |e| events.push(e));
        assert!(out.is_ok(), "{out:?}");
        let last = events.last().unwrap();
        assert_eq!(last["type"], "job-finished");
        assert_eq!(last["command"], "detect_run");
        assert_eq!(last["ok"], true);
    }

    #[test]
    fn a_caller_job_id_names_the_job() {
        let (_dir, ctx) = fixture();
        let mut events = Vec::new();
        call(&ctx, "detect_run", json!({}), Some("j-mine"), &mut |e| {
            events.push(e)
        })
        .unwrap();
        assert!(events.iter().all(|e| e["jobId"] == "j-mine"), "{events:?}");
    }

    #[test]
    fn the_job_id_is_not_added_to_a_query() {
        assert_eq!(
            with_job_id("app_ping", json!({}), Some("j")),
            json!({}),
            "queries keep their request"
        );
        assert_eq!(
            with_job_id("detect_run", Value::Null, Some("j")),
            json!({"jobId": "j"})
        );
    }

    #[test]
    fn cancelling_an_unknown_job_is_not_an_error() {
        let (_dir, ctx) = fixture();
        let out = cancel(&ctx, "j-unknown").unwrap();
        assert_eq!(out["state"], "unknown");
    }

    #[test]
    fn info_has_the_keys_of_the_bridge() {
        let (dir, ctx) = fixture();
        let v = info(&ctx);
        assert!(
            v["dataDir"]
                .as_str()
                .unwrap()
                .starts_with(dir.path().to_str().unwrap())
        );
        assert_eq!(v["commandCount"], registry::ROUTES.len());
        assert_eq!(v["contractHash"], registry::contract_hash());
        assert!(v["platform"].is_string() && v["roots"]["logs"].is_string());
    }

    #[test]
    fn the_command_list_covers_the_registry() {
        let rows = commands();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), registry::ROUTES.len());
        let kinds: Vec<_> = rows.iter().filter_map(|r| r["kind"].as_str()).collect();
        assert!(kinds.contains(&"job") && kinds.contains(&"query"));
    }
}
