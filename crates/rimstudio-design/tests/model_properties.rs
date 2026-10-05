//! Property tests of the model: typed values survive any sequence of suggestions, answers and anchors
//! (IT-003), the CE block is never created by an offer, and drafts round trip through JSON.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use proptest::prelude::*;
use rimstudio_design::model::{
    Anchor, CalibrationMode, DesignSpec, Draft, ScalarField, Sourced, ValueSource,
};

fn fields() -> Vec<ScalarField> {
    vec![
        ScalarField::Mass,
        ScalarField::WorkToMake,
        ScalarField::MarketValue,
        ScalarField::Damage,
        ScalarField::ArmorPenetration,
        ScalarField::Range,
        ScalarField::BurstCount,
        ScalarField::TicksBetweenBurstShots,
        ScalarField::Warmup,
        ScalarField::Cooldown,
        ScalarField::AccuracyTouch,
        ScalarField::AccuracyShort,
        ScalarField::AccuracyMedium,
        ScalarField::AccuracyLong,
        ScalarField::ToolPower(0),
        ScalarField::ToolCooldown(0),
        ScalarField::ToolArmorPenetration(0),
        ScalarField::ToolChanceFactor(1),
        ScalarField::ToolPower(5),
        ScalarField::CeMagazineSize,
        ScalarField::CeReloadTime,
        ScalarField::CeBulk,
        ScalarField::CeSwayFactor,
        ScalarField::CeShotSpread,
        ScalarField::CeMeleeDodgeChance,
        ScalarField::CeToolSharp(0),
        ScalarField::CeToolBlunt(1),
    ]
}

fn field_strategy() -> impl Strategy<Value = ScalarField> {
    let all = fields();
    (0..all.len()).prop_map(move |i| all[i])
}

fn source_strategy() -> impl Strategy<Value = ValueSource> {
    prop_oneof![
        Just(ValueSource::Suggested),
        Just(ValueSource::Anchor),
        Just(ValueSource::Answered),
        Just(ValueSource::Typed),
    ]
}

fn value_strategy() -> impl Strategy<Value = f64> {
    prop_oneof![
        -1000.0f64..1000.0,
        0.0f64..50.0,
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(0.0),
    ]
}

fn start_spec(variant: u8) -> DesignSpec {
    match variant % 4 {
        0 => ranged_spec(),
        1 => melee_spec(),
        2 => {
            let mut s = ranged_spec();
            s.ce = Some(ranged_ce());
            s
        }
        _ => {
            let mut s = melee_spec();
            s.ce = Some(melee_ce());
            s
        }
    }
}

fn typed_snapshot(spec: &DesignSpec) -> BTreeMap<String, f64> {
    spec.set_scalars()
        .into_iter()
        .filter(|(_, s)| s.is_typed())
        .map(|(f, s)| (f.pointer(), s.value))
        .collect()
}

