//! Settings DTOs: the application settings without secrets, the custom mod folders, and the update patch.
//!
//! `SettingsDto` mirrors `rimstudio_core::settings::Settings` and the path and folder parts of
//! `WorkspaceSettings`. The window geometry and unknown keys are not part of the DTO. Updates are section
//! patches: every member of a patch is optional and absent means unchanged; clearing an override goes
//! through the `reset` list of key paths.

use std::collections::BTreeMap;

use rimstudio_core::settings as core;
use serde::{Deserialize, Serialize};

macro_rules! kebab_enum {
    ($(#[$meta:meta])* $name:ident <= $src:ty { $( $(#[$vmeta:meta])* $variant:ident ),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[serde(rename_all = "kebab-case")]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl From<$src> for $name {
            fn from(value: $src) -> Self {
                match value {
                    $( <$src>::$variant => Self::$variant, )+
                }
            }
        }

        impl From<$name> for $src {
            fn from(value: $name) -> Self {
                match value {
                    $( $name::$variant => <$src>::$variant, )+
                }
            }
        }
    };
}

kebab_enum! {
    /// Row density.
    DensityDto <= core::Density {
        /// Default spacing.
        #[default]
        Standard,
        /// Tight spacing.
        Compact,
        /// Large touch targets.
        Touch,
    }
}

kebab_enum! {
    /// A three way switch that can follow the system.
    SystemToggleDto <= core::SystemToggle {
        /// Follow the operating system.
        #[default]
        System,
        /// Always on.
        On,
        /// Always off.
        Off,
    }
}

kebab_enum! {
    /// Where a mod colour is applied.
    ColourModeDto <= core::ColourMode {
        /// Behind the row.
        #[default]
        Background,
        /// On the text.
        Text,
    }
}

kebab_enum! {
    /// How folders are watched.
    WatchModeDto <= core::WatchMode {
        /// Native events with polling fallback.
        #[default]
        Auto,
        /// Polling only.
        Poll,
        /// Not watched.
        Off,
    }
}

kebab_enum! {
    /// Designer calibration mode.
    DesignerModeDto <= core::DesignerMode {
        /// Class medians only.
        #[default]
        Simple,
        /// Use the calibrated pools.
        Calibrated,
    }
}

kebab_enum! {
    /// Update channel.
    UpdateChannelDto <= core::UpdateChannel {
        /// Stable releases.
        #[default]
        Stable,
        /// Beta releases.
        Beta,
    }
}

kebab_enum! {
    /// Log level.
    LogLevelDto <= core::LogLevel {
        /// Errors only.
        Error,
        /// Warnings and errors.
        Warn,
        /// Normal.
        #[default]
        Info,
        /// Debug.
        Debug,
        /// Trace.
        Trace,
    }
}

kebab_enum! {
    /// How a custom folder is read.
    FolderLayoutDto <= core::FolderLayout {
        /// Decide from the contents.
        #[default]
        Auto,
        /// The folder holds mod folders.
        ModsRoot,
        /// The folder is one mod.
        SingleMod,
    }
}

kebab_enum! {
    /// How a source is deployed into the game.
    LinkModeDto <= core::LinkMode {
        /// Links where possible, copies otherwise.
        #[default]
        Auto,
        /// Links only.
        Links,
        /// Copies only.
        Copy,
        /// Never deployed.
        None,
    }
}

/// Appearance settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppearanceDto {
    /// Theme id.
    pub theme: String,
    /// Accent colour as a CSS colour text.
    pub accent: String,
    /// Row density.
    pub density: DensityDto,
    /// Font scale factor.
    pub font_scale: f64,
    /// Reduced motion switch.
    pub reduce_motion: SystemToggleDto,
    /// Where mod colours apply.
    pub colour_mode: ColourModeDto,
    /// Whether descriptions render rich text.
    pub rich_text: bool,
}

/// Folder watching settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WatchSettingsDto {
    /// Watch mode.
    pub mode: WatchModeDto,
    /// Polling interval in seconds.
    pub poll_interval_seconds: u32,
    /// Debounce in milliseconds.
    pub debounce_ms: u32,
}

