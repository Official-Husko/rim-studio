//! The Combat Extended suggestions of a design through the public API: goldens for a fictional gun and melee
//! weapon, typed values that are never overwritten (IT-003), the plain reason without Combat Extended,
//! unreliable stats as asks, the toggle that suggestions never turn on, and determinism.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use proptest::prelude::*;
use rimstudio_design::ce::patchgen::{AskKind, export_ce_plan};
use rimstudio_design::ce::reader::CeModel;
use rimstudio_design::ce::suggest::{
    Accept, CeSuggestion, FieldStatus, SuggestedField, SuggestionSource, accept_suggestions,
    suggest_block,
};
use rimstudio_design::model::{DesignSpec, Sourced, ValueSource};
use rimstudio_design::plan::ProjectLayout;
use serde_json::Value;

use common::{melee_spec, ranged_spec};
use common_ce::{CLASS_TAG, ce_model, ce_model_without_pools};

/// Rounds every float of a JSON value to six decimals, so a golden does not depend on the last digits of a
/// logarithm.
fn rounded(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => serde_json::Number::from_f64((f * 1e6).round() / 1e6)
                .map_or(Value::Null, Value::Number),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(rounded).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, rounded(v))).collect()),
        other => other,
    }
}

fn golden_text(s: &CeSuggestion) -> String {
    let mut text =
        serde_json::to_string_pretty(&rounded(serde_json::to_value(s).unwrap())).unwrap();
    text.push('\n');
    text
}

fn field<'a>(s: &'a CeSuggestion, pointer: &str) -> &'a SuggestedField {
    s.fields
        .iter()
        .find(|f| f.field == pointer)
        .unwrap_or_else(|| panic!("no field {pointer}"))
}

fn with_toggle(mut spec: DesignSpec) -> DesignSpec {
    spec.ce = Some(rimstudio_design::model::CePatchSpec::default());
    spec
}

// ---------------------------------------------------------------------------------------------------------
// goldens

#[test]
fn the_ranged_suggestion_matches_its_golden() {
    let s = suggest_block(&ranged_spec(), &ce_model());
    common::assert_golden("ce_suggest_ranged.json", &golden_text(&s));
}

#[test]
fn the_melee_suggestion_matches_its_golden() {
    let s = suggest_block(&melee_spec(), &ce_model());
    common::assert_golden("ce_suggest_melee.json", &golden_text(&s));
}

// ---------------------------------------------------------------------------------------------------------
// what a suggestion says

#[test]
fn a_gun_gets_derived_numbers_with_source_band_and_rating() {
    let s = suggest_block(&ranged_spec(), &ce_model());
    assert!(s.available);
    assert!(!s.toggle_on);
    assert_eq!(s.pool, 12);
    let bulk = field(&s, "/ce/bulk");
    assert_eq!(bulk.status, FieldStatus::Derived);
    assert!(bulk.value.is_some());
    assert!(matches!(
        bulk.source,
        Some(SuggestionSource::Predicted { n, .. }) if n >= 3
    ));
    let band = bulk.band.unwrap();
    let v = bulk.value.unwrap();
    assert!(band.p50.low <= v && v <= band.p50.high);
    assert!(band.p80.low <= band.p50.low && band.p50.high <= band.p80.high);
    assert!(bulk.rating.is_some_and(|r| r.is_usable()));
    // the gun bash tool is a field too
    assert_eq!(
        field(&s, "/ce/toolPenetration/grip/blunt").status,
        FieldStatus::Derived
    );
}

#[test]
fn the_twin_number_shows_as_identity_in_the_patch_numbers() {
    let s = suggest_block(&ranged_spec(), &ce_model());
    let mass = s.patch_numbers.iter().find(|f| f.field == "mass").unwrap();
    assert_eq!(mass.value, Some(3.3));
    assert!(matches!(
        mass.source,
        Some(SuggestionSource::Identity { .. })
    ));
}

#[test]
fn the_ammo_set_and_the_tag_class_are_asked_and_never_chosen() {
    let s = suggest_block(&ranged_spec(), &ce_model());
    for pointer in ["/ce/ammoSet", "/ce/weaponTagClass"] {
        let c = s.choices.iter().find(|c| c.field == pointer).unwrap();
        assert_eq!(c.status, FieldStatus::Ask, "{pointer}");
        assert!(c.value.is_none());
        assert!(!c.candidates.is_empty());
        assert!(s.asks.asks(pointer));
    }
    assert!(s.missing.iter().any(|m| m == "/ce/ammoSet"));
    assert!(
        s.still_missing_after_accept
            .iter()
            .any(|m| m == "/ce/weaponTagClass")
    );
    let flag = s.choices.iter().find(|c| c.field == "/ce/beltFed").unwrap();
    assert_eq!(flag.kind, AskKind::Flag);
    assert!(!flag.required);
}

