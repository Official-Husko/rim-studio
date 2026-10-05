//! `POST /rpc/COMMAND`, `GET /dev/info` and `GET /dev/commands`.
//!
//! The call runs through [`rimstudio_app::dispatch_blocking_with`], the door the CLI uses, so the
//! bridge executes the same handlers as every other front end. A job command is awaited here and its
//! progress is forwarded to the event hub while it runs. Application errors answer HTTP 200 with
//! `ok: false`; only protocol problems use other statuses.

use rimstudio_app::dispatch::dispatch_blocking_with;
use rimstudio_app::registry;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::jobs::JobEvent;
use serde_json::{Value, json};

use crate::events::bridge_event;
use crate::http::{Reject, Response};
use crate::server::Shared;

/// The longest command name accepted in the path.
const MAX_COMMAND_NAME: usize = 64;

/// True for a snake case command name of a sane length.
#[must_use]
pub fn valid_command_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_COMMAND_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Runs one command and returns the response of the bridge.
///
/// # Errors
/// A 400 for a malformed command name or a body that is not JSON.
pub fn call(shared: &Shared, name: &str, body: &[u8]) -> Result<Response, Reject> {
    if !valid_command_name(name) {
        return Err(Reject::bad(
            "bad-command-name",
            "the command name must be snake case letters and digits",
        ));
    }
    let request: Value = if body.iter().all(u8::is_ascii_whitespace) {
        Value::Null
    } else {
        serde_json::from_slice(body)
            .map_err(|e| Reject::bad("bad-json", format!("the body is not valid JSON: {e}")))?
    };
    let mut diagnostics: Vec<Value> = Vec::new();
    let result = dispatch_blocking_with(&shared.app, name, request, &mut |event| {
        if let JobEvent::Finished { diagnostics: d, .. } = event {
            diagnostics = d
                .iter()
                .filter_map(|x| serde_json::to_value(x).ok())
                .collect();
        }
        if let Some(line) = bridge_event(name, event) {
            shared.hub.broadcast(&line.to_string());
        }
    });
    Ok(match result {
        Ok(data) => {
            let mut envelope = json!({"ok": true, "data": data});
            if let (false, Some(map)) = (diagnostics.is_empty(), envelope.as_object_mut()) {
                map.insert("diagnostics".to_owned(), Value::Array(diagnostics));
            }
            Response::json(200, &envelope)
        }
        Err(error) => Response::json(200, &json!({"ok": false, "error": error_json(&error)})),
    })
}

fn error_json(error: &ApiError) -> Value {
    serde_json::to_value(error).unwrap_or_else(|_| json!({"code": error.code}))
}

/// `GET /dev/info`.
#[must_use]
pub fn info(shared: &Shared) -> Value {
    let app = &shared.app;
    json!({
        "bridgeVersion": crate::BRIDGE_VERSION,
        "platform": format!("{:?}", app.platform.os).to_lowercase(),
        "home": app.platform.env.home_dir().map(|p| p.as_str().to_owned()).unwrap_or_default(),
        "dataDir": shared.data_dir,
        "commandCount": registry::ROUTES.len(),
        "contractHash": registry::contract_hash(),
        "roots": {
            "config": app.roots.config.as_str(),
            "data": app.roots.data.as_str(),
            "cache": app.roots.cache.as_str(),
            "logs": app.roots.logs.as_str(),
        },
    })
}

/// `GET /dev/commands`: every registry row.
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
    use super::*;

    #[test]
    fn command_names_are_snake_case() {
        assert!(valid_command_name("designer_preview"));
        assert!(valid_command_name("a1"));
        assert!(!valid_command_name(""));
        assert!(!valid_command_name("Upper"));
        assert!(!valid_command_name("a-b"));
        assert!(!valid_command_name("a/b"));
        assert!(!valid_command_name("../x"));
        assert!(!valid_command_name(&"a".repeat(65)));
    }

    #[test]
    fn the_command_list_covers_the_registry() {
        let v = commands();
        let rows = v.as_array().unwrap();
        assert_eq!(rows.len(), registry::ROUTES.len());
        let ping = rows.iter().find(|r| r["name"] == "app_ping").unwrap();
        assert_eq!(ping["kind"], "query");
        assert!(ping["request"].is_string() && ping["response"].is_string());
    }
}
