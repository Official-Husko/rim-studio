//! Reference pools, classes and class statistics, derived at runtime from the user's reference items.
//!
//! A [`Pool`] holds the reference items of one kind (ranged, melee, apparel). Nothing here is a table of
//! game values: items arrive as plain [`ReferenceItem`]s (an id, a kind, labels and a map of named numeric
//! stats) and tier, role and group are either given by the caller or derived from the distributions of
//! the pool itself (see [`Pool::build`]).
//!
//! A class is a slice of a pool selected by the fallback chain role and tier, role, group and tier, tier,
//! group, all (each level needs at least [`MIN_POOL`] items with the stat). [`ClassStats`] summarises the
//! class per stat: median, percentiles P10 to P90, spread, a rank regression slope, an elasticity and the
//! pool level that was used, with an explanation string such as "based on 5 items of role rifle at tier 2".
//!
//! The statistics themselves live in [`numeric`]; fictional pools for tests and demos in [`synthetic`].

pub mod numeric;
pub mod synthetic;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use numeric::{average_ranks, finite_sorted, ln_spread, mad, median, quantile_sorted, ridge_solve};

/// Smallest number of items a class needs before it is trusted.
pub const MIN_POOL: usize = 3;
/// Role given to an item for which no role is known or derivable.
pub const DEFAULT_ROLE: &str = "any";
/// A stat whose P10 to P90 range is below this factor is flat and takes the median.
pub const FLAT_FACTOR: f64 = 1.16;
/// A question about a stat is only worth asking when the P10 to P90 range exceeds this factor.
pub const ASK_FACTOR: f64 = 1.65;
/// Ridge strength of the rank regression behind the quantile predictor.
pub const RIDGE_Q: f64 = 0.2;
/// Ridge strength of the elasticity fit behind the anchor predictor.
pub const RIDGE_R: f64 = 0.5;
/// Below this many items in the chosen class the bands are shown as rough.
pub const ROUGH_BELOW: usize = 15;
/// Percentile positions stored in [`StatSummary::percentiles`].
pub const PERCENTILE_POINTS: [u32; 9] = [10, 20, 30, 40, 50, 60, 70, 80, 90];

/// The kind of item a pool describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    /// Guns and bows.
    Ranged,
    /// Melee weapons.
    Melee,
    /// Apparel.
    Apparel,
}

impl ItemKind {
    /// Stable lowercase name of the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ranged => "ranged",
            Self::Melee => "melee",
            Self::Apparel => "apparel",
        }
    }

    /// Guesses the kind of an item from the names of its stats (`range` and `warmup` mean ranged, a
    /// `cooldown` without a range means melee, an armor rating or coverage means apparel).
    ///
    /// Returns `None` when no rule fits; callers then rely on the kind they were given.
    #[must_use]
    pub fn infer(stats: &BTreeMap<String, f64>) -> Option<Self> {
        let has = |name: &str| stats.keys().any(|k| k.eq_ignore_ascii_case(name));
        if has("range") && (has("warmup") || has("burst")) {
            Some(Self::Ranged)
        } else if has("cooldown") {
            Some(Self::Melee)
        } else if has("sharp") || has("coverage") || has("armor") {
            Some(Self::Apparel)
        } else {
            None
        }
    }
}

/// A reference item as the def reader hands it over: plain data, no game types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceItem {
    /// Stable identifier (usually the def name).
    pub id: String,
    /// Kind of the item.
    pub kind: ItemKind,
    /// Labels: the first is the display label, the rest are tags used to derive a role.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Tier, when known (otherwise taken from the stats `tier` or `techLevel`, otherwise derived).
    #[serde(default)]
    pub tier: Option<u8>,
    /// Role, when known (otherwise derived from the tags).
    #[serde(default)]
    pub role: Option<String>,
    /// Group (for example the fire class of a gun), when known.
    #[serde(default)]
    pub group: Option<String>,
    /// Strength index of the item (see the power indices of the math specification). When absent the
    /// stat named `strength` is used; an item without one cannot enter a pool.
    #[serde(default)]
    pub strength: Option<f64>,
    /// Named numeric stats. Non finite values are ignored.
    #[serde(default)]
    pub stats: BTreeMap<String, f64>,
}

impl ReferenceItem {
    /// Builds an item with no labels, tier, role, group or stats.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: ItemKind) -> Self {
        Self {
            id: id.into(),
            kind,
            labels: Vec::new(),
            tier: None,
            role: None,
            group: None,
            strength: None,
            stats: BTreeMap::new(),
        }
    }

    /// Display label: the first label, or the id when there are none.
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.labels.first().map_or(self.id.as_str(), String::as_str)
    }
}

/// Why an item was left out of a pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Excluded {
    /// Id of the item.
    pub id: String,
    /// Short reason.
    pub reason: String,
}

/// An item inside a pool with its derived axes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolItem {
    /// Id of the item.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Tier axis.
    pub tier: u8,
    /// Role axis.
    pub role: String,
    /// Group axis (fire class and so on), when known.
    pub group: Option<String>,
    /// Strength index, strictly positive.
    pub strength: f64,
    /// Natural logarithm of the strength.
    pub ln_strength: f64,
    /// Stats, all finite.
    pub stats: BTreeMap<String, f64>,
    /// The tier was derived from the strength distribution rather than given.
    pub tier_derived: bool,
    /// The role was derived from the tags rather than given.
    pub role_derived: bool,
}

