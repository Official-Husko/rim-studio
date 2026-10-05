//! The solver for melee weapons: the same two steps as the gun solver (a shape, then one strength scale).
//!
//! The shape is a list of tools whose power is a ratio of the install's median swing damage and whose
//! cooldown is a ratio of the install's median cooldown. The scale raises the power and shortens the
//! cooldown in fixed shares until the melee strength index reaches the target.

use crate::error::{DesignError, DesignResult};
use crate::melee::{
    DEFAULT_AP_PER_DAMAGE, MeleeAttack, MeleeCapacity, MeleeModifiers, MeleeTool,
    enumerate_attacks, in_fight_dps, stat_panel_dps, strength,
};
use crate::model::ScalarField;

use super::basis::{MeleeMedians, round_significant, round_to};
use super::data::{ArchetypeRef, Exponents, MeleeShape};
use super::gun::solve_scale;
use super::proposal::{FactorTerm, ProposedTool, ProposedValue};
use super::resolve::Resolved;

/// One tool before the strength scale.
#[derive(Debug, Clone)]
pub struct ToolBase {
    /// The label.
    pub label: String,
    /// The capacities.
    pub capacities: Vec<String>,
    /// Power before the scale.
    pub power: f64,
    /// Cooldown before the scale.
    pub cooldown: f64,
    /// The penetration against the implied one, when the archetype asks for more.
    pub ap_factor: Option<f64>,
}

/// The unscaled shape of a melee weapon.
#[derive(Debug, Clone)]
pub struct BladeShape {
    /// The tools.
    pub tools: Vec<ToolBase>,
    /// Mass before the scale.
    pub mass: f64,
    /// Work before the scale.
    pub work: f64,
}

/// A solved tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolNumbers {
    /// The label.
    pub label: String,
    /// The capacities.
    pub capacities: Vec<String>,
    /// The power.
    pub power: f64,
    /// The cooldown.
    pub cooldown: f64,
    /// The explicit penetration, when written.
    pub armor_penetration: Option<f64>,
}

/// The solved weapon.
#[derive(Debug, Clone)]
pub struct BladeNumbers {
    /// The tools.
    pub tools: Vec<ToolNumbers>,
    /// The mass.
    pub mass: f64,
    /// The work to make.
    pub work: f64,
    /// The scale that was applied.
    pub scale: f64,
    /// True when the target was out of reach.
    pub clamped: bool,
    /// The strength of the rounded tools, with unit modifiers.
    pub achieved_plain: f64,
}

/// Builds the unscaled shape.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for an archetype without a melee shape.
pub fn shape(
    arch: &ArchetypeRef<'_>,
    resolved: &Resolved<'_>,
    m: &MeleeMedians,
    exps: &Exponents,
) -> DesignResult<(BladeShape, MeleeShape)> {
    let s = arch
        .archetype
        .melee
        .as_ref()
        .ok_or_else(|| DesignError::invalid("archetype", "not a melee archetype"))?;
    let h = resolved.handling;
    let tempo = s.cooldown * h.cooldown / resolved.rate.powf(exps.rof_cooldown);
    let tools = s
        .tools
        .iter()
        .map(|t| ToolBase {
            label: t.label.clone(),
            capacities: t.capacities.clone(),
            power: m.swing_damage.value * t.power * h.power,
            cooldown: m.cooldown.value * t.cooldown * tempo,
            ap_factor: t.ap_factor,
        })
        .collect();
    Ok((
        BladeShape {
            tools,
            mass: m.mass.value * s.mass * h.mass,
            work: m.work.value * s.work,
        },
        s.clone(),
    ))
}

fn attacks_of(tools: &[ToolNumbers]) -> DesignResult<Vec<MeleeAttack>> {
    let inputs: Vec<MeleeTool> = tools
        .iter()
        .map(|t| MeleeTool {
            label: t.label.clone(),
            capacities: t
                .capacities
                .iter()
                .map(|c| MeleeCapacity::named(c))
                .collect(),
            power: t.power,
            cooldown: t.cooldown,
            armor_penetration: t.armor_penetration,
            chance_factor: 1.0,
        })
        .collect();
    enumerate_attacks(&inputs, &MeleeModifiers::default())
}

