//! The data types of the def engine: inputs, def records and per def provenance.

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

use crate::provenance::ModOrder;

/// The content of one source file as the boundary crate delivered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum FileContent {
    /// The document element of a parsed file (`Defs` or `Patch` in a healthy file).
    Parsed {
        /// The root element.
        root: Node,
    },
    /// The file could not be read as XML; the game logs a warning and skips it.
    Failed {
        /// Why the file failed, for the diagnostic.
        message: String,
    },
}

impl FileContent {
    /// A parsed file.
    #[must_use]
    pub fn parsed(root: Node) -> Self {
        Self::Parsed { root }
    }

    /// A file that failed to parse.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }
}

/// One `Defs` file of a mod, already read and parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefFile {
    /// The mod that owns the file.
    pub mod_idx: ModIdx,
    /// The session handle of the file (carried in origins and diagnostics).
    pub file: FileId,
    /// The path relative to the load folder, `Defs/...` style (for messages).
    pub rel_path: String,
    /// The parsed content.
    pub content: FileContent,
}

/// One `Patches` file of a mod, already read and parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchFile {
    /// The mod that owns the file.
    pub mod_idx: ModIdx,
    /// The session handle of the file.
    pub file: FileId,
    /// The path relative to the load folder, `Patches/...` style.
    pub rel_path: String,
    /// The parsed content (root element `Patch`).
    pub content: FileContent,
}

/// Where a def (or a node) was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    /// The mod that owns the file.
    pub mod_idx: ModIdx,
    /// The defs file.
    pub file: FileId,
}

/// A patch operation that touched a def's subtree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchRef {
    /// The mod whose patch list holds the operation.
    pub mod_idx: ModIdx,
    /// The patch file, when known.
    pub file: Option<FileId>,
    /// The position of the top-level operation in the mod's patch list.
    pub op_index: u32,
    /// The canonical class name of the operation (the declared name for classes the engine does
    /// not know).
    pub class: String,
    /// The operation's XPath, when it has one (white space collapsed).
    pub xpath: Option<String>,
    /// `Class(xpath)` as one line, the text of the research prototype's `patched_by` entries.
    pub description: String,
}

impl PatchRef {
    /// The research prototype's label: `package:file#index Class(xpath)`; `file_path` resolves the
    /// file handle to a path.
    #[must_use]
    pub fn label(&self, order: &ModOrder, file_path: &dyn Fn(FileId) -> Option<String>) -> String {
        let package = order.package_id(self.mod_idx).unwrap_or("?");
        let file = self.file.and_then(file_path).unwrap_or_default();
        format!("{package}:{file}#{} {}", self.op_index, self.description)
    }
}

/// One ancestor of a def in its inheritance chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentRef {
    /// The mod that declared the parent; `None` for a parent created by a patch.
    pub mod_idx: Option<ModIdx>,
    /// The parent's `Name`.
    pub name: String,
}

impl ParentRef {
    /// The prototype's `mod:Name` label (`patch:Name` for a patch-created parent).
    #[must_use]
    pub fn label(&self, order: &ModOrder) -> String {
        let who = self
            .mod_idx
            .and_then(|m| order.package_id(m))
            .unwrap_or("patch");
        format!("{who}:{}", self.name)
    }
}

/// One def that survived `MayRequire`, `Abstract` and the type checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefRecord {
    /// Position among the created defs (the merged document order).
    pub seq: u32,
    /// The element name of the def node as written.
    pub tag: String,
    /// The full name of the def's type from the type table.
    pub type_name: String,
    /// The def's `defName` (a generated `UnnamedDef<seq>` when absent).
    pub def_name: String,
    /// The file the def came from; `None` for a node created by a patch (no asset).
    pub origin: Option<Origin>,
    /// The resolved node: the parent chain merged in.
    pub node: Node,
    /// The inheritance chain, nearest parent first.
    pub parents: Vec<ParentRef>,
    /// The patch operations that touched the def's subtree, in application order.
    pub patched_by: Vec<PatchRef>,
}

impl DefRecord {
    /// The mod that owns the def, `None` for a patch-created def.
    #[must_use]
    pub fn mod_idx(&self) -> Option<ModIdx> {
        self.origin.map(|o| o.mod_idx)
    }
}

/// Counters of one load, equal to the research prototype's `stats`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadStats {
    /// Defs files read (parsed or not).
    pub files: u32,
    /// Top-level patch operations applied.
    pub patch_ops: u32,
    /// Top-level nodes of the unified document after patching.
    pub top_level_nodes: u32,
    /// Nodes skipped because `Abstract` is true on the original node.
    pub abstract_nodes: u32,
    /// Nodes skipped because `MayRequire` or `MayRequireAnyOf` was not met.
    pub may_require_skipped: u32,
    /// Nodes dropped because their type is no def type.
    pub unknown_type: u32,
    /// Defs created.
    pub defs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::ModEntry;

    #[test]
    fn labels_follow_the_prototype_format() {
        let order = ModOrder::new(vec![ModEntry::new(ModIdx(2), "rs.a", "A")]);
        let p = ParentRef {
            mod_idx: Some(ModIdx(2)),
            name: "Base".into(),
        };
        assert_eq!(p.label(&order), "rs.a:Base");
        let q = ParentRef {
            mod_idx: None,
            name: "Base".into(),
        };
        assert_eq!(q.label(&order), "patch:Base");
        let r = PatchRef {
            mod_idx: ModIdx(2),
            file: Some(FileId(9)),
            op_index: 3,
            class: "PatchOperationAdd".into(),
            xpath: Some("Defs/X".into()),
            description: "PatchOperationAdd(Defs/X)".into(),
        };
        let label = r.label(&order, &|f| Some(format!("Patches/p{}.xml", f.0)));
        assert_eq!(label, "rs.a:Patches/p9.xml#3 PatchOperationAdd(Defs/X)");
    }

    #[test]
    fn file_content_round_trips_through_json() {
        let c = FileContent::parsed(Node::new("Defs"));
        let json = serde_json::to_string(&c).unwrap_or_default();
        let back: Result<FileContent, _> = serde_json::from_str(&json);
        assert_eq!(back.ok(), Some(c));
    }
}
