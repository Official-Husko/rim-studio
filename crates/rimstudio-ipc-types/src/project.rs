//! Project DTOs: opening and closing a mod project in the workspace.

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;

/// Request of `project_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenRequest {
    /// The mod folder the user picked.
    pub path: String,
}

/// Summary of an open project; response of `project_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryDto {
    /// Project id for later calls.
    pub project_id: String,
    /// Display name.
    pub name: String,
    /// Absolute path of the project root.
    pub path: String,
    /// Package id from `About.xml`. Absent when the file lacks one or does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub package_id: Option<String>,
    /// Game versions the mod declares, in file order.
    pub supported_versions: Vec<String>,
    /// `About/About.xml` exists.
    pub has_about: bool,
    /// `LoadFolders.xml` exists.
    pub has_load_folders: bool,
    /// `LoadFolders.xml` already gates a folder on Combat Extended.
    pub has_ce_gate: bool,
    /// Definition files found.
    pub def_files: u32,
    /// Problems found while reading the project.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Request of `project_close`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectCloseRequest {
    /// The project to release.
    pub project_id: String,
}

/// Response of `project_close`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectCloseResponse {
    /// True when a project was released; false when the id was unknown.
    pub closed: bool,
}

/// The convention a project follows for weapon files (RimStudio mod layout v1,
/// `docs/features/mod-layout.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum LayoutProfileDto {
    /// One file per weapon in a category folder (the default of a new project).
    Rimstudio,
    /// The game's own style: one file per category; a new weapon is appended to it.
    CoreStyle,
    /// One folder of weapon files, one file per weapon.
    Flat,
}

/// What a file or folder of a project is for in the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum NodeRoleDto {
    /// `About` and its files.
    About,
    /// `LoadFolders.xml`.
    LoadFolders,
    /// A version folder or `Common`: holds the content folders below it.
    ContentRoot,
    /// Definitions that are not weapons or sounds.
    Defs,
    /// Weapon definitions (`Defs/ThingDefs_Misc/Weapons` or the folder the project uses).
    DefsWeapons,
    /// Sound definitions (`Defs/SoundDefs`).
    DefsSounds,
    /// Patches outside the Combat Extended folder.
    Patches,
    /// The Combat Extended folder (`Compat/CombatExtended` or an older name), gated in `LoadFolders.xml`.
    CeCompat,
    /// `Textures`.
    Textures,
    /// `Sounds`.
    Sounds,
    /// `Languages`.
    Languages,
    /// `Assemblies`.
    Assemblies,
    /// `Source` and `Raw Assets`: author material the game never reads.
    Source,
    /// Anything else.
    Other,
}

/// Whether a tree entry is a folder or a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum TreeNodeKindDto {
    /// A folder.
    Folder,
    /// A file.
    File,
}

/// One folder or file of the annotated project tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TreeNodeDto {
    /// The name of the entry (the project folder name for the root).
    pub name: String,
    /// The path relative to the project root with `/` separators; empty for the root.
    pub path: String,
    /// Folder or file.
    pub kind: TreeNodeKindDto,
    /// The role in the layout.
    pub role: NodeRoleDto,
    /// Bytes: the size of a file, the sum of everything below for a folder.
    pub bytes: u64,
    /// Files below a folder (all depths); 1 for a file.
    pub files: u32,
    /// Layout issues attached to this entry or to anything below it.
    pub issues: u32,
    /// The entries of a folder, folders first, each group by name. Empty for a file, and for a folder
    /// whose files were left out because the tree was truncated.
    pub children: Vec<TreeNodeDto>,
}

/// Totals of a project folder.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectCountsDto {
    /// Folders below the root.
    pub folders: u32,
    /// Files.
    pub files: u32,
    /// Bytes of all files.
    pub bytes: u64,
    /// XML files below a `Defs` folder.
    pub def_files: u32,
    /// Weapon definitions found in them.
    pub weapon_defs: u32,
    /// Projectile definitions found in them.
    pub projectile_defs: u32,
    /// XML files of patches (below `Patches` and the Combat Extended folder).
    pub patch_files: u32,
    /// Texture files.
    pub textures: u32,
    /// Sound clips.
    pub sounds: u32,
}

/// What a suggested fix of a layout issue does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum LayoutFixKindDto {
    /// Create a missing folder.
    CreateFolder,
    /// Move a file to the place the layout names. Never automatic.
    MoveFile,
    /// Edit `LoadFolders.xml`. Never automatic.
    EditLoadFolders,
    /// Add the reserved texture or another file by hand.
    AddFile,
    /// Nothing to change; the issue is a note.
    None,
}

