//! `project fix plan|apply|undo|history`: carry out the suggestions of the layout check, with an undo.
//!
//! `plan` lists the changes (and writes nothing). `apply` writes nothing without `--yes`: it prints the items
//! it would carry out; with `--yes` it carries them out and prints what was moved, edited and skipped and
//! the layout check run again. `undo` reverses an apply from its journal, `history` lists the journals.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bool_at, str_at, u64_at};
use crate::session::{Session, absolute};

fn project_id(s: &Session, path: &str) -> CliResult<String> {
    let abs = absolute(path)?;
    let summary = s.call("project_open", json!({"path": abs}))?.value;
    Ok(str_at(&summary, "/projectId").to_owned())
}

fn render_item(out: &mut String, item: &Value) {
    let risk = if bool_at(item, "/applicable") {
        "safe"
    } else {
        "review"
    };
    let from = str_at(item, "/from");
    let _ = writeln!(
        out,
        "  {}  {:<18} {}{}{}  [{risk}]",
        str_at(item, "/id"),
        str_at(item, "/kind"),
        from,
        if from.is_empty() { "" } else { " -> " },
        str_at(item, "/to"),
    );
    let _ = writeln!(out, "      {}", str_at(item, "/why"));
    let reason = str_at(item, "/reviewReason");
    if !reason.is_empty() {
        let _ = writeln!(out, "      needs review: {reason}");
    }
    if let Some(c) = item.get("conflict") {
        let _ = writeln!(
            out,
            "      the destination exists{}",
            c.get("suggestedTo")
                .and_then(Value::as_str)
                .map(|n| format!("; --rename-on-conflict uses {n}"))
                .unwrap_or_default()
        );
    }
    for r in arr_at(item, "/references") {
        let _ = writeln!(
            out,
            "      mentioned in {} line {}: {}",
            str_at(r, "/path"),
            u64_at(r, "/line"),
            str_at(r, "/text")
        );
    }
    let diff = str_at(item, "/diff");
    if !diff.is_empty() {
        for line in diff.lines() {
            let _ = writeln!(out, "      {line}");
        }
    }
}

fn render_plan(plan: &Value) -> String {
    let mut out = String::new();
    let items = arr_at(plan, "/items");
    if items.is_empty() {
        out.push_str("nothing to fix\n");
        return out;
    }
    let _ = writeln!(
        out,
        "plan {}: {} items ({} can be applied, {} need review, {} conflicts)",
        str_at(plan, "/planId"),
        items.len(),
        u64_at(plan, "/safe"),
        u64_at(plan, "/needsReview"),
        u64_at(plan, "/conflicts")
    );
    for item in items {
        render_item(&mut out, item);
    }
    out
}

/// `project fix plan`.
pub(crate) fn plan(s: &Session, path: &str, only: &[String]) -> CliResult {
    let id = project_id(s, path)?;
    let mut req = json!({"projectId": id});
    if !only.is_empty() {
        req["fixes"] = json!(only);
    }
    let reply = s.call("project_layout_fix_plan", req)?.value;
    s.emit(&reply, || render_plan(&reply));
    if u64_at(&reply, "/needsReview") > 0 {
        s.note_warning();
    }
    Ok(())
}

/// What `project fix apply` was asked to do.
pub(crate) struct ApplyArgs<'a> {
    pub(crate) path: &'a str,
    pub(crate) items: &'a [String],
    pub(crate) all: bool,
    pub(crate) rename_on_conflict: bool,
    pub(crate) plan_id: Option<&'a str>,
    pub(crate) yes: bool,
}

