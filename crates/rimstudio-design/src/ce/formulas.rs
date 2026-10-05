//! Combat Extended conversion formulas over data that the caller supplies.
//!
//! This module holds mechanics only: how a piecewise linear curve is evaluated, how a preset claims a gun or an
//! apparel item, and what values a preset produces. It never contains a Combat Extended number. The preset
//! tables, curves, defaults and rules are all parameters; the reader module of the crate builds them from the
//! user's own Combat Extended install at run time.
//!
//! Combat Extended is an optional patch the user chooses to generate. Nothing in this module is applied
//! automatically and nothing here touches the vanilla design.
//!
//! Semantics, written from observed behaviour:
//!
//! * A curve is a list of points sorted by x. Evaluating below the first x gives the first y, above the last x
//!   gives the last y, in between it interpolates linearly. A curve with one point is a constant.
//! * A range includes both of its ends.
//! * A stat that is missing reads as 0.
//! * Presets are tried in definition load order and the first claim wins. A claim is, in order: a label token
//!   in the preset's names, the whole label in the names, the label with designations discarded (when the preset
//!   asks for it), all four verb values inside the preset's ranges, a shared weapon tag, a special gun entry.
//!   When no preset claims a gun, the preset with the largest sum of range averages takes it (first on ties).

use crate::error::{DesignError, DesignResult, finite};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A piecewise linear curve with clamped ends.
///
/// Built with [`Curve::new`], which sorts the points by x. Serialises as a list of `[x, y]` pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<(f64, f64)>", into = "Vec<(f64, f64)>")]
pub struct Curve {
    points: Vec<(f64, f64)>,
}

impl TryFrom<Vec<(f64, f64)>> for Curve {
    type Error = DesignError;

    fn try_from(points: Vec<(f64, f64)>) -> Result<Self, Self::Error> {
        Self::new(points)
    }
}

impl From<Curve> for Vec<(f64, f64)> {
    fn from(curve: Curve) -> Self {
        curve.points
    }
}

impl Curve {
    /// Builds a curve from points in any order; the sort is stable, so equal x values keep their order.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] when a coordinate is not finite.
    pub fn new(points: impl IntoIterator<Item = (f64, f64)>) -> DesignResult<Self> {
        let mut points: Vec<(f64, f64)> = points.into_iter().collect();
        for (x, y) in &points {
            finite("curve.x", *x)?;
            finite("curve.y", *y)?;
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Self { points })
    }

    /// A curve that returns `y` everywhere.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] when `y` is not finite.
    pub fn constant(y: f64) -> DesignResult<Self> {
        Self::new([(0.0, y)])
    }

    /// The points, sorted by x.
    #[must_use]
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// Whether the curve has no points (it then evaluates to 0).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Evaluates the curve at `x`.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] when `x` is not finite.
    pub fn evaluate(&self, x: f64) -> DesignResult<f64> {
        finite("x", x)?;
        Ok(evaluate_sorted(&self.points, x))
    }
}

fn evaluate_sorted(points: &[(f64, f64)], x: f64) -> f64 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if x <= x1 {
            if x1 == x0 {
                return y1;
            }
            return y0 + (y1 - y0) * ((x - x0) / (x1 - x0));
        }
    }
    last.1
}

/// Evaluates the curve through `points` (any order) at `x`.
///
/// An empty list evaluates to 0. Hand checks: points (1.5, 3.0) and (4.0, 5.5) at 2.75 give `3.0 + 2.5 * 0.5`
/// = 4.25; below 1.5 gives 3.0; above 4.0 gives 5.5; a single point is a constant.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite coordinate or `x`.
pub fn interpolate(points: &[(f64, f64)], x: f64) -> DesignResult<f64> {
    Curve::new(points.iter().copied())?.evaluate(x)
}

/// A closed interval; both ends are included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct FloatRange {
    /// Lower end.
    pub min: f64,
    /// Upper end.
    pub max: f64,
}

impl From<[f64; 2]> for FloatRange {
    fn from(v: [f64; 2]) -> Self {
        Self {
            min: v[0],
            max: v[1],
        }
    }
}

impl From<FloatRange> for [f64; 2] {
    fn from(r: FloatRange) -> Self {
        [r.min, r.max]
    }
}

impl FloatRange {
    /// A range from its two ends.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Whether `x` lies inside the range, ends included. `NaN` is never inside.
    #[must_use]
    pub fn includes(&self, x: f64) -> bool {
        self.min <= x && x <= self.max
    }

    fn average(&self) -> f64 {
        (self.min + self.max) / 2.0
    }

    fn widen(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Guns
// ---------------------------------------------------------------------------------------------------------

/// A caliber entry of a preset: which ammo set a gun gets from its projectile values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaliberRange {
    /// Projectile damage range that selects this caliber.
    pub damage_range: FloatRange,
    /// Projectile speed range that selects this caliber.
    pub speed_range: FloatRange,
    /// Ammo set name.
    pub ammo_set: String,
}

/// A named gun model that overrides some values of a preset.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpecialGun {
    /// Lower case name tokens that select this entry.
    pub names: Vec<String>,
    /// Ammo set override.
    pub caliber: Option<String>,
    /// Magazine size override.
    pub mag_cap: Option<f64>,
    /// Reload time override in seconds.
    pub reload_time: Option<f64>,
    /// Mass override in kg.
    pub mass: Option<f64>,
    /// Bulk override.
    pub bulk: Option<f64>,
    /// Extra stats merged into the result.
    pub stats: BTreeMap<String, f64>,
}

/// A gun preset as data. Field meanings follow the preset definitions of the user's install.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GunPreset {
    /// Definition name of the preset.
    pub def_name: String,
    /// Name tokens that claim a gun (compared case insensitively).
    pub names: Vec<String>,
    /// Weapon tags that claim a gun.
    pub tags: Vec<String>,
    /// Whether the label is also tried with capital "A" designations discarded.
    pub discard_designations: bool,
    /// Special gun entries.
    pub special_guns: Vec<SpecialGun>,
    /// Warmup range (seconds) for claiming by verb values.
    pub warmup_range: FloatRange,
    /// Range range (cells) for claiming by verb values.
    pub range_range: FloatRange,
    /// Damage range for claiming by verb values (widened by the caliber ranges).
    pub damage_range: FloatRange,
    /// Projectile speed range for claiming by verb values (widened by the caliber ranges).
    pub proj_speed_range: FloatRange,
    /// Caliber entries; the first whose damage and speed ranges both match wins.
    pub caliber_ranges: Vec<CaliberRange>,
    /// Whether the caliber is chosen from the projectile values.
    pub determine_caliber: bool,
    /// Default ammo set.
    pub set_caliber: String,
    /// Maps the vanilla range to the new range; `None` uses `flat_range`.
    pub range_curve: Option<Curve>,
    /// Maps the vanilla warmup to the new warmup; `None` uses `flat_warmup`.
    pub warmup_curve: Option<Curve>,
    /// Maps the vanilla cooldown to the new cooldown; `None` uses `cooldown_time`.
    pub cooldown_curve: Option<Curve>,
    /// Maps the vanilla mass to the new mass; `None` uses `mass`.
    pub mass_curve: Option<Curve>,
    /// Range used when there is no range curve.
    pub flat_range: f64,
    /// Warmup used when there is no warmup curve.
    pub flat_warmup: f64,
    /// Cooldown used when there is no cooldown curve.
    pub cooldown_time: f64,
    /// Flat mass.
    pub mass: f64,
    /// Bulk.
    pub bulk: f64,
    /// Shot spread.
    pub spread: f64,
    /// Sway factor.
    pub sway: f64,
    /// Sights efficiency, when the preset sets one.
    pub sights_efficiency: Option<f64>,
    /// Recoil, when the preset sets one.
    pub recoil_amount: Option<f64>,
    /// Shots per burst (1 when the preset sets none).
    pub burst_shot_count: f64,
    /// Ticks between burst shots, when the preset sets them.
    pub ticks_between_burst_shots: Option<f64>,
    /// Magazine size.
    pub ammo_capacity: f64,
    /// Reload time in seconds.
    pub reload_time: f64,
}

