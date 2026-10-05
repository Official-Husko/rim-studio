//! Tests of the vanilla write plan: golden node trees, layout, gating and determinism.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_golden, melee_ce, melee_spec, ranged_ce, ranged_spec};
use rimstudio_core::diag::Severity;
use rimstudio_design::model::{ParentRef, ScalarField, ValueSource};
use rimstudio_design::plan::{
    FileAction, FileKind, ProjectLayout, WritePlan, export_vanilla_plan, vanilla,
};
use rimstudio_design::validation::codes;

fn pretty(plan: &WritePlan) -> String {
    let mut text = serde_json::to_string_pretty(plan).unwrap();
    text.push('\n');
    text
}

#[test]
fn golden_ranged_plan() {
    let plan = export_vanilla_plan(&ranged_spec(), &ProjectLayout::default());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert_golden("ranged_item.json", &pretty(&plan));
}

#[test]
fn golden_melee_plan() {
    let plan = export_vanilla_plan(&melee_spec(), &ProjectLayout::default());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert_golden("melee_item.json", &pretty(&plan));
}

#[test]
fn ranged_plan_has_a_weapon_file_and_a_projectile_file() {
    let plan = export_vanilla_plan(&ranged_spec(), &ProjectLayout::default());
    assert_eq!(
        plan.paths(),
        [
            "Defs/Projectiles/RS_Bullet_TestRifle.xml",
            "Defs/Weapons/RS_TestRifle.xml"
        ]
    );
    for f in &plan.files {
        assert_eq!(f.action, FileAction::Create);
        assert_eq!(f.kind, FileKind::VanillaDefs);
        assert_eq!(f.tree.as_ref().map(|t| t.tag.as_str()), Some("Defs"));
        assert_eq!(f.sections.len(), 1);
        assert!(f.rendered.is_none() && f.diff.is_none() && f.edits.is_empty());
    }
}

#[test]
fn reference_projectile_writes_no_projectile_file() {
    let mut spec = ranged_spec();
    if let Some(r) = spec.ranged.as_mut() {
        r.projectile = Some(rimstudio_design::model::ProjectileChoice::Reference(
            "RS_Bullet_Existing".into(),
        ));
    }
    let plan = export_vanilla_plan(&spec, &ProjectLayout::default());
    assert_eq!(plan.paths(), ["Defs/Weapons/RS_TestRifle.xml"]);
    let weapon = plan.files[0].tree.as_ref().unwrap();
    assert_eq!(
        weapon
            .find("ThingDef/verbs/li/defaultProjectile")
            .and_then(|n| n.leaf_text()),
        Some("RS_Bullet_Existing")
    );
}

#[test]
fn version_folder_layout_moves_every_file() {
    let plan = export_vanilla_plan(&ranged_spec(), &ProjectLayout::with_version_folder("1.6"));
    assert!(plan.paths().iter().all(|p| p.starts_with("1.6/Defs/")));
}

#[test]
fn a_bad_layout_blocks_the_plan() {
    let layout = ProjectLayout {
        weapons_dir: "../out".into(),
        ..ProjectLayout::default()
    };
    let plan = export_vanilla_plan(&ranged_spec(), &layout);
    assert!(plan.files.is_empty());
    assert!(plan.has_errors());
}

#[test]
fn no_vanilla_plan_contains_combat_extended_even_when_the_patch_is_on() {
    for (mut spec, ce) in [(ranged_spec(), ranged_ce()), (melee_spec(), melee_ce())] {
        let off = export_vanilla_plan(&spec, &ProjectLayout::default());
        spec.ce = Some(ce);
        let on = export_vanilla_plan(&spec, &ProjectLayout::default());
        for plan in [&off, &on] {
            assert!(!plan.files.is_empty());
            assert!(!plan.contains_text("CombatExtended"));
            let json = serde_json::to_string(plan).unwrap();
            assert!(!json.contains("CombatExtended"));
            assert!(!json.contains("ceteam"));
            assert!(plan.files.iter().all(|f| f.kind == FileKind::VanillaDefs));
            assert!(
                plan.files
                    .iter()
                    .all(|f| !f.path.contains("CE/") && f.path != "LoadFolders.xml")
            );
        }
        assert_eq!(
            off.files, on.files,
            "the CE block must not change the vanilla files"
        );
    }
}

