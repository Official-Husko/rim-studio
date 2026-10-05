//! The conversion estimator: predicts the Combat Extended numbers of a vanilla design from the user's own
//! conversions, measures its own error, and rates every stat.
//!
//! The predictor of [`super::predict_conversion`] picked a class by exact role, tier and group keys and let a
//! class median ignore the vanilla number, which failed for small classes. This module replaces that with
//! four rules, all generic (no stat or tag table, no game value):
//!
//! 1. **Class by similarity.** A design is compared with every conversion by its vanilla weapon tags
//!    (weighted by how rare a tag is, so a tag every weapon carries counts for nothing), the tag class the
//!    user answered (when known), the verb shape (a burst or a single shot), the tier and, as a tie break
//!    only, the tercile of a strength proxy computed from the vanilla numbers. The class of a design is its
//!    [`EstimateOptions::class_size`] nearest conversions. A class never has fewer than
//!    [`EstimateOptions::min_class`] members: a stat that fewer conversions carry is not predicted at all.
//! 2. **Forms.** A stat is estimated in one of three forms: the vanilla number times the class's median
//!    ratio (`ratio`, with the log ratio shrunk toward the ratio of all conversions with strength
//!    [`EstimateOptions::shrink`], so a class of three cannot drift far from the whole pool), the class
//!    median of the converted number (`median`, not shrunk: inside a good class an absolute number is
//!    constant) or the vanilla number itself (`identity`, only for a stat that the vanilla definition
//!    measures in the same unit).
//! 3. **Selection and measurement by leave one out.** Which form a stat uses is decided by the median
//!    relative error of every form when each conversion is predicted from the others. The forms are tried
//!    in the order identity, ratio, median (simplest first, and the ones that keep the vanilla number
//!    before the one that ignores it); a later form replaces an earlier one only when it is better by
//!    [`EstimateOptions::switch_margin`]. The same measurement gives the band of the estimate and its
//!    [`Reliability`]. With fewer than [`EstimateOptions::min_measured`] conversions a stat is
//!    [`Reliability::Unmeasured`]. (A fourth form, the median of the whole pool, was tried and dropped: on
//!    the real conversion set it helped two stats and added enough selection noise to hurt the melee
//!    stats.)
//! 4. **Reliability gates authority.** A stat rated [`Reliability::Reliable`] or [`Reliability::Rough`] may
//!    be written by the generator (the plan labels it); a stat rated [`Reliability::Unreliable`] or
//!    [`Reliability::Unmeasured`] is only shown as a reference guess and asked instead.
//!
//! The constants of [`EstimateOptions`] are generic statistical settings. They were chosen on a coarse grid
//! over leave one out error on a real conversion set, so the leave one out numbers are mildly optimistic;
//! the held out check of `tests/real_eval.rs` and `docs/research/ce-conversion-eval-0.1.0.md` is the
//! honest measure.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    ConversionPrediction, ConversionPredictor, StatPrediction, driver_of, is_converted_stat,
};
use crate::baseline::{Band, band_from_residuals};
use crate::ce::reader::{CeGun, CeMelee, CeModel};
use crate::classes::numeric::{median, quantile};
use crate::classes::{ChainLevel, ItemKind};
use crate::model::TechLevel;

/// How far a stat can be trusted, from its leave one out error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reliability {
    /// The median leave one out error is below [`EstimateOptions::reliable_below`].
    Reliable,
    /// The error is below [`EstimateOptions::rough_below`]: usable, and labelled as a rough estimate.
    Rough,
    /// The error is too high to be useful: the stat is asked, not written.
    Unreliable,
    /// Too few conversions to measure an error: the stat is asked, not written.
    #[default]
    Unmeasured,
}

impl Reliability {
    /// True when a generator may write the number (reliable or rough).
    #[must_use]
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Reliable | Self::Rough)
    }

    /// A short word for readouts.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Reliable => "reliable",
            Self::Rough => "rough",
            Self::Unreliable => "unreliable",
            Self::Unmeasured => "unmeasured",
        }
    }
}

