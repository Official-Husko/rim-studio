//! Tests of the class statistics over the fictional conversion set.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;
use crate::ce::reader::fixtures_tests::{Build, load_set, rows};
use crate::ce::reader::{CeReadOptions, read_conversions, read_conversions_with};
use crate::classes::{ChainLevel, ItemKind};
use crate::model::TechLevel;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn model_with_twins() -> CeModel {
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows,
        Build {
            ce: false,
            ..Build::default()
        },
    );
    read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            vanilla: Some(&plain.databases),
            ..CeReadOptions::default()
        },
    )
}

fn vanilla_input(mass: f64, range: f64) -> BTreeMap<String, f64> {
    [
        ("mass", mass),
        ("range", range),
        ("warmup", 1.2),
        ("cooldown", 1.6),
        ("burst", 1.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

#[test]
fn the_ranged_strength_proxy_follows_its_definition() {
    let stats: BTreeMap<String, f64> = [
        ("damage", 10.0),
        ("burst", 3.0),
        ("warmup", 1.0),
        ("cooldown", 1.0),
        ("ticks_between", 6.0),
        ("range", 25.0),
        ("ap_sharp", 2.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    // cycle = 1 + 1 + 2 * 6 / 60 = 2.2; 10 * 3 / 2.2 * 1 * (1 + 0.1 * 2).
    let expected = 30.0 / 2.2 * 1.2;
    let got = gun_strength(&stats, &StrengthDefinition::default()).unwrap();
    assert!(close(got, expected));
    let mut missing = stats.clone();
    missing.remove("range");
    assert!(gun_strength(&missing, &StrengthDefinition::default()).is_none());
    let melee: BTreeMap<String, f64> = [("tool_power", 20.0), ("tool_cooldown", 2.0)]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    assert!(close(melee_strength_proxy(&melee).unwrap(), 10.0));
    assert!(melee_strength_proxy(&BTreeMap::new()).is_none());
}

#[test]
fn pools_are_built_per_kind_and_carry_twin_and_ratio_stats() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let ranged = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    assert_eq!(ranged.len(), 12);
    let item = ranged.items.iter().find(|i| i.id == "RS_CeGun04").unwrap();
    assert_eq!(item.role, "CE_AI_RS_Rifle");
    assert_eq!(item.tier, TechLevel::Industrial.index());
    assert!(!item.tier_derived && !item.role_derived);
    assert!(close(item.stats["ratio.mass"], 1.0));
    assert!(close(item.stats["ratio.range"], 1.8));
    assert!(item.stats.contains_key("vanilla.warmup"));
    let melee = build_pool(&model, ItemKind::Melee, &opts).unwrap();
    assert_eq!(melee.len(), 1);
    assert!(build_pool(&model, ItemKind::Apparel, &opts).is_err());
}

#[test]
fn the_class_statistics_find_ratios_and_class_constants() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let key = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Rifle"), None);
    let stats = ClassStats::derive(&pool, &key, &opts);
    assert_eq!(stats.base.level, ChainLevel::RoleTier);
    assert_eq!(stats.base.n, 4);
    assert!(stats.fallback_note().is_none());
    let range = stats.ratio("range").unwrap();
    assert!(close(range.median, 1.8));
    assert!(range.flat);
    let spread = stats.constant("spread").unwrap();
    assert!(close(spread.value, 0.07));
    assert!(stats.constant("sway").is_some());
    assert!(
        stats.constant("range").is_none(),
        "range varies inside the class"
    );
    assert!(
        stats
            .constants
            .iter()
            .all(|c| !c.stat.starts_with("ratio."))
    );
}

#[test]
fn a_sparse_class_widens_stat_by_stat_and_says_so() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    // Two industrial pistols: fewer than three, so the class widens to the role.
    let pistol = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Pistol"), None);
    let stats = ClassStats::derive(&pool, &pistol, &opts);
    assert_eq!(stats.base.level, ChainLevel::Role);
    assert_eq!(stats.base.n, 4);
    assert!(stats.fallback_note().unwrap().contains("widened"));
    // An unknown role falls through to the tier.
    let unknown = class_key(Some(TechLevel::Spacer), Some("CE_AI_RS_Nothing"), None);
    let stats = ClassStats::derive(&pool, &unknown, &opts);
    assert_eq!(stats.base.level, ChainLevel::Tier);
    assert_eq!(stats.base.n, 6);
    // No axis at all: everything.
    let any = ClassStats::derive(&pool, &ClassKey::default(), &opts);
    assert_eq!(any.base.level, ChainLevel::All);
    assert_eq!(any.base.n, 12);
}

#[test]
fn a_conversion_is_predicted_by_the_predictor_that_held_out_best() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let key = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Rifle"), None);
    let p = predict_conversion(&pool, &key, &vanilla_input(3.0, 30.0), &opts);
    let mass = &p.stats["mass"];
    assert_eq!(mass.predictor, ConversionPredictor::Identity);
    assert!(close(mass.value, 3.0));
    assert_eq!(mass.driver.as_deref(), Some("mass"));
    assert!(mass.loo_error.is_some_and(|e| e < 1e-9));
    let range = &p.stats["range"];
    assert_eq!(range.predictor, ConversionPredictor::Ratio);
    assert!(close(range.value, 54.0));
    let bulk = &p.stats["bulk"];
    assert!(close(bulk.value, 6.0), "bulk follows the vanilla mass");
    assert_eq!(bulk.driver.as_deref(), Some("mass"));
    let spread = &p.stats["spread"];
    assert_eq!(spread.predictor, ConversionPredictor::Median);
    assert!(close(spread.value, 0.07));
    assert!(spread.driver.is_none());
    let magazine = &p.stats["magazine"];
    assert!(close(magazine.value, 30.0));
    let burst = &p.stats["burst"];
    assert_eq!(burst.predictor, ConversionPredictor::Identity);
    assert!(close(burst.value, 1.0));
    assert!(p.notes.is_empty(), "{:?}", p.notes);
    assert!(p.class_label.contains("based on"));
    assert!(!p.stats.contains_key("vanilla.mass"));
    assert!(!p.stats.contains_key("ratio.mass"));
    assert!(!p.stats.contains_key("strength"));
    // Exact predictors have a flat band.
    assert!(close(range.band.p50, 1.0));
}

#[test]
fn bands_widen_when_the_class_is_noisy() {
    // Same ranges, but the CE range now scatters around the multiple.
    let mut model = model_with_twins();
    for (i, g) in model.guns.iter_mut().enumerate() {
        if let Some(r) = g.stats.get_mut("range") {
            *r *= if i % 2 == 0 { 1.25 } else { 0.8 };
        }
    }
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let p = predict_conversion(
        &pool,
        &ClassKey::default(),
        &vanilla_input(3.0, 30.0),
        &opts,
    );
    let range = &p.stats["range"];
    assert!(range.band.p50 > 1.05, "{:?}", range.band);
    assert!(range.band.p80 >= range.band.p50);
    assert!(range.loo_error.is_some_and(|e| e > 0.05));
}

#[test]
fn without_twins_every_stat_is_a_class_median_and_the_notes_say_so() {
    let out = load_set(&rows(), Build::default());
    let model = read_conversions(&out.databases);
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let key = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Rifle"), None);
    let p = predict_conversion(&pool, &key, &vanilla_input(3.0, 30.0), &opts);
    assert!(
        p.notes
            .iter()
            .any(|n| n.contains("no converted weapon has a vanilla twin"))
    );
    for (name, s) in &p.stats {
        assert_eq!(s.predictor, ConversionPredictor::Median, "{name}");
        assert!(s.driver.is_none(), "{name}");
    }
    assert!(close(p.stats["spread"].value, 0.07));
}

#[test]
fn a_widened_class_is_reported_in_the_prediction_notes() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let key = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Pistol"), None);
    let p = predict_conversion(&pool, &key, &vanilla_input(3.0, 30.0), &opts);
    assert_eq!(p.level, ChainLevel::Role);
    assert!(p.notes.iter().any(|n| n.contains("widened")));
    assert!(p.stats["spread"].value > 0.1, "the pistol class constant");
}

