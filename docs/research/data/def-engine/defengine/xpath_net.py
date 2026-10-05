"""XPath 1.0 evaluation with the context rules of XmlNode.SelectNodes.

The game evaluates every patch XPath with `xmlDoc.SelectNodes(xpath)`, so the context
node is the DOCUMENT node: the relative expression `Defs/ThingDef[...]` is the same as
`/Defs/ThingDef[...]`. lxml evaluates relative expressions against the root ELEMENT,
where `Defs/ThingDef` would look for a nested Defs element and match nothing. Most
real patches (1654 of 1674 CE root patches) use the relative form, so this matters.

The fix is a lexical rewrite: a location path that starts at predicate depth 0 and is
relative gets a leading "/". Paths inside predicates keep their meaning (they are
relative to the predicate's context node) and are left alone. The tokenizer follows
the disambiguation rules of XPath 1.0 section 3.7.

Behaviour that is NOT reproduced: .NET walks the node list lazily while the patch
mutates the tree, eager evaluation is used here (see the semantics note).
"""
from __future__ import annotations

import re
from typing import Dict, List, Tuple

from lxml import etree

from .xmlnet import AttrRef, Element, TextRef


class XPathError(Exception):
    """Syntax error, or an expression that does not evaluate to a node-set
    (the game gets an XPathException and logs 'Error in patch.Apply()')."""


_NC = r"[^\W\d][\w.\-·]*"
_TOKEN_RE = re.compile(
    r"""
    (?P<ws>\s+)
  | (?P<lit>"[^"]*"|'[^']*')
  | (?P<num>\d+(?:\.\d*)?|\.\d+)
  | (?P<dotdot>\.\.)
  | (?P<dot>\.)
  | (?P<dslash>//)
  | (?P<slash>/)
  | (?P<dcolon>::)
  | (?P<op2>!=|<=|>=)
  | (?P<punct>[()\[\]@,|+\-=<>*])
  | (?P<var>\$%(nc)s(?::%(nc)s)?)
  | (?P<name>%(nc)s(?::(?!:)(?:%(nc)s|\*))?)
    """ % {"nc": _NC},
    re.VERBOSE,
)

_NODE_TYPES = {"comment", "text", "processing-instruction", "node"}
_OP_NAMES = {"and", "or", "mod", "div"}

# Token classes used by the operand/operator disambiguation (XPath 1.0, section 3.7).
_OPEN, _OP, _OPERAND = "open", "op", "operand"


def tokenize(expr: str) -> List[Tuple[str, str, int]]:
    """Split an expression into (kind, text, offset) tokens; whitespace is dropped."""
    toks = []
    pos = 0
    n = len(expr)
    while pos < n:
        m = _TOKEN_RE.match(expr, pos)
        if not m:
            raise XPathError("cannot tokenize XPath at offset %d: %r" % (pos, expr[pos:pos + 20]))
        kind = m.lastgroup
        if kind != "ws":
            toks.append((kind, m.group(), pos))
        pos = m.end()
    return toks


def to_document_context(expr: str) -> str:
    """Rewrite `expr` so that evaluating it with the root element as context node returns what
    XmlDocument.SelectNodes would return (context = document node)."""
    toks = tokenize(expr)
    n = len(toks)
    inserts: List[int] = []
    depth = 0                 # predicate nesting
    prev_cls = None           # class of the previous token (None at the start)
    prev_kind = None          # raw kind of the previous token
    prev_text = None
    for i, (kind, text, start) in enumerate(toks):
        expect_operand = prev_cls in (None, _OPEN, _OP)
        # a step that continues a path or an axis/abbreviation never starts a new path
        mid_path = prev_kind in ("slash", "dslash", "dcolon") or (prev_kind == "punct" and prev_text == "@")
        starts_step = False
        if kind == "punct":
            if text == "[":
                depth += 1
                cls = _OPEN
            elif text == "]":
                depth -= 1
                if depth < 0:
                    raise XPathError("unbalanced ] in XPath")
                cls = _OPERAND
            elif text in ("(", ",", "@"):
                starts_step = (text == "@") and expect_operand and not mid_path
                cls = _OPEN
            elif text == ")":
                cls = _OPERAND
            elif text == "*":
                if expect_operand:
                    starts_step = not mid_path          # wildcard NameTest
                    cls = _OPERAND
                else:
                    cls = _OP                           # MultiplyOperator
            else:                                       # | + - = < >
                cls = _OP
        elif kind in ("op2", "slash", "dslash"):
            cls = _OP
        elif kind == "dcolon":
            cls = _OPEN
        elif kind in ("lit", "num", "var"):
            cls = _OPERAND
        elif kind in ("dot", "dotdot"):
            starts_step = expect_operand and not mid_path
            cls = _OPERAND
        else:  # name
            if expect_operand:
                nxt = toks[i + 1] if i + 1 < n else None
                if nxt is not None and nxt[0] == "punct" and nxt[1] == "(":
                    starts_step = text in _NODE_TYPES and not mid_path      # text() / node() ...
                elif nxt is not None and nxt[0] == "dcolon":
                    starts_step = not mid_path                              # AxisName
                else:
                    starts_step = not mid_path                              # NameTest
                cls = _OPERAND
            elif text in _OP_NAMES:
                cls = _OP
            else:
                raise XPathError("unexpected name %r in XPath" % text)
        if starts_step and depth == 0:
            inserts.append(start)
        prev_cls, prev_kind, prev_text = cls, kind, text
    if depth != 0:
        raise XPathError("unbalanced [ in XPath")
    if not inserts:
        return expr
    out = []
    last = 0
    for pos in inserts:
        out.append(expr[last:pos])
        out.append("/")
        last = pos
    out.append(expr[last:])
    return "".join(out)


_compiled: Dict[str, "etree.XPath"] = {}


def compile_xpath(expr: str):
    c = _compiled.get(expr)
    if c is None:
        try:
            c = etree.XPath(to_document_context(expr))
        except (etree.XPathSyntaxError, etree.XPathEvalError) as exc:
            raise XPathError(str(exc)) from exc
        _compiled[expr] = c
    return c


def select_nodes(root: Element, expr: str) -> list:
    """XmlDocument.SelectNodes(expr) on the document whose root element is `root`.

    Returns Elements, TextRefs and AttrRefs in document order. Raises XPathError when the
    expression is invalid or does not produce a node-set.
    """
    if expr is None:
        raise XPathError("xpath is null")
    xp = compile_xpath(expr)
    try:
        res = xp(root)
    except (etree.XPathEvalError, etree.XPathSyntaxError) as exc:
        raise XPathError(str(exc)) from exc
    if not isinstance(res, list):
        raise XPathError("Expression must evaluate to a node-set.")
    out = []
    for item in res:
        if isinstance(item, etree._Element):
            if isinstance(item.tag, str):
                out.append(item)
            continue
        if isinstance(item, str):
            if getattr(item, "is_attribute", False):
                out.append(AttrRef(item.getparent(), item.attrname))
            elif getattr(item, "is_tail", False):
                out.append(TextRef(item.getparent(), "tail"))
            elif getattr(item, "is_text", False):
                out.append(TextRef(item.getparent(), "text"))
    return out


def select_single_node(root: Element, expr: str):
    """XmlDocument.SelectSingleNode(expr): first node of the node-set or None."""
    res = select_nodes(root, expr)
    return res[0] if res else None
