//! Converts a few real weapons of an install through the automatic flow and prints what comes out, for
//! review. Every test is `#[ignore]`, only reads, and needs environment variables:
//!
//! - `RIMSTUDIO_GAME_DIR`: the RimWorld install folder (holds `Version.txt` and `Data/`);
//! - `RIMSTUDIO_CE_DIR`: a Combat Extended mod folder.
//!
//! Run with `cargo test -p rimstudio-design --release --test real_convert -- --ignored --nocapture
//! --test-threads=1`. Nothing is asserted about numbers and nothing is written: the output is for a human to
//! read and must not be committed.

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
use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, PatchFile, TypeTable, load};
use rimstudio_design::ce::reader::{
    CeClassNames, CeReadOptions, custom_registry, read_conversions_with, with_ce_types,
};
use rimstudio_design::reader::ReaderOptions;
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

use rimstudio_design::ce::patchgen::{
    CeProjectState, ConversionSource, ConvertAnswers, ConvertEnv, ConvertStatus, convert, scan,
};
use rimstudio_design::plan::ProjectLayout;

fn raw_things(input: &LoadInput) -> Vec<rimstudio_core::tree::Node> {
    let mut out = Vec::new();
    for f in &input.def_files {
        if let FileContent::Parsed { root } = &f.content {
            out.extend(root.elements().filter(|e| e.tag == "ThingDef").cloned());
        }
    }
    out
}

fn summary(node: &rimstudio_core::tree::Node) -> String {
    fn walk(n: &rimstudio_core::tree::Node, out: &mut Vec<String>) {
        if n.elements().next().is_none() {
            out.push(format!("{}={}", n.tag, n.text_content()));
        }
        for c in n.elements() {
            walk(c, out);
        }
    }
    let mut parts = Vec::new();
    walk(node, &mut parts);
    parts.join(" ")
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn a_few_real_weapons_are_converted_by_the_automatic_flow() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    // The vanilla load: the targets, exactly as a mod author's project would see them.
    let vanilla_input = prepare(&official(&dir), &version(&dir), Arc::new(types.clone()));
    let project = raw_things(&vanilla_input);
    let vanilla = load(vanilla_input);
    // The Combat Extended load: the model that the conversions are learnt from.
    let classes = CeClassNames::default();
    let ce_types = with_ce_types(&types, &classes).unwrap();
    let mut roots = official(&dir);
    roots.push(ce_dir);
    let mut input = prepare(&roots, &version(&dir), Arc::new(ce_types));
    input.custom_ops = custom_registry(&classes, BTreeMap::new());
    let ce = load(input);
    let model = read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            order: Some(&ce.order),
            vanilla: Some(&vanilla.databases),
            ..CeReadOptions::default()
        },
    );
    assert!(model.is_present());
    let candidates = scan(&project, &vanilla.databases, &model);
    let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
    for c in &candidates {
        *by_status.entry(format!("{:?}", c.status)).or_insert(0) += 1;
    }
    println!("{} candidates: {by_status:?}", candidates.len());
    let layout = ProjectLayout::default();
    let state = CeProjectState::default();
    let reader = ReaderOptions::default();
    let source = ConversionSource::Unknown;
    let env = ConvertEnv {
        dbs: &vanilla.databases,
        model: &model,
        layout: &layout,
        reader: &reader,
        project: &project,
        state: &state,
        source: &source,
    };
    // The weapons that Combat Extended itself converts, so that the output can be compared with its numbers.
    let mut shown = 0usize;
    for c in &candidates {
        if c.status != ConvertStatus::NotConverted || shown >= 5 {
            continue;
        }
        let real_gun = model.gun(&c.def);
        let real_melee = model.melee_weapon(&c.def);
        if real_gun.is_none() && real_melee.is_none() {
            continue;
        }
        shown += 1;
        let answers = ConvertAnswers {
            ammo_set: real_gun.and_then(|g| g.ammo_set.clone()),
            weapon_tag_class: real_gun.and_then(|g| g.ai_class.clone()),
            one_handed: Some(false),
            belt_fed: Some(false),
            ..ConvertAnswers::default()
        };
        let out = convert(c, &answers, &env);
        println!("== {} ({:?}) ==", c.def, c.kind);
        println!(
            "asks: {:?}",
            out.asks.items.iter().map(|a| &a.field).collect::<Vec<_>>()
        );
        for d in &out.derived {
            println!("  derived {} = {} ({:?})", d.field, d.value, d.origin);
        }
        for f in &out.plan.files {
            println!("  file {} ({:?})", f.path, f.kind);
            if let Some(tree) = &f.tree
                && f.path.contains("Patches")
            {
                for op in tree.elements() {
                    println!("    {}", summary(op));
                }
            }
        }
        if let Some(g) = real_gun {
            println!("  real CE numbers: {:?}", g.stats);
        }
        if let Some(m) = real_melee {
            println!("  real CE numbers: {:?}", m.stats);
        }
        for d in &out.plan.diagnostics {
            println!("  diagnostic {} {}", d.code.as_str(), d.message);
        }
    }
    println!("{shown} weapons converted for review");
}

