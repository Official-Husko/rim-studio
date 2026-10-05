//! Weapon archetypes: the catalogue of families and descriptors, the proposal of every number of a weapon, and
//! the application of a proposal to a draft.
//!
//! An archetype describes a weapon by what it is (a bolt action rifle, a submachine gun, a long sword) and the
//! descriptors name how it fires and how heavy it is. The numbers come from the user's own install at run time;
//! the contract carries no value table. The descriptors and the balance target serialise exactly as the design
//! engine's own types do, so the toolkit converts by serialising one and deserialising the other.
//!
//! Combat Extended is optional: the calibre list of the catalogue and the `ce` part of a proposal exist only in
//! Combat Extended mode and only when Combat Extended data is loaded. Vanilla mode needs none of it.

use serde::{Deserialize, Serialize};

use super::draft::DraftDto;
use super::fit::FitReportDto;
use super::spec::{ItemKindDto, TechLevelDto};

/// How fast the weapon fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum RateOfFireDto {
    /// A class id of the rate of fire ladder (`slow`, `medium`, `fast`).
    Class(String),
    /// Rounds per minute, read against the typical rate of the archetype.
    Rpm(f64),
}

/// The descriptors of a weapon beyond its archetype. Every one is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DescriptorsDto {
    /// The action (`bolt`, `lever`, `pump`, `semi`, `burst`, `full-auto`, `draw`). Guns only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub action: Option<String>,
    /// The rate of fire; a melee weapon reads it as swing speed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rof: Option<RateOfFireDto>,
    /// The calibre class (`tiny`, `small`, `medium`, `large`, `huge`). Guns only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub calibre: Option<String>,
    /// A Combat Extended ammo set def name: the real calibre, used in Combat Extended mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_set: Option<String>,
    /// The handling class (`compact`, `standard`, `heavy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub handling: Option<String>,
    /// The tier. Absent means the tech level hint of the archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tier: Option<TechLevelDto>,
}

/// Where the strength of the weapon should land among the install's weapons of its class.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum BalanceTargetDto {
    /// Weaker than most of the class (percentile 1/6).
    Weaker,
    /// The middle of the class.
    Typical,
    /// Stronger than most of the class (percentile 5/6).
    Stronger,
    /// An explicit percentile from 0 to 1.
    Percentile(f64),
}

/// Vanilla numbers only, or also a proposal for the optional Combat Extended block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ArchetypeModeDto {
    /// Vanilla numbers only. Needs no Combat Extended.
    #[default]
    Vanilla,
    /// Vanilla numbers plus the calibre as one of the user's ammo sets.
    CombatExtended,
}

/// What the user chose in the archetype dialog. Stored in the draft so the numbers can be proposed again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeChoiceDto {
    /// The archetype id, `family/archetype` (`rifle/assault`).
    pub archetype: String,
    /// The descriptors.
    #[serde(default)]
    pub descriptors: DescriptorsDto,
    /// The balance target.
    #[serde(default = "typical")]
    pub balance: BalanceTargetDto,
    /// Vanilla or Combat Extended mode.
    #[serde(default)]
    pub mode: ArchetypeModeDto,
    /// A strength index to aim at exactly; replaces the balance target when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub strength: Option<f64>,
}

fn typical() -> BalanceTargetDto {
    BalanceTargetDto::Typical
}

// ---------------------------------------------------------------------------------------------------
// The catalogue
// ---------------------------------------------------------------------------------------------------

/// Request of `designer_archetype_catalog`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerArchetypeCatalogRequest {
    /// Only the archetypes of this kind. Absent lists both.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<ItemKindDto>,
    /// True lists the Combat Extended calibres (the ammo sets of the install) as well.
    pub include_calibres: bool,
}

/// A choice with a label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeChoiceOptionDto {
    /// The id to send back.
    pub id: String,
    /// The label to show.
    pub label: String,
    /// A longer line, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub summary: Option<String>,
    /// The cadence multiplier of a rate of fire class.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rate: Option<f64>,
}

