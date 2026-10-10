//! The weapons a project already has, and opening one of them in the designer.
//!
//! The list is read from the definitions of the open project as the base game loads them (no Combat
//! Extended conversion applied). Opening a weapon stores a draft that edits the definition where it stands:
//! the plan of the draft replaces the definition in its own file and never writes a second one.

use serde::{Deserialize, Serialize};

use super::convert::ConvertVanillaDto;
use super::draft::DraftEntryDto;
use super::spec::ItemKindDto;
use crate::diagnostic::DiagnosticDto;

/// Request of `designer_project_weapons`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerProjectWeaponsRequest {
    /// The open project.
    pub project_id: String,
}

/// One weapon definition of the project, with what the Weapons tab shows about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectWeaponDto {
    /// Definition name.
    pub def_name: String,
    /// Display label; empty when the definition has none.
    pub label: String,
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// The project file that holds the definition, relative to the project root.
    pub file: String,
    /// The `ParentName` of the definition. Absent when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// The tech level the definition resolves to. Absent when it gives none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tech_level: Option<String>,
    /// The projectile the shooting verb fires. Absent for a melee weapon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub projectile: Option<String>,
    /// The weapon tags in definition order. Omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub tags: Vec<String>,
    /// The weapon classes in definition order. Omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub weapon_classes: Vec<String>,
    /// A few numbers of the definition as the game loads it: damage, range, cooldown, warm up, mass, burst
    /// and market value. A number the definition does not give is absent.
    pub numbers: ConvertVanillaDto,
    /// True when a patch file that RimStudio generated holds a Combat Extended conversion of this weapon.
    pub generated_patch: bool,
    /// The id of a stored draft that edits this weapon. Absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draft_id: Option<String>,
}

/// Response of `designer_project_weapons`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerProjectWeaponsResponse {
    /// The weapons, ordered by definition name.
    pub weapons: Vec<ProjectWeaponDto>,
    /// Content problems found while reading the project (a definition file that cannot be read).
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Request of `designer_open_weapon`: start a draft that edits a weapon of the project where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerOpenWeaponRequest {
    /// The open project.
    pub project_id: String,
    /// Definition name of the weapon, as the project's own files name it.
    pub def_name: String,
    /// Read the weapon from its file again and replace the stored draft of it. Absent or false continues
    /// the stored draft when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reload: Option<bool>,
}

/// Response of `designer_open_weapon`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerOpenWeaponResponse {
    /// The stored draft with its id. `edits_file` of the draft names the file the weapon lives in, every
    /// number is the one the definition holds and the Combat Extended block is absent.
    pub entry: DraftEntryDto,
    /// The project file the definition is in.
    pub file: String,
    /// True when a stored draft of this weapon was continued instead of read from the file again.
    pub reused: bool,
    /// Plain notes: fields of the definition the designer keeps as raw fields, a projectile that lives in
    /// another file, a definition that could not be read in full.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}
