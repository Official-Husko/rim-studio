"""PatchOperation classes: parsing (Class attribute) and application on the unified document.

Mirrors Verse/PatchOperation*.cs. The document is an lxml tree whose root element is `Defs`;
the DOM helpers in xmlnet.py reproduce the XmlDocument calls the operations make.

Semantics worth knowing (all verified against the decompiled code, see the semantics note):
  * `success` (Normal / Invert / Always / Never) post-processes the result of every operation.
  * An operation whose class cannot be found is the BASE PatchOperation: it logs an error and
    returns false, but `success` still applies (an unknown class with success=Always returns true).
  * A missing `<value>` is a NullReferenceException in the game: the exception propagates through
    enclosing Sequence/Conditional/FindMod operations up to the top-level Apply, which logs it.
    Here that is `PatchException`; the engine counts it as `patch_exception`.
  * XPath node-sets are evaluated eagerly here; .NET iterates lazily (see xpath_net.py).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional

from .diag import Diagnostics
from .mods import ActiveSet
from .xmlnet import (AttrRef, Element, TextRef, child_elements, child_nodes, dom_append, dom_insert_after,
                     dom_insert_before, dom_prepend, dom_remove, first_child_named, import_children,
                     inner_text, is_element, set_attribute)
from . import xpath_net
from .xpath_net import XPathError


class PatchException(Exception):
    """An exception that the game would let escape from ApplyWorker (caught in ApplyPatches)."""


KNOWN_CLASSES = [
    "PatchOperation", "PatchOperationAdd", "PatchOperationAddModExtension", "PatchOperationAttributeAdd",
    "PatchOperationAttributeRemove", "PatchOperationAttributeSet", "PatchOperationConditional",
    "PatchOperationFindMod", "PatchOperationInsert", "PatchOperationRemove", "PatchOperationReplace",
    "PatchOperationSequence", "PatchOperationSetName", "PatchOperationTest",
]
# Type lookup is case-insensitive for names that go through Assembly.GetType(name, ignoreCase: true),
# which includes "Verse." + name; the exact short-name index is tried first and is case-sensitive.
_KNOWN_LOWER: Dict[str, str] = {}
for _n in KNOWN_CLASSES:
    _KNOWN_LOWER[_n.lower()] = _n
    _KNOWN_LOWER["verse." + _n.lower()] = _n

SUCCESS_VALUES = ("Normal", "Invert", "Always", "Never")


@dataclass
class Ctx:
    """Everything an operation may consult or record while applying."""

    active: ActiveSet
    diag: Diagnostics
    touched: List[Element] = field(default_factory=list)       # nodes mutated by successful steps
    custom: Dict[str, Callable] = field(default_factory=dict)  # lower-case full class name -> factory
    mod: Optional[str] = None
    file: Optional[str] = None


class Op:
    """Base PatchOperation. Behaves like the game's base class: it always fails."""

    class_name = "PatchOperation"
    xpath: Optional[str] = None

    def __init__(self):
        self.success = "Normal"
        self.source_file: Optional[str] = None
        self.declared_class: Optional[str] = None
        self.unknown = False
        self.raw: Optional[Element] = None        # the <Operation> element (kept for operations the engine cannot run)

    # ---- the game's Apply wrapper
    def apply(self, root: Element, ctx: Ctx) -> bool:
        flag = self.apply_worker(root, ctx)
        if self.success == "Always":
            flag = True
        elif self.success == "Never":
            flag = False
        elif self.success == "Invert":
            flag = not flag
        return flag

    def apply_worker(self, root: Element, ctx: Ctx) -> bool:
        ctx.diag.add("error", "patch_base_class",
                     "Attempted to use PatchOperation directly; patch will always fail (class %r)" % self.declared_class,
                     ctx.mod, ctx.file)
        return False

    def describe(self) -> str:
        return self.class_name + ("(%s)" % " ".join(self.xpath.split()) if self.xpath else "")


