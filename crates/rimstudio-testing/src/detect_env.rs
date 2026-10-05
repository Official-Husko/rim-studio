//! [`FakeDetectEnv`]: a `DetectEnv` built from fakes, with presets for the per operating system
//! Steam layouts of the detection research (native Linux, Flatpak, Snap, Proton, Windows registry,
//! macOS, a GOG copy and a pathological tree).
//!
//! The presets create the folder structure that detection reads: library files, the app manifest,
//! the workshop folder, an install with a version file and an executable stub, and the user config
//! folder. File content is fictional; files that real installs hold as XML contain the single word
//! `placeholder` because detection only checks that they exist.

use camino::Utf8PathBuf;
use rimstudio_core::os::Os;
use rimstudio_core::ports::{Clock, DetectEnv, EnvProbe, FsProbe, Hive, RegistryProbe};

use crate::fake_fs::FakeFs;
use crate::fakes::{FakeClock, FakeEnv, FakeRegistry};

/// The Steam app id of the game.
pub const GAME_APP_ID: u64 = 294_100;

/// The fictional home folder used by every Unix preset.
pub const UNIX_HOME: &str = "/home/rs_user";

/// The fictional macOS home folder.
pub const MAC_HOME: &str = "/Users/rs_user";

/// The fictional Windows profile folder.
pub const WINDOWS_HOME: &str = "C:/Users/rs_user";

/// The fictional version file text of every preset install.
pub const VERSION_TEXT: &str = "1.9.9999 rev1\n";

/// A ready made layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SteamPreset {
    /// Steam under `~/.local/share/Steam`, `~/.steam/steam` and `~/.steam/root` link to it.
    LinuxNative,
    /// Native plus a second library under `/mnt` that holds the game; the first holds none.
    LinuxTwoLibs,
    /// Steam inside the Flatpak home, with both user folder variants.
    LinuxFlatpak,
    /// Steam under the snap home.
    LinuxSnap,
    /// Native plus a Proton prefix with a newer `ModsConfig.xml` and a compat tool mapping.
    LinuxProton,
    /// Native plus an empty `compatdata/294100` folder.
    LinuxProtonEmpty,
    /// Windows with both registry values set; paths are written with escaped backslashes.
    WindowsRegistry,
    /// Windows with only the machine wide registry value.
    WindowsHklmOnly,
    /// Windows with no registry values and Steam in its default folder.
    WindowsDefault,
    /// macOS with the game as an app bundle.
    MacOs,
    /// A GOG style Linux copy with no Steam at all.
    GogLinux,
    /// Native Linux plus broken pieces: a zero byte temp manifest, a byte order mark in the library
    /// file, an offline library, a manifest whose install folder is missing and a dangling link.
    Pathological,
}

impl SteamPreset {
    /// Every preset, in a fixed order.
    pub const ALL: [SteamPreset; 12] = [
        SteamPreset::LinuxNative,
        SteamPreset::LinuxTwoLibs,
        SteamPreset::LinuxFlatpak,
        SteamPreset::LinuxSnap,
        SteamPreset::LinuxProton,
        SteamPreset::LinuxProtonEmpty,
        SteamPreset::WindowsRegistry,
        SteamPreset::WindowsHklmOnly,
        SteamPreset::WindowsDefault,
        SteamPreset::MacOs,
        SteamPreset::GogLinux,
        SteamPreset::Pathological,
    ];

    /// A stable kebab-case name, suitable for golden file names.
    pub fn name(self) -> &'static str {
        match self {
            SteamPreset::LinuxNative => "linux-native",
            SteamPreset::LinuxTwoLibs => "linux-two-libs",
            SteamPreset::LinuxFlatpak => "linux-flatpak",
            SteamPreset::LinuxSnap => "linux-snap",
            SteamPreset::LinuxProton => "linux-proton",
            SteamPreset::LinuxProtonEmpty => "linux-proton-empty",
            SteamPreset::WindowsRegistry => "windows-registry",
            SteamPreset::WindowsHklmOnly => "windows-hklm-only",
            SteamPreset::WindowsDefault => "windows-default",
            SteamPreset::MacOs => "macos",
            SteamPreset::GogLinux => "gog-linux",
            SteamPreset::Pathological => "pathological",
        }
    }

    /// The operating system the layout belongs to.
    pub fn os(self) -> Os {
        match self {
            SteamPreset::WindowsRegistry
            | SteamPreset::WindowsHklmOnly
            | SteamPreset::WindowsDefault => Os::Windows,
            SteamPreset::MacOs => Os::MacOs,
            _ => Os::Linux,
        }
    }
}

