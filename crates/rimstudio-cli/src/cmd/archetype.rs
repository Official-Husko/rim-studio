//! `designer archetypes`, `designer propose` and `designer new --archetype`: describe a weapon by its family,
//! action, rate of fire and calibre and get every number of it, each with a plain reason.
//!
//! The numbers come from the weapons of your own install (the pools by tier and role), so nothing is
//! stored. `designer archetypes` lists what is on offer, `designer propose` shows the numbers and changes
//! nothing, and `designer new KIND --archetype FAMILY/TYPE ...` creates a draft filled with them. Typed
//! values are never overwritten. Combat Extended is optional: `--ce` also proposes the calibre as one of
//! your ammo sets and fills the optional Combat Extended block; without it the draft stays vanilla.

use std::fmt::Write as _;

use clap::Args;
use serde_json::{Value, json};

use super::{drafts, project};
use crate::cli::KindArg;
use crate::draftops::apply_sets;
use crate::error::{CliError, CliResult};
use crate::fmt::{Table, arr_at, bool_at, f64_at, num, opt_num, str_at, u64_at};
use crate::session::Session;

/// The descriptors of a weapon, shared by `designer propose` and `designer new --archetype`.
#[derive(Debug, Default, Args)]
pub(crate) struct ArchetypeOpts {
    /// The action: bolt, lever, pump, semi, burst, full-auto or draw (what the archetype allows).
    #[arg(long, value_name = "ACTION")]
    pub(crate) action: Option<String>,
    /// The rate of fire: slow, medium, fast, or a number of rounds per minute.
    #[arg(long, value_name = "CLASS|RPM")]
    pub(crate) rof: Option<String>,
    /// The calibre class (tiny, small, medium, large, huge); with `--ce` the def name of one of your
    /// Combat Extended ammo sets (`designer ammo catalog` lists them).
    #[arg(long, value_name = "CLASS|AMMOSET")]
    pub(crate) caliber: Option<String>,
    /// The handling: compact, standard or heavy.
    #[arg(long, value_name = "HANDLING")]
    pub(crate) handling: Option<String>,
    /// The tech level: neolithic, medieval, industrial, spacer, ultra or archotech (default from the
    /// archetype).
    #[arg(long, value_name = "TIER")]
    pub(crate) tier: Option<String>,
    /// Where the strength should land among your weapons of its class: weaker, typical, stronger, or a
    /// percentile from 0 to 1.
    #[arg(long, value_name = "TARGET", default_value = "typical")]
    pub(crate) balance: String,
    /// Aim at exactly this strength index instead (for example the strength `designer refs` shows for a
    /// reference weapon).
    #[arg(long, value_name = "N")]
    pub(crate) match_strength: Option<f64>,
    /// Also propose the calibre as a Combat Extended ammo set and fill the optional Combat Extended block.
    #[arg(long)]
    pub(crate) ce: bool,
}

/// `designer archetypes` arguments.
#[derive(Debug, Args)]
pub(crate) struct ArchetypesArgs {
    /// Only guns or only melee weapons.
    #[arg(value_enum)]
    pub(crate) kind: Option<KindArg>,
    /// Also list the calibres of your Combat Extended ammo sets.
    #[arg(long)]
    pub(crate) calibres: bool,
}

/// `designer propose` arguments.
#[derive(Debug, Args)]
pub(crate) struct ProposeArgs {
    /// The archetype, `family/type`, for example `rifle/assault` (`designer archetypes` lists them).
    pub(crate) archetype: String,
    /// The descriptors.
    #[command(flatten)]
    pub(crate) opts: ArchetypeOpts,
    /// Show the proposal as it would fit a draft: values the draft decided are marked locked.
    #[arg(long, value_name = "DRAFT")]
    pub(crate) draft: Option<String>,
    /// The mod project folder of the draft.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: Option<String>,
}