#[test]
fn a_melee_weapon_has_no_choices_and_asks_what_cannot_be_measured() {
    let s = suggest_block(&melee_spec(), &ce_model());
    assert!(s.choices.is_empty());
    // the fictional library cannot measure the dodge offset: it is an ask that carries the rejected estimate
    let dodge = field(&s, "/ce/meleeDodgeChance");
    assert_eq!(dodge.status, FieldStatus::Ask);
    assert!(dodge.value.is_none());
    assert!(dodge.reference.is_some());
    assert!(dodge.reason.as_deref().is_some_and(|r| r.contains("rated")));
    let ask = s
        .asks
        .items
        .iter()
        .find(|a| a.field == "/ce/meleeDodgeChance")
        .unwrap();
    assert_eq!(ask.kind, AskKind::Number);
    assert_eq!(ask.suggestion, dodge.reference);
    assert!(
        s.still_missing_after_accept
            .iter()
            .any(|m| m == "/ce/meleeDodgeChance")
    );
    assert!(
        field(&s, "/ce/toolPenetration/blade/sharp").required,
        "a melee cutting tool needs a sharp penetration"
    );
}

#[test]
fn without_conversions_every_number_is_an_ask_with_the_plain_reason() {
    let s = suggest_block(&ranged_spec(), &ce_model_without_pools());
    assert!(s.available);
    assert_eq!(s.pool, 0);
    let bulk = field(&s, "/ce/bulk");
    assert_eq!(bulk.status, FieldStatus::Ask);
    assert!(
        bulk.reason
            .as_deref()
            .is_some_and(|r| r.contains("no converted weapons"))
    );
}

#[test]
fn without_combat_extended_the_suggestion_is_the_plain_reason() {
    let model = CeModel::absent("Combat Extended is not in the load order");
    let s = suggest_block(&ranged_spec(), &model);
    assert!(!s.available);
    assert!(s.fields.is_empty() && s.choices.is_empty() && s.asks.is_empty());
    assert!(
        s.reason
            .as_deref()
            .is_some_and(|r| r.contains("not in the load order"))
    );
    // nothing can be accepted from it
    let out = accept_suggestions(&with_toggle(ranged_spec()), &s, &Accept::All);
    assert!(out.accepted.is_empty());
    assert_eq!(out.spec, with_toggle(ranged_spec()));
    assert!(!out.skipped.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// typed values (IT-003)

#[test]
fn typed_answered_and_anchor_values_are_held_and_never_overwritten() {
    let model = ce_model();
    let mut spec = with_toggle(ranged_spec());
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.bulk = Some(Sourced::typed(99.0));
        ce.reload_time = Some(Sourced::answered(7.0));
        ce.sway_factor = Some(Sourced::anchor(2.5));
        ce.shot_spread = Some(Sourced::suggested(0.5));
    }
    let s = suggest_block(&spec, &model);
    assert!(s.toggle_on);
    let bulk = field(&s, "/ce/bulk");
    assert_eq!(bulk.status, FieldStatus::Held);
    assert_eq!(bulk.held, Some(99.0));
    assert_eq!(bulk.held_source, Some(SuggestionSource::Typed));
    assert!(
        bulk.value.is_some(),
        "the estimate stays visible for comparison"
    );
    assert_eq!(
        field(&s, "/ce/reloadTime").held_source,
        Some(SuggestionSource::Answered)
    );
    assert_eq!(
        field(&s, "/ce/swayFactor").held_source,
        Some(SuggestionSource::Anchor)
    );
    // an earlier suggestion is not a decision
    assert_eq!(field(&s, "/ce/shotSpread").status, FieldStatus::Derived);

    let out = accept_suggestions(&spec, &s, &Accept::All);
    let ce = out.spec.ce.as_ref().unwrap();
    assert_eq!(ce.bulk, Some(Sourced::typed(99.0)));
    assert_eq!(ce.reload_time, Some(Sourced::answered(7.0)));
    assert_eq!(ce.sway_factor, Some(Sourced::anchor(2.5)));
    let spread = ce.shot_spread.unwrap();
    assert_eq!(spread.source, ValueSource::Suggested);
    assert!(
        (spread.value - 0.07).abs() < 1e-9,
        "the suggestion replaced the old suggestion"
    );
    assert!(!out.accepted.iter().any(|a| a.field == "/ce/bulk"));
}

#[test]
fn naming_a_held_field_reports_that_it_was_left_alone() {
    let mut spec = with_toggle(ranged_spec());
    spec.ce.as_mut().unwrap().bulk = Some(Sourced::typed(99.0));
    let s = suggest_block(&spec, &ce_model());
    let out = accept_suggestions(&spec, &s, &Accept::Fields(vec!["bulk".into()]));
    assert!(out.accepted.is_empty());
    assert_eq!(out.spec, spec);
    assert!(out.skipped.iter().any(|k| k.field == "/ce/bulk"));
}

#[test]
fn accepting_named_fields_fills_only_those() {
    let spec = with_toggle(ranged_spec());
    let s = suggest_block(&spec, &ce_model());
    let out = accept_suggestions(
        &spec,
        &s,
        &Accept::Fields(vec![
            "bulk".into(),
            "ce.reloadTime".into(),
            "/ce/toolPenetration/grip/blunt".into(),
            "nonsense".into(),
            "ammoSet".into(),
        ]),
    );
    let names: Vec<&str> = out.accepted.iter().map(|a| a.field.as_str()).collect();
    assert_eq!(
        names,
        [
            "/ce/bulk",
            "/ce/reloadTime",
            "/ce/toolPenetration/grip/blunt"
        ]
    );
    let ce = out.spec.ce.as_ref().unwrap();
    assert!(ce.bulk.is_some() && ce.reload_time.is_some());
    assert!(ce.sway_factor.is_none() && ce.shot_spread.is_none());
    assert!(ce.ammo_set.is_none(), "a choice is never made for the user");
    assert_eq!(ce.tool_penetration.len(), 1);
    assert!(out.skipped.iter().any(|k| k.field == "nonsense"));
    assert!(out.skipped.iter().any(|k| k.field == "/ce/ammoSet"));
}

#[test]
fn accepting_everything_fills_every_derived_number_and_marks_each_one() {
    let spec = with_toggle(ranged_spec());
    let s = suggest_block(&spec, &ce_model());
    let out = accept_suggestions(&spec, &s, &Accept::All);
    let ce = out.spec.ce.as_ref().unwrap();
    assert!(ce.bulk.is_some() && ce.sway_factor.is_some() && ce.shot_spread.is_some());
    assert!(ce.magazine_size.is_some() && ce.reload_time.is_some());
    assert!(
        ce.ammo_set.is_none() && ce.weapon_tag_class.is_none() && ce.default_projectile.is_none()
    );
    assert!(
        ce.bulk.is_some_and(|b| b.source == ValueSource::Suggested),
        "a filled number is marked as suggested"
    );
    assert_eq!(out.diagnostics.len(), out.accepted.len());
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.code.as_str() == "ce.derived-value")
    );
    // accepting twice changes nothing more
    let again = accept_suggestions(
        &out.spec,
        &suggest_block(&out.spec, &ce_model()),
        &Accept::All,
    );
    assert_eq!(again.spec, out.spec);
}

