//! Tests of the conversion estimator over fictional example sets (names start with `RS_`, numbers are
//! invented by simple formulas).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;

use super::*;

fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|t| (*t).to_owned()).collect()
}

/// One example: vanilla range `range`, converted range `range * ratio`, and constant extras.
fn example(
    id: &str,
    tags_: &[&str],
    role: Option<&str>,
    tier: u8,
    vanilla_range: f64,
    ce_range: f64,
) -> Example {
    Example {
        id: id.to_owned(),
        tier: Some(tier),
        tags: tags(tags_),
        role: role.map(str::to_owned),
        stats: map(&[
            ("range", ce_range),
            ("mass", 2.0),
            ("sway", 1.2),
            ("warmup", 0.6),
        ]),
        vanilla: map(&[
            ("range", vanilla_range),
            ("mass", 2.0),
            ("warmup", 1.0),
            ("cooldown", 1.5),
            ("burst", 1.0),
        ]),
    }
}

/// Two families that differ in tags and in how the converted range relates to the vanilla one: the
/// pistols keep a short fixed range, the rifles double it.
fn families() -> ExampleSet {
    let mut examples = Vec::new();
    for i in 0..6u32 {
        let f = f64::from(i);
        examples.push(example(
            &format!("RS_Pistol{i}"),
            &["RS_Gun", "RS_Pistol"],
            Some("RS_AI_Short"),
            3,
            25.0 + f,
            12.0 + 0.1 * f,
        ));
        examples.push(example(
            &format!("RS_Rifle{i}"),
            &["RS_Gun", "RS_Rifle"],
            Some("RS_AI_Long"),
            3,
            28.0 + 2.0 * f,
            (28.0 + 2.0 * f) * 2.0,
        ));
    }
    ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    }
}

fn profile(tags_: &[&str], role: Option<&str>, tier: u8, range: f64) -> Profile {
    Profile {
        tier: Some(tier),
        tags: tags(tags_),
        role: role.map(str::to_owned),
        vanilla: map(&[
            ("range", range),
            ("mass", 2.0),
            ("warmup", 1.0),
            ("cooldown", 1.5),
            ("burst", 1.0),
        ]),
    }
}

fn value(p: &ConversionPrediction, stat: &str) -> f64 {
    p.stats.get(stat).unwrap().value
}

#[test]
fn a_design_is_classed_with_the_conversions_that_share_its_rare_tags() {
    let set = families();
    let opts = EstimateOptions::default();
    // The shared tag counts for nothing; the rifle tag decides, so the estimate follows the rifles.
    let rifle = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &opts,
    );
    assert!(value(&rifle, "range") > 50.0, "{rifle:?}");
    let pistol = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Pistol"], None, 3, 26.0),
        &opts,
    );
    assert!(value(&pistol, "range") < 20.0, "{pistol:?}");
}

#[test]
fn a_tag_every_conversion_carries_does_not_separate_classes() {
    let set = families();
    let opts = EstimateOptions::default();
    // Only the common tag: the design is equally close to both families, so the class mixes them and the
    // estimate is not simply one family's number.
    let p = estimate_conversion(&set, &profile(&["RS_Gun"], None, 3, 30.0), &opts);
    let r = value(&p, "range");
    assert!(r > 12.0 && r < 60.0, "{r}");
}

#[test]
fn a_named_tag_class_outweighs_a_misleading_tag() {
    let set = families();
    let opts = EstimateOptions::default();
    // The tags say pistol, the user says the long class: the role distance (1.0) beats the tag distance.
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Pistol"], Some("RS_AI_Long"), 3, 30.0),
        &opts,
    );
    assert!(
        p.level != ChainLevel::All || value(&p, "range") > 30.0,
        "{p:?}"
    );
    let both = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], Some("RS_AI_Long"), 3, 30.0),
        &opts,
    );
    assert_eq!(both.level, ChainLevel::RoleTier);
}

#[test]
fn verb_shape_separates_burst_weapons_from_single_shot_weapons() {
    let mut examples = Vec::new();
    for i in 0..4u32 {
        let f = f64::from(i);
        let mut single = example(
            &format!("RS_Single{i}"),
            &["RS_Gun"],
            None,
            3,
            30.0 + f,
            50.0 + f,
        );
        single.stats.insert("recoil".into(), 2.0);
        let mut burst = example(
            &format!("RS_Burst{i}"),
            &["RS_Gun"],
            None,
            3,
            30.0 + f,
            50.0 + f,
        );
        burst.vanilla.insert("burst".into(), 3.0);
        burst.stats.insert("recoil".into(), 1.0);
        examples.push(single);
        examples.push(burst);
    }
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    };
    let opts = EstimateOptions::default();
    let mut p = profile(&["RS_Gun"], None, 3, 31.0);
    p.vanilla.insert("burst".into(), 3.0);
    let burst = estimate_conversion(&set, &p, &opts);
    assert!((value(&burst, "recoil") - 1.0).abs() < 1e-9);
    p.vanilla.insert("burst".into(), 1.0);
    let single = estimate_conversion(&set, &p, &opts);
    assert!((value(&single, "recoil") - 2.0).abs() < 1e-9);
}

