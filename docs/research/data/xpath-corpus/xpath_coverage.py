#!/usr/bin/env python3
"""
xpath_coverage.py - measure which XPath 1.0 constructs real RimWorld patches use.

What it does
  1. Finds every folder named "Patches" (any depth, any letter case) inside four corpora:
       vanilla   RimWorld <install>/Data/<Core|Royalty|Ideology|Biotech|Anomaly|Odyssey>
       ce_dev    a Combat Extended source checkout (Patches/ and ModPatches/*/Patches)
       workshop  every folder of the Steam Workshop content directory for the game (294100)
       owner     a directory that holds hand-written mods
  2. Parses every *.xml file below those folders the way the game's loader does
     (UTF-8 BOM stripped, bytes always decoded as UTF-8, comments and blank text dropped,
     illegal XML characters tolerated) and keeps files whose root element is <Patch>.
  3. Walks every <Operation> (including nested operations in <operations>/<li>, <match>,
     <nomatch> and any child that carries a Class attribute, but never <value> content)
     and extracts the <xpath> text plus the operation Class.
  4. Tokenises and parses each expression with a complete XPath 1.0 lexer and parser
     (this file), then tabulates axes, node tests, predicate kinds, functions, operators,
     namespaces, variables, lengths, and the share covered by progressively larger subsets:
       A  absolute/relative child steps, optional final @attribute, predicates of the form
          (@attr | child) = "literal" only
       B  A plus //, ., .., *, and/or/not(), contains(), starts-with(), !=, existence and
          nested-path predicates, numeric equality
       C  B plus [n], last(), position(), count(), text(), relational operators, unions
       D  anything else that is valid XPath 1.0 (explicit axes, other functions, arithmetic,
          filter expressions, node(), comment())
       X  rejected by an XPath 1.0 engine used through XmlDocument.SelectNodes (syntax
          error, unknown function, variable, namespace prefix, non-node-set result)
  5. Writes summary.json next to this script. The raw corpus is NOT stored. Use
     --dump-expressions PATH only to hand the distinct expressions to another tool
     (for example the .NET cross-check); keep that file out of version control.

Usage (defaults assume a Steam install under ~/.steam/steam):
  python3 xpath_coverage.py --ce /path/to/CombatExtended --owner "/path/to/RimWorld Mods"

Only the Python standard library and lxml are required.
Evidence for the game-side behaviour is documented in docs/research/xpath-patch-coverage.md.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import statistics
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from lxml import etree

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[3]
DATE = "2026-10-04"

# ---------------------------------------------------------------------------
# XPath 1.0 lexer and parser (W3C XPath 1.0, section 3.7 lexical structure)
# ---------------------------------------------------------------------------

NAME_START = (
    "A-Za-z_\u00C0-\u00D6\u00D8-\u00F6\u00F8-\u02FF\u0370-\u037D\u037F-\u1FFF\u200C-\u200D"
    "\u2070-\u218F\u2C00-\u2FEF\u3001-\uD7FF\uF900-\uFDCF\uFDF0-\uFFFD\U00010000-\U000EFFFF"
)
NAME_CHAR = NAME_START + "\\-.0-9\u00B7\u0300-\u036F\u203F-\u2040"
NCNAME_RE = re.compile(f"[{NAME_START}][{NAME_CHAR}]*")
NUMBER_RE = re.compile(r"\d+(?:\.\d*)?|\.\d+")
WS_RE = re.compile(r"[ \t\r\n]+")

AXES = {
    "ancestor", "ancestor-or-self", "attribute", "child", "descendant", "descendant-or-self",
    "following", "following-sibling", "namespace", "parent", "preceding", "preceding-sibling",
    "self",
}
NODE_TYPES = {"comment", "text", "processing-instruction", "node"}
OPERATOR_NAMES = {"and", "or", "mod", "div"}
XPATH1_FUNCTIONS = {
    "last", "position", "count", "id", "local-name", "namespace-uri", "name", "string",
    "concat", "starts-with", "contains", "substring-before", "substring-after", "substring",
    "string-length", "normalize-space", "translate", "boolean", "not", "true", "false",
    "lang", "number", "sum", "floor", "ceiling", "round",
}
NODESET_FUNCTIONS = {"id"}
NON_OPERAND_PUNCT = {"@", "::", "(", "[", ",", "/", "//", "|", "+", "-", "=", "!=", "<", "<=", ">", ">="}


class XPathSyntaxError(Exception):
    def __init__(self, msg: str, pos: int = -1):
        super().__init__(msg)
        self.msg = msg
        self.pos = pos


def tokenize(s: str) -> list[tuple[str, str, int]]:
    """Return (kind, value, position) tokens. Kinds: NUM LIT NAME MUL OPNAME AXIS NODETYPE
    FUNC VAR PUNCT. Implements the disambiguation rules of XPath 1.0 section 3.7."""
    toks: list[tuple[str, str, int]] = []
    i, n = 0, len(s)

    def prev_ends_operand() -> bool:
        if not toks:
            return False
        kind, val, _ = toks[-1]
        if kind == "PUNCT" and val in NON_OPERAND_PUNCT:
            return False
        if kind in ("MUL", "OPNAME"):
            return False
        return True

    while i < n:
        m = WS_RE.match(s, i)
        if m:
            i = m.end()
            continue
        c = s[i]
        if c in "\"'":
            j = s.find(c, i + 1)
            if j < 0:
                raise XPathSyntaxError("unterminated string literal", i)
            toks.append(("LIT", s[i + 1:j], i))
            i = j + 1
            continue
        if c.isdigit() or (c == "." and i + 1 < n and s[i + 1].isdigit()):
            m = NUMBER_RE.match(s, i)
            toks.append(("NUM", m.group(), i))
            i = m.end()
            continue
        two = s[i:i + 2]
        if two in ("//", "..", "::", "!=", "<=", ">="):
            toks.append(("PUNCT", two, i))
            i += 2
            continue
        if c in "/.@,()[]|+-=<>":
            toks.append(("PUNCT", c, i))
            i += 1
            continue
        if c == "$":
            m = NCNAME_RE.match(s, i + 1)
            if not m:
                raise XPathSyntaxError("bad variable reference", i)
            name = m.group()
            j = m.end()
            if j < n and s[j] == ":" and not s.startswith("::", j):
                m2 = NCNAME_RE.match(s, j + 1)
                if m2:
                    name += ":" + m2.group()
                    j = m2.end()
            toks.append(("VAR", name, i))
            i = j
            continue
        if c == "*":
            toks.append(("MUL" if prev_ends_operand() else "NAME", "*", i))
            i += 1
            continue
        m = NCNAME_RE.match(s, i)
        if m:
            name = m.group()
            j = m.end()
            if j < n and s[j] == ":" and not s.startswith("::", j):
                if j + 1 < n and s[j + 1] == "*":
                    name += ":*"
                    j += 2
                else:
                    m2 = NCNAME_RE.match(s, j + 1)
                    if not m2:
                        raise XPathSyntaxError("bad qualified name", i)
                    name += ":" + m2.group()
                    j = m2.end()
            if prev_ends_operand() and name in OPERATOR_NAMES:
                toks.append(("OPNAME", name, i))
                i = j
                continue
            ws = WS_RE.match(s, j)
            k = ws.end() if ws else j
            if s.startswith("(", k):
                kind = "NODETYPE" if name in NODE_TYPES else "FUNC"
            elif s.startswith("::", k):
                if name not in AXES:
                    raise XPathSyntaxError(f"unknown axis {name!r}", i)
                kind = "AXIS"
            else:
                kind = "NAME"
            toks.append((kind, name, i))
            i = j
            continue
        raise XPathSyntaxError(f"unexpected character {c!r}", i)
    return toks


@dataclass
class Step:
    axis: str
    test: tuple  # ("name", n) | ("wild", "*") | ("nswild", "p:*") | ("type", t) | ("pi", lit)
    preds: list = field(default_factory=list)
    abbrev: str = ""  # "", "//", ".", ".."


@dataclass
class LPath:
    absolute: bool
    steps: list
    start: object = None  # FilterExpr base for paths such as (a|b)/c


@dataclass
class Filter:
    primary: object
    preds: list


@dataclass
class Bin:
    op: str
    l: object
    r: object


@dataclass
class Neg:
    e: object


@dataclass
class Call:
    name: str
    args: list


@dataclass
class Lit:
    v: str


@dataclass
class Num:
    v: str


@dataclass
class Var:
    name: str


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self, off=0):
        j = self.i + off
        return self.t[j] if j < len(self.t) else ("EOF", "", -1)

    def take(self):
        tok = self.peek()
        self.i += 1
        return tok

    def is_punct(self, v, off=0):
        k, val, _ = self.peek(off)
        return k == "PUNCT" and val == v

    def expect_punct(self, v):
        if not self.is_punct(v):
            k, val, pos = self.peek()
            raise XPathSyntaxError(f"expected {v!r} but found {val or 'end of expression'!r}", pos)
        self.i += 1

    def parse(self):
        if not self.t:
            raise XPathSyntaxError("empty expression", 0)
        e = self.or_expr()
        if self.peek()[0] != "EOF":
            raise XPathSyntaxError(f"unexpected token {self.peek()[1]!r}", self.peek()[2])
        return e

    def or_expr(self):
        e = self.and_expr()
        while self.peek()[0] == "OPNAME" and self.peek()[1] == "or":
            self.take()
            e = Bin("or", e, self.and_expr())
        return e

    def and_expr(self):
        e = self.eq_expr()
        while self.peek()[0] == "OPNAME" and self.peek()[1] == "and":
            self.take()
            e = Bin("and", e, self.eq_expr())
        return e

    def eq_expr(self):
        e = self.rel_expr()
        while self.peek()[0] == "PUNCT" and self.peek()[1] in ("=", "!="):
            op = self.take()[1]
            e = Bin(op, e, self.rel_expr())
        return e

    def rel_expr(self):
        e = self.add_expr()
        while self.peek()[0] == "PUNCT" and self.peek()[1] in ("<", "<=", ">", ">="):
            op = self.take()[1]
            e = Bin(op, e, self.add_expr())
        return e

    def add_expr(self):
        e = self.mul_expr()
        while self.peek()[0] == "PUNCT" and self.peek()[1] in ("+", "-"):
            op = self.take()[1]
            e = Bin(op, e, self.mul_expr())
        return e

    def mul_expr(self):
        e = self.unary_expr()
        while (self.peek()[0] == "MUL") or (self.peek()[0] == "OPNAME" and self.peek()[1] in ("div", "mod")):
            op = "*" if self.peek()[0] == "MUL" else self.peek()[1]
            self.take()
            e = Bin(op, e, self.unary_expr())
        return e

    def unary_expr(self):
        if self.is_punct("-"):
            self.take()
            return Neg(self.unary_expr())
        return self.union_expr()

    def union_expr(self):
        e = self.path_expr()
        while self.is_punct("|"):
            self.take()
            e = Bin("|", e, self.path_expr())
        return e

    def path_expr(self):
        k, v, pos = self.peek()
        # FilterExpr starts: literal, number, variable, '(' or a function call
        if k in ("LIT", "NUM", "VAR") or (k == "PUNCT" and v == "(") or k == "FUNC":
            prim = self.primary()
            preds = []
            while self.is_punct("["):
                preds.append(self.predicate())
            base = Filter(prim, preds) if preds else prim
            if self.is_punct("/") or self.is_punct("//"):
                steps = []
                sep = self.take()[1]
                if sep == "//":
                    steps.append(Step("descendant-or-self", ("type", "node"), [], "//"))
                steps.extend(self.relative_steps())
                return LPath(False, steps, base)
            return base
        return self.location_path()

    def primary(self):
        k, v, pos = self.take()
        if k == "LIT":
            return Lit(v)
        if k == "NUM":
            return Num(v)
        if k == "VAR":
            return Var(v)
        if k == "PUNCT" and v == "(":
            e = self.or_expr()
            self.expect_punct(")")
            return e
        if k == "FUNC":
            self.expect_punct("(")
            args = []
            if not self.is_punct(")"):
                args.append(self.or_expr())
                while self.is_punct(","):
                    self.take()
                    args.append(self.or_expr())
            self.expect_punct(")")
            return Call(v, args)
        raise XPathSyntaxError(f"unexpected token {v!r}", pos)

    def location_path(self):
        if self.is_punct("/"):
            self.take()
            steps = []
            # a bare "/" is legal: it selects the document node
            if self.starts_step():
                steps = self.relative_steps()
            return LPath(True, steps)
        if self.is_punct("//"):
            self.take()
            steps = [Step("descendant-or-self", ("type", "node"), [], "//")]
            steps.extend(self.relative_steps())
            return LPath(True, steps)
        return LPath(False, self.relative_steps())

    def starts_step(self):
        k, v, _ = self.peek()
        return k in ("NAME", "AXIS", "NODETYPE") or (k == "PUNCT" and v in ("@", ".", ".."))

    def relative_steps(self):
        steps = [self.step()]
        while self.is_punct("/") or self.is_punct("//"):
            sep = self.take()[1]
            if sep == "//":
                steps.append(Step("descendant-or-self", ("type", "node"), [], "//"))
            steps.append(self.step())
        return steps

    def step(self):
        k, v, pos = self.peek()
        if k == "PUNCT" and v == ".":
            self.take()
            return Step("self", ("type", "node"), [], ".")
        if k == "PUNCT" and v == "..":
            self.take()
            return Step("parent", ("type", "node"), [], "..")
        axis = "child"
        if k == "AXIS":
            self.take()
            self.expect_punct("::")
            axis = v
        elif k == "PUNCT" and v == "@":
            self.take()
            axis = "attribute"
        k, v, pos = self.peek()
        if k == "NAME":
            self.take()
            if v == "*":
                test = ("wild", "*")
            elif v.endswith(":*"):
                test = ("nswild", v)
            else:
                test = ("name", v)
        elif k == "NODETYPE":
            self.take()
            self.expect_punct("(")
            if v == "processing-instruction" and self.peek()[0] == "LIT":
                test = ("pi", self.take()[1])
            else:
                test = ("type", v)
            self.expect_punct(")")
        else:
            raise XPathSyntaxError(f"expected a node test but found {v or 'end of expression'!r}", pos)
        preds = []
        while self.is_punct("["):
            preds.append(self.predicate())
        return Step(axis, test, preds)

    def predicate(self):
        self.expect_punct("[")
        e = self.or_expr()
        self.expect_punct("]")
        return e


def parse_xpath(s: str):
    return Parser(tokenize(s)).parse()


def shape_of(s: str) -> str:
    """Normalised form: literals replaced, whitespace removed. Used to group expressions."""
    out = []
    for kind, val, _ in tokenize(s):
        if kind == "LIT":
            out.append("'S'")
        elif kind == "NUM":
            out.append("N")
        elif kind in ("OPNAME", "MUL") or (kind == "PUNCT" and val in ("and", "or")):
            out.append(f" {val} ")
        elif kind == "AXIS":
            out.append(val)
        else:
            out.append(val)
    return "".join(out)


# ---------------------------------------------------------------------------
# Feature extraction and subset tiers
# ---------------------------------------------------------------------------

TIER_ORDER = {"A": 0, "B": 1, "C": 2, "D": 3, "X": 4}

# tag -> minimal subset that must implement it
TAG_TIER = {
    "path.absolute": "A",
    "path.relative": "A",
    "step.child.name": "A",
    "step.attribute.final": "A",
    "pred.attr_eq_str": "A",
    "pred.child_eq_str": "A",
    "step.attribute.other": "B",
    "step.wildcard": "B",
    "step.//": "B",
    "step.self": "B",
    "step.parent": "B",
    "pred.and": "B",
    "pred.or": "B",
    "fn.not": "B",
    "fn.contains": "B",
    "fn.starts-with": "B",
    "pred.neq": "B",
    "pred.exists": "B",
    "pred.nested_path_eq": "B",
    "pred.self_eq": "B",
    "pred.num_cmp": "B",
    "step.text()": "C",
    "pred.text_eq": "C",
    "pred.position_number": "C",
    "fn.last": "C",
    "fn.position": "C",
    "fn.count": "C",
    "op.relational": "C",
    "op.union": "C",
    "step.node()": "D",
    "step.comment()": "D",
    "step.processing-instruction()": "D",
    "step.explicit_axis": "D",
    "filter_expr": "D",
    "op.arith": "D",
    "pred.func_cmp": "D",
    "pred.other": "D",
    "fn.other": "D",
    "ns_prefix": "X",
    "var": "X",
    "fn.unknown": "X",
    "non_nodeset": "X",
}


class Feat:
    def __init__(self):
        self.axes = collections.Counter()
        self.tests = collections.Counter()
        self.preds = collections.Counter()
        self.funcs = collections.Counter()
        self.ops = collections.Counter()
        self.tags = set()
        self.prefixes = set()
        self.vars = set()
        self.unknown_funcs = set()
        self.steps = 0
        self.npreds = 0
        self.maxdepth = 0


def operand_kind(x) -> str:
    if isinstance(x, Lit):
        return "str"
    if isinstance(x, Num):
        return "num"
    if isinstance(x, Call):
        return "func"
    if isinstance(x, LPath) and x.start is None and not x.absolute and len(x.steps) == 1:
        st = x.steps[0]
        if not st.preds:
            if st.abbrev == ".":
                return "self"
            if st.axis == "attribute" and st.test[0] == "name":
                return "attr"
            if st.axis == "child" and st.test[0] == "name":
                return "child"
            if st.axis == "child" and st.test == ("type", "text"):
                return "text"
    if isinstance(x, LPath):
        return "path"
    return "other"


def is_nodeset(e) -> bool:
    if isinstance(e, LPath):
        return True
    if isinstance(e, Filter):
        return is_nodeset(e.primary)
    if isinstance(e, Bin) and e.op == "|":
        return is_nodeset(e.l) and is_nodeset(e.r)
    if isinstance(e, Call):
        return e.name in NODESET_FUNCTIONS
    return False


def visit(e, F: Feat, depth: int = 0):
    """Walk an expression and fill the feature collector."""
    if isinstance(e, LPath):
        F.tags.add("path.absolute" if e.absolute else "path.relative")
        if e.start is not None:
            F.tags.add("filter_expr")
            visit(e.start, F, depth)
        n = len(e.steps)
        for idx, st in enumerate(e.steps):
            F.steps += 1
            kind, val = st.test[0], st.test[1]
            if st.abbrev == "//":
                F.axes["descendant-or-self (//)"] += 1
                F.tags.add("step.//")
                continue
            if st.abbrev == ".":
                F.axes["self (.)"] += 1
                F.tags.add("step.self")
            elif st.abbrev == "..":
                F.axes["parent (..)"] += 1
                F.tags.add("step.parent")
            else:
                F.axes[st.axis] += 1
                if st.axis not in ("child", "attribute"):
                    F.tags.add("step.explicit_axis")
                if kind == "name":
                    F.tests["name"] += 1
                    if ":" in val:
                        F.prefixes.add(val.split(":", 1)[0])
                        F.tags.add("ns_prefix")
                    if st.axis == "child":
                        F.tags.add("step.child.name")
                    elif st.axis == "attribute":
                        F.tags.add("step.attribute.final" if (idx == n - 1 and not st.preds) else "step.attribute.other")
                    else:
                        F.tags.add("step.child.name")
                elif kind == "wild":
                    F.tests["* (wildcard)"] += 1
                    F.tags.add("step.wildcard")
                elif kind == "nswild":
                    F.tests["prefix:*"] += 1
                    F.prefixes.add(val.split(":", 1)[0])
                    F.tags.add("ns_prefix")
                elif kind == "type":
                    F.tests[f"{val}()"] += 1
                    F.tags.add(f"step.{val}()")
                elif kind == "pi":
                    F.tests["processing-instruction()"] += 1
                    F.tags.add("step.processing-instruction()")
            for p in st.preds:
                F.npreds += 1
                visit_pred(p, F, depth + 1)
        return
    if isinstance(e, Filter):
        F.tags.add("filter_expr")
        visit(e.primary, F, depth)
        for p in e.preds:
            F.npreds += 1
            visit_pred(p, F, depth + 1)
        return
    if isinstance(e, Bin):
        op = e.op
        if op == "|":
            F.ops["union (|)"] += 1
            F.tags.add("op.union")
        elif op in ("and", "or"):
            F.ops[op] += 1
            F.tags.add(f"pred.{op}")
        elif op in ("=", "!="):
            F.ops[op] += 1
        elif op in ("<", "<=", ">", ">="):
            F.ops["relational (< <= > >=)"] += 1
            F.tags.add("op.relational")
        else:
            F.ops["arithmetic (+ - * div mod)"] += 1
            F.tags.add("op.arith")
        visit(e.l, F, depth)
        visit(e.r, F, depth)
        return
    if isinstance(e, Neg):
        F.ops["arithmetic (+ - * div mod)"] += 1
        F.tags.add("op.arith")
        visit(e.e, F, depth)
        return
    if isinstance(e, Call):
        F.funcs[e.name] += 1
        if e.name not in XPATH1_FUNCTIONS:
            F.unknown_funcs.add(e.name)
            F.tags.add("fn.unknown")
        elif e.name in ("not", "contains", "starts-with", "last", "position", "count"):
            F.tags.add(f"fn.{e.name}")
        else:
            F.tags.add("fn.other")
        for a in e.args:
            visit(a, F, depth)
        return
    if isinstance(e, Var):
        F.vars.add(e.name)
        F.tags.add("var")
        return
    # literals and numbers carry no features


def visit_pred(p, F: Feat, depth: int):
    F.maxdepth = max(F.maxdepth, depth)
    leaves(p, F)
    visit(p, F, depth)


def leaves(e, F: Feat):
    """Classify the boolean structure of one predicate into leaf kinds."""
    if isinstance(e, Bin) and e.op in ("and", "or"):
        leaves(e.l, F)
        leaves(e.r, F)
        return
    if isinstance(e, Call) and e.name == "not" and len(e.args) == 1:
        leaves(e.args[0], F)
        return
    if isinstance(e, Num):
        F.preds["position by number [n]"] += 1
        F.tags.add("pred.position_number")
        return
    if isinstance(e, Bin) and e.op in ("=", "!="):
        lk, rk = operand_kind(e.l), operand_kind(e.r)
        pair = {lk, rk}
        if e.op == "!=":
            F.preds["inequality (!=)"] += 1
            F.tags.add("pred.neq")
            return
        if "str" in pair:
            other = rk if lk == "str" else lk
            name = {
                "attr": "attribute equality (@a = 'v')",
                "child": "child-text equality (c = 'v')",
                "text": "text() equality (text() = 'v')",
                "self": "self equality (. = 'v')",
                "path": "nested-path equality (a/b = 'v')",
                "func": "function result equality (f(..) = 'v')",
            }.get(other, "other equality")
            F.preds[name] += 1
            F.tags.add({
                "attr": "pred.attr_eq_str",
                "child": "pred.child_eq_str",
                "text": "pred.text_eq",
                "self": "pred.self_eq",
                "path": "pred.nested_path_eq",
                "func": "pred.func_cmp",
            }.get(other, "pred.other"))
            return
        if "num" in pair:
            F.preds["numeric equality (x = 5)"] += 1
            F.tags.add("pred.num_cmp" if "func" not in pair else "pred.func_cmp")
            return
        F.preds["comparison of two non-literals"] += 1
        F.tags.add("pred.other")
        return
    if isinstance(e, Bin) and e.op in ("<", "<=", ">", ">="):
        F.preds["relational comparison"] += 1
        return
    if isinstance(e, LPath):
        k = operand_kind(e)
        if k == "attr":
            F.preds["attribute existence ([@a])"] += 1
        elif k == "child":
            F.preds["child existence ([c])"] += 1
        else:
            F.preds["nested-path existence ([a/b])"] += 1
        F.tags.add("pred.exists")
        return
    if isinstance(e, Call):
        F.preds[f"function: {e.name}()"] += 1
        return
    F.preds["other"] += 1
    F.tags.add("pred.other")


def analyse(expr: str):
    """Parse and classify one expression. Returns (Feat | None, error | None, tier)."""
    try:
        ast = parse_xpath(expr)
    except XPathSyntaxError as ex:
        return None, ex.msg, "X"
    F = Feat()
    visit(ast, F, 0)
    if not is_nodeset(ast):
        F.tags.add("non_nodeset")
    tier = "A"
    for t in F.tags:
        tt = TAG_TIER[t]
        if TIER_ORDER[tt] > TIER_ORDER[tier]:
            tier = tt
    return F, None, tier


# ---------------------------------------------------------------------------
# Corpus discovery and extraction
# ---------------------------------------------------------------------------

VANILLA_PATHED = {
    "PatchOperationAdd", "PatchOperationInsert", "PatchOperationRemove", "PatchOperationReplace",
    "PatchOperationAttributeAdd", "PatchOperationAttributeSet", "PatchOperationAttributeRemove",
    "PatchOperationSetName", "PatchOperationAddModExtension", "PatchOperationConditional",
    "PatchOperationTest",
}
VANILLA_OTHER = {"PatchOperation", "PatchOperationSequence", "PatchOperationFindMod"}
VANILLA_ALL = VANILLA_PATHED | VANILLA_OTHER
ILLEGAL_XML = re.compile("[\x00-\x08\x0b\x0c\x0e-\x1f\ufffe\uffff]")
VERSION_DIR = re.compile(r"^\d+\.\d+$")
CURRENT_VERSION = (1, 6, 4871)


def parse_like_game(path: Path):
    """Mimic LoadableXmlAsset: strip a UTF-8 BOM, decode as UTF-8 (invalid bytes replaced),
    ignore the encoding declaration, drop comments and blank text, tolerate illegal XML
    characters. Returns (root | None, status) with status ok | recovered | failed."""
    data = path.read_bytes()
    if data[:3] == b"\xef\xbb\xbf":
        data = data[3:]
    text = ILLEGAL_XML.sub("", data.decode("utf-8", errors="replace"))
    raw = text.encode("utf-8")
    kw = dict(encoding="utf-8", remove_comments=True, remove_blank_text=True,
              resolve_entities=False, no_network=True, huge_tree=True)
    try:
        return etree.fromstring(raw, etree.XMLParser(recover=False, **kw)), "ok"
    except etree.XMLSyntaxError:
        pass
    except ValueError:
        pass
    try:
        root = etree.fromstring(raw, etree.XMLParser(recover=True, **kw))
    except Exception:
        root = None
    return (root, "recovered") if root is not None else (None, "failed")


def find_patch_dirs(unit: Path):
    """Yield every directory below unit whose name is 'patches' (any case). Nested ones are
    covered by the outer walk because the game enumerates Patches/ recursively."""
    for dirpath, dirnames, _files in os.walk(unit):
        keep = []
        for d in dirnames:
            if d.lower() == "patches":
                yield Path(dirpath) / d
            else:
                keep.append(d)
        dirnames[:] = keep


def xml_files_under(d: Path):
    for dirpath, _dirs, files in os.walk(d):
        for f in files:
            if f.lower().endswith(".xml") and not f.startswith("."):
                yield Path(dirpath) / f


def load_folders_for_16(unit: Path):
    """Approximate ModContentPack.InitLoadFolders for game 1.6.4871 ignoring IfModActive
    conditions (so the result is an upper bound). Returns (set of relative folder paths,
    method). The empty string is the mod root."""
    lf = None
    for p in unit.iterdir() if unit.is_dir() else []:
        if p.is_file() and p.name.lower() == "loadfolders.xml":
            lf = p
    if lf is not None:
        root, status = parse_like_game(lf)
        if root is not None:
            versions: dict[str, list[str]] = {}
            for v in root:
                if not isinstance(v.tag, str):
                    continue
                key = v.tag.lower()
                if key.startswith("v"):
                    key = key[1:]
                folders = versions.setdefault(key, [])
                for li in v:
                    if not isinstance(li.tag, str):
                        continue
                    txt = "".join(li.itertext())
                    folders.append("" if txt in ("/", "\\") else txt.replace("\\", "/").strip("/"))
            if versions:
                def parse_ver(k):
                    parts = k.split(".")
                    if len(parts) > 3 or any(not p.isdigit() for p in parts):
                        return None
                    nums = [int(p) for p in parts] + [0] * (3 - len(parts))
                    return tuple(nums)
                cand = [k for k in versions if k != "default" and "." in k
                        and (parse_ver(k) is not None and parse_ver(k) <= CURRENT_VERSION)]
                cand.sort(reverse=True)
                if "1.6.4871" in versions and versions["1.6.4871"]:
                    return set(versions["1.6.4871"]), "loadfolders:exact"
                if cand:
                    return set(versions[cand[0]]), f"loadfolders:{cand[0]}"
                if "default" in versions:
                    return set(versions["default"]), "loadfolders:default"
    out = {""}
    subdirs = [d.name for d in unit.iterdir() if d.is_dir()] if unit.is_dir() else []
    if "1.6" in subdirs:
        out.add("1.6")
        method = "folder:1.6"
    else:
        best = None
        for d in subdirs:
            if VERSION_DIR.match(d):
                v = tuple(int(x) for x in d.split("."))
                if v <= CURRENT_VERSION[:2] and (best is None or v > best):
                    best = v
        if best:
            out.add(".".join(map(str, best)))
            method = f"folder:{'.'.join(map(str, best))}"
        else:
            method = "root-only"
    if "Common" in subdirs:
        out.add("Common")
    return out, method


@dataclass
class OpRec:
    source: str
    unit: str
    file_id: int
    cls: str
    depth: int
    xpaths: list
    extra_xpath_fields: list
    success: str | None
    may_require: bool
    reachable_16: bool


def collect_ops(el, depth, out, ctx):
    cls = (el.get("Class") or "").strip()
    xps = []
    extra = []
    success = None
    for c in el:
        if not isinstance(c.tag, str):
            continue
        if c.tag == "xpath":
            xps.append("".join(c.itertext()))
        elif "xpath" in c.tag.lower():
            extra.append((c.tag, "".join(c.itertext())))
        elif c.tag == "success":
            success = "".join(c.itertext()).strip()
    out.append((cls, depth, xps, extra, success, el.get("MayRequire") is not None or el.get("MayRequireAnyOf") is not None))
    for c in el:
        if not isinstance(c.tag, str) or c.tag in ("value", "xpath"):
            continue
        if c.get("Class") is not None:
            collect_ops(c, depth + 1, out, ctx)
        else:
            for li in c:
                if isinstance(li.tag, str) and li.tag == "li" and li.get("Class") is not None:
                    collect_ops(li, depth + 1, out, ctx)


def discover_units(args):
    units = []
    if args.vanilla and args.vanilla.is_dir():
        for d in sorted(args.vanilla.iterdir()):
            if d.is_dir():
                units.append(("vanilla", d.name, d))
    if args.ce and args.ce.is_dir():
        units.append(("ce_dev", "CombatExtended", args.ce))
    if args.workshop and args.workshop.is_dir():
        for d in sorted(args.workshop.iterdir()):
            if d.is_dir():
                units.append(("workshop", d.name, d))
    if args.owner and args.owner.is_dir():
        for i, d in enumerate(sorted(args.owner.iterdir())):
            if d.is_dir():
                units.append(("owner", f"owner-{i:02d}", d))
    return units


def read_about(unit: Path):
    for cand in (unit / "About" / "About.xml",):
        p = cand
        if not p.exists() and (unit / "About").is_dir():
            for f in (unit / "About").iterdir():
                if f.name.lower() == "about.xml":
                    p = f
        if p.exists():
            root, _ = parse_like_game(p)
            if root is not None:
                pid = root.findtext("packageId")
                return (pid or "").strip()
    return ""


def folder_class(unit: Path, patches_dir: Path):
    rel = patches_dir.parent.relative_to(unit).as_posix()
    if rel in ("", "."):
        return "root", ""
    if rel == "Common":
        return "Common", rel
    if VERSION_DIR.match(rel):
        return "version folder", rel
    if rel.lower() == "common":
        return "Common (wrong case)", rel
    return "other folder", rel


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    steam = Path.home() / ".steam" / "steam" / "steamapps"
    ap.add_argument("--vanilla", type=Path, default=steam / "common" / "RimWorld" / "Data")
    ap.add_argument("--workshop", type=Path, default=steam / "workshop" / "content" / "294100")
    ap.add_argument("--ce", type=Path, default=None, help="Combat Extended source checkout")
    ap.add_argument("--owner", type=Path, default=None, help="directory of hand-written mods")
    ap.add_argument("--out", type=Path, default=HERE / "summary.json")
    ap.add_argument("--dump-expressions", type=Path, default=None,
                    help="write distinct raw expressions as JSON lines (keep out of version control)")
    ap.add_argument("--limit-units", type=int, default=0, help="debug: stop after N units per source")
    args = ap.parse_args()

    t0 = time.time()
    units = discover_units(args)
    per_source = collections.defaultdict(lambda: collections.Counter())
    ops: list[OpRec] = []
    parse_problems = collections.Counter()
    problem_examples = []
    dir_case = collections.Counter()
    folder_classes = collections.Counter()
    seen_per_source = collections.Counter()
    unit_info = {}
    file_counter = 0
    reach_methods = collections.Counter()

    for source, uid, unit in units:
        if args.limit_units and seen_per_source[source] >= args.limit_units:
            continue
        pdirs = list(find_patch_dirs(unit))
        if not pdirs and source != "vanilla":
            per_source[source]["units_without_patches"] += 1
            continue
        seen_per_source[source] += 1
        per_source[source]["units_with_patches_dir"] += 1
        pid = read_about(unit) if source in ("vanilla", "workshop") else ""
        reach_set, reach_method = load_folders_for_16(unit)
        reach_methods[reach_method.split(":")[0] + ":" + (reach_method.split(":")[1] if reach_method.startswith("loadfolders") else "")] += 1
        unit_info[(source, uid)] = {"package_id": pid, "reach": reach_method}
        for pd in pdirs:
            if source == "ce_dev":
                pass
            fc, rel = folder_class(unit, pd)
            folder_classes[(source, fc)] += 1
            dir_case[(source, pd.name)] += 1
            parent_rel = pd.parent.relative_to(unit).as_posix()
            parent_rel = "" if parent_rel == "." else parent_rel
            reachable = parent_rel in reach_set
            for f in xml_files_under(pd):
                file_counter += 1
                per_source[source]["xml_files"] += 1
                root, status = parse_like_game(f)
                if status != "ok":
                    parse_problems[(source, status)] += 1
                    if len(problem_examples) < 12:
                        problem_examples.append({"source": source, "status": status,
                                                 "file": f"{uid}/{f.relative_to(unit).as_posix()}"[:160]})
                if root is None:
                    per_source[source]["files_unparseable"] += 1
                    continue
                if root.tag != "Patch":
                    per_source[source]["files_root_not_patch"] += 1
                    continue
                per_source[source]["patch_files"] += 1
                if reachable:
                    per_source[source]["patch_files_reachable_16"] += 1
                for child in root:
                    if not isinstance(child.tag, str):
                        continue
                    if child.tag != "Operation":
                        per_source[source]["unexpected_root_children"] += 1
                        continue
                    found = []
                    collect_ops(child, 0, found, None)
                    for cls, depth, xps, extra, success, mayreq in found:
                        ops.append(OpRec(source, uid, file_counter, cls, depth, xps, extra, success, mayreq, reachable))
    t1 = time.time()

    # ----- operation class tally ------------------------------------------------------
    class_counts = collections.Counter()
    class_by_source = collections.defaultdict(collections.Counter)
    class_with_xpath = collections.Counter()
    depth_counts = collections.Counter()
    success_counts = collections.Counter()
    mayreq_ops = 0
    multi_xpath_ops = 0
    extra_fields = collections.Counter()
    for o in ops:
        name = o.cls or "(missing Class)"
        class_counts[name] += 1
        class_by_source[o.source][name] += 1
        depth_counts[min(o.depth, 5)] += 1
        if o.xpaths:
            class_with_xpath[name] += 1
        if len(o.xpaths) > 1:
            multi_xpath_ops += 1
        if o.success:
            success_counts[o.success] += 1
        if o.may_require:
            mayreq_ops += 1
        for tag, _txt in o.extra_xpath_fields:
            extra_fields[tag] += 1

    def is_vanilla(name):
        base = name[len("Verse."):] if name.startswith("Verse.") else name
        return base in VANILLA_ALL

    vanilla_ops = sum(c for n, c in class_counts.items() if is_vanilla(n))
    custom_ops = sum(c for n, c in class_counts.items() if not is_vanilla(n))
    custom_with_xpath = sum(c for n, c in class_with_xpath.items() if not is_vanilla(n))

    # ----- expressions ------------------------------------------------------------------
    exprs = []  # (source, unit, class, text, reachable)
    for o in ops:
        for x in o.xpaths:
            exprs.append((o.source, o.unit, o.cls or "(missing Class)", x, o.reachable_16))
    total_expr = len(exprs)

    cache: dict[str, tuple] = {}
    feat_totals = {k: collections.Counter() for k in ("axes", "tests", "preds", "funcs", "ops")}
    feat_expr_presence = {k: collections.Counter() for k in feat_totals}
    tag_expr = collections.Counter()
    prefixes = collections.Counter()
    variables = collections.Counter()
    unknown_funcs = collections.Counter()
    errors = collections.Counter()
    error_examples = {}
    tier_occ = collections.Counter()
    tier_distinct = collections.Counter()
    seen_distinct = set()
    unit_tier = {}
    lengths = []
    steps_per = []
    preds_per = []
    depth_per = []
    root_forms = collections.Counter()
    ws_flags = collections.Counter()
    shape_counts = collections.Counter()
    tier_by_source = collections.defaultdict(collections.Counter)
    tier_by_class_kind = collections.defaultdict(collections.Counter)
    tier_reach = collections.Counter()
    reach_total = 0
    blockers = collections.defaultdict(list)  # tag -> shortest examples
    tagsets_counter = collections.Counter()
    expr_tagsets = []  # frozenset of tags beyond A, for the adoption curve
    lib_disagree = collections.Counter()
    lib_disagree_examples = []

    for source, uid, cls, text, reach in exprs:
        key = text
        if key not in cache:
            stripped = text.strip()
            F, err, tier = analyse(stripped)
            # cross-check with libxml2 syntax acceptance
            try:
                etree.XPath(stripped)
                lib_ok = True
            except Exception:
                lib_ok = False
            mine_ok = err is None
            if mine_ok != lib_ok:
                kind = "mine_accepts_lxml_rejects" if mine_ok else "mine_rejects_lxml_accepts"
                lib_disagree[kind] += 1
                if len(lib_disagree_examples) < 8:
                    lib_disagree_examples.append({"kind": kind, "expr": stripped[:160], "mine": err})
            cache[key] = (F, err, tier, stripped)
        F, err, tier, stripped = cache[key]
        lengths.append(len(stripped))
        if text != text.strip():
            ws_flags["leading or trailing whitespace"] += 1
        if "\n" in text:
            ws_flags["contains a newline"] += 1
        if err:
            errors[err.split(" at ")[0][:60]] += 1
            error_examples.setdefault(err.split(" at ")[0][:60], stripped[:160])
            tier_occ["X"] += 1
            tier_by_source[source]["X"] += 1
            unit_tier[(source, uid)] = max(unit_tier.get((source, uid), "A"), "X", key=lambda t: TIER_ORDER[t])
            if text not in seen_distinct:
                seen_distinct.add(text)
                tier_distinct["X"] += 1
            continue
        for k, counter in (("axes", F.axes), ("tests", F.tests), ("preds", F.preds), ("funcs", F.funcs), ("ops", F.ops)):
            for name, c in counter.items():
                feat_totals[k][name] += c
                feat_expr_presence[k][name] += 1
        for t in F.tags:
            tag_expr[t] += 1
        for p in F.prefixes:
            prefixes[p] += 1
        for v in F.vars:
            variables[v] += 1
        for fn in F.unknown_funcs:
            unknown_funcs[fn] += 1
        steps_per.append(F.steps)
        preds_per.append(F.npreds)
        depth_per.append(F.maxdepth)
        # root form
        if stripped.startswith("//"):
            root_forms["//..."] += 1
        elif stripped.startswith("/Defs"):
            root_forms["/Defs/..."] += 1
        elif stripped.startswith("Defs"):
            root_forms["Defs/..."] += 1
        elif stripped.startswith("/"):
            root_forms["/other"] += 1
        elif stripped.startswith("*"):
            root_forms["*/..."] += 1
        elif stripped.startswith("("):
            root_forms["(...)"] += 1
        else:
            root_forms["other relative"] += 1
        tier_occ[tier] += 1
        tier_by_source[source][tier] += 1
        if text not in seen_distinct:
            seen_distinct.add(text)
            tier_distinct[tier] += 1
        unit_tier[(source, uid)] = max(unit_tier.get((source, uid), "A"), tier, key=lambda t: TIER_ORDER[t])
        kind = "vanilla" if (cls in VANILLA_ALL or cls.startswith("Verse.") and cls[6:] in VANILLA_ALL) else "custom"
        tier_by_class_kind[kind][tier] += 1
        if source == "workshop" or source == "ce_dev":
            pass
        reach_total += 1 if reach else 0
        if reach:
            tier_reach[tier] += 1
        beyond = frozenset(t for t in F.tags if TAG_TIER[t] != "A")
        expr_tagsets.append((beyond, tier))
        try:
            shape_counts[shape_of(stripped)] += 1
        except XPathSyntaxError:
            pass
        for t in beyond:
            lst = blockers[t]
            if len(lst) < 3 and stripped not in lst and len(stripped) <= 200:
                lst.append(stripped)
            elif lst and len(stripped) < max(len(s) for s in lst) and stripped not in lst and len(stripped) <= 200:
                lst[lst.index(max(lst, key=len))] = stripped

    # ----- tiers (cumulative) --------------------------------------------------------------
    ok_expr = total_expr - tier_occ["X"]
    cum = {}
    run = 0
    for t in ("A", "B", "C", "D"):
        run += tier_occ[t]
        cum[t] = run
    run_d = 0
    cum_distinct = {}
    for t in ("A", "B", "C", "D"):
        run_d += tier_distinct[t]
        cum_distinct[t] = run_d
    distinct_total = len(seen_distinct)
    distinct_ok = distinct_total - tier_distinct["X"]

    # per-unit (mod) coverage: share of units whose every expression lies within a tier
    unit_cov = {}
    by_src_units = collections.defaultdict(list)
    for (source, uid), t in unit_tier.items():
        by_src_units[source].append(t)
    for source, lst in by_src_units.items():
        c = collections.Counter(lst)
        n = len(lst)
        run = 0
        cov = {}
        for t in ("A", "B", "C", "D"):
            run += c[t]
            cov[t] = {"units": run, "pct": round(100 * run / n, 2)}
        cov["units_total"] = n
        cov["units_with_invalid_expr"] = c["X"]
        unit_cov[source] = cov

    # ----- feature adoption (greedy, starting from subset A) ---------------------------
    sets = [(s, t) for s, t in expr_tagsets]
    covered_by_a = sum(1 for s, t in sets if not s)
    remaining = [s for s, t in sets if s]
    # drop sets that contain a tag no engine feature can implement (tier X) from greedy
    remaining = [s for s in remaining if all(TAG_TIER[t] != "X" for t in s)]
    added: list[str] = []
    curve = [{"added": "subset A baseline", "covered": covered_by_a, "pct_of_valid": round(100 * covered_by_a / ok_expr, 2)}]
    covered = covered_by_a
    cur = set()
    while True:
        gain = collections.Counter()
        for s in remaining:
            need = s - cur
            if len(need) == 1:
                gain[next(iter(need))] += 1
        if not gain:
            break
        tag, g = gain.most_common(1)[0]
        cur.add(tag)
        added.append(tag)
        covered = covered_by_a + sum(1 for s in remaining if s <= cur)
        curve.append({"added": tag, "tier_of_tag": TAG_TIER[tag], "covered": covered,
                      "pct_of_valid": round(100 * covered / ok_expr, 2)})
        if len(added) >= 14:
            break

    # marginal value of each non-A tag when added alone to A
    solo = collections.Counter()
    for s, t in sets:
        if len(s) == 1:
            solo[next(iter(s))] += 1

    def pct(n, d=None):
        d = d or total_expr
        return round(100 * n / d, 2) if d else 0.0

    def counter_table(c: collections.Counter, presence: collections.Counter, top=None):
        rows = []
        for name, n in c.most_common(top):
            rows.append({"name": name, "occurrences": n, "expressions": presence[name],
                         "pct_of_valid_expressions": pct(presence[name], ok_expr)})
        return rows

    def percentile(vals, q):
        if not vals:
            return 0
        s = sorted(vals)
        return s[min(len(s) - 1, int(round(q * (len(s) - 1))))]

    hist_edges = [(0, 32), (33, 64), (65, 128), (129, 256), (257, 512), (513, 1024), (1025, 10**9)]
    length_hist = []
    for lo, hi in hist_edges:
        n = sum(1 for L in lengths if lo <= L <= hi)
        length_hist.append({"range": f"{lo}-{hi}" if hi < 10**9 else f"{lo}+", "count": n, "pct": pct(n)})

    summary = {
        "meta": {
            "generated": DATE,
            "script": "docs/research/data/xpath-corpus/xpath_coverage.py",
            "python": sys.version.split()[0],
            "lxml": etree.LXML_VERSION and ".".join(map(str, etree.LXML_VERSION)),
            "libxml2": ".".join(map(str, etree.LIBXML_VERSION)),
            "elapsed_seconds": round(time.time() - t0, 1),
            "scan_seconds": round(t1 - t0, 1),
            "game_version_assumed": "1.6.4871",
            "note": "raw expressions are not stored; examples are short excerpts",
        },
        "corpus": {
            "sources": {s: dict(c) for s, c in per_source.items()},
            "patches_dirs_by_name": {f"{s}:{n}": c for (s, n), c in sorted(dir_case.items())},
            "patches_dirs_by_location": {f"{s}:{n}": c for (s, n), c in sorted(folder_classes.items())},
            "units_with_patches": {s: sum(1 for k in unit_info if k[0] == s) for s in {k[0] for k in unit_info}},
            "xml_parse_status_outside_ok": {f"{s}:{st}": c for (s, st), c in parse_problems.items()},
            "xml_parse_problem_examples": problem_examples,
            "load_folder_methods": dict(reach_methods),
            "operations_total": len(ops),
            "expressions_total": total_expr,
            "expressions_distinct": distinct_total,
            "valid_expressions_in_folders_loaded_by_16_upper_bound": reach_total,
        },
        "operation_classes": {
            "distinct_classes": len(class_counts),
            "vanilla_operations": vanilla_ops,
            "custom_operations": custom_ops,
            "custom_operations_with_xpath": custom_with_xpath,
            "nesting_depth_counts": {str(k): v for k, v in sorted(depth_counts.items())},
            "success_field_values": dict(success_counts),
            "operations_with_mayrequire": mayreq_ops,
            "operations_with_multiple_xpath_children": multi_xpath_ops,
            "other_xpath_like_child_names": dict(extra_fields.most_common(15)),
            "vanilla": [{"class": n, "operations": c, "with_xpath": class_with_xpath[n]}
                        for n, c in class_counts.most_common() if is_vanilla(n)],
            "custom_top": [{"class": n, "operations": c, "with_xpath": class_with_xpath[n]}
                           for n, c in class_counts.most_common() if not is_vanilla(n)][:40],
            "by_source_top": {s: class_by_source[s].most_common(12) for s in class_by_source},
        },
        "xpath": {
            "valid_expressions": ok_expr,
            "invalid_expressions": tier_occ["X"],
            "invalid_reasons": dict(errors.most_common(12)),
            "invalid_examples": error_examples,
            "root_forms": dict(root_forms.most_common()),
            "whitespace": dict(ws_flags),
            "axes": counter_table(feat_totals["axes"], feat_expr_presence["axes"]),
            "node_tests": counter_table(feat_totals["tests"], feat_expr_presence["tests"]),
            "predicate_kinds": counter_table(feat_totals["preds"], feat_expr_presence["preds"]),
            "functions": counter_table(feat_totals["funcs"], feat_expr_presence["funcs"]),
            "operators": counter_table(feat_totals["ops"], feat_expr_presence["ops"]),
            "namespace_prefixes": dict(prefixes),
            "variables": dict(variables),
            "unknown_functions": dict(unknown_funcs),
            "length_chars": {
                "min": min(lengths) if lengths else 0, "p50": percentile(lengths, 0.5),
                "p90": percentile(lengths, 0.9), "p99": percentile(lengths, 0.99),
                "max": max(lengths) if lengths else 0,
                "mean": round(statistics.mean(lengths), 1) if lengths else 0,
                "histogram": length_hist,
            },
            "steps_per_expression": {"p50": percentile(steps_per, 0.5), "p90": percentile(steps_per, 0.9),
                                     "p99": percentile(steps_per, 0.99), "max": max(steps_per) if steps_per else 0},
            "predicates_per_expression": {"p50": percentile(preds_per, 0.5), "p90": percentile(preds_per, 0.9),
                                          "p99": percentile(preds_per, 0.99), "max": max(preds_per) if preds_per else 0},
            "predicate_nesting_depth": {str(d): sum(1 for x in depth_per if x == d) for d in sorted(set(depth_per))},
            "tag_expression_counts": {t: {"tier": TAG_TIER[t], "expressions": c,
                                          "pct_of_valid": pct(c, ok_expr)} for t, c in tag_expr.most_common()},
        },
        "tiers": {
            "definition": {
                "A": "child steps (name tests) with optional final @attribute; predicates only (@attr | child) = 'literal'",
                "B": "A + // . .. * and or not() contains() starts-with() != existence nested-path numeric-equality",
                "C": "B + [n] last() position() count() text() relational operators and unions",
                "D": "any other valid XPath 1.0 (explicit axes, other functions, arithmetic, filter expressions, node(), comment())",
                "X": "invalid for XmlDocument.SelectNodes (syntax error, unknown function, variable, prefix, non-node-set result)",
            },
            "occurrences": {t: tier_occ[t] for t in "ABCDX"},
            "cumulative_occurrences": {t: {"expressions": cum[t], "pct_of_all": pct(cum[t]), "pct_of_valid": pct(cum[t], ok_expr)} for t in "ABCD"},
            "distinct": {t: tier_distinct[t] for t in "ABCDX"},
            "cumulative_distinct": {t: {"expressions": cum_distinct[t], "pct_of_distinct_valid": round(100 * cum_distinct[t] / distinct_ok, 2) if distinct_ok else 0}
                                    for t in "ABCD"},
            "distinct_total": distinct_total,
            "per_source_occurrences": {s: dict(c) for s, c in tier_by_source.items()},
            "per_source_cumulative_pct_of_valid": {
                s: {t: round(100 * sum(c[x] for x in "ABCD"[: "ABCD".index(t) + 1]) / max(1, sum(c[x] for x in "ABCD")), 2) for t in "ABCD"}
                for s, c in tier_by_source.items()},
            "by_operation_kind": {k: dict(v) for k, v in tier_by_class_kind.items()},
            "units_fully_covered": unit_cov,
            "reachable_16_upper_bound_tier_counts": dict(tier_reach),
        },
        "feature_adoption": {
            "greedy_after_subset_A": curve,
            "solo_unlock_over_A": {t: c for t, c in solo.most_common(20)},
            "blocking_examples": {t: ex for t, ex in sorted(blockers.items(), key=lambda kv: -tag_expr[kv[0]])},
        },
        "top_shapes": [{"shape": s, "count": c, "pct": pct(c)} for s, c in shape_counts.most_common(40)],
        "parser_validation": {
            "expressions_checked_against_libxml2": len(cache),
            "disagreements": dict(lib_disagree),
            "disagreement_examples": lib_disagree_examples,
        },
    }
    args.out.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    if args.dump_expressions:
        with args.dump_expressions.open("w", encoding="utf-8") as fh:
            for text in sorted(seen_distinct):
                fh.write(json.dumps({"xpath": text}, ensure_ascii=False) + "\n")
    print(f"units={len(unit_info)} files={file_counter} ops={len(ops)} exprs={total_expr} distinct={distinct_total} "
          f"valid={ok_expr} elapsed={time.time()-t0:.1f}s")
    print("tier occurrences:", {t: tier_occ[t] for t in "ABCDX"})


if __name__ == "__main__":
    main()
