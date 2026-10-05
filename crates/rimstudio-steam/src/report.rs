//! The detection report: serde friendly, deterministic types that describe everything detection
//! found, with provenance (`how`), confidence and warnings.
//!
//! The JSON form uses camelCase field names and kebab-case enum strings. Paths are always written
//! with `/` separators, also for Windows (the Windows APIs accept them), so a report is identical on
//! every host. Account identifiers never appear in a report; [`DetectionReport::redacted`] also
//! replaces the home folder and user name for reports that leave the process.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::redact::Redactor;
use rimstudio_core::version::GameVersion;
use serde::{Deserialize, Serialize};

use crate::launch::LaunchDescription;

/// The schema version of the report JSON.
pub const REPORT_SCHEMA: u32 = 1;

/// Stable warning codes. The UI translates them through its message catalogue.
pub mod codes {
    /// A Steam root candidate is a symbolic link whose target does not exist.
    pub const ROOT_DANGLING_LINK: &str = "steam.root-dangling-link";
    /// A library folder does not exist (a removable drive that is not connected).
    pub const LIBRARY_OFFLINE: &str = "steam.library-offline";
    /// Probing a library did not finish within the deadline.
    pub const LIBRARY_TIMEOUT: &str = "steam.library-timeout";
    /// Probing a path other than a library did not finish within the deadline.
    pub const PROBE_TIMEOUT: &str = "steam.probe-timeout";
    /// A `libraryfolders.vdf` could not be read or parsed.
    pub const LIBRARYFOLDERS_UNREADABLE: &str = "steam.libraryfolders-unreadable";
    /// An ACF is empty, unreadable, unparseable or names another app.
    pub const ACF_UNREADABLE: &str = "steam.acf-unreadable";
    /// A leftover temporary copy of the app manifest sits next to the real one.
    pub const MANIFEST_LEFTOVER: &str = "steam.manifest-leftover";
    /// A library's apps table lists the game but no manifest exists.
    pub const APPS_TABLE_STALE: &str = "steam.apps-table-stale";
    /// The manifest names an install folder that does not exist.
    pub const INSTALL_DIR_MISSING: &str = "steam.install-dir-missing";
    /// The install folder exists but lacks `Data/Core/About/About.xml`.
    pub const INSTALL_CONTENT_INVALID: &str = "steam.install-content-invalid";
    /// Steam is updating the game or has an update queued.
    pub const INSTALL_UPDATE_PENDING: &str = "steam.install-update-pending";
    /// The manifest says the install is incomplete, or files are missing or corrupt.
    pub const INSTALL_NEEDS_VERIFY: &str = "steam.install-needs-verify";
    /// The install folder is a symbolic link.
    pub const INSTALL_IS_SYMLINK: &str = "steam.install-is-symlink";
    /// `Version.txt` is missing.
    pub const VERSION_MISSING: &str = "steam.version-missing";
    /// `Version.txt` exists but is not a version.
    pub const VERSION_UNPARSEABLE: &str = "steam.version-unparseable";
    /// More than one install was found.
    pub const MULTIPLE_INSTALLS: &str = "steam.multiple-installs";
    /// The selected user folder's `ModsConfig.xml` was written by another game version.
    pub const USERDIR_VERSION_MISMATCH: &str = "steam.userdir-version-mismatch";
    /// A native and a Proton user folder both exist.
    pub const PROTON_AND_NATIVE: &str = "steam.proton-and-native-userdirs";
    /// The game's install and its Workshop content are in different libraries.
    pub const WORKSHOP_DIFFERENT_LIBRARY: &str = "steam.workshop-different-library";
    /// A user override failed validation. It is kept and detection is used for this run.
    pub const OVERRIDE_INVALID: &str = "deploy.override-invalid";
}

