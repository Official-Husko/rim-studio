//! Boot: roots, the portable marker, the settings file, the crash marker.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::sync::Arc;

use camino::Utf8PathBuf;
use common::platform;
use rimstudio_app::context::Platform;
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootError, BootInput, boot};
use rimstudio_core::os::Os;
use rimstudio_io::roots::DataRoots;
use rimstudio_testing::fakes::{FakeClock, FakeEnv};
use serde_json::json;

fn base() -> (tempfile::TempDir, Utf8PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    (tmp, base)
}

fn input(base: &Utf8PathBuf, clock: &FakeClock) -> BootInput {
    BootInput::new(platform(base, clock))
        .with_roots(DataRoots::under_base(&base.join("app")))
        .with_log(LogConfig::off())
}

#[test]
fn the_portable_marker_beside_the_executable_selects_the_folders_next_to_it() {
    let (_tmp, base) = base();
    let exe = base.join("exe");
    std::fs::create_dir_all(exe.as_std_path()).unwrap();
    std::fs::write(exe.join("rimstudio.portable").as_std_path(), b"").unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let mut p: Platform = platform(&base, &clock);
    p.env = Arc::new(
        FakeEnv::new()
            .with_home(base.join("home"))
            .with_exe_dir(exe.clone()),
    );
    let app = boot(BootInput::new(p).with_log(LogConfig::off())).unwrap();
    assert!(app.roots.portable);
    assert_eq!(app.roots.config, exe.join("data/config"));
    assert_eq!(app.roots.logs, exe.join("data/logs"));
    assert!(app.boot.portable);
    assert!(exe.join("data/config/settings.jsonc").is_file());
}

#[test]
fn without_the_marker_the_standard_folders_of_the_platform_are_used() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let mut p = platform(&base, &clock);
    p.os = Os::Linux;
    p.env = Arc::new(
        FakeEnv::new()
            .with_home(base.join("home"))
            .with_exe_dir(base.join("no_marker_here"))
            .with_var("XDG_CONFIG_HOME", base.join("xdg/config").as_str())
            .with_var("XDG_DATA_HOME", base.join("xdg/data").as_str())
            .with_var("XDG_CACHE_HOME", base.join("xdg/cache").as_str())
            .with_var("XDG_STATE_HOME", base.join("xdg/state").as_str()),
    );
    let app = boot(BootInput::new(p).with_log(LogConfig::off())).unwrap();
    assert!(!app.roots.portable);
    assert!(
        app.roots.config.starts_with(base.join("xdg/config")),
        "{}",
        app.roots.config
    );
    assert!(
        app.roots.logs.starts_with(base.join("xdg")),
        "{}",
        app.roots.logs
    );
}

#[test]
fn a_machine_without_a_home_folder_cannot_boot_and_says_so() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let mut p = platform(&base, &clock);
    p.env = Arc::new(FakeEnv::new());
    let e = boot(BootInput::new(p).with_log(LogConfig::off())).unwrap_err();
    assert!(matches!(e, BootError::NoDataDir), "{e}");
    assert_eq!(e.code(), "boot.no-data-dir");
}

#[test]
fn the_first_run_writes_the_commented_settings_file() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let app = boot(input(&base, &clock)).unwrap();
    assert!(app.boot.settings_created);
    let text = std::fs::read_to_string(base.join("app/config/settings.jsonc")).unwrap();
    assert!(text.contains("// RimStudio settings."));
    let second = boot(input(&base, &clock)).unwrap();
    assert!(!second.boot.settings_created);
}

#[test]
fn a_damaged_settings_file_boots_on_defaults_and_stays_untouched() {
    let (_tmp, base) = base();
    let cfg = base.join("app/config");
    std::fs::create_dir_all(cfg.as_std_path()).unwrap();
    let broken = "{ \"appearance\": { \"density\": ";
    std::fs::write(cfg.join("settings.jsonc").as_std_path(), broken).unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let app = boot(input(&base, &clock)).unwrap();
    assert!(app.boot.settings_problem.is_some());
    assert_eq!(
        std::fs::read_to_string(cfg.join("settings.jsonc")).unwrap(),
        broken
    );
    let e = rimstudio_app::dispatch(
        &app,
        "settings_update",
        json!({"appearance": {"density": "compact"}}),
    );
    assert!(
        e.is_err(),
        "a file that cannot be read faithfully is never overwritten"
    );
    assert_eq!(
        std::fs::read_to_string(cfg.join("settings.jsonc")).unwrap(),
        broken
    );
}

#[test]
fn a_root_that_cannot_be_created_is_a_boot_error() {
    let (_tmp, base) = base();
    let blocked = base.join("app");
    std::fs::create_dir_all(blocked.as_std_path()).unwrap();
    std::fs::write(
        blocked.join("config").as_std_path(),
        b"a file where a folder belongs",
    )
    .unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let e = boot(input(&base, &clock)).unwrap_err();
    assert!(matches!(e, BootError::Roots(_)), "{e}");
}

#[test]
fn a_run_that_never_shut_down_is_noticed_by_the_next_boot() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let first = boot(input(&base, &clock)).unwrap();
    assert!(!first.boot.previous_run_ended_abnormally);
    let first_session = first.boot.session_id.clone();
    drop(first);
    let second = boot(input(&base, &clock)).unwrap();
    assert!(second.boot.previous_run_ended_abnormally);
    assert_eq!(
        second.boot.previous_session.as_deref(),
        Some(first_session.as_str())
    );
}

#[test]
fn a_clean_shutdown_clears_the_notice() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let first = boot(input(&base, &clock)).unwrap();
    assert!(first.shutdown(std::time::Duration::from_secs(5)));
    drop(first);
    let second = boot(input(&base, &clock)).unwrap();
    assert!(!second.boot.previous_run_ended_abnormally);
}

#[test]
fn boot_neither_detects_nor_scans() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let app = boot(input(&base, &clock)).unwrap();
    assert!(app.library.current().is_none());
    assert!(!app.workspace.is_built());
    assert!(!base.join("app/cache/detection-report.json").exists());
    let info = rimstudio_app::dispatch(&app, "app_get_info", json!({})).unwrap();
    assert_eq!(info["previousRunEndedAbnormally"], json!(false));
}

#[test]
fn sessions_have_distinct_ids() {
    let (_tmp, base) = base();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let a = boot(input(&base, &clock)).unwrap();
    clock.advance(5);
    let b = boot(input(&base, &clock)).unwrap();
    assert_ne!(a.boot.session_id, b.boot.session_id);
}
