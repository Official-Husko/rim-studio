//! The design spec DTO: the designer's input for one weapon, always in vanilla terms.
//!
//! The JSON shape is identical to the design engine's own `DesignSpec`, so the toolkit converts by
//! serialising one and deserialising the other. Every numeric input carries its source, and a value the
//! user typed is never replaced by a suggestion (IT-003). The optional Combat Extended block `ce` is absent
//! by default; nothing in the contract turns it on implicitly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::assets::{AssetImportsDto, SoundImportsDto};
use super::carried::{ExtraMeleeDamageDto, RawNodeDto, RecipeSpecDto, SurpriseAttackSpecDto};
use super::ce_block::{CeAttachmentLinkDto, CeGraphicPartDto, CeToolPlanDto, CeUnderBarrelDto};

/// Where a number came from. The order is the replacement rank, `typed` highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ValueSourceDto {
    /// Filled in by the suggestion engine.
    Suggested,
    /// Taken from an anchor weapon.
    Anchor,
    /// Implied by a quiz answer.
    Answered,
    /// Typed by the user; never replaced automatically.
    Typed,
}

fn typed() -> ValueSourceDto {
    ValueSourceDto::Typed
}

/// A number with its source. A missing `source` in input means `typed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SourcedDto<T> {
    /// The value.
    pub value: T,
    /// Where it came from.
    #[serde(default = "typed")]
    pub source: ValueSourceDto,
}

/// The kind of item. Only weapons are supported in this release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ItemKindDto {
    /// A ranged weapon.
    Ranged,
    /// A melee weapon.
    Melee,
}

/// Tech level of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum TechLevelDto {
    /// Neolithic.
    Neolithic,
    /// Medieval.
    Medieval,
    /// Industrial.
    Industrial,
    /// Spacer.
    Spacer,
    /// Ultra.
    Ultra,
    /// Archotech.
    Archotech,
}

/// Item quality.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum QualityDto {
    /// Awful.
    Awful,
    /// Poor.
    Poor,
    /// Normal.
    #[default]
    Normal,
    /// Good.
    Good,
    /// Excellent.
    Excellent,
    /// Masterwork.
    Masterwork,
    /// Legendary.
    Legendary,
}

/// Names of the item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct IdentityDto {
    /// Definition name, for example `RS_TestRifle`.
    pub def_name: String,
    /// Display label.
    pub label: String,
    /// Description text.
    pub description: String,
    /// Prefix the mod uses for its definition names.
    pub mod_prefix: String,
}

/// The parent definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ParentRefDto {
    /// Parent definition name.
    pub def_name: String,
    /// Stats the parent provides, by stat name, ordered by name. Omitted when empty.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, f64>>", optional))]
    pub inherited_stats: BTreeMap<String, f64>,
    /// The tech level the parent supplies. When it equals the spec's tech level no `techLevel` is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub inherited_tech_level: Option<TechLevelDto>,
}

/// One ingredient of the recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CostEntryDto {
    /// Ingredient definition name.
    pub def_name: String,
    /// Amount.
    pub count: f64,
}

/// What the item may be made of.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct StuffSpecDto {
    /// Stuff categories.
    pub categories: Vec<String>,
    /// Amount of stuff. Absent means the engine decides.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub count: Option<SourcedDto<f64>>,
}

/// One melee tool.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ToolSpecDto {
    /// Tool label.
    pub label: String,
    /// Capacity names.
    pub capacities: Vec<String>,
    /// Damage per hit. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub power: Option<SourcedDto<f64>>,
    /// Seconds between hits. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cooldown_time: Option<SourcedDto<f64>>,
    /// Armor penetration. Absent means the game default for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration: Option<SourcedDto<f64>>,
    /// Selection weight. Absent means 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub chance_factor: Option<SourcedDto<f64>>,
    /// Body part group the tool is linked to. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub linked_body_parts_group: Option<String>,
    /// Extra damages the attack deals besides its own. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ExtraMeleeDamageDto>>", optional))]
    pub extra_melee_damages: Vec<ExtraMeleeDamageDto>,
    /// The extra damage of a surprise attack. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub surprise_attack: Option<SurpriseAttackSpecDto>,
    /// Other children of the tool, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
}