impl Default for GunPreset {
    fn default() -> Self {
        let zero = FloatRange::new(0.0, 0.0);
        Self {
            def_name: String::new(),
            names: Vec::new(),
            tags: Vec::new(),
            discard_designations: false,
            special_guns: Vec::new(),
            warmup_range: zero,
            range_range: zero,
            damage_range: zero,
            proj_speed_range: zero,
            caliber_ranges: Vec::new(),
            determine_caliber: false,
            set_caliber: String::new(),
            range_curve: None,
            warmup_curve: None,
            cooldown_curve: None,
            mass_curve: None,
            flat_range: 0.0,
            flat_warmup: 0.0,
            cooldown_time: 0.0,
            mass: 0.0,
            bulk: 0.0,
            spread: 0.0,
            sway: 0.0,
            sights_efficiency: None,
            recoil_amount: None,
            burst_shot_count: 1.0,
            ticks_between_burst_shots: None,
            ammo_capacity: 0.0,
            reload_time: 0.0,
        }
    }
}

/// The vanilla values of a gun that presets look at. A missing value reads as 0.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GunInput {
    /// Label of the gun.
    pub label: String,
    /// Weapon tags.
    pub weapon_tags: Vec<String>,
    /// Verb warmup in seconds.
    pub warmup: Option<f64>,
    /// Verb range in cells.
    pub range: Option<f64>,
    /// Projectile damage.
    pub damage: Option<f64>,
    /// Projectile speed.
    pub speed: Option<f64>,
    /// Vanilla mass in kg.
    pub mass: Option<f64>,
    /// Vanilla cooldown stat in seconds.
    pub cooldown: Option<f64>,
}

impl GunInput {
    fn validate(&self) -> DesignResult<()> {
        for (field, value) in [
            ("warmup", self.warmup),
            ("range", self.range),
            ("damage", self.damage),
            ("speed", self.speed),
            ("mass", self.mass),
            ("cooldown", self.cooldown),
        ] {
            if let Some(v) = value {
                finite(field, v)?;
            }
        }
        Ok(())
    }
}

/// Why a preset claimed a gun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchReason {
    /// A token of the label is in the preset's names.
    NameToken,
    /// The whole label is in the preset's names.
    Name,
    /// A token of the label with designations discarded is in the names.
    NameDesignationsDiscarded,
    /// The four verb values are inside the preset's ranges.
    VerbRanges,
    /// A weapon tag is shared.
    Tag,
    /// A special gun entry matches.
    SpecialGun,
    /// No preset claimed the gun; this is the preset with the largest sum of range averages.
    Fallback,
}

impl MatchReason {
    /// Stable string form, the same as the serde form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameToken => "name-token",
            Self::Name => "name",
            Self::NameDesignationsDiscarded => "name-designations-discarded",
            Self::VerbRanges => "verb-ranges",
            Self::Tag => "tag",
            Self::SpecialGun => "special-gun",
            Self::Fallback => "fallback",
        }
    }
}

/// The preset chosen for a gun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetMatch {
    /// Index into the preset slice that was passed in.
    pub index: usize,
    /// Why it was chosen.
    pub reason: MatchReason,
}