#[test]
fn the_closer_tier_wins_when_tags_do_not_tell() {
    let mut examples = Vec::new();
    for i in 0..4u32 {
        let f = f64::from(i);
        let mut low = example(&format!("RS_Low{i}"), &["RS_Gun"], None, 2, 30.0, 40.0);
        low.stats.insert("sway".into(), 2.0 + 0.01 * f);
        let mut high = example(&format!("RS_High{i}"), &["RS_Gun"], None, 5, 30.0, 40.0);
        high.stats.insert("sway".into(), 1.0 + 0.01 * f);
        examples.push(low);
        examples.push(high);
    }
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    };
    let opts = EstimateOptions::default();
    let near_high = estimate_conversion(&set, &profile(&["RS_Gun"], None, 5, 30.0), &opts);
    assert!(value(&near_high, "sway") < 1.1);
    let near_low = estimate_conversion(&set, &profile(&["RS_Gun"], None, 2, 30.0), &opts);
    assert!(value(&near_low, "sway") > 1.9);
}

#[test]
fn strength_only_breaks_ties_between_otherwise_equal_classes() {
    // Two families with identical tags, tier and shape; only the vanilla strength differs.
    let mut examples = Vec::new();
    for i in 0..6u32 {
        let f = f64::from(i);
        let mut weak = example(&format!("RS_Weak{i}"), &["RS_Gun"], None, 3, 20.0, 30.0);
        weak.vanilla.insert("cooldown".into(), 3.0);
        weak.stats.insert("sway".into(), 3.0 + 0.001 * f);
        let mut strong = example(&format!("RS_Strong{i}"), &["RS_Gun"], None, 3, 60.0, 90.0);
        strong.vanilla.insert("cooldown".into(), 0.5);
        strong.stats.insert("sway".into(), 1.0 + 0.001 * f);
        examples.push(weak);
        examples.push(strong);
    }
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    };
    let opts = EstimateOptions::default();
    let mut strong_design = profile(&["RS_Gun"], None, 3, 60.0);
    strong_design.vanilla.insert("cooldown".into(), 0.5);
    let p = estimate_conversion(&set, &strong_design, &opts);
    assert!(value(&p, "sway") < 1.1, "{p:?}");
    // A different tag outweighs the tie break entirely.
    let mut weak_design = profile(&["RS_Gun", "RS_Odd"], None, 3, 20.0);
    weak_design.vanilla.insert("cooldown".into(), 3.0);
    let q = estimate_conversion(&set, &weak_design, &opts);
    assert!(value(&q, "sway") > 2.9, "{q:?}");
}

#[test]
fn a_stat_that_fewer_conversions_carry_than_the_minimum_class_is_not_predicted() {
    let mut set = families();
    for e in &mut set.examples {
        e.stats.remove("sway");
    }
    // Two conversions carry a number for it: below the minimum class, so it is withheld with a reason.
    for e in set.examples.iter_mut().take(2) {
        e.stats.insert("sway".into(), 1.1);
    }
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    assert!(!p.stats.contains_key("sway"));
    let why = p.withheld.get("sway").unwrap();
    assert!(why.contains("only 2 of 12"), "{why}");
}

#[test]
fn a_pool_smaller_than_the_minimum_class_predicts_nothing_and_says_so() {
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples: families().examples.into_iter().take(2).collect(),
    };
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun"], None, 3, 30.0),
        &EstimateOptions::default(),
    );
    assert!(p.stats.is_empty(), "{p:?}");
    assert!(p.notes.iter().any(|n| n.contains("at least 3")));
}

#[test]
fn too_few_conversions_leave_a_stat_unmeasured_and_not_usable() {
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples: families().examples.into_iter().take(4).collect(),
    };
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    let range = p.stats.get("range").unwrap();
    assert_eq!(range.reliability, Reliability::Unmeasured);
    assert!(!range.is_usable());
    assert!(range.loo_error.is_none());
    assert!(range.accuracy_note().contains("too few"));
}

