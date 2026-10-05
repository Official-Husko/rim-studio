//! Settings types, defaults and pure validation.
//!
//! [`Settings`] is the content of `settings.jsonc` and [`WorkspaceSettings`] the content of
//! `workspace.jsonc`. Reading and writing the files (comments, atomic writes) belongs to
//! `rimstudio-io`; this module only holds the typed model, the defaults and checks that need no IO.
//!
//! Serialisation rules: keys are camelCase, enum values are kebab-case, a section equal to its
//! default is not written, and unknown top level keys (and unknown keys of a custom folder) are kept
//! in `extra` so a file written by a newer build survives a round trip. Unknown keys deeper inside
//! sections are the job of the comment preserving editor, which never rewrites untouched text.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{CoreError, PathFault};
use crate::ids::{ProfileId, SourceId};
use crate::mods::{ModSource, SourceKind};

/// Schema version of `settings.jsonc`.
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;
/// Schema version of `workspace.jsonc`.
pub const WORKSPACE_SCHEMA_VERSION: u32 = 1;
/// Schema version of `launch.jsonc`.
pub const LAUNCH_SCHEMA_VERSION: u32 = 1;

/// The smallest accepted custom folder scan depth.
pub const MIN_SCAN_DEPTH: u8 = 1;
/// The largest accepted custom folder scan depth.
pub const MAX_SCAN_DEPTH: u8 = 4;

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn default_true() -> bool {
    true
}

fn current_settings_version() -> u32 {
    SETTINGS_SCHEMA_VERSION
}

fn current_workspace_version() -> u32 {
    WORKSPACE_SCHEMA_VERSION
}

/// One problem found by a validator: where it is and what is wrong. Problems do not stop the app;
/// the offending value is reported and the default is used for that run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingProblem {
    /// Dotted key path, for example `customModFolders[1].scanDepth`.
    pub field: String,
    /// A short developer facing reason.
    pub reason: String,
}

impl SettingProblem {
    fn new(field: impl Into<String>, reason: impl Into<String>) -> Self {
        SettingProblem {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// The problem as a [`CoreError::InvalidSettings`].
    pub fn to_error(&self) -> CoreError {
        CoreError::invalid_settings(self.field.clone(), self.reason.clone())
    }
}

// ---------------------------------------------------------------------------------------------
// App settings
// ---------------------------------------------------------------------------------------------

/// Row height and spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    /// The default density.
    #[default]
    Standard,
    /// Smaller rows.
    Compact,
    /// Larger touch targets.
    Touch,
}

/// A tri state switch that can follow the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemToggle {
    /// Follow the OS.
    #[default]
    System,
    /// Always on.
    On,
    /// Always off.
    Off,
}

/// How a mod colour is shown on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColourMode {
    /// Fill the row background.
    #[default]
    Background,
    /// Colour the name.
    Text,
}

/// The `appearance` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    /// `light`, `dark`, `system` or a user theme name.
    pub theme: String,
    /// A preset name or `#rrggbb`.
    pub accent: String,
    /// Row density.
    pub density: Density,
    /// Root font scale, 0.8 to 1.6.
    pub font_scale: f64,
    /// Motion reduction.
    pub reduce_motion: SystemToggle,
    /// How mod colours are shown.
    pub colour_mode: ColourMode,
    /// Render rich text in descriptions.
    pub rich_text: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: "system".to_owned(),
            accent: "amber".to_owned(),
            density: Density::Standard,
            font_scale: 1.0,
            reduce_motion: SystemToggle::System,
            colour_mode: ColourMode::Background,
            rich_text: true,
        }
    }
}

/// How the library watches folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatchMode {
    /// Watch where events arrive and poll elsewhere.
    #[default]
    Auto,
    /// Never watch, poll only.
    Poll,
    /// Refresh on request and on focus only.
    Off,
}

/// The `library.watch` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WatchSettings {
    /// The watch mode.
    pub mode: WatchMode,
    /// Poll interval in seconds, 10 to 3600.
    pub poll_interval_seconds: u32,
    /// Event coalescing window in milliseconds, 100 to 5000.
    pub debounce_ms: u32,
}

impl Default for WatchSettings {
    fn default() -> Self {
        WatchSettings {
            mode: WatchMode::Auto,
            poll_interval_seconds: 60,
            debounce_ms: 500,
        }
    }
}

