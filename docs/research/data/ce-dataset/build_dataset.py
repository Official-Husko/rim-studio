#!/usr/bin/env python3
"""Build the resolved Combat Extended equipment datasets (ce-ranged / ce-melee / ce-apparel / ce-ammo .json).

Usage: build_dataset.py [--game DIR] [--ce DIR] [--out DIR]   (env fallbacks RIMWORLD_DIR, CE_DIR)
Runs the def-engine twice (vanilla Core+DLC, then vanilla + CE) and writes one JSON file per class with, for every
item, the CE-resolved values and (when the def exists in vanilla) the vanilla values side by side.
The files are DERIVED NUMBERS for analysis; the app must read the user's own CE install at runtime (licence note in
docs/research/combat-extended-model.md). Deterministic output (sorted keys, one record per line).
"""
import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ce_load                                            # noqa: E402
from defengine.xmlnet import child_elements, inner_text   # noqa: E402

CE_ID = "CETeam.CombatExtended"


def num(s):
    try:
        f = float(s)
        return int(f) if f == int(f) and "." not in s and "e" not in s.lower() else f
    except (ValueError, OverflowError):
        return s


def tojson(el):
    """Generic XML-to-JSON for a def subtree (leaf text -> number or string, li lists -> arrays)."""
    kids = child_elements(el)
    if not kids:
        t = inner_text(el).strip()
        return num(t) if t != "" else ""
    if all(k.tag == "li" for k in kids):
        out = [tojson(k) for k in kids]
        return out
    out = {}
    if el.get("Class"):
        out["_class"] = el.get("Class")
    for k in kids:
        v = tojson(k)
        if k.tag == "li":
            out.setdefault("_li", []).append(v)
        elif k.tag in out:
            if not isinstance(out[k.tag], list) or not out.get("_multi_" + k.tag):
                out[k.tag] = [out[k.tag]]
                out["_multi_" + k.tag] = True
            out[k.tag].append(v)
        else:
            out[k.tag] = v
    if el.get("Class") and len(out) == 1:
        out["_class"] = el.get("Class")
    return out


def dd(x):
    return x if isinstance(x, dict) else {}


def kid(e, n):
    for c in child_elements(e):
        if c.tag == n:
            return c


def sub(d, n, default=None):
    c = kid(d.node, n)
    return tojson(c) if c is not None else default


def comp(d, cls):
    c = kid(d.node, "comps")
    for li in (child_elements(c) if c is not None else []):
        if li.get("Class") == cls:
            return tojson(li)
    return None


def listtext(d, n):
    c = kid(d.node, n)
    return [inner_text(x).strip() for x in child_elements(c)] if c is not None else []


def stats(d):
    s = sub(d, "statBases", {}) or {}
    return {k: v for k, v in s.items() if not k.startswith("_")}


def txt(d, n):
    c = kid(d.node, n)
    return inner_text(c).strip() if c is not None else None


def is_abstract(d):
    return d.node.get("Abstract") == "True"


def verbs_of(d):
    v = kid(d.node, "verbs")
    return [tojson(li) for li in child_elements(v)] if v is not None else []


def ranged_verb(d):
    for v in verbs_of(d):
        if isinstance(v, dict) and "defaultProjectile" in v:
            return v
    return None


