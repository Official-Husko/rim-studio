//! Tests of the baseline model on fictional pools.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;
use crate::classes::synthetic::{SyntheticSpec, synthetic_items};
use crate::classes::{ItemKind, Pool, PoolOptions};

fn pool(n: usize, noise: f64, seed: u64) -> Pool {
    let items = synthetic_items(&SyntheticSpec {
        n,
        noise,
        seed,
        ..SyntheticSpec::default()
    });
    Pool::build(ItemKind::Ranged, &items, &PoolOptions::default())
}

fn model(n: usize, noise: f64) -> Model {
    Model::fit(pool(n, noise, 1), BaselineConfig::default())
}

fn input(role: &str, tier: u8, strength: StrengthInput) -> BaselineInput {
    BaselineInput {
        key: ClassKey::new(role, tier),
        strength: Some(strength),
        constraints: BTreeMap::new(),
    }
}

#[test]
fn mass_prefers_a_strength_driven_predictor_and_beats_the_median() {
    let m = model(90, 0.08);
    let mass = &m.rule_scores["mass"];
    assert_ne!(mass.chosen, Predictor::Median, "{mass:?}");
    let chosen_err = match mass.chosen {
        Predictor::Quantile => mass.median_q,
        _ => mass.median_r,
    }
    .unwrap();
    assert!(chosen_err < mass.median_m.unwrap(), "{mass:?}");
}

#[test]
fn class_constants_do_not_pick_the_anchor_predictor_over_a_better_median() {
    let m = model(90, 0.08);
    let range = &m.rule_scores["range"];
    let best = [range.median_m, range.median_q, range.median_r]
        .into_iter()
        .flatten()
        .fold(f64::INFINITY, f64::min);
    let chosen = match range.chosen {
        Predictor::Median => range.median_m,
        Predictor::Quantile => range.median_q,
        Predictor::Anchor => range.median_r,
    }
    .unwrap();
    assert!((chosen - best).abs() < 1e-12, "{range:?}");
}

#[test]
fn rule_ties_resolve_in_favour_of_the_simpler_predictor() {
    // Three identical items per class: every predictor is exact, so the median must win.
    let items: Vec<_> = (0..12)
        .map(|i| {
            let mut it = crate::classes::ReferenceItem::new(format!("RS_Tie{i}"), ItemKind::Ranged);
            it.strength = Some(2.0 + f64::from(i) * 0.1);
            it.tier = Some(1);
            it.role = Some("RS_x".into());
            it.stats.insert("range".into(), 25.0);
            it
        })
        .collect();
    let m = Model::fit(
        Pool::build(ItemKind::Ranged, &items, &PoolOptions::default()),
        BaselineConfig::default(),
    );
    assert_eq!(m.rules["range"], Predictor::Median);
}

#[test]
fn estimate_labels_sources_and_explains_the_class() {
    let m = model(60, 0.08);
    let e = m
        .estimate(&input(
            "RS_role_b",
            1,
            StrengthInput::Choice {
                choice: StrengthChoice::Typical,
            },
        ))
        .unwrap();
    assert!(e.class_label.starts_with("based on"), "{}", e.class_label);
    assert!(!e.anchors.is_empty() && e.anchors.len() <= 3);
    assert!(e.anchors.windows(2).all(|w| w[0].distance <= w[1].distance));
    for (name, s) in &e.stats {
        assert!(s.value.is_some(), "{name}");
        assert!(
            matches!(
                s.source,
                Some(Source::ClassMedian | Source::Quantile | Source::Anchor)
            ),
            "{name}"
        );
        assert!(s.band.is_some());
        assert!(s.p10 <= s.median && s.median <= s.p90);
    }
}

#[test]
fn typed_values_are_never_overwritten_or_rebanded() {
    let mut config = BaselineConfig::default();
    config.identities.push(Identity::StrengthTimes {
        stat: "damage".into(),
        other: "warmup".into(),
    });
    let m = Model::fit(pool(60, 0.08, 1), config);
    for stat in ["mass", "damage", "burst", "warmup"] {
        let mut inp = input(
            "RS_role_a",
            2,
            StrengthInput::Choice {
                choice: StrengthChoice::Stronger,
            },
        );
        inp.constraints
            .insert(stat.into(), StatConstraint::Typed { value: 123.25 });
        let e = m.estimate(&inp).unwrap();
        let s = &e.stats[stat];
        assert_eq!(s.value, Some(123.25), "{stat}");
        assert_eq!(s.source, Some(Source::Typed));
        assert!(s.band.is_none());
    }
}

