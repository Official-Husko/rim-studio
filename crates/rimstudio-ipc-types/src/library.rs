//! Detection, mod sources and library scan DTOs.
//!
//! `DetectionReportDto` mirrors the report of `rimstudio-steam` with paths as strings and enum strings in
//! kebab case. Source DTOs describe the install, workshop and custom folders with their status. The scan
//! result carries statistics and the diagnostic summary, never the list of mods (the mod list travels in
//! its own snapshot).

use serde::{Deserialize, Serialize};

use crate::diagnostic::{DiagnosticDto, DiagnosticSummaryDto};
use crate::library_facts::{CeInLibraryDto, DuplicateGroupsDto, LibraryCountsDto};
use crate::settings::FolderLayoutDto;

/// Operating system of the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum OsDto {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux and Steam Deck.
    Linux,
    /// Anything else.
    Other,
}

/// How a candidate was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum HowDto {
    /// The user chose it.
    Override,
    /// Windows registry, current user.
    RegistryHkcu,
    /// Windows registry, local machine.
    RegistryHklm,
    /// The XDG data home.
    XdgDataHome,
    /// The legacy Steam symlink.
    SymlinkSteam,
    /// A Flatpak Steam.
    Flatpak,
    /// A Snap Steam.
    Snap,
    /// Listed in `libraryfolders.vdf`.
    LibraryFoldersVdf,
    /// Found through an app manifest.
    Appmanifest,
    /// Found by probing well known folders.
    DirectoryProbe,
}

/// How much to trust a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ConfidenceDto {
    /// A guess.
    Low,
    /// Plausible.
    Medium,
    /// Confirmed by several signals.
    High,
}

/// Where an install came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum InstallKindDto {
    /// Steam.
    Steam,
    /// GOG.
    Gog,
    /// A manual copy.
    Manual,
    /// The user's override.
    Override,
}

/// Update state of an install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum HealthDto {
    /// Fully installed.
    Installed,
    /// An update is pending.
    UpdatePending,
    /// Needs file verification.
    NeedsVerify,
}

/// Kind of a user data folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum UserDirKindDto {
    /// Native location.
    Native,
    /// Inside a Proton prefix.
    Proton,
    /// Flatpak sandbox.
    Flatpak,
    /// Snap sandbox.
    Snap,
    /// macOS library folder.
    Macos,
    /// Windows AppData.
    Windows,
    /// The user's override.
    Override,
}

/// A Steam root candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SteamRootDto {
    /// Path as found.
    pub path: String,
    /// Canonical path.
    pub canonical: String,
    /// How it was found.
    pub how: HowDto,
    /// Confidence.
    pub confidence: ConfidenceDto,
    /// The folder exists.
    pub exists: bool,
    /// The folder has a `steamapps` child.
    pub has_steamapps: bool,
    /// `libraryfolders.vdf` exists.
    pub has_library_folders: bool,
}

/// A Steam library folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryDto {
    /// Path as listed.
    pub path: String,
    /// Canonical path.
    pub canonical: String,
    /// How it was found.
    pub how: HowDto,
    /// The Steam root that lists it.
    pub root: String,
    /// The volume is reachable.
    pub online: bool,
    /// Probing the volume timed out.
    pub timed_out: bool,
    /// Library label; empty when none.
    pub label: String,
    /// The library holds the game.
    pub has_app: bool,
    /// The listing may be out of date.
    pub stale: bool,
}

/// A parsed game version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VersionDto {
    /// The raw text.
    pub raw: String,
    /// Major number.
    pub major: u32,
    /// Minor number.
    pub minor: u32,
    /// Build number. Absent when the text has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub build: Option<u32>,
    /// Revision number. Absent when the text has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rev: Option<u32>,
}

/// How to start the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LaunchDto {
    /// Steam URL. Absent when the game cannot be started through Steam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_url: Option<String>,
    /// Executable. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub executable: Option<String>,
    /// Working directory.
    pub working_dir: String,
    /// Default arguments.
    pub args: Vec<String>,
    /// True when the game must be started through Steam.
    pub steam_only: bool,
}