/// The generic settings of the estimator. Every number is a statistical setting, none is a game value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EstimateOptions {
    /// Members of a class: the nearest conversions that carry the stat.
    pub class_size: usize,
    /// Smallest class (and smallest pool) a stat is predicted from.
    pub min_class: usize,
    /// Fewest conversions for which a leave one out error is measured.
    pub min_measured: usize,
    /// Shrinkage strength of a class ratio: a class of `n` members counts `n` against this many pseudo
    /// members of the whole pool.
    pub shrink: f64,
    /// Distance added when both sides name a tag class and the names differ.
    pub w_role: f64,
    /// Weight of the rarity weighted tag dissimilarity (0 to 1).
    pub w_tag: f64,
    /// Distance added when one side is a burst weapon and the other is not.
    pub w_shape: f64,
    /// Distance per tier step.
    pub w_tier: f64,
    /// Distance per tercile step of the strength proxy (small: a tie break).
    pub w_tercile: f64,
    /// A form later in the tie break order (identity, ratio, median) replaces an earlier one only
    /// when its leave one out error is lower by this share: a guard against choosing noise.
    pub switch_margin: f64,
    /// Median relative error below which a stat is [`Reliability::Reliable`].
    pub reliable_below: f64,
    /// Median relative error below which a stat is [`Reliability::Rough`]; above it the stat is asked.
    pub rough_below: f64,
    /// An 80th percentile relative error at or above this makes a stat [`Reliability::Unreliable`] whatever
    /// its median: a number that is often more than this far off is not authoritative.
    pub worst_p80: f64,
    /// Stats that are whole numbers: the estimate is rounded, at least 1.
    pub integer_stats: Vec<String>,
    /// Converted stats that are not estimated because the chosen ammo supplies them.
    pub skip: Vec<String>,
}

