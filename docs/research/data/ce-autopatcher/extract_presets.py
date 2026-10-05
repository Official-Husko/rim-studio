#!/usr/bin/env python3
"""Parse CE's auto-patcher preset defs (gun and apparel) from a CE folder into plain JSON tables.

Usage: extract_presets.py CE_DIR OUT_DIR
Reads  CE_DIR/Defs/GunPatcherDefs/*.xml, CE_DIR/Defs/ApparelAutoPatcherPresets/*.xml
Writes OUT_DIR/presets_gun.json, OUT_DIR/presets_apparel.json (research artefacts, see README licence note).
"""
import json, os, sys
from pathlib import Path
from lxml import etree

def txt(e):
    return "".join(e.itertext()).strip()

def rng(s):
    a, b = s.split("~")
    return [float(a), float(b)]

def curve(e):
    pts = []
    for li in e.find("points"):
        if isinstance(li.tag, str):
            x, y = txt(li).split(",")
            pts.append([float(x), float(y)])
    return pts

def num(e):
    s = txt(e)
    try:
        return float(s)
    except ValueError:
        return s

def lst(e):
    return [txt(li) for li in e if isinstance(li.tag, str)]

def gun_preset(d):
    o = {"defName": txt(d.find("defName"))}
    for k in ("Mass", "Bulk", "Spread", "Sway", "ReloadTime", "AmmoCapacity", "CooldownTime"):
        e = d.find(k)
        if e is not None:
            o[k] = num(e)
    for k in ("setCaliber", "bipodTag"):
        e = d.find(k)
        if e is not None:
            o[k] = txt(e)
    for k in ("reloadOneAtATime", "addBipods", "DiscardDesignations", "DetermineCaliber"):
        e = d.find(k)
        o[k] = (txt(e).lower() == "true") if e is not None else False
    for k in ("rangeCurve", "warmupCurve", "cooldownCurve", "MassCurve"):
        e = d.find(k)
        if e is not None:
            o[k] = curve(e)
    for k in ("RangeRange", "WarmupRange", "damageRange", "projSpeedRange"):
        e = d.find(k)
        o[k] = rng(txt(e)) if e is not None else None
    for k in ("names", "tags", "addTags"):
        e = d.find(k)
        o[k] = lst(e) if e is not None else []
    gs = d.find("gunStats")
    o["gunStats"] = {c.tag: txt(c) for c in gs if isinstance(c.tag, str)} if gs is not None else {}
    fm = d.find("fireModes")
    o["fireModes"] = {c.tag: txt(c) for c in fm if isinstance(c.tag, str)} if fm is not None else {}
    mo = d.find("MiscOtherStats")
    o["MiscOtherStats"] = {c.tag: num(c) for c in mo if isinstance(c.tag, str)} if mo is not None else {}
    cr = []
    c = d.find("CaliberRanges")
    if c is not None:
        for li in c:
            if isinstance(li.tag, str):
                cr.append({"DamageRange": rng(txt(li.find("DamageRange"))), "SpeedRange": rng(txt(li.find("SpeedRange"))),
                           "AmmoSet": txt(li.find("AmmoSet"))})
    o["CaliberRanges"] = cr
    sg = []
    c = d.find("specialGuns")
    if c is not None:
        for li in c:
            if isinstance(li.tag, str):
                x = {"names": lst(li.find("names")) if li.find("names") is not None else []}
                for k in ("caliber", "reloadTime", "magCap", "mass", "bulk"):
                    e = li.find(k)
                    if e is not None:
                        x[k] = num(e)
                st = li.find("stats")
                x["stats"] = {c2.tag: num(c2) for c2 in st if isinstance(c2.tag, str)} if st is not None else {}
                sg.append(x)
    o["specialGuns"] = sg
    return o

def apparel_preset(d):
    o = {"defName": txt(d.find("defName"))}
    for k in ("Bulk", "BulkWorn", "Mass", "ArmorStaticSharp", "ArmorStaticBlunt"):
        e = d.find(k)
        if e is not None:
            o[k] = num(e)
    for k in ("ArmorCurveSharp", "ArmorCurveBlunt"):
        e = d.find(k)
        if e is not None:
            o[k] = curve(e)
    o["vanillaArmorRatingRange"] = rng(txt(d.find("vanillaArmorRatingRange")))
    o["neededLayers"] = lst(d.find("neededLayers"))
    o["neededGroups"] = lst(d.find("neededGroups"))
    ps = []
    e = d.find("partialStats")
    if e is not None:
        for li in e:
            if isinstance(li.tag, str):
                ps.append({c.tag: (lst(c) if c.tag == "parts" else num(c)) for c in li if isinstance(c.tag, str)})
    o["partialStats"] = ps
    return o

def load(folder, tag, fn):
    out = []
    for f in sorted(Path(folder).glob("*.xml")):
        root = etree.parse(str(f)).getroot()
        for d in root:
            if isinstance(d.tag, str) and d.tag == tag:
                out.append(fn(d))
                out[-1]["_file"] = f.name
    return out

def main():
    ce, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    g = load(ce / "Defs" / "GunPatcherDefs", "CombatExtended.GunPatcherPresetDef", gun_preset)
    a = load(ce / "Defs" / "ApparelAutoPatcherPresets", "CombatExtended.ApparelPatcherPresetDef", apparel_preset)
    (out / "presets_gun.json").write_text(json.dumps(g, indent=1, sort_keys=True) + "\n")
    (out / "presets_apparel.json").write_text(json.dumps(a, indent=1, sort_keys=True) + "\n")
    print(len(g), "gun presets,", len(a), "apparel presets")

if __name__ == "__main__":
    main()
