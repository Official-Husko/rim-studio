//! Where the files of a plan go for each convention of the project (RimStudio mod layout v1): category
//! folders, the game's own category files with an appended section, a flat folder, version folders and the
//! gated Combat Extended folder in its standard and in its older name.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{fixture, project, put, ranged, ranged_ce, request};
use rimstudio_design::plan::FileAction;
use rimstudio_ipc_types::designer::{FileActionDto, FileKindDto};
use rimstudio_toolkit::designer::export_plan;
use rimstudio_toolkit::designer::plan::prepare;

const NEW_PATH: &str = "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml";

const CORE_STYLE_FILE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Defs>\n\n  <!-- ================================= Industrial ==================================== -->\n\n  <ThingDef ParentName=\"BaseBullet\">\n    <defName>RS_OldBullet</defName>\n    <label>old bullet</label>\n  </ThingDef>\n  <ThingDef ParentName=\"BaseGun\">\n    <defName>RS_OldGun</defName>\n    <label>old gun</label>\n  </ThingDef>\n\n</Defs>\n";

#[test]
fn a_new_project_gets_one_file_with_the_projectile_and_the_weapon_in_a_category_folder() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(paths, [NEW_PATH]);
    let file = &plan.files[0];
    assert_eq!(file.kind, FileKindDto::VanillaDefs);
    assert_eq!(file.action, FileActionDto::Create);
    let projectile = file
        .rendered
        .find("<defName>RS_NewBullet</defName>")
        .unwrap();
    let weapon = file
        .rendered
        .find("<defName>RS_NewRifle</defName>")
        .unwrap();
    assert!(projectile < weapon, "the projectile comes first");
    assert!(
        file.rendered
            .contains("<!-- ====== RS_NewBullet ====== -->")
    );
    assert!(file.rendered.contains("<!-- ====== RS_NewRifle ====== -->"));
    // paths are relative to the project root
    assert!(
        plan.files
            .iter()
            .all(|x| !x.path.starts_with('/') && !x.path.contains(p.root.as_str()))
    );
    // the reserved texture is named and written, no image is made
    assert!(
        file.rendered
            .contains("Things/Item/Equipment/WeaponRanged/RS_NewRifle")
    );
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "design.texture-reserved")
    );
    assert!(!p.root.join("Textures").exists());
}

#[test]
fn a_project_in_the_games_own_style_gets_a_marked_section_appended_to_the_category_file() {
    let f = fixture(false);
    let rel = "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml";
    let p = project(&f, &[(rel, CORE_STYLE_FILE)]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    assert!(
        !prepared.plan.has_errors(),
        "{:?}",
        prepared.plan.diagnostics
    );
    assert_eq!(prepared.plan.paths(), [rel], "no new file is created");
    let file = prepared.plan.file(rel).unwrap();
    assert_eq!(file.action, FileAction::UpdateRegion);
    let old = file.previous.as_deref().unwrap();
    assert_eq!(old, CORE_STYLE_FILE);
    // everything that was there is still there, in place; the new sections are added after it
    let keep = &old[..old.rfind("</ThingDef>").unwrap() + "</ThingDef>".len()];
    assert!(
        file.text.starts_with(keep),
        "the existing text is unchanged"
    );
    assert!(file.text.contains("<!-- ====== RS_NewBullet ====== -->"));
    assert!(file.text.contains("<!-- ====== RS_NewRifle ====== -->"));
    assert!(file.text.trim_end().ends_with("</Defs>"));
    assert_eq!(file.edits.len(), 1);
}

#[test]
fn a_missing_category_file_is_created_next_to_the_existing_ones() {
    let f = fixture(false);
    let p = project(
        &f,
        &[("Defs/ThingDefs_Misc/Weapons/RangedSpacer.xml", "<Defs/>")],
    );
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(paths, ["Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml"]);
    assert_eq!(plan.files[0].action, FileActionDto::Create);
}

#[test]
fn a_flat_project_keeps_its_folder_and_gets_one_file_per_weapon() {
    let f = fixture(false);
    let p = project(
        &f,
        &[(
            "Common/Defs/ThingsDef_Misc/Weapons/Weapons_Ranged.xml",
            "<Defs/>",
        )],
    );
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(
        paths,
        ["Common/Defs/ThingsDef_Misc/Weapons/RS_NewRifle.xml"]
    );
    // an existing file of the project is never touched by a plan
    assert!(plan.files.iter().all(|x| x.action == FileActionDto::Create));
}

#[test]
fn a_version_folder_holds_the_files_and_load_folders_stays_at_the_root() {
    let f = fixture(true);
    let p = project(
        &f,
        &[(
            "1.6/Defs/ThingDefs_Misc/Weapons/RangedSpacer/Other.xml",
            "<Defs/>",
        )],
    );
    let plan = export_plan(&f.ctx, request(&p, &ranged_ce())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "1.6/Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml",
            "1.6/Compat/CombatExtended/Patches/testmod_Weapons_Ranged.xml",
            "LoadFolders.xml"
        ]
    );
    let lf = plan
        .files
        .iter()
        .find(|x| x.path == "LoadFolders.xml")
        .unwrap();
    assert!(lf.rendered.contains("<li>1.6</li>"));
    assert!(
        lf.rendered
            .contains("IfModActive=\"ceteam.combatextended\">1.6/Compat/CombatExtended</li>")
    );
}