/// The attacks of solved tools with unit modifiers.
///
/// # Errors
///
/// [`DesignError`] from the attack enumeration.
pub fn attacks(tools: &[ToolNumbers]) -> DesignResult<Vec<MeleeAttack>> {
    attacks_of(tools)
}

fn tools_at(shape: &BladeShape, exps: &Exponents, s: f64, round: bool) -> Vec<ToolNumbers> {
    shape
        .tools
        .iter()
        .map(|t| {
            let power = t.power * s.powf(exps.power_scale);
            let cooldown = t.cooldown * s.powf(-exps.tempo_scale);
            let (power, cooldown) = if round {
                (
                    round_to(power, 1.0).max(1.0),
                    round_to(cooldown, 0.1).max(0.1),
                )
            } else {
                (power, cooldown)
            };
            ToolNumbers {
                label: t.label.clone(),
                capacities: t.capacities.clone(),
                power,
                cooldown,
                armor_penetration: t.ap_factor.map(|f| {
                    if round {
                        round_to(f * DEFAULT_AP_PER_DAMAGE * power, 0.005)
                    } else {
                        f * DEFAULT_AP_PER_DAMAGE * power
                    }
                }),
            }
        })
        .collect()
}

fn plain_strength(tools: &[ToolNumbers]) -> DesignResult<f64> {
    strength(&attacks_of(tools)?)
}

/// Solves the weapon against the target strength with unit modifiers.
///
/// # Errors
///
/// [`DesignError`] from the melee strength index.
pub fn solve(
    shape: &BladeShape,
    exps: &Exponents,
    target_plain: f64,
    strength_work_ratio: f64,
) -> DesignResult<BladeNumbers> {
    let (scale, clamped) = solve_scale(
        |s| plain_strength(&tools_at(shape, exps, s, false)),
        target_plain,
    )?;
    let tools = tools_at(shape, exps, scale, true);
    let achieved_plain = plain_strength(&tools)?;
    let mass_raw = shape.mass * scale.powf(exps.mass_coupling);
    let mass = if mass_raw < 2.0 {
        round_to(mass_raw, 0.05)
    } else {
        round_to(mass_raw, 0.1)
    };
    Ok(BladeNumbers {
        tools,
        mass: mass.max(0.05),
        work: round_significant(shape.work * strength_work_ratio.powf(exps.work_coupling), 2)
            .max(1.0),
        scale,
        clamped,
        achieved_plain,
    })
}

/// The stat panel numbers of the solved tools: swing damage, cooldown, dps, armor penetration and in fight
/// dps.
///
/// # Errors
///
/// [`DesignError`] from the attack enumeration.
pub fn panel(tools: &[ToolNumbers]) -> DesignResult<[f64; 5]> {
    let attacks = attacks_of(tools)?;
    let p = stat_panel_dps(&attacks)?;
    let f = in_fight_dps(&attacks)?;
    Ok([p.damage, p.cooldown, p.dps, p.armor_penetration, f.dps])
}

fn reason(label: &str, value: f64, base: &str, terms: &[FactorTerm]) -> String {
    let mut text = format!("{label} {}: {base}", crate::model::format_number(value));
    let parts: Vec<String> = terms
        .iter()
        .filter(|t| t.factor.ln().abs() > 0.005)
        .map(|t| format!("{} x{:.2}", t.label, t.factor))
        .collect();
    if !parts.is_empty() {
        text.push_str(". ");
        text.push_str(&parts.join(", "));
    }
    text.push('.');
    text
}

