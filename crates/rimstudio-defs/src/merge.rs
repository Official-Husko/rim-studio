//! Merging every `Defs` file into one document, and the document type that patches edit.
//!
//! The game builds one `<Defs>` root, deep-imports every top-level node of every file into it in load
//! order and remembers which file each node came from. [`merge`] does the same on an
//! [`ArenaDoc`]: every imported node carries an [`OriginId`] naming its mod and file, and nodes
//! created later by patches carry the patch operation instead (and therefore have no mod).
//!
//! A file whose root is not `Defs` is reported but its children still load; a file that failed to
//! parse is skipped. The order of `files` is the load order: the caller (the library layer) lists
//! mods in load order and each mod's files in the game's order (highest priority folder first,
//! ordinal path order inside a folder).

use rimstudio_core::diag::{DiagSink, Severity};
use rimstudio_core::tree::{ArenaDoc, Node, NodeId, NodeKind, OriginId};
use rimstudio_xpath::DefKey;
use rustc_hash::FxHashMap;

use crate::diag_codes::{self, Site};
use crate::index::DefIndex;
use crate::provenance::{ModOrder, OriginKind, OriginTable};
use crate::types::{DefFile, FileContent, Origin, PatchRef};

/// The element name of the unified document root.
pub const DEFS_ROOT_TAG: &str = "Defs";

/// The merged `Defs` document of one load, with provenance and a `defName` index.
///
/// Patches edit it in place ([`crate::apply::apply_patches`]); inheritance and def creation read
/// it afterwards. Edits made through [`UnifiedDoc::arena_mut`] invalidate the index, which is then
/// rebuilt on the next lookup.
#[derive(Debug, Clone)]
pub struct UnifiedDoc {
    arena: ArenaDoc,
    defs_root: NodeId,
    order: ModOrder,
    origins: OriginTable,
    index: DefIndex,
    full_dirty: bool,
    dirty_tops: Vec<NodeId>,
    patched_by: FxHashMap<NodeId, Vec<PatchRef>>,
    diagnostics: DiagSink,
    file_count: u32,
}

impl UnifiedDoc {
    /// An empty unified document (just the `Defs` root) for the given mods.
    #[must_use]
    pub fn new(order: ModOrder) -> Self {
        let mut arena = ArenaDoc::new();
        let root = arena.root();
        let defs_root = match arena.append_element(root, DEFS_ROOT_TAG, OriginId::NONE) {
            Ok(id) => id,
            // The tag is a fixed valid name and the arena is empty, so this cannot happen; the
            // document node is a harmless fallback that keeps the type total.
            Err(_) => root,
        };
        Self {
            arena,
            defs_root,
            order,
            origins: OriginTable::new(),
            index: DefIndex::default(),
            full_dirty: false,
            dirty_tops: Vec::new(),
            patched_by: FxHashMap::default(),
            diagnostics: DiagSink::new(),
            file_count: 0,
        }
    }

    /// The arena holding the document.
    #[must_use]
    pub fn arena(&self) -> &ArenaDoc {
        &self.arena
    }

    /// The arena, for edits the engine cannot track; the `defName` index is rebuilt afterwards.
    pub fn arena_mut(&mut self) -> &mut ArenaDoc {
        self.full_dirty = true;
        &mut self.arena
    }

    /// The arena for edits whose effect on the index the caller reports with
    /// [`UnifiedDoc::note_changed`].
    pub(crate) fn edit(&mut self) -> &mut ArenaDoc {
        &mut self.arena
    }

    /// Forces a full index rebuild before the next lookup.
    pub(crate) fn mark_index_dirty(&mut self) {
        self.full_dirty = true;
    }

    /// The `Defs` root element.
    #[must_use]
    pub fn defs_root(&self) -> NodeId {
        self.defs_root
    }

    /// The mods of the load in load order.
    #[must_use]
    pub fn order(&self) -> &ModOrder {
        &self.order
    }

    /// The meaning of the origin tags stored in the arena.
    #[must_use]
    pub fn origins(&self) -> &OriginTable {
        &self.origins
    }

    pub(crate) fn origins_mut(&mut self) -> &mut OriginTable {
        &mut self.origins
    }

    /// Diagnostics found while merging.
    #[must_use]
    pub fn diagnostics(&self) -> &DiagSink {
        &self.diagnostics
    }

    /// Takes the merge diagnostics, leaving an empty sink.
    pub fn take_diagnostics(&mut self) -> DiagSink {
        std::mem::take(&mut self.diagnostics)
    }

    /// Number of defs files that were read (parsed or not).
    #[must_use]
    pub fn file_count(&self) -> u32 {
        self.file_count
    }

    /// The top-level element nodes in document order.
    #[must_use]
    pub fn top_level(&self) -> Vec<NodeId> {
        self.arena
            .children(self.defs_root)
            .filter(|&c| self.arena.kind(c) == Some(NodeKind::Element))
            .collect()
    }

