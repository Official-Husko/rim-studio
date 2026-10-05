//! `project about show|set|preview`, `project load-folders show|set`, `project version add` and
//! `library search`: the basics of a mod from the command line.
//!
//! Each command opens (registers) the folder and calls the matching command of the registry. `about set`
//! and `load-folders set` print the diff and write nothing unless `--yes` is given; `--json` prints the
//! response of the command itself.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::{AboutSetArgs, LoadFoldersSetArgs};
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bool_at, bytes, count_diagnostics, render_diagnostics, str_at, u64_at};
use crate::session::{Session, absolute};

fn project_id(s: &Session, path: &str) -> CliResult<String> {
    let abs = absolute(path)?;
    let summary = s.call("project_open", json!({"path": abs}))?.value;
    Ok(str_at(&summary, "/projectId").to_owned())
}

/// A JSON argument: the text itself, or `@file` to read it from a file.
fn json_arg(text: &str) -> CliResult<Value> {
    let body = match text.strip_prefix('@') {
        Some(file) => std::fs::read_to_string(file)
            .map_err(|e| CliError::usage(format!("cannot read {file}: {e}")))?,
        None => text.to_owned(),
    };
    serde_json::from_str(&body)
        .map_err(|e| CliError::usage(format!("the changes are not JSON: {e}")))
}

fn warn_on(s: &Session, diagnostics: &[Value]) {
    let c = count_diagnostics(diagnostics);
    if c.errors + c.warnings > 0 {
        s.note_warning();
    }
}

fn value_of(v: &Value, field: &str) -> String {
    str_at(v, &format!("/{field}/value")).to_owned()
}

