//! Flow C of the designer: clone and adjust an existing weapon, the diff against the source, and the
//! structure defaults of a new weapon.
//!
//! A clone is a vanilla draft. The Combat Extended toggle of a clone is off even when the source has a
//! Combat Extended conversion in some loaded session, and the clone never edits the source def.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::draft::{DraftDto, DraftEntryDto};
use super::preview::{ReadoutGroupDto, ReadoutUnitDto};

/// Request of `designer_clone`: start a draft from an existing weapon of the loaded defs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerCloneRequest {
    /// The project the draft belongs to.
    pub project_id: String,
    /// Definition name of the source weapon, as the loaded defs name it.
    pub source: String,
    /// Definition name of the new weapon.
    pub def_name: String,
    /// Display label of the new weapon. Absent derives it from the definition name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// Prefix the mod uses for its definition names. When set, a definition name that does not start with
    /// `<prefix>_` gets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mod_prefix: Option<String>,
    /// Give a gun a projectile of its own, copied from the one it fires, so that damage edits change the
    /// written file. Absent means yes; false keeps pointing at the shared projectile of the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub own_projectile: Option<bool>,
}

/// Request of `designer_projectile_own`: switch the own projectile of a draft on or off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerProjectileOwnRequest {
    /// The draft state to change. It need not be saved; the answer is not stored either.
    pub draft: DraftDto,
    /// True gives the weapon a projectile of its own copied from the one it points at; false points it
    /// back at the projectile it was copied from.
    pub own: bool,
}

/// Response of `designer_projectile_own`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerProjectileOwnResponse {
    /// The draft with the projectile switched. Not stored.
    pub draft: DraftDto,
    /// Plain notes about what changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}

/// Response of `designer_clone`: the stored draft and what the copy could not carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerCloneResponse {
    /// The stored draft with its id. Its `clonedFrom` names the source, the source is its first anchor,
    /// every number is marked `anchor` and the Combat Extended block is absent.
    pub entry: DraftEntryDto,
    /// Plain notes: fields of the source the designer does not model, a projectile the clone shares with
    /// its source, a note about a source that could not be read in full.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}

/// Request of `designer_clone_diff`: the changed fields of a clone against its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerCloneDiffRequest {
    /// The draft state to compare; its `clonedFrom` names the source. It need not be saved.
    pub draft: DraftDto,
}

/// One field that differs between a clone and its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CloneChangeDto {
    /// JSON pointer into the spec, for example `/ranged/damage` or `/tools/0/power`.
    pub field: String,
    /// Short English label of the field.
    pub label: String,
    /// The source's value. Absent when the source does not have the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub old: Option<Value>,
    /// The clone's value. Absent when the clone dropped the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub new: Option<Value>,
}

/// An exact readout of the source next to the clone's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReadoutDeltaDto {
    /// Readout key, for example `dps`.
    pub key: String,
    /// Group of the readout.
    pub group: ReadoutGroupDto,
    /// Unit of the readout.
    pub unit: ReadoutUnitDto,
    /// The value for the source. Absent when an input is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub old: Option<f64>,
    /// The value for the clone. Absent when an input is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub new: Option<f64>,
    /// `new` minus `old`. Absent when either is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub delta: Option<f64>,
}

/// Response of `designer_clone_diff`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerCloneDiffResponse {
    /// Definition name of the source.
    pub source: String,
    /// Label of the source.
    pub source_label: String,
    /// The changed fields, in the order of the spec. The names and the label of the clone are not changes.
    pub changes: Vec<CloneChangeDto>,
    /// The exact readouts of both, in display order (cycle time, DPS and strength for a ranged weapon;
    /// panel and in fight DPS for a melee weapon; market value).
    pub readouts: Vec<ReadoutDeltaDto>,
    /// Plain notes, for example that a changed damage is not written while the projectile is shared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}

/// Request of `designer_structure_defaults`: fill the structure of a new weapon from its nearest reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerStructureDefaultsRequest {
    /// The draft, with the numbers already filled.
    pub draft: DraftDto,
}

/// The reference weapon the structure was copied from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StructureReferenceDto {
    /// Definition name.
    pub def_name: String,
    /// Display label.
    pub label: String,
}

/// Response of `designer_structure_defaults`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerStructureDefaultsResponse {
    /// The draft with the empty structure fields filled. Typed values and fields that already hold a value
    /// are never replaced.
    pub draft: DraftDto,
    /// The nearest reference weapon of the class pool, by the numbers the draft holds. Absent when none
    /// could be chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reference: Option<StructureReferenceDto>,
    /// JSON pointers of the filled fields, in spec order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub filled: Vec<String>,
    /// Plain notes: that the values are suggestions copied from the reference, or why nothing was filled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}
