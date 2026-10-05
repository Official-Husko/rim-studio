//! `convert scan|plan|apply`: the automatic Combat Extended patch generator for an existing mod.
//!
//! The conversion never changes the existing definitions. It writes a patch file in the gated Combat
//! Extended folder and edits `LoadFolders.xml`. It only runs when asked for, and `apply` writes nothing
//! without `--yes`.

use std::fmt::Write as _;

use serde_json::{Map, Value, json};

use crate::cli::{ConvertCmd, ConvertTarget, KindArg};
use crate::cmd::designer::{note_apply_flags, render_apply, render_plan};
use crate::cmd::drafts::project_id;
use crate::cmd::project;
use crate::draftops::{apply_sets, new_draft};
use crate::error::{CliError, CliResult};
use crate::fmt::{Table, arr_at, bool_at, count_diagnostics, render_diagnostics, str_at, u64_at};
use crate::session::Session;

/// Runs the convert scan job for a project.
///
/// # Errors
/// The envelope of `designer_convert_scan` (`designer.reference-unavailable` without Combat Extended).
pub(crate) fn scan_project(
    s: &Session,
    summary: &Value,
    include_converted: bool,
) -> CliResult<Value> {
    let request = json!({"projectId": project_id(summary), "includeConverted": include_converted});
    let reply = s.call("designer_convert_scan", request)?;
    let mut value = reply.value;
    if !reply.diagnostics.is_empty() {
        let mut all = arr_at(&value, "/diagnostics").to_vec();
        all.extend(reply.diagnostics);
        value["diagnostics"] = Value::Array(all);
    }
    Ok(value)
}

/// Renders a scan result.
#[must_use]
pub(crate) fn render_scan(v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} can be converted, {} already converted, {} unsupported, {} without a target",
        u64_at(v, "/counts/notConverted"),
        u64_at(v, "/counts/alreadyCe"),
        u64_at(v, "/counts/unsupportedKind"),
        u64_at(v, "/counts/targetNotFound"),
    );
    let mut t = Table::new(&[
        "defName",
        "kind",
        "status",
        "open questions",
        "family",
        "file",
    ]);
    for c in arr_at(v, "/candidates") {
        t.row(vec![
            str_at(c, "/defName").to_owned(),
            str_at(c, "/kind").to_owned(),
            str_at(c, "/status").to_owned(),
            arr_at(c, "/asks").len().to_string(),
            str_at(c, "/family").to_owned(),
            str_at(c, "/file").to_owned(),
        ]);
    }
    out.push_str(&t.render());
    for c in arr_at(v, "/candidates") {
        let asks = arr_at(c, "/asks");
        if asks.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\nQuestions for {}:", str_at(c, "/defName"));
        for a in asks {
            let options: Vec<&str> = arr_at(a, "/options")
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let _ = writeln!(
                out,
                "  {} [{}]{}",
                str_at(a, "/label"),
                str_at(a, "/field"),
                if options.is_empty() {
                    String::new()
                } else {
                    format!(" one of: {}", options.join(", "))
                }
            );
            out.push_str(&render_ask_detail(a));
        }
    }
    if shares_a_family(v) {
        out.push_str(
            "\nWeapons with the same family key can share one answer set: put it in an answers file under \
             groups with that family (see `convert plan --help`).\n",
        );
    }
    let diags = arr_at(v, "/diagnostics");
    if !diags.is_empty() {
        out.push_str("Diagnostics:\n");
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// True when two or more weapons with open questions share a family key.
fn shares_a_family(scan: &Value) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    arr_at(scan, "/candidates")
        .iter()
        .filter(|c| !arr_at(c, "/asks").is_empty())
        .map(|c| str_at(c, "/family"))
        .filter(|f| !f.is_empty())
        .any(|f| !seen.insert(f.to_owned()))
}

/// The lines that say why a number is asked and what the rejected estimate was. Empty for a question
/// that has neither.
fn render_ask_detail(ask: &Value) -> String {
    let mut out = String::new();
    if let Some(reason) = ask.get("reason").and_then(Value::as_str) {
        let _ = writeln!(out, "      why it is asked: {reason}");
    }
    if let Some(s) = ask.get("suggestion").and_then(Value::as_f64) {
        let _ = writeln!(
            out,
            "      rejected estimate: {s} (never written unless you answer with it)"
        );
    }
    out
}

/// What an answers file holds: the groups in order (the defaults first) and the answers per definition.
struct AnswerFile {
    groups: Vec<Value>,
    per_def: Map<String, Value>,
}

