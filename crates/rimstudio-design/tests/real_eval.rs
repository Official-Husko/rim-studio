//! Measures how well the conversion predictors reproduce Combat Extended conversions, on a real install.
//! Every test is `#[ignore]`, only reads, and needs environment variables:
//!
//! - `RIMSTUDIO_GAME_DIR`: the RimWorld install folder (holds `Version.txt` and `Data/`);
//! - `RIMSTUDIO_CE_DIR`: a Combat Extended mod folder;
//! - `RIMSTUDIO_CUSTOM_DIR` (optional): a folder of mods whose hand written Combat Extended patches form
//!   the held out set.
//!
//! Run with `cargo test -p rimstudio-design --release --test real_eval -- --ignored --nocapture
//! --test-threads=1`.
//!
//! Method. For every converted gun and melee weapon of the Combat Extended mod that has a vanilla twin on
//! the install, the converted numbers are predicted from the twin alone: the vanilla numbers and the
//! vanilla weapon tags, never the target's own conversion. The target is left out of every class pool,
//! ratio, median and reliability measure (leave one out). Per stat the tables report the number of
//! targets, the median and the 80th percentile of the absolute relative error, the share of targets inside
//! the claimed P50 and P80 bands, and the reliability rating the stat received. A second, held out set
//! (hand written conversions of the mods in `RIMSTUDIO_CUSTOM_DIR`, read through the engine) is predicted
//! from the full Combat Extended pool and never used to tune anything.
//!
//! Only aggregates are printed (set `RIMSTUDIO_EVAL_DETAIL=1` to also list the held out targets one by one
//! for local reading). Nothing is asserted about numbers and nothing is written; the output is not to be
//! committed.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::{FileId, ModIdx, SourceId};
use rimstudio_core::load_plan::{Listing, Subdir, collect_files, resolve_load_folders_os};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::os::Os;
use rimstudio_core::version::GameVersion;
use rimstudio_defs::{
    DefFile, FileContent, LoadInput, LoadOutput, ModEntry, PatchFile, TypeTable, load,
};
use rimstudio_design::baseline::Band;
use rimstudio_design::ce::classes::{
    CeClassOptions, EstimateOptions, ExampleSet, Profile, build_pool, class_key,
    estimate_conversion, predict_conversion,
};
use rimstudio_design::ce::reader::{
    CeClassNames, CeGun, CeMelee, CeModel, CeReadOptions, custom_registry, read_conversions_with,
    with_ce_types,
};
use rimstudio_design::classes::{ItemKind, Pool};
use rimstudio_design::model::TechLevel;
use rimstudio_testing::loaders::{
    DirParse, parse_defs_dir_report, parse_patch_dir_report, read_about, read_load_folders,
};

const OFFICIAL_PACKS: [&str; 6] = [
    "Core", "Royalty", "Ideology", "Biotech", "Anomaly", "Odyssey",
];

struct DiskListing;

fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let r = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&entry.path(), &r, out),
            Ok(_) => out.push(r),
            Err(_) => {}
        }
    }
}

impl Listing for DiskListing {
    fn list_files(&self, dir: &Utf8Path) -> Vec<String> {
        let mut out = Vec::new();
        walk(dir.as_std_path(), "", &mut out);
        out
    }

    fn dir_exists(&self, dir: &Utf8Path) -> bool {
        dir.as_std_path().is_dir()
    }
}

