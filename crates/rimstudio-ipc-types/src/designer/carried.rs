//! The fields a weapon definition has beyond the calculated numbers: the crafting recipe, the extra attacks
//! of a tool, and the raw XML fields a clone carries from its source.
//!
//! Raw fields travel as node trees ([`RawNodeDto`]): the JSON form of an XML element, the same shape the
//! engine uses. The webview never parses XML; it shows a node tree as text and sends back the tree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// An XML element as JSON: a tag, ordered attributes and ordered children.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RawNodeDto {
    /// Element name.
    pub tag: String,
    /// Attributes in source order as `[name, value]` pairs. Absent in input means none.
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    /// Child elements and text in document order. Absent in input means none.
    #[serde(default)]
    pub children: Vec<RawChildDto>,
}

/// A child of an element: another element or a text run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum RawChildDto {
    /// A nested element.
    Element(RawNodeDto),
    /// A text run.
    Text(String),
}

/// The crafting recipe of a weapon (`recipeMaker` without the research prerequisite).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct RecipeSpecDto {
    /// Skill levels needed to craft the weapon, by skill name. Omitted when empty.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, u32>>", optional))]
    pub skill_requirements: BTreeMap<String, u32>,
    /// Position of the recipe in the bill menu. Absent means the game default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub display_priority: Option<f64>,
    /// Workbenches that offer the recipe. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub recipe_users: Vec<String>,
    /// The unfinished thing that stands in for the weapon while it is made. Absent means inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub unfinished_thing_def: Option<String>,
    /// The skill that works the recipe. Absent means inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub work_skill: Option<String>,
    /// Other children of the recipe, as written. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
    /// Attributes of the recipe element (`IsNull`, `Inherit`), by name. Omitted when empty.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, String>>", optional))]
    pub attrs: BTreeMap<String, String>,
}

/// One extra damage of a melee attack.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ExtraMeleeDamageDto {
    /// Damage definition name.
    pub def: String,
    /// Damage amount. Absent means the game default.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub amount: Option<f64>,
    /// Chance that the extra damage applies. Absent means always.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub chance: Option<f64>,
}

/// The extra damage of a surprise attack.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct SurpriseAttackSpecDto {
    /// The extra damages. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ExtraMeleeDamageDto>>", optional))]
    pub extra_melee_damages: Vec<ExtraMeleeDamageDto>,
}
