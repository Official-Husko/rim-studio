//! `defs search` and `defs resolve`: the def explorer on the command line.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::DefsCmd;
use crate::cmd::drafts::project_id;
use crate::cmd::project;
use crate::error::CliResult;
use crate::fmt::{Table, arr_at, bool_at, str_at, u64_at};
use crate::session::Session;

fn session_id(s: &Session, session: &str, project_dir: Option<&String>) -> CliResult<String> {
    match project_dir {
        Some(dir) => {
            let summary = project::open(s, dir)?;
            Ok(format!("project:{}", project_id(&summary)))
        }
        None => Ok(session.to_owned()),
    }
}

/// Renders a resolved definition tree (`{tag, attrs, children}`) as an indented outline.
pub(crate) fn render_tree(node: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let tag = str_at(node, "/tag");
    let attrs: Vec<String> = arr_at(node, "/attrs")
        .iter()
        .filter_map(|p| {
            let pair = p.as_array()?;
            Some(format!(
                "{}={}",
                pair.first()?.as_str()?,
                pair.get(1)?.as_str()?
            ))
        })
        .collect();
    let attr_text = if attrs.is_empty() {
        String::new()
    } else {
        format!(" ({})", attrs.join(", "))
    };
    let children = arr_at(node, "/children");
    let only_text = children.iter().all(Value::is_string);
    if only_text {
        let text: String = children.iter().filter_map(Value::as_str).collect();
        if text.is_empty() {
            let _ = writeln!(out, "{pad}{tag}{attr_text}");
        } else {
            let _ = writeln!(out, "{pad}{tag}{attr_text}: {}", text.trim());
        }
        return;
    }
    let _ = writeln!(out, "{pad}{tag}{attr_text}");
    for c in children {
        if c.is_object() {
            render_tree(c, depth + 1, out);
        } else if let Some(t) = c.as_str().map(str::trim).filter(|t| !t.is_empty()) {
            let _ = writeln!(out, "{pad}  {t}");
        }
    }
}

/// Runs a `defs` command.
///
/// # Errors
/// The envelope of a failing command (for example `defs.session-not-found`).
pub(crate) fn run(s: &Session, cmd: &DefsCmd) -> CliResult {
    match cmd {
        DefsCmd::Search {
            query,
            session,
            project,
            def_types,
            hide_abstract,
            limit,
            offset,
        } => {
            let sid = session_id(s, session, project.as_ref())?;
            let reply = s.call(
                "defs_search",
                json!({
                    "sessionId": sid,
                    "query": query,
                    "defTypes": def_types,
                    "hideAbstract": hide_abstract,
                    "limit": limit,
                    "offset": offset,
                    "queryId": "cli",
                }),
            )?;
            let v = &reply.value;
            s.emit(v, || {
                let mut out = format!(
                    "{} matches, showing {} from {}\n",
                    u64_at(v, "/total"),
                    arr_at(v, "/items").len(),
                    u64_at(v, "/offset")
                );
                let mut t = Table::new(&["type", "defName", "label", "mod", "file", "abstract"]);
                for r in arr_at(v, "/items") {
                    t.row(vec![
                        str_at(r, "/defType").to_owned(),
                        str_at(r, "/defName").to_owned(),
                        str_at(r, "/label").to_owned(),
                        str_at(r, "/modId").to_owned(),
                        str_at(r, "/file").to_owned(),
                        if bool_at(r, "/isAbstract") {
                            "yes".to_owned()
                        } else {
                            String::new()
                        },
                    ]);
                }
                out.push_str(&t.render());
                out
            });
        }
        DefsCmd::Resolve {
            def_type,
            def_name,
            session,
            project,
        } => {
            let sid = session_id(s, session, project.as_ref())?;
            let reply = s.call(
                "defs_get_resolved",
                json!({"sessionId": sid, "defType": def_type, "defName": def_name}),
            )?;
            let v = &reply.value;
            s.emit(v, || {
                let mut out = format!(
                    "{} {} from {} ({})\n",
                    str_at(v, "/defType"),
                    str_at(v, "/defName"),
                    str_at(v, "/modId"),
                    str_at(v, "/file")
                );
                for e in arr_at(v, "/patchEvents") {
                    let _ = writeln!(
                        out,
                        "patch {} {} by {}: {}",
                        str_at(e, "/operation"),
                        str_at(e, "/file"),
                        str_at(e, "/modId"),
                        str_at(e, "/outcome")
                    );
                }
                out.push('\n');
                render_tree(&v["tree"], 0, &mut out);
                out
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_renders_as_an_outline_with_text_leaves() {
        let tree = json!({"tag": "ThingDef", "attrs": [["Name", "RS_Base"]], "children": [
            {"tag": "defName", "attrs": [], "children": ["RS_Gun"]},
            {"tag": "statBases", "attrs": [], "children": [
                {"tag": "Mass", "attrs": [], "children": ["3"]}]},
            {"tag": "empty", "attrs": [], "children": []}]});
        let mut out = String::new();
        render_tree(&tree, 0, &mut out);
        let expected =
            "ThingDef (Name=RS_Base)\n  defName: RS_Gun\n  statBases\n    Mass: 3\n  empty\n";
        assert_eq!(out, expected);
    }
}
