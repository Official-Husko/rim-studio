//! Suggestions for the optional Combat Extended block of a design.
//!
//! The vanilla design is the twin. [`suggest_block`] reads it, asks the conversion estimator of
//! [`crate::ce::classes`] what the user's own conversions say about every field of the Combat Extended
//! block, and reports per field the suggested number, where it comes from (the vanilla number itself, a
//! predictor over `n` conversions, or something the user already decided), the error band, the reliability
//! rating of the stat and whether the number is derived or must be asked. Next to the numbers it lists the
//! choices (ammo set, default projectile, weapon tag class, one handed, belt fed) with candidates ranked by
//! how the user's conversions of similar weapons were set up, the questions that remain open
//! ([`AskList`]) and the required fields that are still missing.
//!
//! Rules this module keeps (owner rules and IT-003):
//!
//! - A suggestion is a report. It never turns the Combat Extended toggle on: a spec without a block
//!   (`spec.ce` is `None`) gets suggestions, and [`accept_suggestions`] leaves such a spec unchanged.
//! - A value the user decided (typed, answered or taken from an anchor) is never overwritten. Only empty
//!   fields and fields that hold an earlier suggestion are filled.
//! - Only numbers rated reliable or rough are ever derived. A stat rated unreliable or unmeasured is an
//!   ask that carries the reason and the rejected estimate for reference.
//! - The ammo set and the weapon tag class are never chosen for the user: they stay asks. The default
//!   projectile follows the ammo set once the user has chosen it.
//! - Pure and deterministic: no clock, no randomness, and no number of the game or of Combat Extended is a
//!   constant of this module. Candidate ordering uses the tags and numbers of the model only.

use std::collections::BTreeSet;

use rimstudio_core::diag::Diagnostic;
use serde::{Deserialize, Serialize};

use super::classes::{ConversionPrediction, ConversionPredictor, Reliability};
use super::lint::codes::DERIVED_VALUE;
use super::patchgen::values::{predict_for, predict_tool_ratios, tidy};
use super::patchgen::{AskItem, AskKind, AskList, ConvertAnswers, derive_ce_block};
use super::reader::{CeGun, CeModel};
use crate::baseline::Band;
use crate::model::{
    CePatchSpec, CeToolPenetration, DesignSpec, ItemKind, Sourced, ToolSpec, ValueSource,
};

/// The most candidates listed for a choice (ammo sets and tag classes of an install can be many dozens).
pub const MAX_CANDIDATES: usize = 12;

/// Where the value shown for a field comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum SuggestionSource {
    /// The user typed the number.
    Typed,
    /// The number was copied from an anchor item.
    Anchor,
    /// The number follows from a quiz answer.
    Answered,
    /// The converted number equals the vanilla number (a stat that conversion leaves alone).
    Identity {
        /// Converted weapons behind the finding that it stays.
        n: usize,
    },
    /// A predictor over the user's conversions.
    Predicted {
        /// The predictor form.
        predictor: ConversionPredictor,
        /// Converted weapons behind the estimate.
        n: usize,
    },
    /// The vanilla number, carried over because the estimate was too poor to use.
    Vanilla,
    /// The first member of the chosen ammo set.
    FirstOfSet,
}

impl SuggestionSource {
    /// A short word for readouts.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Typed => "typed",
            Self::Anchor => "anchor",
            Self::Answered => "answered",
            Self::Identity { .. } => "identity",
            Self::Predicted { .. } => "predicted",
            Self::Vanilla => "vanilla",
            Self::FirstOfSet => "first of the ammo set",
        }
    }

    fn of_value_source(source: ValueSource) -> Self {
        match source {
            ValueSource::Typed => Self::Typed,
            ValueSource::Anchor => Self::Anchor,
            ValueSource::Answered => Self::Answered,
            ValueSource::Suggested => Self::Vanilla,
        }
    }
}

/// What the suggestion engine did for a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldStatus {
    /// The draft holds a value the user decided; it is kept.
    Held,
    /// A usable suggestion exists and may be accepted.
    Derived,
    /// No usable suggestion: the user must answer.
    Ask,
}

/// A closed interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// The lower end.
    pub low: f64,
    /// The upper end.
    pub high: f64,
}

/// The error band around a suggestion: where half and four fifths of the held out conversions fell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SuggestionBand {
    /// The interval that held half of the held out conversions.
    pub p50: Interval,
    /// The interval that held four fifths of them.
    pub p80: Interval,
}

impl SuggestionBand {
    fn of(band: &Band, value: f64) -> Self {
        let (a, b) = band.interval50(value);
        let (c, d) = band.interval80(value);
        Self {
            p50: Interval { low: a, high: b },
            p80: Interval { low: c, high: d },
        }
    }
}

/// One number of the Combat Extended block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedField {
    /// The JSON pointer into the spec, for example `/ce/bulk` or `/ce/toolPenetration/blade/blunt`.
    pub field: String,
    /// A short English label.
    pub label: String,
    /// True when a plan that accepts suggestions needs the field.
    pub required: bool,
    /// What the engine did.
    pub status: FieldStatus,
    /// The value the draft already holds, when the user decided it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<f64>,
    /// Where the held value came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_source: Option<SuggestionSource>,
    /// The suggested number: present when the rating allows writing it (also shown beside a held value,
    /// for comparison).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Where the suggested number comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SuggestionSource>,
    /// The error band of the suggested number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<SuggestionBand>,
    /// The rating of the stat, when the estimator measured one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<Reliability>,
    /// The typical relative error of the estimate (leave one out), when it was measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<f64>,
    /// An estimate that was rejected (rated unreliable or unmeasured), for reference only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<f64>,
    /// Why there is no usable suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One candidate of a choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The def name or tag.
    pub name: String,
    /// Converted guns of the library that use it.
    pub used_by: usize,
    /// How close the guns that use it are to the design (the sum of their tag similarities).
    pub score: f64,
    /// The damage of the first projectile of an ammo set, when the model has it (information for the
    /// caliber choice, never a requirement).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_damage: Option<f64>,
}

