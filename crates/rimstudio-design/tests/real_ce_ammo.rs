//! Custom ammunition against the user's own Combat Extended.
//!
//! For every ammo set of the installed Combat Extended the test rebuilds a custom ammo spec from the real
//! values as typed values, generates the definition files, loads them as one more mod after the real defs,
//! and compares each generated def (the set, every ammo item, projectile and recipe) with the real one leaf
//! by leaf on the resolved nodes. It also prints the catalogue size, the lint and dry load results of the
//! generated files and the plausibility warnings the checks give for the real values.
//!
//! Needs `RIMSTUDIO_GAME_DIR` and `RIMSTUDIO_CE_DIR`. Run with `cargo test -p rimstudio-design --release
//! --test real_ce_ammo -- --ignored --nocapture`. Nothing is written.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_ammo;
mod common_ce;
mod common_real;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use common_real::{game_dir, official, prepare, type_table, version};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::Node;
use rimstudio_defs::{DefFile, FileContent, ModEntry, load};
use rimstudio_design::ce::ammo::{self, CatalogQuery};
use rimstudio_design::ce::reader::{
    CeClassNames, CeReadOptions, custom_registry, read_conversions_with, with_ce_types,
};

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn rebuilt_ammunition_matches_the_real_definitions() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let classes = CeClassNames::default();
    let ce_types = Arc::new(with_ce_types(&types, &classes).unwrap());
    let mut roots = official(&dir);
    roots.push(ce_dir);
    let build = |extra: Option<Node>| {
        let mut input = prepare(&roots, &version(&dir), ce_types.clone());
        input.custom_ops = custom_registry(&classes, BTreeMap::new());
        if let Some(root) = extra {
            let idx = ModIdx(u32::try_from(input.mods.len()).unwrap());
            input
                .mods
                .push(ModEntry::new(idx, "rs.generated", "generated ammunition"));
            let file =
                FileId(u32::try_from(input.def_files.len() + input.patch_files.len() + 1).unwrap());
            input.def_files.push(DefFile {
                mod_idx: idx,
                file,
                rel_path: "Defs/Generated.xml".to_owned(),
                content: FileContent::parsed(root),
            });
        }
        load(input)
    };
    let started = std::time::Instant::now();
    let real = build(None);
    let read_started = std::time::Instant::now();
    let model = read_conversions_with(
        &real.databases,
        &CeReadOptions {
            order: Some(&real.order),
            ..CeReadOptions::default()
        },
    );
    println!("read_conversions_with took {:?}", read_started.elapsed());
    assert!(model.is_present());
    let lib = &model.ammo;
    println!(
        "library: {} classes, {} ammo items, {} projectiles, {} recipes, {} thing categories ({:?} pairs), read in {:?}",
        lib.classes.len(),
        lib.ammo.len(),
        lib.projectiles.len(),
        lib.recipes.len(),
        lib.categories.len(),
        lib.pair_form,
        started.elapsed()
    );
    let page = ammo::catalog(
        &model,
        &CatalogQuery {
            page_size: Some(200),
            ..CatalogQuery::default()
        },
        &BTreeMap::new(),
    );
    let all = ammo::catalog(
        &model,
        &CatalogQuery {
            page_size: Some(200),
            page: 1,
            ..CatalogQuery::default()
        },
        &BTreeMap::new(),
    );
    let both: Vec<_> = page.entries.iter().chain(all.entries.iter()).collect();
    println!(
        "catalogue: {} ammo sets, {} calibers and families, {} ammo classes in use; pages of 200: {} and {} entries; {} generic sets; {} sets with similarTo; {} sets used by a converted gun; {} ammo types in all",
        page.total,
        page.calibers.len(),
        page.classes.len(),
        page.entries.len(),
        all.entries.len(),
        both.iter().filter(|e| e.generic).count(),
        both.iter().filter(|e| e.similar_to.is_some()).count(),
        both.iter().filter(|e| e.weapon_count > 0).count(),
        both.iter().map(|e| e.types.len()).sum::<usize>(),
    );
    assert!(
        page.total >= 300,
        "a Combat Extended install has 300 or more ammo sets"
    );

    let rebuilt = common_ammo::rebuild_all(&model, "RX");
    let mut root = Node::new("Defs");
    for r in &rebuilt {
        for d in &r.made.defs {
            root.push_child(d.clone());
        }
    }
    let generated_defs: usize = rebuilt.iter().map(|r| r.made.defs.len()).sum();
    println!(
        "rebuilt {} sets into {generated_defs} generated defs",
        rebuilt.len()
    );
    let with = build(Some(root));
    let report = common_ammo::compare_all(&model, "RX", &rebuilt, &with.databases);
    println!(
        "compared {} sets and {} types; unresolved {}; missing {}, different {}, extra {}",
        report.sets,
        report.types,
        report.unresolved,
        report.count("missing"),
        report.count("different"),
        report.count("extra")
    );
    for (k, n) in &report.diffs {
        println!(
            "  {n:5} {k}   e.g. {}",
            report.examples.get(k).cloned().unwrap_or_default()
        );
    }
    if let Ok(name) = std::env::var("RIMSTUDIO_AMMO_SHOW") {
        // print the real and the generated resolved def of one name pair, `real=generated`
        if let Some((real_name, gen_name)) = name.split_once('=') {
            for db in ["ThingDef", "RecipeDef"] {
                if let (Some(a), Some(b)) = (
                    with.databases.get(db, real_name),
                    with.databases.get(db, gen_name),
                ) {
                    println!("REAL {}", a.node.to_json_string());
                    println!("GEN  {}", b.node.to_json_string());
                }
            }
        }
    }

    let (mut lint_hits, mut dry_failed, mut errors) = (
        BTreeMap::<String, usize>::new(),
        0usize,
        BTreeMap::<String, usize>::new(),
    );
    let mut warnings = BTreeMap::<String, usize>::new();
    let mut error_detail = BTreeMap::<String, usize>::new();
    for r in &rebuilt {
        let mut file = Node::new("Defs");
        for d in &r.made.defs {
            file.push_child(d.clone());
        }
        for d in ammo::lint_defs(&[(None, file)], &model, &BTreeSet::new()) {
            *lint_hits.entry(d.code.as_str().to_owned()).or_insert(0) += 1;
        }
        let run = ammo::dry_load(&r.made.defs, &model);
        if !run.is_clean() {
            dry_failed += 1;
            if dry_failed <= 4 {
                println!(
                    "  dry load of {}: {:?}",
                    r.set,
                    run.diagnostics
                        .iter()
                        .map(|d| d.message.clone())
                        .collect::<Vec<_>>()
                );
            }
        }
        for d in ammo::validate_custom_ammo(&r.spec, "RX", &model) {
            if d.severity == rimstudio_core::diag::Severity::Error {
                let key = format!(
                    "{} {} {}",
                    d.code.as_str(),
                    d.args.get("what").cloned().unwrap_or_default(),
                    d.args.get("reason").cloned().unwrap_or_default()
                );
                *error_detail.entry(key).or_insert(0) += 1;
            }
            let bucket = if d.severity == rimstudio_core::diag::Severity::Error {
                &mut errors
            } else {
                &mut warnings
            };
            *bucket.entry(d.code.as_str().to_owned()).or_insert(0) += 1;
        }
    }
    println!("lint findings on the generated files: {lint_hits:?}");
    println!("dry loads that failed: {dry_failed} of {}", rebuilt.len());
    println!("validation errors on the real values: {errors:?}");
    println!("validation warnings on the real values: {warnings:?}");
    for (k, n) in &error_detail {
        println!("  {n:4} {k}");
    }
    println!("total {:?}", started.elapsed());
    assert_eq!(report.unresolved, 0);
    if std::env::var_os("RIMSTUDIO_AMMO_STRICT").is_some() {
        assert_eq!(report.count("missing"), 0);
    }
}
