"""DOM layer that reproduces the node semantics the game relies on (System.Xml.XmlDocument).

RimWorld loads every XML file with an XmlReader configured to drop comments and
whitespace-only text, then manipulates the result through the XmlDocument API.
lxml is the tree used here, but its text model differs: character data hangs off
`.text` / `.tail` instead of being separate sibling nodes. This module hides that
difference behind a handful of operations that behave like the DOM calls the game
makes (AppendChild, PrependChild, InsertBefore, InsertAfter, RemoveChild, ImportNode).

Text nodes are addressed with `TextRef(owner, slot)` where slot is "text" (the text
node before the first child of `owner`) or "tail" (the text node after `owner`).

Known divergences from XmlDocument (documented in the semantics note):
  * lxml merges adjacent text nodes; .NET keeps them separate until Normalize().
  * CDATA sections are read as plain text (see `parse_xml_bytes`).
  * Processing instructions are dropped.
"""
from __future__ import annotations

import copy
from typing import Iterable, List, Optional, Union

from lxml import etree

XML_WS = " \t\r\n"

Element = etree._Element


class XmlParseError(Exception):
    """The file could not be parsed (the game logs a warning and skips the asset)."""


class TextRef:
    """A text node, addressed as (owner element, 'text' | 'tail')."""

    __slots__ = ("owner", "slot")

    def __init__(self, owner: Element, slot: str):
        self.owner = owner
        self.slot = slot

    @property
    def value(self) -> str:
        return (self.owner.text if self.slot == "text" else self.owner.tail) or ""

    def set(self, s: Optional[str]) -> None:
        if self.slot == "text":
            self.owner.text = s
        else:
            self.owner.tail = s

    def dom_parent(self) -> Optional[Element]:
        return self.owner if self.slot == "text" else self.owner.getparent()

    def __eq__(self, other):
        return isinstance(other, TextRef) and other.owner is self.owner and other.slot == self.slot

    def __hash__(self):
        return hash((id(self.owner), self.slot))

    def __repr__(self):
        return "TextRef(%s, %s, %r)" % (self.owner.tag, self.slot, self.value[:20])


class AttrRef:
    """An attribute node selected by an XPath expression (its DOM ParentNode is null)."""

    __slots__ = ("owner", "name")

    def __init__(self, owner: Element, name: str):
        self.owner = owner
        self.name = name

    def __repr__(self):
        return "AttrRef(%s@%s)" % (self.owner.tag, self.name)


Node = Union[Element, TextRef]


# ---------------------------------------------------------------------------- loading

def parse_xml_bytes(data: bytes) -> Element:
    """Parse one asset the way LoadableXmlAsset does.

    * A UTF-8 byte order mark is stripped, the bytes are decoded as UTF-8 (invalid
      sequences become U+FFFD) and the declared encoding is ignored.
    * Comments are dropped (IgnoreComments) and whitespace-only text is dropped
      (IgnoreWhitespace), except below an element that sets xml:space="preserve".
    * DTDs are prohibited (the .NET default), so a DOCTYPE is a parse error.
    """
    if data[:3] == b"\xef\xbb\xbf":
        data = data[3:]
    text = data.decode("utf-8", errors="replace")
    if "<!DOCTYPE" in text:
        raise XmlParseError("DTD is prohibited")
    parser = etree.XMLParser(
        encoding="utf-8",
        remove_comments=True,
        remove_pis=True,
        resolve_entities=False,
        load_dtd=False,
        no_network=True,
        huge_tree=True,
        strip_cdata=True,
    )
    try:
        root = etree.fromstring(text.encode("utf-8"), parser=parser)
    except etree.XMLSyntaxError as exc:
        raise XmlParseError(str(exc)) from exc
    if root is None:
        raise XmlParseError("no document element")
    drop_ignorable_whitespace(root)
    return root


def drop_ignorable_whitespace(root: Element) -> None:
    """Remove text nodes that consist only of XML whitespace (outside xml:space=preserve)."""
    XMLSPACE = "{http://www.w3.org/XML/1998/namespace}space"

    def walk(el: Element, preserve: bool) -> None:
        sp = el.get(XMLSPACE)
        if sp == "preserve":
            preserve = True
        elif sp == "default":
            preserve = False
        if not preserve and el.text is not None and not el.text.strip(XML_WS):
            el.text = None
        for child in el:
            walk(child, preserve)
            if not preserve and child.tail is not None and not child.tail.strip(XML_WS):
                child.tail = None

    walk(root, False)


# ---------------------------------------------------------------------------- reading

def is_element(x) -> bool:
    return isinstance(x, etree._Element) and isinstance(x.tag, str)


def child_nodes(el: Element) -> List[Node]:
    """ChildNodes of an element: element nodes and text nodes in document order."""
    out: List[Node] = []
    if el.text:
        out.append(TextRef(el, "text"))
    for c in el:
        if not isinstance(c.tag, str):
            continue
        out.append(c)
        if c.tail:
            out.append(TextRef(c, "tail"))
    return out


def child_elements(el: Element) -> List[Element]:
    return [c for c in el if isinstance(c.tag, str)]


def first_child_named(el: Element, name: str) -> Optional[Element]:
    """XmlNode[name]: the first child element whose Name equals `name`."""
    for c in el:
        if c.tag == name:
            return c
    return None


def has_child_nodes(el: Element) -> bool:
    return bool(el.text) or len(el) > 0


def last_text_child(el: Element) -> Optional[TextRef]:
    """The last Text child node of `el` (None if there is none)."""
    last = None
    if el.text:
        last = TextRef(el, "text")
    for c in el:
        if c.tail:
            last = TextRef(c, "tail")
    return last


def inner_text(el: Element) -> str:
    """XmlNode.InnerText: concatenation of all descendant text."""
    return "".join(el.itertext())


