#!/usr/bin/env python3
"""Experiment: validate vanilla Defs (Core + DLC) against a schema derived by reflection from Assembly-CSharp.

The schema (schema.json) comes from Program.cs (MetadataLoadContext). No game code is executed.
Rules mirror decompiled:Verse/XmlToObjectUtils (DoFieldSearch), DirectXmlToObject (list/dict/custom loaders),
decompiled:Verse/ParseHelper (scalar parsers) and GenTypes.IgnoredNamespaceNames.
"""
import json, sys, os, glob, collections
from lxml import etree

HERE = os.path.dirname(os.path.abspath(__file__))
schema = json.load(open(os.environ.get("RIMSTUDIO_SCHEMA", os.path.join(HERE, "schema.json"))))
IGN_NS = ["RimWorld", "Verse", "LudeonTK", "Verse.AI", "Verse.AI.Group", "Verse.Sound", "Verse.Grammar",
          "RimWorld.Planet", "RimWorld.BaseGen", "RimWorld.QuestGen", "RimWorld.SketchGen", "System"]
SCALARS = {
    "System.String", "System.Boolean", "System.SByte", "System.Byte", "System.UInt32", "System.Int32", "System.UInt16",
    "System.Int16", "System.UInt64", "System.Int64", "System.Single", "System.Double", "UnityEngine.Vector3",
    "UnityEngine.Vector2", "UnityEngine.Vector4", "UnityEngine.Quaternion", "UnityEngine.Rect", "System.Type",
    "System.Action", "UnityEngine.Color", "Steamworks.PublishedFileId_t", "Verse.IntVec2", "Verse.IntVec3",
    "Verse.Rot4", "Verse.CellRect", "Verse.CurvePoint", "Verse.NameTriple", "Verse.FloatRange", "Verse.IntRange",
    "RimWorld.QualityRange", "Verse.ColorInt", "Verse.TaggedString", "RimWorld.Planet.PlanetTile",
    "System.Char", "System.Decimal", "System.DateTime", "System.TimeSpan",
}


def split_args(s):
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    if cur:
        out.append(cur)
    return out


def parse(ts):
    """Return nested tuple structure for the type string."""
    for pre, tag in (("L<", "list"), ("H<", "list"), ("A<", "list"), ("N<", "nullable")):
        if ts.startswith(pre) and ts.endswith(">"):
            return (tag, parse(ts[2:-1]))
    if ts.startswith("D<") and ts.endswith(">"):
        a = split_args(ts[2:-1])
        return ("dict", parse(a[0]), parse(a[1]))
    if ts.startswith("G:"):
        return ("generic", ts)
    return ("name", ts)


def classify(ts):
    p = parse(ts)
    return p


chain_cache = {}


def chain(tn):
    if tn in chain_cache:
        return chain_cache[tn]
    out = []
    t = schema.get(tn)
    seen = set()
    while t is not None and tn not in seen:
        seen.add(tn)
        out.append((tn, t))
        tn = t.get("b")
        t = schema.get(tn) if tn else None
    chain_cache[out[0][0] if out else tn] = out
    return out


field_cache = {}


def find_field(tn, name):
    key = (tn, name)
    if key in field_cache:
        return field_cache[key]
    res = None
    ch = chain(tn)
    for _, t in ch:
        for f in t["f"]:
            if f["n"] == name:
                res = (f, "exact")
                break
        if res:
            break
    if not res:
        low = name.lower()
        for _, t in ch:
            for f in t["f"]:
                if any((a or "").lower() == low for a in f.get("a", [])):
                    res = (f, "alias")
                    break
            if res:
                break
    if not res:
        low = name.lower()
        for _, t in ch:
            for f in t["f"]:
                if f["n"].lower() == low:
                    res = (f, "case")
                    break
            if res:
                break
    field_cache[key] = res
    return res


def ignored_elements(tn):
    out = set()
    for _, t in chain(tn):
        for e in t.get("ign") or []:
            out.add((e or "").lower())
    return out


def resolve_type(name, base=None):
    if name in schema:
        return name
    for ns in IGN_NS:
        k = ns + "." + name
        if k in schema:
            return k
    return None


stats = collections.Counter()
unresolved = collections.Counter()
examples = {}
casefix = collections.Counter()
custom_skipped = collections.Counter()
unknown_class = collections.Counter()
nonli = collections.Counter()
scalar_with_children = collections.Counter()
unsaved_used = collections.Counter()


def is_def_type(n):
    t = schema.get(n)
    return n == "Verse.Def" or (t is not None and t.get("def") == 1)