class Pathed(Op):
    def select(self, root: Element, ctx: Ctx) -> list:
        try:
            return xpath_net.select_nodes(root, self.xpath)
        except XPathError as exc:
            raise PatchException("XPath error in %r: %s" % (self.xpath, exc))


def _need_value(op) -> Element:
    if op.value is None:
        raise PatchException("NullReferenceException: <value> is missing")
    return op.value


class Add(Pathed):
    class_name = "PatchOperationAdd"

    def __init__(self):
        super().__init__()
        self.value: Optional[Element] = None
        self.order = "Append"

    def apply_worker(self, root, ctx):
        value = _need_value(self)
        result = False
        for target in self.select(root, ctx):
            result = True
            if isinstance(target, AttrRef):
                # an XmlAttribute may contain text children only
                kids = child_nodes(value)
                if any(is_element(k) for k in kids):
                    raise PatchException("InvalidOperationException: attribute cannot contain elements")
                text = "".join(k.value for k in kids)
                cur = target.owner.get(target.name) or ""
                target.owner.set(target.name, cur + text if self.order == "Append" else text + cur)
                ctx.touched.append(target.owner)
                continue
            if isinstance(target, TextRef):
                raise PatchException("InvalidOperationException: text node cannot contain children")
            items = import_children(value)
            if self.order == "Append":
                for it in items:
                    dom_append(target, it)
            elif self.order == "Prepend":
                for it in reversed(items):
                    dom_prepend(target, it)
            ctx.touched.append(target)
        return result


class Insert(Pathed):
    class_name = "PatchOperationInsert"

    def __init__(self):
        super().__init__()
        self.value: Optional[Element] = None
        self.order = "Prepend"

    def apply_worker(self, root, ctx):
        value = _need_value(self)
        result = False
        for target in self.select(root, ctx):
            result = True
            if isinstance(target, AttrRef):
                raise PatchException("NullReferenceException: attribute has no parent node")
            items = import_children(value)
            if self.order == "Append":
                # every child is inserted directly after the target, so the copies end up REVERSED
                for it in items:
                    dom_insert_after(target, it)
            elif self.order == "Prepend":
                for it in reversed(items):
                    dom_insert_before(target, it)
            ctx.touched.append(target if is_element(target) else target.dom_parent())
        return result


class Replace(Pathed):
    class_name = "PatchOperationReplace"

    def __init__(self):
        super().__init__()
        self.value: Optional[Element] = None

    def apply_worker(self, root, ctx):
        value = _need_value(self)
        result = False
        targets = self.select(root, ctx)             # ToArray(): a snapshot
        for target in targets:
            result = True
            if isinstance(target, AttrRef):
                raise PatchException("NullReferenceException: attribute has no parent node")
            parent = target.getparent() if is_element(target) else target.dom_parent()
            if parent is None:
                raise PatchException("cannot replace the document element")
            items = import_children(value)
            ctx.touched.append(parent)
            if isinstance(target, TextRef):
                # a text node is addressed by slot, so detach it first and put the copies in its place
                target.set(None)
                for it in reversed(items):
                    if target.slot == "text":
                        dom_prepend(target.owner, it)
                    else:
                        dom_insert_after(target.owner, it)
                continue
            for it in items:
                dom_insert_before(target, it)
            dom_remove(target)
        return result


class Remove(Pathed):
    class_name = "PatchOperationRemove"

    def apply_worker(self, root, ctx):
        result = False
        for target in self.select(root, ctx):
            result = True
            if isinstance(target, AttrRef):
                raise PatchException("NullReferenceException: attribute has no parent node")
            parent = target.getparent() if is_element(target) else target.dom_parent()
            if parent is None:
                raise PatchException("cannot remove the document element")
            ctx.touched.append(parent)
            dom_remove(target)
        return result