/// Accuracy at the four range bands.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct AccuracyInputsDto {
    /// Touch range. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub touch: Option<SourcedDto<f64>>,
    /// Short range. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub short: Option<SourcedDto<f64>>,
    /// Medium range. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub medium: Option<SourcedDto<f64>>,
    /// Long range. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub long: Option<SourcedDto<f64>>,
}

/// An inline projectile definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectileSpecDto {
    /// Definition name.
    pub def_name: String,
    /// Label.
    pub label: String,
    /// Parent definition. Absent means the standard bullet parent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// Damage definition. Absent means the standard bullet damage.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage_def: Option<String>,
    /// Texture path. Absent means inherit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub texture_path: Option<String>,
    /// Graphic class. Absent means the single graphic class.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub graphic_class: Option<String>,
    /// Flight speed. Absent means inherit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<SourcedDto<f64>>,
    /// Stopping power. Absent means inherit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stopping_power: Option<SourcedDto<f64>>,
    /// Other children of the `projectile` element, as written (explosion fields). Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
    /// Other children of the graphic data, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub graphic_extra: Vec<RawNodeDto>,
    /// Other children of the projectile definition, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub thing_extra: Vec<RawNodeDto>,
    /// Fields the writer leaves out because the parent supplies them: `damageDef`, `damageAmountBase`,
    /// `armorPenetrationBase`, `graphicClass`. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub omit_defaults: Vec<String>,
    /// The projectile this one was copied from. Absent when it was written from scratch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub copied_from: Option<String>,
}

/// A reference to an existing projectile or an inline definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case", tag = "mode", content = "def")]
#[allow(clippy::large_enum_variant)] // the DTO mirrors the engine enum, which consumers match on
pub enum ProjectileChoiceDto {
    /// Use an existing projectile by definition name.
    Reference(String),
    /// Define a new projectile with the item.
    Inline(ProjectileSpecDto),
}

/// Ranged weapon inputs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct RangedInputsDto {
    /// Verb class. Absent means the standard shooting verb.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub verb_class: Option<String>,
    /// Damage per projectile. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage: Option<SourcedDto<f64>>,
    /// Armor penetration. Absent means implied by the damage.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration: Option<SourcedDto<f64>>,
    /// Range in tiles. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub range: Option<SourcedDto<f64>>,
    /// Shots per burst. Absent means 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub burst_count: Option<SourcedDto<u32>>,
    /// Ticks between burst shots. Absent means the game default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ticks_between_burst_shots: Option<SourcedDto<f64>>,
    /// Aiming time in seconds. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warmup: Option<SourcedDto<f64>>,
    /// Cooldown in seconds. Absent means not set yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cooldown: Option<SourcedDto<f64>>,
    /// Accuracy per range band.
    pub accuracy: AccuracyInputsDto,
    /// The projectile. Absent means not chosen yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub projectile: Option<ProjectileChoiceDto>,
    /// Cast sound definition. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_cast: Option<String>,
    /// Cast tail sound definition. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_cast_tail: Option<String>,
    /// Muzzle flash scale. Absent means the game default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub muzzle_flash_scale: Option<f64>,
    /// Radius around the target where a missed shot lands. Absent means the game default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub forced_miss_radius: Option<f64>,
    /// Other children of the shooting verb, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub verb_extra: Vec<RawNodeDto>,
}

/// Melee tool penetration values of the Combat Extended block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeToolPenetrationDto {
    /// Tool label the values belong to.
    pub tool: String,
    /// Sharp penetration. Absent means derive it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sharp: Option<SourcedDto<f64>>,
    /// Blunt penetration. Absent means derive it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub blunt: Option<SourcedDto<f64>>,
}