/// Why a candidate was found. A closed set; a UI shows an unknown value (from a newer build) as its
/// raw string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum How {
    /// The user's override.
    #[serde(rename = "override")]
    Override,
    /// The registry value `HKCU\Software\Valve\Steam\SteamPath`.
    #[serde(rename = "registry-hkcu")]
    RegistryHkcu,
    /// A machine wide registry value `InstallPath`.
    #[serde(rename = "registry-hklm")]
    RegistryHklm,
    /// `$XDG_DATA_HOME/Steam` or its default.
    #[serde(rename = "xdg-data-home")]
    XdgDataHome,
    /// The compatibility links `~/.steam/steam` and `~/.steam/root`.
    #[serde(rename = "symlink-steam")]
    SymlinkSteam,
    /// The Flatpak Steam folders.
    #[serde(rename = "flatpak")]
    Flatpak,
    /// The Snap Steam folders.
    #[serde(rename = "snap")]
    Snap,
    /// Listed in a `libraryfolders.vdf`.
    #[serde(rename = "libraryfolders.vdf")]
    LibraryFoldersVdf,
    /// Named by `appmanifest_294100.acf`.
    #[serde(rename = "appmanifest")]
    Appmanifest,
    /// Found by probing a well known folder.
    #[serde(rename = "directory-probe")]
    DirectoryProbe,
}

impl How {
    /// The stable string of the value, the same as its JSON form.
    pub fn as_str(self) -> &'static str {
        match self {
            How::Override => "override",
            How::RegistryHkcu => "registry-hkcu",
            How::RegistryHklm => "registry-hklm",
            How::XdgDataHome => "xdg-data-home",
            How::SymlinkSteam => "symlink-steam",
            How::Flatpak => "flatpak",
            How::Snap => "snap",
            How::LibraryFoldersVdf => "libraryfolders.vdf",
            How::Appmanifest => "appmanifest",
            How::DirectoryProbe => "directory-probe",
        }
    }
}

/// How sure detection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// Name based or stale.
    Low,
    /// Content test only, or a well known folder.
    Medium,
    /// Manifest plus content test, or a user override that validated.
    High,
}

/// The kind of an install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallKind {
    /// A Steam install.
    Steam,
    /// A GOG copy.
    Gog,
    /// Any other copy found by content.
    Manual,
    /// A folder the user chose.
    Override,
}

impl InstallKind {
    /// The stable string.
    pub fn as_str(self) -> &'static str {
        match self {
            InstallKind::Steam => "steam",
            InstallKind::Gog => "gog",
            InstallKind::Manual => "manual",
            InstallKind::Override => "override",
        }
    }
}

/// The health of a Steam install according to its manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Health {
    /// Fully installed, nothing pending (also the value for installs without a manifest).
    Installed,
    /// An update is running or queued: do not treat the files as final.
    UpdatePending,
    /// Incomplete, or files missing or corrupt.
    NeedsVerify,
}

/// The kind of a user data folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UserDirKind {
    /// The Unity folder of a native Linux run.
    Native,
    /// Inside a Proton prefix.
    Proton,
    /// Inside the Flatpak Steam home.
    Flatpak,
    /// Inside the Snap Steam home.
    Snap,
    /// The macOS folder.
    Macos,
    /// The Windows `LocalLow` folder.
    Windows,
    /// A folder the user chose.
    Override,
}

/// What was checked on a Steam root candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootChecks {
    /// The folder exists.
    pub exists: bool,
    /// `steamapps` exists inside it.
    pub has_steamapps: bool,
    /// `config/libraryfolders.vdf` exists inside it.
    pub has_library_folders: bool,
}

/// A Steam root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamRoot {
    /// The path of the first candidate that led here.
    pub path: Utf8PathBuf,
    /// The canonical path (links resolved), the identity of the root.
    pub canonical: Utf8PathBuf,
    /// Why it was found.
    pub how: How,
    /// How sure detection is.
    pub confidence: Confidence,
    /// The validity checks.
    pub checks: RootChecks,
}

