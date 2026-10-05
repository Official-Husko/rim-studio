//! XML inheritance: registration, parent choice, resolution and the child over parent merge.
//!
//! A port of the behaviour of the game's `XmlInheritance` (research note, section 6), including its
//! quirks, each pinned by a research vector:
//!
//! - nodes with `Name` or `ParentName` register, unless their `MayRequire` is not met; a `Name`
//!   used twice inside one mod is an error and the second node is not registered;
//! - a child sees the parent with the highest load order that is not after its own mod; a node
//!   created by a patch has no mod and prefers a patch-created parent (a mod child falls back to one);
//! - resolution is top down; members of a cycle (and everything below them) are never reached, are
//!   reported once each, and stay unmerged;
//! - the merge clones the resolved parent, drops its attributes (not under `Inherit="false"`), and
//!   merges the child into it: text replaces the content (but only the FIRST child node of a
//!   multi-child parent is removed, the live list quirk), an empty element keeps the parent's
//!   element children, `li` and the children of a few marked fields are appended, every other
//!   element merges into the first same-named element;
//! - `Abstract`, `Name` and `Class` are not inherited.
//!
//! Each inheritance tree is resolved independently in its own scratch arena, trees in parallel with
//! rayon; the output is assembled in document order, so the result does not depend on the thread
//! count. Resolved nodes are exported as owned [`Node`]s with adjacent text merged.

use std::collections::BTreeSet;

use rayon::prelude::*;
use rimstudio_core::diag::{DiagSink, Severity};
use rimstudio_core::ids::ModIdx;
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::{ArenaDoc, Child, Node, NodeId, NodeKind, TreeError};
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::diag_codes::{self, Site};
use crate::merge::UnifiedDoc;
use crate::types::{Origin, ParentRef, PatchRef};

/// The field names that carry `XmlInheritanceAllowDuplicateNodes` in game version 1.6.4871; mods can
/// add more through their own assemblies, so the set is configurable.
pub const DEFAULT_ALLOW_DUPLICATE_NODES: [&str; 6] = [
    "descriptionHyperlinks",
    "nullifyingTraitDegrees",
    "forcedTraits",
    "disallowedTraitsWithDegree",
    "agreeableTraits",
    "disagreeableTraits",
];

/// Settings of the inheritance pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritConfig {
    /// Fields whose children are appended like `li` instead of merged by name.
    pub allow_duplicate_nodes: BTreeSet<String>,
    /// Remove `li` entries whose `MayRequire` or `MayRequireAnyOf` is not met from the resolved
    /// nodes (the game does this when it reads a node into objects). Off by default: the resolved
    /// nodes then show every entry.
    pub prune_li_may_require: bool,
}

impl Default for InheritConfig {
    fn default() -> Self {
        Self {
            allow_duplicate_nodes: DEFAULT_ALLOW_DUPLICATE_NODES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            prune_li_may_require: false,
        }
    }
}

/// One top-level node after inheritance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedNode {
    /// Position among the top-level nodes of the unified document.
    pub top_index: u32,
    /// The node in the unified document (valid for that document only).
    #[serde(skip_serializing)]
    pub source: NodeId,
    /// The element name of the node as written.
    pub tag: String,
    /// The file the node was read from; `None` for a node a patch created.
    pub origin: Option<Origin>,
    /// The resolved node: the parent chain merged in (the node itself when it has no parent or
    /// failed to resolve).
    pub node: Node,
    /// The inheritance chain, nearest parent first (empty for unresolved nodes).
    pub parents: Vec<ParentRef>,
    /// The node's `ParentName`.
    pub parent_name: Option<String>,
    /// False when the node has a `ParentName` but is not in the resolved table (it did not
    /// register, or is part of a cycle). Nodes without a `ParentName` are always resolved.
    pub resolved: bool,
    /// The `MayRequire` attribute of the node as it stood after patching.
    pub may_require: Option<String>,
    /// The `MayRequireAnyOf` attribute of the node as it stood after patching.
    pub may_require_any_of: Option<String>,
    /// The `Abstract` attribute of the node as it stood after patching (never inherited).
    pub abstract_attr: Option<String>,
    /// The patch operations that touched the node's subtree.
    pub patched_by: Vec<PatchRef>,
}