fn descriptors(o: &ArchetypeOpts) -> Value {
    let mut d = json!({});
    if let Some(a) = &o.action {
        d["action"] = json!(a);
    }
    if let Some(r) = &o.rof {
        d["rof"] = match r.parse::<f64>() {
            Ok(rpm) => json!({"rpm": rpm}),
            Err(_) => json!({"class": r.to_lowercase()}),
        };
    }
    if let Some(c) = &o.caliber {
        if o.ce {
            d["ammoSet"] = json!(c);
        } else {
            d["calibre"] = json!(c.to_lowercase());
        }
    }
    if let Some(h) = &o.handling {
        d["handling"] = json!(h.to_lowercase());
    }
    if let Some(t) = &o.tier {
        d["tier"] = json!(t.to_lowercase());
    }
    d
}

fn balance(o: &ArchetypeOpts) -> Result<Value, CliError> {
    match o.balance.to_lowercase().as_str() {
        b @ ("weaker" | "typical" | "stronger") => Ok(json!(b)),
        other => match other.parse::<f64>() {
            Ok(p) if (0.0..=1.0).contains(&p) => Ok(json!({"percentile": p})),
            _ => Err(CliError::usage(
                "--balance is weaker, typical, stronger or a percentile from 0 to 1",
            )),
        },
    }
}

fn request(
    kind: &str,
    archetype: &str,
    o: &ArchetypeOpts,
    draft: Option<Value>,
) -> Result<Value, CliError> {
    let mut req = json!({
        "kind": kind,
        "archetype": archetype,
        "descriptors": descriptors(o),
        "balanceTarget": balance(o)?,
        "mode": if o.ce { "combat-extended" } else { "vanilla" },
    });
    if let Some(n) = o.match_strength {
        req["strength"] = json!(n);
    }
    if let Some(d) = draft {
        req["draft"] = d;
    }
    Ok(req)
}

/// The kind word of an archetype id, from the catalogue.
fn kind_of(s: &Session, archetype: &str) -> CliResult<String> {
    let reply = s.call("designer_archetype_catalog", json!({}))?;
    for f in arr_at(&reply.value, "/families") {
        for a in arr_at(f, "/archetypes") {
            if str_at(a, "/id") == archetype {
                return Ok(str_at(a, "/kind").to_owned());
            }
        }
    }
    Err(CliError::usage(format!(
        "`{archetype}` is not an archetype: `designer archetypes` lists them as family/type, for example rifle/assault"
    )))
}

/// `designer archetypes`: the families and archetypes with the descriptors each takes.
///
/// # Errors
/// The envelope of a failing command.
pub(crate) fn list(s: &Session, a: &ArchetypesArgs) -> CliResult {
    let mut req = json!({"includeCalibres": a.calibres});
    if let Some(k) = a.kind {
        req["kind"] = json!(match k {
            KindArg::Ranged => "ranged",
            KindArg::Melee => "melee",
        });
    }
    let reply = s.call("designer_archetype_catalog", req)?;
    let v = reply.value;
    s.emit(&v, || render_catalog(&v));
    if !bool_at(&v, "/proposalsAvailable") {
        s.note_warning();
    }
    Ok(())
}