/// The `library.recentlyUpdated` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecentlyUpdated {
    /// Show the badge.
    pub enabled: bool,
    /// Threshold in days, 1 to 90.
    pub days: u32,
}

impl Default for RecentlyUpdated {
    fn default() -> Self {
        RecentlyUpdated {
            enabled: false,
            days: 3,
        }
    }
}

/// The `library` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibrarySettings {
    /// Scan workers, 0 (automatic) to 8.
    pub scan_threads: u8,
    /// Change detection.
    pub watch: WatchSettings,
    /// Compute Workshop update status from the ACF.
    pub check_workshop_updates: bool,
    /// The recently updated badge.
    pub recently_updated: RecentlyUpdated,
    /// Mark mods that are in or not in the latest save.
    pub show_save_comparison: bool,
    /// Open the log viewer on game exit when the log has errors.
    pub auto_load_player_log: bool,
}

impl Default for LibrarySettings {
    fn default() -> Self {
        LibrarySettings {
            scan_threads: 0,
            watch: WatchSettings::default(),
            check_workshop_updates: true,
            recently_updated: RecentlyUpdated::default(),
            show_save_comparison: true,
            auto_load_player_log: false,
        }
    }
}

/// The `sorting` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SortingSettings {
    /// Treat declared dependencies as load-after rules.
    pub dependencies_as_load_after: bool,
    /// An alternative package id satisfies a dependency.
    pub alternative_ids_satisfy_dependencies: bool,
    /// Offer to add missing dependencies when sorting.
    pub check_dependencies_on_sort: bool,
}

impl Default for SortingSettings {
    fn default() -> Self {
        SortingSettings {
            dependencies_as_load_after: false,
            alternative_ids_satisfy_dependencies: true,
            check_dependencies_on_sort: true,
        }
    }
}

/// The `history` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistorySettings {
    /// Write automatic snapshots.
    pub enabled: bool,
    /// Snapshots kept per profile; `None` keeps all.
    pub keep: Option<u32>,
}

impl Default for HistorySettings {
    fn default() -> Self {
        HistorySettings {
            enabled: true,
            keep: Some(50),
        }
    }
}

/// The `tools.textEditor` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextEditorSettings {
    /// The editor executable; empty uses the OS opener.
    pub command: String,
    /// Arguments when opening a folder (`{path}`).
    pub folder_args: Vec<String>,
    /// Arguments when opening a file (`{path}`, `{line}`).
    pub file_args: Vec<String>,
}

impl Default for TextEditorSettings {
    fn default() -> Self {
        TextEditorSettings {
            command: String::new(),
            folder_args: vec!["{path}".to_owned()],
            file_args: vec!["{path}".to_owned()],
        }
    }
}

/// The `tools` section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolsSettings {
    /// The external editor.
    pub text_editor: TextEditorSettings,
}

/// The item designer mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignerMode {
    /// The generic baseline.
    #[default]
    Simple,
    /// The quiz calibrated baseline.
    Calibrated,
}

/// The `designer` section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignerSettings {
    /// The designer mode.
    pub mode: DesignerMode,
}

/// The update feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateChannel {
    /// Stable releases.
    #[default]
    Stable,
    /// Beta releases.
    Beta,
}

/// The `updates` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateSettings {
    /// The feed.
    pub channel: UpdateChannel,
    /// Check at start.
    pub check_on_start: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            channel: UpdateChannel::Stable,
            check_on_start: true,
        }
    }
}

/// Log verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    /// Errors only.
    Error,
    /// Warnings and errors.
    Warn,
    /// The default.
    #[default]
    Info,
    /// Debug detail.
    Debug,
    /// Trace detail.
    Trace,
}

/// The `logging` section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoggingSettings {
    /// The verbosity.
    pub level: LogLevel,
}

/// The `onboarding` section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OnboardingSettings {
    /// Set when the setup wizard finished.
    pub completed: bool,
}