impl PoolItem {
    /// Value of a stat, if present and finite.
    #[must_use]
    pub fn stat(&self, name: &str) -> Option<f64> {
        self.stats.get(name).copied().filter(|v| v.is_finite())
    }
}

/// Options of [`Pool::build`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolOptions {
    /// Derive a tier (terciles of the strength distribution) for items that have none.
    pub derive_tier: bool,
    /// Display names for tiers, used in explanation strings ("Industrial"); optional.
    pub tier_labels: BTreeMap<u8, String>,
}

impl Default for PoolOptions {
    fn default() -> Self {
        Self {
            derive_tier: true,
            tier_labels: BTreeMap::new(),
        }
    }
}

/// The reference items of one kind, sorted by strength then id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    /// Kind of every item.
    pub kind: ItemKind,
    /// Items sorted ascending by strength, ties by id. Positions are stable item indices.
    pub items: Vec<PoolItem>,
    /// Items that could not enter the pool, in input order.
    pub excluded: Vec<Excluded>,
    /// Display names for tiers.
    #[serde(default)]
    pub tier_labels: BTreeMap<u8, String>,
}

fn tier_from_stats(stats: &BTreeMap<String, f64>) -> Option<u8> {
    for key in ["tier", "techLevel"] {
        if let Some(v) = stats
            .get(key)
            .copied()
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 255.0)
        {
            return Some(v.round() as u8);
        }
    }
    None
}

impl Pool {
    /// Builds the pool of `kind` from reference items.
    ///
    /// Items of another kind, with a duplicate id or without a finite positive strength are excluded and
    /// listed in [`Pool::excluded`]. Tier is the explicit field, else the stat `tier` or `techLevel`, else
    /// (when [`PoolOptions::derive_tier`]) the tercile of the strength mid rank within the pool. Role is
    /// the explicit field, else the most common tag shared by at least [`MIN_POOL`] items but not by all of
    /// them (ties go to the smaller tag), else [`DEFAULT_ROLE`]. The result is deterministic.
    #[must_use]
    pub fn build(kind: ItemKind, items: &[ReferenceItem], options: &PoolOptions) -> Self {
        let mut excluded = Vec::new();
        let mut seen = BTreeSet::new();
        let mut accepted: Vec<(&ReferenceItem, f64)> = Vec::new();
        for item in items {
            if item.kind != kind {
                excluded.push(Excluded {
                    id: item.id.clone(),
                    reason: "kind differs from the pool".into(),
                });
                continue;
            }
            if !seen.insert(item.id.clone()) {
                excluded.push(Excluded {
                    id: item.id.clone(),
                    reason: "duplicate id".into(),
                });
                continue;
            }
            let strength = item
                .strength
                .or_else(|| item.stats.get("strength").copied());
            match strength {
                Some(s) if s.is_finite() && s > 0.0 => accepted.push((item, s)),
                _ => excluded.push(Excluded {
                    id: item.id.clone(),
                    reason: "no finite positive strength index".into(),
                }),
            }
        }
        let strengths: Vec<f64> = accepted.iter().map(|(_, s)| *s).collect();
        let ranks = average_ranks(&strengths);
        let n = accepted.len();
        // Tag frequencies for role derivation (each tag counted once per item).
        let mut tag_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for (item, _) in &accepted {
            let tags: BTreeSet<&str> = item.labels.iter().skip(1).map(String::as_str).collect();
            for tag in tags {
                *tag_counts.entry(tag).or_insert(0) += 1;
            }
        }
        let mut out: Vec<PoolItem> = Vec::with_capacity(n);
        for ((item, strength), rank) in accepted.iter().zip(&ranks) {
            let stats: BTreeMap<String, f64> = item
                .stats
                .iter()
                .filter(|(_, v)| v.is_finite())
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            let explicit_tier = item.tier.or_else(|| tier_from_stats(&stats));
            let (tier, tier_derived) = match explicit_tier {
                Some(t) => (t, false),
                None if options.derive_tier && n > 0 => {
                    let mid = (rank - 0.5) / n as f64;
                    ((mid * 3.0).floor().clamp(0.0, 2.0) as u8, true)
                }
                None => (0, true),
            };
            let (role, role_derived) = match item.role.as_ref().filter(|r| !r.trim().is_empty()) {
                Some(r) => (r.clone(), false),
                None => {
                    let mut best: Option<(&str, usize)> = None;
                    let tags: BTreeSet<&str> =
                        item.labels.iter().skip(1).map(String::as_str).collect();
                    for tag in tags {
                        let count = tag_counts.get(tag).copied().unwrap_or(0);
                        if count < MIN_POOL || count >= n {
                            continue;
                        }
                        if best.is_none_or(|(_, c)| count > c) {
                            best = Some((tag, count));
                        }
                    }
                    match best {
                        Some((tag, _)) => (tag.to_string(), true),
                        None => (DEFAULT_ROLE.to_string(), true),
                    }
                }
            };
            out.push(PoolItem {
                id: item.id.clone(),
                label: item.display_label().to_string(),
                tier,
                role,
                group: item.group.clone().filter(|g| !g.trim().is_empty()),
                strength: *strength,
                ln_strength: strength.ln(),
                stats,
                tier_derived,
                role_derived,
            });
        }
        out.sort_by(|a, b| {
            a.strength
                .total_cmp(&b.strength)
                .then_with(|| a.id.cmp(&b.id))
        });
        Self {
            kind,
            items: out,
            excluded,
            tier_labels: options.tier_labels.clone(),
        }
    }

