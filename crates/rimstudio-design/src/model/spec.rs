//! The design spec: the designer's input for one weapon, in the vanilla terms of the game.
//!
//! The spec holds what the user typed, what the baseline suggested and what a quiz answer implied, each
//! numeric value tagged with its [`ValueSource`]. It always describes a vanilla definition. The optional
//! Combat Extended choices live in [`CePatchSpec`] and are `None` by default: nothing in this module turns
//! them on, and no function of this crate writes them into the vanilla definition (owner rule D-085).
//!
//! JSON shape: camelCase field names, kebab-case enum strings, absent options omitted. Field paths in
//! diagnostics are JSON pointers into this structure, for example `/ranged/damage` or `/tools/1/power`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rimstudio_core::tree::Node;

use super::assets::{AssetImports, SoundImports};
use super::carried::{ExtraMeleeDamage, RecipeSpec, SurpriseAttackSpec};
use super::source::{OfferOutcome, Sourced, ValueSource, offer};

/// The kind of item a spec describes. Apparel is deferred, hence the non exhaustive marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ItemKind {
    /// A gun or bow: a shooting verb, a projectile, accuracy by range.
    Ranged,
    /// A melee weapon: tools with capacities and cooldowns.
    Melee,
}

impl ItemKind {
    /// The kebab-case name used on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ranged => "ranged",
            Self::Melee => "melee",
        }
    }
}

/// The tier of an item, named after the game's tech levels (Neolithic is tier 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TechLevel {
    /// Tier 0.
    Neolithic,
    /// Tier 1.
    Medieval,
    /// Tier 2.
    Industrial,
    /// Tier 3.
    Spacer,
    /// Tier 4.
    Ultra,
    /// Tier 5.
    Archotech,
}

impl TechLevel {
    /// All levels, lowest first.
    pub const ALL: [TechLevel; 6] = [
        TechLevel::Neolithic,
        TechLevel::Medieval,
        TechLevel::Industrial,
        TechLevel::Spacer,
        TechLevel::Ultra,
        TechLevel::Archotech,
    ];

    /// The tier index, 0 for Neolithic to 5 for Archotech.
    #[must_use]
    pub fn index(self) -> u8 {
        match self {
            Self::Neolithic => 0,
            Self::Medieval => 1,
            Self::Industrial => 2,
            Self::Spacer => 3,
            Self::Ultra => 4,
            Self::Archotech => 5,
        }
    }

    /// The name the game writes in a `techLevel` element.
    #[must_use]
    pub fn xml_name(self) -> &'static str {
        match self {
            Self::Neolithic => "Neolithic",
            Self::Medieval => "Medieval",
            Self::Industrial => "Industrial",
            Self::Spacer => "Spacer",
            Self::Ultra => "Ultra",
            Self::Archotech => "Archotech",
        }
    }
}

/// Item quality used for previews only. It is never written into a definition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Quality {
    /// Worst quality.
    Awful,
    /// Below normal.
    Poor,
    /// The default.
    #[default]
    Normal,
    /// Above normal.
    Good,
    /// Well above normal.
    Excellent,
    /// Near the top.
    Masterwork,
    /// The best quality.
    Legendary,
}

/// Identity of the item: names and the mod prefix used by the naming rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Identity {
    /// The def name, unique among loaded defs. REQ.
    pub def_name: String,
    /// The label shown in game. REQ.
    pub label: String,
    /// The description text (OPT, inherited from the parent when empty).
    pub description: String,
    /// The mod prefix without the trailing underscore. When set, the def name should start with `<prefix>_`.
    pub mod_prefix: String,
}

/// The parent base def and the values inherited from it.
///
/// Inherited values are copied from the parent for display and for the family rule (a generated def lists
/// only the stats the family lists and inherits the rest). They are never written into the output.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ParentRef {
    /// The def name written in `ParentName`.
    pub def_name: String,
    /// Stat values the parent provides, by stat def name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub inherited_stats: BTreeMap<String, f64>,
    /// The tech level the parent supplies. When it equals the spec's tech level the definition does not
    /// write a `techLevel` of its own: the parent already provides it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherited_tech_level: Option<TechLevel>,
}

impl ParentRef {
    /// A parent reference with no recorded inherited values.
    #[must_use]
    pub fn named(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            inherited_stats: BTreeMap::new(),
            inherited_tech_level: None,
        }
    }
}

/// One line of the cost list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CostEntry {
    /// The def name of the ingredient.
    pub def_name: String,
    /// How many are needed.
    pub count: f64,
}

impl CostEntry {
    /// A cost line.
    #[must_use]
    pub fn new(def_name: impl Into<String>, count: f64) -> Self {
        Self {
            def_name: def_name.into(),
            count,
        }
    }
}

/// The stuff part of a stuffed weapon: which materials it can be made of and how many units it takes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StuffSpec {
    /// Stuff category def names.
    pub categories: Vec<String>,
    /// Stuff units per item (`costStuffCount`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<Sourced<f64>>,
}

