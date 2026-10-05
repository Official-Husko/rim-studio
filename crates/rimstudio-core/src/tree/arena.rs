//! The mutable arena document used by the XPath, patch and inheritance engines.
//!
//! An [`ArenaDoc`] stores every node in one `Vec` of slots linked by parent, first child, last
//! child, previous sibling and next sibling indices, so append, insert, detach and replace are
//! O(1) and navigation never allocates. Handles are [`NodeId`]s made of a slot index and a
//! generation: removing a node bumps its slot generation, so a stale handle is detected instead
//! of silently addressing a recycled slot. Element and attribute names are interned per arena
//! ([`Sym`]), which makes tag comparison a `u32` comparison.
//!
//! Nodes are `Document` (exactly one, the root, never removable), `Element` and `Text`. Nodes can
//! be created detached (own tree, no parent) and linked in later; an operation that would give a
//! node two parents or create a cycle is rejected with a [`TreeError`] before anything changes.
//!
//! Document order: [`ArenaDoc::compare_order`] is exact for any two live nodes. After structural
//! changes it falls back to a lowest-common-ancestor walk whose cost is O(depth plus the smaller
//! sibling distance); [`ArenaDoc::prepare_order`] and [`ArenaDoc::sort_document_order`] build a
//! cached numbering of the attached tree (O(n) once, invalidated by every link or unlink), after
//! which comparisons are O(1). Attribute-only edits and text edits do not invalidate the cache.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use super::MAX_DEPTH;
use super::error::TreeError;
use super::names::{check_name, first_invalid_char};
use super::node::{Child, Node};

/// Index value meaning "no node" inside the slot links.
const NIL: u32 = u32::MAX;

/// An opaque tag saying where a node came from; the meaning (mod index, file, patch operation)
/// belongs to the caller. Nodes created without a source use [`OriginId::NONE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OriginId(pub u32);

impl OriginId {
    /// "No origin recorded".
    pub const NONE: OriginId = OriginId(u32::MAX);
}

impl Default for OriginId {
    fn default() -> Self {
        Self::NONE
    }
}

/// A handle to a node of one [`ArenaDoc`].
///
/// Copyable and cheap. A handle stays valid across every mutation of the arena until its node
/// (or an ancestor) is removed; afterwards every operation reports it as stale. The derived
/// ordering is by slot and generation and is NOT document order, use
/// [`ArenaDoc::compare_order`] for that. A handle used with a different arena is
/// indistinguishable from a stale one only if the slot and generation happen to coincide, so do
/// not mix arenas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    /// The slot index, dense and below [`ArenaDoc::slot_count`]; use it to key side tables and
    /// bit sets. Two live nodes never share a slot.
    #[must_use]
    pub fn slot(self) -> usize {
        self.index as usize
    }

    /// The generation of the slot this handle was issued for.
    #[must_use]
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// An interned element or attribute name of one arena. Equal names have equal symbols within an
/// arena; symbols from different arenas are unrelated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sym(u32);

/// The kind of an arena node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// The single root of the arena; parent of the document element.
    Document,
    /// An element with a name, attributes and children.
    Element,
    /// A text run.
    Text,
}

#[derive(Debug, Clone, Default)]
struct Interner {
    names: Vec<Arc<str>>,
    map: FxHashMap<Arc<str>, u32>,
}

impl Interner {
    fn get(&self, name: &str) -> Option<u32> {
        self.map.get(name).copied()
    }

    fn resolve(&self, sym: u32) -> &str {
        self.names.get(sym as usize).map_or("", |s| s)
    }

    fn insert(&mut self, name: &str) -> Result<u32, TreeError> {
        let id = u32::try_from(self.names.len()).map_err(|_| TreeError::Capacity)?;
        let shared: Arc<str> = Arc::from(name);
        self.names.push(Arc::clone(&shared));
        self.map.insert(shared, id);
        Ok(id)
    }

    fn intern_trusted(&mut self, name: &str) -> Result<u32, TreeError> {
        match self.get(name) {
            Some(s) => Ok(s),
            None => self.insert(name),
        }
    }

    fn intern_checked(&mut self, name: &str, path: &str) -> Result<u32, TreeError> {
        if let Some(s) = self.get(name) {
            return Ok(s);
        }
        check_name(name).map_err(|reason| TreeError::InvalidName {
            path: path.to_owned(),
            name: name.to_owned(),
            reason,
        })?;
        self.insert(name)
    }
}

#[derive(Debug, Clone)]
enum Data {
    None,
    Attrs(Vec<(u32, String)>),
    Text(String),
}

#[derive(Debug, Clone)]
struct Slot {
    generation: u32,
    live: bool,
    kind: NodeKind,
    origin: u32,
    parent: u32,
    first: u32,
    last: u32,
    prev: u32,
    next: u32,
    name: u32,
    data: Data,
}

impl Slot {
    fn blank(generation: u32) -> Self {
        Self {
            generation,
            live: false,
            kind: NodeKind::Text,
            origin: NIL,
            parent: NIL,
            first: NIL,
            last: NIL,
            prev: NIL,
            next: NIL,
            name: NIL,
            data: Data::None,
        }
    }
}

/// A mutable document stored in an arena. See the [module documentation](self).
#[derive(Debug, Clone)]
pub struct ArenaDoc {
    slots: Vec<Slot>,
    free: Vec<u32>,
    names: Interner,
    live: usize,
    order: OnceLock<Vec<u32>>,
}

impl Default for ArenaDoc {
    fn default() -> Self {
        Self::new()
    }
}

/// One node of a snapshot used by the clone routines.
struct Rec {
    depth: usize,
    kind: NodeKind,
    origin: u32,
    name: u32,
    data: Data,
}

impl ArenaDoc {
    // ------------------------------------------------------------------ construction

    /// An arena holding only the document node.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// An arena with room for about `nodes` nodes.
    #[must_use]
    pub fn with_capacity(nodes: usize) -> Self {
        let mut slots = Vec::with_capacity(nodes.saturating_add(1));
        let mut doc = Slot::blank(0);
        doc.live = true;
        doc.kind = NodeKind::Document;
        slots.push(doc);
        Self {
            slots,
            free: Vec::new(),
            names: Interner::default(),
            live: 1,
            order: OnceLock::new(),
        }
    }

    /// A document whose document element is an import of `root`.
    ///
    /// # Errors
    /// Invalid names, characters or depth in `root`.
    pub fn from_node(root: &Node, origin: OriginId) -> Result<Self, TreeError> {
        let mut doc = Self::new();
        let top = doc.root();
        doc.append_node(top, root, origin)?;
        Ok(doc)
    }

    /// The document node (never stale, never removable).
    #[must_use]
    pub fn root(&self) -> NodeId {
        NodeId {
            index: 0,
            generation: 0,
        }
    }