#[test]
fn the_default_projectile_follows_a_chosen_ammo_set_and_the_plan_then_has_no_error() {
    let model = ce_model();
    let mut spec = with_toggle(ranged_spec());
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.ammo_set = Some("RS_AmmoSet".into());
        ce.weapon_tag_class = Some(CLASS_TAG.into());
    }
    let s = suggest_block(&spec, &model);
    let projectile = s
        .choices
        .iter()
        .find(|c| c.field == "/ce/defaultProjectile")
        .unwrap();
    assert_eq!(projectile.status, FieldStatus::Derived);
    assert_eq!(projectile.value.as_deref(), Some("RS_Bullet_CE"));
    assert!(
        s.still_missing_after_accept.is_empty(),
        "{:?}",
        s.still_missing_after_accept
    );
    let out = accept_suggestions(&spec, &s, &Accept::All);
    assert_eq!(
        out.spec.ce.as_ref().unwrap().default_projectile.as_deref(),
        Some("RS_Bullet_CE")
    );
    let plan = export_ce_plan(&out.spec, &model, &ProjectLayout::default());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert!(!plan.files.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// the toggle

#[test]
fn a_spec_without_the_block_is_never_changed_and_never_yields_a_ce_file() {
    let model = ce_model();
    let spec = ranged_spec();
    assert!(spec.ce.is_none());
    let s = suggest_block(&spec, &model);
    assert!(s.available && !s.toggle_on);
    for which in [Accept::All, Accept::Fields(vec!["bulk".into()])] {
        let out = accept_suggestions(&spec, &s, &which);
        assert!(out.spec.ce.is_none(), "the toggle stays off");
        assert_eq!(out.spec, spec);
        assert!(out.accepted.is_empty() && out.diagnostics.is_empty());
        assert!(
            out.skipped.iter().any(|k| k.reason.contains("off")),
            "{:?}",
            out.skipped
        );
        let plan = export_ce_plan(&out.spec, &model, &ProjectLayout::default());
        assert!(
            plan.files.is_empty(),
            "no Combat Extended file with the toggle off"
        );
    }
    // suggesting does not mutate its input either
    assert_eq!(spec, ranged_spec());
}

// ---------------------------------------------------------------------------------------------------------
// candidates

#[test]
fn ammo_sets_are_ranked_by_the_tags_of_the_guns_that_use_them() {
    let mut model = ce_model();
    for gun in model.guns.iter_mut().take(3) {
        gun.ammo_set = Some("RS_AmmoSetOther".into());
        gun.twin_tags = vec!["RS_OtherGun".into()];
    }
    let mut spec = ranged_spec();
    let first = |s: &CeSuggestion| {
        s.choices
            .iter()
            .find(|c| c.field == "/ce/ammoSet")
            .unwrap()
            .candidates
            .first()
            .unwrap()
            .name
            .clone()
    };
    assert_eq!(first(&suggest_block(&spec, &model)), "RS_AmmoSet");
    spec.weapon_tags = vec!["RS_OtherGun".into()];
    let s = suggest_block(&spec, &model);
    assert_eq!(first(&s), "RS_AmmoSetOther");
    let c = &s.choices[0].candidates;
    assert_eq!(c[0].used_by, 3);
    assert_eq!(c[1].used_by, 9);
}

#[test]
fn candidates_are_capped() {
    let mut model = ce_model();
    model.ai_class_tags = (0..40).map(|i| format!("RS_Tag{i:02}")).collect();
    let s = suggest_block(&ranged_spec(), &model);
    let c = s
        .choices
        .iter()
        .find(|c| c.field == "/ce/weaponTagClass")
        .unwrap();
    assert_eq!(
        c.candidates.len(),
        rimstudio_design::ce::suggest::MAX_CANDIDATES
    );
}

// ---------------------------------------------------------------------------------------------------------
// determinism and robustness

#[test]
fn the_same_input_gives_the_same_json() {
    for spec in [ranged_spec(), melee_spec(), with_toggle(ranged_spec())] {
        let a = serde_json::to_string(&suggest_block(&spec, &ce_model())).unwrap();
        let b = serde_json::to_string(&suggest_block(&spec, &ce_model())).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn the_suggestion_round_trips_through_json() {
    let s = suggest_block(&with_toggle(ranged_spec()), &ce_model());
    let text = serde_json::to_string(&s).unwrap();
    let back: CeSuggestion = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s);
}

proptest! {
    #[test]
    fn suggesting_never_panics_and_accepting_never_touches_decided_values(
        mass in 0.0f64..50.0,
        range in 0.0f64..120.0,
        bulk in prop::option::of(0.0f64..40.0),
        magazine in prop::option::of(0u32..200),
    ) {
        let mut spec = with_toggle(ranged_spec());
        spec.mass = Some(Sourced::typed(mass));
        spec.ranged.as_mut().unwrap().range = Some(Sourced::typed(range));
        {
            let ce = spec.ce.as_mut().unwrap();
            ce.bulk = bulk.map(Sourced::typed);
            ce.magazine_size = magazine.map(Sourced::typed);
        }
        let model = ce_model();
        let s = suggest_block(&spec, &model);
        let out = accept_suggestions(&spec, &s, &Accept::All);
        let ce = out.spec.ce.as_ref().unwrap();
        if let Some(b) = bulk {
            prop_assert_eq!(ce.bulk, Some(Sourced::typed(b)));
        }
        if let Some(m) = magazine {
            prop_assert_eq!(ce.magazine_size, Some(Sourced::typed(m)));
        }
        let again = serde_json::to_string(&suggest_block(&spec, &model)).unwrap();
        prop_assert_eq!(serde_json::to_string(&s).unwrap(), again);
    }
}

#[test]
fn accepting_everything_does_not_list_open_asks_as_skipped_but_naming_one_does() {
    let spec = with_toggle(melee_spec());
    let s = suggest_block(&spec, &ce_model());
    // the fictional library cannot measure the dodge offset
    let all = accept_suggestions(&spec, &s, &Accept::All);
    assert!(all.skipped.is_empty(), "{:?}", all.skipped);
    assert!(all.spec.ce.as_ref().unwrap().melee_dodge_chance.is_none());
    let named = accept_suggestions(&spec, &s, &Accept::Fields(vec!["meleeDodgeChance".into()]));
    assert!(named.accepted.is_empty());
    assert!(
        named
            .skipped
            .iter()
            .any(|k| k.field == "/ce/meleeDodgeChance")
    );
}
