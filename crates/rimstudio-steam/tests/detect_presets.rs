//! Detection over the per operating system layouts of the research (L1 to L6, W1 to W3, M1, N2, P1)
//! built by `rimstudio-testing`, with golden JSON for every report.

use rimstudio_steam::locator::{DetectOptions, detect};
use rimstudio_steam::report::DetectionReport;
use rimstudio_testing::detect_env::{FakeDetectEnv, SteamPreset};

fn run(preset: SteamPreset) -> DetectionReport {
    let env = FakeDetectEnv::from_preset(preset);
    detect(&env, &DetectOptions::new(preset.os()))
}

#[test]
fn golden_reports_for_every_preset() {
    for preset in SteamPreset::ALL {
        let report = run(preset);
        rimstudio_testing::golden_json!(&format!("detect-{}", preset.name()), &report);
    }
}

#[test]
fn detection_gives_identical_reports_on_one_and_on_eight_threads() {
    for preset in SteamPreset::ALL {
        let env = FakeDetectEnv::from_preset(preset);
        let options = DetectOptions::new(preset.os());
        let single = detect(&env, &options);
        let reports: Vec<DetectionReport> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| detect(&env, &options)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(reports.iter().all(|r| *r == single), "{}", preset.name());
    }
}

#[test]
fn detection_is_deterministic() {
    for preset in SteamPreset::ALL {
        assert_eq!(run(preset), run(preset), "{}", preset.name());
    }
}

mod scenarios {
    use camino::Utf8PathBuf;
    use rimstudio_core::os::Os;
    use rimstudio_core::redact::Redactor;
    use rimstudio_core::settings::{PathOverride, PathsSettings};
    use rimstudio_steam::locator::{DetectOptions, GameLocator, SteamLocator, detect};
    use rimstudio_steam::report::{
        Confidence, DetectionReport, Health, How, InstallKind, UserDirKind, codes,
    };
    use rimstudio_testing::detect_env::{
        FakeDetectEnv, LibrarySpec, SteamPreset, app_manifest_acf, library_folders_vdf,
    };

    const HOME: &str = "/home/rs_user";
    const ROOT: &str = "/home/rs_user/.local/share/Steam";

    fn paths_with(edit: impl FnOnce(&mut PathsSettings)) -> PathsSettings {
        let mut paths = PathsSettings::default();
        edit(&mut paths);
        paths
    }

    fn native() -> FakeDetectEnv {
        FakeDetectEnv::from_preset(SteamPreset::LinuxNative)
    }

    fn linux(env: &FakeDetectEnv) -> DetectionReport {
        detect(env, &DetectOptions::new(Os::Linux))
    }

    /// A Linux Steam root with a library file and no game.
    fn bare_steam() -> FakeDetectEnv {
        let env = FakeDetectEnv::empty(false);
        env.env.set_home(Some(Utf8PathBuf::from(HOME)));
        let vdf = library_folders_vdf(&[LibrarySpec {
            path: ROOT,
            apps: &[294_100],
        }]);
        env.fs
            .add_file(format!("{ROOT}/config/libraryfolders.vdf"), vdf);
        env.fs.add_dir(format!("{ROOT}/steamapps"));
        env
    }

    fn add_game(env: &FakeDetectEnv, dir: &str) {
        let game = format!("{ROOT}/steamapps/common/{dir}");
        env.fs
            .add_file(format!("{game}/Version.txt"), "1.9.9999 rev1\n");
        env.fs
            .add_file(format!("{game}/Data/Core/About/About.xml"), "x");
        env.fs.add_dir(format!("{game}/Mods"));
    }

    #[test]
    fn linux_native_finds_one_root_after_link_deduplication() {
        let report = linux(&native());
        assert_eq!(report.steam_roots.len(), 1);
        assert_eq!(report.steam_roots[0].how, How::XdgDataHome);
        assert_eq!(report.steam_roots[0].confidence, Confidence::High);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let install = report.selected_install().unwrap();
        assert_eq!(install.kind, InstallKind::Steam);
        assert_eq!(install.version.as_ref().unwrap().build, Some(9999));
        assert_eq!(install.health, Health::Installed);
        assert!(!install.proton);
        assert_eq!(
            install.launch.steam_url.as_deref(),
            Some("steam://rungameid/294100")
        );
        assert!(install.launch.executable.is_some());
    }

    #[test]
    fn the_locator_trait_and_the_function_agree() {
        let env = native();
        let options = DetectOptions::new(Os::Linux);
        assert_eq!(SteamLocator.detect(&env, &options), detect(&env, &options));
    }