impl Default for EstimateOptions {
    fn default() -> Self {
        Self {
            class_size: 4,
            min_class: 3,
            min_measured: 5,
            shrink: 2.0,
            w_role: 1.0,
            w_tag: 1.0,
            w_shape: 0.5,
            w_tier: 0.3,
            w_tercile: 0.05,
            switch_margin: 0.2,
            reliable_below: 0.15,
            rough_below: 0.35,
            worst_p80: 0.8,
            integer_stats: vec!["burst".to_owned(), "magazine".to_owned()],
            skip: [
                "damage",
                "speed",
                "ap_sharp",
                "ap_blunt",
                "pellets",
                "ticks_between",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

impl EstimateOptions {
    /// The rating of a leave one out measurement over `folds` conversions: the median and the 80th
    /// percentile of the relative error.
    #[must_use]
    pub fn rate(
        &self,
        median_error: Option<f64>,
        p80_error: Option<f64>,
        folds: usize,
    ) -> Reliability {
        match (median_error, p80_error) {
            (Some(m), Some(p)) if folds >= self.min_measured && m.is_finite() && p.is_finite() => {
                if p >= self.worst_p80 {
                    Reliability::Unreliable
                } else if m < self.reliable_below {
                    Reliability::Reliable
                } else if m < self.rough_below {
                    Reliability::Rough
                } else {
                    Reliability::Unreliable
                }
            }
            _ => Reliability::Unmeasured,
        }
    }
}

/// One converted weapon as a training example: what a designer would have known (the twin) and what the
/// conversion did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    /// The def name.
    pub id: String,
    /// The tier (tech level index), when known.
    pub tier: Option<u8>,
    /// Vanilla weapon tags: those of the twin when there is one, else the converted weapon's own tags
    /// without the Combat Extended prefix.
    pub tags: Vec<String>,
    /// The tag class (AI class) of the conversion, when it has one.
    pub role: Option<String>,
    /// The converted numbers by name.
    pub stats: BTreeMap<String, f64>,
    /// The numbers of the vanilla twin (empty without a twin).
    pub vanilla: BTreeMap<String, f64>,
}

/// What a designer knows about a vanilla weapon before it is converted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// The tier, when known.
    pub tier: Option<u8>,
    /// The vanilla weapon tags.
    pub tags: Vec<String>,
    /// The tag class the user chose, when known.
    pub role: Option<String>,
    /// The vanilla numbers by name (`mass`, `range`, `warmup`, `cooldown`, `burst` for guns; `mass`,
    /// `tool_power`, `tool_cooldown` for melee weapons).
    pub vanilla: BTreeMap<String, f64>,
}

/// The training examples of one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleSet {
    /// The kind of weapon.
    pub kind: ItemKind,
    /// The examples, in the order of the model.
    pub examples: Vec<Example>,
}

fn strip_prefix_tags(tags: &[String], prefix: &str) -> Vec<String> {
    tags.iter()
        .filter(|t| prefix.is_empty() || !t.starts_with(prefix))
        .cloned()
        .collect()
}

fn gun_example(g: &CeGun, prefix: &str) -> Example {
    Example {
        id: g.def_name.clone(),
        tier: g.tech_level.map(TechLevel::index),
        tags: if g.twin.is_some() {
            g.twin_tags.clone()
        } else {
            strip_prefix_tags(&g.weapon_tags, prefix)
        },
        role: g.ai_class.clone(),
        stats: g.stats.clone(),
        vanilla: g.twin.clone().unwrap_or_default(),
    }
}

fn melee_example(m: &CeMelee, prefix: &str) -> Example {
    Example {
        id: m.def_name.clone(),
        tier: m.tech_level.map(TechLevel::index),
        tags: if m.twin.is_some() {
            m.twin_tags.clone()
        } else {
            strip_prefix_tags(&m.weapon_tags, prefix)
        },
        role: None,
        stats: m.stats.clone(),
        vanilla: m.twin.clone().unwrap_or_default(),
    }
}

impl ExampleSet {
    /// The examples of the converted weapons of `kind`: weapons with an exclusion reason (turret guns,
    /// launchers, grenades) are left out. The apparel kind has no examples in this release.
    #[must_use]
    pub fn from_model(model: &CeModel, kind: ItemKind) -> Self {
        let prefix = model.classes.tag_prefix.as_str();
        let examples = match kind {
            ItemKind::Ranged => model
                .guns
                .iter()
                .filter(|g| g.excluded.is_none())
                .map(|g| gun_example(g, prefix))
                .collect(),
            ItemKind::Melee => model
                .melee
                .iter()
                .filter(|m| m.excluded.is_none())
                .map(|m| melee_example(m, prefix))
                .collect(),
            ItemKind::Apparel => Vec::new(),
        };
        Self { kind, examples }
    }

    /// The set without the example `id` (leave one out).
    #[must_use]
    pub fn without(&self, id: &str) -> Self {
        Self {
            kind: self.kind,
            examples: self
                .examples
                .iter()
                .filter(|e| e.id != id)
                .cloned()
                .collect(),
        }
    }

