//! `designer weapons` and `designer open`: the weapons a mod already defines, and opening one as a draft
//! that edits it where it stands (`designer_project_weapons`, `designer_open_weapon`).

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::{OpenArgs, WeaponsArgs};
use crate::cmd::drafts::project_id;
use crate::cmd::project;
use crate::error::CliResult;
use crate::fmt::{Table, arr_at, bool_at, count_diagnostics, render_diagnostics, str_at};
use crate::session::Session;

/// A number of the weapon list, or an empty cell when the definition gives none.
fn number(weapon: &Value, pointer: &str) -> String {
    weapon
        .pointer(pointer)
        .and_then(Value::as_f64)
        .map_or_else(String::new, |n| {
            let text = format!("{n:.2}");
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        })
}

/// Renders the weapon list.
#[must_use]
pub(crate) fn render_list(v: &Value) -> String {
    let weapons = arr_at(v, "/weapons");
    let mut out = String::new();
    if weapons.is_empty() {
        out.push_str("this mod defines no weapon the designer can open\n");
    } else {
        let mut t = Table::new(&[
            "defName", "kind", "damage", "range", "cooldown", "mass", "file", "notes",
        ]);
        for w in weapons {
            let mut notes: Vec<&str> = Vec::new();
            if bool_at(w, "/generatedPatch") {
                notes.push("generated CE patch");
            }
            if !str_at(w, "/draftId").is_empty() {
                notes.push("has a draft");
            }
            t.row(vec![
                str_at(w, "/defName").to_owned(),
                str_at(w, "/kind").to_owned(),
                number(w, "/numbers/damage"),
                number(w, "/numbers/range"),
                number(w, "/numbers/cooldown"),
                number(w, "/numbers/mass"),
                str_at(w, "/file").to_owned(),
                notes.join(", "),
            ]);
        }
        out.push_str(&t.render());
    }
    let diags = arr_at(v, "/diagnostics");
    if !diags.is_empty() {
        out.push_str("Diagnostics:\n");
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// `designer weapons --project DIR`.
///
/// # Errors
/// The envelope of `designer_project_weapons`.
pub(crate) fn list(s: &Session, args: &WeaponsArgs) -> CliResult {
    let summary = project::open(s, &args.project)?;
    let reply = s.call(
        "designer_project_weapons",
        json!({"projectId": project_id(&summary)}),
    )?;
    let value = reply.value;
    s.emit(&value, || render_list(&value));
    if count_diagnostics(arr_at(&value, "/diagnostics")).warnings > 0 {
        s.note_warning();
    }
    Ok(())
}

/// `designer open DEFNAME --project DIR [--reload]`.
///
/// # Errors
/// The envelope of `designer_open_weapon`.
pub(crate) fn open(s: &Session, args: &OpenArgs) -> CliResult {
    let summary = project::open(s, &args.project)?;
    let mut request = json!({"projectId": project_id(&summary), "defName": args.def_name});
    if args.reload {
        request["reload"] = json!(true);
    }
    let reply = s.call("designer_open_weapon", request)?;
    let value = reply.value;
    s.emit(&value, || {
        let id = str_at(&value, "/entry/id");
        let project = str_at(&summary, "/path");
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{} draft {id} for {} in {}",
            if bool_at(&value, "/reused") {
                "continued"
            } else {
                "created"
            },
            args.def_name,
            str_at(&value, "/file"),
        );
        for note in arr_at(&value, "/notes") {
            if let Some(note) = note.as_str() {
                let _ = writeln!(out, "  note: {note}");
            }
        }
        let _ = writeln!(
            out,
            "  next: rimstudio-cli designer drafts set {id} --project {project} --set KEY=VALUE   (for example ranged.range=30)\n  then: rimstudio-cli designer plan {id} --project {project}   (the files it would write: the definition is replaced in {})",
            str_at(&value, "/file"),
        );
        out
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_names_numbers_files_and_notes() {
        let v = json!({"weapons": [
            {"defName": "RS_G", "kind": "ranged", "file": "Defs/G.xml", "generatedPatch": true, "draftId": "d-1",
             "numbers": {"damage": 9.0, "range": 24.0, "cooldown": 1.25, "mass": 1.5}},
            {"defName": "RS_B", "kind": "melee", "file": "Defs/B.xml", "generatedPatch": false, "numbers": {"damage": 11.0}}
        ], "diagnostics": []});
        let text = render_list(&v);
        assert!(text.contains("RS_G") && text.contains("Defs/G.xml"));
        assert!(text.contains("generated CE patch, has a draft"));
        assert!(text.contains("1.25"));
        assert_eq!(number(&v["weapons"][0], "/numbers/damage"), "9");
        assert_eq!(number(&v["weapons"][1], "/numbers/range"), "");
    }

    #[test]
    fn an_empty_list_says_so() {
        assert!(render_list(&json!({"weapons": [], "diagnostics": []})).contains("no weapon"));
    }
}
