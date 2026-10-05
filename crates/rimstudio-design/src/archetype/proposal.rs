//! The output of the solver: proposed numbers, each with its reason, and the strength report.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fit::FitReport;
use crate::model::{ArchetypeMode, ItemKind, ScalarField, TechLevel};

/// The source chip of every proposed number. The values are stored in a draft as suggested.
pub const SOURCE_CHIP: &str = "archetype";

/// One multiplier of a derivation, so the UI can show how a number came about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactorTerm {
    /// What the factor stands for (`archetype`, `action`, `calibre`, `handling`, `rate of fire`,
    /// `strength scale`).
    pub label: String,
    /// The multiplier.
    pub factor: f64,
}

impl FactorTerm {
    /// A term.
    #[must_use]
    pub fn new(label: impl Into<String>, factor: f64) -> Self {
        Self {
            label: label.into(),
            factor,
        }
    }
}

/// A proposed number of a field with its reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedValue {
    /// The field of the spec the number goes to.
    pub field: ScalarField,
    /// The reference stat the number is compared with (`range`, `swing_damage`, ...).
    pub stat: String,
    /// The proposed value, rounded the way the install writes such numbers.
    pub value: f64,
    /// False when the field is better left unset: the game derives it (armor penetration that equals the
    /// implied value, ticks between shots of a single shot weapon).
    pub write: bool,
    /// A short plain reason.
    pub reason: String,
    /// The install median the shape started from, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median: Option<f64>,
    /// How many reference weapons stand behind the median.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_n: Option<usize>,
    /// The multipliers applied to the median.
    pub terms: Vec<FactorTerm>,
}

/// A proposed tool of a weapon (a melee attack, or the stock bash of a gun).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedTool {
    /// The label.
    pub label: String,
    /// The capacity def names.
    pub capacities: Vec<String>,
    /// The tool power.
    pub power: ProposedValue,
    /// The cooldown in seconds.
    pub cooldown: ProposedValue,
    /// The explicit armor penetration, when the archetype asks for more than the implied one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration: Option<ProposedValue>,
}

/// One line of a proposed cost list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedCost {
    /// The ingredient def name.
    pub def_name: String,
    /// How many.
    pub count: f64,
    /// A short plain reason.
    pub reason: String,
}

/// The stuff part of a proposed melee weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedStuff {
    /// The stuff category def names.
    pub categories: Vec<String>,
    /// The stuff units per item.
    pub count: f64,
    /// A short plain reason.
    pub reason: String,
}

/// The descriptors as the solver read them, with the defaults filled in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedChoice {
    /// The action id (guns only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The rate of fire class nearest to the choice (a numeric rate maps to its class).
    pub rof: String,
    /// The cadence multiplier the solver used.
    pub rate: f64,
    /// The calibre class id (guns only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibre: Option<String>,
    /// The Combat Extended ammo set, when the calibre came from one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// The handling id.
    pub handling: String,
    /// The tier.
    pub tier: TechLevel,
}

/// How the strength of the proposal was aimed and where it landed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrengthReport {
    /// The percentile of the class the balance target stands for.
    pub percentile: f64,
    /// The strength index the proposal aimed at.
    pub target: f64,
    /// The strength index of the rounded proposal.
    pub achieved: f64,
    /// The relative difference of the two.
    pub error: f64,
    /// The scale the solver applied to damage and tempo (1 means the shape already fitted).
    pub scale: f64,
    /// True when the target could not be reached inside the scale limits.
    pub clamped: bool,
    /// The class the target was read from, for example "based on 5 items of role rifle at Industrial".
    pub class_label: String,
    /// How many reference weapons stand behind the class.
    pub class_n: usize,
    /// The percentile of the achieved strength among the class (0 to 1).
    pub achieved_percentile: f64,
}

/// A proposal: every number of the weapon the archetype describes, with reasons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    /// The archetype id (`rifle/assault`).
    pub archetype: String,
    /// The archetype label.
    pub label: String,
    /// The kind of weapon.
    pub kind: ItemKind,
    /// The mode the proposal was made for.
    pub mode: ArchetypeMode,
    /// The source chip of every number.
    pub source: String,
    /// The descriptors with defaults.
    pub choice: ResolvedChoice,
    /// The pool role the proposal is compared with, when the install has one for the archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The scalar fields in display order.
    pub values: Vec<ProposedValue>,
    /// The proposal as reference stats (`damage`, `range`, `swing_damage`, `dps`, ...): what the fit meter
    /// and the validation harness compare with the install's weapons.
    pub stats: BTreeMap<String, f64>,
    /// The tools: the attacks of a melee weapon, or the bash tools of a gun.
    pub tools: Vec<ProposedTool>,
    /// The cost list.
    pub cost_list: Vec<ProposedCost>,
    /// The stuff of a melee weapon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stuff: Option<ProposedStuff>,
    /// The weapon tags.
    pub weapon_tags: Vec<String>,
    /// The weapon classes.
    pub weapon_classes: Vec<String>,
    /// The market value the price math gives for the proposed cost list and work (informational).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_value: Option<f64>,
    /// The forced miss radius, when the archetype has one (none of the built in archetypes does).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_miss_radius: Option<f64>,
    /// The strength report.
    pub strength: StrengthReport,
    /// Remarks: fallbacks, widened classes, missing data.
    pub notes: Vec<String>,
    /// The Combat Extended part, in Combat Extended mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ce: Option<CeProposal>,
    /// The fit meter's verdict on the rounded proposal, when it was checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit: Option<FitReport>,
}

/// The Combat Extended part of a proposal: the calibre and the hints for the block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeProposal {
    /// The ammo set def name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// The caliber text of the set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caliber: Option<String>,
    /// The projectile of the first ammo type of the set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_projectile: Option<String>,
    /// The AI class tag the archetype maps to in the install, when one matches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_tag_class: Option<String>,
    /// How the damage of the set compares with the median set of its family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_ratio: Option<f64>,
    /// Remarks.
    pub notes: Vec<String>,
}

impl Proposal {
    /// The value proposed for a field.
    #[must_use]
    pub fn value(&self, field: ScalarField) -> Option<f64> {
        self.values
            .iter()
            .find(|v| v.field == field)
            .map(|v| v.value)
    }

    /// The value proposed for a reference stat.
    #[must_use]
    pub fn stat(&self, stat: &str) -> Option<f64> {
        self.values.iter().find(|v| v.stat == stat).map(|v| v.value)
    }
}
