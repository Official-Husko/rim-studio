//! The calibration quiz as a serialisable state machine.
//!
//! The quiz has two phases (section 8 of the math specification). Phase 1 finds the anchors: setup taps for
//! tier and role, then a comparison dialogue over the reference items sorted by strength. Phase 2 refines:
//! optional questions on single stats. Every answer is exactly one of three kinds of constraint: a pool
//! selector (tier, role, group), a strength position (the bracket found by the dialogue) or a constraint on
//! one stat (an interval, a typed number or a ratio bucket against a named anchor).
//!
//! A [`Quiz`] stores only its configuration and the list of answers, so a draft can persist it as JSON and
//! reopen it identically. The state (bracket, constraints, class key) is rebuilt by replaying the answers
//! against a [`Model`]; each stored answer carries the id of the question it answered, and a replay that
//! meets a different question reports [`QuizError::Stale`] instead of applying an answer to the wrong
//! question. The next comparison is chosen by information gain: the pivot whose three outcomes (weaker,
//! about the same, stronger) have the highest entropy under the normal prior on ln strength, restricted to
//! the current bracket; ties go to the pivot closest to the middle of the bracket, then to the weaker one.
//! The estimate is recomputed from the answers after each step, so the quiz never gates anything: back,
//! skip, "not sure" and "use what I have" are always available.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::baseline::{
    Anchor, BandTable, BaselineInput, Bucket, Estimate, Model, StatConstraint, StrengthInput,
};
use crate::classes::numeric::{median, quantile_sorted};
use crate::classes::{ClassKey, ItemKind, StatSummary};
use crate::error::DesignError;

/// Hard stop of [`Quiz::play`] against an oracle that never finishes.
const MAX_QUESTIONS: usize = 64;

/// Version of the stored quiz layout.
pub const QUIZ_VERSION: u32 = 1;

/// Result alias of the quiz.
pub type QuizResult<T> = Result<T, QuizError>;

/// Errors of the quiz.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum QuizError {
    /// The answer does not fit the question that is open.
    #[error("the answer does not fit the open question `{question}`")]
    WrongAnswer {
        /// Id of the open question.
        question: String,
    },
    /// The quiz has no open question (it is finished).
    #[error("the quiz has no open question")]
    Finished,
    /// A stored answer no longer matches the question the model asks at that point (the pool changed).
    #[error("stored answer {index} no longer matches the question the model asks")]
    Stale {
        /// Position of the first answer that no longer fits.
        index: usize,
    },
    /// An answer names something the pool does not contain (a tier, a role or a group).
    #[error("unknown choice `{what}`")]
    UnknownChoice {
        /// What was named.
        what: String,
    },
    /// An error of the underlying estimation.
    #[error(transparent)]
    Design(#[from] DesignError),
}

impl QuizError {
    /// Stable code string.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::WrongAnswer { .. } => "quiz.wrong-answer",
            Self::Finished => "quiz.finished",
            Self::Stale { .. } => "quiz.stale",
            Self::UnknownChoice { .. } => "quiz.unknown-choice",
            Self::Design(e) => e.code(),
        }
    }
}

/// How the comparison list is scoped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonScope {
    /// Every item of the kind (weapons).
    Kind,
    /// Only items of the chosen role when it has at least four (apparel, where strength is not comparable
    /// across a vest and a hat).
    Role,
}

/// One optional refine question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RefineSpec {
    /// "How does it fire?": a group selector.
    Group,
    /// An interval question on a stat (answer options are the quartile bins of the class).
    Interval {
        /// Stat name.
        stat: String,
    },
    /// "Compared with {anchor}, is it lighter, similar or heavier?" on a stat.
    VsAnchor {
        /// Stat name.
        stat: String,
    },
}

impl RefineSpec {
    fn key(&self) -> String {
        match self {
            Self::Group => "group".to_string(),
            Self::Interval { stat } => format!("interval:{stat}"),
            Self::VsAnchor { stat } => format!("vs:{stat}"),
        }
    }
}

/// Configuration of a quiz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuizConfig {
    /// Most comparison questions (the closer-to question counts): 3 for weapons, 5 for apparel.
    pub comparison_budget: usize,
    /// How the comparison list is scoped.
    pub scope: ComparisonScope,
    /// Candidate refine questions in priority order.
    pub refine: Vec<RefineSpec>,
    /// Most refine questions.
    pub max_refine: usize,
}

