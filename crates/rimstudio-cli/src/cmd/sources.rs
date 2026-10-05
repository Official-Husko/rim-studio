//! `sources list|add|remove`.

use std::fmt::Write as _;

use serde_json::json;

use crate::cli::SourcesCmd;
use crate::error::CliResult;
use crate::fmt::{Table, arr_at, bool_at, str_at, u64_at};
use crate::session::{Session, absolute};

/// Runs a `sources` command.
///
/// # Errors
/// The envelope of a failing command.
pub(crate) fn run(s: &Session, cmd: &SourcesCmd) -> CliResult {
    match cmd {
        SourcesCmd::List => {
            let reply = s.call("sources_list", json!({}))?;
            s.emit(&reply.value, || {
                let mut t = Table::new(&["id", "kind", "status", "mods", "enabled", "path"]);
                for x in arr_at(&reply.value, "/sources") {
                    t.row(vec![
                        str_at(x, "/id").to_owned(),
                        str_at(x, "/kind").to_owned(),
                        str_at(x, "/status").to_owned(),
                        x.get("modCount")
                            .and_then(serde_json::Value::as_u64)
                            .map_or("-".into(), |n| n.to_string()),
                        bool_at(x, "/enabled").to_string(),
                        str_at(x, "/path").to_owned(),
                    ]);
                }
                t.render()
            });
        }
        SourcesCmd::Add { path, label } => {
            let abs = absolute(path)?;
            let mut request = json!({"path": abs});
            if let Some(label) = label {
                request["label"] = json!(label);
            }
            let probe = s.call("sources_probe_folder", json!({"path": abs}))?.value;
            let reply = s.call("sources_add_folder", request)?;
            let doc = json!({"source": reply.value, "probe": probe});
            s.emit(&doc, || {
                let mut out = String::new();
                let _ = writeln!(
                    out,
                    "added source {} ({}) with {} mods",
                    str_at(&reply.value, "/id"),
                    str_at(&reply.value, "/path"),
                    u64_at(&probe, "/modCount")
                );
                out
            });
        }
        SourcesCmd::Remove { id } => {
            let reply = s.call("sources_remove", json!({"id": id}))?;
            s.emit(&reply.value, || {
                if bool_at(&reply.value, "/removed") {
                    format!("removed source {id}\n")
                } else {
                    format!("no source with id {id}\n")
                }
            });
        }
    }
    Ok(())
}
