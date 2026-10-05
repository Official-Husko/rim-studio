#!/usr/bin/env python3
"""Survey of Combat Extended (CE) patch idioms across several corpora.

Static analysis only (no game code is executed).  The script reads patch XML
files, LoadFolders.xml and About.xml files, classifies every PatchOperation,
measures how patches are gated, how weapons, apparel, ammo and tools are
converted, and runs a set of lint rules that encode the most common mistakes.

Corpora (all read-only):
  ce-core        <ce-root>/Patches/**                  CE's own core patches
  ce-modpatches  <ce-root>/ModPatches/*/Patches/**     CE's integrated third-party patches
  ce-defs        <ce-root>/Defs/** and ModPatches/*/Defs/**   CE's ammo/def files
  owner          <owner-root>/<mod>/**                 the owner's own mods that mention CE
  workshop       <workshop-root>/<id>/**               third-party Steam mods that ship CE patches
                                                       (CE itself, id 2890901044, is excluded)

Usage:
  survey_ce_patches.py [survey] [--out FILE] [--examples N] [--skip-workshop]
  survey_ce_patches.py lint FILE_OR_DIR [FILE_OR_DIR ...]   (lint arbitrary patch files)

Paths come from arguments or environment variables (defaults in README.md of
this folder):  RIMSTUDIO_CE_ROOT, RIMSTUDIO_OWNER_MODS, RIMSTUDIO_WORKSHOP,
RIMSTUDIO_GAME_ROOT.  The script is deterministic: sorted traversal, no
timestamps, no randomness.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sys
from typing import Any, Dict, Iterable, Iterator, List, Optional, Set, Tuple

from lxml import etree

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_CE_ROOT = os.environ.get(
    "RIMSTUDIO_CE_ROOT",
    os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", "CombatExtended-Development")),
)
DEFAULT_OWNER = os.environ.get(
    "RIMSTUDIO_OWNER_MODS",
    "/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods",
)
DEFAULT_WORKSHOP = os.environ.get(
    "RIMSTUDIO_WORKSHOP",
    "/home/pawbeans/.steam/steam/steamapps/workshop/content/294100",
)
DEFAULT_GAME = os.environ.get(
    "RIMSTUDIO_GAME_ROOT",
    "/home/pawbeans/.steam/steam/steamapps/common/RimWorld",
)
DEFAULT_OUT = os.path.join(HERE, "survey-summary.json")
CE_WORKSHOP_ID = "2890901044"
CE_PACKAGE_ID = "ceteam.combatextended"
CE_MOD_NAME = "Combat Extended"

PATHED_CLASSES = {
    "PatchOperationAdd", "PatchOperationInsert", "PatchOperationRemove", "PatchOperationReplace",
    "PatchOperationAttributeAdd", "PatchOperationAttributeSet", "PatchOperationAttributeRemove",
    "PatchOperationAddModExtension", "PatchOperationSetName", "PatchOperationTest",
    "PatchOperationConditional",
}
VALUE_CLASSES = {
    "PatchOperationAdd", "PatchOperationInsert", "PatchOperationReplace", "PatchOperationAddModExtension",
}
# Containers that a def may or may not declare itself (they can be inherited).
OPTIONAL_CONTAINERS = {
    "statBases", "comps", "tools", "verbs", "weaponTags", "equippedStatOffsets", "costList",
    "modExtensions", "thingCategories", "tradeTags", "stuffCategories", "apparel", "recipeMaker",
}

PARSER_STRICT = etree.XMLParser(remove_comments=False, recover=False, resolve_entities=False, huge_tree=True)
PARSER_RECOVER = etree.XMLParser(remove_comments=False, recover=True, resolve_entities=False, huge_tree=True)


# ----------------------------------------------------------------------------
# small helpers
# ----------------------------------------------------------------------------
def kids(el: etree._Element) -> List[etree._Element]:
    return [c for c in el if isinstance(c.tag, str)]


def norm_ws(text: Optional[str]) -> str:
    return re.sub(r"\s+", " ", (text or "").strip())


def walk_files(root: str, pred) -> List[str]:
    out: List[str] = []
    for dp, dn, fn in os.walk(root):
        dn.sort()
        for f in sorted(fn):
            p = os.path.join(dp, f)
            if pred(p):
                out.append(p)
    return out


def read_text(path: str) -> str:
    with open(path, "rb") as fh:
        raw = fh.read()
    return raw.decode("utf-8-sig", errors="replace")


def parse_xml(path: str) -> Tuple[Optional[etree._ElementTree], Optional[str], bool]:
    """Return (tree, error, recovered)."""
    try:
        return etree.parse(path, PARSER_STRICT), None, False
    except etree.XMLSyntaxError as exc:
        try:
            return etree.parse(path, PARSER_RECOVER), str(exc)[:160], True
        except Exception as exc2:  # pragma: no cover - defensive
            return None, str(exc2)[:160], False
    except OSError as exc:
        return None, str(exc)[:160], False


def pct(n: int, d: int) -> float:
    return round(100.0 * n / d, 2) if d else 0.0


def top(counter: collections.Counter, n: int = 30) -> List[List[Any]]:
    return [[k if not isinstance(k, tuple) else list(k), v]
            for k, v in sorted(counter.items(), key=lambda kv: (-kv[1], str(kv[0])))[:n]]


def sorted_counter(counter: collections.Counter) -> Dict[str, int]:
    return {str(k): v for k, v in sorted(counter.items(), key=lambda kv: (-kv[1], str(kv[0])))}


def iter_ops(op: etree._Element, top_level: bool = True, parent=None, role=None, depth=0):
    """Yield (op, is_top_level, parent_op, role, depth) for an operation and every nested operation."""
    yield op, top_level, parent, role, depth
    for ch in kids(op):
        if ch.tag in ("match", "nomatch"):
            yield from iter_ops(ch, False, op, ch.tag, depth + 1)
        elif ch.tag == "operations":
            for li in kids(ch):
                yield from iter_ops(li, False, op, "operations", depth + 1)


# ----------------------------------------------------------------------------
# XPath analysis (string level, no evaluation)
# ----------------------------------------------------------------------------
def split_steps(xpath: str) -> List[str]:
    """Split on '/' that is outside [] and quotes."""
    steps: List[str] = []
    buf: List[str] = []
    depth = 0
    quote = ""
    i = 0
    while i < len(xpath):
        ch = xpath[i]
        if quote:
            buf.append(ch)
            if ch == quote:
                quote = ""
        elif ch in "\"'":
            quote = ch
            buf.append(ch)
        elif ch == "[":
            depth += 1
            buf.append(ch)
        elif ch == "]":
            depth -= 1
            buf.append(ch)
        elif ch == "/" and depth == 0:
            steps.append("".join(buf))
            buf = []
        else:
            buf.append(ch)
        i += 1
    steps.append("".join(buf))
    return steps


_EQ = re.compile(r"""^\s*(@?[A-Za-z_][\w]*)\s*=\s*(["'])(.*?)\2\s*$""")


def split_or(pred: str) -> List[str]:
    parts: List[str] = []
    buf: List[str] = []
    quote = ""
    i = 0
    n = len(pred)
    while i < n:
        ch = pred[i]
        if quote:
            buf.append(ch)
            if ch == quote:
                quote = ""
            i += 1
            continue
        if ch in "\"'":
            quote = ch
            buf.append(ch)
            i += 1
            continue
        m = re.match(r"\s+or\s+", pred[i:])
        if m and not quote:
            parts.append("".join(buf))
            buf = []
            i += m.end()
            continue
        buf.append(ch)
        i += 1
    parts.append("".join(buf))
    return parts


def analyze_xpath(xpath: str) -> Dict[str, Any]:
    """Classify an xpath string: target def type, selector kind, patched field."""
    xp = norm_ws(xpath)
    res: Dict[str, Any] = {
        "absolute": xp.startswith("/"),
        "descendant": "//" in xp,
        "deftype": None,
        "selector": "none",
        "n_alt": 0,
        "names": [],
        "field": None,
        "tail": [],
        "last_is_li": False,
    }
    steps = [s for s in split_steps(xp.lstrip("/")) if s != ""]
    if not steps:
        res["selector"] = "empty"
        return res
    if steps[0] != "Defs":
        res["selector"] = "not-defs"
        return res
    if len(steps) < 2:
        res["selector"] = "defs-only"
        return res
    step = steps[1]
    m = re.match(r"^([A-Za-z0-9_.*]+)(\[(.*)\])?$", step, re.S)
    if not m:
        res["selector"] = "other"
        return res
    res["deftype"] = m.group(1)
    pred = m.group(3)
    if pred is None:
        res["selector"] = "type-only"
    else:
        low = pred
        if re.search(r"\b(starts-with|contains|ends-with|matches|normalize-space|translate)\s*\(", low):
            res["selector"] = "fuzzy"
        elif re.search(r"\band\b|\bnot\s*\(", low):
            res["selector"] = "compound"
        else:
            parts = split_or(pred)
            kinds, names = set(), []
            ok = True
            for part in parts:
                mm = _EQ.match(part)
                if not mm:
                    ok = False
                    break
                kinds.add(mm.group(1))
                names.append(mm.group(3))
            if ok and kinds == {"defName"}:
                res["selector"] = "defName"
            elif ok and kinds == {"@Name"}:
                res["selector"] = "@Name"
            elif ok and kinds == {"@ParentName"}:
                res["selector"] = "@ParentName"
            elif ok:
                res["selector"] = "mixed-eq"
            else:
                res["selector"] = "other-predicate"
            res["names"] = names if ok else []
            res["n_alt"] = len(names) if ok else 0
    rest = steps[2:]
    if rest:
        res["field"] = re.sub(r"\[.*$", "", rest[0], flags=re.S)
        res["tail"] = [re.sub(r"\[.*$", "", s, flags=re.S) for s in rest[1:]]
        res["last_is_li"] = bool(re.match(r"^li\b", rest[-1]))
    return res


# ----------------------------------------------------------------------------
# CE source schema (class names and fields) used by lint rules
# ----------------------------------------------------------------------------
def load_ce_schema(ce_root: str) -> Dict[str, Any]:
    src = os.path.join(ce_root, "Source")
    classes: Set[str] = set()
    fields: Dict[str, Set[str]] = {}
    if not os.path.isdir(src):
        return {"classes": classes, "fields": fields}
    class_re = re.compile(r"^\s*(?:public|internal|protected|private)?\s*(?:static\s+|abstract\s+|sealed\s+|partial\s+)*"
                          r"(?:class|struct|enum|interface)\s+([A-Za-z_][\w]*)", re.M)
    ns_re = re.compile(r"^\s*namespace\s+([A-Za-z_][\w.]*)", re.M)
    field_re = re.compile(r"^\s*public\s+(?!class|enum|struct|static|override|virtual|abstract|const|readonly)"
                          r"[\w<>\[\],.? ]+?\s+([A-Za-z_][\w]*)\s*(?:=[^;]*)?;", re.M)
    for path in walk_files(src, lambda p: p.endswith(".cs")):
        try:
            text = read_text(path)
        except OSError:
            continue
        nss = ns_re.findall(text)
        ns = nss[0] if nss else ""
        for m in class_re.finditer(text):
            name = m.group(1)
            classes.add(name)
            if ns:
                classes.add(ns + "." + name)
        # fields per class (coarse: attribute fields to the first class in the file)
        cm = class_re.search(text)
        if cm:
            fields.setdefault(cm.group(1), set()).update(field_re.findall(text))
    return {"classes": classes, "fields": fields}


# ----------------------------------------------------------------------------
# Def index (names of defs) for reference-resolution lints
# ----------------------------------------------------------------------------
DEF_NODE_SKIP = {"Patch", "LanguageData"}


def index_defs(paths: Iterable[str], index: Dict[str, Set[str]], abstract_names: Optional[Dict[str, Set[str]]] = None) -> int:
    n = 0
    for path in paths:
        tree, err, _ = parse_xml(path)
        if tree is None:
            continue
        root = tree.getroot()
        if root.tag != "Defs":
            continue
        for d in kids(root):
            dn = d.findtext("defName")
            tag = d.tag
            if dn:
                index.setdefault(tag, set()).add(dn.strip())
                index.setdefault("*", set()).add(dn.strip())
                n += 1
            nm = d.get("Name")
            if nm and abstract_names is not None:
                abstract_names.setdefault(tag, set()).add(nm)
    return n


def defs_files(root: str) -> List[str]:
    return walk_files(root, lambda p: p.lower().endswith(".xml"))


# ----------------------------------------------------------------------------
# Mod metadata: About.xml, LoadFolders.xml
# ----------------------------------------------------------------------------
def find_ci(directory: str, name: str) -> Optional[str]:
    """Find a file in `directory` ignoring case."""
    try:
        for fn in os.listdir(directory):
            if fn.lower() == name.lower():
                return os.path.join(directory, fn)
    except OSError:
        return None
    return None


def parse_about(mod_dir: str) -> Dict[str, Any]:
    out: Dict[str, Any] = {"name": "", "packageId": "", "supportedVersions": [], "loadAfter": [],
                           "loadBefore": [], "deps": [], "found": False}
    about_dir = None
    for cand in ("About", "about"):
        p = os.path.join(mod_dir, cand)
        if os.path.isdir(p):
            about_dir = p
            break
    if not about_dir:
        return out
    ap = find_ci(about_dir, "About.xml")
    if not ap:
        return out
    tree, err, _ = parse_xml(ap)
    if tree is None:
        return out
    r = tree.getroot()
    out["found"] = True
    out["name"] = (r.findtext("name") or "").strip()
    out["packageId"] = (r.findtext("packageId") or "").strip()
    out["supportedVersions"] = [(li.text or "").strip() for li in r.findall("supportedVersions/li")]
    out["loadAfter"] = [(li.text or "").strip() for li in r.findall("loadAfter/li")]
    out["loadBefore"] = [(li.text or "").strip() for li in r.findall("loadBefore/li")]
    out["deps"] = [(li.findtext("packageId") or "").strip() for li in r.findall("modDependencies/li")]
    return out


def parse_loadfolders(mod_dir: str) -> Optional[Dict[str, List[Dict[str, Any]]]]:
    p = find_ci(mod_dir, "LoadFolders.xml")
    if not p:
        return None
    tree, err, _ = parse_xml(p)
    if tree is None:
        return None
    res: Dict[str, List[Dict[str, Any]]] = {}
    for ver in kids(tree.getroot()):
        key = ver.tag.lower()
        if key.startswith("v"):
            key = key[1:]
        lst = res.setdefault(key, [])
        for li in kids(ver):
            def split(v: Optional[str]) -> List[str]:
                return [x.strip() for x in v.split(",")] if v else []
            folder = (li.text or "").strip().replace("\\", "/").strip("/")
            lst.append({
                "folder": folder,
                "any": split(li.get("IfModActive")),
                "all": split(li.get("IfModActiveAll")),
                "not": split(li.get("IfModNotActive")),
            })
    return res


def select_load_entries(lf: Optional[Dict[str, List[Dict[str, Any]]]], game_version=(1, 6)) -> Optional[List[Dict[str, Any]]]:
    """Mimic ModContentPack.InitLoadFolders version selection (exact, then highest lower, then default)."""
    if not lf:
        return None
    best = None
    bestv = None
    for key in lf:
        if key == "default":
            continue
        m = re.match(r"^(\d+)\.(\d+)", key)
        if not m:
            continue
        v = (int(m.group(1)), int(m.group(2)))
        if v <= game_version and (bestv is None or v > bestv):
            best, bestv = key, v
    if best is not None and lf[best]:
        return lf[best]
    if "default" in lf and lf["default"]:
        return lf["default"]
    return None


def owning_folder(rel_path: str) -> str:
    """Folder (relative to mod root, '' for root) whose Patches/ directory holds this file."""
    parts = rel_path.replace("\\", "/").split("/")
    for i, part in enumerate(parts[:-1]):
        if part.lower() == "patches":
            return "/".join(parts[:i])
    return "/".join(parts[:-1])


def gate_for_file(rel_path: str, entries: Optional[List[Dict[str, Any]]]) -> Dict[str, Any]:
    """How is a patch file gated by LoadFolders?  Returns dict(kind, ids)."""
    folder = owning_folder(rel_path)
    if entries is None:
        # implicit loading: root, Common, and the best version folder
        first = folder.split("/")[0] if folder else ""
        if folder == "" or first.lower() == "common" or re.match(r"^v?\d+\.\d+", first or ""):
            return {"kind": "implicit-loaded", "ids": []}
        return {"kind": "never-loaded", "ids": []}
    matches = [e for e in entries if e["folder"].lower() == folder.lower()]
    if not matches:
        return {"kind": "never-loaded", "ids": []}
    ids: List[str] = []
    unconditional = False
    for e in matches:
        if not e["any"] and not e["all"] and not e["not"]:
            unconditional = True
        ids += e["any"] + e["all"]
    if unconditional:
        return {"kind": "loaded", "ids": []}
    return {"kind": "conditional", "ids": sorted(set(ids)), "not": any(e["not"] for e in matches)}


# ----------------------------------------------------------------------------
# Patch file collection
# ----------------------------------------------------------------------------
class PatchFile:
    __slots__ = ("corpus", "path", "rel", "mod", "tree", "root", "error", "recovered", "bom",
                 "text", "gate", "dir_case_ok", "ext_case_ok")

    def __init__(self, corpus: str, path: str, rel: str, mod: str):
        self.corpus, self.path, self.rel, self.mod = corpus, path, rel, mod
        self.tree = None
        self.root = None
        self.error = None
        self.recovered = False
        self.bom = False
        self.text = ""
        self.gate: Dict[str, Any] = {"kind": "n/a", "ids": []}
        self.dir_case_ok = True
        self.ext_case_ok = True

    def load(self) -> "PatchFile":
        with open(self.path, "rb") as fh:
            raw = fh.read()
        self.bom = raw.startswith(b"\xef\xbb\xbf")
        self.text = raw.decode("utf-8-sig", errors="replace")
        tree, err, rec = parse_xml(self.path)
        self.tree, self.error, self.recovered = tree, err, rec
        if tree is not None:
            self.root = tree.getroot()
        self.ext_case_ok = self.path.endswith(".xml")
        parts = self.rel.split("/")
        self.dir_case_ok = not any(p.lower() == "patches" and p != "Patches" for p in parts[:-1])
        return self

    @property
    def ops(self) -> List[etree._Element]:
        if self.root is None or self.root.tag != "Patch":
            return []
        return kids(self.root)


def is_patch_dir_file(rel: str) -> bool:
    parts = rel.replace("\\", "/").split("/")
    return any(p.lower() == "patches" for p in parts[:-1]) and rel.lower().endswith(".xml")


def collect_ce_core(ce_root: str) -> List[PatchFile]:
    base = os.path.join(ce_root, "Patches")
    out = []
    for p in walk_files(base, lambda q: q.lower().endswith(".xml")):
        rel = os.path.relpath(p, ce_root).replace(os.sep, "/")
        out.append(PatchFile("ce-core", p, rel, "core"))
    return out


def collect_ce_modpatches(ce_root: str) -> List[PatchFile]:
    base = os.path.join(ce_root, "ModPatches")
    out = []
    for p in walk_files(base, lambda q: q.lower().endswith(".xml") and is_patch_dir_file(os.path.relpath(q, base))):
        relb = os.path.relpath(p, base).replace(os.sep, "/")
        folder = owning_folder(relb)
        out.append(PatchFile("ce-modpatches", p, "ModPatches/" + relb, folder))
    return out


MENTIONS_CE = re.compile(r"combat\s*extended|combatextended", re.I)


def collect_third_party(corpus: str, root: str, skip_ids: Set[str]) -> Tuple[List[PatchFile], Dict[str, Dict[str, Any]]]:
    """Patch files that mention CE in <root>/<mod>/..., plus per-mod metadata."""
    files: List[PatchFile] = []
    mods: Dict[str, Dict[str, Any]] = {}
    if not os.path.isdir(root):
        return files, mods
    for mod in sorted(os.listdir(root)):
        mdir = os.path.join(root, mod)
        if not os.path.isdir(mdir) or mod in skip_ids:
            continue
        cands = []
        for p in walk_files(mdir, lambda q: q.lower().endswith(".xml")):
            rel = os.path.relpath(p, mdir).replace(os.sep, "/")
            low = rel.lower()
            if "/obj/" in "/" + low or low.startswith("languages/") or "/languages/" in low:
                continue
            if not is_patch_dir_file(rel):
                continue
            try:
                with open(p, "rb") as fh:
                    head = fh.read()
            except OSError:
                continue
            if MENTIONS_CE.search(head.decode("utf-8", errors="replace")):
                cands.append((p, rel))
        if not cands:
            continue
        about = parse_about(mdir)
        lf = parse_loadfolders(mdir)
        entries = select_load_entries(lf)
        mods[mod] = {"about": about, "has_loadfolders": lf is not None, "entries": entries,
                     "loadfolders_raw": lf, "dir": mdir}
        for p, rel in cands:
            pf = PatchFile(corpus, p, rel, mod)
            files.append(pf)
    return files, mods


# ----------------------------------------------------------------------------
# Operation scanning
# ----------------------------------------------------------------------------
CE_PREFIX = "CombatExtended."
MAKEGUN = "CombatExtended.PatchOperationMakeGunCECompatible"
CE_FINDMOD = "CombatExtended.PatchOperationFindMod"
CE_SETTINGS = "CombatExtended.PatchOperationSettingsConditional"
PKGID_LIKE = re.compile(r"^[A-Za-z0-9_]+(\.[A-Za-z0-9_]+)+$")
CE_NAME_LIKE = re.compile(r"combat\s*extended|combatextended", re.I)


def text_of(el: Optional[etree._Element]) -> Optional[str]:
    if el is None:
        return None
    return el.text if el.text is not None else ""


def value_node(op: etree._Element) -> Optional[etree._Element]:
    return op.find("value")


def ce_classes_in(el: etree._Element) -> List[str]:
    out = []
    for e in el.iter():
        if isinstance(e.tag, str):
            c = e.get("Class")
            if c and c.startswith(CE_PREFIX):
                out.append(c)
    return out


def all_classes_in(el: etree._Element) -> List[str]:
    out = []
    for e in el.iter():
        if isinstance(e.tag, str):
            c = e.get("Class")
            if c:
                out.append(c)
    return out


def makegun_details(op: etree._Element) -> Dict[str, Any]:
    d: Dict[str, Any] = {}
    sections = [c.tag for c in kids(op)]
    d["sections"] = sections
    d["defName"] = (op.findtext("defName") or "").strip()
    props = op.find("Properties")
    d["props"] = [c.tag for c in kids(props)] if props is not None else None
    d["verbClass"] = norm_ws(props.findtext("verbClass")) if props is not None else None
    d["defaultProjectile"] = norm_ws(props.findtext("defaultProjectile")) if props is not None else None
    au = op.find("AmmoUser")
    d["ammo"] = [c.tag for c in kids(au)] if au is not None else None
    d["ammoSet"] = norm_ws(au.findtext("ammoSet")) if au is not None else None
    d["magazineSize"] = norm_ws(au.findtext("magazineSize")) if au is not None else None
    fm = op.find("FireModes")
    d["fire"] = [c.tag for c in kids(fm)] if fm is not None else None
    sb = op.find("statBases")
    d["stats"] = [c.tag for c in kids(sb)] if sb is not None else None
    d["stat_values"] = {c.tag: norm_ws(c.text) for c in kids(sb)} if sb is not None else {}
    cl = op.find("costList")
    d["cost"] = [c.tag for c in kids(cl)] if cl is not None else None
    wt = op.find("weaponTags")
    d["tags"] = [norm_ws(li.text) for li in kids(wt)] if wt is not None else []
    d["research"] = norm_ws(op.findtext("researchPrerequisite")) if op.find("researchPrerequisite") is not None else None
    d["allowRunAndGun"] = norm_ws(op.findtext("AllowWithRunAndGun")) if op.find("AllowWithRunAndGun") is not None else None
    d["texPath"] = op.find("texPath") is not None
    d["platform"] = op.find("isWeaponPlatform") is not None or op.find("attachmentLinks") is not None
    d["prop_values"] = {c.tag: norm_ws(c.text) for c in kids(props)} if props is not None else {}
    d["ammo_values"] = {c.tag: norm_ws(c.text) for c in kids(au)} if au is not None else {}
    return d


def tool_entries(op: etree._Element) -> List[Dict[str, Any]]:
    """Every <li> that is a melee tool in the op value (inside a <tools> node, or the value itself for Add to .../tools)."""
    out = []
    val = value_node(op)
    if val is None:
        return out
    xp = norm_ws(op.findtext("xpath"))
    candidates: List[etree._Element] = []
    for t in val.iter("tools"):
        candidates += kids(t)
    if not candidates and re.search(r"/tools$", xp):
        candidates = [c for c in kids(val) if c.tag == "li"]
    for li in candidates:
        if li.tag != "li":
            continue
        out.append({
            "class": li.get("Class") or "",
            "children": [c.tag for c in kids(li)],
            "caps": [norm_ws(c.text) for c in li.findall("capacities/li")],
            "line": li.sourceline,
        })
    return out


def scan_file_ops(pf: PatchFile) -> List[Dict[str, Any]]:
    records: List[Dict[str, Any]] = []
    for top_op in pf.ops:
        if top_op.tag != "Operation":
            records.append({"corpus": pf.corpus, "mod": pf.mod, "file": pf.rel, "line": top_op.sourceline,
                            "cls": "(not-Operation:%s)" % top_op.tag, "top": True, "role": None, "depth": 0,
                            "parent_cls": None, "bad_child": True})
            continue
        for op, is_top, parent, role, depth in iter_ops(top_op):
            cls = op.get("Class") or ""
            rec: Dict[str, Any] = {
                "corpus": pf.corpus, "mod": pf.mod, "file": pf.rel, "line": op.sourceline,
                "cls": cls, "top": is_top, "role": role, "depth": depth,
                "parent_cls": (parent.get("Class") if parent is not None else None),
                "mayrequire": op.get("MayRequire") is not None or op.get("MayRequireAnyOf") is not None,
                "success": norm_ws(op.findtext("success")) if op.find("success") is not None else None,
                "order": norm_ws(op.findtext("order")) if op.find("order") is not None else None,
            }
            xpath_el = op.find("xpath")
            if xpath_el is not None:
                raw = "".join(xpath_el.itertext())
                rec["xpath"] = norm_ws(raw)
                rec["xp"] = analyze_xpath(raw)
            val = value_node(op)
            if val is not None:
                rec["value_tags"] = [c.tag for c in kids(val)]
                rec["value_ce_classes"] = ce_classes_in(val)
                rec["value_classes"] = all_classes_in(val)
                rec["value_children_text"] = [(c.tag, norm_ws(c.text)) for c in kids(val)][:12]
            if cls in VALUE_CLASSES:
                rec["tools"] = tool_entries(op)
            if cls == "PatchOperationFindMod":
                rec["mods"] = [li.text if li.text is not None else "" for li in op.findall("mods/li")]
                rec["has_match"] = op.find("match") is not None
                rec["has_nomatch"] = op.find("nomatch") is not None
            if cls == CE_FINDMOD:
                rec["modName"] = text_of(op.find("modName"))
            if cls == CE_SETTINGS:
                rec["settingName"] = norm_ws(op.findtext("settingName"))
            if cls == MAKEGUN:
                rec["mg"] = makegun_details(op)
            if cls == "PatchOperationSequence":
                first = None
                ops_el = op.find("operations")
                if ops_el is not None and kids(ops_el):
                    first = kids(ops_el)[0].get("Class")
                rec["first_nested"] = first
                rec["n_nested"] = len(kids(ops_el)) if ops_el is not None else 0
            if cls in ("PatchOperationConditional", "PatchOperationFindMod", CE_SETTINGS):
                for role_name in ("match", "nomatch"):
                    child = op.find(role_name)
                    rec["has_" + role_name] = child is not None
                    rec[role_name + "_cls"] = child.get("Class") if child is not None else None
                    if child is not None and child.find("value") is not None:
                        rec[role_name + "_value_tags"] = [c.tag for c in kids(child.find("value"))]
                        rec[role_name + "_xpath"] = norm_ws("".join(child.find("xpath").itertext())) if child.find("xpath") is not None else None
            if op.get("MayRequire") is not None:
                rec["mayrequire_value"] = op.get("MayRequire")
            records.append(rec)
    return records


# ----------------------------------------------------------------------------
# Context: def indexes, known tags, attested field names
# ----------------------------------------------------------------------------
def vanilla_def_files(game_root: str) -> List[str]:
    out: List[str] = []
    data = os.path.join(game_root, "Data")
    if not os.path.isdir(data):
        return out
    for d in sorted(os.listdir(data)):
        p = os.path.join(data, d, "Defs")
        if os.path.isdir(p):
            out += walk_files(p, lambda q: q.lower().endswith(".xml"))
    return out


def observed_fields(files: List[str]) -> Dict[str, Set[str]]:
    """Child element names observed under verbs/li, tools/li, projectile, comps/li in vanilla defs."""
    res: Dict[str, Set[str]] = {"verb": set(), "tool": set(), "projectile": set(), "comp": set()}
    for f in files:
        t, _, _ = parse_xml(f)
        if t is None:
            continue
        for d in kids(t.getroot()):
            for li in d.findall("verbs/li"):
                res["verb"].update(c.tag for c in kids(li))
            for li in d.findall("tools/li"):
                res["tool"].update(c.tag for c in kids(li))
            p = d.find("projectile")
            if p is not None:
                res["projectile"].update(c.tag for c in kids(p))
            for li in d.findall("comps/li"):
                res["comp"].update(c.tag for c in kids(li))
    return res


def known_ce_tags(ce_root: str, core_files: List[PatchFile]) -> Set[str]:
    tags: Set[str] = set()
    tag_re = re.compile(r"\bCE_[A-Za-z0-9_]+\b")
    for base in ("Defs", "Patches"):
        for p in walk_files(os.path.join(ce_root, base), lambda q: q.lower().endswith(".xml")):
            try:
                tags.update(tag_re.findall(read_text(p)))
            except OSError:
                pass
    for p in walk_files(os.path.join(ce_root, "Source"), lambda q: q.endswith(".cs")):
        try:
            tags.update(tag_re.findall(read_text(p)))
        except OSError:
            pass
    return tags


class Context:
    def __init__(self, ce_root: str, owner_root: str, workshop_root: str, game_root: str,
                 skip_workshop: bool = False):
        self.ce_root = ce_root
        self.owner_root, self.workshop_root, self.game_root = owner_root, workshop_root, game_root
        self.schema = load_ce_schema(ce_root)
        self.files: Dict[str, List[PatchFile]] = {}
        self.mods: Dict[str, Dict[str, Dict[str, Any]]] = {"owner": {}, "workshop": {}}
        self.records: List[Dict[str, Any]] = []
        self.skip_workshop = skip_workshop
        self.def_index: Dict[str, Set[str]] = {}
        self.def_index_modpatch_defs: Dict[str, Set[str]] = {}
        self.ce_load_entries: List[Dict[str, Any]] = []
        self.vanilla_fields: Dict[str, Set[str]] = {}
        self.ce_tags: Set[str] = set()

    def load_all(self) -> None:
        core = [pf.load() for pf in collect_ce_core(self.ce_root)]
        mp = [pf.load() for pf in collect_ce_modpatches(self.ce_root)]
        lf = parse_loadfolders(self.ce_root) or {}
        self.ce_load_entries = select_load_entries(lf) or []
        for pf in core:
            pf.gate = {"kind": "loaded", "ids": []}
        for pf in mp:
            pf.gate = gate_for_file(pf.rel, self.ce_load_entries)
        self.files["ce-core"] = core
        self.files["ce-modpatches"] = mp
        owner, omods = collect_third_party("owner", self.owner_root, set())
        self.mods["owner"] = omods
        self.files["owner"] = [pf.load() for pf in owner]
        if self.skip_workshop:
            ws, wmods = [], {}
        else:
            ws, wmods = collect_third_party("workshop", self.workshop_root, {CE_WORKSHOP_ID})
        self.mods["workshop"] = wmods
        self.files["workshop"] = [pf.load() for pf in ws]
        for corpus in ("owner", "workshop"):
            for pf in self.files[corpus]:
                pf.gate = gate_for_file(pf.rel, self.mods[corpus][pf.mod]["entries"])
        for corpus, lst in self.files.items():
            for pf in lst:
                self.records += scan_file_ops(pf)
        # def indexes
        ce_defs = defs_files(os.path.join(self.ce_root, "Defs"))
        mp_defs = [p for p in walk_files(os.path.join(self.ce_root, "ModPatches"), lambda q: q.lower().endswith(".xml"))
                   if "/Defs/" in p]
        index_defs(ce_defs, self.def_index)
        index_defs(mp_defs, self.def_index_modpatch_defs)
        index_defs(vanilla_def_files(self.game_root), self.def_index)
        self.vanilla_fields = observed_fields(vanilla_def_files(self.game_root))
        self.ce_tags = known_ce_tags(self.ce_root, core)
        self.ce_def_files = ce_defs
        self.modpatch_def_files = mp_defs

    def all_files(self) -> List[PatchFile]:
        out: List[PatchFile] = []
        for lst in self.files.values():
            out += lst
        return out

    def is_known_def(self, name: str, extra: Optional[Set[str]] = None) -> bool:
        if name in self.def_index.get("*", set()) or name in self.def_index_modpatch_defs.get("*", set()):
            return True
        return bool(extra and name in extra)


# ----------------------------------------------------------------------------
# Lint rules (static validation checklist, machine-checkable subset)
# ----------------------------------------------------------------------------
RULES: Dict[str, str] = {
    "CEP001": "FindMod entry looks like a packageId; FindMod compares the About.xml <name>, so it never matches",
    "CEP002": "FindMod entry spells Combat Extended differently from the exact name 'Combat Extended'",
    "CEP003": "FindMod entry has leading or trailing whitespace; names are compared without trimming",
    "CEP004": "CE-specific operation class in a patch file that is loaded even when CE is absent (load-time errors without CE)",
    "CEP005": "MayRequire on a top-level <Operation>; it is only honoured on <li> list items, so it is ignored here",
    "CEP007": "same defName converted more than once with PatchOperationMakeGunCECompatible in one mod",
    "CEP008": "PatchOperationMakeGunCECompatible missing a section or a key field (Properties, AmmoUser, FireModes, verbClass, defaultProjectile, ammoSet, magazineSize)",
    "CEP009": "Properties/verbClass is not a CombatExtended verb class",
    "CEP010": "Class attribute names a CombatExtended type that does not exist in the CE source",
    "CEP011": "ToolCE entry without armorPenetrationSharp and armorPenetrationBlunt",
    "CEP012": "tools replaced with entries that carry no Class attribute (vanilla Tool, no armor penetration)",
    "CEP013": "ammoSet name not defined in CE, ModPatches or the patched mod (upper bound: patched mod unavailable)",
    "CEP014": "defaultProjectile name not defined in CE, ModPatches, vanilla or the patched mod (upper bound)",
    "CEP015": "field name inside Properties/AmmoUser/FireModes/ToolCE is not a known field (typo, wrong case or obsolete)",
    "CEP016": "CE_ weapon tag unknown to CE core data and source",
    "CEP017": "patch file does not parse, has a root other than <Patch>, or has a child other than <Operation>",
    "CEP018": "patch file sits in a folder that LoadFolders.xml never loads",
    "CEP019": "LoadFolders IfModActive uses an id variant that cannot match (for example a _copy suffix)",
    "CEP020": "Patches folder or .xml extension spelled with different case (breaks on case-sensitive file systems)",
    "CEP021": "exact duplicate operation (same class, xpath and value) inside one mod",
    "CEP022": "xpath is statically malformed (root not Defs, unbalanced brackets, bare literal or bare name inside a predicate)",
}


class Lint:
    def __init__(self, max_examples: int):
        self.max_examples = max_examples
        self.counts: Dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
        self.examples: Dict[str, Dict[str, List[Dict[str, Any]]]] = collections.defaultdict(lambda: collections.defaultdict(list))
        self.mods: Dict[str, Dict[str, Set[str]]] = collections.defaultdict(lambda: collections.defaultdict(set))

    def hit(self, rule: str, corpus: str, mod: str, file: str, line: Optional[int], detail: str = "") -> None:
        self.counts[rule][corpus] += 1
        self.mods[rule][corpus].add(mod)
        ex = self.examples[rule][corpus]
        if len(ex) < self.max_examples:
            ex.append({"file": file, "line": line, "detail": detail[:200]})

    def summary(self) -> Dict[str, Any]:
        out: Dict[str, Any] = {}
        for rule in sorted(RULES):
            out[rule] = {
                "description": RULES[rule],
                "hits": dict(sorted(self.counts[rule].items())),
                "mods_affected": {c: len(m) for c, m in sorted(self.mods[rule].items())},
                "examples": {c: v for c, v in sorted(self.examples[rule].items())},
            }
        return out


def xpath_defects(xpath: str) -> List[str]:
    xp = norm_ws(xpath)
    defects: List[str] = []
    if xp.count("[") != xp.count("]"):
        defects.append("unbalanced-brackets")
    stripped = xp.lstrip("/")
    if not xp.startswith("//") and stripped and stripped.split("/")[0] != "Defs":
        defects.append("root-not-Defs")
    steps = [s for s in split_steps(stripped) if s]
    if len(steps) >= 2:
        m = re.match(r"^([^\[]+)\[(.*)\]$", steps[1], re.S)
        if m:
            pred = m.group(2)
            if re.search(r"\b(and|or)\s+[\"']", pred):
                defects.append("bare-literal-in-predicate")
            for part in split_or(pred):
                if re.match(r"^\s*[A-Za-z_][\w]*\s*$", part) and not re.search(r"\s(and|or)\s", part):
                    defects.append("bare-name-in-predicate")
                    break
            if re.search(r"\band\b", pred) and not re.search(r"[<>=]", pred.split(" and ")[-1]) and re.search(r"\band\s+\"", pred):
                defects.append("and-between-literals")
    return sorted(set(defects))


def run_lints(ctx: Context, max_examples: int) -> Lint:
    lint = Lint(max_examples)
    schema_classes = ctx.schema["classes"]
    classes_ci = {c.lower() for c in schema_classes}
    fields = ctx.schema["fields"]
    vf = ctx.vanilla_fields
    verb_fields = set(fields.get("VerbPropertiesCE", set())) | vf["verb"] | {"verbClass"}
    ammo_fields = set(fields.get("CompProperties_AmmoUser", set())) | {"compClass"}
    fire_fields = set(fields.get("CompProperties_FireModes", set())) | {"compClass"}
    tool_fields = set(fields.get("ToolCE", set())) | vf["tool"]
    # attested set: fields used by CE core patches themselves
    attested: Dict[str, Set[str]] = {"verb": set(), "ammo": set(), "fire": set(), "tool": set()}
    for r in ctx.records:
        if r["corpus"] == "ce-core" and r["cls"] == MAKEGUN:
            mg = r["mg"]
            attested["verb"].update(mg["props"] or [])
            attested["ammo"].update(mg["ammo"] or [])
            attested["fire"].update(mg["fire"] or [])
    verb_fields |= attested["verb"]
    ammo_fields |= attested["ammo"]
    fire_fields |= attested["fire"]

    mod_defs_cache: Dict[Tuple[str, str], Set[str]] = {}

    def mod_defs(corpus: str, mod: str) -> Set[str]:
        key = (corpus, mod)
        if key in mod_defs_cache:
            return mod_defs_cache[key]
        names: Set[str] = set()
        if corpus in ctx.mods and mod in ctx.mods[corpus]:
            mdir = ctx.mods[corpus][mod]["dir"]
            idx: Dict[str, Set[str]] = {}
            paths = [p for p in walk_files(mdir, lambda q: q.lower().endswith(".xml"))
                     if re.search(r"/Defs/", p, re.I) and "/Languages/" not in p]
            index_defs(paths, idx)
            names = idx.get("*", set())
        mod_defs_cache[key] = names
        return names

    # ----- file-level rules
    for pf in ctx.all_files():
        c = pf.corpus
        if pf.error or (pf.root is not None and pf.root.tag != "Patch"):
            lint.hit("CEP017", c, pf.mod, pf.rel, None, (pf.error or ("root=" + str(pf.root.tag))) if pf.root is not None or pf.error else "")
        if pf.root is not None and pf.root.tag == "Patch":
            for ch in kids(pf.root):
                if ch.tag != "Operation":
                    lint.hit("CEP017", c, pf.mod, pf.rel, ch.sourceline, "child <%s>" % ch.tag)
        if pf.gate["kind"] == "never-loaded":
            lint.hit("CEP018", c, pf.mod, pf.rel, None, owning_folder(pf.rel))
        if not pf.dir_case_ok or not pf.ext_case_ok:
            lint.hit("CEP020", c, pf.mod, pf.rel, None, "dir_case_ok=%s ext_case_ok=%s" % (pf.dir_case_ok, pf.ext_case_ok))

    # ----- LoadFolders id variants
    def check_lf(corpus: str, mod: str, entries_by_ver: Optional[Dict[str, List[Dict[str, Any]]]], where: str) -> None:
        if not entries_by_ver:
            return
        for ver, lst in entries_by_ver.items():
            for e in lst:
                for pid in e["any"] + e["all"] + e["not"]:
                    low = pid.lower()
                    if low.endswith("_copy") or low.endswith("_local"):
                        lint.hit("CEP019", corpus, mod, where, None, "%s in %s" % (pid, ver))

    for corpus in ("owner", "workshop"):
        for mod, info in ctx.mods[corpus].items():
            check_lf(corpus, mod, info["loadfolders_raw"], "%s/LoadFolders.xml" % mod)
    check_lf("ce-modpatches", "CE", parse_loadfolders(ctx.ce_root), "LoadFolders.xml")

    # ----- op-level rules
    seen_mg: Dict[Tuple[str, str, str], List[Dict[str, Any]]] = collections.defaultdict(list)
    dup_keys: Dict[Tuple[str, str, str, str, str], int] = collections.Counter()
    file_gate = {pf.rel: pf for pf in ctx.all_files()}
    for r in ctx.records:
        c, mod, f, ln = r["corpus"], r["mod"], r["file"], r["line"]
        cls = r["cls"]
        if r.get("bad_child"):
            continue
        # FindMod strings
        names: List[str] = []
        if cls == "PatchOperationFindMod":
            names = r.get("mods", [])
        elif cls == CE_FINDMOD and r.get("modName") is not None:
            names = [r["modName"]]
        for nm in names:
            if nm != nm.strip():
                lint.hit("CEP003", c, mod, f, ln, repr(nm))
            s = nm.strip()
            if PKGID_LIKE.match(s):
                lint.hit("CEP001", c, mod, f, ln, s)
            if CE_NAME_LIKE.search(s) and s != CE_MOD_NAME:
                lint.hit("CEP002", c, mod, f, ln, repr(nm))
        if r["top"] and r.get("mayrequire"):
            lint.hit("CEP005", c, mod, f, ln, cls)
        # CE class outside CE-gated folders (third party only)
        if c in ("owner", "workshop") and cls.startswith(CE_PREFIX):
            pf = file_gate[f]
            minfo = ctx.mods[c][mod]
            standalone = any(d.lower() == CE_PACKAGE_ID for d in minfo["about"]["deps"])
            gated_lf = pf.gate["kind"] == "conditional" and any(i.lower().split("_")[0] == CE_PACKAGE_ID for i in pf.gate["ids"])
            if not gated_lf and not standalone:
                lint.hit("CEP004", c, mod, f, ln, cls)
        # unknown CE classes
        for cn in [cls] + r.get("value_ce_classes", []):
            if cn.startswith(CE_PREFIX) and cn not in schema_classes:
                if cn.lower() in classes_ci:
                    lint.hit("CEP010", c, mod, f, ln, cn + " (case differs; resolved case-insensitively by the game)")
                else:
                    lint.hit("CEP010", c, mod, f, ln, cn)
        # xpath defects
        if "xpath" in r:
            for d in xpath_defects(r["xpath"]):
                lint.hit("CEP022", c, mod, f, ln, d + ": " + r["xpath"][:120])
            dup_keys[(c, mod, cls, r["xpath"], str(r.get("value_children_text")))] += 1
        # tools
        if "tools" in r:
            for t in r["tools"]:
                if t["class"] == CE_PREFIX + "ToolCE":
                    ch = set(t["children"])
                    if "armorPenetrationSharp" not in ch and "armorPenetrationBlunt" not in ch:
                        lint.hit("CEP011", c, mod, f, t["line"], "caps=%s" % t["caps"])
                    for tag in t["children"]:
                        if tag not in tool_fields:
                            lint.hit("CEP015", c, mod, f, t["line"], "ToolCE/%s" % tag)
                elif t["class"] == "" and cls == "PatchOperationReplace" and r.get("xp") and r["xp"]["field"] == "tools":
                    lint.hit("CEP012", c, mod, f, t["line"], "caps=%s" % t["caps"])
        # make-gun checks
        if cls == MAKEGUN:
            mg = r["mg"]
            seen_mg[(c, mod, mg["defName"])].append(r)
            missing = []
            if mg["props"] is None:
                missing.append("Properties")
            else:
                for k in ("verbClass", "defaultProjectile"):
                    if k not in mg["props"]:
                        missing.append("Properties/" + k)
            one_use = (mg["verbClass"] or "").endswith("OneUse") or (mg["verbClass"] or "").endswith("OneUseStatic")
            if mg["ammo"] is None and not one_use and mg["props"] is not None:
                missing.append("AmmoUser")
            elif mg["ammo"] is not None:
                if "ammoSet" not in mg["ammo"]:
                    missing.append("AmmoUser/ammoSet")
            if mg["fire"] is None and not one_use and mg["props"] is not None:
                missing.append("FireModes")
            for m in missing:
                lint.hit("CEP008", c, mod, f, ln, "%s: %s" % (mg["defName"], m))
            vc = mg["verbClass"]
            if vc and not vc.startswith(CE_PREFIX):
                lint.hit("CEP009", c, mod, f, ln, "%s: %s" % (mg["defName"], vc))
            for tag in mg["props"] or []:
                if tag not in verb_fields:
                    lint.hit("CEP015", c, mod, f, ln, "Properties/%s (%s)" % (tag, mg["defName"]))
            for tag in mg["ammo"] or []:
                if tag not in ammo_fields:
                    lint.hit("CEP015", c, mod, f, ln, "AmmoUser/%s (%s)" % (tag, mg["defName"]))
            for tag in mg["fire"] or []:
                if tag not in fire_fields:
                    lint.hit("CEP015", c, mod, f, ln, "FireModes/%s (%s)" % (tag, mg["defName"]))
            extra = mod_defs(c, mod) if c in ("owner", "workshop") else set()
            if mg["ammoSet"] and not ctx.is_known_def(mg["ammoSet"], extra):
                lint.hit("CEP013", c, mod, f, ln, "%s: %s" % (mg["defName"], mg["ammoSet"]))
            if mg["defaultProjectile"] and not ctx.is_known_def(mg["defaultProjectile"], extra):
                lint.hit("CEP014", c, mod, f, ln, "%s: %s" % (mg["defName"], mg["defaultProjectile"]))
            for tg in mg["tags"]:
                if tg.startswith("CE_") and tg not in ctx.ce_tags:
                    lint.hit("CEP016", c, mod, f, ln, "%s: %s" % (mg["defName"], tg))
        # weaponTags added directly
        if cls == "PatchOperationAdd" and r.get("xp") and r["xp"]["field"] == "weaponTags" and not r["xp"]["tail"]:
            for tag, txt in r.get("value_children_text", []):
                if txt.startswith("CE_") and txt not in ctx.ce_tags:
                    lint.hit("CEP016", c, mod, f, ln, txt)
    for (c, mod, dn), lst in seen_mg.items():
        if dn and len(lst) > 1:
            lint.hit("CEP007", c, mod, lst[1]["file"], lst[1]["line"], "%s x%d" % (dn, len(lst)))
    for (c, mod, cls, xp, vt), n in dup_keys.items():
        if n > 1 and cls in VALUE_CLASSES | {"PatchOperationRemove", "PatchOperationAttributeSet"}:
            lint.hit("CEP021", c, mod, "", None, "%s x%d %s" % (cls, n, xp[:100]))
    return lint


# ----------------------------------------------------------------------------
# Defs survey (CE core Defs and ModPatches Defs): ammo sets, ammo, projectiles
# ----------------------------------------------------------------------------
def survey_defs(ctx: Context) -> Dict[str, Any]:
    nodes: List[Dict[str, Any]] = []
    for label, files in (("ce-core-defs", ctx.ce_def_files), ("ce-modpatches-defs", ctx.modpatch_def_files)):
        for p in files:
            t, err, _ = parse_xml(p)
            if t is None or t.getroot().tag != "Defs":
                continue
            rel = os.path.relpath(p, ctx.ce_root).replace(os.sep, "/")
            for d in kids(t.getroot()):
                nodes.append({
                    "corpus": label, "file": rel, "tag": d.tag, "defName": (d.findtext("defName") or "").strip(),
                    "Name": d.get("Name"), "ParentName": d.get("ParentName"), "Class": d.get("Class"),
                    "abstract": (d.get("Abstract") or "").lower() == "true", "node": d, "line": d.sourceline,
                })
    out: Dict[str, Any] = {"files": {"ce-core-defs": len(ctx.ce_def_files), "ce-modpatches-defs": len(ctx.modpatch_def_files)}}
    by_corpus_tags: Dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for n in nodes:
        by_corpus_tags[n["corpus"]][n["tag"] + (" [Class=%s]" % n["Class"] if n["Class"] else "")] += 1
    out["def_node_kinds"] = {c: top(cnt, 14) for c, cnt in sorted(by_corpus_tags.items())}
    # name map for ParentName resolution (CE data only, first definition wins)
    name_map: Dict[str, Dict[str, Any]] = {}
    for n in nodes:
        if n["Name"] and n["Name"] not in name_map:
            name_map[n["Name"]] = n

    def tag_list(n: Dict[str, Any], field: str, depth: int = 0) -> List[str]:
        el = n["node"].find(field)
        own = [norm_ws(li.text) for li in kids(el)] if el is not None else []
        if el is not None and (el.get("Inherit") or "").lower() == "false":
            return own
        parent = name_map.get(n["ParentName"]) if n["ParentName"] else None
        base = tag_list(parent, field, depth + 1) if parent and depth < 12 else []
        return base + own

    def field_value(n: Dict[str, Any], path: str, depth: int = 0) -> Optional[str]:
        el = n["node"].find(path)
        if el is not None and el.text is not None:
            return norm_ws(el.text)
        parent = name_map.get(n["ParentName"]) if n["ParentName"] else None
        return field_value(parent, path, depth + 1) if parent and depth < 12 else None

    # duplicates
    seen: Dict[Tuple[str, str], List[Dict[str, Any]]] = collections.defaultdict(list)
    for n in nodes:
        if n["defName"]:
            seen[(n["tag"], n["defName"])].append(n)
    dups = [(k, v) for k, v in seen.items() if len(v) > 1]
    out["duplicate_defnames"] = {
        "count": len(dups),
        "examples": [{"def": "%s/%s" % k, "files": [x["file"] for x in v][:4]} for k, v in sorted(dups)[:12]],
    }
    # ammo sets
    sets = [n for n in nodes if n["tag"].endswith("AmmoSetDef")]
    sizes = collections.Counter(len(kids(n["node"].find("ammoTypes"))) if n["node"].find("ammoTypes") is not None else 0 for n in sets)
    ammo_defs = {n["defName"]: n for n in nodes if n["defName"] and (n["Class"] or "").endswith("AmmoDef")}
    thing_defs = {n["defName"]: n for n in nodes if n["defName"] and n["tag"] == "ThingDef"}
    recipes = {n["defName"] for n in nodes if n["tag"] == "RecipeDef" and n["defName"]}
    dangling_ammo = dangling_proj = dup_class = 0
    similar = 0
    for n in sets:
        at = n["node"].find("ammoTypes")
        if n["node"].find("similarTo") is not None:
            similar += 1
        classes_seen: List[str] = []
        for link in kids(at) if at is not None else []:
            amm = link.tag
            proj = (link.text or "").strip()
            known_a = amm in ammo_defs or ctx.is_known_def(amm)
            known_p = proj in thing_defs or ctx.is_known_def(proj)
            if not known_a:
                dangling_ammo += 1
            if not known_p:
                dangling_proj += 1
            a = ammo_defs.get(amm)
            if a is not None:
                classes_seen.append(field_value(a, "ammoClass") or "")
        if len([c for c in classes_seen if c]) != len(set(c for c in classes_seen if c)):
            dup_class += 1
    out["ammo_sets"] = {
        "count": len(sets),
        "size_histogram": {str(k): v for k, v in sorted(sizes.items())},
        "with_similarTo": similar,
        "dangling_ammo_links": dangling_ammo,
        "dangling_projectile_links": dangling_proj,
        "sets_with_duplicate_ammoClass": dup_class,
    }
    # ammo defs: crafting recipe naming rule and trade tags
    craft_missing = []
    tag_counter: collections.Counter = collections.Counter()
    parent_counter: collections.Counter = collections.Counter()
    ammo_class_counter: collections.Counter = collections.Counter()
    concrete_ammo = [n for n in ammo_defs.values() if not n["abstract"]]
    for n in concrete_ammo:
        tags = tag_list(n, "tradeTags")
        for t in tags:
            tag_counter[t] += 1
        parent_counter[n["ParentName"] or "(none)"] += 1
        ac = field_value(n, "ammoClass")
        ammo_class_counter[ac or "(none)"] += 1
        if any(t.startswith("CE_AutoEnableCrafting") for t in tags):
            if ("Make" + n["defName"]) not in recipes:
                craft_missing.append(n["defName"])
    out["ammo_defs"] = {
        "concrete_count": len(concrete_ammo),
        "abstract_count": len([n for n in ammo_defs.values() if n["abstract"]]),
        "trade_tags": top(tag_counter, 14),
        "parents": top(parent_counter, 14),
        "ammo_class": top(ammo_class_counter, 20),
        "crafting_tag_without_Make_recipe": {"count": len(craft_missing), "examples": sorted(craft_missing)[:10]},
        "recipes_named_MakeAmmo": len([r for r in recipes if r.startswith("MakeAmmo_")]),
    }
    # projectiles
    proj_fields: collections.Counter = collections.Counter()
    proj_parents: collections.Counter = collections.Counter()
    thing_class: collections.Counter = collections.Counter()
    n_proj = 0
    for n in nodes:
        if n["tag"] != "ThingDef" or n["abstract"]:
            continue
        p = n["node"].find("projectile")
        if p is None:
            continue
        n_proj += 1
        proj_parents[n["ParentName"] or "(none)"] += 1
        thing_class[field_value(n, "thingClass") or "(inherited-or-none)"] += 1
        for c in kids(p):
            proj_fields[c.tag] += 1
    out["projectiles"] = {
        "concrete_with_projectile_node": n_proj,
        "parents": top(proj_parents, 14),
        "thingClass": top(thing_class, 10),
        "fields_in_own_projectile_node": top(proj_fields, 30),
    }
    return out


# ----------------------------------------------------------------------------
# Aggregation
# ----------------------------------------------------------------------------
CORPORA = ("ce-core", "ce-modpatches", "owner", "workshop")


def short_cls(cls: str) -> str:
    return cls.replace("PatchOperation", "").replace("CombatExtended.", "CE.") or "(none)"


def op_signature(r: Dict[str, Any]) -> str:
    cls = short_cls(r["cls"])
    xp = r.get("xp")
    if cls == "CE.MakeGunCECompatible":
        return "MakeGun"
    if cls == "AddModExtension":
        return "AddModExt:" + ",".join(sorted({c.replace("CombatExtended.", "") for c in r.get("value_ce_classes", [])} or {"other"}))
    if xp and xp.get("field"):
        tail = "/" + "/".join(xp["tail"][:1]) if xp["tail"] else ""
        return "%s:%s%s" % (cls, xp["field"], tail)
    if xp:
        return "%s:<def>" % cls
    return cls


def build_corpus_overview(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        files = ctx.files.get(c, [])
        recs = [r for r in ctx.records if r["corpus"] == c]
        mods = {pf.mod for pf in files}
        out[c] = {
            "patch_files": len(files),
            "mods_or_folders": len(mods),
            "operations_top_level": sum(1 for r in recs if r["top"]),
            "operations_nested": sum(1 for r in recs if not r["top"]),
            "files_with_bom": sum(1 for pf in files if pf.bom),
            "files_with_parse_error": sum(1 for pf in files if pf.error),
            "files_using_ce_classes": sum(1 for pf in files if any(
                r["file"] == pf.rel and (r["cls"].startswith(CE_PREFIX) or r.get("value_ce_classes")) for r in [])),
            "bytes": sum(len(pf.text.encode("utf-8")) for pf in files),
        }
    return out


def build_operation_classes(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        recs = [r for r in ctx.records if r["corpus"] == c and not r.get("bad_child")]
        top_c = collections.Counter(r["cls"] for r in recs if r["top"])
        nest_c = collections.Counter(r["cls"] for r in recs if not r["top"])
        files_with: Dict[str, Set[str]] = collections.defaultdict(set)
        for r in recs:
            files_with[r["cls"]].add(r["file"])
        nfiles = len(ctx.files.get(c, [])) or 1
        total = sum(top_c.values()) + sum(nest_c.values())
        out[c] = {
            "total_operations": total,
            "top_level": [[k, v, pct(v, sum(top_c.values()))] for k, v in sorted(top_c.items(), key=lambda kv: (-kv[1], kv[0]))],
            "nested": [[k, v, pct(v, sum(nest_c.values()))] for k, v in sorted(nest_c.items(), key=lambda kv: (-kv[1], kv[0]))],
            "files_containing": {k: [len(v), pct(len(v), nfiles)] for k, v in sorted(files_with.items(), key=lambda kv: (-len(kv[1]), kv[0]))},
        }
    return out


def build_xpath_stats(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        recs = [r for r in ctx.records if r["corpus"] == c and r.get("xp")]
        n = len(recs)
        sel = collections.Counter(r["xp"]["selector"] for r in recs)
        alt = collections.Counter()
        for r in recs:
            if r["xp"]["selector"] == "defName":
                k = r["xp"]["n_alt"]
                alt["1" if k == 1 else "2-3" if k <= 3 else "4-9" if k <= 9 else "10+"] += 1
        deftype = collections.Counter(r["xp"]["deftype"] for r in recs if r["xp"]["deftype"])
        field = collections.Counter((short_cls(r["cls"]), r["xp"]["field"] or "<def>") for r in recs)
        abstract_targets = collections.Counter()
        for r in recs:
            if r["xp"]["selector"] in ("@Name", "@ParentName"):
                for nm in r["xp"]["names"]:
                    abstract_targets[nm] += 1
        out[c] = {
            "xpath_operations": n,
            "selector_kinds": {k: [v, pct(v, n)] for k, v in sorted(sel.items(), key=lambda kv: (-kv[1], kv[0]))},
            "defName_alternatives": dict(alt),
            "absolute_leading_slash": [sum(1 for r in recs if r["xp"]["absolute"]), pct(sum(1 for r in recs if r["xp"]["absolute"]), n)],
            "descendant_axis": sum(1 for r in recs if r["xp"]["descendant"]),
            "def_node_types": top(deftype, 12),
            "op_class_by_field": top(field, 40),
            "last_step_is_list_item": sum(1 for r in recs if r["xp"]["last_is_li"]),
            "abstract_base_targets": top(abstract_targets, 25),
        }
    return out


def build_sequences(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        bigrams: collections.Counter = collections.Counter()
        trigrams: collections.Counter = collections.Counter()
        by_file: Dict[str, List[str]] = collections.defaultdict(list)
        for r in ctx.records:
            if r["corpus"] == c and r["top"] and not r.get("bad_child"):
                by_file[r["file"]].append(op_signature(r))
        for sigs in by_file.values():
            for i in range(len(sigs) - 1):
                bigrams[(sigs[i], sigs[i + 1])] += 1
            for i in range(len(sigs) - 2):
                trigrams[(sigs[i], sigs[i + 1], sigs[i + 2])] += 1
        out[c] = {"bigrams": top(bigrams, 12), "trigrams": top(trigrams, 12)}
    return out


def build_idioms(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        recs = [r for r in ctx.records if r["corpus"] == c and not r.get("bad_child")]
        ensure = collections.Counter()
        cond_shapes = collections.Counter()
        for r in recs:
            if r["cls"] == "PatchOperationConditional":
                cond_shapes[("match=%s" % short_cls(r["match_cls"]) if r.get("match_cls") else "match=-",
                             "nomatch=%s" % short_cls(r["nomatch_cls"]) if r.get("nomatch_cls") else "nomatch=-")] += 1
                xp = r.get("xp")
                if xp and r.get("nomatch_cls") == "PatchOperationAdd" and xp["field"] and not xp["tail"]:
                    if r.get("nomatch_value_tags") == [xp["field"]]:
                        ensure[xp["field"]] += 1
        # container-add guard analysis (per file order, top level only)
        total = guarded = 0
        by_file: Dict[str, List[Dict[str, Any]]] = collections.defaultdict(list)
        for r in recs:
            if r["top"]:
                by_file[r["file"]].append(r)
        unguarded_fields = collections.Counter()
        for f, lst in by_file.items():
            guard: Set[Tuple[str, str]] = set()
            for r in lst:
                xp = r.get("xp")
                if not xp or xp["selector"] != "defName":
                    continue
                if r["cls"] == "PatchOperationConditional" and xp["field"] in OPTIONAL_CONTAINERS and not xp["tail"] \
                        and r.get("nomatch_cls") == "PatchOperationAdd":
                    for nm in xp["names"]:
                        guard.add((nm, xp["field"]))
                if r["cls"] in ("PatchOperationAdd", "PatchOperationReplace") and not xp["field"]:
                    for tag in r.get("value_tags", []):
                        for nm in xp["names"]:
                            guard.add((nm, tag))
                if r["cls"] == "PatchOperationAdd" and xp["field"] in OPTIONAL_CONTAINERS and not xp["tail"]:
                    total += 1
                    if all((nm, xp["field"]) in guard for nm in xp["names"]):
                        guarded += 1
                    else:
                        unguarded_fields[xp["field"]] += 1
        sigs = collections.Counter()
        for r in recs:
            if r["top"]:
                sigs[op_signature(r)] += 1
        out[c] = {
            "conditional_branch_shapes": top(cond_shapes, 10),
            "ensure_container_idiom_by_field": top(ensure, 12),
            "add_into_optional_container": {"total": total, "preceded_by_ensure_or_def_add": guarded,
                                            "not_guarded_in_file": total - guarded,
                                            "unguarded_by_field": top(unguarded_fields, 10)},
            "top_level_signatures": top(sigs, 30),
        }
    return out


def build_gating(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        recs = [r for r in ctx.records if r["corpus"] == c and r["top"] and not r.get("bad_child")]
        kinds: collections.Counter = collections.Counter()
        seq_success: collections.Counter = collections.Counter()
        for r in recs:
            cls = r["cls"]
            if cls == "PatchOperationFindMod":
                k = "FindMod(vanilla)"
                if r.get("has_match") and not r.get("has_nomatch"):
                    k += " match-only"
                elif r.get("has_nomatch") and not r.get("has_match"):
                    k += " nomatch-only"
                elif r.get("has_match") and r.get("has_nomatch"):
                    k += " match+nomatch"
            elif cls == CE_FINDMOD:
                k = "CE.FindMod(top-level)"
            elif cls == "PatchOperationSequence":
                fn = r.get("first_nested")
                k = "Sequence[first=%s]" % (short_cls(fn) if fn else "-")
                seq_success[(k, r.get("success") or "default")] += 1
            elif cls == "PatchOperationConditional":
                k = "Conditional(xpath)"
            elif cls == CE_SETTINGS:
                k = "CE.SettingsConditional"
            else:
                k = "direct"
            if r.get("mayrequire"):
                k += " +MayRequire"
            kinds[k] += 1
        n = sum(kinds.values()) or 1
        out[c] = {
            "top_level_operations": sum(kinds.values()),
            "gate_kinds": [[k, v, pct(v, n)] for k, v in sorted(kinds.items(), key=lambda kv: (-kv[1], kv[0]))],
            "sequence_success_flags": top(seq_success, 12),
        }
        # file level LoadFolders gating
        gates = collections.Counter(pf.gate["kind"] for pf in ctx.files.get(c, []))
        ids = collections.Counter()
        for pf in ctx.files.get(c, []):
            if pf.gate["kind"] == "conditional":
                ids[",".join(i for i in pf.gate["ids"])] += 1
        out[c]["file_gate_by_loadfolders"] = dict(sorted(gates.items()))
        out[c]["conditional_gate_ids_top"] = top(ids, 12)
    # detection strings
    strings: Dict[str, collections.Counter] = {c: collections.Counter() for c in CORPORA}
    for r in ctx.records:
        if r.get("bad_child"):
            continue
        if r["cls"] == "PatchOperationFindMod":
            for m in r.get("mods", []):
                strings[r["corpus"]][m.strip()] += 1
        elif r["cls"] == CE_FINDMOD:
            strings[r["corpus"]]["[CE.FindMod] " + (r.get("modName") or "").strip()] += 1
    out["findmod_strings_top"] = {c: top(cnt, 15) for c, cnt in strings.items()}
    mr: Dict[str, collections.Counter] = {c: collections.Counter() for c in CORPORA}
    for r in ctx.records:
        if r.get("mayrequire_value"):
            mr[r["corpus"]][r["mayrequire_value"]] += 1
    out["mayrequire_values_top"] = {c: top(cnt, 10) for c, cnt in mr.items() if cnt}
    # CE detection in LoadFolders
    lf_ce = collections.Counter()
    for corpus in ("owner", "workshop"):
        for mod, info in ctx.mods[corpus].items():
            for ver, lst in (info["loadfolders_raw"] or {}).items():
                for e in lst:
                    for pid in e["any"] + e["all"]:
                        if pid.lower().split("_")[0] == CE_PACKAGE_ID:
                            lf_ce[(corpus, pid)] += 1
    out["loadfolders_ce_ids"] = top(lf_ce, 12)
    return out


def mod_style_summary(ctx: Context) -> Dict[str, Any]:
    """Per third-party mod: which integration style(s) does it use for its CE patches."""
    out: Dict[str, Any] = {}
    for corpus in ("owner", "workshop"):
        per_mod: Dict[str, Dict[str, Any]] = {}
        recs_by_mod: Dict[str, List[Dict[str, Any]]] = collections.defaultdict(list)
        for r in ctx.records:
            if r["corpus"] == corpus and not r.get("bad_child"):
                recs_by_mod[r["mod"]].append(r)
        files_by_mod: Dict[str, List[PatchFile]] = collections.defaultdict(list)
        for pf in ctx.files[corpus]:
            files_by_mod[pf.mod].append(pf)
        style_counter: collections.Counter = collections.Counter()
        for mod, info in sorted(ctx.mods[corpus].items()):
            about = info["about"]
            recs = recs_by_mod.get(mod, [])
            top_recs = [r for r in recs if r["top"]]
            standalone = any(d.lower() == CE_PACKAGE_ID for d in about["deps"])
            lf_ce = any(pf.gate["kind"] == "conditional" and any(i.lower().split("_")[0] == CE_PACKAGE_ID for i in pf.gate["ids"])
                        for pf in files_by_mod[mod])
            fm_ce = any(r["cls"] in ("PatchOperationFindMod", CE_FINDMOD, "PatchOperationSequence") and (
                (r["cls"] == "PatchOperationFindMod" and any(CE_NAME_LIKE.search(m or "") for m in r.get("mods", [])))
                or (r["cls"] == CE_FINDMOD) or (r["cls"] == "PatchOperationSequence" and r.get("first_nested") == CE_FINDMOD))
                for r in top_recs)
            direct_ce = any(r["cls"] not in ("PatchOperationFindMod", CE_FINDMOD, "PatchOperationSequence", "PatchOperationConditional")
                            and (r["cls"].startswith(CE_PREFIX) or r.get("value_ce_classes")) for r in top_recs)
            ce_ops = sum(1 for r in recs if r["cls"].startswith(CE_PREFIX) or r.get("value_ce_classes"))
            loadafter_ce = any(x.lower() == CE_PACKAGE_ID for x in about["loadAfter"])
            styles = []
            if lf_ce:
                styles.append("loadfolders-ce-folder")
            if fm_ce:
                styles.append("findmod-name")
            if standalone:
                styles.append("hard-dependency-on-ce")
            if not lf_ce and not fm_ce and not standalone:
                styles.append("ungated-or-other")
            for s_ in styles:
                style_counter[s_] += 1
            per_mod[mod] = {
                "name": about["name"], "packageId": about["packageId"], "styles": styles,
                "patch_files_mentioning_ce": len(files_by_mod[mod]),
                "operations": len(recs), "ops_using_ce_types": ce_ops,
                "loadAfter_ce": loadafter_ce,
                "supports_1_6": any(v.startswith("1.6") for v in about["supportedVersions"]),
                "has_loadfolders": info["has_loadfolders"],
            }
        out[corpus] = {"mods": per_mod, "style_counts": dict(sorted(style_counter.items(), key=lambda kv: -kv[1])),
                       "mod_count": len(per_mod),
                       "loadAfter_ce_count": sum(1 for m in per_mod.values() if m["loadAfter_ce"])}
    return out


def to_float(s: Any) -> Optional[float]:
    try:
        return float(str(s).strip())
    except (TypeError, ValueError):
        return None


def quantiles(vals: List[float]) -> Dict[str, float]:
    if not vals:
        return {}
    v = sorted(vals)

    def q(p: float) -> float:
        i = (len(v) - 1) * p
        lo, hi = int(i), min(int(i) + 1, len(v) - 1)
        return round(v[lo] + (v[hi] - v[lo]) * (i - lo), 4)
    return {"n": len(v), "min": round(v[0], 4), "p10": q(0.10), "median": q(0.5), "p90": q(0.9), "max": round(v[-1], 4)}


def build_weapons(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        mgs = [r for r in ctx.records if r["corpus"] == c and r["cls"] == MAKEGUN]
        if not mgs:
            out[c] = {"make_gun_operations": 0}
            continue
        n = len(mgs)
        d = [r["mg"] for r in mgs]
        sect = collections.Counter()
        for m in d:
            for s_ in set(m["sections"]):
                sect[s_] += 1
        props = collections.Counter()
        ammo = collections.Counter()
        fire = collections.Counter()
        stats = collections.Counter()
        tags = collections.Counter()
        tag_kind = collections.Counter()
        for m in d:
            props.update(set(m["props"] or []))
            ammo.update(set(m["ammo"] or []))
            fire.update(set(m["fire"] or []))
            stats.update(set(m["stats"] or []))
            for t in set(m["tags"]):
                tags[t] += 1
                tag_kind["CE_AI_*" if t.startswith("CE_AI_") else "CE_*" if t.startswith("CE_") else "Bipod_*" if t.startswith("Bipod_") else "other"] += 1
        vc = collections.Counter(m["verbClass"] or "(missing)" for m in d)
        ammoset = collections.Counter(m["ammoSet"] or "(none)" for m in d)
        proj = collections.Counter((m["defaultProjectile"] or "(none)").split("_")[0] + "_*" for m in d)
        research = collections.Counter(m["research"] or "(none)" for m in d)
        runandgun = collections.Counter(m["allowRunAndGun"] or "(absent)" for m in d)
        aim = collections.Counter()
        burst_mode = collections.Counter()
        for r in mgs:
            av = r["mg"]
            fm = None
        nums: Dict[str, List[float]] = collections.defaultdict(list)
        for m in d:
            for k, v in m["stat_values"].items():
                f = to_float(v)
                if f is not None:
                    nums["statBases/" + k].append(f)
            for k in ("recoilAmount", "range", "warmupTime", "burstShotCount", "ticksBetweenBurstShots", "minRange", "muzzleFlashScale"):
                f = to_float(m["prop_values"].get(k))
                if f is not None:
                    nums["Properties/" + k].append(f)
            for k in ("magazineSize", "reloadTime", "AmmoGenPerMagOverride"):
                f = to_float(m["ammo_values"].get(k))
                if f is not None:
                    nums["AmmoUser/" + k].append(f)
        ro = sum(1 for m in d if (m["ammo_values"].get("reloadOneAtATime") or "").lower() == "true")
        out[c] = {
            "make_gun_operations": n,
            "distinct_defNames": len({m["defName"] for m in d}),
            "sections_present": {k: [v, pct(v, n)] for k, v in sorted(sect.items(), key=lambda kv: (-kv[1], kv[0]))},
            "Properties_children": top(props, 40),
            "AmmoUser_children": top(ammo, 10),
            "FireModes_children": top(fire, 10),
            "statBases_names": top(stats, 30),
            "verbClass": top(vc, 8),
            "ammoSet_top": top(ammoset, 25),
            "projectile_name_prefix": top(proj, 6),
            "weaponTag_kinds": top(tag_kind, 6),
            "weaponTags_top": top(tags, 40),
            "researchPrerequisite_top": top(research, 15),
            "AllowWithRunAndGun": top(runandgun, 6),
            "reloadOneAtATime_true": ro,
            "numeric_ranges": {k: quantiles(v) for k, v in sorted(nums.items())},
        }
    return out


def build_tools(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        entries = []
        n_ops = 0
        for r in ctx.records:
            if r["corpus"] == c and r.get("tools"):
                n_ops += 1
                entries += r["tools"]
        if not entries:
            out[c] = {"tool_entries": 0}
            continue
        ap = collections.Counter()
        classes = collections.Counter()
        caps = collections.Counter()
        childfreq = collections.Counter()
        for t in entries:
            classes[t["class"] or "(no Class: vanilla Tool)"] += 1
            ch = set(t["children"])
            has_s, has_b = "armorPenetrationSharp" in ch, "armorPenetrationBlunt" in ch
            ap["both" if has_s and has_b else "sharp-only" if has_s else "blunt-only" if has_b else "none"] += 1
            for cp in t["caps"]:
                caps[cp] += 1
            childfreq.update(ch)
        n = len(entries)
        out[c] = {
            "operations_with_tools": n_ops,
            "tool_entries": n,
            "class_attribute": top(classes, 6),
            "armor_penetration_fields": {k: [v, pct(v, n)] for k, v in sorted(ap.items(), key=lambda kv: -kv[1])},
            "capacities": top(caps, 20),
            "child_fields": top(childfreq, 14),
        }
    return out


def build_stats_and_extensions(ctx: Context) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for c in CORPORA:
        stat_ops: collections.Counter = collections.Counter()
        equip_ops: collections.Counter = collections.Counter()
        ext: collections.Counter = collections.Counter()
        ce_cls: collections.Counter = collections.Counter()
        whole_def = 0
        for r in ctx.records:
            if r["corpus"] != c or r.get("bad_child"):
                continue
            xp = r.get("xp")
            if r["cls"] == "PatchOperationAddModExtension":
                for cn in r.get("value_ce_classes", []):
                    ext[cn.replace("CombatExtended.", "")] += 1
            for cn in r.get("value_ce_classes", []):
                ce_cls[cn.replace("CombatExtended.", "")] += 1
            if xp and xp["field"] in ("statBases", "equippedStatOffsets"):
                target = stat_ops if xp["field"] == "statBases" else equip_ops
                if xp["tail"] and r["cls"] in ("PatchOperationReplace", "PatchOperationRemove"):
                    target[(short_cls(r["cls"]), xp["tail"][0])] += 1
                elif not xp["tail"] and r["cls"] == "PatchOperationAdd":
                    for tag in r.get("value_tags", []):
                        target[("Add", tag)] += 1
            if r["cls"] == "PatchOperationReplace" and xp and not xp["field"] and "ThingDef" in r.get("value_tags", []):
                whole_def += 1
        out[c] = {
            "statBases_stats_touched": top(stat_ops, 30),
            "equippedStatOffsets_stats_touched": top(equip_ops, 16),
            "mod_extensions_added": top(ext, 16),
            "ce_classes_in_values": top(ce_cls, 25),
            "whole_def_replace_operations": whole_def,
        }
    return out


def build_ce_loadfolders(ctx: Context) -> Dict[str, Any]:
    lf = parse_loadfolders(ctx.ce_root) or {}
    entries = select_load_entries(lf) or []
    kinds = collections.Counter()
    multi = 0
    present, missing = 0, 0
    for e in entries:
        kinds["IfModActive" if e["any"] else "unconditional"] += 1
        if len(e["any"]) > 1:
            multi += 1
        if e["folder"] == "":
            present += 1
        elif os.path.isdir(os.path.join(ctx.ce_root, e["folder"])):
            present += 1
        else:
            missing += 1
    on_disk = set()
    base = os.path.join(ctx.ce_root, "ModPatches")
    for dp, dn, fn in os.walk(base):
        rel = os.path.relpath(dp, base).replace(os.sep, "/")
        parts = rel.split("/")
        if parts and parts[-1] in ("Patches", "Defs"):
            on_disk.add("ModPatches/" + "/".join(parts[:-1]))
    referenced = {e["folder"] for e in entries}
    orphan = sorted(f for f in on_disk if f not in referenced and f != "ModPatches/")
    conflicts = collections.Counter()
    for pf in ctx.files["ce-modpatches"]:
        conflicts[pf.rel.split("/Patches/", 1)[1] if "/Patches/" in pf.rel else pf.rel] += 1
    shared_rel = sum(1 for v in conflicts.values() if v > 1)
    return {
        "versions_defined": sorted(lf.keys()),
        "entries_in_selected_version": len(entries),
        "entry_kinds": dict(kinds),
        "entries_with_multiple_package_ids": multi,
        "entries_whose_folder_exists": present,
        "entries_whose_folder_is_missing": missing,
        "modpatch_folders_on_disk": len(on_disk),
        "modpatch_folders_not_referenced": {"count": len(orphan), "examples": orphan[:10]},
        "patch_files_sharing_a_relative_path_after_Patches": shared_rel,
        "patch_files_total": len(ctx.files["ce-modpatches"]),
    }


# ----------------------------------------------------------------------------
# Entry points
# ----------------------------------------------------------------------------
def read_version_info(ce_root: str, game_root: str) -> Dict[str, str]:
    info = {"ce_modVersion": "", "ce_supportedVersions": "", "game_version": ""}
    about = parse_about(ce_root)
    ap = find_ci(os.path.join(ce_root, "About"), "About.xml")
    if ap:
        t, _, _ = parse_xml(ap)
        if t is not None:
            info["ce_modVersion"] = (t.getroot().findtext("modVersion") or "").strip()
    info["ce_supportedVersions"] = ",".join(about["supportedVersions"])
    vp = os.path.join(game_root, "Version.txt")
    if os.path.exists(vp):
        info["game_version"] = read_text(vp).strip()
    return info


def run_survey(args: argparse.Namespace) -> int:
    ctx = Context(args.ce_root, args.owner_root, args.workshop_root, args.game_root, args.skip_workshop)
    ctx.load_all()
    lint = run_lints(ctx, args.examples)
    summary: Dict[str, Any] = {
        "meta": {
            "tool": "survey_ce_patches.py",
            "corpora": {
                "ce-core": "Patches/** of the CE repository checkout",
                "ce-modpatches": "ModPatches/*/Patches/** of the CE repository checkout",
                "owner": "the owner's own mods (patch files that mention Combat Extended)",
                "workshop": "Steam workshop items other than Combat Extended itself, patch files that mention Combat Extended",
            },
            "versions": read_version_info(args.ce_root, args.game_root),
            "known_ce_weapon_tags_in_core": len(ctx.ce_tags),
            "ce_schema_classes": len(ctx.schema["classes"]),
        },
        "corpus_overview": build_corpus_overview(ctx),
        "operation_classes": build_operation_classes(ctx),
        "xpath": build_xpath_stats(ctx),
        "sequences": build_sequences(ctx),
        "idioms": build_idioms(ctx),
        "gating": build_gating(ctx),
        "integration_styles": mod_style_summary(ctx),
        "ce_loadfolders": build_ce_loadfolders(ctx),
        "weapons_make_gun": build_weapons(ctx),
        "tools": build_tools(ctx),
        "stats_and_extensions": build_stats_and_extensions(ctx),
        "defs": survey_defs(ctx),
        "lint": lint.summary(),
    }
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(summary, fh, indent=1, sort_keys=False, ensure_ascii=False)
        fh.write("\n")
    # compact console report
    print("wrote", args.out)
    for c in CORPORA:
        o = summary["corpus_overview"][c]
        print("%-14s files=%-5d mods=%-4d top-level ops=%-6d nested=%-6d parse-errors=%d" % (
            c, o["patch_files"], o["mods_or_folders"], o["operations_top_level"], o["operations_nested"], o["files_with_parse_error"]))
    for rule in sorted(RULES):
        hits = summary["lint"][rule]["hits"]
        print("%s %-60s %s" % (rule, RULES[rule][:60], hits))
    return 0


def run_lint_cmd(args: argparse.Namespace) -> int:
    paths: List[str] = []
    for p in args.paths:
        if os.path.isdir(p):
            paths += walk_files(p, lambda q: q.lower().endswith(".xml"))
        else:
            paths.append(p)
    ctx = Context(args.ce_root, args.owner_root, args.workshop_root, args.game_root, True)
    ctx.schema = load_ce_schema(args.ce_root)
    ctx.vanilla_fields = observed_fields(vanilla_def_files(args.game_root))
    ctx.ce_tags = known_ce_tags(args.ce_root, [])
    idx: Dict[str, Set[str]] = {}
    index_defs(defs_files(os.path.join(args.ce_root, "Defs")), idx)
    index_defs(vanilla_def_files(args.game_root), idx)
    ctx.def_index = idx
    ctx.def_index_modpatch_defs = {}
    ctx.mods["adhoc"] = {}
    files = []
    for p in paths:
        pf = PatchFile("adhoc", p, os.path.basename(p), os.path.basename(os.path.dirname(os.path.abspath(p))))
        pf.load()
        pf.gate = {"kind": "loaded", "ids": []}
        files.append(pf)
        ctx.records += scan_file_ops(pf)
    ctx.files["adhoc"] = files
    lint = run_lints(ctx, 10 ** 6)
    n = 0
    for rule in sorted(RULES):
        for corpus, exs in lint.examples[rule].items():
            for e in exs:
                print("%s:%s: %s %s" % (e["file"], e["line"] or "-", rule, e["detail"]))
                n += 1
    print("%d finding(s) in %d file(s)" % (n, len(files)))
    return 1 if n else 0


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ce-root", default=DEFAULT_CE_ROOT)
    ap.add_argument("--owner-root", default=DEFAULT_OWNER)
    ap.add_argument("--workshop-root", default=DEFAULT_WORKSHOP)
    ap.add_argument("--game-root", default=DEFAULT_GAME)
    sub = ap.add_subparsers(dest="cmd")
    sp = sub.add_parser("survey", help="run the corpus survey (default)")
    sp.add_argument("--out", default=DEFAULT_OUT)
    sp.add_argument("--examples", type=int, default=5)
    sp.add_argument("--skip-workshop", action="store_true")
    lp = sub.add_parser("lint", help="lint patch files or directories")
    lp.add_argument("paths", nargs="+")
    args = ap.parse_args(argv)
    if args.cmd == "lint":
        return run_lint_cmd(args)
    if args.cmd is None:
        args.out, args.examples, args.skip_workshop = DEFAULT_OUT, 5, False
    return run_survey(args)


if __name__ == "__main__":
    sys.exit(main())