class AddModExtension(Pathed):
    class_name = "PatchOperationAddModExtension"

    def __init__(self):
        super().__init__()
        self.value: Optional[Element] = None

    def apply_worker(self, root, ctx):
        value = _need_value(self)
        result = False
        for target in self.select(root, ctx):
            if not is_element(target):
                raise PatchException("NullReferenceException: target is not an element")
            ext = first_child_named(target, "modExtensions")
            if ext is None:
                ext = target.makeelement("modExtensions", {})
                dom_append(target, ext)
            for it in import_children(value):
                dom_append(ext, it)
            ctx.touched.append(target)
            result = True
        return result


class AttributeOp(Pathed):
    def __init__(self):
        super().__init__()
        self.attribute: Optional[str] = None
        self.value: Optional[str] = None

    def describe(self):
        return "%s(%s)(%s)" % (self.class_name, " ".join((self.xpath or "").split()), self.attribute)

    def _elements(self, root, ctx):
        for t in self.select(root, ctx):
            if not is_element(t):
                raise PatchException("NullReferenceException: node has no attributes")
            yield t


class AttributeAdd(AttributeOp):
    class_name = "PatchOperationAttributeAdd"

    def apply_worker(self, root, ctx):
        result = False
        for el in self._elements(root, ctx):
            if self.attribute not in el.attrib:
                set_attribute(el, self.attribute, self.value if self.value is not None else "")
                ctx.touched.append(el)
                result = True
        return result


class AttributeRemove(AttributeOp):
    class_name = "PatchOperationAttributeRemove"

    def apply_worker(self, root, ctx):
        result = False
        for el in self._elements(root, ctx):
            if self.attribute in el.attrib:
                del el.attrib[self.attribute]
                ctx.touched.append(el)
                result = True
        return result


class AttributeSet(AttributeOp):
    class_name = "PatchOperationAttributeSet"

    def apply_worker(self, root, ctx):
        result = False
        for el in self._elements(root, ctx):
            val = self.value if self.value is not None else ""
            if self.attribute in el.attrib:
                el.set(self.attribute, val)           # same slot: attribute order is unchanged
            else:
                set_attribute(el, self.attribute, val)
            ctx.touched.append(el)
            result = True
        return result


class SetName(Pathed):
    class_name = "PatchOperationSetName"

    def __init__(self):
        super().__init__()
        self.name: Optional[str] = None

    def apply_worker(self, root, ctx):
        result = False
        for target in self.select(root, ctx):
            result = True
            if not is_element(target):
                raise PatchException("node is not an element")
            parent = target.getparent()
            if parent is None:
                raise PatchException("cannot rename the document element")
            new = target.makeelement(self.name, {})          # attributes are NOT carried over
            new.text = target.text
            for c in list(target):
                new.append(c)                                # moves the child together with its tail text
            dom_insert_before(target, new)
            ctx.touched.append(parent)
            dom_remove(target)
        return result


class Test(Pathed):
    class_name = "PatchOperationTest"

    def apply_worker(self, root, ctx):
        try:
            return xpath_net.select_single_node(root, self.xpath) is not None
        except XPathError as exc:
            raise PatchException(str(exc))


class Conditional(Pathed):
    class_name = "PatchOperationConditional"

    def __init__(self):
        super().__init__()
        self.match: Optional[Op] = None
        self.nomatch: Optional[Op] = None

    def apply_worker(self, root, ctx):
        try:
            found = xpath_net.select_single_node(root, self.xpath) is not None
        except XPathError as exc:
            raise PatchException(str(exc))
        if found:
            if self.match is not None:
                return self.match.apply(root, ctx)
        elif self.nomatch is not None:
            return self.nomatch.apply(root, ctx)
        if self.match is None:
            return self.nomatch is not None
        return True


