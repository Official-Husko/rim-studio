"""MayRequire / MayRequireAnyOf outside the XML merge: list items inside a def.

The game honours these attributes at four places:
  1. top-level nodes: registration for inheritance (MayRequire only) and def creation (both);
     implemented in engine.py / inherit.py;
  2. `li` items of any list while the def is deserialised (this module);
  3. patch lists (`operations`, `mods`): implemented in patches.py;
  4. fields whose C# type is a Def (cross references): an unmet requirement silently drops the reference.
     That needs field types and is out of scope for an XML level engine (see prune_fields).
Any other element carrying MayRequire is NOT filtered by the game: the attribute is simply ignored.
"""
from __future__ import annotations

from .mods import ActiveSet
from .xmlnet import Element, child_elements


def li_passes(li: Element, active: ActiveSet) -> bool:
    mr = li.get("MayRequire")
    if mr is not None and mr != "" and not active.all_mods_active_no_suffix(mr.split(",")):
        return False
    any_of = li.get("MayRequireAnyOf")
    if any_of is not None and any_of != "" and not active.any_mod_active_no_suffix(any_of.lower().split(",")):
        return False
    return True


def prune_li(node: Element, active: ActiveSet) -> int:
    """Remove every `li` below `node` whose MayRequire / MayRequireAnyOf is not met. Returns the count.

    Works on a resolved def node (after inheritance), like the game, which evaluates the attributes when it
    reads the merged node. A removed li takes its subtree with it; nested li are only inspected when their
    ancestors survive.
    """
    removed = 0
    for c in list(child_elements(node)):
        if c.tag == "li" and not li_passes(c, active):
            _drop(c)
            removed += 1
        else:
            removed += prune_li(c, active)
    return removed


def _drop(el: Element) -> None:
    parent = el.getparent()
    tail = el.tail
    prev = el.getprevious()
    parent.remove(el)
    if tail:
        if prev is not None:
            prev.tail = (prev.tail or "") + tail
        else:
            parent.text = (parent.text or "") + tail
