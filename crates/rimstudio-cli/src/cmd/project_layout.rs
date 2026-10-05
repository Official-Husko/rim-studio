//! `project tree|check|scaffold-missing|read`: the layout commands of a mod folder.
//!
//! Each command opens (registers) the folder and then calls the matching command of the registry; the text
//! output is a plain rendering of the response, `--json` prints the response itself.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::error::CliResult;
use crate::fmt::{arr_at, bool_at, bytes, str_at, u64_at};
use crate::session::{Session, absolute};

fn project_id(s: &Session, path: &str) -> CliResult<String> {
    let abs = absolute(path)?;
    let summary = s.call("project_open", json!({"path": abs}))?.value;
    Ok(str_at(&summary, "/projectId").to_owned())
}

fn render_issues(issues: &[Value]) -> String {
    let mut out = String::new();
    for i in issues {
        let _ = writeln!(
            out,
            "  {:<8} {}  {}",
            str_at(i, "/severity"),
            str_at(i, "/code"),
            if str_at(i, "/path").is_empty() {
                "(project)"
            } else {
                str_at(i, "/path")
            }
        );
        let _ = writeln!(out, "           {}", str_at(i, "/message"));
        let summary = str_at(i, "/fix/summary");
        if !summary.is_empty() {
            let how = if bool_at(i, "/fix/automatic") {
                "automatic fix"
            } else {
                "suggested fix"
            };
            let _ = writeln!(out, "           {how}: {summary}");
        }
    }
    out
}

fn render_node(out: &mut String, node: &Value, indent: usize, depth: usize, files: bool) {
    let name = str_at(node, "/name");
    let is_dir = str_at(node, "/kind") == "folder";
    let issues = u64_at(node, "/issues");
    let mut line = format!("{}{name}", "  ".repeat(indent));
    if is_dir {
        line.push('/');
    }
    let _ = write!(line, "  [{}]", str_at(node, "/role"));
    if is_dir {
        let _ = write!(
            line,
            " {} files, {}",
            u64_at(node, "/files"),
            bytes(u64_at(node, "/bytes"))
        );
    } else {
        let _ = write!(line, " {}", bytes(u64_at(node, "/bytes")));
    }
    if issues > 0 {
        let _ = write!(line, "  ({issues} issues)");
    }
    out.push_str(&line);
    out.push('\n');
    if !is_dir || indent >= depth {
        return;
    }
    for child in arr_at(node, "/children") {
        if files || str_at(child, "/kind") == "folder" {
            render_node(out, child, indent + 1, depth, files);
        }
    }
}

/// `project tree`.
pub(crate) fn tree(s: &Session, path: &str, depth: usize, files: bool) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s.call("project_tree", json!({"projectId": id}))?.value;
    s.emit(&reply, || {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "layout: {}, weapon folder {}, Combat Extended folder {}{}{}",
            str_at(&reply, "/profile"),
            str_at(&reply, "/weaponsFolder"),
            str_at(&reply, "/ceFolder"),
            if bool_at(&reply, "/ceFolderExists") {
                ""
            } else {
                " (not there)"
            },
            if bool_at(&reply, "/ceFolderLegacy") {
                " (older name, kept)"
            } else {
                ""
            }
        );
        render_node(&mut out, &reply["root"], 0, depth, files);
        let c = &reply["counts"];
        let _ = writeln!(
            out,
            "{} files, {} definition files ({} weapons, {} projectiles), {} patch files, {} textures, {} sounds",
            u64_at(c, "/files"),
            u64_at(c, "/defFiles"),
            u64_at(c, "/weaponDefs"),
            u64_at(c, "/projectileDefs"),
            u64_at(c, "/patchFiles"),
            u64_at(c, "/textures"),
            u64_at(c, "/sounds")
        );
        let issues = arr_at(&reply, "/issues");
        if issues.is_empty() {
            out.push_str("no layout issues\n");
        } else {
            let _ = writeln!(out, "{} layout issues:", issues.len());
            out.push_str(&render_issues(issues));
        }
        out
    });
    Ok(())
}

/// `project check`.
pub(crate) fn check(s: &Session, path: &str) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call("project_layout_check", json!({"projectId": id}))?
        .value;
    s.emit(&reply, || {
        let mut out = format!(
            "layout {}: {} errors, {} warnings, {} notes, {} fixable by scaffold-missing\n",
            str_at(&reply, "/profile"),
            u64_at(&reply, "/errors"),
            u64_at(&reply, "/warnings"),
            u64_at(&reply, "/infos"),
            u64_at(&reply, "/autoFixable")
        );
        out.push_str(&render_issues(arr_at(&reply, "/issues")));
        out
    });
    if u64_at(&reply, "/errors") + u64_at(&reply, "/warnings") > 0 {
        s.note_warning();
    }
    Ok(())
}

/// `project scaffold-missing`.
pub(crate) fn scaffold_missing(s: &Session, path: &str, dry_run: bool) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call(
            "project_scaffold_missing",
            json!({"projectId": id, "dryRun": dry_run}),
        )?
        .value;
    s.emit(&reply, || {
        let verb = if dry_run { "would create" } else { "created" };
        let mut out = String::new();
        let folders = arr_at(&reply, "/folders");
        if folders.is_empty() {
            out.push_str("nothing is missing\n");
        }
        for f in folders {
            let _ = writeln!(out, "{verb} {}", f.as_str().unwrap_or(""));
        }
        for f in arr_at(&reply, "/skipped") {
            let _ = writeln!(out, "left alone: {}", f.as_str().unwrap_or(""));
        }
        out
    });
    Ok(())
}

/// `project read`.
pub(crate) fn read(s: &Session, path: &str, file: &str, max_bytes: Option<u32>) -> CliResult {
    let id = project_id(s, path)?;
    let mut request = json!({"projectId": id, "path": file});
    if let Some(n) = max_bytes {
        request["maxBytes"] = json!(n);
    }
    let reply = s.call("project_read_file", request)?.value;
    s.emit(&reply, || {
        if bool_at(&reply, "/binary") {
            return format!(
                "{} is not a text file ({})\n",
                str_at(&reply, "/path"),
                bytes(u64_at(&reply, "/bytes"))
            );
        }
        let mut text = str_at(&reply, "/text").to_owned();
        if bool_at(&reply, "/truncated") {
            let _ = write!(
                text,
                "\n... cut at {} of {}",
                bytes(u64::try_from(text.len()).unwrap_or(u64::MAX)),
                bytes(u64_at(&reply, "/bytes"))
            );
        }
        text
    });
    Ok(())
}