/// One archetype of a family and which descriptors apply to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeDto {
    /// The full id, `family/archetype`.
    pub id: String,
    /// The label.
    pub label: String,
    /// One line describing the weapon type.
    pub summary: String,
    /// The kind of weapon.
    pub kind: ItemKindDto,
    /// The descriptors that apply: any of `action`, `rof`, `calibre`, `handling`, `tier`, `balance`.
    pub applies: Vec<String>,
    /// The allowed actions.
    pub actions: Vec<String>,
    /// The default action.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_action: Option<String>,
    /// The allowed rate of fire classes.
    pub rof_classes: Vec<String>,
    /// The default rate of fire class.
    pub default_rof: String,
    /// The typical rounds per minute, the reference of a numeric rate (absent for melee).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ref_rpm: Option<f64>,
    /// The allowed calibre classes.
    pub calibres: Vec<String>,
    /// The default calibre class.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_calibre: Option<String>,
    /// The allowed handling classes.
    pub handlings: Vec<String>,
    /// The default handling class.
    pub default_handling: String,
    /// The tier used when none is chosen.
    pub default_tier: TechLevelDto,
    /// The role of the install's reference pool this archetype is compared with, when the install has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pool_role: Option<String>,
    /// The Combat Extended ammo families that fit.
    pub ammo_families: Vec<String>,
}

/// A family of archetypes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeFamilyDto {
    /// The family id.
    pub id: String,
    /// The label.
    pub label: String,
    /// The kind of weapon.
    pub kind: ItemKindDto,
    /// The archetypes.
    pub archetypes: Vec<ArchetypeDto>,
}

/// A tier of the install.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeTierDto {
    /// The tech level.
    pub tier: TechLevelDto,
    /// The label.
    pub label: String,
    /// How many reference weapons of the kind the install has at the tier (0 for a tier with none).
    pub ranged_count: u32,
    /// How many reference melee weapons the install has at the tier.
    pub melee_count: u32,
}

/// One balance choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BalanceOptionDto {
    /// The value to send back.
    pub target: BalanceTargetDto,
    /// The label.
    pub label: String,
    /// The percentile of the class.
    pub percentile: f64,
}

/// A Combat Extended ammo set offered as a calibre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeCalibreDto {
    /// The ammo set def name.
    pub set: String,
    /// The label of the set.
    pub label: String,
    /// The caliber text.
    pub caliber: String,
    /// The family (pistol, rifle, shotgun, ...), when the categories say.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub family: Option<String>,
    /// The projectile of the first ammo type.
    pub projectile: String,
    /// Damage per shot of the first ammo type.
    pub damage: f64,
    /// Sharp penetration of the first ammo type.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ap_sharp: Option<f64>,
    /// The damage against the median set of the family.
    pub ratio: f64,
    /// How many converted weapons use the set.
    pub weapon_count: u32,
}

/// The Combat Extended part of the catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeCeCatalogDto {
    /// True when Combat Extended data is loaded.
    pub available: bool,
    /// Why not, when it is not available.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
    /// The calibres, by label. Empty unless the request asked for them.
    pub calibres: Vec<CeCalibreDto>,
    /// The AI class tags of the install.
    pub ai_class_tags: Vec<String>,
}

/// Response of `designer_archetype_catalog`: the taxonomy and the choices of every descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeCatalogDto {
    /// The families with their archetypes.
    pub families: Vec<ArchetypeFamilyDto>,
    /// The rate of fire classes.
    pub rof: Vec<ArchetypeChoiceOptionDto>,
    /// The gun actions.
    pub actions: Vec<ArchetypeChoiceOptionDto>,
    /// The calibre classes.
    pub calibres: Vec<ArchetypeChoiceOptionDto>,
    /// The handling classes.
    pub handlings: Vec<ArchetypeChoiceOptionDto>,
    /// The tiers: the six tech levels with the number of reference weapons the install has at each.
    pub tiers: Vec<ArchetypeTierDto>,
    /// The balance choices.
    pub balance: Vec<BalanceOptionDto>,
    /// The size of the install's reference pools: guns and bows, melee weapons.
    pub pool_sizes: [u32; 2],
    /// True when the reference pools exist (a game install is loaded). Without them nothing can be proposed.
    pub proposals_available: bool,
    /// The Combat Extended part.
    pub ce: ArchetypeCeCatalogDto,
}