/// The inheritance result: every top-level node in document order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDoc {
    /// The nodes in document order.
    pub nodes: Vec<ResolvedNode>,
    /// Number of nodes that took part in inheritance (registered).
    pub registered: u32,
    /// Number of nodes that are members of, or below, a cycle.
    pub cyclic: u32,
    /// Diagnostics found while resolving.
    #[serde(skip_serializing)]
    pub diagnostics: DiagSink,
}

/// A registered node.
#[derive(Debug)]
struct Entry {
    top: usize,
    mod_idx: Option<ModIdx>,
    load: Option<usize>,
    parent_name: Option<String>,
    parent: Option<usize>,
    children: Vec<usize>,
}

/// Read only inputs shared by the parallel tasks.
struct Shared<'a> {
    doc: &'a UnifiedDoc,
    tops: &'a [NodeId],
    entries: &'a [Entry],
    cfg: &'a InheritConfig,
    active: &'a ActiveSet,
}

/// Resolves the inheritance of every top-level node of the unified document.
#[must_use]
pub fn resolve_inheritance(
    doc: &UnifiedDoc,
    active: &ActiveSet,
    cfg: &InheritConfig,
) -> ResolvedDoc {
    let mut sink = DiagSink::new();
    let tops = doc.top_level();
    let arena = doc.arena();

    // 1. registration
    let mut entries: Vec<Entry> = Vec::new();
    let mut by_name: FxHashMap<String, Vec<usize>> = FxHashMap::default();
    let mut entry_of_top: Vec<Option<usize>> = vec![None; tops.len()];
    for (i, &top) in tops.iter().enumerate() {
        let name = arena.attr(top, "Name").map(str::to_owned);
        let parent_name = arena.attr(top, "ParentName").map(str::to_owned);
        if name.is_none() && parent_name.is_none() {
            continue;
        }
        if let Some(may) = arena.attr(top, "MayRequire") {
            let ids: Vec<&str> = may.split(',').collect();
            if !active.all_active(&ids) {
                continue;
            }
        }
        let origin = doc.origin_of(top);
        let mod_idx = origin.map(|o| o.mod_idx);
        let load = mod_idx.map(|m| doc.order().load_order(m).unwrap_or(usize::MAX));
        if let Some(n) = &name
            && let Some(list) = by_name.get(n)
            && list
                .iter()
                .any(|&e| entries.get(e).is_some_and(|x| x.mod_idx == mod_idx))
        {
            sink.push(Site::new(mod_idx, origin.map(|o| o.file)).diag(
                diag_codes::INHERIT_DUPLICATE_NAME,
                Severity::Error,
                format!(
                    "Could not register node named \"{n}\" in mod {} because this name is already \
                     used in this mod.",
                    package_label(doc, mod_idx)
                ),
            ));
            continue;
        }
        let idx = entries.len();
        entries.push(Entry {
            top: i,
            mod_idx,
            load,
            parent_name,
            parent: None,
            children: Vec::new(),
        });
        if let Some(slot) = entry_of_top.get_mut(i) {
            *slot = Some(idx);
        }
        if let Some(n) = name {
            by_name.entry(n).or_default().push(idx);
        }
    }

    // 2. parents
    for e in 0..entries.len() {
        let Some(pn) = entries.get(e).and_then(|x| x.parent_name.clone()) else {
            continue;
        };
        let parent = best_parent(&entries, &by_name, e, &pn);
        match parent {
            Some(p) => {
                if let Some(x) = entries.get_mut(e) {
                    x.parent = Some(p);
                }
                if let Some(x) = entries.get_mut(p) {
                    x.children.push(e);
                }
            }
            None => {
                let (mod_idx, top) = entries.get(e).map_or((None, 0), |x| (x.mod_idx, x.top));
                let tag = tops
                    .get(top)
                    .and_then(|&t| arena.name(t))
                    .unwrap_or_default();
                sink.push(Site::new(mod_idx, None).diag(
                    diag_codes::INHERIT_MISSING_PARENT,
                    Severity::Error,
                    format!("Could not find parent node named \"{pn}\" for node \"{tag}\"."),
                ));
            }
        }
    }

    // 3. resolution: one task per tree that has children
    let tree_roots: Vec<usize> = (0..entries.len())
        .filter(|&e| {
            entries
                .get(e)
                .is_some_and(|x| x.parent.is_none() && !x.children.is_empty())
        })
        .collect();
    let shared = Shared {
        doc,
        tops: &tops,
        entries: &entries,
        cfg,
        active,
    };
    let trees: Vec<TreeResult> = tree_roots
        .par_iter()
        .map(|&root| resolve_tree(&shared, root))
        .collect();
    let mut exported: Vec<Option<Node>> = vec![None; tops.len()];
    let mut resolved_entry = vec![false; entries.len()];
    for t in trees {
        sink.merge(t.diag);
        for (e, node) in t.members {
            if let Some(x) = entries.get(e) {
                if let Some(slot) = exported.get_mut(x.top) {
                    *slot = Some(node);
                }
                if let Some(r) = resolved_entry.get_mut(e) {
                    *r = true;
                }
            }
        }
    }
    // roots without a parent are resolved to themselves
    for (e, x) in entries.iter().enumerate() {
        if x.parent.is_none()
            && let Some(r) = resolved_entry.get_mut(e)
        {
            *r = true;
        }
    }
    let mut cyclic = 0u32;
    for (e, x) in entries.iter().enumerate() {
        if !resolved_entry.get(e).copied().unwrap_or(false) {
            cyclic = cyclic.saturating_add(1);
            let tag = tops
                .get(x.top)
                .and_then(|&t| arena.name(t))
                .unwrap_or_default();
            sink.push(Site::new(x.mod_idx, None).diag(
                diag_codes::INHERIT_CYCLE,
                Severity::Error,
                format!("Cyclic inheritance hierarchy detected for node \"{tag}\"."),
            ));
        }
    }

    // 4. export the nodes that were not merged
    let originals: Vec<(usize, Node)> = exported
        .par_iter()
        .enumerate()
        .filter(|(_, slot)| slot.is_none())
        .filter_map(|(i, _)| {
            let id = *tops.get(i)?;
            let mut node = arena.to_node(id)?;
            finish_node(&mut node, cfg, active);
            Some((i, node))
        })
        .collect();
    for (i, node) in originals {
        if let Some(slot) = exported.get_mut(i) {
            *slot = Some(node);
        }
    }

    // 5. assemble
    let mut nodes = Vec::with_capacity(tops.len());
    for (i, (&top, slot)) in tops.iter().zip(exported).enumerate() {
        let entry = entry_of_top.get(i).copied().flatten();
        let resolved_ok = match entry {
            Some(e) => resolved_entry.get(e).copied().unwrap_or(false),
            None => false,
        };
        let parent_name = arena.attr(top, "ParentName").map(str::to_owned);
        let in_table = parent_name.is_none() || resolved_ok;
        let parents = match entry {
            Some(e) if resolved_ok => parent_chain(doc, &entries, &tops, e),
            _ => Vec::new(),
        };
        let node = match slot {
            Some(n) => n,
            None => Node::new(arena.name(top).unwrap_or_default()),
        };
        nodes.push(ResolvedNode {
            top_index: u32::try_from(i).unwrap_or(u32::MAX),
            source: top,
            tag: arena.name(top).unwrap_or_default().to_owned(),
            origin: doc.origin_of(top),
            node,
            parents,
            parent_name,
            resolved: in_table,
            may_require: arena.attr(top, "MayRequire").map(str::to_owned),
            may_require_any_of: arena.attr(top, "MayRequireAnyOf").map(str::to_owned),
            abstract_attr: arena.attr(top, "Abstract").map(str::to_owned),
            patched_by: doc.patched_by(top).to_vec(),
        });
    }
    ResolvedDoc {
        nodes,
        registered: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        cyclic,
        diagnostics: sink,
    }
}