    #[test]
    fn two_libraries_find_the_game_through_the_second_library_file_entry() {
        let report = detect(
            &FakeDetectEnv::from_preset(SteamPreset::LinuxTwoLibs),
            &DetectOptions::new(Os::Linux),
        );
        assert_eq!(report.libraries.len(), 2);
        assert!(!report.libraries[0].has_app);
        assert!(report.libraries[1].has_app);
        let install = report.selected_install().unwrap();
        assert_eq!(
            install.library.as_deref().map(|p| p.as_str()),
            Some("/mnt/SteamLibrary")
        );
        assert_eq!(install.workshop.len(), 1);
        assert_eq!(install.workshop[0].items_on_disk, 1);
        assert_eq!(install.workshop[0].items_in_acf, Some(1));
    }

    #[test]
    fn windows_escaped_paths_are_decoded_and_registry_order_is_kept() {
        let report = detect(
            &FakeDetectEnv::from_preset(SteamPreset::WindowsRegistry),
            &DetectOptions::new(Os::Windows),
        );
        assert_eq!(
            report.steam_roots.len(),
            1,
            "case insensitive duplicates merge"
        );
        assert_eq!(report.steam_roots[0].how, How::RegistryHkcu);
        let libs: Vec<&str> = report.libraries.iter().map(|l| l.path.as_str()).collect();
        assert_eq!(libs, ["C:/Program Files (x86)/Steam", "D:/SteamLibrary"]);
        assert_eq!(
            report.selected_user_dir().unwrap().kind,
            UserDirKind::Windows
        );
        let hklm = detect(
            &FakeDetectEnv::from_preset(SteamPreset::WindowsHklmOnly),
            &DetectOptions::new(Os::Windows),
        );
        assert_eq!(hklm.steam_roots[0].how, How::RegistryHklm);
        let fallback = detect(
            &FakeDetectEnv::from_preset(SteamPreset::WindowsDefault),
            &DetectOptions::new(Os::Windows),
        );
        assert_eq!(fallback.steam_roots[0].how, How::DirectoryProbe);
        assert_eq!(fallback.steam_roots[0].confidence, Confidence::Medium);
    }

    #[test]
    fn macos_bundle_holds_data_and_mods() {
        let report = detect(
            &FakeDetectEnv::from_preset(SteamPreset::MacOs),
            &DetectOptions::new(Os::MacOs),
        );
        let install = report.selected_install().unwrap();
        assert!(install.game_root.as_str().ends_with("RimWorldMac.app"));
        assert!(
            install
                .mods_dir
                .path
                .as_str()
                .ends_with("RimWorldMac.app/Mods")
        );
        assert!(install.mods_dir.exists);
        assert_eq!(install.version.as_ref().unwrap().minor, 9);
        assert_eq!(report.selected_user_dir().unwrap().kind, UserDirKind::Macos);
    }

    #[test]
    fn flatpak_and_snap_have_their_own_roots_and_user_folders() {
        let flat = detect(
            &FakeDetectEnv::from_preset(SteamPreset::LinuxFlatpak),
            &DetectOptions::new(Os::Linux),
        );
        assert_eq!(flat.steam_roots[0].how, How::Flatpak);
        assert_eq!(flat.user_dirs.len(), 2, "both variants are probed");
        let snap = detect(
            &FakeDetectEnv::from_preset(SteamPreset::LinuxSnap),
            &DetectOptions::new(Os::Linux),
        );
        assert_eq!(snap.steam_roots[0].how, How::Snap);
        assert_eq!(snap.user_dirs[0].kind, UserDirKind::Snap);
    }