    /// Number of examples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// True when there is no example.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }
}

/// The profile an example presents when it is held out (what its twin would tell a designer).
#[must_use]
pub fn profile_of(example: &Example, with_role: bool) -> Profile {
    Profile {
        tier: example.tier,
        tags: example.tags.clone(),
        role: if with_role {
            example.role.clone()
        } else {
            None
        },
        vanilla: example.vanilla.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Similarity
// ---------------------------------------------------------------------------------------------------------

/// A strength proxy of a vanilla weapon on a logarithmic scale: shots per second times the square root of
/// the range for a gun, power over cooldown for a melee weapon. Only terciles of it are used, as a tie
/// break.
fn strength_proxy(v: &BTreeMap<String, f64>) -> Option<f64> {
    let pos = |name: &str| v.get(name).copied().filter(|x| x.is_finite() && *x > 0.0);
    if let (Some(range), Some(cooldown)) = (pos("range"), pos("cooldown")) {
        let cycle = cooldown + pos("warmup").unwrap_or(0.0);
        let burst = pos("burst").unwrap_or(1.0);
        return Some((burst / cycle).ln() + 0.5 * range.ln());
    }
    let (power, cooldown) = (pos("tool_power")?, pos("tool_cooldown")?);
    Some((power / cooldown).ln())
}

fn shape_of(v: &BTreeMap<String, f64>) -> Option<bool> {
    v.get("burst").copied().map(|b| b > 1.0)
}

/// The comparable features of an example or a profile.
struct Features {
    tier: Option<u8>,
    tags: BTreeSet<String>,
    role: Option<String>,
    shape: Option<bool>,
    tercile: Option<u8>,
}

struct Space {
    weights: BTreeMap<String, f64>,
    unknown_weight: f64,
    cuts: Option<(f64, f64)>,
}

impl Space {
    fn new(examples: &[Example]) -> Self {
        let n = examples.len() as f64;
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in examples {
            let tags: BTreeSet<&String> = e.tags.iter().collect();
            for t in tags {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
        let weights = counts
            .into_iter()
            .map(|(t, c)| (t, ((n + 1.0) / (c as f64 + 0.5)).ln()))
            .collect();
        let mut proxies: Vec<f64> = examples
            .iter()
            .filter_map(|e| strength_proxy(&e.vanilla))
            .collect();
        proxies.sort_by(f64::total_cmp);
        let cuts = (proxies.len() >= 3).then(|| {
            let at = |i: usize| proxies.get(i).copied().unwrap_or(0.0);
            (at(proxies.len() / 3), at(2 * proxies.len() / 3))
        });
        Self {
            weights,
            unknown_weight: ((n + 1.0) / 0.5).ln(),
            cuts,
        }
    }

    fn features(
        &self,
        tier: Option<u8>,
        tags: &[String],
        role: Option<&String>,
        vanilla: &BTreeMap<String, f64>,
    ) -> Features {
        let tercile = self.cuts.and_then(|(lo, hi)| {
            let s = strength_proxy(vanilla)?;
            Some(u8::from(s >= lo) + u8::from(s >= hi))
        });
        Features {
            tier,
            tags: tags.iter().cloned().collect(),
            role: role.cloned(),
            shape: shape_of(vanilla),
            tercile,
        }
    }

    fn weight(&self, tag: &str) -> f64 {
        self.weights
            .get(tag)
            .copied()
            .unwrap_or(self.unknown_weight)
    }

    fn tag_dissimilarity(&self, a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
        let shared: f64 = a.intersection(b).map(|t| self.weight(t)).sum();
        let all: f64 = a.union(b).map(|t| self.weight(t)).sum();
        if all > 0.0 { 1.0 - shared / all } else { 1.0 }
    }

    fn distance(&self, a: &Features, b: &Features, o: &EstimateOptions) -> f64 {
        let mut d = o.w_tag * self.tag_dissimilarity(&a.tags, &b.tags);
        if let (Some(x), Some(y)) = (&a.role, &b.role)
            && x != y
        {
            d += o.w_role;
        }
        if let (Some(x), Some(y)) = (a.shape, b.shape)
            && x != y
        {
            d += o.w_shape;
        }
        if let (Some(x), Some(y)) = (a.tier, b.tier) {
            d += o.w_tier * f64::from(x.abs_diff(y));
        }
        if let (Some(x), Some(y)) = (a.tercile, b.tercile) {
            d += o.w_tercile * f64::from(x.abs_diff(y));
        }
        d
    }
}

/// Indices of the examples ordered by distance to `from`, ties by id; `skip` leaves one example out.
fn neighbours(
    space: &Space,
    examples: &[Example],
    feats: &[Features],
    from: &Features,
    skip: Option<usize>,
    o: &EstimateOptions,
) -> Vec<usize> {
    let mut order: Vec<(f64, &str, usize)> = feats
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != skip)
        .filter_map(|(i, f)| Some((space.distance(from, f, o), examples.get(i)?.id.as_str(), i)))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    order.into_iter().map(|(_, _, i)| i).collect()
}

// ---------------------------------------------------------------------------------------------------------
// Forms and leave one out
// ---------------------------------------------------------------------------------------------------------

/// A form of estimate, in tie break order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Identity,
    Ratio,
    Median,
}

impl Form {
    fn predictor(self) -> ConversionPredictor {
        match self {
            Self::Identity => ConversionPredictor::Identity,
            Self::Median => ConversionPredictor::Median,
            Self::Ratio => ConversionPredictor::Ratio,
        }
    }
}

fn finite_positive(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
}

/// Median of the logarithms, back transformed (the geometric median of positive numbers).
fn geometric_median(values: &[f64]) -> Option<f64> {
    let logs: Vec<f64> = values
        .iter()
        .filter(|v| **v > 0.0)
        .map(|v| v.ln())
        .collect();
    median(&logs).map(f64::exp)
}

/// The class and pool numbers of one stat.
struct StatView<'a> {
    set: &'a [Example],
    stat: &'a str,
    driver: Option<&'a str>,
}

impl StatView<'_> {
    fn y(&self, i: usize) -> Option<f64> {
        self.set
            .get(i)?
            .stats
            .get(self.stat)
            .copied()
            .filter(|v| v.is_finite())
    }

    fn x(&self, i: usize) -> Option<f64> {
        finite_positive(self.set.get(i)?.vanilla.get(self.driver?).copied())
    }

    fn ratio(&self, i: usize) -> Option<f64> {
        let y = finite_positive(self.y(i))?;
        Some(y / self.x(i)?)
    }
}

struct Estimator<'a> {
    examples: &'a [Example],
    /// For every example, the other examples ordered by distance (the class order of a held out fold).
    fold_orders: Vec<Vec<usize>>,
    opts: &'a EstimateOptions,
}