    /// The first element child of the document node.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(self.root())
            .find(|&c| self.kind(c) == Some(NodeKind::Element))
    }

    /// Number of live nodes including the document node.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live
    }

    /// True when the arena holds only the document node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live <= 1
    }

    /// Upper bound (exclusive) of [`NodeId::slot`] for every live node, for sizing side tables.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    // ------------------------------------------------------------------ internal access

    fn get(&self, id: NodeId) -> Option<&Slot> {
        self.slots
            .get(id.index as usize)
            .filter(|s| s.live && s.generation == id.generation)
    }

    fn idx(&self, id: NodeId) -> Result<u32, TreeError> {
        self.get(id).map(|_| id.index).ok_or(TreeError::StaleId)
    }

    fn sl(&self, i: u32) -> &Slot {
        &self.slots[i as usize]
    }

    fn sl_mut(&mut self, i: u32) -> &mut Slot {
        &mut self.slots[i as usize]
    }

    fn id_of(&self, i: u32) -> Option<NodeId> {
        if i == NIL {
            return None;
        }
        self.slots.get(i as usize).map(|s| NodeId {
            index: i,
            generation: s.generation,
        })
    }

    fn id_at(&self, i: u32) -> NodeId {
        NodeId {
            index: i,
            generation: self.sl(i).generation,
        }
    }

    fn element_index(&self, id: NodeId, op: &'static str) -> Result<u32, TreeError> {
        let s = self.get(id).ok_or(TreeError::StaleId)?;
        if s.kind == NodeKind::Element {
            Ok(id.index)
        } else {
            Err(TreeError::WrongKind { kind: s.kind, op })
        }
    }

    fn container_index(&self, id: NodeId, op: &'static str) -> Result<u32, TreeError> {
        let s = self.get(id).ok_or(TreeError::StaleId)?;
        if s.kind == NodeKind::Text {
            Err(TreeError::WrongKind { kind: s.kind, op })
        } else {
            Ok(id.index)
        }
    }

    // ------------------------------------------------------------------ queries

    /// True when the handle refers to a live node of this arena.
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }

    /// The kind of a node, or `None` for a stale handle.
    #[must_use]
    pub fn kind(&self, id: NodeId) -> Option<NodeKind> {
        self.get(id).map(|s| s.kind)
    }

    /// The parent, or `None` for the document node, a detached node or a stale handle.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.id_of(self.get(id)?.parent)
    }

    /// The first child.
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.id_of(self.get(id)?.first)
    }

    /// The last child.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.id_of(self.get(id)?.last)
    }

    /// The previous sibling.
    #[must_use]
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.id_of(self.get(id)?.prev)
    }

    /// The next sibling.
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.id_of(self.get(id)?.next)
    }

    /// True when the node has at least one child.
    #[must_use]
    pub fn has_children(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|s| s.first != NIL)
    }

    /// Number of children, O(children).
    #[must_use]
    pub fn child_count(&self, id: NodeId) -> usize {
        self.children(id).count()
    }

    /// True when the node is reachable from the document node.
    #[must_use]
    pub fn is_attached(&self, id: NodeId) -> bool {
        let Some(mut i) = self.get(id).map(|_| id.index) else {
            return false;
        };
        while self.sl(i).parent != NIL {
            i = self.sl(i).parent;
        }
        i == 0
    }

    /// The element name, or `None` for text, the document node and stale handles.
    #[must_use]
    pub fn name(&self, id: NodeId) -> Option<&str> {
        let s = self.get(id)?;
        (s.kind == NodeKind::Element).then(|| self.names.resolve(s.name))
    }

    /// The interned element name.
    #[must_use]
    pub fn name_sym(&self, id: NodeId) -> Option<Sym> {
        let s = self.get(id)?;
        (s.kind == NodeKind::Element).then_some(Sym(s.name))
    }

    /// The symbol of `name` when some node of this arena has used it. `None` proves that no
    /// element or attribute currently or previously carried the name.
    #[must_use]
    pub fn sym(&self, name: &str) -> Option<Sym> {
        self.names.get(name).map(Sym)
    }

    /// The text of a symbol.
    #[must_use]
    pub fn sym_name(&self, sym: Sym) -> Option<&str> {
        self.names.names.get(sym.0 as usize).map(|s| &**s)
    }

    /// The content of a text node.
    #[must_use]
    pub fn text(&self, id: NodeId) -> Option<&str> {
        match &self.get(id)?.data {
            Data::Text(t) => Some(t),
            _ => None,
        }
    }

    /// The origin recorded for the node.
    #[must_use]
    pub fn origin(&self, id: NodeId) -> Option<OriginId> {
        self.get(id).map(|s| OriginId(s.origin))
    }

    /// The value of an attribute.
    #[must_use]
    pub fn attr(&self, id: NodeId, name: &str) -> Option<&str> {
        let sym = self.sym(name)?;
        self.attr_by_sym(id, sym)
    }

    /// The value of an attribute by interned name.
    #[must_use]
    pub fn attr_by_sym(&self, id: NodeId, sym: Sym) -> Option<&str> {
        match &self.get(id)?.data {
            Data::Attrs(a) => a.iter().find(|(k, _)| *k == sym.0).map(|(_, v)| v.as_str()),
            _ => None,
        }
    }

    /// Number of attributes (0 for non elements).
    #[must_use]
    pub fn attr_count(&self, id: NodeId) -> usize {
        match self.get(id).map(|s| &s.data) {
            Some(Data::Attrs(a)) => a.len(),
            _ => 0,
        }
    }

    /// The attribute at a position, as `(name, value)`; positions follow attribute order.
    #[must_use]
    pub fn attr_at(&self, id: NodeId, position: usize) -> Option<(&str, &str)> {
        match &self.get(id)?.data {
            Data::Attrs(a) => a
                .get(position)
                .map(|(k, v)| (self.names.resolve(*k), v.as_str())),
            _ => None,
        }
    }

    /// The attributes in order, as `(name, value)` pairs. Empty for non elements.
    pub fn attrs(&self, id: NodeId) -> impl Iterator<Item = (&str, &str)> {
        let list: &[(u32, String)] = match self.get(id).map(|s| &s.data) {
            Some(Data::Attrs(a)) => a,
            _ => &[],
        };
        list.iter()
            .map(|(k, v)| (self.names.resolve(*k), v.as_str()))
    }

    /// The XPath string value: a text node's content, or the concatenation of all descendant
    /// text. Borrowed when it is a single run (or empty), so the common `<defName>X</defName>`
    /// lookup does not allocate.
    #[must_use]
    pub fn string_value(&self, id: NodeId) -> Option<Cow<'_, str>> {
        let s = self.get(id)?;
        if let Data::Text(t) = &s.data {
            return Some(Cow::Borrowed(t));
        }
        let mut first: Option<&str> = None;
        let mut owned: Option<String> = None;
        let mut cur = self.sl(id.index).first;
        let root = id.index;
        while cur != NIL {
            if let Data::Text(t) = &self.sl(cur).data {
                if let Some(o) = owned.as_mut() {
                    o.push_str(t);
                } else if let Some(f) = first {
                    let mut o = String::with_capacity(f.len() + t.len());
                    o.push_str(f);
                    o.push_str(t);
                    owned = Some(o);
                } else {
                    first = Some(t);
                }
            }
            cur = self.advance(root, cur);
        }
        Some(match (owned, first) {
            (Some(o), _) => Cow::Owned(o),
            (None, Some(f)) => Cow::Borrowed(f),
            (None, None) => Cow::Borrowed(""),
        })
    }

    /// Appends the string value to `out`; returns false for a stale handle.
    pub fn string_value_into(&self, id: NodeId, out: &mut String) -> bool {
        match self.string_value(id) {
            Some(v) => {
                out.push_str(&v);
                true
            }
            None => false,
        }
    }

    // ------------------------------------------------------------------ creation

    fn alloc(
        &mut self,
        kind: NodeKind,
        name: u32,
        origin: u32,
        data: Data,
    ) -> Result<u32, TreeError> {
        let fill = |s: &mut Slot| {
            s.live = true;
            s.kind = kind;
            s.origin = origin;
            s.name = name;
            s.data = data;
        };
        if let Some(i) = self.free.pop() {
            fill(self.sl_mut(i));
            self.live += 1;
            return Ok(i);
        }
        if self.slots.len() >= NIL as usize {
            return Err(TreeError::Capacity);
        }
        let i = u32::try_from(self.slots.len()).map_err(|_| TreeError::Capacity)?;
        let mut s = Slot::blank(0);
        fill(&mut s);
        self.slots.push(s);
        self.live += 1;
        Ok(i)
    }

    /// Creates a detached element.
    ///
    /// # Errors
    /// An invalid XML name, or a full arena.
    pub fn create_element(&mut self, name: &str, origin: OriginId) -> Result<NodeId, TreeError> {
        let sym = self.names.intern_checked(name, name)?;
        let i = self.alloc(NodeKind::Element, sym, origin.0, Data::Attrs(Vec::new()))?;
        Ok(self.id_at(i))
    }

    /// Creates a detached text node.
    ///
    /// # Errors
    /// A character XML cannot represent, or a full arena.
    pub fn create_text(&mut self, text: &str, origin: OriginId) -> Result<NodeId, TreeError> {
        if let Some(ch) = first_invalid_char(text) {
            return Err(TreeError::InvalidChar {
                path: String::new(),
                ch: u32::from(ch),
            });
        }
        let i = self.alloc(NodeKind::Text, NIL, origin.0, Data::Text(text.to_owned()))?;
        Ok(self.id_at(i))
    }

    /// Creates an element and appends it to `parent` in one step.
    ///
    /// # Errors
    /// Stale or text parent, invalid name, full arena.
    pub fn append_element(
        &mut self,
        parent: NodeId,
        name: &str,
        origin: OriginId,
    ) -> Result<NodeId, TreeError> {
        let p = self.container_index(parent, "append_element")?;
        let id = self.create_element(name, origin)?;
        self.link_last(p, id.index);
        Ok(id)
    }

    /// Creates a text node and appends it to `parent` in one step.
    ///
    /// # Errors
    /// Stale or text parent, invalid character, full arena.
    pub fn append_text(
        &mut self,
        parent: NodeId,
        text: &str,
        origin: OriginId,
    ) -> Result<NodeId, TreeError> {
        let p = self.container_index(parent, "append_text")?;
        let id = self.create_text(text, origin)?;
        self.link_last(p, id.index);
        Ok(id)
    }

    // ------------------------------------------------------------------ linking

    fn link_last(&mut self, p: u32, c: u32) {
        let last = self.sl(p).last;
        {
            let cs = self.sl_mut(c);
            cs.parent = p;
            cs.prev = last;
            cs.next = NIL;
        }
        if last == NIL {
            self.sl_mut(p).first = c;
        } else {
            self.sl_mut(last).next = c;
        }
        self.sl_mut(p).last = c;
        self.order.take();
    }

    fn link_first(&mut self, p: u32, c: u32) {
        let first = self.sl(p).first;
        {
            let cs = self.sl_mut(c);
            cs.parent = p;
            cs.prev = NIL;
            cs.next = first;
        }
        if first == NIL {
            self.sl_mut(p).last = c;
        } else {
            self.sl_mut(first).prev = c;
        }
        self.sl_mut(p).first = c;
        self.order.take();
    }

    fn link_before(&mut self, r: u32, c: u32) {
        let (p, prev) = (self.sl(r).parent, self.sl(r).prev);
        {
            let cs = self.sl_mut(c);
            cs.parent = p;
            cs.prev = prev;
            cs.next = r;
        }
        self.sl_mut(r).prev = c;
        if prev == NIL {
            self.sl_mut(p).first = c;
        } else {
            self.sl_mut(prev).next = c;
        }
        self.order.take();
    }

    fn link_after(&mut self, r: u32, c: u32) {
        let (p, next) = (self.sl(r).parent, self.sl(r).next);
        {
            let cs = self.sl_mut(c);
            cs.parent = p;
            cs.prev = r;
            cs.next = next;
        }
        self.sl_mut(r).next = c;
        if next == NIL {
            self.sl_mut(p).last = c;
        } else {
            self.sl_mut(next).prev = c;
        }
        self.order.take();
    }

    fn unlink(&mut self, c: u32) {
        let (p, prev, next) = {
            let cs = self.sl(c);
            (cs.parent, cs.prev, cs.next)
        };
        if prev == NIL {
            if p != NIL {
                self.sl_mut(p).first = next;
            }
        } else {
            self.sl_mut(prev).next = next;
        }
        if next == NIL {
            if p != NIL {
                self.sl_mut(p).last = prev;
            }
        } else {
            self.sl_mut(next).prev = prev;
        }
        let cs = self.sl_mut(c);
        cs.parent = NIL;
        cs.prev = NIL;
        cs.next = NIL;
        self.order.take();
    }

    fn root_of(&self, mut i: u32) -> u32 {
        while self.sl(i).parent != NIL {
            i = self.sl(i).parent;
        }
        i
    }

    /// Validates a batch of nodes about to become children of `parent` and returns their slots.
    /// Nothing is modified, so a failure leaves the arena untouched.
    fn check_batch(&self, parent: u32, nodes: &[NodeId]) -> Result<Vec<u32>, TreeError> {
        let mut out = Vec::with_capacity(nodes.len());
        let mut needs_root = false;
        for &n in nodes {
            let c = self.idx(n)?;
            let cs = self.sl(c);
            if cs.kind == NodeKind::Document {
                return Err(TreeError::DocumentRoot);
            }
            if c == parent {
                return Err(TreeError::Cycle);
            }
            if cs.parent != NIL {
                return Err(TreeError::AlreadyAttached);
            }
            needs_root |= cs.first != NIL;
            out.push(c);
        }
        if out.len() > 1 {
            let mut sorted = out.clone();
            sorted.sort_unstable();
            if sorted.windows(2).any(|w| w.first() == w.get(1)) {
                return Err(TreeError::AlreadyAttached);
            }
        }
        if needs_root {
            let root = self.root_of(parent);
            if out.contains(&root) {
                return Err(TreeError::Cycle);
            }
        }
        Ok(out)
    }

    /// Appends detached nodes as the last children of `parent`, in order.
    ///
    /// # Errors
    /// Stale handles, a text parent, a node that already has a parent, duplicates in the list,
    /// or a cycle. On error nothing was changed.
    pub fn append_children(&mut self, parent: NodeId, nodes: &[NodeId]) -> Result<(), TreeError> {
        let p = self.container_index(parent, "append_children")?;
        for c in self.check_batch(p, nodes)? {
            self.link_last(p, c);
        }
        Ok(())
    }

    /// Appends one detached node as the last child of `parent`. See [`ArenaDoc::append_children`].
    ///
    /// # Errors
    /// See [`ArenaDoc::append_children`].
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), TreeError> {
        self.append_children(parent, &[child])
    }

    /// Inserts detached nodes before the first child of `parent`; the nodes keep the list order.
    ///
    /// # Errors
    /// See [`ArenaDoc::append_children`].
    pub fn prepend_children(&mut self, parent: NodeId, nodes: &[NodeId]) -> Result<(), TreeError> {
        let p = self.container_index(parent, "prepend_children")?;
        let list = self.check_batch(p, nodes)?;
        for c in list.into_iter().rev() {
            self.link_first(p, c);
        }
        Ok(())
    }

    /// Inserts one detached node before the first child of `parent`.
    ///
    /// # Errors
    /// See [`ArenaDoc::append_children`].
    pub fn prepend_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), TreeError> {
        self.prepend_children(parent, &[child])
    }

    fn reference_parent(&self, reference: NodeId) -> Result<(u32, u32), TreeError> {
        let r = self.idx(reference)?;
        let s = self.sl(r);
        if s.kind == NodeKind::Document {
            return Err(TreeError::DocumentRoot);
        }
        if s.parent == NIL {
            return Err(TreeError::NotAttached);
        }
        Ok((r, s.parent))
    }

    /// Inserts detached nodes directly before `reference`, keeping the list order.
    ///
    /// # Errors
    /// [`TreeError::NotAttached`] when `reference` has no parent, otherwise as
    /// [`ArenaDoc::append_children`].
    pub fn insert_all_before(
        &mut self,
        reference: NodeId,
        nodes: &[NodeId],
    ) -> Result<(), TreeError> {
        let (r, p) = self.reference_parent(reference)?;
        for c in self.check_batch(p, nodes)? {
            self.link_before(r, c);
        }
        Ok(())
    }

    /// Inserts one detached node directly before `reference`.
    ///
    /// # Errors
    /// See [`ArenaDoc::insert_all_before`].
    pub fn insert_before(&mut self, reference: NodeId, node: NodeId) -> Result<(), TreeError> {
        self.insert_all_before(reference, &[node])
    }

    /// Inserts detached nodes directly after `reference`, keeping the list order.
    ///
    /// # Errors
    /// See [`ArenaDoc::insert_all_before`].
    pub fn insert_all_after(
        &mut self,
        reference: NodeId,
        nodes: &[NodeId],
    ) -> Result<(), TreeError> {
        let (r, p) = self.reference_parent(reference)?;
        let mut cursor = r;
        for c in self.check_batch(p, nodes)? {
            self.link_after(cursor, c);
            cursor = c;
        }
        Ok(())
    }

    /// Inserts one detached node directly after `reference`.
    ///
    /// # Errors
    /// See [`ArenaDoc::insert_all_before`].
    pub fn insert_after(&mut self, reference: NodeId, node: NodeId) -> Result<(), TreeError> {
        self.insert_all_after(reference, &[node])
    }

    /// Replaces `id` by the detached `nodes` (in list order, possibly none). `id` stays alive as
    /// a detached tree; call [`ArenaDoc::remove`] afterwards to free it.
    ///
    /// # Errors
    /// [`TreeError::NotAttached`] when `id` has no parent, otherwise as
    /// [`ArenaDoc::append_children`]. On error nothing was changed.
    pub fn replace_with(&mut self, id: NodeId, nodes: &[NodeId]) -> Result<(), TreeError> {
        let (r, p) = self.reference_parent(id)?;
        for c in self.check_batch(p, nodes)? {
            self.link_before(r, c);
        }
        self.unlink(r);
        Ok(())
    }

    /// Unlinks a node (with its subtree) from its parent; it stays alive as a detached tree.
    /// Detaching an already detached node is a no-op.
    ///
    /// # Errors
    /// Stale handle, or the document node.
    pub fn detach(&mut self, id: NodeId) -> Result<(), TreeError> {
        let i = self.idx(id)?;
        if self.sl(i).kind == NodeKind::Document {
            return Err(TreeError::DocumentRoot);
        }
        if self.sl(i).parent != NIL {
            self.unlink(i);
        }
        Ok(())
    }

    /// Detaches a node and frees its whole subtree. Every handle into the subtree becomes stale.
    ///
    /// # Errors
    /// Stale handle, or the document node.
    pub fn remove(&mut self, id: NodeId) -> Result<(), TreeError> {
        self.detach(id)?;
        self.free_subtree(id.index);
        Ok(())
    }

    /// Frees every child of a node (the node stays).
    ///
    /// # Errors
    /// Stale handle or a text node.
    pub fn clear_children(&mut self, id: NodeId) -> Result<(), TreeError> {
        let i = self.container_index(id, "clear_children")?;
        loop {
            let c = self.sl(i).first;
            if c == NIL {
                return Ok(());
            }
            self.unlink(c);
            self.free_subtree(c);
        }
    }

    fn free_subtree(&mut self, root: u32) {
        let mut all = Vec::new();
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            all.push(i);
            let mut c = self.sl(i).first;
            while c != NIL {
                stack.push(c);
                c = self.sl(c).next;
            }
        }
        for i in all {
            let s = self.sl_mut(i);
            s.live = false;
            s.data = Data::None;
            s.parent = NIL;
            s.first = NIL;
            s.last = NIL;
            s.prev = NIL;
            s.next = NIL;
            s.name = NIL;
            // A slot whose generation counter is exhausted is retired instead of recycled, so a
            // handle can never become valid again.
            if s.generation < u32::MAX {
                s.generation += 1;
                self.free.push(i);
            }
            self.live = self.live.saturating_sub(1);
        }
        self.order.take();
    }

    // ------------------------------------------------------------------ attributes, text, names

    /// Sets an attribute: replaces the value in place or appends the attribute. Returns the
    /// previous value.
    ///
    /// # Errors
    /// Stale handle, non element, invalid attribute name or value character.
    pub fn set_attr(
        &mut self,
        id: NodeId,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, TreeError> {
        let i = self.element_index(id, "set_attr")?;
        if let Some(ch) = first_invalid_char(value) {
            return Err(TreeError::InvalidChar {
                path: name.to_owned(),
                ch: u32::from(ch),
            });
        }
        let sym = self.names.intern_checked(name, name)?;
        if let Data::Attrs(a) = &mut self.sl_mut(i).data {
            if let Some((_, v)) = a.iter_mut().find(|(k, _)| *k == sym) {
                return Ok(Some(std::mem::replace(v, value.to_owned())));
            }
            a.push((sym, value.to_owned()));
        }
        Ok(None)
    }

    /// Removes an attribute and returns its value.
    ///
    /// # Errors
    /// Stale handle or non element.
    pub fn remove_attr(&mut self, id: NodeId, name: &str) -> Result<Option<String>, TreeError> {
        let i = self.element_index(id, "remove_attr")?;
        let Some(sym) = self.names.get(name) else {
            return Ok(None);
        };
        if let Data::Attrs(a) = &mut self.sl_mut(i).data
            && let Some(pos) = a.iter().position(|(k, _)| *k == sym)
        {
            return Ok(Some(a.remove(pos).1));
        }
        Ok(None)
    }

    /// Removes every attribute.
    ///
    /// # Errors
    /// Stale handle or non element.
    pub fn clear_attrs(&mut self, id: NodeId) -> Result<(), TreeError> {
        let i = self.element_index(id, "clear_attrs")?;
        if let Data::Attrs(a) = &mut self.sl_mut(i).data {
            a.clear();
        }
        Ok(())
    }

    /// Renames an element.
    ///
    /// # Errors
    /// Stale handle, non element or invalid name.
    pub fn set_name(&mut self, id: NodeId, name: &str) -> Result<(), TreeError> {
        let i = self.element_index(id, "set_name")?;
        let sym = self.names.intern_checked(name, name)?;
        self.sl_mut(i).name = sym;
        Ok(())
    }

    /// Replaces the content of a text node.
    ///
    /// # Errors
    /// Stale handle, non text node or invalid character.
    pub fn set_text(&mut self, id: NodeId, text: &str) -> Result<(), TreeError> {
        let s = self.get(id).ok_or(TreeError::StaleId)?;
        if s.kind != NodeKind::Text {
            return Err(TreeError::WrongKind {
                kind: s.kind,
                op: "set_text",
            });
        }
        if let Some(ch) = first_invalid_char(text) {
            return Err(TreeError::InvalidChar {
                path: String::new(),
                ch: u32::from(ch),
            });
        }
        if let Data::Text(t) = &mut self.sl_mut(id.index).data {
            t.clear();
            t.push_str(text);
        }
        Ok(())
    }

    /// Replaces all children of an element or the document by a single text node (the DOM
    /// `InnerText` setter); an empty string leaves no child.
    ///
    /// # Errors
    /// Stale handle, text node or invalid character.
    pub fn set_text_content(
        &mut self,
        id: NodeId,
        text: &str,
        origin: OriginId,
    ) -> Result<(), TreeError> {
        let i = self.container_index(id, "set_text_content")?;
        if let Some(ch) = first_invalid_char(text) {
            return Err(TreeError::InvalidChar {
                path: String::new(),
                ch: u32::from(ch),
            });
        }
        self.clear_children(id)?;
        if !text.is_empty() {
            let t = self.create_text(text, origin)?;
            self.link_last(i, t.index);
        }
        Ok(())
    }

    /// Records a new origin for one node.
    ///
    /// # Errors
    /// Stale handle.
    pub fn set_origin(&mut self, id: NodeId, origin: OriginId) -> Result<(), TreeError> {
        let i = self.idx(id)?;
        self.sl_mut(i).origin = origin.0;
        Ok(())
    }

    /// Records a new origin for a node and everything below it.
    ///
    /// # Errors
    /// Stale handle.
    pub fn set_origin_subtree(&mut self, id: NodeId, origin: OriginId) -> Result<(), TreeError> {
        let root = self.idx(id)?;
        let mut cur = root;
        while cur != NIL {
            self.sl_mut(cur).origin = origin.0;
            cur = self.advance(root, cur);
        }
        Ok(())
    }

    // ------------------------------------------------------------------ iteration

    /// Slot following `cur` in pre order, bounded by the subtree of `root` (`NIL` for unbounded).
    fn advance(&self, root: u32, cur: u32) -> u32 {
        let s = self.sl(cur);
        if s.first != NIL {
            return s.first;
        }
        let mut c = cur;
        loop {
            if c == root {
                return NIL;
            }
            let sc = self.sl(c);
            if sc.next != NIL {
                return sc.next;
            }
            c = sc.parent;
            if c == NIL {
                return NIL;
            }
        }
    }

    /// The children in document order. Empty for a stale handle.
    #[must_use]
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            doc: self,
            next: self.get(id).map_or(NIL, |s| s.first),
        }
    }

    /// The child elements with the given name, in document order.
    pub fn children_named<'a>(
        &'a self,
        id: NodeId,
        name: &str,
    ) -> impl Iterator<Item = NodeId> + use<'a> {
        let sym = self.names.get(name);
        self.children(id)
            .filter(move |&c| sym.is_some() && self.name_sym(c).map(|s| s.0) == sym)
    }

    /// The first child element with the given name.
    #[must_use]
    pub fn child_named(&self, id: NodeId, name: &str) -> Option<NodeId> {
        self.children_named(id, name).next()
    }

    /// The nodes below `id` in pre order (document order), excluding `id`.
    #[must_use]
    pub fn descendants(&self, id: NodeId) -> Preorder<'_> {
        match self.get(id) {
            Some(s) => Preorder {
                doc: self,
                root: id.index,
                next: s.first,
            },
            None => Preorder {
                doc: self,
                root: NIL,
                next: NIL,
            },
        }
    }

    /// `id` followed by its descendants in pre order.
    #[must_use]
    pub fn preorder(&self, id: NodeId) -> Preorder<'_> {
        match self.get(id) {
            Some(_) => Preorder {
                doc: self,
                root: id.index,
                next: id.index,
            },
            None => Preorder {
                doc: self,
                root: NIL,
                next: NIL,
            },
        }
    }

    /// The ancestors, nearest first, ending with the document node (or the root of a detached
    /// tree).
    #[must_use]
    pub fn ancestors(&self, id: NodeId) -> Ancestors<'_> {
        Ancestors {
            doc: self,
            next: self.get(id).map_or(NIL, |s| s.parent),
        }
    }

    /// The following siblings in document order.
    #[must_use]
    pub fn following_siblings(&self, id: NodeId) -> SiblingIter<'_> {
        SiblingIter {
            doc: self,
            next: self.get(id).map_or(NIL, |s| s.next),
            forward: true,
        }
    }

    /// The preceding siblings, nearest first (reverse document order).
    #[must_use]
    pub fn preceding_siblings(&self, id: NodeId) -> SiblingIter<'_> {
        SiblingIter {
            doc: self,
            next: self.get(id).map_or(NIL, |s| s.prev),
            forward: false,
        }
    }

    /// The XPath `following` axis: every node after the subtree of `id` in document order.
    #[must_use]
    pub fn following(&self, id: NodeId) -> Preorder<'_> {
        if self.get(id).is_none() {
            return Preorder {
                doc: self,
                root: NIL,
                next: NIL,
            };
        }
        let mut c = id.index;
        let start = loop {
            let sc = self.sl(c);
            if sc.next != NIL {
                break sc.next;
            }
            if sc.parent == NIL {
                break NIL;
            }
            c = sc.parent;
        };
        Preorder {
            doc: self,
            root: NIL,
            next: start,
        }
    }

    /// The XPath `preceding` axis: every node before `id` in document order that is not an
    /// ancestor, nearest first (reverse document order).
    #[must_use]
    pub fn preceding(&self, id: NodeId) -> Preceding<'_> {
        if self.get(id).is_none() {
            return Preceding {
                doc: self,
                cur: NIL,
                ancestors: Vec::new(),
                pos: 0,
            };
        }
        let ancestors: Vec<u32> = self.ancestors(id).map(|a| a.index).collect();
        Preceding {
            doc: self,
            cur: id.index,
            ancestors,
            pos: 0,
        }
    }

    // ------------------------------------------------------------------ document order

    /// Builds the cached document order numbering of the attached tree (O(n)) unless it is
    /// current. Call before many [`ArenaDoc::compare_order`] or [`ArenaDoc::order_key`] calls.
    pub fn prepare_order(&self) {
        self.order_table();
    }

    fn order_table(&self) -> &Vec<u32> {
        self.order.get_or_init(|| {
            let mut keys = vec![NIL; self.slots.len()];
            let mut counter = 0u32;
            let mut cur = 0u32;
            while cur != NIL {
                if let Some(k) = keys.get_mut(cur as usize) {
                    *k = counter;
                }
                counter = counter.wrapping_add(1);
                cur = self.advance(NIL, cur);
            }
            keys
        })
    }

    /// The pre order number of an attached node (the document node is 0), building the cache if
    /// needed. `None` for stale or detached nodes. Keys are only comparable until the next
    /// structural change.
    #[must_use]
    pub fn order_key(&self, id: NodeId) -> Option<u32> {
        self.get(id)?;
        let k = *self.order_table().get(id.index as usize)?;
        (k != NIL).then_some(k)
    }

    /// Compares two nodes in document order. `None` when either handle is stale.
    ///
    /// Nodes of different trees (an attached tree and detached ones, or two detached trees) are
    /// ordered by the slot of their tree roots: arbitrary but consistent until the next
    /// structural change, and the attached tree always sorts first.
    ///
    /// Cost: O(1) when the cache from [`ArenaDoc::prepare_order`] is current and both nodes are
    /// attached; otherwise O(depth + min sibling distance) with no allocation.
    #[must_use]
    pub fn compare_order(&self, a: NodeId, b: NodeId) -> Option<Ordering> {
        self.get(a)?;
        self.get(b)?;
        if a.index == b.index {
            return Some(Ordering::Equal);
        }
        if let Some(table) = self.order.get() {
            let ka = table.get(a.index as usize).copied().unwrap_or(NIL);
            let kb = table.get(b.index as usize).copied().unwrap_or(NIL);
            if ka != NIL && kb != NIL {
                return Some(ka.cmp(&kb));
            }
        }
        Some(self.order_slow(a.index, b.index))
    }

    fn depth_of(&self, mut i: u32) -> usize {
        let mut d = 0usize;
        while self.sl(i).parent != NIL {
            i = self.sl(i).parent;
            d += 1;
        }
        d
    }

    fn order_slow(&self, a: u32, b: u32) -> Ordering {
        let (da, db) = (self.depth_of(a), self.depth_of(b));
        let (mut x, mut y) = (a, b);
        for _ in db..da {
            x = self.sl(x).parent;
        }
        for _ in da..db {
            y = self.sl(y).parent;
        }
        if x == y {
            // One node is an ancestor of the other; the ancestor comes first.
            return if da > db {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        while self.sl(x).parent != self.sl(y).parent {
            x = self.sl(x).parent;
            y = self.sl(y).parent;
        }
        if self.sl(x).parent == NIL {
            return x.cmp(&y);
        }
        // Distinct siblings: scan both forward in lockstep, so the cost is bounded by the
        // smaller of the two distances to the end or to each other.
        let (mut p, mut q) = (x, y);
        loop {
            match self.sl(p).next {
                n if n == y => return Ordering::Less,
                NIL => return Ordering::Greater,
                n => p = n,
            }
            match self.sl(q).next {
                n if n == x => return Ordering::Greater,
                NIL => return Ordering::Less,
                n => q = n,
            }
        }
    }

    /// Sorts handles into document order, dropping stale handles and duplicates. Builds the
    /// order cache first, so the cost is O(n) on a freshly mutated arena plus O(k log k).
    pub fn sort_document_order(&self, ids: &mut Vec<NodeId>) {
        ids.retain(|&id| self.contains(id));
        self.prepare_order();
        ids.sort_by(|&a, &b| self.compare_order(a, b).unwrap_or(Ordering::Equal));
        ids.dedup();
    }

    // ------------------------------------------------------------------ import and export

    fn element_from(&mut self, n: &Node, origin: OriginId) -> Result<u32, TreeError> {
        let name = self.names.intern_checked(&n.tag, &n.tag)?;
        let mut attrs: Vec<(u32, String)> = Vec::with_capacity(n.attrs.len());
        for (k, v) in &n.attrs {
            let sym = self.names.intern_checked(k, &n.tag)?;
            if let Some(ch) = first_invalid_char(v) {
                return Err(TreeError::InvalidChar {
                    path: n.tag.clone(),
                    ch: u32::from(ch),
                });
            }
            if attrs.iter().any(|(s, _)| *s == sym) {
                return Err(TreeError::DuplicateAttr {
                    path: n.tag.clone(),
                    name: k.clone(),
                });
            }
            attrs.push((sym, v.clone()));
        }
        self.alloc(NodeKind::Element, name, origin.0, Data::Attrs(attrs))
    }

    fn import_below(&mut self, node: &Node, root: u32, origin: OriginId) -> Result<(), TreeError> {
        let mut stack: Vec<(&Node, usize, u32)> = vec![(node, 0, root)];
        while let Some(top) = stack.last_mut() {
            let (n, cursor, at) = (top.0, top.1, top.2);
            top.1 += 1;
            match n.children.get(cursor) {
                None => {
                    stack.pop();
                }
                Some(Child::Text(t)) => {
                    if let Some(ch) = first_invalid_char(t) {
                        return Err(TreeError::InvalidChar {
                            path: n.tag.clone(),
                            ch: u32::from(ch),
                        });
                    }
                    let i = self.alloc(NodeKind::Text, NIL, origin.0, Data::Text(t.clone()))?;
                    self.link_last(at, i);
                }
                Some(Child::Element(c)) => {
                    if stack.len() >= MAX_DEPTH {
                        return Err(TreeError::TooDeep {
                            path: c.tag.clone(),
                            max: MAX_DEPTH,
                        });
                    }
                    let i = self.element_from(c, origin)?;
                    self.link_last(at, i);
                    stack.push((c, 0, i));
                }
            }
        }
        Ok(())
    }

    /// Copies an owned tree into the arena as a detached subtree and returns its root. Every
    /// created node gets `origin`.
    ///
    /// # Errors
    /// Invalid names, characters, duplicate attributes, depth over [`MAX_DEPTH`], or a full
    /// arena. On error nothing remains allocated from this import.
    pub fn import(&mut self, node: &Node, origin: OriginId) -> Result<NodeId, TreeError> {
        let root = self.element_from(node, origin)?;
        if let Err(e) = self.import_below(node, root, origin) {
            self.free_subtree(root);
            return Err(e);
        }
        Ok(self.id_at(root))
    }

    /// Imports an element or a text child as a detached node.
    ///
    /// # Errors
    /// See [`ArenaDoc::import`].
    pub fn import_child(&mut self, child: &Child, origin: OriginId) -> Result<NodeId, TreeError> {
        match child {
            Child::Element(n) => self.import(n, origin),
            Child::Text(t) => self.create_text(t, origin),
        }
    }

    /// Imports `node` and appends it to `parent`.
    ///
    /// # Errors
    /// Stale or text parent, plus everything [`ArenaDoc::import`] reports.
    pub fn append_node(
        &mut self,
        parent: NodeId,
        node: &Node,
        origin: OriginId,
    ) -> Result<NodeId, TreeError> {
        let p = self.container_index(parent, "append_node")?;
        let id = self.import(node, origin)?;
        self.link_last(p, id.index);
        Ok(id)
    }

    /// Exports the subtree of an element as an owned tree. `None` for stale handles and for
    /// nodes that are not elements. Round trip identity: `import` then `to_node` returns an equal
    /// [`Node`].
    #[must_use]
    pub fn to_node(&self, id: NodeId) -> Option<Node> {
        let s = self.get(id)?;
        if s.kind != NodeKind::Element {
            return None;
        }
        let mut stack: Vec<(Node, u32)> = vec![(self.shell(id.index), s.first)];
        loop {
            let cursor = stack.last()?.1;
            if cursor == NIL {
                let (done, _) = stack.pop()?;
                match stack.last_mut() {
                    Some(parent) => parent.0.children.push(Child::Element(done)),
                    None => return Some(done),
                }
                continue;
            }
            let cs = self.sl(cursor);
            if let Some(top) = stack.last_mut() {
                top.1 = cs.next;
            }
            match &cs.data {
                Data::Text(t) => {
                    if let Some(top) = stack.last_mut() {
                        top.0.children.push(Child::Text(t.clone()));
                    }
                }
                _ => stack.push((self.shell(cursor), cs.first)),
            }
        }
    }

    /// A node with the tag and attributes of the slot and no children.
    fn shell(&self, i: u32) -> Node {
        let s = self.sl(i);
        let attrs = match &s.data {
            Data::Attrs(a) => a
                .iter()
                .map(|(k, v)| (self.names.resolve(*k).to_owned(), v.clone()))
                .collect(),
            _ => Vec::new(),
        };
        Node {
            tag: self.names.resolve(s.name).to_owned(),
            attrs,
            children: Vec::new(),
        }
    }

    /// Exports an element or text node as a [`Child`].
    #[must_use]
    pub fn to_child(&self, id: NodeId) -> Option<Child> {
        match self.get(id)?.kind {
            NodeKind::Text => self.text(id).map(|t| Child::Text(t.to_owned())),
            NodeKind::Element => self.to_node(id).map(Child::Element),
            NodeKind::Document => None,
        }
    }

    /// Exports the children of an element or of the document node.
    #[must_use]
    pub fn children_to_vec(&self, id: NodeId) -> Option<Vec<Child>> {
        self.get(id)?;
        Some(self.children(id).filter_map(|c| self.to_child(c)).collect())
    }

    /// Exports the document element (see [`ArenaDoc::document_element`]).
    #[must_use]
    pub fn to_document_node(&self) -> Option<Node> {
        self.to_node(self.document_element()?)
    }

    // ------------------------------------------------------------------ cloning

    fn snapshot(&self, root: u32) -> Vec<Rec> {
        let mut out = Vec::new();
        let mut cur = root;
        let mut depth = 0usize;
        loop {
            let s = self.sl(cur);
            out.push(Rec {
                depth,
                kind: s.kind,
                origin: s.origin,
                name: s.name,
                data: s.data.clone(),
            });
            if s.first != NIL {
                cur = s.first;
                depth += 1;
                continue;
            }
            loop {
                if cur == root {
                    return out;
                }
                let sc = self.sl(cur);
                if sc.next != NIL {
                    cur = sc.next;
                    break;
                }
                cur = sc.parent;
                depth = depth.saturating_sub(1);
            }
        }
    }

    fn build(&mut self, recs: Vec<Rec>, from: Option<&Interner>) -> Result<NodeId, TreeError> {
        let mut memo: FxHashMap<u32, u32> = FxHashMap::default();
        let mut stack: Vec<u32> = Vec::new();
        let mut root = NIL;
        let result = (|| -> Result<(), TreeError> {
            for rec in recs {
                let (name, data) = match from {
                    None => (rec.name, rec.data),
                    Some(src) => {
                        let mut map = |sym: u32, names: &mut Interner| -> Result<u32, TreeError> {
                            if let Some(&m) = memo.get(&sym) {
                                return Ok(m);
                            }
                            let m = names.intern_trusted(src.resolve(sym))?;
                            memo.insert(sym, m);
                            Ok(m)
                        };
                        let name = if rec.kind == NodeKind::Element {
                            map(rec.name, &mut self.names)?
                        } else {
                            NIL
                        };
                        let data = match rec.data {
                            Data::Attrs(a) => {
                                let mut out = Vec::with_capacity(a.len());
                                for (k, v) in a {
                                    out.push((map(k, &mut self.names)?, v));
                                }
                                Data::Attrs(out)
                            }
                            other => other,
                        };
                        (name, data)
                    }
                };
                let i = self.alloc(rec.kind, name, rec.origin, data)?;
                stack.truncate(rec.depth);
                match stack.last() {
                    Some(&p) => self.link_last(p, i),
                    None => root = i,
                }
                stack.push(i);
            }
            Ok(())
        })();
        match result {
            Ok(()) if root != NIL => Ok(self.id_at(root)),
            Ok(()) => Err(TreeError::StaleId),
            Err(e) => {
                if root != NIL {
                    self.free_subtree(root);
                }
                Err(e)
            }
        }
    }

    /// Deep copies a subtree inside this arena. The copy is detached, keeps the origins of the
    /// original nodes and is independent of it.
    ///
    /// # Errors
    /// Stale handle, the document node, or a full arena.
    pub fn deep_clone(&mut self, id: NodeId) -> Result<NodeId, TreeError> {
        let i = self.idx(id)?;
        if self.sl(i).kind == NodeKind::Document {
            return Err(TreeError::DocumentRoot);
        }
        let recs = self.snapshot(i);
        self.build(recs, None)
    }

    /// Deep copies a subtree of another arena into this one (names are re-interned). The copy is
    /// detached and keeps the origin values of the source nodes.
    ///
    /// # Errors
    /// A stale handle of `src`, the document node, or a full arena.
    pub fn deep_clone_from(&mut self, src: &ArenaDoc, id: NodeId) -> Result<NodeId, TreeError> {
        let i = src.idx(id)?;
        if src.sl(i).kind == NodeKind::Document {
            return Err(TreeError::DocumentRoot);
        }
        let recs = src.snapshot(i);
        self.build(recs, Some(&src.names))
    }

    // ------------------------------------------------------------------ integrity

    /// Verifies every internal invariant: link symmetry, parent and child agreement, node kinds,
    /// the live count, the free list and reachability. O(n); meant for tests and debugging.
    ///
    /// # Errors
    /// [`TreeError::Corrupt`] describing the first violation.
    pub fn check_integrity(&self) -> Result<(), TreeError> {
        let bad = |m: String| Err(TreeError::Corrupt(m));
        let live = self.slots.iter().filter(|s| s.live).count();
        if live != self.live {
            return bad(format!("live count {} but {} live slots", self.live, live));
        }
        for &f in &self.free {
            if self.slots.get(f as usize).is_none_or(|s| s.live) {
                return bad(format!("free list holds live or missing slot {f}"));
            }
        }
        let mut reached = 0usize;
        for (i, s) in self.slots.iter().enumerate() {
            if !s.live {
                continue;
            }
            let i = i as u32;
            if s.kind == NodeKind::Text && s.first != NIL {
                return bad(format!("text node {i} has children"));
            }
            if s.kind == NodeKind::Document && (i != 0 || s.parent != NIL) {
                return bad("document node misplaced".to_owned());
            }
            let mut prev = NIL;
            let mut c = s.first;
            let mut steps = 0usize;
            while c != NIL {
                steps += 1;
                if steps > self.slots.len() {
                    return bad(format!("child chain of {i} does not terminate"));
                }
                let Some(cs) = self.slots.get(c as usize).filter(|x| x.live) else {
                    return bad(format!("child {c} of {i} is dead"));
                };
                if cs.parent != i || cs.prev != prev {
                    return bad(format!("child {c} of {i} has inconsistent links"));
                }
                if cs.kind == NodeKind::Document {
                    return bad(format!("document node linked below {i}"));
                }
                prev = c;
                c = cs.next;
            }
            if s.last != prev {
                return bad(format!("last child of {i} is wrong"));
            }
            if s.parent == NIL {
                if s.prev != NIL || s.next != NIL {
                    return bad(format!("root {i} has siblings"));
                }
                let mut cur = i;
                while cur != NIL {
                    reached += 1;
                    cur = self.advance(i, cur);
                }
            }
        }
        if reached != live {
            return bad(format!(
                "{reached} nodes reachable from roots but {live} live"
            ));
        }
        Ok(())
    }
}

/// Iterator over the children of a node, see [`ArenaDoc::children`].
#[derive(Debug, Clone)]
pub struct Children<'a> {
    doc: &'a ArenaDoc,
    next: u32,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let i = self.next;
        let s = self.doc.slots.get(i as usize)?;
        self.next = s.next;
        Some(NodeId {
            index: i,
            generation: s.generation,
        })
    }
}

