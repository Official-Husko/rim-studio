//! The validation harness of the archetype solver, on a real install. Every test is `#[ignore]`, only reads and
//! needs `RIMSTUDIO_GAME_DIR` (the RimWorld install folder); the Combat Extended part also needs
//! `RIMSTUDIO_CE_DIR`.
//!
//! For every gun and melee weapon of the install the harness names its archetype and descriptors by the rules of
//! the taxonomy (from tags, verbs and tools; the calibre of a vanilla gun is unknown, so the damage class stands
//! in for it), builds the proposal for the typical balance target against the pool WITHOUT that weapon (leave one
//! out) and compares the proposal with the real numbers: the median and the 80th percentile of the relative error
//! per stat and per family, the share of proposals the fit meter calls typical or plausible, the strength
//! index, and the orderings of the families. Nothing from the install is copied into the repository; the test
//! prints a table and asserts the gates of the specification.
//!
//! Run with `cargo test -p rimstudio-design --release --test archetype_real -- --ignored --nocapture
//! --test-threads=1`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::too_many_lines,
    dead_code
)]

mod common_real;

use std::collections::BTreeMap;
use std::sync::Arc;

use common_real::{game_dir, official, prepare, type_table, version};
use rimstudio_defs::load;
use rimstudio_design::archetype::basis::{MeleeMedians, RangedMedians};
use rimstudio_design::archetype::{
    Basis, Context, CostBasis, ProposeRequest, Taxonomy, Verdict, derive_melee, derive_ranged,
    propose_with, verdict,
};
use rimstudio_design::baseline::{BaselineConfig, Model};
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_design::classes::numeric::{median, quantile};
use rimstudio_design::model::{ArchetypeMode, BalanceTarget, Descriptors};
use rimstudio_design::reader::{ReaderOptions, ReferencePools, WeaponDetail, build_pools};

/// The stats compared per kind.
const RANGED_STATS: [&str; 12] = [
    "damage", "range", "warmup", "cooldown", "burst", "mass", "work", "dps", "touch", "short",
    "medium", "long",
];
const MELEE_STATS: [&str; 6] = ["swing_damage", "cooldown", "dps", "ap", "mass", "work"];
/// The stats the gate of the specification is about.
const GATE_STATS: [&str; 4] = ["damage", "range", "cooldown", "mass"];

#[derive(Default)]
struct Row {
    errors: BTreeMap<String, Vec<f64>>,
    verdicts: Vec<Verdict>,
    real_verdicts: Vec<Verdict>,
    strength_errors: Vec<f64>,
    strength_unusual: usize,
    members: Vec<String>,
}

fn rel(p: f64, t: f64) -> Option<f64> {
    (t.is_finite() && t > 0.0 && p.is_finite()).then(|| (p - t).abs() / t)
}