    /// The top-level node that contains `id` (or is `id`), `None` when `id` is detached or is the
    /// `Defs` root itself.
    #[must_use]
    pub fn top_level_of(&self, id: NodeId) -> Option<NodeId> {
        if !self.arena.contains(id) {
            return None;
        }
        let mut cur = id;
        loop {
            let parent = self.arena.parent(cur)?;
            if parent == self.defs_root {
                return Some(cur);
            }
            cur = parent;
        }
    }

    /// What produced a node: a defs file or a patch operation.
    #[must_use]
    pub fn origin_kind(&self, id: NodeId) -> Option<OriginKind> {
        self.arena.origin(id).and_then(|o| self.origins.get(o))
    }

    /// The mod and file a top-level node was read from; `None` for a node a patch created.
    #[must_use]
    pub fn origin_of(&self, id: NodeId) -> Option<Origin> {
        match self.origin_kind(id)? {
            OriginKind::File { mod_idx, file } => Some(Origin { mod_idx, file }),
            OriginKind::Patch { .. } => None,
        }
    }

    /// The load order position of the mod that owns a node, `None` for patch-created nodes.
    #[must_use]
    pub fn load_order_of(&self, id: NodeId) -> Option<usize> {
        self.origin_of(id)
            .and_then(|o| self.order.load_order(o.mod_idx))
    }