/// Library settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibrarySettingsDto {
    /// Worker threads of a scan; zero means automatic.
    pub scan_threads: u8,
    /// Watching.
    pub watch: WatchSettingsDto,
    /// Whether Workshop update checks run.
    pub check_workshop_updates: bool,
    /// Whether the recently updated highlight is on.
    pub recently_updated_enabled: bool,
    /// Days that count as recent.
    pub recently_updated_days: u32,
    /// Whether the save comparison is shown.
    pub show_save_comparison: bool,
    /// Whether the player log loads automatically.
    pub auto_load_player_log: bool,
}

/// Sorting settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SortingSettingsDto {
    /// Treat dependencies as load after hints.
    pub dependencies_as_load_after: bool,
    /// Let alternative package ids satisfy dependencies.
    pub alternative_ids_satisfy_dependencies: bool,
    /// Check dependencies when sorting.
    pub check_dependencies_on_sort: bool,
}

/// List history settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HistorySettingsDto {
    /// Whether history is kept.
    pub enabled: bool,
    /// Entries kept. Absent means unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub keep: Option<u32>,
}

/// Designer settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerSettingsDto {
    /// Calibration mode.
    pub mode: DesignerModeDto,
}

/// Update settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsDto {
    /// Channel.
    pub channel: UpdateChannelDto,
    /// Whether to check when the app starts.
    pub check_on_start: bool,
}

/// A path override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PathOverrideDto {
    /// The path.
    pub path: String,
    /// True when detection must not replace the override.
    pub pinned: bool,
}

/// Path overrides.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PathsDto {
    /// Game install override. Absent means detection decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_install: Option<PathOverrideDto>,
    /// User data folder override. Absent means detection decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub user_dir: Option<PathOverrideDto>,
    /// Steam root override. Absent means detection decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_root: Option<PathOverrideDto>,
    /// Extra Workshop content folders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub extra_workshop_dirs: Vec<String>,
}

/// Hint about the volume a folder lives on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VolumeHintDto {
    /// Mount point.
    pub mount: String,
    /// Volume label; empty when unknown.
    pub label: String,
    /// Volume uuid. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub uuid: Option<String>,
}

/// A user added mod folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CustomFolderDto {
    /// Stable source id.
    pub id: String,
    /// Absolute path.
    pub path: String,
    /// Display label; empty means the folder name.
    pub label: String,
    /// Whether the folder takes part in scans.
    pub enabled: bool,
    /// How the folder is read.
    pub layout: FolderLayoutDto,
    /// Scan depth, 1 to 4.
    pub scan_depth: u8,
    /// Watch override. Absent means follow the library watch setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub watch: Option<bool>,
    /// Priority override for duplicates. Absent means the default order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub priority: Option<i32>,
    /// True when the app must never write into the folder.
    pub read_only: bool,
    /// Deployment mode.
    pub link: LinkModeDto,
    /// Volume hint. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub volume_hint: Option<VolumeHintDto>,
    /// Row colour. Absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub colour: Option<String>,
}

impl From<&core::CustomFolder> for CustomFolderDto {
    fn from(value: &core::CustomFolder) -> Self {
        Self {
            id: value.id.to_string(),
            path: value.path.as_str().to_owned(),
            label: value.label.clone(),
            enabled: value.enabled,
            layout: value.layout.into(),
            scan_depth: value.scan_depth,
            watch: value.watch,
            priority: value.priority,
            read_only: value.read_only,
            link: value.link.into(),
            volume_hint: value.volume_hint.as_ref().map(|v| VolumeHintDto {
                mount: v.mount.clone(),
                label: v.label.clone(),
                uuid: v.uuid.clone(),
            }),
            colour: value.colour.clone(),
        }
    }
}

/// The settings the webview sees. Secrets are never part of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    /// Schema version of the settings file.
    pub schema_version: u32,
    /// Revision of the settings; every write bumps it and updates carry the revision they were based on.
    pub rev: u64,
    /// UI language tag. Absent means the system language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub language: Option<String>,
    /// Appearance.
    pub appearance: AppearanceDto,
    /// Library.
    pub library: LibrarySettingsDto,
    /// Sorting.
    pub sorting: SortingSettingsDto,
    /// History.
    pub history: HistorySettingsDto,
    /// Keyboard shortcuts by action id, ordered by id.
    pub shortcuts: BTreeMap<String, String>,
    /// Designer.
    pub designer: DesignerSettingsDto,
    /// Updates.
    pub updates: UpdateSettingsDto,
    /// Log level.
    pub log_level: LogLevelDto,
    /// Whether onboarding is complete.
    pub onboarding_completed: bool,
    /// Path overrides.
    pub paths: PathsDto,
    /// Custom mod folders, in the order of the file.
    pub custom_folders: Vec<CustomFolderDto>,
}

