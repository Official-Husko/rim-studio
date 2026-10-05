//! The market value breakdown of a loaded def, from its cost list, stuff and work.
//!
//! The game prices an item without its own `MarketValue` as the sum of its ingredients, the stuff it is made
//! from and its work (see [`crate::price`]). This module reads those inputs out of the resolved defs of a load
//! and runs the pure formula, so the designer and the parity checks price real items the way the game does.
//!
//! Two rules matter beyond the formula itself. The stuff count is divided by the stuff's volume per unit, which is
//! 0.1 for a material flagged `smallVolume` (silver and gold) and 1 otherwise. And the work is the larger of
//! `WorkToMake` and `WorkToBuild`, each scaled by the material's stat factor for it. Items whose price comes from
//! a recipe (no cost list and no stuff count) are priced from their work alone: the recipe is not read.

use rimstudio_defs::DefDatabases;

use super::access::{child_bool, child_number, named_numbers, number_map};
use super::options::ReaderOptions;
use crate::error::{DesignError, DesignResult};
use crate::price::{
    IngredientLine, PriceBreakdown, PriceInput, StuffCost, display_round, market_value,
};

/// Ingredient chains deeper than this are cut off (a def that lists itself as an ingredient).
const MAX_DEPTH: u32 = 8;

fn stat_factor(node: &rimstudio_core::tree::Node, stat: &str) -> f64 {
    node.child("stuffProps")
        .map(|p| number_map(p, "statFactors"))
        .and_then(|m| m.get(stat).copied())
        .unwrap_or(1.0)
}

/// The base market value of one item of a def: its own `MarketValue` stat, or the computed value with no stuff.
fn base_value(dbs: &DefDatabases, opts: &ReaderOptions, name: &str, depth: u32) -> f64 {
    if depth > MAX_DEPTH {
        return 0.0;
    }
    let Some(def) = dbs.get(&opts.db_type, name) else {
        return 0.0;
    };
    if let Some(v) = number_map(&def.node, "statBases").get("MarketValue") {
        return *v;
    }
    computed(dbs, opts, name, None, depth).map_or(0.0, |b| b.base_value)
}

fn computed(
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    name: &str,
    stuff: Option<&str>,
    depth: u32,
) -> DesignResult<PriceBreakdown> {
    let def = dbs
        .get(&opts.db_type, name)
        .ok_or_else(|| DesignError::invalid("def", format!("{name} is not a loaded def")))?;
    let node = &def.node;
    let ingredients: Vec<IngredientLine> = named_numbers(node, "costList")
        .into_iter()
        .map(|(ingredient, count)| IngredientLine {
            count,
            unit_value: base_value(dbs, opts, &ingredient, depth + 1),
        })
        .collect();
    let count = child_number(node, "costStuffCount").unwrap_or(0.0);
    let stats = number_map(node, "statBases");
    let mut work_to_make = stats.get("WorkToMake").copied().unwrap_or(0.0);
    let mut work_factor = 1.0;
    let build = stats.get("WorkToBuild").copied().unwrap_or(0.0);
    let mut stuff_cost = None;
    if count > 0.0 {
        stuff_cost = Some(match stuff {
            None => StuffCost::Unknown { count },
            Some(stuff_name) => {
                let stuff_def = dbs.get(&opts.db_type, stuff_name).ok_or_else(|| {
                    DesignError::invalid("stuff", format!("{stuff_name} is not a loaded def"))
                })?;
                work_factor = stat_factor(&stuff_def.node, "WorkToMake");
                StuffCost::Known {
                    count,
                    volume_per_unit: if child_bool(&stuff_def.node, "smallVolume") == Some(true) {
                        0.1
                    } else {
                        1.0
                    },
                    unit_value: base_value(dbs, opts, stuff_name, depth + 1),
                }
            }
        });
    }
    if build > work_to_make * work_factor {
        work_to_make = build;
        work_factor = stuff
            .and_then(|s| dbs.get(&opts.db_type, s))
            .map_or(1.0, |d| stat_factor(&d.node, "WorkToBuild"));
    }
    market_value(&PriceInput {
        ingredients,
        stuff: stuff_cost,
        work_to_make,
        work_factor,
    })
}

/// The market value breakdown of a def made from `stuff` (a material def name; `None` prices the stuff count
/// at the game's guess of 2 silver per unit).
///
/// A def that sets `MarketValue` in its `statBases` is returned as it is written (its parts are zero), with the
/// displayed rounding applied. The quality factor and the hit point curve are not applied: the value is the one of
/// a Normal quality item at full health.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when the def or the material is not loaded, or when a number of the def is
/// not usable by the formula.
pub fn market_value_of(
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    def_name: &str,
    stuff: Option<&str>,
) -> DesignResult<PriceBreakdown> {
    let def = dbs
        .get(&opts.db_type, def_name)
        .ok_or_else(|| DesignError::invalid("def", format!("{def_name} is not a loaded def")))?;
    if let Some(v) = number_map(&def.node, "statBases").get("MarketValue") {
        return Ok(PriceBreakdown {
            ingredients: 0.0,
            stuff: 0.0,
            work: 0.0,
            base_value: *v,
            displayed: display_round(*v)?,
        });
    }
    computed(dbs, opts, def_name, stuff, 0)
}