/// Turns the `groups` list of a file into request groups: `family` and `defNames` stay, `answers` must be
/// an object.
fn read_groups(list: &Value, path: &str) -> CliResult<Vec<Value>> {
    let Some(items) = list.as_array() else {
        return Err(CliError::usage(format!("{path}: groups must be a list")));
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let n = i + 1;
        let Some(group) = item.as_object() else {
            return Err(CliError::usage(format!(
                "{path}: group {n} must be an object"
            )));
        };
        if let Some(extra) = group
            .keys()
            .find(|k| !matches!(k.as_str(), "family" | "defNames" | "answers"))
        {
            return Err(CliError::usage(format!(
                "{path}: group {n} has an unknown member `{extra}` (use family, defNames and answers)"
            )));
        }
        if !group.get("answers").is_some_and(Value::is_object) {
            return Err(CliError::usage(format!(
                "{path}: group {n} needs an answers object"
            )));
        }
        if group.get("family").is_some_and(|f| !f.is_string())
            || group
                .get("defNames")
                .is_some_and(|d| !d.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
        {
            return Err(CliError::usage(format!(
                "{path}: in group {n} family is a text and defNames a list of texts"
            )));
        }
        if group.get("family").is_none() && group.get("defNames").is_none() {
            return Err(CliError::usage(format!(
                "{path}: group {n} names no family and no defNames (put answers for every weapon under default)"
            )));
        }
        out.push(item.clone());
    }
    Ok(out)
}

fn read_answers(path: &str) -> CliResult<AnswerFile> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("{path} cannot be read: {e}")))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{path} is not valid JSON: {e}")))?;
    let Value::Object(mut map) = v else {
        return Err(CliError::usage(format!(
            "{path} must hold a JSON object of answers"
        )));
    };
    if ["defs", "default", "groups"]
        .iter()
        .any(|k| map.contains_key(*k))
    {
        let per_def = map
            .remove("defs")
            .and_then(|d| d.as_object().cloned())
            .unwrap_or_default();
        let mut groups = Vec::new();
        if let Some(default) = map.remove("default") {
            if !default.is_object() {
                return Err(CliError::usage(format!(
                    "{path}: default must be an object of answers"
                )));
            }
            groups.push(json!({"answers": default}));
        }
        if let Some(list) = map.remove("groups") {
            groups.extend(read_groups(&list, path)?);
        }
        Ok(AnswerFile { groups, per_def })
    } else {
        // a bare answers object applies to every weapon
        Ok(AnswerFile {
            groups: vec![json!({"answers": Value::Object(map)})],
            per_def: Map::new(),
        })
    }
}

/// Merges `from` into `into`; objects merge member by member, everything else is replaced.
fn merge_into(into: &mut Map<String, Value>, from: &Map<String, Value>) {
    for (k, v) in from {
        match (into.get_mut(k), v) {
            (Some(Value::Object(a)), Value::Object(b)) => merge_into(a, b),
            _ => {
                into.insert(k.clone(), v.clone());
            }
        }
    }
}

/// The answers of one definition: the default object, then the per definition object, then `--set`.
///
/// # Errors
/// A usage error for a bad `--set`.
pub(crate) fn answers_for(
    default: &Value,
    per_def: &Map<String, Value>,
    def_name: &str,
    sets: &[String],
) -> CliResult<Value> {
    let mut merged = default.as_object().cloned().unwrap_or_default();
    if let Some(Value::Object(own)) = per_def.get(def_name) {
        merge_into(&mut merged, own);
    }
    let mut holder = json!({"spec": Value::Object(merged)});
    apply_sets(&mut holder, sets).map_err(CliError::usage)?;
    Ok(holder["spec"].clone())
}

fn select(s: &Session, summary: &Value, target: &ConvertTarget) -> CliResult<Vec<String>> {
    if target.all && !target.defs.is_empty() {
        return Err(CliError::usage("give --all or --def NAME, not both"));
    }
    if target.all {
        let scan = scan_project(s, summary, false)?;
        let names: Vec<String> = arr_at(&scan, "/candidates")
            .iter()
            .filter(|c| str_at(c, "/status") == "not-converted")
            .map(|c| str_at(c, "/defName").to_owned())
            .collect();
        if names.is_empty() {
            return Err(CliError::failed(
                "no weapon of this mod can be converted (see `convert scan`)",
            ));
        }
        return Ok(names);
    }
    if target.defs.is_empty() {
        return Err(CliError::usage(
            "say what to convert: --def NAME (repeatable) or --all",
        ));
    }
    Ok(target.defs.clone())
}