fn path_override(value: &core::PathOverride) -> PathOverrideDto {
    PathOverrideDto {
        path: value.path.as_str().to_owned(),
        pinned: value.pinned,
    }
}

impl SettingsDto {
    /// Builds the DTO from the two settings documents of the core crate and a revision.
    #[must_use]
    pub fn from_core(
        settings: &core::Settings,
        workspace: &core::WorkspaceSettings,
        rev: u64,
    ) -> Self {
        let a = &settings.appearance;
        let l = &settings.library;
        Self {
            schema_version: settings.schema_version,
            rev,
            language: settings.language.clone(),
            appearance: AppearanceDto {
                theme: a.theme.clone(),
                accent: a.accent.clone(),
                density: a.density.into(),
                font_scale: a.font_scale,
                reduce_motion: a.reduce_motion.into(),
                colour_mode: a.colour_mode.into(),
                rich_text: a.rich_text,
            },
            library: LibrarySettingsDto {
                scan_threads: l.scan_threads,
                watch: WatchSettingsDto {
                    mode: l.watch.mode.into(),
                    poll_interval_seconds: l.watch.poll_interval_seconds,
                    debounce_ms: l.watch.debounce_ms,
                },
                check_workshop_updates: l.check_workshop_updates,
                recently_updated_enabled: l.recently_updated.enabled,
                recently_updated_days: l.recently_updated.days,
                show_save_comparison: l.show_save_comparison,
                auto_load_player_log: l.auto_load_player_log,
            },
            sorting: SortingSettingsDto {
                dependencies_as_load_after: settings.sorting.dependencies_as_load_after,
                alternative_ids_satisfy_dependencies: settings
                    .sorting
                    .alternative_ids_satisfy_dependencies,
                check_dependencies_on_sort: settings.sorting.check_dependencies_on_sort,
            },
            history: HistorySettingsDto {
                enabled: settings.history.enabled,
                keep: settings.history.keep,
            },
            shortcuts: settings.shortcuts.clone(),
            designer: DesignerSettingsDto {
                mode: settings.designer.mode.into(),
            },
            updates: UpdateSettingsDto {
                channel: settings.updates.channel.into(),
                check_on_start: settings.updates.check_on_start,
            },
            log_level: settings.logging.level.into(),
            onboarding_completed: settings.onboarding.completed,
            paths: PathsDto {
                game_install: workspace.paths.game_install.as_ref().map(path_override),
                user_dir: workspace.paths.user_dir.as_ref().map(path_override),
                steam_root: workspace.paths.steam_root.as_ref().map(path_override),
                extra_workshop_dirs: workspace
                    .paths
                    .extra_workshop_dirs
                    .iter()
                    .map(|p| p.as_str().to_owned())
                    .collect(),
            },
            custom_folders: workspace
                .custom_mod_folders
                .iter()
                .map(CustomFolderDto::from)
                .collect(),
        }
    }
}

/// Request of `settings_get`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsGetRequest {
    /// Section names to include; empty means all sections.
    pub sections: Vec<String>,
}

/// Patch of the appearance section; absent members stay unchanged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct AppearancePatch {
    /// New theme id.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub theme: Option<String>,
    /// New accent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub accent: Option<String>,
    /// New density.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub density: Option<DensityDto>,
    /// New font scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub font_scale: Option<f64>,
    /// New reduced motion switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reduce_motion: Option<SystemToggleDto>,
    /// New colour mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub colour_mode: Option<ColourModeDto>,
    /// New rich text switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rich_text: Option<bool>,
}