/// Pre order iterator, see [`ArenaDoc::descendants`], [`ArenaDoc::preorder`] and
/// [`ArenaDoc::following`].
#[derive(Debug, Clone)]
pub struct Preorder<'a> {
    doc: &'a ArenaDoc,
    root: u32,
    next: u32,
}

impl Iterator for Preorder<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let i = self.next;
        let s = self.doc.slots.get(i as usize)?;
        self.next = self.doc.advance(self.root, i);
        Some(NodeId {
            index: i,
            generation: s.generation,
        })
    }
}

/// Iterator over the ancestors of a node, see [`ArenaDoc::ancestors`].
#[derive(Debug, Clone)]
pub struct Ancestors<'a> {
    doc: &'a ArenaDoc,
    next: u32,
}

impl Iterator for Ancestors<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let i = self.next;
        let s = self.doc.slots.get(i as usize)?;
        self.next = s.parent;
        Some(NodeId {
            index: i,
            generation: s.generation,
        })
    }
}

/// Iterator over following or preceding siblings, see [`ArenaDoc::following_siblings`].
#[derive(Debug, Clone)]
pub struct SiblingIter<'a> {
    doc: &'a ArenaDoc,
    next: u32,
    forward: bool,
}

impl Iterator for SiblingIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let i = self.next;
        let s = self.doc.slots.get(i as usize)?;
        self.next = if self.forward { s.next } else { s.prev };
        Some(NodeId {
            index: i,
            generation: s.generation,
        })
    }
}