def handle_value(node, ts, ctx):
    p = classify(ts)
    kind = p[0]
    if kind == "nullable":
        return handle_value(node, p[1][1] if p[1][0] == "name" else ts, ctx) if p[1][0] == "name" else None
    if kind == "list":
        stats["lists"] += 1
        el = p[1]
        for c in node:
            if not isinstance(c.tag, str):
                continue
            if c.tag != "li":
                nonli[ctx] += 1
                continue
            stats["li"] += 1
            handle_parsed(c, el, ctx)
        return
    if kind == "dict":
        stats["dict_skipped"] += 1
        return
    if kind == "generic":
        stats["generic_skipped"] += 1
        return
    handle_parsed(node, p, ctx)


def handle_parsed(node, p, ctx):
    kind = p[0]
    if kind == "nullable":
        return handle_parsed(node, p[1], ctx)
    if kind == "list":
        # nested list in li
        for c in node:
            if isinstance(c.tag, str):
                handle_parsed(c, p[1], ctx)
        return
    if kind in ("dict", "generic"):
        stats["dict_or_generic_skipped"] += 1
        return
    name = p[1]
    t = schema.get(name)
    if name in SCALARS:
        has_children = any(isinstance(c.tag, str) for c in node)
        if has_children:
            scalar_with_children[(ctx, name)] += 1
        stats["scalars"] += 1
        return
    if t is None:
        stats["outside_schema"] += 1
        return
    if t["kind"] == "enum":
        stats["enums"] += 1
        return
    if is_def_type(name):
        stats["def_refs"] += 1
        return
    # class / struct
    cls = node.get("Class")
    tn = name
    if cls:
        r = resolve_type(cls)
        if r is None:
            unknown_class[cls] += 1
            stats["class_attr_unresolved"] += 1
            return
        tn = r
    walk(node, tn, ctx)


def walk(node, tn, ctx):
    t = schema.get(tn)
    if t is None:
        stats["walk_outside_schema"] += 1
        return
    if t["kind"] == "enum":
        return
    # custom loader anywhere in hierarchy => skip
    for n2, t2 in chain(tn):
        if t2.get("custom"):
            custom_skipped[n2] += 1
            stats["custom_loader_skipped"] += 1
            return
    ign = ignored_elements(tn)
    # texts only?
    for c in node:
        if not isinstance(c.tag, str):
            continue
        stats["elements"] += 1
        ff = find_field(tn, c.tag)
        if ff is None:
            if c.get("IgnoreIfNoMatchingField", "").lower() == "true" or c.tag.lower() in ign:
                stats["ignored_missing"] += 1
                continue
            key = (tn, c.tag)
            unresolved[key] += 1
            if key not in examples:
                examples[key] = ctx
            continue
        f, how = ff
        if how == "case":
            casefix[(tn, c.tag, f["n"])] += 1
        if f.get("u"):
            unsaved_used[(tn, c.tag)] += 1
        handle_value(c, f["t"], ctx + "/" + c.tag)


def validate_file(path):
    try:
        root = etree.parse(path, etree.XMLParser(recover=False)).getroot()
    except Exception as e:
        stats["xml_errors"] += 1
        return
    for d in root:
        if not isinstance(d.tag, str):
            continue
        stats["defs"] += 1
        tn = resolve_type(d.tag)
        if tn is None:
            stats["unknown_def_type"] += 1
            unresolved[("<top-level>", d.tag)] += 1
            continue
        cls = d.get("Class")
        if cls:
            r = resolve_type(cls)
            if r:
                tn = r
        walk(d, tn, d.tag)


def main():
    base = sys.argv[1]
    pkgs = [p for p in sorted(os.listdir(base)) if os.path.isdir(os.path.join(base, p, "Defs"))]
    print("packages:", pkgs)
    for p in pkgs:
        for f in glob.glob(os.path.join(base, p, "Defs", "**", "*.xml"), recursive=True):
            stats["files"] += 1
            validate_file(f)
    print(dict(stats))
    tot_unres = sum(unresolved.values())
    print("unresolved element occurrences:", tot_unres, "distinct (type,field):", len(unresolved))
    print("ratio of unresolved to elements checked: %.5f%%" % (100.0 * tot_unres / max(1, stats["elements"])))
    for (tn, tag), c in unresolved.most_common(25):
        print("  UNRESOLVED", c, tn, "<%s>" % tag, "e.g.", examples.get((tn, tag)))
    print("case-insensitive-only matches:", sum(casefix.values()), casefix.most_common(5))
    print("custom loader types skipped:", sum(custom_skipped.values()), "distinct", len(custom_skipped), custom_skipped.most_common(8))
    print("unresolved Class= values:", unknown_class.most_common(8))
    print("list children that are not <li>:", sum(nonli.values()), nonli.most_common(5))
    print("scalar fields having element children:", sum(scalar_with_children.values()), scalar_with_children.most_common(8))
    print("[Unsaved] fields set in vanilla XML:", sum(unsaved_used.values()), unsaved_used.most_common(5))


if __name__ == "__main__":
    main()