fn package_label(doc: &UnifiedDoc, mod_idx: Option<ModIdx>) -> String {
    mod_idx
        .and_then(|m| doc.order().package_id(m))
        .unwrap_or("patch")
        .to_owned()
}

/// `GetBestParentFor`: see the module documentation.
fn best_parent(
    entries: &[Entry],
    by_name: &FxHashMap<String, Vec<usize>>,
    child: usize,
    parent_name: &str,
) -> Option<usize> {
    let cands = by_name.get(parent_name)?;
    let child_load = entries.get(child)?.load;
    let child_has_mod = entries.get(child)?.mod_idx.is_some();
    let load_of = |c: usize| entries.get(c).and_then(|x| x.load);
    let patch_created = |c: usize| entries.get(c).is_some_and(|x| x.mod_idx.is_none());
    if !child_has_mod {
        if let Some(&c) = cands.iter().find(|&&c| patch_created(c)) {
            return Some(c);
        }
        // the lowest load order, the first one on ties
        let mut best: Option<(usize, usize)> = None;
        for &c in cands {
            if let Some(l) = load_of(c)
                && best.is_none_or(|(_, bl)| l < bl)
            {
                best = Some((c, l));
            }
        }
        return best.map(|(c, _)| c);
    }
    let limit = child_load.unwrap_or(usize::MAX);
    let mut best: Option<(usize, usize)> = None;
    for &c in cands {
        if let Some(l) = load_of(c)
            && l <= limit
            && best.is_none_or(|(_, bl)| l > bl)
        {
            best = Some((c, l));
        }
    }
    if let Some((c, _)) = best {
        return Some(c);
    }
    cands.iter().copied().find(|&c| patch_created(c))
}