/// The content of `settings.jsonc`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The `$schema` key.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The schema version of the file.
    #[serde(default = "current_settings_version")]
    pub schema_version: u32,
    /// The UI language code; `None` follows the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Appearance.
    #[serde(skip_serializing_if = "is_default")]
    pub appearance: Appearance,
    /// Library behaviour.
    #[serde(skip_serializing_if = "is_default")]
    pub library: LibrarySettings,
    /// Sorting.
    #[serde(skip_serializing_if = "is_default")]
    pub sorting: SortingSettings,
    /// History.
    #[serde(skip_serializing_if = "is_default")]
    pub history: HistorySettings,
    /// Command id to chord overrides.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub shortcuts: BTreeMap<String, String>,
    /// External tools.
    #[serde(skip_serializing_if = "is_default")]
    pub tools: ToolsSettings,
    /// The item designer.
    #[serde(skip_serializing_if = "is_default")]
    pub designer: DesignerSettings,
    /// Updates.
    #[serde(skip_serializing_if = "is_default")]
    pub updates: UpdateSettings,
    /// Logging.
    #[serde(skip_serializing_if = "is_default")]
    pub logging: LoggingSettings,
    /// Onboarding state.
    #[serde(skip_serializing_if = "is_default")]
    pub onboarding: OnboardingSettings,
    /// Remembered window geometry, opaque to core.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Value>,
    /// Unknown top level keys, preserved.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: None,
            schema_version: SETTINGS_SCHEMA_VERSION,
            language: None,
            appearance: Appearance::default(),
            library: LibrarySettings::default(),
            sorting: SortingSettings::default(),
            history: HistorySettings::default(),
            shortcuts: BTreeMap::new(),
            tools: ToolsSettings::default(),
            designer: DesignerSettings::default(),
            updates: UpdateSettings::default(),
            logging: LoggingSettings::default(),
            onboarding: OnboardingSettings::default(),
            window: None,
            extra: Map::new(),
        }
    }
}

impl Settings {
    /// True when the file was written by a newer schema than this build knows (read only mode).
    pub fn is_newer_than_supported(&self) -> bool {
        self.schema_version > SETTINGS_SCHEMA_VERSION
    }

    /// Checks every range of the settings table. The list is empty when everything is valid.
    pub fn validate(&self) -> Vec<SettingProblem> {
        let mut out = Vec::new();
        let a = &self.appearance;
        if !(0.8..=1.6).contains(&a.font_scale) {
            out.push(SettingProblem::new(
                "appearance.fontScale",
                "must be between 0.8 and 1.6",
            ));
        }
        if a.theme.trim().is_empty() {
            out.push(SettingProblem::new("appearance.theme", "must not be empty"));
        }
        if !accent_is_valid(&a.accent) {
            out.push(SettingProblem::new(
                "appearance.accent",
                "must be a preset name or #rrggbb",
            ));
        }
        let l = &self.library;
        if l.scan_threads > 8 {
            out.push(SettingProblem::new(
                "library.scanThreads",
                "must be between 0 and 8",
            ));
        }
        if !(10..=3600).contains(&l.watch.poll_interval_seconds) {
            out.push(SettingProblem::new(
                "library.watch.pollIntervalSeconds",
                "must be between 10 and 3600",
            ));
        }
        if !(100..=5000).contains(&l.watch.debounce_ms) {
            out.push(SettingProblem::new(
                "library.watch.debounceMs",
                "must be between 100 and 5000",
            ));
        }
        if !(1..=90).contains(&l.recently_updated.days) {
            out.push(SettingProblem::new(
                "library.recentlyUpdated.days",
                "must be between 1 and 90",
            ));
        }
        if self.history.keep == Some(0) {
            out.push(SettingProblem::new(
                "history.keep",
                "must be positive or null",
            ));
        }
        for (command, chord) in &self.shortcuts {
            if command.trim().is_empty() || chord.trim().is_empty() {
                out.push(SettingProblem::new(
                    format!("shortcuts.{command}"),
                    "command and chord must not be empty",
                ));
            }
        }
        if self.schema_version == 0 {
            out.push(SettingProblem::new("schemaVersion", "must be at least 1"));
        }
        out
    }
}

fn accent_is_valid(text: &str) -> bool {
    if let Some(hex) = text.strip_prefix('#') {
        return hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

// ---------------------------------------------------------------------------------------------
// Workspace settings
// ---------------------------------------------------------------------------------------------

/// A path override with a pin flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathOverride {
    /// The overriding path.
    pub path: Utf8PathBuf,
    /// Stop auto switching to a better detection.
    #[serde(default, skip_serializing_if = "is_default")]
    pub pinned: bool,
}

/// The `paths` section: overrides that win over detection.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PathsSettings {
    /// Override of the install folder.
    pub game_install: Option<PathOverride>,
    /// Override of the user data folder.
    pub user_dir: Option<PathOverride>,
    /// Override of the Steam root.
    pub steam_root: Option<PathOverride>,
    /// Additional Workshop content folders.
    pub extra_workshop_dirs: Vec<Utf8PathBuf>,
    /// Detected installs the user does not want listed.
    pub ignored_installs: Vec<String>,
}