/// A folder check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DirCheckDto {
    /// The path.
    pub path: String,
    /// The folder exists.
    pub exists: bool,
    /// The folder can be read.
    pub readable: bool,
}

/// A Workshop content folder tied to an install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WorkshopDirDto {
    /// The Steam library that holds it. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub library: Option<String>,
    /// The content folder.
    pub content_dir: String,
    /// The workshop manifest. Absent when none was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub acf: Option<String>,
    /// Item folders on disk.
    pub items_on_disk: u32,
    /// Items listed in the manifest. Absent when there is no manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub items_in_acf: Option<u32>,
    /// How it was found.
    pub how: HowDto,
}

/// A RimWorld install candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InstallDto {
    /// Stable id of the candidate.
    pub id: String,
    /// Kind.
    pub kind: InstallKindDto,
    /// The install folder as found.
    pub path: String,
    /// The folder that holds the game data.
    pub game_root: String,
    /// How it was found.
    pub how: HowDto,
    /// Confidence.
    pub confidence: ConfidenceDto,
    /// Game version. Absent when it could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub version: Option<VersionDto>,
    /// Installed build id (a Steam build id as text). Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub build_id: Option<String>,
    /// Target build id. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub target_build_id: Option<String>,
    /// Steam state flags. Absent when there is no manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub state_flags: Option<u32>,
    /// Update state.
    pub health: HealthDto,
    /// How to start it.
    pub launch: LaunchDto,
    /// The `Mods` folder check.
    pub mods_dir: DirCheckDto,
    /// Data folders found.
    pub data_dirs: Vec<String>,
    /// The install runs through Proton.
    pub proton: bool,
    /// The Steam library that holds it. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub library: Option<String>,
    /// The install folder is a symbolic link.
    pub is_symlink: bool,
    /// Workshop folders tied to the install.
    pub workshop: Vec<WorkshopDirDto>,
}

/// A user data folder candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UserDirDto {
    /// The path.
    pub path: String,
    /// Kind.
    pub kind: UserDirKindDto,
    /// `ModsConfig.xml` exists.
    pub mods_config_exists: bool,
    /// Modification time of `ModsConfig.xml` in Unix milliseconds. Absent when it does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mods_config_mtime_ms: Option<i64>,
    /// The game version recorded in `ModsConfig.xml`. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_version: Option<String>,
    /// The player log. Absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub player_log: Option<String>,
    /// The preferences file. Absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub prefs: Option<String>,
    /// True for the most recently used candidate.
    pub newest: bool,
}

/// The selected candidates.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedDto {
    /// Id of the selected install. Absent when none qualifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub install: Option<String>,
    /// Selected user data folder. Absent when none qualifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub user_dir: Option<String>,
}

/// A detection warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DetectionWarningDto {
    /// Stable code.
    pub code: String,
    /// English fallback text.
    pub message: String,
    /// Paths involved.
    pub paths: Vec<String>,
}

/// Everything detection found. Result of the `detect_run` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DetectionReportDto {
    /// Schema version of the report.
    pub schema: u32,
    /// Unix time in milliseconds when the report was made.
    pub generated_at_ms: u64,
    /// Operating system.
    pub os: OsDto,
    /// Steam roots.
    pub steam_roots: Vec<SteamRootDto>,
    /// Steam libraries.
    pub libraries: Vec<SteamLibraryDto>,
    /// Install candidates.
    pub installs: Vec<InstallDto>,
    /// User data folder candidates.
    pub user_dirs: Vec<UserDirDto>,
    /// The selection.
    pub selected: SelectedDto,
    /// Warnings.
    pub warnings: Vec<DetectionWarningDto>,
}

