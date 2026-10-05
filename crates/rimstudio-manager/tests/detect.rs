//! Detection use cases: run, cached report and overrides.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Harness;
use rimstudio_manager::detect::{
    DetectGetReportRequest, DetectRunRequest, DetectSetOverrideRequest, OverrideField, get_report,
    run, set_override,
};
use rimstudio_manager::error::ManagerError;
use rimstudio_steam::report::InstallKind;

fn request(field: OverrideField, path: Option<String>) -> DetectSetOverrideRequest {
    DetectSetOverrideRequest {
        field,
        path,
        pinned: true,
    }
}

#[test]
fn detection_without_overrides_finds_nothing_on_an_empty_machine() {
    let h = Harness::bare();
    let out = run(&h.ctx(), DetectRunRequest::default()).unwrap();
    assert!(out.report.installs.is_empty());
    assert_eq!(out.selected_install, None);
    assert!(out.overrides.is_empty());
    assert!(out.cached);
}

#[test]
fn a_game_override_with_core_content_is_accepted_selected_and_stored() {
    let h = Harness::bare();
    let out = set_override(
        &h.ctx(),
        request(
            OverrideField::GameInstall,
            Some(h.install.game_dir.to_string()),
        ),
    )
    .unwrap();
    let install = out.report.selected_install().unwrap();
    assert_eq!(install.kind, InstallKind::Override);
    assert_eq!(install.game_root, h.install.game_dir);
    assert_eq!(out.selected_install.as_deref(), Some(install.id.as_str()));
    assert_eq!(out.overrides.len(), 1);
    assert!(out.overrides[0].valid && out.overrides[0].pinned);
    let ws = h.read(&h.workspace_path());
    assert!(ws.contains("gameInstall"));
    assert!(ws.contains("\"pinned\": true"));
}

#[test]
fn a_game_override_without_core_content_is_refused_and_nothing_is_stored() {
    let h = Harness::bare();
    let err = set_override(
        &h.ctx(),
        request(
            OverrideField::GameInstall,
            Some(h.install.mods_dir.to_string()),
        ),
    )
    .unwrap_err();
    match &err {
        ManagerError::OverrideInvalid { field, reason } => {
            assert_eq!(*field, "gameInstall");
            assert!(reason.contains("Data/Core/About/About.xml"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(!h.workspace_path().exists());
}

#[test]
fn bad_paths_are_refused_with_typed_errors() {
    let h = Harness::bare();
    let err = set_override(
        &h.ctx(),
        request(OverrideField::GameInstall, Some("relative/dir".into())),
    )
    .unwrap_err();
    assert_eq!(err.code(), "core.invalid-path");
    let missing = h.install.root.join("does-not-exist");
    let err = set_override(
        &h.ctx(),
        request(OverrideField::UserDir, Some(missing.to_string())),
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.override-invalid");
    let err = set_override(
        &h.ctx(),
        request(
            OverrideField::SteamRoot,
            Some(h.install.mods_dir.to_string()),
        ),
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.override-invalid");
}

#[test]
fn clearing_an_override_removes_it_and_detection_forgets_the_install() {
    let h = Harness::with_overrides();
    let out = set_override(&h.ctx(), request(OverrideField::GameInstall, None)).unwrap();
    assert!(out.report.installs.is_empty());
    assert!(
        out.overrides
            .iter()
            .all(|o| o.field != OverrideField::GameInstall)
    );
    let out = set_override(&h.ctx(), request(OverrideField::WorkshopDir, None)).unwrap();
    assert!(out.overrides.is_empty());
}

#[test]
fn the_report_is_cached_and_served_without_running_detection() {
    let h = Harness::bare();
    let none = get_report(&h.ctx(), DetectGetReportRequest::default()).unwrap();
    assert!(none.report.is_none() && !none.ignored_cache);
    let first = h.pin_install();
    let cached = get_report(&h.ctx(), DetectGetReportRequest::default()).unwrap();
    assert_eq!(cached.report.as_ref(), Some(&first.report));
    assert_eq!(cached.generated_at_ms, Some(first.report.generated_at_ms));
    assert_eq!(cached.selected_install, first.selected_install);
}

#[test]
fn a_damaged_cache_is_ignored_and_reported() {
    let h = Harness::bare();
    run(&h.ctx(), DetectRunRequest::default()).unwrap();
    std::fs::write(h.roots.cache.join("detection-report.json"), "{ not json").unwrap();
    let out = get_report(&h.ctx(), DetectGetReportRequest::default()).unwrap();
    assert!(out.report.is_none());
    assert!(out.ignored_cache);
}

#[test]
fn an_override_that_became_invalid_is_kept_and_flagged() {
    let h = Harness::with_overrides();
    std::fs::remove_dir_all(&h.install.workshop_dir).unwrap();
    let out = run(&h.ctx(), DetectRunRequest::default()).unwrap();
    let workshop = out
        .overrides
        .iter()
        .find(|o| o.field == OverrideField::WorkshopDir)
        .unwrap();
    assert!(!workshop.valid);
    assert!(workshop.reason.is_some());
    assert!(h.read(&h.workspace_path()).contains("extraWorkshopDirs"));
}

#[test]
fn a_damaged_workspace_file_is_never_overwritten_by_an_override() {
    let h = Harness::bare();
    std::fs::create_dir_all(&h.roots.config).unwrap();
    let text = "{ \"paths\": { ";
    std::fs::write(h.workspace_path(), text).unwrap();
    let err = set_override(
        &h.ctx(),
        request(
            OverrideField::GameInstall,
            Some(h.install.game_dir.to_string()),
        ),
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.read-only");
    assert_eq!(h.read(&h.workspace_path()), text);
}

#[test]
fn a_valid_steam_root_override_finds_the_install_the_normal_way() {
    let h = Harness::bare();
    let out = set_override(
        &h.ctx(),
        request(OverrideField::SteamRoot, Some(h.install.root.to_string())),
    )
    .unwrap();
    let install = out.report.selected_install().unwrap();
    assert_eq!(install.kind, InstallKind::Steam);
    assert_eq!(install.workshop.len(), 1);
}

#[test]
fn detection_runs_over_the_fake_environment_presets() {
    use rimstudio_core::os::Os;
    use rimstudio_manager::Ctx;
    use rimstudio_testing::detect_env::{FakeDetectEnv, SteamPreset};

    let h = Harness::bare();
    let env = FakeDetectEnv::from_preset(SteamPreset::LinuxNative);
    let ctx = Ctx::new(&h.roots, &env, &env.fs, &env.clock).with_os(Os::Linux);
    let out = run(&ctx, DetectRunRequest::default()).unwrap();
    assert!(!out.report.installs.is_empty());
    assert!(out.selected_install.is_some());
    let cached = get_report(&ctx, DetectGetReportRequest::default()).unwrap();
    assert_eq!(cached.report.as_ref(), Some(&out.report));
}
