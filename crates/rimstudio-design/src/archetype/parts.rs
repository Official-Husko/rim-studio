//! The rest of a weapon beyond its combat numbers: the bash tools of a gun, weapon tags and classes, the
//! materials and the market value. All of it is read from the reference weapons of the install.

use std::collections::{BTreeMap, BTreeSet};

use crate::classes::numeric::median;
use crate::model::{ItemKind, ScalarField};
use crate::price::{IngredientLine, PriceInput, StuffCost, market_value};
use crate::reader::WeaponDetail;

use super::basis::{Basis, item_kind, round_to};
use super::costs::{CostBasis, staples};
use super::data::{ArchetypeRef, Exponents};
use super::proposal::{FactorTerm, ProposedCost, ProposedStuff, ProposedTool, ProposedValue};

/// The share of the cohort that must carry a weapon tag for the proposal to take it.
const TAG_SHARE: f64 = 0.4;

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The bash tools of a gun: the stock, the barrel. Their numbers are the medians of the matching tools of
/// the install's guns, so the bash of the new gun is as weak as the bash of the others.
#[must_use]
pub fn bash_tools(arch: &ArchetypeRef<'_>, basis: &Basis<'_>) -> Vec<ProposedTool> {
    let Some(shape) = arch.archetype.ranged.as_ref() else {
        return Vec::new();
    };
    let guns: Vec<&WeaponDetail> = basis
        .weapons
        .iter()
        .filter(|w| item_kind(w.kind) == ItemKind::Ranged)
        .collect();
    let all_tools: Vec<&crate::reader::ToolDetail> =
        guns.iter().flat_map(|w| w.tools.iter()).collect();
    let mut out = Vec::new();
    for (i, wanted) in shape.bash.iter().enumerate() {
        let key = sorted(wanted.capacities.clone());
        let same: Vec<&&crate::reader::ToolDetail> = all_tools
            .iter()
            .filter(|t| sorted(t.capacities.clone()) == key)
            .collect();
        let pick = |f: fn(&crate::reader::ToolDetail) -> Option<f64>,
                    from: &[&&crate::reader::ToolDetail]| {
            let v: Vec<f64> = from.iter().filter_map(|t| f(t)).collect();
            (median(&v), v.len())
        };
        let everyone: Vec<&&crate::reader::ToolDetail> = all_tools.iter().collect();
        let (mut power, mut n) = pick(|t| t.power, &same);
        let (mut cooldown, mut nc) = pick(|t| t.cooldown, &same);
        let mut scope = "guns with the same attack";
        if power.is_none() || cooldown.is_none() {
            (power, n) = pick(|t| t.power, &everyone);
            (cooldown, nc) = pick(|t| t.cooldown, &everyone);
            scope = "all guns";
        }
        let (Some(power), Some(cooldown)) = (power, cooldown) else {
            continue;
        };
        let note = |what: &str, v: f64, count: usize| {
            format!(
                "{} {what} {}: the median of the {count} tools of {scope} in your install.",
                wanted.label,
                crate::model::format_number(v)
            )
        };
        out.push(ProposedTool {
            label: wanted.label.clone(),
            capacities: wanted.capacities.clone(),
            power: ProposedValue {
                field: ScalarField::ToolPower(i),
                stat: "bash_power".to_owned(),
                value: power,
                write: true,
                reason: note("power", power, n),
                median: Some(power),
                median_n: Some(n),
                terms: Vec::new(),
            },
            cooldown: ProposedValue {
                field: ScalarField::ToolCooldown(i),
                stat: "bash_cooldown".to_owned(),
                value: cooldown,
                write: true,
                reason: note("cooldown", cooldown, nc),
                median: Some(cooldown),
                median_n: Some(nc),
                terms: Vec::new(),
            },
            armor_penetration: None,
        });
    }
    out
}