class FindMod(Op):
    class_name = "PatchOperationFindMod"

    def __init__(self):
        super().__init__()
        self.mods: List[str] = []
        self.match: Optional[Op] = None
        self.nomatch: Optional[Op] = None

    def describe(self):
        return "%s(%s)" % (self.class_name, ", ".join(self.mods))

    def apply_worker(self, root, ctx):
        flag = any(ctx.active.has_active_mod_with_name(m) for m in self.mods)   # exact, case-sensitive Name
        if flag:
            if self.match is not None:
                return self.match.apply(root, ctx)
        elif self.nomatch is not None:
            return self.nomatch.apply(root, ctx)
        return True


class Sequence(Op):
    class_name = "PatchOperationSequence"

    def __init__(self):
        super().__init__()
        self.operations: List[Op] = []
        self.has_operations = False

    def apply_worker(self, root, ctx):
        if not self.has_operations:
            raise PatchException("NullReferenceException: operations is missing")
        for op in self.operations:
            if not op.apply(root, ctx):
                return False
        return True


CLASSES = {
    "PatchOperation": Op, "PatchOperationAdd": Add, "PatchOperationAddModExtension": AddModExtension,
    "PatchOperationAttributeAdd": AttributeAdd, "PatchOperationAttributeRemove": AttributeRemove,
    "PatchOperationAttributeSet": AttributeSet, "PatchOperationConditional": Conditional,
    "PatchOperationFindMod": FindMod, "PatchOperationInsert": Insert, "PatchOperationRemove": Remove,
    "PatchOperationReplace": Replace, "PatchOperationSequence": Sequence, "PatchOperationSetName": SetName,
    "PatchOperationTest": Test,
}


# ---------------------------------------------------------------------------- parsing

def _text_of(el: Element) -> str:
    """String field: the inner text (the game keeps inner XML when the node is not a single text)."""
    return inner_text(el)


def _list_items(el: Element, ctx: Ctx):
    """ListFromXml for non-Def element types: `li` entries honouring MayRequire / MayRequireAnyOf."""
    for c in child_elements(el):
        if c.tag != "li":
            ctx.diag.add("error", "patch_list_item_not_li", "List item found with name %s" % c.tag, ctx.mod, ctx.file)
            continue
        mr = c.get("MayRequire")
        if mr is not None and mr != "":
            if not ctx.active.all_mods_active_no_suffix(mr.split(",")):
                continue
        else:
            any_of = c.get("MayRequireAnyOf")
            if any_of is not None and any_of != "" and not ctx.active.any_mod_active_no_suffix(any_of.split(",")):
                continue
        yield c


def resolve_class(name: Optional[str], ctx: Ctx):
    """Map the Class attribute to (kind, canonical name, factory). kind: known | custom | unknown."""
    if name is None:
        return "known", "PatchOperation", Op
    low = name.lower()
    canon = _KNOWN_LOWER.get(low)
    if canon is not None:
        return "known", canon, CLASSES[canon]
    custom = ctx.custom.get(low)
    if custom is not None:
        return "custom", name, custom
    return "unknown", name, Op


def parse_operation(el: Element, ctx: Ctx) -> Op:
    kind, canon, factory = resolve_class(el.get("Class"), ctx)
    op = factory() if kind != "custom" else factory()
    op.declared_class = el.get("Class")
    op.raw = el
    if kind == "unknown":
        op.unknown = True
        ctx.diag.add("warning", "patch_unknown_class",
                     "Could not find type named %s: the operation becomes the base PatchOperation (always fails "
                     "unless success=Always/Invert)" % el.get("Class"), ctx.mod, ctx.file)
    seen = set()
    for c in child_elements(el):
        name = c.tag
        if name in seen:
            ctx.diag.add("error", "patch_duplicate_field", "defines the same field twice: %s" % name, ctx.mod, ctx.file)
        seen.add(name)
        field_name = _find_field(op, name, ctx)
        if field_name is None:
            ctx.diag.add("error" if not op.unknown else "info", "patch_unknown_field",
                         "%s has no field %r" % (op.class_name, name), ctx.mod, ctx.file)
            continue
        try:
            _set_field(op, field_name, c, ctx)
        except ValueError as exc:
            ctx.diag.add("error", "patch_field_parse_error", "%s.%s: %s" % (op.class_name, field_name, exc),
                         ctx.mod, ctx.file)
    return op