/// One choice of the block: a def, a tag or a flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceField {
    /// The JSON pointer into the spec, for example `/ce/ammoSet`.
    pub field: String,
    /// A short English label.
    pub label: String,
    /// The kind of answer.
    pub kind: AskKind,
    /// True when a plan that accepts suggestions needs an answer.
    pub required: bool,
    /// What the engine did.
    pub status: FieldStatus,
    /// What the draft holds, as text (`true` and `false` for flags).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
    /// A derived value (the default projectile follows the ammo set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The source of a derived value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SuggestionSource>,
    /// The candidates, best first, at most [`MAX_CANDIDATES`].
    pub candidates: Vec<Candidate>,
    /// Why the choice is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The suggestion for the Combat Extended block of one design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeSuggestion {
    /// The kind of item.
    pub kind: ItemKind,
    /// The def name of the design.
    pub def_name: String,
    /// False when there is nothing to suggest from; `reason` says why.
    pub available: bool,
    /// The plain reason when `available` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// True when the draft's Combat Extended toggle is on. The suggestion never changes it.
    pub toggle_on: bool,
    /// How the class behind the estimates was formed, in plain words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_label: Option<String>,
    /// Converted weapons of the kind in the library.
    pub pool: usize,
    /// The numbers of the block, in a fixed order.
    pub fields: Vec<SuggestedField>,
    /// The choices of the block, in a fixed order.
    pub choices: Vec<ChoiceField>,
    /// Numbers the patch writes from its own estimate although the block has no field for them (mass,
    /// range, warmup, cooldown): shown for transparency, never editable here.
    pub patch_numbers: Vec<SuggestedField>,
    /// The open questions: choices and numbers that have no usable suggestion and no held value.
    pub asks: AskList,
    /// The required fields the draft does not hold today.
    pub missing: Vec<String>,
    /// The required fields that stay open even after every derived suggestion is accepted.
    pub still_missing_after_accept: Vec<String>,
    /// Plain words about the estimates (widened classes, missing twins).
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------------------------------------
// the table of fields

/// A numeric field of the block.
struct NumField {
    pointer: &'static str,
    label: &'static str,
    stat: &'static str,
    required: bool,
}

const RANGED_FIELDS: [NumField; 7] = [
    NumField {
        pointer: "/ce/bulk",
        label: "CE bulk",
        stat: "bulk",
        required: true,
    },
    NumField {
        pointer: "/ce/swayFactor",
        label: "CE sway factor",
        stat: "sway",
        required: true,
    },
    NumField {
        pointer: "/ce/shotSpread",
        label: "CE shot spread",
        stat: "spread",
        required: true,
    },
    NumField {
        pointer: "/ce/sightsEfficiency",
        label: "CE sights efficiency",
        stat: "sights",
        required: false,
    },
    NumField {
        pointer: "/ce/recoilAmount",
        label: "CE recoil",
        stat: "recoil",
        required: false,
    },
    NumField {
        pointer: "/ce/magazineSize",
        label: "CE magazine size",
        stat: "magazine",
        required: true,
    },
    NumField {
        pointer: "/ce/reloadTime",
        label: "CE reload time",
        stat: "reload",
        required: true,
    },
];

const MELEE_FIELDS: [NumField; 5] = [
    NumField {
        pointer: "/ce/bulk",
        label: "CE bulk",
        stat: "bulk",
        required: true,
    },
    NumField {
        pointer: "/ce/parryBonus",
        label: "CE counter parry bonus",
        stat: "counter_parry",
        required: false,
    },
    NumField {
        pointer: "/ce/meleeCritChance",
        label: "CE melee crit chance",
        stat: "crit",
        required: true,
    },
    NumField {
        pointer: "/ce/meleeParryChance",
        label: "CE melee parry chance",
        stat: "parry",
        required: true,
    },
    NumField {
        pointer: "/ce/meleeDodgeChance",
        label: "CE melee dodge chance",
        stat: "dodge",
        required: true,
    },
];

/// The value the block holds for a numeric pointer.
fn block_number(ce: &CePatchSpec, pointer: &str) -> Option<Sourced<f64>> {
    match pointer {
        "/ce/bulk" => ce.bulk,
        "/ce/swayFactor" => ce.sway_factor,
        "/ce/shotSpread" => ce.shot_spread,
        "/ce/sightsEfficiency" => ce.sights_efficiency,
        "/ce/recoilAmount" => ce.recoil_amount,
        "/ce/reloadTime" => ce.reload_time,
        "/ce/parryBonus" => ce.parry_bonus,
        "/ce/meleeCritChance" => ce.melee_crit_chance,
        "/ce/meleeParryChance" => ce.melee_parry_chance,
        "/ce/meleeDodgeChance" => ce.melee_dodge_chance,
        "/ce/magazineSize" => ce
            .magazine_size
            .map(|m| Sourced::new(f64::from(m.value), m.source)),
        _ => None,
    }
}