proptest! {
    /// IT-003 at model level: a non typed offer never changes a typed value, whatever came before.
    #[test]
    fn typed_values_survive_any_sequence_of_offers(
        variant in 0u8..4,
        ops in proptest::collection::vec((field_strategy(), value_strategy(), source_strategy()), 0..80),
    ) {
        let mut spec = start_spec(variant);
        let had_ce = spec.ce.is_some();
        for (field, value, source) in ops {
            let before = typed_snapshot(&spec);
            spec.offer(field, value, source);
            let after = typed_snapshot(&spec);
            if source == ValueSource::Typed {
                // A typed offer may change only the field it targets.
                for (pointer, v) in &before {
                    if *pointer != field.pointer() {
                        prop_assert_eq!(after.get(pointer), Some(v));
                    }
                }
            } else {
                for (pointer, v) in &before {
                    prop_assert_eq!(after.get(pointer), Some(v), "typed value at {} changed", pointer);
                }
            }
            prop_assert_eq!(spec.ce.is_some(), had_ce, "an offer must never toggle the CE patch");
        }
    }

    /// The last applied typed value of a field is what the spec holds, and it is marked typed.
    #[test]
    fn the_last_typed_value_wins_and_stays_typed(
        variant in 0u8..4,
        ops in proptest::collection::vec((field_strategy(), 0.0f64..500.0, source_strategy()), 1..60),
    ) {
        let mut spec = start_spec(variant);
        let mut last_typed: BTreeMap<String, Sourced<f64>> = BTreeMap::new();
        for (field, value, source) in ops {
            let outcome = spec.offer(field, value, source);
            if source == ValueSource::Typed
                && outcome.applied()
                && let Some(now) = spec.scalar(field)
            {
                last_typed.insert(field.pointer(), now);
            }
        }
        for (field, _) in spec.set_scalars() {
            if let Some(expected) = last_typed.get(&field.pointer()) {
                prop_assert_eq!(spec.scalar(field), Some(*expected));
                prop_assert!(expected.is_typed());
            }
        }
    }

    /// A spec that went through any offers still round trips through the draft envelope.
    #[test]
    fn drafts_round_trip_after_any_offers(
        variant in 0u8..4,
        ops in proptest::collection::vec((field_strategy(), value_strategy(), source_strategy()), 0..40),
        quiz in any::<bool>(),
    ) {
        let mut spec = start_spec(variant);
        for (field, value, source) in ops {
            spec.offer(field, value, source);
        }
        let mut draft = Draft::new(spec);
        if quiz {
            draft.calibration = CalibrationMode::Quiz;
            draft.answers.insert("q1".into(), serde_json::json!({"pick": 2}));
            draft.anchors.push(Anchor::new("RS_Anchor"));
        }
        let text = serde_json::to_string(&draft).unwrap();
        let back: Draft = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(&back, &draft);
        prop_assert_eq!(serde_json::to_string(&back).unwrap(), text);
    }
}

#[test]
fn a_suggestion_pass_over_a_typed_draft_changes_nothing_typed() {
    let mut spec = ranged_spec();
    spec.offer(ScalarField::Damage, 13.0, ValueSource::Typed);
    spec.offer(ScalarField::Range, 31.0, ValueSource::Typed);
    let typed_before = typed_snapshot(&spec);
    for field in fields() {
        spec.offer(field, 1.5, ValueSource::Suggested);
        spec.offer(field, 2.5, ValueSource::Answered);
        spec.offer(field, 3.5, ValueSource::Anchor);
    }
    let typed_after = typed_snapshot(&spec);
    for (pointer, value) in typed_before {
        assert_eq!(typed_after.get(&pointer), Some(&value));
    }
    assert_eq!(spec.scalar(ScalarField::Damage), Some(Sourced::typed(13.0)));
}

#[test]
fn draft_ce_toggle_defaults_to_off_and_survives_a_round_trip() {
    let draft = Draft::new(ranged_spec());
    assert!(!draft.ce_patch_enabled());
    let json = serde_json::to_value(&draft).unwrap();
    assert!(json["spec"].get("ce").is_none());
    let mut on = draft.clone();
    on.spec.ce = Some(ranged_ce());
    let back: Draft = serde_json::from_value(serde_json::to_value(&on).unwrap()).unwrap();
    assert!(back.ce_patch_enabled());
    assert_eq!(back, on);
}

#[test]
fn a_draft_with_seventeen_digit_floats_round_trips_bit_for_bit() {
    let values = [
        0.1_f64 + 0.2,
        1.0 / 3.0,
        12.345_678_901_234_567,
        2.0_f64.sqrt(),
    ];
    let fields = [
        ScalarField::Damage,
        ScalarField::Range,
        ScalarField::Warmup,
        ScalarField::Mass,
    ];
    let mut spec = ranged_spec();
    for (field, value) in fields.iter().zip(values) {
        spec.offer(*field, value, ValueSource::Typed);
    }
    let draft = Draft::new(spec);
    let text = serde_json::to_string(&draft).unwrap();
    let back: Draft = serde_json::from_str(&text).unwrap();
    for (field, value) in fields.iter().zip(values) {
        let got = back.spec.scalar(*field).unwrap().value;
        assert_eq!(got.to_bits(), value.to_bits());
    }
    assert_eq!(back, draft);
}