_FIELDS = {
    "Op": ["success"],
    "Add": ["success", "xpath", "value", "order"], "Insert": ["success", "xpath", "value", "order"],
    "Replace": ["success", "xpath", "value"], "Remove": ["success", "xpath"], "Test": ["success", "xpath"],
    "AddModExtension": ["success", "xpath", "value"],
    "AttributeAdd": ["success", "xpath", "attribute", "value"], "AttributeSet": ["success", "xpath", "attribute", "value"],
    "AttributeRemove": ["success", "xpath", "attribute"],
    "SetName": ["success", "xpath", "name"], "Conditional": ["success", "xpath", "match", "nomatch"],
    "FindMod": ["success", "mods", "match", "nomatch"], "Sequence": ["success", "operations"],
}


def _fields_of(op: Op) -> List[str]:
    if getattr(op, "custom_fields", None) is not None:
        return op.custom_fields
    return _FIELDS.get(type(op).__name__, ["success"])


def _find_field(op: Op, tag: str, ctx: Ctx) -> Optional[str]:
    fields = _fields_of(op)
    if tag in fields:
        return tag
    for f in fields:                                           # SearchTypeHierarchy(ignoreCase: true)
        if f.lower() == tag.lower():
            ctx.diag.add("error", "patch_field_case_mismatch", "xml tags are case-sensitive: %s" % tag,
                         ctx.mod, ctx.file)
            return f
    return None


def _set_field(op: Op, field_name: str, el: Element, ctx: Ctx) -> None:
    if field_name == "success":
        txt = _text_of(el)
        if txt not in SUCCESS_VALUES:
            raise ValueError("%r is not a valid value for Success" % txt)
        op.success = txt
    elif field_name == "order":
        txt = _text_of(el)
        if txt not in ("Append", "Prepend"):
            raise ValueError("%r is not a valid value for Order" % txt)
        op.order = txt
    elif field_name == "value" and isinstance(op, (Add, Insert, Replace, AddModExtension)):
        op.value = el                                          # XmlContainer: the <value> node itself
    elif field_name in ("xpath", "attribute", "name", "value"):
        setattr(op, field_name, _text_of(el))
    elif field_name in ("match", "nomatch"):
        setattr(op, field_name, parse_operation(el, ctx))
    elif field_name == "mods":
        op.mods = [inner_text(li) for li in _list_items(el, ctx)]
    elif field_name == "operations":
        op.has_operations = True
        op.operations = [parse_operation(li, ctx) for li in _list_items(el, ctx)]
    else:                                                      # custom operation fields
        h = getattr(op, "set_field", None)
        if h is None:
            raise ValueError("unsupported field")
        h(field_name, el, ctx)


def load_patch_file(root: Element, ctx: Ctx, source: str) -> List[Op]:
    """ModContentPack.LoadPatches for one file: <Patch><Operation Class="..."> ... </Operation></Patch>."""
    ops: List[Op] = []
    if root.tag != "Patch":
        ctx.diag.add("error", "patch_bad_root", "Unexpected document element in patch XML; got %s, expected 'Patch'" % root.tag,
                     ctx.mod, ctx.file)
        return ops
    for c in child_elements(root):
        if c.tag != "Operation":
            ctx.diag.add("error", "patch_bad_element", "Unexpected element in patch XML; got %s, expected 'Operation'" % c.tag,
                         ctx.mod, ctx.file)
            continue
        op = parse_operation(c, ctx)
        op.source_file = source
        ops.append(op)
    return ops
