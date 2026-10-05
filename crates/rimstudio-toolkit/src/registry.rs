//! The static table of tools (ADR 0004).
//!
//! The app composes its tool list from [`TOOLS`]: one row per tool module, present only when the Cargo
//! feature of the module is on. A row names the command areas the tool uses and the capabilities it needs;
//! [`descriptors`] resolves the table against the capabilities of the current machine into the DTOs of
//! `app_list_tools`. Nothing here depends on the tool modules themselves, so a module can be compiled out
//! without touching another one.

use rimstudio_ipc_types::tools::{Capability, ToolDescriptor};

/// One tool of the toolkit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDef {
    /// Stable id, for example `designer`; the translation keys derive from it.
    pub id: &'static str,
    /// The Cargo feature that compiles the module.
    pub feature: &'static str,
    /// Command areas the tool uses (the part of a command name before the first underscore).
    pub areas: &'static [&'static str],
    /// Capabilities the tool needs to be usable, in a stable order.
    pub requires: &'static [Capability],
    /// Capabilities that unlock extra parts of the tool but are not needed to open it (the designer works
    /// without Combat Extended; only the optional patch needs it).
    pub optional: &'static [Capability],
}

impl ToolDef {
    /// The descriptor of this tool resolved against the capabilities that are present.
    #[must_use]
    pub fn descriptor(&self, present: &[Capability]) -> ToolDescriptor {
        ToolDescriptor::resolve(self.id, self.areas, self.requires, present)
    }
}

/// The tools compiled into this build, in display order.
pub static TOOLS: &[ToolDef] = &[
    #[cfg(feature = "tool-designer")]
    ToolDef {
        id: "designer",
        feature: "tool-designer",
        areas: &["designer"],
        requires: &[Capability::GameInstall],
        optional: &[Capability::CombatExtended],
    },
    #[cfg(feature = "tool-project")]
    ToolDef {
        id: "project",
        feature: "tool-project",
        areas: &["project"],
        requires: &[],
        optional: &[],
    },
    #[cfg(feature = "tool-defs")]
    ToolDef {
        id: "defs",
        feature: "tool-defs",
        areas: &["defs"],
        requires: &[Capability::GameInstall],
        optional: &[],
    },
];

/// The row of the tool with this id.
#[must_use]
pub fn find(id: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.id == id)
}

/// Every tool as a descriptor, availability resolved against `present`.
#[must_use]
pub fn descriptors(present: &[Capability]) -> Vec<ToolDescriptor> {
    TOOLS.iter().map(|t| t.descriptor(present)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(all(
        feature = "tool-designer",
        feature = "tool-project",
        feature = "tool-defs"
    ))]
    fn every_default_feature_has_a_row_with_a_unique_id() {
        let ids: Vec<&str> = TOOLS.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["designer", "project", "defs"]);
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }

    #[test]
    #[cfg(feature = "tool-designer")]
    fn designer_needs_the_game_but_not_combat_extended() {
        let Some(designer) = find("designer") else {
            panic!("designer row missing");
        };
        assert!(designer.requires.contains(&Capability::GameInstall));
        assert!(!designer.requires.contains(&Capability::CombatExtended));
        assert!(designer.optional.contains(&Capability::CombatExtended));
        assert_eq!(designer.feature, "tool-designer");
    }

    #[test]
    #[cfg(all(
        feature = "tool-designer",
        feature = "tool-project",
        feature = "tool-defs"
    ))]
    fn descriptors_resolve_missing_capabilities() {
        let none = descriptors(&[]);
        let designer = none.iter().find(|d| d.id == "designer");
        assert_eq!(
            designer.map(|d| d.missing.clone()),
            Some(vec![Capability::GameInstall])
        );
        assert!(none.iter().any(|d| d.id == "project" && d.available));
        let with_game = descriptors(&[Capability::GameInstall]);
        assert!(with_game.iter().all(|d| d.available));
    }

    #[test]
    fn areas_name_commands_of_the_tool() {
        for t in TOOLS {
            assert!(t.areas.contains(&t.id), "{} should own its own area", t.id);
        }
    }
}