    /// A copy of the pool without the items at the given positions (the order of the rest is kept).
    #[must_use]
    pub fn without(&self, drop: &[usize]) -> Self {
        let items = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, _)| !drop.contains(i))
            .map(|(_, item)| item.clone())
            .collect();
        Self {
            kind: self.kind,
            items,
            excluded: self.excluded.clone(),
            tier_labels: self.tier_labels.clone(),
        }
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when the pool has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Item at a position.
    #[must_use]
    pub fn item(&self, index: usize) -> Option<&PoolItem> {
        self.items.get(index)
    }

    /// Position of the item with `id`.
    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    /// Distinct tiers present, ascending.
    #[must_use]
    pub fn tiers(&self) -> Vec<u8> {
        self.items
            .iter()
            .map(|i| i.tier)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Distinct roles present with their item counts, by role name.
    #[must_use]
    pub fn roles(&self) -> Vec<(String, usize)> {
        let mut map: BTreeMap<&str, usize> = BTreeMap::new();
        for i in &self.items {
            *map.entry(i.role.as_str()).or_insert(0) += 1;
        }
        map.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    /// Distinct groups present with their item counts, by group name.
    #[must_use]
    pub fn groups(&self) -> Vec<(String, usize)> {
        let mut map: BTreeMap<&str, usize> = BTreeMap::new();
        for i in &self.items {
            if let Some(g) = i.group.as_deref() {
                *map.entry(g).or_insert(0) += 1;
            }
        }
        map.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    /// Names of all stats that at least one item carries, sorted.
    #[must_use]
    pub fn stat_names(&self) -> Vec<String> {
        self.items
            .iter()
            .flat_map(|i| i.stats.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Display text of a tier: its label when one is known, otherwise `tier N`.
    #[must_use]
    pub fn tier_text(&self, tier: u8) -> String {
        self.tier_labels
            .get(&tier)
            .cloned()
            .unwrap_or_else(|| format!("tier {tier}"))
    }

    /// Selects the class for `key` by the fallback chain.
    ///
    /// With `stat` set only items that carry the stat count. `exclude` removes one item (used by the
    /// leave one out harness). The first level of the chain that holds at least `min` items wins; the last
    /// level (all items) is returned even when it is smaller.
    #[must_use]
    pub fn cohort(
        &self,
        key: &ClassKey,
        stat: Option<&str>,
        exclude: Option<usize>,
        min: usize,
    ) -> Cohort {
        let eligible: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, item)| Some(*i) != exclude && stat.is_none_or(|s| item.stat(s).is_some()))
            .map(|(i, _)| i)
            .collect();
        let requested = ChainLevel::most_specific(key);
        let mut started = false;
        for level in ChainLevel::CHAIN {
            if level == requested {
                started = true;
            }
            if !started || !level.applies(key) {
                continue;
            }
            let members: Vec<usize> = eligible
                .iter()
                .copied()
                .filter(|i| {
                    self.items
                        .get(*i)
                        .is_some_and(|item| level.matches(key, item))
                })
                .collect();
            if members.len() >= min || level == ChainLevel::All {
                return Cohort {
                    level,
                    requested,
                    members,
                };
            }
        }
        Cohort {
            level: ChainLevel::All,
            requested,
            members: eligible,
        }
    }

    /// One line explanation of a cohort, for example "based on 5 items of role rifle at tier 2" or
    /// "based on 9 items of tier 2 (widened from role and tier: fewer than 3 items qualify)".
    #[must_use]
    pub fn explain(&self, key: &ClassKey, cohort: &Cohort) -> String {
        let n = cohort.members.len();
        let mut parts: Vec<String> = Vec::new();
        if matches!(cohort.level, ChainLevel::RoleTier | ChainLevel::Role)
            && let Some(role) = &key.role
        {
            parts.push(format!("of role {role}"));
        }
        if matches!(cohort.level, ChainLevel::GroupTier | ChainLevel::Group)
            && let Some(group) = &key.group
        {
            parts.push(format!("of group {group}"));
        }
        if matches!(
            cohort.level,
            ChainLevel::RoleTier | ChainLevel::GroupTier | ChainLevel::Tier
        ) && let Some(tier) = key.tier
        {
            parts.push(format!("at {}", self.tier_text(tier)));
        }
        let scope = if parts.is_empty() {
            "of this kind".to_string()
        } else {
            parts.join(" ")
        };
        let noun = if n == 1 { "item" } else { "items" };
        let mut text = format!("based on {n} {noun} {scope}");
        if cohort.level != cohort.requested {
            text.push_str(&format!(
                " (widened from {}: fewer than {MIN_POOL} items qualify)",
                cohort.requested.describe()
            ));
        }
        text
    }
}

/// Level of the fallback chain used to select a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChainLevel {
    /// Same role and same tier.
    RoleTier,
    /// Same role.
    Role,
    /// Same group and same tier.
    GroupTier,
    /// Same tier.
    Tier,
    /// Same group.
    Group,
    /// Every item of the kind.
    All,
}

impl ChainLevel {
    /// The chain in order, most specific first.
    pub const CHAIN: [Self; 6] = [
        Self::RoleTier,
        Self::Role,
        Self::GroupTier,
        Self::Tier,
        Self::Group,
        Self::All,
    ];

    /// Whether the key carries what this level needs.
    #[must_use]
    pub fn applies(self, key: &ClassKey) -> bool {
        match self {
            Self::RoleTier => key.role.is_some() && key.tier.is_some(),
            Self::Role => key.role.is_some(),
            Self::GroupTier => key.group.is_some() && key.tier.is_some(),
            Self::Tier => key.tier.is_some(),
            Self::Group => key.group.is_some(),
            Self::All => true,
        }
    }

    /// The most specific level the key allows.
    #[must_use]
    pub fn most_specific(key: &ClassKey) -> Self {
        Self::CHAIN
            .into_iter()
            .find(|l| l.applies(key))
            .unwrap_or(Self::All)
    }

    /// Whether `item` belongs to this level for `key`.
    #[must_use]
    pub fn matches(self, key: &ClassKey, item: &PoolItem) -> bool {
        let role = || key.role.as_deref().is_some_and(|r| r == item.role);
        let tier = || key.tier.is_some_and(|t| t == item.tier);
        let group = || {
            key.group
                .as_deref()
                .is_some_and(|g| item.group.as_deref() == Some(g))
        };
        match self {
            Self::RoleTier => role() && tier(),
            Self::Role => role(),
            Self::GroupTier => group() && tier(),
            Self::Tier => tier(),
            Self::Group => group(),
            Self::All => true,
        }
    }

    /// Stable lowercase name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoleTier => "role-tier",
            Self::Role => "role",
            Self::GroupTier => "group-tier",
            Self::Tier => "tier",
            Self::Group => "group",
            Self::All => "all",
        }
    }

    /// Human readable name ("role and tier").
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::RoleTier => "role and tier",
            Self::Role => "role",
            Self::GroupTier => "group and tier",
            Self::Tier => "tier",
            Self::Group => "group",
            Self::All => "all items",
        }
    }
}

