//! The layout fix plan: golden plans of the owner's two weapon mods (as committed fixtures) and the
//! classification rules (what is safe, what needs review, conflicts, references).
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common_fix;

use common_fix::*;
use rimstudio_ipc_types::project_fix::{
    LayoutFixItemKindDto as Kind, LayoutFixRiskDto as Risk, ProjectLayoutFixPlanRequest,
};
use rimstudio_toolkit::project::fix::plan;

fn golden(name: &str, value: &impl serde::Serialize) {
    let mut text = serde_json::to_string_pretty(value).unwrap();
    text.push('\n');
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.json"));
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {} ({e}); run with RIMSTUDIO_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(
        text.replace("\r\n", "\n"),
        expected.replace("\r\n", "\n"),
        "golden mismatch for {name}"
    );
}

/// The project id depends on the temporary folder; the golden files do not.
fn anonymous(
    p: &rimstudio_ipc_types::project_fix::ProjectLayoutFixPlanDto,
) -> rimstudio_ipc_types::project_fix::ProjectLayoutFixPlanDto {
    let mut q = p.clone();
    q.project_id = "p-fixture".to_owned();
    q
}

#[test]
fn gewehr_41_plan_is_golden() {
    let w = world();
    let (_, id) = fixture(&w, "gewehr41");
    let p = plan_of(&w, &id);
    golden("fix_plan_gewehr41", &anonymous(&p));
    // what is proposed: two folders, a new LoadFolders.xml that gates Combat Extended, the patch moved
    let kinds: Vec<Kind> = p.items.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::CreateFolder,
            Kind::CreateFolder,
            Kind::CreateLoadFolders,
            Kind::MoveFile
        ]
    );
    let m = item(&p, "layout.ce-outside-gate");
    assert_eq!(m.from, "Patches/ce_patch.xml");
    assert_eq!(m.to, "Compat/CombatExtended/Patches/ce_patch.xml");
    assert_eq!(m.risk, Risk::Safe);
    assert_eq!(m.requires.len(), 1);
    assert!(p.needs_review == 0 && p.conflicts == 0);
}

#[test]
fn the_lone_wolf_plan_is_golden() {
    let w = world();
    let (_, id) = fixture(&w, "lonewolf");
    let p = plan_of(&w, &id);
    golden("fix_plan_lonewolf", &anonymous(&p));
    let m = item(&p, "layout.ce-outside-gate");
    assert_eq!(m.to, "Common/Compat/CombatExtended/Patches/CE_Patch.xml");
    let gate = p
        .items
        .iter()
        .find(|i| i.kind == Kind::CreateLoadFolders)
        .unwrap();
    let diff = gate.diff.as_deref().unwrap();
    // the new file keeps loading the root and Common, and gates only the Combat Extended folder
    assert!(
        diff.contains("+<li>/</li>") || diff.contains("<li>/</li>"),
        "{diff}"
    );
    assert!(diff.contains("Common/Compat/CombatExtended"));
    assert!(diff.contains("IfModActive=\"ceteam.combatextended\""));
}

#[test]
fn a_filter_keeps_the_items_of_the_codes_and_what_they_need() {
    let w = world();
    let (_, id) = fixture(&w, "gewehr41");
    let p = plan(
        &w.env,
        &ProjectLayoutFixPlanRequest {
            project_id: id,
            fixes: Some(vec!["layout.ce-outside-gate".to_owned()]),
        },
        &[],
    )
    .unwrap();
    let kinds: Vec<Kind> = p.items.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, [Kind::CreateLoadFolders, Kind::MoveFile]);
    // the plan id does not depend on the filter
    assert_eq!(p.plan_id, plan_of(&w, &p.project_id).plan_id);
}

#[test]
fn the_plan_is_deterministic_and_read_only() {
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let before = snapshot(&root);
    let a = plan_of(&w, &id);
    let b = plan_of(&w, &id);
    assert_eq!(a, b);
    assert_eq!(before, snapshot(&root));
}

