//! The members of the optional Combat Extended block that go beyond the plain numbers: bows, weapon
//! platforms and the under barrel unit, the ammo comp extras, the explicit tool plan, the companion tags
//! and the raw extras.
//!
//! The JSON shape is identical to the design engine's own types, so the toolkit converts by serialising one
//! and deserialising the other. Everything here is optional (D-085): nothing is written unless the block
//! carries it.

use serde::{Deserialize, Serialize};

use super::carried::RawNodeDto;
use super::spec::SourcedDto;

/// A stat name with a number, one entry of an attachment link's stat lists.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeStatEntryDto {
    /// The stat definition name.
    pub stat: String,
    /// The number.
    pub value: f64,
}

/// One attachment a weapon platform accepts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeAttachmentLinkDto {
    /// The attachment definition name.
    pub attachment: String,
    /// Draw scale as the game writes a vector. Absent means the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_scale: Option<String>,
    /// Draw offset as the game writes a vector. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_offset: Option<String>,
    /// Stat offsets while the attachment is fitted. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeStatEntryDto>>", optional))]
    pub stat_offsets: Vec<CeStatEntryDto>,
    /// Stat multipliers while the attachment is fitted. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeStatEntryDto>>", optional))]
    pub stat_multipliers: Vec<CeStatEntryDto>,
    /// Stat replacers while the attachment is fitted. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeStatEntryDto>>", optional))]
    pub stat_replacers: Vec<CeStatEntryDto>,
}

/// One default graphic part of a weapon platform.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeGraphicPartDto {
    /// The part graphic, a raw element. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub part_graphic: Option<RawNodeDto>,
    /// The outline graphic, a raw element. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub outline_graphic: Option<RawNodeDto>,
    /// The slot tags the part belongs to. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub slot_tags: Vec<String>,
}

/// The fire mode choices of an under barrel unit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeUnderBarrelFireModesDto {
    /// Whether the AI uses burst mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ai_use_burst_mode: Option<bool>,
    /// The AI aim mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ai_aim_mode: Option<String>,
    /// Shots of an aimed burst.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub aimed_burst_shot_count: Option<u32>,
    /// True when the unit has no single shot mode.
    pub no_single_shot: bool,
}

/// The under barrel unit of a weapon.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeUnderBarrelDto {
    /// Label of the standard mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub standard_label: Option<String>,
    /// Label of the under barrel mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub under_barrel_label: Option<String>,
    /// The unit shares the ammo of the main weapon.
    pub one_ammo_holder: bool,
    /// The unit has its own reload.
    pub requires_reload: bool,
    /// Ammo set definition name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_set: Option<String>,
    /// Default projectile definition name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_projectile: Option<String>,
    /// Magazine size.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub magazine_size: Option<SourcedDto<u32>>,
    /// Reload time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reload_time: Option<SourcedDto<f64>>,
    /// Recoil amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub recoil_amount: Option<SourcedDto<f64>>,
    /// Warm up in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warmup_time: Option<SourcedDto<f64>>,
    /// Range in tiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub range: Option<SourcedDto<f64>>,
    /// Minimum range in tiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub min_range: Option<f64>,
    /// Shots of one burst.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub burst_shot_count: Option<u32>,
    /// Ticks between the shots of a burst.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ticks_between_burst_shots: Option<u32>,
    /// Ammo used by one shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_consumed_per_shot: Option<u32>,
    /// Friendly fire avoidance radius.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub avoid_friendly_fire_radius: Option<f64>,
    /// Sound of the shot (a sound definition name).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_cast: Option<String>,
    /// Muzzle flash scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub muzzle_flash_scale: Option<f64>,
    /// Other children of the verb, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub verb_extra: Vec<RawNodeDto>,
    /// Fire mode choices. Omitted when empty.
    #[serde(skip_serializing_if = "CeUnderBarrelFireModesDto::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<CeUnderBarrelFireModesDto>", optional))]
    pub fire_modes: CeUnderBarrelFireModesDto,
    /// Other children of the verb properties, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub props_extra: Vec<RawNodeDto>,
    /// Other children of the unit, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
    /// The vanilla component the unit replaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub replaces_comp: Option<String>,
}

impl CeUnderBarrelFireModesDto {
    /// True when no member is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// One entry of an explicit tool list for a conversion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeToolPlanDto {
    /// The label of the tool in the converted weapon. May be empty.
    pub label: String,
    /// The vanilla tool it comes from. Absent means the tool of the same label.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<String>,
    /// The capacities of the tool. Absent means as the vanilla tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub capacities: Option<Vec<String>>,
    /// Power. Absent means as the vanilla tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub power: Option<f64>,
    /// Cooldown in seconds. Absent means as the vanilla tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cooldown: Option<f64>,
    /// Chance factor. Absent means as the vanilla tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub chance_factor: Option<f64>,
    /// Sharp armor penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_sharp: Option<f64>,
    /// Blunt armor penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_blunt: Option<f64>,
    /// The body part group the tool is linked to.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub linked_body_parts_group: Option<String>,
}

/// What accepting an optional suggestion writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum CeOptionValueDto {
    /// Weapon tags to add.
    Tags(Vec<String>),
    /// A flag to set.
    Flag(bool),
    /// A text value to set.
    Text(String),
    /// An explicit tool list.
    ToolPlan(Vec<CeToolPlanDto>),
}

/// One optional addition that the user's conversions suggest. It is only a suggestion: the user opts in by
/// writing its value into the block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeOptionDto {
    /// The stable id.
    pub id: String,
    /// The JSON pointer of the block field the option fills.
    pub field: String,
    /// A short English label.
    pub label: String,
    /// What accepting writes.
    pub value: CeOptionValueDto,
    /// Converted weapons that show the habit.
    pub examples: u32,
    /// Converted weapons of the same group in all.
    pub of: u32,
    /// Why the option is offered, in plain words.
    pub why: String,
}