/// A `DetectEnv` made of a clock, an environment, a registry and an in memory file system. The
/// parts are public so a test can edit them before or after the preset is applied.
#[derive(Debug)]
pub struct FakeDetectEnv {
    /// The clock.
    pub clock: FakeClock,
    /// The environment.
    pub env: FakeEnv,
    /// The registry.
    pub registry: FakeRegistry,
    /// The file system.
    pub fs: FakeFs,
}

impl DetectEnv for FakeDetectEnv {
    fn clock(&self) -> &dyn Clock {
        &self.clock
    }
    fn env(&self) -> &dyn EnvProbe {
        &self.env
    }
    fn registry(&self) -> &dyn RegistryProbe {
        &self.registry
    }
    fn fs(&self) -> &dyn FsProbe {
        &self.fs
    }
}

impl FakeDetectEnv {
    /// An empty environment: no variables, no registry values, an empty tree.
    pub fn empty(case_insensitive: bool) -> Self {
        FakeDetectEnv {
            clock: FakeClock::default(),
            env: FakeEnv::new(),
            registry: FakeRegistry::new(),
            fs: if case_insensitive {
                FakeFs::case_insensitive()
            } else {
                FakeFs::new()
            },
        }
    }

    /// Builds the layout of a preset.
    pub fn from_preset(preset: SteamPreset) -> Self {
        match preset {
            SteamPreset::LinuxNative => linux_native(),
            SteamPreset::LinuxTwoLibs => linux_two_libs(),
            SteamPreset::LinuxFlatpak => linux_flatpak(),
            SteamPreset::LinuxSnap => linux_snap(),
            SteamPreset::LinuxProton => linux_proton(true),
            SteamPreset::LinuxProtonEmpty => linux_proton(false),
            SteamPreset::WindowsRegistry => windows(true, true, true),
            SteamPreset::WindowsHklmOnly => windows(false, true, true),
            SteamPreset::WindowsDefault => windows(false, false, true),
            SteamPreset::MacOs => macos(),
            SteamPreset::GogLinux => gog_linux(),
            SteamPreset::Pathological => pathological(),
        }
    }

    /// The home folder of the environment.
    pub fn home(&self) -> Option<Utf8PathBuf> {
        self.env.home_dir()
    }

    /// Adds a text file (and its parents) to the tree.
    pub fn with_file(self, path: &str, text: &str) -> Self {
        self.fs.add_file(path, text);
        self
    }

    /// Adds a folder to the tree.
    pub fn with_dir(self, path: &str) -> Self {
        self.fs.add_dir(path);
        self
    }

    /// Sets an environment variable.
    pub fn with_var(self, key: &str, value: &str) -> Self {
        self.env.set_var(key, value);
        self
    }

    /// Sets a registry string value.
    pub fn with_registry(self, hive: Hive, key: &str, value: &str, data: &str) -> Self {
        self.registry.set(hive, key, value, data);
        self
    }
}

// ---------------------------------------------------------------------------------------------
// Text templates
// ---------------------------------------------------------------------------------------------

/// One library entry of a `libraryfolders` file.
#[derive(Debug, Clone, Copy)]
pub struct LibrarySpec<'a> {
    /// The library path as written in the file (already escaped when it must be).
    pub path: &'a str,
    /// App ids that the `apps` table lists.
    pub apps: &'a [u64],
}