/// One tool: a melee attack source (the weapon's blade, handle, or the gun bash of a gun).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolSpec {
    /// The label of the tool. REQ.
    pub label: String,
    /// Capacity def names; each capacity is one attack. REQ, at least one.
    pub capacities: Vec<String>,
    /// Tool power, the base damage. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power: Option<Sourced<f64>>,
    /// Cooldown in seconds. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_time: Option<Sourced<f64>>,
    /// Explicit armor penetration as a fraction. OPT: the game uses 0.015 per damage point when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration: Option<Sourced<f64>>,
    /// Relative weight when the game picks an attack. OPT, 1 when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chance_factor: Option<Sourced<f64>>,
    /// The body part group that carries the tool (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_body_parts_group: Option<String>,
    /// Extra damages the attack deals besides its own (OPT).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_melee_damages: Vec<ExtraMeleeDamage>,
    /// The extra damage of a surprise attack (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surprise_attack: Option<SurpriseAttackSpec>,
    /// Other children of the tool, as written (for example `labelUsedInLogging`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
}

impl ToolSpec {
    /// A tool with a label and capacities and no numbers yet.
    #[must_use]
    pub fn new(label: impl Into<String>, capacities: &[&str]) -> Self {
        Self {
            label: label.into(),
            capacities: capacities.iter().map(|c| (*c).to_owned()).collect(),
            ..Self::default()
        }
    }

    /// Sets the power and cooldown with the given source.
    #[must_use]
    pub fn with_numbers(mut self, power: f64, cooldown: f64, source: ValueSource) -> Self {
        self.power = Some(Sourced::new(power, source));
        self.cooldown_time = Some(Sourced::new(cooldown, source));
        self
    }
}

/// Accuracy by range band, each a fraction from 0 to 1.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccuracyInputs {
    /// Accuracy at touch range. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub touch: Option<Sourced<f64>>,
    /// Accuracy at short range. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<Sourced<f64>>,
    /// Accuracy at medium range. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<Sourced<f64>>,
    /// Accuracy at long range. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<Sourced<f64>>,
}

/// A projectile definition the designer creates together with the weapon.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectileSpec {
    /// The def name of the new projectile. REQ.
    pub def_name: String,
    /// Label of the projectile (OPT).
    pub label: String,
    /// Parent base of the projectile. OPT: [`DEFAULT_PROJECTILE_PARENT`] when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The damage def name. OPT: [`DEFAULT_DAMAGE_DEF`] when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_def: Option<String>,
    /// Texture path of the projectile (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_path: Option<String>,
    /// Graphic class of the projectile (OPT, [`DEFAULT_GRAPHIC_CLASS`] when a texture is set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphic_class: Option<String>,
    /// Flight speed (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Sourced<f64>>,
    /// Stopping power (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopping_power: Option<Sourced<f64>>,
    /// Other children of the `projectile` element, as written (explosion fields, flight behaviour).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
    /// Other children of the `graphicData` element, as written (draw size, shader).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graphic_extra: Vec<Node>,
    /// Other children of the projectile's thing def, as written (thing class, label hints).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub thing_extra: Vec<Node>,
    /// Fields the writer would add (its defaults and the values of the inputs) that the projectile this one
    /// was copied from gets from its parent instead: `damageDef`, `damageAmountBase`,
    /// `armorPenetrationBase`, `graphicClass`. They are not written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub omit_defaults: Vec<String>,
    /// The def name of the projectile this one was copied from (OPT). Giving the weapon back the shared
    /// projectile uses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
}

/// Parent base written when a new projectile names none. A def name, not a value.
pub const DEFAULT_PROJECTILE_PARENT: &str = "BaseBullet";
/// Damage def written when a new projectile names none.
pub const DEFAULT_DAMAGE_DEF: &str = "Bullet";
/// Graphic class written next to a texture path when none is chosen.
pub const DEFAULT_GRAPHIC_CLASS: &str = "Graphic_Single";
/// Verb class written for a ranged weapon when none is chosen.
pub const DEFAULT_VERB_CLASS: &str = "Verb_Shoot";

/// Which projectile a ranged weapon fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "mode", content = "def")]
#[allow(clippy::large_enum_variant)] // boxing would change the public shape every consumer matches on
pub enum ProjectileChoice {
    /// An existing projectile def, referenced by name. The damage input is informational (it feeds the
    /// readouts); nothing about the projectile is written.
    Reference(String),
    /// A new projectile def created with the weapon. The damage and armor penetration inputs are written
    /// into it.
    Inline(ProjectileSpec),
}

impl ProjectileChoice {
    /// The def name the verb will use as `defaultProjectile`.
    #[must_use]
    pub fn def_name(&self) -> &str {
        match self {
            Self::Reference(name) => name,
            Self::Inline(spec) => &spec.def_name,
        }
    }
}

/// The inputs only a ranged weapon has: the shooting verb and the projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RangedInputs {
    /// The verb class (OPT, [`DEFAULT_VERB_CLASS`] when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_class: Option<String>,
    /// Projectile damage per shot. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<Sourced<f64>>,
    /// Explicit armor penetration as a fraction. OPT: the game implies 0.015 per damage point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration: Option<Sourced<f64>>,
    /// Maximum range in cells. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Sourced<f64>>,
    /// Shots per burst. OPT, 1 when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_count: Option<Sourced<u32>>,
    /// Ticks between burst shots. OPT, 15 when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks_between_burst_shots: Option<Sourced<f64>>,
    /// Warmup in seconds. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warmup: Option<Sourced<f64>>,
    /// Cooldown in seconds. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<Sourced<f64>>,
    /// Accuracy by range band. REQ.
    pub accuracy: AccuracyInputs,
    /// The projectile. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projectile: Option<ProjectileChoice>,
    /// Sound def of the shot (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_cast: Option<String>,
    /// Sound def of the shot's tail (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_cast_tail: Option<String>,
    /// Muzzle flash scale (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muzzle_flash_scale: Option<f64>,
    /// Radius in cells around the target where a missed shot lands (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_miss_radius: Option<f64>,
    /// Other children of the shooting verb, as written (aiming effects, log rules).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub verb_extra: Vec<Node>,
}