fn child_dirs(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn content_of(parse: &Option<DirParse>, rel: &str) -> FileContent {
    let Some(p) = parse else {
        return FileContent::failed("folder missing");
    };
    if let Some(f) = p.files.iter().find(|f| f.rel_path.as_str() == rel) {
        return FileContent::parsed(f.root.clone());
    }
    if let Some(f) = p.failures.iter().find(|f| f.rel_path.as_str() == rel) {
        return FileContent::failed(f.error.to_string());
    }
    FileContent::failed("file not parsed")
}

fn prepare(roots: &[PathBuf], version: &str, types: Arc<TypeTable>) -> LoadInput {
    let game = GameVersion::parse(version).unwrap();
    let mut metas = Vec::new();
    for root in roots {
        let about = read_about(root).unwrap();
        let utf = Utf8PathBuf::from_path_buf(root.clone()).unwrap();
        let mut meta = about
            .about
            .into_meta(SourceId::new("rs").unwrap(), utf)
            .unwrap();
        if let Some(lf) = read_load_folders(root).unwrap() {
            meta.load_folders = Some(lf.spec);
        }
        meta.root_dirs = child_dirs(root);
        metas.push(meta);
    }
    let active = ActiveSet::from_ids(metas.iter().map(|m| m.package_id.as_str().to_owned()));
    let mut mods = Vec::new();
    let mut def_files = Vec::new();
    let mut patch_files = Vec::new();
    let mut next_file = 0u32;
    let mut cache: BTreeMap<(Utf8PathBuf, bool), Option<DirParse>> = BTreeMap::new();
    for (i, meta) in metas.iter().enumerate() {
        let idx = ModIdx(u32::try_from(i).unwrap());
        mods.push(ModEntry::new(
            idx,
            meta.package_id.as_str(),
            meta.name.clone(),
        ));
        let plan = resolve_load_folders_os(meta, &game, &active, Os::Linux);
        for (defs, subdir) in [(true, Subdir::Defs), (false, Subdir::Patches)] {
            for f in collect_files(&plan, subdir, &DiskListing) {
                let folder = &plan.folders[f.folder].path;
                let parse = cache
                    .entry((folder.clone(), defs))
                    .or_insert_with(|| {
                        if defs {
                            parse_defs_dir_report(folder.join("Defs").as_std_path()).ok()
                        } else {
                            parse_patch_dir_report(folder.join("Patches").as_std_path()).ok()
                        }
                    })
                    .clone();
                let rel_in = f.relative.split_once('/').map_or("", |(_, r)| r);
                let content = content_of(&parse, rel_in);
                let file = FileId(next_file);
                next_file += 1;
                if defs {
                    def_files.push(DefFile {
                        mod_idx: idx,
                        file,
                        rel_path: f.relative.clone(),
                        content,
                    });
                } else {
                    patch_files.push(PatchFile {
                        mod_idx: idx,
                        file,
                        rel_path: f.relative.clone(),
                        content,
                    });
                }
            }
        }
    }
    let mut input = LoadInput::new(mods, types);
    input.def_files = def_files;
    input.patch_files = patch_files;
    input
}

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("RIMSTUDIO_GAME_DIR")?);
    dir.join("Version.txt").is_file().then_some(dir)
}

fn type_table() -> Option<TypeTable> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data/def-engine/data/def_types_vanilla.json");
    TypeTable::from_json_str(&fs::read_to_string(path).ok()?).ok()
}