/// Mod folders (children of `dir` with an About file) whose own def files hold a melee shaped weapon with
/// no weapon tags that the reference pools would still count.
fn suspect_mods(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for name in child_dirs(dir) {
        let root = dir.join(&name);
        if read_about(&root).is_err() {
            continue;
        }
        // The top level Defs folder and the Defs folder of every version or option folder below the root.
        let mut defs_dirs = vec![root.join("Defs")];
        defs_dirs.extend(
            child_dirs(&root)
                .into_iter()
                .map(|c| root.join(c).join("Defs")),
        );
        let hit = defs_dirs.iter().any(|d| {
            parse_defs_dir_report(d).is_ok_and(|parse| {
                parse.files.iter().any(|f| {
                    f.root.elements().any(|n| {
                        n.tag == "ThingDef"
                            && n.child("tools").is_some()
                            && n.child("weaponTags").is_none()
                            && rimstudio_design::reader::is_weapon_def(n)
                    })
                })
            })
        });
        if hit {
            found.push(root);
        }
    }
    found
}

fn is_melee_candidate(node: &rimstudio_core::tree::Node, require_tags: bool) -> bool {
    let shooting = node.child("verbs").is_some_and(|v| {
        v.elements()
            .any(|li| li.child("defaultProjectile").is_some())
    });
    !shooting
        && node.child("tools").is_some()
        && if require_tags {
            node.child("weaponTags").is_some()
        } else {
            rimstudio_design::reader::is_weapon_def(node)
        }
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and a folder of mods in RIMSTUDIO_WORKSHOP_DIR or RIMSTUDIO_CUSTOM_DIR"]
fn the_melee_candidate_count_before_and_after_the_pool_rule_is_printed() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let mut mods = Vec::new();
    for var in ["RIMSTUDIO_WORKSHOP_DIR", "RIMSTUDIO_CUSTOM_DIR"] {
        if let Some(d) = std::env::var_os(var).map(PathBuf::from) {
            mods.extend(suspect_mods(&d));
        }
    }
    println!(
        "{} mod folders hold a weapon without weapon tags",
        mods.len()
    );
    for var in ["RIMSTUDIO_WORKSHOP_DIR", "RIMSTUDIO_CUSTOM_DIR"] {
        let d = std::env::var_os(var).map(PathBuf::from).unwrap_or_default();
        println!("{var}: {} child folders", child_dirs(&d).len());
    }
    let official = official(&dir);
    let official_count = official.len();
    let mut roots = official;
    roots.extend(mods.iter().cloned());
    let out = load(prepare(&roots, &version(&dir), Arc::new(types)));
    let mut before = 0usize;
    let mut after = 0usize;
    println!("{} defs loaded", out.databases.defs().len());
    for rec in out.databases.defs() {
        if rec.tag != "ThingDef" && !rec.type_name.ends_with("ThingDef") {
            continue;
        }
        let old = is_melee_candidate(&rec.node, true);
        let new = is_melee_candidate(&rec.node, false);
        before += usize::from(old);
        after += usize::from(new);
        if new && !old {
            let from = rec.mod_idx().map_or(0, |m| m.0 as usize);
            let from = from.checked_sub(official_count).and_then(|i| mods.get(i));
            println!("  gained: {} from {:?}", rec.def_name, from);
        }
    }
    println!("melee candidates before {before}, after {after}");
}