impl QuizConfig {
    /// The default configuration of a kind. The stat names are the conventional ones of the reader
    /// (`mass`, `range`, `warmup`, `cooldown`, `insulationCold`); the lists are data and can be replaced.
    #[must_use]
    pub fn for_kind(kind: ItemKind) -> Self {
        let interval = |s: &str| RefineSpec::Interval {
            stat: s.to_string(),
        };
        let vs = |s: &str| RefineSpec::VsAnchor {
            stat: s.to_string(),
        };
        match kind {
            ItemKind::Ranged => Self {
                comparison_budget: 3,
                scope: ComparisonScope::Kind,
                refine: vec![
                    RefineSpec::Group,
                    interval("warmup"),
                    interval("range"),
                    vs("mass"),
                ],
                max_refine: 4,
            },
            ItemKind::Melee => Self {
                comparison_budget: 3,
                scope: ComparisonScope::Kind,
                refine: vec![interval("cooldown"), vs("mass")],
                max_refine: 2,
            },
            ItemKind::Apparel => Self {
                comparison_budget: 5,
                scope: ComparisonScope::Role,
                refine: vec![vs("mass"), vs("insulationCold")],
                max_refine: 2,
            },
        }
    }
}

/// An answer to the open question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Answer {
    /// Setup: the tier.
    Tier {
        /// Tier value.
        tier: u8,
    },
    /// Setup: the role.
    Role {
        /// Role name.
        role: String,
    },
    /// Refine: the group (fire class).
    Group {
        /// Group name.
        group: String,
    },
    /// Comparison: weaker than the shown item.
    Weaker,
    /// Comparison: about the same as the shown item.
    Same,
    /// Comparison: stronger than the shown item.
    Stronger,
    /// Closer-to question: closer to the lower item.
    CloserToLower,
    /// Closer-to question: closer to the upper item.
    CloserToUpper,
    /// Interval question: index of the chosen bin.
    Bin {
        /// Bin index.
        index: usize,
    },
    /// Anchor question: the ratio bucket.
    Bucket {
        /// The bucket.
        bucket: Bucket,
    },
    /// A typed number for a stat question.
    Typed {
        /// The number.
        value: f64,
    },
    /// "Not sure": the question is skipped, the predictor falls back and the band widens.
    NotSure,
    /// Skip this question.
    Skip,
    /// "Use what I have": finish now with the current estimate.
    UseWhatIHave,
}

/// A stored answer with the id of the question it answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerEntry {
    /// Id of the question (see [`Question::id`]).
    pub question: String,
    /// The answer.
    pub answer: Answer,
}

/// A reference item shown on a card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorCard {
    /// Id of the item.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Strength index.
    pub strength: f64,
    /// Tier.
    pub tier: u8,
    /// Role.
    pub role: String,
    /// The stats of the item (the index components with real numbers).
    pub stats: BTreeMap<String, f64>,
}

impl AnchorCard {
    fn of(model: &Model, pos: usize) -> Option<Self> {
        let item = model.pool.item(pos)?;
        Some(Self {
            id: item.id.clone(),
            label: item.label.clone(),
            strength: item.strength,
            tier: item.tier,
            role: item.role.clone(),
            stats: item.stats.clone(),
        })
    }

    fn of_anchor(a: &Anchor) -> Self {
        Self {
            id: a.id.clone(),
            label: a.label.clone(),
            strength: a.strength,
            tier: a.tier,
            role: a.role.clone(),
            stats: a.stats.clone(),
        }
    }
}

/// One tier choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierOption {
    /// Tier value.
    pub tier: u8,
    /// Display text.
    pub label: String,
    /// Number of reference items of the tier.
    pub count: usize,
}

/// One role or group choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedOption {
    /// Name of the role or group.
    pub name: String,
    /// Number of reference items.
    pub count: usize,
}

/// One bin of an interval question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bin {
    /// Inclusive lower edge, or open.
    pub lo: Option<f64>,
    /// Exclusive upper edge, or open.
    pub hi: Option<f64>,
    /// Display text such as "under 0.5", "0.5 to 1.2" or "over 2.5".
    pub label: String,
}