/// The suggested fix of a layout issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixDto {
    /// What the fix does.
    pub kind: LayoutFixKindDto,
    /// One plain sentence.
    pub summary: String,
    /// True when `project_scaffold_missing` applies it. Only creating a missing folder is automatic, and
    /// it is always safe: nothing is moved, replaced or deleted.
    pub automatic: bool,
    /// The paths the fix touches (the folder to create, the place to move a file to).
    pub targets: Vec<String>,
}

/// One finding of the layout check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutIssueDto {
    /// The stable code, `layout.<name>`.
    pub code: String,
    /// How serious it is.
    pub severity: crate::diagnostic::SeverityDto,
    /// The path the issue is about, relative to the project root; empty for the whole project.
    pub path: String,
    /// What is wrong, in plain words.
    pub message: String,
    /// The suggested fix.
    pub fix: LayoutFixDto,
}

/// Request of `project_tree`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectTreeRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// The most entries the tree holds; beyond it files are counted but not listed. Absent means 4000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_nodes: Option<u32>,
}

/// The annotated tree of a project; response of `project_tree`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectTreeDto {
    /// The project.
    pub project_id: String,
    /// The convention the project follows for weapon files.
    pub profile: LayoutProfileDto,
    /// The folder that holds the content when it is not the project root (`1.6`, `Common`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub content_folder: Option<String>,
    /// The folder of weapon definitions, relative to the project root.
    pub weapons_folder: String,
    /// The Combat Extended folder, relative to the content folder (the standard one when the project has none).
    pub ce_folder: String,
    /// The Combat Extended folder exists on disk.
    pub ce_folder_exists: bool,
    /// The Combat Extended folder is an older name than the standard `Compat/CombatExtended`.
    pub ce_folder_legacy: bool,
    /// `LoadFolders.xml` gates a folder on Combat Extended.
    pub ce_gated: bool,
    /// The tree.
    pub root: TreeNodeDto,
    /// Totals.
    pub counts: ProjectCountsDto,
    /// The layout issues, most serious first.
    pub issues: Vec<LayoutIssueDto>,
    /// Files were left out of the tree because it hit its entry limit; the counts are still complete.
    pub truncated: bool,
}

/// Request of `project_layout_check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutCheckRequest {
    /// The project (from `project_open`).
    pub project_id: String,
}

/// The layout issues of a project; response of `project_layout_check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutCheckDto {
    /// The project.
    pub project_id: String,
    /// The convention the project follows for weapon files.
    pub profile: LayoutProfileDto,
    /// The issues, most serious first.
    pub issues: Vec<LayoutIssueDto>,
    /// Issues of severity error.
    pub errors: u32,
    /// Issues of severity warning.
    pub warnings: u32,
    /// Issues of severity info.
    pub infos: u32,
    /// Issues that `project_scaffold_missing` fixes.
    pub auto_fixable: u32,
}

/// Request of `project_scaffold_missing`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectScaffoldMissingRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// Only report what would be created.
    pub dry_run: bool,
}

/// What `project_scaffold_missing` did (or would do).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectScaffoldMissingDto {
    /// The project.
    pub project_id: String,
    /// True when nothing was written.
    pub dry_run: bool,
    /// The folders created, or that would be created, relative to the project root.
    pub folders: Vec<String>,
    /// The files created, or that would be created. A file that exists is never replaced.
    pub files: Vec<String>,
    /// What was left alone and why (a path that is not a folder, a refused path).
    pub skipped: Vec<String>,
}

/// Request of `project_read_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectReadFileRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// The file, relative to the project root.
    pub path: String,
    /// The most bytes read. Absent means 262144; larger requests are cut to 1048576.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_bytes: Option<u32>,
}

/// A text file of a project; response of `project_read_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileDto {
    /// The path relative to the project root.
    pub path: String,
    /// The role of the file in the layout.
    pub role: NodeRoleDto,
    /// The size of the whole file in bytes.
    pub bytes: u64,
    /// The text (without a byte order mark). Empty for a binary file.
    pub text: String,
    /// The file is longer than what was read.
    pub truncated: bool,
    /// The file is not text (it has a NUL byte or is not UTF-8); no text is returned.
    pub binary: bool,
}