/// Penetration of one tool in a Combat Extended patch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeToolPenetration {
    /// The label of the vanilla tool this entry converts.
    pub tool: String,
    /// Sharp penetration in mm of rolled homogeneous armor. REQ for Cut and Stab tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharp: Option<Sourced<f64>>,
    /// Blunt penetration in MPa. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blunt: Option<Sourced<f64>>,
}

/// The optional Combat Extended choices of an item.
///
/// A spec has this only when the user turned on "Add a Combat Extended patch (optional)". The vanilla
/// definition is unaffected by anything in here. Values arrive already decided (typed, answered or
/// suggested); the patch generator shapes them and computes nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CePatchSpec {
    /// The caliber the user picked (informational; the ammo set is what the patch writes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caliber: Option<String>,
    /// The ammo set def. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// The default projectile, a member of the ammo set. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_projectile: Option<String>,
    /// The one `CE_AI_*` class tag chosen from the installed CE data (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_tag_class: Option<String>,
    /// Magazine size in rounds. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magazine_size: Option<Sourced<u32>>,
    /// Reload time in seconds. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reload_time: Option<Sourced<f64>>,
    /// The `Bulk` stat. REQ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulk: Option<Sourced<f64>>,
    /// The `SwayFactor` stat. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sway_factor: Option<Sourced<f64>>,
    /// The `ShotSpread` stat. REQ for guns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot_spread: Option<Sourced<f64>>,
    /// The `SightsEfficiency` stat (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sights_efficiency: Option<Sourced<f64>>,
    /// The verb's `recoilAmount` (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoil_amount: Option<Sourced<f64>>,
    /// Overrides `RangedWeapon_Cooldown` in the patch (OPT; the vanilla cooldown is the default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<Sourced<f64>>,
    /// The `MeleeCounterParryBonus` stat (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parry_bonus: Option<Sourced<f64>>,
    /// Melee crit chance offset. REQ for melee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub melee_crit_chance: Option<Sourced<f64>>,
    /// Melee parry chance offset. REQ for melee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub melee_parry_chance: Option<Sourced<f64>>,
    /// Melee dodge chance offset. REQ for melee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub melee_dodge_chance: Option<Sourced<f64>>,
    /// Penetration of each converted tool.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_penetration: Vec<CeToolPenetration>,
    /// The weapon is held in one hand.
    pub one_handed: bool,
    /// The weapon is belt fed.
    pub belt_fed: bool,
    /// Convert the weapon in the bow style (an arrow or bolt set, empty fire modes, a spawn count instead
    /// of a magazine). `None` lets the generator tell from the weapon's shape (role, tags, projectile);
    /// `Some(false)` keeps a bow shaped weapon on the gun conversion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bow: Option<bool>,
    /// The `AmmoGenPerMagOverride` of the ammo component: how many rounds of ammo a pawn spawns with per
    /// magazine, for a weapon that has no real magazine (a bow). OPT.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_gen_per_mag: Option<Sourced<u32>>,
    /// Whether the weapon may be fired while the Run and Gun mod lets pawns run. `None` is the default of
    /// the style (a bow may not, a gun may); only `Some(false)` is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_with_run_and_gun: Option<bool>,
    /// The `Mass` stat of the patch, written only when it differs from the vanilla mass (conversion keeps
    /// the mass of a bow). OPT.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass: Option<Sourced<f64>>,
    /// Convert the weapon into a weapon platform that accepts attachments (OPT, off by default). Also on
    /// when attachment links or default graphic parts are given.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_weapon_platform: bool,
    /// The attachment links of the platform (OPT).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachment_links: Vec<super::platform::CeAttachmentLink>,
    /// The default graphic parts of the platform, drawn while their slots are empty (OPT).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default_graphic_parts: Vec<super::platform::CeGraphicPart>,
    /// An under barrel unit with its own ammo, verb and fire modes (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under_barrel: Option<super::platform::CeUnderBarrel>,
    /// The ammo component loads one round at a time (`reloadOneAtATime`), as a pump shotgun does. OPT, a
    /// gun only; only `Some(true)` is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reload_one_at_a_time: Option<bool>,
    /// The `recoilPattern` of the converted verb, a value of the recoil pattern enumeration (OPT, a gun
    /// only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoil_pattern: Option<String>,
    /// An explicit tool list: when it is not empty the converted tools are exactly these entries, in this
    /// order, in place of the tools derived from the design (OPT; the way to write a muzzle tool or
    /// relabelled tools on purpose).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_plan: Vec<super::ce_extras::CeToolPlan>,
    /// Children of the vanilla tools that the conversion keeps although the user's own conversions drop
    /// them (OPT; the opt out of the habit that removes `labelUsedInLogging`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keep_tool_fields: Vec<String>,
    /// Weapon tags written besides the class tag and the one handed mark: the companion tags of the
    /// weapon class that the user accepted (OPT; never filled in by the generator on its own).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_tags: Vec<String>,
    /// Raw nodes added to the converted def as written, for art or platform specific Combat Extended
    /// additions that are not modelled (for example `modExtensions` with a gun draw extension). Each node
    /// is checked for its XML shape; a node whose name the conversion owns is refused (OPT).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub raw_extras: Vec<Node>,
    /// A custom caliber with its own projectiles, ammo items and recipes (OPT). When present the plan adds
    /// the ammunition definition files to the Combat Extended folder and the conversion uses the custom ammo
    /// set and the projectile of its default type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_ammo: Option<super::ce_ammo::CustomAmmoSpec>,
}