/// `project fix apply`.
pub(crate) fn apply(s: &Session, args: &ApplyArgs<'_>) -> CliResult {
    if !args.all && args.items.is_empty() {
        return Err(CliError::usage(
            "name the items to apply with --item ID (repeatable), or use --all for every item that can be applied",
        ));
    }
    let id = project_id(s, args.path)?;
    let plan = s
        .call("project_layout_fix_plan", json!({"projectId": id}))?
        .value;
    let plan_id = args
        .plan_id
        .map_or_else(|| str_at(&plan, "/planId").to_owned(), str::to_owned);
    let selected: Vec<Value> = arr_at(&plan, "/items")
        .iter()
        .filter(|i| bool_at(i, "/applicable"))
        .filter(|i| args.all || args.items.iter().any(|w| w == str_at(i, "/id")))
        .map(|i| json!({"id": str_at(i, "/id"), "renameOnConflict": args.rename_on_conflict}))
        .collect();
    if !args.yes {
        s.emit(&json!({"applied": false, "plan": plan, "selected": selected}), || {
            let mut out = String::new();
            for item in arr_at(&plan, "/items") {
                if selected.iter().any(|w| str_at(w, "/id") == str_at(item, "/id")) {
                    render_item(&mut out, item);
                }
            }
            if selected.is_empty() {
                out.push_str("no item matches\n");
            }
            out.push_str("\nNothing was changed. Run the same command with --yes to carry these items out.\n");
            out
        });
        return Ok(());
    }
    let report = s
        .call(
            "project_layout_fix_apply",
            json!({"projectId": id, "planId": plan_id, "items": selected}),
        )?
        .value;
    s.emit(&report, || {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "apply {} of plan {}",
            str_at(&report, "/applyId"),
            str_at(&report, "/planId")
        );
        for d in arr_at(&report, "/done") {
            let from = str_at(d, "/from");
            let _ = writeln!(
                out,
                "  done     {}  {}{}{}",
                str_at(d, "/kind"),
                from,
                if from.is_empty() { "" } else { " -> " },
                str_at(d, "/to")
            );
        }
        for e in arr_at(&report, "/edited") {
            let _ = writeln!(
                out,
                "  edited   {}{}",
                str_at(e, "/path"),
                if bool_at(e, "/created") {
                    " (created)"
                } else {
                    ""
                }
            );
            let backup = str_at(e, "/backupPath");
            if !backup.is_empty() {
                let _ = writeln!(out, "           backup {backup}");
            }
        }
        for k in arr_at(&report, "/skipped") {
            let _ = writeln!(
                out,
                "  skipped  {}: {}",
                str_at(k, "/id"),
                str_at(k, "/reason")
            );
        }
        let _ = writeln!(
            out,
            "the layout check now reports {} errors, {} warnings, {} notes",
            u64_at(&report, "/check/errors"),
            u64_at(&report, "/check/warnings"),
            u64_at(&report, "/check/infos"),
        );
        if !arr_at(&report, "/done").is_empty() {
            let _ = writeln!(
                out,
                "undo with: project fix undo {} {}",
                args.path,
                str_at(&report, "/applyId")
            );
        }
        out
    });
    if !arr_at(&report, "/skipped").is_empty() || bool_at(&report, "/cancelled") {
        s.note_warning();
    }
    Ok(())
}

/// `project fix undo`.
pub(crate) fn undo(s: &Session, path: &str, apply_id: &str) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call(
            "project_layout_fix_undo",
            json!({"projectId": id, "applyId": apply_id}),
        )?
        .value;
    s.emit(&reply, || {
        let mut out = format!("undone {}\n", str_at(&reply, "/applyId"));
        for p in arr_at(&reply, "/movedBack") {
            let _ = writeln!(out, "  moved back {}", p.as_str().unwrap_or(""));
        }
        for p in arr_at(&reply, "/restored") {
            let _ = writeln!(out, "  restored   {}", p.as_str().unwrap_or(""));
        }
        for p in arr_at(&reply, "/removedFolders") {
            let _ = writeln!(out, "  removed    {}", p.as_str().unwrap_or(""));
        }
        out
    });
    Ok(())
}

/// `project fix history`.
pub(crate) fn history(s: &Session, path: &str) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call("project_layout_fix_history", json!({"projectId": id}))?
        .value;
    s.emit(&reply, || {
        let journals = arr_at(&reply, "/journals");
        if journals.is_empty() {
            return "no fixes were applied to this project\n".to_owned();
        }
        let mut out = String::new();
        for j in journals {
            let state = if bool_at(j, "/undone") {
                "undone".to_owned()
            } else if bool_at(j, "/undoPossible") {
                "can be undone".to_owned()
            } else {
                format!("cannot be undone: {}", str_at(j, "/undoBlocker"))
            };
            let _ = writeln!(
                out,
                "{}  plan {}  {} of {} items done  {state}",
                str_at(j, "/applyId"),
                str_at(j, "/planId"),
                u64_at(j, "/itemsDone"),
                u64_at(j, "/itemsTotal")
            );
        }
        out
    });
    Ok(())
}
