//! The materials of a tier: cost lists and stuff counts read from the reference weapons of the install.
//!
//! The solver proposes a cost list from what the install's own weapons of the same tier are made of: the
//! ingredients that most of them need, in counts that follow the median of those weapons scaled by how big
//! and how strong the new weapon is. Nothing about materials is stored in the repository.

use std::collections::BTreeMap;

use rimstudio_defs::DefDatabases;

use crate::classes::numeric::median;
use crate::model::ItemKind;
use crate::reader::access::{child_number, named_numbers};
use crate::reader::{ReaderOptions, ReferenceSet, market_value_of};

/// What one reference weapon is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct CostRow {
    /// The def name of the weapon.
    pub def_name: String,
    /// The kind of weapon.
    pub kind: ItemKind,
    /// The tier index.
    pub tier: Option<u8>,
    /// The pool role.
    pub role: Option<String>,
    /// The strength index.
    pub strength: Option<f64>,
    /// The cost list: ingredient def name to count.
    pub cost: BTreeMap<String, f64>,
    /// The stuff categories the weapon accepts.
    pub stuff_categories: Vec<String>,
    /// The stuff units per item.
    pub stuff_count: Option<f64>,
}

/// The cost lists of the reference weapons and the unit prices of their ingredients.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CostBasis {
    /// One row per reference weapon.
    pub rows: Vec<CostRow>,
    /// The base market value of one unit of each ingredient.
    pub unit_values: BTreeMap<String, f64>,
}

impl CostBasis {
    /// Reads the cost lists of the reference weapons from the resolved defs.
    #[must_use]
    pub fn read(dbs: &DefDatabases, opts: &ReaderOptions, set: &ReferenceSet) -> Self {
        let mut rows = Vec::new();
        let mut unit_values = BTreeMap::new();
        for w in &set.weapons {
            let Some(def) = dbs.get(&opts.db_type, &w.def_name) else {
                continue;
            };
            let cost: BTreeMap<String, f64> = named_numbers(&def.node, "costList")
                .into_iter()
                .filter(|(_, c)| c.is_finite() && *c > 0.0)
                .collect();
            for name in cost.keys() {
                if !unit_values.contains_key(name)
                    && let Ok(b) = market_value_of(dbs, opts, name, None)
                {
                    unit_values.insert(name.clone(), b.base_value);
                }
            }
            rows.push(CostRow {
                def_name: w.def_name.clone(),
                kind: super::basis::item_kind(w.kind),
                tier: w.tech_level.map(|t| t.index()),
                role: w.role.clone(),
                strength: w.strength,
                cost,
                stuff_categories: w.stuff_categories.clone(),
                stuff_count: child_number(&def.node, "costStuffCount").filter(|c| *c > 0.0),
            });
        }
        Self { rows, unit_values }
    }

    /// The rows of a kind that make up the cohort of a tier: the weapons of the tier, or all of the kind
    /// when fewer than three qualify. The flag says whether the tier cohort was used.
    #[must_use]
    pub fn cohort(&self, kind: ItemKind, tier: u8) -> (Vec<&CostRow>, bool) {
        let of_kind: Vec<&CostRow> = self.rows.iter().filter(|r| r.kind == kind).collect();
        let tiered: Vec<&CostRow> = of_kind
            .iter()
            .copied()
            .filter(|r| r.tier == Some(tier))
            .collect();
        if tiered.len() >= crate::classes::MIN_POOL {
            (tiered, true)
        } else {
            (of_kind, false)
        }
    }
}

/// A material of the proposed list with the median count of the reference weapons that use it.
#[derive(Debug, Clone, PartialEq)]
pub struct Staple {
    /// The ingredient def name.
    pub def_name: String,
    /// The median count among the weapons that use it.
    pub median_count: f64,
    /// How many weapons of the cohort use it.
    pub users: usize,
    /// The share of the cohort that uses it.
    pub share: f64,
}

/// The ingredients that at least half of the cohort's weapons with a cost list need, most used first.
#[must_use]
pub fn staples(rows: &[&CostRow]) -> Vec<Staple> {
    let listed: Vec<&&CostRow> = rows.iter().filter(|r| !r.cost.is_empty()).collect();
    if listed.is_empty() {
        return Vec::new();
    }
    let mut counts: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for row in &listed {
        for (name, count) in &row.cost {
            counts.entry(name.as_str()).or_default().push(*count);
        }
    }
    let mut out: Vec<Staple> = counts
        .into_iter()
        .filter_map(|(name, values)| {
            let share = values.len() as f64 / listed.len() as f64;
            (share >= 0.5).then(|| Staple {
                def_name: name.to_owned(),
                median_count: median(&values).unwrap_or(1.0),
                users: values.len(),
                share,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.users
            .cmp(&a.users)
            .then_with(|| b.median_count.total_cmp(&a.median_count))
            .then_with(|| a.def_name.cmp(&b.def_name))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, tier: u8, cost: &[(&str, f64)]) -> CostRow {
        CostRow {
            def_name: name.into(),
            kind: ItemKind::Ranged,
            tier: Some(tier),
            role: None,
            strength: None,
            cost: cost.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect(),
            stuff_categories: Vec::new(),
            stuff_count: None,
        }
    }

    #[test]
    fn staples_are_the_ingredients_most_weapons_need() {
        let rows = [
            row("A", 2, &[("RS_Steel", 20.0), ("RS_Part", 2.0)]),
            row("B", 2, &[("RS_Steel", 30.0), ("RS_Part", 3.0)]),
            row("C", 2, &[("RS_Steel", 40.0), ("RS_Rare", 1.0)]),
        ];
        let refs: Vec<&CostRow> = rows.iter().collect();
        let s = staples(&refs);
        let names: Vec<&str> = s.iter().map(|x| x.def_name.as_str()).collect();
        assert_eq!(names, ["RS_Steel", "RS_Part"]);
        assert_eq!(s[0].median_count, 30.0);
    }

    #[test]
    fn the_cohort_widens_to_the_kind_when_the_tier_is_thin() {
        let basis = CostBasis {
            rows: vec![row("A", 2, &[]), row("B", 2, &[]), row("C", 1, &[])],
            unit_values: BTreeMap::new(),
        };
        let (rows, tiered) = basis.cohort(ItemKind::Ranged, 2);
        assert_eq!(rows.len(), 3);
        assert!(!tiered);
    }
}