fn cohort<'a>(
    weapons: &'a [WeaponDetail],
    kind: ItemKind,
    tier: u8,
    role: Option<&str>,
) -> Vec<&'a WeaponDetail> {
    let of_kind: Vec<&WeaponDetail> = weapons
        .iter()
        .filter(|w| item_kind(w.kind) == kind)
        .collect();
    let tiered = |w: &&WeaponDetail| w.tech_level.map(|t| t.index()) == Some(tier);
    let roled = |w: &&WeaponDetail| role.is_some() && w.role.as_deref() == role;
    let both: Vec<&WeaponDetail> = of_kind
        .iter()
        .copied()
        .filter(|w| tiered(w) && roled(w))
        .collect();
    if both.len() >= crate::classes::MIN_POOL {
        return both;
    }
    let t: Vec<&WeaponDetail> = of_kind.iter().copied().filter(tiered).collect();
    if t.len() >= crate::classes::MIN_POOL {
        return t;
    }
    let r: Vec<&WeaponDetail> = of_kind.iter().copied().filter(roled).collect();
    if r.len() >= crate::classes::MIN_POOL {
        return r;
    }
    of_kind
}

/// The weapon tags that at least 40 percent of the comparable reference weapons carry, most common first.
#[must_use]
pub fn weapon_tags(basis: &Basis<'_>, kind: ItemKind, tier: u8, role: Option<&str>) -> Vec<String> {
    let c = cohort(basis.weapons, kind, tier, role);
    if c.is_empty() {
        return Vec::new();
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for w in &c {
        for t in &w.weapon_tags {
            *counts.entry(t.as_str()).or_insert(0) += 1;
        }
    }
    let mut v: Vec<(&str, usize)> = counts
        .into_iter()
        .filter(|(_, n)| *n as f64 / c.len() as f64 >= TAG_SHARE)
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    v.into_iter().map(|(t, _)| t.to_owned()).collect()
}

/// The weapon classes of the archetype that the install knows: the hints, kept when some reference weapon
/// carries the class.
#[must_use]
pub fn weapon_classes(basis: &Basis<'_>, hints: &[String]) -> Vec<String> {
    let known: BTreeSet<&str> = basis
        .weapons
        .iter()
        .flat_map(|w| w.weapon_classes.iter().map(String::as_str))
        .collect();
    hints
        .iter()
        .filter(|h| known.contains(h.as_str()))
        .cloned()
        .collect()
}

/// The materials of the proposal.
#[derive(Debug, Clone, Default)]
pub struct Materials {
    /// The cost list.
    pub cost_list: Vec<ProposedCost>,
    /// The stuff, for a weapon made of a material.
    pub stuff: Option<ProposedStuff>,
    /// Remarks.
    pub notes: Vec<String>,
}

/// What the materials depend on.
#[derive(Debug, Clone, Copy)]
pub struct MaterialInputs<'a> {
    /// The kind of weapon.
    pub kind: ItemKind,
    /// The tier index.
    pub tier: u8,
    /// The strength the proposal aims at.
    pub target: f64,
    /// The mass of the proposal against the median mass (the size of the weapon).
    pub size: f64,
    /// The stuff count ratio of the archetype.
    pub stuff_factor: f64,
    /// The stuff categories the archetype allows.
    pub stuff_categories: &'a [String],
}

