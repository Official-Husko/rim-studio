//! Flow D: convert the weapons of an existing mod to Combat Extended with an automatic patch.
//!
//! [`scan`] lists the weapon defs of a project with a status: not converted, already converted (update mode
//! applies), an unsupported kind, or a target that does not exist as a concrete def. [`convert`] turns one
//! candidate into a write plan: it reads the def into a vanilla spec (the twin), derives everything it can
//! from the user's own conversions ([`derive_ce_block`]) and returns an [`AskList`] for what cannot be
//! derived: the caliber and ammo set, the weapon tag class, one handed or belt fed, and the tool penetration
//! when the model has no melee conversion to learn from. The existing definition is never edited: the plan
//! holds only patch files and `LoadFolders.xml`.

use std::collections::BTreeMap;

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, DefRecord};
use serde::{Deserialize, Serialize};

use super::bow::{bow_prediction, is_bow_spec};
use super::container::{Container, ConversionSource};
use super::export::{CeProjectState, export_ce_plan_with};
use super::values::{
    DerivedValue, ValueOrigin, predict_for, predict_tool_ratios, tidy, tool_scales,
};
use crate::ce::lint::codes::DEFERRED;
use crate::ce::reader::{CeModel, detect_markers};
use crate::model::{CePatchSpec, CeToolPenetration, DesignSpec, ItemKind, Sourced, ValueSource};
use crate::plan::{PlanBuilder, ProjectLayout, WritePlan};
use crate::reader::access::{child_number, child_text, class_attr, list_items, list_texts};
use crate::reader::options::ReaderOptions;
use crate::reader::{OwnSource, is_weapon_def, spec_from_def, spec_from_def_own};
use crate::validation::codes::VALUE_INVALID;

/// The thing def database name.
const THING_TYPE: &str = "ThingDef";

/// The status of a weapon def of the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConvertStatus {
    /// The def has no Combat Extended conversion; it can be converted.
    NotConverted,
    /// The def already carries a conversion; update mode applies.
    AlreadyCe,
    /// A grenade, launcher, turret, mechanoid or custom class weapon: listed, not converted.
    UnsupportedKind,
    /// The def is abstract, or is not among the resolved defs (a patch operation cannot target it).
    TargetNotFound,
}

/// One weapon def of the project and what can be done with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertCandidate {
    /// The def name (for an abstract base, its `Name`).
    pub def: String,
    /// The label, when the def has one.
    pub label: String,
    /// The kind of weapon, when it could be told.
    pub kind: Option<ItemKind>,
    /// The status.
    pub status: ConvertStatus,
    /// Why, in plain words.
    pub reason: String,
    /// The weapon family: weapons with the same family key can share one answer set. See [`family_key`].
    /// Empty when the def is not a listed weapon (an abstract base, an unresolved def).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
}

/// The family key of a resolved weapon def: the kind, the first weapon tag (`untagged` when there is none)
/// and, for a gun, the default projectile of its shooting verb. Weapons of one family have the same
/// caliber and weapon class, which is what most answers of a conversion depend on, so a mod of many similar
/// guns needs one answer set. The key is the same for the same def on every run.
#[must_use]
pub fn family_key(node: &Node, kind: ItemKind) -> String {
    let tag = list_texts(node, "weaponTags")
        .into_iter()
        .next()
        .unwrap_or_else(|| "untagged".to_owned());
    match kind {
        ItemKind::Ranged => {
            let projectile = shooting_verb(node)
                .and_then(|v| child_text(v, "defaultProjectile"))
                .unwrap_or_else(|| "noprojectile".to_owned());
            format!("ranged/{tag}/{projectile}")
        }
        _ => format!("melee/{tag}"),
    }
}