fn parent_chain(
    doc: &UnifiedDoc,
    entries: &[Entry],
    tops: &[NodeId],
    start: usize,
) -> Vec<ParentRef> {
    let mut out = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    let mut cur = entries.get(start).and_then(|e| e.parent);
    while let Some(p) = cur {
        if seen.contains(&p) {
            break;
        }
        seen.push(p);
        let Some(entry) = entries.get(p) else { break };
        let name = tops
            .get(entry.top)
            .and_then(|&t| doc.arena().attr(t, "Name"))
            .unwrap_or_default();
        out.push(ParentRef {
            mod_idx: entry.mod_idx,
            name: name.to_owned(),
        });
        cur = entry.parent;
    }
    out
}

/// The members of one inheritance tree below its root, resolved.
struct TreeResult {
    members: Vec<(usize, Node)>,
    diag: DiagSink,
}

fn resolve_tree(sh: &Shared<'_>, root: usize) -> TreeResult {
    let src = sh.doc.arena();
    let mut local = ArenaDoc::new();
    let mut ids: FxHashMap<usize, NodeId> = FxHashMap::default();
    let mut members: Vec<(usize, Node)> = Vec::new();
    let mut diag = DiagSink::new();
    let mut stack: Vec<usize> = match sh.entries.get(root) {
        Some(r) => r.children.iter().rev().copied().collect(),
        None => Vec::new(),
    };
    while let Some(e) = stack.pop() {
        let Some(entry) = sh.entries.get(e) else {
            continue;
        };
        let Some(&child_src) = sh.tops.get(entry.top) else {
            continue;
        };
        let site = Site::new(entry.mod_idx, None);
        check_duplicates(sh, child_src, &label_of(src, child_src), site, &mut diag);
        let parent = entry.parent.unwrap_or(root);
        let base = if parent == root {
            sh.entries
                .get(root)
                .and_then(|r| sh.tops.get(r.top))
                .map(|&t| local.deep_clone_from(src, t))
        } else {
            ids.get(&parent).map(|&p| local.deep_clone(p))
        };
        let outcome = match base {
            Some(Ok(current)) => merge_into(sh, &mut local, child_src, current).map(|()| current),
            Some(Err(e)) => Err(e),
            None => Err(TreeError::StaleId),
        };
        match outcome {
            Ok(current) => {
                ids.insert(e, current);
                if let Some(mut node) = local.to_node(current) {
                    finish_node(&mut node, sh.cfg, sh.active);
                    members.push((e, node));
                }
                stack.extend(entry.children.iter().rev().copied());
            }
            Err(err) => diag.push(site.diag(
                diag_codes::INHERIT_NOT_RESOLVED,
                Severity::Error,
                format!("The node could not be merged with its parent: {err}"),
            )),
        }
    }
    TreeResult { members, diag }
}

