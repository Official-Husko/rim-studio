//! The weapons designer: reference list, calibration, drafts, preview, plan, apply and the quiz.
//!
//! The designer writes vanilla definitions. The Combat Extended patch is part of a plan only when the
//! command is given `--ce`; without it the Combat Extended block of a draft is removed from the request
//! before it is sent, so no Combat Extended file can appear. Every write needs an explicit project folder
//! and `apply` writes nothing without `--yes`.

use std::fmt::Write as _;
use std::io::{BufRead as _, IsTerminal as _, Write as _};

use serde_json::{Value, json};

use crate::cli::{ApplyArgs, DraftArgs, KindArg, NewArgs, PlanArgs, QuizArgs};
use crate::cmd::drafts::{self, Loaded, project_id};
use crate::cmd::project;
use crate::draftops::{
    apply_reference, apply_sets, ensure_ce, fill_from_suggestions, has_ce, new_draft, strip_ce,
};
use crate::error::{CliError, CliResult};
use crate::fmt::{
    Table, arr_at, bool_at, bytes, count_diagnostics, f64_at, num, opt_num, render_diagnostics,
    str_at, u64_at,
};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------------
// references and calibration
// ---------------------------------------------------------------------------------------------------

/// `designer refs KIND`: the reference weapons.
///
/// # Errors
/// The envelope of `designer_reference_list` (`designer.reference-unavailable` without an install).
pub(crate) fn refs(
    s: &Session,
    kind: KindArg,
    role: Option<&str>,
    tier: Option<&str>,
    limit: u32,
    offset: u32,
) -> CliResult {
    let mut request = json!({"kind": kind.wire(), "limit": limit, "offset": offset});
    if let Some(role) = role {
        request["role"] = json!(role);
    }
    if let Some(tier) = tier {
        request["tier"] = json!(tier.to_lowercase());
    }
    let reply = s.call("designer_reference_list", request)?;
    let v = &reply.value;
    s.emit(v, || {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Reference weapons ({}): {} in the class, showing {} from {}",
            str_at(v, "/classLabel"),
            u64_at(v, "/total"),
            arr_at(v, "/items").len(),
            u64_at(v, "/offset")
        );
        let mut t = Table::new(&["#", "defName", "label", "tier", "role", "strength"]);
        for i in arr_at(v, "/items") {
            t.row(vec![
                u64_at(i, "/index").to_string(),
                str_at(i, "/defName").to_owned(),
                str_at(i, "/label").to_owned(),
                str_at(i, "/tier").to_owned(),
                str_at(i, "/role").to_owned(),
                opt_num(f64_at(i, "/strength")),
            ]);
        }
        out.push_str(&t.render());
        out
    });
    Ok(())
}

/// `designer calibrate`: builds the pools and measures the estimates.
///
/// # Errors
/// The envelope of the `designer_calibrate` job.
pub(crate) fn calibrate(s: &Session, kind: KindArg, force: bool, threads: Option<u8>) -> CliResult {
    let mut request = json!({"kind": kind.wire(), "force": force});
    if let Some(t) = threads {
        request["threads"] = json!(t);
    }
    let reply = s.call("designer_calibrate", request)?;
    let v = &reply.value;
    s.emit(v, || {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "calibration of {} weapons: pool of {}{}",
            str_at(v, "/kind"),
            u64_at(v, "/poolSize"),
            if bool_at(v, "/fromCache") {
                " (from the cache)"
            } else {
                ""
            }
        );
        let mut t = Table::new(&[
            "variant",
            "macro median error",
            "mean questions",
            "max questions",
        ]);
        if let Some(map) = v.get("variants").and_then(Value::as_object) {
            for (name, m) in map {
                t.row(vec![
                    name.clone(),
                    opt_num(f64_at(m, "/macroMedianError")),
                    f64_at(m, "/meanQuestions").map_or("-".into(), num),
                    u64_at(m, "/maxQuestions").to_string(),
                ]);
            }
        }
        out.push_str(&t.render());
        if let Some(twin) = v.get("twin") {
            let _ = writeln!(
                out,
                "twin check: error {} with twins, {} without{}",
                opt_num(f64_at(twin, "/plainError")),
                opt_num(f64_at(twin, "/twinFreeError")),
                if bool_at(twin, "/optimistic") {
                    " (bands may be optimistic)"
                } else {
                    ""
                }
            );
        }
        out
    });
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------------------------------

fn find_reference(s: &Session, kind: KindArg, def_name: &str) -> CliResult<Value> {
    let mut offset = 0_u64;
    loop {
        let reply = s.call(
            "designer_reference_list",
            json!({"kind": kind.wire(), "limit": 500, "offset": offset}),
        )?;
        let items = arr_at(&reply.value, "/items");
        if let Some(found) = items.iter().find(|i| str_at(i, "/defName") == def_name) {
            return Ok(found.clone());
        }
        offset += items.len() as u64;
        if items.is_empty() || offset >= u64_at(&reply.value, "/total") {
            return Err(CliError::failed(format!(
                "`{def_name}` is not among the {} reference weapons; see `rimstudio-cli designer refs {}`",
                kind.wire(),
                kind.wire()
            )));
        }
    }
}

/// True when a failed `designer_clone` means the def could not be read as a whole, so the pool numbers of
/// the reference list are the only thing left to copy. A bad name, a missing source or a Combat Extended
/// conversion are the user's to fix and never fall back.
#[must_use]
fn clone_falls_back(code: &str) -> bool {
    code.starts_with("design.")
}

/// The next steps printed after a draft was created.
fn next_steps(id: &str, project: &str, cloned: bool) -> String {
    if cloned {
        format!(
            "  next: rimstudio-cli designer diff {id} --project {project} --set KEY=VALUE   (what a change does; for example ranged.damage=14)\n  then: rimstudio-cli designer plan {id} --project {project}   (the files it would write)\n"
        )
    } else {
        format!(
            "  next: rimstudio-cli designer preview {id} --project {project}\n  then: rimstudio-cli designer plan {id} --project {project}\n"
        )
    }
}