fn run_kind(
    taxonomy: &'static Taxonomy,
    pools: &ReferencePools,
    costs: &CostBasis,
    kind: PoolKind,
    stats: &[&str],
    known: bool,
) -> BTreeMap<String, Row> {
    let pool = if kind == PoolKind::Melee {
        &pools.melee
    } else {
        &pools.ranged
    };
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for w in pools.set.weapons.iter().filter(|w| w.kind == kind) {
        let Some(index) = pool.index_of(&w.def_name) else {
            continue;
        };
        let loo = pool.without(&[index]);
        let model = Model::fit(loo.clone(), BaselineConfig::default());
        let others: Vec<WeaponDetail> = pools
            .set
            .weapons
            .iter()
            .filter(|o| o.def_name != w.def_name)
            .cloned()
            .collect();
        let derived = if kind == PoolKind::Melee {
            MeleeMedians::read(&loo)
                .ok()
                .and_then(|m| derive_melee(taxonomy, w, &m))
        } else {
            RangedMedians::read(&loo)
                .ok()
                .and_then(|m| derive_ranged(taxonomy, w, &m))
        };
        let Some(derived) = derived else {
            println!("  {:<34} no archetype by the rules", w.def_name);
            continue;
        };
        let ctx = Context {
            taxonomy,
            basis: Basis {
                model: &model,
                weapons: &others,
                armor: &pools.set.armor.ratings,
                costs: Some(costs),
            },
            ce: None,
        };
        let req = ProposeRequest {
            archetype: derived.archetype.clone(),
            descriptors: derived.descriptors.clone(),
            balance: BalanceTarget::Typical,
            mode: ArchetypeMode::Vanilla,
            strength: if known { w.strength } else { None },
        };
        let proposal = match propose_with(&ctx, &req, None) {
            Ok(p) => p,
            Err(e) => {
                println!("  {:<34} {} failed: {e}", w.def_name, derived.archetype);
                continue;
            }
        };
        let family = derived.archetype.split('/').next().unwrap_or("").to_owned();
        let row = rows.entry(family).or_default();
        row.members
            .push(format!("{}={}", w.def_name, derived.archetype));
        let mut line = format!("  {:<34} {:<22}", w.def_name, derived.archetype);
        for stat in stats {
            let (Some(p), Some(t)) = (proposal.stats.get(*stat), w.stats.get(*stat)) else {
                continue;
            };
            // A weapon the game prices explicitly has a work of 1: its work says nothing about the shape.
            if *stat == "work" && *t <= 10.0 {
                continue;
            }
            if let Some(e) = rel(*p, *t) {
                row.errors.entry((*stat).to_owned()).or_default().push(e);
                if GATE_STATS.contains(stat) {
                    line.push_str(&format!(" {stat} {p:.2}/{t:.2}"));
                }
            }
        }
        if let Some(e) = w
            .strength
            .and_then(|real| rel(proposal.strength.achieved, real))
        {
            row.strength_errors.push(e);
        }
        let v = proposal.fit.as_ref().map_or(Verdict::Unusual, verdict);
        if let Some(fit) = &proposal.fit {
            let off: Vec<String> = fit
                .per_stat
                .iter()
                .filter(|f| f.level != rimstudio_design::fit::FitLevel::Typical)
                .map(|f| {
                    format!(
                        "{}:{:?}({:.2} vs {:.2})",
                        f.stat, f.level, f.value, f.predicted
                    )
                })
                .collect();
            if !off.is_empty() && std::env::var_os("RIMSTUDIO_ARCHETYPE_FITS").is_some() {
                println!("      fit off: {}", off.join(" "));
            }
        }
        row.verdicts.push(v);
        // The same meter on the real weapon, at its own strength: the ceiling a proposal can reach.
        let real_key = rimstudio_design::classes::ClassKey {
            role: proposal.role.clone(),
            tier: w.tech_level.map(|t| t.index()),
            group: None,
        };
        if let Some(real) = w.strength {
            let placed = rimstudio_design::baseline::StrengthInput::Placed {
                ln_power: real.ln(),
            };
            if let Ok(r) =
                rimstudio_design::archetype::fit_of(&model, &real_key, placed, &w.stats, None)
            {
                row.real_verdicts.push(verdict(&r));
            }
        }
        // The strength index: unusual when the rounded proposal misses the strength it aimed at by more than
        // 15 percent, which would mean the solver could not land the target.
        if proposal.strength.error.abs() > 0.15 {
            row.strength_unusual += 1;
        }
        println!(
            "{line} {:?} s={:.2} target {:.2} real {:.2} [{}]",
            v,
            proposal.strength.scale,
            proposal.strength.target,
            w.strength.unwrap_or(0.0),
            proposal.strength.class_label
        );
    }
    rows
}

