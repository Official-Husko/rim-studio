//! DTOs of the mod basics commands: `project_about_get`, `project_about_preview`, `project_about_update`,
//! `project_about_set_preview`, `project_about_remove_preview`, `project_load_folders_get`,
//! `project_load_folders_update`, `project_version_add` and `library_mod_search`.
//!
//! The About model carries every basic of a mod (name, authors, package id, description, url, version, the
//! supported game versions, the dependencies and the load order lists, the icon path) with, for each field,
//! whether the file has it, so a form can tell a field that is set from one that the game defaults. The
//! per version blocks (`ByVersion`) are marked advanced: they are shown and can be edited, but a form should
//! not lead with them. A change is one small operation (set, clear, add to a list, move, remove) so a form
//! sends exactly what the person did; the backend applies them as byte span edits and returns the diff.
//! Diagnostics never block a save; only text that is not well formed XML does.

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;
use crate::library::SourceKindDto;

// ------------------------------------------------------------------------------------------ fields

/// A text field of `About.xml` that a form edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AboutTextFieldDto {
    /// `name`.
    Name,
    /// `shortName`.
    ShortName,
    /// `author`.
    Author,
    /// `packageId`.
    PackageId,
    /// `description`.
    Description,
    /// `url`.
    Url,
    /// `modVersion`.
    ModVersion,
    /// `modIconPath`.
    ModIconPath,
}

impl AboutTextFieldDto {
    /// The element name in the file.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::ShortName => "shortName",
            Self::Author => "author",
            Self::PackageId => "packageId",
            Self::Description => "description",
            Self::Url => "url",
            Self::ModVersion => "modVersion",
            Self::ModIconPath => "modIconPath",
        }
    }
}

/// A list field of `About.xml` made of `li` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AboutListFieldDto {
    /// `authors`.
    Authors,
    /// `supportedVersions`.
    SupportedVersions,
    /// `loadBefore`.
    LoadBefore,
    /// `loadAfter`.
    LoadAfter,
    /// `forceLoadBefore`.
    ForceLoadBefore,
    /// `forceLoadAfter`.
    ForceLoadAfter,
    /// `incompatibleWith`.
    IncompatibleWith,
}

impl AboutListFieldDto {
    /// The element name in the file.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Authors => "authors",
            Self::SupportedVersions => "supportedVersions",
            Self::LoadBefore => "loadBefore",
            Self::LoadAfter => "loadAfter",
            Self::ForceLoadBefore => "forceLoadBefore",
            Self::ForceLoadAfter => "forceLoadAfter",
            Self::IncompatibleWith => "incompatibleWith",
        }
    }
}

/// A relation list that has a `ByVersion` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AboutByVersionFieldDto {
    /// `loadBeforeByVersion`.
    LoadBefore,
    /// `loadAfterByVersion`.
    LoadAfter,
    /// `forceLoadBeforeByVersion`.
    ForceLoadBefore,
    /// `forceLoadAfterByVersion`.
    ForceLoadAfter,
    /// `incompatibleWithByVersion`.
    IncompatibleWith,
}

impl AboutByVersionFieldDto {
    /// The element name in the file.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::LoadBefore => "loadBeforeByVersion",
            Self::LoadAfter => "loadAfterByVersion",
            Self::ForceLoadBefore => "forceLoadBeforeByVersion",
            Self::ForceLoadAfter => "forceLoadAfterByVersion",
            Self::IncompatibleWith => "incompatibleWithByVersion",
        }
    }
}

// ------------------------------------------------------------------------------------------- model

/// A text field with its place in the file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutTextDto {
    /// The text as the game reads it (trimmed, except the description). Empty when the field is absent or
    /// empty.
    pub value: String,
    /// True when the file has an element for the field. False means the game uses its default.
    pub present: bool,
}

/// A list field with its place in the file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutListDto {
    /// The entries in file order, trimmed, empty entries left out.
    pub items: Vec<String>,
    /// True when the file has an element for the list.
    pub present: bool,
}

/// One dependency of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutDependencyDto {
    /// The package id of the required mod.
    pub package_id: String,
    /// The name shown to people who lack it.
    #[serde(default)]
    pub display_name: String,
    /// The Steam Workshop page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_workshop_url: Option<String>,
    /// A direct download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub download_url: Option<String>,
    /// Package ids that satisfy the dependency as well.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub alternatives: Vec<String>,
}

/// The description of one game version (`descriptionsByVersion`). Advanced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutVersionTextDto {
    /// The version key (`1.6`).
    pub version: String,
    /// The description for that version.
    pub text: String,
}