impl Estimator<'_> {
    /// The estimate of `form` for a design with vanilla driver value `x`, from the class of the neighbours
    /// `order` (nearest first), leaving `skip` out. `None` when fewer than the minimum class carry what the
    /// form needs.
    fn estimate(
        &self,
        form: Form,
        view: &StatView<'_>,
        order: &[usize],
        skip: Option<usize>,
        x: Option<f64>,
    ) -> Option<(f64, usize)> {
        let o = self.opts;
        let k = o.shrink.max(0.0);
        match form {
            Form::Identity => Some((x?, 0)),
            Form::Median => {
                let carriers = |i: &usize| Some(*i) != skip && view.y(*i).is_some();
                let class: Vec<f64> = order
                    .iter()
                    .filter(|i| carriers(i))
                    .take(o.class_size)
                    .filter_map(|i| view.y(*i))
                    .collect();
                if class.len() < o.min_class {
                    return None;
                }
                // No shrinkage: an absolute number is constant inside a good class, and pulling it toward
                // the pool only moves it away from the class.
                Some((median(&class)?, class.len()))
            }
            Form::Ratio => {
                let x = x?;
                let carriers = |i: &usize| Some(*i) != skip && view.ratio(*i).is_some();
                let class: Vec<f64> = order
                    .iter()
                    .filter(|i| carriers(i))
                    .take(o.class_size)
                    .filter_map(|i| view.ratio(*i))
                    .collect();
                if class.len() < o.min_class {
                    return None;
                }
                let all: Vec<f64> = (0..self.examples.len())
                    .filter(|i| carriers(i))
                    .filter_map(|i| view.ratio(i))
                    .collect();
                let (c, g) = (geometric_median(&class)?, geometric_median(&all)?);
                let n = class.len() as f64;
                let log_ratio = (n * c.ln() + k * g.ln()) / (n + k);
                Some((x * log_ratio.exp(), class.len()))
            }
        }
    }

    /// Leave one out: relative errors and log deviations of `form` over the examples that carry the stat.
    fn loo(&self, form: Form, view: &StatView<'_>) -> (Vec<f64>, Vec<f64>) {
        let mut rel = Vec::new();
        let mut dev = Vec::new();
        for (i, order) in self.fold_orders.iter().enumerate() {
            let Some(y) = finite_positive(view.y(i)) else {
                continue;
            };
            let x = view.x(i);
            if matches!(form, Form::Identity | Form::Ratio) && x.is_none() {
                continue;
            }
            let Some((p, _)) = self.estimate(form, view, order, Some(i), x) else {
                continue;
            };
            if p.is_finite() && p > 0.0 {
                rel.push((p - y).abs() / y);
                dev.push((p / y).ln().abs());
            }
        }
        (rel, dev)
    }
}