/// The proposed tools and scalar values of a solved melee weapon, with reasons.
#[must_use]
pub fn values(
    numbers: &BladeNumbers,
    shape: &MeleeShape,
    resolved: &Resolved<'_>,
    arch_label: &str,
    m: &MeleeMedians,
    exps: &Exponents,
) -> (Vec<ProposedTool>, Vec<ProposedValue>) {
    let s = numbers.scale;
    let h = resolved.handling;
    let hl = format!("{} handling", h.label.to_lowercase());
    let rate = FactorTerm::new(
        format!("{} swing speed", resolved.rof_id),
        resolved.rate.powf(-exps.rof_cooldown),
    );
    let mut tools = Vec::new();
    for (i, (t, base)) in numbers.tools.iter().zip(&shape.tools).enumerate() {
        let power_terms = vec![
            FactorTerm::new(format!("{} {}", arch_label, t.label), base.power),
            FactorTerm::new(hl.clone(), h.power),
            FactorTerm::new("strength scale", s.powf(exps.power_scale)),
        ];
        let cool_terms = vec![
            FactorTerm::new(
                format!("{} {}", arch_label, t.label),
                base.cooldown * shape.cooldown,
            ),
            FactorTerm::new(hl.clone(), h.cooldown),
            rate.clone(),
            FactorTerm::new("strength scale", s.powf(-exps.tempo_scale)),
        ];
        let power_base = format!(
            "the install's median swing damage is {} ({} reference weapons)",
            crate::model::format_number((m.swing_damage.value * 100.0).round() / 100.0),
            m.swing_damage.n
        );
        let cool_base = format!(
            "the install's median cooldown is {} ({} reference weapons)",
            crate::model::format_number((m.cooldown.value * 100.0).round() / 100.0),
            m.cooldown.n
        );
        let power = ProposedValue {
            field: ScalarField::ToolPower(i),
            stat: "swing_damage".to_owned(),
            value: t.power,
            write: true,
            reason: reason(
                &format!("{} power", t.label),
                t.power,
                &power_base,
                &power_terms,
            ),
            median: Some(m.swing_damage.value),
            median_n: Some(m.swing_damage.n),
            terms: power_terms,
        };
        let cooldown = ProposedValue {
            field: ScalarField::ToolCooldown(i),
            stat: "cooldown".to_owned(),
            value: t.cooldown,
            write: true,
            reason: reason(
                &format!("{} cooldown", t.label),
                t.cooldown,
                &cool_base,
                &cool_terms,
            ),
            median: Some(m.cooldown.value),
            median_n: Some(m.cooldown.n),
            terms: cool_terms,
        };
        let armor_penetration = t.armor_penetration.map(|ap| {
            let terms = vec![FactorTerm::new(
                format!(
                    "{} {} against the implied 0.015 per damage point",
                    arch_label, t.label
                ),
                base.ap_factor.unwrap_or(1.0),
            )];
            ProposedValue {
                field: ScalarField::ToolArmorPenetration(i),
                stat: "ap".to_owned(),
                value: ap,
                write: true,
                reason: reason(&format!("{} armor penetration", t.label), ap, "", &terms),
                median: None,
                median_n: None,
                terms,
            }
        });
        tools.push(ProposedTool {
            label: t.label.clone(),
            capacities: t.capacities.clone(),
            power,
            cooldown,
            armor_penetration,
        });
    }
    let mass_terms = vec![
        FactorTerm::new(arch_label, shape.mass),
        FactorTerm::new(hl, h.mass),
        FactorTerm::new("strength scale", s.powf(exps.mass_coupling)),
    ];
    let work_terms = vec![FactorTerm::new(arch_label, shape.work)];
    let mass_base = format!(
        "the install's median mass is {} ({} reference weapons)",
        crate::model::format_number((m.mass.value * 100.0).round() / 100.0),
        m.mass.n
    );
    let work_base = format!(
        "the install's median work is {} ({} reference weapons)",
        crate::model::format_number(m.work.value.round()),
        m.work.n
    );
    let scalars = vec![
        ProposedValue {
            field: ScalarField::Mass,
            stat: "mass".to_owned(),
            value: numbers.mass,
            write: true,
            reason: reason("mass", numbers.mass, &mass_base, &mass_terms),
            median: Some(m.mass.value),
            median_n: Some(m.mass.n),
            terms: mass_terms,
        },
        ProposedValue {
            field: ScalarField::WorkToMake,
            stat: "work".to_owned(),
            value: numbers.work,
            write: true,
            reason: reason("work to make", numbers.work, &work_base, &work_terms),
            median: Some(m.work.value),
            median_n: Some(m.work.n),
            terms: work_terms,
        },
    ];
    (tools, scalars)
}