/// Iterator for the XPath `preceding` axis, see [`ArenaDoc::preceding`].
#[derive(Debug, Clone)]
pub struct Preceding<'a> {
    doc: &'a ArenaDoc,
    cur: u32,
    ancestors: Vec<u32>,
    pos: usize,
}

impl Iterator for Preceding<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        loop {
            let s = self.doc.slots.get(self.cur as usize)?;
            let cand = if s.prev != NIL {
                let mut d = s.prev;
                while self.doc.sl(d).last != NIL {
                    d = self.doc.sl(d).last;
                }
                d
            } else {
                s.parent
            };
            if cand == NIL {
                self.cur = NIL;
                return None;
            }
            self.cur = cand;
            if self.ancestors.get(self.pos) == Some(&cand) {
                self.pos += 1;
                continue;
            }
            return Some(self.doc.id_at(cand));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: OriginId = OriginId(7);

    fn sample() -> (ArenaDoc, NodeId) {
        let node = Node::builder("Defs")
            .child(
                Node::builder("A")
                    .attr("k", "v")
                    .text_elem("x", "1")
                    .text_elem("y", "2"),
            )
            .text_elem("B", "bee")
            .build();
        let doc = ArenaDoc::from_node(&node, O).expect("import");
        let top = doc.document_element().expect("document element");
        (doc, top)
    }

