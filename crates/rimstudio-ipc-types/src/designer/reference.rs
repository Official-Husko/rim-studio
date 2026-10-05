//! Reference weapons: the list the designer compares against, with per stat pool summaries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::spec::{ItemKindDto, TechLevelDto};

/// Request of `designer_reference_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerReferenceListRequest {
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// Restrict to a role. Absent means every role.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub role: Option<String>,
    /// Restrict to a tech level. Absent means every tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tier: Option<TechLevelDto>,
    /// Zero based offset of the first row.
    pub offset: u32,
    /// Maximum rows; the backend caps it at 500.
    pub limit: u32,
}

impl Default for DesignerReferenceListRequest {
    fn default() -> Self {
        Self {
            kind: ItemKindDto::Ranged,
            role: None,
            tier: None,
            offset: 0,
            limit: 100,
        }
    }
}

/// One reference weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReferenceItemDto {
    /// Position in the strength order of the class, zero is the weakest.
    pub index: u32,
    /// Definition name.
    pub def_name: String,
    /// Display label.
    pub label: String,
    /// Tech level. Absent when the definition has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tier: Option<TechLevelDto>,
    /// Role. Absent when no role rule matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub role: Option<String>,
    /// Stable id of the defining mod. Absent for official content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mod_id: Option<String>,
    /// Strength score. Absent when the weapon lacks the inputs to compute one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub strength: Option<f64>,
    /// Exact stat values by stat name, ordered by name.
    pub stats: BTreeMap<String, f64>,
}

/// Distribution of one stat in the class pool, for ruler tick sliders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StatPoolDto {
    /// Stat name.
    pub stat: String,
    /// Pool size behind the numbers.
    pub n: u32,
    /// Minimum.
    pub min: f64,
    /// Tenth percentile.
    pub p10: f64,
    /// Median.
    pub median: f64,
    /// Ninetieth percentile.
    pub p90: f64,
    /// Maximum.
    pub max: f64,
}

/// Response of `designer_reference_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReferenceListDto {
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// Human readable name of the class the list covers.
    pub class_label: String,
    /// Total items of the class.
    pub total: u32,
    /// Offset of the first returned item.
    pub offset: u32,
    /// The items.
    pub items: Vec<ReferenceItemDto>,
    /// Pool distributions by stat, ordered by stat name.
    pub pools: Vec<StatPoolDto>,
}