fn label_of(src: &ArenaDoc, node: NodeId) -> String {
    let tag = src.name(node).unwrap_or_default();
    match src
        .child_named(node, "defName")
        .and_then(|d| src.string_value(d))
    {
        Some(n) => format!("{tag} {n}"),
        None => format!(
            "{tag} (no defName: Name={})",
            src.attr(node, "Name").unwrap_or("None")
        ),
    }
}

/// `true` for `li` and for the children of a field marked to allow duplicate nodes.
fn is_list_element(sh: &Shared<'_>, src: &ArenaDoc, node: NodeId) -> bool {
    if src.name(node) == Some("li") {
        return true;
    }
    src.parent(node)
        .and_then(|p| src.name(p))
        .is_some_and(|n| sh.cfg.allow_duplicate_nodes.contains(n))
}

fn check_duplicates(sh: &Shared<'_>, node: NodeId, label: &str, site: Site, diag: &mut DiagSink) {
    let src = sh.doc.arena();
    let mut used: Vec<&str> = Vec::new();
    for c in src.children(node) {
        if src.kind(c) != Some(NodeKind::Element) {
            continue;
        }
        let name = src.name(c).unwrap_or_default();
        if !is_list_element(sh, src, c) && used.contains(&name) {
            diag.push(
                site.diag(
                    diag_codes::INHERIT_DUPLICATE_NODE_NAME,
                    Severity::Error,
                    format!(
                        "Duplicate XML node name {name} in {} of {label}",
                        src.name(node).unwrap_or_default()
                    ),
                )
                .with_arg("node", name),
            );
        }
        used.push(name);
        check_duplicates(sh, c, label, site, diag);
    }
}

/// `RecursiveNodeCopyOverwriteElements(child, current)` with `child` in the unified document and
/// `current` a clone of the resolved parent in the scratch arena.
fn merge_into(
    sh: &Shared<'_>,
    local: &mut ArenaDoc,
    child: NodeId,
    current: NodeId,
) -> Result<(), TreeError> {
    let src = sh.doc.arena();
    let inherit_false = src
        .attr(child, "Inherit")
        .is_some_and(|v| v.eq_ignore_ascii_case("false"));
    if inherit_false {
        local.clear_children(current)?;
        for n in src.children(child) {
            let copy = local.deep_clone_from(src, n)?;
            local.append_child(current, copy)?;
        }
        for (k, v) in src.attrs(child) {
            if k != "Inherit" {
                local.remove_attr(current, k)?;
                local.set_attr(current, k, v)?;
            }
        }
        return Ok(());
    }
    local.clear_attrs(current)?;
    for (k, v) in src.attrs(child) {
        local.set_attr(current, k, v)?;
    }
    let last_text = src
        .children(child)
        .filter(|&c| {
            src.kind(c) == Some(NodeKind::Text) && src.text(c).is_some_and(|t| !t.is_empty())
        })
        .last();
    let has_elem = src
        .children(child)
        .any(|c| src.kind(c) == Some(NodeKind::Element));
    if let Some(text) = last_text {
        // the live list quirk: only the first child node of the parent goes
        if let Some(first) = local.first_child(current) {
            local.remove(first)?;
        }
        let copy = local.deep_clone_from(src, text)?;
        local.append_child(current, copy)?;
        return Ok(());
    }
    if !has_elem {
        let cur_has_elem = local
            .children(current)
            .any(|c| local.kind(c) == Some(NodeKind::Element));
        if cur_has_elem {
            return Ok(());
        }
        if let Some(first) = local.first_child(current) {
            local.remove(first)?;
        }
        return Ok(());
    }
    let items: Vec<NodeId> = src
        .children(child)
        .filter(|&c| src.kind(c) == Some(NodeKind::Element))
        .collect();
    for item in items {
        if is_list_element(sh, src, item) {
            let copy = local.deep_clone_from(src, item)?;
            local.append_child(current, copy)?;
            continue;
        }
        let name = src.name(item).unwrap_or_default();
        match local.child_named(current, name) {
            Some(target) => merge_into(sh, local, item, target)?,
            None => {
                let copy = local.deep_clone_from(src, item)?;
                local.append_child(current, copy)?;
            }
        }
    }
    Ok(())
}