/// The relation lists of one game version (the `ByVersion` blocks). Advanced.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutVersionRelationsDto {
    /// The version key (`1.6`).
    pub version: String,
    /// `modDependenciesByVersion` entries.
    pub mod_dependencies: Vec<AboutDependencyDto>,
    /// `loadBeforeByVersion` entries.
    pub load_before: Vec<String>,
    /// `loadAfterByVersion` entries.
    pub load_after: Vec<String>,
    /// `forceLoadBeforeByVersion` entries.
    pub force_load_before: Vec<String>,
    /// `forceLoadAfterByVersion` entries.
    pub force_load_after: Vec<String>,
    /// `incompatibleWithByVersion` entries.
    pub incompatible_with: Vec<String>,
}

/// The `ByVersion` blocks of the file. Advanced: the game replaces the base value with the block of the
/// running version, it does not merge them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutByVersionDto {
    /// Always true: a form shows these behind an "advanced" switch.
    pub advanced: bool,
    /// `descriptionsByVersion`.
    pub descriptions: Vec<AboutVersionTextDto>,
    /// The relation blocks, one per version.
    pub relations: Vec<AboutVersionRelationsDto>,
}

/// `About/Preview.png` as found on disk.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutPreviewDto {
    /// The file exists.
    pub exists: bool,
    /// The path relative to the mod root.
    pub path: String,
    /// Size in bytes; absent when the file does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bytes: Option<u64>,
    /// Width in pixels, when the file is a PNG with a readable header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub width: Option<u32>,
    /// Height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub height: Option<u32>,
    /// SHA-256 of the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sha256: Option<String>,
    /// `data:image/png;base64,...` of the image, only when the request asked for it and the file is small.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub data_url: Option<String>,
}

/// Every basic of a mod; response of `project_about_get` and the result part of the preview and the update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutDto {
    /// The project.
    pub project_id: String,
    /// The About file relative to the mod root, as it is spelled on disk (`About/About.xml`).
    pub path: String,
    /// SHA-256 of the file bytes; pass it back as `expectedHash` to refuse a save over a changed file.
    pub file_hash: String,
    /// Size of the file in bytes.
    pub bytes: u64,
    /// The file was parsed. False when it is not well formed XML: the model is then empty and nothing can be
    /// saved until the text is fixed.
    pub parsed: bool,
    /// The edits of this API can be applied: the file is well formed, UTF-8 and not too large.
    pub editable: bool,
    /// Why not, in a plain sentence, when `editable` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub not_editable_reason: Option<String>,
    /// `name`.
    pub name: AboutTextDto,
    /// `shortName`.
    pub short_name: AboutTextDto,
    /// `author`.
    pub author: AboutTextDto,
    /// `authors`.
    pub authors: AboutListDto,
    /// `packageId`.
    pub package_id: AboutTextDto,
    /// `description`.
    pub description: AboutTextDto,
    /// `url`.
    pub url: AboutTextDto,
    /// `modVersion`.
    pub mod_version: AboutTextDto,
    /// `modIconPath`.
    pub mod_icon_path: AboutTextDto,
    /// `supportedVersions`.
    pub supported_versions: AboutListDto,
    /// `modDependencies`.
    pub mod_dependencies: Vec<AboutDependencyDto>,
    /// The file has a `modDependencies` element.
    pub mod_dependencies_present: bool,
    /// `loadBefore`.
    pub load_before: AboutListDto,
    /// `loadAfter`.
    pub load_after: AboutListDto,
    /// `forceLoadBefore`.
    pub force_load_before: AboutListDto,
    /// `forceLoadAfter`.
    pub force_load_after: AboutListDto,
    /// `incompatibleWith`.
    pub incompatible_with: AboutListDto,
    /// `steamAppId`, when the file has a number there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_app_id: Option<u32>,
    /// The `ByVersion` blocks (advanced).
    pub by_version: AboutByVersionDto,
    /// Elements of the root that the game does not read, in file order. They are never touched.
    pub unknown_tags: Vec<String>,
    /// `About/Preview.png`.
    pub preview: AboutPreviewDto,
    /// The installed game version the diagnostics compared against, when known (`1.6`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_version: Option<String>,
    /// The text of the file (cut at `rawText` limit of 262144 bytes, read only).
    pub raw_text: String,
    /// The raw text was cut.
    pub raw_truncated: bool,
    /// Findings about the values, with stable codes `about.*` and `field` pointers such as `/packageId`.
    /// They never block a save.
    pub diagnostics: Vec<DiagnosticDto>,
}