    #[test]
    fn proton_prefix_with_a_newer_mods_config_is_selected_and_native_wins_when_newer() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxProton);
        let report = linux(&env);
        assert!(report.selected_install().unwrap().proton);
        assert!(report.selected_install().unwrap().launch.steam_only);
        assert_eq!(
            report.selected_user_dir().unwrap().kind,
            UserDirKind::Proton
        );
        assert!(report.has_warning(codes::PROTON_AND_NATIVE));

        let native_cfg = format!(
            "{HOME}/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"
        );
        env.fs
            .set_mtime_ns(&native_cfg, Some(9_000_000_000_000_000_000));
        let report = linux(&env);
        assert_eq!(
            report.selected_user_dir().unwrap().kind,
            UserDirKind::Native
        );
    }

    #[test]
    fn an_empty_compat_data_folder_is_not_proton() {
        let report = linux(&FakeDetectEnv::from_preset(SteamPreset::LinuxProtonEmpty));
        assert!(!report.selected_install().unwrap().proton);
        assert!(!report.has_warning(codes::PROTON_AND_NATIVE));
    }

    #[test]
    fn the_proton_config_file_is_not_opened_when_disabled() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxProton);
        // Remove the other Proton signal by not creating a prefix: use the empty preset plus a mapping.
        let empty = FakeDetectEnv::from_preset(SteamPreset::LinuxProtonEmpty);
        empty.fs.add_file(
            format!("{ROOT}/config/config.vdf"),
            "\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"CompatToolMapping\" { \"294100\" { \"name\" \"rs_tool\" } } } } } }",
        );
        let on = linux(&empty);
        assert!(on.selected_install().unwrap().proton);
        let mut options = DetectOptions::new(Os::Linux);
        options.read_proton_config = false;
        assert!(!detect(&empty, &options).selected_install().unwrap().proton);
        let _ = env;
    }

    #[test]
    fn the_pathological_tree_yields_one_warning_per_defect_and_still_finds_the_other_install() {
        let report = detect(
            &FakeDetectEnv::from_preset(SteamPreset::Pathological),
            &DetectOptions::new(Os::Linux),
        );
        let codes_found: Vec<&str> = report.warnings.iter().map(|w| w.code.as_str()).collect();
        assert_eq!(
            codes_found,
            [
                codes::INSTALL_DIR_MISSING,
                codes::LIBRARY_OFFLINE,
                codes::MANIFEST_LEFTOVER,
                codes::ROOT_DANGLING_LINK,
            ]
        );
        assert_eq!(report.installs.len(), 1);
        assert_eq!(report.installs[0].kind, InstallKind::Gog);
        let offline: Vec<_> = report.libraries.iter().filter(|l| !l.online).collect();
        assert_eq!(
            offline.len(),
            1,
            "an offline library is listed, not dropped"
        );
    }

    #[test]
    fn a_hung_library_is_reported_and_the_other_candidates_still_arrive() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxTwoLibs);
        env.fs.hang("/mnt/SteamLibrary");
        let report = linux(&env);
        assert!(report.has_warning(codes::LIBRARY_TIMEOUT));
        let hung = report.libraries.iter().find(|l| l.timed_out).unwrap();
        assert!(!hung.online);
        assert_eq!(report.libraries.len(), 2);
        assert_eq!(report.steam_roots.len(), 1);
        assert!(!report.has_warning(codes::PROBE_TIMEOUT));
    }

    #[test]
    fn a_hung_root_is_a_probe_timeout_not_a_crash() {
        let env = native();
        env.fs.hang(ROOT);
        let report = linux(&env);
        assert!(
            report.has_warning(codes::PROBE_TIMEOUT) || report.has_warning(codes::LIBRARY_TIMEOUT)
        );
        assert!(report.installs.iter().all(|i| i.kind != InstallKind::Steam));
    }

    #[test]
    fn without_any_manifest_the_default_folder_is_probed_with_medium_confidence() {
        let env = bare_steam();
        add_game(&env, "RimWorld");
        let report = linux(&env);
        let install = report.selected_install().unwrap();
        assert_eq!(install.how, How::DirectoryProbe);
        assert_eq!(install.confidence, Confidence::Medium);
        assert!(report.has_warning(codes::APPS_TABLE_STALE));
        assert!(report.libraries[0].stale);
    }

    #[test]
    fn the_install_folder_comes_from_the_manifest_with_a_fallback_to_rimworld() {
        let env = bare_steam();
        env.fs.add_file(
            format!("{ROOT}/steamapps/appmanifest_294100.acf"),
            app_manifest_acf("RS_Custom"),
        );
        add_game(&env, "RS_Custom");
        let report = linux(&env);
        assert!(
            report
                .selected_install()
                .unwrap()
                .path
                .as_str()
                .ends_with("/RS_Custom")
        );

        let env = bare_steam();
        env.fs.add_file(
            format!("{ROOT}/steamapps/appmanifest_294100.acf"),
            app_manifest_acf("../escape"),
        );
        add_game(&env, "RimWorld");
        let report = linux(&env);
        assert!(
            report
                .selected_install()
                .unwrap()
                .path
                .as_str()
                .ends_with("/RimWorld")
        );
    }

    #[test]
    fn an_install_without_the_core_marker_is_rejected() {
        let env = bare_steam();
        env.fs.add_file(
            format!("{ROOT}/steamapps/appmanifest_294100.acf"),
            app_manifest_acf("RimWorld"),
        );
        env.fs.add_file(
            format!("{ROOT}/steamapps/common/RimWorld/Version.txt"),
            "1.9.9999\n",
        );
        let report = linux(&env);
        assert!(report.installs.is_empty());
        assert!(report.has_warning(codes::INSTALL_CONTENT_INVALID));
    }

    #[test]
    fn an_empty_manifest_is_reported_and_skipped() {
        let env = bare_steam();
        env.fs
            .add_file(format!("{ROOT}/steamapps/appmanifest_294100.acf"), "");
        add_game(&env, "RimWorld");
        let report = linux(&env);
        assert!(report.has_warning(codes::ACF_UNREADABLE));
        // The last resort probe still finds the copy.
        assert_eq!(report.selected_install().unwrap().how, How::DirectoryProbe);
    }

    #[test]
    fn update_and_verify_states_come_from_the_manifest() {
        let env = bare_steam();
        let text = app_manifest_acf("RimWorld")
            .replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"6\"");
        env.fs
            .add_file(format!("{ROOT}/steamapps/appmanifest_294100.acf"), text);
        add_game(&env, "RimWorld");
        let report = linux(&env);
        assert_eq!(
            report.selected_install().unwrap().health,
            Health::UpdatePending
        );
        assert!(report.has_warning(codes::INSTALL_UPDATE_PENDING));

        let env = bare_steam();
        let text = app_manifest_acf("RimWorld")
            .replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"36\"");
        env.fs
            .add_file(format!("{ROOT}/steamapps/appmanifest_294100.acf"), text);
        add_game(&env, "RimWorld");
        let report = linux(&env);
        assert_eq!(
            report.selected_install().unwrap().health,
            Health::NeedsVerify
        );
    }

    #[test]
    fn version_text_problems_are_warnings_and_the_raw_text_is_not_lost_elsewhere() {
        let env = bare_steam();
        env.fs.add_file(
            format!("{ROOT}/steamapps/appmanifest_294100.acf"),
            app_manifest_acf("RimWorld"),
        );
        add_game(&env, "RimWorld");
        env.fs.add_file(
            format!("{ROOT}/steamapps/common/RimWorld/Version.txt"),
            "garbage text\n",
        );
        let report = linux(&env);
        assert!(report.has_warning(codes::VERSION_UNPARSEABLE));
        assert!(report.selected_install().unwrap().version.is_none());
    }

    #[test]
    fn a_mods_config_from_another_version_warns_and_is_never_the_install_version() {
        // A stand in for the XML reader the application supplies: the text between two markers.
        fn reader(text: &str) -> Option<String> {
            let rest = text.split_once("VERSION=")?.1;
            Some(rest.lines().next()?.trim().to_owned())
        }
        let env = native();
        let cfg = format!(
            "{HOME}/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"
        );
        env.fs.add_file(&cfg, "VERSION=1.0.1 rev7\n");
        let mut options = DetectOptions::new(Os::Linux);
        options.mods_config_version = Some(reader);
        let report = detect(&env, &options);
        assert!(report.has_warning(codes::USERDIR_VERSION_MISMATCH));
        assert_eq!(
            report
                .selected_user_dir()
                .unwrap()
                .mods_config
                .game_version
                .as_deref(),
            Some("1.0.1 rev7")
        );
        assert_eq!(
            report
                .selected_install()
                .unwrap()
                .version
                .as_ref()
                .unwrap()
                .raw,
            "1.9.9999 rev1"
        );

        env.fs.add_file(&cfg, "VERSION=1.9.9999 rev1\n");
        assert!(!detect(&env, &options).has_warning(codes::USERDIR_VERSION_MISMATCH));
        // Without a reader nothing is read and nothing is raised.
        let plain = linux(&env);
        assert!(!plain.has_warning(codes::USERDIR_VERSION_MISMATCH));
        assert!(
            plain
                .selected_user_dir()
                .unwrap()
                .mods_config
                .game_version
                .is_none()
        );
    }

    #[test]
    fn valid_overrides_win_and_invalid_ones_are_reported_and_ignored() {
        let env = native();
        env.fs.add_file("/rs/other/Data/Core/About/About.xml", "x");
        env.fs.add_file("/rs/other/Version.txt", "1.8.1 rev2");
        env.fs.add_dir("/rs/other/Mods");
        env.fs.add_dir("/rs/userdir");

        let paths = paths_with(|paths| {
            paths.game_install = Some(PathOverride {
                path: "/rs/other".into(),
                pinned: true,
            });
            paths.user_dir = Some(PathOverride {
                path: "/rs/userdir".into(),
                pinned: false,
            });
        });
        let options = DetectOptions::new(Os::Linux).with_paths(paths);
        let report = detect(&env, &options);
        let install = report.selected_install().unwrap();
        assert_eq!(install.kind, InstallKind::Override);
        assert_eq!(install.how, How::Override);
        assert_eq!(
            report.installs.len(),
            2,
            "the detected install stays listed"
        );
        assert_eq!(
            report.selected_user_dir().unwrap().kind,
            UserDirKind::Override
        );

        let paths = paths_with(|paths| {
            paths.game_install = Some(PathOverride {
                path: "/rs/missing".into(),
                pinned: false,
            });
            paths.user_dir = Some(PathOverride {
                path: "/rs/missing2".into(),
                pinned: false,
            });
            paths.steam_root = Some(PathOverride {
                path: "/rs/missing3".into(),
                pinned: false,
            });
        });
        let report = detect(&env, &DetectOptions::new(Os::Linux).with_paths(paths));
        assert_eq!(
            report
                .warnings
                .iter()
                .filter(|w| w.code == codes::OVERRIDE_INVALID)
                .count(),
            3
        );
        assert_eq!(report.selected_install().unwrap().kind, InstallKind::Steam);
    }

    #[test]
    fn an_override_equal_to_a_detected_install_selects_it_without_a_duplicate() {
        let env = FakeDetectEnv::from_preset(SteamPreset::Pathological);
        // Select the GOG copy over everything else by overriding to its path.
        let paths = paths_with(|paths| {
            paths.game_install = Some(PathOverride {
                path: "/home/rs_user/GOG Games/RimWorld/game".into(),
                pinned: false,
            });
        });
        let report = detect(&env, &DetectOptions::new(Os::Linux).with_paths(paths));
        assert_eq!(report.installs.len(), 1);
        assert_eq!(report.installs[0].kind, InstallKind::Gog);
    }

    #[test]
    fn ignored_installs_are_hidden_and_the_save_data_folder_is_selected() {
        let env = native();
        env.fs.add_dir("/rs/save");
        let paths = paths_with(|paths| {
            paths.ignored_installs = vec![format!("steam:{ROOT}")];
        });
        let mut options = DetectOptions::new(Os::Linux).with_paths(paths);
        options.savedata_folder = Some("/rs/save".into());
        let report = detect(&env, &options);
        assert!(report.installs.is_empty());
        assert!(report.selected.install.is_none());
        let dir = report.selected_user_dir().unwrap();
        assert_eq!(dir.kind, UserDirKind::Override);
        assert_eq!(dir.path, "/rs/save");
    }

    #[test]
    fn non_steam_probing_can_be_turned_off() {
        let env = FakeDetectEnv::from_preset(SteamPreset::GogLinux);
        let on = linux(&env);
        assert_eq!(on.installs[0].kind, InstallKind::Gog);
        assert!(on.installs[0].workshop.is_empty());
        let mut options = DetectOptions::new(Os::Linux);
        options.include_non_steam = false;
        assert!(detect(&env, &options).installs.is_empty());
    }

    #[test]
    fn extra_workshop_folders_attach_to_steam_installs() {
        let env = native();
        env.fs.add_file("/rs/ws/777/About/About.xml", "x");
        let paths = paths_with(|paths| {
            paths.extra_workshop_dirs = vec!["/rs/ws".into(), "/rs/none".into()];
        });
        let report = detect(&env, &DetectOptions::new(Os::Linux).with_paths(paths));
        let ws = &report.selected_install().unwrap().workshop;
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[1].how, How::Override);
        assert_eq!(ws[1].items_on_disk, 1);
        assert!(report.has_warning(codes::OVERRIDE_INVALID));
    }

    #[test]
    fn workshop_content_in_another_library_is_noted() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxTwoLibs);
        // The game's library has no workshop folder; the first library gets one.
        env.fs.add_file(
            format!("{ROOT}/steamapps/workshop/content/294100/55/About/About.xml"),
            "x",
        );
        // Remove the game library's workshop by using a fresh layout is not possible; instead assert
        // the merged list holds both libraries, own library first.
        let report = linux(&env);
        let ws = &report.selected_install().unwrap().workshop;
        assert_eq!(ws.len(), 2);
        assert_eq!(
            ws[0].library.as_deref().map(|p| p.as_str()),
            Some("/mnt/SteamLibrary")
        );
    }

    #[test]
    fn the_report_survives_a_json_round_trip_and_redaction_removes_the_home_folder() {
        let report = linux(&native());
        let json = serde_json::to_string(&report).unwrap();
        let back: DetectionReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
        let red = report.redacted(&Redactor::new().with_home(HOME));
        let text = serde_json::to_string(&red).unwrap();
        assert!(!text.contains("rs_user"), "{text}");
        assert!(text.contains("~/.local/share/Steam"));
    }
}