/// An entry of `modSources`: order and enabled flag of a built in source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEntry {
    /// The source id.
    pub id: SourceId,
    /// The source kind.
    pub kind: SourceKind,
    /// Include in scans.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// How a custom folder is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderLayout {
    /// Classify at scan time.
    #[default]
    Auto,
    /// The children are mods.
    #[serde(alias = "modsRoot")]
    ModsRoot,
    /// The path is itself a mod.
    #[serde(alias = "singleMod")]
    SingleMod,
}

/// How the game gets the mods of a custom folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkMode {
    /// Choose per mod.
    #[default]
    Auto,
    /// Always link.
    Links,
    /// Always copy.
    Copy,
    /// Do not make the mods visible.
    None,
}

/// Where a custom folder's volume was when it was added, to re-find a moved drive.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeHint {
    /// The mount point or drive root.
    pub mount: String,
    /// The volume label.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The volume UUID when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

/// A user added mod folder. Any number may exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFolder {
    /// The stable id (`cf_` plus hex); it is the source id inside every mod id.
    pub id: SourceId,
    /// The absolute folder path.
    pub path: Utf8PathBuf,
    /// The display name.
    #[serde(default)]
    pub label: String,
    /// Include in scans and lists.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How to read the folder.
    #[serde(default, skip_serializing_if = "is_default")]
    pub layout: FolderLayout,
    /// Levels below the root to search, 1 to 4.
    #[serde(default = "default_scan_depth")]
    pub scan_depth: u8,
    /// Use a watcher; `None` means on for local volumes and off for network volumes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<bool>,
    /// Duplicate tie break, lower wins; `None` means list order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Never write inside the folder.
    #[serde(default, skip_serializing_if = "is_default")]
    pub read_only: bool,
    /// How the game gets these mods.
    #[serde(default, skip_serializing_if = "is_default")]
    pub link: LinkMode,
    /// Re-finds a moved drive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_hint: Option<VolumeHint>,
    /// An optional colour tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Unknown keys, preserved.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn default_scan_depth() -> u8 {
    MIN_SCAN_DEPTH
}

impl CustomFolder {
    /// Creates an enabled folder with defaults; the label defaults to the last path component.
    pub fn new(id: SourceId, path: Utf8PathBuf) -> Self {
        let label = path.file_name().unwrap_or_default().to_owned();
        CustomFolder {
            id,
            path,
            label,
            enabled: true,
            layout: FolderLayout::Auto,
            scan_depth: MIN_SCAN_DEPTH,
            watch: None,
            priority: None,
            read_only: false,
            link: LinkMode::Auto,
            volume_hint: None,
            colour: None,
            extra: Map::new(),
        }
    }

    /// Whether to watch this folder, given whether its volume is local.
    pub fn watch_enabled(&self, local_volume: bool) -> bool {
        self.watch.unwrap_or(local_volume)
    }

    /// The folder as a mod source; `list_position` is the priority when none is set.
    pub fn to_source(&self, list_position: usize) -> ModSource {
        let fallback = i32::try_from(list_position).unwrap_or(i32::MAX);
        ModSource {
            id: self.id.clone(),
            kind: SourceKind::Custom,
            path: self.path.clone(),
            label: self.label.clone(),
            enabled: self.enabled,
            priority: self.priority.unwrap_or(fallback),
            read_only: self.read_only,
        }
    }
}

/// The `deploy` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DeploySettings {
    /// Names skipped by copy mode.
    pub copy_ignore: Vec<String>,
    /// Warn before copying a mod larger than this many megabytes.
    pub copy_warn_megabytes: u32,
}

impl Default for DeploySettings {
    fn default() -> Self {
        DeploySettings {
            copy_ignore: [".git", "Source", ".vs", "obj", "bin"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            copy_warn_megabytes: 500,
        }
    }
}

/// How the game is launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchMethod {
    /// The Steam URL for Steam installs, the executable otherwise.
    #[default]
    Auto,
    /// The Steam URL.
    Steam,
    /// The executable.
    Direct,
}

