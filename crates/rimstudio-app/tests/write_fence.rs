//! The game write fence of the app: built from the detected install and the settings, handed to the toolkit's
//! project environment, and refusing writes under the install on its own account (not only through the
//! folder lists of the tools).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::sync::Arc;

use camino::Utf8Path;
use common::{fixture, select_install};
use rimstudio_app::dispatch_blocking;
use rimstudio_io::guard::RootGuard;
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::shared::writer::GuardedWriter;
use serde_json::json;

#[test]
fn without_an_install_there_is_no_fence() {
    let f = fixture(false);
    let (env, _) = f.app.workspace.write_env(&f.app).unwrap();
    assert!(env.fence().is_none());
}

#[test]
fn the_fence_is_installed_with_the_install_and_denies_its_folders() {
    let f = fixture(true);
    select_install(&f);
    let (env, protected) = f.app.workspace.write_env(&f.app).unwrap();
    let fence = env.fence().expect("a fence once an install is selected");
    assert!(!protected.is_empty());
    for inside in [
        f.install.game_dir.join("Data/Core/Defs/x.xml"),
        f.install.mods_dir.join("RS_Mod/Defs/x.xml"),
    ] {
        assert!(fence.is_protected(&inside).unwrap(), "{inside}");
    }
    let elsewhere = f.base.join("projects/RS_Mod/Defs/x.xml");
    assert!(!fence.is_protected(&elsewhere).unwrap());
    // the built toolkit context carries the same kind of fence
    let ctx = f.app.workspace.designer_ctx(&f.app).unwrap();
    let built = ctx.env().fence().expect("the built context has the fence");
    assert!(
        built
            .is_protected(&f.install.game_dir.join("Data/x.xml"))
            .unwrap()
    );
}

#[test]
fn the_user_settings_widen_what_the_fence_denies() {
    let f = fixture(false);
    select_install(&f);
    let user = f.base.join("RS_UserData");
    std::fs::create_dir_all(user.join("Config").as_std_path()).unwrap();
    let out = dispatch_blocking(
        &f.app,
        "detect_set_override",
        json!({"field": "user-dir", "path": user.as_str()}),
    );
    assert!(out.is_ok(), "{out:?}");
    let (env, _) = f.app.workspace.write_env(&f.app).unwrap();
    let fence = env.fence().expect("fence");
    assert!(
        fence
            .is_protected(&user.join("Config/ModsConfig.xml"))
            .unwrap()
    );
    assert!(
        !fence
            .is_protected(&f.base.join("projects/RS_Mod/x.xml"))
            .unwrap()
    );
}

#[test]
fn a_plan_that_targets_a_path_under_the_install_is_refused_by_the_fence_itself() {
    let f = fixture(false);
    select_install(&f);
    let (env, _) = f.app.workspace.write_env(&f.app).unwrap();
    let fence = Arc::clone(env.fence().expect("fence"));
    // A project root that is an ancestor of the install (the folder above the Steam library): the root itself
    // is not protected, so only a check of the written path can stop a file addressed through the install.
    let parent = f
        .install
        .game_dir
        .parent()
        .and_then(Utf8Path::parent)
        .and_then(Utf8Path::parent)
        .unwrap()
        .to_path_buf();
    let rel = f
        .install
        .game_dir
        .strip_prefix(&parent)
        .unwrap()
        .join("Data/Core/Defs/RS_Planted.xml")
        .to_string();
    // the fence alone, with no folder list at all
    let writer = GuardedWriter::new(
        &parent,
        &[],
        f.base.join("backups"),
        Some(Arc::clone(&fence)),
        Arc::new(FakeClock::new(1)),
    )
    .unwrap();
    let err = writer.write(&rel, "x", false).unwrap_err();
    match &err {
        ToolkitError::PathRefused { reason, .. } => {
            assert!(reason.contains("write fence refused"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(
        !f.install
            .game_dir
            .join("Data/Core/Defs/RS_Planted.xml")
            .exists()
    );
    // the same check, as the guard sees it
    let guard = RootGuard::new([parent.clone()]).unwrap();
    assert!(
        fence
            .check_project_write(&guard, &parent.join(&rel))
            .is_err()
    );
    // a path the fence allows is not refused by it
    let ok = parent.join("RS_Elsewhere/Defs/ok.xml");
    assert!(fence.check_project_write(&guard, &ok).is_ok());
    // and the environment of the app refuses the same write, by whichever layer answers first
    let writer = env.writer(&parent, "rs-parent", &[]).unwrap();
    assert!(writer.write(&rel, "x", false).is_err());
}

#[test]
fn creating_a_project_outside_the_install_still_works_with_the_fence() {
    let f = fixture(false);
    select_install(&f);
    let target = f.base.join("projects/RS_Fenced");
    let out = dispatch_blocking(
        &f.app,
        "project_create",
        json!({"path": target.as_str(), "name": "RS Fenced", "packageId": "rs.fenced"}),
    )
    .unwrap();
    assert!(out["projectId"].is_string(), "{out}");
    assert!(target.join("About/About.xml").exists());
}