/// A Steam library folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// The path as normalised.
    pub path: Utf8PathBuf,
    /// The canonical path when it could be resolved, else the normalised path.
    pub canonical: Utf8PathBuf,
    /// Why it is listed.
    pub how: How,
    /// The canonical path of the Steam root whose files listed it.
    pub root: Utf8PathBuf,
    /// The folder exists and was reachable in time.
    pub online: bool,
    /// Probing it timed out.
    pub timed_out: bool,
    /// The user label.
    pub label: String,
    /// A valid app manifest for the game exists here.
    pub has_app: bool,
    /// The apps table lists the game but no manifest exists (or the reverse).
    pub stale: bool,
}

/// A game version as numbers plus the original text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// The text of `Version.txt`.
    pub raw: String,
    /// The major number.
    pub major: u32,
    /// The minor number.
    pub minor: u32,
    /// The build number.
    pub build: Option<u32>,
    /// The revision.
    pub rev: Option<u32>,
}

impl From<&GameVersion> for VersionInfo {
    fn from(v: &GameVersion) -> Self {
        VersionInfo {
            raw: v.raw.clone(),
            major: v.major,
            minor: v.minor,
            build: v.build,
            rev: v.rev,
        }
    }
}

/// A folder and what is known about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirCheck {
    /// The path.
    pub path: Utf8PathBuf,
    /// It exists and is a directory.
    pub exists: bool,
    /// Its entries can be listed.
    pub readable: bool,
}

/// Workshop content of one library, or an extra folder from the overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopDirInfo {
    /// The library that holds the content, `None` for an extra folder.
    pub library: Option<Utf8PathBuf>,
    /// The content folder (`.../workshop/content/294100`).
    pub content_dir: Utf8PathBuf,
    /// The workshop ACF when it exists.
    pub acf: Option<Utf8PathBuf>,
    /// Numeric item folders on disk.
    pub items_on_disk: usize,
    /// Items the ACF lists as installed, `None` without a readable ACF.
    pub items_in_acf: Option<usize>,
    /// Why it is listed.
    pub how: How,
}

/// A game install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    /// The stable id, `kind:canonical path` (the library for Steam installs).
    pub id: String,
    /// The kind.
    pub kind: InstallKind,
    /// The install folder.
    pub path: Utf8PathBuf,
    /// The folder that holds `Data` and `Mods` (the app bundle on macOS, else the install folder).
    pub game_root: Utf8PathBuf,
    /// Why it was found.
    pub how: How,
    /// How sure detection is.
    pub confidence: Confidence,
    /// The version from `Version.txt`.
    pub version: Option<VersionInfo>,
    /// The installed build id from the manifest.
    pub build_id: Option<String>,
    /// The pending target build id from the manifest.
    pub target_build_id: Option<String>,
    /// The manifest's state flags.
    pub state_flags: Option<u64>,
    /// The health.
    pub health: Health,
    /// The executable or bundle found in the install folder.
    pub executable: Option<Utf8PathBuf>,
    /// How to start the game.
    pub launch: LaunchDescription,
    /// The mods folder.
    pub mods_dir: DirCheck,
    /// The folder names under `Data`, sorted.
    pub data_dirs: Vec<String>,
    /// The game runs through a Proton prefix.
    pub proton: bool,
    /// The canonical library path for Steam installs.
    pub library: Option<Utf8PathBuf>,
    /// The install folder is a symbolic link.
    pub is_symlink: bool,
    /// Workshop content: the install's own library first, then the others. Empty for non-Steam installs.
    pub workshop: Vec<WorkshopDirInfo>,
}

/// What is known about a user folder's `ModsConfig.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsConfigInfo {
    /// The file exists.
    pub exists: bool,
    /// Modification time in milliseconds since the epoch.
    pub mtime_ms: Option<i64>,
    /// The `<version>` text the game wrote (the version of the last run, not of the install).
    pub game_version: Option<String>,
}