/// Request of the `detect_run` job.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DetectRunRequest {
    /// True to ignore the cached report and probe again.
    pub force: bool,
}

/// Request of `detect_get_report`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DetectGetReportRequest {}

/// Response of `detect_get_report`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DetectGetReportResponse {
    /// The last report. Absent when detection never ran in this installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub report: Option<DetectionReportDto>,
}

/// Which path a manual override sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum PathFieldDto {
    /// The game install.
    GameInstall,
    /// The user data folder.
    UserDir,
    /// The Steam root.
    SteamRoot,
}

/// Request of `detect_set_override`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DetectSetOverrideRequest {
    /// The path field to set.
    pub field: PathFieldDto,
    /// The chosen path. Absent clears the override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub path: Option<String>,
}

/// Kind of a mod source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SourceKindDto {
    /// The install `Data` folder.
    GameData,
    /// The install `Mods` folder.
    GameMods,
    /// A Steam Workshop content folder.
    Workshop,
    /// A user added folder.
    Custom,
}

/// Status of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SourceStatusDto {
    /// Ready to scan.
    Ready,
    /// Switched off.
    Disabled,
    /// The volume is not reachable.
    Offline,
    /// The path is not a folder.
    NotDirectory,
}

/// One mod source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    /// Stable source id.
    pub id: String,
    /// Kind.
    pub kind: SourceKindDto,
    /// Absolute path.
    pub path: String,
    /// Display label; empty means derive from the path.
    pub label: String,
    /// Whether the source takes part in scans.
    pub enabled: bool,
    /// Status.
    pub status: SourceStatusDto,
    /// Mods found at the last scan. Absent before the first scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mod_count: Option<u32>,
    /// Layout (custom folders only; others are `auto`).
    pub layout: FolderLayoutDto,
    /// Scan depth, 1 to 4.
    pub scan_depth: u8,
    /// True when the app never writes into the source.
    pub read_only: bool,
}

/// Request of `sources_list`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SourcesListRequest {}

/// Response of `sources_list`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesListResponse {
    /// The sources, in scan order.
    pub sources: Vec<SourceDto>,
}

/// Request of `sources_add_folder`; the response is a [`SourceDto`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesAddFolderRequest {
    /// The folder the user picked.
    pub path: String,
    /// Display label. Absent means derive from the folder name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// Layout. Absent means use the probe suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub layout: Option<FolderLayoutDto>,
    /// Scan depth. Absent means use the probe suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scan_depth: Option<u8>,
}

/// Request of `sources_update`; absent members stay unchanged. The response is a [`SourceDto`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesUpdateRequest {
    /// The source to change.
    pub id: String,
    /// New label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// New enabled flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub enabled: Option<bool>,
    /// New position in the scan order, zero based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub order: Option<u32>,
    /// New layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub layout: Option<FolderLayoutDto>,
    /// New scan depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scan_depth: Option<u8>,
}

/// Request of `sources_remove`; files are never touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesRemoveRequest {
    /// The source to forget.
    pub id: String,
}

/// Response of `sources_remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesRemoveResponse {
    /// True when a source was removed; false when the id was unknown.
    pub removed: bool,
}

/// Request of `sources_probe_folder`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesProbeFolderRequest {
    /// The folder to look at.
    pub path: String,
}

/// What a probed folder looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum FolderKindDto {
    /// Does not exist.
    Missing,
    /// Not a folder.
    NotDirectory,
    /// A game `Data` folder.
    GameData,
    /// A game `Mods` folder.
    GameMods,
    /// A Workshop content folder.
    Workshop,
    /// One mod.
    SingleMod,
    /// A folder of mods.
    ModsRoot,
    /// Empty.
    Empty,
}

/// A warning about a probed folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum FolderWarningDto {
    /// No mods found.
    NoModsFound,
    /// Mods exist deeper than the suggested depth would reach.
    ModsFoundDeeper,
    /// The layout is ambiguous.
    AmbiguousLayout,
    /// The path is a link.
    PathIsLink,
    /// The folder is unreachable.
    Unreachable,
}