/// The `launch` section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LaunchSettings {
    /// The launch method.
    pub method: LaunchMethod,
    /// Command line arguments.
    pub arguments: String,
    /// The `-savedatafolder` value.
    pub save_data_folder: Option<Utf8PathBuf>,
    /// Environment variables for a direct launch.
    pub environment: BTreeMap<String, String>,
    /// Toggle the game's developer mode.
    pub dev_mode: bool,
    /// Add the quick test flag.
    pub quick_test: bool,
    /// Add the borderless window flag.
    pub popup_window: bool,
}

/// The settings of one dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DatasetSetting {
    /// Use this dataset.
    pub enabled: bool,
    /// A source override.
    pub url: Option<String>,
    /// A local file used instead of fetching.
    pub local_file: Option<Utf8PathBuf>,
    /// Hours before a copy is refetched; `None` uses the dataset default.
    pub refresh_hours: Option<u32>,
}

impl Default for DatasetSetting {
    fn default() -> Self {
        DatasetSetting {
            enabled: true,
            url: None,
            local_file: None,
            refresh_hours: None,
        }
    }
}

/// The `datasets` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DatasetsSettings {
    /// Refresh at start and on the interval.
    pub auto_update: bool,
    /// Per dataset settings keyed by dataset id.
    pub items: BTreeMap<String, DatasetSetting>,
}

impl Default for DatasetsSettings {
    fn default() -> Self {
        DatasetsSettings {
            auto_update: true,
            items: BTreeMap::new(),
        }
    }
}

/// The content of `workspace.jsonc`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkspaceSettings {
    /// The `$schema` key.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The schema version of the file.
    #[serde(default = "current_workspace_version")]
    pub schema_version: u32,
    /// Path overrides.
    #[serde(skip_serializing_if = "is_default")]
    pub paths: PathsSettings,
    /// Order and enabled flag of built in sources.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mod_sources: Vec<SourceEntry>,
    /// Any number of custom mod folders.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_mod_folders: Vec<CustomFolder>,
    /// Deploy options.
    #[serde(skip_serializing_if = "is_default")]
    pub deploy: DeploySettings,
    /// Launch defaults.
    #[serde(skip_serializing_if = "is_default")]
    pub launch: LaunchSettings,
    /// Datasets.
    #[serde(skip_serializing_if = "is_default")]
    pub datasets: DatasetsSettings,
    /// The install in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_install: Option<String>,
    /// The last used profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<ProfileId>,
    /// Unknown top level keys, preserved.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        WorkspaceSettings {
            schema: None,
            schema_version: WORKSPACE_SCHEMA_VERSION,
            paths: PathsSettings::default(),
            mod_sources: Vec::new(),
            custom_mod_folders: Vec::new(),
            deploy: DeploySettings::default(),
            launch: LaunchSettings::default(),
            datasets: DatasetsSettings::default(),
            active_install: None,
            active_profile: None,
            extra: Map::new(),
        }
    }
}

impl WorkspaceSettings {
    /// True when the file was written by a newer schema than this build knows.
    pub fn is_newer_than_supported(&self) -> bool {
        self.schema_version > WORKSPACE_SCHEMA_VERSION
    }

    /// The enabled and disabled custom folders as mod sources, in list order.
    pub fn custom_sources(&self) -> Vec<ModSource> {
        self.custom_mod_folders
            .iter()
            .enumerate()
            .map(|(i, f)| f.to_source(i))
            .collect()
    }

