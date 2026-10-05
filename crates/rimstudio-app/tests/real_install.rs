//! Tests against a real install. All are `#[ignore]`: they only read the install and write only into
//! temporary folders. Nothing is asserted about counts, only about structure; the output is for the
//! person running them and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! cargo test -p rimstudio-app --release --test real_install -- --ignored --nocapture
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::time::Instant;

use camino::Utf8PathBuf;
use common::{draft_json, platform, ranged_spec};
use rimstudio_app::context::AppOptions;
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootInput, boot, dispatch_blocking};
use rimstudio_io::roots::DataRoots;
use rimstudio_testing::fakes::FakeClock;
use serde_json::{Value, json};

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(std::env::var(name).ok()?);
    path.is_dir().then_some(path)
}

fn timed(label: &str, app: &rimstudio_app::AppContext, name: &str, req: Value) -> Value {
    let start = Instant::now();
    let out = dispatch_blocking(app, name, req).unwrap_or_else(|e| panic!("{name}: {e}"));
    println!("{label}: {} ms", start.elapsed().as_millis());
    out
}

#[test]
#[ignore = "reads a real install; set RIMSTUDIO_GAME_DIR"]
fn the_designer_reads_a_real_install_through_dispatch() {
    let Some(game) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; nothing to do");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let app = boot(
        BootInput::new(platform(&base, &clock))
            .with_roots(DataRoots::under_base(&base.join("app")))
            .with_log(LogConfig::off())
            .with_options(AppOptions::default()),
    )
    .unwrap();
    timed(
        "select the install",
        &app,
        "detect_set_override",
        json!({"field": "game-install", "path": game.as_str()}),
    );
    if let Some(ce) = dir_from_env("RIMSTUDIO_CE_DIR") {
        timed(
            "add the Combat Extended folder",
            &app,
            "sources_add_folder",
            json!({"path": ce.as_str(), "layout": "single-mod"}),
        );
    }
    let scan = timed("library scan", &app, "library_scan", json!({}));
    println!("mods found: {}", scan["stats"]["modsFound"]);
    let refs = timed(
        "reference list (builds the sessions)",
        &app,
        "designer_reference_list",
        json!({"kind": "ranged", "limit": 5}),
    );
    println!("ranged reference items: {}", refs["total"]);
    let preview = timed(
        "preview of a fictional rifle",
        &app,
        "designer_preview",
        json!({"draft": draft_json(&ranged_spec())}),
    );
    println!(
        "readouts: {}",
        preview["readouts"].as_array().map_or(0, Vec::len)
    );
    let page = timed(
        "def search",
        &app,
        "defs_search",
        json!({"sessionId": "", "query": "", "limit": 1}),
    );
    println!("defs in the reference set: {}", page["total"]);
    match dispatch_blocking(
        &app,
        "defs_search",
        json!({"sessionId": "ce", "query": "", "limit": 1}),
    ) {
        Ok(page) => println!("defs with Combat Extended: {}", page["total"]),
        Err(e) => println!("no Combat Extended session: {}", e.code),
    }
    let tools = timed("tools", &app, "app_list_tools", json!({}));
    println!("tools: {tools}");
}