#[test]
fn identity_derives_the_stat_unless_it_was_answered() {
    let mut config = BaselineConfig::default();
    config.identities.push(Identity::StrengthTimes {
        stat: "damage".into(),
        other: "warmup".into(),
    });
    let m = Model::fit(pool(60, 0.08, 1), config);
    let e = m
        .estimate(&input(
            "RS_role_b",
            1,
            StrengthInput::Placed { ln_power: 0.9 },
        ))
        .unwrap();
    let expected = 0.9f64.exp() * e.value("warmup").unwrap();
    assert_eq!(e.source("damage"), Some(Source::Derived));
    assert!((e.value("damage").unwrap() - expected).abs() < 1e-9);
    let mut inp = input("RS_role_b", 1, StrengthInput::Placed { ln_power: 0.9 });
    inp.constraints.insert(
        "damage".into(),
        StatConstraint::Interval {
            lo: Some(1.0),
            hi: Some(1000.0),
        },
    );
    let e2 = m.estimate(&inp).unwrap();
    assert_eq!(e2.source("damage"), Some(Source::Answer));
}

#[test]
fn interval_answers_restrict_the_value() {
    let m = model(80, 0.08);
    let class = m.class_stats(&ClassKey::new("RS_role_b", 1));
    let mass = &class.stats["mass"];
    let (lo, hi) = (mass.percentile(55.0), mass.percentile(90.0));
    let mut inp = input("RS_role_b", 1, StrengthInput::Unknown);
    inp.constraints.insert(
        "mass".into(),
        StatConstraint::Interval {
            lo: Some(lo),
            hi: Some(hi),
        },
    );
    let e = m.estimate(&inp).unwrap();
    let v = e.value("mass").unwrap();
    assert!(v >= lo && v < hi, "{v} outside [{lo}, {hi})");
    assert_eq!(e.source("mass"), Some(Source::Answer));
}

#[test]
fn an_interval_with_no_items_uses_its_centre() {
    let m = model(40, 0.08);
    let mut inp = input("RS_role_a", 0, StrengthInput::Unknown);
    inp.constraints.insert(
        "mass".into(),
        StatConstraint::Interval {
            lo: Some(1000.0),
            hi: Some(4000.0),
        },
    );
    let v = m.estimate(&inp).unwrap().value("mass").unwrap();
    assert!((v - (1000.0f64 * 4000.0).sqrt()).abs() < 1e-6);
}

#[test]
fn vs_anchor_answer_scales_the_anchor_value_by_the_learned_ratio() {
    let m = model(80, 0.08);
    let ratios = m.bucket_ratios["mass"];
    assert!(
        ratios[0] < 1.0 && ratios[1] > 0.8 && ratios[1] < 1.25 && ratios[2] > 1.0,
        "{ratios:?}"
    );
    let base = m
        .estimate(&input(
            "RS_role_c",
            2,
            StrengthInput::Choice {
                choice: StrengthChoice::Typical,
            },
        ))
        .unwrap();
    let anchor = base.anchors[0].clone();
    for (bucket, idx) in [
        (Bucket::Lower, 0),
        (Bucket::Similar, 1),
        (Bucket::Higher, 2),
    ] {
        let mut inp = input(
            "RS_role_c",
            2,
            StrengthInput::Choice {
                choice: StrengthChoice::Typical,
            },
        );
        inp.constraints.insert(
            "mass".into(),
            StatConstraint::VsAnchor {
                anchor: anchor.id.clone(),
                bucket,
            },
        );
        let e = m.estimate(&inp).unwrap();
        let expected = anchor.stats["mass"] * ratios[idx];
        assert!((e.value("mass").unwrap() - expected).abs() < 1e-9);
        assert_eq!(e.source("mass"), Some(Source::Answer));
    }
}

#[test]
fn unknown_anchor_in_an_answer_is_ignored_with_a_note() {
    let m = model(40, 0.08);
    let mut inp = input("RS_role_a", 1, StrengthInput::Unknown);
    inp.constraints.insert(
        "mass".into(),
        StatConstraint::VsAnchor {
            anchor: "RS_Nope".into(),
            bucket: Bucket::Higher,
        },
    );
    let e = m.estimate(&inp).unwrap();
    assert!(!e.notes.is_empty());
    assert_ne!(e.source("mass"), Some(Source::Answer));
}