#[rstest]
#[case(Some(0.05), Some(0.2), 8, Reliability::Reliable)]
#[case(Some(0.149), Some(0.2), 8, Reliability::Reliable)]
#[case(Some(0.15), Some(0.2), 8, Reliability::Rough)]
#[case(Some(0.349), Some(0.5), 8, Reliability::Rough)]
#[case(Some(0.35), Some(0.5), 8, Reliability::Unreliable)]
#[case(Some(0.05), Some(0.8), 8, Reliability::Unreliable)]
#[case(Some(0.05), Some(0.2), 4, Reliability::Unmeasured)]
#[case(None, None, 8, Reliability::Unmeasured)]
#[case(Some(f64::NAN), Some(0.2), 8, Reliability::Unmeasured)]
fn ratings_follow_the_median_and_the_worst_case(
    #[case] median: Option<f64>,
    #[case] p80: Option<f64>,
    #[case] folds: usize,
    #[case] want: Reliability,
) {
    assert_eq!(EstimateOptions::default().rate(median, p80, folds), want);
}

#[test]
fn only_reliable_and_rough_ratings_may_be_written() {
    assert!(Reliability::Reliable.is_usable());
    assert!(Reliability::Rough.is_usable());
    assert!(!Reliability::Unreliable.is_usable());
    assert!(!Reliability::Unmeasured.is_usable());
}

#[test]
fn a_stat_with_noisy_conversions_is_rated_unreliable() {
    let mut set = families();
    // The sway of every conversion is unrelated to its class.
    let noise = [0.4, 3.1, 0.9, 2.2, 0.5, 2.9, 1.1, 0.3, 2.6, 1.7, 0.6, 3.3];
    for (e, n) in set.examples.iter_mut().zip(noise) {
        e.stats.insert("sway".into(), n);
    }
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    let sway = p.stats.get("sway").unwrap();
    assert_eq!(sway.reliability, Reliability::Unreliable, "{sway:?}");
    assert!(!sway.is_usable());
    assert!(sway.accuracy_note().contains("unreliable"));
    // A stat that is constant in its class is rated reliable.
    assert_eq!(
        p.stats.get("warmup").unwrap().reliability,
        Reliability::Reliable
    );
}

#[test]
fn identity_wins_when_the_vanilla_number_is_carried_over_exactly() {
    let p = estimate_conversion(
        &families(),
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    let mass = p.stats.get("mass").unwrap();
    assert_eq!(mass.predictor, ConversionPredictor::Identity);
    assert!((mass.value - 2.0).abs() < 1e-12);
    assert_eq!(mass.reliability, Reliability::Reliable);
    assert_eq!(mass.loo_error, Some(0.0));
}

#[test]
fn a_class_ratio_keeps_the_vanilla_number_in_the_estimate() {
    // Every conversion doubles the vanilla range, whatever the range is: the ratio form is exact and the
    // class median (which ignores the vanilla number) is not.
    let examples = (0..8u32)
        .map(|i| {
            let v = 15.0 + 7.0 * f64::from(i);
            example(&format!("RS_Gun{i}"), &["RS_Gun"], None, 3, v, 2.0 * v)
        })
        .collect();
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    };
    let opts = EstimateOptions::default();
    let short = estimate_conversion(&set, &profile(&["RS_Gun"], None, 3, 30.0), &opts);
    let long = estimate_conversion(&set, &profile(&["RS_Gun"], None, 3, 50.0), &opts);
    assert_eq!(
        short.stats.get("range").unwrap().predictor,
        ConversionPredictor::Ratio
    );
    // The same class, a longer vanilla range: the estimate scales with it.
    let scale = value(&long, "range") / value(&short, "range");
    assert!((scale - 50.0 / 30.0).abs() < 1e-9, "{scale}");
    assert!((value(&short, "range") - 60.0).abs() < 1e-6);
}

fn view_of<'a>(set: &'a ExampleSet, stat: &'a str) -> StatView<'a> {
    StatView {
        set: &set.examples,
        stat,
        driver: Some(stat),
    }
}

fn estimator<'a>(set: &'a ExampleSet, opts: &'a EstimateOptions) -> Estimator<'a> {
    let space = Space::new(&set.examples);
    let feats: Vec<Features> = set
        .examples
        .iter()
        .map(|e| space.features(e.tier, &e.tags, e.role.as_ref(), &e.vanilla))
        .collect();
    let fold_orders = feats
        .iter()
        .enumerate()
        .map(|(i, f)| neighbours(&space, &set.examples, &feats, f, Some(i), opts))
        .collect();
    Estimator {
        examples: &set.examples,
        fold_orders,
        opts,
    }
}