/// A question of the quiz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Question {
    /// Which tech level is it?
    Tier {
        /// Tiers present in the pool.
        options: Vec<TierOption>,
    },
    /// What is it?
    Role {
        /// Roles present in the pool.
        options: Vec<NamedOption>,
    },
    /// Is your item weaker, about the same, or stronger than the anchor?
    Compare {
        /// The pivot item.
        anchor: AnchorCard,
        /// Items still in the bracket.
        remaining: usize,
        /// Comparison questions asked so far.
        asked: usize,
        /// Comparison budget.
        budget: usize,
        /// Entropy (nats) of the three outcomes under the prior: the information the answer can give.
        information: f64,
    },
    /// Is it closer to the lower or the upper item?
    CloserTo {
        /// Lower neighbour.
        lower: AnchorCard,
        /// Upper neighbour.
        upper: AnchorCard,
    },
    /// How does it fire?
    Group {
        /// Groups present in the pool.
        options: Vec<NamedOption>,
    },
    /// Which interval does a stat fall into?
    Interval {
        /// Stat name.
        stat: String,
        /// Answer bins, ascending.
        bins: Vec<Bin>,
        /// Spread of the stat in the class (`ln(P90 / P10)`).
        spread: f64,
    },
    /// Compared with the anchor, is the stat lower, similar or higher?
    VsAnchor {
        /// Stat name.
        stat: String,
        /// The anchor card.
        anchor: AnchorCard,
        /// The anchor's value of the stat.
        value: f64,
    },
}

impl Question {
    /// Stable id of the question, stored with its answer. It names the items and edges shown so that a
    /// changed pool is detected.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Tier { .. } => "tier".to_string(),
            Self::Role { .. } => "role".to_string(),
            Self::Compare { anchor, .. } => format!("cmp:{}", anchor.id),
            Self::CloserTo { lower, upper } => format!("closer:{}:{}", lower.id, upper.id),
            Self::Group { .. } => "group".to_string(),
            Self::Interval { stat, bins, .. } => {
                let edges: Vec<String> =
                    bins.iter().filter_map(|b| b.hi).map(format_edge).collect();
                format!("bin:{stat}:{}", edges.join("|"))
            }
            Self::VsAnchor { stat, anchor, .. } => format!("vs:{stat}:{}", anchor.id),
        }
    }
}

/// An open question with its position in the quiz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    /// Id of the question.
    pub id: String,
    /// The question.
    pub question: Question,
    /// One based number of this question.
    pub number: usize,
    /// "About m": the estimated total number of questions, recomputed from the bracket and the pool.
    pub about_total: usize,
}

/// The comparison bracket: positions in the comparison list known to hold the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bracket {
    /// Position of the greatest item known to be weaker (-1 for none).
    pub lo: i64,
    /// Position of the least item known to be stronger (list length for none).
    pub hi: i64,
}

/// The state rebuilt by replaying the answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuizState {
    /// The class selectors answered (or implied by a single choice).
    pub key: ClassKey,
    /// The comparison bracket once the dialogue started.
    pub bracket: Option<Bracket>,
    /// Comparison questions answered.
    pub comparisons_used: usize,
    /// Strength fixed exactly by an "about the same" answer, as ln strength.
    pub placed: Option<f64>,
    /// Quarter position set by the closer-to answer: 0.25, 0.5 or 0.75.
    pub closer: Option<f64>,
    /// The dialogue has ended.
    pub dialogue_done: bool,
    /// Constraints on single stats.
    pub constraints: BTreeMap<String, StatConstraint>,
    /// Refine questions asked (answered or skipped), by spec key.
    pub refined: BTreeSet<String>,
    /// The user chose to use what they have.
    pub finished: bool,
    tier_done: bool,
    role_done: bool,
}

/// The quiz: configuration plus the answers given so far.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quiz {
    /// Layout version.
    pub version: u32,
    /// Kind of the item.
    pub kind: ItemKind,
    /// Configuration.
    pub config: QuizConfig,
    /// The answers, oldest first.
    pub answers: Vec<AnswerEntry>,
}

// ---------------------------------------------------------------------------------------------------
// Small numeric helpers
// ---------------------------------------------------------------------------------------------------

/// Standard normal cumulative distribution (Abramowitz and Stegun 7.1.26, error below 2e-7).
fn normal_cdf(z: f64) -> f64 {
    let x = z / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    0.5 * (1.0 + if x >= 0.0 { erf } else { -erf })
}

fn entropy(probs: &[f64]) -> f64 {
    probs
        .iter()
        .filter(|p| **p > 1e-12)
        .map(|p| -p * p.ln())
        .sum()
}

/// Rounds to `digits` significant digits.
fn round_sig(v: f64, digits: i32) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let scale = 10f64.powi(digits - 1 - v.abs().log10().floor() as i32);
    (v * scale).round() / scale
}