/// A numeric input addressed by name, used by suggestions, answers and anchors to write into a spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScalarField {
    /// `/mass`
    Mass,
    /// `/workToMake`
    WorkToMake,
    /// `/marketValue`
    MarketValue,
    /// `/stuff/count`
    StuffCount,
    /// `/ranged/damage`
    Damage,
    /// `/ranged/armorPenetration`
    ArmorPenetration,
    /// `/ranged/range`
    Range,
    /// `/ranged/burstCount`
    BurstCount,
    /// `/ranged/ticksBetweenBurstShots`
    TicksBetweenBurstShots,
    /// `/ranged/warmup`
    Warmup,
    /// `/ranged/cooldown`
    Cooldown,
    /// `/ranged/accuracy/touch`
    AccuracyTouch,
    /// `/ranged/accuracy/short`
    AccuracyShort,
    /// `/ranged/accuracy/medium`
    AccuracyMedium,
    /// `/ranged/accuracy/long`
    AccuracyLong,
    /// `/tools/<i>/power`
    ToolPower(usize),
    /// `/tools/<i>/cooldownTime`
    ToolCooldown(usize),
    /// `/tools/<i>/armorPenetration`
    ToolArmorPenetration(usize),
    /// `/tools/<i>/chanceFactor`
    ToolChanceFactor(usize),
    /// `/ce/magazineSize`
    CeMagazineSize,
    /// `/ce/reloadTime`
    CeReloadTime,
    /// `/ce/bulk`
    CeBulk,
    /// `/ce/swayFactor`
    CeSwayFactor,
    /// `/ce/shotSpread`
    CeShotSpread,
    /// `/ce/sightsEfficiency`
    CeSightsEfficiency,
    /// `/ce/recoilAmount`
    CeRecoilAmount,
    /// `/ce/cooldown`
    CeCooldown,
    /// `/ce/parryBonus`
    CeParryBonus,
    /// `/ce/meleeCritChance`
    CeMeleeCritChance,
    /// `/ce/meleeParryChance`
    CeMeleeParryChance,
    /// `/ce/meleeDodgeChance`
    CeMeleeDodgeChance,
    /// `/ce/toolPenetration/<i>/sharp`
    CeToolSharp(usize),
    /// `/ce/toolPenetration/<i>/blunt`
    CeToolBlunt(usize),
}

impl ScalarField {
    /// The JSON pointer of the field inside a [`DesignSpec`].
    #[must_use]
    pub fn pointer(self) -> String {
        match self {
            Self::Mass => "/mass".into(),
            Self::WorkToMake => "/workToMake".into(),
            Self::MarketValue => "/marketValue".into(),
            Self::StuffCount => "/stuff/count".into(),
            Self::Damage => "/ranged/damage".into(),
            Self::ArmorPenetration => "/ranged/armorPenetration".into(),
            Self::Range => "/ranged/range".into(),
            Self::BurstCount => "/ranged/burstCount".into(),
            Self::TicksBetweenBurstShots => "/ranged/ticksBetweenBurstShots".into(),
            Self::Warmup => "/ranged/warmup".into(),
            Self::Cooldown => "/ranged/cooldown".into(),
            Self::AccuracyTouch => "/ranged/accuracy/touch".into(),
            Self::AccuracyShort => "/ranged/accuracy/short".into(),
            Self::AccuracyMedium => "/ranged/accuracy/medium".into(),
            Self::AccuracyLong => "/ranged/accuracy/long".into(),
            Self::ToolPower(i) => format!("/tools/{i}/power"),
            Self::ToolCooldown(i) => format!("/tools/{i}/cooldownTime"),
            Self::ToolArmorPenetration(i) => format!("/tools/{i}/armorPenetration"),
            Self::ToolChanceFactor(i) => format!("/tools/{i}/chanceFactor"),
            Self::CeMagazineSize => "/ce/magazineSize".into(),
            Self::CeReloadTime => "/ce/reloadTime".into(),
            Self::CeBulk => "/ce/bulk".into(),
            Self::CeSwayFactor => "/ce/swayFactor".into(),
            Self::CeShotSpread => "/ce/shotSpread".into(),
            Self::CeSightsEfficiency => "/ce/sightsEfficiency".into(),
            Self::CeRecoilAmount => "/ce/recoilAmount".into(),
            Self::CeCooldown => "/ce/cooldown".into(),
            Self::CeParryBonus => "/ce/parryBonus".into(),
            Self::CeMeleeCritChance => "/ce/meleeCritChance".into(),
            Self::CeMeleeParryChance => "/ce/meleeParryChance".into(),
            Self::CeMeleeDodgeChance => "/ce/meleeDodgeChance".into(),
            Self::CeToolSharp(i) => format!("/ce/toolPenetration/{i}/sharp"),
            Self::CeToolBlunt(i) => format!("/ce/toolPenetration/{i}/blunt"),
        }
    }