#[test]
fn an_existing_destination_is_a_conflict_with_a_numbered_name() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_Conflict",
        &[
            ("Patches/ce.xml", CE_PATCH),
            ("Compat/CombatExtended/Patches/ce.xml", "<Patch/>"),
        ],
    );
    let p = plan_of(&w, &id);
    let m = item(&p, "layout.ce-outside-gate");
    let c = m.conflict.as_ref().unwrap();
    assert!(c.destination_exists);
    assert_eq!(
        c.suggested_to.as_deref(),
        Some("Compat/CombatExtended/Patches/ce_2.xml")
    );
    assert_eq!(p.conflicts, 1);
}

#[test]
fn a_patch_with_a_vanilla_operation_needs_review() {
    let w = world();
    let mixed = "<Patch><Operation Class=\"PatchOperationAdd\"><xpath>/Defs/ThingDef</xpath><value><x/></value></Operation>\
<Operation Class=\"PatchOperationAdd\"><xpath>/Defs/ThingDef</xpath><value><li Class=\"CombatExtended.ToolCE\"/></value></Operation></Patch>";
    let (_, id) = mod_at(&w, "RS_Mixed", &[("Patches/mixed.xml", mixed)]);
    let p = plan_of(&w, &id);
    let m = p
        .items
        .iter()
        .find(|i| i.from == "Patches/mixed.xml")
        .unwrap();
    assert_eq!(m.risk, Risk::NeedsReview);
    assert!(!m.applicable);
    assert!(
        m.review_reason
            .as_deref()
            .unwrap()
            .contains("do not use Combat Extended")
    );
    // nothing needs a gate when no patch moves
    assert!(p.items.iter().all(|i| i.kind != Kind::CreateLoadFolders));
}

#[test]
fn a_file_with_several_weapons_is_not_split_by_a_move() {
    let w = world();
    let two = format!(
        "<Defs>{}{}</Defs>",
        rifle("RS_A", "Things/A")
            .trim_start_matches("<Defs>")
            .trim_end_matches("</Defs>"),
        rifle("RS_B", "Things/B")
            .trim_start_matches("<Defs>")
            .trim_end_matches("</Defs>")
    );
    let (_, id) = mod_at(
        &w,
        "RS_Two",
        &[
            ("Defs/Guns.xml", two.as_str()),
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/Old.xml",
                &rifle("RS_C", "Things/C"),
            ),
        ],
    );
    let p = plan_of(&w, &id);
    let m = p.items.iter().find(|i| i.from == "Defs/Guns.xml").unwrap();
    assert_eq!(m.risk, Risk::NeedsReview);
    assert!(m.review_reason.as_deref().unwrap().contains("splitting"));
}

#[test]
fn a_single_weapon_file_in_the_wrong_category_moves() {
    let w = world();
    let sword = "<Defs><ThingDef ParentName=\"BaseMeleeWeapon\"><defName>RS_Blade</defName><techLevel>Medieval</techLevel><tools><li/></tools></ThingDef></Defs>";
    let (_, id) = mod_at(
        &w,
        "RS_Cat",
        &[(
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Blade.xml",
            sword,
        )],
    );
    let p = plan_of(&w, &id);
    let m = item(&p, "layout.def-wrong-category");
    assert_eq!(m.kind, Kind::MoveFile);
    assert_eq!(m.risk, Risk::Safe);
    assert!(
        m.to.starts_with("Defs/ThingDefs_Misc/Weapons/Melee"),
        "{}",
        m.to
    );
    assert!(m.to.ends_with("/RS_Blade.xml"));
}

