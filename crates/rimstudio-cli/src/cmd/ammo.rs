//! `designer ammo`: the catalogue of the ammo sets of the user's Combat Extended and the suggestions for a
//! custom ammo type.
//!
//! `designer ammo catalog` lists every ammo set of the install (searchable, filterable, paged). With
//! `--draft` the sets that the relevance ranking of that design suggests are marked. `designer ammo suggest
//! CLASS` fills a new ammo type from the nearest of your own ammunition, or copies an existing type with
//! `--copy-from AMMO`. Both are read only and never turn the Combat Extended patch on.
//!
//! The custom ammo of a draft is set with `--set` on the draft commands (`ce.customAmmo.name=Foo`,
//! `ce.customAmmo.types.0.projectile.damage=16`, or a whole type as JSON from `designer ammo suggest
//! --json`) or by writing `ce.customAmmo` into the draft file. The files of the caliber are part of the plan
//! of `designer plan --ce`.

use std::fmt::Write as _;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{drafts, project};
use crate::draftops::apply_sets;
use crate::error::{CliError, CliResult};
use crate::fmt::{Table, arr_at, bool_at, f64_at, num, opt_num, str_at, u64_at};
use crate::session::Session;

/// `designer ammo` arguments.
#[derive(Debug, Args)]
pub(crate) struct AmmoArgs {
    /// What to do.
    #[command(subcommand)]
    pub(crate) cmd: AmmoCmd,
}

/// The subcommands of `designer ammo`.
#[derive(Debug, Subcommand)]
pub(crate) enum AmmoCmd {
    /// List the ammo sets of your Combat Extended with their ammo types and projectile numbers.
    Catalog(CatalogArgs),
    /// Suggest the values of a new ammo type, or copy an existing one.
    Suggest(SuggestArgs),
}

/// `designer ammo catalog` arguments.
#[derive(Debug, Args)]
pub(crate) struct CatalogArgs {
    /// Words that must all appear in a set (names, labels, caliber, ammo, classes, example weapons).
    #[arg(value_name = "WORDS")]
    pub(crate) query: Vec<String>,
    /// Only this caliber or family (as the facets name them).
    #[arg(long, value_name = "NAME")]
    pub(crate) caliber: Option<String>,
    /// Only sets with an ammo type of this ammo class (a def name).
    #[arg(long = "class", value_name = "CLASS")]
    pub(crate) class: Option<String>,
    /// The page, counted from zero.
    #[arg(long, default_value_t = 0)]
    pub(crate) page: u32,
    /// Entries per page.
    #[arg(long, default_value_t = 25)]
    pub(crate) page_size: u32,
    /// A draft (id or file) whose design ranks the sets.
    #[arg(long, value_name = "DRAFT")]
    pub(crate) draft: Option<String>,
    /// The mod project folder of the draft.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: Option<String>,
    /// Override a value of the draft for this run only: `key=value` (repeatable).
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub(crate) set: Vec<String>,
}

/// `designer ammo suggest` arguments.
#[derive(Debug, Args)]
pub(crate) struct SuggestArgs {
    /// The ammo class of the new type (a def name such as the ones `designer ammo catalog` lists).
    pub(crate) class: String,
    /// A caliber name for your own reference.
    #[arg(long, value_name = "TEXT")]
    pub(crate) caliber: Option<String>,
    /// The damage you want; used as given and places the type among your ammunition.
    #[arg(long, value_name = "N")]
    pub(crate) damage: Option<f64>,
    /// The speed you want.
    #[arg(long, value_name = "N")]
    pub(crate) speed: Option<f64>,
    /// An existing ammo set whose ammunition is the yardstick.
    #[arg(long, value_name = "SET")]
    pub(crate) similar_set: Option<String>,
    /// Start from this existing ammo item: every value is copied.
    #[arg(long, value_name = "AMMO")]
    pub(crate) copy_from: Option<String>,
}

/// Runs a `designer ammo` command.
///
/// # Errors
/// The envelope of the command called.
pub(crate) fn run(s: &Session, args: &AmmoArgs) -> CliResult {
    match &args.cmd {
        AmmoCmd::Catalog(a) => catalog(s, a),
        AmmoCmd::Suggest(a) => suggest(s, a),
    }
}

