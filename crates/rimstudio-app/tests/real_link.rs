//! A project linked into a game folder with the real platform (real symbolic links). The game folder is a
//! temporary copy-less fake: a temporary `Mods` folder, a link to the real `Data` and a copy of
//! `Version.txt`. The owner's real `Mods` folder is never used. `#[ignore]`d: it needs the real install.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! cargo test -p rimstudio-app --test real_link -- --ignored --nocapture
//! ```
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use camino::Utf8PathBuf;
use rimstudio_app::context::{AppOptions, Platform};
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootInput, boot, dispatch_blocking};
use rimstudio_io::roots::DataRoots;
use serde_json::json;

#[test]
#[ignore = "needs a real install for Data; set RIMSTUDIO_GAME_DIR"]
fn a_project_is_linked_into_a_temporary_mods_folder_and_removed_again() {
    let Ok(real) = std::env::var("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; nothing to do");
        return;
    };
    let real = Utf8PathBuf::from(real);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(std::fs::canonicalize(tmp.path()).unwrap()).unwrap();
    let game = base.join("RimWorld");
    std::fs::create_dir_all(game.join("Mods")).unwrap();
    std::os::unix::fs::symlink(real.join("Data"), game.join("Data")).unwrap();
    std::fs::copy(real.join("Version.txt"), game.join("Version.txt")).unwrap();
    let project = base.join("projects/RS_RealLink");
    std::fs::create_dir_all(project.join("About")).unwrap();
    std::fs::write(
        project.join("About/About.xml"),
        "<ModMetaData><name>RS Real</name><packageId>rs.reallink</packageId></ModMetaData>",
    )
    .unwrap();

    let app = boot(
        BootInput::new(Platform::system())
            .with_roots(DataRoots::under_base(&base.join("app")))
            .with_log(LogConfig::off())
            .with_options(AppOptions::default()),
    )
    .unwrap();
    dispatch_blocking(
        &app,
        "detect_set_override",
        json!({"field": "game-install", "path": game.as_str()}),
    )
    .unwrap();
    let id = dispatch_blocking(&app, "project_open", json!({"path": project.as_str()})).unwrap()
        ["projectId"]
        .as_str()
        .unwrap()
        .to_owned();

    let status = dispatch_blocking(&app, "project_link_status", json!({"projectId": id})).unwrap();
    assert_eq!(status["state"], "not-linked", "{status}");
    let made = dispatch_blocking(&app, "project_link_create", json!({"projectId": id})).unwrap();
    assert_eq!(made["done"], true, "{made}");
    assert_eq!(made["status"]["state"], "linked");
    let entry = game.join("Mods/RS_RealLink");
    assert!(entry.as_std_path().is_symlink());
    assert!(entry.join("About/About.xml").is_file());
    let gone = dispatch_blocking(&app, "project_link_remove", json!({"projectId": id})).unwrap();
    assert_eq!(gone["done"], true, "{gone}");
    assert!(!entry.as_std_path().exists());
    assert!(project.join("About/About.xml").is_file());
    println!("link, create and remove through the real platform: ok");
}