/// The text of a `libraryfolders.vdf` listing `libraries` (index 0 first).
pub fn library_folders_vdf(libraries: &[LibrarySpec<'_>]) -> String {
    let mut out = String::from("\"libraryfolders\"\n{\n");
    for (i, lib) in libraries.iter().enumerate() {
        out.push_str(&format!("\t\"{i}\"\n\t{{\n"));
        out.push_str(&format!("\t\t\"path\"\t\t\"{}\"\n", lib.path));
        out.push_str("\t\t\"label\"\t\t\"\"\n\t\t\"apps\"\n\t\t{\n");
        for app in lib.apps {
            out.push_str(&format!("\t\t\t\"{app}\"\t\t\"1000\"\n"));
        }
        out.push_str("\t\t}\n\t}\n");
    }
    out.push_str("}\n");
    out
}

/// The text of an app manifest for the game with the given install folder name. The state flags
/// say fully installed and the build ids match (no update pending).
pub fn app_manifest_acf(install_dir: &str) -> String {
    format!(
        "\"AppState\"\n{{\n\t\"appid\"\t\t\"{GAME_APP_ID}\"\n\t\"name\"\t\t\"RS_TestGame\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"{install_dir}\"\n\t\"buildid\"\t\t\"1000\"\n\t\"TargetBuildID\"\t\t\"1000\"\n}}\n"
    )
}

/// The text of a workshop manifest that lists the given item ids as installed.
pub fn workshop_acf(items: &[u64]) -> String {
    let mut out = format!(
        "\"AppWorkshop\"\n{{\n\t\"appid\"\t\t\"{GAME_APP_ID}\"\n\t\"WorkshopItemsInstalled\"\n\t{{\n"
    );
    for id in items {
        out.push_str(&format!(
            "\t\t\"{id}\"\n\t\t{{\n\t\t\t\"timeupdated\"\t\t\"1\"\n\t\t}}\n"
        ));
    }
    out.push_str("\t}\n}\n");
    out
}

const PLACEHOLDER: &str = "placeholder\n";

fn escaped(path: &str) -> String {
    path.replace('/', "\\\\")
}

// ---------------------------------------------------------------------------------------------
// Tree builders
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Linux,
    Windows,
    Mac,
}

fn add_install(fs: &FakeFs, game_dir: &str, style: Style) {
    match style {
        Style::Linux => {
            fs.add_file(format!("{game_dir}/Version.txt"), VERSION_TEXT);
            fs.add_file(format!("{game_dir}/Data/Core/About/About.xml"), PLACEHOLDER);
            fs.add_dir(format!("{game_dir}/Mods"));
            fs.add_file(format!("{game_dir}/RimWorldLinux"), "");
            fs.add_dir(format!("{game_dir}/RimWorldLinux_Data"));
        }
        Style::Windows => {
            fs.add_file(format!("{game_dir}/Version.txt"), VERSION_TEXT);
            fs.add_file(format!("{game_dir}/Data/Core/About/About.xml"), PLACEHOLDER);
            fs.add_dir(format!("{game_dir}/Mods"));
            fs.add_file(format!("{game_dir}/RimWorldWin64.exe"), "");
        }
        Style::Mac => {
            let bundle = format!("{game_dir}/RimWorldMac.app");
            fs.add_file(format!("{bundle}/Version.txt"), VERSION_TEXT);
            fs.add_file(format!("{bundle}/Data/Core/About/About.xml"), PLACEHOLDER);
            fs.add_dir(format!("{bundle}/Mods"));
        }
    }
}

/// Creates the files of one library: manifest, install and workshop when `with_game`.
fn add_library(fs: &FakeFs, lib: &str, style: Style, with_game: bool) {
    fs.add_dir(format!("{lib}/steamapps"));
    if with_game {
        fs.add_file(
            format!("{lib}/steamapps/appmanifest_{GAME_APP_ID}.acf"),
            app_manifest_acf("RimWorld"),
        );
        add_install(fs, &format!("{lib}/steamapps/common/RimWorld"), style);
        fs.add_file(
            format!("{lib}/steamapps/workshop/content/{GAME_APP_ID}/111/About/About.xml"),
            PLACEHOLDER,
        );
        fs.add_file(
            format!("{lib}/steamapps/workshop/appworkshop_{GAME_APP_ID}.acf"),
            workshop_acf(&[111]),
        );
    }
}