fn render_catalog(v: &Value) -> String {
    let mut out = String::new();
    if !bool_at(v, "/proposalsAvailable") {
        out.push_str("No game install is loaded: the archetypes are listed, but no numbers can be proposed.\n");
    } else {
        let _ = writeln!(
            out,
            "reference weapons in your install: {} guns and bows, {} melee weapons",
            u64_at(v, "/poolSizes/0"),
            u64_at(v, "/poolSizes/1")
        );
    }
    let mut t = Table::new(&[
        "archetype",
        "name",
        "actions",
        "rate of fire",
        "calibres",
        "pool role",
    ]);
    for f in arr_at(v, "/families") {
        for a in arr_at(f, "/archetypes") {
            let list = |p: &str| {
                arr_at(a, p)
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            t.row(vec![
                str_at(a, "/id").to_owned(),
                str_at(a, "/label").to_owned(),
                list("/actions"),
                list("/rofClasses"),
                list("/calibres"),
                str_at(a, "/poolRole").to_owned(),
            ]);
        }
    }
    out.push_str(&t.render());
    out.push_str("descriptors: --action, --rof (slow, medium, fast or rounds per minute), --caliber, --handling (compact, standard, heavy), --tier, --balance (weaker, typical, stronger)\n");
    let tiers: Vec<String> = arr_at(v, "/tiers")
        .iter()
        .map(|t| {
            format!(
                "{} ({} guns, {} melee)",
                str_at(t, "/label"),
                u64_at(t, "/rangedCount"),
                u64_at(t, "/meleeCount")
            )
        })
        .collect();
    let _ = writeln!(out, "tiers: {}", tiers.join(", "));
    let ce = v.get("ce").cloned().unwrap_or(Value::Null);
    if bool_at(&ce, "/available") {
        let calibres = arr_at(&ce, "/calibres");
        if calibres.is_empty() {
            out.push_str("Combat Extended is loaded: add --calibres to list its ammo sets.\n");
        } else {
            let mut t = Table::new(&[
                "ammo set",
                "caliber",
                "family",
                "damage",
                "vs family median",
            ]);
            for c in calibres {
                t.row(vec![
                    str_at(c, "/set").to_owned(),
                    str_at(c, "/caliber").to_owned(),
                    str_at(c, "/family").to_owned(),
                    opt_num(f64_at(c, "/damage")),
                    opt_num(f64_at(c, "/ratio")),
                ]);
            }
            out.push_str("Combat Extended calibres:\n");
            out.push_str(&t.render());
        }
    } else if !str_at(&ce, "/reason").is_empty() {
        let _ = writeln!(out, "Combat Extended: {}", str_at(&ce, "/reason"));
    }
    out
}

fn render_proposal(p: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Proposal for {} ({}), source {}",
        str_at(p, "/label"),
        str_at(p, "/choice/archetype"),
        str_at(p, "/source")
    );
    let _ = writeln!(
        out,
        "  action {}, rate of fire {} (x{}), calibre {}, handling {}, tier {}, pool role {}",
        or_dash(str_at(p, "/resolved/action")),
        str_at(p, "/resolved/rof"),
        opt_num(f64_at(p, "/resolved/rate")),
        or_dash(str_at(p, "/resolved/calibre")),
        str_at(p, "/resolved/handling"),
        str_at(p, "/resolved/tier"),
        or_dash(str_at(p, "/role"))
    );
    let _ = writeln!(
        out,
        "  strength target {} at percentile {} ({}), achieved {} ({:+.1}%), fit meter: {}",
        opt_num(f64_at(p, "/strength/target")),
        opt_num(f64_at(p, "/strength/percentile")),
        str_at(p, "/strength/classLabel"),
        opt_num(f64_at(p, "/strength/achieved")),
        f64_at(p, "/strength/error").unwrap_or(0.0) * 100.0,
        or_dash(str_at(p, "/verdict"))
    );
    let mut t = Table::new(&["field", "value", "locked", "reason"]);
    let mut push = |v: &Value| {
        let note = if bool_at(v, "/write") {
            ""
        } else {
            " (not written)"
        };
        t.row(vec![
            format!("{}{note}", str_at(v, "/field")),
            opt_num(f64_at(v, "/value")),
            if bool_at(v, "/locked") { "yes" } else { "" }.to_owned(),
            str_at(v, "/reason").to_owned(),
        ]);
    };
    for v in arr_at(p, "/values") {
        push(v);
    }
    for tool in arr_at(p, "/tools") {
        push(&tool["power"]);
        push(&tool["cooldown"]);
        if tool.get("armorPenetration").is_some_and(Value::is_object) {
            push(&tool["armorPenetration"]);
        }
    }
    out.push_str(&t.render());
    let cost: Vec<String> = arr_at(p, "/costList")
        .iter()
        .map(|c| {
            format!(
                "{} x{}",
                str_at(c, "/defName"),
                num(f64_at(c, "/count").unwrap_or(0.0))
            )
        })
        .collect();
    if !cost.is_empty() {
        let _ = writeln!(out, "cost list: {}", cost.join(", "));
    }
    if let Some(stuff) = p.get("stuff").filter(|s| s.is_object()) {
        let cats: Vec<&str> = arr_at(stuff, "/categories")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let _ = writeln!(
            out,
            "stuff: {} x{}",
            cats.join("/"),
            opt_num(f64_at(stuff, "/count"))
        );
    }
    let tags: Vec<&str> = arr_at(p, "/weaponTags")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if !tags.is_empty() {
        let _ = writeln!(out, "weapon tags: {}", tags.join(", "));
    }
    if let Some(mv) = f64_at(p, "/marketValue") {
        let _ = writeln!(out, "market value from the price math: {}", num(mv));
    }
    if let Some(ce) = p.get("ce").filter(|c| c.is_object()) {
        let _ = writeln!(
            out,
            "Combat Extended: ammo set {}, caliber {}, projectile {}, AI class {}",
            or_dash(str_at(ce, "/ammoSet")),
            or_dash(str_at(ce, "/caliber")),
            or_dash(str_at(ce, "/defaultProjectile")),
            or_dash(str_at(ce, "/weaponTagClass"))
        );
    }
    for n in arr_at(p, "/notes") {
        if let Some(n) = n.as_str() {
            let _ = writeln!(out, "note: {n}");
        }
    }
    out
}