#[test]
fn discrete_stats_are_whole_numbers_taken_from_the_pool() {
    let m = model(60, 0.08);
    assert!(m.traits["burst"].discrete && m.traits["burst"].integer);
    for choice in [
        StrengthChoice::Weaker,
        StrengthChoice::Typical,
        StrengthChoice::Stronger,
    ] {
        let e = m
            .estimate(&input("RS_role_a", 1, StrengthInput::Choice { choice }))
            .unwrap();
        let burst = e.value("burst").unwrap();
        assert!(burst == 1.0 || burst == 4.0, "{burst}");
    }
}

#[test]
fn ask_flagged_stats_have_no_value_unless_typed() {
    let mut config = BaselineConfig::default();
    config.rules.push(StatRule {
        name: "damage".into(),
        discrete: None,
        integer: None,
        ask: true,
    });
    let m = Model::fit(pool(40, 0.08, 1), config);
    let e = m
        .estimate(&input("RS_role_a", 1, StrengthInput::Unknown))
        .unwrap();
    let d = &e.stats["damage"];
    assert!(d.value.is_none() && d.source.is_none() && d.band.is_none());
    assert!(d.p10 > 0.0 && d.p90 >= d.p10);
}

#[test]
fn bad_inputs_are_typed_errors() {
    let m = model(30, 0.08);
    let mut inp = input("RS_role_a", 1, StrengthInput::Unknown);
    inp.constraints
        .insert("mass".into(), StatConstraint::Typed { value: f64::NAN });
    assert_eq!(m.estimate(&inp).unwrap_err().code(), "design.invalid-input");
    let mut inp = input("RS_role_a", 1, StrengthInput::Unknown);
    inp.constraints.insert(
        "mass".into(),
        StatConstraint::Interval {
            lo: Some(5.0),
            hi: Some(5.0),
        },
    );
    assert!(m.estimate(&inp).is_err());
    assert!(
        m.estimate(&input("RS_role_a", 1, StrengthInput::Percentile { p: 1.5 }))
            .is_err()
    );
    assert!(
        m.estimate(&input(
            "RS_role_a",
            1,
            StrengthInput::Placed {
                ln_power: f64::INFINITY
            }
        ))
        .is_err()
    );
    let empty = Model::fit(
        Pool::build(ItemKind::Ranged, &[], &PoolOptions::default()),
        BaselineConfig::default(),
    );
    assert_eq!(
        empty
            .estimate(&BaselineInput::default())
            .unwrap_err()
            .code(),
        "design.empty-input"
    );
}

#[test]
fn fitting_and_estimating_are_deterministic() {
    let a = model(50, 0.1);
    let b = model(50, 0.1);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let inp = input("RS_role_b", 2, StrengthInput::Placed { ln_power: 1.2 });
    assert_eq!(
        serde_json::to_string(&a.estimate(&inp).unwrap()).unwrap(),
        serde_json::to_string(&b.estimate(&inp).unwrap()).unwrap()
    );
}

#[test]
fn simple_estimate_works_from_class_statistics_alone() {
    let m = model(60, 0.08);
    let class = m.class_stats(&ClassKey::new("RS_role_b", 1));
    let typed = BTreeMap::from([("range".to_string(), 77.0)]);
    let e = simple_estimate(&class, StrengthChoice::Stronger, &typed, &m.config, None).unwrap();
    assert_eq!(e.value("range"), Some(77.0));
    assert_eq!(e.source("range"), Some(Source::Typed));
    assert!(e.anchors.is_empty());
    assert!((e.strength_percentile - 5.0 / 6.0).abs() < 1e-12);
    assert!(
        e.stats
            .values()
            .filter(|s| s.source != Some(Source::Typed))
            .all(|s| s.band == Some(Band::fallback()))
    );
}

