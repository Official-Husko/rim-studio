"""XML inheritance, a transliteration of the behaviour of Verse/XmlInheritance.cs.

Registration, parent selection, resolution order and the child-over-parent merge follow the
game exactly, including its quirks (each one is covered by a test vector):

  * the merge removes the attributes of the cloned parent and imports the child's attributes
    (so `Abstract`, `Name` and `Class` of the parent are NOT inherited), except under
    Inherit="false", where the parent's attributes stay and the child's are appended;
  * a child node that holds text replaces everything below the parent node, but the game
    deletes children with a foreach over a live child list, which removes only the FIRST child
    (verified with a Mono probe, probes/Probe1.cs);
  * a child element without child elements keeps the parent's element children untouched;
  * `li` (and the children of a few fields marked XmlInheritanceAllowDuplicateNodes) are
    appended, every other element is merged into the first same-named element of the parent.
"""
from __future__ import annotations

import copy
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional, Set

from .diag import Diagnostics
from .mods import ActiveSet
from .xmlnet import (Element, inner_text, child_elements, child_nodes, clear_children, dom_append, first_child_named,
                     import_node, is_element, last_text_child, set_attribute, TextRef)

# Field names carrying [XmlInheritanceAllowDuplicateNodes] in 1.6.4871 (decompiled: Def, ThoughtDef,
# PawnKindDef, MemeDef). Mods may add more through their own assemblies: configurable.
DEFAULT_ALLOW_DUPLICATE_NODES = frozenset({
    "descriptionHyperlinks", "nullifyingTraitDegrees", "forcedTraits", "disallowedTraitsWithDegree",
    "agreeableTraits", "disagreeableTraits",
})


@dataclass(eq=False)
class INode:
    xml: Element
    mod: Optional[object]            # ModInfo or None (nodes created by patches have no mod)
    parent: Optional["INode"] = None
    children: List["INode"] = field(default_factory=list)
    resolved: Optional[Element] = None

    @property
    def load_order(self) -> int:
        return self.mod.index


