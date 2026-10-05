//! The calibration job: determinism at any thread count (IT-071), the cache, cancellation and keys.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_core::jobs::{CancelToken, CollectingProgress, NoopProgress};
use rimstudio_ipc_types::designer::{CalibrateResultDto, DesignerCalibrateRequest, ItemKindDto};
use rimstudio_toolkit::designer::calibrate::{CalibrationState, calibration_state};
use rimstudio_toolkit::designer::{Ctx, calibrate};

fn request(kind: ItemKindDto, force: bool, threads: Option<u8>) -> DesignerCalibrateRequest {
    DesignerCalibrateRequest {
        kind,
        force,
        threads,
    }
}

fn run(ctx: &Ctx, req: DesignerCalibrateRequest) -> CalibrateResultDto {
    calibrate(ctx, req, &NoopProgress, &CancelToken::new()).unwrap()
}

#[test]
fn the_result_is_identical_at_one_and_eight_threads() {
    let f = common::fixture(false);
    let one = run(&f.ctx, request(ItemKindDto::Ranged, true, Some(1)));
    let eight = run(&f.ctx, request(ItemKindDto::Ranged, true, Some(8)));
    assert!(!one.from_cache && !eight.from_cache);
    assert_eq!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&eight).unwrap()
    );
    assert_eq!(one.pool_size, 15);
    assert!(one.variants.contains_key("noisy"));
    assert!(one.variants.contains_key("simple"));
    assert_eq!(
        one.calibrated_at_ms,
        Some(rimstudio_testing::fakes::FakeClock::DEFAULT_START_MS)
    );
}

#[test]
fn the_second_call_is_served_from_the_cache_with_identical_numbers() {
    let f = common::fixture(false);
    let first = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    assert!(!first.from_cache);
    let second = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    assert!(second.from_cache);
    let mut expected = first.clone();
    expected.from_cache = true;
    assert_eq!(second, expected);
}

#[test]
fn a_new_context_reads_the_cache_from_disk() {
    let f = common::fixture(false);
    let first = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    let again = Ctx::new(
        f.session.clone(),
        &f.roots,
        std::sync::Arc::new(f.clock.clone()),
    )
    .unwrap();
    let engine = again.require_engine().unwrap();
    let state =
        calibration_state(&again, &engine, rimstudio_design::classes::ItemKind::Ranged).unwrap();
    assert!(state.metrics().is_some());
    let second = run(&again, request(ItemKindDto::Ranged, false, None));
    assert!(second.from_cache);
    assert_eq!(second.variants, first.variants);
}

#[test]
fn force_recalibrates_even_when_a_fresh_result_exists() {
    let f = common::fixture(false);
    let _ = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    f.clock.advance(60_000);
    let forced = run(&f.ctx, request(ItemKindDto::Ranged, true, None));
    assert!(!forced.from_cache);
    assert_eq!(
        forced.calibrated_at_ms,
        Some(rimstudio_testing::fakes::FakeClock::DEFAULT_START_MS + 60_000)
    );
}

#[test]
fn a_changed_reference_set_changes_the_key_and_marks_the_old_result_stale() {
    let f = common::fixture(false);
    let _ = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    // the same install without the extra mod: one weapon fewer
    let fewer = f
        .ctx
        .with_session(common::open_session_with(&f.install, &["ludeon.rimworld"]));
    let engine = fewer.require_engine().unwrap();
    let state =
        calibration_state(&fewer, &engine, rimstudio_design::classes::ItemKind::Ranged).unwrap();
    assert!(matches!(state, CalibrationState::Stale), "{state:?}");
    let result = run(&fewer, request(ItemKindDto::Ranged, false, None));
    assert!(!result.from_cache);
    assert_eq!(result.pool_size, 14);
    // both results are kept side by side
    let original = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    assert!(original.from_cache);
    assert_eq!(original.pool_size, 15);
}

