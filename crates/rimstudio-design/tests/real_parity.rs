//! Numeric parity between the Rust math and the research prototypes, on a real install. Every test is
//! `#[ignore]`, only reads, and needs `RIMSTUDIO_GAME_DIR` (the RimWorld install folder).
//!
//! The prototypes (`docs/research/data/vanilla-weapons`, `docs/research/data/vanilla-apparel`) computed ranged
//! and melee numbers from the same install with small Python scripts. Their tables are read at run time and
//! nothing from them is copied into the repository. The Rust side loads the install through the def engine,
//! reads the weapons with `design::reader` and recomputes: cycle time, nominal and hit adjusted DPS, implied
//! armor penetration, the ranged strength index, the melee stat panel and in fight numbers, the melee strength
//! index, the market value and the armor survival of the worked examples. A deviation table (maximum and
//! median per quantity) is printed and the tolerance (2 percent) is asserted.
//!
//! Run with `cargo test -p rimstudio-design --release --test real_parity -- --ignored --nocapture
//! --test-threads=1`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::too_many_lines
)]

use std::collections::{BTreeMap, BTreeSet};
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
use rimstudio_design::armor::{layer_survival, stack_survival_ratings};
use rimstudio_design::classes::ItemKind;
use rimstudio_design::melee::{
    MeleeAttack, in_fight_dps, stat_panel_dps, strength as melee_strength,
};
use rimstudio_design::ranged::{cycle_time, hit_adjusted_dps, nominal_dps};
use rimstudio_design::reader::access::number_map;
use rimstudio_design::reader::{
    ArmorSource, ReadContext, ReaderOptions, ReferencePools, build_pools, market_value_of,
    read_reference_set,
};
use rimstudio_design::stats::{StatPart, StatPipeline, StatRules, StuffEffect};
use rimstudio_testing::loaders::{
    DirParse, parse_defs_dir_report, parse_patch_dir_report, read_about, read_load_folders,
};

/// The tolerance of the parity check: 2 percent relative.
const TOLERANCE: f64 = 0.02;

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

fn load_vanilla(dir: &Path, types: &TypeTable) -> LoadOutput {
    let input = prepare(&official(dir), &version(dir), Arc::new(types.clone()));
    load(input)
}

// ---------------------------------------------------------------------------------------------- tables

type Row = BTreeMap<String, String>;

fn data_dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data")
        .join(sub)
}

/// Reads a CSV written by the prototypes: a header line, no quoting, no commas inside fields.
fn read_csv(path: &Path) -> Vec<Row> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap_or("")
        .split(',')
        .map(str::to_owned)
        .collect();
    lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            header
                .iter()
                .cloned()
                .zip(l.split(',').map(str::to_owned))
                .collect()
        })
        .collect()
}

fn num(row: &Row, key: &str) -> Option<f64> {
    row.get(key)?.trim().parse().ok()
}

