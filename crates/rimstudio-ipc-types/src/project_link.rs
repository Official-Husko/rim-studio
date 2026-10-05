//! DTOs of the link commands: `project_link_status`, `project_link_create` and `project_link_remove`.
//!
//! A mod made in a custom folder is invisible to the game, which reads only its own `Mods` folder. The
//! link commands make one project visible by creating a link (or, only when asked, a marked copy) named
//! after the project folder inside `<game install>/Mods`, and take it away again. Every refusal is a value
//! in the answer (`refusal`), never an error, so the page can show the reason and the manual command.

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;

/// What stands at `Mods/<name>`, seen from the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ProjectLinkStateDto {
    /// Nothing is there: the project can be linked.
    NotLinked,
    /// A link RimStudio created, pointing at the project.
    Linked,
    /// A link RimStudio did not create that points at the project. It is not removed by RimStudio.
    LinkedByHand,
    /// A link RimStudio created whose target is not the project any more, or is gone.
    Stale,
    /// A link RimStudio did not create that points somewhere else.
    ForeignLink,
    /// A real folder or file RimStudio did not create.
    ForeignFolder,
    /// A marked copy RimStudio created.
    Copy,
    /// The project folder is itself inside the game's `Mods` folder.
    InMods,
    /// Nothing can be done: no game, no `Mods` folder or no project folder.
    Unavailable,
}

/// How the entry is (or is to be) made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ProjectLinkModeDto {
    /// A symbolic link (the default).
    #[default]
    Symlink,
    /// A Windows junction.
    Junction,
    /// A copy of the folder with an ownership marker. It does not follow later edits.
    Copy,
}

/// Whether the game is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum GameRunningDto {
    /// A game process was found.
    Running,
    /// No game process was found.
    NotRunning,
    /// The process list is hidden, so the answer is not known.
    Unknown,
}

/// Whether a mod is in the game's active mod list (`ModsConfig.xml`, read only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ActiveInGameDto {
    /// The package id is in the active list.
    Active,
    /// The list was read and the package id is not in it.
    Inactive,
    /// The list could not be read, or the mod has no package id.
    Unknown,
}

/// What the link backend of this system can create.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LinkSupportDto {
    /// Symbolic links can be created.
    pub symlink: bool,
    /// Junctions can be created.
    pub junction: bool,
    /// Creating a symbolic link needs a privilege the app may lack (Windows).
    pub needs_privilege: bool,
}

/// Request of `project_link_status`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLinkStatusRequest {
    /// The project (from `project_open`).
    pub project_id: String,
}

/// Request of `project_link_create`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLinkCreateRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// How to make the entry; absent means a symbolic link.
    pub mode: ProjectLinkModeDto,
    /// The person confirmed that the game is running and will be restarted. Without it a running game
    /// refuses the call.
    pub confirm_game_running: bool,
}

/// Request of `project_link_remove`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLinkRemoveRequest {
    /// The project (from `project_open`).
    pub project_id: String,
}

/// The answer of `project_link_status`: whether the game can see the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinkStatusDto {
    /// The project.
    pub project_id: String,
    /// The `packageId` of the project's About file, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub package_id: Option<String>,
    /// A game install is known.
    pub game_found: bool,
    /// The game folder, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_folder: Option<String>,
    /// The game's `Mods` folder, when a game is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mods_folder: Option<String>,
    /// The `Mods` folder exists.
    pub mods_exists: bool,
    /// The `Mods` folder carries a read only flag.
    pub mods_read_only: bool,
    /// The name the project is linked under (its folder name, made safe).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub link_name: Option<String>,
    /// The full path of the entry in `Mods`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub entry_path: Option<String>,
    /// The state of the entry.
    pub state: ProjectLinkStateDto,
    /// How RimStudio made the entry, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mode: Option<ProjectLinkModeDto>,
    /// Where an existing link points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub points_to: Option<String>,
    /// Whether the game is running.
    pub game_running: GameRunningDto,
    /// What this system can create.
    pub support: LinkSupportDto,
    /// `project_link_create` may be called.
    pub can_create: bool,
    /// `project_link_remove` may be called.
    pub can_remove: bool,
    /// The command to run by hand to create the link (`ln -s` or `mklink /J`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub manual_command: Option<String>,
    /// Whether the project's package id is in the game's active mod list.
    pub active_in_game: ActiveInGameDto,
    /// The project has a Combat Extended patch (its `LoadFolders.xml` gates a folder on Combat Extended).
    pub has_ce_patch: bool,
    /// Whether Combat Extended is in the game's active mod list.
    pub ce_active_in_game: ActiveInGameDto,
    /// Findings, most important first.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Why a create or remove did nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinkRefusalDto {
    /// Stable code, `deploy.<kebab-name>`.
    pub code: String,
    /// English fallback text.
    pub message: String,
}

/// The answer of `project_link_create` and `project_link_remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinkResultDto {
    /// True when the file system changed.
    pub done: bool,
    /// Why nothing was done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub refusal: Option<ProjectLinkRefusalDto>,
    /// The state after the call.
    pub status: ProjectLinkStatusDto,
}
