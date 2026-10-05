#!/usr/bin/env python3
"""Resolve every apparel ThingDef, every stuff and the stat/body definitions they depend on.

Usage:  extract.py [--game DIR] [--out DIR] [--engine DIR]
  RIMWORLD_DIR is the environment fallback for --game (folder holding Version.txt and Data/).
  --engine defaults to ../def-engine next to this script (the reference def engine, read only).

Writes (into --out, default: this script's folder):
  vanilla-apparel.json   one record per apparel ThingDef (identity, layers, groups, stats, cost, recipe, comps ...)
  vanilla-stuff.json     one record per stuff, plus the stat definitions the model needs and the human body parts
Nothing is read from Combat Extended or RimSort. The game files are read at runtime from --game.
"""
import argparse, json, os, sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
DLC = "Core Royalty Ideology Biotech Anomaly Odyssey".split()


def num(s):
    try:
        v = float(s)
        return int(v) if v == int(v) and "." not in s else v
    except (TypeError, ValueError):
        return s


def kv(el):
    """<A>1</A><B>2</B> -> {"A": 1, "B": 2} (statBases, costList, factors ...)."""
    if el is None:
        return {}
    return {c.tag: num((c.text or "").strip()) for c in el if isinstance(c.tag, str)}


def lis(el):
    if el is None:
        return []
    return [(c.text or "").strip() for c in el if isinstance(c.tag, str) and c.tag == "li"]


def txt(el, tag, default=None):
    c = el.find(tag)
    return (c.text or "").strip() if c is not None and c.text is not None else default


def flt(el, tag, default=None):
    v = txt(el, tag)
    return num(v) if v is not None else default