/// What is known about the item being designed: the axes that select a class.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassKey {
    /// Role, when answered.
    #[serde(default)]
    pub role: Option<String>,
    /// Tier, when answered.
    #[serde(default)]
    pub tier: Option<u8>,
    /// Group, when answered.
    #[serde(default)]
    pub group: Option<String>,
}

impl ClassKey {
    /// Key with a role and a tier.
    #[must_use]
    pub fn new(role: impl Into<String>, tier: u8) -> Self {
        Self {
            role: Some(role.into()),
            tier: Some(tier),
            group: None,
        }
    }

    /// The key an existing pool item describes (its true axes).
    #[must_use]
    pub fn of_item(item: &PoolItem) -> Self {
        Self {
            role: Some(item.role.clone()),
            tier: Some(item.tier),
            group: item.group.clone(),
        }
    }
}

/// The members chosen by the fallback chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cohort {
    /// Level that was used.
    pub level: ChainLevel,
    /// Most specific level the key allowed.
    pub requested: ChainLevel,
    /// Pool positions of the members, ascending.
    pub members: Vec<usize>,
}

impl Cohort {
    /// True when the chain had to widen beyond the requested level.
    #[must_use]
    pub fn widened(&self) -> bool {
        self.level != self.requested
    }
}

/// Which predictor produces a stat (section 6 of the math specification).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Predictor {
    /// Class median (M).
    #[default]
    Median,
    /// Pool quantile moved by the rank regression (Q).
    Quantile,
    /// Ratio scaling from the nearest anchors (R).
    Anchor,
}

/// Tunable constants of the class statistics. The defaults are the documented priors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeriveOptions {
    /// Smallest class size.
    pub min_pool: usize,
    /// Ridge of the quantile regression.
    pub ridge_q: f64,
    /// Ridge of the elasticity fit.
    pub ridge_r: f64,
    /// Flat factor (P10 to P90 ratio below which a stat takes the median).
    pub flat_factor: f64,
    /// Name of the mass stat used as the third regressor, when any.
    pub mass_stat: Option<String>,
}

impl Default for DeriveOptions {
    fn default() -> Self {
        Self {
            min_pool: MIN_POOL,
            ridge_q: RIDGE_Q,
            ridge_r: RIDGE_R,
            flat_factor: FLAT_FACTOR,
            mass_stat: Some("mass".to_string()),
        }
    }
}

/// Sorted regressor columns of a class, kept so that a prediction needs no pool.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatBasis {
    /// Strengths of the members, ascending.
    pub strength: Vec<f64>,
    /// Tiers of the members as numbers, ascending.
    pub tier: Vec<f64>,
    /// Masses of the members, ascending (empty when the mass column is not used for this stat).
    pub mass: Vec<f64>,
}

/// Distribution of one stat inside a class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatSummary {
    /// Stat name.
    pub stat: String,
    /// Level of the chain this stat was drawn from.
    pub level: ChainLevel,
    /// Most specific level the key allowed.
    pub requested_level: ChainLevel,
    /// Number of items behind the numbers.
    pub n: usize,
    /// Median.
    pub median: f64,
    /// Median absolute deviation (unscaled).
    pub mad: f64,
    /// `ln(P90 / P10)` for positive stats, `P90 - P10` otherwise.
    pub spread: f64,
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
    /// Percentiles P10, P20 to P90 (see [`PERCENTILE_POINTS`]).
    pub percentiles: [f64; 9],
    /// The member values, ascending.
    pub values: Vec<f64>,
    /// Regressor columns of the members.
    pub basis: StatBasis,
    /// Ridge slopes of the mid ranked stat on the mid ranks of strength, tier and mass.
    pub rank_beta: [f64; 3],
    /// Elasticity of the stat with respect to strength (fitted within role over the whole pool).
    pub elasticity: f64,
    /// The stat is flat in this class (its spread is below the flat factor) and takes the median.
    pub flat: bool,
    /// Predictor chosen for the stat (the default is the median until a model assigns one).
    #[serde(default)]
    pub predictor: Predictor,
    /// Properties of the stat (discrete, integer, ask flag).
    #[serde(default)]
    pub traits: StatTraits,
    /// Multipliers of the "lighter, similar, heavier than the anchor" answers: lower, similar, higher.
    #[serde(default = "default_bucket_ratios")]
    pub bucket_ratios: [f64; 3],
}