    /// Validates the whole workspace document.
    pub fn validate(&self) -> Vec<SettingProblem> {
        let mut out = validate_custom_folders(&self.custom_mod_folders);
        if self
            .launch
            .save_data_folder
            .as_ref()
            .is_some_and(|f| f.as_str().contains('='))
        {
            out.push(SettingProblem::new(
                "launch.saveDataFolder",
                "must not contain '='",
            ));
        }
        for (id, item) in &self.datasets.items {
            if item.refresh_hours.is_some_and(|h| !(1..=720).contains(&h)) {
                out.push(SettingProblem::new(
                    format!("datasets.items.{id}.refreshHours"),
                    "must be between 1 and 720",
                ));
            }
        }
        for (field, over) in [
            ("paths.gameInstall", &self.paths.game_install),
            ("paths.userDir", &self.paths.user_dir),
            ("paths.steamRoot", &self.paths.steam_root),
        ] {
            if let Some(Err(e)) = over.as_ref().map(|o| normalize_path(o.path.as_str())) {
                out.push(SettingProblem::new(field, e.to_string()));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Pure path helpers and custom folder validation
// ---------------------------------------------------------------------------------------------

fn looks_like_drive_path(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Normalises an absolute path text without touching the file system.
///
/// Leading white space is trimmed, empty and `.` segments are dropped and `..`
/// segments are resolved lexically. Windows style paths (a drive letter or a UNC prefix) have their
/// backslashes turned into forward slashes; on other paths a backslash is an ordinary character.
/// Symbolic links are not resolved, so the result is a lexical identity only.
///
/// # Errors
/// [`CoreError::InvalidPath`] for empty text, control characters, a relative path or a `..` that
/// climbs above the root.
pub fn normalize_path(input: &str) -> Result<Utf8PathBuf, CoreError> {
    let text = input.trim_start();
    if text.is_empty() {
        return Err(CoreError::invalid_path(input, PathFault::Empty));
    }
    if text.chars().any(char::is_control) {
        return Err(CoreError::invalid_path(input, PathFault::ControlChar));
    }
    let windows_style =
        looks_like_drive_path(text) || text.starts_with("\\\\") || text.starts_with("//");
    let unified = if windows_style && (looks_like_drive_path(text) || text.starts_with("\\\\")) {
        text.replace('\\', "/")
    } else {
        text.to_owned()
    };

    let (prefix, rest) = if looks_like_drive_path(&unified) {
        let (drive, rest) = unified.split_at(2);
        if !rest.starts_with('/') {
            return Err(CoreError::invalid_path(input, PathFault::Relative));
        }
        (drive.to_owned(), rest)
    } else if unified.starts_with("//") {
        ("/".to_owned(), unified.as_str())
    } else if unified.starts_with('/') {
        (String::new(), unified.as_str())
    } else {
        return Err(CoreError::invalid_path(input, PathFault::Relative));
    };

    let mut parts: Vec<&str> = Vec::new();
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(CoreError::invalid_path(input, PathFault::EscapesRoot));
                }
            }
            s => parts.push(s),
        }
    }
    let mut out = String::new();
    out.push_str(&prefix);
    if parts.is_empty() {
        out.push('/');
    } else {
        for p in &parts {
            out.push('/');
            out.push_str(p);
        }
    }
    Ok(Utf8PathBuf::from(out))
}

fn folds_case(path: &str) -> bool {
    looks_like_drive_path(path) || path.starts_with("//")
}

fn components_of(path: &Utf8Path) -> Vec<String> {
    let fold = folds_case(path.as_str());
    path.as_str()
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| if fold { s.to_lowercase() } else { s.to_owned() })
        .collect()
}

/// How two normalised folder paths relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRelation {
    /// Unrelated folders.
    Disjoint,
    /// The same folder (case insensitive for drive and UNC paths).
    Same,
    /// The first path is inside the second.
    Inside,
    /// The first path contains the second.
    Contains,
}

/// Compares two normalised absolute paths lexically.
pub fn path_relation(a: &Utf8Path, b: &Utf8Path) -> PathRelation {
    let (ca, cb) = (components_of(a), components_of(b));
    let common = ca.iter().zip(cb.iter()).take_while(|(x, y)| x == y).count();
    match (common == ca.len(), common == cb.len()) {
        (true, true) => PathRelation::Same,
        (true, false) => PathRelation::Contains,
        (false, true) => PathRelation::Inside,
        (false, false) => PathRelation::Disjoint,
    }
}

