//! `settings get|set|reset`: the settings through `settings_get` and `settings_update`.

use std::fmt::Write as _;

use serde_json::{Map, Value, json};

use crate::cli::SettingsCmd;
use crate::cmd::generic::request_problem;
use crate::draftops::parse_value;
use crate::error::{CliError, CliResult};
use crate::session::Session;

/// Builds the nested patch of `settings set a.b value`.
///
/// # Errors
/// A usage error for an empty key or an empty key segment.
pub(crate) fn patch_for(key: &str, value: &str) -> CliResult<Value> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(CliError::usage(format!(
            "`{key}` is not a settings key such as appearance.density"
        )));
    }
    let mut node = parse_value(value);
    for part in parts.iter().rev() {
        let mut map = Map::new();
        map.insert((*part).to_owned(), node);
        node = Value::Object(map);
    }
    Ok(node)
}

/// Flattens a JSON document into `dotted.key = value` lines.
pub(crate) fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, x) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, x, out);
            }
        }
        Value::String(s) => out.push((prefix.to_owned(), s.clone())),
        other => out.push((prefix.to_owned(), other.to_string())),
    }
}

fn lookup<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(v, |node, part| node.get(part))
}

/// Runs a `settings` command.
///
/// # Errors
/// A usage error for a bad key, the envelope (for example `settings.invalid`) otherwise.
pub(crate) fn run(s: &Session, cmd: &SettingsCmd) -> CliResult {
    match cmd {
        SettingsCmd::Get { key } => {
            let reply = s.call("settings_get", json!({}))?;
            let doc = match key {
                Some(k) => lookup(&reply.value, k)
                    .cloned()
                    .ok_or_else(|| CliError::usage(format!("no setting named `{k}`")))?,
                None => reply.value,
            };
            s.emit(&doc, || {
                let mut lines = Vec::new();
                flatten(key.as_deref().unwrap_or(""), &doc, &mut lines);
                let mut out = String::new();
                for (k, v) in lines {
                    let _ = writeln!(out, "{k} = {v}");
                }
                out
            });
        }
        SettingsCmd::Set { key, value } => {
            let patch = patch_for(key, value)?;
            let reply = s.call("settings_update", patch).map_err(request_problem)?;
            let doc = reply.value;
            s.emit(&doc, || {
                let shown = lookup(&doc, key).map_or_else(
                    || "?".to_owned(),
                    |v| match v {
                        Value::String(t) => t.clone(),
                        other => other.to_string(),
                    },
                );
                format!("{key} = {shown}\n")
            });
        }
        SettingsCmd::Reset { key } => {
            let reply = s.call("settings_update", json!({"reset": [key]}))?;
            let doc = reply.value;
            s.emit(&doc, || format!("{key} reset to its default\n"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dotted_key_becomes_a_nested_patch() {
        let p = patch_for("appearance.density", "compact").unwrap_or(Value::Null);
        assert_eq!(p, json!({"appearance": {"density": "compact"}}));
        let p = patch_for("onboardingCompleted", "true").unwrap_or(Value::Null);
        assert_eq!(p, json!({"onboardingCompleted": true}));
        let p = patch_for("appearance.fontScale", "1.25").unwrap_or(Value::Null);
        assert_eq!(p, json!({"appearance": {"fontScale": 1.25}}));
    }

    #[test]
    fn empty_segments_are_refused() {
        assert!(patch_for("a..b", "1").is_err());
        assert!(patch_for("", "1").is_err());
    }

    #[test]
    fn flatten_lists_leaves_with_dotted_keys() {
        let mut out = Vec::new();
        flatten("", &json!({"a": {"b": 1, "c": "x"}, "d": true}), &mut out);
        assert!(out.contains(&("a.b".to_owned(), "1".to_owned())));
        assert!(out.contains(&("a.c".to_owned(), "x".to_owned())));
        assert!(out.contains(&("d".to_owned(), "true".to_owned())));
    }

    #[test]
    fn lookup_follows_dotted_keys() {
        let v = json!({"a": {"b": 2}});
        assert_eq!(lookup(&v, "a.b"), Some(&json!(2)));
        assert_eq!(lookup(&v, "a.z"), None);
    }
}