/// Writes a suggested number into the block at a numeric pointer. Returns false for an unknown pointer or a
/// value that is not a count where a count is needed.
fn set_block_number(ce: &mut CePatchSpec, pointer: &str, value: f64) -> bool {
    let s = Some(Sourced::new(value, ValueSource::Suggested));
    match pointer {
        "/ce/bulk" => ce.bulk = s,
        "/ce/swayFactor" => ce.sway_factor = s,
        "/ce/shotSpread" => ce.shot_spread = s,
        "/ce/sightsEfficiency" => ce.sights_efficiency = s,
        "/ce/recoilAmount" => ce.recoil_amount = s,
        "/ce/reloadTime" => ce.reload_time = s,
        "/ce/parryBonus" => ce.parry_bonus = s,
        "/ce/meleeCritChance" => ce.melee_crit_chance = s,
        "/ce/meleeParryChance" => ce.melee_parry_chance = s,
        "/ce/meleeDodgeChance" => ce.melee_dodge_chance = s,
        "/ce/magazineSize" => {
            let r = value.round();
            if !(r >= 1.0 && r <= f64::from(u32::MAX)) {
                return false;
            }
            ce.magazine_size = Some(Sourced::new(r as u32, ValueSource::Suggested));
        }
        _ => return false,
    }
    true
}

/// True when the source marks a value the user stands behind: only a suggestion may be replaced.
fn is_decided(source: ValueSource) -> bool {
    source != ValueSource::Suggested
}

fn nonempty(text: &Option<String>) -> Option<&str> {
    text.as_deref().filter(|t| !t.is_empty())
}

/// The pointer of a tool penetration entry.
fn tool_pointer(label: &str, which: &str) -> String {
    format!("/ce/toolPenetration/{label}/{which}")
}

fn tool_held(ce: &CePatchSpec, label: &str, blunt: bool) -> Option<Sourced<f64>> {
    let entry = ce.tool_penetration.iter().find(|p| p.tool == label)?;
    if blunt { entry.blunt } else { entry.sharp }
}

fn needs_sharp(tool: &ToolSpec) -> bool {
    tool.capacities
        .iter()
        .any(|c| matches!(c.as_str(), "Cut" | "Stab"))
}

// ---------------------------------------------------------------------------------------------------------
// suggesting

/// What the estimator knows about one stat, used to fill the fields.
struct StatView<'a> {
    prediction: Option<&'a ConversionPrediction>,
}

impl StatView<'_> {
    fn get(&self, stat: &str) -> Option<&super::classes::StatPrediction> {
        self.prediction?.stats.get(stat)
    }

    /// The reason a stat has no usable number, in plain words, and the rejected estimate.
    fn rejection(&self, stat: &str) -> (Option<f64>, Option<String>) {
        let Some(prediction) = self.prediction else {
            return (
                None,
                Some("the library has no converted weapons of this kind to estimate from".into()),
            );
        };
        match prediction.stats.get(stat) {
            Some(p) if p.value.is_finite() && !p.is_usable() => (
                Some(tidy(p.value)),
                Some(format!(
                    "the estimate {} is not used: it is {}",
                    tidy(p.value),
                    p.accuracy_note()
                )),
            ),
            Some(_) => (None, None),
            None => (
                None,
                Some(match prediction.withheld.get(stat) {
                    Some(why) => format!("no estimate: {why}"),
                    None => "no estimate: no converted weapon in the library carries it".into(),
                }),
            ),
        }
    }
}

fn source_of(p: &super::classes::StatPrediction) -> SuggestionSource {
    if p.predictor == ConversionPredictor::Identity {
        SuggestionSource::Identity { n: p.n }
    } else {
        SuggestionSource::Predicted {
            predictor: p.predictor,
            n: p.n,
        }
    }
}

/// Fills the estimate part of a field (`value`, `source`, `band`, `rating`, `error`, `reference`,
/// `reason`) from the stat prediction. `value` is the number the generator would write for the stat (the
/// same one the conversion flow derives), `None` when it would ask.
fn fill_estimate(
    field: &mut SuggestedField,
    view: &StatView<'_>,
    stat: &str,
    value: Option<f64>,
    ask: Option<&AskItem>,
) {
    let p = view.get(stat);
    field.rating = p.map(|p| p.reliability);
    field.error = p.and_then(|p| p.loo_error);
    match (value, p) {
        (Some(v), Some(p)) => {
            field.value = Some(v);
            field.source = Some(source_of(p));
            field.band = Some(SuggestionBand::of(&p.band, v));
        }
        (Some(v), None) => {
            field.value = Some(v);
        }
        (None, _) => {
            let (reference, reason) = view.rejection(stat);
            field.reference = ask.and_then(|a| a.suggestion).or(reference);
            field.reason = ask.and_then(|a| a.reason.clone()).or(reason);
        }
    }
}

fn base_field(pointer: String, label: String, required: bool) -> SuggestedField {
    SuggestedField {
        field: pointer,
        label,
        required,
        status: FieldStatus::Ask,
        held: None,
        held_source: None,
        value: None,
        source: None,
        band: None,
        rating: None,
        error: None,
        reference: None,
        reason: None,
    }
}

