//! Reference set resolution over a fictional install on disk, scanned by the library crate.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use camino::Utf8Path;
use common::{active, game, install, scan};
use rimstudio_workspace::refset::{DesignerReferenceOptions, PackKind, ReferenceSet};

#[test]
fn game_dlc_and_mods_come_in_load_order() {
    let inst = install();
    let index = scan(&inst);
    let set = ReferenceSet::resolve(
        &index,
        &active(&["ludeon.rimworld", "rs.gated", "rs.base"]),
        &game(),
    );
    let ids: Vec<&str> = set.packs.iter().map(|p| p.package_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "ludeon.rimworld",
            "ludeon.rimworld.royalty",
            "rs.gated",
            "rs.base"
        ]
    );
    assert_eq!(set.packs[0].kind, PackKind::Core);
    assert_eq!(set.packs[1].kind, PackKind::Dlc);
    assert!(set.diagnostics.is_empty(), "{:?}", set.diagnostics);
    let core = &set.packs[0];
    assert_eq!(core.defs.len(), 1);
    assert!(core.defs[0].path.ends_with("Data/Core/Defs/RS_Core.xml"));
}

#[test]
fn a_load_folders_entry_gated_on_a_mod_loads_only_when_that_mod_is_in_the_set() {
    let inst = install();
    let index = scan(&inst);
    let with = ReferenceSet::resolve(&index, &active(&["rs.base", "rs.gated"]), &game());
    let gated = with.find("rs.gated").unwrap();
    assert_eq!(gated.plan.relative_folders(), vec!["CE", ""]);
    let names: Vec<&str> = gated.defs.iter().map(|f| f.relative.as_str()).collect();
    assert!(names.contains(&"Defs/RS_GatedCE.xml"));
    assert!(names.contains(&"Defs/RS_Gated.xml"));

    let without = ReferenceSet::resolve(&index, &active(&["rs.gated"]), &game());
    let gated = without.find("rs.gated").unwrap();
    assert_eq!(gated.plan.relative_folders(), vec![""]);
    assert_eq!(gated.defs.len(), 1);
    assert_eq!(gated.plan.skipped, vec!["CE".to_owned()]);
}

#[test]
fn the_designer_reference_is_game_plus_expansions_without_mods_by_default() {
    let inst = install();
    let index = scan(&inst);
    let options = DesignerReferenceOptions::new(game());
    let set = ReferenceSet::reference_for_designer(&index, &inst.game_dir, &options);
    let ids: Vec<&str> = set.packs.iter().map(|p| p.package_id.as_str()).collect();
    assert_eq!(ids, vec!["ludeon.rimworld", "ludeon.rimworld.royalty"]);

    let with_mod = ReferenceSet::reference_for_designer(
        &index,
        &inst.game_dir,
        &options.clone().with_mod("rs.base"),
    );
    assert_eq!(with_mod.packs.last().unwrap().package_id, "rs.base");
    assert!(
        with_mod
            .packs
            .last()
            .unwrap()
            .root
            .starts_with(&inst.mods_dir)
    );
}

#[test]
fn a_project_folder_becomes_the_last_pack_and_replaces_a_listed_copy() {
    let inst = install();
    let index = scan(&inst);
    let set = ReferenceSet::resolve(&index, &active(&["rs.base", "rs.gated"]), &game());
    // the project is the same folder as a listed mod
    let base_root = set.find("rs.base").unwrap().root.clone();
    let with_project = set
        .with_project(&base_root, &rimstudio_workspace::listing::DiskListing)
        .unwrap();
    let ids: Vec<&str> = with_project
        .packs
        .iter()
        .map(|p| p.package_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "ludeon.rimworld",
            "ludeon.rimworld.royalty",
            "rs.gated",
            "rs.base"
        ]
    );
    assert_eq!(with_project.project().unwrap().kind, PackKind::Project);
    assert!(
        with_project
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "workspace.project-in-list")
    );
    // the gated mod still sees rs.base as active
    assert_eq!(
        with_project
            .find("rs.gated")
            .unwrap()
            .plan
            .relative_folders(),
        vec!["CE", ""]
    );
    // a folder without About.xml is refused
    let err = set
        .with_project(
            Utf8Path::new(inst.root.as_str()),
            &rimstudio_workspace::listing::DiskListing,
        )
        .unwrap_err();
    assert_eq!(err.code(), "workspace.not-a-project");
}