/// Normalises an exported node (merges adjacent text, drops empty text) and prunes `li` entries
/// when configured.
fn finish_node(node: &mut Node, cfg: &InheritConfig, active: &ActiveSet) {
    normalize_text(node);
    if cfg.prune_li_may_require {
        prune_li_may_require(node, active);
    }
}

fn normalize_text(node: &mut Node) {
    let needs = {
        let mut prev_text = false;
        let mut needs = false;
        for c in &node.children {
            match c {
                Child::Text(t) => {
                    if t.is_empty() || prev_text {
                        needs = true;
                        break;
                    }
                    prev_text = true;
                }
                Child::Element(_) => prev_text = false,
            }
        }
        needs
    };
    if needs {
        let mut out: Vec<Child> = Vec::with_capacity(node.children.len());
        for c in std::mem::take(&mut node.children) {
            match c {
                Child::Text(t) => {
                    if t.is_empty() {
                        continue;
                    }
                    if let Some(Child::Text(prev)) = out.last_mut() {
                        prev.push_str(&t);
                    } else {
                        out.push(Child::Text(t));
                    }
                }
                other => out.push(other),
            }
        }
        node.children = out;
    }
    for c in node.elements_mut() {
        normalize_text(c);
    }
}

/// Whether an `li` entry of a def field is kept: `MayRequire` (all of) and `MayRequireAnyOf` (any of)
/// are both tested, and an empty value is ignored.
fn li_passes(li: &Node, active: &ActiveSet) -> bool {
    if let Some(mr) = li.attr("MayRequire").filter(|v| !v.is_empty()) {
        let ids: Vec<&str> = mr.split(',').collect();
        if !active.all_active(&ids) {
            return false;
        }
    }
    if let Some(any) = li.attr("MayRequireAnyOf").filter(|v| !v.is_empty()) {
        let ids: Vec<&str> = any.split(',').collect();
        if !active.any_active(&ids) {
            return false;
        }
    }
    true
}