/// A user data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDir {
    /// The folder.
    pub path: Utf8PathBuf,
    /// The kind.
    pub kind: UserDirKind,
    /// The `Config/ModsConfig.xml` state.
    pub mods_config: ModsConfigInfo,
    /// The log file when it exists.
    pub player_log: Option<Utf8PathBuf>,
    /// `Config/Prefs.xml` when it exists.
    pub prefs: Option<Utf8PathBuf>,
    /// This folder has the newest `ModsConfig.xml` of all candidates.
    pub newest: bool,
}

/// The default choices of a run.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selected {
    /// The id of the selected install.
    pub install: Option<String>,
    /// The path of the selected user folder.
    pub user_dir: Option<Utf8PathBuf>,
}

/// A problem or note, with a stable code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// The stable code, see [`codes`].
    pub code: String,
    /// An English sentence for logs; the UI translates the code.
    pub message: String,
    /// The paths the warning is about.
    pub paths: Vec<Utf8PathBuf>,
}

/// The result of a detection run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionReport {
    /// The schema version, [`REPORT_SCHEMA`].
    pub schema: u32,
    /// When the run happened, milliseconds since the epoch (from the clock port).
    pub generated_at_ms: u64,
    /// The operating system the run assumed.
    pub os: Os,
    /// Every valid Steam root, deduplicated by canonical path, in candidate order.
    pub steam_roots: Vec<SteamRoot>,
    /// Every library, deduplicated, in root and file order.
    pub libraries: Vec<Library>,
    /// Every accepted install, best first.
    pub installs: Vec<Install>,
    /// Every user folder candidate that exists, in candidate order.
    pub user_dirs: Vec<UserDir>,
    /// The default choices.
    pub selected: Selected,
    /// Problems and notes, sorted by code then path.
    pub warnings: Vec<Warning>,
}

impl DetectionReport {
    /// The install with this id.
    pub fn install(&self, id: &str) -> Option<&Install> {
        self.installs.iter().find(|i| i.id == id)
    }

    /// The selected install.
    pub fn selected_install(&self) -> Option<&Install> {
        self.install(self.selected.install.as_deref()?)
    }

    /// The selected user folder.
    pub fn selected_user_dir(&self) -> Option<&UserDir> {
        let path = self.selected.user_dir.as_ref()?;
        self.user_dirs.iter().find(|u| &u.path == path)
    }

    /// True when a warning with this code exists.
    pub fn has_warning(&self, code: &str) -> bool {
        self.warnings.iter().any(|w| w.code == code)
    }