    /// The patch operations that touched a top-level node's subtree, in application order.
    #[must_use]
    pub fn patched_by(&self, top: NodeId) -> &[PatchRef] {
        self.patched_by.get(&top).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn record_patch(&mut self, top: NodeId, pref: PatchRef) {
        self.patched_by.entry(top).or_default().push(pref);
    }

    /// Notes that the children of `container` changed, so the index can follow. Changes below a
    /// top-level node re-index that node; a change of the `Defs` root or the document node (or of a
    /// detached container) forces a full rebuild, so callers report top-level additions and
    /// removals with [`UnifiedDoc::note_top_level`] instead.
    pub(crate) fn note_changed(&mut self, container: NodeId) {
        if self.full_dirty {
            return;
        }
        if container == self.defs_root
            || container == self.arena.root()
            || !self.arena.contains(container)
        {
            self.full_dirty = true;
            return;
        }
        if let Some(top) = self.top_level_of(container) {
            self.dirty_tops.push(top);
        }
    }

    /// Notes that a top-level node was added, removed or replaced (the node may be gone or detached
    /// by the time the index is refreshed).
    pub(crate) fn note_top_level(&mut self, top: NodeId) {
        if !self.full_dirty {
            self.dirty_tops.push(top);
        }
    }

    /// True when index lookups are valid (the `Defs` root is still the document element).
    pub(crate) fn index_usable(&self) -> bool {
        self.arena.contains(self.defs_root) && self.arena.document_element() == Some(self.defs_root)
    }

    fn refresh_index(&mut self) {
        if self.full_dirty {
            self.index = DefIndex::build(&self.arena, self.defs_root);
            self.full_dirty = false;
            self.dirty_tops.clear();
            return;
        }
        let mut dirty = std::mem::take(&mut self.dirty_tops);
        dirty.sort_unstable();
        dirty.dedup();
        for top in dirty {
            self.index.reindex(&self.arena, self.defs_root, top);
        }
    }

    /// The top-level nodes matching any key, in document order without duplicates.
    pub fn lookup_keys(&mut self, keys: &[DefKey]) -> Vec<NodeId> {
        self.refresh_index();
        self.index.lookup_keys(&self.arena, keys)
    }

    /// The top-level nodes of element type `def_type` whose `defName` equals `name`.
    pub fn find_defs(&mut self, def_type: &str, name: &str) -> Vec<NodeId> {
        self.refresh_index();
        self.index.lookup(&self.arena, def_type, name)
    }

    /// The unified document as an owned tree (the `Defs` element), for export and tests.
    #[must_use]
    pub fn to_node(&self) -> Option<Node> {
        self.arena.to_node(self.defs_root)
    }
}

/// Merges every defs file into one unified document.
///
/// `files` must be in load order. Diagnostics (`defs.bad-root`, `defs.xml-parse-error`,
/// `defs.unknown-parse-failure`) are available through [`UnifiedDoc::diagnostics`].
#[must_use]
pub fn merge(order: &ModOrder, files: &[DefFile]) -> UnifiedDoc {
    let mut doc = UnifiedDoc::new(order.clone());
    for file in files {
        doc.file_count = doc.file_count.saturating_add(1);
        merge_file(&mut doc, file);
    }
    doc
}

fn merge_file(doc: &mut UnifiedDoc, file: &DefFile) {
    let site = Site::new(Some(file.mod_idx), Some(file.file));
    match &file.content {
        FileContent::Failed { message } => {
            doc.diagnostics.push(site.diag(
                diag_codes::XML_PARSE_ERROR,
                Severity::Warning,
                format!("Exception reading {} as XML: {message}", file.rel_path),
            ));
            doc.diagnostics.push(site.diag(
                diag_codes::UNKNOWN_PARSE_FAILURE,
                Severity::Error,
                format!("{}: unknown parse failure", file.rel_path),
            ));
        }
        FileContent::Parsed { root } => {
            if root.tag != DEFS_ROOT_TAG {
                doc.diagnostics.push(site.diag(
                    diag_codes::BAD_ROOT,
                    Severity::Error,
                    format!(
                        "{}: root element named {}; should be named {DEFS_ROOT_TAG}",
                        file.rel_path, root.tag
                    ),
                ));
            }
            let origin = doc.origins.push(OriginKind::File {
                mod_idx: file.mod_idx,
                file: file.file,
            });
            let parent = doc.defs_root;
            for child in root.elements() {
                if let Err(e) = doc.arena.append_node(parent, child, origin) {
                    doc.diagnostics.push(site.diag(
                        diag_codes::IMPORT_FAILED,
                        Severity::Error,
                        format!("{}: a {} node was skipped: {e}", file.rel_path, child.tag),
                    ));
                }
            }
        }
    }
    doc.full_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::ModEntry;
    use rimstudio_core::ids::{FileId, ModIdx};
    use rimstudio_core::tree::NodeBuilder;

    fn order() -> ModOrder {
        ModOrder::new(vec![
            ModEntry::new(ModIdx(0), "ludeon.rimworld", "Core"),
            ModEntry::new(ModIdx(1), "rs.a", "RS A"),
        ])
    }

    fn defs(names: &[&str]) -> Node {
        let mut b = NodeBuilder::new("Defs");
        for n in names {
            b = b.child(NodeBuilder::new("RS_Def").text_elem("defName", *n));
        }
        b.build()
    }

    fn file(m: u32, f: u32, root: Node) -> DefFile {
        DefFile {
            mod_idx: ModIdx(m),
            file: FileId(f),
            rel_path: format!("Defs/f{f}.xml"),
            content: FileContent::parsed(root),
        }
    }

    #[test]
    fn files_merge_in_the_given_order_with_origins() {
        let files = vec![file(0, 0, defs(&["a", "b"])), file(1, 1, defs(&["c"]))];
        let mut doc = merge(&order(), &files);
        assert_eq!(doc.file_count(), 2);
        let tops = doc.top_level();
        assert_eq!(tops.len(), 3);
        let first = doc.origin_of(tops[0]);
        let last = doc.origin_of(tops[2]);
        assert_eq!(
            first.map(|o| (o.mod_idx, o.file)),
            Some((ModIdx(0), FileId(0)))
        );
        assert_eq!(
            last.map(|o| (o.mod_idx, o.file)),
            Some((ModIdx(1), FileId(1)))
        );
        assert_eq!(doc.load_order_of(tops[2]), Some(1));
        assert_eq!(doc.find_defs("RS_Def", "c"), vec![tops[2]]);
        assert!(doc.diagnostics().is_empty());
    }

    #[test]
    fn a_wrong_root_is_reported_but_children_load() {
        let bad = NodeBuilder::new("Other")
            .child(NodeBuilder::new("RS_Def").text_elem("defName", "x"))
            .build();
        let doc = merge(&order(), &[file(0, 0, bad)]);
        assert_eq!(doc.top_level().len(), 1);
        assert_eq!(doc.diagnostics().count(&diag_codes::BAD_ROOT), 1);
    }

    #[test]
    fn a_failed_file_is_skipped_with_two_diagnostics() {
        let f = DefFile {
            mod_idx: ModIdx(0),
            file: FileId(4),
            rel_path: "Defs/broken.xml".into(),
            content: FileContent::failed("unexpected end"),
        };
        let doc = merge(&order(), &[f, file(0, 5, defs(&["ok"]))]);
        assert_eq!(doc.top_level().len(), 1);
        assert_eq!(doc.diagnostics().count(&diag_codes::XML_PARSE_ERROR), 1);
        assert_eq!(
            doc.diagnostics().count(&diag_codes::UNKNOWN_PARSE_FAILURE),
            1
        );
        assert_eq!(doc.file_count(), 2);
    }

    #[test]
    fn top_level_of_walks_up_and_rejects_detached_nodes() {
        let files = vec![file(0, 0, defs(&["a"]))];
        let doc = merge(&order(), &files);
        let top = doc.top_level()[0];
        let name = doc.arena().child_named(top, "defName");
        assert_eq!(name.and_then(|n| doc.top_level_of(n)), Some(top));
        assert_eq!(doc.top_level_of(top), Some(top));
        assert_eq!(doc.top_level_of(doc.defs_root()), None);
    }
}
