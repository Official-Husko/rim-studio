//! Provenance: where a node of the unified document came from, and the order of mods.
//!
//! Every node of the unified document carries an [`OriginId`] (the arena's opaque tag). The
//! [`OriginTable`] gives the tags their meaning: a defs file of a mod, or a patch operation that
//! created the node. A top-level node created by a patch has no mod, which is exactly the game's
//! "no asset" case that inheritance and the databases treat specially.
//!
//! [`ModOrder`] is the load order of the mods taking part in a load, with the display names that
//! `PatchOperationFindMod` compares and the overwrite priority that decides database order.

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::OriginId;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// The package id of the game's own Core content (compared ignoring case).
pub const CORE_PACKAGE_ID: &str = "ludeon.rimworld";

/// What an [`OriginId`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OriginKind {
    /// The node was read from a defs file of a mod.
    File {
        /// The mod that owns the file.
        mod_idx: ModIdx,
        /// The file.
        file: FileId,
    },
    /// The node was created by a patch operation.
    Patch {
        /// The mod whose patch list holds the operation.
        mod_idx: ModIdx,
        /// The patch file, when known.
        file: Option<FileId>,
        /// The position of the top-level operation in the mod's patch list.
        op_index: u32,
    },
}

/// The interned origins of one unified document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OriginTable {
    entries: Vec<OriginKind>,
}

impl OriginTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an origin and returns its tag. Tags are dense and start at zero.
    pub fn push(&mut self, kind: OriginKind) -> OriginId {
        let id = u32::try_from(self.entries.len()).unwrap_or(u32::MAX - 1);
        self.entries.push(kind);
        OriginId(id)
    }

    /// The meaning of a tag, `None` for [`OriginId::NONE`] and unknown tags.
    #[must_use]
    pub fn get(&self, id: OriginId) -> Option<OriginKind> {
        self.entries.get(id.0 as usize).copied()
    }

    /// Number of origins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no origin was added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One mod taking part in a load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    /// The session handle used in origins and diagnostics.
    pub idx: ModIdx,
    /// The package id as declared (compared ignoring case).
    pub package_id: String,
    /// The display name (`PatchOperationFindMod` compares it exactly).
    pub name: String,
    /// True for the game's Core content: its defs enter the databases first.
    pub core: bool,
}

impl ModEntry {
    /// An entry; `core` is derived from the package id ([`CORE_PACKAGE_ID`]).
    pub fn new(idx: ModIdx, package_id: impl Into<String>, name: impl Into<String>) -> Self {
        let package_id = package_id.into();
        let core = package_id.trim().eq_ignore_ascii_case(CORE_PACKAGE_ID);
        Self {
            idx,
            package_id,
            name: name.into(),
            core,
        }
    }

    /// The game's `OverwritePriority`: 0 for Core, 1 for everything else.
    #[must_use]
    pub fn overwrite_priority(&self) -> u8 {
        u8::from(!self.core)
    }
}

/// The mods of a load in load order (position 0 loads first).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ModOrder {
    entries: Vec<ModEntry>,
    #[serde(skip)]
    pos: FxHashMap<ModIdx, usize>,
}

impl ModOrder {
    /// Builds the order from entries in load order. A repeated handle keeps its first position.
    #[must_use]
    pub fn new(entries: Vec<ModEntry>) -> Self {
        let mut pos = FxHashMap::default();
        for (i, e) in entries.iter().enumerate() {
            pos.entry(e.idx).or_insert(i);
        }
        Self { entries, pos }
    }

    /// The load order position of a mod (0 loads first).
    #[must_use]
    pub fn load_order(&self, idx: ModIdx) -> Option<usize> {
        self.pos.get(&idx).copied()
    }

    /// The entry of a mod.
    #[must_use]
    pub fn get(&self, idx: ModIdx) -> Option<&ModEntry> {
        self.load_order(idx).and_then(|p| self.entries.get(p))
    }

    /// The package id of a mod.
    #[must_use]
    pub fn package_id(&self, idx: ModIdx) -> Option<&str> {
        self.get(idx).map(|e| e.package_id.as_str())
    }

    /// The entries in load order.
    pub fn iter(&self) -> impl Iterator<Item = &ModEntry> {
        self.entries.iter()
    }

    /// Number of mods.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no mods.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The mods in the order `DefDatabase<T>.AddAllInMods` visits them: Core first, then the rest
    /// by load order.
    #[must_use]
    pub fn database_order(&self) -> Vec<ModIdx> {
        let mut order: Vec<(u8, usize, ModIdx)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.overwrite_priority(), i, e.idx))
            .collect();
        order.sort_unstable();
        order.into_iter().map(|(_, _, idx)| idx).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> ModOrder {
        ModOrder::new(vec![
            ModEntry::new(ModIdx(7), "RS.Other", "Other"),
            ModEntry::new(ModIdx(3), "Ludeon.RimWorld", "Core"),
            ModEntry::new(ModIdx(5), "rs.last", "Last"),
        ])
    }

    #[test]
    fn load_order_is_the_position() {
        let o = entries();
        assert_eq!(o.load_order(ModIdx(7)), Some(0));
        assert_eq!(o.load_order(ModIdx(5)), Some(2));
        assert_eq!(o.load_order(ModIdx(1)), None);
        assert_eq!(o.package_id(ModIdx(3)), Some("Ludeon.RimWorld"));
    }

    #[test]
    fn database_order_puts_core_first() {
        assert_eq!(
            entries().database_order(),
            vec![ModIdx(3), ModIdx(7), ModIdx(5)]
        );
    }

    #[test]
    fn origin_tags_are_dense() {
        let mut t = OriginTable::new();
        let a = t.push(OriginKind::File {
            mod_idx: ModIdx(0),
            file: FileId(0),
        });
        let b = t.push(OriginKind::Patch {
            mod_idx: ModIdx(0),
            file: None,
            op_index: 4,
        });
        assert_eq!((a, b), (OriginId(0), OriginId(1)));
        assert_eq!(t.get(OriginId::NONE), None);
        assert_eq!(t.len(), 2);
    }
}