fn is_true(node: &Node, attr: &str) -> bool {
    node.attr(attr)
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

/// True when a raw node looks like a weapon (it names weapon tags, a shooting verb or tools).
fn looks_like_weapon(node: &Node) -> bool {
    node.child("weaponTags").is_some()
        || node.child("verbs").is_some()
        || node.child("tools").is_some()
}

fn shooting_verb(node: &Node) -> Option<&Node> {
    list_items(node, "verbs")
        .into_iter()
        .find(|v| child_text(v, "defaultProjectile").is_some())
}

/// The reason a weapon is not converted in this release, or `None` when it is supported.
fn unsupported_reason(node: &Node, dbs: &DefDatabases, model: &CeModel) -> Option<String> {
    if node.child("race").is_some() || node.child("building").is_some() {
        return Some("a creature or a building, not a carried weapon".to_owned());
    }
    let tags: Vec<String> = list_texts(node, "weaponTags")
        .into_iter()
        .map(|t| t.to_lowercase())
        .collect();
    if tags
        .iter()
        .any(|t| t.contains("turret") || t.contains("mechanoid"))
    {
        return Some("a turret or mechanoid weapon".to_owned());
    }
    if shooting_verb(node).is_none() && super::platform::has_projectile_less_verb(node) {
        return Some(
            "a weapon whose shooting verb names no projectile (a beam or a spray): there is no ammo to convert"
                .to_owned(),
        );
    }
    if let Some(verb) = shooting_verb(node) {
        let class = child_text(verb, "verbClass").unwrap_or_default();
        if class.contains("OneUse") {
            return Some("a one use weapon (grenade or throwable)".to_owned());
        }
        if class.contains("Launch") {
            return Some("a launcher".to_owned());
        }
        let explosive = child_text(verb, "defaultProjectile")
            .and_then(|p| dbs.get(THING_TYPE, &p))
            .and_then(|d| {
                d.node
                    .child("projectile")
                    .and_then(|p| child_number(p, "explosionRadius"))
            })
            .is_some_and(|r| r > 0.0);
        if explosive {
            return Some("an explosive projectile (grenade or launcher)".to_owned());
        }
    }
    let custom = list_items(node, "verbs")
        .into_iter()
        .chain(list_items(node, "tools"))
        .filter_map(class_attr)
        .find(|c| {
            !c.eq_ignore_ascii_case(&model.classes.verb_properties)
                && !c.eq_ignore_ascii_case(&model.classes.tool)
        });
    custom.map(|c| format!("a custom verb or tool class ({c})"))
}

/// Lists the weapon defs of a project.
///
/// `project` holds the raw top level def nodes of the project's own files (the unresolved nodes that a
/// patch operation addresses), `dbs` the resolved defs of the load. Only `ThingDef` nodes that look like
/// weapons are listed, sorted by def name. A def that is abstract or missing from the resolved defs is
/// [`ConvertStatus::TargetNotFound`]; a def with a conversion marker is [`ConvertStatus::AlreadyCe`].
#[must_use]
pub fn scan(project: &[Node], dbs: &DefDatabases, model: &CeModel) -> Vec<ConvertCandidate> {
    let mut out: BTreeMap<String, ConvertCandidate> = BTreeMap::new();
    for raw in project.iter().filter(|n| n.tag == THING_TYPE) {
        let abstract_def = is_true(raw, "Abstract");
        let name = child_text(raw, "defName").or_else(|| raw.attr("Name").map(str::to_owned));
        let Some(name) = name else { continue };
        let label = child_text(raw, "label").unwrap_or_default();
        let candidate = |status, reason: &str, kind| ConvertCandidate {
            def: name.clone(),
            label: label.clone(),
            kind,
            status,
            reason: reason.to_owned(),
            family: String::new(),
        };
        if abstract_def {
            if looks_like_weapon(raw) {
                out.insert(
                    name.clone(),
                    candidate(
                        ConvertStatus::TargetNotFound,
                        "an abstract base: a conversion targets concrete defs by name",
                        None,
                    ),
                );
            }
            continue;
        }
        let Some(record) = dbs.get(THING_TYPE, &name) else {
            if looks_like_weapon(raw) {
                out.insert(
                    name.clone(),
                    candidate(
                        ConvertStatus::TargetNotFound,
                        "the def is not among the resolved defs",
                        None,
                    ),
                );
            }
            continue;
        };
        let node = &record.node;
        // A weapon whose verb names no projectile (a beam, a spray) is a gun with nothing to convert, not a
        // melee weapon: it is listed with its reason.
        let beam = super::platform::has_projectile_less_verb(node);
        let ranged = shooting_verb(node).is_some() || beam;
        let melee = !ranged && node.child("tools").is_some() && is_weapon_def(node);
        if !ranged && !melee {
            continue;
        }
        let kind = Some(if ranged {
            ItemKind::Ranged
        } else {
            ItemKind::Melee
        });
        let family = family_key(
            node,
            if ranged {
                ItemKind::Ranged
            } else {
                ItemKind::Melee
            },
        );
        let entry = if let Some(why) = unsupported_reason(node, dbs, model) {
            candidate(ConvertStatus::UnsupportedKind, &why, kind)
        } else if detect_markers(node, &model.classes).is_conversion() {
            candidate(
                ConvertStatus::AlreadyCe,
                "the def already carries a Combat Extended conversion; update mode applies",
                kind,
            )
        } else {
            candidate(
                ConvertStatus::NotConverted,
                super::platform::convert_reason(node),
                kind,
            )
        };
        out.insert(name.clone(), ConvertCandidate { family, ..entry });
    }
    out.into_values().collect()
}

/// What the kind of an ask is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AskKind {
    /// One of the listed options.
    Choice,
    /// Yes or no.
    Flag,
    /// A number.
    Number,
}

