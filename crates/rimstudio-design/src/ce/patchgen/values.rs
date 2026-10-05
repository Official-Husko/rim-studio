//! The numbers a patch writes: where each one comes from.
//!
//! A value is taken, in this order, from the Combat Extended block of the spec (typed or answered by the user,
//! or suggested earlier), from the vanilla design (identity: the number the vanilla twin has), and last from
//! the predictors of [`crate::ce::classes`] (the class medians, ratios and elastic fits derived from the
//! user's own conversions). Nothing is a constant of this module. Every number that did not come from the
//! block is listed in [`DerivedValue`] with its origin, so the designer can show it and the user can overrule
//! it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ce::classes::{
    ConversionPrediction, ConversionPredictor, EstimateOptions, ExampleSet, Profile, Reliability,
    estimate_conversion,
};
use crate::ce::reader::CeModel;
use crate::classes::ItemKind as PoolKind;
use crate::model::{
    CePatchSpec, DesignSpec, ExtraMeleeDamage, ItemKind, SurpriseAttackSpec, ToolSpec,
};
use rimstudio_core::tree::Node;

/// Where a derived number came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "origin")]
pub enum ValueOrigin {
    /// The vanilla design's own number (identity).
    Vanilla,
    /// A predictor over the user's conversions.
    Predicted {
        /// The predictor that was chosen for the stat.
        predictor: ConversionPredictor,
        /// Converted weapons behind the estimate.
        n: usize,
    },
}

/// A number the patch writes that the Combat Extended block did not hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedValue {
    /// The field, for example `mass`, `range`, `tool 1 armorPenetrationBlunt`.
    pub field: String,
    /// The value written.
    pub value: f64,
    /// Where it came from.
    pub origin: ValueOrigin,
    /// How far the estimate behind the number can be trusted, when one was involved. A number with origin
    /// [`ValueOrigin::Vanilla`] and a rating of unreliable or unmeasured was carried over because the
    /// class estimate was too poor to write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<Reliability>,
    /// The typical relative error of the estimate (leave one out), when it was measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<f64>,
}

impl DerivedValue {
    /// A derived number without a rating (a value carried over from the vanilla design).
    #[must_use]
    pub fn plain(field: impl Into<String>, value: f64, origin: ValueOrigin) -> Self {
        Self {
            field: field.into(),
            value,
            origin,
            rating: None,
            error: None,
        }
    }
}

/// Rounds to four significant digits, so that estimates read like hand written numbers.
#[must_use]
pub fn tidy(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return if value.is_finite() { 0.0 } else { value };
    }
    let magnitude = value.abs().log10().floor();
    let scale = 10f64.powf(3.0 - magnitude);
    if !scale.is_finite() || scale == 0.0 {
        return value;
    }
    (value * scale).round() / scale
}

/// The mean of the numbers, `None` when there are none.
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / n as f64)
}

/// The predictions for a design: the conversion estimates of its nearest conversions in the user's
/// Combat Extended (see [`crate::ce::classes::estimate`]). `None` when the model has no converted weapon
/// of the kind to learn from.
#[must_use]
pub fn predict_for(spec: &DesignSpec, model: &CeModel) -> Option<ConversionPrediction> {
    let kind = match spec.kind {
        ItemKind::Ranged => PoolKind::Ranged,
        ItemKind::Melee => PoolKind::Melee,
    };
    let set = ExampleSet::from_model(model, kind);
    if set.is_empty() {
        return None;
    }
    let mut vanilla: BTreeMap<String, f64> = BTreeMap::new();
    let mut put = |name: &str, v: Option<f64>| {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            vanilla.insert(name.to_owned(), v);
        }
    };
    put("mass", spec.mass.map(|m| m.value));
    match spec.kind {
        ItemKind::Ranged => {
            let r = spec.ranged.as_ref();
            put("range", r.and_then(|r| r.range).map(|v| v.value));
            put("warmup", r.and_then(|r| r.warmup).map(|v| v.value));
            put("cooldown", r.and_then(|r| r.cooldown).map(|v| v.value));
            put(
                "burst",
                Some(
                    r.and_then(|r| r.burst_count)
                        .map_or(1.0, |b| f64::from(b.value)),
                ),
            );
        }
        ItemKind::Melee => {
            put(
                "tool_power",
                mean(spec.tools.iter().filter_map(|t| t.power).map(|p| p.value)),
            );
            put(
                "tool_cooldown",
                mean(
                    spec.tools
                        .iter()
                        .filter_map(|t| t.cooldown_time)
                        .map(|p| p.value),
                ),
            );
        }
    }
    let profile = Profile {
        tier: spec.tech_level.map(crate::model::TechLevel::index),
        tags: spec.weapon_tags.clone(),
        role: match spec.kind {
            ItemKind::Ranged => spec.ce.as_ref().and_then(|c| c.weapon_tag_class.clone()),
            ItemKind::Melee => None,
        },
        vanilla,
    };
    Some(estimate_conversion(
        &set,
        &profile,
        &EstimateOptions::default(),
    ))
}

