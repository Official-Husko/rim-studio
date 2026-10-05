//! Fields a weapon definition has beyond the numbers the designer calculates: the crafting recipe, the
//! extra attacks of a tool, and the raw fields that a clone carries from its source.
//!
//! The first-class types ([`RecipeSpec`], [`ExtraMeleeDamage`], [`SurpriseAttackSpec`]) hold the fields
//! modders edit by hand. Everything else a definition defines for itself travels as raw node trees
//! ([`Node`], the JSON tree of the XML boundary): the draft stores them as they were read, the writer emits
//! them verbatim after the modelled fields, and the user can remove them. A clone therefore reproduces its
//! source, and nothing the designer does not understand is lost silently.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

/// The element names of the root of a thing definition that the writer produces from modelled fields. A raw
/// extra field with one of these names would be written twice, so the validator refuses it.
pub const MODELLED_THING_FIELDS: [&str; 20] = [
    "defName",
    "label",
    "description",
    "techLevel",
    "statBases",
    "costList",
    "costStuffCount",
    "stuffCategories",
    "recipeMaker",
    "weaponTags",
    "tradeTags",
    "weaponClasses",
    "graphicData",
    "verbs",
    "tools",
    "comps",
    "soundInteract",
    "uiIconPath",
    "uiIconScale",
    "equippedStatOffsets",
];

/// The names of the list containers that may be written with `Inherit="False"`.
pub const INHERIT_RESETTABLE: [&str; 12] = [
    "weaponTags",
    "tradeTags",
    "weaponClasses",
    "comps",
    "tools",
    "verbs",
    "costList",
    "stuffCategories",
    "recipeUsers",
    "equippedStatOffsets",
    "statBases",
    "skillRequirements",
];

/// The crafting recipe of a weapon: the `recipeMaker` element without the research prerequisite (which has
/// its own field on the spec).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecipeSpec {
    /// The skill levels needed to craft the weapon, by skill def name (`Crafting` to `5`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub skill_requirements: BTreeMap<String, u32>,
    /// The position of the recipe in the bill menu (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_priority: Option<f64>,
    /// The thing defs of the workbenches that offer the recipe (OPT).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recipe_users: Vec<String>,
    /// The unfinished thing def that stands in for the weapon while it is made (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unfinished_thing_def: Option<String>,
    /// The skill that works the recipe (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_skill: Option<String>,
    /// Other children of `recipeMaker`, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
    /// Attributes of the `recipeMaker` element, by name (`IsNull="True"` removes the recipe of the parent,
    /// `Inherit="False"` replaces it).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
}

impl RecipeSpec {
    /// True when the recipe holds nothing: an empty `recipeMaker` element.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skill_requirements.is_empty()
            && self.display_priority.is_none()
            && self.recipe_users.is_empty()
            && self.unfinished_thing_def.is_none()
            && self.work_skill.is_none()
            && self.extra.is_empty()
            && self.attrs.is_empty()
    }
}

/// One extra damage a melee attack deals (`extraMeleeDamages`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtraMeleeDamage {
    /// The damage def name.
    pub def: String,
    /// The damage amount (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    /// The chance that the extra damage applies (OPT, 1 when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chance: Option<f64>,
}

/// The extra damage of a surprise attack (`surpriseAttack`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SurpriseAttackSpec {
    /// The extra damages of the surprise attack.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_melee_damages: Vec<ExtraMeleeDamage>,
}

/// Parses a raw node as the typed damage entry, `None` when it holds anything else than `def`, `amount`
/// and `chance` or a value that is not a number.
#[must_use]
pub fn extra_damage_from_node(node: &Node) -> Option<ExtraMeleeDamage> {
    if node.tag != "li" || !node.attrs.is_empty() {
        return None;
    }
    let mut out = ExtraMeleeDamage::default();
    for child in node.elements() {
        let text = child.leaf_text()?.trim();
        match child.tag.as_str() {
            "def" => out.def = text.to_owned(),
            "amount" => out.amount = Some(text.parse().ok()?),
            "chance" => out.chance = Some(text.parse().ok()?),
            _ => return None,
        }
    }
    (!out.def.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_recipe_is_empty_and_serializes_to_nothing() {
        let r = RecipeSpec::default();
        assert!(r.is_empty());
        assert_eq!(serde_json::to_string(&r).unwrap(), "{}");
    }

    #[test]
    fn a_recipe_round_trips_with_raw_extras() {
        let mut r = RecipeSpec::default();
        r.skill_requirements.insert("Crafting".into(), 5);
        r.display_priority = Some(450.0);
        r.recipe_users = vec!["TableMachining".into()];
        r.extra
            .push(Node::with_text("soundWorking", "Recipe_Machining"));
        let text = serde_json::to_string(&r).unwrap();
        let back: RecipeSpec = serde_json::from_str(&text).unwrap();
        assert_eq!(back, r);
        assert!(!back.is_empty());
    }

    #[test]
    fn a_damage_entry_with_only_known_children_is_typed() {
        let mut li = Node::new("li");
        li.push_child(Node::with_text("def", "Stun"));
        li.push_child(Node::with_text("amount", "14"));
        let d = extra_damage_from_node(&li).unwrap();
        assert_eq!(d.def, "Stun");
        assert_eq!(d.amount, Some(14.0));
        assert_eq!(d.chance, None);
    }

    #[test]
    fn a_damage_entry_with_a_foreign_child_stays_raw() {
        let mut li = Node::new("li");
        li.push_child(Node::with_text("def", "Stun"));
        li.push_child(Node::with_text("armorPenetration", "1"));
        assert!(extra_damage_from_node(&li).is_none());
        let mut bad = Node::new("li");
        bad.push_child(Node::with_text("def", "Stun"));
        bad.push_child(Node::with_text("amount", "many"));
        assert!(extra_damage_from_node(&bad).is_none());
    }
}
