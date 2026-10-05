//! A chosen stuff and the stat pipeline of a weapon made from it.
//!
//! A stuffed weapon has no single value for stats such as its mass or its melee multipliers: the value depends
//! on the material. A reference item is read for one reference material (chosen by the caller, or none). The
//! material's effect on a stat is read from its `stuffProps` (stat factors and stat offsets), and the number is
//! produced by [`StatPipeline`], the same function the designer uses everywhere else.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;

use super::access::{list_texts, number_map};
use crate::stats::{StatPipeline, StatRules, StuffEffect};

/// What a material does to item stats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StuffProfile {
    /// Def name of the material.
    pub def_name: String,
    /// Stuff category def names the material belongs to.
    pub categories: Vec<String>,
    /// Stat factors by stat def name.
    pub factors: BTreeMap<String, f64>,
    /// Stat offsets by stat def name.
    pub offsets: BTreeMap<String, f64>,
    /// The material's own `statBases` by stat def name. The game reads a few stats from the material itself
    /// rather than through `stuffProps`: the sharp and blunt damage multipliers of melee attacks, the stuff
    /// powers of armor and the market value.
    pub base_stats: BTreeMap<String, f64>,
}

impl StuffProfile {
    /// Reads the profile from the resolved `ThingDef` of a material. A def without `stuffProps` gives an empty
    /// profile (no factors, no offsets).
    #[must_use]
    pub fn from_def(def_name: &str, node: &Node) -> Self {
        let props = node.child("stuffProps");
        Self {
            def_name: def_name.to_owned(),
            categories: props
                .map(|p| list_texts(p, "categories"))
                .unwrap_or_default(),
            factors: props
                .map(|p| number_map(p, "statFactors"))
                .unwrap_or_default(),
            offsets: props
                .map(|p| number_map(p, "statOffsets"))
                .unwrap_or_default(),
            base_stats: number_map(node, "statBases"),
        }
    }

    /// The material's own value of a stat (its `statBases` entry), `None` when it sets none.
    #[must_use]
    pub fn own_stat(&self, stat: &str) -> Option<f64> {
        self.base_stats.get(stat).copied()
    }

    /// The direct effect on one stat.
    #[must_use]
    pub fn effect(&self, stat: &str) -> StuffEffect {
        StuffEffect {
            factor: self.factors.get(stat).copied(),
            offset: self.offsets.get(stat).copied().unwrap_or(0.0),
        }
    }

    /// True when the material can make an item that accepts the given stuff categories.
    #[must_use]
    pub fn fits(&self, accepted: &[String]) -> bool {
        self.categories.iter().any(|c| accepted.contains(c))
    }
}

/// Evaluates one stat of an item through the pipeline.
///
/// `base` is the item's `statBases` value (`None` uses the rules' default). The stuff only applies when the item
/// is stuffable (`accepted` is not empty) and the profile fits one of its categories.
#[must_use]
pub fn evaluate_stat(
    base: Option<f64>,
    stat: &str,
    stuff: Option<&StuffProfile>,
    accepted: &[String],
    rules: &StatRules,
) -> Option<f64> {
    let chosen = stuff.filter(|s| !accepted.is_empty() && s.fits(accepted));
    let pipeline = StatPipeline {
        rules: *rules,
        base,
        stuff: chosen.map(|s| s.effect(stat)),
        parts: Vec::new(),
    };
    pipeline.evaluate().ok().map(|o| o.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    fn material() -> StuffProfile {
        let node = NodeBuilder::new("ThingDef")
            .elem("stuffProps", |p| {
                p.elem("categories", |c| c.li("RS_Metallic"))
                    .elem("statFactors", |f| f.text_elem("RS_Sharpness", "1.5"))
                    .elem("statOffsets", |f| f.text_elem("RS_Mass", "0.5"))
            })
            .elem("statBases", |b| b.text_elem("RS_SharpMultiplier", "1.25"))
            .build();
        StuffProfile::from_def("RS_Steel", &node)
    }

    #[test]
    fn the_profile_reads_factors_offsets_and_categories() {
        let m = material();
        assert_eq!(m.categories, vec!["RS_Metallic"]);
        assert_eq!(m.effect("RS_Sharpness").factor, Some(1.5));
        assert_eq!(m.effect("RS_Mass").offset, 0.5);
        assert_eq!(m.effect("other").factor, None);
        assert_eq!(m.own_stat("RS_SharpMultiplier"), Some(1.25));
        assert_eq!(m.own_stat("RS_Other"), None);
    }

    #[test]
    fn a_stuff_only_applies_to_a_stuffable_item_it_fits() {
        let m = material();
        let rules = StatRules {
            default_base: 1.0,
            ..StatRules::default()
        };
        let fits = vec!["RS_Metallic".to_owned()];
        let other = vec!["RS_Woody".to_owned()];
        let v = evaluate_stat(None, "RS_Sharpness", Some(&m), &fits, &rules);
        assert!((v.unwrap_or(0.0) - 1.5).abs() < 1e-12);
        let v = evaluate_stat(None, "RS_Sharpness", Some(&m), &other, &rules);
        assert!((v.unwrap_or(0.0) - 1.0).abs() < 1e-12);
        let v = evaluate_stat(Some(2.0), "RS_Sharpness", Some(&m), &[], &rules);
        assert!((v.unwrap_or(0.0) - 2.0).abs() < 1e-12);
        let v = evaluate_stat(Some(2.0), "RS_Mass", Some(&m), &fits, &rules);
        assert!((v.unwrap_or(0.0) - 2.5).abs() < 1e-12);
    }
}