fn version(dir: &Path) -> String {
    fs::read_to_string(dir.join("Version.txt"))
        .unwrap()
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn official(dir: &Path) -> Vec<PathBuf> {
    OFFICIAL_PACKS
        .iter()
        .map(|p| dir.join("Data").join(p))
        .filter(|p| p.is_dir())
        .collect()
}

/// The mod folders of a folder that carry a readable About file with a package id, without duplicates.
fn usable_mods(dir: &Path) -> Vec<PathBuf> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let mut names: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for p in names {
        let Ok(about) = read_about(&p) else { continue };
        let Ok(meta) = about.about.into_meta(
            SourceId::new("rs").unwrap(),
            Utf8PathBuf::from_path_buf(p.clone()).unwrap(),
        ) else {
            continue;
        };
        if seen.insert(meta.package_id.as_str().to_lowercase()) {
            out.push(p);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Targets, methods and metrics
// ---------------------------------------------------------------------------------------------------------

/// One weapon to predict: what a designer knows (the vanilla twin) and the truth (the conversion).
struct Target {
    id: String,
    ranged: bool,
    tier: Option<TechLevel>,
    tags: Vec<String>,
    role: Option<String>,
    vanilla: BTreeMap<String, f64>,
    truth: BTreeMap<String, f64>,
}

/// A predicted number with its claimed band.
struct Pred {
    value: f64,
    band: Band,
    /// The reliability rating the predictor gave the stat (empty for the old predictor).
    note: String,
    /// True when a generator would write the number (the old predictor always did).
    usable: bool,
}

type Preds = BTreeMap<String, Pred>;

fn gun_targets(model: &CeModel, only_mods_from: Option<u32>) -> Vec<Target> {
    model
        .guns
        .iter()
        .filter(|g: &&CeGun| g.excluded.is_none())
        .filter(|g| only_mods_from.is_none_or(|from| g.mod_idx.is_some_and(|m| m.0 >= from)))
        .filter_map(|g| {
            let twin = g.twin.clone()?;
            Some(Target {
                id: g.def_name.clone(),
                ranged: true,
                tier: g.tech_level,
                tags: g.twin_tags.clone(),
                role: g.ai_class.clone(),
                vanilla: twin,
                truth: g.stats.clone(),
            })
        })
        .collect()
}

fn melee_targets(model: &CeModel, only_mods_from: Option<u32>) -> Vec<Target> {
    model
        .melee
        .iter()
        .filter(|m: &&CeMelee| m.excluded.is_none())
        .filter(|m| only_mods_from.is_none_or(|from| m.mod_idx.is_some_and(|i| i.0 >= from)))
        .filter_map(|m| {
            let twin = m.twin.clone()?;
            Some(Target {
                id: m.def_name.clone(),
                ranged: false,
                tier: m.tech_level,
                tags: m.twin_tags.clone(),
                role: None,
                vanilla: twin,
                truth: m.stats.clone(),
            })
        })
        .collect()
}

/// The predictor as it was before this round: class median, ratio or elastic fit over a pool.
fn legacy(pool: &Pool, t: &Target, with_role: bool) -> Preds {
    let p = pool
        .index_of(&t.id)
        .map_or_else(|| pool.clone(), |i| pool.without(&[i]));
    let group = (t.ranged).then(|| {
        if t.vanilla.get("burst").copied().unwrap_or(1.0) > 1.0 {
            "burst"
        } else {
            "single"
        }
    });
    let role = if with_role { t.role.as_deref() } else { None };
    let key = class_key(t.tier, role, group);
    let opts = CeClassOptions::default();
    predict_conversion(&p, &key, &t.vanilla, &opts)
        .stats
        .into_iter()
        .map(|(k, s)| {
            (
                k,
                Pred {
                    value: s.value,
                    band: s.band,
                    note: String::new(),
                    usable: true,
                },
            )
        })
        .collect()
}

/// The estimator of this round: similarity classes, forms chosen by leave one out, reliability ratings.
fn after(set: &ExampleSet, t: &Target, with_role: bool) -> Preds {
    let rest = set.without(&t.id);
    let profile = Profile {
        tier: t.tier.map(TechLevel::index),
        tags: t.tags.clone(),
        role: if with_role { t.role.clone() } else { None },
        vanilla: t.vanilla.clone(),
    };
    estimate_conversion(&rest, &profile, &EstimateOptions::default())
        .stats
        .into_iter()
        .map(|(k, s)| {
            (
                k,
                Pred {
                    value: s.value,
                    band: s.band,
                    note: format!("{}/{}", s.predictor_name(), s.reliability.label()),
                    usable: s.is_usable(),
                },
            )
        })
        .collect()
}

#[derive(Default)]
struct Obs {
    kept: Vec<f64>,
    ape: Vec<f64>,
    in50: usize,
    in80: usize,
    notes: BTreeMap<String, usize>,
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

fn evaluate(targets: &[Target], method: &dyn Fn(&Target) -> Preds) -> BTreeMap<String, Obs> {
    let mut out: BTreeMap<String, Obs> = BTreeMap::new();
    for t in targets {
        for (stat, p) in method(t) {
            let Some(truth) = t.truth.get(&stat).copied().filter(|v| *v > 0.0) else {
                continue;
            };
            if !p.value.is_finite() {
                continue;
            }
            let o = out.entry(stat).or_default();
            let ape = (p.value - truth).abs() / truth;
            o.ape.push(ape);
            if p.usable {
                o.kept.push(ape);
            }
            let (l5, h5) = p.band.interval50(p.value);
            let (l8, h8) = p.band.interval80(p.value);
            o.in50 += usize::from(truth >= l5 && truth <= h5);
            o.in80 += usize::from(truth >= l8 && truth <= h8);
            if !p.note.is_empty() {
                *o.notes.entry(p.note.clone()).or_insert(0) += 1;
            }
        }
    }
    out
}

fn print_table(title: &str, rows: &BTreeMap<String, Obs>) {
    println!("--- {title}");
    println!(
        "{:<16} {:>3} {:>9} {:>9} {:>8} {:>8} {:>6} {:>9}  ratings",
        "stat", "n", "med APE", "P80 APE", "in P50", "in P80", "kept", "kept APE"
    );
    for (stat, o) in rows {
        let mut a = o.ape.clone();
        a.sort_by(f64::total_cmp);
        let n = a.len();
        let notes: Vec<String> = o.notes.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        let mut kept = o.kept.clone();
        kept.sort_by(f64::total_cmp);
        let kept_error = if kept.is_empty() {
            "-".to_owned()
        } else {
            format!("{:.1}%", 100.0 * quantile(&kept, 0.5))
        };
        println!(
            "{:<16} {:>3} {:>8.1}% {:>8.1}% {:>7.0}% {:>7.0}% {:>5.0}% {:>9}  {}",
            stat,
            n,
            100.0 * quantile(&a, 0.5),
            100.0 * quantile(&a, 0.8),
            100.0 * o.in50 as f64 / n as f64,
            100.0 * o.in80 as f64 / n as f64,
            100.0 * o.kept.len() as f64 / n as f64,
            kept_error,
            notes.join(" ")
        );
    }
}

fn read_model(
    types: &TypeTable,
    ver: &str,
    ce_roots: &[PathBuf],
    vanilla_roots: &[PathBuf],
) -> CeModel {
    let vanilla = load(prepare(vanilla_roots, ver, Arc::new(types.clone())));
    let classes = CeClassNames::default();
    let ce_types = with_ce_types(types, &classes).unwrap();
    let mut input = prepare(ce_roots, ver, Arc::new(ce_types));
    input.custom_ops = custom_registry(&classes, BTreeMap::new());
    let ce: LoadOutput = load(input);
    read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            order: Some(&ce.order),
            vanilla: Some(&vanilla.databases),
            ..CeReadOptions::default()
        },
    )
}

type Method<'a> = Box<dyn Fn(&Target) -> Preds + 'a>;

fn methods<'a>(
    guns: &'a Pool,
    melee: &'a Pool,
    gun_set: &'a ExampleSet,
    melee_set: &'a ExampleSet,
) -> Vec<(&'static str, Method<'a>)> {
    vec![
        (
            "before: class median and ratio, tag class answered",
            Box::new(move |t: &Target| legacy(if t.ranged { guns } else { melee }, t, true)),
        ),
        (
            "before: class median and ratio, no tag class",
            Box::new(move |t: &Target| legacy(if t.ranged { guns } else { melee }, t, false)),
        ),
        (
            "after: similarity classes with ratings, tag class answered",
            Box::new(move |t: &Target| after(if t.ranged { gun_set } else { melee_set }, t, true)),
        ),
        (
            "after: similarity classes with ratings, no tag class",
            Box::new(move |t: &Target| after(if t.ranged { gun_set } else { melee_set }, t, false)),
        ),
    ]
}

/// The stats a patch actually uses: the estimators are compared on these (the old predictor also
/// returned the numbers that the chosen ammo supplies, which are read from the ammo set now).
const PATCH_GUN_STATS: [&str; 11] = [
    "mass", "bulk", "sway", "spread", "sights", "recoil", "magazine", "reload", "range", "warmup",
    "cooldown",
];
const PATCH_MELEE_STATS: [&str; 10] = [
    "mass",
    "bulk",
    "counter_parry",
    "crit",
    "parry",
    "dodge",
    "tool_power",
    "tool_cooldown",
    "ap_sharp_ratio",
    "ap_blunt_ratio",
];

/// The mean over the patch stats of the median absolute relative error, over all predictions and over the
/// kept ones only (the stats a generator would write), and the share of patch stats that are kept.
fn print_macro(rows: &BTreeMap<String, Obs>, stats: &[&str]) {
    let mut all = Vec::new();
    let mut kept = Vec::new();
    let mut written = 0usize;
    let mut total = 0usize;
    for stat in stats {
        let Some(o) = rows.get(*stat) else { continue };
        let mut a = o.ape.clone();
        a.sort_by(f64::total_cmp);
        all.push(quantile(&a, 0.5));
        total += o.ape.len();
        written += o.kept.len();
        if !o.kept.is_empty() {
            let mut k = o.kept.clone();
            k.sort_by(f64::total_cmp);
            kept.push(quantile(&k, 0.5));
        }
    }
    if all.is_empty() {
        println!("macro over patch stats: no targets");
        return;
    }
    let mean = |v: &[f64]| 100.0 * v.iter().sum::<f64>() / v.len().max(1) as f64;
    println!(
        "macro over patch stats: all predictions {:.1}% ({} stats), written ones {:.1}% ({} stats), {:.0}% of the predictions are written",
        mean(&all),
        all.len(),
        mean(&kept),
        kept.len(),
        100.0 * written as f64 / total.max(1) as f64
    );
}

/// Lists the predictions of the new estimator for each target (held out set only, for local reading).
fn print_detail(
    gun_set: &[Target],
    melee_set: &[Target],
    gun_examples: &ExampleSet,
    melee_examples: &ExampleSet,
) {
    for t in gun_set.iter().chain(melee_set) {
        let set = if t.ranged {
            gun_examples
        } else {
            melee_examples
        };
        println!("  target {}", t.id);
        for (stat, p) in after(set, t, true) {
            if let Some(truth) = t.truth.get(&stat) {
                println!(
                    "    {stat:<16} predicted {:>9.4} actual {:>9.4}  {}",
                    p.value, truth, p.note
                );
            }
        }
    }
}

fn run_sets(title: &str, model_train: &CeModel, gun_set: &[Target], melee_set: &[Target]) {
    let opts = CeClassOptions::default();
    let guns = build_pool(model_train, ItemKind::Ranged, &opts).unwrap();
    let melee = build_pool(model_train, ItemKind::Melee, &opts).unwrap();
    println!(
        "=== {title}: {} guns and {} melee weapons as targets (training pools {} and {})",
        gun_set.len(),
        melee_set.len(),
        guns.len(),
        melee.len()
    );
    let gun_examples = ExampleSet::from_model(model_train, ItemKind::Ranged);
    let melee_examples = ExampleSet::from_model(model_train, ItemKind::Melee);
    if title.contains("held out") && std::env::var_os("RIMSTUDIO_EVAL_DETAIL").is_some() {
        print_detail(gun_set, melee_set, &gun_examples, &melee_examples);
    }
    for (name, method) in methods(&guns, &melee, &gun_examples, &melee_examples) {
        let g = evaluate(gun_set, &*method);
        print_table(&format!("guns, {name}"), &g);
        print_macro(&g, &PATCH_GUN_STATS);
        let m = evaluate(melee_set, &*method);
        print_table(&format!("melee, {name}"), &m);
        print_macro(&m, &PATCH_MELEE_STATS);
    }
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn conversions_are_predicted_from_their_twins_with_leave_one_out() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let ver = version(&dir);
    let base = official(&dir);
    let mut ce_roots = base.clone();
    ce_roots.push(ce_dir.clone());
    let model = read_model(&types, &ver, &ce_roots, &base);
    assert!(model.is_present());
    let guns = gun_targets(&model, None);
    let melee = melee_targets(&model, None);
    run_sets(
        "leave one out over the Combat Extended conversions",
        &model,
        &guns,
        &melee,
    );

    // The held out set: hand written conversions of the custom mods, predicted from the full pool.
    let Some(custom_dir) = std::env::var_os("RIMSTUDIO_CUSTOM_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CUSTOM_DIR is not set: no held out set");
        return;
    };
    let custom = usable_mods(&custom_dir);
    let mut with_custom = ce_roots.clone();
    with_custom.extend(custom.iter().cloned());
    let mut vanilla_custom = base.clone();
    vanilla_custom.extend(custom.iter().cloned());
    let held_model = read_model(&types, &ver, &with_custom, &vanilla_custom);
    let first_custom = u32::try_from(ce_roots.len()).unwrap();
    let held_guns = gun_targets(&held_model, Some(first_custom));
    let held_melee = melee_targets(&held_model, Some(first_custom));
    println!(
        "held out candidates: {} custom mods loaded, {} converted guns and {} converted melee weapons with a twin",
        custom.len(),
        held_guns.len(),
        held_melee.len()
    );
    run_sets(
        "held out hand written conversions",
        &model,
        &held_guns,
        &held_melee,
    );
}