fn convert_request(
    project_id: &str,
    def_name: &str,
    answers: &Value,
    groups: &[Value],
) -> CliResult<Value> {
    let filler = new_draft(KindArg::Ranged, "RS_Unused", None).map_err(CliError::failed)?;
    let mut convert = json!({"defName": def_name, "answers": answers});
    if !groups.is_empty() {
        convert["groups"] = Value::Array(groups.to_vec());
    }
    Ok(json!({
        "projectId": project_id,
        "draft": filler,
        "convert": convert,
    }))
}

fn needs_answers(plan: &Value) -> bool {
    arr_at(plan, "/diagnostics")
        .iter()
        .any(|d| str_at(d, "/code") == "designer.convert-needs-answer")
}

struct Targets {
    summary: Value,
    names: Vec<String>,
    groups: Vec<Value>,
    per_def: Map<String, Value>,
}

fn targets(s: &Session, target: &ConvertTarget) -> CliResult<Targets> {
    let summary = project::open(s, &target.project)?;
    let AnswerFile { groups, per_def } = match &target.answers {
        Some(path) => read_answers(path)?,
        None => AnswerFile {
            groups: Vec::new(),
            per_def: Map::new(),
        },
    };
    let names = select(s, &summary, target)?;
    Ok(Targets {
        summary,
        names,
        groups,
        per_def,
    })
}

fn plan_one(s: &Session, t: &Targets, name: &str, sets: &[String]) -> CliResult<(Value, Value)> {
    // the groups (the defaults first) go to the toolkit, which picks the ones that apply to the weapon; the
    // answers of the weapon itself and --set win over every group
    let answers = answers_for(&json!({}), &t.per_def, name, sets)?;
    let request = convert_request(&project_id(&t.summary), name, &answers, &t.groups)?;
    let plan = s.call("designer_export_plan", request.clone())?.value;
    Ok((request, plan))
}

/// Runs a `convert` command.
///
/// # Errors
/// A usage error for bad arguments, the envelope of a failing command, a reported failure when a plan has
/// errors or open questions.
pub(crate) fn run(s: &Session, cmd: &ConvertCmd) -> CliResult {
    match cmd {
        ConvertCmd::Scan {
            project: dir,
            hide_converted,
        } => {
            let summary = project::open(s, dir)?;
            let scan = scan_project(s, &summary, !hide_converted)?;
            s.emit(&scan, || render_scan(&scan));
            if count_diagnostics(arr_at(&scan, "/diagnostics")).warnings > 0 {
                s.note_warning();
            }
            Ok(())
        }
        ConvertCmd::Plan(target) => plan_all(s, target, false, true, true),
        ConvertCmd::Apply {
            target,
            yes,
            no_backup,
            no_dry_apply,
        } => plan_all(s, target, *yes, !no_backup, !no_dry_apply),
    }
}

