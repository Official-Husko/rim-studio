//! Stat definitions read from the user's install: defaults and limits per stat.

use std::collections::BTreeMap;

use rimstudio_defs::DefDatabases;

use super::access::{child_bool, child_number};
use crate::stats::StatRules;

/// The finalisation rules of every stat definition that is loaded, by stat def name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatTable {
    rules: BTreeMap<String, StatRules>,
}

impl StatTable {
    /// Reads every def of the database `db_type` (normally `StatDef`). An unknown type gives an empty table.
    #[must_use]
    pub fn from_databases(dbs: &DefDatabases, db_type: &str) -> Self {
        let mut rules = BTreeMap::new();
        let Ok(view) = dbs.database(db_type) else {
            return Self { rules };
        };
        for def in view.iter() {
            let mut r = StatRules::default();
            if let Some(v) = child_number(&def.node, "defaultBaseValue") {
                r.default_base = v;
            }
            if let Some(v) = child_number(&def.node, "minValue") {
                r.min_value = v;
            }
            if let Some(v) = child_number(&def.node, "maxValue") {
                r.max_value = v;
            }
            if let Some(v) = child_bool(&def.node, "applyFactorsIfNegative") {
                r.apply_factors_if_negative = v;
            }
            if let Some(v) = child_number(&def.node, "roundToFiveOver") {
                r.round_to_five_over = Some(v);
            }
            if let Some(v) = child_bool(&def.node, "roundValue") {
                r.round_value = v;
            }
            if r.min_value > r.max_value {
                r = StatRules::default();
            }
            rules.insert(def.def_name.clone(), r);
        }
        Self { rules }
    }

    /// The rules of a stat; a stat without a definition has the open default.
    #[must_use]
    pub fn rules(&self, stat: &str) -> StatRules {
        self.rules.get(stat).copied().unwrap_or_default()
    }

    /// Whether a definition of the stat is loaded.
    #[must_use]
    pub fn knows(&self, stat: &str) -> bool {
        self.rules.contains_key(stat)
    }

    /// Number of stat definitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// True when no stat definition is loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::fixtures_tests::{load_defs, types};
    use rimstudio_core::tree::NodeBuilder;

    #[test]
    fn rounding_rules_of_a_stat_definition_are_read() {
        let rounded = NodeBuilder::new("StatDef")
            .text_elem("defName", "RS_Rounded")
            .text_elem("roundToFiveOver", "200")
            .text_elem("roundValue", "true")
            .text_elem("minValue", "0")
            .build();
        let plain = NodeBuilder::new("StatDef")
            .text_elem("defName", "RS_Plain")
            .build();
        let out = load_defs(vec![rounded, plain], &types(&[]));
        let table = StatTable::from_databases(&out.databases, "StatDef");
        let r = table.rules("RS_Rounded");
        assert_eq!(r.round_to_five_over, Some(200.0));
        assert!(r.round_value);
        assert_eq!(r.min_value, 0.0);
        let p = table.rules("RS_Plain");
        assert_eq!(p.round_to_five_over, None);
        assert!(!p.round_value);
        assert!(table.knows("RS_Plain") && !table.knows("RS_Other"));
    }
}