fn report(
    title: &str,
    rows: &BTreeMap<String, Row>,
    stats: &[&str],
) -> (usize, usize, usize, Vec<String>) {
    println!("\n{title}");
    println!(
        "{:<14} {:>3}  {}",
        "family",
        "n",
        stats
            .iter()
            .map(|s| format!("{s:>14}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut all_verdicts = 0usize;
    let mut good = 0usize;
    let mut unusual_strength = 0usize;
    let (mut real_total, mut real_good) = (0usize, 0usize);
    let mut failures = Vec::new();
    let mut pooled: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (family, row) in rows {
        let mut cells = Vec::new();
        for stat in stats {
            let e = row.errors.get(*stat).cloned().unwrap_or_default();
            let med = median(&e);
            let p80 = quantile(&e, 0.8);
            cells.push(match (med, p80) {
                (Some(m), Some(p)) => format!("{:>6.0}% /{:>4.0}%", m * 100.0, p * 100.0),
                _ => format!("{:>14}", "-"),
            });
            pooled
                .entry((*stat).to_owned())
                .or_default()
                .extend(e.iter().copied());
            if row.members.len() >= 3
                && GATE_STATS.contains(stat)
                && let Some(m) = med.filter(|m| *m >= 0.25)
            {
                failures.push(format!("{family} {stat} median {:.0}%", m * 100.0));
            }
        }
        println!("{family:<14} {:>3}  {}", row.members.len(), cells.join(" "));
        all_verdicts += row.verdicts.len();
        real_total += row.real_verdicts.len();
        real_good += row
            .real_verdicts
            .iter()
            .filter(|v| **v != Verdict::Unusual)
            .count();
        good += row
            .verdicts
            .iter()
            .filter(|v| **v != Verdict::Unusual)
            .count();
        unusual_strength += row.strength_unusual;
    }
    let cells: Vec<String> = stats
        .iter()
        .map(|s| {
            let e = pooled.get(*s).cloned().unwrap_or_default();
            match (median(&e), quantile(&e, 0.8)) {
                (Some(m), Some(p)) => format!("{:>6.0}% /{:>4.0}%", m * 100.0, p * 100.0),
                _ => format!("{:>14}", "-"),
            }
        })
        .collect();
    println!("{:<14} {:>3}  {}", "ALL", all_verdicts, cells.join(" "));
    println!(
        "the meter on the real weapons themselves: typical or plausible {real_good}/{real_total}"
    );
    println!(
        "typical or plausible: {good}/{all_verdicts}; strength missed by more than 15%: {unusual_strength}"
    );
    let strength: Vec<f64> = rows
        .values()
        .flat_map(|r| r.strength_errors.iter().copied())
        .collect();
    println!(
        "strength index of the proposal against the real weapon: median error {:.0}%",
        median(&strength).unwrap_or(0.0) * 100.0
    );
    (all_verdicts, good, unusual_strength, failures)
}

fn orderings(
    taxonomy: &'static Taxonomy,
    pools: &ReferencePools,
    costs: &CostBasis,
) -> Vec<String> {
    let model = Model::fit(pools.ranged.clone(), BaselineConfig::default());
    let ctx = Context {
        taxonomy,
        basis: Basis {
            model: &model,
            weapons: &pools.set.weapons,
            armor: &pools.set.armor.ratings,
            costs: Some(costs),
        },
        ce: None,
    };
    let make = |id: &str| {
        propose_with(
            &ctx,
            &ProposeRequest {
                archetype: id.to_owned(),
                descriptors: Descriptors::default(),
                balance: BalanceTarget::Typical,
                mode: ArchetypeMode::Vanilla,
                strength: None,
            },
            None,
        )
        .unwrap()
    };
    println!("\ncanonical proposals (typical, defaults)");
    let mut by: BTreeMap<&str, BTreeMap<String, f64>> = BTreeMap::new();
    for id in [
        "pistol/revolver",
        "pistol/light",
        "smg/light",
        "smg/heavy",
        "shotgun/pump",
        "rifle/carbine",
        "rifle/assault",
        "rifle/bolt",
        "rifle/dmr",
        "rifle/sniper",
        "rifle/anti-materiel",
        "machine-gun/lmg",
        "machine-gun/heavy",
        "bow/short",
        "bow/great",
    ] {
        let p = make(id);
        let s = &p.stats;
        println!(
            "  {id:<20} dmg {:>5.1} ap {:>5.3} range {:>5.1} warm {:>4.2} cd {:>4.2} burst {:>2} mass {:>4.1} work {:>6.0} price {:>6.0} role {:?} strength {:.2} ({:+.1}%) fit {:?}",
            s["damage"],
            s["ap"],
            s["range"],
            s["warmup"],
            s["cooldown"],
            s["burst"] as u32,
            s["mass"],
            s["work"],
            s.get("market_value").copied().unwrap_or(0.0),
            p.role,
            p.strength.achieved,
            p.strength.error * 100.0,
            p.fit.as_ref().map(verdict)
        );
        by.insert(id, p.stats.clone());
    }
    let get = |id: &str, stat: &str| by[id][stat];
    let mut problems = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            problems.push(what.to_owned());
        }
    };
    check(
        get("rifle/sniper", "range") > get("rifle/assault", "range"),
        "sniper range above assault rifle range",
    );
    check(
        get("rifle/assault", "range") > get("smg/light", "range"),
        "assault rifle range above SMG range",
    );
    check(
        get("rifle/bolt", "range") > get("rifle/assault", "range"),
        "bolt action range above assault rifle range",
    );
    check(
        get("rifle/sniper", "warmup") > get("rifle/assault", "warmup"),
        "sniper warmup above assault rifle warmup",
    );
    check(
        get("rifle/assault", "warmup") > get("smg/light", "warmup"),
        "assault rifle warmup above SMG warmup",
    );
    check(
        get("smg/light", "cooldown") < get("rifle/assault", "cooldown"),
        "SMG cooldown below assault rifle cooldown",
    );
    check(
        get("rifle/sniper", "damage") > get("rifle/assault", "damage"),
        "sniper damage above assault rifle damage",
    );
    check(
        get("rifle/assault", "damage") > get("smg/light", "damage"),
        "assault rifle damage above SMG damage",
    );
    check(
        get("rifle/anti-materiel", "ap") > get("rifle/sniper", "ap"),
        "anti materiel penetration above sniper penetration",
    );
    check(
        get("machine-gun/lmg", "burst") > get("rifle/assault", "burst"),
        "LMG burst above assault rifle burst",
    );
    problems
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn proposals_against_the_vanilla_weapons() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let input = prepare(&official(&dir), &version(&dir), Arc::new(types));
    let out = load(input);
    let opts = ReaderOptions::default();
    let pools = build_pools(&out.databases, &opts).unwrap();
    let costs = CostBasis::read(&out.databases, &opts, &pools.set);
    let taxonomy = Taxonomy::builtin().unwrap();
    println!(
        "ranged pool {} items, melee pool {} items, cost rows {}",
        pools.ranged.len(),
        pools.melee.len(),
        costs.rows.len()
    );
    let known = std::env::var_os("RIMSTUDIO_ARCHETYPE_KNOWN").is_some();
    println!(
        "\nranged weapons (proposal/real){}",
        if known {
            ", aimed at the real strength"
        } else {
            ""
        }
    );
    let ranged = run_kind(
        taxonomy,
        &pools,
        &costs,
        PoolKind::Ranged,
        &RANGED_STATS,
        known,
    );
    let (rn, rgood, runusual, mut failures) = report(
        "guns and bows: median / P80 relative error per stat",
        &ranged,
        &RANGED_STATS,
    );
    println!("\nmelee weapons (proposal/real)");
    let melee = run_kind(
        taxonomy,
        &pools,
        &costs,
        PoolKind::Melee,
        &MELEE_STATS,
        known,
    );
    let (mn, mgood, munusual, mfailures) = report(
        "melee weapons: median / P80 relative error per stat",
        &melee,
        &MELEE_STATS,
    );
    failures.extend(mfailures);
    let problems = orderings(taxonomy, &pools, &costs);
    println!("\nordering problems: {problems:?}");
    println!("gate failures: {failures:?}");
    let total = rn + mn;
    let good = rgood + mgood;
    println!(
        "typical or plausible overall: {good}/{total} ({:.0}%); strength unusual: {}",
        100.0 * good as f64 / total.max(1) as f64,
        runusual + munusual
    );
    if std::env::var_os("RIMSTUDIO_ARCHETYPE_ASSERT").is_some() {
        assert!(problems.is_empty(), "{problems:?}");
        assert!(failures.is_empty(), "{failures:?}");
        assert!(good * 10 >= total * 9, "{good}/{total}");
        assert_eq!(runusual + munusual, 0);
    }
}

/// The Combat Extended part: for every converted gun that has a vanilla twin the harness derives the archetype
/// from the twin, proposes in Combat Extended mode with the gun's own ammo set as the calibre, applies the
/// proposal to a new design and asks the existing predictors for the block, then compares the block and the
/// patch numbers with the real conversion, and the proposed AI class tag with the real one.
#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn combat_extended_proposals_against_the_converted_guns() {
    use rimstudio_defs::TypeTable;
    use rimstudio_design::archetype::{CeInfo, ce_info};
    use rimstudio_design::ce::reader::{
        CeClassNames, CeReadOptions, custom_registry, read_conversions_with, with_ce_types,
    };
    use rimstudio_design::ce::suggest::{Accept, accept_suggestions, suggest_block};
    use rimstudio_design::model::DesignSpec;

    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(std::path::PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let types: TypeTable = types;
    let vanilla = load(prepare(
        &official(&dir),
        &version(&dir),
        Arc::new(types.clone()),
    ));
    let classes = CeClassNames::default();
    let ce_types = with_ce_types(&types, &classes).unwrap();
    let mut roots = official(&dir);
    roots.push(ce_dir);
    let mut input = prepare(&roots, &version(&dir), Arc::new(ce_types));
    input.custom_ops = custom_registry(&classes, std::collections::BTreeMap::new());
    let out = load(input);
    let model = read_conversions_with(
        &out.databases,
        &CeReadOptions {
            order: Some(&out.order),
            vanilla: Some(&vanilla.databases),
            ..CeReadOptions::default()
        },
    );
    let info: CeInfo = ce_info(&model);
    let opts = ReaderOptions::default();
    let pools = build_pools(&vanilla.databases, &opts).unwrap();
    let costs = CostBasis::read(&vanilla.databases, &opts, &pools.set);
    let taxonomy = Taxonomy::builtin().unwrap();
    let medians = RangedMedians::read(&pools.ranged).unwrap();
    let base_model = Model::fit(pools.ranged.clone(), BaselineConfig::default());
    let ctx = Context {
        taxonomy,
        basis: Basis {
            model: &base_model,
            weapons: &pools.set.weapons,
            armor: &pools.set.armor.ratings,
            costs: Some(&costs),
        },
        ce: Some(&info),
    };
    println!(
        "ammo sets as calibres {}, converted guns {} ({} with twins)",
        info.calibres.len(),
        model.guns.len(),
        model.twin_count()
    );
    let pointers = [
        ("/ce/bulk", "bulk"),
        ("/ce/shotSpread", "spread"),
        ("/ce/swayFactor", "sway"),
        ("/ce/magazineSize", "magazine"),
        ("/ce/reloadTime", "reload"),
        ("mass", "mass"),
        ("range", "range"),
        ("warmup", "warmup"),
        ("cooldown", "cooldown"),
    ];
    let mut errors: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let (mut tags, mut tag_agree, mut guns) = (0usize, 0usize, 0usize);
    for gun in model
        .guns
        .iter()
        .filter(|g| g.twin.is_some() && !g.bow && g.excluded.is_none())
    {
        let Some(w) = pools.set.weapon(&gun.def_name) else {
            continue;
        };
        let Some(derived) = derive_ranged(taxonomy, w, &medians) else {
            continue;
        };
        let Some(set) = gun
            .ammo_set
            .as_deref()
            .filter(|s| info.calibre(s).is_some())
        else {
            continue;
        };
        let mut descriptors = derived.descriptors.clone();
        descriptors.ammo_set = Some(set.to_owned());
        let req = ProposeRequest {
            archetype: derived.archetype.clone(),
            descriptors,
            balance: BalanceTarget::Typical,
            mode: ArchetypeMode::CombatExtended,
            strength: None,
        };
        let Ok(proposal) = propose_with(&ctx, &req, None) else {
            continue;
        };
        let mut spec = DesignSpec::new_ranged("RS_Probe", "probe");
        rimstudio_design::archetype::apply(
            &mut spec,
            &proposal,
            rimstudio_design::archetype::ApplyOptions::default(),
        )
        .unwrap();
        let ce = proposal.ce.clone().unwrap();
        let block = spec.ce.get_or_insert_with(Default::default);
        block.ammo_set.clone_from(&ce.ammo_set);
        block.default_projectile.clone_from(&ce.default_projectile);
        let suggestion = suggest_block(&spec, &model);
        let outcome = accept_suggestions(&spec, &suggestion, &Accept::All);
        let after = suggest_block(&outcome.spec, &model);
        guns += 1;
        for (pointer, stat) in pointers {
            let found = after
                .fields
                .iter()
                .chain(after.patch_numbers.iter())
                .find(|f| {
                    f.field == pointer
                        || f.field.ends_with(&format!("/{stat}")) && pointer.starts_with('/')
                });
            let value = found.and_then(|f| f.value.or(f.held));
            if let (Some(p), Some(t)) = (value, gun.stats.get(stat))
                && let Some(e) = rel(p, *t)
            {
                errors.entry(stat).or_default().push(e);
            }
        }
        if let Some(real) = &gun.ai_class {
            tags += 1;
            if ce.weapon_tag_class.as_deref() == Some(real.as_str()) {
                tag_agree += 1;
            } else {
                println!(
                    "  tag {:<26} {:<22} proposed {:?} real {real}",
                    gun.def_name, derived.archetype, ce.weapon_tag_class
                );
            }
        }
    }
    println!(
        "converted guns compared {guns}; AI class tags of the install {:?}",
        info.ai_class_tags
    );
    for (stat, e) in &errors {
        println!(
            "  {stat:<10} n {:>3}  median {:>4.0}%  P80 {:>4.0}%",
            e.len(),
            median(e).unwrap_or(0.0) * 100.0,
            quantile(e, 0.8).unwrap_or(0.0) * 100.0
        );
    }
    println!("AI class tag agrees with the real one: {tag_agree}/{tags}");
    assert!(guns > 0);
}
