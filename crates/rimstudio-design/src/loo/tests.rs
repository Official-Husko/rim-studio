//! Tests of the leave one out harness on fictional pools.

use super::*;
use crate::classes::synthetic::{SyntheticSpec, synthetic_items};
use crate::classes::{PoolOptions, ReferenceItem};

fn pool(n: usize, noise: f64, seed: u64) -> Pool {
    let items = synthetic_items(&SyntheticSpec {
        n,
        noise,
        seed,
        ..SyntheticSpec::default()
    });
    Pool::build(ItemKind::Ranged, &items, &PoolOptions::default())
}

fn quick_options() -> ValidateOptions {
    let mut noisy = AnswerProfile::noisy();
    noisy.replicates = 3;
    ValidateOptions {
        profiles: vec![
            AnswerProfile::ideal(),
            noisy,
            AnswerProfile::simple(),
            AnswerProfile::class_median(),
        ],
        twin_check: false,
        ..ValidateOptions::default()
    }
}

fn run(n: usize, options: &ValidateOptions) -> CalibrationMetrics {
    validate(&pool(n, 0.08, 1), &BaselineConfig::default(), options).unwrap()
}

fn mean_factor(v: &VariantMetrics) -> f64 {
    let f: Vec<f64> = v.per_stat.values().map(|m| m.factor_p80).collect();
    f.iter().sum::<f64>() / f.len() as f64
}

#[test]
fn leave_one_out_beats_the_class_median_baseline() {
    let m = run(48, &quick_options());
    let ideal = &m.variants["ideal"];
    let base = &m.variants["class-median"];
    assert!(
        ideal.macro_mean_error.unwrap() < base.macro_mean_error.unwrap(),
        "{ideal:?} vs {base:?}"
    );
    let mass = &ideal.per_stat["mass"];
    assert!(
        mass.median_error.unwrap() < mass.baseline_median_error.unwrap(),
        "{mass:?}"
    );
    let damage = &ideal.per_stat["damage"];
    assert!(
        damage.median_error.unwrap() < damage.baseline_median_error.unwrap(),
        "{damage:?}"
    );
    // the quiz is also better than simple mode on the strength driven stat
    assert!(
        mass.median_error.unwrap()
            <= m.variants["simple"].per_stat["mass"].median_error.unwrap() + 0.02
    );
}

#[test]
fn predictions_rank_items_correctly() {
    let m = run(48, &quick_options());
    let r = m.variants["ideal"].per_stat["mass"]
        .rank_correlation
        .unwrap();
    assert!(r > 0.6, "rank correlation {r}");
}

#[test]
fn bands_widen_when_answers_get_worse() {
    let mut options = quick_options();
    options.profiles.push(AnswerProfile::dropped(0.7));
    let m = run(48, &options);
    let ideal = mean_factor(&m.variants["ideal"]);
    let noisy = mean_factor(&m.variants["noisy"]);
    let simple = mean_factor(&m.variants["simple"]);
    assert!(noisy >= ideal * 0.98, "noisy {noisy} ideal {ideal}");
    assert!(simple > ideal, "simple {simple} ideal {ideal}");
    // dropping answers (comparisons included) widens the bands of the strength driven stats
    let dropped = mean_factor(&m.variants["dropped"]);
    assert!(dropped > ideal, "dropped {dropped} ideal {ideal}");
    let mass = |name: &str| m.variants[name].per_stat["mass"].factor_p80;
    assert!(
        mass("dropped") > mass("ideal"),
        "{} vs {}",
        mass("dropped"),
        mass("ideal")
    );
    assert!(mass("simple") > mass("ideal"));
}

#[test]
fn bands_cover_at_least_their_nominal_share_inside_the_pool() {
    let m = run(48, &quick_options());
    let noisy = &m.variants["noisy"];
    for (stat, s) in &noisy.per_stat {
        if s.factor_p50 <= 1.0 + 1e-9 {
            continue; // a discrete stat that is mostly exact has no meaningful band
        }
        assert!(
            s.coverage_p80 >= 0.72,
            "{stat}: p80 coverage {}",
            s.coverage_p80
        );
        assert!(
            s.coverage_p50 >= 0.40 && s.coverage_p50 <= 0.60,
            "{stat}: p50 coverage {}",
            s.coverage_p50
        );
        assert!(s.factor_p80 >= s.factor_p50 && s.factor_p50 >= 1.0);
    }
}

#[test]
fn bands_cover_on_held_out_items_of_a_fresh_pool() {
    let calibration_pool = pool(60, 0.08, 1);
    let mut profile = AnswerProfile::noisy();
    profile.replicates = 4;
    let options = ValidateOptions {
        profiles: vec![profile.clone()],
        twin_check: false,
        ..ValidateOptions::default()
    };
    let metrics = validate(&calibration_pool, &BaselineConfig::default(), &options).unwrap();
    let bands = metrics.bands_for(CalibrationMode::Quiz);
    let model = Model::fit(calibration_pool, BaselineConfig::default());
    let fresh = pool(60, 0.08, 2);
    let quiz_config = QuizConfig::for_kind(ItemKind::Ranged);
    let (mut inside, mut total) = (0usize, 0usize);
    for (i, item) in fresh.items.iter().enumerate() {
        let mut rng = SplitMix64::new(SEED_BASE + i as u64);
        let sim = simulate_item(&model, item, &profile, &quiz_config, &mut rng).unwrap();
        for stat in ["mass", "damage", "range", "warmup"] {
            let (Some(pred), Some(truth), Some(band)) =
                (sim.values.get(stat), item.stat(stat), bands.get(stat))
            else {
                continue;
            };
            total += 1;
            if band.deviation(*pred, truth) <= band.p80.ln() {
                inside += 1;
            }
        }
    }
    let share = inside as f64 / total as f64;
    assert!(share >= 0.68, "held out P80 coverage {share} over {total}");
}