/// How a probed folder relates to a registered source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum OverlapRelationDto {
    /// The same folder.
    Same,
    /// Inside the source.
    Inside,
    /// Contains the source.
    Contains,
}

/// An overlap with a registered source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceOverlapDto {
    /// The registered source.
    pub with: String,
    /// The relation.
    pub relation: OverlapRelationDto,
    /// Stable diagnostic code.
    pub code: String,
    /// True when the overlap blocks adding the folder.
    pub hard: bool,
}

/// Response of `sources_probe_folder`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourcesProbeFolderResponse {
    /// What the folder looks like.
    pub kind: FolderKindDto,
    /// Number of mods found within the suggested depth.
    pub mod_count: u32,
    /// Suggested scan depth.
    pub suggested_depth: u8,
    /// Suggested layout.
    pub suggested_layout: FolderLayoutDto,
    /// Warnings.
    pub warnings: Vec<FolderWarningDto>,
    /// Overlaps with registered sources.
    pub overlaps: Vec<SourceOverlapDto>,
    /// Content problems.
    pub diagnostics: Vec<DiagnosticDto>,
    /// True when the folder can be added.
    pub can_save: bool,
}

/// Request of the `library_scan` job.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryScanRequest {
    /// The library to scan. Absent means the default library.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub library_id: Option<String>,
    /// True to ignore the cache and read every file again.
    pub full: bool,
}

/// Scan counters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ScanStatsDto {
    /// Folders probed.
    pub folders_probed: u64,
    /// Mods found.
    pub mods_found: u64,
    /// Mods indexed.
    pub mods_indexed: u64,
    /// `About.xml` files parsed.
    pub about_parsed: u64,
    /// `About.xml` results reused from the cache.
    pub about_reused: u64,
    /// `LoadFolders.xml` files parsed.
    pub load_folders_parsed: u64,
    /// `LoadFolders.xml` results reused.
    pub load_folders_reused: u64,
    /// Definition files parsed.
    pub def_files_parsed: u64,
    /// Definition files reused.
    pub def_files_reused: u64,
    /// Definitions found.
    pub defs: u64,
    /// Files hashed.
    pub files_hashed: u64,
    /// Bytes read.
    pub bytes_read: u64,
    /// Directory errors.
    pub dir_errors: u64,
    /// Non UTF-8 names skipped.
    pub non_utf8: u64,
}

/// Scan phase timings in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ScanTimingsDto {
    /// Discovery.
    pub discover_ms: u64,
    /// Metadata.
    pub metadata_ms: u64,
    /// Definitions.
    pub definitions_ms: u64,
    /// Total.
    pub total_ms: u64,
}

/// Per source scan report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceReportDto {
    /// Source id.
    pub id: String,
    /// Kind.
    pub kind: SourceKindDto,
    /// Path.
    pub path: String,
    /// Status.
    pub status: SourceStatusDto,
    /// Mods found.
    pub mods: u32,
    /// Mods of this source that the game can load as they are. Absent in a result of an older backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub loadable: Option<u32>,
    /// Mods of this source that exist only in a custom folder and need a link. Absent in a result of an
    /// older backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub custom_only: Option<u32>,
}