// ---------------------------------------------------------------------------------------------------
// The proposal
// ---------------------------------------------------------------------------------------------------

/// Request of `designer_archetype_propose`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerArchetypeProposeRequest {
    /// The kind of weapon; must be the kind of the archetype.
    pub kind: ItemKindDto,
    /// The archetype id (`rifle/assault`).
    pub archetype: String,
    /// The descriptors.
    #[serde(default)]
    pub descriptors: DescriptorsDto,
    /// The balance target.
    #[serde(default = "typical")]
    pub balance_target: BalanceTargetDto,
    /// Vanilla or Combat Extended mode.
    #[serde(default)]
    pub mode: ArchetypeModeDto,
    /// A strength index to aim at exactly; replaces the balance target when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub strength: Option<f64>,
    /// The draft the proposal is for. Only used to mark the fields the user decided (`locked`); it is never
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draft: Option<DraftDto>,
}

/// One multiplier of a derivation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FactorTermDto {
    /// What the factor stands for.
    pub label: String,
    /// The multiplier.
    pub factor: f64,
}

/// A proposed number with its reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProposedValueDto {
    /// The JSON pointer of the field in the spec, for example `/ranged/range` or `/tools/1/power`.
    pub field: String,
    /// The reference stat the number is compared with.
    pub stat: String,
    /// The value.
    pub value: f64,
    /// False when the field is better left unset because the game derives it.
    pub write: bool,
    /// The source chip: always `archetype`.
    pub source: String,
    /// A short plain reason.
    pub reason: String,
    /// The install median the shape started from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub median: Option<f64>,
    /// How many reference weapons stand behind the median.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub median_n: Option<u32>,
    /// The multipliers applied to the median.
    pub terms: Vec<FactorTermDto>,
    /// True when the draft holds a value the user decided for this field: the proposal will not replace it.
    pub locked: bool,
}

/// A proposed tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProposedToolDto {
    /// The label.
    pub label: String,
    /// The capacity def names.
    pub capacities: Vec<String>,
    /// The power.
    pub power: ProposedValueDto,
    /// The cooldown.
    pub cooldown: ProposedValueDto,
    /// The explicit armor penetration, when the archetype asks for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration: Option<ProposedValueDto>,
}

/// A line of a proposed cost list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProposedCostDto {
    /// The ingredient def name.
    pub def_name: String,
    /// How many.
    pub count: f64,
    /// A short plain reason.
    pub reason: String,
}

/// The stuff of a proposed melee weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProposedStuffDto {
    /// The stuff category def names.
    pub categories: Vec<String>,
    /// The stuff units per item.
    pub count: f64,
    /// A short plain reason.
    pub reason: String,
}

/// The descriptors as the solver read them, with the defaults filled in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedChoiceDto {
    /// The action (guns only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub action: Option<String>,
    /// The rate of fire class nearest to the choice.
    pub rof: String,
    /// The cadence multiplier.
    pub rate: f64,
    /// The calibre class (guns only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub calibre: Option<String>,
    /// The Combat Extended ammo set the calibre came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_set: Option<String>,
    /// The handling class.
    pub handling: String,
    /// The tier.
    pub tier: TechLevelDto,
}

/// How the strength was aimed and where it landed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StrengthReportDto {
    /// The percentile of the class the target stands for.
    pub percentile: f64,
    /// The strength index aimed at.
    pub target: f64,
    /// The strength index of the rounded proposal.
    pub achieved: f64,
    /// The relative difference of the two.
    pub error: f64,
    /// The scale the solver applied (1 means the shape already fitted).
    pub scale: f64,
    /// True when the target was out of reach.
    pub clamped: bool,
    /// The class the target was read from.
    pub class_label: String,
    /// How many reference weapons stand behind the class.
    pub class_n: u32,
    /// The percentile of the achieved strength among the class.
    pub achieved_percentile: f64,
}