/// The prediction of a melee class for the penetration ratios of gun tools: the melee conversions nearest
/// to a weapon of the spec's tier with the spec's tool numbers (a gun's tags describe no melee weapon, so
/// none are used).
#[must_use]
pub fn predict_tool_ratios(spec: &DesignSpec, model: &CeModel) -> Option<ConversionPrediction> {
    let set = ExampleSet::from_model(model, PoolKind::Melee);
    if set.is_empty() {
        return None;
    }
    let mut vanilla = BTreeMap::new();
    if let Some(p) = mean(spec.tools.iter().filter_map(|t| t.power).map(|p| p.value)) {
        vanilla.insert("tool_power".to_owned(), p);
    }
    if let Some(c) = mean(
        spec.tools
            .iter()
            .filter_map(|t| t.cooldown_time)
            .map(|p| p.value),
    ) {
        vanilla.insert("tool_cooldown".to_owned(), c);
    }
    let profile = Profile {
        tier: spec.tech_level.map(crate::model::TechLevel::index),
        tags: Vec::new(),
        role: None,
        vanilla,
    };
    Some(estimate_conversion(
        &set,
        &profile,
        &EstimateOptions::default(),
    ))
}

/// A usable estimate of a stat: finite, positive and rated reliable or rough. An estimate that is rated
/// unreliable or unmeasured is never written (the caller asks or carries the vanilla number over).
fn predicted(
    prediction: Option<&ConversionPrediction>,
    stat: &str,
) -> Option<(f64, ConversionPredictor, usize, Reliability, Option<f64>)> {
    let p = prediction?.stats.get(stat)?;
    (p.is_usable() && p.value > 0.0).then_some((
        tidy(p.value),
        p.predictor,
        p.n,
        p.reliability,
        p.loo_error,
    ))
}

/// One resolved number: the block's value, else the vanilla one, else the prediction.
pub(super) struct Pick {
    pub(super) value: Option<f64>,
}

pub(super) fn resolve(
    derived: &mut Vec<DerivedValue>,
    field: &str,
    block: Option<f64>,
    vanilla: Option<f64>,
    prediction: Option<&ConversionPrediction>,
    stat: &str,
) -> Pick {
    if let Some(v) = block {
        return Pick { value: Some(v) };
    }
    if let Some((value, predictor, n, rating, error)) = predicted(prediction, stat) {
        derived.push(DerivedValue {
            field: field.to_owned(),
            value,
            origin: ValueOrigin::Predicted { predictor, n },
            rating: Some(rating),
            error,
        });
        return Pick { value: Some(value) };
    }
    if let Some(v) = vanilla.filter(|v| v.is_finite()) {
        // A rejected estimate is recorded so that the plan can say why the vanilla number stays.
        let rejected = prediction.and_then(|p| p.stats.get(stat));
        derived.push(DerivedValue {
            field: field.to_owned(),
            value: v,
            origin: ValueOrigin::Vanilla,
            rating: rejected.map(|p| p.reliability),
            error: rejected.and_then(|p| p.loo_error),
        });
        return Pick { value: Some(v) };
    }
    Pick { value: None }
}

/// The numbers of a converted gun, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct GunValues {
    /// The `Mass` stat.
    pub mass: Option<f64>,
    /// The `Bulk` stat.
    pub bulk: f64,
    /// The `SwayFactor` stat.
    pub sway: f64,
    /// The `ShotSpread` stat.
    pub spread: f64,
    /// The `SightsEfficiency` stat.
    pub sights: Option<f64>,
    /// The `RangedWeapon_Cooldown` stat.
    pub cooldown: Option<f64>,
    /// The verb's warmup in seconds.
    pub warmup: Option<f64>,
    /// The verb's range in cells.
    pub range: Option<f64>,
    /// The verb's recoil.
    pub recoil: Option<f64>,
    /// Shots per burst, written only when above one.
    pub burst: Option<u32>,
    /// Shots of an aimed burst (the vanilla burst of a burst weapon), written only when above one.
    pub aimed_burst: Option<u32>,
    /// Ticks between burst shots.
    pub ticks_between: Option<f64>,
    /// Magazine size.
    pub magazine: u32,
    /// Reload time in seconds.
    pub reload: f64,
    /// The ammo set.
    pub ammo_set: String,
    /// The default projectile.
    pub projectile: String,
    /// The AI class tag.
    pub tag: Option<String>,
    /// The numbers that were not in the block.
    pub derived: Vec<DerivedValue>,
}