/// The optional Combat Extended patch choices. The block is absent by default; present means the user
/// opted in to generating a patch. Every member that is absent is derived from the vanilla design and the
/// user's own converted weapons, and the derivation is reported back as derived values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CePatchSpecDto {
    /// Caliber name. Absent means ask or derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub caliber: Option<String>,
    /// Ammo set definition name. Absent means ask.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_set: Option<String>,
    /// Default projectile definition name. Absent means derive from the ammo set.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_projectile: Option<String>,
    /// Weapon tag class. Absent means ask.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub weapon_tag_class: Option<String>,
    /// Magazine size. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub magazine_size: Option<SourcedDto<u32>>,
    /// Reload time in seconds. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reload_time: Option<SourcedDto<f64>>,
    /// Bulk. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bulk: Option<SourcedDto<f64>>,
    /// Sway factor. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sway_factor: Option<SourcedDto<f64>>,
    /// Shot spread. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub shot_spread: Option<SourcedDto<f64>>,
    /// Sights efficiency. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sights_efficiency: Option<SourcedDto<f64>>,
    /// Recoil amount. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub recoil_amount: Option<SourcedDto<f64>>,
    /// Cooldown override. Absent means identity with the vanilla cooldown.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cooldown: Option<SourcedDto<f64>>,
    /// Parry bonus (melee). Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parry_bonus: Option<SourcedDto<f64>>,
    /// Melee crit chance. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub melee_crit_chance: Option<SourcedDto<f64>>,
    /// Melee parry chance. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub melee_parry_chance: Option<SourcedDto<f64>>,
    /// Melee dodge chance. Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub melee_dodge_chance: Option<SourcedDto<f64>>,
    /// Per tool penetration values. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeToolPenetrationDto>>", optional))]
    pub tool_penetration: Vec<CeToolPenetrationDto>,
    /// One handed weapon.
    pub one_handed: bool,
    /// Belt fed weapon.
    pub belt_fed: bool,
    /// Bow conversion. Absent means decide from the weapon's shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bow: Option<bool>,
    /// Ammo generated with each magazine of a pawn's kit (`AmmoGenPerMagOverride`). Absent means derive.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_gen_per_mag: Option<SourcedDto<u32>>,
    /// Whether a pawn may fire the weapon while running. Absent means the conversion default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub allow_with_run_and_gun: Option<bool>,
    /// Mass override in kilograms. Absent means the vanilla mass.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mass: Option<SourcedDto<f64>>,
    /// The weapon is a platform that accepts attachments. Omitted when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub is_weapon_platform: bool,
    /// The attachments a platform accepts. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeAttachmentLinkDto>>", optional))]
    pub attachment_links: Vec<CeAttachmentLinkDto>,
    /// The default graphic parts of a platform. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeGraphicPartDto>>", optional))]
    pub default_graphic_parts: Vec<CeGraphicPartDto>,
    /// The under barrel unit. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub under_barrel: Option<CeUnderBarrelDto>,
    /// Reload one round at a time. Absent means not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reload_one_at_a_time: Option<bool>,
    /// The recoil pattern name. Absent means none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub recoil_pattern: Option<String>,
    /// An explicit tool list for the converted weapon. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CeToolPlanDto>>", optional))]
    pub tool_plan: Vec<CeToolPlanDto>,
    /// Vanilla tool fields kept as they are. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub keep_tool_fields: Vec<String>,
    /// Weapon tags added next to the weapon tag class. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub extra_tags: Vec<String>,
    /// Elements written into the patch as given, each validated by the backend. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub raw_extras: Vec<RawNodeDto>,
}

