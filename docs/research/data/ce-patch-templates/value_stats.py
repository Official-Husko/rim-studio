#!/usr/bin/env python3
"""Numeric distributions of CE patch values (apparel armour, bulk, weapon stats, tool penetration).

Usage: value_stats.py [--ce-root DIR] [--out FILE]
Env: RIMSTUDIO_CE_ROOT. Reads Patches/** and ModPatches/*/Patches/** (read-only), prints a small JSON summary.
"""
import argparse, glob, json, os, statistics as st
from lxml import etree

HERE = os.path.dirname(os.path.abspath(__file__))
ap = argparse.ArgumentParser()
ap.add_argument("--ce-root", default=os.environ.get("RIMSTUDIO_CE_ROOT", os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", "CombatExtended-Development"))))
ap.add_argument("--out", default=os.path.join(HERE, "value_stats.json"))
a = ap.parse_args()
files = sorted(glob.glob(a.ce_root + "/Patches/**/*.xml", recursive=True)) + sorted(glob.glob(a.ce_root + "/ModPatches/*/Patches/**/*.xml", recursive=True))
acc = {}
def put(k, v):
    try: acc.setdefault(k, []).append(float(v))
    except (TypeError, ValueError): pass
parser = etree.XMLParser(recover=True, remove_comments=True)
for f in files:
    try: root = etree.parse(f, parser).getroot()
    except Exception: continue
    if root is None: continue
    for op in root.iter("Operation"):
        cls = op.get("Class", "")
        if cls.endswith("MakeGunCECompatible"):
            for stat in op.findall("statBases/*"): put("gun.stat." + stat.tag, stat.text)
            for t in ("magazineSize", "reloadTime"): put("gun.ammoUser." + t, op.findtext("AmmoUser/" + t))
            for t in ("range", "warmupTime", "recoilAmount", "burstShotCount"): put("gun.verb." + t, op.findtext("Properties/" + t))
        elif cls in ("PatchOperationAdd", "PatchOperationReplace"):
            xp = op.findtext("xpath") or ""
            for stat in op.findall("value/*"):
                if stat.tag.startswith("ArmorRating_"): put("armor." + stat.tag, stat.text)
                elif stat.tag in ("Bulk", "WornBulk", "StuffEffectMultiplierArmor"): put("misc." + stat.tag, stat.text)
            for stat in op.findall("value/statBases/*"):
                if stat.tag.startswith("ArmorRating_") or stat.tag in ("Bulk", "WornBulk"): put(("armor." if stat.tag.startswith("Armor") else "misc.") + stat.tag, stat.text)
            for tool in op.iter("li"):
                if tool.get("Class") == "CombatExtended.ToolCE":
                    p = tool.findtext("power"); b = tool.findtext("armorPenetrationBlunt"); s = tool.findtext("armorPenetrationSharp")
                    put("tool.power", p); put("tool.apBlunt", b); put("tool.apSharp", s)
                    try:
                        if b: put("tool.apBlunt_over_power", float(b) / float(p))
                        if s: put("tool.apSharp_over_power", float(s) / float(p))
                    except (TypeError, ValueError, ZeroDivisionError): pass
def q(xs):
    xs = sorted(xs); n = len(xs)
    f = lambda p: xs[min(n - 1, int(p * (n - 1) + 0.5))]
    return {"n": n, "min": xs[0], "p10": f(.1), "p50": f(.5), "p90": f(.9), "max": xs[-1], "mean": round(st.mean(xs), 3)}
out = {k: q(v) for k, v in sorted(acc.items()) if v}
json.dump(out, open(a.out, "w"), indent=1, sort_keys=True)
for k, v in out.items(): print(k, v)