def proj_of(res, name):
    if not name:
        return None
    p = res.get("ThingDef", name)
    if p is None:
        return None
    pr = sub(p, "projectile")
    out = {"defName": name, "thingClass": txt(p, "thingClass"), "label": txt(p, "label")}
    if isinstance(pr, dict):
        out["props"] = pr
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default=os.environ.get("RIMWORLD_DIR"))
    ap.add_argument("--ce", default=os.environ.get("CE_DIR"))
    ap.add_argument("--out", default=str(HERE))
    a = ap.parse_args()
    game, ce, out = Path(a.game), Path(a.ce), Path(a.out)
    with tempfile.TemporaryDirectory() as t:
        van = ce_load.load(game, ce, Path(t), with_ce=False)
        cer = ce_load.load(game, ce, Path(t), with_ce=True)

    # ---- ammo sets and ammo defs
    sets = {}
    for s in cer.database("CombatExtended.AmmoSetDef").defs:
        node = tojson(s.node)
        sets[s.def_name] = {"defName": s.def_name, "label": txt(s, "label"), "similarTo": txt(s, "similarTo"),
                            "ammoTypes": node.get("ammoTypes", {}), "source": s.mod_id}
    ammo_in_sets = {}
    for sn, s in sets.items():
        for am, pr in s["ammoTypes"].items():
            ammo_in_sets.setdefault(am, []).append({"set": sn, "projectile": pr})
    ammo = []
    for d in cer.database("CombatExtended.AmmoDef").defs:
        if is_abstract(d):
            continue
        st = stats(d)
        mem = ammo_in_sets.get(d.def_name, [])
        projs = {m["projectile"] for m in mem}
        if not projs and txt(d, "cookOffProjectile"):
            projs = {txt(d, "cookOffProjectile")}
        ammo.append({"defName": d.def_name, "label": txt(d, "label"), "source": d.mod_id, "ammoClass": txt(d, "ammoClass"),
                     "categories": listtext(d, "thingCategories"), "stats": {k: st[k] for k in ("Mass", "Bulk", "MarketValue", "MaxHitPoints") if k in st},
                     "stackLimit": txt(d, "stackLimit"), "cookOffProjectile": txt(d, "cookOffProjectile"),
                     "sets": [m["set"] for m in mem],
                     "projectiles": [proj_of(cer, p) for p in sorted(projs)]})
    ammo.sort(key=lambda r: r["defName"])

    # ---- ranged weapons
    def snapshot_ranged(res, d, with_ammo):
        verb = ranged_verb(d)
        r = {"stats": stats(d), "verb": verb}
        if verb:
            r["defaultProjectile"] = proj_of(res, verb.get("defaultProjectile"))
        return r

    ranged, melee = [], []
    for d in cer.database("ThingDef").defs:
        if is_abstract(d) or txt(d, "category") != "Item" or kid(d.node, "equipmentType") is None:
            continue
        v = van.get("ThingDef", d.def_name)
        verb = ranged_verb(d)
        base = {"defName": d.def_name, "label": txt(d, "label"), "source": d.mod_id, "techLevel": txt(d, "techLevel"),
                "weaponTags": listtext(d, "weaponTags"), "weaponClasses": listtext(d, "weaponClasses"),
                "costList": sub(d, "costList", {}), "stuffCategories": listtext(d, "stuffCategories"),
                "researchPrerequisite": (sub(d, "recipeMaker", {}) or {}).get("researchPrerequisite") if isinstance(sub(d, "recipeMaker", {}), dict) else None}
        if verb is not None:
            au = comp(d, "CombatExtended.CompProperties_AmmoUser")
            fm = comp(d, "CombatExtended.CompProperties_FireModes")
            rec = dict(base, kind="ranged", stats=stats(d), verb=verb, ammoUser=au, fireModes=fm,
                       defaultProjectile=proj_of(cer, verb.get("defaultProjectile")),
                       ceNative=(d.mod_id == CE_ID), isCEConverted=au is not None)
            ammoset = (au or {}).get("ammoSet")
            if ammoset in sets:
                rec["ammoSet"] = {"defName": ammoset, "label": sets[ammoset]["label"], "ammoTypes": sets[ammoset]["ammoTypes"]}
            if v is not None and not v.node.get("Class"):
                rec["vanilla"] = snapshot_ranged(van, v, False)
            ranged.append(rec)
        if kid(d.node, "tools") is not None:
            tools = sub(d, "tools", [])
            rec = dict(base, kind="melee", stats=stats(d), equippedStatOffsets=sub(d, "equippedStatOffsets", {}), tools=tools, ceNative=(d.mod_id == CE_ID),
                       hasRangedVerb=verb is not None, ceTools=any(isinstance(x, dict) and x.get("_class") == "CombatExtended.ToolCE" for x in tools))
            if v is not None:
                rec["vanilla"] = {"stats": stats(v), "tools": sub(v, "tools", [])}
            melee.append(rec)
    ranged.sort(key=lambda r: r["defName"])
    melee.sort(key=lambda r: r["defName"])

    # ---- apparel and stuff
    apparel = []
    for d in cer.database("ThingDef").defs:
        if is_abstract(d) or kid(d.node, "apparel") is None or txt(d, "category") != "Item":
            continue
        v = van.get("ThingDef", d.def_name)
        ap_ = dd(sub(d, "apparel", {}))
        mods = sub(d, "modExtensions", []) or []
        pa = [m for m in mods if isinstance(m, dict) and m.get("_class") == "CombatExtended.PartialArmorExt"]
        rec = {"defName": d.def_name, "label": txt(d, "label"), "source": d.mod_id, "techLevel": txt(d, "techLevel"),
               "stats": stats(d), "layers": ap_.get("layers", []), "bodyPartGroups": ap_.get("bodyPartGroups", []),
               "tags": ap_.get("tags", []), "stuffCategories": listtext(d, "stuffCategories"), "costList": sub(d, "costList", {}),
               "costStuffCount": txt(d, "costStuffCount"), "equippedStatOffsets": sub(d, "equippedStatOffsets", {}),
               "partialArmor": pa[0].get("stats", []) if pa else [],
               "extensions": sorted({m.get("_class") for m in mods if isinstance(m, dict) and m.get("_class")})}
        if v is not None:
            rec["vanilla"] = {"stats": stats(v), "layers": dd(sub(v, "apparel", {})).get("layers", []),
                              "bodyPartGroups": dd(sub(v, "apparel", {})).get("bodyPartGroups", [])}
        apparel.append(rec)
    apparel.sort(key=lambda r: r["defName"])
    stuffs = []
    for d in cer.database("ThingDef").defs:
        if is_abstract(d) or kid(d.node, "stuffProps") is None:
            continue
        v = van.get("ThingDef", d.def_name)
        sp = dd(sub(d, "stuffProps", {}))
        rec = {"defName": d.def_name, "label": txt(d, "label"), "source": d.mod_id, "stats": stats(d),
               "categories": sp.get("categories", []), "statFactors": sp.get("statFactors", {}), "statOffsets": sp.get("statOffsets", {})}
        if v is not None:
            vp = dd(sub(v, "stuffProps", {}))
            rec["vanilla"] = {"stats": stats(v), "statFactors": vp.get("statFactors", {}), "statOffsets": vp.get("statOffsets", {})}
        stuffs.append(rec)
    stuffs.sort(key=lambda r: r["defName"])

    def dump(name, key, rows, extra):
        with open(out / name, "w", encoding="utf-8") as f:
            f.write("{\n")
            for k, v in extra.items():
                f.write(json.dumps(k) + ": " + json.dumps(v, sort_keys=True, separators=(",", ":")) + ",\n")
            f.write(json.dumps(key) + ": [\n")
            f.write(",\n".join(json.dumps(r, sort_keys=True, separators=(",", ":")) for r in rows))
            f.write("\n]\n}\n")

    presets = {"gunPresets": [dict(tojson(d.node), defName=d.def_name) for d in cer.database("CombatExtended.GunPatcherPresetDef").defs],
               "apparelPresets": [dict(tojson(d.node), defName=d.def_name) for d in cer.database("CombatExtended.ApparelPatcherPresetDef").defs]}
    meta = {"generated": "2026-10-04", "note": "derived numbers for analysis; read the user's own CE install at runtime in the product",
            "game": (game / "Version.txt").read_text().strip() if (game / "Version.txt").exists() else None}
    dump("ce-ranged.json", "weapons", ranged, {"meta": meta})
    dump("ce-melee.json", "weapons", melee, {"meta": meta})
    dump("ce-apparel.json", "apparel", apparel, {"meta": meta, "stuffs": stuffs})
    dump("ce-ammo.json", "ammo", ammo, {"meta": meta, "ammoSets": [sets[k] for k in sorted(sets)]})
    (out / "ce-presets.json").write_text(json.dumps(dict(meta=meta, **presets), sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
    print({"ranged": len(ranged), "melee": len(melee), "apparel": len(apparel), "stuffs": len(stuffs), "ammo": len(ammo), "ammoSets": len(sets)})


if __name__ == "__main__":
    main()