/// Resolves the numbers of a gun patch. `None` when a field the block must hold is missing; validation
/// reports which (see `validate_ce_patch`).
#[must_use]
pub fn resolve_gun(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    prediction: Option<&ConversionPrediction>,
) -> Option<GunValues> {
    let ranged = spec.ranged.as_ref();
    let mut derived = Vec::new();
    let mass = resolve(
        &mut derived,
        "mass",
        None,
        spec.mass.map(|m| m.value),
        prediction,
        "mass",
    )
    .value;
    let range = resolve(
        &mut derived,
        "range",
        None,
        ranged.and_then(|r| r.range).map(|v| v.value),
        prediction,
        "range",
    )
    .value;
    let warmup = resolve(
        &mut derived,
        "warmup",
        None,
        ranged.and_then(|r| r.warmup).map(|v| v.value),
        prediction,
        "warmup",
    )
    .value;
    let cooldown = resolve(
        &mut derived,
        "cooldown",
        ce.cooldown.map(|c| c.value),
        ranged.and_then(|r| r.cooldown).map(|v| v.value),
        prediction,
        "cooldown",
    )
    .value;
    let sights = resolve(
        &mut derived,
        "sights",
        ce.sights_efficiency.map(|v| v.value),
        None,
        prediction,
        "sights",
    )
    .value;
    let recoil = resolve(
        &mut derived,
        "recoil",
        ce.recoil_amount.map(|v| v.value),
        None,
        prediction,
        "recoil",
    )
    .value;
    let vanilla_burst = ranged
        .and_then(|r| r.burst_count)
        .map(|b| b.value)
        .filter(|b| *b > 1);
    let vanilla_ticks = ranged
        .and_then(|r| r.ticks_between_burst_shots)
        .map(|t| t.value);
    // A burst weapon keeps its burst unless the converted weapons of its class show another one.
    let burst = vanilla_burst.and_then(|vb| {
        let picked = resolve(
            &mut derived,
            "burst",
            None,
            Some(f64::from(vb)),
            prediction,
            "burst",
        )
        .value?;
        let rounded = picked.round();
        (rounded >= 2.0 && rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
    });
    let ticks_between = if burst.is_some() {
        resolve(
            &mut derived,
            "ticks between shots",
            None,
            vanilla_ticks,
            prediction,
            "ticks_between",
        )
        .value
    } else {
        None
    };
    Some(GunValues {
        mass,
        bulk: ce.bulk?.value,
        sway: ce.sway_factor?.value,
        spread: ce.shot_spread?.value,
        sights,
        cooldown,
        warmup,
        range,
        recoil,
        burst,
        aimed_burst: vanilla_burst,
        ticks_between,
        magazine: ce.magazine_size?.value,
        reload: ce.reload_time?.value,
        ammo_set: ce.ammo_set.clone().filter(|s| !s.is_empty())?,
        projectile: ce.default_projectile.clone().filter(|s| !s.is_empty())?,
        tag: ce.weapon_tag_class.clone().filter(|s| !s.is_empty()),
        derived,
    })
}

/// A converted tool, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolValues {
    /// The tool label.
    pub label: String,
    /// Capacity def names.
    pub capacities: Vec<String>,
    /// Power.
    pub power: Option<f64>,
    /// Cooldown in seconds.
    pub cooldown: Option<f64>,
    /// Sharp penetration, when the tool needs one.
    pub ap_sharp: Option<f64>,
    /// Blunt penetration.
    pub ap_blunt: Option<f64>,
    /// Pick weight.
    pub chance_factor: Option<f64>,
    /// The body part group that carries the tool.
    pub linked_body_parts_group: Option<String>,
    /// Extra damages of the attack, carried over from the vanilla tool.
    pub extra_melee_damages: Vec<ExtraMeleeDamage>,
    /// The extra damage of a surprise attack, carried over from the vanilla tool.
    pub surprise_attack: Option<SurpriseAttackSpec>,
    /// Other children of the vanilla tool, carried over as written.
    pub extra: Vec<Node>,
}

/// True for capacities whose attacks cut or pierce, which carry a sharp penetration.
#[must_use]
pub fn is_sharp_capacity(capacity: &str) -> bool {
    matches!(capacity, "Cut" | "Stab" | "Scratch" | "Bite")
}

fn ratio_of(
    prediction: Option<&ConversionPrediction>,
    stat: &str,
) -> Option<(f64, ConversionPredictor, usize, Reliability, Option<f64>)> {
    predicted(prediction, stat)
}