/// Result of the `library_scan` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanResult {
    /// Revision of the list after the scan.
    pub rev: u64,
    /// Counters.
    pub stats: ScanStatsDto,
    /// Timings.
    pub timings: ScanTimingsDto,
    /// One entry per source, in scan order.
    pub sources: Vec<SourceReportDto>,
    /// Diagnostic counts and samples.
    pub diagnostics: DiagnosticSummaryDto,
    /// True when the scan was cancelled and the index is partial.
    pub cancelled: bool,
    /// Counts over the library: loadable, custom only, duplicates and the rest. Absent in a result of an
    /// older backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub counts: Option<LibraryCountsDto>,
    /// The duplicate groups, capped; `total` says how many there are. Absent in a result of an older
    /// backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub duplicates: Option<DuplicateGroupsDto>,
    /// Whether Combat Extended is in the scanned library. Absent in a result of an older backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ce_in_library: Option<CeInLibraryDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detection() -> DetectionReportDto {
        DetectionReportDto {
            schema: 1,
            generated_at_ms: 1_700_000_000_000,
            os: OsDto::Linux,
            steam_roots: vec![SteamRootDto {
                path: "/fiction/steam".into(),
                canonical: "/fiction/steam".into(),
                how: HowDto::XdgDataHome,
                confidence: ConfidenceDto::High,
                exists: true,
                has_steamapps: true,
                has_library_folders: true,
            }],
            libraries: vec![],
            installs: vec![InstallDto {
                id: "steam-1".into(),
                kind: InstallKindDto::Steam,
                path: "/fiction/steam/RS_Game".into(),
                game_root: "/fiction/steam/RS_Game".into(),
                how: HowDto::LibraryFoldersVdf,
                confidence: ConfidenceDto::High,
                version: Some(VersionDto {
                    raw: "9.9.1 rev1".into(),
                    major: 9,
                    minor: 9,
                    build: Some(1),
                    rev: Some(1),
                }),
                build_id: Some("123".into()),
                target_build_id: None,
                state_flags: Some(4),
                health: HealthDto::Installed,
                launch: LaunchDto {
                    steam_url: None,
                    executable: Some("/fiction/steam/RS_Game/run".into()),
                    working_dir: "/fiction/steam/RS_Game".into(),
                    args: vec![],
                    steam_only: false,
                },
                mods_dir: DirCheckDto {
                    path: "/fiction/steam/RS_Game/Mods".into(),
                    exists: true,
                    readable: true,
                },
                data_dirs: vec!["Data".into()],
                proton: false,
                library: None,
                is_symlink: false,
                workshop: vec![],
            }],
            user_dirs: vec![],
            selected: SelectedDto {
                install: Some("steam-1".into()),
                user_dir: None,
            },
            warnings: vec![],
        }
    }

    #[test]
    fn detection_report_round_trips_and_omits_unknown_values() {
        let report = detection();
        let text = serde_json::to_string(&report).unwrap_or_default();
        assert!(!text.contains("targetBuildId"));
        assert!(text.contains("\"how\":\"library-folders-vdf\""));
        let back: Result<DetectionReportDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(report));
    }

    #[test]
    fn scan_request_defaults_from_empty_object() {
        let request: Result<LibraryScanRequest, _> = serde_json::from_str("{}");
        assert_eq!(request.ok(), Some(LibraryScanRequest::default()));
    }

    #[test]
    fn override_request_without_path_means_clear() {
        let request = DetectSetOverrideRequest {
            field: PathFieldDto::GameInstall,
            path: None,
        };
        assert_eq!(
            serde_json::to_string(&request).unwrap_or_default(),
            r#"{"field":"game-install"}"#
        );
    }

    #[test]
    fn scan_result_round_trips() {
        let result = LibraryScanResult {
            rev: 4,
            stats: ScanStatsDto::default(),
            timings: ScanTimingsDto::default(),
            sources: vec![SourceReportDto {
                id: "custom-1".into(),
                kind: SourceKindDto::Custom,
                path: "/fiction/mods".into(),
                status: SourceStatusDto::Ready,
                mods: 12,
                loadable: Some(10),
                custom_only: Some(2),
            }],
            diagnostics: DiagnosticSummaryDto::default(),
            cancelled: false,
            counts: Some(LibraryCountsDto::default()),
            duplicates: Some(DuplicateGroupsDto::default()),
            ce_in_library: Some(CeInLibraryDto::default()),
        };
        let text = serde_json::to_string(&result).unwrap_or_default();
        let back: Result<LibraryScanResult, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(result));
    }
}