/// `designer new KIND --name N --from DEFNAME`: a real clone of a loaded weapon.
fn clone_cmd(s: &Session, args: &NewArgs, summary: &Value, from: &str) -> CliResult {
    let pid = project_id(summary);
    let mut request = json!({"projectId": pid, "source": from, "defName": args.name});
    if let Some(label) = &args.label {
        request["label"] = json!(label);
    }
    if let Some(prefix) = &args.prefix {
        request["modPrefix"] = json!(prefix);
    }
    match s.call("designer_clone", request) {
        Ok(reply) => finish_clone(s, args, summary, from, &reply.value),
        Err(e) if clone_falls_back(&e.code) => {
            let item = find_reference(s, args.kind, from)?;
            let mut draft = new_draft(args.kind, &args.name, args.label.as_deref())
                .map_err(CliError::failed)?;
            let filled = apply_reference(&mut draft, &item);
            apply_sets(&mut draft, &args.set).map_err(CliError::usage)?;
            let note = format!(
                "{from} could not be read as a whole ({}); only the {filled} pool numbers were copied, not its parent, projectile, cost list or tools",
                e.message
            );
            let id = drafts::save(s, &pid, None, &draft)?;
            let doc = json!({"id": id, "projectId": pid, "filled": filled, "draft": draft, "notes": [note]});
            s.emit(&doc, || {
                format!(
                    "created draft {id} for {} in {}\n  note: {note}\n{}",
                    args.name,
                    str_at(summary, "/path"),
                    next_steps(&id, str_at(summary, "/path"), false),
                )
            });
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn finish_clone(
    s: &Session,
    args: &NewArgs,
    summary: &Value,
    from: &str,
    reply: &Value,
) -> CliResult {
    let pid = project_id(summary);
    let id = str_at(reply, "/entry/id").to_owned();
    let mut draft = reply
        .pointer("/entry/draft")
        .cloned()
        .unwrap_or(Value::Null);
    if str_at(&draft, "/kind") != args.kind.wire() {
        let actual = str_at(&draft, "/kind").to_owned();
        // the clone was stored before its kind could be compared; take it back
        s.call("designer_draft_delete", json!({"projectId": pid, "id": id}))?;
        return Err(CliError::usage(format!(
            "{from} is a {actual} weapon, not a {} one; use `new {actual}` to clone it",
            args.kind.wire()
        )));
    }
    if !args.set.is_empty() {
        apply_sets(&mut draft, &args.set).map_err(CliError::usage)?;
        drafts::save(s, &pid, Some(&id), &draft)?;
    }
    let notes: Vec<String> = arr_at(reply, "/notes")
        .iter()
        .filter_map(|n| n.as_str().map(str::to_owned))
        .collect();
    let doc = json!({
        "id": id, "projectId": pid, "clonedFrom": from,
        "notes": notes, "draft": draft,
    });
    s.emit(&doc, || {
        let root = str_at(summary, "/path");
        let mut out = format!(
            "created draft {id} for {} in {root}\n  cloned from {from}: every field of the source was copied (numbers are marked anchor, inherited values stay inherited); Combat Extended stays off\n",
            args.name
        );
        for n in &notes {
            let _ = writeln!(out, "  note: {n}");
        }
        out.push_str(&next_steps(&id, root, true));
        out
    });
    Ok(())
}

/// `designer new KIND --name N --project DIR`: creates a draft in the project's draft store, a clone of a
/// loaded weapon with `--from`, or a new one whose numbers and structure come from the class pool with
/// `--strength`.
///
/// # Errors
/// A usage error for a bad `--set`, the envelope of a failing command.
pub(crate) fn new_draft_cmd(s: &Session, args: &NewArgs) -> CliResult {
    let summary = project::open(s, &args.project)?;
    if let Some(from) = &args.from {
        return clone_cmd(s, args, &summary, from);
    }
    let mut draft =
        new_draft(args.kind, &args.name, args.label.as_deref()).map_err(CliError::failed)?;
    let mut filled = 0;
    apply_sets(&mut draft, &args.set).map_err(CliError::usage)?;
    let mut notes: Vec<String> = Vec::new();
    if let Some(strength) = args.strength {
        let preview = s.call("designer_preview", json!({"draft": draft}))?;
        filled +=
            fill_from_suggestions(&mut draft, arr_at(&preview.value, "/suggestions"), strength);
        match s.call("designer_structure_defaults", json!({"draft": draft})) {
            Ok(reply) => {
                if let Some(next) = reply.value.get("draft").filter(|d| d.is_object()) {
                    draft = next.clone();
                }
                filled += arr_at(&reply.value, "/filled").len();
                notes.extend(
                    arr_at(&reply.value, "/notes")
                        .iter()
                        .filter_map(|n| n.as_str().map(str::to_owned)),
                );
            }
            Err(e) => notes.push(format!(
                "the parent, projectile and cost list stay empty: {}",
                e.message
            )),
        }
    }
    let id = drafts::save(s, &project_id(&summary), None, &draft)?;
    let doc = json!({
        "id": id, "projectId": project_id(&summary), "filled": filled,
        "notes": notes, "draft": draft,
    });
    s.emit(&doc, || {
        let root = str_at(&summary, "/path");
        let mut out = format!(
            "created draft {id} for {} in {root}\n  filled {filled} numbers and structure fields from the class pool; the rest stay empty\n",
            args.name
        );
        for n in &notes {
            let _ = writeln!(out, "  note: {n}");
        }
        out.push_str(&next_steps(&id, root, false));
        out
    });
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// preview
// ---------------------------------------------------------------------------------------------------

struct Prepared {
    summary: Option<Value>,
    loaded: Loaded,
}

fn prepare(s: &Session, args: &DraftArgs, need_project: bool) -> CliResult<Prepared> {
    let summary = match (&args.project, need_project) {
        (Some(dir), _) => Some(project::open(s, dir)?),
        (None, true) => {
            return Err(CliError::usage(
                "this command needs the mod project folder: add --project DIR (nothing is ever written to the game folders)",
            ));
        }
        (None, false) => None,
    };
    let mut loaded = drafts::load(s, &args.draft, summary.as_ref())?;
    apply_sets(&mut loaded.draft, &args.set).map_err(CliError::usage)?;
    Ok(Prepared { summary, loaded })
}

/// Renders a preview response.
#[must_use]
pub(crate) fn render_preview(def_name: &str, v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Preview of {def_name}");
    let mut t = Table::new(&["readout", "value", "unit", "group"]);
    for r in arr_at(v, "/readouts") {
        t.row(vec![
            str_at(r, "/key").to_owned(),
            opt_num(f64_at(r, "/value")),
            str_at(r, "/unit").to_owned(),
            str_at(r, "/group").to_owned(),
        ]);
    }
    if !t.is_empty() {
        out.push_str("Exact readouts:\n");
        out.push_str(&t.render());
    }
    let mut t = Table::new(&[
        "field",
        "stat",
        "suggested",
        "source",
        "band (p10 / median / p90)",
        "locked",
    ]);
    for x in arr_at(v, "/suggestions") {
        let band = x.get("band").map_or_else(
            || "-".to_owned(),
            |b| {
                format!(
                    "{} / {} / {}",
                    opt_num(f64_at(b, "/p10")),
                    opt_num(f64_at(b, "/median")),
                    opt_num(f64_at(b, "/p90"))
                )
            },
        );
        t.row(vec![
            str_at(x, "/field").to_owned(),
            str_at(x, "/stat").to_owned(),
            opt_num(f64_at(x, "/value")),
            str_at(x, "/source").to_owned(),
            band,
            bool_at(x, "/locked").to_string(),
        ]);
    }
    if !t.is_empty() {
        out.push_str("Suggestions (estimates, never written without you):\n");
        out.push_str(&t.render());
    }
    if let Some(e) = v.get("estimate") {
        let _ = writeln!(
            out,
            "Estimate: class {} ({} weapons, level {}), strength {} (percentile {})",
            str_at(e, "/classLabel"),
            u64_at(e, "/classN"),
            str_at(e, "/level"),
            opt_num(f64_at(e, "/strength")),
            opt_num(f64_at(e, "/strengthPercentile")),
        );
        for n in arr_at(e, "/notes") {
            let _ = writeln!(out, "  note: {}", n.as_str().unwrap_or(""));
        }
    }
    let diags = arr_at(v, "/diagnostics");
    if !diags.is_empty() {
        out.push_str("Diagnostics:\n");
        out.push_str(&render_diagnostics(diags));
    }
    out
}

fn value_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".to_owned(),
        Some(Value::Number(n)) => n.as_f64().map_or_else(|| n.to_string(), num),
        Some(Value::String(t)) => t.clone(),
        Some(other) => other.to_string(),
    }
}

/// Renders a clone diff response.
#[must_use]
pub(crate) fn render_diff(def_name: &str, v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{def_name} against its source {} ({})",
        str_at(v, "/source"),
        str_at(v, "/sourceLabel")
    );
    let changes = arr_at(v, "/changes");
    if changes.is_empty() {
        out.push_str("No field differs from the source.\n");
    } else {
        let mut t = Table::new(&["field", "label", "source", "now"]);
        for c in changes {
            t.row(vec![
                str_at(c, "/field").to_owned(),
                str_at(c, "/label").to_owned(),
                value_text(c.get("old")),
                value_text(c.get("new")),
            ]);
        }
        out.push_str("Changed fields:\n");
        out.push_str(&t.render());
    }
    let mut t = Table::new(&["readout", "source", "now", "change", "unit"]);
    for r in arr_at(v, "/readouts") {
        let (old, new) = (f64_at(r, "/old"), f64_at(r, "/new"));
        if old.is_none() && new.is_none() {
            continue;
        }
        t.row(vec![
            str_at(r, "/key").to_owned(),
            opt_num(old),
            opt_num(new),
            f64_at(r, "/delta").map_or_else(
                || "-".to_owned(),
                |d| format!("{}{}", if d > 0.0 { "+" } else { "" }, num(d)),
            ),
            str_at(r, "/unit").to_owned(),
        ]);
    }
    if !t.is_empty() {
        out.push_str("Effect on the exact readouts:\n");
        out.push_str(&t.render());
    }
    for n in arr_at(v, "/notes") {
        let _ = writeln!(out, "note: {}", n.as_str().unwrap_or(""));
    }
    out
}

/// `designer diff DRAFT [--project DIR]`: the changed fields of a cloned draft against its source and the
/// effect on the exact readouts; changes nothing. `--set` tries a change without saving it.
///
/// # Errors
/// The envelope of `designer_clone_diff` (`designer.invalid-draft` for a draft that is not a clone).
pub(crate) fn diff(s: &Session, args: &DraftArgs) -> CliResult {
    let p = prepare(s, args, false)?;
    let reply = s.call("designer_clone_diff", json!({"draft": p.loaded.draft}))?;
    s.emit(&reply.value, || {
        render_diff(def_name_of(&p.loaded.draft), &reply.value)
    });
    Ok(())
}

fn def_name_of(draft: &Value) -> &str {
    str_at(draft, "/spec/identity/defName")
}

/// `designer preview DRAFT`: readouts, suggestions and diagnostics of a draft.
///
/// # Errors
/// The envelope of `designer_preview`.
pub(crate) fn preview(s: &Session, args: &DraftArgs) -> CliResult {
    let p = prepare(s, args, false)?;
    let reply = s.call("designer_preview", json!({"draft": p.loaded.draft}))?;
    let c = count_diagnostics(arr_at(&reply.value, "/diagnostics"));
    s.emit(&reply.value, || {
        render_preview(def_name_of(&p.loaded.draft), &reply.value)
    });
    if c.errors + c.warnings > 0 {
        s.note_warning();
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// Combat Extended suggestions
// ---------------------------------------------------------------------------------------------------

/// The source of a suggestion as a short phrase: `predicted ratio, 4 weapons`, `identity`, `typed`.
fn source_text(source: Option<&Value>) -> String {
    let Some(v) = source else {
        return "-".to_owned();
    };
    let n = u64_at(v, "/n");
    match str_at(v, "/kind") {
        "predicted" => format!("predicted {} from {n}", str_at(v, "/predictor")),
        "identity" => format!("identity ({n} weapons)"),
        "first-of-set" => "first of the ammo set".to_owned(),
        other => other.to_owned(),
    }
}

fn band_text(band: Option<&Value>, which: &str) -> String {
    band.map_or_else(
        || "-".to_owned(),
        |b| {
            format!(
                "{} to {}",
                opt_num(f64_at(b, &format!("/{which}/low"))),
                opt_num(f64_at(b, &format!("/{which}/high")))
            )
        },
    )
}

fn percent(error: Option<f64>) -> String {
    error.map_or_else(|| "-".to_owned(), |e| format!("{:.0}%", 100.0 * e))
}

/// Renders a Combat Extended suggestion.
#[must_use]
pub(crate) fn render_ce_suggestion(v: &Value) -> String {
    let mut out = String::new();
    let def = str_at(v, "/defName");
    if !bool_at(v, "/available") {
        let _ = writeln!(
            out,
            "No Combat Extended suggestions for {def}: {}",
            str_at(v, "/reason")
        );
        return out;
    }
    let toggle = if bool_at(v, "/toggleOn") {
        "on"
    } else {
        "off; nothing here turns it on"
    };
    let _ = writeln!(
        out,
        "Combat Extended suggestions for {def} ({}), patch toggle {toggle}",
        str_at(v, "/kind")
    );
    let _ = writeln!(
        out,
        "  estimated from your own conversions: {} ({} converted weapons of this kind)",
        v.get("classLabel").and_then(Value::as_str).unwrap_or("-"),
        u64_at(v, "/pool")
    );
    let mut t = Table::new(&[
        "field",
        "status",
        "held",
        "suggested",
        "source",
        "50% band",
        "rating",
        "error",
        "req",
    ]);
    let add = |t: &mut Table, f: &Value| {
        let held = f64_at(f, "/held").map_or_else(
            || "-".to_owned(),
            |h| format!("{} ({})", num(h), source_text(f.get("heldSource"))),
        );
        t.row(vec![
            str_at(f, "/field").to_owned(),
            str_at(f, "/status").to_owned(),
            held,
            opt_num(f64_at(f, "/value")),
            source_text(f.get("source")),
            band_text(f.get("band"), "p50"),
            f.get("rating")
                .and_then(Value::as_str)
                .unwrap_or("-")
                .to_owned(),
            percent(f64_at(f, "/error")),
            if bool_at(f, "/required") { "yes" } else { "no" }.to_owned(),
        ]);
    };
    for f in arr_at(v, "/fields") {
        add(&mut t, f);
    }
    if !t.is_empty() {
        out.push_str("Numbers of the block:\n");
        out.push_str(&t.render());
    }
    let mut c = Table::new(&[
        "field",
        "status",
        "held or derived",
        "candidates, best first",
    ]);
    for f in arr_at(v, "/choices") {
        let shown = f
            .get("held")
            .or_else(|| f.get("value"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        let names: Vec<String> = arr_at(f, "/candidates")
            .iter()
            .take(5)
            .map(|c| {
                if u64_at(c, "/usedBy") > 0 {
                    format!("{} ({} guns)", str_at(c, "/name"), u64_at(c, "/usedBy"))
                } else {
                    str_at(c, "/name").to_owned()
                }
            })
            .collect();
        c.row(vec![
            str_at(f, "/field").to_owned(),
            str_at(f, "/status").to_owned(),
            shown.to_owned(),
            if names.is_empty() {
                "-".to_owned()
            } else {
                names.join(", ")
            },
        ]);
    }
    if !c.is_empty() {
        out.push_str("Choices of the block:\n");
        out.push_str(&c.render());
    }
    let mut p = Table::new(&["number the patch writes", "value", "source", "rating"]);
    for f in arr_at(v, "/patchNumbers") {
        p.row(vec![
            str_at(f, "/field").to_owned(),
            opt_num(f64_at(f, "/value")),
            source_text(f.get("source")),
            f.get("rating")
                .and_then(Value::as_str)
                .unwrap_or("-")
                .to_owned(),
        ]);
    }
    if !p.is_empty() {
        out.push_str("Numbers the patch derives itself (the block has no field for them):\n");
        out.push_str(&p.render());
    }
    let asks = arr_at(v, "/asks/items");
    if !asks.is_empty() {
        out.push_str("Open questions (never answered for you):\n");
        for a in asks {
            let _ = write!(out, "  {}: {}", str_at(a, "/field"), str_at(a, "/label"));
            if let Some(r) = a.get("suggestion").and_then(Value::as_f64) {
                let _ = write!(out, " [rejected estimate {}]", num(r));
            }
            out.push('\n');
            if let Some(r) = a.get("reason").and_then(Value::as_str) {
                let _ = writeln!(out, "      {r}");
            }
        }
    }
    let list = |key: &str| -> Vec<String> {
        arr_at(v, key)
            .iter()
            .filter_map(|x| x.as_str().map(str::to_owned))
            .collect()
    };
    let missing = list("/missing");
    if !missing.is_empty() {
        let _ = writeln!(out, "Required and not held yet: {}", missing.join(", "));
    }
    let still = list("/stillMissingAfterAccept");
    let _ = if still.is_empty() {
        writeln!(
            out,
            "Accepting every suggestion would leave nothing required open."
        )
    } else {
        writeln!(
            out,
            "Still open after accepting every suggestion: {}",
            still.join(", ")
        )
    };
    for n in arr_at(v, "/notes") {
        let _ = writeln!(out, "  note: {}", n.as_str().unwrap_or(""));
    }
    out.push_str(
        "Nothing was changed. To use the suggestions: designer plan DRAFT --project DIR --ce --accept-suggestions [FIELD ...]\n",
    );
    out
}

/// `designer ce-suggest DRAFT`: what the user's own conversions say about the Combat Extended block of a
/// draft. Read only; the draft's toggle is never changed (the block may be absent). Exit code 3 when no
/// Combat Extended data is loaded.
///
/// # Errors
/// The envelope of `designer_ce_suggest`.
pub(crate) fn ce_suggest(s: &Session, args: &DraftArgs) -> CliResult {
    let p = prepare(s, args, false)?;
    let reply = s.call("designer_ce_suggest", json!({"draft": p.loaded.draft}))?;
    let v = reply.value;
    s.emit(&v, || render_ce_suggestion(&v));
    if !bool_at(&v, "/available") {
        s.note_warning();
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// plan and apply
// ---------------------------------------------------------------------------------------------------

/// The export request of a draft: the Combat Extended block is removed unless `ce` is true, in which case
/// it is created when absent.
///
/// `accept` is the opt in to suggestions: it lists the fields to accept (empty means every derived field).
/// It is sent only together with `ce`, because suggestions never turn the patch on.
#[must_use]
pub(crate) fn export_request(
    project_id: &str,
    draft: &Value,
    ce: bool,
    accept: Option<&[String]>,
) -> Value {
    let mut d = draft.clone();
    if ce {
        ensure_ce(&mut d);
    } else {
        strip_ce(&mut d);
    }
    let mut request = json!({"projectId": project_id, "draft": d});
    if let (true, Some(fields)) = (ce, accept) {
        request["acceptSuggestions"] = json!({"fields": fields});
    }
    request
}

fn join_path(root: &str, relative: &str) -> String {
    std::path::Path::new(root)
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

const CONTENT_LINES: usize = 60;

/// Renders a plan: the files with absolute paths, the diffs and the diagnostics.
#[must_use]
pub(crate) fn render_plan(plan: &Value, root: &str, show_content: bool) -> String {
    let mut out = String::new();
    let files = arr_at(plan, "/files");
    let _ = writeln!(
        out,
        "Plan {} for the project folder {root}",
        str_at(plan, "/planId")
    );
    if files.is_empty() {
        out.push_str("  no files\n");
    }
    for f in files {
        let _ = writeln!(
            out,
            "  {:<13} {}  [{}, {}]",
            str_at(f, "/action"),
            join_path(root, str_at(f, "/path")),
            str_at(f, "/kind"),
            bytes(u64_at(f, "/bytes")),
        );
    }
    for f in files {
        let path = str_at(f, "/path");
        if let Some(diff) = f.get("diff").and_then(Value::as_str) {
            let _ = writeln!(out, "\nDiff of {path}:");
            for line in diff.lines() {
                let _ = writeln!(out, "    {line}");
            }
        } else if show_content && str_at(f, "/action") == "create" {
            let _ = writeln!(out, "\nNew file {path}:");
            let text = str_at(f, "/rendered");
            for line in text.lines().take(CONTENT_LINES) {
                let _ = writeln!(out, "    {line}");
            }
            let total = text.lines().count();
            if total > CONTENT_LINES {
                let _ = writeln!(out, "    ... {} more lines", total - CONTENT_LINES);
            }
        }
    }
    let diags = arr_at(plan, "/diagnostics");
    if !diags.is_empty() {
        out.push_str("\nDiagnostics:\n");
        out.push_str(&render_diagnostics(diags));
    }
    out
}

fn root_of(summary: &Value) -> &str {
    str_at(summary, "/path")
}

fn make_plan(
    s: &Session,
    summary: &Value,
    draft: &Value,
    ce: bool,
    accept: Option<&[String]>,
) -> CliResult<(Value, Value)> {
    if !ce && has_ce(draft) {
        s.note("note: the draft has Combat Extended settings; they are ignored without --ce");
    }
    let request = export_request(&project_id(summary), draft, ce, accept);
    let plan = s.call("designer_export_plan", request.clone())?.value;
    if !ce
        && arr_at(&plan, "/files")
            .iter()
            .any(|f| str_at(f, "/kind") == "ce-patch")
    {
        return Err(CliError::failed(
            "internal check: a Combat Extended file appeared in a vanilla plan; nothing was written",
        ));
    }
    Ok((request, plan))
}

fn finish_plan_flags(s: &Session, plan: &Value) -> CliResult {
    let c = count_diagnostics(arr_at(plan, "/diagnostics"));
    if bool_at(plan, "/hasErrors") || c.errors > 0 {
        return Err(CliError::Reported);
    }
    if c.warnings > 0 {
        s.note_warning();
    }
    Ok(())
}

/// `designer plan DRAFT --project DIR [--ce]`: the files a draft would write; writes nothing.
///
/// # Errors
/// A usage error without `--project`, the envelope of `designer_export_plan`, and a reported failure
/// (exit 1) when the plan has errors.
pub(crate) fn plan(s: &Session, args: &PlanArgs) -> CliResult {
    let p = prepare(s, &args.draft, true)?;
    let summary = p.summary.unwrap_or(Value::Null);
    let (_, plan) = make_plan(
        s,
        &summary,
        &p.loaded.draft,
        args.ce,
        args.accept_suggestions.as_deref(),
    )?;
    let doc = json!({"ce": args.ce, "plan": plan});
    s.emit(&doc, || {
        let mut out = render_plan(&plan, root_of(&summary), true);
        if bool_at(&plan, "/hasErrors") {
            out.push_str("\nThe plan has errors and cannot be applied. Nothing was written.\n");
        } else {
            out.push_str("\nNothing was written. Use `apply` to write these files.\n");
        }
        out
    });
    finish_plan_flags(s, &plan)
}

/// `designer apply DRAFT --project DIR [--ce] [--yes]`: writes the plan; without `--yes` it only prints
/// the diff and the list of files.
///
/// # Errors
/// As [`plan`], plus the envelope of `designer_apply_plan` (`designer.plan-stale`).
pub(crate) fn apply(s: &Session, args: &ApplyArgs) -> CliResult {
    let p = prepare(s, &args.draft, true)?;
    let summary = p.summary.unwrap_or(Value::Null);
    let (request, plan) = make_plan(
        s,
        &summary,
        &p.loaded.draft,
        args.ce,
        args.accept_suggestions.as_deref(),
    )?;
    if bool_at(&plan, "/hasErrors") {
        s.emit(
            &json!({"applied": false, "ce": args.ce, "plan": plan}),
            || {
                let mut out = render_plan(&plan, root_of(&summary), false);
                out.push_str("\nThe plan has errors; nothing was written.\n");
                out
            },
        );
        return Err(CliError::Reported);
    }
    if !args.yes {
        s.emit(&json!({"applied": false, "ce": args.ce, "plan": plan}), || {
            let mut out = render_plan(&plan, root_of(&summary), true);
            out.push_str("\nNothing was written. Run the same command with --yes to write exactly these files.\n");
            out
        });
        finish_plan_flags(s, &plan)?;
        return Ok(());
    }
    let report = s
        .call(
            "designer_apply_plan",
            json!({
                "planId": str_at(&plan, "/planId"),
                "request": request,
                "backup": !args.no_backup,
                "dryApply": !args.no_dry_apply,
            }),
        )?
        .value;
    let doc = json!({"applied": true, "ce": args.ce, "plan": plan, "report": report});
    s.emit(&doc, || render_apply(&report, root_of(&summary)));
    note_apply_flags(s, &report);
    Ok(())
}

/// Renders an apply report.
#[must_use]
pub(crate) fn render_apply(report: &Value, root: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Applied plan {} to {root}", str_at(report, "/planId"));
    for f in arr_at(report, "/written") {
        let _ = writeln!(
            out,
            "  wrote {} ({}, {}{})",
            join_path(root, str_at(f, "/path")),
            str_at(f, "/action"),
            bytes(u64_at(f, "/bytes")),
            if bool_at(f, "/verified") {
                ", verified"
            } else {
                ", NOT verified"
            },
        );
        if let Some(b) = f.get("backupPath").and_then(Value::as_str) {
            let _ = writeln!(out, "    backup: {}", join_path(root, b));
        }
    }
    for u in arr_at(report, "/unchanged") {
        let _ = writeln!(
            out,
            "  unchanged {}",
            join_path(root, u.as_str().unwrap_or(""))
        );
    }
    match report.get("dryApplyOk").and_then(Value::as_bool) {
        Some(true) => out.push_str("  dry run of the patch: ok\n"),
        Some(false) => out.push_str("  dry run of the patch: FAILED\n"),
        None => {}
    }
    let diags = arr_at(report, "/diagnostics");
    if !diags.is_empty() {
        out.push_str("Diagnostics:\n");
        out.push_str(&render_diagnostics(diags));
    }
    out
}

/// Marks warnings of an apply report: any warning, an unverified file or a failed dry run.
pub(crate) fn note_apply_flags(s: &Session, report: &Value) {
    let c = count_diagnostics(arr_at(report, "/diagnostics"));
    let unverified = arr_at(report, "/written")
        .iter()
        .any(|f| !bool_at(f, "/verified"));
    if c.errors + c.warnings > 0
        || unverified
        || report.get("dryApplyOk") == Some(&Value::Bool(false))
    {
        s.note_warning();
    }
}

// ---------------------------------------------------------------------------------------------------
// quiz
// ---------------------------------------------------------------------------------------------------

/// "1 weapon" or "N weapons".
fn weapons(count: u64) -> String {
    if count == 1 {
        "1 weapon".to_owned()
    } else {
        format!("{count} weapons")
    }
}

/// The choices of a question: a label and the answer each one stands for.
#[must_use]
pub(crate) fn choices(question: &Value) -> Vec<(String, Value)> {
    let kind = str_at(question, "/kind");
    let named = |key: &str| -> Vec<(String, Value)> {
        arr_at(question, "/options")
            .iter()
            .map(|o| {
                let name = str_at(o, "/name").to_owned();
                (
                    format!("{name} ({})", weapons(u64_at(o, "/count"))),
                    json!({"kind": key, key: name}),
                )
            })
            .collect()
    };
    match kind {
        "tier" => arr_at(question, "/options")
            .iter()
            .map(|o| {
                (
                    format!("{} ({})", str_at(o, "/label"), weapons(u64_at(o, "/count"))),
                    json!({"kind": "tier", "tier": u64_at(o, "/tier")}),
                )
            })
            .collect(),
        "role" => named("role"),
        "group" => named("group"),
        "compare" => vec![
            ("weaker".into(), json!({"kind": "weaker"})),
            ("same".into(), json!({"kind": "same"})),
            ("stronger".into(), json!({"kind": "stronger"})),
        ],
        "closer-to" => vec![
            (
                "closer to the lower one".into(),
                json!({"kind": "closer-to-lower"}),
            ),
            (
                "closer to the upper one".into(),
                json!({"kind": "closer-to-upper"}),
            ),
        ],
        "vs-anchor" => vec![
            ("lower".into(), json!({"kind": "bucket", "bucket": "lower"})),
            (
                "similar".into(),
                json!({"kind": "bucket", "bucket": "similar"}),
            ),
            (
                "higher".into(),
                json!({"kind": "bucket", "bucket": "higher"}),
            ),
        ],
        "interval" => arr_at(question, "/bins")
            .iter()
            .enumerate()
            .map(|(i, b)| {
                (
                    str_at(b, "/label").to_owned(),
                    json!({"kind": "bin", "index": i}),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Renders the open prompt of a quiz step as numbered choices.
#[must_use]
pub(crate) fn render_prompt(prompt: &Value) -> String {
    let q = &prompt["question"];
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\nQuestion {} of about {}",
        u64_at(prompt, "/number"),
        u64_at(prompt, "/aboutTotal")
    );
    match str_at(q, "/kind") {
        "tier" => out.push_str("Which tech tier is the weapon?\n"),
        "role" => out.push_str("What role does the weapon play?\n"),
        "group" => out.push_str("Which group is the weapon in?\n"),
        "compare" => {
            let a = &q["anchor"];
            let _ = writeln!(
                out,
                "Compared with {} ({}), the weapon is:",
                str_at(a, "/label"),
                str_at(a, "/id")
            );
        }
        "closer-to" => {
            let _ = writeln!(
                out,
                "Is the weapon closer to {} or to {}?",
                str_at(q, "/lower/label"),
                str_at(q, "/upper/label")
            );
        }
        "interval" => {
            let _ = writeln!(out, "In which range does the {} fall?", str_at(q, "/stat"));
        }
        "vs-anchor" => {
            let _ = writeln!(
                out,
                "Is the {} lower, similar or higher than {} ({})?",
                str_at(q, "/stat"),
                str_at(q, "/anchor/label"),
                opt_num(f64_at(q, "/value"))
            );
        }
        other => {
            let _ = writeln!(out, "Question of kind {other}");
        }
    }
    for (i, (label, _)) in choices(q).iter().enumerate() {
        let _ = writeln!(out, "  {}) {label}", i + 1);
    }
    let typed = if accepts_typed(q) {
        ", =NUMBER to type a value"
    } else {
        ""
    };
    let _ = writeln!(
        out,
        "  (number or name; ? not sure, s skip, b back, d done and use what is known{typed})"
    );
    out
}

/// True for the questions that take a typed number: the interval and the versus anchor questions.
fn accepts_typed(question: &Value) -> bool {
    matches!(str_at(question, "/kind"), "interval" | "vs-anchor")
}

/// Turns what the person typed into an answer of the open question. The reply `back` is the pseudo
/// answer `{"kind": "back"}`; it goes to `designer_quiz_back`, never to `designer_quiz_answer`.
///
/// # Errors
/// A message when the text matches no choice.
pub(crate) fn parse_reply(question: &Value, input: &str) -> Result<Value, String> {
    let text = input.trim();
    let lower = text.to_lowercase();
    match lower.as_str() {
        "?" | "not sure" | "notsure" => return Ok(json!({"kind": "not-sure"})),
        "s" | "skip" => return Ok(json!({"kind": "skip"})),
        "d" | "done" | "use" => return Ok(json!({"kind": "use-what-i-have"})),
        "b" | "back" => return Ok(json!({"kind": "back"})),
        _ => {}
    }
    if let Some(number) = lower.strip_prefix('=') {
        if !accepts_typed(question) {
            return Err(
                "this question takes a choice; a typed number answers the range questions"
                    .to_owned(),
            );
        }
        let value: f64 = number
            .trim()
            .parse()
            .map_err(|_| format!("`{number}` is not a number"))?;
        return Ok(json!({"kind": "typed", "value": value}));
    }
    let options = choices(question);
    if let Ok(n) = lower.parse::<usize>() {
        return n
            .checked_sub(1)
            .and_then(|i| options.get(i))
            .map(|(_, a)| a.clone())
            .ok_or_else(|| format!("choose a number from 1 to {}", options.len()));
    }
    let matches: Vec<&(String, Value)> = options
        .iter()
        .filter(|(label, _)| label.to_lowercase().starts_with(&lower))
        .collect();
    match matches.as_slice() {
        [(_, a)] => Ok(a.clone()),
        [] => Err(format!("`{text}` matches no choice")),
        _ => Err(format!(
            "`{text}` matches several choices; type more letters or the number"
        )),
    }
}

enum Answers {
    Interactive,
    List(Vec<Value>, usize),
    ById(serde_json::Map<String, Value>),
}

impl Answers {
    fn from_file(path: &str) -> CliResult<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CliError::usage(format!("{path} cannot be read: {e}")))?;
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Array(items)) => Ok(Self::List(items, 0)),
            Ok(Value::Object(map)) => Ok(Self::ById(map)),
            Ok(_) => Err(CliError::usage(format!(
                "{path} must hold a JSON array or object of answers"
            ))),
            Err(e) => Err(CliError::usage(format!("{path} is not valid JSON: {e}"))),
        }
    }

    fn next(&mut self, question_id: &str, question: &Value) -> CliResult<Option<Value>> {
        match self {
            Self::Interactive => loop {
                let mut line = String::new();
                print!("> ");
                let _ = std::io::stdout().flush();
                let read = std::io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(|e| CliError::failed(format!("standard input cannot be read: {e}")))?;
                if read == 0 {
                    return Ok(None);
                }
                match parse_reply(question, &line) {
                    Ok(a) => return Ok(Some(a)),
                    Err(m) => println!("{m}"),
                }
            },
            Self::List(items, at) => {
                let a = items.get(*at).cloned();
                *at += 1;
                Ok(a)
            }
            Self::ById(map) => Ok(map.get(question_id).cloned()),
        }
    }
}

/// `designer quiz DRAFT --project DIR`: answers the estimate quiz and stores the answers in the draft.
///
/// # Errors
/// A usage error when there is no terminal and no `--answers`, the envelope of a failing command.
pub(crate) fn quiz(s: &Session, args: &QuizArgs) -> CliResult {
    let mut answers = match &args.answers {
        Some(path) => Answers::from_file(path)?,
        None => {
            if s.json || !std::io::stdin().is_terminal() {
                return Err(CliError::usage(
                    "the quiz needs a terminal to ask its questions; give --answers FILE to answer from a file",
                ));
            }
            Answers::Interactive
        }
    };
    let summary = project::open(s, &args.project)?;
    let pid = project_id(&summary);
    let loaded = drafts::load(s, &args.draft, Some(&summary))?;
    let mut draft = loaded.draft;
    draft["calibration"] = json!("quiz");
    let interactive = matches!(answers, Answers::Interactive);
    let mut asked = 0_u32;
    let mut stopped = String::new();
    let mut last_step = Value::Null;
    let mut rounds = 0_u32;
    while asked < 200 && rounds < 2000 {
        rounds += 1;
        let step = match s.call("designer_quiz_next", json!({"draft": draft})) {
            Ok(r) => r.value,
            Err(e) if e.code == "designer.quiz-finished" => break,
            Err(e) => return Err(e.into()),
        };
        let finished = bool_at(&step, "/finished");
        let Some(prompt) = step.get("prompt").filter(|p| p.is_object()).cloned() else {
            last_step = step;
            break;
        };
        last_step = step;
        if finished {
            break;
        }
        if interactive {
            print!("{}", render_prompt(&prompt));
        }
        let id = str_at(&prompt, "/id").to_owned();
        let Some(answer) = answers.next(&id, &prompt["question"])? else {
            stopped = if interactive {
                "input ended".to_owned()
            } else {
                format!("no answer for question {id}")
            };
            break;
        };
        if answer["kind"] == json!("back") {
            // Back is the registry action: the toolkit drops the last stored answer of the draft.
            match s.call("designer_quiz_back", json!({"draft": draft})) {
                Ok(reply) => {
                    draft = reply.value["draft"].clone();
                    last_step = reply.value["step"].clone();
                    asked = asked.saturating_sub(1);
                }
                Err(e) if e.code == "designer.quiz-wrong-answer" => {
                    println!("there is no answer to take back");
                }
                Err(e) => return Err(e.into()),
            }
            continue;
        }
        match s.call(
            "designer_quiz_answer",
            json!({"draft": draft, "questionId": id, "answer": answer}),
        ) {
            Ok(reply) => {
                draft = reply.value["draft"].clone();
                last_step = reply.value["step"].clone();
                asked += 1;
            }
            Err(e) if interactive && e.code == "designer.quiz-wrong-answer" => {
                println!("{}", e.message);
            }
            Err(e) => {
                // Keep what was answered so far before the command fails.
                let _ = drafts::save(s, &pid, loaded.id.as_deref(), &draft);
                return Err(e.into());
            }
        }
    }
    let saved = drafts::save(s, &pid, loaded.id.as_deref(), &draft)?;
    let doc = json!({"id": saved, "answered": asked, "stopped": stopped, "step": last_step, "draft": draft});
    s.emit(&doc, || {
        let mut out = format!("answered {asked} questions; saved draft {saved}\n");
        if !stopped.is_empty() {
            let _ = writeln!(out, "stopped early: {stopped}");
        }
        if let Some(e) = last_step.get("estimate") {
            let _ = writeln!(
                out,
                "estimate: class {}, strength {} (percentile {})",
                str_at(e, "/classLabel"),
                opt_num(f64_at(e, "/strength")),
                opt_num(f64_at(e, "/strengthPercentile")),
            );
        }
        out
    });
    if !stopped.is_empty() && !interactive {
        s.note_warning();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier_question() -> Value {
        json!({"kind": "tier", "options": [
            {"tier": 2, "label": "Industrial", "count": 5},
            {"tier": 3, "label": "Spacer", "count": 4}]})
    }

    #[test]
    fn a_vanilla_request_drops_the_ce_block_and_ce_adds_it() {
        let mut d = new_draft(KindArg::Ranged, "RS_A", None).unwrap_or(Value::Null);
        crate::draftops::set_value(&mut d, "ce.ammoSet", "RS_AmmoSetA").unwrap_or_default();
        assert!(has_ce(&d));
        let vanilla = export_request("p1", &d, false, None);
        assert!(vanilla["draft"]["spec"].get("ce").is_none());
        let with = export_request("p1", &d, true, None);
        assert_eq!(with["draft"]["spec"]["ce"]["ammoSet"], json!("RS_AmmoSetA"));
        let mut bare = new_draft(KindArg::Ranged, "RS_A", None).unwrap_or(Value::Null);
        strip_ce(&mut bare);
        assert!(export_request("p1", &bare, true, None)["draft"]["spec"]["ce"].is_object());
        assert!(d["spec"]["ce"].is_object(), "the input is not changed");
    }

    #[test]
    fn suggestions_are_requested_only_together_with_ce() {
        let d = new_draft(KindArg::Ranged, "RS_A", None).unwrap_or(Value::Null);
        let fields = vec!["bulk".to_owned()];
        let with = export_request("p1", &d, true, Some(&fields));
        assert_eq!(with["acceptSuggestions"]["fields"], json!(["bulk"]));
        let all = export_request("p1", &d, true, Some(&[]));
        assert_eq!(all["acceptSuggestions"]["fields"], json!([]));
        let vanilla = export_request("p1", &d, false, Some(&fields));
        assert!(vanilla.get("acceptSuggestions").is_none());
        assert!(
            export_request("p1", &d, true, None)
                .get("acceptSuggestions")
                .is_none()
        );
    }

    #[test]
    fn a_suggestion_renders_with_numbers_choices_and_open_questions() {
        let v = json!({
            "kind": "ranged", "defName": "RS_A", "available": true, "toggleOn": false,
            "classLabel": "based on the 4 nearest of 12", "pool": 12,
            "fields": [{"field": "/ce/bulk", "label": "CE bulk", "required": true, "status": "derived",
                "value": 7.0, "source": {"kind": "predicted", "predictor": "ratio", "n": 4},
                "band": {"p50": {"low": 6.0, "high": 8.0}, "p80": {"low": 5.0, "high": 9.0}},
                "rating": "reliable", "error": 0.1}],
            "choices": [{"field": "/ce/ammoSet", "label": "ammo", "kind": "choice", "required": true,
                "status": "ask", "candidates": [{"name": "RS_Set", "usedBy": 3, "score": 1.0}]}],
            "patchNumbers": [], "asks": {"items": [{"field": "/ce/ammoSet", "label": "Which?", "kind": "choice",
                "options": ["RS_Set"], "reason": "pick one"}]},
            "missing": ["/ce/ammoSet"], "stillMissingAfterAccept": ["/ce/ammoSet"], "notes": []
        });
        let text = render_ce_suggestion(&v);
        assert!(
            text.contains("patch toggle off; nothing here turns it on"),
            "{text}"
        );
        assert!(text.contains("predicted ratio from 4"), "{text}");
        assert!(text.contains("RS_Set (3 guns)"), "{text}");
        assert!(
            text.contains("Still open after accepting every suggestion: /ce/ammoSet"),
            "{text}"
        );
        let none = render_ce_suggestion(
            &json!({"defName": "RS_A", "available": false, "reason": "no CE"}),
        );
        assert!(
            none.contains("No Combat Extended suggestions for RS_A: no CE"),
            "{none}"
        );
    }

    #[test]
    fn replies_select_choices_by_number_name_or_prefix() {
        let q = tier_question();
        assert_eq!(parse_reply(&q, "1"), Ok(json!({"kind": "tier", "tier": 2})));
        assert_eq!(
            parse_reply(&q, "spacer"),
            Ok(json!({"kind": "tier", "tier": 3}))
        );
        assert_eq!(
            parse_reply(&q, "Ind"),
            Ok(json!({"kind": "tier", "tier": 2}))
        );
        assert!(parse_reply(&q, "3").is_err());
        assert!(parse_reply(&q, "0").is_err());
        assert!(parse_reply(&q, "zzz").is_err());
    }

    #[test]
    fn special_replies_work_for_every_question() {
        let q = json!({"kind": "compare", "anchor": {"label": "a", "id": "a"}});
        assert_eq!(parse_reply(&q, "?"), Ok(json!({"kind": "not-sure"})));
        assert_eq!(parse_reply(&q, "skip"), Ok(json!({"kind": "skip"})));
        assert_eq!(parse_reply(&q, "d"), Ok(json!({"kind": "use-what-i-have"})));
        assert_eq!(parse_reply(&q, "back"), Ok(json!({"kind": "back"})));
        assert_eq!(parse_reply(&q, "B"), Ok(json!({"kind": "back"})));
        assert_eq!(parse_reply(&q, "str"), Ok(json!({"kind": "stronger"})));
        assert!(parse_reply(&q, "s").is_ok());
    }

    #[test]
    fn a_typed_value_is_accepted_only_by_the_range_questions() {
        let range = json!({"kind": "interval", "stat": "range", "bins": [{"label": "low"}]});
        assert_eq!(
            parse_reply(&range, "=12.5"),
            Ok(json!({"kind": "typed", "value": 12.5}))
        );
        assert!(parse_reply(&range, "=abc").is_err());
        let group = json!({"kind": "group", "options": [{"name": "single", "count": 1}]});
        assert!(parse_reply(&group, "=3").is_err());
        let prompt = json!({"number": 1, "aboutTotal": 3, "question": group});
        assert!(!render_prompt(&prompt).contains("=NUMBER"));
        assert!(render_prompt(&prompt).contains("1 weapon)"));
    }

    #[test]
    fn interval_bins_answer_by_zero_based_index() {
        let q = json!({"kind": "interval", "stat": "range", "bins": [{"label": "low"}, {"label": "high"}]});
        assert_eq!(parse_reply(&q, "2"), Ok(json!({"kind": "bin", "index": 1})));
    }

    #[test]
    fn a_prompt_lists_numbered_choices() {
        let prompt =
            json!({"id": "tier", "number": 1, "aboutTotal": 8, "question": tier_question()});
        let text = render_prompt(&prompt);
        assert!(text.contains("Question 1 of about 8"));
        assert!(text.contains("1) Industrial (5 weapons)"));
        assert!(text.contains("2) Spacer (4 weapons)"));
    }

    #[test]
    fn a_plan_lists_absolute_paths_kinds_and_diffs() {
        let plan = json!({"planId": "abc", "hasErrors": false, "diagnostics": [],
            "files": [
              {"path": "Defs/RS_A.xml", "kind": "vanilla-defs", "action": "create", "bytes": 120, "rendered": "line1\nline2"},
              {"path": "LoadFolders.xml", "kind": "load-folders", "action": "update-region", "bytes": 10, "rendered": "", "diff": "-a\n+b"}]});
        let text = render_plan(&plan, "/proj", true);
        assert!(text.contains("/proj/Defs/RS_A.xml"), "{text}");
        assert!(text.contains("[vanilla-defs, 120 B]"));
        assert!(text.contains("    -a"));
        assert!(text.contains("New file Defs/RS_A.xml"));
        assert!(text.contains("    line2"));
        let quiet = render_plan(&plan, "/proj", false);
        assert!(!quiet.contains("New file"));
    }

    #[test]
    fn an_apply_report_names_files_backups_and_the_dry_run() {
        let report = json!({"planId": "abc", "diagnostics": [], "unchanged": ["About/About.xml"], "dryApplyOk": true,
            "written": [{"path": "Defs/A.xml", "action": "create", "bytes": 10, "verified": true, "backupPath": "x.bak"}]});
        let text = render_apply(&report, "/proj");
        assert!(
            text.contains("wrote /proj/Defs/A.xml (create, 10 B, verified)"),
            "{text}"
        );
        assert!(text.contains("backup: /proj/x.bak"));
        assert!(text.contains("unchanged /proj/About/About.xml"));
        assert!(text.contains("dry run of the patch: ok"));
    }

    #[test]
    fn only_a_def_that_cannot_be_read_falls_back_to_the_pool_numbers() {
        assert!(clone_falls_back("design.invalid-input"));
        for code in [
            "designer.invalid-draft",
            "designer.reference-unavailable",
            "io.write-failed",
        ] {
            assert!(!clone_falls_back(code), "{code}");
        }
    }

    #[test]
    fn a_diff_renders_changes_deltas_and_notes() {
        let v = json!({
            "source": "RS_Gun03", "sourceLabel": "gun three",
            "changes": [{"field": "/ranged/damage", "label": "ranged damage", "old": 11.0, "new": 13.0},
                        {"field": "/weaponTags/1", "label": "weapon tag 2", "new": "RS_Auto"}],
            "readouts": [{"key": "dps", "unit": "damage-per-second", "old": 4.0, "new": 4.5, "delta": 0.5},
                         {"key": "strength-index", "unit": "number"}],
            "notes": ["the projectile is shared"]
        });
        let text = render_diff("RS_Copy", &v);
        assert!(
            text.contains("RS_Copy against its source RS_Gun03 (gun three)"),
            "{text}"
        );
        assert!(text.contains("/ranged/damage"), "{text}");
        assert!(text.contains("RS_Auto"), "{text}");
        assert!(text.contains("+0.5"), "{text}");
        assert!(
            !text.contains("strength-index"),
            "a readout without a value is left out: {text}"
        );
        assert!(text.contains("note: the projectile is shared"), "{text}");
    }

    #[test]
    fn a_diff_without_changes_says_so() {
        let v = json!({"source": "RS_Gun03", "sourceLabel": "g", "changes": [], "readouts": []});
        assert!(render_diff("RS_Copy", &v).contains("No field differs from the source."));
    }

    #[test]
    fn next_steps_differ_for_a_clone() {
        let clone = next_steps("d-1", "/p", true);
        assert!(clone.contains("designer diff d-1 --project /p"), "{clone}");
        let fresh = next_steps("d-1", "/p", false);
        assert!(fresh.contains("designer preview d-1"), "{fresh}");
        assert!(!fresh.contains("designer diff"));
    }
}