#[test]
fn melee_calibrates_on_its_own_pool() {
    let f = common::fixture(false);
    let result = run(&f.ctx, request(ItemKindDto::Melee, false, None));
    assert_eq!(result.kind, ItemKindDto::Melee);
    assert_eq!(result.pool_size, 8);
}

#[test]
fn a_cancelled_job_stores_nothing() {
    let f = common::fixture(false);
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = calibrate(
        &f.ctx,
        request(ItemKindDto::Ranged, false, None),
        &NoopProgress,
        &cancel,
    )
    .unwrap_err();
    assert_eq!(err.code(), "job.cancelled");
    let engine = f.ctx.require_engine().unwrap();
    let state =
        calibration_state(&f.ctx, &engine, rimstudio_design::classes::ItemKind::Ranged).unwrap();
    assert!(matches!(state, CalibrationState::Missing));
}

#[test]
fn progress_is_reported_in_steps() {
    let f = common::fixture(false);
    let progress = CollectingProgress::new();
    calibrate(
        &f.ctx,
        request(ItemKindDto::Ranged, false, Some(1)),
        &progress,
        &CancelToken::new(),
    )
    .unwrap();
    let phases: Vec<String> = progress.records().into_iter().map(|p| p.phase).collect();
    assert_eq!(phases, vec!["pool", "harness", "store", "done"]);
    assert_eq!(progress.last().and_then(|p| p.total), Some(3));
}

#[test]
fn a_pool_that_is_too_small_cannot_be_calibrated() {
    let install = common::install_custom(3, 8, false, false);
    let session = common::open_session_with(&install, &["ludeon.rimworld"]);
    let tmp = tempfile::tempdir().unwrap();
    let base = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = rimstudio_io::roots::DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session,
        &roots,
        std::sync::Arc::new(rimstudio_testing::fakes::FakeClock::new(1_700_000_000_000)),
    )
    .unwrap();
    let err = calibrate(
        &ctx,
        request(ItemKindDto::Ranged, false, None),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.calibration-unavailable");
}

#[test]
fn calibration_needs_a_game_install() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = calibrate(
        &ctx,
        request(ItemKindDto::Ranged, false, None),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn the_preview_reports_a_stale_calibration() {
    let f = common::fixture(false);
    let _ = run(&f.ctx, request(ItemKindDto::Ranged, false, None));
    let fewer = f
        .ctx
        .with_session(common::open_session_with(&f.install, &["ludeon.rimworld"]));
    let p = rimstudio_toolkit::designer::preview(
        &fewer,
        rimstudio_ipc_types::designer::DesignerPreviewRequest {
            draft: common::dto(common::typed_ranged()),
        },
    )
    .unwrap();
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == "design.calibration-stale")
    );
    let fresh = rimstudio_toolkit::designer::preview(
        &f.ctx,
        rimstudio_ipc_types::designer::DesignerPreviewRequest {
            draft: common::dto(common::typed_ranged()),
        },
    )
    .unwrap();
    assert!(
        fresh
            .diagnostics
            .iter()
            .all(|d| d.code != "design.calibration-stale")
    );
}

#[test]
fn a_role_and_tier_held_by_a_single_weapon_do_not_break_the_harness() {
    use rimstudio_testing::install_tree::InstallBuilder;

    // The real install has such a weapon (a lone neolithic bow): when it is held out, the training pool
    // has neither its role nor its tier, and the simulated modder has to say "not sure".
    let mut defs = common::core_defs_n(10, 0);
    defs.push(common::gun("RS_Odd", "RS_Shot00", 3, "Neolithic", "RS_Odd"));
    let install = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .build_temp()
        .unwrap();
    let session = common::open_session_with(&install, &["ludeon.rimworld"]);
    let tmp = tempfile::tempdir().unwrap();
    let base = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = rimstudio_io::roots::DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session,
        &roots,
        std::sync::Arc::new(rimstudio_testing::fakes::FakeClock::new(1_700_000_000_000)),
    )
    .unwrap();
    let result = run(&ctx, request(ItemKindDto::Ranged, false, Some(2)));
    assert_eq!(result.pool_size, 11);
}
