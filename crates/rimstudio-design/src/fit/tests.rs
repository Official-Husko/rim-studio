//! Tests of the fit meter.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;
use crate::baseline::{BaselineConfig, BaselineInput, Model, StrengthChoice, StrengthInput};
use crate::classes::synthetic::{SyntheticSpec, synthetic_items};
use crate::classes::{ClassKey, ItemKind, Pool, PoolOptions};
use crate::loo::{AnswerProfile, TwinCheck, ValidateOptions, validate};

fn setup() -> (Model, CalibrationMetrics) {
    let items = synthetic_items(&SyntheticSpec {
        n: 60,
        ..SyntheticSpec::default()
    });
    let pool = Pool::build(ItemKind::Ranged, &items, &PoolOptions::default());
    let mut noisy = AnswerProfile::noisy();
    noisy.replicates = 3;
    let options = ValidateOptions {
        profiles: vec![AnswerProfile::ideal(), noisy, AnswerProfile::simple()],
        twin_check: false,
        now_unix_ms: Some(1_700_000_000_000),
        ..ValidateOptions::default()
    };
    let metrics = validate(&pool, &BaselineConfig::default(), &options).unwrap();
    (Model::fit(pool, BaselineConfig::default()), metrics)
}

fn class_and_prediction(model: &Model) -> (ClassStats, Estimate) {
    let key = ClassKey::new("RS_role_b", 1);
    let class = model.class_stats(&key);
    let input = BaselineInput {
        key,
        strength: Some(StrengthInput::Choice {
            choice: StrengthChoice::Typical,
        }),
        constraints: BTreeMap::new(),
    };
    (class, model.estimate(&input).unwrap())
}

#[test]
fn typicality_matches_the_published_vector() {
    let peers = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0];
    let ln: Vec<f64> = peers.iter().map(|v: &f64| v.ln()).collect();
    assert!((median(&ln).unwrap() - 2.3502).abs() < 1e-4);
    assert!((MAD_SCALE * mad(&ln).unwrap() - 0.2133).abs() < 1e-4);
    assert!((typicality_of(&peers, 13.0).unwrap() - 1.0).abs() < 1e-12);
    let far = typicality_of(&peers, 30.0).unwrap();
    assert!((far - 0.003).abs() < 0.001, "{far}");
}

#[test]
fn typicality_needs_peers_and_a_positive_value() {
    assert!(typicality_of(&[1.0, 2.0], 1.5).is_none());
    assert!(typicality_of(&[1.0, 2.0, 3.0], 0.0).is_none());
    assert!(typicality_of(&[1.0, 2.0, 3.0], f64::NAN).is_none());
    assert!(typicality_of(&[0.0, 0.0, 0.0, 5.0], 5.0).is_none());
}

#[test]
fn the_prediction_is_typical_and_far_values_are_unusual() {
    let (model, metrics) = setup();
    let (class, est) = class_and_prediction(&model);
    let proposal = Proposal::new()
        .with_estimate(&est)
        .with_value("mass", est.value("mass").unwrap());
    let report = score(&proposal, &class, &metrics);
    let fit = report.per_stat.iter().find(|s| s.stat == "mass").unwrap();
    assert_eq!(fit.level, FitLevel::Typical);
    assert!(fit.p50.low < fit.predicted && fit.predicted < fit.p50.high);
    assert!(fit.p80.low <= fit.p50.low && fit.p50.high <= fit.p80.high);
    assert!(!fit.default_band);

    let far = Proposal::new()
        .with_estimate(&est)
        .with_value("mass", est.value("mass").unwrap() * 8.0);
    let report = score(&far, &class, &metrics);
    assert_eq!(report.per_stat[0].level, FitLevel::Unusual);
    assert_eq!(report.summary.unusual, 1);
    assert!(report.summary.share_in_p80 < 1e-12);
}