fn format_edge(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Bins of an interval question: quartile edges of the class values rounded to two significant digits.
fn bins_for(summary: &StatSummary) -> Vec<Bin> {
    let mut edges: Vec<f64> = [0.25, 0.5, 0.75]
        .iter()
        .filter_map(|q| quantile_sorted(&summary.values, *q))
        .map(|v| round_sig(v, 2))
        .collect();
    edges.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    if edges.is_empty() {
        return Vec::new();
    }
    let mut bins = Vec::with_capacity(edges.len() + 1);
    for (i, edge) in edges.iter().enumerate() {
        let lo = if i == 0 {
            None
        } else {
            edges.get(i - 1).copied()
        };
        let label = match lo {
            None => format!("under {}", format_edge(*edge)),
            Some(l) => format!("{} to {}", format_edge(l), format_edge(*edge)),
        };
        bins.push(Bin {
            lo,
            hi: Some(*edge),
            label,
        });
    }
    if let Some(last) = edges.last() {
        bins.push(Bin {
            lo: Some(*last),
            hi: None,
            label: format!("over {}", format_edge(*last)),
        });
    }
    bins
}

// ---------------------------------------------------------------------------------------------------
// The state machine
// ---------------------------------------------------------------------------------------------------

impl Quiz {
    /// A new quiz with no answers.
    #[must_use]
    pub fn new(kind: ItemKind, config: QuizConfig) -> Self {
        Self {
            version: QUIZ_VERSION,
            kind,
            config,
            answers: Vec::new(),
        }
    }

    /// A new quiz with the default configuration of the kind.
    #[must_use]
    pub fn for_kind(kind: ItemKind) -> Self {
        Self::new(kind, QuizConfig::for_kind(kind))
    }

    fn initial_state(&self, model: &Model) -> QuizState {
        let mut key = ClassKey::default();
        let tiers = model.pool.tiers();
        let mut tier_done = false;
        if let [only] = tiers.as_slice() {
            key.tier = Some(*only);
            tier_done = true;
        }
        let roles = model.pool.roles();
        let mut role_done = false;
        if let [(only, _)] = roles.as_slice() {
            key.role = Some(only.clone());
            role_done = true;
        }
        QuizState {
            key,
            bracket: None,
            comparisons_used: 0,
            placed: None,
            closer: None,
            dialogue_done: model.pool.len() < 2 || self.config.comparison_budget == 0,
            constraints: BTreeMap::new(),
            refined: BTreeSet::new(),
            finished: false,
            tier_done,
            role_done,
        }
    }

    /// The comparison list: pool positions ascending by strength.
    fn comparison_list(&self, model: &Model, key: &ClassKey) -> Vec<usize> {
        if self.config.scope == ComparisonScope::Role
            && let Some(role) = &key.role
        {
            let same: Vec<usize> = model
                .pool
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| i.role == *role)
                .map(|(p, _)| p)
                .collect();
            if same.len() >= 4 {
                return same;
            }
        }
        (0..model.pool.len()).collect()
    }

    fn list_ln(model: &Model, list: &[usize]) -> Vec<f64> {
        list.iter()
            .filter_map(|p| model.pool.item(*p))
            .map(|i| i.ln_strength)
            .collect()
    }

    /// The pivot with the highest information gain inside the bracket.
    fn pick_pivot(
        &self,
        model: &Model,
        state: &QuizState,
        ln: &[f64],
        bracket: Bracket,
    ) -> Option<(usize, f64)> {
        let m = ln.len() as i64;
        let band = model.config.same_band;
        let sd = model.config.prior_sd.max(1e-6);
        let mu = model.prior_ln_strength(&state.key);
        let cdf = |x: f64| normal_cdf((x - mu) / sd);
        let a = if bracket.lo >= 0 {
            ln.get(bracket.lo as usize).copied()
        } else {
            None
        };
        let b = if bracket.hi < m {
            ln.get(bracket.hi as usize).copied()
        } else {
            None
        };
        let (ca, cb) = (a.map_or(0.0, cdf), b.map_or(1.0, cdf));
        let total = (cb - ca).max(1e-12);
        let clamp = |x: f64| a.map_or(x, |a| x.max(a)).min(b.unwrap_or(f64::INFINITY));
        let mid = (bracket.lo + bracket.hi) as f64 / 2.0;
        let mut best: Option<(usize, f64, f64)> = None;
        for k in (bracket.lo + 1)..bracket.hi {
            let Some(lk) = ln.get(k as usize).copied() else {
                continue;
            };
            let lower = cdf(clamp(lk - band));
            let upper = cdf(clamp(lk + band));
            let probs = [
                (lower - ca) / total,
                (upper - lower) / total,
                (cb - upper) / total,
            ];
            let h = entropy(&probs);
            let dist = (k as f64 - mid).abs();
            let better = match best {
                None => true,
                Some((_, bh, bd)) => h > bh + 1e-9 || ((h - bh).abs() <= 1e-9 && dist < bd - 1e-12),
            };
            if better {
                best = Some((k as usize, h, dist));
            }
        }
        best.map(|(k, h, _)| (k, h))
    }

    fn refine_candidate(&self, model: &Model, state: &QuizState) -> Option<Question> {
        if state.refined.len() >= self.config.max_refine {
            return None;
        }
        let mass_stat = model.config.derive.mass_stat.as_deref();
        let mut best: Option<(f64, usize, Question)> = None;
        for (order, spec) in self.config.refine.iter().enumerate() {
            if state.refined.contains(&spec.key()) {
                continue;
            }
            match spec {
                RefineSpec::Group => {
                    let groups = model.pool.groups();
                    if groups.len() < 2 || state.key.group.is_some() {
                        continue;
                    }
                    let options = groups
                        .into_iter()
                        .map(|(name, count)| NamedOption { name, count })
                        .collect();
                    // The group selector is a pool selector: it goes first.
                    return Some(Question::Group { options });
                }
                RefineSpec::Interval { stat } | RefineSpec::VsAnchor { stat } => {
                    if state.constraints.contains_key(stat) {
                        continue;
                    }
                    let elasticity = model.elasticity.get(stat).copied().unwrap_or(0.0);
                    let Some(summary) = StatSummary::derive(
                        &model.pool,
                        &state.key,
                        stat,
                        None,
                        &model.config.derive,
                        elasticity,
                    ) else {
                        continue;
                    };
                    let is_mass = mass_stat == Some(stat.as_str());
                    if summary.n < model.config.derive.min_pool || !(summary.varies() || is_mass) {
                        continue;
                    }
                    let question = match spec {
                        RefineSpec::Interval { .. } => {
                            let bins = bins_for(&summary);
                            if bins.len() < 2 {
                                continue;
                            }
                            Question::Interval {
                                stat: stat.clone(),
                                bins,
                                spread: summary.spread,
                            }
                        }
                        _ => {
                            let ln_p = self.current_ln_strength(model, state);
                            let anchor = model
                                .ranked(&state.key, ln_p, None)
                                .into_iter()
                                .filter_map(|(i, d)| model.pool.item(i).map(|it| (it, d)))
                                .find(|(it, _)| it.stat(stat).is_some_and(|v| v > 0.0));
                            let Some((item, d)) = anchor else { continue };
                            let card = AnchorCard::of_anchor(&Anchor::from_item(item, d));
                            let value = item.stat(stat).unwrap_or(0.0);
                            Question::VsAnchor {
                                stat: stat.clone(),
                                anchor: card,
                                value,
                            }
                        }
                    };
                    let spread = summary.spread;
                    let better = match &best {
                        None => true,
                        Some((bs, bo, _)) => {
                            spread > *bs + 1e-12 || ((spread - *bs).abs() <= 1e-12 && order < *bo)
                        }
                    };
                    if better {
                        best = Some((spread, order, question));
                    }
                }
            }
        }
        best.map(|(_, _, q)| q)
    }

    /// Ln strength the quiz currently assumes (placed, interpolated, or the prior).
    fn current_ln_strength(&self, model: &Model, state: &QuizState) -> f64 {
        self.strength_from_state(model, state)
            .and_then(|s| match s {
                StrengthInput::Placed { ln_power } => Some(ln_power),
                _ => None,
            })
            .unwrap_or_else(|| model.prior_ln_strength(&state.key))
    }

    fn strength_from_state(&self, model: &Model, state: &QuizState) -> Option<StrengthInput> {
        if let Some(p) = state.placed {
            return Some(StrengthInput::Placed { ln_power: p });
        }
        let bracket = state.bracket?;
        let list = self.comparison_list(model, &state.key);
        let ln = Self::list_ln(model, &list);
        let m = ln.len() as i64;
        if m == 0 || (bracket.lo < 0 && bracket.hi >= m) {
            return None;
        }
        let gaps: Vec<f64> = ln.windows(2).map(|w| w[1] - w[0]).collect();
        let gap = median(&gaps).filter(|g| *g > 0.0).unwrap_or(0.2);
        let at = |i: i64| ln.get(usize::try_from(i).ok()?).copied();
        let (a, b, t) = if bracket.hi - bracket.lo == 1 {
            let a = at(bracket.lo).unwrap_or_else(|| ln.first().copied().unwrap_or(0.0) - gap);
            let b = at(bracket.hi).unwrap_or_else(|| ln.last().copied().unwrap_or(0.0) + gap);
            (a, b, state.closer.unwrap_or(0.5))
        } else {
            let a = at(bracket.lo + 1).unwrap_or_else(|| ln.first().copied().unwrap_or(0.0));
            let b = at(bracket.hi - 1).unwrap_or_else(|| ln.last().copied().unwrap_or(0.0));
            (a, b, 0.5)
        };
        Some(StrengthInput::Placed {
            ln_power: a + t * (b - a),
        })
    }

    fn next_question(&self, model: &Model, state: &QuizState) -> Option<Question> {
        if state.finished || model.pool.is_empty() {
            return None;
        }
        if !state.tier_done {
            let options: Vec<TierOption> = model
                .pool
                .tiers()
                .into_iter()
                .map(|tier| TierOption {
                    tier,
                    label: model.pool.tier_text(tier),
                    count: model.pool.items.iter().filter(|i| i.tier == tier).count(),
                })
                .collect();
            return Some(Question::Tier { options });
        }
        if !state.role_done {
            let options = model
                .pool
                .roles()
                .into_iter()
                .map(|(name, count)| NamedOption { name, count })
                .collect();
            return Some(Question::Role { options });
        }
        if !state.dialogue_done {
            let list = self.comparison_list(model, &state.key);
            let ln = Self::list_ln(model, &list);
            let m = ln.len() as i64;
            let bracket = state.bracket.unwrap_or(Bracket { lo: -1, hi: m });
            if state.comparisons_used < self.config.comparison_budget && m >= 2 {
                if bracket.hi - bracket.lo > 1 {
                    if let Some((k, information)) = self.pick_pivot(model, state, &ln, bracket) {
                        let pos = *list.get(k)?;
                        return Some(Question::Compare {
                            anchor: AnchorCard::of(model, pos)?,
                            remaining: (bracket.hi - bracket.lo - 1) as usize,
                            asked: state.comparisons_used,
                            budget: self.config.comparison_budget,
                            information,
                        });
                    }
                } else if bracket.lo >= 0 && bracket.hi < m {
                    let lower = AnchorCard::of(model, *list.get(bracket.lo as usize)?)?;
                    let upper = AnchorCard::of(model, *list.get(bracket.hi as usize)?)?;
                    return Some(Question::CloserTo { lower, upper });
                }
            }
        }
        self.refine_candidate(model, state)
    }

    /// Applies an answer to the state. `question` must be the open question.
    fn apply_to_state(
        &self,
        model: &Model,
        state: &mut QuizState,
        question: &Question,
        answer: &Answer,
    ) -> QuizResult<()> {
        let wrong = || QuizError::WrongAnswer {
            question: question.id(),
        };
        if matches!(answer, Answer::UseWhatIHave) {
            state.finished = true;
            return Ok(());
        }
        let skipped = matches!(answer, Answer::NotSure | Answer::Skip);
        match question {
            Question::Tier { .. } => {
                match answer {
                    Answer::Tier { tier } => {
                        if !model.pool.tiers().contains(tier) {
                            return Err(QuizError::UnknownChoice {
                                what: format!("tier {tier}"),
                            });
                        }
                        state.key.tier = Some(*tier);
                    }
                    _ if skipped => {}
                    _ => return Err(wrong()),
                }
                state.tier_done = true;
            }
            Question::Role { .. } => {
                match answer {
                    Answer::Role { role } => {
                        if !model.pool.roles().iter().any(|(r, _)| r == role) {
                            return Err(QuizError::UnknownChoice { what: role.clone() });
                        }
                        state.key.role = Some(role.clone());
                    }
                    _ if skipped => {}
                    _ => return Err(wrong()),
                }
                state.role_done = true;
            }
            Question::Compare { anchor, .. } => {
                let list = self.comparison_list(model, &state.key);
                let ln = Self::list_ln(model, &list);
                let m = ln.len() as i64;
                let mut bracket = state.bracket.unwrap_or(Bracket { lo: -1, hi: m });
                let pos = list
                    .iter()
                    .position(|p| model.pool.item(*p).is_some_and(|i| i.id == anchor.id))
                    .ok_or(QuizError::Stale { index: 0 })? as i64;
                match answer {
                    Answer::Stronger => {
                        bracket.lo = pos;
                        state.comparisons_used += 1;
                    }
                    Answer::Weaker => {
                        bracket.hi = pos;
                        state.comparisons_used += 1;
                    }
                    Answer::Same => {
                        state.placed = ln.get(pos as usize).copied();
                        state.comparisons_used += 1;
                        state.dialogue_done = true;
                    }
                    _ if skipped => state.dialogue_done = true,
                    _ => return Err(wrong()),
                }
                state.bracket = Some(bracket);
                if state.comparisons_used >= self.config.comparison_budget {
                    state.dialogue_done = true;
                }
            }
            Question::CloserTo { .. } => {
                match answer {
                    Answer::CloserToLower => state.closer = Some(0.25),
                    Answer::CloserToUpper => state.closer = Some(0.75),
                    _ if skipped => state.closer = Some(0.5),
                    _ => return Err(wrong()),
                }
                state.comparisons_used += 1;
                state.dialogue_done = true;
            }
            Question::Group { .. } => {
                match answer {
                    Answer::Group { group } => {
                        if !model.pool.groups().iter().any(|(g, _)| g == group) {
                            return Err(QuizError::UnknownChoice {
                                what: group.clone(),
                            });
                        }
                        state.key.group = Some(group.clone());
                    }
                    _ if skipped => {}
                    _ => return Err(wrong()),
                }
                state.refined.insert("group".to_string());
            }
            Question::Interval { stat, bins, .. } => {
                match answer {
                    Answer::Bin { index } => {
                        let bin = bins.get(*index).ok_or_else(wrong)?;
                        state.constraints.insert(
                            stat.clone(),
                            StatConstraint::Interval {
                                lo: bin.lo,
                                hi: bin.hi,
                            },
                        );
                    }
                    Answer::Typed { value } if value.is_finite() => {
                        state
                            .constraints
                            .insert(stat.clone(), StatConstraint::Typed { value: *value });
                    }
                    _ if skipped => {}
                    _ => return Err(wrong()),
                }
                state.refined.insert(format!("interval:{stat}"));
            }
            Question::VsAnchor { stat, anchor, .. } => {
                match answer {
                    Answer::Bucket { bucket } => {
                        state.constraints.insert(
                            stat.clone(),
                            StatConstraint::VsAnchor {
                                anchor: anchor.id.clone(),
                                bucket: *bucket,
                            },
                        );
                    }
                    Answer::Typed { value } if value.is_finite() => {
                        state
                            .constraints
                            .insert(stat.clone(), StatConstraint::Typed { value: *value });
                    }
                    _ if skipped => {}
                    _ => return Err(wrong()),
                }
                state.refined.insert(format!("vs:{stat}"));
            }
        }
        Ok(())
    }

    /// Rebuilds the state by replaying the answers against `model`.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when a stored answer no longer matches the question the model asks.
    pub fn state(&self, model: &Model) -> QuizResult<QuizState> {
        let mut state = self.initial_state(model);
        for (index, entry) in self.answers.iter().enumerate() {
            let question = self
                .next_question(model, &state)
                .ok_or(QuizError::Stale { index })?;
            if question.id() != entry.question {
                return Err(QuizError::Stale { index });
            }
            self.apply_to_state(model, &mut state, &question, &entry.answer)
                .map_err(|e| {
                    if matches!(e, QuizError::Stale { .. }) {
                        QuizError::Stale { index }
                    } else {
                        e
                    }
                })?;
        }
        Ok(state)
    }

    /// The open question with its number and the "about m" total, or `None` when the quiz is finished.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when the stored answers no longer fit the model.
    pub fn next(&self, model: &Model) -> QuizResult<Option<Prompt>> {
        let state = self.state(model)?;
        let Some(question) = self.next_question(model, &state) else {
            return Ok(None);
        };
        let about_total = self.about_total(model, &state, &question);
        Ok(Some(Prompt {
            id: question.id(),
            question,
            number: self.answers.len() + 1,
            about_total,
        }))
    }

    /// Estimated total number of questions (answered plus open plus the ones still expected).
    fn about_total(&self, model: &Model, state: &QuizState, open: &Question) -> usize {
        let mut sim = state.clone();
        let mut total = self.answers.len() + 1;
        let mut question = open.clone();
        // Walk the quiz with a neutral answer to each question: bracket questions halve the bracket.
        for _ in 0..32 {
            let answer = match &question {
                Question::Tier { options } => {
                    options.first().map(|o| Answer::Tier { tier: o.tier })
                }
                Question::Role { options } => options.first().map(|o| Answer::Role {
                    role: o.name.clone(),
                }),
                Question::Compare { .. } => Some(Answer::Stronger),
                Question::CloserTo { .. } => Some(Answer::Skip),
                Question::Group { .. } | Question::Interval { .. } | Question::VsAnchor { .. } => {
                    Some(Answer::Skip)
                }
            };
            let Some(answer) = answer else { break };
            if self
                .apply_to_state(model, &mut sim, &question, &answer)
                .is_err()
            {
                break;
            }
            match self.next_question(model, &sim) {
                Some(q) => {
                    total += 1;
                    question = q;
                }
                None => break,
            }
        }
        total
    }

    /// Gives an answer to the open question.
    ///
    /// # Errors
    ///
    /// [`QuizError::Finished`] when no question is open, [`QuizError::WrongAnswer`] when the answer does
    /// not fit it, [`QuizError::UnknownChoice`] for a tier, role or group the pool lacks and
    /// [`QuizError::Stale`] when earlier answers no longer fit the model. The quiz is unchanged on error.
    pub fn answer(&mut self, model: &Model, answer: Answer) -> QuizResult<()> {
        let mut state = self.state(model)?;
        let question = self
            .next_question(model, &state)
            .ok_or(QuizError::Finished)?;
        self.apply_to_state(model, &mut state, &question, &answer)?;
        self.answers.push(AnswerEntry {
            question: question.id(),
            answer,
        });
        Ok(())
    }

    /// Finishes the quiz with what it has ("use what I have" and "skip to result"). Does nothing when the
    /// quiz is already finished.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when earlier answers no longer fit the model.
    pub fn use_what_i_have(&mut self, model: &Model) -> QuizResult<()> {
        match self.answer(model, Answer::UseWhatIHave) {
            Err(QuizError::Finished) => Ok(()),
            other => other,
        }
    }

    /// Plays the quiz to the end: every open question is shown to `oracle`, which returns the answer
    /// (the calibration harness and the tests use it to simulate a modder). The state is carried along
    /// instead of being replayed for every step. Stops when no question is open.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when earlier answers no longer fit the model, or an error of the oracle's
    /// answer ([`QuizError::WrongAnswer`], [`QuizError::UnknownChoice`]); the quiz keeps the answers given
    /// before the failing one.
    pub fn play<F>(&mut self, model: &Model, mut oracle: F) -> QuizResult<()>
    where
        F: FnMut(&Question) -> Answer,
    {
        let mut state = self.state(model)?;
        for _ in 0..MAX_QUESTIONS {
            let Some(question) = self.next_question(model, &state) else {
                return Ok(());
            };
            let answer = oracle(&question);
            self.apply_to_state(model, &mut state, &question, &answer)?;
            self.answers.push(AnswerEntry {
                question: question.id(),
                answer,
            });
        }
        Ok(())
    }

    /// Undoes the last answer and returns it.
    pub fn back(&mut self) -> Option<AnswerEntry> {
        self.answers.pop()
    }

    /// Drops the first stale answer and everything after it (used when the pool changed under a draft).
    /// Returns the number of answers dropped.
    pub fn truncate_stale(&mut self, model: &Model) -> usize {
        match self.state(model) {
            Err(QuizError::Stale { index }) => {
                let dropped = self.answers.len().saturating_sub(index);
                self.answers.truncate(index);
                dropped
            }
            _ => 0,
        }
    }

    /// True when no question is open.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when the stored answers no longer fit the model.
    pub fn is_finished(&self, model: &Model) -> QuizResult<bool> {
        Ok(self.next(model)?.is_none())
    }

    /// The baseline input the answers describe: class key, strength and stat constraints.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when the stored answers no longer fit the model.
    pub fn input(&self, model: &Model) -> QuizResult<BaselineInput> {
        let state = self.state(model)?;
        let strength = self.strength_from_state(model, &state);
        Ok(BaselineInput {
            key: state.key,
            strength,
            constraints: state.constraints,
        })
    }

    /// Recomputes the estimate from the answers; the estimate is live after every answer.
    ///
    /// # Errors
    ///
    /// [`QuizError::Stale`] when the stored answers no longer fit the model, or an estimation error.
    pub fn estimate(&self, model: &Model, bands: Option<&BandTable>) -> QuizResult<Estimate> {
        let input = self.input(model)?;
        Ok(match bands {
            Some(b) => model.estimate_with_bands(&input, b)?,
            None => model.estimate(&input)?,
        })
    }
}

/// Applies an answer to a quiz (the free function form of [`Quiz::answer`]).
///
/// # Errors
///
/// See [`Quiz::answer`].
pub fn apply(quiz: &mut Quiz, model: &Model, answer: Answer) -> QuizResult<()> {
    quiz.answer(model, answer)
}

#[cfg(test)]
mod tests;