/// One thing the automatic mode could not derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskItem {
    /// The JSON pointer into the spec the answer belongs to (`/ce/ammoSet`).
    pub field: String,
    /// A short English question.
    pub label: String,
    /// The kind of answer.
    pub kind: AskKind,
    /// The options of a choice, in the order to show.
    pub options: Vec<String>,
    /// Why the number is asked although the predictors have an estimate: the estimate was rated
    /// unreliable or could not be measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The rejected estimate, for reference only: it is never written to a patch unless the user answers
    /// with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<f64>,
}

/// Everything the automatic mode needs from the user, in a fixed order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AskList {
    /// The questions.
    pub items: Vec<AskItem>,
}

impl AskList {
    /// True when nothing is left to ask.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when a question about the field exists.
    #[must_use]
    pub fn asks(&self, field: &str) -> bool {
        self.items.iter().any(|i| i.field == field)
    }
}

/// The user's answers. Every field is optional; an answer always wins over a derived value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConvertAnswers {
    /// The ammo set.
    pub ammo_set: Option<String>,
    /// The default projectile; the first projectile of the ammo set when unset.
    pub default_projectile: Option<String>,
    /// The weapon tag class (an AI class tag of the installed data).
    pub weapon_tag_class: Option<String>,
    /// One handed.
    pub one_handed: Option<bool>,
    /// Belt fed.
    pub belt_fed: Option<bool>,
    /// Penetration by tool label.
    pub tool_penetration: Vec<CeToolPenetration>,
    /// Field level overrides: every number of the block that is `Some` replaces the derived one. The
    /// platform choices and the under barrel unit are answered here too (`underBarrel`, `isWeaponPlatform`,
    /// `attachmentLinks`, `defaultGraphicParts`).
    pub overrides: CePatchSpec,
    /// Convert the weapon without its own under barrel unit, although its vanilla definition has one.
    pub skip_under_barrel: bool,
}

/// Where a number of the derived block came from, collected while deriving.
struct Deriver<'a> {
    model: &'a CeModel,
    answers: &'a ConvertAnswers,
    asks: Vec<AskItem>,
    derived: Vec<DerivedValue>,
}

impl Deriver<'_> {
    /// Asks for a number and shows the rejected estimate with the reason, when there is one.
    fn ask_guess(&mut self, field: &str, label: &str, guess: &Guess) {
        self.asks.push(AskItem {
            field: field.to_owned(),
            label: label.to_owned(),
            kind: AskKind::Number,
            options: Vec::new(),
            reason: guess.reason.clone(),
            suggestion: guess.reference,
        });
    }

    /// A number: the override, else the prediction of `stat`, else an ask.
    fn number(
        &mut self,
        field: &str,
        label: &str,
        over: Option<Sourced<f64>>,
        guess: &Guess,
        optional: bool,
    ) -> Option<Sourced<f64>> {
        if let Some(o) = over {
            return Some(o);
        }
        if let Some(v) = guess.value.filter(|v| v.is_finite()) {
            return Some(Sourced::new(tidy(v), ValueSource::Suggested));
        }
        if !optional {
            self.ask_guess(field, label, guess);
        }
        None
    }
}

/// Why a stat has no estimate at all.
fn absent_reason(prediction: &crate::ce::classes::ConversionPrediction, name: &str) -> String {
    match prediction.withheld.get(name) {
        Some(why) => format!("no estimate: {why}"),
        None => "no estimate: no converted weapon in the library carries it".to_owned(),
    }
}

/// What the predictors say about one stat: a number a patch may carry, or only a reference guess with the
/// reason it is not carried.
struct Guess {
    /// The estimate, when its rating allows writing it.
    value: Option<f64>,
    /// The estimate that was rejected, for reference.
    reference: Option<f64>,
    /// Why it was rejected.
    reason: Option<String>,
}

impl Guess {
    fn none() -> Self {
        Self {
            value: None,
            reference: None,
            reason: None,
        }
    }
}