fn label_tokens(label: &str) -> Vec<String> {
    label
        .to_lowercase()
        .replace('-', "")
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn designation_tokens(label: &str) -> Vec<String> {
    label
        .replace('A', " ")
        .to_lowercase()
        .replace('-', "")
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn names_contain(names: &[String], token: &str) -> bool {
    names.iter().any(|n| n.to_lowercase() == token)
}

/// Damage and speed match ranges widened to cover every caliber range of the preset.
fn widened_ranges(preset: &GunPreset) -> (FloatRange, FloatRange) {
    let mut damage = preset.damage_range;
    let mut speed = preset.proj_speed_range;
    for caliber in &preset.caliber_ranges {
        damage = damage.widen(&caliber.damage_range);
        speed = speed.widen(&caliber.speed_range);
    }
    (damage, speed)
}

fn match_reason(gun: &GunInput, preset: &GunPreset) -> Option<MatchReason> {
    let tokens = label_tokens(&gun.label);
    if tokens.iter().any(|t| names_contain(&preset.names, t)) {
        return Some(MatchReason::NameToken);
    }
    if names_contain(&preset.names, &gun.label.to_lowercase()) {
        return Some(MatchReason::Name);
    }
    if preset.discard_designations
        && designation_tokens(&gun.label)
            .iter()
            .any(|t| names_contain(&preset.names, t))
    {
        return Some(MatchReason::NameDesignationsDiscarded);
    }
    let (damage, speed) = widened_ranges(preset);
    if preset.warmup_range.includes(gun.warmup.unwrap_or(0.0))
        && preset.range_range.includes(gun.range.unwrap_or(0.0))
        && damage.includes(gun.damage.unwrap_or(0.0))
        && speed.includes(gun.speed.unwrap_or(0.0))
    {
        return Some(MatchReason::VerbRanges);
    }
    if preset.tags.iter().any(|t| gun.weapon_tags.contains(t)) {
        return Some(MatchReason::Tag);
    }
    if preset
        .special_guns
        .iter()
        .any(|sg| sg.names.contains(&gun.label) || sg.names.iter().any(|n| tokens.contains(n)))
    {
        return Some(MatchReason::SpecialGun);
    }
    None
}

fn fallback_index(presets: &[GunPreset]) -> usize {
    let score = |p: &GunPreset| {
        let (damage, speed) = widened_ranges(p);
        damage.average() + p.range_range.average() + speed.average() + p.warmup_range.average()
    };
    let mut best = 0;
    for (i, p) in presets.iter().enumerate().skip(1) {
        if score(p) > score(&presets[best]) {
            best = i;
        }
    }
    best
}

/// Chooses the preset for a gun.
///
/// Presets are tried in the given order and the first claim wins; see the module documentation for the order of
/// the tests. A gun nobody claims goes to the fallback preset, and the result says so
/// ([`MatchReason::Fallback`]) so the caller can warn instead of silently applying a preset.
///
/// Hand check with fictional presets A (names `zap`, damage 4 to 9) and B (names `blip`, damage 16 to 24 widened
/// by calibers to 16 to 30): the label `Zap Mk3` has the token `zap`, so A claims it by name token; the label
/// `x-blip` becomes `xblip`, which is not the token `blip`, so nothing claims it and B takes it as fallback
/// (its range averages sum to more than A's).
///
/// # Errors
///
/// [`DesignError::EmptyInput`] without presets, [`DesignError::InvalidInput`] for a non finite gun value.
pub fn preset_for(gun: &GunInput, presets: &[GunPreset]) -> DesignResult<PresetMatch> {
    if presets.is_empty() {
        return Err(DesignError::EmptyInput {
            what: "gun presets",
        });
    }
    gun.validate()?;
    for (index, preset) in presets.iter().enumerate() {
        if let Some(reason) = match_reason(gun, preset) {
            return Ok(PresetMatch { index, reason });
        }
    }
    Ok(PresetMatch {
        index: fallback_index(presets),
        reason: MatchReason::Fallback,
    })
}

/// The values a preset gives a gun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GunValues {
    /// Verb range in cells.
    pub range: f64,
    /// Warmup in seconds.
    pub warmup: f64,
    /// Mass in kg.
    pub mass: f64,
    /// Cooldown in seconds.
    pub cooldown: f64,
    /// Bulk.
    pub bulk: f64,
    /// Shot spread.
    pub spread: f64,
    /// Sway factor.
    pub sway: f64,
    /// Sights efficiency, when set.
    pub sights: Option<f64>,
    /// Recoil, when set.
    pub recoil: Option<f64>,
    /// Shots per burst.
    pub burst: f64,
    /// Ticks between burst shots, when set.
    pub ticks_between: Option<f64>,
    /// Magazine size.
    pub magazine: f64,
    /// Reload time in seconds.
    pub reload: f64,
    /// Chosen ammo set.
    pub ammo_set: String,
    /// Extra stats merged from a special gun entry.
    pub extra: BTreeMap<String, f64>,
}

/// Values a preset produces for a gun.
///
/// Range, warmup, cooldown and mass go through the preset's curves at the gun's vanilla values (a missing value
/// reads 0) or fall back to the preset's flat values; mass uses its curve only when the gun has a mass.
/// Bulk, spread, sway, burst, magazine and reload are preset constants. The ammo set is the first caliber range
/// whose damage and speed ranges both include the gun's projectile values when the preset determines the
/// caliber, else the default. A special gun whose name token appears in the label overrides magazine, reload,
/// mass, bulk and the caliber, and merges its extra stats.
///
/// Projectile damage and penetration are not computed: they come from the chosen ammo set.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite gun value.
pub fn gun_values(gun: &GunInput, preset: &GunPreset) -> DesignResult<GunValues> {
    gun.validate()?;
    let range = match &preset.range_curve {
        Some(c) if !c.is_empty() => c.evaluate(gun.range.unwrap_or(0.0))?,
        _ => preset.flat_range,
    };
    let warmup = match &preset.warmup_curve {
        Some(c) if !c.is_empty() => c.evaluate(gun.warmup.unwrap_or(0.0))?,
        _ => preset.flat_warmup,
    };
    let cooldown = match &preset.cooldown_curve {
        Some(c) if !c.is_empty() => c.evaluate(gun.cooldown.unwrap_or(0.0))?,
        _ => preset.cooldown_time,
    };
    let mass = match (&preset.mass_curve, gun.mass) {
        (Some(c), Some(m)) if !c.is_empty() => c.evaluate(m)?,
        _ => preset.mass,
    };
    let tokens = label_tokens(&gun.label);
    let special = preset
        .special_guns
        .iter()
        .find(|sg| sg.names.iter().any(|n| tokens.contains(n)));
    let mut ammo_set = special
        .and_then(|sg| sg.caliber.clone())
        .unwrap_or_else(|| preset.set_caliber.clone());
    if preset.determine_caliber {
        let damage = gun.damage.unwrap_or(0.0);
        let speed = gun.speed.unwrap_or(0.0);
        if let Some(c) = preset
            .caliber_ranges
            .iter()
            .find(|c| c.damage_range.includes(damage) && c.speed_range.includes(speed))
        {
            ammo_set.clone_from(&c.ammo_set);
        }
    }
    let mut values = GunValues {
        range,
        warmup,
        mass,
        cooldown,
        bulk: preset.bulk,
        spread: preset.spread,
        sway: preset.sway,
        sights: preset.sights_efficiency,
        recoil: preset.recoil_amount,
        burst: preset.burst_shot_count,
        ticks_between: preset.ticks_between_burst_shots,
        magazine: preset.ammo_capacity,
        reload: preset.reload_time,
        ammo_set,
        extra: BTreeMap::new(),
    };
    if let Some(sg) = special {
        values.magazine = sg.mag_cap.unwrap_or(values.magazine);
        values.reload = sg.reload_time.unwrap_or(values.reload);
        values.mass = sg.mass.unwrap_or(values.mass);
        values.bulk = sg.bulk.unwrap_or(values.bulk);
        values.extra.clone_from(&sg.stats);
    }
    Ok(values)
}

// ---------------------------------------------------------------------------------------------------------
// Apparel
// ---------------------------------------------------------------------------------------------------------

/// An apparel preset as data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApparelPreset {
    /// Definition name of the preset.
    pub def_name: String,
    /// Every layer of the apparel must be in this list.
    pub needed_layers: Vec<String>,
    /// Every body part group of the apparel must be in this list.
    pub needed_groups: Vec<String>,
    /// The vanilla armor range that claims an item (sharp rating or stuff multiplier inside it).
    pub vanilla_armor_rating_range: FloatRange,
    /// Bulk written to the item.
    pub bulk: f64,
    /// Worn bulk written to the item.
    pub bulk_worn: f64,
    /// Mass written to the item; `None` writes 0.
    pub mass: Option<f64>,
    /// Sharp rating curve; `None` uses `static_sharp`.
    pub sharp_curve: Option<Curve>,
    /// Blunt rating curve; `None` uses `static_blunt`.
    pub blunt_curve: Option<Curve>,
    /// Sharp rating when there is no curve.
    pub static_sharp: f64,
    /// Blunt rating when there is no curve.
    pub static_blunt: f64,
    /// Whether the preset adds partial armor entries.
    pub has_partial_armor: bool,
}

impl Default for ApparelPreset {
    fn default() -> Self {
        Self {
            def_name: String::new(),
            needed_layers: Vec::new(),
            needed_groups: Vec::new(),
            vanilla_armor_rating_range: FloatRange::new(0.0, 0.0),
            bulk: 0.0,
            bulk_worn: 0.0,
            mass: None,
            sharp_curve: None,
            blunt_curve: None,
            static_sharp: 0.0,
            static_blunt: 0.0,
            has_partial_armor: false,
        }
    }
}

/// The vanilla values of an apparel item that presets look at. A missing stat reads as 0.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApparelInput {
    /// Apparel layers.
    pub layers: Vec<String>,
    /// Body part groups.
    pub groups: Vec<String>,
    /// Explicit sharp rating from the stat bases, if the item has one.
    pub sharp: Option<f64>,
    /// Explicit blunt rating from the stat bases, if the item has one.
    pub blunt: Option<f64>,
    /// The stuff effect multiplier for armor from the stat bases, if the item has one.
    pub stuff_multiplier_armor: Option<f64>,
}

impl ApparelInput {
    fn validate(&self) -> DesignResult<()> {
        for (field, value) in [
            ("sharp", self.sharp),
            ("blunt", self.blunt),
            ("stuff_multiplier_armor", self.stuff_multiplier_armor),
        ] {
            if let Some(v) = value {
                finite(field, v)?;
            }
        }
        Ok(())
    }
}

/// Chooses the apparel preset for an item, or `None` when no preset claims it (the item is left alone).
///
/// A preset claims an item when every layer is in its needed layers, every group is in its needed groups, and
/// its vanilla armor range includes the sharp rating or the stuff multiplier (a missing stat reads 0, so an
/// item without armor stats lies inside any range that contains 0). Presets are tried in order.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite rating.
pub fn apparel_preset_for(
    item: &ApparelInput,
    presets: &[ApparelPreset],
) -> DesignResult<Option<usize>> {
    item.validate()?;
    let sharp = item.sharp.unwrap_or(0.0);
    let multiplier = item.stuff_multiplier_armor.unwrap_or(0.0);
    Ok(presets.iter().position(|p| {
        item.layers.iter().all(|l| p.needed_layers.contains(l))
            && item.groups.iter().all(|g| p.needed_groups.contains(g))
            && (p.vanilla_armor_rating_range.includes(sharp)
                || p.vanilla_armor_rating_range.includes(multiplier))
    }))
}