/// The level that describes how pure the class of a prediction is: the members all share the design's tag
/// class (and tier), or its tier, or nothing in particular.
fn class_level(profile: &Profile, members: &[&Example]) -> ChainLevel {
    if members.is_empty() {
        return ChainLevel::All;
    }
    let same_tier = profile.tier.is_some() && members.iter().all(|m| m.tier == profile.tier);
    let same_role = profile.role.is_some() && members.iter().all(|m| m.role == profile.role);
    match (same_role, same_tier) {
        (true, true) => ChainLevel::RoleTier,
        (true, false) => ChainLevel::Role,
        (false, true) => ChainLevel::Tier,
        (false, false) => ChainLevel::All,
    }
}

/// Estimates the converted numbers of a vanilla design from the conversions in `set`.
///
/// Per stat the form (identity, median or ratio) is chosen by leave one out error; the result carries the
/// band of that measurement and a [`Reliability`]. Stats that fewer than [`EstimateOptions::min_class`]
/// conversions carry are left out. The same inputs always give the same outputs.
#[must_use]
pub fn estimate_conversion(
    set: &ExampleSet,
    profile: &Profile,
    opts: &EstimateOptions,
) -> ConversionPrediction {
    let examples = set.examples.as_slice();
    let space = Space::new(examples);
    let feats: Vec<Features> = examples
        .iter()
        .map(|e| space.features(e.tier, &e.tags, e.role.as_ref(), &e.vanilla))
        .collect();
    let target = space.features(
        profile.tier,
        &profile.tags,
        profile.role.as_ref(),
        &profile.vanilla,
    );
    let order = neighbours(&space, examples, &feats, &target, None, opts);
    let fold_orders: Vec<Vec<usize>> = feats
        .iter()
        .enumerate()
        .map(|(i, f)| neighbours(&space, examples, &feats, f, Some(i), opts))
        .collect();
    let est = Estimator {
        examples,
        fold_orders,
        opts,
    };
    let mut notes = Vec::new();
    let twins = examples.iter().filter(|e| !e.vanilla.is_empty()).count();
    if examples.len() < opts.min_class {
        notes.push(format!(
            "only {} converted weapons of this kind: nothing is predicted (at least {} are needed)",
            examples.len(),
            opts.min_class
        ));
    } else if twins < opts.min_class {
        notes.push(
            "too few converted weapons have a vanilla twin: only class medians are possible"
                .to_owned(),
        );
    }
    let names: BTreeSet<&String> = examples.iter().flat_map(|e| e.stats.keys()).collect();
    let mut stats = BTreeMap::new();
    let mut withheld = BTreeMap::new();
    let top: Vec<&Example> = order
        .iter()
        .take(opts.class_size)
        .filter_map(|i| examples.get(*i))
        .collect();
    let level = class_level(profile, &top);
    let class_label = format!(
        "based on the {} nearest of {} converted weapons",
        top.len(),
        examples.len()
    );
    for name in names {
        if !is_converted_stat(name) || opts.skip.contains(name) {
            continue;
        }
        let driver = driver_of(name);
        let view = StatView {
            set: examples,
            stat: name,
            driver,
        };
        let x = driver.and_then(|d| finite_positive(profile.vanilla.get(d).copied()));
        let mut forms = Vec::new();
        if x.is_some() && driver == Some(name.as_str()) {
            forms.push(Form::Identity);
        }
        if x.is_some() {
            forms.push(Form::Ratio);
        }
        forms.push(Form::Median);
        let mut best: Option<(Form, f64, f64, Vec<f64>, usize)> = None;
        for form in &forms {
            let (rel, dev) = est.loo(*form, &view);
            if rel.len() < opts.min_measured {
                continue;
            }
            let Some(err) = median(&rel) else { continue };
            if best.as_ref().is_none_or(|(_, e, _, _, _)| {
                err < *e * (1.0 - opts.switch_margin.clamp(0.0, 0.9)) - 1e-12
            }) {
                best = Some((
                    *form,
                    err,
                    quantile(&rel, 0.8).unwrap_or(f64::NAN),
                    dev,
                    rel.len(),
                ));
            }
        }
        // Without a measurement the form that uses the vanilla number is the better guess.
        let (form, error, p80, dev, folds) = match best {
            Some((f, e, p, d, n)) => (f, Some(e), Some(p), d, n),
            None => (
                forms
                    .iter()
                    .copied()
                    .find(|f| *f != Form::Identity)
                    .unwrap_or(Form::Median),
                None,
                None,
                Vec::new(),
                0,
            ),
        };
        let Some((mut value, n)) = est.estimate(form, &view, &order, None, x) else {
            let carriers = (0..examples.len()).filter(|i| view.y(*i).is_some()).count();
            withheld.insert(
                name.clone(),
                format!(
                    "only {carriers} of {} converted weapons carry it; at least {} are needed for a class",
                    examples.len(),
                    opts.min_class
                ),
            );
            continue;
        };
        if opts.integer_stats.contains(name) {
            value = value.round().max(1.0);
        }
        if !value.is_finite() {
            continue;
        }
        let members: Vec<&Example> = order
            .iter()
            .filter(|i| match form {
                Form::Ratio => view.ratio(**i).is_some(),
                _ => view.y(**i).is_some(),
            })
            .take(opts.class_size)
            .filter_map(|i| examples.get(*i))
            .collect();
        let stat_level = class_level(profile, &members);
        let reliability = opts.rate(error, p80, folds);
        stats.insert(
            name.clone(),
            StatPrediction {
                value,
                predictor: form.predictor(),
                driver: driver.filter(|_| x.is_some()).map(str::to_owned),
                band: if dev.len() >= 3 {
                    band_from_residuals(&dev, 0.0)
                } else {
                    Band::fallback()
                },
                n: if form == Form::Identity { folds } else { n },
                level: stat_level,
                loo_error: error,
                reliability,
                pool: (0..examples.len()).filter(|i| view.y(*i).is_some()).count(),
            },
        );
    }
    ConversionPrediction {
        class_label,
        level,
        stats,
        notes,
        withheld,
    }
}

/// Leave one out of the whole procedure: every example is predicted from the others and the results are
/// handed to `visit` with the example. Used by the evaluation harness and by tests.
pub fn leave_one_out(
    set: &ExampleSet,
    opts: &EstimateOptions,
    with_role: bool,
    mut visit: impl FnMut(&Example, &ConversionPrediction),
) {
    for e in &set.examples {
        let rest = set.without(&e.id);
        let prediction = estimate_conversion(&rest, &profile_of(e, with_role), opts);
        visit(e, &prediction);
    }
}

#[cfg(test)]
mod tests;