/// The estimate of a stat as a [`Guess`]. A usable estimate is recorded in `derived` with its rating.
fn guess_of(
    prediction: Option<&crate::ce::classes::ConversionPrediction>,
    name: &str,
    derived: &mut Vec<DerivedValue>,
    field: &str,
) -> Guess {
    let Some(prediction) = prediction else {
        return Guess {
            value: None,
            reference: None,
            reason: Some(
                "the library has no converted weapons of this kind to estimate from".to_owned(),
            ),
        };
    };
    let Some(p) = prediction.stats.get(name) else {
        return Guess {
            value: None,
            reference: None,
            reason: Some(absent_reason(prediction, name)),
        };
    };
    if !p.value.is_finite() {
        return Guess::none();
    }
    if p.is_usable() {
        derived.push(DerivedValue {
            field: field.to_owned(),
            value: tidy(p.value),
            origin: ValueOrigin::Predicted {
                predictor: p.predictor,
                n: p.n,
            },
            rating: Some(p.reliability),
            error: p.loo_error,
        });
        return Guess {
            value: Some(p.value),
            reference: None,
            reason: None,
        };
    }
    Guess {
        value: None,
        reference: Some(tidy(p.value)),
        reason: Some(format!(
            "the estimate {} is not written: it is {}",
            tidy(p.value),
            p.accuracy_note()
        )),
    }
}

/// Derives the Combat Extended block of a vanilla spec (the twin) in automatic mode.
///
/// Returns the block (with the fields that could be decided), the questions for what could not and the
/// numbers that were derived, with their origin. The derivation never invents a value: a number comes from
/// an answer, an override or a predictor over the conversions in the model, else it is asked.
#[must_use]
pub fn derive_ce_block(
    spec: &DesignSpec,
    model: &CeModel,
    answers: &ConvertAnswers,
) -> (CePatchSpec, AskList, Vec<DerivedValue>) {
    let mut d = Deriver {
        model,
        answers,
        asks: Vec::new(),
        derived: Vec::new(),
    };
    let over = &answers.overrides;
    let mut block = CePatchSpec {
        ammo_set: answers.ammo_set.clone().or_else(|| over.ammo_set.clone()),
        default_projectile: answers
            .default_projectile
            .clone()
            .or_else(|| over.default_projectile.clone()),
        weapon_tag_class: answers
            .weapon_tag_class
            .clone()
            .or_else(|| over.weapon_tag_class.clone()),
        caliber: over.caliber.clone(),
        one_handed: answers.one_handed.unwrap_or(over.one_handed),
        belt_fed: answers.belt_fed.unwrap_or(over.belt_fed),
        ..CePatchSpec::default()
    };
    // A bow or a crossbow is converted in its own style: other choices, other numbers, other examples.
    let bow = spec.kind == ItemKind::Ranged && over.bow.unwrap_or_else(|| is_bow_spec(spec));
    if bow || over.bow.is_some() {
        block.bow = Some(bow);
    }
    if bow {
        bow_derive::choices(&mut d, &mut block);
    } else if spec.kind == ItemKind::Ranged {
        ranged_choices(&mut d, &mut block);
    }
    // The prediction depends on the tag class, so it is made after the choices.
    let mut probe = spec.clone();
    probe.ce = Some(block.clone());
    let prediction = if bow {
        bow_prediction(&probe, model)
    } else {
        predict_for(&probe, model)
    };
    match spec.kind {
        ItemKind::Ranged if bow => {
            bow_derive::numbers(&mut d, &mut block, prediction.as_ref());
            ranged_tools(&mut d, &mut block, spec);
        }
        ItemKind::Ranged => {
            ranged_numbers(&mut d, &mut block, prediction.as_ref());
            ranged_tools(&mut d, &mut block, spec);
        }
        ItemKind::Melee => {
            melee_numbers(&mut d, &mut block, spec, prediction.as_ref());
        }
    }
    let Deriver { asks, derived, .. } = d;
    (block, AskList { items: asks }, derived)
}

