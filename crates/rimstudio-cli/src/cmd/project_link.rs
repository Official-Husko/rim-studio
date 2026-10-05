//! `project link status|create|remove`: make a mod visible to the game by linking it into the game's
//! `Mods` folder, and take the link away again.
//!
//! `status` reads only. `create` changes nothing without `--yes`: it prints what it would create. A
//! refusal (the name is taken, the game is running, the folder is read only) exits with the failure code and
//! prints the reason and the command to run by hand.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::LinkModeArg;
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bool_at, render_diagnostics, str_at};
use crate::session::{Session, absolute};

fn project_id(s: &Session, path: &str) -> CliResult<String> {
    let abs = absolute(path)?;
    let summary = s.call("project_open", json!({"path": abs}))?.value;
    Ok(str_at(&summary, "/projectId").to_owned())
}

fn state_text(state: &str) -> &'static str {
    match state {
        "not-linked" => "not linked: the game cannot see this mod yet",
        "linked" => "linked: the game sees this mod",
        "linked-by-hand" => {
            "linked by hand: the game sees this mod (RimStudio did not make the link and will not remove it)"
        }
        "stale" => "stale link: it points at an older place of this mod",
        "foreign-link" => "something else is linked under this name",
        "foreign-folder" => "a folder with this name already exists in Mods",
        "copy" => "copied: the game sees a copy that does not follow later edits",
        "in-mods" => "the mod folder is inside the game's Mods folder: the game sees it",
        _ => "not available",
    }
}

fn render_status(st: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", state_text(str_at(st, "/state")));
    if bool_at(st, "/gameFound") {
        let _ = writeln!(out, "  game folder    {}", str_at(st, "/gameFolder"));
        let _ = writeln!(out, "  entry          {}", str_at(st, "/entryPath"));
    } else {
        let _ = writeln!(
            out,
            "  no RimWorld install is selected (run `detect --install PATH`)"
        );
    }
    let points = str_at(st, "/pointsTo");
    if !points.is_empty() {
        let _ = writeln!(out, "  points to      {points}");
    }
    let _ = writeln!(out, "  game running   {}", str_at(st, "/gameRunning"));
    let _ = writeln!(
        out,
        "  in the active list   {}",
        str_at(st, "/activeInGame")
    );
    if bool_at(st, "/hasCePatch") {
        let _ = writeln!(
            out,
            "  Combat Extended in the active list   {}",
            str_at(st, "/ceActiveInGame")
        );
    }
    let command = str_at(st, "/manualCommand");
    if !command.is_empty() && !bool_at(st, "/canRemove") {
        let _ = writeln!(out, "  by hand        {command}");
    }
    let diags = arr_at(st, "/diagnostics");
    if !diags.is_empty() {
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// `project link status`.
pub(crate) fn status(s: &Session, path: &str) -> CliResult {
    let id = project_id(s, path)?;
    let st = s
        .call("project_link_status", json!({"projectId": id}))?
        .value;
    s.emit(&st, || render_status(&st));
    Ok(())
}

fn finish(s: &Session, reply: &Value, verb: &str) -> CliResult {
    s.emit(reply, || {
        let mut out = String::new();
        if bool_at(reply, "/done") {
            let _ = writeln!(out, "{verb} done");
        } else {
            let _ = writeln!(
                out,
                "nothing was changed: {}",
                str_at(reply, "/refusal/message")
            );
        }
        out.push_str(&render_status(&reply["status"]));
        out
    });
    if bool_at(reply, "/done") {
        Ok(())
    } else {
        Err(CliError::Reported)
    }
}

/// `project link create`.
pub(crate) fn create(
    s: &Session,
    path: &str,
    mode: LinkModeArg,
    confirm_game_running: bool,
    yes: bool,
) -> CliResult {
    let abs = absolute(path)?;
    let id = project_id(s, path)?;
    if !yes {
        let st = s
            .call("project_link_status", json!({"projectId": id}))?
            .value;
        s.emit(&json!({"created": false, "status": st}), || {
            let mut out = render_status(&st);
            if bool_at(&st, "/canCreate") {
                let _ = writeln!(
                    out,
                    "would create {} -> {} ({}). Nothing was changed; add --yes to create it.",
                    str_at(&st, "/entryPath"),
                    abs,
                    mode.as_str()
                );
            }
            out
        });
        return Ok(());
    }
    let reply = s
        .call(
            "project_link_create",
            json!({"projectId": id, "mode": mode.as_str(), "confirmGameRunning": confirm_game_running}),
        )?
        .value;
    finish(s, &reply, "link")
}

/// `project link remove`.
pub(crate) fn remove(s: &Session, path: &str) -> CliResult {
    let id = project_id(s, path)?;
    let reply = s
        .call("project_link_remove", json!({"projectId": id}))?
        .value;
    finish(s, &reply, "unlink")
}