#[test]
fn an_empty_pool_gives_an_empty_prediction() {
    let model = CeModel::absent("no CE");
    let opts = CeClassOptions::default();
    let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    assert!(pool.is_empty());
    let p = predict_conversion(
        &pool,
        &ClassKey::default(),
        &vanilla_input(3.0, 30.0),
        &opts,
    );
    assert!(p.stats.is_empty());
    let stats = ClassStats::derive(&pool, &ClassKey::default(), &opts);
    assert_eq!(stats.base.n, 0);
}

#[test]
fn derivation_is_deterministic() {
    let model = model_with_twins();
    let opts = CeClassOptions::default();
    let a = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    let b = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
    assert_eq!(a, b);
    let key = ClassKey::default();
    assert_eq!(
        predict_conversion(&a, &key, &vanilla_input(3.0, 30.0), &opts),
        predict_conversion(&b, &key, &vanilla_input(3.0, 30.0), &opts)
    );
}

proptest! {
    #[test]
    fn predictions_are_finite_positive_and_monotone_in_the_vanilla_range(
        mass in 0.2f64..30.0,
        low in 5.0f64..60.0,
        extra in 0.0f64..40.0,
    ) {
        let model = model_with_twins();
        let opts = CeClassOptions::default();
        let pool = build_pool(&model, ItemKind::Ranged, &opts).unwrap();
        let key = class_key(Some(TechLevel::Industrial), Some("CE_AI_RS_Rifle"), None);
        let a = predict_conversion(&pool, &key, &vanilla_input(mass, low), &opts);
        let b = predict_conversion(&pool, &key, &vanilla_input(mass, low + extra), &opts);
        for (name, s) in &a.stats {
            prop_assert!(s.value.is_finite() && s.value >= 0.0, "{name}");
        }
        prop_assert!(b.stats["range"].value + 1e-9 >= a.stats["range"].value);
    }
}