/// The values an apparel preset gives an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApparelValues {
    /// Sharp rating (mm RHA).
    pub sharp: f64,
    /// Blunt rating (MPa).
    pub blunt: f64,
    /// Bulk.
    pub bulk: f64,
    /// Worn bulk.
    pub worn_bulk: f64,
    /// Mass in kg (0 when the preset sets none).
    pub mass: f64,
    /// Whether partial armor entries are added.
    pub partial_armor: bool,
}

/// Values an apparel preset produces for an item.
///
/// With an explicit sharp stat, the sharp curve is evaluated at it and the blunt curve at the blunt stat
/// (0 when absent). Without one, both curves are evaluated at the stuff multiplier (0 when absent). A preset
/// without a curve for a rating writes its static value. The result replaces stuff scaling with plain ratings.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite rating.
pub fn apparel_values(item: &ApparelInput, preset: &ApparelPreset) -> DesignResult<ApparelValues> {
    item.validate()?;
    let eval = |curve: &Option<Curve>, fixed: f64, x: f64| -> DesignResult<f64> {
        match curve {
            Some(c) if !c.is_empty() => c.evaluate(x),
            _ => Ok(fixed),
        }
    };
    let (sharp, blunt) = if let Some(s) = item.sharp {
        (
            eval(&preset.sharp_curve, preset.static_sharp, s)?,
            eval(
                &preset.blunt_curve,
                preset.static_blunt,
                item.blunt.unwrap_or(0.0),
            )?,
        )
    } else {
        let x = item.stuff_multiplier_armor.unwrap_or(0.0);
        (
            eval(&preset.sharp_curve, preset.static_sharp, x)?,
            eval(&preset.blunt_curve, preset.static_blunt, x)?,
        )
    };
    Ok(ApparelValues {
        sharp,
        blunt,
        bulk: preset.bulk,
        worn_bulk: preset.bulk_worn,
        mass: preset.mass.unwrap_or(0.0),
        partial_armor: preset.has_partial_armor,
    })
}

// ---------------------------------------------------------------------------------------------------------
// Melee tools, toughness, race armor
// ---------------------------------------------------------------------------------------------------------

/// Defaults used when converting a melee tool. Supplied by the caller from the user's install.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefaults {
    /// Cooldown in seconds used when the tool has none (0 or negative).
    pub cooldown: f64,
    /// Sharp penetration used when the tool has no positive penetration.
    pub ap_sharp: f64,
    /// Blunt penetration used when the tool has no positive penetration.
    pub ap_blunt: f64,
}

/// A vanilla tool, reduced to the numbers the conversion looks at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInput {
    /// Tool power.
    pub power: f64,
    /// Cooldown in seconds; `None`, 0 or negative means unset.
    pub cooldown: Option<f64>,
    /// Armor penetration; `None`, 0 or negative means unset.
    pub armor_penetration: Option<f64>,
}

/// A converted tool.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedTool {
    /// Power, copied.
    pub power: f64,
    /// Cooldown in seconds.
    pub cooldown: f64,
    /// Sharp penetration.
    pub ap_sharp: f64,
    /// Blunt penetration.
    pub ap_blunt: f64,
}

/// Converts a vanilla tool: power is copied, a missing cooldown becomes the default, and the penetration is
/// copied to both the sharp and the blunt value unless it is missing, in which case the two defaults apply.
///
/// Hand check with defaults (cooldown 1.5, sharp 0.4, blunt 1.2): a tool with power 8, cooldown 0 and no
/// penetration becomes (8, 1.5, 0.4, 1.2); power 10, cooldown 2.5 and penetration 1.2 becomes
/// (10, 2.5, 1.2, 1.2).
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite number.
pub fn convert_tool(tool: &ToolInput, defaults: &ToolDefaults) -> DesignResult<ConvertedTool> {
    let power = finite("power", tool.power)?;
    let cooldown = match tool.cooldown {
        Some(c) if finite("cooldown", c)? > 0.0 => c,
        _ => finite("defaults.cooldown", defaults.cooldown)?,
    };
    let (ap_sharp, ap_blunt) = match tool.armor_penetration {
        Some(ap) if finite("armor_penetration", ap)? > 0.0 => (ap, ap),
        _ => (
            finite("defaults.ap_sharp", defaults.ap_sharp)?,
            finite("defaults.ap_blunt", defaults.ap_blunt)?,
        ),
    };
    Ok(ConvertedTool {
        power,
        cooldown,
        ap_sharp,
        ap_blunt,
    })
}

/// Rules of the toughness conversion. Supplied by the caller from the user's install.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToughnessRules {
    /// Thickness multiplier by tech level name; a level that is not listed multiplies by 1.
    pub tech_multipliers: BTreeMap<String, f64>,
    /// Capacity names that count as sharp.
    pub sharp_capacities: Vec<String>,
    /// Extra factor for melee weapons none of whose capacities are sharp.
    pub blunt_only_factor: f64,
}

/// Toughness thickness of a weapon: `sqrt(bulk)`, times the tech level multiplier, times the blunt-only factor
/// for melee weapons without a sharp capacity. `None` when the weapon has no bulk (0).
///
/// Hand check with multipliers `Mid = 3`, `High = 5`, `Top = 9` and blunt-only factor 2: bulk 16 at the base
/// level with a sharp capacity gives `sqrt(16) = 4`; bulk 16 at `Mid` gives 12; bulk 16 blunt only at the base
/// level gives 8; bulk 25 at `High` blunt only gives `5 * 5 * 2 = 50`; bulk 25 ranged at `Top` gives
/// `5 * 9 = 45`.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a negative or non finite bulk or a non finite rule.
pub fn stuff_toughness_multiplier(
    rules: &ToughnessRules,
    bulk: f64,
    tech: &str,
    ranged: bool,
    capacities: &[String],
) -> DesignResult<Option<f64>> {
    finite("bulk", bulk)?;
    if bulk < 0.0 {
        return Err(DesignError::invalid("bulk", "must not be negative"));
    }
    if bulk == 0.0 {
        return Ok(None);
    }
    let mut thickness = bulk.sqrt();
    if let Some(m) = rules.tech_multipliers.get(tech) {
        thickness *= finite("tech_multiplier", *m)?;
    }
    if !ranged
        && !capacities
            .iter()
            .any(|c| rules.sharp_capacities.contains(c))
    {
        thickness *= finite("blunt_only_factor", rules.blunt_only_factor)?;
    }
    Ok(Some(thickness))
}

/// Toughness rating of a non stuffable weapon: the thickness of [`stuff_toughness_multiplier`] times the
/// strongest main ingredient's sharp power times the ingredient's optional toughness multiplier.
///
/// # Errors
///
/// See [`stuff_toughness_multiplier`]; also for a non finite ingredient number.
pub fn toughness_rating_fixed(
    rules: &ToughnessRules,
    bulk: f64,
    tech: &str,
    ranged: bool,
    capacities: &[String],
    ingredient_sharp_power: f64,
    toughness_multiplier: f64,
) -> DesignResult<Option<f64>> {
    let thickness = stuff_toughness_multiplier(rules, bulk, tech, ranged, capacities)?;
    Ok(match thickness {
        Some(t) => Some(
            t * finite("ingredient_sharp_power", ingredient_sharp_power)?
                * finite("toughness_multiplier", toughness_multiplier)?,
        ),
        None => None,
    })
}

/// Rules of the animal and alien race armor conversion. Supplied by the caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceArmorRules {
    /// Sharp rating curve.
    pub sharp_curve: Curve,
    /// Blunt rating curve.
    pub blunt_curve: Curve,
    /// Sharp rating written when the race has none.
    pub default_sharp: f64,
    /// Blunt rating written when the race has none.
    pub default_blunt: f64,
}

