//! Reads a real install and prints what the reader and the class statistics make of it. Every test is
//! `#[ignore]`, only reads, and needs environment variables:
//!
//! - `RIMSTUDIO_GAME_DIR`: the RimWorld install folder (holds `Version.txt` and `Data/`);
//! - `RIMSTUDIO_CE_DIR` (optional): a Combat Extended mod folder.
//!
//! Run with `cargo test -p rimstudio-design --release --test real_data -- --ignored --nocapture
//! --test-threads=1`. The def type table is read at run time from `docs/research/data/def-engine`; nothing
//! from an install is copied into the repository and the tests assert structure only. End to end acceptance
//! tests belong to the integration task.

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
use rimstudio_design::ce::classes::{
    CeClassOptions, ClassStats, build_pool, class_key, predict_conversion,
};
use rimstudio_design::ce::reader::{
    CeClassNames, CeReadOptions, custom_registry, read_conversions, read_conversions_with,
    with_ce_types,
};
use rimstudio_design::classes::ItemKind;
use rimstudio_design::reader::{ReaderOptions, build_pools};
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

fn load_vanilla(dir: &Path, types: &TypeTable) -> LoadOutput {
    let input = prepare(&official(dir), &version(dir), Arc::new(types.clone()));
    load(input)
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn the_official_packs_give_ranged_and_melee_pools() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let out = load_vanilla(&dir, &types);
    let pools = build_pools(&out.databases, &ReaderOptions::default()).unwrap();
    println!(
        "ranged pool {} items, melee pool {} items, skipped {} candidates, armor layers {:?}",
        pools.ranged.len(),
        pools.melee.len(),
        pools.set.skipped.len(),
        pools.set.armor
    );
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for s in &pools.set.skipped {
        *reasons.entry(format!("{:?}", s.reason)).or_insert(0) += 1;
    }
    println!("skip reasons {reasons:?}");
    println!(
        "ranged roles {:?}, tiers {:?}",
        pools.ranged.roles(),
        pools.ranged.tiers()
    );
    println!(
        "melee roles {:?}, tiers {:?}",
        pools.melee.roles(),
        pools.melee.tiers()
    );
    println!(
        "melee items {:?}",
        pools
            .melee
            .items
            .iter()
            .map(|i| format!("{}:{}", i.id, i.role))
            .collect::<Vec<_>>()
    );
    println!(
        "ranged items {:?}",
        pools
            .ranged
            .items
            .iter()
            .map(|i| format!("{}:{}:{:.2}", i.id, i.role, i.strength))
            .collect::<Vec<_>>()
    );
    assert!(!pools.ranged.is_empty());
    assert!(!pools.melee.is_empty());
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn combat_extended_conversions_are_read_and_paired_with_their_vanilla_twins() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let vanilla = load_vanilla(&dir, &types);
    let classes = CeClassNames::default();
    let ce_types = with_ce_types(&types, &classes).unwrap();
    let mut roots = official(&dir);
    roots.push(ce_dir);
    let mut input = prepare(&roots, &version(&dir), Arc::new(ce_types));
    let settings: BTreeMap<String, bool> = BTreeMap::new();
    input.custom_ops = custom_registry(&classes, settings);
    let out = load(input);
    let options = CeReadOptions {
        order: Some(&out.order),
        vanilla: Some(&vanilla.databases),
        ..CeReadOptions::default()
    };
    let model = read_conversions_with(&out.databases, &options);
    let plain = read_conversions(&out.databases);
    println!(
        "present {} (without the load order: {}), ammo sets {}, guns {} ({} with twins), melee {}, tags {}, ai classes {:?}, gun presets {}, apparel presets {}",
        model.is_present(),
        plain.is_present(),
        model.ammo_sets.len(),
        model.guns.len(),
        model.twin_count(),
        model.melee.len(),
        model.weapon_tags.len(),
        model.ai_class_tags,
        model.gun_presets.len(),
        model.apparel_presets.len()
    );
    println!(
        "names {:?}, shoot verb {}",
        model.names, model.classes.shoot_verb
    );
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    println!(
        "CE ranged pool {} items (excluded {})",
        pool.len(),
        pool.excluded.len()
    );
    let key = class_key(None, None, None);
    let stats = ClassStats::derive(&pool, &key, &opts);
    println!(
        "class: {} ; constants {:?}",
        stats.base.explanation,
        stats
            .constants
            .iter()
            .map(|c| (&c.stat, c.value))
            .collect::<Vec<_>>()
    );
    let vanilla_input: BTreeMap<String, f64> = [
        ("mass", 3.0),
        ("range", 30.0),
        ("warmup", 1.0),
        ("cooldown", 1.5),
        ("burst", 1.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let p = predict_conversion(&pool, &key, &vanilla_input, &opts);
    for (name, s) in &p.stats {
        println!(
            "  {name}: {:.3} by {:?} (n {}, band x{:.2})",
            s.value, s.predictor, s.n, s.band.p50
        );
    }
    assert!(model.is_present());
}