    /// A short English name for messages: `damage`, `tool 2 power` (tools count from 1), `CE bulk`.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Mass => "mass".into(),
            Self::WorkToMake => "work to make".into(),
            Self::MarketValue => "market value".into(),
            Self::StuffCount => "stuff count".into(),
            Self::Damage => "damage".into(),
            Self::ArmorPenetration => "armor penetration".into(),
            Self::Range => "range".into(),
            Self::BurstCount => "burst count".into(),
            Self::TicksBetweenBurstShots => "ticks between burst shots".into(),
            Self::Warmup => "warmup".into(),
            Self::Cooldown => "cooldown".into(),
            Self::AccuracyTouch => "touch accuracy".into(),
            Self::AccuracyShort => "short accuracy".into(),
            Self::AccuracyMedium => "medium accuracy".into(),
            Self::AccuracyLong => "long accuracy".into(),
            Self::ToolPower(i) => format!("tool {} power", i.saturating_add(1)),
            Self::ToolCooldown(i) => format!("tool {} cooldown", i.saturating_add(1)),
            Self::ToolArmorPenetration(i) => {
                format!("tool {} armor penetration", i.saturating_add(1))
            }
            Self::ToolChanceFactor(i) => format!("tool {} chance factor", i.saturating_add(1)),
            Self::CeMagazineSize => "CE magazine size".into(),
            Self::CeReloadTime => "CE reload time".into(),
            Self::CeBulk => "CE bulk".into(),
            Self::CeSwayFactor => "CE sway factor".into(),
            Self::CeShotSpread => "CE shot spread".into(),
            Self::CeSightsEfficiency => "CE sights efficiency".into(),
            Self::CeRecoilAmount => "CE recoil amount".into(),
            Self::CeCooldown => "CE cooldown".into(),
            Self::CeParryBonus => "CE parry bonus".into(),
            Self::CeMeleeCritChance => "CE melee crit chance".into(),
            Self::CeMeleeParryChance => "CE melee parry chance".into(),
            Self::CeMeleeDodgeChance => "CE melee dodge chance".into(),
            Self::CeToolSharp(i) => format!("CE tool {} sharp penetration", i.saturating_add(1)),
            Self::CeToolBlunt(i) => format!("CE tool {} blunt penetration", i.saturating_add(1)),
        }
    }

    /// True for the fields of the optional Combat Extended patch.
    #[must_use]
    pub fn is_ce(self) -> bool {
        matches!(
            self,
            Self::CeMagazineSize
                | Self::CeReloadTime
                | Self::CeBulk
                | Self::CeSwayFactor
                | Self::CeShotSpread
                | Self::CeSightsEfficiency
                | Self::CeRecoilAmount
                | Self::CeCooldown
                | Self::CeParryBonus
                | Self::CeMeleeCritChance
                | Self::CeMeleeParryChance
                | Self::CeMeleeDodgeChance
                | Self::CeToolSharp(_)
                | Self::CeToolBlunt(_)
        )
    }

    /// True for the fields that hold a whole count (shots, rounds) and not a measurement.
    #[must_use]
    pub fn is_count(self) -> bool {
        matches!(self, Self::BurstCount | Self::CeMagazineSize)
    }
}

/// A mutable view of the slot behind a [`ScalarField`].
enum Slot<'a> {
    Float(&'a mut Option<Sourced<f64>>),
    Count(&'a mut Option<Sourced<u32>>),
}

/// The designer's input for one item. See the module documentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignSpec {
    /// The kind of item.
    pub kind: ItemKind,
    /// Names and prefix.
    #[serde(default)]
    pub identity: Identity,
    /// The parent base def and inherited values. REQ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ParentRef>,
    /// The tier. REQ (pool selector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tech_level: Option<TechLevel>,
    /// The role label (a pool selector such as `rifle` or `blade`; the set of roles is generated at run
    /// time). REQ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Mass in kilograms. REQ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass: Option<Sourced<f64>>,
    /// Work to make. REQ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_to_make: Option<Sourced<f64>>,
    /// Explicit market value override. OPT: normally the value follows from the cost list and the work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_value: Option<Sourced<f64>>,
    /// The cost list. REQ for ranged weapons; a melee weapon needs it or a stuff spec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cost_list: Vec<CostEntry>,
    /// Stuff categories and count. A melee weapon needs this or a cost list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stuff: Option<StuffSpec>,
    /// Research prerequisite def (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_prerequisite: Option<String>,
    /// Weapon tags (OPT).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapon_tags: Vec<String>,
    /// Trade tags (OPT).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trade_tags: Vec<String>,
    /// Weapon class defs (OPT).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapon_classes: Vec<String>,
    /// Texture path (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_path: Option<String>,
    /// Graphic class (OPT, [`DEFAULT_GRAPHIC_CLASS`] when a texture path is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphic_class: Option<String>,
    /// Further stat bases the user sets explicitly, by stat def name. Inherited stats are not repeated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_stats: BTreeMap<String, Sourced<f64>>,
    /// Tools: the attacks of a melee weapon (REQ for melee) or the gun bash of a gun (OPT).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
    /// Ranged inputs. Present for ranged items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranged: Option<RangedInputs>,
    /// Quality used by previews only; never written.
    #[serde(default)]
    pub preview_quality: Quality,
    /// The optional Combat Extended patch. `None` means the toggle is off, which is the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ce: Option<CePatchSpec>,
    /// The sound played when a pawn picks up or interacts with the weapon (OPT, a `SoundDef` name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_interact: Option<String>,
    /// The crafting recipe: skill requirements, display priority, workbenches (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeSpec>,
    /// Stat offsets while the weapon is equipped, by stat def name (OPT).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub equipped_stat_offsets: BTreeMap<String, f64>,
    /// The `comps` entries (quality, art, charges), each a raw `li` node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comps: Vec<Node>,
    /// The icon texture path of the item in menus (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_icon_path: Option<String>,
    /// The scale of the menu icon (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_icon_scale: Option<f64>,
    /// The draw size of the item graphic as the game writes a vector, `(1.5,1.5)` (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draw_size: Option<String>,
    /// The graphic color as the game writes it, `(0.8,0.8,0.8)` (OPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphic_color: Option<String>,
    /// Other children of `graphicData`, as written (shader, second color, offsets).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphic_extra: Vec<Node>,
    /// Verbs of the definition other than the shooting verb, each a raw `li` node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_verbs: Vec<Node>,
    /// Fields of the definition the designer does not model, as written. They are emitted verbatim after
    /// the modelled fields and the user can remove them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_fields: Vec<Node>,
    /// Attributes of the root element other than `ParentName`, `Name` and `Abstract` (for example
    /// `MayRequire`), by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_attrs: BTreeMap<String, String>,
    /// Fields the writer would add by default (`graphicClass`, `verbClass`, `hasStandardCommand`) that the
    /// source of a clone does not define, because its parent supplies them. They are not written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omit_defaults: Vec<String>,
    /// The list containers written with `Inherit="False"` (they replace the parent's list), by element name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inherit_reset: Vec<String>,
    /// JSON pointers of required fields the source of a clone does not define either. Such a field is not
    /// an error: the written definition lacks it as the source does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_missing: Vec<String>,
    /// Texture files to copy into the project (OPT). Absent by default.
    #[serde(default, skip_serializing_if = "AssetImports::is_empty")]
    pub assets: AssetImports,
    /// Custom sounds made from clip files (OPT). Absent by default.
    #[serde(default, skip_serializing_if = "SoundImports::is_empty")]
    pub sounds: SoundImports,
}

