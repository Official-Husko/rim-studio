//! IT-030: every REQ field of each kind, removed one at a time, is reported at its own field pointer and
//! blocks the plan.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_design::model::DesignSpec;
use rimstudio_design::plan::{ProjectLayout, export_vanilla_plan};
use rimstudio_design::validation::{
    diagnostic_field, has_errors, validate_ce_patch, validate_spec, validate_vanilla,
};

type Mutation = fn(&mut DesignSpec);

fn missing_at(diags: &[Diagnostic], pointer: &str) -> bool {
    diags.iter().any(|d| {
        d.code.as_str() == "design.required-missing"
            && d.severity == Severity::Error
            && diagnostic_field(d) == Some(pointer)
    })
}

fn ranged_mut(s: &mut DesignSpec) -> &mut rimstudio_design::model::RangedInputs {
    s.ranged.as_mut().unwrap()
}

#[test]
fn complete_specs_have_no_errors() {
    assert!(!has_errors(&validate_spec(&ranged_spec())));
    assert!(!has_errors(&validate_spec(&melee_spec())));
    let mut r = ranged_spec();
    r.ce = Some(ranged_ce());
    assert!(!has_errors(&validate_spec(&r)));
    let mut m = melee_spec();
    m.ce = Some(melee_ce());
    assert!(!has_errors(&validate_spec(&m)));
}

#[test]
fn each_required_ranged_field_is_enforced() {
    let cases: Vec<(&str, Mutation)> = vec![
        ("/identity/defName", |s| s.identity.def_name.clear()),
        ("/identity/label", |s| s.identity.label.clear()),
        ("/parent", |s| s.parent = None),
        ("/techLevel", |s| s.tech_level = None),
        ("/role", |s| s.role = None),
        ("/mass", |s| s.mass = None),
        ("/workToMake", |s| s.work_to_make = None),
        ("/costList", |s| s.cost_list.clear()),
        ("/ranged/damage", |s| ranged_mut(s).damage = None),
        ("/ranged/range", |s| ranged_mut(s).range = None),
        ("/ranged/warmup", |s| ranged_mut(s).warmup = None),
        ("/ranged/cooldown", |s| ranged_mut(s).cooldown = None),
        ("/ranged/accuracy/touch", |s| {
            ranged_mut(s).accuracy.touch = None
        }),
        ("/ranged/accuracy/short", |s| {
            ranged_mut(s).accuracy.short = None
        }),
        ("/ranged/accuracy/medium", |s| {
            ranged_mut(s).accuracy.medium = None
        }),
        ("/ranged/accuracy/long", |s| {
            ranged_mut(s).accuracy.long = None
        }),
        ("/ranged/projectile", |s| ranged_mut(s).projectile = None),
        ("/tools/0/label", |s| s.tools[0].label.clear()),
        ("/tools/0/capacities", |s| s.tools[0].capacities.clear()),
        ("/tools/0/power", |s| s.tools[0].power = None),
        ("/tools/0/cooldownTime", |s| s.tools[0].cooldown_time = None),
    ];
    for (pointer, mutate) in cases {
        let mut spec = ranged_spec();
        mutate(&mut spec);
        let diags = validate_vanilla(&spec);
        assert!(missing_at(&diags, pointer), "{pointer}: {diags:?}");
        let plan = export_vanilla_plan(&spec, &ProjectLayout::default());
        assert!(plan.files.is_empty(), "{pointer} must block the plan");
        assert!(plan.has_errors());
    }
}

#[test]
fn removing_the_ranged_block_reports_every_ranged_field() {
    let mut spec = ranged_spec();
    spec.ranged = None;
    let diags = validate_vanilla(&spec);
    for pointer in [
        "/ranged/damage",
        "/ranged/range",
        "/ranged/warmup",
        "/ranged/cooldown",
        "/ranged/accuracy/touch",
        "/ranged/projectile",
    ] {
        assert!(missing_at(&diags, pointer), "{pointer}");
    }
}