/// Creates the root library plus its two library files listing `libraries` (index 0 is the root).
fn add_root(fs: &FakeFs, root: &str, style: Style, libraries: &[LibrarySpec<'_>]) {
    let text = library_folders_vdf(libraries);
    fs.add_file(format!("{root}/config/libraryfolders.vdf"), text.clone());
    fs.add_file(format!("{root}/steamapps/libraryfolders.vdf"), text);
    let _ = style;
}

fn user_config(home: &str, rel: &str) -> String {
    format!("{home}/{rel}/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml")
}

fn unix_base() -> FakeDetectEnv {
    let d = FakeDetectEnv::empty(false);
    d.env.set_home(Some(Utf8PathBuf::from(UNIX_HOME)));
    d.env.set_var("HOME", UNIX_HOME);
    d.fs.set_default_volume(rimstudio_core::ports::VolumeClass::Posix);
    d
}

fn link_dot_steam(d: &FakeDetectEnv, root: &str) {
    d.fs.add_symlink(format!("{UNIX_HOME}/.steam/steam"), root);
    d.fs.add_symlink(format!("{UNIX_HOME}/.steam/root"), root);
}

fn linux_native() -> FakeDetectEnv {
    let d = unix_base();
    let root = format!("{UNIX_HOME}/.local/share/Steam");
    add_library(&d.fs, &root, Style::Linux, true);
    add_root(
        &d.fs,
        &root,
        Style::Linux,
        &[LibrarySpec {
            path: &root,
            apps: &[GAME_APP_ID],
        }],
    );
    link_dot_steam(&d, &root);
    d.fs.add_file(user_config(UNIX_HOME, ".config/unity3d"), PLACEHOLDER);
    d
}

fn linux_two_libs() -> FakeDetectEnv {
    let d = unix_base();
    let root = format!("{UNIX_HOME}/.local/share/Steam");
    let lib1 = "/mnt/SteamLibrary";
    add_library(&d.fs, &root, Style::Linux, false);
    add_library(&d.fs, lib1, Style::Linux, true);
    d.fs.add_file(
        format!("{lib1}/steamapps/appmanifest_281990.acf"),
        "\"AppState\"\n{\n\t\"appid\"\t\t\"281990\"\n}\n",
    );
    add_root(
        &d.fs,
        &root,
        Style::Linux,
        &[
            LibrarySpec {
                path: &root,
                apps: &[],
            },
            LibrarySpec {
                path: lib1,
                apps: &[GAME_APP_ID, 281_990],
            },
        ],
    );
    link_dot_steam(&d, &root);
    d.fs.add_file(user_config(UNIX_HOME, ".config/unity3d"), PLACEHOLDER);
    d
}

fn linux_flatpak() -> FakeDetectEnv {
    let d = unix_base();
    let app = format!("{UNIX_HOME}/.var/app/com.valvesoftware.Steam");
    let root = format!("{app}/.local/share/Steam");
    add_library(&d.fs, &root, Style::Linux, true);
    add_root(
        &d.fs,
        &root,
        Style::Linux,
        &[LibrarySpec {
            path: &root,
            apps: &[GAME_APP_ID],
        }],
    );
    d.fs.add_symlink(format!("{app}/.steam/steam"), &root);
    d.fs.add_file(user_config(&app, ".config/unity3d"), PLACEHOLDER);
    d.fs.add_file(user_config(&app, "config/unity3d"), PLACEHOLDER);
    d
}

fn linux_snap() -> FakeDetectEnv {
    let d = unix_base();
    let root = format!("{UNIX_HOME}/snap/steam/common/.local/share/Steam");
    add_library(&d.fs, &root, Style::Linux, true);
    add_root(
        &d.fs,
        &root,
        Style::Linux,
        &[LibrarySpec {
            path: &root,
            apps: &[GAME_APP_ID],
        }],
    );
    d.fs.add_file(
        user_config(&format!("{UNIX_HOME}/snap/steam/common"), ".config/unity3d"),
        PLACEHOLDER,
    );
    d
}

fn linux_proton(with_config: bool) -> FakeDetectEnv {
    let d = linux_native();
    let root = format!("{UNIX_HOME}/.local/share/Steam");
    let prefix = format!("{root}/steamapps/compatdata/{GAME_APP_ID}");
    d.fs.add_dir(&prefix);
    if with_config {
        d.fs.add_file(
            user_config(
                &format!("{prefix}/pfx/drive_c/users/steamuser"),
                "AppData/LocalLow",
            ),
            PLACEHOLDER,
        );
        d.fs.add_file(
            format!("{root}/config/config.vdf"),
            format!(
                "\"InstallConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t\t{{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{{\n\t\t\t\t\t\"{GAME_APP_ID}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"name\"\t\t\"proton_rs_test\"\n\t\t\t\t\t}}\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
            ),
        );
    }
    d
}

fn windows(hkcu: bool, hklm: bool, folder: bool) -> FakeDetectEnv {
    let d = FakeDetectEnv::empty(true);
    d.env.set_home(Some(Utf8PathBuf::from(WINDOWS_HOME)));
    d.env.set_var("USERPROFILE", "C:\\Users\\rs_user");
    d.env
        .set_var("APPDATA", "C:\\Users\\rs_user\\AppData\\Roaming");
    d.env
        .set_var("LOCALAPPDATA", "C:\\Users\\rs_user\\AppData\\Local");
    d.fs.set_default_volume(rimstudio_core::ports::VolumeClass::Ntfs);
    let root = "C:/Program Files (x86)/Steam";
    if hkcu {
        d.registry.set(
            Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
            "c:/program files (x86)/steam",
        );
    }
    if hklm {
        d.registry.set(
            Hive::LocalMachine,
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            "InstallPath",
            "C:\\Program Files (x86)\\Steam",
        );
    }
    if folder {
        add_library(&d.fs, root, Style::Windows, true);
        add_library(&d.fs, "D:/SteamLibrary", Style::Windows, false);
        let root_text = escaped(root);
        add_root(
            &d.fs,
            root,
            Style::Windows,
            &[
                LibrarySpec {
                    path: &root_text,
                    apps: &[GAME_APP_ID],
                },
                LibrarySpec {
                    path: "D:\\\\SteamLibrary",
                    apps: &[],
                },
            ],
        );
        d.fs.add_file("C:/Program Files (x86)/Steam/steam.exe", "");
    }
    d.fs.add_file(user_config(WINDOWS_HOME, "AppData/LocalLow"), PLACEHOLDER);
    d
}

fn macos() -> FakeDetectEnv {
    let d = FakeDetectEnv::empty(true);
    d.env.set_home(Some(Utf8PathBuf::from(MAC_HOME)));
    d.env.set_var("HOME", MAC_HOME);
    let root = format!("{MAC_HOME}/Library/Application Support/Steam");
    add_library(&d.fs, &root, Style::Mac, true);
    add_root(
        &d.fs,
        &root,
        Style::Mac,
        &[LibrarySpec {
            path: &root,
            apps: &[GAME_APP_ID],
        }],
    );
    d.fs.add_file(
        format!("{MAC_HOME}/Library/Application Support/RimWorld/Config/ModsConfig.xml"),
        PLACEHOLDER,
    );
    d
}

fn gog_linux() -> FakeDetectEnv {
    let d = unix_base();
    add_install(
        &d.fs,
        &format!("{UNIX_HOME}/GOG Games/RimWorld/game"),
        Style::Linux,
    );
    d.fs.add_file(user_config(UNIX_HOME, ".config/unity3d"), PLACEHOLDER);
    d
}

fn pathological() -> FakeDetectEnv {
    let d = unix_base();
    let root = format!("{UNIX_HOME}/.local/share/Steam");
    let offline = "/mnt/OfflineLibrary";
    d.fs.add_dir(format!("{root}/steamapps"));
    // A manifest whose install folder does not exist.
    d.fs.add_file(
        format!("{root}/steamapps/appmanifest_{GAME_APP_ID}.acf"),
        app_manifest_acf("MissingFolder"),
    );
    // A leftover temporary manifest of zero bytes.
    d.fs.add_file(
        format!("{root}/steamapps/appmanifest_{GAME_APP_ID}.acf.123.tmp"),
        "",
    );
    // A library file that starts with a byte order mark and lists an offline library.
    let text = format!(
        "\u{feff}{}",
        library_folders_vdf(&[
            LibrarySpec {
                path: &root,
                apps: &[GAME_APP_ID],
            },
            LibrarySpec {
                path: offline,
                apps: &[GAME_APP_ID],
            },
        ])
    );
    d.fs.add_file(format!("{root}/config/libraryfolders.vdf"), text.clone());
    d.fs.add_file(format!("{root}/steamapps/libraryfolders.vdf"), text);
    d.fs.add_symlink(
        format!("{UNIX_HOME}/.steam/steam"),
        format!("{UNIX_HOME}/.steam/missing-target"),
    );
    d.fs.add_symlink(format!("{UNIX_HOME}/.steam/root"), &root);
    // A second, non Steam install.
    add_install(
        &d.fs,
        &format!("{UNIX_HOME}/GOG Games/RimWorld/game"),
        Style::Linux,
    );
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use std::time::Duration;

    const D: Duration = Duration::from_millis(10);

    fn exists(d: &FakeDetectEnv, p: &str) -> bool {
        d.fs().exists(Utf8Path::new(p), D).unwrap()
    }

    #[test]
    fn every_preset_builds_and_has_a_unique_name() {
        let mut names: Vec<&str> = SteamPreset::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SteamPreset::ALL.len());
        for p in SteamPreset::ALL {
            let d = FakeDetectEnv::from_preset(p);
            assert!(d.home().is_some(), "{}", p.name());
        }
    }

    #[test]
    fn linux_native_has_the_documented_files_and_links() {
        let d = FakeDetectEnv::from_preset(SteamPreset::LinuxNative);
        let root = "/home/rs_user/.local/share/Steam";
        for rel in [
            "steamapps/appmanifest_294100.acf",
            "steamapps/common/RimWorld/Version.txt",
            "steamapps/common/RimWorld/RimWorldLinux",
            "steamapps/common/RimWorld/Data/Core/About/About.xml",
            "steamapps/workshop/content/294100/111/About/About.xml",
            "steamapps/workshop/appworkshop_294100.acf",
            "config/libraryfolders.vdf",
            "steamapps/libraryfolders.vdf",
        ] {
            assert!(exists(&d, &format!("{root}/{rel}")), "{rel}");
        }
        assert_eq!(
            d.fs()
                .canonicalize(Utf8Path::new("/home/rs_user/.steam/steam"), D)
                .unwrap(),
            root
        );
        assert!(exists(
            &d,
            "/home/rs_user/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"
        ));
        assert_eq!(d.env().var("HOME").as_deref(), Some("/home/rs_user"));
    }

    #[test]
    fn two_libs_puts_the_game_in_the_second_library_only() {
        let d = FakeDetectEnv::from_preset(SteamPreset::LinuxTwoLibs);
        assert!(!exists(
            &d,
            "/home/rs_user/.local/share/Steam/steamapps/appmanifest_294100.acf"
        ));
        assert!(exists(
            &d,
            "/mnt/SteamLibrary/steamapps/appmanifest_294100.acf"
        ));
        let vdf = d
            .fs()
            .read_to_string(
                Utf8Path::new("/home/rs_user/.local/share/Steam/config/libraryfolders.vdf"),
                D,
            )
            .unwrap();
        assert!(vdf.contains("/mnt/SteamLibrary"));
    }

    #[test]
    fn flatpak_has_both_user_folder_variants_and_snap_its_own_root() {
        let f = FakeDetectEnv::from_preset(SteamPreset::LinuxFlatpak);
        let app = "/home/rs_user/.var/app/com.valvesoftware.Steam";
        assert!(exists(
            &f,
            &format!(
                "{app}/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"
            )
        ));
        assert!(exists(
            &f,
            &format!(
                "{app}/config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"
            )
        ));
        let s = FakeDetectEnv::from_preset(SteamPreset::LinuxSnap);
        assert!(exists(
            &s,
            "/home/rs_user/snap/steam/common/.local/share/Steam/steamapps/appmanifest_294100.acf"
        ));
    }

    #[test]
    fn proton_presets_differ_in_the_prefix_content() {
        let full = FakeDetectEnv::from_preset(SteamPreset::LinuxProton);
        let empty = FakeDetectEnv::from_preset(SteamPreset::LinuxProtonEmpty);
        let cfg = "/home/rs_user/.local/share/Steam/steamapps/compatdata/294100/pfx/drive_c/users/steamuser/AppData/LocalLow/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml";
        assert!(exists(&full, cfg));
        assert!(!exists(&empty, cfg));
        assert!(exists(
            &empty,
            "/home/rs_user/.local/share/Steam/steamapps/compatdata/294100"
        ));
        let vdf = full
            .fs()
            .read_to_string(
                Utf8Path::new("/home/rs_user/.local/share/Steam/config/config.vdf"),
                D,
            )
            .unwrap();
        assert!(vdf.contains("CompatToolMapping"));
        // The Proton config file is newer than the native one.
        let native = full
            .fs()
            .metadata(Utf8Path::new("/home/rs_user/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"), D)
            .unwrap()
            .mtime_ns;
        let proton = full.fs().metadata(Utf8Path::new(cfg), D).unwrap().mtime_ns;
        assert!(proton > native);
    }

    #[test]
    fn windows_presets_set_the_registry_as_documented() {
        let reg = FakeDetectEnv::from_preset(SteamPreset::WindowsRegistry);
        assert_eq!(
            reg.registry()
                .read_string(Hive::CurrentUser, "Software\\Valve\\Steam", "SteamPath")
                .as_deref(),
            Some("c:/program files (x86)/steam")
        );
        assert!(
            reg.registry()
                .read_string(
                    Hive::LocalMachine,
                    "SOFTWARE\\WOW6432Node\\Valve\\Steam",
                    "InstallPath"
                )
                .is_some()
        );
        let hklm = FakeDetectEnv::from_preset(SteamPreset::WindowsHklmOnly);
        assert!(
            hklm.registry()
                .read_string(Hive::CurrentUser, "Software\\Valve\\Steam", "SteamPath")
                .is_none()
        );
        let none = FakeDetectEnv::from_preset(SteamPreset::WindowsDefault);
        assert!(
            none.registry()
                .read_string(
                    Hive::LocalMachine,
                    "SOFTWARE\\WOW6432Node\\Valve\\Steam",
                    "InstallPath"
                )
                .is_none()
        );
        assert!(exists(
            &none,
            "c:\\program files (x86)\\steam\\steamapps\\appmanifest_294100.acf"
        ));
        let vdf = reg
            .fs()
            .read_to_string(
                Utf8Path::new("C:/Program Files (x86)/Steam/config/libraryfolders.vdf"),
                D,
            )
            .unwrap();
        assert!(vdf.contains("D:\\\\SteamLibrary"), "{vdf}");
    }

    #[test]
    fn macos_has_the_app_bundle() {
        let d = FakeDetectEnv::from_preset(SteamPreset::MacOs);
        let game = "/Users/rs_user/Library/Application Support/Steam/steamapps/common/RimWorld/RimWorldMac.app";
        assert!(exists(&d, &format!("{game}/Version.txt")));
        assert!(exists(&d, &format!("{game}/Mods")));
        assert!(exists(
            &d,
            "/users/rs_user/library/application support/rimworld/config/modsconfig.xml"
        ));
    }

    #[test]
    fn pathological_tree_has_each_broken_piece() {
        let d = FakeDetectEnv::from_preset(SteamPreset::Pathological);
        let root = "/home/rs_user/.local/share/Steam";
        let tmp = d
            .fs()
            .read_to_string(
                Utf8Path::new(&format!("{root}/steamapps/appmanifest_294100.acf.123.tmp")),
                D,
            )
            .unwrap();
        assert!(tmp.is_empty());
        let vdf = d
            .fs()
            .read_to_string(
                Utf8Path::new(&format!("{root}/config/libraryfolders.vdf")),
                D,
            )
            .unwrap();
        assert!(vdf.starts_with('\u{feff}'));
        assert!(!exists(&d, "/mnt/OfflineLibrary"));
        assert!(!exists(&d, "/home/rs_user/.steam/steam"));
        assert!(exists(&d, "/home/rs_user/.steam/root"));
        assert!(!exists(
            &d,
            &format!("{root}/steamapps/common/MissingFolder")
        ));
        assert!(exists(
            &d,
            "/home/rs_user/GOG Games/RimWorld/game/Version.txt"
        ));
    }

    #[test]
    fn builder_helpers_edit_the_parts() {
        let d = FakeDetectEnv::empty(false)
            .with_file("/x/y.txt", "t")
            .with_dir("/z")
            .with_var("K", "v")
            .with_registry(Hive::CurrentUser, "A", "B", "c");
        assert!(exists(&d, "/x/y.txt") && exists(&d, "/z"));
        assert_eq!(d.env().var("K").as_deref(), Some("v"));
        assert_eq!(
            d.registry()
                .read_string(Hive::CurrentUser, "A", "B")
                .as_deref(),
            Some("c")
        );
        assert_eq!(d.clock().now_unix_ms(), FakeClock::DEFAULT_START_MS);
        assert_eq!(d.home(), None);
    }

    #[test]
    fn templates_have_the_documented_shape() {
        let v = library_folders_vdf(&[LibrarySpec {
            path: "/a",
            apps: &[1, 2],
        }]);
        assert!(v.starts_with("\"libraryfolders\"\n{\n\t\"0\""));
        assert!(v.contains("\"path\"\t\t\"/a\""));
        assert!(v.contains("\"2\"\t\t\"1000\""));
        assert!(app_manifest_acf("Dir").contains("\"installdir\"\t\t\"Dir\""));
        assert!(workshop_acf(&[5, 6]).contains("\"6\""));
        assert_eq!(SteamPreset::MacOs.os(), Os::MacOs);
        assert_eq!(SteamPreset::WindowsDefault.os(), Os::Windows);
        assert_eq!(SteamPreset::GogLinux.os(), Os::Linux);
    }
}