    /// A copy safe to log or send out of the process: the home folder, the user name and any
    /// account identifier are replaced by the redactor in every path, id and message.
    #[must_use]
    pub fn redacted(&self, redactor: &Redactor) -> DetectionReport {
        let path = |p: &Utf8Path| Utf8PathBuf::from(redactor.redact(p.as_str()));
        let opt = |p: &Option<Utf8PathBuf>| p.as_deref().map(path);
        DetectionReport {
            schema: self.schema,
            generated_at_ms: self.generated_at_ms,
            os: self.os,
            steam_roots: self
                .steam_roots
                .iter()
                .map(|r| SteamRoot {
                    path: path(&r.path),
                    canonical: path(&r.canonical),
                    how: r.how,
                    confidence: r.confidence,
                    checks: r.checks.clone(),
                })
                .collect(),
            libraries: self
                .libraries
                .iter()
                .map(|l| Library {
                    path: path(&l.path),
                    canonical: path(&l.canonical),
                    how: l.how,
                    root: path(&l.root),
                    online: l.online,
                    timed_out: l.timed_out,
                    label: redactor.redact(&l.label),
                    has_app: l.has_app,
                    stale: l.stale,
                })
                .collect(),
            installs: self
                .installs
                .iter()
                .map(|i| Install {
                    id: redactor.redact(&i.id),
                    kind: i.kind,
                    path: path(&i.path),
                    game_root: path(&i.game_root),
                    how: i.how,
                    confidence: i.confidence,
                    version: i.version.clone(),
                    build_id: i.build_id.clone(),
                    target_build_id: i.target_build_id.clone(),
                    state_flags: i.state_flags,
                    health: i.health,
                    executable: opt(&i.executable),
                    launch: LaunchDescription {
                        steam_url: i.launch.steam_url.clone(),
                        executable: opt(&i.launch.executable),
                        working_dir: path(&i.launch.working_dir),
                        args: i.launch.args.iter().map(|a| redactor.redact(a)).collect(),
                        steam_only: i.launch.steam_only,
                    },
                    mods_dir: DirCheck {
                        path: path(&i.mods_dir.path),
                        exists: i.mods_dir.exists,
                        readable: i.mods_dir.readable,
                    },
                    data_dirs: i.data_dirs.clone(),
                    proton: i.proton,
                    library: opt(&i.library),
                    is_symlink: i.is_symlink,
                    workshop: i
                        .workshop
                        .iter()
                        .map(|w| WorkshopDirInfo {
                            library: opt(&w.library),
                            content_dir: path(&w.content_dir),
                            acf: opt(&w.acf),
                            items_on_disk: w.items_on_disk,
                            items_in_acf: w.items_in_acf,
                            how: w.how,
                        })
                        .collect(),
                })
                .collect(),
            user_dirs: self
                .user_dirs
                .iter()
                .map(|u| UserDir {
                    path: path(&u.path),
                    kind: u.kind,
                    mods_config: u.mods_config.clone(),
                    player_log: opt(&u.player_log),
                    prefs: opt(&u.prefs),
                    newest: u.newest,
                })
                .collect(),
            selected: Selected {
                install: self.selected.install.as_deref().map(|s| redactor.redact(s)),
                user_dir: opt(&self.selected.user_dir),
            },
            warnings: self
                .warnings
                .iter()
                .map(|w| Warning {
                    code: w.code.clone(),
                    message: redactor.redact(&w.message),
                    paths: w.paths.iter().map(|p| path(p)).collect(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_strings_match_their_json_form() {
        for how in [
            How::Override,
            How::RegistryHkcu,
            How::RegistryHklm,
            How::XdgDataHome,
            How::SymlinkSteam,
            How::Flatpak,
            How::Snap,
            How::LibraryFoldersVdf,
            How::Appmanifest,
            How::DirectoryProbe,
        ] {
            let json = serde_json::to_string(&how).unwrap();
            assert_eq!(json, format!("\"{}\"", how.as_str()));
        }
    }

    #[test]
    fn confidence_orders_low_to_high() {
        assert!(Confidence::Low < Confidence::Medium);
        assert!(Confidence::Medium < Confidence::High);
        assert_eq!(
            serde_json::to_string(&Confidence::High).unwrap(),
            "\"high\""
        );
    }

    #[test]
    fn redaction_replaces_the_home_folder_in_paths_and_ids() {
        let report = DetectionReport {
            schema: REPORT_SCHEMA,
            generated_at_ms: 1,
            os: Os::Linux,
            steam_roots: vec![],
            libraries: vec![],
            installs: vec![],
            user_dirs: vec![],
            selected: Selected {
                install: Some("steam:/home/rs_person/lib".into()),
                user_dir: Some("/home/rs_person/.config/x".into()),
            },
            warnings: vec![Warning {
                code: "x.y".into(),
                message: "see /home/rs_person/a".into(),
                paths: vec!["/home/rs_person/a".into()],
            }],
        };
        let red = Redactor::new().with_home("/home/rs_person");
        let out = report.redacted(&red);
        let json = serde_json::to_string(&out).unwrap();
        assert!(!json.contains("rs_person"), "{json}");
        assert!(json.contains("steam:~/lib"));
    }
}