#[test]
fn levels_follow_the_band_factors() {
    let (model, metrics) = setup();
    let (class, est) = class_and_prediction(&model);
    let band = metrics.bands("noisy").unwrap()["mass"];
    let predicted = est.value("mass").unwrap();
    let at = |factor: f64| {
        let p = Proposal::new()
            .with_estimate(&est)
            .with_value("mass", predicted * factor);
        score(&p, &class, &metrics).per_stat[0].level
    };
    assert_eq!(at(1.0 + (band.p50 - 1.0) * 0.9), FitLevel::Typical);
    assert_eq!(
        at(band.p50 * 1.02).max(FitLevel::Plausible),
        at(band.p50 * 1.02)
    );
    assert_eq!(at(band.p80 * 1.05), FitLevel::Unusual);
    if band.p80 > band.p50 * 1.1 {
        assert_eq!(at((band.p50 * band.p80).sqrt()), FitLevel::Plausible);
    }
    // symmetric on the log scale
    assert_eq!(at(1.0 / (band.p80 * 1.05)), FitLevel::Unusual);
}

#[test]
fn there_is_no_red_level() {
    let names: Vec<String> = [FitLevel::Typical, FitLevel::Plausible, FitLevel::Unusual]
        .iter()
        .map(|l| serde_json::to_string(l).unwrap())
        .collect();
    assert_eq!(names, vec!["\"typical\"", "\"plausible\"", "\"unusual\""]);
    assert!(["green", "blue", "amber"].contains(&FitLevel::Unusual.colour()));
    assert_eq!(FitLevel::Typical.colour(), "green");
}

#[test]
fn rank_counts_reference_items() {
    let (model, metrics) = setup();
    let (class, _) = class_and_prediction(&model);
    let mass = &class.stats["mass"];
    let value = mass.values[mass.values.len() / 2];
    let report = score(&Proposal::new().with_value("mass", value), &class, &metrics);
    let rank = report.per_stat[0].rank;
    assert_eq!(rank.total, mass.n);
    assert!(rank.equal >= 1);
    assert_eq!(
        rank.below,
        mass.values.iter().filter(|v| **v < value - 1e-9).count()
    );
    assert!((report.per_stat[0].nearest_reference - value).abs() < 1e-9);
    // no prediction given: centred on the class median
    assert!((report.per_stat[0].predicted - mass.median).abs() < 1e-12);
}

#[test]
fn unknown_and_non_finite_stats_are_listed_not_scored() {
    let (model, metrics) = setup();
    let (class, _) = class_and_prediction(&model);
    let p = Proposal::new()
        .with_value("RS_nothing", 3.0)
        .with_value("mass", f64::NAN)
        .with_value("range", 20.0);
    let report = score(&p, &class, &metrics);
    assert_eq!(
        report.unscored,
        vec!["RS_nothing".to_string(), "mass".to_string()]
    );
    assert_eq!(report.per_stat.len(), 1);
}

#[test]
fn a_stat_without_calibration_uses_the_default_band() {
    let (model, mut metrics) = setup();
    metrics.variants.clear();
    let (class, _) = class_and_prediction(&model);
    let report = score(
        &Proposal::new().with_value("mass", class.stats["mass"].median),
        &class,
        &metrics,
    );
    assert!(report.per_stat[0].default_band);
    assert_eq!(report.per_stat[0].level, FitLevel::Typical);
}

#[test]
fn notices_carry_the_date_the_pool_size_and_the_rough_and_twin_flags() {
    let (model, mut metrics) = setup();
    let (class, _) = class_and_prediction(&model);
    let report = score(&Proposal::new().with_value("mass", 5.0), &class, &metrics);
    assert_eq!(report.notices.calibrated_at_ms, Some(1_700_000_000_000));
    assert_eq!(report.notices.pool_size, 60);
    assert_eq!(report.notices.class_n, class.n);
    assert!(report.notices.class_label.starts_with("based on"));
    assert!(!report.notices.optimistic);
    metrics.twin = Some(TwinCheck {
        plain_error: 0.1,
        twin_free_error: 0.3,
        gap: 0.2,
        optimistic: true,
    });
    assert!(score(&Proposal::new(), &class, &metrics).notices.optimistic);
    // a small class is rough, a big one is not
    assert_eq!(report.notices.rough, class.n < 15);
    let all = model.class_stats(&ClassKey::default());
    assert!(!score(&Proposal::new(), &all, &metrics).notices.rough);
}