fn ranged_choices(d: &mut Deriver<'_>, block: &mut CePatchSpec) {
    let set_names: Vec<String> = {
        let mut v: Vec<String> = d
            .model
            .ammo_sets
            .iter()
            .map(|a| a.def_name.clone())
            .collect();
        v.sort();
        v
    };
    if block.ammo_set.is_none() {
        d.asks.push(AskItem {
            field: "/ce/ammoSet".into(),
            label: "Which caliber (ammo set) does the weapon use?".into(),
            kind: AskKind::Choice,
            options: set_names,
            reason: None,
            suggestion: None,
        });
    }
    if block.default_projectile.is_none() {
        let first = block
            .ammo_set
            .as_deref()
            .and_then(|s| d.model.ammo_set(s))
            .and_then(|s| s.first())
            .map(|a| a.projectile.clone());
        match first {
            Some(p) => block.default_projectile = Some(p),
            None if block.ammo_set.is_some() => d.asks.push(AskItem {
                field: "/ce/defaultProjectile".into(),
                label: "Which projectile of the ammo set is the default?".into(),
                kind: AskKind::Choice,
                options: Vec::new(),
                reason: None,
                suggestion: None,
            }),
            None => {}
        }
    }
    if block.weapon_tag_class.is_none() {
        d.asks.push(AskItem {
            field: "/ce/weaponTagClass".into(),
            label: "Which weapon class tag does the weapon get?".into(),
            kind: AskKind::Choice,
            options: d.model.ai_class_tags.clone(),
            reason: None,
            suggestion: None,
        });
    }
    if d.answers.one_handed.is_none() && !d.answers.overrides.one_handed {
        d.asks.push(AskItem {
            field: "/ce/oneHanded".into(),
            label: "Is the weapon held in one hand?".into(),
            kind: AskKind::Flag,
            options: Vec::new(),
            reason: None,
            suggestion: None,
        });
    }
    if d.answers.belt_fed.is_none() && !d.answers.overrides.belt_fed {
        d.asks.push(AskItem {
            field: "/ce/beltFed".into(),
            label: "Is the weapon belt fed?".into(),
            kind: AskKind::Flag,
            options: Vec::new(),
            reason: None,
            suggestion: None,
        });
    }
}

fn whole(v: Option<f64>) -> Option<u32> {
    let v = v?.round();
    (v >= 1.0 && v <= f64::from(u32::MAX)).then_some(v as u32)
}

fn ranged_numbers(
    d: &mut Deriver<'_>,
    block: &mut CePatchSpec,
    prediction: Option<&crate::ce::classes::ConversionPrediction>,
) {
    let over = d.answers.overrides.clone();
    let mut derived = Vec::new();
    let bulk = guess_of(prediction, "bulk", &mut derived, "ce.bulk");
    let sway = guess_of(prediction, "sway", &mut derived, "ce.swayFactor");
    let spread = guess_of(prediction, "spread", &mut derived, "ce.shotSpread");
    let sights = guess_of(prediction, "sights", &mut derived, "ce.sightsEfficiency");
    let recoil = guess_of(prediction, "recoil", &mut derived, "ce.recoilAmount");
    let magazine = guess_of(prediction, "magazine", &mut derived, "ce.magazineSize");
    let reload = guess_of(prediction, "reload", &mut derived, "ce.reloadTime");
    d.derived.extend(derived);
    block.bulk = d.number("/ce/bulk", "CE bulk", over.bulk, &bulk, false);
    block.sway_factor = d.number(
        "/ce/swayFactor",
        "CE sway factor",
        over.sway_factor,
        &sway,
        false,
    );
    block.shot_spread = d.number(
        "/ce/shotSpread",
        "CE shot spread",
        over.shot_spread,
        &spread,
        false,
    );
    block.sights_efficiency = d.number(
        "/ce/sightsEfficiency",
        "CE sights efficiency",
        over.sights_efficiency,
        &sights,
        true,
    );
    block.recoil_amount = d.number(
        "/ce/recoilAmount",
        "CE recoil",
        over.recoil_amount,
        &recoil,
        true,
    );
    block.cooldown = over.cooldown;
    block.magazine_size = match over.magazine_size {
        Some(m) => Some(m),
        None => match whole(magazine.value) {
            Some(m) => Some(Sourced::new(m, ValueSource::Suggested)),
            None => {
                d.ask_guess("/ce/magazineSize", "CE magazine size", &magazine);
                None
            }
        },
    };
    block.reload_time = d.number(
        "/ce/reloadTime",
        "CE reload time",
        over.reload_time,
        &reload,
        false,
    );
}

fn ranged_tools(d: &mut Deriver<'_>, block: &mut CePatchSpec, spec: &DesignSpec) {
    if spec.tools.is_empty() {
        return;
    }
    let ratios = predict_tool_ratios(spec, d.model);
    tools_block(d, block, spec, ratios.as_ref(), None, false);
}