def inner_xml(el: Element) -> str:
    parts = []
    if el.text:
        parts.append(_escape_text(el.text))
    for c in el:
        s = etree.tostring(c, encoding="unicode", with_tail=False)
        parts.append(s)
        if c.tail:
            parts.append(_escape_text(c.tail))
    return "".join(parts)


def _escape_text(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


# ---------------------------------------------------------------------------- writing

def import_node(node: Union[Element, TextRef, str]) -> Union[Element, str]:
    """XmlDocument.ImportNode(node, deep: true): detached deep copy (no tail)."""
    if isinstance(node, TextRef):
        return node.value
    if isinstance(node, str):
        return node
    c = copy.deepcopy(node)
    c.tail = None
    return c


def import_children(container: Element) -> List[Union[Element, str]]:
    """Imported copies of every child node of `container` (used for patch <value>)."""
    return [import_node(n) for n in child_nodes(container)]


def dom_append(parent: Element, item: Union[Element, str]) -> None:
    """AppendChild."""
    if isinstance(item, str):
        if len(parent):
            last = parent[-1]
            last.tail = (last.tail or "") + item
        else:
            parent.text = (parent.text or "") + item
    else:
        parent.append(item)


def dom_prepend(parent: Element, item: Union[Element, str]) -> None:
    """PrependChild: the node becomes the very first child (before any leading text)."""
    if isinstance(item, str):
        parent.text = item + (parent.text or "")
    else:
        lead = parent.text
        parent.text = None
        parent.insert(0, item)
        item.tail = lead


def dom_insert_before(ref: Node, item: Union[Element, str]) -> None:
    """InsertBefore(newChild, refChild)."""
    if isinstance(ref, TextRef):
        if ref.slot == "text":
            dom_prepend(ref.owner, item)
        else:
            dom_insert_after(ref.owner, item)
        return
    if isinstance(item, str):
        prev = ref.getprevious()
        if prev is not None:
            prev.tail = (prev.tail or "") + item
        else:
            parent = ref.getparent()
            parent.text = (parent.text or "") + item
    else:
        ref.addprevious(item)


def dom_insert_after(ref: Node, item: Union[Element, str]) -> None:
    """InsertAfter(newChild, refChild): the node lands directly after `ref`."""
    if isinstance(ref, TextRef):
        if ref.slot == "text":
            owner = ref.owner
            if isinstance(item, str):
                owner.text = (owner.text or "") + item
            else:
                owner.insert(0, item)
        else:
            owner = ref.owner
            if isinstance(item, str):
                owner.tail = (owner.tail or "") + item
            else:
                # lxml's addnext places the element after the owner's tail text, which is
                # exactly "directly after that text node".
                owner.addnext(item)
        return
    if isinstance(item, str):
        ref.tail = item + (ref.tail or "")
    else:
        old_tail = ref.tail
        ref.addnext(item)
        ref.tail = None
        item.tail = old_tail


def dom_remove(node: Node) -> None:
    """RemoveChild: detach the node; its following text node stays in place."""
    if isinstance(node, TextRef):
        node.set(None)
        return
    parent = node.getparent()
    if parent is None:
        raise ValueError("cannot remove the document element")
    tail = node.tail
    prev = node.getprevious()
    parent.remove(node)
    if tail:
        if prev is not None:
            prev.tail = (prev.tail or "") + tail
        else:
            parent.text = (parent.text or "") + tail


def clear_children(el: Element) -> None:
    """Remove every child node (elements and text) of `el`; attributes are kept."""
    for c in list(el):
        el.remove(c)
    el.text = None


def set_attribute(el: Element, name: str, value: str) -> None:
    """XmlAttributeCollection.Append semantics: a same-named attribute is replaced and the
    new one lands at the end of the attribute list."""
    if name in el.attrib:
        del el.attrib[name]
    el.set(name, value)


def replace_attributes(el: Element, attrs: Iterable) -> None:
    el.attrib.clear()
    for k, v in attrs:
        el.set(k, v)


# ---------------------------------------------------------------------------- canonical form

def to_canonical(el: Element) -> dict:
    """JSON-friendly canonical form used for golden files and differential comparison.

    {"tag": str, "attrs": [[name, value], ...], "children": [ node, ... ]}
    where a text node is the plain string. Adjacent text is merged (see module note).
    """
    children: list = []
    if el.text:
        children.append(el.text)
    for c in el:
        if not isinstance(c.tag, str):
            continue
        children.append(to_canonical(c))
        if c.tail:
            children.append(c.tail)
    return {"tag": el.tag, "attrs": [[k, v] for k, v in el.attrib.items()], "children": children}


def canonical_to_xml(node, indent: int = 0) -> str:
    """Pretty text rendering of a canonical node (debugging aid, not used for comparison)."""
    pad = "  " * indent
    if isinstance(node, str):
        return pad + _escape_text(node)
    attrs = "".join(' %s="%s"' % (k, v.replace("&", "&amp;").replace('"', "&quot;")) for k, v in node["attrs"])
    kids = node["children"]
    if not kids:
        return "%s<%s%s />" % (pad, node["tag"], attrs)
    if len(kids) == 1 and isinstance(kids[0], str):
        return "%s<%s%s>%s</%s>" % (pad, node["tag"], attrs, _escape_text(kids[0]), node["tag"])
    inner = "\n".join(canonical_to_xml(k, indent + 1) for k in kids)
    return "%s<%s%s>\n%s\n%s</%s>" % (pad, node["tag"], attrs, inner, pad, node["tag"])


def serialize(el: Element, pretty: bool = True) -> str:
    return canonical_to_xml(to_canonical(el)) if pretty else etree.tostring(el, encoding="unicode", with_tail=False)