fn list_of(v: &Value, field: &str) -> String {
    arr_at(v, &format!("/{field}/items"))
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_about(v: &Value) -> String {
    let mut out = String::new();
    let line = |out: &mut String, label: &str, text: &str, present: bool| {
        let shown = if text.is_empty() {
            if present { "(empty)" } else { "-" }
        } else {
            text
        };
        let _ = writeln!(out, "  {label:<18} {shown}");
    };
    let _ = writeln!(
        out,
        "{} ({}){}",
        str_at(v, "/path"),
        bytes(u64_at(v, "/bytes")),
        if bool_at(v, "/editable") {
            String::new()
        } else {
            format!("  not editable: {}", str_at(v, "/notEditableReason"))
        }
    );
    let _ = writeln!(out, "  hash               {}", str_at(v, "/fileHash"));
    for (label, field) in [
        ("name", "name"),
        ("short name", "shortName"),
        ("author", "author"),
        ("package id", "packageId"),
        ("url", "url"),
        ("mod version", "modVersion"),
        ("icon path", "modIconPath"),
    ] {
        line(
            &mut out,
            label,
            &value_of(v, field),
            bool_at(v, &format!("/{field}/present")),
        );
    }
    for (label, field) in [
        ("authors", "authors"),
        ("game versions", "supportedVersions"),
        ("load after", "loadAfter"),
        ("load before", "loadBefore"),
        ("force load after", "forceLoadAfter"),
        ("force load before", "forceLoadBefore"),
        ("incompatible with", "incompatibleWith"),
    ] {
        line(
            &mut out,
            label,
            &list_of(v, field),
            bool_at(v, &format!("/{field}/present")),
        );
    }
    let deps = arr_at(v, "/modDependencies");
    let _ = writeln!(
        out,
        "  dependencies       {}",
        if deps.is_empty() { "-" } else { "" }
    );
    for d in deps {
        let _ = writeln!(
            out,
            "    {}  {}  {}",
            str_at(d, "/packageId"),
            str_at(d, "/displayName"),
            [str_at(d, "/steamWorkshopUrl"), str_at(d, "/downloadUrl")]
                .iter()
                .find(|u| !u.is_empty())
                .copied()
                .unwrap_or("(no link)")
        );
    }
    let descr = value_of(v, "description");
    let _ = writeln!(
        out,
        "  description        {} characters",
        descr.chars().count()
    );
    if bool_at(v, "/preview/exists") {
        let _ = writeln!(
            out,
            "  preview            {} ({} by {}, {})",
            str_at(v, "/preview/path"),
            u64_at(v, "/preview/width"),
            u64_at(v, "/preview/height"),
            bytes(u64_at(v, "/preview/bytes"))
        );
    } else {
        let _ = writeln!(out, "  preview            missing");
    }
    let bv = &v["byVersion"];
    let blocks = arr_at(bv, "/relations").len() + arr_at(bv, "/descriptions").len();
    if blocks > 0 {
        let _ = writeln!(out, "  by version (advanced): {blocks} blocks, see --json");
    }
    let diags = arr_at(v, "/diagnostics");
    if diags.is_empty() {
        out.push_str("no findings\n");
    } else {
        let _ = writeln!(out, "{} findings:", diags.len());
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// `project about show`.
pub(crate) fn about_show(s: &Session, path: &str, raw: bool) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s.call("project_about_get", json!({"projectId": id}))?.value;
    s.emit(&reply, || {
        if raw {
            return str_at(&reply, "/rawText").to_owned();
        }
        render_about(&reply)
    });
    warn_on(s, arr_at(&reply, "/diagnostics"));
    Ok(())
}

/// The changes the flags of `about set` stand for, in a fixed order.
fn about_changes(args: &AboutSetArgs) -> CliResult<Vec<Value>> {
    let mut out: Vec<Value> = Vec::new();
    if let Some(text) = &args.changes {
        match json_arg(text)? {
            Value::Array(list) => out.extend(list),
            _ => return Err(CliError::usage("--changes must be a JSON array of changes")),
        }
    }
    let mut set = |field: &str, value: &Option<String>| {
        if let Some(v) = value {
            out.push(json!({"op": "set", "field": field, "value": v}));
        }
    };
    set("name", &args.name);
    set("shortName", &args.short_name);
    set("author", &args.author);
    set("packageId", &args.package_id);
    set("url", &args.url);
    set("modVersion", &args.mod_version);
    set("modIconPath", &args.icon_path);
    if let Some(file) = &args.description_file {
        let text = std::fs::read_to_string(file)
            .map_err(|e| CliError::usage(format!("cannot read {file}: {e}")))?;
        out.push(json!({"op": "set", "field": "description", "value": text.trim_end()}));
    } else if let Some(d) = &args.description {
        out.push(json!({"op": "set", "field": "description", "value": d}));
    }
    for field in &args.clear {
        out.push(json!({"op": "clear", "field": field}));
    }
    if !args.game_versions.is_empty() {
        out.push(
            json!({"op": "list-set", "field": "supportedVersions", "items": args.game_versions}),
        );
    }
    for (field, adds, removes) in [
        ("loadAfter", &args.add_load_after, &args.remove_load_after),
        (
            "loadBefore",
            &args.add_load_before,
            &args.remove_load_before,
        ),
        (
            "incompatibleWith",
            &args.add_incompatible,
            &args.remove_incompatible,
        ),
    ] {
        for v in adds {
            out.push(json!({"op": "list-add", "field": field, "value": v}));
        }
        for v in removes {
            out.push(json!({"op": "list-remove", "field": field, "value": v}));
        }
    }
    for id in &args.remove_dependency {
        out.push(json!({"op": "dependency-remove", "packageId": id}));
    }
    if let Some(id) = &args.add_dependency {
        let mut dep = json!({"packageId": id, "displayName": args.dependency_name.clone().unwrap_or_default()});
        if let Some(u) = &args.workshop_url {
            dep["steamWorkshopUrl"] = json!(u);
        }
        if let Some(u) = &args.download_url {
            dep["downloadUrl"] = json!(u);
        }
        out.push(json!({"op": "dependency-add", "dependency": dep}));
    }
    Ok(out)
}

/// `project about set`.
pub(crate) fn about_set(s: &Session, args: &AboutSetArgs) -> CliResult {
    let changes = about_changes(args)?;
    if changes.is_empty() {
        return Err(CliError::usage(
            "nothing to change: give at least one option",
        ));
    }
    let id = project_id(s, &args.path)?;
    let current = s
        .call("project_about_get", json!({"projectId": id.clone()}))?
        .value;
    let hash = str_at(&current, "/fileHash").to_owned();
    let request = json!({"projectId": id, "changes": changes, "expectedHash": hash});
    if !args.yes {
        let reply = s.call("project_about_preview", request)?.value;
        s.emit(&reply, || {
            let mut out = String::new();
            if str_at(&reply, "/diff").is_empty() {
                out.push_str("the changes alter no byte\n");
            } else {
                out.push_str(str_at(&reply, "/diff"));
                out.push_str("nothing was written (add --yes to write it)\n");
            }
            out
        });
        warn_on(s, arr_at(&reply["result"], "/diagnostics"));
        return Ok(());
    }
    let reply = s.call("project_about_update", request)?.value;
    s.emit(&reply, || {
        let mut out = String::new();
        if bool_at(&reply, "/written") {
            out.push_str(str_at(&reply, "/diff"));
            let _ = writeln!(out, "written; backup {}", str_at(&reply, "/backup"));
        } else {
            out.push_str("the changes alter no byte, nothing was written\n");
        }
        out.push_str(&render_diagnostics(arr_at(&reply["about"], "/diagnostics")));
        out
    });
    warn_on(s, arr_at(&reply["about"], "/diagnostics"));
    Ok(())
}

/// `project about preview`: the preview image.
pub(crate) fn about_preview(
    s: &Session,
    path: &str,
    image: Option<&str>,
    remove: bool,
) -> CliResult {
    let id = project_id(s, path)?;
    if remove {
        let reply = s
            .call("project_about_remove_preview", json!({"projectId": id}))?
            .value;
        s.emit(&reply, || {
            if bool_at(&reply, "/removed") {
                format!(
                    "removed About/Preview.png; backup {}\n",
                    str_at(&reply, "/backup")
                )
            } else {
                "there was no preview image\n".to_owned()
            }
        });
        return Ok(());
    }
    if let Some(image) = image {
        let abs = absolute(image)?;
        let reply = s
            .call(
                "project_about_set_preview",
                json!({"projectId": id, "sourcePath": abs}),
            )?
            .value;
        s.emit(&reply, || {
            let mut out = format!(
                "copied {abs} to {} ({} by {}, {})\n",
                str_at(&reply, "/preview/path"),
                u64_at(&reply, "/preview/width"),
                u64_at(&reply, "/preview/height"),
                bytes(u64_at(&reply, "/preview/bytes"))
            );
            if bool_at(&reply, "/replaced") {
                let _ = writeln!(
                    out,
                    "replaced the earlier image; backup {}",
                    str_at(&reply, "/backup")
                );
            }
            out.push_str(&render_diagnostics(arr_at(&reply, "/diagnostics")));
            out
        });
        warn_on(s, arr_at(&reply, "/diagnostics"));
        return Ok(());
    }
    let reply = s.call("project_about_get", json!({"projectId": id}))?.value;
    let preview = reply["preview"].clone();
    s.emit(&preview, || {
        if bool_at(&preview, "/exists") {
            format!(
                "{}: {} by {}, {}\n",
                str_at(&preview, "/path"),
                u64_at(&preview, "/width"),
                u64_at(&preview, "/height"),
                bytes(u64_at(&preview, "/bytes"))
            )
        } else {
            "no preview image (use --image FILE.png to add one)\n".to_owned()
        }
    });
    Ok(())
}

fn render_load_folders(v: &Value) -> String {
    let mut out = String::new();
    if !bool_at(v, "/exists") {
        out.push_str("the mod has no LoadFolders.xml\n");
    }
    for b in arr_at(v, "/blocks") {
        let _ = writeln!(out, "block {} ({})", u64_at(b, "/index"), str_at(b, "/tag"));
        for e in arr_at(b, "/entries") {
            let mut conds = String::new();
            for (label, key) in [
                ("if active", "/ifModActive"),
                ("if all active", "/ifModActiveAll"),
                ("if not active", "/ifModNotActive"),
            ] {
                let ids: Vec<&str> = arr_at(e, key).iter().filter_map(Value::as_str).collect();
                if !ids.is_empty() {
                    let _ = write!(conds, "  {label}: {}", ids.join(", "));
                }
            }
            let folder = if str_at(e, "/path").is_empty() {
                "/"
            } else {
                str_at(e, "/path")
            };
            let _ = writeln!(
                out,
                "  {:>2}  {folder}{}{conds}",
                u64_at(e, "/index"),
                if bool_at(e, "/folderExists") {
                    ""
                } else {
                    "  (missing)"
                }
            );
        }
    }
    let diags = arr_at(v, "/diagnostics");
    if diags.is_empty() {
        out.push_str("no findings\n");
    } else {
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// `project load-folders show`.
pub(crate) fn load_folders_show(s: &Session, path: &str) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call("project_load_folders_get", json!({"projectId": id}))?
        .value;
    s.emit(&reply, || render_load_folders(&reply));
    warn_on(s, arr_at(&reply, "/diagnostics"));
    Ok(())
}

fn load_folder_changes(args: &LoadFoldersSetArgs) -> CliResult<Vec<Value>> {
    let mut out: Vec<Value> = Vec::new();
    if let Some(text) = &args.changes {
        match json_arg(text)? {
            Value::Array(list) => out.extend(list),
            _ => return Err(CliError::usage("--changes must be a JSON array of changes")),
        }
    }
    let gate: Vec<&str> = args
        .gate
        .iter()
        .flat_map(|g| g.split(','))
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .collect();
    for v in &args.add_block {
        out.push(json!({"op": "add-block", "version": v, "entries": [{"path": "/"}]}));
    }
    for spec in &args.add_entry {
        let Some((block, folder)) = spec.split_once('=') else {
            return Err(CliError::usage(
                "--add-entry takes BLOCK=FOLDER, for example 1=Compat/CE",
            ));
        };
        let block: u32 = block
            .trim()
            .parse()
            .map_err(|_| CliError::usage("the block of --add-entry is a number"))?;
        let mut entry = json!({"path": folder});
        if !gate.is_empty() {
            entry["ifModActive"] = json!(gate);
        }
        out.push(json!({"op": "add-entry", "block": block, "entry": entry}));
    }
    for spec in &args.remove_entry {
        let Some((block, entry)) = spec.split_once(':') else {
            return Err(CliError::usage(
                "--remove-entry takes BLOCK:ENTRY, for example 1:2",
            ));
        };
        let (Ok(block), Ok(entry)) = (block.trim().parse::<u32>(), entry.trim().parse::<u32>())
        else {
            return Err(CliError::usage(
                "the block and the entry of --remove-entry are numbers",
            ));
        };
        out.push(json!({"op": "remove-entry", "block": block, "entry": entry}));
    }
    for b in &args.remove_block {
        out.push(json!({"op": "remove-block", "block": b}));
    }
    Ok(out)
}

/// `project load-folders set`.
pub(crate) fn load_folders_set(s: &Session, args: &LoadFoldersSetArgs) -> CliResult {
    let changes = load_folder_changes(args)?;
    if changes.is_empty() {
        return Err(CliError::usage(
            "nothing to change: give at least one option",
        ));
    }
    let id = project_id(s, &args.path)?;
    let current = s
        .call("project_load_folders_get", json!({"projectId": id.clone()}))?
        .value;
    let mut request = json!({
        "projectId": id,
        "changes": changes,
        "create": args.create,
        "dryRun": !args.yes,
    });
    if bool_at(&current, "/exists") {
        request["expectedHash"] = json!(str_at(&current, "/fileHash"));
    }
    let reply = s.call("project_load_folders_update", request)?.value;
    s.emit(&reply, || {
        let mut out = String::new();
        if str_at(&reply, "/diff").is_empty() {
            out.push_str("the changes alter no byte\n");
        } else {
            out.push_str(str_at(&reply, "/diff"));
        }
        if bool_at(&reply, "/written") {
            let _ = writeln!(out, "written; backup {}", str_at(&reply, "/backup"));
        } else {
            out.push_str("nothing was written (add --yes to write it)\n");
        }
        out.push_str(&render_load_folders(&reply["loadFolders"]));
        out
    });
    warn_on(s, arr_at(&reply["loadFolders"], "/diagnostics"));
    Ok(())
}

/// `project version add`.
pub(crate) fn version_add(
    s: &Session,
    path: &str,
    version: &str,
    standard_folders: bool,
    add_block: bool,
    yes: bool,
) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call(
            "project_version_add",
            json!({
                "projectId": id,
                "version": version,
                "standardFolders": standard_folders,
                "addBlock": add_block,
                "dryRun": !yes,
            }),
        )?
        .value;
    s.emit(&reply, || {
        let verb = if yes { "created" } else { "would create" };
        let mut out = String::new();
        for f in arr_at(&reply, "/createdFolders") {
            let _ = writeln!(out, "{verb} {}", f.as_str().unwrap_or(""));
        }
        if bool_at(&reply, "/blockAdded") {
            let _ = writeln!(
                out,
                "{verb} a block for {} in LoadFolders.xml{}",
                str_at(&reply, "/version"),
                if bool_at(&reply, "/loadFoldersCreated") { " (a new file)" } else { "" }
            );
        }
        if bool_at(&reply, "/notInSupportedVersions") {
            let _ = writeln!(
                out,
                "note: {} is not in supportedVersions of About.xml (use `project about set --game-versions`)",
                str_at(&reply, "/version")
            );
        }
        out.push_str(&render_diagnostics(arr_at(&reply, "/diagnostics")));
        if !yes {
            out.push_str("nothing was written (add --yes to do it)\n");
        }
        out
    });
    Ok(())
}

/// `library search`.
pub(crate) fn library_search(s: &Session, query: &str, limit: Option<u32>) -> CliResult {
    // every run of the command line starts without a scan, so this command scans first (the command of
    // the registry never does)
    if let Err(e) = s.call("library_scan", json!({})) {
        s.note(&format!("the library could not be scanned: {}", e.message));
    }
    let mut request = json!({"query": query});
    if let Some(n) = limit {
        request["limit"] = json!(n);
    }
    let reply = s.call("library_mod_search", request)?.value;
    s.emit(&reply, || {
        let mut out = String::new();
        for h in arr_at(&reply, "/hits") {
            let _ = writeln!(
                out,
                "{}  {}  ({}{}){}",
                str_at(h, "/packageId"),
                str_at(h, "/name"),
                str_at(h, "/source"),
                if str_at(h, "/workshopId").is_empty() {
                    String::new()
                } else {
                    format!(", workshop {}", str_at(h, "/workshopId"))
                },
                if bool_at(h, "/loadable") {
                    ""
                } else {
                    "  needs a link"
                }
            );
            let _ = writeln!(out, "    {}", str_at(h, "/path"));
        }
        if let Some(hint) = reply.get("hint").and_then(Value::as_str) {
            let _ = writeln!(out, "{hint}");
        }
        out
    });
    Ok(())
}