/// Multipliers used for the three buckets of a "compared with the anchor" answer until a model has
/// learned them from the pool.
#[must_use]
pub fn default_bucket_ratios() -> [f64; 3] {
    [(-0.4f64).exp(), 1.0, 0.4f64.exp()]
}

/// Properties of a stat that change how it is predicted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatTraits {
    /// Takes few distinct values: anchors are copied (weighted median), never scaled.
    #[serde(default)]
    pub discrete: bool,
    /// Predictions are rounded to whole numbers.
    #[serde(default)]
    pub integer: bool,
    /// Not predictable from the inputs: shown as a plain input with the pool range beside it.
    #[serde(default)]
    pub ask: bool,
}

/// Detects whether a stat is integer valued and discrete from the pool itself: integer when every value
/// is a whole number, discrete when it is integer valued with at most [`MAX_DISCRETE_VALUES`] distinct
/// values. The ask flag is never detected.
#[must_use]
pub fn detect_traits(pool: &Pool, stat: &str) -> StatTraits {
    let values: Vec<f64> = pool.items.iter().filter_map(|i| i.stat(stat)).collect();
    if values.is_empty() {
        return StatTraits::default();
    }
    let integer = values.iter().all(|v| (v - v.round()).abs() < 1e-9);
    let mut distinct = finite_sorted(&values);
    distinct.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    StatTraits {
        discrete: integer && distinct.len() <= MAX_DISCRETE_VALUES,
        integer,
        ask: false,
    }
}

/// Largest number of distinct whole number values for a stat to count as discrete.
pub const MAX_DISCRETE_VALUES: usize = 8;

impl StatSummary {
    /// Percentile `pct` (0 to 100) of the member values by interpolation.
    #[must_use]
    pub fn percentile(&self, pct: f64) -> f64 {
        quantile_sorted(&self.values, pct / 100.0).unwrap_or(self.median)
    }

    /// P10.
    #[must_use]
    pub fn p10(&self) -> f64 {
        self.percentiles[0]
    }

    /// P90.
    #[must_use]
    pub fn p90(&self) -> f64 {
        self.percentiles[8]
    }

    /// True when a question about the stat could move the answer (the P10 to P90 range exceeds
    /// [`ASK_FACTOR`]).
    #[must_use]
    pub fn varies(&self) -> bool {
        self.spread >= ASK_FACTOR.ln()
    }

    /// Number of members with a value strictly below `x`.
    #[must_use]
    pub fn count_below(&self, x: f64) -> usize {
        self.values.iter().filter(|v| **v < x).count()
    }

    /// Derives the summary of `stat` for the class selected by `key`, optionally leaving one item out.
    ///
    /// Returns `None` when no item carries the stat.
    #[must_use]
    pub fn derive(
        pool: &Pool,
        key: &ClassKey,
        stat: &str,
        exclude: Option<usize>,
        options: &DeriveOptions,
        elasticity: f64,
    ) -> Option<Self> {
        let cohort = pool.cohort(key, Some(stat), exclude, options.min_pool);
        Self::from_cohort(pool, &cohort, stat, options, elasticity)
    }

    /// Summarises `stat` over the members of an already selected cohort (every member must carry it).
    #[must_use]
    pub fn from_cohort(
        pool: &Pool,
        cohort: &Cohort,
        stat: &str,
        options: &DeriveOptions,
        elasticity: f64,
    ) -> Option<Self> {
        let members: Vec<&PoolItem> = cohort
            .members
            .iter()
            .filter_map(|i| pool.item(*i))
            .collect();
        let raw: Vec<f64> = members.iter().filter_map(|m| m.stat(stat)).collect();
        if raw.is_empty() {
            return None;
        }
        let values = finite_sorted(&raw);
        let med = median(&values)?;
        let mut percentiles = [med; 9];
        for (slot, p) in percentiles.iter_mut().zip(PERCENTILE_POINTS) {
            *slot = quantile_sorted(&values, f64::from(p) / 100.0).unwrap_or(med);
        }
        let spread = ln_spread(&values);
        let strengths: Vec<f64> = members.iter().map(|m| m.strength).collect();
        let tiers: Vec<f64> = members.iter().map(|m| f64::from(m.tier)).collect();
        let use_mass = options.mass_stat.as_deref().is_some_and(|ms| ms != stat)
            && members.iter().all(|m| {
                options
                    .mass_stat
                    .as_deref()
                    .and_then(|ms| m.stat(ms))
                    .is_some()
            });
        let masses: Vec<f64> = if use_mass {
            members
                .iter()
                .filter_map(|m| options.mass_stat.as_deref().and_then(|ms| m.stat(ms)))
                .collect()
        } else {
            Vec::new()
        };
        let rank_beta = rank_betas(&raw, &strengths, &tiers, &masses, options);
        Some(Self {
            stat: stat.to_string(),
            level: cohort.level,
            requested_level: cohort.requested,
            n: values.len(),
            median: med,
            mad: mad(&values).unwrap_or(0.0),
            spread,
            min: values.first().copied().unwrap_or(med),
            max: values.last().copied().unwrap_or(med),
            percentiles,
            flat: spread < options.flat_factor.ln(),
            values,
            basis: StatBasis {
                strength: finite_sorted(&strengths),
                tier: finite_sorted(&tiers),
                mass: finite_sorted(&masses),
            },
            rank_beta,
            elasticity,
            predictor: Predictor::Median,
            traits: StatTraits::default(),
            bucket_ratios: default_bucket_ratios(),
        })
    }
}