fn or_dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}

/// `designer propose ARCHETYPE`: every number of the weapon, each with a reason. Changes nothing.
///
/// # Errors
/// The envelope of a failing command.
pub(crate) fn propose(s: &Session, a: &ProposeArgs) -> CliResult {
    let kind = kind_of(s, &a.archetype)?;
    let draft = match (&a.draft, &a.project) {
        (Some(reference), Some(dir)) => {
            let summary = project::open(s, dir)?;
            Some(drafts::load(s, reference, Some(&summary))?.draft)
        }
        (Some(_), None) => return Err(CliError::usage("--draft needs --project DIR")),
        _ => None,
    };
    let req = request(&kind, &a.archetype, &a.opts, draft)?;
    let reply = s.call("designer_archetype_propose", req)?;
    let v = reply.value;
    s.emit(&v, || render_proposal(&v));
    Ok(())
}

/// The part of `designer new` that starts from an archetype: proposes, applies to the new draft and returns
/// the new draft with the rendered proposal.
///
/// # Errors
/// A usage error for a bad `--set` or descriptor, the envelope of a failing command.
pub(crate) fn fill_new_draft(
    s: &Session,
    kind: KindArg,
    archetype: &str,
    o: &ArchetypeOpts,
    sets: &[String],
    mut draft: Value,
) -> CliResult<(Value, Value, usize)> {
    apply_sets(&mut draft, sets).map_err(CliError::usage)?;
    let kind_word = match kind {
        KindArg::Ranged => "ranged",
        KindArg::Melee => "melee",
    };
    let req = request(kind_word, archetype, o, Some(draft.clone()))?;
    let proposal = s.call("designer_archetype_propose", req)?.value;
    let reply = s.call(
        "designer_archetype_apply",
        json!({"draft": draft, "proposal": proposal, "includeCe": o.ce}),
    )?;
    let filled = arr_at(&reply.value, "/filled").len();
    let notes: Vec<String> = arr_at(&reply.value, "/notes")
        .iter()
        .filter_map(|n| n.as_str().map(str::to_owned))
        .collect();
    let next = reply.value.get("draft").cloned().unwrap_or(draft);
    let mut applied = reply.value.get("proposal").cloned().unwrap_or(proposal);
    if let Some(list) = applied.get_mut("notes").and_then(Value::as_array_mut) {
        list.extend(notes.into_iter().map(Value::from));
    }
    Ok((next, applied, filled))
}

/// Renders a proposal for the output of `designer new`.
#[must_use]
pub(crate) fn render(p: &Value) -> String {
    render_proposal(p)
}
