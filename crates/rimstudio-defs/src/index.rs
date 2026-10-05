//! The `defName` hash index of the unified document.
//!
//! The dominant patch shape is `Defs/Type[defName="x"]/...`; scanning every top-level node for each
//! of thousands of operations is the slow part of the game's own loader. [`DefIndex`] maps each
//! `defName` text to the top-level nodes that carry it, so a lookup costs one hash probe, and it can
//! re-index a single node after a patch touched it.
//!
//! Matching mirrors the XPath comparison `defName = "x"`: a node matches when ANY of its `defName`
//! child elements has the string value `x`, and the type is the element name of the top-level node.
//! Lookups return nodes in document order without duplicates.

use rimstudio_core::tree::{ArenaDoc, NodeId, NodeKind};
use rimstudio_xpath::DefKey;
use rustc_hash::FxHashMap;

/// The element that names a def.
pub const DEF_NAME_TAG: &str = "defName";

/// A hash index from `defName` text to the top-level nodes of a unified document.
#[derive(Debug, Clone, Default)]
pub struct DefIndex {
    by_name: FxHashMap<String, Vec<NodeId>>,
    names_of: FxHashMap<NodeId, Vec<String>>,
}

impl DefIndex {
    /// Indexes every element child of `defs_root`.
    #[must_use]
    pub fn build(doc: &ArenaDoc, defs_root: NodeId) -> Self {
        let mut index = Self::default();
        for top in doc.children(defs_root) {
            if doc.kind(top) == Some(NodeKind::Element) {
                index.add(doc, top);
            }
        }
        index
    }

    fn add(&mut self, doc: &ArenaDoc, top: NodeId) {
        let mut names: Vec<String> = Vec::new();
        for c in doc.children_named(top, DEF_NAME_TAG) {
            if let Some(v) = doc.string_value(c) {
                let v = v.into_owned();
                if !names.contains(&v) {
                    names.push(v);
                }
            }
        }
        for n in &names {
            self.by_name.entry(n.clone()).or_default().push(top);
        }
        if !names.is_empty() {
            self.names_of.insert(top, names);
        }
    }

    /// Removes a node from the index (call before the node is freed or renamed).
    pub fn forget(&mut self, top: NodeId) {
        if let Some(names) = self.names_of.remove(&top) {
            for n in names {
                if let Some(list) = self.by_name.get_mut(&n) {
                    list.retain(|&x| x != top);
                    if list.is_empty() {
                        self.by_name.remove(&n);
                    }
                }
            }
        }
    }

    /// Re-reads one node after it was added, removed, replaced or changed: the old entries are
    /// forgotten, and the node is indexed again when it is still an element child of `defs_root`.
    pub fn reindex(&mut self, doc: &ArenaDoc, defs_root: NodeId, top: NodeId) {
        self.forget(top);
        if doc.contains(top)
            && doc.kind(top) == Some(NodeKind::Element)
            && doc.parent(top) == Some(defs_root)
        {
            self.add(doc, top);
        }
    }

    /// Number of indexed nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names_of.len()
    }

    /// True when no node has a `defName`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names_of.is_empty()
    }

    /// The top-level nodes named `name` whatever their type, in index order (not document order).
    #[must_use]
    pub fn named(&self, name: &str) -> &[NodeId] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// The nodes of element type `def_type` with a `defName` equal to `name`, in document order.
    #[must_use]
    pub fn lookup(&self, doc: &ArenaDoc, def_type: &str, name: &str) -> Vec<NodeId> {
        self.lookup_keys(
            doc,
            &[DefKey {
                def_type: def_type.to_owned(),
                name: name.to_owned(),
            }],
        )
    }

    /// The nodes matching any of the keys, in document order without duplicates.
    #[must_use]
    pub fn lookup_keys(&self, doc: &ArenaDoc, keys: &[DefKey]) -> Vec<NodeId> {
        let mut found: Vec<NodeId> = Vec::new();
        for key in keys {
            for &id in self.named(&key.name) {
                if doc.name(id) == Some(key.def_type.as_str()) && !found.contains(&id) {
                    found.push(id);
                }
            }
        }
        if found.len() > 1 {
            // `compare_order` needs no numbering of the whole document, which `sort_document_order`
            // builds in O(n) after every structural change.
            found.sort_by(|&a, &b| doc.compare_order(a, b).unwrap_or(std::cmp::Ordering::Equal));
            found.dedup();
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::{NodeBuilder, OriginId};

    fn doc() -> (ArenaDoc, NodeId) {
        let defs = NodeBuilder::new("Defs")
            .child(NodeBuilder::new("RS_A").text_elem("defName", "one"))
            .child(NodeBuilder::new("RS_B").text_elem("defName", "one"))
            .child(
                NodeBuilder::new("RS_A")
                    .text_elem("defName", "two")
                    .text_elem("defName", "one"),
            )
            .build();
        let d = ArenaDoc::from_node(&defs, OriginId::NONE).unwrap();
        let root = d.document_element().unwrap();
        (d, root)
    }

    #[test]
    fn lookup_filters_by_type_and_returns_document_order() {
        let (d, root) = doc();
        let idx = DefIndex::build(&d, root);
        let a = idx.lookup(&d, "RS_A", "one");
        assert_eq!(a.len(), 2);
        assert_eq!(d.compare_order(a[0], a[1]), Some(std::cmp::Ordering::Less));
        assert_eq!(idx.lookup(&d, "RS_B", "one").len(), 1);
        assert!(idx.lookup(&d, "RS_B", "two").is_empty());
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn a_node_with_two_names_matches_either() {
        let (d, root) = doc();
        let idx = DefIndex::build(&d, root);
        assert_eq!(idx.lookup(&d, "RS_A", "two").len(), 1);
    }

    #[test]
    fn reindex_follows_edits() {
        let (mut d, root) = doc();
        let mut idx = DefIndex::build(&d, root);
        let first = idx.lookup(&d, "RS_B", "one")[0];
        let name = d.child_named(first, "defName").unwrap();
        d.set_text_content(name, "renamed", OriginId::NONE).unwrap();
        idx.reindex(&d, root, first);
        assert!(idx.lookup(&d, "RS_B", "one").is_empty());
        assert_eq!(idx.lookup(&d, "RS_B", "renamed"), vec![first]);
        d.remove(first).unwrap();
        idx.reindex(&d, root, first);
        assert!(idx.lookup(&d, "RS_B", "renamed").is_empty());
    }
}