#[test]
fn the_result_is_byte_identical_for_any_thread_count() {
    let mut options = quick_options();
    options.twin_check = true;
    let p = pool(26, 0.08, 3);
    let one = validate(
        &p,
        &BaselineConfig::default(),
        &ValidateOptions {
            threads: 1,
            ..options.clone()
        },
    )
    .unwrap();
    let eight = validate(
        &p,
        &BaselineConfig::default(),
        &ValidateOptions {
            threads: 8,
            ..options.clone()
        },
    )
    .unwrap();
    let three = validate(
        &p,
        &BaselineConfig::default(),
        &ValidateOptions {
            threads: 3,
            ..options
        },
    )
    .unwrap();
    let (a, b, c) = (
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&eight).unwrap(),
        serde_json::to_string(&three).unwrap(),
    );
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn a_pool_full_of_near_twins_is_flagged_optimistic() {
    let base = synthetic_items(&SyntheticSpec {
        n: 16,
        noise: 0.15,
        ..SyntheticSpec::default()
    });
    let mut items = Vec::new();
    for it in &base {
        for copy in 0..3 {
            let mut c: ReferenceItem = it.clone();
            c.id = format!("{}_c{copy}", it.id);
            let wobble = 1.0 + 0.002 * f64::from(copy);
            c.strength = c.strength.map(|s| s * wobble);
            for v in c.stats.values_mut() {
                *v *= wobble;
            }
            items.push(c);
        }
    }
    let twins = Pool::build(ItemKind::Ranged, &items, &PoolOptions::default());
    let options = ValidateOptions {
        profiles: vec![AnswerProfile::ideal(), AnswerProfile::class_median()],
        twin_check: true,
        ..ValidateOptions::default()
    };
    let m = validate(&twins, &BaselineConfig::default(), &options).unwrap();
    let t = m.twin.as_ref().unwrap();
    assert!(t.gap > TWIN_GAP && t.optimistic, "{t:?}");
    assert!(m.optimistic_twins());
}

#[test]
fn question_counts_stay_within_the_caps() {
    let m = run(40, &quick_options());
    let ideal = &m.variants["ideal"];
    assert!(ideal.max_questions <= 9, "{}", ideal.max_questions);
    assert!(ideal.mean_questions >= 2.0);
    assert_eq!(m.variants["simple"].mean_questions, 3.0);
}

#[test]
fn too_small_pools_are_rejected() {
    let err = validate(
        &pool(3, 0.1, 1),
        &BaselineConfig::default(),
        &quick_options(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "design.degenerate");
}

#[test]
fn simulation_is_reproducible_for_a_seed() {
    let p = pool(30, 0.1, 5);
    let model = Model::fit(p.without(&[7]), BaselineConfig::default());
    let item = p.items[7].clone();
    let profile = AnswerProfile::noisy();
    let cfg = QuizConfig::for_kind(ItemKind::Ranged);
    let a = simulate_item(&model, &item, &profile, &cfg, &mut SplitMix64::new(42)).unwrap();
    let b = simulate_item(&model, &item, &profile, &cfg, &mut SplitMix64::new(42)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn metadata_and_band_selection() {
    let mut options = quick_options();
    options.now_unix_ms = Some(1_700_000_000_000);
    let m = run(30, &options);
    assert_eq!(m.calibrated_at_ms, Some(1_700_000_000_000));
    assert_eq!(m.pool_size, 30);
    assert_eq!(m.harness_version, HARNESS_VERSION);
    assert_eq!(
        m.bands_for(CalibrationMode::Quiz),
        m.bands("noisy").unwrap()
    );
    assert_eq!(
        m.bands_for(CalibrationMode::Simple),
        m.bands("simple").unwrap()
    );
    assert!(m.bands("missing").is_none());
    assert!(m.twin.is_none() && !m.optimistic_twins());
    let empty = CalibrationMetrics {
        variants: BTreeMap::new(),
        ..m.clone()
    };
    assert!(empty.bands_for(CalibrationMode::Quiz).is_empty());
}

#[test]
fn metrics_round_trip_through_json() {
    let m = run(24, &quick_options());
    let json = serde_json::to_string(&m).unwrap();
    let back: CalibrationMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pool_size, m.pool_size);
    assert_eq!(
        back.variants.keys().collect::<Vec<_>>(),
        m.variants.keys().collect::<Vec<_>>()
    );
    let (a, b) = (
        &m.variants["noisy"].per_stat["mass"],
        &back.variants["noisy"].per_stat["mass"],
    );
    assert!((a.factor_p80 - b.factor_p80).abs() < 1e-12);
}