#[test]
fn a_typical_held_out_item_scores_well_and_an_absurd_one_does_not() {
    let (model, metrics) = setup();
    let fresh = synthetic_items(&SyntheticSpec {
        n: 30,
        seed: 77,
        ..SyntheticSpec::default()
    });
    let fresh_pool = Pool::build(ItemKind::Ranged, &fresh, &PoolOptions::default());
    let (mut typical_scores, mut share) = (Vec::new(), Vec::new());
    for item in fresh_pool.items.iter().take(20) {
        let key = ClassKey::of_item(item);
        let class = model.class_stats(&key);
        let est = model
            .estimate(&BaselineInput {
                key,
                strength: Some(StrengthInput::Placed {
                    ln_power: item.ln_strength,
                }),
                constraints: BTreeMap::new(),
            })
            .unwrap();
        let mut p = Proposal::new().with_estimate(&est);
        for (k, v) in &item.stats {
            p = p.with_value(k.clone(), *v);
        }
        let r = score(&p, &class, &metrics);
        typical_scores.push(r.typicality.unwrap());
        share.push(r.summary.share_in_p80);
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    assert!(
        mean(&typical_scores) > 70.0,
        "typicality {}",
        mean(&typical_scores)
    );
    assert!(mean(&share) > 0.7, "share {}", mean(&share));

    let key = ClassKey::new("RS_role_a", 1);
    let class = model.class_stats(&key);
    let mut absurd = Proposal::new();
    for name in ["mass", "damage", "range", "warmup"] {
        absurd = absurd.with_value(name, class.stats[name].median * 25.0);
    }
    let r = score(&absurd, &class, &metrics);
    assert!(r.typicality.unwrap() < 10.0, "{:?}", r.typicality);
    assert_eq!(r.summary.unusual, 4);
}

#[test]
fn simple_mode_bands_are_wider_than_quiz_bands() {
    let (model, metrics) = setup();
    let (class, est) = class_and_prediction(&model);
    let p = Proposal::new()
        .with_estimate(&est)
        .with_value("mass", est.value("mass").unwrap());
    let quiz = score_with(&p, &class, &metrics, CalibrationMode::Quiz);
    let simple = score_with(&p, &class, &metrics, CalibrationMode::Simple);
    let width = |r: &FitReport| r.per_stat[0].p80.high - r.per_stat[0].p80.low;
    assert!(width(&simple) > width(&quiz));
}

#[test]
fn reports_are_deterministic_and_serialisable() {
    let (model, metrics) = setup();
    let (class, est) = class_and_prediction(&model);
    let p = Proposal::new()
        .with_estimate(&est)
        .with_value("mass", 4.0)
        .with_value("range", 30.0);
    let a = serde_json::to_string(&score(&p, &class, &metrics)).unwrap();
    let b = serde_json::to_string(&score(&p, &class, &metrics)).unwrap();
    assert_eq!(a, b);
    let back: FitReport = serde_json::from_str(&a).unwrap();
    assert_eq!(back.per_stat.len(), 2);
    assert_eq!(
        back.per_stat[0].level,
        score(&p, &class, &metrics).per_stat[0].level
    );
}

proptest! {
    #[test]
    fn typicality_is_in_unit_range(peers in proptest::collection::vec(0.1f64..1000.0, 3..20), x in 0.01f64..10_000.0) {
        let t = typicality_of(&peers, x).unwrap();
        prop_assert!((0.0..=1.0).contains(&t));
    }

    #[test]
    fn levels_never_improve_as_a_value_moves_away(factor in 1.0f64..6.0) {
        let (model, metrics) = setup_cached();
        let key = ClassKey::new("RS_role_b", 1);
        let class = model.class_stats(&key);
        let predicted = class.stats["mass"].median;
        let near = Proposal::new().with_prediction("mass", predicted).with_value("mass", predicted * factor);
        let far = Proposal::new().with_prediction("mass", predicted).with_value("mass", predicted * factor * 1.5);
        let ln = score(&near, &class, metrics).per_stat[0].level;
        let lf = score(&far, &class, metrics).per_stat[0].level;
        prop_assert!(lf >= ln);
    }
}

fn setup_cached() -> &'static (Model, CalibrationMetrics) {
    static CELL: std::sync::OnceLock<(Model, CalibrationMetrics)> = std::sync::OnceLock::new();
    CELL.get_or_init(setup)
}