def parse_body(res):
    """Human body parts with the game's coverageAbs rule (Verse.BodyDef, decompiled:Verse/BodyDef.cs)."""
    root = res.get("BodyDef", "Human").node.find("corePart")
    out = []

    def walk(n, parent_abs_children, depth, path):
        cov = flt(n, "coverage", 1.0)
        abs_with = 1.0 if parent_abs_children is None else parent_abs_children * cov
        kids = n.find("parts")
        kids = [k for k in kids if isinstance(k.tag, str)] if kids is not None else []
        rest = 1.0 - sum(flt(k, "coverage", 0.0) for k in kids)
        if abs(rest) < 1e-5 or rest <= 0:
            rest = 0.0
        out.append({"def": txt(n, "def"), "label": txt(n, "customLabel"), "coverage": cov, "height": txt(n, "height", "Undefined"),
                    "depth": txt(n, "depth", "Outside"), "groups": lis(n.find("groups")), "coverageAbs": round(abs_with * rest, 6),
                    "coverageAbsWithChildren": round(abs_with, 6), "path": path})
        for k in kids:
            walk(k, abs_with, depth + 1, path + "/" + txt(k, "def"))
    walk(root, None, 0, txt(root, "def"))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default=os.environ.get("RIMWORLD_DIR"))
    ap.add_argument("--out", default=str(HERE))
    ap.add_argument("--engine", default=str(HERE.parent / "def-engine"))
    a = ap.parse_args()
    if not a.game:
        sys.exit("give --game or set RIMWORLD_DIR")
    G = Path(a.game)
    sys.path.insert(0, a.engine)
    from defengine.engine import LoadConfig, load_game
    mods = [G / "Data" / n for n in DLC if (G / "Data" / n).is_dir()]
    res = load_game(LoadConfig(mods=mods, game_dir=G, types=str(Path(a.engine) / "data" / "def_types_vanilla.json")))
    version = (G / "Version.txt").read_text().strip()
    things = res.database("ThingDef").defs

    # recipes that make a thing (hand written RecipeDefs)
    recipes = {}
    for r in res.database("RecipeDef").defs:
        p = r.node.find("products")
        if p is None:
            continue
        for c in p:
            if isinstance(c.tag, str):
                recipes.setdefault(c.tag, []).append({"recipe": r.def_name, "workAmount": flt(r.node, "workAmount"),
                                                      "ingredients": [{"fixed": txt(i, "filter/thingDefs/li") if i.find("filter/thingDefs/li") is not None else None,
                                                                       "count": flt(i, "count")} for i in (r.node.find("ingredients") if r.node.find("ingredients") is not None else [])]})

    def thing_common(d):
        n = d.node
        comps = []
        c = n.find("comps")
        if c is not None:
            for li in c:
                if not isinstance(li.tag, str):
                    continue
                comps.append(li.get("Class") or txt(li, "compClass") or "?")
        return {"defName": d.def_name, "label": txt(n, "label"), "mod": d.mod_id, "file": d.file, "parents": d.parents,
                "techLevel": txt(n, "techLevel", "Undefined"), "thingClass": txt(n, "thingClass"),
                "statBases": kv(n.find("statBases")), "costList": kv(n.find("costList")), "costStuffCount": flt(n, "costStuffCount", 0),
                "comps": comps, "tradeTags": lis(n.find("tradeTags")), "thingCategories": lis(n.find("thingCategories")),
                "tradeability": txt(n, "tradeability", "All"), "thingSetMakerTags": lis(n.find("thingSetMakerTags")),
                "smeltable": txt(n, "smeltable"), "burnableByRecipe": txt(n, "burnableByRecipe"), "healthAffectsPrice": txt(n, "healthAffectsPrice"),
                "useHitPoints": txt(n, "useHitPoints"), "generateCommonality": flt(n, "generateCommonality"),
                "stuffCategories": lis(n.find("stuffCategories"))}

    apparel, stuff = [], []
    for d in things:
        n = d.node
        ap_el = n.find("apparel")
        if ap_el is not None:
            r = thing_common(d)
            rm = n.find("recipeMaker")
            rmd = None
            if rm is not None:
                rmd = {"workSkill": txt(rm, "workSkill"), "workSpeedStat": txt(rm, "workSpeedStat"), "productCount": flt(rm, "productCount", 1),
                       "researchPrerequisite": txt(rm, "researchPrerequisite"), "researchPrerequisites": lis(rm.find("researchPrerequisites")),
                       "skillRequirements": kv(rm.find("skillRequirements")), "recipeUsers": lis(rm.find("recipeUsers")),
                       "unfinishedThingDef": txt(rm, "unfinishedThingDef")}
            r.update({
                "layers": lis(ap_el.find("layers")), "bodyPartGroups": lis(ap_el.find("bodyPartGroups")), "tags": lis(ap_el.find("tags")),
                "defaultOutfitTags": lis(ap_el.find("defaultOutfitTags")), "abilities": lis(ap_el.find("abilities")),
                "wearPerDay": flt(ap_el, "wearPerDay", 0.4), "useDeflectMetalEffect": txt(ap_el, "useDeflectMetalEffect"),
                "scoreOffset": flt(ap_el, "scoreOffset", 0), "gender": txt(ap_el, "gender", "None"),
                "developmentalStageFilter": txt(ap_el, "developmentalStageFilter"), "slaveApparel": txt(ap_el, "slaveApparel"),
                "mechanitorApparel": txt(ap_el, "mechanitorApparel"), "legsNakedUnlessCoveredBySomethingElse": txt(ap_el, "legsNakedUnlessCoveredBySomethingElse"),
                "canBeGeneratedToSatisfyWarmth": txt(ap_el, "canBeGeneratedToSatisfyWarmth"), "countsAsClothingForNudity": txt(ap_el, "countsAsClothingForNudity"),
                "equippedStatOffsets": kv(n.find("equippedStatOffsets")), "recipeMaker": rmd, "recipeDefs": recipes.get(d.def_name, []),
                "stuffed": n.find("stuffCategories") is not None})
            apparel.append(r)
        sp = n.find("stuffProps")
        if sp is not None:
            r = thing_common(d)
            def ql(tag):
                e = sp.find(tag)
                return [{"stat": c.tag, **{q.tag: num((q.text or "").strip()) for q in c if isinstance(q.tag, str)}} for c in e] if e is not None else []
            r.update({"stuffCategories_of_stuff": lis(sp.find("categories")), "commonality": flt(sp, "commonality", 1.0),
                      "allowedInStuffGeneration": txt(sp, "allowedInStuffGeneration", "true"), "stuffAdjective": txt(sp, "stuffAdjective"),
                      "statFactors": kv(sp.find("statFactors")), "statOffsets": kv(sp.find("statOffsets")),
                      "statFactorsQuality": ql("statFactorsQuality"), "statOffsetsQuality": ql("statOffsetsQuality"),
                      "smallVolume": txt(n, "smallVolume", "false"), "stackLimit": flt(n, "stackLimit", 1), "isAirtight": txt(sp, "isAirtight"),
                      "appearance": txt(sp, "appearance"), "canSuggestUseDefaultStuff": txt(sp, "canSuggestUseDefaultStuff")})
            stuff.append(r)

    # stat definitions needed by the model
    stats = {}
    for d in res.database("StatDef").defs:
        n = d.node
        parts = []
        pe = n.find("parts")
        if pe is not None:
            for li in pe:
                if not isinstance(li.tag, str):
                    continue
                p = {"class": li.get("Class"), "priority": flt(li, "priority", 0)}
                for k in ("stuffPowerStat", "multiplierStat", "applyToNegativeValues"):
                    if txt(li, k) is not None:
                        p[k] = txt(li, k)
                for c in li:
                    if isinstance(c.tag, str) and (c.tag.startswith("factor") or c.tag.startswith("maxGain")):
                        p[c.tag] = num((c.text or "").strip())
                parts.append(p)
        stats[d.def_name] = {"defaultBaseValue": flt(n, "defaultBaseValue", 0), "minValue": flt(n, "minValue", 0), "maxValue": flt(n, "maxValue", 1e9),
                             "roundValue": txt(n, "roundValue", "false"), "roundToFiveOver": flt(n, "roundToFiveOver", 1e9),
                             "applyFactorsIfNegative": txt(n, "applyFactorsIfNegative", "true"), "workerClass": txt(n, "workerClass"), "parts": parts}
    keep = {k: v for k, v in stats.items() if k.startswith(("ArmorRating", "Insulation", "StuffPower", "StuffEffect")) or k in
            ("MarketValue", "MaxHitPoints", "Mass", "WorkToMake", "Flammability", "Beauty", "DeteriorationRate", "SharpDamageMultiplier", "BluntDamageMultiplier",
             "EquipDelay", "MoveSpeed", "WorkSpeedGlobal", "CarryingCapacity", "SellPriceFactor", "ShootingAccuracyPawn", "AimingDelayFactor", "MeleeDodgeChance",
             "ToxicEnvironmentResistance", "VacuumResistance", "PsychicSensitivity", "ComfyTemperatureMin", "ComfyTemperatureMax")}
    # base market values of every thing used as a cost ingredient (None: the game calculates it from a recipe)
    refd = set()
    for r in apparel:
        refd |= set(r["costList"])
    thing_values = {}
    for name in sorted(refd):
        rec = res.get("ThingDef", name)
        thing_values[name] = rec.node.find("statBases/MarketValue") is not None and flt(rec.node.find("statBases"), "MarketValue") or None
    layers = {d.def_name: {"drawOrder": flt(d.node, "drawOrder")} for d in res.database("ApparelLayerDef").defs}
    groups = {d.def_name: {"label": txt(d.node, "label"), "listOrder": flt(d.node, "listOrder")} for d in res.database("BodyPartGroupDef").defs}
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    meta = {"game_version": version, "mods": [m.package_id for m in res.mods], "apparel_count": len(apparel), "stuff_count": len(stuff)}
    (out / "vanilla-apparel.json").write_text(json.dumps({"meta": meta, "apparel": apparel}, indent=1, sort_keys=True))
    (out / "vanilla-stuff.json").write_text(json.dumps({"meta": meta, "stuff": stuff, "stats": keep, "apparelLayers": layers,
                                                         "bodyPartGroups": groups, "humanBodyParts": parse_body(res),
                                                         "thingValues": thing_values, "qualityLevels": ["Awful", "Poor", "Normal", "Good", "Excellent", "Masterwork", "Legendary"]},
                                                        indent=1, sort_keys=True))
    print(meta)


if __name__ == "__main__":
    main()
