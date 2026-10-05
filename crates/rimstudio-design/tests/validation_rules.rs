//! Tests of the pure validation rules: unit sanity, structural errors, names, the duplicate capacity
//! warning, vanilla purity and the reference seam.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_design::model::{
    CostEntry, DesignSpec, ProjectileChoice, ScalarField, Sourced, ToolSpec, ValueSource,
};
use rimstudio_design::validation::{
    DefLookup, RefKind, diagnostic_field, has_errors, validate_ce_patch, validate_draft,
    validate_refs, validate_spec, validate_vanilla,
};
use rstest::rstest;

fn find<'a>(diags: &'a [Diagnostic], code: &str, field: &str) -> Option<&'a Diagnostic> {
    diags
        .iter()
        .find(|d| d.code.as_str() == code && diagnostic_field(d) == Some(field))
}

#[rstest]
#[case(ScalarField::AccuracyMedium, 70.0, "/ranged/accuracy/medium")]
#[case(ScalarField::ArmorPenetration, 5.0, "/ranged/armorPenetration")]
#[case(ScalarField::Range, 300.0, "/ranged/range")]
#[case(ScalarField::Warmup, 90.0, "/ranged/warmup")]
#[case(ScalarField::Cooldown, 120.0, "/ranged/cooldown")]
#[case(ScalarField::Mass, 500.0, "/mass")]
#[case(ScalarField::ToolCooldown(0), 240.0, "/tools/0/cooldownTime")]
#[case(ScalarField::ToolArmorPenetration(0), 3.0, "/tools/0/armorPenetration")]
fn unit_flags_warn_and_never_error_or_clamp(
    #[case] field: ScalarField,
    #[case] value: f64,
    #[case] pointer: &str,
) {
    let mut spec = ranged_spec();
    assert!(spec.offer(field, value, ValueSource::Typed).applied());
    let before = spec.clone();
    let diags = validate_vanilla(&spec);
    let d = find(&diags, "design.unit-mismatch", pointer).unwrap_or_else(|| panic!("{diags:?}"));
    assert_eq!(d.severity, Severity::Warning);
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(spec, before, "validation must not touch the spec");
    assert_eq!(spec.scalar(field), Some(Sourced::typed(value)));
}

#[test]
fn plausible_values_raise_no_unit_flag() {
    assert!(
        validate_vanilla(&ranged_spec())
            .iter()
            .all(|d| d.code.as_str() != "design.unit-mismatch")
    );
    assert!(
        validate_vanilla(&melee_spec())
            .iter()
            .all(|d| d.code.as_str() != "design.unit-mismatch")
    );
}

#[test]
fn a_long_ce_reload_time_is_flagged_as_ticks() {
    let mut spec = ranged_spec();
    let mut ce = ranged_ce();
    ce.reload_time = Some(Sourced::typed(240.0));
    spec.ce = Some(ce);
    let diags = validate_ce_patch(&spec);
    assert!(find(&diags, "design.unit-mismatch", "/ce/reloadTime").is_some());
    assert!(!has_errors(&diags));
}

#[rstest]
#[case(ScalarField::Damage, f64::NAN)]
#[case(ScalarField::Mass, f64::INFINITY)]
#[case(ScalarField::Cooldown, -1.0)]
#[case(ScalarField::Range, -0.5)]
#[case(ScalarField::AccuracyTouch, -0.1)]
#[case(ScalarField::WorkToMake, -5.0)]
fn structurally_impossible_numbers_are_errors(#[case] field: ScalarField, #[case] value: f64) {
    let mut spec = ranged_spec();
    // Programmatic edits bypass `offer`, which refuses non finite numbers.
    let r = spec.ranged.as_mut().unwrap();
    match field {
        ScalarField::Damage => r.damage = Some(Sourced::typed(value)),
        ScalarField::Mass => spec.mass = Some(Sourced::typed(value)),
        ScalarField::Cooldown => r.cooldown = Some(Sourced::typed(value)),
        ScalarField::Range => r.range = Some(Sourced::typed(value)),
        ScalarField::AccuracyTouch => r.accuracy.touch = Some(Sourced::typed(value)),
        _ => spec.work_to_make = Some(Sourced::typed(value)),
    }
    let diags = validate_vanilla(&spec);
    let d = find(&diags, "design.value-invalid", &field.pointer())
        .unwrap_or_else(|| panic!("{diags:?}"));
    assert_eq!(d.severity, Severity::Error);
}