/// Converts the vanilla armor ratings of a race: each present rating goes through its curve, an absent one gets
/// the default. Returns `(sharp, blunt)`.
///
/// Hand check with curves sharp (0.1, 1) to (1.9, 19) and blunt (0.1, 3) to (1.9, 39): both ratings 1.0 give
/// `(10, 21)`; ratings far outside clamp to the end values.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite number.
pub fn race_armor(
    rules: &RaceArmorRules,
    sharp: Option<f64>,
    blunt: Option<f64>,
) -> DesignResult<(f64, f64)> {
    let s = match sharp {
        Some(v) => rules.sharp_curve.evaluate(v)?,
        None => finite("default_sharp", rules.default_sharp)?,
    };
    let b = match blunt {
        Some(v) => rules.blunt_curve.evaluate(v)?,
        None => finite("default_blunt", rules.default_blunt)?,
    };
    Ok((s, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // ---- curves: 13 cases ---------------------------------------------------------------------------

    #[rstest]
    // Two point curve (1.5, 3.0) to (4.0, 5.5): ends, clamps, midpoint.
    #[case(&[(1.5, 3.0), (4.0, 5.5)], 4.0, 5.5)] // last point
    #[case(&[(1.5, 3.0), (4.0, 5.5)], 1.5, 3.0)] // first point
    #[case(&[(1.5, 3.0), (4.0, 5.5)], 0.5, 3.0)] // below the range clamps to the first y
    #[case(&[(1.5, 3.0), (4.0, 5.5)], 9.0, 5.5)] // above the range clamps to the last y
    #[case(&[(1.5, 3.0), (4.0, 5.5)], 2.75, 4.25)] // midpoint (3.0 + 5.5) / 2
    // A single point is a constant.
    #[case(&[(12.0, 40.0)], 1.0, 40.0)]
    #[case(&[(12.0, 40.0)], 99.0, 40.0)]
    // Unsorted input: sorted to (14, 4) and (20, 8); x = 17 is the midpoint: 4 + 4 * 0.5 = 6.
    #[case(&[(20.0, 8.0), (14.0, 4.0)], 17.0, 6.0)]
    // Steep segment: 1.0 + 2.0 * (0.1 / 0.5) = 1.4.
    #[case(&[(2.0, 1.0), (2.5, 3.0)], 2.1, 1.4)]
    // Four points, x between the 2nd and 3rd: 7 + 1 * 0.5 = 7.5.
    #[case(&[(0.5, 6.0), (0.6, 7.0), (0.7, 8.0), (0.8, 9.0)], 0.65, 7.5)]
    // 2.0 + 6.0 * 0.5 = 5.0.
    #[case(&[(0.3, 2.0), (0.9, 8.0)], 0.6, 5.0)]
    // Wide segments: 1 + 18 * 0.5 = 10 and 3 + 36 * 0.5 = 21.
    #[case(&[(0.1, 1.0), (1.9, 19.0)], 1.0, 10.0)]
    #[case(&[(0.1, 3.0), (1.9, 39.0)], 1.0, 21.0)]
    fn curve_evaluation(#[case] points: &[(f64, f64)], #[case] x: f64, #[case] expected: f64) {
        let got = interpolate(points, x).unwrap();
        assert!(close(got, expected), "got {got}, expected {expected}");
        let curve = Curve::new(points.iter().copied()).unwrap();
        assert!(close(curve.evaluate(x).unwrap(), expected));
    }

    #[test]
    fn empty_curve_reads_zero() {
        assert_eq!(interpolate(&[], 5.0).unwrap(), 0.0);
        assert!(Curve::new([]).unwrap().is_empty());
    }

    #[test]
    fn duplicate_x_does_not_divide_by_zero() {
        // Points (1, 2), (1, 6), (3, 8): at x = 1 the first y wins (x <= first x); at 2 it interpolates the
        // second and third points: 6 + 2 * 0.5 = 7.
        let curve = Curve::new([(1.0, 2.0), (1.0, 6.0), (3.0, 8.0)]).unwrap();
        assert_eq!(curve.evaluate(1.0).unwrap(), 2.0);
        assert!(close(curve.evaluate(2.0).unwrap(), 7.0));
    }

    #[test]
    fn curve_rejects_non_finite_values() {
        assert!(Curve::new([(f64::NAN, 1.0)]).is_err());
        assert!(Curve::new([(1.0, f64::INFINITY)]).is_err());
        let curve = Curve::constant(2.0).unwrap();
        assert!(curve.evaluate(f64::NAN).is_err());
        assert_eq!(curve.evaluate(-5.0).unwrap(), 2.0);
    }

    #[test]
    fn curve_serialises_as_pairs() {
        let curve = Curve::new([(2.0, 4.0), (1.0, 3.0)]).unwrap();
        let json = serde_json::to_string(&curve).unwrap();
        assert_eq!(json, "[[1.0,3.0],[2.0,4.0]]");
        let back: Curve = serde_json::from_str(&json).unwrap();
        assert_eq!(back, curve);
        assert!(serde_json::from_str::<Curve>("[[1.0, null]]").is_err());
    }

    // ---- ranges: 3 cases ----------------------------------------------------------------------------

    #[rstest]
    #[case(5.0, true)] // inclusive low
    #[case(15.0, true)] // inclusive high
    #[case(15.01, false)] // outside
    fn range_inclusion(#[case] x: f64, #[case] expected: bool) {
        assert_eq!(FloatRange::new(5.0, 15.0).includes(x), expected);
    }

    #[test]
    fn range_never_includes_nan_and_round_trips() {
        assert!(!FloatRange::new(0.0, 1.0).includes(f64::NAN));
        let json = serde_json::to_string(&FloatRange::new(1.0, 2.5)).unwrap();
        assert_eq!(json, "[1.0,2.5]");
        let back: FloatRange = serde_json::from_str(&json).unwrap();
        assert_eq!(back, FloatRange::new(1.0, 2.5));
    }

    // ---- tool conversion: 3 cases -------------------------------------------------------------------

    fn tool_defaults() -> ToolDefaults {
        ToolDefaults {
            cooldown: 1.5,
            ap_sharp: 0.4,
            ap_blunt: 1.2,
        }
    }

    #[rstest]
    // No cooldown and no penetration: both defaults.
    #[case(8.0, Some(0.0), Some(0.0), (8.0, 1.5, 0.4, 1.2))]
    // Penetration is copied to both kinds; the cooldown is kept.
    #[case(10.0, Some(2.5), Some(1.2), (10.0, 2.5, 1.2, 1.2))]
    // Missing penetration counts as zero.
    #[case(10.0, Some(2.5), None, (10.0, 2.5, 0.4, 1.2))]
    fn tool_conversion(
        #[case] power: f64,
        #[case] cooldown: Option<f64>,
        #[case] ap: Option<f64>,
        #[case] expected: (f64, f64, f64, f64),
    ) {
        let got = convert_tool(
            &ToolInput {
                power,
                cooldown,
                armor_penetration: ap,
            },
            &tool_defaults(),
        )
        .unwrap();
        assert!(close(got.power, expected.0));
        assert!(close(got.cooldown, expected.1));
        assert!(close(got.ap_sharp, expected.2));
        assert!(close(got.ap_blunt, expected.3));
    }

    #[test]
    fn tool_without_cooldown_value_uses_the_default() {
        let got = convert_tool(
            &ToolInput {
                power: 1.0,
                cooldown: None,
                armor_penetration: Some(0.3),
            },
            &tool_defaults(),
        )
        .unwrap();
        assert_eq!(got.cooldown, 1.5);
        assert_eq!(got.ap_sharp, 0.3);
    }

    // ---- toughness: 6 cases -------------------------------------------------------------------------

    fn toughness_rules() -> ToughnessRules {
        ToughnessRules {
            tech_multipliers: [("Mid", 3.0), ("High", 5.0), ("Top", 9.0)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            sharp_capacities: vec!["Cut".to_owned(), "Stab".to_owned()],
            blunt_only_factor: 2.0,
        }
    }

    #[rstest]
    #[case(16.0, "Base", false, &["Cut"], Some(4.0))] // sqrt(16)
    #[case(16.0, "Mid", false, &["Stab"], Some(12.0))] // sqrt(16) * 3
    #[case(16.0, "Base", false, &["Blunt"], Some(8.0))] // blunt only doubles: 4 * 2
    #[case(25.0, "High", false, &["Blunt"], Some(50.0))] // 5 * 5 * 2
    #[case(25.0, "Top", true, &[], Some(45.0))] // ranged is not doubled: 5 * 9
    #[case(0.0, "Base", false, &["Cut"], None)] // no bulk, no toughness
    fn toughness_cases(
        #[case] bulk: f64,
        #[case] tech: &str,
        #[case] ranged: bool,
        #[case] caps: &[&str],
        #[case] expected: Option<f64>,
    ) {
        let caps: Vec<String> = caps.iter().map(|c| (*c).to_owned()).collect();
        let got =
            stuff_toughness_multiplier(&toughness_rules(), bulk, tech, ranged, &caps).unwrap();
        match (got, expected) {
            (Some(g), Some(e)) => assert!(close(g, e), "got {g}"),
            (None, None) => {}
            other => panic!("mismatch {other:?}"),
        }
    }

    #[test]
    fn toughness_rating_scales_by_the_ingredient() {
        // Thickness 4 (bulk 16, sharp capacity) * ingredient power 0.8 * multiplier 1.5 = 4.8.
        let caps = vec!["Cut".to_owned()];
        let got = toughness_rating_fixed(&toughness_rules(), 16.0, "Base", false, &caps, 0.8, 1.5)
            .unwrap()
            .unwrap();
        assert!(close(got, 4.8));
        assert!(
            toughness_rating_fixed(&toughness_rules(), 0.0, "Base", false, &caps, 0.8, 1.5)
                .unwrap()
                .is_none()
        );
        assert!(stuff_toughness_multiplier(&toughness_rules(), -1.0, "Base", true, &[]).is_err());
    }

    // ---- race armor: 3 cases ------------------------------------------------------------------------

    fn race_rules() -> RaceArmorRules {
        RaceArmorRules {
            sharp_curve: Curve::new([(0.1, 1.0), (1.9, 19.0)]).unwrap(),
            blunt_curve: Curve::new([(0.1, 3.0), (1.9, 39.0)]).unwrap(),
            default_sharp: 0.2,
            default_blunt: 1.5,
        }
    }

    #[rstest]
    #[case(Some(1.0), Some(1.0), (10.0, 21.0))] // both curves at 1.0
    #[case(None, None, (0.2, 1.5))] // defaults
    #[case(Some(0.01), Some(50.0), (1.0, 39.0))] // clamped on both ends
    fn race_cases(
        #[case] sharp: Option<f64>,
        #[case] blunt: Option<f64>,
        #[case] expected: (f64, f64),
    ) {
        let (s, b) = race_armor(&race_rules(), sharp, blunt).unwrap();
        assert!(close(s, expected.0) && close(b, expected.1), "got {s} {b}");
    }

    // ---- gun presets: classification 6, patch 4 -----------------------------------------------------

    fn preset_a() -> GunPreset {
        GunPreset {
            def_name: "RS_A".into(),
            names: vec!["zap".into()],
            tags: vec!["RS_TagA".into()],
            warmup_range: FloatRange::new(0.8, 1.8),
            range_range: FloatRange::new(18.0, 32.0),
            damage_range: FloatRange::new(4.0, 9.0),
            proj_speed_range: FloatRange::new(40.0, 56.0),
            set_caliber: "RS_SetA".into(),
            range_curve: Some(Curve::new([(22.0, 36.0)]).unwrap()),
            warmup_curve: Some(Curve::new([(1.3, 0.9)]).unwrap()),
            cooldown_curve: Some(Curve::new([(0.9, 0.5)]).unwrap()),
            mass_curve: Some(Curve::new([(1.5, 2.0), (3.5, 3.0)]).unwrap()),
            mass: 2.5,
            bulk: 6.0,
            spread: 0.2,
            sway: 0.9,
            recoil_amount: Some(1.2),
            burst_shot_count: 4.0,
            ammo_capacity: 24.0,
            reload_time: 3.0,
            ..GunPreset::default()
        }
    }

    fn preset_b() -> GunPreset {
        GunPreset {
            def_name: "RS_B".into(),
            names: vec!["blip".into()],
            tags: vec!["RS_TagB".into()],
            warmup_range: FloatRange::new(2.0, 4.5),
            range_range: FloatRange::new(36.0, 64.0),
            damage_range: FloatRange::new(16.0, 24.0),
            proj_speed_range: FloatRange::new(70.0, 110.0),
            caliber_ranges: vec![
                CaliberRange {
                    damage_range: FloatRange::new(16.0, 20.0),
                    speed_range: FloatRange::new(70.0, 90.0),
                    ammo_set: "RS_SetB1".into(),
                },
                CaliberRange {
                    damage_range: FloatRange::new(21.0, 30.0),
                    speed_range: FloatRange::new(80.0, 120.0),
                    ammo_set: "RS_SetB2".into(),
                },
            ],
            determine_caliber: true,
            set_caliber: "RS_SetB0".into(),
            range_curve: Some(Curve::new([(40.0, 66.0)]).unwrap()),
            warmup_curve: Some(Curve::new([(3.0, 2.2)]).unwrap()),
            cooldown_curve: Some(Curve::new([(1.8, 1.6)]).unwrap()),
            mass_curve: Some(Curve::new([(4.0, 5.5)]).unwrap()),
            mass: 6.0,
            bulk: 11.0,
            sights_efficiency: Some(1.8),
            ammo_capacity: 5.0,
            reload_time: 4.0,
            ..GunPreset::default()
        }
    }

    fn gun(
        label: &str,
        tags: &[&str],
        warmup: f64,
        range: f64,
        damage: f64,
        speed: f64,
    ) -> GunInput {
        GunInput {
            label: label.into(),
            weapon_tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            warmup: Some(warmup),
            range: Some(range),
            damage: Some(damage),
            speed: Some(speed),
            mass: Some(2.5),
            cooldown: Some(1.0),
        }
    }

    #[rstest]
    // A name token, case folded: `zap` is in A's names.
    #[case(gun("Zap Mk3", &[], 9.0, 9.0, 1.0, 1.0), 0, MatchReason::NameToken)]
    // Hyphen removal gives `xblip`, which is not the token `blip`; nothing claims it, B has the larger sum
    // of range averages (A: 6.5 + 25 + 48 + 1.3 = 80.8, B: 23 + 50 + 95 + 3.25 = 171.25).
    #[case(gun("x-blip", &[], 9.0, 9.0, 1.0, 1.0), 1, MatchReason::Fallback)]
    // Tag intersection.
    #[case(gun("gadget", &["RS_TagB"], 9.0, 9.0, 1.0, 1.0), 1, MatchReason::Tag)]
    // All four values on the inclusive boundary of A.
    #[case(gun("gadget", &[], 0.8, 18.0, 4.0, 40.0), 0, MatchReason::VerbRanges)]
    // Inside B: damage 27 and speed 115 are covered because B's caliber ranges widen its ranges to 16 to 30
    // and 70 to 120.
    #[case(gun("gadget", &[], 3.0, 50.0, 27.0, 115.0), 1, MatchReason::VerbRanges)]
    // Nobody claims it: fallback to B.
    #[case(gun("gadget", &[], 0.1, 1.0, 1.0, 1.0), 1, MatchReason::Fallback)]
    fn classification(#[case] gun: GunInput, #[case] index: usize, #[case] reason: MatchReason) {
        let got = preset_for(&gun, &[preset_a(), preset_b()]).unwrap();
        assert_eq!((got.index, got.reason), (index, reason));
    }

    #[test]
    fn first_claimant_wins_and_whole_label_is_tried() {
        let mut a = preset_a();
        a.names = vec!["big zap".into()];
        // The whole lower cased label matches although no single token does.
        let got = preset_for(&gun("Big Zap", &[], 9.0, 9.0, 1.0, 1.0), &[a, preset_b()]).unwrap();
        assert_eq!((got.index, got.reason), (0, MatchReason::Name));
        // `big` and `zap` tokens: names "big zap" matches neither token, so the whole label test fires.
        let mut a = preset_a();
        a.names = vec!["big zap".into()];
        let got = preset_for(&gun("Big-Zap", &[], 9.0, 9.0, 1.0, 1.0), &[a, preset_b()]).unwrap();
        assert_eq!(got.reason, MatchReason::Fallback);
    }

    #[test]
    fn whole_label_claims_when_no_token_does() {
        let mut a = preset_a();
        a.names = vec!["big zap".into()];
        let got = preset_for(&gun("Big Zap", &[], 9.0, 9.0, 1.0, 1.0), &[a, preset_b()]).unwrap();
        assert_eq!(got.reason, MatchReason::Name);
    }

    #[test]
    fn designations_are_discarded_only_when_asked() {
        // Capital A becomes a space: "ZapA2" -> "Zap 2" -> tokens `zap` and `2`.
        let label = "ZapA2";
        let plain = preset_for(
            &gun(label, &[], 9.0, 9.0, 1.0, 1.0),
            &[preset_a(), preset_b()],
        )
        .unwrap();
        assert_eq!(plain.reason, MatchReason::Fallback);
        let mut a = preset_a();
        a.discard_designations = true;
        let got = preset_for(&gun(label, &[], 9.0, 9.0, 1.0, 1.0), &[a, preset_b()]).unwrap();
        assert_eq!(
            (got.index, got.reason),
            (0, MatchReason::NameDesignationsDiscarded)
        );
    }

    #[test]
    fn special_gun_claims_by_token_or_exact_label() {
        let mut b = preset_b();
        b.special_guns.push(SpecialGun {
            names: vec!["rs_sidearm".into()],
            ..SpecialGun::default()
        });
        let presets = [preset_a(), b];
        let by_token =
            preset_for(&gun("rs_sidearm Mk2", &[], 9.0, 9.0, 1.0, 1.0), &presets).unwrap();
        assert_eq!(
            (by_token.index, by_token.reason),
            (1, MatchReason::SpecialGun)
        );
        let exact = preset_for(&gun("rs_sidearm", &[], 9.0, 9.0, 1.0, 1.0), &presets).unwrap();
        assert_eq!(exact.index, 1);
    }

    #[test]
    fn preset_for_error_cases() {
        let g = gun("x", &[], 1.0, 1.0, 1.0, 1.0);
        assert_eq!(
            preset_for(&g, &[]).unwrap_err().code(),
            "design.empty-input"
        );
        let mut bad = g;
        bad.damage = Some(f64::NAN);
        assert_eq!(
            preset_for(&bad, &[preset_a()]).unwrap_err().code(),
            "design.invalid-input"
        );
    }

    #[test]
    fn missing_verb_values_read_zero() {
        let mut g = gun("gadget", &[], 0.0, 0.0, 0.0, 0.0);
        g.warmup = None;
        g.range = None;
        g.damage = None;
        g.speed = None;
        let mut a = preset_a();
        a.warmup_range = FloatRange::new(0.0, 1.0);
        a.range_range = FloatRange::new(0.0, 1.0);
        a.damage_range = FloatRange::new(0.0, 1.0);
        a.proj_speed_range = FloatRange::new(0.0, 1.0);
        let got = preset_for(&g, &[a, preset_b()]).unwrap();
        assert_eq!(got.reason, MatchReason::VerbRanges);
    }

    #[test]
    fn patch_with_single_point_curves_is_constant() {
        // Preset A at mass 3.0: mass curve (1.5, 2.0) to (3.5, 3.0) gives 2.0 + 1.0 * (1.5 / 2.0) = 2.75.
        let mut g = gun("gadget", &[], 1.2, 28.0, 8.0, 55.0);
        g.mass = Some(3.0);
        g.cooldown = Some(1.7);
        let v = gun_values(&g, &preset_a()).unwrap();
        assert!(close(v.range, 36.0));
        assert!(close(v.warmup, 0.9));
        assert!(close(v.mass, 2.75));
        assert!(close(v.cooldown, 0.5));
        assert!(close(v.bulk, 6.0) && close(v.spread, 0.2) && close(v.sway, 0.9));
        assert_eq!(v.recoil, Some(1.2));
        assert_eq!(v.sights, None);
        assert!(close(v.burst, 4.0) && close(v.magazine, 24.0) && close(v.reload, 3.0));
        assert_eq!(v.ammo_set, "RS_SetA");
    }

    #[test]
    fn patch_mass_curve_at_the_midpoint() {
        // Mass 2.5 is the midpoint of (1.5, 3.5): 2.0 + 1.0 * 0.5 = 2.5.
        let g = gun("gadget", &[], 1.2, 28.0, 8.0, 55.0);
        assert!(close(gun_values(&g, &preset_a()).unwrap().mass, 2.5));
    }

    #[rstest]
    // Damage 18 and speed 80 are in the first caliber range.
    #[case(18.0, 80.0, "RS_SetB1")]
    // Damage 25 and speed 110 only match the second.
    #[case(25.0, 110.0, "RS_SetB2")]
    // Nothing matches: the preset default.
    #[case(60.0, 5.0, "RS_SetB0")]
    fn caliber_choice(#[case] damage: f64, #[case] speed: f64, #[case] expected: &str) {
        let mut g = gun("gadget", &[], 3.0, 50.0, damage, speed);
        g.mass = Some(9.0);
        let v = gun_values(&g, &preset_b()).unwrap();
        assert_eq!(v.ammo_set, expected);
        assert!(close(v.range, 66.0) && close(v.warmup, 2.2) && close(v.cooldown, 1.6));
        assert!(close(v.mass, 5.5) && close(v.bulk, 11.0));
        assert_eq!(v.sights, Some(1.8));
        assert!(close(v.burst, 1.0));
    }

    #[test]
    fn missing_gun_mass_uses_the_flat_mass() {
        let mut g = gun("gadget", &[], 1.0, 20.0, 5.0, 45.0);
        g.mass = None;
        assert!(close(gun_values(&g, &preset_a()).unwrap().mass, 2.5));
    }

    #[test]
    fn flat_values_apply_without_curves() {
        let mut p = preset_a();
        p.range_curve = None;
        p.warmup_curve = None;
        p.cooldown_curve = Some(Curve::new([]).unwrap());
        p.flat_range = 20.0;
        p.flat_warmup = 0.7;
        p.cooldown_time = 0.3;
        let v = gun_values(&gun("gadget", &[], 1.0, 20.0, 5.0, 45.0), &p).unwrap();
        assert!(close(v.range, 20.0) && close(v.warmup, 0.7) && close(v.cooldown, 0.3));
    }

    #[test]
    fn special_gun_overrides_and_merges_stats() {
        let mut p = preset_a();
        p.special_guns.push(SpecialGun {
            names: vec!["sidearm".into()],
            caliber: Some("RS_SetS".into()),
            mag_cap: Some(7.0),
            reload_time: Some(2.0),
            mass: Some(1.1),
            bulk: Some(2.0),
            stats: [("RS_Extra".to_owned(), 0.5)].into_iter().collect(),
        });
        let v = gun_values(&gun("RS Sidearm", &[], 1.0, 20.0, 5.0, 45.0), &p).unwrap();
        assert_eq!(v.ammo_set, "RS_SetS");
        assert!(close(v.magazine, 7.0) && close(v.reload, 2.0));
        assert!(close(v.mass, 1.1) && close(v.bulk, 2.0));
        assert_eq!(v.extra.get("RS_Extra"), Some(&0.5));
    }

    #[test]
    fn gun_preset_json_round_trips() {
        let json = serde_json::to_string(&preset_b()).unwrap();
        let back: GunPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(back, preset_b());
        // Missing fields default.
        let sparse: GunPreset = serde_json::from_str(r#"{"defName":"RS_Sparse"}"#).unwrap();
        assert_eq!(sparse.burst_shot_count, 1.0);
    }

    // ---- apparel presets: 5 cases -------------------------------------------------------------------

    fn coat() -> ApparelPreset {
        ApparelPreset {
            def_name: "RS_Coat".into(),
            needed_layers: vec!["Shell".into()],
            needed_groups: vec!["Torso".into(), "Neck".into()],
            vanilla_armor_rating_range: FloatRange::new(0.1, 0.5),
            bulk: 4.0,
            bulk_worn: 2.0,
            mass: None,
            sharp_curve: Some(Curve::new([(0.1, 1.0), (0.5, 5.0)]).unwrap()),
            blunt_curve: Some(Curve::new([(0.05, 2.0), (0.15, 4.0)]).unwrap()),
            ..ApparelPreset::default()
        }
    }

    fn cap() -> ApparelPreset {
        ApparelPreset {
            def_name: "RS_Cap".into(),
            needed_layers: vec!["Overhead".into()],
            needed_groups: vec!["UpperHead".into()],
            vanilla_armor_rating_range: FloatRange::new(0.0, 0.3),
            bulk: 1.0,
            bulk_worn: 1.0,
            mass: Some(0.5),
            sharp_curve: Some(Curve::new([(0.0, 0.0), (0.3, 1.5)]).unwrap()),
            blunt_curve: Some(Curve::new([(0.0, 0.0), (0.3, 2.5)]).unwrap()),
            ..ApparelPreset::default()
        }
    }

    fn item(
        layers: &[&str],
        groups: &[&str],
        sharp: Option<f64>,
        blunt: Option<f64>,
        sema: Option<f64>,
    ) -> ApparelInput {
        ApparelInput {
            layers: layers.iter().map(|s| (*s).to_owned()).collect(),
            groups: groups.iter().map(|s| (*s).to_owned()).collect(),
            sharp,
            blunt,
            stuff_multiplier_armor: sema,
        }
    }

    #[test]
    fn stuffed_item_feeds_the_multiplier_to_both_curves() {
        // Multiplier 0.2: sharp 1 + 4 * (0.1 / 0.4) = 2.0; blunt clamps to 4.0; the preset writes no mass.
        let i = item(&["Shell"], &["Torso", "Neck"], None, None, Some(0.2));
        let presets = [coat(), cap()];
        assert_eq!(apparel_preset_for(&i, &presets).unwrap(), Some(0));
        let v = apparel_values(&i, &presets[0]).unwrap();
        assert!(close(v.sharp, 2.0) && close(v.blunt, 4.0));
        assert!(close(v.bulk, 4.0) && close(v.worn_bulk, 2.0) && close(v.mass, 0.0));
    }

    #[test]
    fn extra_layer_blocks_the_claim() {
        let i = item(&["Shell", "Middle"], &["Torso"], Some(0.3), Some(0.1), None);
        assert_eq!(apparel_preset_for(&i, &[coat(), cap()]).unwrap(), None);
    }

    #[test]
    fn extra_group_blocks_the_claim() {
        let i = item(&["Shell"], &["Legs"], Some(0.3), Some(0.1), None);
        assert_eq!(apparel_preset_for(&i, &[coat(), cap()]).unwrap(), None);
    }

    #[test]
    fn missing_stats_read_zero_and_can_match_a_range_with_zero() {
        let i = item(&["Overhead"], &["UpperHead"], None, None, None);
        let presets = [coat(), cap()];
        assert_eq!(apparel_preset_for(&i, &presets).unwrap(), Some(1));
        let v = apparel_values(&i, &presets[1]).unwrap();
        assert!(close(v.sharp, 0.0) && close(v.blunt, 0.0) && close(v.mass, 0.5));
    }

    #[test]
    fn explicit_stats_use_their_own_curves() {
        // Sharp 0.3: 1 + 4 * 0.5 = 3.0; blunt 0.1: 2 + 2 * 0.5 = 3.0.
        let i = item(&["Shell"], &["Torso"], Some(0.3), Some(0.1), None);
        let v = apparel_values(&i, &coat()).unwrap();
        assert!(close(v.sharp, 3.0) && close(v.blunt, 3.0));
    }

    #[test]
    fn static_ratings_apply_without_curves() {
        let mut p = coat();
        p.sharp_curve = None;
        p.blunt_curve = None;
        p.static_sharp = 7.0;
        p.static_blunt = 9.0;
        p.has_partial_armor = true;
        let v = apparel_values(&item(&["Shell"], &["Torso"], Some(0.3), None, None), &p).unwrap();
        assert!(close(v.sharp, 7.0) && close(v.blunt, 9.0));
        assert!(v.partial_armor);
        assert!(apparel_values(&item(&[], &[], Some(f64::NAN), None, None), &p).is_err());
    }

    proptest! {
        #[test]
        fn evaluation_stays_between_the_curve_extremes(
            points in proptest::collection::vec((-100.0f64..100.0, -100.0f64..100.0), 1..6),
            x in -200.0f64..200.0,
        ) {
            let lo = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
            let hi = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
            let y = interpolate(&points, x).unwrap();
            prop_assert!(y >= lo - 1e-9 && y <= hi + 1e-9);
        }

        #[test]
        fn evaluation_at_a_distinct_point_returns_its_y(
            ys in proptest::collection::vec(-50.0f64..50.0, 1..6),
        ) {
            // Distinct x values 0, 10, 20, ...
            let points: Vec<(f64, f64)> = ys.iter().enumerate().map(|(i, y)| (i as f64 * 10.0, *y)).collect();
            for (x, y) in &points {
                prop_assert!((interpolate(&points, *x).unwrap() - y).abs() < 1e-9);
            }
        }

        #[test]
        fn a_monotone_curve_is_monotone(
            deltas in proptest::collection::vec(0.0f64..10.0, 1..6),
            a in -50.0f64..100.0,
            b in -50.0f64..100.0,
        ) {
            let mut y = 0.0;
            let points: Vec<(f64, f64)> = deltas.iter().enumerate().map(|(i, d)| { y += d; (i as f64 * 7.0, y) }).collect();
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(interpolate(&points, lo).unwrap() <= interpolate(&points, hi).unwrap() + 1e-9);
        }

        #[test]
        fn non_finite_x_is_an_error(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)]) {
            prop_assert!(interpolate(&[(0.0, 1.0), (1.0, 2.0)], bad).is_err());
            prop_assert!(interpolate(&[(bad, 1.0)], 0.0).is_err());
        }
    }
}