/// Settles the status of a field from what is held and what is suggested.
fn settle(field: &mut SuggestedField, held: Option<Sourced<f64>>) {
    match held.filter(|h| is_decided(h.source)) {
        Some(h) => {
            field.status = FieldStatus::Held;
            field.held = Some(h.value);
            field.held_source = Some(SuggestionSource::of_value_source(h.source));
            // a held number needs no reason
            field.reason = None;
        }
        None if field.value.is_some() => field.status = FieldStatus::Derived,
        None => field.status = FieldStatus::Ask,
    }
}

fn similarity(design: &BTreeSet<&str>, gun_tags: &[String]) -> f64 {
    let other: BTreeSet<&str> = gun_tags.iter().map(String::as_str).collect();
    let union = design.union(&other).count();
    if union == 0 {
        return 0.0;
    }
    design.intersection(&other).count() as f64 / union as f64
}

fn gun_tags(gun: &CeGun, prefix: &str) -> Vec<String> {
    if gun.twin.is_some() {
        gun.twin_tags.clone()
    } else {
        gun.weapon_tags
            .iter()
            .filter(|t| prefix.is_empty() || !t.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// The candidates for ammo sets and tag classes: every def of the model, ordered by how the design's
/// nearest conversions are set up. The order is: score (sum of tag similarity of the guns that use it),
/// then, for ammo sets, how close the first projectile's damage is to the design's damage, then the number
/// of guns, then the name.
fn rank_candidates(spec: &DesignSpec, model: &CeModel) -> (Vec<Candidate>, Vec<Candidate>) {
    let design_tags: BTreeSet<&str> = spec.weapon_tags.iter().map(String::as_str).collect();
    let prefix = model.classes.tag_prefix.as_str();
    let guns: Vec<(&CeGun, f64)> = model
        .guns
        .iter()
        .filter(|g| g.excluded.is_none())
        .map(|g| (g, similarity(&design_tags, &gun_tags(g, prefix))))
        .collect();
    let design_damage = spec
        .ranged
        .as_ref()
        .and_then(|r| r.damage)
        .map(|d| d.value)
        .filter(|d| d.is_finite() && *d > 0.0);
    let mut sets: Vec<(Candidate, f64)> = model
        .ammo_sets
        .iter()
        .map(|set| {
            let users: Vec<&(&CeGun, f64)> = guns
                .iter()
                .filter(|(g, _)| g.ammo_set.as_deref() == Some(set.def_name.as_str()))
                .collect();
            let first_damage = set
                .first()
                .and_then(|a| a.info.damage)
                .filter(|d| d.is_finite() && *d > 0.0);
            let closeness = match (design_damage, first_damage) {
                (Some(want), Some(have)) => (have / want).ln().abs(),
                _ => f64::INFINITY,
            };
            (
                Candidate {
                    name: set.def_name.clone(),
                    used_by: users.len(),
                    score: users.iter().fold(0.0, |acc, (_, s)| acc + *s),
                    first_damage,
                },
                closeness,
            )
        })
        .collect();
    sets.sort_by(|a, b| {
        b.0.score
            .total_cmp(&a.0.score)
            .then_with(|| a.1.total_cmp(&b.1))
            .then_with(|| b.0.used_by.cmp(&a.0.used_by))
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
    let mut classes: Vec<Candidate> = model
        .ai_class_tags
        .iter()
        .map(|tag| {
            let users: Vec<&(&CeGun, f64)> = guns
                .iter()
                .filter(|(g, _)| g.ai_class.as_deref() == Some(tag.as_str()))
                .collect();
            Candidate {
                name: tag.clone(),
                used_by: users.len(),
                score: users.iter().fold(0.0, |acc, (_, s)| acc + *s),
                first_damage: None,
            }
        })
        .collect();
    classes.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.used_by.cmp(&a.used_by))
            .then_with(|| a.name.cmp(&b.name))
    });
    let take = |v: Vec<Candidate>| v.into_iter().take(MAX_CANDIDATES).collect();
    (
        take(sets.into_iter().map(|(c, _)| c).collect()),
        take(classes),
    )
}

fn choice(field: &str, label: &str, kind: AskKind, required: bool) -> ChoiceField {
    ChoiceField {
        field: field.to_owned(),
        label: label.to_owned(),
        kind,
        required,
        status: FieldStatus::Ask,
        held: None,
        value: None,
        source: None,
        candidates: Vec::new(),
        reason: None,
    }
}

fn choices_of(spec: &DesignSpec, model: &CeModel, held: &CePatchSpec) -> Vec<ChoiceField> {
    if spec.kind != ItemKind::Ranged {
        return Vec::new();
    }
    let toggle = spec.ce.is_some();
    let (sets, classes) = rank_candidates(spec, model);
    let mut out = Vec::new();

    let mut ammo = choice(
        "/ce/ammoSet",
        "Which caliber (ammo set) does the weapon use?",
        AskKind::Choice,
        true,
    );
    ammo.candidates = sets;
    match nonempty(&held.ammo_set) {
        Some(set) => {
            ammo.status = FieldStatus::Held;
            ammo.held = Some(set.to_owned());
        }
        None => {
            ammo.reason = Some(
                "the caliber is never chosen for you: pick one of the candidates, best fit first"
                    .into(),
            );
        }
    }
    let projectile_of_set = nonempty(&held.ammo_set)
        .and_then(|s| model.ammo_set(s))
        .map(|set| {
            (
                set.ammo_types
                    .iter()
                    .map(|a| a.projectile.clone())
                    .collect::<Vec<_>>(),
                set.first().map(|a| a.projectile.clone()),
            )
        });
    out.push(ammo);

    let mut projectile = choice(
        "/ce/defaultProjectile",
        "Which projectile of the ammo set is the default?",
        AskKind::Choice,
        true,
    );
    if let Some((members, _)) = &projectile_of_set {
        projectile.candidates = members
            .iter()
            .map(|name| Candidate {
                name: name.clone(),
                used_by: 0,
                score: 0.0,
                first_damage: None,
            })
            .collect();
    }
    match (nonempty(&held.default_projectile), &projectile_of_set) {
        (Some(p), _) => {
            projectile.status = FieldStatus::Held;
            projectile.held = Some(p.to_owned());
        }
        (None, Some((_, Some(first)))) => {
            projectile.status = FieldStatus::Derived;
            projectile.value = Some(first.clone());
            projectile.source = Some(SuggestionSource::FirstOfSet);
        }
        (None, Some((_, None))) => {
            projectile.reason = Some("the ammo set has no members in the loaded data".into());
        }
        (None, None) => {
            projectile.reason = Some("it follows the ammo set: choose the caliber first".into());
        }
    }
    out.push(projectile);

    let mut class = choice(
        "/ce/weaponTagClass",
        "Which weapon class tag does the weapon get?",
        AskKind::Choice,
        true,
    );
    class.candidates = classes;
    match nonempty(&held.weapon_tag_class) {
        Some(tag) => {
            class.status = FieldStatus::Held;
            class.held = Some(tag.to_owned());
        }
        None => {
            class.reason = Some(
                "the class tag is never chosen for you; answering it also sharpens the estimates of the numbers below"
                    .into(),
            );
        }
    }
    out.push(class);

    for (field, label, on) in [
        (
            "/ce/oneHanded",
            "Is the weapon held in one hand?",
            held.one_handed,
        ),
        ("/ce/beltFed", "Is the weapon belt fed?", held.belt_fed),
    ] {
        let mut flag = choice(field, label, AskKind::Flag, false);
        if toggle {
            flag.status = FieldStatus::Held;
            flag.held = Some(on.to_string());
        } else {
            flag.reason = Some("a flag of the block; it is false until you set it".into());
        }
        out.push(flag);
    }
    out
}

/// The numbers the patch derives although the block has no field for them: the estimate when it is usable,
/// else the vanilla number of the design.
fn patch_numbers(spec: &DesignSpec, view: &StatView<'_>) -> Vec<SuggestedField> {
    let mut stats: Vec<(&str, &str, Option<f64>)> =
        vec![("mass", "Mass in the patch", spec.mass.map(|m| m.value))];
    if spec.kind == ItemKind::Ranged {
        let r = spec.ranged.as_ref();
        stats.push((
            "range",
            "Range in the patch",
            r.and_then(|r| r.range).map(|v| v.value),
        ));
        stats.push((
            "warmup",
            "Warmup in the patch",
            r.and_then(|r| r.warmup).map(|v| v.value),
        ));
        stats.push((
            "cooldown",
            "Cooldown in the patch",
            r.and_then(|r| r.cooldown).map(|v| v.value),
        ));
    }
    let mut out = Vec::new();
    for (stat, label, vanilla) in stats {
        let mut field = base_field(stat.to_owned(), label.to_owned(), false);
        let p = view.get(stat);
        field.rating = p.map(|p| p.reliability);
        field.error = p.and_then(|p| p.loo_error);
        match p.filter(|p| p.is_usable() && p.value > 0.0) {
            Some(p) => {
                let v = tidy(p.value);
                field.value = Some(v);
                field.source = Some(source_of(p));
                field.band = Some(SuggestionBand::of(&p.band, v));
                field.status = FieldStatus::Derived;
            }
            None => {
                if let Some(v) = vanilla.filter(|v| v.is_finite()) {
                    field.value = Some(v);
                    field.source = Some(SuggestionSource::Vanilla);
                    field.status = FieldStatus::Derived;
                    let (reference, reason) = view.rejection(stat);
                    field.reference = reference;
                    field.reason =
                        reason.map(|r| format!("the vanilla number is carried over: {r}"));
                }
            }
        }
        if field.value.is_some() {
            out.push(field);
        }
    }
    out
}

/// A suggestion for a design that cannot be served, with the plain reason.
fn unavailable(spec: &DesignSpec, reason: String) -> CeSuggestion {
    CeSuggestion {
        kind: spec.kind,
        def_name: spec.identity.def_name.clone(),
        available: false,
        reason: Some(reason),
        toggle_on: spec.ce.is_some(),
        class_label: None,
        pool: 0,
        fields: Vec::new(),
        choices: Vec::new(),
        patch_numbers: Vec::new(),
        asks: AskList::default(),
        missing: Vec::new(),
        still_missing_after_accept: Vec::new(),
        notes: Vec::new(),
    }
}

/// Suggests the Combat Extended block of a design: the vanilla design is the twin.
///
/// Works with and without a block in the spec (`spec.ce`): without it the suggestion describes what turning
/// the toggle on would offer and nothing changes. Never panics; without Combat Extended data the result is
/// unavailable with the plain reason.
#[must_use]
pub fn suggest_block(spec: &DesignSpec, model: &CeModel) -> CeSuggestion {
    if !model.is_present() {
        let reason = model
            .absent
            .clone()
            .unwrap_or_else(|| "Combat Extended is not available".to_owned());
        return unavailable(spec, format!("no Combat Extended suggestions: {reason}"));
    }
    let toggle = spec.ce.is_some();
    let held = spec.ce.clone().unwrap_or_default();
    let answers = ConvertAnswers {
        ammo_set: nonempty(&held.ammo_set).map(str::to_owned),
        default_projectile: nonempty(&held.default_projectile).map(str::to_owned),
        weapon_tag_class: nonempty(&held.weapon_tag_class).map(str::to_owned),
        one_handed: Some(held.one_handed),
        belt_fed: Some(held.belt_fed),
        ..ConvertAnswers::default()
    };
    // The conversion flow derives the block from the vanilla twin; the held numbers are not passed on, so
    // that an estimate exists beside a held value.
    let (block, derive_asks, _derived) = derive_ce_block(spec, model, &answers);
    let mut probe = spec.clone();
    probe.ce = Some(CePatchSpec {
        weapon_tag_class: answers.weapon_tag_class.clone(),
        ..CePatchSpec::default()
    });
    let prediction = predict_for(&probe, model);
    let ratios = match spec.kind {
        ItemKind::Ranged => predict_tool_ratios(spec, model),
        ItemKind::Melee => prediction.clone(),
    };
    let numbers = StatView {
        prediction: prediction.as_ref(),
    };
    let tool_ratios = StatView {
        prediction: ratios.as_ref(),
    };
    let ask_of = |pointer: &str| derive_asks.items.iter().find(|a| a.field == pointer);

    let table: &[NumField] = match spec.kind {
        ItemKind::Ranged => &RANGED_FIELDS,
        ItemKind::Melee => &MELEE_FIELDS,
    };
    let mut fields = Vec::new();
    for nf in table {
        let mut field = base_field(nf.pointer.to_owned(), nf.label.to_owned(), nf.required);
        let value = block_number(&block, nf.pointer).map(|s| s.value);
        fill_estimate(&mut field, &numbers, nf.stat, value, ask_of(nf.pointer));
        settle(
            &mut field,
            block_number(&held, nf.pointer).filter(|_| toggle),
        );
        fields.push(field);
    }
    for tool in &spec.tools {
        let derived_entry = block.tool_penetration.iter().find(|p| p.tool == tool.label);
        let mut parts = vec![("blunt", "armorPenetrationBlunt", "ap_blunt_ratio", true)];
        if needs_sharp(tool) {
            parts.push(("sharp", "armorPenetrationSharp", "ap_sharp_ratio", false));
        }
        for (which, _name, stat, is_blunt) in parts {
            let pointer = tool_pointer(&tool.label, which);
            let required = is_blunt || spec.kind == ItemKind::Melee;
            let mut field = base_field(
                pointer.clone(),
                format!(
                    "{} penetration of the tool {}",
                    if is_blunt { "Blunt" } else { "Sharp" },
                    tool.label
                ),
                required,
            );
            let value = derived_entry
                .and_then(|e| if is_blunt { e.blunt } else { e.sharp })
                .map(|s| s.value);
            fill_estimate(&mut field, &tool_ratios, stat, value, ask_of(&pointer));
            settle(
                &mut field,
                tool_held(&held, &tool.label, is_blunt).filter(|_| toggle),
            );
            fields.push(field);
        }
    }
    let choices = choices_of(
        spec,
        model,
        &if toggle {
            held.clone()
        } else {
            CePatchSpec::default()
        },
    );
    let pool = match spec.kind {
        ItemKind::Ranged => model.guns.iter().filter(|g| g.excluded.is_none()).count(),
        ItemKind::Melee => model.melee.iter().filter(|m| m.excluded.is_none()).count(),
    };
    let patch_numbers = patch_numbers(spec, &numbers);

    let mut asks = Vec::new();
    for c in &choices {
        match (c.status, c.field.as_str()) {
            (FieldStatus::Ask, "/ce/defaultProjectile") if c.candidates.is_empty() => {}
            (FieldStatus::Ask, _) => asks.push(AskItem {
                field: c.field.clone(),
                label: c.label.clone(),
                kind: c.kind,
                options: c.candidates.iter().map(|x| x.name.clone()).collect(),
                reason: c.reason.clone(),
                suggestion: None,
            }),
            _ => {}
        }
    }
    for f in &fields {
        if f.status == FieldStatus::Ask {
            asks.push(AskItem {
                field: f.field.clone(),
                label: f.label.clone(),
                kind: AskKind::Number,
                options: Vec::new(),
                reason: f.reason.clone(),
                suggestion: f.reference,
            });
        }
    }
    let mut missing = Vec::new();
    let mut still = Vec::new();
    for f in fields.iter().filter(|f| f.required) {
        if f.status != FieldStatus::Held {
            missing.push(f.field.clone());
        }
        if f.status == FieldStatus::Ask {
            still.push(f.field.clone());
        }
    }
    for c in choices.iter().filter(|c| c.required) {
        if c.status != FieldStatus::Held {
            missing.push(c.field.clone());
        }
        // a derived projectile needs the ammo set, which is never derived
        let settled_by_accept = c.status == FieldStatus::Derived;
        if c.status != FieldStatus::Held && !settled_by_accept {
            still.push(c.field.clone());
        }
    }
    let mut notes = prediction
        .as_ref()
        .map(|p| p.notes.clone())
        .unwrap_or_default();
    if let Some(p) = &prediction {
        for (stat, why) in &p.withheld {
            notes.push(format!("{stat}: {why}"));
        }
    }
    CeSuggestion {
        kind: spec.kind,
        def_name: spec.identity.def_name.clone(),
        available: true,
        reason: None,
        toggle_on: toggle,
        class_label: prediction.as_ref().map(|p| p.class_label.clone()),
        pool,
        fields,
        choices,
        patch_numbers,
        asks: AskList { items: asks },
        missing,
        still_missing_after_accept: still,
        notes,
    }
}

// ---------------------------------------------------------------------------------------------------------
// accepting

/// Which suggestions to accept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "mode", content = "fields")]
pub enum Accept {
    /// Every derived field: the numbers rated reliable or rough, and the default projectile of a chosen
    /// ammo set.
    All,
    /// Only the named fields (a pointer such as `/ce/bulk`, or a short name such as `bulk`).
    Fields(Vec<String>),
}

/// A field that was filled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedField {
    /// The pointer of the field.
    pub field: String,
    /// The value written, as text.
    pub value: String,
    /// Where it came from.
    pub source: SuggestionSource,
    /// The rating of the stat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<Reliability>,
    /// The typical relative error of the estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<f64>,
}

/// A field that was named or eligible but not filled, with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedField {
    /// The pointer of the field, or the name as given when it matches no field.
    pub field: String,
    /// Why it was left alone.
    pub reason: String,
}

