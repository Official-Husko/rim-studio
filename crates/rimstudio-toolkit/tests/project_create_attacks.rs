//! Attacks on `project_create`: hostile target folders, links, Steam libraries, and the recovery of a
//! scaffold that stopped half way. Names are fictional; the tests write only into temporary folders.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_io::roots::DataRoots;
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::project::create;
use rimstudio_toolkit::shared::env::ProjectEnv;
use rimstudio_workspace::scaffold::ScaffoldSpec;

struct World {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    env: ProjectEnv,
}

fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    World {
        _tmp: tmp,
        base,
        env,
    }
}

fn spec(target: &Utf8PathBuf) -> ScaffoldSpec {
    ScaffoldSpec::new(target.clone(), "RS Attack Mod", "rs.attackmod")
}

#[test]
fn a_target_folder_with_a_windows_device_name_or_a_trailing_dot_is_refused() {
    let w = world();
    for name in ["CON", "nul.d", "RS_Mod.", "RS_Mod ", "a:b", "COM3"] {
        let target = w.base.join("mods").join(name);
        let err = create(&w.env, &spec(&target), &[]).unwrap_err();
        assert!(
            matches!(err, ToolkitError::PathRefused { .. }),
            "{name}: {err:?}"
        );
        assert!(!w.base.join("mods").exists(), "{name}: nothing was created");
    }
}

#[test]
fn a_target_with_a_parent_component_or_that_is_relative_is_refused() {
    let w = world();
    let sneaky = Utf8PathBuf::from(format!("{}/mods/../../escape", w.base));
    assert!(create(&w.env, &spec(&sneaky), &[]).is_err());
    assert!(!w.base.join("escape").exists());
    assert!(create(&w.env, &spec(&Utf8PathBuf::from("relative/RS_Mod")), &[]).is_err());
    assert!(!w.base.join("mods").exists());
}

#[test]
fn a_target_inside_a_steam_library_is_refused() {
    let w = world();
    let target = w
        .base
        .join("Steam/steamapps/workshop/content/294100/RS_Mod");
    let err = create(&w.env, &spec(&target), &[]).unwrap_err();
    assert!(matches!(err, ToolkitError::PathRefused { .. }), "{err:?}");
    assert!(!w.base.join("Steam").exists());
}

#[cfg(unix)]
#[test]
fn a_target_that_is_a_link_is_refused_and_the_link_target_stays_empty() {
    let w = world();
    let real = w.base.join("real");
    std::fs::create_dir_all(real.as_std_path()).unwrap();
    let link = w.base.join("RS_Link");
    std::os::unix::fs::symlink(real.as_std_path(), link.as_std_path()).unwrap();
    let err = create(&w.env, &spec(&link), &[]).unwrap_err();
    assert!(matches!(err, ToolkitError::PathRefused { .. }), "{err:?}");
    assert_eq!(std::fs::read_dir(real.as_std_path()).unwrap().count(), 0);
}

#[test]
fn a_scaffold_that_stopped_half_way_can_be_created_again_without_overwriting_anything() {
    let w = world();
    let target = w.base.join("RS_Half");
    let mut s = spec(&target);
    s.ce_patch_folder = true;
    let first = create(&w.env, &s, &[]).unwrap();
    // simulate a crash after some files: remove the About file, which is written last
    let about = target.join("About/About.xml");
    assert_eq!(
        first.written.last().map(String::as_str),
        Some("About/About.xml")
    );
    std::fs::remove_file(about.as_std_path()).unwrap();
    let lf_before = std::fs::read_to_string(target.join("LoadFolders.xml").as_std_path()).unwrap();
    let second = create(&w.env, &s, &[]).unwrap();
    assert!(about.exists());
    assert_eq!(
        std::fs::read_to_string(target.join("LoadFolders.xml").as_std_path()).unwrap(),
        lf_before
    );
    assert!(second.written.iter().any(|p| p == "About/About.xml"));
}

#[test]
fn creating_a_complete_scaffold_again_is_refused() {
    let w = world();
    let target = w.base.join("RS_Done");
    create(&w.env, &spec(&target), &[]).unwrap();
    let err = create(&w.env, &spec(&target), &[]).unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed", "{err:?}");
}

#[test]
fn the_about_file_is_written_last_so_a_stopped_scaffold_is_not_taken_for_a_mod() {
    let w = world();
    let target = w.base.join("RS_Order");
    let mut s = spec(&target);
    s.ce_patch_folder = true;
    let r = create(&w.env, &s, &[]).unwrap();
    assert_eq!(
        r.written.last().map(String::as_str),
        Some("About/About.xml")
    );
}
