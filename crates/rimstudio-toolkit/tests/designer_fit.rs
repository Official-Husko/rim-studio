//! The fit meter through the toolkit.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::model::{ScalarField, ValueSource};
use rimstudio_ipc_types::designer::{
    DesignerCalibrateRequest, DesignerFitRequest, FitLevelDto, FitReportDto, ItemKindDto,
};
use rimstudio_toolkit::designer::{calibrate, fit};

fn score(f: &common::Fixture, spec: rimstudio_design::model::DesignSpec) -> FitReportDto {
    fit(
        &f.ctx,
        DesignerFitRequest {
            draft: common::dto(spec),
        },
    )
    .unwrap()
}

fn level(r: &FitReportDto, stat: &str) -> Option<FitLevelDto> {
    r.per_stat.iter().find(|s| s.stat == stat).map(|s| s.level)
}

#[test]
fn numbers_in_the_middle_of_the_class_are_typical() {
    let f = common::fixture(false);
    let r = score(&f, common::typed_ranged());
    assert_eq!(level(&r, "damage"), Some(FitLevelDto::Typical));
    assert_eq!(level(&r, "mass"), Some(FitLevelDto::Typical));
    assert!(r.summary.scored >= 8);
    assert_eq!(
        r.summary.typical + r.summary.plausible + r.summary.unusual,
        r.summary.scored
    );
    assert!(r.summary.share_in_p80 > 0.5);
    assert!(r.typicality.is_some());
    assert!(r.per_stat.windows(2).all(|w| w[0].stat <= w[1].stat));
}

#[test]
fn a_wild_value_is_unusual_and_the_value_is_not_changed() {
    let f = common::fixture(false);
    let mut spec = common::typed_ranged();
    spec.offer(ScalarField::Damage, 400.0, ValueSource::Typed);
    let r = score(&f, spec);
    let damage = r.per_stat.iter().find(|s| s.stat == "damage").unwrap();
    assert_eq!(damage.level, FitLevelDto::Unusual);
    assert!((damage.value - 400.0).abs() < 1e-12);
    assert!(damage.rank.below == damage.rank.total);
    assert!(damage.nearest_reference < 400.0);
}

#[test]
fn without_a_calibration_the_default_bands_are_flagged_and_there_is_no_date() {
    let f = common::fixture(false);
    let r = score(&f, common::typed_ranged());
    assert!(r.per_stat.iter().all(|s| s.default_band));
    assert_eq!(r.notices.calibrated_at_ms, None);
    assert!(r.notices.rough, "seven reference weapons are a rough pool");
    assert_eq!(r.notices.class_n, 7);
    assert!(r.notices.class_label.starts_with("based on 7 items"));
}

#[test]
fn after_a_calibration_the_bands_come_from_it() {
    let f = common::fixture(false);
    let result = calibrate(
        &f.ctx,
        DesignerCalibrateRequest {
            kind: ItemKindDto::Ranged,
            force: false,
            threads: Some(2),
        },
        &rimstudio_core::jobs::NoopProgress,
        &rimstudio_core::jobs::CancelToken::new(),
    )
    .unwrap();
    let r = score(&f, common::typed_ranged());
    assert_eq!(r.notices.calibrated_at_ms, result.calibrated_at_ms);
    assert!(r.notices.calibrated_at_ms.is_some());
    assert!(r.per_stat.iter().any(|s| !s.default_band));
    assert_eq!(r.notices.pool_size, 15);
}

#[test]
fn an_empty_draft_scores_nothing_and_lists_nothing_unscored() {
    let f = common::fixture(false);
    let r = score(&f, common::ranged_spec());
    assert!(r.per_stat.iter().all(|s| s.stat != "damage"));
    assert_eq!(r.summary.scored as usize, r.per_stat.len());
}

#[test]
fn the_fit_needs_a_game_install() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = fit(
        &ctx,
        DesignerFitRequest {
            draft: common::dto(common::typed_ranged()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn melee_designs_are_scored_against_the_melee_pool() {
    let f = common::fixture(false);
    let mut spec = common::melee_spec();
    spec.tools = vec![
        rimstudio_design::model::ToolSpec::new("head", &["Cut"]).with_numbers(
            9.0,
            2.0,
            ValueSource::Typed,
        ),
    ];
    spec.offer(ScalarField::Mass, 1.2, ValueSource::Typed);
    let r = score(&f, spec);
    assert!(r.per_stat.iter().any(|s| s.stat == "swing_damage"));
    assert!(r.per_stat.iter().all(|s| s.stat != "tools"));
    assert_eq!(r.notices.pool_size, 8);
}
