"""Builds mods from a JSON test vector in a temp dir, runs the engine and checks the expectations.

Vector format (JSON, app-internal; the XML inside strings is RimWorld's own format):
{
  "name": str, "doc": str, "game_version": "1.6.4871",
  "mods": [ {"id": package id, "name": display name, "files": {"relative/path": "text"}} ... ]   # load order
  "extra_types": {"Ns.Type": "Verse.ThingDef"},          # optional, added to the vanilla Def type table
  "extra_active_ids": [..],                               # optional
  "expect": {
    "defs": {"ThingDef/Name": "<ThingDef>...</ThingDef>"},   # resolved node, compared structurally
    "absent": ["ThingDef/Name"],                              # not in the database
    "provenance": {"ThingDef/Name": {"mod": id, "file": "Defs/x.xml", "parents": [...]}},
    "diag": {"code": count},                                  # exact counts for the listed codes
    "patch_results": [true, false],                           # per top-level op in load order
    "db_order": {"ThingDef": ["A", "B"]},                     # database order
    "stats": {"defs": 3}
  }
}
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT))

from defengine.engine import LoadConfig, TypeTable, load_game      # noqa: E402
from defengine.xmlnet import parse_xml_bytes, to_canonical          # noqa: E402

TYPES = ROOT / "data" / "def_types_vanilla.json"


def canon(el):
    d = to_canonical(el)
    return _norm(d)


def _norm(d):
    if isinstance(d, str):
        return d
    return {"tag": d["tag"], "attrs": sorted(d["attrs"]), "children": [_norm(c) for c in d["children"]]}


def build_mods(vec, base: Path):
    paths = []
    for i, m in enumerate(vec["mods"]):
        root = base / ("%02d_%s" % (i, m["id"].replace("/", "_")))
        files = dict(m.get("files", {}))
        if not any(k.lower() == "about/about.xml" for k in files):
            files["About/About.xml"] = "<ModMetaData><name>%s</name><packageId>%s</packageId></ModMetaData>" % (
                m.get("name", m["id"]), m["id"])
        for rel, text in files.items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        paths.append(root)
    return paths


def make_types(vec):
    data = json.loads(TYPES.read_text(encoding="utf-8"))
    for name, base in vec.get("extra_types", {}).items():
        data["types"][name] = {"base": base, "abstract": False, "assembly": "TestMod"}
    return TypeTable(data)


def run_vector(vec, custom_ops=None):
    with tempfile.TemporaryDirectory() as td:
        paths = build_mods(vec, Path(td))
        cfg = LoadConfig(mods=paths, game_version=vec.get("game_version", "1.6.4871"),
                         extra_active_ids=vec.get("extra_active_ids", []), custom_ops=custom_ops or {})
        cfg.types = make_types(vec)
        res = load_game(cfg)
        return res


def check_vector(vec):
    """Returns a list of failure strings (empty when the vector passes)."""
    res = run_vector(vec)
    exp = vec["expect"]
    fails = []
    for key, xml in exp.get("defs", {}).items():
        t, n = key.split("/", 1)
        d = res.get(t, n)
        if d is None:
            fails.append("missing def %s" % key)
            continue
        want = canon(parse_xml_bytes(xml.encode("utf-8")))
        got = canon(d.node)
        if want != got:
            fails.append("def %s differs:\n  want %s\n  got  %s" % (key, json.dumps(want), json.dumps(got)))
    for key in exp.get("absent", []):
        t, n = key.split("/", 1)
        if res.get(t, n) is not None:
            fails.append("def %s should be absent" % key)
    for key, want in exp.get("provenance", {}).items():
        t, n = key.split("/", 1)
        d = res.get(t, n)
        if d is None:
            fails.append("provenance: missing def %s" % key)
            continue
        got = d.provenance()
        for k, v in want.items():
            if got.get(k) != v:
                fails.append("provenance %s.%s: want %r got %r" % (key, k, v, got.get(k)))
    if "patch_results" in exp:
        got = [e.result for e in res.patch_events]
        if got != exp["patch_results"]:
            fails.append("patch_results: want %r got %r" % (exp["patch_results"], got))
    for t, names in exp.get("db_order", {}).items():
        got = [d.def_name for d in res.database(t).defs]
        if got != names:
            fails.append("db_order %s: want %r got %r" % (t, names, got))
    for code, n in exp.get("diag", {}).items():
        if res.diag.count(code) != n:
            fails.append("diag %s: want %d got %d" % (code, n, res.diag.count(code)))
    for k, v in exp.get("stats", {}).items():
        if res.stats.get(k) != v:
            fails.append("stats %s: want %r got %r" % (k, v, res.stats.get(k)))
    return fails


def load_vectors():
    out = []
    for p in sorted((HERE / "vectors").glob("*.json")):
        doc = json.loads(p.read_text(encoding="utf-8"))
        items = doc if isinstance(doc, list) else [doc]
        for v in items:
            v["_file"] = p.name
            out.append(v)
    return out


if __name__ == "__main__":
    bad = 0
    vs = load_vectors()
    for v in vs:
        f = check_vector(v)
        print(("FAIL " if f else "ok   ") + v["name"])
        for line in f:
            print("     " + line)
        bad += bool(f)
    print("%d vectors, %d failed" % (len(vs), bad))
    sys.exit(1 if bad else 0)