/// The complete design input for one weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignSpecDto {
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// Names.
    #[serde(default)]
    pub identity: IdentityDto,
    /// Parent definition. Absent means no parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<ParentRefDto>,
    /// Tech level. Absent means not chosen yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tech_level: Option<TechLevelDto>,
    /// Role of the weapon, for example `rifle`. Absent means not chosen yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub role: Option<String>,
    /// Mass. Absent means not set yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mass: Option<SourcedDto<f64>>,
    /// Work to make. Absent means not set yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub work_to_make: Option<SourcedDto<f64>>,
    /// Market value. Absent means derive from the recipe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub market_value: Option<SourcedDto<f64>>,
    /// Recipe ingredients. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CostEntryDto>>", optional))]
    pub cost_list: Vec<CostEntryDto>,
    /// Stuff. Absent means the item is not made of stuff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stuff: Option<StuffSpecDto>,
    /// Research prerequisite definition name. Absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub research_prerequisite: Option<String>,
    /// Weapon tags. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub weapon_tags: Vec<String>,
    /// Trade tags. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub trade_tags: Vec<String>,
    /// Weapon classes. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub weapon_classes: Vec<String>,
    /// Texture path. Absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub texture_path: Option<String>,
    /// Graphic class. Absent means the single graphic class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub graphic_class: Option<String>,
    /// Other stat bases by stat name, ordered by name. Omitted when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<BTreeMap<String, SourcedDto<f64>>>", optional)
    )]
    pub extra_stats: BTreeMap<String, SourcedDto<f64>>,
    /// Melee tools. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ToolSpecDto>>", optional))]
    pub tools: Vec<ToolSpecDto>,
    /// Ranged inputs. Absent for a melee weapon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ranged: Option<RangedInputsDto>,
    /// Quality used for the preview readouts.
    #[serde(default)]
    pub preview_quality: QualityDto,
    /// Optional Combat Extended patch choices. Absent by default: the designer writes vanilla only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ce: Option<CePatchSpecDto>,
    /// Sound played when a pawn interacts with the weapon (a sound definition name). Absent means inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_interact: Option<String>,
    /// The crafting recipe. Absent means inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub recipe: Option<RecipeSpecDto>,
    /// Stat offsets while the weapon is equipped, by stat name. Omitted when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, f64>>", optional))]
    pub equipped_stat_offsets: BTreeMap<String, f64>,
    /// The `comps` entries, each a raw list entry. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub comps: Vec<RawNodeDto>,
    /// Menu icon texture path. Absent means inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ui_icon_path: Option<String>,
    /// Menu icon scale. Absent means the game default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ui_icon_scale: Option<f64>,
    /// Draw size of the graphic as the game writes a vector, `(1.5,1.5)`. Absent means inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_size: Option<String>,
    /// Graphic color as the game writes it, `(0.8,0.8,0.8)`. Absent means inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub graphic_color: Option<String>,
    /// Other children of the graphic data, as written. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub graphic_extra: Vec<RawNodeDto>,
    /// Verbs other than the shooting verb, each a raw list entry. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub other_verbs: Vec<RawNodeDto>,
    /// Fields of the definition the designer does not model, as written; the user can remove them.
    /// Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra_fields: Vec<RawNodeDto>,
    /// Attributes of the root element other than `ParentName`, `Name` and `Abstract`. Omitted when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, String>>", optional))]
    pub extra_attrs: BTreeMap<String, String>,
    /// Fields the writer leaves out because the parent supplies them (`graphicClass`, `verbClass`,
    /// `hasStandardCommand`). Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub omit_defaults: Vec<String>,
    /// List containers written with `Inherit="False"`, by element name. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub inherit_reset: Vec<String>,
    /// JSON pointers of required fields the source of a clone does not set either; they are not errors.
    /// Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub accepted_missing: Vec<String>,
    /// Texture files to copy into the project. Omitted when none.
    #[serde(default, skip_serializing_if = "AssetImportsDto::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<AssetImportsDto>", optional))]
    pub assets: AssetImportsDto,
    /// Custom sounds made from clip files. Omitted when none.
    #[serde(default, skip_serializing_if = "SoundImportsDto::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<SoundImportsDto>", optional))]
    pub sounds: SoundImportsDto,
}

impl DesignSpecDto {
    /// A spec of the given kind with empty identity and every optional member absent.
    #[must_use]
    pub fn new(kind: ItemKindDto) -> Self {
        Self {
            kind,
            identity: IdentityDto::default(),
            parent: None,
            tech_level: None,
            role: None,
            mass: None,
            work_to_make: None,
            market_value: None,
            cost_list: Vec::new(),
            stuff: None,
            research_prerequisite: None,
            weapon_tags: Vec::new(),
            trade_tags: Vec::new(),
            weapon_classes: Vec::new(),
            texture_path: None,
            graphic_class: None,
            extra_stats: BTreeMap::new(),
            tools: Vec::new(),
            ranged: None,
            preview_quality: QualityDto::Normal,
            ce: None,
            sound_interact: None,
            recipe: None,
            equipped_stat_offsets: BTreeMap::new(),
            comps: Vec::new(),
            ui_icon_path: None,
            ui_icon_scale: None,
            draw_size: None,
            graphic_color: None,
            graphic_extra: Vec::new(),
            other_verbs: Vec::new(),
            extra_fields: Vec::new(),
            extra_attrs: BTreeMap::new(),
            omit_defaults: Vec::new(),
            inherit_reset: Vec::new(),
            accepted_missing: Vec::new(),
            assets: AssetImportsDto::default(),
            sounds: SoundImportsDto::default(),
        }
    }

    /// True when the user opted in to a Combat Extended patch.
    #[must_use]
    pub fn wants_ce_patch(&self) -> bool {
        self.ce.is_some()
    }
}
