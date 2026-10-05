//! End to end calibration on fictional pools, through the public API only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use rimstudio_design::baseline::{
    BaselineConfig, BaselineInput, Model, Source, StrengthChoice, StrengthInput,
};
use rimstudio_design::classes::synthetic::{SyntheticSpec, synthetic_items};
use rimstudio_design::classes::{ClassKey, ItemKind, Pool, PoolItem, PoolOptions, ReferenceItem};
use rimstudio_design::fit::{FitLevel, Proposal, score};
use rimstudio_design::loo::{
    AnswerProfile, CalibrationMetrics, CalibrationMode, ValidateOptions, validate,
};
use rimstudio_design::quiz::{Answer, Question, Quiz};

fn pool_of(items: &[ReferenceItem]) -> Pool {
    Pool::build(ItemKind::Ranged, items, &PoolOptions::default())
}

fn options() -> ValidateOptions {
    let mut noisy = AnswerProfile::noisy();
    noisy.replicates = 3;
    ValidateOptions {
        profiles: vec![
            AnswerProfile::ideal(),
            noisy,
            AnswerProfile::simple(),
            AnswerProfile::class_median(),
        ],
        now_unix_ms: Some(1_800_000_000_000),
        threads: 2,
        ..ValidateOptions::default()
    }
}

fn truthful(model: &Model, item: &PoolItem, q: &Question) -> Answer {
    use rimstudio_design::baseline::Bucket;
    let edge = model.config.bucket_edge;
    match q {
        Question::Tier { .. } => Answer::Tier { tier: item.tier },
        Question::Role { .. } => Answer::Role {
            role: item.role.clone(),
        },
        Question::Group { .. } => Answer::Group {
            group: item.group.clone().unwrap(),
        },
        Question::Compare { anchor, .. } => {
            let d = item.ln_strength - anchor.strength.ln();
            if d.abs() < model.config.same_band {
                Answer::Same
            } else if d > 0.0 {
                Answer::Stronger
            } else {
                Answer::Weaker
            }
        }
        Question::CloserTo { lower, upper } => {
            if item.ln_strength - lower.strength.ln() < upper.strength.ln() - item.ln_strength {
                Answer::CloserToLower
            } else {
                Answer::CloserToUpper
            }
        }
        Question::Interval { stat, bins, .. } => {
            let v = item.stat(stat).unwrap();
            let index = bins
                .iter()
                .position(|b| b.lo.is_none_or(|l| v >= l) && b.hi.is_none_or(|h| v < h))
                .unwrap();
            Answer::Bin { index }
        }
        Question::VsAnchor { stat, value, .. } => {
            let r = (item.stat(stat).unwrap() / value).ln();
            Answer::Bucket {
                bucket: if r < -edge {
                    Bucket::Lower
                } else if r > edge {
                    Bucket::Higher
                } else {
                    Bucket::Similar
                },
            }
        }
    }
}

#[test]
fn calibrate_ask_estimate_and_score() {
    let all = synthetic_items(&SyntheticSpec {
        n: 56,
        ..SyntheticSpec::default()
    });
    let (held_out, training) = all.split_at(1);
    let pool = pool_of(training);
    let metrics = validate(&pool, &BaselineConfig::default(), &options()).unwrap();
    assert_eq!(metrics.pool_size, 55);
    let bands = metrics.bands_for(CalibrationMode::Quiz);
    assert!(bands.contains_key("mass"));

    let model = Model::fit(pool, BaselineConfig::default());
    let item = pool_of(held_out).items[0].clone();
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    quiz.play(&model, |q| truthful(&model, &item, q)).unwrap();
    assert!(quiz.is_finished(&model).unwrap());

    let estimate = quiz.estimate(&model, Some(&bands)).unwrap();
    assert!(
        estimate
            .stats
            .values()
            .all(|s| s.value.is_some() && s.band.is_some())
    );
    assert!(
        estimate
            .stats
            .values()
            .any(|s| s.source == Some(Source::Answer))
    );

    let class = model.class_stats(&quiz.input(&model).unwrap().key);
    let mut proposal = Proposal::new().with_estimate(&estimate);
    for (k, v) in &item.stats {
        proposal = proposal.with_value(k.clone(), *v);
    }
    let report = score(&proposal, &class, &metrics);
    assert_eq!(report.per_stat.len(), 5);
    assert!(report.summary.share_in_p80 >= 0.6, "{:?}", report.summary);
    assert_eq!(report.notices.calibrated_at_ms, Some(1_800_000_000_000));
    assert_eq!(report.notices.pool_size, 55);
    assert!(report.typicality.unwrap() > 50.0);
}