fn melee_numbers(
    d: &mut Deriver<'_>,
    block: &mut CePatchSpec,
    spec: &DesignSpec,
    prediction: Option<&crate::ce::classes::ConversionPrediction>,
) {
    let over = d.answers.overrides.clone();
    let mut derived = Vec::new();
    let bulk = guess_of(prediction, "bulk", &mut derived, "ce.bulk");
    let parry_bonus = guess_of(prediction, "counter_parry", &mut derived, "ce.parryBonus");
    let crit = guess_of(prediction, "crit", &mut derived, "ce.meleeCritChance");
    let parry = guess_of(prediction, "parry", &mut derived, "ce.meleeParryChance");
    let dodge = guess_of(prediction, "dodge", &mut derived, "ce.meleeDodgeChance");
    d.derived.extend(derived);
    block.bulk = d.number("/ce/bulk", "CE bulk", over.bulk, &bulk, false);
    block.parry_bonus = d.number(
        "/ce/parryBonus",
        "CE counter parry bonus",
        over.parry_bonus,
        &parry_bonus,
        true,
    );
    block.melee_crit_chance = d.number(
        "/ce/meleeCritChance",
        "CE melee crit chance",
        over.melee_crit_chance,
        &crit,
        false,
    );
    block.melee_parry_chance = d.number(
        "/ce/meleeParryChance",
        "CE melee parry chance",
        over.melee_parry_chance,
        &parry,
        false,
    );
    block.melee_dodge_chance = d.number(
        "/ce/meleeDodgeChance",
        "CE melee dodge chance",
        over.melee_dodge_chance,
        &dodge,
        false,
    );
    tools_block(d, block, spec, prediction, prediction, true);
}

/// Decides the penetration of every tool of the spec: an answer, else the class ratio over the (converted)
/// power, else a question. `ratios` is the prediction that holds the ratios, `scale_from` the one that
/// scales power (the same for melee weapons).
fn tools_block(
    d: &mut Deriver<'_>,
    block: &mut CePatchSpec,
    spec: &DesignSpec,
    ratios: Option<&crate::ce::classes::ConversionPrediction>,
    scale_from: Option<&crate::ce::classes::ConversionPrediction>,
    scale: bool,
) {
    let (power_scale, _) = tool_scales(spec, scale_from, scale);
    let ratio = |stat: &str| {
        ratios
            .and_then(|p| p.stats.get(stat))
            .filter(|p| p.value.is_finite() && p.value > 0.0)
    };
    for (i, tool) in spec.tools.iter().enumerate() {
        let number = i.saturating_add(1);
        let entry = d
            .answers
            .tool_penetration
            .iter()
            .chain(d.answers.overrides.tool_penetration.iter())
            .find(|p| p.tool == tool.label)
            .cloned();
        let power = tool.power.map(|p| p.value * power_scale);
        let sharp_needed = tool
            .capacities
            .iter()
            .any(|c| matches!(c.as_str(), "Cut" | "Stab"));
        let derive = |d: &mut Deriver<'_>,
                      stat: &str,
                      name: &str|
         -> (Option<Sourced<f64>>, Guess) {
            let Some(power) = power else {
                return (None, Guess::none());
            };
            let Some(r) = ratio(stat) else {
                let reason = match ratios {
                    Some(p) => absent_reason(p, stat),
                    None => {
                        "the library has no converted melee weapons to estimate from".to_owned()
                    }
                };
                return (
                    None,
                    Guess {
                        value: None,
                        reference: None,
                        reason: Some(reason),
                    },
                );
            };
            let value = tidy(r.value * power);
            if !r.is_usable() {
                return (
                    None,
                    Guess {
                        value: None,
                        reference: Some(value),
                        reason: Some(format!(
                            "the estimate {value} is not written: the ratio of penetration to power is {}",
                            r.accuracy_note()
                        )),
                    },
                );
            }
            d.derived.push(DerivedValue {
                field: format!("tool {number} {name}"),
                value,
                origin: ValueOrigin::Predicted {
                    predictor: r.predictor,
                    n: r.n,
                },
                rating: Some(r.reliability),
                error: r.loo_error,
            });
            (
                Some(Sourced::new(value, ValueSource::Suggested)),
                Guess::none(),
            )
        };
        let (blunt, blunt_guess) = match entry.as_ref().and_then(|e| e.blunt) {
            Some(b) => (Some(b), Guess::none()),
            None => derive(d, "ap_blunt_ratio", "armorPenetrationBlunt"),
        };
        let (sharp, sharp_guess) = match entry.as_ref().and_then(|e| e.sharp) {
            Some(s) => (Some(s), Guess::none()),
            None if sharp_needed => derive(d, "ap_sharp_ratio", "armorPenetrationSharp"),
            None => (None, Guess::none()),
        };
        if blunt.is_none() {
            d.ask_guess(
                &format!("/ce/toolPenetration/{}/blunt", tool.label),
                &format!("Blunt penetration of the tool {}", tool.label),
                &blunt_guess,
            );
        }
        if sharp_needed && sharp.is_none() {
            d.ask_guess(
                &format!("/ce/toolPenetration/{}/sharp", tool.label),
                &format!("Sharp penetration of the tool {}", tool.label),
                &sharp_guess,
            );
        }
        block.tool_penetration.push(CeToolPenetration {
            tool: tool.label.clone(),
            sharp,
            blunt,
        });
    }
}