#[test]
fn the_ce_patch_goes_to_the_standard_folder_and_is_gated_in_load_folders() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &ranged_ce())).unwrap();
    let patch = plan
        .files
        .iter()
        .find(|x| x.kind == FileKindDto::CePatch)
        .unwrap();
    assert_eq!(
        patch.path,
        "Compat/CombatExtended/Patches/testmod_Weapons_Ranged.xml"
    );
    let lf = plan
        .files
        .iter()
        .find(|x| x.path == "LoadFolders.xml")
        .unwrap();
    assert!(
        lf.rendered
            .contains("IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>")
    );
    // the vanilla file holds no Combat Extended class and is not inside the gated folder
    let vanilla = plan.files.iter().find(|x| x.path == NEW_PATH).unwrap();
    assert!(!vanilla.rendered.contains("CombatExtended"));
}

#[test]
fn an_older_ce_folder_is_detected_used_and_never_moved() {
    let f = fixture(true);
    let gate = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n  </v1.6>\n</loadFolders>\n";
    let p = project(
        &f,
        &[
            ("CE/Patches/RS_Existing.xml", "<Patch/>"),
            ("LoadFolders.xml", gate),
        ],
    );
    let plan = export_plan(&f.ctx, request(&p, &ranged_ce())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert!(
        paths.contains(&"CE/Patches/testmod_Weapons_Ranged.xml"),
        "{paths:?}"
    );
    assert!(paths.iter().all(|x| !x.contains("Compat")), "{paths:?}");
    let lf = plan
        .files
        .iter()
        .find(|x| x.path == "LoadFolders.xml")
        .unwrap();
    assert_eq!(
        lf.action,
        FileActionDto::Unchanged,
        "the gate is already there"
    );
    assert!(p.root.join("CE/Patches/RS_Existing.xml").exists());
    assert!(!p.root.join("Compat").exists());
}

#[test]
fn a_ce_folder_without_a_gate_is_found_by_its_name() {
    let f = fixture(true);
    let p = project(&f, &[("CE/Patches/RS_Existing.xml", "<Patch/>")]);
    let plan = export_plan(&f.ctx, request(&p, &ranged_ce())).unwrap();
    let paths: Vec<&str> = plan.files.iter().map(|x| x.path.as_str()).collect();
    assert!(
        paths.contains(&"CE/Patches/testmod_Weapons_Ranged.xml"),
        "{paths:?}"
    );
    let lf = plan
        .files
        .iter()
        .find(|x| x.path == "LoadFolders.xml")
        .unwrap();
    assert!(
        lf.rendered
            .contains("IfModActive=\"ceteam.combatextended\">CE</li>")
    );
}

#[test]
fn the_vanilla_plan_never_names_the_ce_folder_even_in_a_project_that_has_one() {
    let f = fixture(true);
    let p = project(&f, &[("Compat/CombatExtended/Patches/x.xml", "<Patch/>")]);
    put(
        &p.root,
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/Other.xml",
        "<Defs/>",
    );
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert!(
        plan.files
            .iter()
            .all(|x| x.kind == FileKindDto::VanillaDefs)
    );
    assert!(plan.files.iter().all(|x| !x.path.contains("Compat")));
}