fn catalog(s: &Session, a: &CatalogArgs) -> CliResult {
    let mut request = json!({"page": a.page, "pageSize": a.page_size});
    if !a.query.is_empty() {
        request["query"] = json!(a.query.join(" "));
    }
    if let Some(c) = &a.caliber {
        request["caliber"] = json!(c);
    }
    if let Some(c) = &a.class {
        request["class"] = json!(c);
    }
    if let Some(reference) = &a.draft {
        let summary = match &a.project {
            Some(dir) => Some(project::open(s, dir)?),
            None => None,
        };
        let mut loaded = drafts::load(s, reference, summary.as_ref())?;
        apply_sets(&mut loaded.draft, &a.set).map_err(CliError::usage)?;
        request["draft"] = loaded.draft;
    } else if !a.set.is_empty() {
        return Err(CliError::usage("--set needs --draft"));
    }
    let reply = s.call("designer_ce_ammo_catalog", request)?;
    let v = reply.value;
    s.emit(&v, || render_catalog(&v));
    if !bool_at(&v, "/available") {
        s.note_warning();
    }
    Ok(())
}

fn types_of(entry: &Value) -> String {
    arr_at(entry, "/types")
        .iter()
        .map(|t| {
            let class = str_at(t, "/ammoClassLabel");
            if class.is_empty() {
                str_at(t, "/ammoDef")
            } else {
                class
            }
            .to_owned()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_catalog(v: &Value) -> String {
    if !bool_at(v, "/available") {
        return format!("No ammunition to list: {}\n", str_at(v, "/reason"));
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} ammo sets in the install, {} match; page {} of {} per page",
        u64_at(v, "/total"),
        u64_at(v, "/matching"),
        u64_at(v, "/page"),
        u64_at(v, "/pageSize")
    );
    let mut t = Table::new(&["set", "caliber", "family", "types", "weapons", "notes"]);
    for e in arr_at(v, "/entries") {
        let mut notes = Vec::new();
        if bool_at(e, "/suggested") {
            notes.push(format!("suggested {}", opt_num(f64_at(e, "/score"))));
        }
        if bool_at(e, "/generic") {
            notes.push(format!("generic ({} similar)", u64_at(e, "/similarSets")));
        }
        if !str_at(e, "/similarTo").is_empty() {
            notes.push(format!("similar to {}", str_at(e, "/similarTo")));
        }
        t.row(vec![
            str_at(e, "/defName").to_owned(),
            str_at(e, "/caliber").to_owned(),
            str_at(e, "/family").to_owned(),
            types_of(e),
            u64_at(e, "/weaponCount").to_string(),
            notes.join(", "),
        ]);
    }
    out.push_str(&t.render());
    out
}

fn suggest(s: &Session, a: &SuggestArgs) -> CliResult {
    let mut hints = json!({});
    if let Some(c) = &a.caliber {
        hints["caliber"] = json!(c);
    }
    if let Some(d) = a.damage {
        hints["damage"] = json!(d);
    }
    if let Some(d) = a.speed {
        hints["speed"] = json!(d);
    }
    if let Some(d) = &a.similar_set {
        hints["similarSet"] = json!(d);
    }
    let mut request = json!({"class": a.class, "hints": hints});
    if let Some(c) = &a.copy_from {
        request["copyFrom"] = json!(c);
    }
    let reply = s.call("designer_ce_ammo_suggest", request)?;
    let v = reply.value;
    s.emit(&v, || render_suggestion(&v));
    if !bool_at(&v, "/available") {
        s.note_warning();
    }
    Ok(())
}

fn render_suggestion(v: &Value) -> String {
    if !bool_at(v, "/available") {
        return format!("Nothing to suggest: {}\n", str_at(v, "/reason"));
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "New ammo type of the class {}",
        str_at(v, "/classLabel")
    );
    if !str_at(v, "/copiedFrom").is_empty() {
        let _ = writeln!(out, "copied from {}", str_at(v, "/copiedFrom"));
    }
    let mut t = Table::new(&["nearest ammo", "set", "damage", "speed", "distance"]);
    for n in arr_at(v, "/nearest") {
        t.row(vec![
            str_at(n, "/ammoDef").to_owned(),
            str_at(n, "/set").to_owned(),
            opt_num(f64_at(n, "/damage")),
            opt_num(f64_at(n, "/speed")),
            num(f64_at(n, "/distance").unwrap_or(0.0)),
        ]);
    }
    if !t.is_empty() {
        out.push_str(&t.render());
    }
    let mut t = Table::new(&["field", "value", "source", "rating", "n"]);
    for f in arr_at(v, "/fields") {
        let value = f64_at(f, "/value").map_or_else(|| str_at(f, "/text").to_owned(), num);
        t.row(vec![
            str_at(f, "/field").to_owned(),
            value,
            str_at(f, "/source").to_owned(),
            str_at(f, "/rating").to_owned(),
            u64_at(f, "/n").to_string(),
        ]);
    }
    if !t.is_empty() {
        out.push_str(&t.render());
    }
    for note in arr_at(v, "/notes") {
        let _ = writeln!(out, "note: {}", note.as_str().unwrap_or_default());
    }
    out.push_str("Use --json to get the whole type for `--set ce.customAmmo.types.0=...`.\n");
    out
}