/// The Combat Extended part of a proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeProposalDto {
    /// The ammo set def name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ammo_set: Option<String>,
    /// The caliber text of the set.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub caliber: Option<String>,
    /// The projectile of the first ammo type.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_projectile: Option<String>,
    /// The AI class tag that fits the archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub weapon_tag_class: Option<String>,
    /// How the damage of the set compares with the median set of its family.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage_ratio: Option<f64>,
    /// Remarks.
    pub notes: Vec<String>,
}

/// The proposal for an archetype: every number of the weapon with its reason, and the fit meter's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeProposalDto {
    /// What produced the proposal; `designer_archetype_apply` re-derives the numbers from it.
    pub choice: ArchetypeChoiceDto,
    /// The archetype label.
    pub label: String,
    /// The kind of weapon.
    pub kind: ItemKindDto,
    /// The source chip of every number: `archetype`.
    pub source: String,
    /// The descriptors with defaults.
    pub resolved: ResolvedChoiceDto,
    /// The role of the pool the proposal is compared with.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub role: Option<String>,
    /// The scalar fields in display order.
    pub values: Vec<ProposedValueDto>,
    /// The tools: the attacks of a melee weapon or the bash tools of a gun.
    pub tools: Vec<ProposedToolDto>,
    /// The cost list.
    pub cost_list: Vec<ProposedCostDto>,
    /// The stuff of a melee weapon.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stuff: Option<ProposedStuffDto>,
    /// The weapon tags.
    pub weapon_tags: Vec<String>,
    /// The weapon classes.
    pub weapon_classes: Vec<String>,
    /// The market value the price math gives (informational; it is not written).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub market_value: Option<f64>,
    /// The strength report.
    pub strength: StrengthReportDto,
    /// Remarks: widened classes, missing data, ignored descriptors.
    pub notes: Vec<String>,
    /// The Combat Extended part (Combat Extended mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ce: Option<CeProposalDto>,
    /// The fit meter's verdict on the proposed numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fit: Option<FitReportDto>,
    /// The overall verdict: `typical`, `plausible` or `unusual`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub verdict: Option<String>,
}

// ---------------------------------------------------------------------------------------------------
// Applying
// ---------------------------------------------------------------------------------------------------

/// Request of `designer_archetype_apply`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerArchetypeApplyRequest {
    /// The draft to fill.
    pub draft: DraftDto,
    /// The proposal to apply. Its `choice` is what is applied: the numbers are derived again from it, so a
    /// stale proposal cannot write numbers the solver would not give now.
    pub proposal: ArchetypeProposalDto,
    /// True also fills the optional Combat Extended block (the ammo set, the default projectile and the
    /// suggestions of the existing predictors). False leaves the Combat Extended switch alone.
    #[serde(default)]
    pub include_ce: bool,
    /// True replaces the cost list, stuff, tags, classes, role and tech level even when they are not empty.
    #[serde(default)]
    pub refresh_structure: bool,
    /// True (the default) also fills what is still empty of the structure from the nearest reference weapon
    /// of the install: the parent base and, for a gun, a projectile of its own, so the draft can be planned.
    #[serde(default = "yes")]
    pub complete_structure: bool,
}

fn yes() -> bool {
    true
}

/// Response of `designer_archetype_apply`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerArchetypeApplyResponse {
    /// The draft with the numbers filled in and the choice recorded.
    pub draft: DraftDto,
    /// The pointers of the fields that now hold a proposed number.
    pub filled: Vec<String>,
    /// The pointers of the fields that kept the value the user decided.
    pub kept: Vec<String>,
    /// The proposal that was applied.
    pub proposal: ArchetypeProposalDto,
    /// Plain notes about the structure that was completed, or why it could not be.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub notes: Vec<String>,
}