/// The factors that turn the vanilla power and cooldown of the tools into the converted ones: the predicted
/// mean over the vanilla mean, 1 when `scale` is off or the class gives no prediction.
#[must_use]
pub fn tool_scales(
    spec: &DesignSpec,
    prediction: Option<&ConversionPrediction>,
    scale: bool,
) -> (f64, f64) {
    if !scale {
        return (1.0, 1.0);
    }
    let factor = |stat: &str, vanilla: Option<f64>| -> f64 {
        predicted(prediction, stat)
            .and_then(|(p, ..)| Some(p / vanilla.filter(|v| *v > 0.0)?))
            .filter(|f| f.is_finite() && *f > 0.0)
            // A class that predicts the same power within a percent is not a reason to change the number.
            .map(|f| if (f - 1.0).abs() < 0.01 { 1.0 } else { f })
            .unwrap_or(1.0)
    };
    (
        factor(
            "tool_power",
            mean(spec.tools.iter().filter_map(|t| t.power).map(|p| p.value)),
        ),
        factor(
            "tool_cooldown",
            mean(
                spec.tools
                    .iter()
                    .filter_map(|t| t.cooldown_time)
                    .map(|p| p.value),
            ),
        ),
    )
}

/// Resolves the tools of a weapon. Power and cooldown follow the vanilla tool, scaled by the predicted
/// change of the class when `scale` is set (melee weapons); penetration comes from the block by tool label,
/// else from the class ratio over power. The second result lists the tools that could not get a blunt
/// penetration at all (labels), and the third the derived numbers.
#[must_use]
pub fn resolve_tools(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    prediction: Option<&ConversionPrediction>,
    scale: bool,
) -> (Vec<ToolValues>, Vec<String>, Vec<DerivedValue>) {
    let mut derived = Vec::new();
    let mut missing = Vec::new();
    let (power_scale, cooldown_scale) = tool_scales(spec, prediction, scale);
    let mut out = Vec::new();
    for (i, tool) in spec.tools.iter().enumerate() {
        let number = i.saturating_add(1);
        let entry = ce.tool_penetration.iter().find(|p| p.tool == tool.label);
        let power = tool.power.map(|p| tidy(p.value * power_scale));
        let cooldown = tool.cooldown_time.map(|c| tidy(c.value * cooldown_scale));
        let sharp_needed = tool.capacities.iter().any(|c| is_sharp_capacity(c));
        let ap_blunt = entry.and_then(|e| e.blunt).map(|v| v.value).or_else(|| {
            let (ratio, predictor, n, rating, error) = ratio_of(prediction, "ap_blunt_ratio")?;
            let value = tidy(ratio * power?);
            derived.push(DerivedValue {
                field: format!("tool {number} armorPenetrationBlunt"),
                value,
                origin: ValueOrigin::Predicted { predictor, n },
                rating: Some(rating),
                error,
            });
            Some(value)
        });
        let ap_sharp = if sharp_needed {
            entry.and_then(|e| e.sharp).map(|v| v.value).or_else(|| {
                let (ratio, predictor, n, rating, error) = ratio_of(prediction, "ap_sharp_ratio")?;
                let value = tidy(ratio * power?);
                derived.push(DerivedValue {
                    field: format!("tool {number} armorPenetrationSharp"),
                    value,
                    origin: ValueOrigin::Predicted { predictor, n },
                    rating: Some(rating),
                    error,
                });
                Some(value)
            })
        } else {
            None
        };
        if ap_blunt.is_none() {
            missing.push(tool.label.clone());
        }
        out.push(tool_values(tool, power, cooldown, ap_sharp, ap_blunt));
    }
    (out, missing, derived)
}

fn tool_values(
    tool: &ToolSpec,
    power: Option<f64>,
    cooldown: Option<f64>,
    ap_sharp: Option<f64>,
    ap_blunt: Option<f64>,
) -> ToolValues {
    ToolValues {
        label: tool.label.clone(),
        capacities: tool.capacities.clone(),
        power,
        cooldown,
        ap_sharp,
        ap_blunt,
        chance_factor: tool.chance_factor.map(|c| c.value),
        linked_body_parts_group: tool.linked_body_parts_group.clone(),
        extra_melee_damages: tool.extra_melee_damages.clone(),
        surprise_attack: tool.surprise_attack.clone(),
        extra: tool.extra.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(1.23456, 1.235)]
    #[case(0.071234, 0.07123)]
    #[case(1234.56, 1235.0)]
    #[case(0.0, 0.0)]
    #[case(-2.34567, -2.346)]
    fn tidy_keeps_four_significant_digits(#[case] v: f64, #[case] want: f64) {
        assert!((tidy(v) - want).abs() < 1e-9, "{v}");
    }

    #[test]
    fn tidy_passes_non_finite_numbers_through() {
        assert!(tidy(f64::NAN).is_nan());
        assert!(tidy(f64::INFINITY).is_infinite());
    }

    #[test]
    fn sharp_capacities_are_the_cutting_ones() {
        assert!(is_sharp_capacity("Cut"));
        assert!(is_sharp_capacity("Stab"));
        assert!(!is_sharp_capacity("Blunt"));
    }
}