#[test]
fn each_required_melee_field_is_enforced() {
    let cases: Vec<(&str, Mutation)> = vec![
        ("/identity/defName", |s| s.identity.def_name.clear()),
        ("/parent", |s| s.parent = None),
        ("/techLevel", |s| s.tech_level = None),
        ("/role", |s| s.role = None),
        ("/mass", |s| s.mass = None),
        ("/workToMake", |s| s.work_to_make = None),
        ("/tools", |s| s.tools.clear()),
        ("/tools/1/label", |s| s.tools[1].label.clear()),
        ("/tools/1/capacities", |s| s.tools[1].capacities.clear()),
        ("/tools/2/power", |s| s.tools[2].power = None),
        ("/tools/0/cooldownTime", |s| s.tools[0].cooldown_time = None),
        ("/stuff/categories", |s| {
            s.stuff.as_mut().unwrap().categories.clear()
        }),
        ("/stuff/count", |s| s.stuff.as_mut().unwrap().count = None),
        ("/stuff", |s| s.stuff = None),
    ];
    for (pointer, mutate) in cases {
        let mut spec = melee_spec();
        mutate(&mut spec);
        let diags = validate_vanilla(&spec);
        assert!(missing_at(&diags, pointer), "{pointer}: {diags:?}");
        assert!(
            export_vanilla_plan(&spec, &ProjectLayout::default())
                .files
                .is_empty()
        );
    }
}

#[test]
fn a_melee_weapon_made_from_a_cost_list_needs_no_stuff() {
    let mut spec = melee_spec();
    spec.stuff = None;
    spec.cost_list = vec![rimstudio_design::model::CostEntry::new("RS_Metal", 20.0)];
    assert!(!has_errors(&validate_vanilla(&spec)));
}

#[test]
fn each_required_ce_field_of_a_gun_is_enforced() {
    type CeMutation = fn(&mut rimstudio_design::model::CePatchSpec);
    let cases: Vec<(&str, CeMutation)> = vec![
        ("/ce/ammoSet", |c| c.ammo_set = None),
        ("/ce/defaultProjectile", |c| c.default_projectile = None),
        ("/ce/magazineSize", |c| c.magazine_size = None),
        ("/ce/reloadTime", |c| c.reload_time = None),
        ("/ce/bulk", |c| c.bulk = None),
        ("/ce/swayFactor", |c| c.sway_factor = None),
        ("/ce/shotSpread", |c| c.shot_spread = None),
    ];
    for (pointer, mutate) in cases {
        let mut spec = ranged_spec();
        let mut ce = ranged_ce();
        mutate(&mut ce);
        spec.ce = Some(ce);
        let diags = validate_ce_patch(&spec);
        assert!(missing_at(&diags, pointer), "{pointer}: {diags:?}");
        // The vanilla definition is not held back by an incomplete patch.
        assert!(
            !export_vanilla_plan(&spec, &ProjectLayout::default())
                .files
                .is_empty()
        );
    }
}

#[test]
fn each_required_ce_field_of_a_melee_weapon_is_enforced() {
    type CeMutation = fn(&mut rimstudio_design::model::CePatchSpec);
    let cases: Vec<(&str, CeMutation)> = vec![
        ("/ce/bulk", |c| c.bulk = None),
        ("/ce/meleeCritChance", |c| c.melee_crit_chance = None),
        ("/ce/meleeParryChance", |c| c.melee_parry_chance = None),
        ("/ce/meleeDodgeChance", |c| c.melee_dodge_chance = None),
        ("/ce/toolPenetration/1/blunt", |c| {
            c.tool_penetration[1].blunt = None
        }),
        ("/ce/toolPenetration/1/sharp", |c| {
            c.tool_penetration[1].sharp = None
        }),
        ("/ce/toolPenetration/2/sharp", |c| {
            c.tool_penetration[2].sharp = None
        }),
        ("/ce/toolPenetration", |c| {
            c.tool_penetration.pop();
        }),
    ];
    for (pointer, mutate) in cases {
        let mut spec = melee_spec();
        let mut ce = melee_ce();
        mutate(&mut ce);
        spec.ce = Some(ce);
        let diags = validate_ce_patch(&spec);
        assert!(missing_at(&diags, pointer), "{pointer}: {diags:?}");
    }
}

#[test]
fn a_handle_tool_needs_no_sharp_penetration() {
    let mut spec = melee_spec();
    spec.ce = Some(melee_ce());
    assert!(
        spec.ce.as_ref().unwrap().tool_penetration[0]
            .sharp
            .is_none()
    );
    assert!(!has_errors(&validate_ce_patch(&spec)));
}

#[test]
fn without_the_ce_block_there_is_nothing_to_check() {
    assert!(validate_ce_patch(&ranged_spec()).is_empty());
}

#[test]
fn every_required_diagnostic_carries_a_field_pointer() {
    let mut spec = DesignSpec::new_ranged("", "");
    spec.ranged = None;
    for d in validate_spec(&spec) {
        let field = diagnostic_field(&d).unwrap_or_default();
        assert!(field.is_empty() || field.starts_with('/'), "{field}");
        assert!(diagnostic_field(&d).is_some());
    }
}