/// Mid ranks `(average rank - 0.5) / n` of every value.
fn mid_ranks(values: &[f64]) -> Vec<f64> {
    let n = values.len().max(1) as f64;
    average_ranks(values)
        .into_iter()
        .map(|r| (r - 0.5) / n)
        .collect()
}

/// Ridge slopes of the mid ranked stat on the mid ranks of strength, tier and (when given) mass.
fn rank_betas(
    y: &[f64],
    strengths: &[f64],
    tiers: &[f64],
    masses: &[f64],
    options: &DeriveOptions,
) -> [f64; 3] {
    let mut beta = [0.0; 3];
    if y.len() < options.min_pool || y.len() != strengths.len() {
        return beta;
    }
    let uy: Vec<f64> = mid_ranks(y).into_iter().map(|v| v - 0.5).collect();
    let xs: Vec<f64> = mid_ranks(strengths).into_iter().map(|v| v - 0.5).collect();
    let xt: Vec<f64> = mid_ranks(tiers).into_iter().map(|v| v - 0.5).collect();
    let with_mass = masses.len() == y.len();
    let xm: Vec<f64> = if with_mass {
        mid_ranks(masses).into_iter().map(|v| v - 0.5).collect()
    } else {
        Vec::new()
    };
    let rows: Vec<Vec<f64>> = (0..y.len())
        .map(|i| {
            let mut row = vec![
                xs.get(i).copied().unwrap_or(0.0),
                xt.get(i).copied().unwrap_or(0.0),
            ];
            if with_mass {
                row.push(xm.get(i).copied().unwrap_or(0.0));
            }
            row
        })
        .collect();
    if let Some(solved) = ridge_solve(&rows, &uy, options.ridge_q) {
        for (slot, v) in beta.iter_mut().zip(solved) {
            *slot = v;
        }
    }
    beta
}

/// Elasticity of `stat` with respect to strength: the slope of `ln y` on `ln strength` within role (role
/// fixed effects), ridge `ridge`, clipped to `[-0.5, 1.5]`. Falls back to pooling every item when fewer
/// than eight items have a role peer. Items with a non positive value are ignored. Returns 0 when nothing
/// can be fitted.
#[must_use]
pub fn fit_elasticity(pool: &Pool, stat: &str, ridge: f64, exclude: Option<usize>) -> f64 {
    let rows: Vec<(&str, f64, f64)> = pool
        .items
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != exclude)
        .filter_map(|(_, it)| {
            it.stat(stat)
                .filter(|v| *v > 0.0)
                .map(|v| (it.role.as_str(), it.ln_strength, v.ln()))
        })
        .collect();
    if rows.len() < 2 {
        return 0.0;
    }
    let mut roles: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
    for (role, x, y) in &rows {
        roles.entry(role).or_default().push((*x, *y));
    }
    let (mut sxx, mut sxy, mut used) = (0.0, 0.0, 0usize);
    for group in roles.values().filter(|g| g.len() >= 2) {
        let n = group.len() as f64;
        let mx = group.iter().map(|p| p.0).sum::<f64>() / n;
        let my = group.iter().map(|p| p.1).sum::<f64>() / n;
        for (x, y) in group {
            sxx += (x - mx) * (x - mx);
            sxy += (x - mx) * (y - my);
        }
        used += group.len();
    }
    if used < 8 {
        let n = rows.len() as f64;
        let mx = rows.iter().map(|p| p.1).sum::<f64>() / n;
        let my = rows.iter().map(|p| p.2).sum::<f64>() / n;
        sxx = rows.iter().map(|p| (p.1 - mx) * (p.1 - mx)).sum();
        sxy = rows.iter().map(|p| (p.1 - mx) * (p.2 - my)).sum();
    }
    (sxy / (sxx + ridge.max(0.0))).clamp(-0.5, 1.5)
}

/// Statistics of one class: the class level used, the explanation and one summary per stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassStats {
    /// The key the class was derived for.
    pub key: ClassKey,
    /// Level used for the class as a whole (no particular stat).
    pub level: ChainLevel,
    /// Most specific level the key allowed.
    pub requested_level: ChainLevel,
    /// Number of items in the class as a whole.
    pub n: usize,
    /// Explanation string such as "based on 5 items of role rifle at tier 2".
    pub explanation: String,
    /// Distribution of the strength index in the class.
    pub strength: Option<StatSummary>,
    /// Per stat summaries, each drawn from the first chain level with enough items carrying that stat.
    pub stats: BTreeMap<String, StatSummary>,
}

impl ClassStats {
    /// Derives the class statistics of `key` from `pool`. The result is deterministic.
    #[must_use]
    pub fn derive(pool: &Pool, key: &ClassKey, options: &DeriveOptions) -> Self {
        let elasticity: BTreeMap<String, f64> = pool
            .stat_names()
            .into_iter()
            .map(|name| {
                let e = fit_elasticity(pool, &name, options.ridge_r, None);
                (name, e)
            })
            .collect();
        Self::derive_with(pool, key, options, &elasticity)
    }