/// Checks a list of custom folders: ids unique, paths absolute, scan depth in range, no two folders
/// the same and no folder nested inside another (a nested folder would list every mod twice).
pub fn validate_custom_folders(folders: &[CustomFolder]) -> Vec<SettingProblem> {
    let mut out = Vec::new();
    let mut normalised: Vec<Option<Utf8PathBuf>> = Vec::with_capacity(folders.len());
    for (i, f) in folders.iter().enumerate() {
        let at = format!("customModFolders[{i}]");
        if !f.id.is_custom() {
            out.push(SettingProblem::new(
                format!("{at}.id"),
                "must start with cf_",
            ));
        }
        if !(MIN_SCAN_DEPTH..=MAX_SCAN_DEPTH).contains(&f.scan_depth) {
            out.push(SettingProblem::new(
                format!("{at}.scanDepth"),
                "must be between 1 and 4",
            ));
        }
        match normalize_path(f.path.as_str()) {
            Ok(p) => normalised.push(Some(p)),
            Err(e) => {
                out.push(SettingProblem::new(format!("{at}.path"), e.to_string()));
                normalised.push(None);
            }
        }
    }
    for i in 0..folders.len() {
        for j in (i + 1)..folders.len() {
            if folders.get(i).map(|f| &f.id) == folders.get(j).map(|f| &f.id) {
                out.push(SettingProblem::new(
                    format!("customModFolders[{j}].id"),
                    format!("duplicates entry {i}"),
                ));
            }
            let (Some(Some(a)), Some(Some(b))) = (normalised.get(i), normalised.get(j)) else {
                continue;
            };
            let reason = match path_relation(a, b) {
                PathRelation::Disjoint => continue,
                PathRelation::Same => format!("is the same folder as entry {i}"),
                PathRelation::Contains => format!("is inside entry {i}"),
                PathRelation::Inside => format!("contains entry {i}"),
            };
            out.push(SettingProblem::new(
                format!("customModFolders[{j}].path"),
                reason,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn folder(id: &str, path: &str) -> CustomFolder {
        CustomFolder::new(SourceId::new(id).unwrap(), Utf8PathBuf::from(path))
    }

    #[rstest]
    #[case("/a/b/../c/./d//", "/a/c/d")]
    #[case("/", "/")]
    #[case("  /x  ", "/x  ")]
    #[case("C:\\Mods\\RS\\..\\Other", "C:/Mods/Other")]
    #[case("C:\\", "C:/")]
    #[case("\\\\srv\\share\\m", "//srv/share/m")]
    #[case("/a\\b", "/a\\b")]
    fn normalises_paths(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_path(input).unwrap().as_str(), expected);
    }

    #[rstest]
    #[case("", PathFault::Empty)]
    #[case("rel/path", PathFault::Relative)]
    #[case("C:rel", PathFault::Relative)]
    #[case("/a/../..", PathFault::EscapesRoot)]
    #[case("/a\0b", PathFault::ControlChar)]
    fn rejects_bad_paths(#[case] input: &str, #[case] fault: PathFault) {
        match normalize_path(input) {
            Err(CoreError::InvalidPath { fault: f, .. }) => assert_eq!(f, fault),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn nested_and_duplicate_folders_are_reported() {
        let list = vec![
            folder("cf_aaaa", "/m/RS"),
            folder("cf_bbbb", "/m/RS/sub"),
            folder("cf_cccc", "/m/RS/"),
            folder("cf_dddd", "/m/other"),
            folder("cf_eeee", "/m"),
        ];
        let problems = validate_custom_folders(&list);
        let fields: Vec<&str> = problems.iter().map(|p| p.field.as_str()).collect();
        assert!(fields.contains(&"customModFolders[1].path"));
        assert!(fields.contains(&"customModFolders[2].path"));
        assert!(fields.contains(&"customModFolders[4].path"));
        assert!(
            !fields.contains(&"customModFolders[3].path")
                || problems.iter().any(|p| p.reason.contains("inside"))
        );
    }

    #[test]
    fn sibling_prefix_names_are_not_nested() {
        let list = vec![folder("cf_aaaa", "/m/RS"), folder("cf_bbbb", "/m/RS2")];
        assert!(validate_custom_folders(&list).is_empty());
    }

    #[test]
    fn windows_paths_compare_case_insensitively() {
        let a = normalize_path("C:\\Mods").unwrap();
        let b = normalize_path("c:/mods/x").unwrap();
        assert_eq!(path_relation(&b, &a), PathRelation::Inside);
    }

    #[test]
    fn duplicate_ids_scan_depth_and_foreign_ids_are_reported() {
        let mut a = folder("cf_aaaa", "/m/a");
        a.scan_depth = 9;
        let b = folder("cf_aaaa", "/m/b");
        let c = folder("game-mods", "/m/c");
        let fields: Vec<String> = validate_custom_folders(&[a, b, c])
            .into_iter()
            .map(|p| p.field)
            .collect();
        assert!(fields.contains(&"customModFolders[0].scanDepth".to_owned()));
        assert!(fields.contains(&"customModFolders[1].id".to_owned()));
        assert!(fields.contains(&"customModFolders[2].id".to_owned()));
    }

    #[test]
    fn defaults_serialise_to_a_minimal_document() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json, serde_json::json!({"schemaVersion": 1}));
        let json = serde_json::to_value(WorkspaceSettings::default()).unwrap();
        assert_eq!(json, serde_json::json!({"schemaVersion": 1}));
    }

    #[test]
    fn empty_object_reads_as_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        assert!(s.validate().is_empty());
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let text = r#"{"schemaVersion":2,"futureThing":{"a":[1,2]},"appearance":{"theme":"dark"}}"#;
        let s: Settings = serde_json::from_str(text).unwrap();
        assert!(s.is_newer_than_supported());
        assert_eq!(s.appearance.theme, "dark");
        let back = serde_json::to_value(&s).unwrap();
        assert_eq!(back["futureThing"], serde_json::json!({"a": [1, 2]}));
        assert_eq!(back["appearance"]["theme"], "dark");
        assert_eq!(back["appearance"]["density"], "standard");
    }

    #[test]
    fn custom_folder_reads_the_documented_shape_and_keeps_unknown_keys() {
        let text = r#"{
            "customModFolders": [{
                "id": "cf_7f3a", "path": "/p/RimWorld Mods", "label": "My mods",
                "layout": "modsRoot", "scanDepth": 2, "link": "copy", "readOnly": true,
                "volumeHint": {"mount": "/p", "label": "projects"}, "lastSeen": 5
            }]
        }"#;
        let w: WorkspaceSettings = serde_json::from_str(text).unwrap();
        let f = &w.custom_mod_folders[0];
        assert_eq!(f.layout, FolderLayout::ModsRoot);
        assert_eq!(f.link, LinkMode::Copy);
        assert!(f.read_only && f.enabled);
        assert_eq!(f.extra.get("lastSeen"), Some(&serde_json::json!(5)));
        let again = serde_json::to_value(&w).unwrap();
        assert_eq!(again["customModFolders"][0]["layout"], "mods-root");
        assert_eq!(again["customModFolders"][0]["lastSeen"], 5);
        let src = f.to_source(3);
        assert_eq!(src.priority, 3);
        assert_eq!(src.kind, SourceKind::Custom);
        assert!(w.validate().is_empty());
    }

    #[test]
    fn many_custom_folders_are_allowed() {
        let list: Vec<CustomFolder> = (0..50)
            .map(|i| folder(&format!("cf_{i:04x}"), &format!("/m/f{i}")))
            .collect();
        assert!(validate_custom_folders(&list).is_empty());
    }

    #[test]
    fn range_violations_are_reported_by_key_path() {
        let mut s = Settings::default();
        s.appearance.font_scale = 3.0;
        s.library.scan_threads = 20;
        s.library.watch.poll_interval_seconds = 1;
        s.appearance.accent = "#12".into();
        let fields: Vec<String> = s.validate().into_iter().map(|p| p.field).collect();
        for key in [
            "appearance.fontScale",
            "library.scanThreads",
            "library.watch.pollIntervalSeconds",
            "appearance.accent",
        ] {
            assert!(fields.iter().any(|f| f == key), "{key}");
        }
    }

    #[test]
    fn save_data_folder_with_equals_is_refused() {
        let mut w = WorkspaceSettings::default();
        w.launch.save_data_folder = Some("/a=b".into());
        assert_eq!(w.validate()[0].field, "launch.saveDataFolder");
    }

    proptest! {
        #[test]
        fn normalisation_is_idempotent(segs in proptest::collection::vec("[a-zA-Z0-9 ._-]{0,6}|\\.\\.", 0..8)) {
            let text = format!("/{}", segs.join("/"));
            if let Ok(once) = normalize_path(&text) {
                let twice = normalize_path(once.as_str()).unwrap();
                prop_assert_eq!(once, twice);
            }
        }

        #[test]
        fn relation_is_antisymmetric(a in "(/[a-z]{1,3}){1,4}", b in "(/[a-z]{1,3}){1,4}") {
            let (pa, pb) = (Utf8PathBuf::from(a), Utf8PathBuf::from(b));
            let flipped = match path_relation(&pa, &pb) {
                PathRelation::Inside => PathRelation::Contains,
                PathRelation::Contains => PathRelation::Inside,
                other => other,
            };
            prop_assert_eq!(path_relation(&pb, &pa), flipped);
        }
    }
}