/// Removes every `li` below `node` whose `MayRequire` or `MayRequireAnyOf` is not met and returns
/// how many were removed.
///
/// The game evaluates these attributes when it reads the merged node into objects, so this is a
/// post pass over a resolved def. A removed `li` takes its subtree with it; nested entries are only
/// inspected when their ancestors survive.
pub fn prune_li_may_require(node: &mut Node, active: &ActiveSet) -> usize {
    let mut removed = 0;
    let before = node.children.len();
    node.children.retain(|c| match c {
        Child::Element(e) => e.tag != "li" || li_passes(e, active),
        Child::Text(_) => true,
    });
    removed += before.saturating_sub(node.children.len());
    for c in node.elements_mut() {
        removed += prune_li_may_require(c, active);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge;
    use crate::provenance::{ModEntry, ModOrder};
    use crate::types::{DefFile, FileContent};
    use rimstudio_core::ids::FileId;
    use rimstudio_core::tree::NodeBuilder;

    fn order(n: u32) -> ModOrder {
        ModOrder::new(
            (0..n)
                .map(|i| ModEntry::new(ModIdx(i), format!("rs.m{i}"), format!("M{i}")))
                .collect(),
        )
    }

    fn def(tag: &str, attrs: &[(&str, &str)], fields: &[(&str, &str)]) -> NodeBuilder {
        let mut b = NodeBuilder::new(tag);
        for (k, v) in attrs {
            b = b.attr(*k, *v);
        }
        for (k, v) in fields {
            b = b.text_elem(*k, *v);
        }
        b
    }

    fn doc_of(files: Vec<(u32, Vec<NodeBuilder>)>) -> UnifiedDoc {
        let n = files.iter().map(|f| f.0).max().map_or(1, |m| m + 1);
        let defs: Vec<DefFile> = files
            .into_iter()
            .enumerate()
            .map(|(i, (m, nodes))| DefFile {
                mod_idx: ModIdx(m),
                file: FileId(u32::try_from(i).unwrap()),
                rel_path: format!("Defs/f{i}.xml"),
                content: FileContent::parsed(NodeBuilder::new("Defs").children(nodes).build()),
            })
            .collect();
        merge(&order(n), &defs)
    }

    fn resolve(doc: &UnifiedDoc) -> ResolvedDoc {
        resolve_inheritance(doc, &ActiveSet::new(), &InheritConfig::default())
    }

    #[test]
    fn child_fields_override_parent_fields_in_place_and_new_ones_append() {
        let doc = doc_of(vec![(
            0,
            vec![
                def(
                    "RS_T",
                    &[("Name", "Base"), ("Abstract", "True")],
                    &[("label", "base"), ("size", "1")],
                ),
                def(
                    "RS_T",
                    &[("ParentName", "Base")],
                    &[("defName", "C"), ("size", "2")],
                ),
            ],
        )]);
        let r = resolve(&doc);
        let c = &r.nodes[1];
        assert!(c.resolved);
        let got = c.node.clone();
        let want = NodeBuilder::new("RS_T")
            .attr("ParentName", "Base")
            .text_elem("label", "base")
            .text_elem("size", "2")
            .text_elem("defName", "C")
            .build();
        assert_eq!(got, want);
        assert_eq!(c.parents.len(), 1);
        assert_eq!(c.parents[0].name, "Base");
    }

    #[test]
    fn li_children_append_and_nested_elements_merge() {
        let doc = doc_of(vec![(
            0,
            vec![
                NodeBuilder::new("RS_T")
                    .attr("Name", "P")
                    .child(
                        NodeBuilder::new("stats")
                            .text_elem("A", "1")
                            .text_elem("B", "2"),
                    )
                    .child(NodeBuilder::new("comps").text_elem("li", "x")),
                NodeBuilder::new("RS_T")
                    .attr("ParentName", "P")
                    .child(NodeBuilder::new("stats").text_elem("B", "9"))
                    .child(NodeBuilder::new("comps").text_elem("li", "y")),
            ],
        )]);
        let r = resolve(&doc);
        let c = &r.nodes[1].node;
        let stats = c.child("stats").unwrap();
        assert_eq!(stats.child_text("A"), Some("1"));
        assert_eq!(stats.child_text("B"), Some("9"));
        let li: Vec<&str> = c
            .child("comps")
            .unwrap()
            .children_named("li")
            .filter_map(Node::leaf_text)
            .collect();
        assert_eq!(li, vec!["x", "y"]);
    }

    #[test]
    fn a_cycle_is_reported_for_every_member_and_left_unmerged() {
        let doc = doc_of(vec![(
            0,
            vec![
                def(
                    "RS_T",
                    &[("Name", "A"), ("ParentName", "B")],
                    &[("defName", "DA")],
                ),
                def(
                    "RS_T",
                    &[("Name", "B"), ("ParentName", "A")],
                    &[("defName", "DB")],
                ),
                def("RS_T", &[], &[("defName", "N")]),
            ],
        )]);
        let r = resolve(&doc);
        assert_eq!(r.diagnostics.count(&diag_codes::INHERIT_CYCLE), 2);
        assert_eq!(r.cyclic, 2);
        assert!(!r.nodes[0].resolved);
        assert!(!r.nodes[1].resolved);
        assert!(r.nodes[2].resolved);
        assert_eq!(r.nodes[0].node.child_text("defName"), Some("DA"));
    }

    #[test]
    fn a_missing_parent_is_reported_and_the_node_stays_resolved_to_itself() {
        let doc = doc_of(vec![(
            0,
            vec![def("RS_T", &[("ParentName", "Nope")], &[("defName", "M")])],
        )]);
        let r = resolve(&doc);
        assert_eq!(r.diagnostics.count(&diag_codes::INHERIT_MISSING_PARENT), 1);
        assert!(r.nodes[0].resolved);
    }

    #[test]
    fn a_child_cannot_see_a_parent_of_a_later_mod() {
        let doc = doc_of(vec![
            (
                0,
                vec![def(
                    "RS_T",
                    &[("ParentName", "P")],
                    &[("defName", "InCore")],
                )],
            ),
            (1, vec![def("RS_T", &[("Name", "P")], &[("label", "one")])]),
            (
                2,
                vec![def("RS_T", &[("ParentName", "P")], &[("defName", "InTwo")])],
            ),
        ]);
        let r = resolve(&doc);
        assert_eq!(r.diagnostics.count(&diag_codes::INHERIT_MISSING_PARENT), 1);
        assert_eq!(r.nodes[2].node.child_text("label"), Some("one"));
    }

    #[test]
    fn the_same_name_twice_in_one_mod_registers_only_the_first() {
        let doc = doc_of(vec![(
            0,
            vec![
                def("RS_T", &[("Name", "X")], &[("label", "1")]),
                def("RS_T", &[("Name", "X")], &[("label", "2")]),
                def("RS_T", &[("ParentName", "X")], &[("defName", "C")]),
            ],
        )]);
        let r = resolve(&doc);
        assert_eq!(r.diagnostics.count(&diag_codes::INHERIT_DUPLICATE_NAME), 1);
        assert_eq!(r.nodes[2].node.child_text("label"), Some("1"));
    }

    #[test]
    fn inherit_false_replaces_the_content_and_keeps_the_parent_attributes() {
        let doc = doc_of(vec![(
            0,
            vec![
                def(
                    "RS_T",
                    &[("Name", "P"), ("Abstract", "True")],
                    &[("label", "p")],
                ),
                def(
                    "RS_T",
                    &[("ParentName", "P"), ("Inherit", "false")],
                    &[("defName", "C")],
                ),
            ],
        )]);
        let r = resolve(&doc);
        let n = &r.nodes[1].node;
        assert_eq!(
            n.attrs,
            vec![
                ("Name".to_owned(), "P".to_owned()),
                ("Abstract".to_owned(), "True".to_owned()),
                ("ParentName".to_owned(), "P".to_owned()),
            ]
        );
        assert_eq!(n.children.len(), 1);
    }

    #[test]
    fn text_over_a_multi_child_parent_removes_only_the_first_child() {
        let doc = doc_of(vec![(
            0,
            vec![
                NodeBuilder::new("RS_T").attr("Name", "P").child(
                    NodeBuilder::new("x")
                        .child(NodeBuilder::new("a"))
                        .child(NodeBuilder::new("b"))
                        .child(NodeBuilder::new("c")),
                ),
                NodeBuilder::new("RS_T")
                    .attr("ParentName", "P")
                    .text_elem("x", "t"),
            ],
        )]);
        let r = resolve(&doc);
        let x = r.nodes[1].node.child("x").unwrap();
        let tags: Vec<&str> = x.elements().map(|e| e.tag.as_str()).collect();
        assert_eq!(tags, vec!["b", "c"]);
        assert_eq!(x.children.last().and_then(Child::as_text), Some("t"));
    }

    #[test]
    fn prune_removes_unmet_li_entries_and_their_subtrees() {
        let active = ActiveSet::from_ids(["rs.on"]);
        let mut n = NodeBuilder::new("RS_T")
            .child(
                NodeBuilder::new("list")
                    .child(
                        NodeBuilder::new("li")
                            .attr("MayRequire", "rs.off")
                            .text("a"),
                    )
                    .child(NodeBuilder::new("li").attr("MayRequire", "RS.ON").text("b"))
                    .child(
                        NodeBuilder::new("li")
                            .attr("MayRequireAnyOf", "rs.off,rs.on")
                            .text("c"),
                    )
                    .child(
                        NodeBuilder::new("li")
                            .attr("MayRequireAnyOf", "rs.off")
                            .text("d"),
                    )
                    .child(NodeBuilder::new("li").attr("MayRequire", "").text("e")),
            )
            .build();
        let removed = prune_li_may_require(&mut n, &active);
        assert_eq!(removed, 2);
        let kept: Vec<&str> = n
            .child("list")
            .unwrap()
            .elements()
            .filter_map(Node::leaf_text)
            .collect();
        assert_eq!(kept, vec!["b", "c", "e"]);
    }

    #[test]
    fn normalise_merges_adjacent_text() {
        let mut n = Node::new("a");
        n.push_text("x");
        n.push_text("");
        n.push_text("y");
        n.push_child(Node::new("b"));
        n.push_text("z");
        normalize_text(&mut n);
        assert_eq!(n.children.len(), 3);
        assert_eq!(n.children[0].as_text(), Some("xy"));
    }
}