    /// Like [`ClassStats::derive`] with precomputed elasticities (a stat missing from the map gets 0).
    #[must_use]
    pub fn derive_with(
        pool: &Pool,
        key: &ClassKey,
        options: &DeriveOptions,
        elasticity: &BTreeMap<String, f64>,
    ) -> Self {
        let cohort = pool.cohort(key, None, None, options.min_pool);
        let explanation = pool.explain(key, &cohort);
        let strength = strength_summary(pool, &cohort);
        let mut stats = BTreeMap::new();
        for name in pool.stat_names() {
            let e = elasticity.get(&name).copied().unwrap_or(0.0);
            if let Some(mut summary) = StatSummary::derive(pool, key, &name, None, options, e) {
                summary.traits = detect_traits(pool, &name);
                stats.insert(name, summary);
            }
        }
        Self {
            key: key.clone(),
            level: cohort.level,
            requested_level: cohort.requested,
            n: cohort.members.len(),
            explanation,
            strength,
            stats,
        }
    }

    /// True when the class is small enough that bands should be shown as rough ([`ROUGH_BELOW`]).
    #[must_use]
    pub fn is_rough(&self) -> bool {
        self.n < ROUGH_BELOW
    }

    /// Summary of one stat.
    #[must_use]
    pub fn stat(&self, name: &str) -> Option<&StatSummary> {
        self.stats.get(name)
    }
}

/// Summary of the strength index over a cohort (the stat is named `strength`).
#[must_use]
pub fn strength_summary(pool: &Pool, cohort: &Cohort) -> Option<StatSummary> {
    let members: Vec<&PoolItem> = cohort
        .members
        .iter()
        .filter_map(|i| pool.item(*i))
        .collect();
    if members.is_empty() {
        return None;
    }
    let values = finite_sorted(&members.iter().map(|m| m.strength).collect::<Vec<_>>());
    let med = median(&values)?;
    let mut percentiles = [med; 9];
    for (slot, p) in percentiles.iter_mut().zip(PERCENTILE_POINTS) {
        *slot = quantile_sorted(&values, f64::from(p) / 100.0).unwrap_or(med);
    }
    let spread = ln_spread(&values);
    Some(StatSummary {
        stat: "strength".to_string(),
        level: cohort.level,
        requested_level: cohort.requested,
        n: values.len(),
        median: med,
        mad: mad(&values).unwrap_or(0.0),
        spread,
        min: values.first().copied().unwrap_or(med),
        max: values.last().copied().unwrap_or(med),
        percentiles,
        flat: false,
        basis: StatBasis {
            strength: values.clone(),
            tier: Vec::new(),
            mass: Vec::new(),
        },
        values,
        rank_beta: [0.0; 3],
        elasticity: 0.0,
        predictor: Predictor::Median,
        traits: StatTraits::default(),
        bucket_ratios: default_bucket_ratios(),
    })
}

#[cfg(test)]
mod tests {
    use super::synthetic::{SyntheticSpec, synthetic_items};
    use super::*;

    fn pool(n: usize) -> Pool {
        let items = synthetic_items(&SyntheticSpec {
            n,
            ..SyntheticSpec::default()
        });
        Pool::build(ItemKind::Ranged, &items, &PoolOptions::default())
    }

    #[test]
    fn pool_is_sorted_by_strength_and_excludes_bad_items() {
        let mut items = synthetic_items(&SyntheticSpec {
            n: 12,
            ..SyntheticSpec::default()
        });
        items.push(ReferenceItem::new("RS_NoStrength", ItemKind::Ranged));
        items.push(ReferenceItem::new("RS_Wrong", ItemKind::Melee));
        let dup = items[0].clone();
        items.push(dup);
        let p = Pool::build(ItemKind::Ranged, &items, &PoolOptions::default());
        assert_eq!(p.len(), 12);
        assert_eq!(p.excluded.len(), 3);
        assert!(p.items.windows(2).all(|w| w[0].strength <= w[1].strength));
        assert!(p.excluded.iter().any(|e| e.reason == "duplicate id"));
    }

    #[test]
    fn build_is_independent_of_input_order() {
        let items = synthetic_items(&SyntheticSpec {
            n: 20,
            ..SyntheticSpec::default()
        });
        let mut rev = items.clone();
        rev.reverse();
        let a = Pool::build(ItemKind::Ranged, &items, &PoolOptions::default());
        let b = Pool::build(ItemKind::Ranged, &rev, &PoolOptions::default());
        assert_eq!(a.items, b.items);
    }

    #[test]
    fn tier_and_role_are_derived_when_missing() {
        let spec = SyntheticSpec {
            n: 30,
            explicit_axes: false,
            ..SyntheticSpec::default()
        };
        let items = synthetic_items(&spec);
        assert!(items.iter().all(|i| i.tier.is_none() && i.role.is_none()));
        let p = Pool::build(ItemKind::Ranged, &items, &PoolOptions::default());
        assert!(p.items.iter().all(|i| i.tier_derived && i.role_derived));
        assert_eq!(p.tiers(), vec![0, 1, 2]);
        let roles = p.roles();
        assert!(roles.len() >= 3, "roles: {roles:?}");
        assert!(roles.iter().all(|(r, _)| r != DEFAULT_ROLE));
    }