class Inheritance:
    def __init__(self, diag: Diagnostics, active: ActiveSet, allow_duplicate_nodes=DEFAULT_ALLOW_DUPLICATE_NODES,
                 mod_of: Callable[[Element], Optional[object]] = lambda e: None):
        self.diag = diag
        self.active = active
        self.allow = allow_duplicate_nodes
        self.mod_of = mod_of
        self.unresolved: List[INode] = []
        self.by_name: Dict[str, List[INode]] = {}
        self.resolved_nodes: Dict[Element, INode] = {}

    # -------------------------------------------------------------- registration
    def try_register(self, node: Element, mod) -> Optional[INode]:
        name = node.get("Name")
        parent_name = node.get("ParentName")
        may = node.get("MayRequire")
        if (name is None and parent_name is None) or (may is not None and not self.active.all_mods_active_no_suffix(may.split(","))):
            return None
        lst = None
        if name is not None:
            lst = self.by_name.get(name)
            if lst:
                for other in lst:
                    if other.mod is mod:
                        self.diag.add("error", "inherit_duplicate_name",
                                      'Could not register node named "%s" in mod %s because this name is already used in this mod.'
                                      % (name, getattr(mod, "package_id", None)),
                                      getattr(mod, "package_id", None))
                        return None
        inode = INode(node, mod)
        self.unresolved.append(inode)
        if name is not None:
            if lst is not None:
                lst.append(inode)
            else:
                self.by_name[name] = [inode]
        return inode

    # -------------------------------------------------------------- resolution
    def resolve(self) -> None:
        self._link_parents()
        self._resolve_nodes()

    def _best_parent(self, node: INode, parent_name: str) -> Optional[INode]:
        cands = self.by_name.get(parent_name)
        best: Optional[INode] = None
        if cands:
            if node.mod is None:
                for c in cands:
                    if c.mod is None:
                        best = c
                        break
                if best is None:
                    for c in cands:
                        if best is None or c.mod.index < best.mod.index:
                            best = c
            else:
                for c in cands:
                    if c.mod is not None and c.mod.index <= node.mod.index and (best is None or c.mod.index > best.mod.index):
                        best = c
                if best is None:
                    for c in cands:
                        if c.mod is None:
                            best = c
                            break
        if best is None:
            self.diag.add("error", "inherit_missing_parent",
                          'Could not find parent node named "%s" for node "%s".' % (parent_name, node.xml.tag),
                          getattr(node.mod, "package_id", None))
        return best

    def _link_parents(self) -> None:
        for n in self.unresolved:
            pn = n.xml.get("ParentName")
            if pn is not None:
                n.parent = self._best_parent(n, pn)
                if n.parent is not None:
                    n.parent.children.append(n)

    def _resolve_nodes(self) -> None:
        roots = [n for n in self.unresolved if n.parent is None or n.parent.resolved is not None]
        for r in roots:
            self._resolve_recursively(r)
        for n in self.unresolved:
            if n.resolved is None:
                self.diag.add("error", "inherit_cycle", 'Cyclic inheritance hierarchy detected for node "%s".' % n.xml.tag,
                              getattr(n.mod, "package_id", None))
            else:
                self.resolved_nodes[n.xml] = n
        self.unresolved = []

    def _resolve_recursively(self, node: INode) -> None:
        # iterative depth-first walk (the game recurses; inheritance chains are short but cycles are possible)
        stack = [node]
        while stack:
            n = stack.pop()
            if n.resolved is not None:
                self.diag.add("error", "inherit_cycle", 'Cyclic inheritance hierarchy detected for node "%s".' % n.xml.tag)
                continue
            self._resolve_one(n)
            for ch in reversed(n.children):
                stack.append(ch)

    def _resolve_one(self, n: INode) -> None:
        if n.parent is None:
            n.resolved = n.xml
            return
        self._check_duplicates(n.xml, n.xml, getattr(n.mod, "package_id", None), _def_label(n.xml))
        cur = copy.deepcopy(n.parent.resolved)
        cur.tail = None
        self.merge(n.xml, cur)
        n.resolved = cur

    # -------------------------------------------------------------- lookups
    def resolved_for(self, node: Element) -> Element:
        """GetResolvedNodeFor: nodes with a ParentName come from the resolved table, others are themselves."""
        if node.get("ParentName") is not None:
            r = self.resolved_nodes.get(node)
            if r is not None:
                return r.resolved
            self.diag.add("error", "inherit_not_resolved",
                          'Tried to get resolved node for node "%s" which uses a ParentName attribute, but it was never '
                          "registered or failed to resolve." % node.tag)
        return node

    def parent_chain(self, node: Element) -> List[INode]:
        out = []
        n = self.resolved_nodes.get(node)
        seen = set()
        while n is not None and n.parent is not None and id(n.parent) not in seen:
            seen.add(id(n.parent))
            out.append(n.parent)
            n = n.parent
        return out

    # -------------------------------------------------------------- the merge
    def _is_list_element(self, node: Element) -> bool:
        if node.tag == "li":
            return True
        p = node.getparent()
        return p is not None and p.tag in self.allow

    def _check_duplicates(self, node: Element, root: Element, mod_id=None, label="") -> None:
        used: Set[str] = set()
        for c in child_elements(node):
            if not self._is_list_element(c) and c.tag in used:
                self.diag.add("error", "inherit_duplicate_node_name", "Duplicate XML node name %s in %s of %s" % (c.tag, node.tag, label), mod_id)
            used.add(c.tag)
            self._check_duplicates(c, root, mod_id, label)

    def merge(self, child: Element, current: Element) -> None:
        """RecursiveNodeCopyOverwriteElements(child, current)."""
        inherit = child.get("Inherit")
        if inherit is not None and inherit.lower() == "false":
            clear_children(current)
            for it in child_nodes(child):
                dom_append(current, import_node(it))
            for k, v in child.attrib.items():
                if k != "Inherit":
                    set_attribute(current, k, v)
            return
        current.attrib.clear()
        for k, v in child.attrib.items():
            current.set(k, v)
        text_node = last_text_child(child)
        has_elem = any(True for _ in child_elements(child))
        if text_node is not None:
            _remove_first_child_node(current)             # live-list quirk: only the first node goes
            dom_append(current, import_node(text_node))
            return
        if not has_elem:
            if any(True for _ in child_elements(current)):
                return
            _remove_first_child_node(current)
            return
        for item in child_elements(child):
            if self._is_list_element(item):
                dom_append(current, import_node(item))
                continue
            target = first_child_named(current, item.tag)
            if target is not None:
                self.merge(item, target)
            else:
                dom_append(current, import_node(item))


def _remove_first_child_node(el: Element) -> None:
    """Remove only the first child node (text or element) of `el`; attributes are untouched."""
    nodes = child_nodes(el)
    if not nodes:
        return
    first = nodes[0]
    if isinstance(first, TextRef):
        first.set(None)
        return
    # first child is an element: drop it, keep whatever text followed it in place
    tail = first.tail
    prev = first.getprevious()
    el.remove(first)
    if tail:
        el.text = (el.text or "") + tail


def _def_label(node: Element) -> str:
    d = first_child_named(node, "defName")
    return "%s %s" % (node.tag, inner_text(d) if d is not None else "(no defName: Name=%s)" % node.get("Name"))