impl DesignSpec {
    /// An empty spec of the given kind with the def name and label set. The CE patch is off.
    #[must_use]
    pub fn new(kind: ItemKind, def_name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            kind,
            identity: Identity {
                def_name: def_name.into(),
                label: label.into(),
                ..Identity::default()
            },
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
            ranged: (kind == ItemKind::Ranged).then(RangedInputs::default),
            preview_quality: Quality::Normal,
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
            assets: AssetImports::default(),
            sounds: SoundImports::default(),
        }
    }

    /// An empty ranged spec.
    #[must_use]
    pub fn new_ranged(def_name: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ItemKind::Ranged, def_name, label)
    }

    /// An empty melee spec.
    #[must_use]
    pub fn new_melee(def_name: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ItemKind::Melee, def_name, label)
    }

    /// True when the user turned the optional Combat Extended patch on for this item.
    #[must_use]
    pub fn ce_patch_enabled(&self) -> bool {
        self.ce.is_some()
    }

    /// The value of a scalar field, if it is set.
    #[must_use]
    pub fn scalar(&self, field: ScalarField) -> Option<Sourced<f64>> {
        match field {
            ScalarField::BurstCount => self
                .ranged
                .as_ref()?
                .burst_count
                .map(|s| Sourced::new(f64::from(s.value), s.source)),
            ScalarField::CeMagazineSize => self
                .ce
                .as_ref()?
                .magazine_size
                .map(|s| Sourced::new(f64::from(s.value), s.source)),
            other => self.float_slot_ref(other).copied().flatten(),
        }
    }

    fn float_slot_ref(&self, field: ScalarField) -> Option<&Option<Sourced<f64>>> {
        let ranged = self.ranged.as_ref();
        let ce = self.ce.as_ref();
        Some(match field {
            ScalarField::Mass => &self.mass,
            ScalarField::WorkToMake => &self.work_to_make,
            ScalarField::MarketValue => &self.market_value,
            ScalarField::StuffCount => &self.stuff.as_ref()?.count,
            ScalarField::Damage => &ranged?.damage,
            ScalarField::ArmorPenetration => &ranged?.armor_penetration,
            ScalarField::Range => &ranged?.range,
            ScalarField::TicksBetweenBurstShots => &ranged?.ticks_between_burst_shots,
            ScalarField::Warmup => &ranged?.warmup,
            ScalarField::Cooldown => &ranged?.cooldown,
            ScalarField::AccuracyTouch => &ranged?.accuracy.touch,
            ScalarField::AccuracyShort => &ranged?.accuracy.short,
            ScalarField::AccuracyMedium => &ranged?.accuracy.medium,
            ScalarField::AccuracyLong => &ranged?.accuracy.long,
            ScalarField::ToolPower(i) => &self.tools.get(i)?.power,
            ScalarField::ToolCooldown(i) => &self.tools.get(i)?.cooldown_time,
            ScalarField::ToolArmorPenetration(i) => &self.tools.get(i)?.armor_penetration,
            ScalarField::ToolChanceFactor(i) => &self.tools.get(i)?.chance_factor,
            ScalarField::CeReloadTime => &ce?.reload_time,
            ScalarField::CeBulk => &ce?.bulk,
            ScalarField::CeSwayFactor => &ce?.sway_factor,
            ScalarField::CeShotSpread => &ce?.shot_spread,
            ScalarField::CeSightsEfficiency => &ce?.sights_efficiency,
            ScalarField::CeRecoilAmount => &ce?.recoil_amount,
            ScalarField::CeCooldown => &ce?.cooldown,
            ScalarField::CeParryBonus => &ce?.parry_bonus,
            ScalarField::CeMeleeCritChance => &ce?.melee_crit_chance,
            ScalarField::CeMeleeParryChance => &ce?.melee_parry_chance,
            ScalarField::CeMeleeDodgeChance => &ce?.melee_dodge_chance,
            ScalarField::CeToolSharp(i) => &ce?.tool_penetration.get(i)?.sharp,
            ScalarField::CeToolBlunt(i) => &ce?.tool_penetration.get(i)?.blunt,
            ScalarField::BurstCount | ScalarField::CeMagazineSize => return None,
        })
    }

    /// The slot behind a field, creating the ranged block of a ranged item on demand. Returns `None` when the
    /// field does not apply (wrong kind, missing tool, CE patch off). The CE block is never created here.
    fn slot_mut(&mut self, field: ScalarField) -> Option<Slot<'_>> {
        match field {
            ScalarField::Mass => Some(Slot::Float(&mut self.mass)),
            ScalarField::WorkToMake => Some(Slot::Float(&mut self.work_to_make)),
            ScalarField::MarketValue => Some(Slot::Float(&mut self.market_value)),
            ScalarField::StuffCount => Some(Slot::Float(&mut self.stuff.as_mut()?.count)),
            ScalarField::ToolPower(i) => Some(Slot::Float(&mut self.tools.get_mut(i)?.power)),
            ScalarField::ToolCooldown(i) => {
                Some(Slot::Float(&mut self.tools.get_mut(i)?.cooldown_time))
            }
            ScalarField::ToolArmorPenetration(i) => {
                Some(Slot::Float(&mut self.tools.get_mut(i)?.armor_penetration))
            }
            ScalarField::ToolChanceFactor(i) => {
                Some(Slot::Float(&mut self.tools.get_mut(i)?.chance_factor))
            }
            ScalarField::Damage
            | ScalarField::ArmorPenetration
            | ScalarField::Range
            | ScalarField::BurstCount
            | ScalarField::TicksBetweenBurstShots
            | ScalarField::Warmup
            | ScalarField::Cooldown
            | ScalarField::AccuracyTouch
            | ScalarField::AccuracyShort
            | ScalarField::AccuracyMedium
            | ScalarField::AccuracyLong => {
                if self.kind != ItemKind::Ranged {
                    return None;
                }
                let r = self.ranged.get_or_insert_with(RangedInputs::default);
                Some(match field {
                    ScalarField::Damage => Slot::Float(&mut r.damage),
                    ScalarField::ArmorPenetration => Slot::Float(&mut r.armor_penetration),
                    ScalarField::Range => Slot::Float(&mut r.range),
                    ScalarField::BurstCount => Slot::Count(&mut r.burst_count),
                    ScalarField::TicksBetweenBurstShots => {
                        Slot::Float(&mut r.ticks_between_burst_shots)
                    }
                    ScalarField::Warmup => Slot::Float(&mut r.warmup),
                    ScalarField::Cooldown => Slot::Float(&mut r.cooldown),
                    ScalarField::AccuracyTouch => Slot::Float(&mut r.accuracy.touch),
                    ScalarField::AccuracyShort => Slot::Float(&mut r.accuracy.short),
                    ScalarField::AccuracyMedium => Slot::Float(&mut r.accuracy.medium),
                    _ => Slot::Float(&mut r.accuracy.long),
                })
            }
            ScalarField::CeMagazineSize => Some(Slot::Count(&mut self.ce.as_mut()?.magazine_size)),
            ScalarField::CeReloadTime => Some(Slot::Float(&mut self.ce.as_mut()?.reload_time)),
            ScalarField::CeBulk => Some(Slot::Float(&mut self.ce.as_mut()?.bulk)),
            ScalarField::CeSwayFactor => Some(Slot::Float(&mut self.ce.as_mut()?.sway_factor)),
            ScalarField::CeShotSpread => Some(Slot::Float(&mut self.ce.as_mut()?.shot_spread)),
            ScalarField::CeSightsEfficiency => {
                Some(Slot::Float(&mut self.ce.as_mut()?.sights_efficiency))
            }
            ScalarField::CeRecoilAmount => Some(Slot::Float(&mut self.ce.as_mut()?.recoil_amount)),
            ScalarField::CeCooldown => Some(Slot::Float(&mut self.ce.as_mut()?.cooldown)),
            ScalarField::CeParryBonus => Some(Slot::Float(&mut self.ce.as_mut()?.parry_bonus)),
            ScalarField::CeMeleeCritChance => {
                Some(Slot::Float(&mut self.ce.as_mut()?.melee_crit_chance))
            }
            ScalarField::CeMeleeParryChance => {
                Some(Slot::Float(&mut self.ce.as_mut()?.melee_parry_chance))
            }
            ScalarField::CeMeleeDodgeChance => {
                Some(Slot::Float(&mut self.ce.as_mut()?.melee_dodge_chance))
            }
            ScalarField::CeToolSharp(i) => Some(Slot::Float(
                &mut self.ce.as_mut()?.tool_penetration.get_mut(i)?.sharp,
            )),
            ScalarField::CeToolBlunt(i) => Some(Slot::Float(
                &mut self.ce.as_mut()?.tool_penetration.get_mut(i)?.blunt,
            )),
        }
    }

    /// Offers a value from a suggestion, answer or anchor (or a typed edit) to a field.
    ///
    /// The replacement rule of [`ValueSource::may_replace`] decides: typed values are kept against every
    /// other source (IT-003). A value that is not finite, a negative or fractional-beyond-rounding count and
    /// a field that does not apply to this draft give [`OfferOutcome::NotApplicable`] and change nothing.
    /// This never creates the CE block.
    pub fn offer(&mut self, field: ScalarField, value: f64, source: ValueSource) -> OfferOutcome {
        if !value.is_finite() {
            return OfferOutcome::NotApplicable;
        }
        match self.slot_mut(field) {
            None => OfferOutcome::NotApplicable,
            Some(Slot::Float(slot)) => offer(slot, value, source),
            Some(Slot::Count(slot)) => match count_from(value) {
                Some(n) => offer(slot, n, source),
                None => OfferOutcome::NotApplicable,
            },
        }
    }

    /// Every scalar field that currently holds a value, with its JSON pointer, in a stable order.
    #[must_use]
    pub fn set_scalars(&self) -> Vec<(ScalarField, Sourced<f64>)> {
        let mut fields = vec![
            ScalarField::Mass,
            ScalarField::WorkToMake,
            ScalarField::MarketValue,
            ScalarField::StuffCount,
            ScalarField::Damage,
            ScalarField::ArmorPenetration,
            ScalarField::Range,
            ScalarField::BurstCount,
            ScalarField::TicksBetweenBurstShots,
            ScalarField::Warmup,
            ScalarField::Cooldown,
            ScalarField::AccuracyTouch,
            ScalarField::AccuracyShort,
            ScalarField::AccuracyMedium,
            ScalarField::AccuracyLong,
        ];
        for i in 0..self.tools.len() {
            fields.extend([
                ScalarField::ToolPower(i),
                ScalarField::ToolCooldown(i),
                ScalarField::ToolArmorPenetration(i),
                ScalarField::ToolChanceFactor(i),
            ]);
        }
        fields.extend([
            ScalarField::CeMagazineSize,
            ScalarField::CeReloadTime,
            ScalarField::CeBulk,
            ScalarField::CeSwayFactor,
            ScalarField::CeShotSpread,
            ScalarField::CeSightsEfficiency,
            ScalarField::CeRecoilAmount,
            ScalarField::CeCooldown,
            ScalarField::CeParryBonus,
            ScalarField::CeMeleeCritChance,
            ScalarField::CeMeleeParryChance,
            ScalarField::CeMeleeDodgeChance,
        ]);
        if let Some(ce) = &self.ce {
            for i in 0..ce.tool_penetration.len() {
                fields.extend([ScalarField::CeToolSharp(i), ScalarField::CeToolBlunt(i)]);
            }
        }
        fields
            .into_iter()
            .filter_map(|f| self.scalar(f).map(|v| (f, v)))
            .collect()
    }
}