#[test]
fn counts_and_whole_numbers_are_checked() {
    let mut spec = ranged_spec();
    spec.ranged.as_mut().unwrap().burst_count = Some(Sourced::typed(0));
    spec.ranged.as_mut().unwrap().ticks_between_burst_shots = Some(Sourced::typed(7.5));
    spec.cost_list = vec![
        CostEntry::new("RS_Metal", 2.5),
        CostEntry::new("RS_Part", 0.0),
    ];
    let diags = validate_vanilla(&spec);
    assert!(find(&diags, "design.value-invalid", "/ranged/burstCount").is_some());
    assert!(
        find(
            &diags,
            "design.value-invalid",
            "/ranged/ticksBetweenBurstShots"
        )
        .is_some()
    );
    assert!(find(&diags, "design.value-invalid", "/costList/0/count").is_some());
    assert!(find(&diags, "design.value-invalid", "/costList/1/count").is_some());
}

#[test]
fn a_fractional_damage_is_an_error_only_when_the_projectile_is_created() {
    let mut spec = ranged_spec();
    spec.offer(ScalarField::Damage, 11.5, ValueSource::Typed);
    assert!(
        find(
            &validate_vanilla(&spec),
            "design.value-invalid",
            "/ranged/damage"
        )
        .is_some()
    );
    spec.ranged.as_mut().unwrap().projectile =
        Some(ProjectileChoice::Reference("RS_Bullet".into()));
    assert!(
        find(
            &validate_vanilla(&spec),
            "design.value-invalid",
            "/ranged/damage"
        )
        .is_none()
    );
}

#[test]
fn ce_offsets_may_be_negative_but_other_ce_numbers_may_not() {
    let mut spec = melee_spec();
    let mut ce = melee_ce();
    ce.bulk = Some(Sourced::typed(-1.0));
    spec.ce = Some(ce);
    let diags = validate_ce_patch(&spec);
    assert!(find(&diags, "design.value-invalid", "/ce/bulk").is_some());
    assert!(find(&diags, "design.value-invalid", "/ce/meleeDodgeChance").is_none());
}

#[rstest]
#[case("RS_Test Rifle")]
#[case("RS_Test<Rifle")]
#[case("RS.Test")]
#[case("null")]
#[case("UnnamedDef")]
fn bad_def_names_are_errors(#[case] name: &str) {
    let mut spec = ranged_spec();
    spec.identity.def_name = name.into();
    let diags = validate_vanilla(&spec);
    assert!(
        find(&diags, "design.name-invalid", "/identity/defName").is_some(),
        "{name}"
    );
}

#[test]
fn the_prefix_rule_is_a_warning() {
    let mut spec = ranged_spec();
    spec.identity.def_name = "Other_Rifle".into();
    let diags = validate_vanilla(&spec);
    let d = find(&diags, "design.defname-prefix", "/identity/defName").unwrap();
    assert_eq!(d.severity, Severity::Warning);
    assert!(!has_errors(&diags));
    spec.identity.mod_prefix.clear();
    assert!(
        find(
            &validate_vanilla(&spec),
            "design.defname-prefix",
            "/identity/defName"
        )
        .is_none()
    );
}

#[test]
fn ingredient_names_must_be_usable_as_element_names() {
    let mut spec = ranged_spec();
    spec.cost_list = vec![CostEntry::new("1Metal", 2.0)];
    assert!(
        find(
            &validate_vanilla(&spec),
            "design.name-invalid",
            "/costList/0/defName"
        )
        .is_some()
    );
}

#[test]
fn a_def_cannot_be_its_own_parent_or_share_a_name_with_its_projectile() {
    let mut spec = ranged_spec();
    spec.parent = Some(rimstudio_design::model::ParentRef::named("RS_TestRifle"));
    if let Some(ProjectileChoice::Inline(p)) = spec.ranged.as_mut().unwrap().projectile.as_mut() {
        p.def_name = "RS_TestRifle".into();
    }
    let diags = validate_vanilla(&spec);
    assert!(find(&diags, "design.value-invalid", "/parent/defName").is_some());
    assert!(
        find(
            &diags,
            "design.defname-conflict",
            "/ranged/projectile/def/defName"
        )
        .is_some()
    );
}

#[test]
fn control_characters_in_text_are_errors() {
    let mut spec = ranged_spec();
    spec.identity.description = "bad\u{1}text".into();
    let diags = validate_vanilla(&spec);
    assert!(find(&diags, "design.text-invalid", "/identity/description").is_some());
}

#[test]
fn a_tool_with_two_capacities_gets_the_duplicate_capacity_warning() {
    let mut spec = melee_spec();
    spec.tools[1] =
        ToolSpec::new("blade", &["Cut", "Stab"]).with_numbers(18.0, 2.4, ValueSource::Typed);
    let diags = validate_vanilla(&spec);
    let d = find(&diags, "design.duplicate-capacity", "/tools/1/capacities").unwrap();
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.args.get("tool").map(String::as_str), Some("blade"));
    assert_eq!(d.args.get("count").map(String::as_str), Some("2"));
    assert!(!has_errors(&diags));
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.code.as_str() == "design.duplicate-capacity")
            .count(),
        1
    );
}