#[test]
fn textures_and_sounds_are_listed_for_review_only() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_Case",
        &[
            ("textures/Things/A.png", "png"),
            (
                "Defs/ThingDefs_Misc/Weapons/Rifle.xml",
                &rifle("RS_R", "Things/A"),
            ),
            ("sounds/Weapons/shot.wav", "wav"),
        ],
    );
    let p = plan_of(&w, &id);
    let folder_items: Vec<_> = p
        .items
        .iter()
        .filter(|i| i.issue_code == "layout.folder-case")
        .collect();
    assert_eq!(folder_items.len(), 2, "{folder_items:?}");
    for i in folder_items {
        assert_eq!(i.risk, Risk::NeedsReview);
        assert!(!i.applicable);
        assert!(
            i.review_reason
                .as_deref()
                .unwrap()
                .contains("referenced by path")
        );
    }
}

#[test]
fn a_move_whose_old_path_other_files_mention_needs_review_with_the_references() {
    let w = world();
    // the patch names the definition file it was written for; moving the file would leave the comment stale
    let patch = "<Patch>\n<!-- written for Defs/Gun.xml -->\n<Operation Class=\"PatchOperationRemove\"><xpath>/Defs/ThingDef</xpath></Operation></Patch>";
    let (_, id) = mod_at(
        &w,
        "RS_Refs",
        &[
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/Old.xml",
                &rifle("RS_Old", "Things/Old"),
            ),
            ("Defs/Gun.xml", &rifle("RS_Gun", "Things/Gun")),
            ("Patches/notes.xml", patch),
        ],
    );
    let p = plan_of(&w, &id);
    let m = p.items.iter().find(|i| i.from == "Defs/Gun.xml").unwrap();
    assert_eq!(m.risk, Risk::NeedsReview);
    assert_eq!(m.references.len(), 1);
    assert_eq!(m.references[0].path, "Patches/notes.xml");
    assert_eq!(m.references[0].line, 2);
}

#[test]
fn a_legacy_combat_extended_folder_is_renamed_with_its_entry() {
    let w = world();
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n  </v1.6>\n</loadFolders>\n";
    let (_, id) = mod_at(
        &w,
        "RS_Legacy",
        &[("LoadFolders.xml", lf), ("CE/Patches/a.xml", CE_PATCH)],
    );
    let p = plan_of(&w, &id);
    let rename = p.items.iter().find(|i| i.kind == Kind::MoveFolder).unwrap();
    assert_eq!(
        (rename.from.as_str(), rename.to.as_str()),
        ("CE", "Compat/CombatExtended")
    );
    let edit = p
        .items
        .iter()
        .find(|i| i.kind == Kind::EditLoadFolders)
        .unwrap();
    let diff = edit.diff.as_deref().unwrap();
    assert!(
        diff.contains("-    <li IfModActive=\"ceteam.combatextended\">CE</li>"),
        "{diff}"
    );
    assert!(
        diff.contains("+    <li IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>"),
        "{diff}"
    );
    assert_eq!(rename.requires, vec![edit.id.clone()]);
    assert_eq!(edit.requires, vec![rename.id.clone()]);
}

#[test]
fn an_entry_for_the_folder_without_a_condition_is_gated_in_place() {
    let w = world();
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li>Compat/CombatExtended</li>\n  </v1.6>\n</loadFolders>\n";
    let (_, id) = mod_at(
        &w,
        "RS_Ungated",
        &[
            ("LoadFolders.xml", lf),
            ("Compat/CombatExtended/Patches/a.xml", CE_PATCH),
        ],
    );
    let p = plan_of(&w, &id);
    let gate = p
        .items
        .iter()
        .find(|i| i.kind == Kind::EditLoadFolders)
        .unwrap();
    let diff = gate.diff.as_deref().unwrap();
    assert!(
        diff.contains("-    <li>Compat/CombatExtended</li>"),
        "{diff}"
    );
    assert!(
        diff.contains("+    <li IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>"),
        "{diff}"
    );
    // listed once, not twice
    assert_eq!(diff.matches("Compat/CombatExtended").count(), 2, "{diff}");
}