fn text(row: &Row, key: &str) -> String {
    row.get(key).cloned().unwrap_or_default()
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

// ---------------------------------------------------------------------------------------------- deviations

struct Dev {
    stat: String,
    item: String,
    rust: f64,
    proto: f64,
    dev: f64,
}

/// Relative deviation; an absolute difference when the prototype value is zero.
fn deviation(rust: f64, proto: f64) -> f64 {
    if proto.abs() < 1e-9 {
        (rust - proto).abs()
    } else {
        (rust - proto).abs() / proto.abs()
    }
}

#[derive(Default)]
struct Deviations {
    rows: Vec<Dev>,
}

impl Deviations {
    fn add(&mut self, stat: &str, item: &str, rust: f64, proto: f64) {
        self.rows.push(Dev {
            stat: stat.to_owned(),
            item: item.to_owned(),
            rust,
            proto,
            dev: deviation(rust, proto),
        });
    }

    fn add_opt(&mut self, stat: &str, item: &str, rust: Option<f64>, proto: Option<f64>) {
        if let (Some(r), Some(p)) = (rust, proto) {
            self.add(stat, item, r, p);
        }
    }

    /// Prints the table: per quantity the count, the maximum and the median deviation and the worst item.
    fn print(&self, title: &str) {
        let mut by: BTreeMap<&str, Vec<&Dev>> = BTreeMap::new();
        for d in &self.rows {
            by.entry(d.stat.as_str()).or_default().push(d);
        }
        println!("\n== {title} ==");
        println!(
            "{:<18} {:>4} {:>10} {:>10}  worst item",
            "quantity", "n", "max dev", "median"
        );
        for (stat, rows) in by {
            let mut devs: Vec<f64> = rows.iter().map(|d| d.dev).collect();
            devs.sort_by(f64::total_cmp);
            let median = devs.get(devs.len() / 2).copied().unwrap_or(0.0);
            let worst = rows
                .iter()
                .max_by(|a, b| a.dev.total_cmp(&b.dev))
                .map(|d| format!("{} (rust {:.4}, proto {:.4})", d.item, d.rust, d.proto))
                .unwrap_or_default();
            println!(
                "{stat:<18} {:>4} {:>9.3}% {:>9.3}%  {worst}",
                rows.len(),
                devs.last().copied().unwrap_or(0.0) * 100.0,
                median * 100.0
            );
        }
    }

    fn offenders(&self, tolerance: f64) -> Vec<&Dev> {
        self.rows.iter().filter(|d| d.dev > tolerance).collect()
    }

    /// Asserts that no row is beyond the tolerance, listing every offender.
    fn assert_within(&self, tolerance: f64) {
        let bad = self.offenders(tolerance);
        let list: Vec<String> = bad
            .iter()
            .map(|d| {
                format!(
                    "{} {}: rust {:.5} proto {:.5} ({:.2}%)",
                    d.stat,
                    d.item,
                    d.rust,
                    d.proto,
                    d.dev * 100.0
                )
            })
            .collect();
        assert!(
            bad.is_empty(),
            "{} deviations beyond {:.1}%:\n{}",
            bad.len(),
            tolerance * 100.0,
            list.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------------------------- setup

/// The reader options of the parity check: the reference armor of the prototype (unarmored, 0.55 and 1.0
/// sharp) and an optional reference material for stuffed weapons.
fn options(stuff: Option<&str>) -> ReaderOptions {
    ReaderOptions {
        armor: ArmorSource::Given {
            ratings: vec![0.0, 0.55, 1.0],
        },
        reference_stuff: stuff.map(str::to_owned),
        ..ReaderOptions::default()
    }
}

fn install() -> Option<LoadOutput> {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return None;
    };
    Some(load_vanilla(&dir, &types))
}

fn small_volume(out: &LoadOutput, stuff: &str) -> bool {
    out.databases
        .get("ThingDef", stuff)
        .and_then(|d| rimstudio_design::reader::access::child_bool(&d.node, "smallVolume"))
        == Some(true)
}

// ---------------------------------------------------------------------------------------------- ranged

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn ranged_numbers_match_the_prototype_table() {
    let Some(out) = install() else { return };
    let opts = options(None);
    let set = read_reference_set(&out.databases, &opts).unwrap();
    let rows = read_csv(&data_dir("vanilla-weapons/ranged_table.csv"));
    let mut dev = Deviations::default();
    let (mut compared, mut missing) = (0, Vec::new());
    for row in &rows {
        let name = text(row, "defName");
        let Some(w) = set.weapon(&name) else {
            missing.push(name);
            continue;
        };
        let Some(r) = w.ranged.as_ref() else { continue };
        compared += 1;
        let p = r.profile();
        dev.add_opt("damage", &name, Some(r.damage), num(row, "damage"));
        dev.add_opt("ap", &name, Some(r.armor_penetration), num(row, "ap"));
        dev.add_opt("warmup", &name, Some(r.warmup), num(row, "warmup"));
        dev.add_opt("cooldown", &name, Some(r.cooldown), num(row, "cooldown"));
        dev.add_opt("range", &name, Some(r.range), num(row, "range"));
        dev.add_opt(
            "cycle",
            &name,
            cycle_time(&p.cycle_input()).ok(),
            num(row, "cycle"),
        );
        let nominal = nominal_dps(&p).unwrap_or(0.0);
        dev.add_opt("dps_nominal", &name, Some(nominal), num(row, "dps_nominal"));
        for d in [3.0, 12.0, 25.0, 40.0] {
            dev.add_opt(
                &format!("dps_at_{d}"),
                &name,
                Some(hit_adjusted_dps(&p, d).unwrap_or(0.0)),
                num(row, &format!("dps_at_{d}")),
            );
        }
        dev.add_opt("strength_p4", &name, w.strength, num(row, "power"));
        dev.add_opt(
            "market_value",
            &name,
            market_value_of(&out.databases, &opts, &name, None)
                .ok()
                .map(|b| b.base_value),
            num(row, "mv"),
        );
    }
    println!(
        "ranged: {compared} weapons compared, {} prototype rows not read by the reader",
        missing.len()
    );
    dev.print("ranged weapons");
    assert!(compared >= 15, "too few weapons compared: {compared}");
    dev.assert_within(TOLERANCE);
}

// ---------------------------------------------------------------------------------------------- melee

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn melee_numbers_match_the_prototype_tables_for_every_material() {
    let Some(out) = install() else { return };
    let plain = read_csv(&data_dir("vanilla-weapons/melee_table.csv"));
    let stuffed = read_csv(&data_dir("vanilla-weapons/melee_stuff_table.csv"));
    // Rows by reference material ("" for a weapon that is not stuffed).
    let mut by_stuff: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in plain.iter().filter(|r| text(r, "stuff").is_empty()) {
        by_stuff.entry(String::new()).or_default().push(row);
    }
    for row in stuffed.iter().filter(|r| !text(r, "stuff").is_empty()) {
        by_stuff.entry(text(row, "stuff")).or_default().push(row);
    }
    let mut dev = Deviations::default();
    // Rows whose price the prototype gets wrong (its stuff volume rule ignores small volume materials).
    let mut known_price_mismatch: Vec<String> = Vec::new();
    let (mut compared, mut missing) = (0, BTreeSet::new());
    for (stuff, rows) in &by_stuff {
        let opts = options((!stuff.is_empty()).then_some(stuff.as_str()));
        let ctx = ReadContext::new(&out.databases, &opts);
        let set = read_reference_set(&out.databases, &opts).unwrap();
        for row in rows {
            let name = text(row, "defName");
            let Some(w) = set.weapon(&name) else {
                missing.insert(name);
                continue;
            };
            if w.kind != ItemKind::Melee {
                continue;
            }
            compared += 1;
            let item = format!("{name}/{stuff}");
            let attacks = ctx.attacks(w).unwrap();
            let panel = stat_panel_dps(&attacks).unwrap();
            let fight = in_fight_dps(&attacks).unwrap();
            dev.add_opt(
                "panel_damage",
                &item,
                Some(panel.damage),
                num(row, "avg_damage"),
            );
            dev.add_opt(
                "panel_cooldown",
                &item,
                Some(panel.cooldown),
                num(row, "avg_cooldown"),
            );
            dev.add_opt("panel_dps", &item, Some(panel.dps), num(row, "dps"));
            dev.add_opt(
                "panel_ap",
                &item,
                Some(panel.armor_penetration),
                num(row, "avg_ap"),
            );
            dev.add_opt(
                "fight_damage",
                &item,
                Some(fight.swing_damage),
                num(row, "sel_damage"),
            );
            dev.add_opt(
                "fight_cooldown",
                &item,
                Some(fight.swing_cooldown),
                num(row, "sel_cooldown"),
            );
            dev.add_opt("fight_dps", &item, Some(fight.dps), num(row, "dps_select"));
            dev.add_opt(
                "fight_ap",
                &item,
                Some(fight.armor_penetration),
                num(row, "sel_ap"),
            );
            dev.add_opt(
                "strength_pm",
                &item,
                melee_strength(&attacks).ok(),
                num(row, "dps_select")
                    .zip(num(row, "sel_ap"))
                    .map(|(d, a)| d * (1.0 + a)),
            );
            let price = market_value_of(
                &out.databases,
                &opts,
                &name,
                (!stuff.is_empty()).then_some(stuff.as_str()),
            )
            .ok()
            .map(|b| b.base_value);
            if !stuff.is_empty() && small_volume(&out, stuff) {
                known_price_mismatch.push(format!(
                    "{item}: rust {:?}, prototype {:?}",
                    price,
                    num(row, "mv")
                ));
            } else {
                dev.add_opt("market_value", &item, price, num(row, "mv"));
            }
        }
    }
    println!(
        "melee: {compared} (weapon, material) rows compared, {} materials, prototype weapons not read: {missing:?}",
        by_stuff.len()
    );
    println!(
        "price rows with a small volume material (prototype ignores the volume rule): {}",
        known_price_mismatch.len()
    );
    for line in known_price_mismatch.iter().take(6) {
        println!("  {line}");
    }
    dev.print("melee weapons");
    assert!(compared >= 40, "too few rows compared: {compared}");
    dev.assert_within(TOLERANCE);
}

// ---------------------------------------------------------------------------------------------- worked examples

fn close(rust: f64, proto: f64) -> bool {
    deviation(rust, proto) < 1e-6
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn the_worked_examples_of_the_notes_reproduce_to_six_digits() {
    let Some(out) = install() else { return };
    let worked = read_json(&data_dir("vanilla-weapons/worked_examples.json"));
    let opts = options(None);
    let set = read_reference_set(&out.databases, &opts).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (name, e) in worked["ranged"].as_object().unwrap() {
        let Some(w) = set.weapon(name) else {
            println!("worked ranged example {name} is not read by the reader (excluded weapon)");
            continue;
        };
        let r = w.ranged.as_ref().unwrap();
        let p = r.profile();
        let get = |k: &str| e[k].as_f64().unwrap();
        let mut check = |what: &str, rust: f64, proto: f64| {
            checked += 1;
            if !close(rust, proto) {
                failures.push(format!("{name} {what}: rust {rust} proto {proto}"));
            }
        };
        check("damage", r.damage, get("damage"));
        check("ap", r.armor_penetration, get("ap"));
        check("cycle", cycle_time(&p.cycle_input()).unwrap(), get("cycle"));
        check("dps_nominal", nominal_dps(&p).unwrap(), get("dps_nominal"));
        check("range", r.range, get("range"));
        let mv = market_value_of(&out.databases, &opts, name, None).unwrap();
        check("mv", mv.base_value, get("mv_formula").max(get("mv")));
    }
    // Melee: stat panel and in fight numbers per material, then a Legendary quality variant.
    let quality = read_json(&data_dir("vanilla-weapons/quality_factors.json"));
    let legendary = quality["MeleeWeapon_DamageMultiplier"]["factorLegendary"]
        .as_f64()
        .unwrap();
    for (name, per_stuff) in worked["melee"].as_object().unwrap() {
        for (key, e) in per_stuff.as_object().unwrap() {
            let (stuff, quality_factor) = match key.as_str() {
                "steel" => ("Steel", 1.0),
                "plasteel" => ("Plasteel", 1.0),
                "steel_legendary" => ("Steel", legendary),
                _ => continue,
            };
            let stuff_opts = options(Some(stuff));
            let ctx = ReadContext::new(&out.databases, &stuff_opts);
            let stuffed_set = read_reference_set(&out.databases, &stuff_opts).unwrap();
            let Some(w) = stuffed_set.weapon(name) else {
                continue;
            };
            if w.stuff_categories.is_empty() {
                continue;
            }
            let mut attacks = ctx.attacks(w).unwrap();
            if (quality_factor - 1.0).abs() > 1e-12 {
                // The quality multiplies damage and the explicit penetration of every attack alike.
                attacks = attacks
                    .into_iter()
                    .map(|a| MeleeAttack {
                        damage: a.damage * quality_factor,
                        armor_penetration: a.armor_penetration * quality_factor,
                        ..a
                    })
                    .collect();
            }
            let panel = stat_panel_dps(&attacks).unwrap();
            let fight = in_fight_dps(&attacks).unwrap();
            let get = |k: &str| e[k].as_f64().unwrap();
            for (what, rust, proto) in [
                ("avg_damage", panel.damage, get("avg_damage")),
                ("avg_cooldown", panel.cooldown, get("avg_cooldown")),
                ("dps", panel.dps, get("dps")),
                ("sel_damage", fight.swing_damage, get("sel_damage")),
                ("dps_select", fight.dps, get("dps_select")),
            ] {
                checked += 1;
                // The legendary variant scales explicit penetration only by the damage multiplier in the
                // prototype, and implied penetration by the scaled damage; both scale by the same factor.
                if !close(rust, proto) {
                    failures.push(format!("{name}/{key} {what}: rust {rust} proto {proto}"));
                }
            }
        }
    }
    println!("worked examples: {checked} numbers checked");
    assert!(checked > 40);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------------------------------------- armor

/// The sharp, blunt and heat rating of an apparel made from a material, through the stat pipeline with the stuff
/// part (item multiplier times stuff power), clamped to `[0, 2]`.
fn armor_ratings(out: &LoadOutput, item: &str, stuff: &str) -> Option<[f64; 3]> {
    let item_node = &out.databases.get("ThingDef", item)?.node;
    let stuff_node = &out.databases.get("ThingDef", stuff)?.node;
    let item_stats = number_map(item_node, "statBases");
    let stuff_stats = number_map(stuff_node, "statBases");
    let multiplier = item_stats
        .get(concat!("StuffEffect", "MultiplierArmor"))
        .copied()?;
    let rules = StatRules {
        min_value: 0.0,
        max_value: 2.0,
        ..StatRules::default()
    };
    let mut ratings = [0.0; 3];
    for (slot, (rating, power)) in [
        (
            concat!("ArmorRating", "_Sharp"),
            concat!("StuffPower", "_Armor_Sharp"),
        ),
        (
            concat!("ArmorRating", "_Blunt"),
            concat!("StuffPower", "_Armor_Blunt"),
        ),
        (
            concat!("ArmorRating", "_Heat"),
            concat!("StuffPower", "_Armor_Heat"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let pipeline = StatPipeline {
            rules,
            base: item_stats.get(rating).copied(),
            stuff: Some(StuffEffect {
                factor: None,
                offset: 0.0,
            }),
            parts: vec![StatPart::Stuff {
                priority: 1,
                multiplier,
                stuff_power: stuff_stats.get(power).copied().unwrap_or(0.0),
            }],
        };
        ratings[slot] = pipeline.evaluate().ok()?.value;
    }
    Some(ratings)
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn plate_armor_ratings_price_and_survival_match_the_apparel_notes() {
    let Some(out) = install() else { return };
    let models = read_json(&data_dir("vanilla-apparel/models.json"));
    let opts = options(None);
    let mut dev = Deviations::default();
    for e in models["worked"].as_array().unwrap() {
        if e["quality"].as_str() != Some("Normal") {
            continue;
        }
        let (item, stuff) = (e["defName"].as_str().unwrap(), e["stuff"].as_str().unwrap());
        let Some(r) = armor_ratings(&out, item, stuff) else {
            println!("{item} in {stuff}: not computable from the install");
            continue;
        };
        let id = format!("{item}/{stuff}");
        dev.add("sharp", &id, r[0], e["sharp"].as_f64().unwrap());
        dev.add("blunt", &id, r[1], e["blunt"].as_f64().unwrap());
        dev.add("heat", &id, r[2], e["heat"].as_f64().unwrap());
        let price = market_value_of(&out.databases, &opts, item, Some(stuff)).unwrap();
        dev.add(
            "price_displayed",
            &id,
            price.displayed,
            e["market_value"].as_f64().unwrap(),
        );
        // Survival of the layer at the penetration of a few weapons, against the closed form of the notes
        // evaluated on the prototype's rating.
        let proto_sharp = e["sharp"].as_f64().unwrap();
        for ap in [0.0, 0.15, 0.165, 0.30] {
            let e_eff = (proto_sharp - ap).max(0.0);
            let closed = if e_eff <= 1.0 {
                1.0 - 0.75 * e_eff
            } else {
                0.5 - e_eff / 4.0
            };
            dev.add(
                &format!("survival_ap_{ap}"),
                &id,
                layer_survival(r[0], ap).unwrap(),
                closed.max(0.0),
            );
        }
    }
    // The stacked layer examples of the notes, rounded to four digits there.
    for s in models["layer_stacking"].as_array().unwrap() {
        let ratings: Vec<f64> = s["ratings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        dev.add(
            "stacked_layers",
            &format!("{ratings:?}@{}", s["ap"]),
            stack_survival_ratings(&ratings, s["ap"].as_f64().unwrap()).unwrap(),
            s["mean_factor"].as_f64().unwrap(),
        );
    }
    let torso = &models["pawn_example"]["torso"];
    let ratings: Vec<f64> = torso["ratings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    dev.add(
        "pawn_torso",
        "AR-5",
        stack_survival_ratings(&ratings, 0.15).unwrap(),
        torso["closed_form_ap0.15"].as_f64().unwrap(),
    );
    dev.print("apparel and armor survival");
    assert!(dev.rows.len() > 20);
    // The notes round to four digits, so allow that much on top of the tolerance for tiny values.
    dev.assert_within(TOLERANCE);
}

// ---------------------------------------------------------------------------------------------- pools

/// The roles of the prototype that count as direct fire weapons.
const DIRECT_ROLES: [&str; 7] = [
    "bow", "pistol", "smg", "rifle", "sniper", "shotgun", "heavy",
];

fn set_of<'a>(it: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    it.map(str::to_owned).collect()
}

fn compare_sets(label: &str, proto: &BTreeSet<String>, rust: &BTreeSet<String>) {
    let only_proto: Vec<_> = proto.difference(rust).collect();
    let only_rust: Vec<_> = rust.difference(proto).collect();
    println!(
        "{label}: prototype {} items, reader {} items, shared {}",
        proto.len(),
        rust.len(),
        proto.intersection(rust).count()
    );
    println!("  only in the prototype: {only_proto:?}");
    println!("  only in the reader: {only_rust:?}");
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn reference_pools_match_the_prototype_classes_and_drop_no_real_weapon() {
    let Some(out) = install() else { return };
    // Steel is the reference material of the melee prototype.
    let opts = options(Some("Steel"));
    let pools: ReferencePools = build_pools(&out.databases, &opts).unwrap();
    let ranged_rows = read_csv(&data_dir("vanilla-weapons/ranged_table.csv"));
    let melee_rows = read_csv(&data_dir("vanilla-weapons/melee_table.csv"));

    // Ranged: the direct fire set of the prototype is the standard group with a direct fire role.
    let proto_ranged = set_of(
        ranged_rows
            .iter()
            .filter(|r| {
                text(r, "group") == "standard" && DIRECT_ROLES.contains(&text(r, "role").as_str())
            })
            .map(|r| r.get("defName").map_or("", String::as_str)),
    );
    let rust_ranged = set_of(pools.ranged.items.iter().map(|i| i.id.as_str()));
    compare_sets("ranged pool", &proto_ranged, &rust_ranged);
    let skip_reason = |name: &str| {
        pools
            .set
            .skipped
            .iter()
            .find(|s| s.def_name == name)
            .map_or_else(
                || "not a weapon candidate".to_owned(),
                |s| format!("{:?}", s.reason),
            )
    };
    for name in proto_ranged.difference(&rust_ranged) {
        println!(
            "  {name} missing from the reader pool: {}",
            skip_reason(name)
        );
    }
    for name in rust_ranged.difference(&proto_ranged) {
        let proto = ranged_rows.iter().find(|r| text(r, "defName") == *name);
        println!(
            "  {name} extra in the reader pool; prototype group/role: {:?}",
            proto.map(|r| (text(r, "group"), text(r, "role"), text(r, "kind")))
        );
    }
    // Tier and role agreement on the shared items.
    let (mut tier_same, mut tier_n, mut role_same, mut role_n) = (0, 0, 0, 0);
    let mut role_pairs: BTreeMap<(String, String), usize> = BTreeMap::new();
    for item in &pools.ranged.items {
        let Some(row) = ranged_rows.iter().find(|r| text(r, "defName") == item.id) else {
            continue;
        };
        // The prototype does not class its creature weapons; they take no part in the role agreement.
        if !proto_ranged.contains(&item.id) {
            continue;
        }
        tier_n += 1;
        if num(row, "tier").is_some_and(|t| (t - f64::from(item.tier)).abs() < 0.5) {
            tier_same += 1;
        }
        role_n += 1;
        *role_pairs
            .entry((text(row, "role"), item.role.clone()))
            .or_insert(0) += 1;
        if text(row, "role") == item.role {
            role_same += 1;
        }
    }
    println!(
        "ranged tiers agree on {tier_same} of {tier_n}; roles agree on {role_same} of {role_n} \
         (the reader's decision list against the prototype's hand table)"
    );
    println!("  role pairs (prototype, reader): {role_pairs:?}");

    // Melee: every prototype melee weapon, standard and bladelink.
    let proto_melee = set_of(
        melee_rows
            .iter()
            .map(|r| r.get("defName").map_or("", String::as_str)),
    );
    let rust_melee = set_of(pools.melee.items.iter().map(|i| i.id.as_str()));
    compare_sets("melee pool", &proto_melee, &rust_melee);
    for name in proto_melee.difference(&rust_melee) {
        println!(
            "  {name} missing from the reader pool: {}",
            skip_reason(name)
        );
    }
    let (mut mt_same, mut mt_n) = (0, 0);
    for item in &pools.melee.items {
        if let Some(row) = melee_rows.iter().find(|r| text(r, "defName") == item.id) {
            mt_n += 1;
            if num(row, "tier").is_some_and(|t| (t - f64::from(item.tier)).abs() < 0.5) {
                mt_same += 1;
            }
        }
    }
    println!("melee tiers agree on {mt_same} of {mt_n}");

    // Strength of the shared ranged items against the prototype power column.
    let mut dev = Deviations::default();
    for item in &pools.ranged.items {
        if let Some(row) = ranged_rows.iter().find(|r| text(r, "defName") == item.id) {
            dev.add_opt(
                "pool_strength",
                &item.id,
                Some(item.strength),
                num(row, "power"),
            );
        }
    }
    for item in &pools.melee.items {
        if let Some(row) = melee_rows.iter().find(|r| text(r, "defName") == item.id) {
            dev.add_opt(
                "pool_strength_melee",
                &item.id,
                Some(item.strength),
                num(row, "dps_select")
                    .zip(num(row, "sel_ap"))
                    .map(|(d, a)| d * (1.0 + a)),
            );
        }
    }
    for item in &pools.ranged.items {
        if let Some(row) = ranged_rows.iter().find(|r| text(r, "defName") == item.id) {
            dev.add_opt(
                "pool_market_value",
                &item.id,
                item.stat("market_value"),
                num(row, "mv"),
            );
        }
    }
    for item in &pools.melee.items {
        if let Some(row) = melee_rows.iter().find(|r| text(r, "defName") == item.id) {
            dev.add_opt(
                "pool_market_value_melee",
                &item.id,
                item.stat("market_value"),
                num(row, "mv"),
            );
        }
    }
    dev.print("pool strength and price");

    // The audit: every primary equipment def with verbs or tools and where it ended up.
    let mut outcome: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let in_pool: BTreeSet<&str> = pools
        .set
        .weapons
        .iter()
        .map(|w| w.def_name.as_str())
        .collect();
    if let Ok(view) = out.databases.database("ThingDef") {
        for def in view.iter() {
            let node = &def.node;
            let primary = node
                .child("equipmentType")
                .is_some_and(|n| n.text_content().trim() == "Primary");
            let has_attack = node.child("verbs").is_some() || node.child("tools").is_some();
            if !(primary && has_attack) {
                continue;
            }
            let key = if in_pool.contains(def.def_name.as_str()) {
                "in a pool".to_owned()
            } else if let Some(s) = pools
                .set
                .skipped
                .iter()
                .find(|s| s.def_name == def.def_name)
            {
                format!("skipped: {:?}", s.reason)
            } else {
                "not read (no weapon tags, or verbs without a projectile)".to_owned()
            };
            outcome.entry(key).or_default().push(def.def_name.clone());
        }
    }
    println!("\naudit of primary equipment with verbs or tools:");
    for (k, v) in &outcome {
        if k == "in a pool" {
            println!("  {k}: {}", v.len());
        } else {
            println!("  {k}: {} {v:?}", v.len());
        }
    }

    // The gates: nothing the prototype counts as a direct fire or melee weapon is dropped by mistake.
    let dropped_ranged: Vec<_> = proto_ranged.difference(&rust_ranged).collect();
    let dropped_melee: Vec<_> = proto_melee.difference(&rust_melee).collect();
    assert!(
        dropped_ranged.is_empty(),
        "dropped ranged: {dropped_ranged:?}"
    );
    assert!(dropped_melee.is_empty(), "dropped melee: {dropped_melee:?}");
    dev.assert_within(TOLERANCE);
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn the_default_reader_options_rank_the_weapons_like_the_prototype() {
    let Some(out) = install() else { return };
    // The defaults derive the reference armor from the install (unarmored, median and 90th percentile of the
    // positive sharp ratings of fixed apparel) instead of the prototype's two named layers.
    let opts = ReaderOptions::default();
    let pools = build_pools(&out.databases, &opts).unwrap();
    println!(
        "derived reference armor {:?} from {} fixed armor items (fallback {})",
        pools.set.armor.ratings, pools.set.armor.derived_from, pools.set.armor.fallback
    );
    let ranged_rows = read_csv(&data_dir("vanilla-weapons/ranged_table.csv"));
    let melee_rows = read_csv(&data_dir("vanilla-weapons/melee_table.csv"));
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for item in &pools.ranged.items {
        if let Some(p) = ranged_rows
            .iter()
            .find(|r| text(r, "defName") == item.id)
            .and_then(|r| num(r, "power"))
        {
            a.push(item.strength);
            b.push(p);
        }
    }
    let rho = rimstudio_design::classes::numeric::spearman(&a, &b).unwrap();
    let mut dev = Deviations::default();
    for (x, y) in a.iter().zip(&b) {
        dev.add("ranged_strength_ratio", "all", x / y, 1.0);
    }
    println!("ranged strength with derived armor: Spearman {rho:.4} against the prototype index");
    dev.print("derived armor against the prototype layers (ratio to 1)");
    assert!(rho > 0.97, "ranking changed too much: {rho}");
    let (mut m, mut n) = (Vec::new(), Vec::new());
    for item in &pools.melee.items {
        if let Some(row) = melee_rows.iter().find(|r| text(r, "defName") == item.id)
            && let (Some(d), Some(ap)) = (num(row, "dps_select"), num(row, "sel_ap"))
        {
            m.push(item.strength);
            n.push(d * (1.0 + ap));
        }
    }
    let rho_melee = rimstudio_design::classes::numeric::spearman(&m, &n).unwrap();
    println!("melee strength, unstuffed reading: Spearman {rho_melee:.4}");
    assert!(rho_melee > 0.99);
    // Without a reference material the stuffed weapons are priced with the game's guess of 2 silver per stuff
    // unit and read with neutral multipliers (the prototype used Steel).
    let mut price = Deviations::default();
    let mut strength = Deviations::default();
    for item in &pools.melee.items {
        if let Some(row) = melee_rows.iter().find(|r| text(r, "defName") == item.id) {
            price.add_opt(
                "market_value",
                &item.id,
                item.stat("market_value"),
                num(row, "mv"),
            );
            strength.add_opt(
                "strength_pm",
                &item.id,
                Some(item.strength),
                num(row, "dps_select")
                    .zip(num(row, "sel_ap"))
                    .map(|(d, a)| d * (1.0 + a)),
            );
        }
    }
    price.print("melee, default options (no reference material)");
    strength.print("melee strength, default options");
    strength.assert_within(TOLERANCE);
}