// ------------------------------------------------------------------------------------------ requests

/// Request of `project_about_get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutGetRequest {
    /// The project.
    pub project_id: String,
    /// Also return the preview image as a data URL (up to 2 MiB).
    #[serde(default)]
    pub include_preview_image: bool,
}

/// What to change in one dependency; an absent member is left as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AboutDependencyChangeDto {
    /// New package id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub package_id: Option<String>,
    /// New display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub display_name: Option<String>,
    /// New Workshop URL; an empty text removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub steam_workshop_url: Option<String>,
    /// New download URL; an empty text removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub download_url: Option<String>,
    /// New alternatives; an empty list removes them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub alternatives: Option<Vec<String>>,
}

/// One edit of `About.xml`. A request holds a list of them, applied in order; list positions refer to the
/// list as it is after the earlier edits of the same request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum AboutChangeDto {
    /// Sets a text field. An empty text removes the element (the game then uses its default).
    Set {
        /// The field.
        field: AboutTextFieldDto,
        /// The new text.
        value: String,
    },
    /// Removes a text field.
    Clear {
        /// The field.
        field: AboutTextFieldDto,
    },
    /// Replaces a list. An empty list removes the element.
    ListSet {
        /// The field.
        field: AboutListFieldDto,
        /// The new entries.
        items: Vec<String>,
    },
    /// Adds an entry to a list (not twice).
    ListAdd {
        /// The field.
        field: AboutListFieldDto,
        /// The entry.
        value: String,
        /// The position; the end when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        at: Option<u32>,
    },
    /// Removes an entry from a list.
    ListRemove {
        /// The field.
        field: AboutListFieldDto,
        /// The entry.
        value: String,
    },
    /// Moves an entry of a list.
    ListMove {
        /// The field.
        field: AboutListFieldDto,
        /// The entry.
        value: String,
        /// The new position (zero based).
        to: u32,
    },
    /// Adds a dependency (not twice).
    DependencyAdd {
        /// The dependency.
        dependency: AboutDependencyDto,
        /// The position; the end when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        at: Option<u32>,
    },
    /// Changes fields of a dependency.
    DependencyUpdate {
        /// The package id the dependency has now.
        package_id: String,
        /// What to change.
        change: AboutDependencyChangeDto,
    },
    /// Removes a dependency.
    DependencyRemove {
        /// The package id.
        package_id: String,
    },
    /// Moves a dependency.
    DependencyMove {
        /// The package id.
        package_id: String,
        /// The new position (zero based).
        to: u32,
    },
    /// Sets the entries of one `ByVersion` relation block (advanced). An empty list removes the block.
    ByVersionListSet {
        /// The relation.
        field: AboutByVersionFieldDto,
        /// The version key (`1.6`).
        version: String,
        /// The new entries.
        items: Vec<String>,
    },
    /// Sets or removes the description of one game version (advanced).
    ByVersionDescriptionSet {
        /// The version key (`1.6`).
        version: String,
        /// The text; absent removes the block.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        value: Option<String>,
    },
}

/// Request of `project_about_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutPreviewRequest {
    /// The project.
    pub project_id: String,
    /// The edits.
    pub changes: Vec<AboutChangeDto>,
    /// The `fileHash` the edits were made against; the call is refused when the file differs now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub expected_hash: Option<String>,
}

/// Response of `project_about_preview`: what the edits would do, nothing written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutPreviewDto {
    /// The edits change at least one byte.
    pub changed: bool,
    /// The unified diff of the file; empty when nothing changes.
    pub diff: String,
    /// Hash of the file now.
    pub current_hash: String,
    /// The model of the text the edits would produce, with its diagnostics.
    pub result: ProjectAboutDto,
}

/// Request of `project_about_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutUpdateRequest {
    /// The project.
    pub project_id: String,
    /// The edits.
    pub changes: Vec<AboutChangeDto>,
    /// The `fileHash` the edits were made against; the call is refused when the file differs now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub expected_hash: Option<String>,
}

/// Response of `project_about_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutUpdateDto {
    /// The file was written (false when the edits changed nothing).
    pub written: bool,
    /// The unified diff that was applied; empty when nothing changed.
    pub diff: String,
    /// The backup of the replaced file, in the app data folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup: Option<String>,
    /// The file was read back and equals what was written.
    pub verified: bool,
    /// The model after the write.
    pub about: ProjectAboutDto,
}

/// Request of `project_about_set_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutSetPreviewRequest {
    /// The project.
    pub project_id: String,
    /// The PNG to copy (an absolute path on this machine).
    pub source_path: String,
}