#[test]
fn a_draft_stores_the_quiz_and_reopens_identically() {
    let items = synthetic_items(&SyntheticSpec {
        n: 40,
        ..SyntheticSpec::default()
    });
    let model = Model::fit(pool_of(&items), BaselineConfig::default());
    let target = model.pool.items[10].clone();
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    for _ in 0..4 {
        let p = quiz.next(&model).unwrap().unwrap();
        let a = truthful(&model, &target, &p.question);
        quiz.answer(&model, a).unwrap();
    }
    let draft = serde_json::json!({ "name": "RS_Draft", "cePatch": false, "quiz": quiz });
    let text = serde_json::to_string_pretty(&draft).unwrap();
    let reopened: serde_json::Value = serde_json::from_str(&text).unwrap();
    let back: Quiz = serde_json::from_value(reopened["quiz"].clone()).unwrap();
    assert_eq!(back, quiz);
    assert_eq!(back.next(&model).unwrap(), quiz.next(&model).unwrap());
    assert_eq!(draft["cePatch"], serde_json::json!(false));
}

#[test]
fn calibration_is_deterministic_and_survives_a_cache_round_trip() {
    let items = synthetic_items(&SyntheticSpec {
        n: 30,
        seed: 9,
        ..SyntheticSpec::default()
    });
    let pool = pool_of(&items);
    let a = validate(&pool, &BaselineConfig::default(), &options()).unwrap();
    let b = validate(
        &pool,
        &BaselineConfig::default(),
        &ValidateOptions {
            threads: 8,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let cached: CalibrationMetrics =
        serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    let (x, y) = (a.bands("noisy").unwrap(), cached.bands("noisy").unwrap());
    assert_eq!(x.keys().collect::<Vec<_>>(), y.keys().collect::<Vec<_>>());
    for (k, band) in &x {
        assert!((band.p80 - y[k].p80).abs() < 1e-12);
    }
}

#[test]
fn thin_pools_degrade_to_wider_classes_with_an_explanation() {
    let items = synthetic_items(&SyntheticSpec {
        n: 6,
        ..SyntheticSpec::default()
    });
    let model = Model::fit(pool_of(&items), BaselineConfig::default());
    assert!(!model.quiz_available());
    let input = BaselineInput {
        key: ClassKey::new("RS_role_a", 3),
        strength: Some(StrengthInput::Choice {
            choice: StrengthChoice::Stronger,
        }),
        constraints: BTreeMap::new(),
    };
    let e = model.estimate(&input).unwrap();
    assert!(e.class_label.contains("widened"), "{}", e.class_label);
    assert!(e.stats.values().all(|s| s.value.is_some()));
}

#[test]
fn half_the_install_gives_a_thinner_calibration_not_a_failure() {
    let items = synthetic_items(&SyntheticSpec {
        n: 60,
        seed: 4,
        ..SyntheticSpec::default()
    });
    let half: Vec<ReferenceItem> = items.iter().step_by(2).cloned().collect();
    let opts = ValidateOptions {
        twin_check: false,
        ..options()
    };
    let full = validate(&pool_of(&items), &BaselineConfig::default(), &opts).unwrap();
    let thin = validate(&pool_of(&half), &BaselineConfig::default(), &opts).unwrap();
    let f = full.variants["noisy"].macro_median_error.unwrap();
    let t = thin.variants["noisy"].macro_median_error.unwrap();
    assert!(t < 0.5 && f < 0.5);
    assert!(
        thin.variants["ideal"].macro_median_error.unwrap()
            < thin.variants["class-median"].macro_median_error.unwrap()
    );
}

#[test]
fn fit_levels_are_never_red() {
    let items = synthetic_items(&SyntheticSpec {
        n: 40,
        ..SyntheticSpec::default()
    });
    let pool = pool_of(&items);
    let metrics = validate(
        &pool,
        &BaselineConfig::default(),
        &ValidateOptions {
            twin_check: false,
            ..options()
        },
    )
    .unwrap();
    let model = Model::fit(pool, BaselineConfig::default());
    let class = model.class_stats(&ClassKey::default());
    let mut p = Proposal::new();
    for (name, s) in &class.stats {
        p = p.with_value(name.clone(), s.median * 40.0);
    }
    let report = score(&p, &class, &metrics);
    assert!(report.per_stat.iter().all(|s| matches!(
        s.level,
        FitLevel::Typical | FitLevel::Plausible | FitLevel::Unusual
    )));
    assert!(
        report
            .per_stat
            .iter()
            .filter(|s| s.level == FitLevel::Unusual)
            .count()
            >= 4
    );
}
