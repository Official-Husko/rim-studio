//! Optional additions of the Combat Extended block that go beyond the converted numbers: an explicit tool
//! plan, accepted companion tags and raw nodes.
//!
//! Combat Extended's own conversions differ from the plain conversion of a design in ways the design has no
//! field for: a tool list that was restructured by hand (a muzzle tool, relabelled tools), tags beyond the
//! class tag, art or platform specific additions. Each of them is a choice, never written on its own (D-085):
//! the user turns it on, usually by accepting a suggestion ([`crate::ce::suggest::options`]), and the
//! generator then writes exactly what the block says. Everything here is plain data with `serde` support
//! and optional by default: a spec without these fields converts exactly as before.

use serde::{Deserialize, Serialize};

/// One tool of an explicit tool plan.
///
/// A plan entry starts from a tool of the vanilla design (the one named by `from`, else the one with the
/// same label), takes over its numbers, penetration, body part group and carried children, and then applies
/// the fields it sets itself. An entry without a vanilla tool to start from has to give every field the
/// converted tool needs: capacities, power, cooldown and a blunt penetration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeToolPlan {
    /// The label the converted tool carries. REQ.
    pub label: String,
    /// The label of the vanilla tool this entry starts from (OPT; the entry's own label when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The capacities of the tool (OPT with a vanilla tool to start from).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacities: Option<Vec<String>>,
    /// Tool power (OPT with a vanilla tool to start from).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power: Option<f64>,
    /// Cooldown in seconds (OPT with a vanilla tool to start from).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<f64>,
    /// The pick weight (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chance_factor: Option<f64>,
    /// Sharp penetration in mm of rolled homogeneous armor (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration_sharp: Option<f64>,
    /// Blunt penetration in MPa (REQ without a vanilla tool to start from).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration_blunt: Option<f64>,
    /// The body part group that carries the tool (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_body_parts_group: Option<String>,
}

impl CeToolPlan {
    /// An entry that starts from the vanilla tool with the same label and changes nothing yet.
    #[must_use]
    pub fn keep(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    /// The label of the vanilla tool the entry starts from.
    #[must_use]
    pub fn source_label(&self) -> &str {
        self.from.as_deref().unwrap_or(&self.label)
    }
}