    #[test]
    fn tier_stat_is_used_before_derivation() {
        let mut item = ReferenceItem::new("RS_A", ItemKind::Ranged);
        item.strength = Some(2.0);
        item.stats.insert("techLevel".into(), 3.0);
        let p = Pool::build(ItemKind::Ranged, &[item], &PoolOptions::default());
        assert_eq!(p.items[0].tier, 3);
        assert!(!p.items[0].tier_derived);
    }

    #[test]
    fn chain_falls_back_with_an_explanation() {
        let p = pool(40);
        let role = p.items[0].role.clone();
        let tier = p.items[0].tier;
        let c = p.cohort(&ClassKey::new(role.clone(), tier), None, None, 3);
        assert!(c.members.len() >= 3);
        // an impossible tier forces widening
        let key = ClassKey::new(role, 99);
        let c2 = p.cohort(&key, None, None, 3);
        assert!(c2.widened());
        assert_eq!(c2.level, ChainLevel::Role);
        let text = p.explain(&key, &c2);
        assert!(text.contains("widened from role and tier"), "{text}");
        let none = p.cohort(&ClassKey::default(), None, None, 3);
        assert_eq!(none.level, ChainLevel::All);
        assert_eq!(none.members.len(), 40);
    }

    #[test]
    fn exclude_removes_exactly_one_member() {
        let p = pool(30);
        let all = p.cohort(&ClassKey::default(), None, None, 3);
        let ex = p.cohort(&ClassKey::default(), None, Some(4), 3);
        assert_eq!(ex.members.len(), all.members.len() - 1);
        assert!(!ex.members.contains(&4));
    }

    #[test]
    fn class_stats_match_hand_computed_values() {
        let items: Vec<ReferenceItem> = [
            (1.0, 10.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
            (5.0, 50.0),
        ]
        .iter()
        .enumerate()
        .map(|(i, (s, m))| {
            let mut it = ReferenceItem::new(format!("RS_Hand{i}"), ItemKind::Melee);
            it.strength = Some(*s);
            it.tier = Some(1);
            it.role = Some("RS_blade".into());
            it.stats.insert("mass".into(), *m);
            it
        })
        .collect();
        let p = Pool::build(ItemKind::Melee, &items, &PoolOptions::default());
        let cs = ClassStats::derive(&p, &ClassKey::new("RS_blade", 1), &DeriveOptions::default());
        let m = cs.stat("mass").unwrap();
        assert_eq!(cs.n, 5);
        assert_eq!(cs.level, ChainLevel::RoleTier);
        assert!((m.median - 30.0).abs() < 1e-9);
        assert!((m.p10() - 14.0).abs() < 1e-9 && (m.p90() - 46.0).abs() < 1e-9);
        assert!((m.mad - 10.0).abs() < 1e-9);
        assert!(m.rank_beta[0] > 0.3);
        assert!(m.varies() && !m.flat);
        assert!(cs.is_rough());
        assert!(cs.explanation.starts_with("based on 5 items"));
    }

    #[test]
    fn elasticity_is_recovered_within_role() {
        let p = pool(60);
        let mass = fit_elasticity(&p, "mass", RIDGE_R, None);
        assert!(mass > 0.3 && mass <= 1.5, "mass elasticity {mass}");
        let range = fit_elasticity(&p, "range", RIDGE_R, None);
        assert!(range.abs() < 0.3, "range elasticity {range}");
        assert_eq!(fit_elasticity(&p, "missing", RIDGE_R, None), 0.0);
    }

    #[test]
    fn class_stats_round_trip_through_json() {
        let p = pool(25);
        let cs = ClassStats::derive(&p, &ClassKey::default(), &DeriveOptions::default());
        let json = serde_json::to_string(&cs).unwrap();
        let back: ClassStats = serde_json::from_str(&json).unwrap();
        // serde_json without the float_roundtrip feature may differ by one unit in the last place.
        assert_eq!(cs.n, back.n);
        assert_eq!(cs.explanation, back.explanation);
        assert_eq!(
            cs.stats.keys().collect::<Vec<_>>(),
            back.stats.keys().collect::<Vec<_>>()
        );
        for (name, a) in &cs.stats {
            let b = &back.stats[name];
            assert!((a.median - b.median).abs() <= 1e-12 * a.median.abs().max(1.0));
            assert_eq!(a.values.len(), b.values.len());
            assert_eq!(a.level, b.level);
        }
    }

    #[test]
    fn derive_is_deterministic() {
        let p = pool(25);
        let a = serde_json::to_string(&ClassStats::derive(
            &p,
            &ClassKey::new("RS_role_a", 1),
            &DeriveOptions::default(),
        ))
        .unwrap();
        let b = serde_json::to_string(&ClassStats::derive(
            &p,
            &ClassKey::new("RS_role_a", 1),
            &DeriveOptions::default(),
        ))
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn infer_kind_from_stat_names() {
        let mut s = BTreeMap::new();
        s.insert("range".to_string(), 1.0);
        s.insert("warmup".to_string(), 1.0);
        assert_eq!(ItemKind::infer(&s), Some(ItemKind::Ranged));
        let mut m = BTreeMap::new();
        m.insert("cooldown".to_string(), 1.0);
        assert_eq!(ItemKind::infer(&m), Some(ItemKind::Melee));
        assert_eq!(ItemKind::infer(&BTreeMap::new()), None);
    }
}