/// Response of `project_about_set_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutSetPreviewDto {
    /// The file written.
    pub preview: AboutPreviewDto,
    /// A different file was replaced.
    pub replaced: bool,
    /// The backup of the replaced file, in the app data folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup: Option<String>,
    /// Findings about the image (size, dimensions).
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Request of `project_about_remove_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutRemovePreviewRequest {
    /// The project.
    pub project_id: String,
}

/// Response of `project_about_remove_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectAboutRemovePreviewDto {
    /// A file was removed (false when there was none).
    pub removed: bool,
    /// Where the removed file was kept, in the app data folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup: Option<String>,
}

// ------------------------------------------------------------------------------------ load folders

/// One folder entry of `LoadFolders.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LoadEntryDto {
    /// Position in the block (zero based, counting every child element as the game does).
    pub index: u32,
    /// The folder relative to the mod root; empty is the mod root itself.
    pub path: String,
    /// `IfModActive`: loads when any of these is active.
    pub if_mod_active: Vec<String>,
    /// `IfModActiveAll`: loads when all of these are active.
    pub if_mod_active_all: Vec<String>,
    /// `IfModNotActive`: does not load when any of these is active.
    pub if_mod_not_active: Vec<String>,
    /// Attributes the game does not read (for example `IfModActiveAny`); such an entry loads always.
    pub ignored_attributes: Vec<String>,
    /// The folder exists in the mod.
    pub folder_exists: bool,
}

/// One version block of `LoadFolders.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LoadBlockDto {
    /// Position among the children of the root (zero based).
    pub index: u32,
    /// The element name as written (`v1.6`).
    pub tag: String,
    /// The version key the game uses (`1.6`, or `default`).
    pub key: String,
    /// The entries in file order.
    pub entries: Vec<LoadEntryDto>,
}

/// Request of `project_load_folders_get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadFoldersGetRequest {
    /// The project.
    pub project_id: String,
}

/// The `LoadFolders.xml` of a project; response of `project_load_folders_get` and part of the update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadFoldersDto {
    /// The project.
    pub project_id: String,
    /// The mod has a `LoadFolders.xml`.
    pub exists: bool,
    /// The file relative to the mod root, as spelled on disk; `LoadFolders.xml` when there is none yet.
    pub path: String,
    /// SHA-256 of the file; absent when there is no file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub file_hash: Option<String>,
    /// The edits can be applied (the file is well formed and UTF-8, or there is none to create).
    pub editable: bool,
    /// Why not, when `editable` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub not_editable_reason: Option<String>,
    /// The blocks in file order.
    pub blocks: Vec<LoadBlockDto>,
    /// `supportedVersions` of the About file.
    pub supported_versions: Vec<String>,
    /// The folders of the mod root that are named like a game version (`1.5`, `1.6`), sorted.
    pub version_folders: Vec<String>,
    /// The text of the file (cut at 262144 bytes, read only); empty when there is none.
    pub raw_text: String,
    /// The raw text was cut.
    pub raw_truncated: bool,
    /// Findings: a folder that does not exist, a block for an unsupported version, a gated folder without
    /// its condition, a condition the game does not read. They never block a save.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// An entry to add.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LoadEntryInputDto {
    /// The folder relative to the mod root; empty is the root.
    pub path: String,
    /// `IfModActive` ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub if_mod_active: Vec<String>,
    /// `IfModActiveAll` ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub if_mod_active_all: Vec<String>,
    /// `IfModNotActive` ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub if_mod_not_active: Vec<String>,
}

/// What to change in one entry; an absent member is left as it is and an empty list removes the attribute.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LoadEntryChangeDto {
    /// New folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub path: Option<String>,
    /// New `IfModActive` ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub if_mod_active: Option<Vec<String>>,
    /// New `IfModActiveAll` ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub if_mod_active_all: Option<Vec<String>>,
    /// New `IfModNotActive` ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub if_mod_not_active: Option<Vec<String>>,
    /// Remove the attributes the game does not read.
    #[serde(default)]
    pub drop_ignored_attributes: bool,
}

