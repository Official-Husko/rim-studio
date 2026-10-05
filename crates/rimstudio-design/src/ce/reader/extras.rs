//! What the converted weapons of the install say about the parts of a conversion that the model of numbers
//! does not cover: which children of the vanilla tools a conversion dropped, and the recoil pattern of the
//! converted verb.
//!
//! The data is read at run time from the resolved defs (R11): for every converted weapon the names of the
//! tool children it carries and the names its vanilla twin carries. Habits built on it live in
//! [`crate::ce::patchgen::extras`]; nothing here decides anything.

use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, DefRecord};
use serde::{Deserialize, Serialize};

use super::names::CeClassNames;
use crate::reader::access::{child_text, class_attr, list_items};

/// The children of a tool that the converted tool is built from, so that they are never "carried" or
/// "dropped" children: the modelled fields of [`crate::model::ToolSpec`] and their converted spellings.
pub const MODELLED_TOOL_FIELDS: [&str; 11] = [
    "label",
    "capacities",
    "power",
    "cooldownTime",
    "chanceFactor",
    "armorPenetration",
    "armorPenetrationSharp",
    "armorPenetrationBlunt",
    "linkedBodyPartsGroup",
    "extraMeleeDamages",
    "surpriseAttack",
];

/// What one converted weapon says about its tools and verb beyond the numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeaponExtras {
    /// The def name.
    pub def_name: String,
    /// The names of the tool children the converted weapon carries (not the modelled fields), sorted.
    pub tool_fields: Vec<String>,
    /// The names of the tool children the vanilla twin carries (not the modelled fields), sorted. Empty
    /// without a twin.
    pub twin_tool_fields: Vec<String>,
    /// The `recoilPattern` of the converted verb, when it is written.
    pub recoil_pattern: Option<String>,
}

/// The extras of every converted weapon of the install.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtrasLibrary {
    /// The converted guns, by def name.
    pub guns: Vec<WeaponExtras>,
    /// The converted melee weapons, by def name.
    pub melee: Vec<WeaponExtras>,
}

impl ExtrasLibrary {
    /// The extras of a converted gun.
    #[must_use]
    pub fn gun(&self, def_name: &str) -> Option<&WeaponExtras> {
        self.guns.iter().find(|g| g.def_name == def_name)
    }

    /// The extras of a converted melee weapon.
    #[must_use]
    pub fn melee_weapon(&self, def_name: &str) -> Option<&WeaponExtras> {
        self.melee.iter().find(|g| g.def_name == def_name)
    }
}

/// The sorted names of the tool children of a def that are not modelled fields.
fn tool_fields(node: &Node) -> Vec<String> {
    let mut out: Vec<String> = list_items(node, "tools")
        .into_iter()
        .flat_map(Node::elements)
        .map(|e| e.tag.clone())
        .filter(|t| !MODELLED_TOOL_FIELDS.contains(&t.as_str()))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn one(
    def: &DefRecord,
    classes: &CeClassNames,
    twin: Option<&DefDatabases>,
    db_type: &str,
) -> WeaponExtras {
    let recoil_pattern = list_items(&def.node, "verbs")
        .into_iter()
        .find(|li| class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(&classes.verb_properties)))
        .and_then(|v| child_text(v, "recoilPattern"));
    WeaponExtras {
        def_name: def.def_name.clone(),
        tool_fields: tool_fields(&def.node),
        twin_tool_fields: twin
            .and_then(|t| t.get(db_type, &def.def_name))
            .map(|t| tool_fields(&t.node))
            .unwrap_or_default(),
        recoil_pattern,
    }
}

/// Reads the extras of the converted guns and melee weapons. `twin` is the vanilla load, when given.
#[must_use]
pub fn read_extras(
    guns: &[&DefRecord],
    melee: &[&DefRecord],
    classes: &CeClassNames,
    twin: Option<&DefDatabases>,
    db_type: &str,
) -> ExtrasLibrary {
    ExtrasLibrary {
        guns: guns
            .iter()
            .map(|d| one(d, classes, twin, db_type))
            .collect(),
        melee: melee
            .iter()
            .map(|d| one(d, classes, twin, db_type))
            .collect(),
    }
}