/// Rounds a finite non negative value to a count.
fn count_from(value: f64) -> Option<u32> {
    let rounded = value.round();
    if rounded < 0.0 || rounded > f64::from(u32::MAX) {
        return None;
    }
    // The range check above makes the conversion exact.
    Some(rounded as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_specs_have_the_ce_patch_off() {
        let spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        assert!(spec.ce.is_none());
        assert!(!spec.ce_patch_enabled());
        assert!(spec.ranged.is_some());
        assert!(
            DesignSpec::new_melee("RS_TestBlade", "test blade")
                .ranged
                .is_none()
        );
    }

    #[test]
    fn json_shape_is_camel_case_with_kebab_enums() {
        let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        spec.tech_level = Some(TechLevel::Industrial);
        spec.offer(ScalarField::Damage, 12.0, ValueSource::Typed);
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["kind"], "ranged");
        assert_eq!(json["techLevel"], "industrial");
        assert_eq!(json["identity"]["defName"], "RS_TestRifle");
        assert_eq!(json["ranged"]["damage"]["source"], "typed");
        assert!(json.get("ce").is_none());
    }

    #[test]
    fn offer_never_creates_the_ce_block_or_a_ranged_block_on_melee() {
        let mut spec = DesignSpec::new_melee("RS_TestBlade", "test blade");
        assert_eq!(
            spec.offer(ScalarField::CeBulk, 2.0, ValueSource::Suggested),
            OfferOutcome::NotApplicable
        );
        assert_eq!(
            spec.offer(ScalarField::Damage, 2.0, ValueSource::Suggested),
            OfferOutcome::NotApplicable
        );
        assert!(spec.ce.is_none());
        assert!(spec.ranged.is_none());
    }

    #[test]
    fn counts_round_and_reject_negatives() {
        let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        assert!(
            spec.offer(ScalarField::BurstCount, 2.6, ValueSource::Suggested)
                .applied()
        );
        assert_eq!(
            spec.scalar(ScalarField::BurstCount),
            Some(Sourced::suggested(3.0))
        );
        assert_eq!(
            spec.offer(ScalarField::BurstCount, -1.0, ValueSource::Typed),
            OfferOutcome::NotApplicable
        );
        assert_eq!(
            spec.offer(ScalarField::Damage, f64::NAN, ValueSource::Typed),
            OfferOutcome::NotApplicable
        );
    }

    #[test]
    fn tool_fields_need_an_existing_tool() {
        let mut spec = DesignSpec::new_melee("RS_TestBlade", "test blade");
        assert_eq!(
            spec.offer(ScalarField::ToolPower(0), 9.0, ValueSource::Typed),
            OfferOutcome::NotApplicable
        );
        spec.tools.push(ToolSpec::new("blade", &["Cut"]));
        assert!(
            spec.offer(ScalarField::ToolPower(0), 9.0, ValueSource::Typed)
                .applied()
        );
        assert_eq!(spec.set_scalars().len(), 1);
    }
}