#[test]
fn anchor_baseline_scales_given_anchors_by_elasticity() {
    let m = model(60, 0.0);
    let key = ClassKey::new("RS_role_b", 1);
    let mut class = m.class_stats(&key);
    for s in class.stats.values_mut() {
        s.predictor = Predictor::Anchor;
    }
    let anchors = m.anchors(&key, 0.8, 6);
    let input = AnchorInput {
        strength: StrengthInput::Placed { ln_power: 0.8 },
        constraints: BTreeMap::new(),
    };
    let e = anchor_baseline(&anchors, &class, &input, &m.config, None).unwrap();
    assert_eq!(e.source("mass"), Some(Source::Anchor));
    // the noise free mass follows exp(1 + 0.6 ln P + 0.1 role index) with role b = 1
    let truth = (1.0f64 + 0.6 * 0.8 + 0.1).exp();
    let got = e.value("mass").unwrap();
    assert!((got / truth - 1.0).abs() < 0.12, "{got} vs {truth}");
}

#[test]
fn anchor_estimates_follow_a_noise_free_power_law_monotonically() {
    let m = model(90, 0.0);
    let key = ClassKey::new("RS_role_b", 1);
    let mut last = 0.0;
    for step in 0..40 {
        let ln_p = -0.2 + 0.07 * f64::from(step);
        let inp = BaselineInput {
            key: key.clone(),
            strength: Some(StrengthInput::Placed { ln_power: ln_p }),
            constraints: BTreeMap::new(),
        };
        let v = m.estimate(&inp).unwrap().value("mass").unwrap();
        assert!(
            v >= last * 0.97,
            "mass fell from {last} to {v} at ln P {ln_p}"
        );
        last = v;
    }
    assert!(last > 0.0);
}

proptest! {
    #[test]
    fn quantile_estimates_are_monotone_in_the_percentile(p1 in 0.0f64..1.0, p2 in 0.0f64..1.0, seed in 1u64..40) {
        let m = Model::fit(pool(45, 0.1, seed), BaselineConfig::default());
        let mut class = m.class_stats(&ClassKey::new("RS_role_a", 1));
        for s in class.stats.values_mut() {
            s.predictor = Predictor::Quantile;
        }
        let (lo, hi) = if p1 <= p2 { (p1, p2) } else { (p2, p1) };
        let run = |p: f64| {
            let inp = AnchorInput { strength: StrengthInput::Percentile { p }, constraints: BTreeMap::new() };
            anchor_baseline(&[], &class, &inp, &m.config, None).unwrap()
        };
        let (a, b) = (run(lo), run(hi));
        for (name, summary) in &class.stats {
            if summary.traits.integer { continue; }
            let (va, vb) = (a.value(name).unwrap(), b.value(name).unwrap());
            let slope = summary.rank_beta[0];
            if slope >= 0.0 { prop_assert!(va <= vb + 1e-9, "{name}: {va} > {vb}"); }
            else { prop_assert!(va >= vb - 1e-9, "{name}: {va} < {vb}"); }
        }
    }

    #[test]
    fn estimates_are_finite_for_any_placed_strength(ln_p in -3.0f64..6.0) {
        let m = model(30, 0.1);
        let inp = BaselineInput { key: ClassKey::new("RS_role_c", 3), strength: Some(StrengthInput::Placed { ln_power: ln_p }), constraints: BTreeMap::new() };
        let e = m.estimate(&inp).unwrap();
        for s in e.stats.values() {
            prop_assert!(s.value.is_some_and(f64::is_finite));
        }
    }
}

#[test]
fn bands_have_sane_geometry() {
    let b = Band::new(1.2, 1.5, 0.0);
    let (lo, hi) = b.interval50(10.0);
    assert!((lo - 10.0 / 1.2).abs() < 1e-9 && (hi - 12.0).abs() < 1e-9);
    let (lo, hi) = b.interval80(10.0);
    assert!((lo - 10.0 / 1.5).abs() < 1e-9 && (hi - 15.0).abs() < 1e-9);
    assert_eq!(
        Band::new(0.5, 0.2, -1.0),
        Band {
            p50: 1.0,
            p80: 1.0,
            shift: 0.0
        }
    );
    assert!(Band::new(1.1, 1.3, 0.0).deviation(10.0, 11.0) < 1.1f64.ln() + 1e-9);
    let from = band_from_residuals(&[0.0, 0.1, 0.2, 0.3, 0.4], 0.0);
    assert!((from.p50 - 0.2f64.exp()).abs() < 1e-9);
    assert_eq!(band_from_residuals(&[0.1], 0.0), Band::fallback());
}