/// One edit of `LoadFolders.xml`, applied in order; positions refer to the file as the earlier edits of the
/// same request left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum LoadFoldersChangeDto {
    /// Adds a version block.
    AddBlock {
        /// The version key (`1.6` or `default`).
        version: String,
        /// The entries of the block.
        entries: Vec<LoadEntryInputDto>,
        /// The position among the blocks; the end when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        at: Option<u32>,
    },
    /// Removes a block with its entries.
    RemoveBlock {
        /// The block position.
        block: u32,
    },
    /// Adds an entry to a block.
    AddEntry {
        /// The block position.
        block: u32,
        /// The entry.
        entry: LoadEntryInputDto,
        /// The position in the block; the end when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        at: Option<u32>,
    },
    /// Removes an entry.
    RemoveEntry {
        /// The block position.
        block: u32,
        /// The entry position.
        entry: u32,
    },
    /// Moves an entry inside its block.
    MoveEntry {
        /// The block position.
        block: u32,
        /// The entry position now.
        entry: u32,
        /// The new position.
        to: u32,
    },
    /// Changes the folder or the conditions of an entry.
    SetEntry {
        /// The block position.
        block: u32,
        /// The entry position.
        entry: u32,
        /// What to change.
        change: LoadEntryChangeDto,
    },
}

/// Request of `project_load_folders_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadFoldersUpdateRequest {
    /// The project.
    pub project_id: String,
    /// The edits.
    pub changes: Vec<LoadFoldersChangeDto>,
    /// The `fileHash` the edits were made against; the call is refused when the file differs now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub expected_hash: Option<String>,
    /// Create `LoadFolders.xml` when the mod has none (an empty `loadFolders` root the edits then fill).
    #[serde(default)]
    pub create: bool,
    /// Compute the result and the diff and write nothing.
    #[serde(default)]
    pub dry_run: bool,
}

/// Response of `project_load_folders_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadFoldersUpdateDto {
    /// The file was written (false for a dry run or when nothing changed).
    pub written: bool,
    /// The file did not exist and is now created.
    pub created: bool,
    /// The unified diff.
    pub diff: String,
    /// The backup of the replaced file, in the app data folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup: Option<String>,
    /// The file was read back and equals what was written (true for a dry run).
    pub verified: bool,
    /// The state after the edits (of the text that was written, or would be written for a dry run).
    pub load_folders: ProjectLoadFoldersDto,
}

/// Request of `project_version_add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersionAddRequest {
    /// The project.
    pub project_id: String,
    /// The game version (`1.6`).
    pub version: String,
    /// Also create the standard sub folders of a version folder (folders only, no file).
    #[serde(default)]
    pub standard_folders: bool,
    /// Add a block for the version to `LoadFolders.xml`: it lists the mod root and the new folder.
    /// Without this the block is added only when the mod already has a `LoadFolders.xml`.
    #[serde(default)]
    pub add_block: bool,
    /// Compute what would happen and write nothing.
    #[serde(default)]
    pub dry_run: bool,
}

/// Response of `project_version_add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersionAddDto {
    /// The version key (`1.6`).
    pub version: String,
    /// The folder created, relative to the mod root.
    pub folder: String,
    /// Every folder created (or that a dry run would create), relative to the mod root.
    pub created_folders: Vec<String>,
    /// A block was added to `LoadFolders.xml`.
    pub block_added: bool,
    /// `LoadFolders.xml` was created.
    pub load_folders_created: bool,
    /// The version is not in `supportedVersions` of About.xml (the call does not change that file).
    pub not_in_supported_versions: bool,
    /// Findings, for example that the folder exists already.
    pub diagnostics: Vec<DiagnosticDto>,
}

// ----------------------------------------------------------------------------------- library search

/// Request of `library_mod_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryModSearchRequest {
    /// Words to find in the name, the package id and the authors.
    pub query: String,
    /// The most rows to return (default 20, at most 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub limit: Option<u32>,
}

/// One mod of the last library scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryModHitDto {
    /// The package id as the mod declares it.
    pub package_id: String,
    /// The display name.
    pub name: String,
    /// The authors.
    pub authors: Vec<String>,
    /// The kind of folder the mod was found in.
    pub source: SourceKindDto,
    /// The Steam Workshop item id, when the folder is a Workshop item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub workshop_id: Option<String>,
    /// The Workshop page of the item, built from the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub workshop_url: Option<String>,
    /// The mod folder.
    pub path: String,
    /// The game can load the folder as it is.
    pub loadable: bool,
    /// The `url` of the mod's own About file, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub url: Option<String>,
}

/// Response of `library_mod_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryModSearchDto {
    /// A scan exists. False means the list is empty because nothing was scanned, not because nothing matched.
    pub scanned: bool,
    /// The matches, best first.
    pub hits: Vec<LibraryModHitDto>,
    /// A plain sentence for the person when there is nothing to show (no scan yet, empty query).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub hint: Option<String>,
}
