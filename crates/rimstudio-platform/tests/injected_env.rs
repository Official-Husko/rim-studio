//! Directory resolution, install source and sandbox detection driven by fake environments from
//! `rimstudio-testing`, so every operating system is covered on any host.

use rimstudio_core::os::Os;
use rimstudio_core::ports::InstallSource;
use rimstudio_platform::dirs::platform_dirs;
use rimstudio_platform::install::{decide_install_source, inputs_from_env};
use rimstudio_platform::sandbox::detect_sandbox;
use rimstudio_testing::prelude::*;

#[test]
fn directories_follow_the_injected_environment_per_os() {
    let linux = FakeEnv::new()
        .with_home("/home/rs_user")
        .with_var("XDG_DATA_HOME", "/data");
    let d = platform_dirs(Os::Linux, &linux).unwrap();
    assert_eq!(d.data, "/data/rimstudio");
    assert_eq!(d.config, "/home/rs_user/.config/rimstudio");

    let mac = FakeEnv::new().with_home("/Users/rs_user");
    let d = platform_dirs(Os::MacOs, &mac).unwrap();
    assert_eq!(d.cache, "/Users/rs_user/Library/Caches/RimStudio");

    let win = FakeEnv::new()
        .with_home("C:\\Users\\rs_user")
        .with_var("LOCALAPPDATA", "C:\\Users\\rs_user\\AppData\\Local");
    let d = platform_dirs(Os::Windows, &win).unwrap();
    assert_eq!(
        d.logs,
        "C:\\Users\\rs_user\\AppData\\Local\\RimStudio\\logs"
    );
}

#[test]
fn install_source_is_decided_from_injected_variables() {
    let env = FakeEnv::new()
        .with_var("APPIMAGE", "/home/rs_user/RimStudio.AppImage")
        .with_exe_dir("/tmp/.mount_x/usr/bin");
    let inputs = inputs_from_env(Os::Linux, &env, false);
    assert_eq!(decide_install_source(&inputs), InstallSource::AppImage);

    let env = FakeEnv::new().with_exe_dir("/usr/bin");
    let inputs = inputs_from_env(Os::Linux, &env, false);
    assert_eq!(decide_install_source(&inputs), InstallSource::SystemPackage);
}

#[test]
fn sandbox_detection_reads_the_flatpak_info_text() {
    let env = FakeEnv::new()
        .with_home("/home/rs_user")
        .with_var("FLATPAK_ID", "dev.example.RS_Studio");
    let info = detect_sandbox(&env, Some("[Context]\nfilesystems=/run/media;home;\n"));
    assert_eq!(info.kind.as_deref(), Some("flatpak"));
    assert!(info.hides_processes);
    assert_eq!(info.granted_paths.len(), 2);
}