/// The result of [`accept_suggestions`].
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptOutcome {
    /// The spec with the accepted fields filled. Equal to the input when nothing was accepted.
    pub spec: DesignSpec,
    /// The fields that were filled, in suggestion order.
    pub accepted: Vec<AcceptedField>,
    /// Named fields that were not filled (held, asked, unknown) with the reason. A field that `All` leaves
    /// alone because the user holds it is not listed.
    pub skipped: Vec<SkippedField>,
    /// One `ce.derived-value` info diagnostic per filled field.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lower case, without separators: `/ce/swayFactor`, `ce.swayFactor` and `sway-factor` compare equal to
/// `swayfactor` after the optional `ce` prefix.
fn normal(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '.' | '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// True when `name` as the user wrote it names the field with this pointer.
fn names_field(name: &str, pointer: &str) -> bool {
    let want = normal(name);
    let full = normal(pointer);
    let short = full.strip_prefix("ce").unwrap_or(&full).to_owned();
    want == full || want == short
}

fn how(source: &SuggestionSource, rating: Option<Reliability>, error: Option<f64>) -> String {
    let rating = rating.map_or(String::new(), |r| format!(", rated {}", r.label()));
    let error = error
        .map(|e| format!(", typical error about {:.0} percent", 100.0 * e))
        .unwrap_or_default();
    match source {
        SuggestionSource::Identity { n } => format!(
            "taken from the Combat Extended suggestion: the number of the vanilla twin, as in {n} converted weapons{rating}{error}"
        ),
        SuggestionSource::Predicted { predictor, n } => format!(
            "taken from the Combat Extended suggestion: predicted ({predictor:?}) from {n} of the converted weapons in the library{rating}{error}; check it"
        ),
        SuggestionSource::FirstOfSet => {
            "taken from the Combat Extended suggestion: the first projectile of the ammo set".into()
        }
        _ => "taken from the Combat Extended suggestion".into(),
    }
}

/// Fills the derived fields of `suggestion` into a copy of `spec` and reports what was done.
///
/// - A spec without a Combat Extended block is returned unchanged: accepting never turns the toggle on.
/// - A field the user decided (typed, answered, anchor) is never overwritten (IT-003); a field that holds an
///   earlier suggestion is replaced.
/// - A field that has no usable suggestion stays empty (it remains an ask).
/// - The ammo set and the weapon tag class are never filled.
#[must_use]
pub fn accept_suggestions(
    spec: &DesignSpec,
    suggestion: &CeSuggestion,
    which: &Accept,
) -> AcceptOutcome {
    let mut out = AcceptOutcome {
        spec: spec.clone(),
        accepted: Vec::new(),
        skipped: Vec::new(),
        diagnostics: Vec::new(),
    };
    let named: Vec<&String> = match which {
        Accept::All => Vec::new(),
        Accept::Fields(names) => names.iter().collect(),
    };
    let selected = |pointer: &str| match which {
        Accept::All => true,
        Accept::Fields(_) => named.iter().any(|n| names_field(n, pointer)),
    };
    if spec.ce.is_none() {
        if !named.is_empty() || matches!(which, Accept::All) {
            out.skipped.push(SkippedField {
                field: "/ce".into(),
                reason: "the Combat Extended patch is off for this design; suggestions are never written unless you turn it on".into(),
            });
        }
        return out;
    }
    if !suggestion.available {
        out.skipped.push(SkippedField {
            field: "/ce".into(),
            reason: suggestion
                .reason
                .clone()
                .unwrap_or_else(|| "no suggestions are available".into()),
        });
        return out;
    }
    let mut matched: BTreeSet<usize> = BTreeSet::new();
    let record = |out: &mut AcceptOutcome,
                  pointer: &str,
                  text: String,
                  source: SuggestionSource,
                  rating: Option<Reliability>,
                  error: Option<f64>| {
        out.diagnostics.push(DERIVED_VALUE.diagnostic(
            pointer,
            &[
                ("field", pointer),
                ("value", text.as_str()),
                ("how", &how(&source, rating, error)),
            ],
        ));
        out.accepted.push(AcceptedField {
            field: pointer.to_owned(),
            value: text,
            source,
            rating,
            error,
        });
    };
    let mut ce = spec.ce.clone().unwrap_or_default();
    for f in &suggestion.fields {
        if !selected(&f.field) {
            continue;
        }
        for (i, n) in named.iter().enumerate() {
            if names_field(n, &f.field) {
                matched.insert(i);
            }
        }
        match f.status {
            FieldStatus::Held => {
                if !named.is_empty() {
                    out.skipped.push(SkippedField {
                        field: f.field.clone(),
                        reason: "the draft holds a value you decided; it is never overwritten"
                            .into(),
                    });
                }
            }
            FieldStatus::Ask => {
                // a field that `All` cannot fill is an open question, reported by the asks; only a field the
                // caller named is reported as skipped
                if !named.is_empty() {
                    out.skipped.push(SkippedField {
                        field: f.field.clone(),
                        reason: f
                            .reason
                            .clone()
                            .unwrap_or_else(|| "there is no usable suggestion".into()),
                    });
                }
            }
            FieldStatus::Derived => {
                let (Some(value), Some(source)) = (f.value, f.source.clone()) else {
                    continue;
                };
                let done = if let Some(rest) = f.field.strip_prefix("/ce/toolPenetration/") {
                    set_tool(&mut ce, rest, value)
                } else {
                    set_block_number(&mut ce, &f.field, value)
                };
                if done {
                    let text = if f.field == "/ce/magazineSize" {
                        value.round().to_string()
                    } else {
                        value.to_string()
                    };
                    record(&mut out, &f.field, text, source, f.rating, f.error);
                }
            }
        }
    }
    for c in &suggestion.choices {
        if !selected(&c.field) {
            continue;
        }
        for (i, n) in named.iter().enumerate() {
            if names_field(n, &c.field) {
                matched.insert(i);
            }
        }
        if c.field == "/ce/defaultProjectile" && c.status == FieldStatus::Derived {
            if let Some(v) = c.value.clone() {
                ce.default_projectile = Some(v.clone());
                record(
                    &mut out,
                    &c.field,
                    v,
                    SuggestionSource::FirstOfSet,
                    None,
                    None,
                );
            }
        } else if !named.is_empty() && c.status != FieldStatus::Held {
            out.skipped.push(SkippedField {
                field: c.field.clone(),
                reason: "this is a choice that is never made for you; answer it".into(),
            });
        }
    }
    for (i, n) in named.iter().enumerate() {
        if !matched.contains(&i) {
            out.skipped.push(SkippedField {
                field: (*n).clone(),
                reason: "this names no field of the Combat Extended block".into(),
            });
        }
    }
    out.spec.ce = Some(ce);
    out
}

/// Fills a tool penetration given as `<label>/<blunt|sharp>`. Returns false when the entry is malformed.
fn set_tool(ce: &mut CePatchSpec, rest: &str, value: f64) -> bool {
    let Some((label, which)) = rest.rsplit_once('/') else {
        return false;
    };
    let new = Sourced::new(value, ValueSource::Suggested);
    let pos = match ce.tool_penetration.iter().position(|p| p.tool == label) {
        Some(i) => i,
        None => {
            ce.tool_penetration.push(CeToolPenetration {
                tool: label.to_owned(),
                sharp: None,
                blunt: None,
            });
            ce.tool_penetration.len().saturating_sub(1)
        }
    };
    let Some(entry) = ce.tool_penetration.get_mut(pos) else {
        return false;
    };
    match which {
        "blunt" => entry.blunt = Some(new),
        "sharp" => entry.sharp = Some(new),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("bulk", "/ce/bulk", true)]
    #[case("/ce/bulk", "/ce/bulk", true)]
    #[case("ce.bulk", "/ce/bulk", true)]
    #[case("SwayFactor", "/ce/swayFactor", true)]
    #[case("sway-factor", "/ce/swayFactor", true)]
    #[case(
        "ce.toolPenetration.blade.blunt",
        "/ce/toolPenetration/blade/blunt",
        true
    )]
    #[case("bulky", "/ce/bulk", false)]
    #[case("mass", "/ce/bulk", false)]
    fn field_names_match_loosely(#[case] name: &str, #[case] pointer: &str, #[case] want: bool) {
        assert_eq!(names_field(name, pointer), want);
    }

    #[test]
    fn similarity_of_disjoint_tags_is_zero_and_of_equal_tags_is_one() {
        let design: BTreeSet<&str> = ["a", "b"].into_iter().collect();
        assert!((similarity(&design, &["a".into(), "b".into()]) - 1.0).abs() < 1e-12);
        assert!(similarity(&design, &["c".into()]).abs() < 1e-12);
        assert!(similarity(&BTreeSet::new(), &[]).abs() < 1e-12);
    }

    #[test]
    fn a_tool_pointer_is_label_and_part() {
        let mut ce = CePatchSpec::default();
        assert!(set_tool(&mut ce, "blade/blunt", 0.5));
        assert!(set_tool(&mut ce, "blade/sharp", 0.7));
        assert!(!set_tool(&mut ce, "blade/other", 0.7));
        assert!(!set_tool(&mut ce, "no-part", 0.7));
        assert_eq!(ce.tool_penetration.len(), 1);
    }

    #[test]
    fn a_count_that_is_not_a_whole_number_of_at_least_one_is_refused() {
        let mut ce = CePatchSpec::default();
        assert!(!set_block_number(&mut ce, "/ce/magazineSize", 0.2));
        assert!(set_block_number(&mut ce, "/ce/magazineSize", 29.6));
        assert_eq!(ce.magazine_size.map(|m| m.value), Some(30));
    }
}