#[test]
fn a_class_ratio_is_shrunk_toward_the_ratio_of_the_whole_pool() {
    // Three members at ratio 2 and nine other conversions at ratio 1.
    let mut examples = Vec::new();
    for i in 0..3u32 {
        examples.push(example(
            &format!("RS_Own{i}"),
            &["RS_Own"],
            None,
            3,
            20.0,
            40.0,
        ));
    }
    for i in 0..9u32 {
        examples.push(example(
            &format!("RS_Other{i}"),
            &["RS_Other"],
            None,
            3,
            20.0,
            20.0,
        ));
    }
    let set = ExampleSet {
        kind: ItemKind::Ranged,
        examples,
    };
    let opts = EstimateOptions {
        class_size: 3,
        ..EstimateOptions::default()
    };
    let est = estimator(&set, &opts);
    let space = Space::new(&set.examples);
    let target = space.features(Some(3), &tags(&["RS_Own"]), None, &BTreeMap::new());
    let order = neighbours(
        &space,
        &set.examples,
        &est_feats(&set, &space),
        &target,
        None,
        &opts,
    );
    let (estimate, n) = est
        .estimate(
            Form::Ratio,
            &view_of(&set, "range"),
            &order,
            None,
            Some(10.0),
        )
        .unwrap();
    assert_eq!(n, 3);
    // Global median ratio is 1 (nine of twelve), class median ratio is 2: shrunk with strength 2.
    let expected = 10.0 * ((3.0 * 2f64.ln() + 2.0 * 1f64.ln()) / 5.0).exp();
    assert!(
        (estimate - expected).abs() < 1e-9,
        "{estimate} vs {expected}"
    );
    // No shrinkage keeps the class ratio, infinite shrinkage the global one.
    let none = EstimateOptions {
        shrink: 0.0,
        ..opts.clone()
    };
    let e0 = estimator(&set, &none)
        .estimate(
            Form::Ratio,
            &view_of(&set, "range"),
            &order,
            None,
            Some(10.0),
        )
        .unwrap()
        .0;
    assert!((e0 - 20.0).abs() < 1e-9);
    let heavy = EstimateOptions {
        shrink: 1.0e9,
        ..opts
    };
    let e1 = estimator(&set, &heavy)
        .estimate(
            Form::Ratio,
            &view_of(&set, "range"),
            &order,
            None,
            Some(10.0),
        )
        .unwrap()
        .0;
    assert!((e1 - 10.0).abs() < 1e-4);
}

fn est_feats(set: &ExampleSet, space: &Space) -> Vec<Features> {
    set.examples
        .iter()
        .map(|e| space.features(e.tier, &e.tags, e.role.as_ref(), &e.vanilla))
        .collect()
}

#[test]
fn a_ratio_class_below_the_minimum_size_is_never_used() {
    // Only two conversions carry a vanilla twin value for range.
    let mut set = families();
    for e in set.examples.iter_mut().skip(2) {
        e.vanilla.remove("range");
    }
    let opts = EstimateOptions::default();
    let est = estimator(&set, &opts);
    let order: Vec<usize> = (0..set.examples.len()).collect();
    assert!(
        est.estimate(
            Form::Ratio,
            &view_of(&set, "range"),
            &order,
            None,
            Some(30.0)
        )
        .is_none()
    );
}

#[test]
fn the_estimate_does_not_depend_on_the_order_of_the_examples() {
    let set = families();
    let mut reversed = set.clone();
    reversed.examples.reverse();
    let opts = EstimateOptions::default();
    let p = profile(&["RS_Gun", "RS_Rifle"], Some("RS_AI_Long"), 3, 41.0);
    assert_eq!(
        estimate_conversion(&set, &p, &opts),
        estimate_conversion(&reversed, &p, &opts)
    );
    assert_eq!(
        estimate_conversion(&set, &p, &opts),
        estimate_conversion(&set, &p, &opts)
    );
}

#[test]
fn leave_one_out_never_sees_the_example_it_predicts() {
    let mut set = families();
    // An outlier whose own number would drag any class it belongs to.
    set.examples.push(example(
        "RS_Outlier",
        &["RS_Gun", "RS_Rifle"],
        Some("RS_AI_Long"),
        3,
        40.0,
        4000.0,
    ));
    let opts = EstimateOptions::default();
    let mut seen = 0;
    leave_one_out(&set, &opts, true, |held, prediction| {
        seen += 1;
        let rest = set.without(&held.id);
        let expected = estimate_conversion(&rest, &profile_of(held, true), &opts);
        assert_eq!(*prediction, expected, "{}", held.id);
        if held.id == "RS_Outlier" {
            // Predicted from the other rifles: nowhere near its own 4000.
            assert!(value(prediction, "range") < 200.0);
        }
    });
    assert_eq!(seen, set.len());
}

