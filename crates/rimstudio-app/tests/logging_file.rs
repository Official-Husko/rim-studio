//! The rolling JSON lines file: one test in its own binary, because installing a global subscriber
//! happens once per process.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use camino::Utf8PathBuf;
use common::platform;
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootInput, boot, dispatch};
use rimstudio_io::roots::DataRoots;
use rimstudio_testing::fakes::FakeClock;
use serde_json::{Value, json};

#[test]
fn failures_reach_a_json_lines_file_without_private_text() {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let logs;
    let error_id;
    {
        let log = LogConfig {
            panic_hook: false,
            ..LogConfig::file_only()
        };
        let app = boot(
            BootInput::new(platform(&base, &clock))
                .with_roots(DataRoots::under_base(&base.join("app")))
                .with_log(log),
        )
        .unwrap();
        assert!(app.log.installed());
        assert!(app.log.has_file());
        logs = app.roots.logs.clone();
        let secret = base.join("home/secret_place");
        let e = dispatch(
            &app,
            "sources_probe_folder",
            json!({"path": format!("relative/{secret}")}),
        )
        .unwrap_err();
        error_id = e.error_id;
    }
    // every clone of the context is gone, so the guard has flushed the writer
    let mut text = String::new();
    for entry in std::fs::read_dir(logs.as_std_path()).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("rimstudio.") && name.ends_with(".jsonl") {
            text.push_str(&std::fs::read_to_string(entry.path()).unwrap());
        }
    }
    assert!(!text.is_empty(), "no log file in {logs}");
    let mut found = false;
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        if line.contains(&error_id) {
            found = true;
            assert_eq!(value["level"], json!("ERROR"));
        }
    }
    assert!(found, "the error id {error_id} is not in the log");
    let home = base.join("home");
    assert!(
        !text.contains(home.as_str()),
        "the home folder must be redacted out of the log"
    );
}
