//! The designer plan: vanilla by default, golden XML, no Combat Extended without the opt in block.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{assert_text_golden, fixture, melee, project, ranged, ranged_ce, request};
use rimstudio_ipc_types::designer::{FileActionDto, FileKindDto};
use rimstudio_toolkit::designer::export_plan;

#[test]
fn a_ranged_design_plans_the_vanilla_files_only_and_matches_the_golden() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert!(
        plan.files
            .iter()
            .all(|x| x.kind == FileKindDto::VanillaDefs)
    );
    assert!(plan.files.iter().all(|x| x.action == FileActionDto::Create));
    let weapon = plan
        .files
        .iter()
        .find(|x| x.path == "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml")
        .expect("weapon file");
    assert_text_golden("ranged_vanilla.xml", &weapon.rendered);
    assert!(!weapon.rendered.contains("CombatExtended"));
    assert_eq!(weapon.bytes as usize, weapon.rendered.len());
}

#[test]
fn a_melee_design_matches_the_golden() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &melee())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let weapon = plan
        .files
        .iter()
        .find(|x| x.path == "Defs/ThingDefs_Misc/Weapons/MeleeMedieval/RS_NewBlade.xml")
        .expect("weapon file");
    assert_text_golden("melee_vanilla.xml", &weapon.rendered);
}

#[test]
fn without_the_block_no_plan_holds_a_combat_extended_file_or_class() {
    let f = fixture(true);
    // a project that already names the gate must not make the vanilla plan change
    let p = project(
        &f,
        &[(
            "LoadFolders.xml",
            "<loadFolders><v1.6><li>/</li><li IfModActive=\"ceteam.combatextended\">CE</li></v1.6></loadFolders>",
        )],
    );
    for spec in [ranged(), melee()] {
        let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
        assert!(!plan.has_errors, "{:?}", plan.diagnostics);
        assert!(
            plan.files
                .iter()
                .all(|x| x.kind == FileKindDto::VanillaDefs)
        );
        assert!(
            plan.files
                .iter()
                .all(|x| !x.rendered.contains("CombatExtended"))
        );
        assert!(
            plan.files
                .iter()
                .all(|x| !x.path.starts_with("Compat/CombatExtended/"))
        );
    }
}

#[test]
fn the_block_without_combat_extended_data_keeps_the_plan_vanilla_and_says_why() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &ranged_ce())).unwrap();
    assert!(!plan.has_errors);
    assert!(
        plan.files
            .iter()
            .all(|x| x.kind == FileKindDto::VanillaDefs)
    );
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.ce-unavailable"),
        "{:?}",
        plan.diagnostics
    );
}

#[test]
fn the_plan_is_deterministic_and_export_writes_nothing() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let a = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    let b = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert_eq!(a, b);
    assert!(
        !p.root.join("Defs").exists(),
        "export must not write (IT-014)"
    );
}

#[test]
fn an_invalid_spec_gives_an_error_plan_without_files() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut spec = ranged();
    spec.identity.def_name = String::new();
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
}

#[test]
fn a_name_that_exists_in_the_reference_set_is_reported() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut spec = ranged();
    spec.identity.def_name = "RS_Gun00".into();
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "design.defname-conflict")
    );
    assert!(plan.has_errors);
}

#[test]
fn an_unknown_project_is_an_error() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut req = request(&p, &ranged());
    req.project_id = "p-00000000".into();
    assert_eq!(
        export_plan(&f.ctx, req).unwrap_err().code(),
        "project.not-open"
    );
}

#[test]
fn a_versioned_project_gets_its_files_inside_the_version_folder() {
    let f = fixture(true);
    let p = project(&f, &[("1.6/Defs/.keep", "")]);
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert!(
        plan.files.iter().all(|x| x.path.starts_with("1.6/Defs/")),
        "{:?}",
        plan.files.iter().map(|x| &x.path).collect::<Vec<_>>()
    );
}

#[test]
fn a_second_design_is_appended_to_nothing_and_a_repeat_is_unchanged_after_apply() {
    use rimstudio_core::jobs::{CancelToken, NoopProgress};
    use rimstudio_ipc_types::designer::DesignerApplyPlanRequest;
    use rimstudio_toolkit::designer::apply_plan;
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let report = apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id.clone(),
            request: req.clone(),
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.written.len(), plan.files.len());
    let again = export_plan(&f.ctx, req).unwrap();
    assert!(
        again
            .files
            .iter()
            .all(|x| x.action == FileActionDto::Unchanged),
        "{:?}",
        again
            .files
            .iter()
            .map(|x| (&x.path, x.action))
            .collect::<Vec<_>>()
    );
}