/// Proposes the cost list (and the stuff of a melee weapon) from the materials of the tier.
#[must_use]
pub fn materials(costs: &CostBasis, input: &MaterialInputs<'_>, exps: &Exponents) -> Materials {
    let (rows, tiered) = costs.cohort(input.kind, input.tier);
    let mut out = Materials::default();
    if rows.is_empty() {
        out.notes
            .push("no reference weapon to read materials from".to_owned());
        return out;
    }
    let strengths: Vec<f64> = rows.iter().filter_map(|r| r.strength).collect();
    let ratio = median(&strengths)
        .filter(|m| *m > 0.0)
        .map_or(1.0, |m| (input.target / m).clamp(0.2, 5.0));
    let scale = input.size.max(0.05).sqrt() * ratio.powf(exps.cost_coupling);
    let scope = if tiered {
        format!("the {} reference weapons of the same tier", rows.len())
    } else {
        format!(
            "the {} reference weapons of the same kind (too few of the tier)",
            rows.len()
        )
    };
    for s in staples(&rows) {
        let count = (s.median_count * scale).round().max(1.0);
        out.cost_list.push(ProposedCost {
            def_name: s.def_name.clone(),
            count,
            reason: format!(
                "{} of {}% of {scope} need {}; median {} scaled by x{:.2} for size and strength.",
                s.def_name,
                (s.share * 100.0).round(),
                "it",
                crate::model::format_number(s.median_count),
                scale
            ),
        });
    }
    if input.kind == ItemKind::Melee {
        let counts: Vec<f64> = rows.iter().filter_map(|r| r.stuff_count).collect();
        if let Some(med) = median(&counts) {
            let present: BTreeSet<&str> = rows
                .iter()
                .flat_map(|r| r.stuff_categories.iter().map(String::as_str))
                .collect();
            let mut categories: Vec<String> = input
                .stuff_categories
                .iter()
                .filter(|c| present.contains(c.as_str()))
                .cloned()
                .collect();
            if categories.is_empty() {
                categories = input.stuff_categories.to_vec();
            }
            let count = (med * input.stuff_factor * ratio.powf(exps.cost_coupling))
                .round()
                .max(1.0);
            out.stuff = Some(ProposedStuff {
                categories,
                count,
                reason: format!(
                    "the median stuff count of {scope} is {}, times {:.2} for the archetype and x{:.2} for strength.",
                    crate::model::format_number(med),
                    input.stuff_factor,
                    ratio.powf(exps.cost_coupling)
                ),
            });
        }
    }
    out
}

/// The market value the price math gives for a proposed cost list, stuff and work. `None` when an
/// ingredient has no known unit price.
#[must_use]
pub fn price(costs: &CostBasis, m: &Materials, work: f64) -> Option<f64> {
    let mut ingredients = Vec::new();
    for c in &m.cost_list {
        ingredients.push(IngredientLine {
            count: c.count,
            unit_value: *costs.unit_values.get(&c.def_name)?,
        });
    }
    let stuff = m
        .stuff
        .as_ref()
        .map(|s| StuffCost::Unknown { count: s.count });
    market_value(&PriceInput {
        ingredients,
        stuff,
        work_to_make: work,
        work_factor: 1.0,
    })
    .ok()
    .map(|b| round_to(b.displayed, 1.0))
}

/// A reason line for the terms of a value, shared by the tests.
#[must_use]
pub fn describe(terms: &[FactorTerm]) -> String {
    terms
        .iter()
        .map(|t| format!("{} x{:.2}", t.label, t.factor))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archetype::costs::CostRow;

    #[test]
    fn describe_lists_terms() {
        let t = [FactorTerm::new("a", 1.5), FactorTerm::new("b", 0.5)];
        assert_eq!(describe(&t), "a x1.50, b x0.50");
    }

    #[test]
    fn the_price_follows_the_price_math() {
        let costs = CostBasis {
            rows: vec![CostRow {
                def_name: "RS_A".into(),
                kind: ItemKind::Ranged,
                tier: Some(2),
                role: None,
                strength: None,
                cost: BTreeMap::new(),
                stuff_categories: Vec::new(),
                stuff_count: None,
            }],
            unit_values: [("RS_Steel".to_owned(), 2.0)].into_iter().collect(),
        };
        let m = Materials {
            cost_list: vec![ProposedCost {
                def_name: "RS_Steel".into(),
                count: 30.0,
                reason: String::new(),
            }],
            ..Materials::default()
        };
        // 30 x 2 = 60 plus work 20000 x 0.0036 = 72 gives 132
        let v = price(&costs, &m, 20000.0).unwrap_or(0.0);
        assert!((v - 132.0).abs() < 1.0, "{v}");
        let unknown = Materials {
            cost_list: vec![ProposedCost {
                def_name: "RS_Other".into(),
                count: 1.0,
                reason: String::new(),
            }],
            ..Materials::default()
        };
        assert_eq!(price(&costs, &unknown, 100.0), None);
    }
}