/// What [`convert`] works on besides the candidate.
#[derive(Debug, Clone, Copy)]
pub struct ConvertEnv<'a> {
    /// The resolved defs of the load.
    pub dbs: &'a DefDatabases,
    /// The model of the user's Combat Extended.
    pub model: &'a CeModel,
    /// The file layout of the project.
    pub layout: &'a ProjectLayout,
    /// The reader options.
    pub reader: &'a ReaderOptions,
    /// The raw top level def nodes of the project (to read the containers of the target).
    pub project: &'a [Node],
    /// The `LoadFolders.xml` state of the project and the game version.
    pub state: &'a CeProjectState,
    /// Where the existing conversion of an already converted target came from.
    pub source: &'a ConversionSource,
}

/// The result of [`convert`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOutcome {
    /// The plan: the CE patch file and `LoadFolders.xml`. Empty while questions are open.
    pub plan: WritePlan,
    /// What is still to ask.
    pub asks: AskList,
    /// The spec the plan was built from (the vanilla twin with the derived block), when it could be read.
    pub spec: Option<DesignSpec>,
    /// The numbers derived instead of given.
    pub derived: Vec<DerivedValue>,
    /// True when the target already carried a conversion and update mode was used.
    pub update: bool,
}

fn empty(diagnostics: Vec<Diagnostic>) -> ConvertOutcome {
    let mut builder = PlanBuilder::new();
    builder.extend_diagnostics(diagnostics);
    ConvertOutcome {
        plan: builder.build(),
        asks: AskList::default(),
        spec: None,
        derived: Vec::new(),
        update: false,
    }
}

fn overlay(base: &CePatchSpec, with: &CePatchSpec) -> CePatchSpec {
    let mut out = base.clone();
    macro_rules! take {
        ($($f:ident),*) => { $( if with.$f.is_some() { out.$f.clone_from(&with.$f); } )* };
    }
    take!(
        caliber,
        ammo_set,
        default_projectile,
        weapon_tag_class,
        magazine_size,
        reload_time,
        bulk,
        sway_factor,
        shot_spread,
        sights_efficiency,
        recoil_amount,
        cooldown,
        parry_bonus,
        melee_crit_chance,
        melee_parry_chance,
        melee_dodge_chance,
        bow,
        ammo_gen_per_mag,
        allow_with_run_and_gun,
        mass,
        reload_one_at_a_time,
        recoil_pattern
    );
    if !with.tool_plan.is_empty() {
        out.tool_plan.clone_from(&with.tool_plan);
    }
    for f in &with.keep_tool_fields {
        if !out.keep_tool_fields.contains(f) {
            out.keep_tool_fields.push(f.clone());
        }
    }
    for t in &with.extra_tags {
        if !out.extra_tags.contains(t) {
            out.extra_tags.push(t.clone());
        }
    }
    if !with.raw_extras.is_empty() {
        out.raw_extras.clone_from(&with.raw_extras);
    }
    for p in &with.tool_penetration {
        match out.tool_penetration.iter_mut().find(|e| e.tool == p.tool) {
            Some(e) => {
                if p.sharp.is_some() {
                    e.sharp = p.sharp;
                }
                if p.blunt.is_some() {
                    e.blunt = p.blunt;
                }
            }
            None => out.tool_penetration.push(p.clone()),
        }
    }
    out.one_handed |= with.one_handed;
    out.belt_fed |= with.belt_fed;
    super::platform::overlay(&mut out, with);
    out
}