#[test]
fn examples_without_a_twin_carry_the_converted_tags_without_the_prefix() {
    let model = crate::ce::reader::CeModel::absent("test");
    assert!(ExampleSet::from_model(&model, ItemKind::Ranged).is_empty());
    assert!(ExampleSet::from_model(&model, ItemKind::Apparel).is_empty());
    assert_eq!(
        strip_prefix_tags(&tags(&["RS_Gun", "CE_AI_X", "CE_Y"]), "CE_"),
        ["RS_Gun"]
    );
    assert_eq!(strip_prefix_tags(&tags(&["A", "B"]), ""), ["A", "B"]);
}

#[test]
fn examples_from_a_model_carry_the_twin_tags_and_numbers() {
    use crate::ce::reader::fixtures_tests::{Build, load_set, rows};
    use crate::ce::reader::{CeReadOptions, read_conversions_with};
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows,
        Build {
            ce: false,
            ..Build::default()
        },
    );
    let model = read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            vanilla: Some(&plain.databases),
            ..CeReadOptions::default()
        },
    );
    let set = ExampleSet::from_model(&model, ItemKind::Ranged);
    assert!(!set.is_empty());
    for e in &set.examples {
        assert!(!e.vanilla.is_empty(), "{}", e.id);
        assert!(!e.tags.is_empty(), "{}", e.id);
        assert!(e.tags.iter().all(|t| !t.starts_with("CE_")), "{:?}", e.tags);
        assert!(e.stats.contains_key("range"));
    }
    // A design built from an example's twin predicts something for every stat the conversions carry.
    let held = set.examples.first().unwrap();
    let p = estimate_conversion(
        &set.without(&held.id),
        &profile_of(held, true),
        &EstimateOptions::default(),
    );
    assert!(p.stats.contains_key("range"));
}

#[test]
fn stats_the_ammo_supplies_are_not_estimated() {
    let mut set = families();
    for e in &mut set.examples {
        e.stats.insert("damage".into(), 12.0);
        e.stats.insert("ap_sharp".into(), 3.0);
    }
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    assert!(!p.stats.contains_key("damage"));
    assert!(!p.stats.contains_key("ap_sharp"));
    assert!(p.stats.contains_key("range"));
}

#[test]
fn whole_number_stats_are_rounded_and_at_least_one() {
    let mut set = families();
    for e in &mut set.examples {
        e.stats.insert("magazine".into(), 24.4);
    }
    let p = estimate_conversion(
        &set,
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    assert!((value(&p, "magazine") - 24.0).abs() < 1e-12);
}

#[test]
fn the_band_comes_from_the_leave_one_out_residuals() {
    let p = estimate_conversion(
        &families(),
        &profile(&["RS_Gun", "RS_Rifle"], None, 3, 40.0),
        &EstimateOptions::default(),
    );
    // A stat that is constant in its class has residuals of zero: the narrowest possible band.
    let warmup = p.stats.get("warmup").unwrap();
    assert!((warmup.band.p50 - 1.0).abs() < 1e-9);
    // A noisy class gives a wider band than that.
    let range = p.stats.get("range").unwrap();
    assert!(range.band.p80 >= warmup.band.p80);
}

proptest::proptest! {
    #[test]
    fn estimates_are_finite_and_deterministic_for_any_numbers(
        values in proptest::collection::vec(0.001f64..1000.0, 12),
        range in 0.5f64..500.0,
    ) {
        let mut set = families();
        for (e, v) in set.examples.iter_mut().zip(&values) {
            e.stats.insert("sway".into(), *v);
            e.stats.insert("range".into(), *v);
        }
        let opts = EstimateOptions::default();
        let p = profile(&["RS_Gun", "RS_Rifle"], None, 3, range);
        let a = estimate_conversion(&set, &p, &opts);
        proptest::prop_assert_eq!(&a, &estimate_conversion(&set, &p, &opts));
        for s in a.stats.values() {
            proptest::prop_assert!(s.value.is_finite());
            proptest::prop_assert!(s.band.p80 >= s.band.p50 && s.band.p50 >= 1.0);
        }
    }
}