/// Patch of the library section; absent members stay unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryPatch {
    /// New thread count.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scan_threads: Option<u8>,
    /// New watch mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub watch_mode: Option<WatchModeDto>,
    /// New polling interval.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub poll_interval_seconds: Option<u32>,
    /// New debounce.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub debounce_ms: Option<u32>,
    /// New update check switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub check_workshop_updates: Option<bool>,
}

/// Patch of the sorting section; absent members stay unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct SortingPatch {
    /// New switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub dependencies_as_load_after: Option<bool>,
    /// New switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub alternative_ids_satisfy_dependencies: Option<bool>,
    /// New switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub check_dependencies_on_sort: Option<bool>,
}

/// Patch of the path overrides; absent members stay unchanged (clear through `reset`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct PathsPatch {
    /// New game install override.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_install: Option<PathOverrideDto>,
    /// New user data folder override.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub user_dir: Option<PathOverrideDto>,
    /// New Steam root override.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_root: Option<PathOverrideDto>,
    /// Replacement list of extra Workshop folders.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub extra_workshop_dirs: Option<Vec<String>>,
}

/// Request of `settings_update`: section patches applied atomically against a revision.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsUpdate {
    /// The revision the caller based the patch on; a mismatch is rejected with `settings.revision-conflict`.
    pub expected_rev: u64,
    /// New language tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub language: Option<String>,
    /// Appearance patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub appearance: Option<AppearancePatch>,
    /// Library patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub library: Option<LibraryPatch>,
    /// Sorting patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sorting: Option<SortingPatch>,
    /// New designer mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub designer_mode: Option<DesignerModeDto>,
    /// New log level.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub log_level: Option<LogLevelDto>,
    /// New onboarding flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub onboarding_completed: Option<bool>,
    /// Path override patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub paths: Option<PathsPatch>,
    /// Key paths to reset to their defaults (for example `paths.gameInstall`), applied after the patches.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub reset: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_convert_and_round_trip() {
        let dto = SettingsDto::from_core(
            &core::Settings::default(),
            &core::WorkspaceSettings::default(),
            3,
        );
        assert_eq!(dto.rev, 3);
        assert!(dto.custom_folders.is_empty());
        let text = serde_json::to_string(&dto).unwrap_or_default();
        let back: Result<SettingsDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(dto));
    }

    #[test]
    fn enum_conversions_are_inverse() {
        for d in [DensityDto::Standard, DensityDto::Compact, DensityDto::Touch] {
            assert_eq!(DensityDto::from(core::Density::from(d)), d);
        }
        for l in [
            LogLevelDto::Error,
            LogLevelDto::Warn,
            LogLevelDto::Info,
            LogLevelDto::Debug,
            LogLevelDto::Trace,
        ] {
            assert_eq!(LogLevelDto::from(core::LogLevel::from(l)), l);
        }
    }

    #[test]
    fn empty_update_serialises_to_the_revision_only() {
        let update = SettingsUpdate {
            expected_rev: 9,
            ..SettingsUpdate::default()
        };
        assert_eq!(
            serde_json::to_string(&update).unwrap_or_default(),
            r#"{"expectedRev":9}"#
        );
    }

    #[test]
    fn partial_patch_deserialises_with_defaults() {
        let update: Result<SettingsUpdate, _> = serde_json::from_str(
            r#"{"expectedRev":1,"appearance":{"density":"compact"},"reset":["paths.gameInstall"]}"#,
        );
        let update = update.unwrap_or_default();
        assert_eq!(
            update.appearance.and_then(|a| a.density),
            Some(DensityDto::Compact)
        );
        assert_eq!(update.reset, vec!["paths.gameInstall".to_owned()]);
    }

    #[test]
    fn custom_folder_converts_from_core() {
        let folder: Result<core::CustomFolder, _> = serde_json::from_str(
            r#"{"id":"custom-1","path":"/fiction/mods","label":"RS_Mods","scanDepth":2}"#,
        );
        let Ok(folder) = folder else {
            panic!("fixture must parse");
        };
        let dto = CustomFolderDto::from(&folder);
        assert_eq!(dto.path, "/fiction/mods");
        assert_eq!(dto.scan_depth, 2);
        assert!(dto.enabled);
        assert_eq!(dto.layout, FolderLayoutDto::Auto);
    }
}