#[test]
fn a_ce_class_typed_into_a_vanilla_field_is_an_error() {
    let mut spec = ranged_spec();
    if let Some(r) = spec.ranged.as_mut() {
        r.verb_class = Some("CombatExtended.Verb_ShootCE".into());
    }
    let plan = export_vanilla_plan(&spec, &ProjectLayout::default());
    assert!(plan.files.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.ce-in-vanilla")
    );
}

#[test]
fn errors_block_every_file_and_warnings_do_not() {
    let mut broken = ranged_spec();
    broken.mass = None;
    let plan = export_vanilla_plan(&broken, &ProjectLayout::default());
    assert!(plan.files.is_empty());
    assert!(plan.has_errors());

    let mut odd = ranged_spec();
    odd.offer(ScalarField::AccuracyLong, 70.0, ValueSource::Typed);
    odd.market_value = Some(rimstudio_design::model::Sourced::typed(300.0));
    let plan = export_vanilla_plan(&odd, &ProjectLayout::default());
    assert!(!plan.has_errors());
    assert_eq!(plan.files.len(), 2);
    let codes_seen: Vec<&str> = plan.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes_seen.contains(&"design.unit-mismatch"));
    assert!(codes_seen.contains(&"design.explicit-price"));
    assert!(
        plan.diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error)
    );
}

#[test]
fn the_plan_is_deterministic() {
    let a = pretty(&export_vanilla_plan(
        &ranged_spec(),
        &ProjectLayout::default(),
    ));
    let b = pretty(&export_vanilla_plan(
        &ranged_spec(),
        &ProjectLayout::default(),
    ));
    assert_eq!(a, b);
}

#[test]
fn inherited_stats_are_not_written() {
    let mut spec = ranged_spec();
    let mut parent = ParentRef::named("RS_BaseGun");
    parent.inherited_stats.insert("RS_Durability".into(), 100.0);
    spec.parent = Some(parent);
    let plan = export_vanilla_plan(&spec, &ProjectLayout::default());
    assert!(!plan.contains_text("RS_Durability"));
    let weapon = plan
        .file("Defs/Weapons/RS_TestRifle.xml")
        .and_then(|f| f.tree.as_ref())
        .unwrap();
    assert_eq!(
        weapon.find("ThingDef").and_then(|n| n.attr("ParentName")),
        Some("RS_BaseGun")
    );
}

#[test]
fn explicit_stats_that_collide_with_a_standard_stat_are_written_once() {
    let mut spec = ranged_spec();
    spec.extra_stats
        .insert("Mass".into(), rimstudio_design::model::Sourced::typed(9.0));
    spec.extra_stats.insert(
        "RS_ExtraStat".into(),
        rimstudio_design::model::Sourced::typed(2.5),
    );
    let node = vanilla::ranged_def(&spec);
    let stats = node.child("statBases").unwrap();
    assert_eq!(stats.children_named("Mass").count(), 1);
    assert_eq!(stats.child_text("Mass"), Some("3.3"));
    assert_eq!(stats.child_text("RS_ExtraStat"), Some("2.5"));
}

#[test]
fn melee_weapons_have_no_verbs_and_ranged_weapons_have_one() {
    assert!(vanilla::melee_def(&melee_spec()).child("verbs").is_none());
    let ranged = vanilla::ranged_def(&ranged_spec());
    assert_eq!(ranged.find_all("verbs/li").len(), 1);
    assert_eq!(
        ranged
            .find("verbs/li/verbClass")
            .and_then(|n| n.leaf_text()),
        Some("Verb_Shoot")
    );
    assert_eq!(
        ranged
            .find("verbs/li/hasStandardCommand")
            .and_then(|n| n.leaf_text()),
        Some("true")
    );
}

#[test]
fn section_groups_match_the_renderer_shape() {
    let plan = export_vanilla_plan(&melee_spec(), &ProjectLayout::default());
    let groups = plan.files[0].section_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].comment.as_deref(),
        Some("====== RS_TestBlade ======")
    );
    assert_eq!(groups[0].nodes.len(), 1);
}

#[test]
fn error_codes_in_a_plan_are_registered() {
    let mut broken = melee_spec();
    broken.tools.clear();
    let plan = export_vanilla_plan(&broken, &ProjectLayout::default());
    assert!(
        plan.diagnostics
            .iter()
            .all(|d| codes::lookup(d.code.as_str()).is_some())
    );
}