#[test]
fn an_explicit_price_is_info_and_duplicates_are_warnings() {
    let mut spec = ranged_spec();
    spec.market_value = Some(Sourced::typed(250.0));
    spec.weapon_tags = vec!["RS_Gun".into(), "RS_Gun".into()];
    spec.cost_list.push(CostEntry::new("RS_Metal", 1.0));
    let diags = validate_vanilla(&spec);
    assert_eq!(
        find(&diags, "design.explicit-price", "/marketValue")
            .unwrap()
            .severity,
        Severity::Info
    );
    assert!(find(&diags, "design.duplicate-entry", "/weaponTags/1").is_some());
    assert!(find(&diags, "design.duplicate-entry", "/costList/2").is_some());
    assert!(!has_errors(&diags));
}

#[test]
fn ranged_inputs_on_a_melee_item_are_ignored_with_an_info() {
    let mut spec = melee_spec();
    spec.ranged = Some(rimstudio_design::model::RangedInputs::default());
    let d = find(&validate_vanilla(&spec), "design.ignored-input", "/ranged")
        .unwrap()
        .clone();
    assert_eq!(d.severity, Severity::Info);
}

#[test]
fn ce_classes_are_refused_in_vanilla_fields_but_the_ce_block_may_name_ce_things() {
    let mut spec = melee_spec();
    spec.weapon_tags = vec!["CombatExtended_Tag".into()];
    spec.ce = Some(melee_ce());
    let vanilla = validate_vanilla(&spec);
    assert!(find(&vanilla, "design.ce-in-vanilla", "/weaponTags/0").is_some());
    let mut ok = melee_spec();
    let mut ce = melee_ce();
    ce.weapon_tag_class = Some("CombatExtended_Class".into());
    ok.ce = Some(ce);
    assert!(
        validate_vanilla(&ok)
            .iter()
            .all(|d| d.code.as_str() != "design.ce-in-vanilla")
    );
}

#[test]
fn a_vanilla_description_that_mentions_the_mod_by_words_is_fine() {
    let mut spec = ranged_spec();
    spec.identity.description = "Works with Combat Extended after patching.".into();
    assert!(!has_errors(&validate_vanilla(&spec)));
}

#[test]
fn draft_validation_flags_a_kind_mismatch() {
    let mut draft = rimstudio_design::model::Draft::new(ranged_spec());
    assert!(!has_errors(&validate_draft(&draft)));
    draft.kind = rimstudio_design::model::ItemKind::Melee;
    let diags = validate_draft(&draft);
    assert!(find(&diags, "design.draft-inconsistent", "/kind").is_some());
}

#[test]
fn only_errors_block() {
    let mut spec = ranged_spec();
    spec.offer(ScalarField::AccuracyLong, 60.0, ValueSource::Typed);
    spec.identity.def_name = "Other_Rifle".into();
    spec.market_value = Some(Sourced::typed(10.0));
    let diags = validate_spec(&spec);
    assert!(!diags.is_empty());
    assert!(!has_errors(&diags));
}

struct Fake(BTreeSet<(RefKind, &'static str)>, bool);

impl DefLookup for Fake {
    fn contains(&self, kind: RefKind, name: &str) -> Option<bool> {
        if !self.1 {
            return None;
        }
        Some(self.0.iter().any(|(k, n)| *k == kind && *n == name))
    }
}

#[test]
fn references_are_checked_against_the_lookup() {
    let known: BTreeSet<_> = [
        (RefKind::Thing, "RS_BaseGun"),
        (RefKind::Thing, "RS_Metal"),
        (RefKind::Thing, "RS_TestRifle"),
        (RefKind::ResearchProject, "RS_Research"),
    ]
    .into_iter()
    .collect();
    let diags = validate_refs(&ranged_spec(), &Fake(known, true));
    assert!(find(&diags, "design.defname-conflict", "/identity/defName").is_some());
    assert!(find(&diags, "design.ref-unresolved", "/costList/1/defName").is_some());
    assert!(find(&diags, "design.ref-unresolved", "/costList/0/defName").is_none());
    assert!(find(&diags, "design.ref-unresolved", "/researchPrerequisite").is_none());
    assert!(find(&diags, "design.ref-unresolved", "/tools/0/capacities/0").is_some());
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
}

#[test]
fn unknown_answers_produce_no_diagnostics() {
    assert!(validate_refs(&ranged_spec(), &Fake(BTreeSet::new(), false)).is_empty());
}

#[test]
fn validation_is_deterministic_and_leaves_the_spec_alone() {
    let mut spec = DesignSpec::new_melee("RS_Odd", "odd");
    spec.tools.push(ToolSpec::new("a", &["Cut", "Stab"]));
    let before = spec.clone();
    let a = validate_spec(&spec);
    let b = validate_spec(&spec);
    assert_eq!(a, b);
    assert_eq!(spec, before);
}