    #[test]
    fn import_export_round_trips() {
        let (doc, top) = sample();
        let node = doc.to_node(top).expect("export");
        assert_eq!(node.find("A/x").and_then(Node::leaf_text), Some("1"));
        assert_eq!(doc.origin(top), Some(O));
        assert_eq!(doc.check_integrity(), Ok(()));
    }

    #[test]
    fn stale_ids_report_errors_instead_of_panicking() {
        let (mut doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        doc.remove(a).expect("remove");
        assert!(!doc.contains(a));
        assert_eq!(doc.kind(a), None);
        assert_eq!(doc.name(a), None);
        assert_eq!(doc.append_child(a, top), Err(TreeError::StaleId));
        assert_eq!(doc.set_attr(a, "k", "v"), Err(TreeError::StaleId));
        assert_eq!(doc.children(a).count(), 0);
        assert!(doc.string_value(a).is_none());
        assert!(doc.compare_order(a, top).is_none());
        let fresh: Vec<NodeId> = (0..5)
            .map(|_| doc.create_element("Z", O).expect("create"))
            .collect();
        let reused = fresh
            .iter()
            .find(|n| n.slot() == a.slot())
            .copied()
            .expect("slot is recycled");
        assert!(
            !doc.contains(a),
            "old handle stays stale after the slot is reused"
        );
        assert!(doc.contains(reused));
        assert_ne!(reused.generation(), a.generation());
    }

    #[test]
    fn linking_rules_are_enforced() {
        let (mut doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        let b = doc.child_named(top, "B").expect("B");
        assert_eq!(doc.append_child(b, a), Err(TreeError::AlreadyAttached));
        doc.detach(top).expect("detach document element");
        assert_eq!(doc.append_child(a, top), Err(TreeError::Cycle));
        assert_eq!(doc.append_child(top, top), Err(TreeError::Cycle));
        let t = doc.create_text("t", O).expect("text");
        assert!(matches!(
            doc.append_child(t, top),
            Err(TreeError::WrongKind { .. })
        ));
        let root = doc.root();
        assert_eq!(doc.detach(root), Err(TreeError::DocumentRoot));
        assert_eq!(doc.remove(root), Err(TreeError::DocumentRoot));
        assert_eq!(doc.append_child(top, root), Err(TreeError::DocumentRoot));
        assert_eq!(doc.insert_before(root, t), Err(TreeError::DocumentRoot));
        let loose = doc.create_element("L", O).expect("el");
        assert_eq!(doc.insert_after(loose, t), Err(TreeError::NotAttached));
        assert_eq!(doc.check_integrity(), Ok(()));
    }

    #[test]
    fn batch_failure_changes_nothing() {
        let (mut doc, top) = sample();
        let n1 = doc.create_element("N1", O).expect("n1");
        let a = doc.child_named(top, "A").expect("A");
        let before = doc.to_node(top);
        assert_eq!(
            doc.append_children(top, &[n1, a]),
            Err(TreeError::AlreadyAttached)
        );
        assert_eq!(
            doc.append_children(top, &[n1, n1]),
            Err(TreeError::AlreadyAttached)
        );
        assert_eq!(doc.to_node(top), before);
        assert!(doc.parent(n1).is_none());
    }

    #[test]
    fn replace_with_list_and_remove() {
        let (mut doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        let p = doc.create_element("P", O).expect("p");
        let q = doc.create_text("q", O).expect("q");
        doc.replace_with(a, &[p, q]).expect("replace");
        assert!(doc.contains(a) && doc.parent(a).is_none());
        let names: Vec<_> = doc.children(top).map(|c| doc.kind(c)).collect();
        assert_eq!(
            names,
            vec![
                Some(NodeKind::Element),
                Some(NodeKind::Text),
                Some(NodeKind::Element)
            ]
        );
        doc.remove(a).expect("free");
        assert!(!doc.contains(a));
        doc.replace_with(p, &[]).expect("replace by nothing");
        assert_eq!(doc.child_count(top), 2);
        assert_eq!(doc.check_integrity(), Ok(()));
    }

    #[test]
    fn attributes_keep_order_and_replace_in_place() {
        let (mut doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        assert_eq!(doc.set_attr(a, "z", "1"), Ok(None));
        assert_eq!(doc.set_attr(a, "k", "w"), Ok(Some("v".to_owned())));
        let attrs: Vec<_> = doc.attrs(a).collect();
        assert_eq!(attrs, vec![("k", "w"), ("z", "1")]);
        assert_eq!(doc.attr(a, "z"), Some("1"));
        assert_eq!(doc.remove_attr(a, "k"), Ok(Some("w".to_owned())));
        assert_eq!(doc.remove_attr(a, "k"), Ok(None));
        assert_eq!(doc.attr(a, "k"), None);
        assert!(matches!(
            doc.set_attr(a, "bad name", "x"),
            Err(TreeError::InvalidName { .. })
        ));
        assert!(matches!(
            doc.set_attr(a, "k", "\u{1}"),
            Err(TreeError::InvalidChar { .. })
        ));
        assert!(matches!(
            doc.set_attr(doc.root(), "k", "v"),
            Err(TreeError::WrongKind { .. })
        ));
    }

    #[test]
    fn string_value_concatenates_descendant_text() {
        let (doc, top) = sample();
        assert_eq!(doc.string_value(top).as_deref(), Some("12bee"));
        let b = doc.child_named(top, "B").expect("B");
        assert!(matches!(doc.string_value(b), Some(Cow::Borrowed("bee"))));
    }

    #[test]
    fn deep_clone_is_independent_and_crosses_arenas() {
        let (mut doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        let copy = doc.deep_clone(a).expect("clone");
        assert_ne!(copy, a);
        doc.set_attr(copy, "k", "changed").expect("set");
        assert_eq!(doc.attr(a, "k"), Some("v"));
        assert_eq!(doc.origin(copy), Some(O));
        let mut other = ArenaDoc::new();
        let far = other.deep_clone_from(&doc, copy).expect("cross clone");
        assert_eq!(other.to_node(far), doc.to_node(copy));
        assert_eq!(doc.deep_clone(doc.root()), Err(TreeError::DocumentRoot));
        assert_eq!(other.check_integrity(), Ok(()));
    }

    #[test]
    fn order_comparison_matches_between_cache_and_walk() {
        let (mut doc, top) = sample();
        let all: Vec<_> = doc.preorder(doc.root()).collect();
        let extra = doc.create_element("E", O).expect("e");
        doc.prepend_child(top, extra).expect("prepend");
        let all2: Vec<_> = doc.preorder(doc.root()).collect();
        assert_eq!(all2.len(), all.len() + 1);
        for (i, &a) in all2.iter().enumerate() {
            for (j, &b) in all2.iter().enumerate() {
                assert_eq!(doc.compare_order(a, b), Some(i.cmp(&j)), "walk");
            }
        }
        doc.prepare_order();
        for (i, &a) in all2.iter().enumerate() {
            for (j, &b) in all2.iter().enumerate() {
                assert_eq!(doc.compare_order(a, b), Some(i.cmp(&j)), "cache");
            }
        }
    }

    #[test]
    fn following_and_preceding_axes() {
        let (doc, top) = sample();
        let a = doc.child_named(top, "A").expect("A");
        let y = doc.child_named(a, "y").expect("y");
        let b = doc.child_named(top, "B").expect("B");
        let following: Vec<_> = doc
            .following(y)
            .filter_map(|n| doc.name(n).map(str::to_owned))
            .collect();
        assert_eq!(following, vec!["B"]);
        let preceding: Vec<_> = doc
            .preceding(b)
            .filter_map(|n| doc.name(n).map(str::to_owned))
            .collect();
        assert_eq!(preceding, vec!["y", "x", "A"]);
        let preceding: Vec<_> = doc
            .preceding(y)
            .filter_map(|n| doc.name(n).map(str::to_owned))
            .collect();
        assert_eq!(preceding, vec!["x"]);
    }
}
