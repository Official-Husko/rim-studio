//! Tool descriptors: the static table of tools the shell shows, with the capabilities each one needs.
//!
//! `app_list_tools` returns the table with the availability of each capability resolved against the
//! current machine, so the webview can disable a tool and say why without probing anything itself.

use serde::{Deserialize, Serialize};

/// Something the machine or the session must have for a tool to work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// A RimWorld install was found or chosen.
    GameInstall,
    /// A Steam Workshop content folder was found or chosen.
    WorkshopFolder,
    /// Combat Extended is installed (needed for reading its values at run time).
    CombatExtended,
    /// A mod project is open in the workspace.
    OpenProject,
    /// The Steam client is installed.
    SteamClient,
    /// Network access is allowed by the user.
    Network,
}

/// One tool of the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    /// Stable id, for example `designer`.
    pub id: String,
    /// Catalog key of the tool name.
    pub label_key: String,
    /// Catalog key of the one line description.
    pub description_key: String,
    /// Command areas the tool uses, for example `designer` and `project`.
    pub areas: Vec<String>,
    /// Capabilities the tool needs, in a stable order.
    pub requires: Vec<Capability>,
    /// The subset of `requires` that is missing now; empty when the tool is usable.
    pub missing: Vec<Capability>,
    /// True when `missing` is empty.
    pub available: bool,
}

impl ToolDescriptor {
    /// Builds a descriptor and resolves availability against `present`.
    #[must_use]
    pub fn resolve(
        id: &str,
        areas: &[&str],
        requires: &[Capability],
        present: &[Capability],
    ) -> Self {
        let missing: Vec<Capability> = requires
            .iter()
            .copied()
            .filter(|c| !present.contains(c))
            .collect();
        Self {
            id: id.to_owned(),
            label_key: format!("tool.{id}.name"),
            description_key: format!("tool.{id}.description"),
            areas: areas.iter().map(|a| (*a).to_owned()).collect(),
            requires: requires.to_vec(),
            available: missing.is_empty(),
            missing,
        }
    }
}

/// Response of `app_list_tools`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppListToolsResponse {
    /// The tools, in display order.
    pub tools: Vec<ToolDescriptor>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_lists_missing_capabilities_in_requirement_order() {
        let tool = ToolDescriptor::resolve(
            "designer",
            &["designer", "project"],
            &[Capability::GameInstall, Capability::OpenProject],
            &[Capability::GameInstall],
        );
        assert_eq!(tool.missing, vec![Capability::OpenProject]);
        assert!(!tool.available);
        assert_eq!(tool.label_key, "tool.designer.name");
    }

    #[test]
    fn tool_with_everything_present_is_available_and_round_trips() {
        let tool = ToolDescriptor::resolve(
            "designer",
            &["designer"],
            &[Capability::GameInstall],
            &[Capability::GameInstall],
        );
        assert!(tool.available && tool.missing.is_empty());
        let text = serde_json::to_string(&tool).unwrap_or_default();
        let back: Result<ToolDescriptor, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(tool));
    }
}