fn plan_all(
    s: &Session,
    target: &ConvertTarget,
    write: bool,
    backup: bool,
    dry_apply: bool,
) -> CliResult {
    let writing = write;
    let t = targets(s, target)?;
    let root = str_at(&t.summary, "/path").to_owned();
    let mut results: Vec<Value> = Vec::new();
    let mut text = String::new();
    let mut failed = false;
    let mut asked = false;
    let mut error: Option<CliError> = None;
    for name in &t.names {
        let (request, plan) = match plan_one(s, &t, name, &target.set) {
            Ok(x) => x,
            Err(e) => {
                error = Some(e);
                break;
            }
        };
        let _ = writeln!(text, "== {name}");
        text.push_str(&render_plan(&plan, &root, true));
        let plan_failed = bool_at(&plan, "/hasErrors");
        failed |= plan_failed;
        asked |= needs_answers(&plan);
        let mut entry = json!({"defName": name, "plan": plan});
        if writing && !plan_failed {
            match s.call(
                "designer_apply_plan",
                json!({"planId": str_at(&plan, "/planId"), "request": request, "backup": backup, "dryApply": dry_apply}),
            ) {
                Ok(r) => {
                    text.push_str(&render_apply(&r.value, &root));
                    note_apply_flags(s, &r.value);
                    entry["report"] = r.value;
                }
                Err(e) => {
                    error = Some(e.into());
                    results.push(entry);
                    break;
                }
            }
        } else if count_diagnostics(arr_at(&plan, "/diagnostics")).warnings > 0 {
            s.note_warning();
        }
        results.push(entry);
        text.push('\n');
    }
    let doc = json!({"applied": writing && !failed && error.is_none(), "results": results});
    s.emit(&doc, || {
        let mut out = text.clone();
        if asked {
            out.push_str("Some questions are open. Answer them with --set KEY=VALUE (for example --set ammoSet=NAME, --set oneHanded=false, --set overrides.bulk=6.5) or --answers FILE; `convert scan` lists them.\n");
        }
        if !writing {
            out.push_str("Nothing was written.");
            if !failed {
                out.push_str(" Run `convert apply` with --yes to write exactly these files.");
            }
            out.push('\n');
        } else if failed {
            out.push_str("No file was written for the conversions that have errors.\n");
        }
        out
    });
    if let Some(e) = error {
        return Err(e);
    }
    if failed {
        return Err(CliError::Reported);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_merge_default_then_per_definition_then_sets() {
        let default = json!({"ammoSet": "RS_A", "oneHanded": false});
        let mut per = Map::new();
        per.insert("RS_G".into(), json!({"ammoSet": "RS_B"}));
        let a = answers_for(&default, &per, "RS_G", &["weaponTagClass=RS_C".to_owned()])
            .unwrap_or_default();
        assert_eq!(a["ammoSet"], json!("RS_B"));
        assert_eq!(a["oneHanded"], json!(false));
        assert_eq!(a["weaponTagClass"], json!("RS_C"));
        let other = answers_for(&default, &per, "RS_Other", &[]).unwrap_or_default();
        assert_eq!(other["ammoSet"], json!("RS_A"));
    }

    #[test]
    fn per_definition_objects_merge_into_the_default_member_by_member() {
        let default = json!({"overrides": {"bulk": 1, "swayFactor": 2}});
        let mut per = Map::new();
        per.insert("RS_G".into(), json!({"overrides": {"swayFactor": 5}}));
        let a = answers_for(&default, &per, "RS_G", &[]).unwrap_or_default();
        assert_eq!(a["overrides"], json!({"bulk": 1, "swayFactor": 5}));
    }

    #[test]
    fn overrides_are_typed_numbers_and_flags_are_booleans() {
        let a = answers_for(
            &json!({}),
            &Map::new(),
            "RS_G",
            &[
                "overrides.magazineSize=30".to_owned(),
                "beltFed=true".to_owned(),
            ],
        )
        .unwrap_or_default();
        assert_eq!(
            a["overrides"]["magazineSize"],
            json!({"value": 30, "source": "typed"})
        );
        assert_eq!(a["beltFed"], json!(true));
    }

    #[test]
    fn a_bad_set_is_a_usage_error() {
        assert!(matches!(
            answers_for(&json!({}), &Map::new(), "x", &["nothing".to_owned()]),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn a_scan_renders_counts_questions_and_files() {
        let v = json!({"counts": {"notConverted": 1, "alreadyCe": 0, "unsupportedKind": 0, "targetNotFound": 0},
            "diagnostics": [],
            "candidates": [{"defName": "RS_G", "kind": "ranged", "status": "not-converted", "file": "Defs/G.xml",
                            "asks": [{"label": "Ammo set", "field": "/ce/ammoSet"}, {"label": "Tag class", "field": "/ce/weaponTagClass"},
                                     {"label": "Shot spread", "field": "/ce/shotSpread", "reason": "the estimate 1.25 is not written: it is unreliable", "suggestion": 1.25}]}]});
        let text = render_scan(&v);
        assert!(text.contains("1 can be converted"));
        assert!(text.contains("Questions for RS_G"));
        assert!(text.contains("Ammo set [/ce/ammoSet]"));
        assert!(text.contains("Defs/G.xml"));
        assert!(
            text.contains("why it is asked: the estimate 1.25 is not written: it is unreliable")
        );
        assert!(text.contains("rejected estimate: 1.25"));
    }

    #[test]
    fn the_scan_hints_at_group_answers_only_when_a_family_has_several_open_weapons() {
        let one = json!({"counts": {}, "diagnostics": [], "candidates": [
            {"defName": "RS_A", "family": "f", "asks": [{"label": "x", "field": "/ce/ammoSet"}]}]});
        assert!(!render_scan(&one).contains("share one answer set"));
        let two = json!({"counts": {}, "diagnostics": [], "candidates": [
            {"defName": "RS_A", "family": "f", "asks": [{"label": "x", "field": "/ce/ammoSet"}]},
            {"defName": "RS_B", "family": "f", "asks": [{"label": "x", "field": "/ce/ammoSet"}]}]});
        assert!(render_scan(&two).contains("share one answer set"));
    }

    #[test]
    fn a_question_without_an_estimate_shows_no_detail_lines() {
        assert_eq!(render_ask_detail(&json!({"label": "Ammo set"})), "");
    }

    #[test]
    fn an_open_question_is_recognised_by_its_code() {
        let plan = json!({"diagnostics": [{"code": "designer.convert-needs-answer"}]});
        assert!(needs_answers(&plan));
        assert!(!needs_answers(&json!({"diagnostics": []})));
    }
}