fn raw_node<'a>(project: &'a [Node], def_name: &str) -> Option<&'a Node> {
    project
        .iter()
        .find(|n| n.tag == THING_TYPE && child_text(n, "defName").as_deref() == Some(def_name))
}

fn block_from_answers(answers: &ConvertAnswers) -> CePatchSpec {
    let mut block = answers.overrides.clone();
    if answers.ammo_set.is_some() {
        block.ammo_set.clone_from(&answers.ammo_set);
    }
    if answers.default_projectile.is_some() {
        block
            .default_projectile
            .clone_from(&answers.default_projectile);
    }
    if answers.weapon_tag_class.is_some() {
        block.weapon_tag_class.clone_from(&answers.weapon_tag_class);
    }
    if !answers.tool_penetration.is_empty() {
        block
            .tool_penetration
            .extend(answers.tool_penetration.iter().cloned());
    }
    block
}

/// Converts one candidate.
///
/// - A candidate that is not [`ConvertStatus::NotConverted`] or [`ConvertStatus::AlreadyCe`] gives an empty
///   plan with a diagnostic.
/// - A not converted weapon: the block is derived ([`derive_ce_block`]); open questions give an empty plan
///   and the [`AskList`], none gives the plan of [`export_ce_plan_with`].
/// - An already converted weapon: update mode. The block is the existing conversion overlaid with the
///   answers, so only the fields the user changed produce operations.
///
/// The existing definition is never edited: the plan has patch files and `LoadFolders.xml` only.
#[must_use]
pub fn convert(
    candidate: &ConvertCandidate,
    answers: &ConvertAnswers,
    env: &ConvertEnv<'_>,
) -> ConvertOutcome {
    if !matches!(
        candidate.status,
        ConvertStatus::NotConverted | ConvertStatus::AlreadyCe
    ) {
        return empty(vec![DEFERRED.diagnostic(
            "",
            &[(
                "what",
                &format!("the conversion of {} ({})", candidate.def, candidate.reason),
            )],
        )]);
    }
    let Some(record): Option<&DefRecord> = env.dbs.get(THING_TYPE, &candidate.def) else {
        return empty(vec![VALUE_INVALID.diagnostic(
            "",
            &[
                ("label", "the def"),
                ("value", &candidate.def),
                ("reason", "it is not among the resolved defs"),
            ],
        )]);
    };
    // With the def as its file writes it, the spec also carries what the conversion must not lose: the
    // fields of the shooting verb and the extras of the tools that the designer has no field for.
    let reading = match match raw_node(env.project, &candidate.def) {
        Some(raw) => spec_from_def_own(
            record,
            env.dbs,
            env.reader,
            ValueSource::Typed,
            &OwnSource {
                def: raw,
                projectile: None,
            },
        ),
        None => spec_from_def(record, env.dbs, env.reader, ValueSource::Typed),
    } {
        Ok(r) => r,
        Err(e) => {
            return empty(vec![VALUE_INVALID.diagnostic(
                "",
                &[
                    ("label", "the def"),
                    ("value", &candidate.def),
                    ("reason", &e.to_string()),
                ],
            )]);
        }
    };
    let mut spec = reading.spec;
    let container = Container::from_def(
        record,
        raw_node(env.project, &candidate.def),
        &env.model.classes,
        env.source.clone(),
    );
    let mut state = env.state.clone();
    state.container = Some(container.clone());
    if let Some(existing) = &container.existing {
        spec.ce = Some(overlay(&existing.block, &block_from_answers(answers)));
        let plan = export_ce_plan_with(&spec, env.model, env.layout, &state);
        return ConvertOutcome {
            plan,
            asks: AskList::default(),
            spec: Some(spec),
            derived: Vec::new(),
            update: true,
        };
    }
    let (mut block, mut asks, derived) = derive_ce_block(&spec, env.model, answers);
    if spec.kind == ItemKind::Ranged && !block.bow.unwrap_or(false) {
        super::platform::derive::apply(
            &mut block,
            &mut asks,
            &record.node,
            env.dbs,
            env.model,
            answers,
        );
    }
    spec.ce = Some(block);
    if !asks.is_empty() {
        return ConvertOutcome {
            plan: WritePlan::default(),
            asks,
            spec: Some(spec),
            derived,
            update: false,
        };
    }
    let plan = export_ce_plan_with(&spec, env.model, env.layout, &state);
    ConvertOutcome {
        plan,
        asks,
        spec: Some(spec),
        derived,
        update: false,
    }
}

mod bow_derive;
#[cfg(test)]
mod bow_tests;
