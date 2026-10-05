#!/usr/bin/env python3
"""Resolve every vanilla weapon with the reference def engine and write vanilla-ranged.json / vanilla-melee.json.

Usage:
    PYTHONDONTWRITEBYTECODE=1 python3 extract.py --game /path/to/RimWorld [--out DIR] [--engine DIR]
Environment fallbacks: RIMWORLD_DIR (game folder), DEF_ENGINE_DIR (folder of the def-engine prototype).
Runs from any working directory. Output is deterministic (sorted by defName).

Also writes stuffs.json (materials), quality_factors.json (StatPart_Quality factors of weapon stats) and
damage_defs.json (damage kinds used by weapons).
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE))
import wlib  # noqa: E402

CORE_DIRS = "Core,Royalty,Ideology,Biotech,Anomaly,Odyssey".split(",")
RANGED_CATS = {"WeaponsRanged", "Grenades", "WeaponsUnique"}
MELEE_CATS = {"WeaponsMelee", "WeaponsMeleeBladelink"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default=os.environ.get("RIMWORLD_DIR"))
    ap.add_argument("--engine", default=os.environ.get("DEF_ENGINE_DIR", str(HERE.parent / "def-engine")))
    ap.add_argument("--out", default=str(HERE))
    a = ap.parse_args()
    if not a.game:
        ap.error("--game or RIMWORLD_DIR required")
    sys.path.insert(0, a.engine)
    from defengine import LoadConfig, load_game  # noqa: E402

    game = Path(a.game)
    mods = [game / "Data" / n for n in CORE_DIRS if (game / "Data" / n).is_dir()]
    res = load_game(LoadConfig(mods=mods, game_dir=game, types=str(Path(a.engine) / "data" / "def_types_vanilla.json")))
    things = res.database("ThingDef")
    dmgdefs = res.database("DamageDef")
    projects = res.database("ResearchProjectDef")
    stats = res.database("StatDef")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    version = (game / "Version.txt").read_text().strip() if (game / "Version.txt").exists() else None

    # ------------------------------------------------------------------ market value helper (formula, recursive)
    mv_cache = {}

    def base_mv(name: str) -> float:
        if name in mv_cache:
            return mv_cache[name]
        d = things.by_name.get(name)
        if d is None:
            mv_cache[name] = 0.0
            return 0.0
        sb = wlib.stat_dict(d.node)
        if "MarketValue" in sb:
            v = sb["MarketValue"]
        else:
            v = computed_mv(d.node, None)["value"]
        mv_cache[name] = v
        return v

    def computed_mv(node, stuff):
        """StatWorker_MarketValue.CalculatedBaseMarketValue for a def without an explicit MarketValue."""
        sb = wlib.stat_dict(node)
        cost = wlib.pairs(node, "costList")
        ingredients = sum(cnt * base_mv(k) for k, cnt in cost.items())
        stuff_count = int(wlib.num(node.findtext("costStuffCount"), 0))
        stuff_part = 0.0
        if stuff_count > 0:
            if stuff is None:
                stuff_part = stuff_count * wlib.DEFAULT_STUFF_GUESS
            else:
                sd = things.by_name[stuff]
                vol = wlib.num(sd.node.findtext("stuffProps/volumePerUnit") or sd.node.findtext("volumePerUnit"), 1.0) or 1.0
                stuff_part = stuff_count / vol * base_mv(stuff)
        work = max(sb.get("WorkToMake", 0.0), sb.get("WorkToBuild", 0.0))
        work_part = work * wlib.VALUE_PER_WORK if work > 2 else 0.0
        return {"value": ingredients + stuff_part + work_part, "ingredients": ingredients, "stuff": stuff_part,
                "work": work_part}

    # ------------------------------------------------------------------ stuffs
    stuffs = {}
    for d in things.defs:
        sp = d.node.find("stuffProps")
        if sp is None:
            continue
        cats = [li.text for li in sp.findall("categories/li")]
        sb = wlib.stat_dict(d.node)
        stuffs[d.def_name] = {
            "categories": cats,
            "market_value": sb.get("MarketValue", base_mv(d.def_name)),
            "volume_per_unit": wlib.num(d.node.findtext("stuffProps/volumePerUnit") or d.node.findtext("volumePerUnit"), 1.0),
            "sharp_damage_mult": sb.get("SharpDamageMultiplier", 1.0),
            "blunt_damage_mult": sb.get("BluntDamageMultiplier", 1.0),
            "sharp_armor": sb.get("StuffPower_Armor_Sharp"),
            "stat_factors": wlib.pairs(d.node, "stuffProps/statFactors"),
            "stat_offsets": wlib.pairs(d.node, "stuffProps/statOffsets"),
            "tech_level": d.node.findtext("techLevel"),
        }
    (out / "stuffs.json").write_text(json.dumps(stuffs, indent=1, sort_keys=True))

    # ------------------------------------------------------------------ quality factors of weapon stats
    qf = {}
    for s in stats.defs:
        parts = s.node.find("parts")
        if parts is None:
            continue
        for li in parts.findall("li"):
            if (li.get("Class") or "").startswith("StatPart_Quality"):
                row = {c.tag: wlib.num(c.text) for c in li if isinstance(c.tag, str)}
                qf[s.def_name] = {"class": li.get("Class"), **row}
    (out / "quality_factors.json").write_text(json.dumps(qf, indent=1, sort_keys=True))


    # ------------------------------------------------------------------ maneuvers (tool capacity -> damage kind)
    cats_def = res.database("DamageArmorCategoryDef")
    man = {}
    for d in res.database("ManeuverDef").defs:
        dn = d.node.findtext("verb/meleeDamageDef")
        dd_ = dmgdefs.by_name.get(dn)
        ac = dd_.node.findtext("armorCategory") if dd_ is not None else None
        ms = None
        if ac and ac in cats_def.by_name:
            ms = cats_def.by_name[ac].node.findtext("multStat")
        man.setdefault(d.node.findtext("requiredCapacity"), []).append({
            "maneuver": d.def_name, "verbClass": d.node.findtext("verb/verbClass"), "meleeDamageDef": dn,
            "armorCategory": ac, "multStat": ms,
            "commonality": wlib.num(d.node.findtext("verb/commonality"), 1.0)})
    (out / "maneuvers.json").write_text(json.dumps(man, indent=1, sort_keys=True))

    used_damage = set()

    def damage_info(name):
        d = dmgdefs.by_name.get(name)
        if d is None:
            return None
        n = d.node
        used_damage.add(name)
        return {"defName": name, "workerClass": n.findtext("workerClass"),
                "defaultDamage": wlib.num(n.findtext("defaultDamage"), -1),
                "defaultArmorPenetration": wlib.num(n.findtext("defaultArmorPenetration"), -1),
                "defaultStoppingPower": wlib.num(n.findtext("defaultStoppingPower"), 0),
                "armorCategory": n.findtext("armorCategory"), "isRanged": n.findtext("isRanged"),
                "isExplosive": n.findtext("isExplosive")}

    def research_info(node):
        names = [li.text for li in node.findall("recipeMaker/researchPrerequisites/li")]
        one = node.findtext("recipeMaker/researchPrerequisite")
        if one:
            names.append(one)
        names += [li.text for li in node.findall("researchPrerequisites/li")]
        info = []
        for nme in names:
            p = projects.by_name.get(nme)
            if p:
                info.append({"defName": nme, "baseCost": wlib.num(p.node.findtext("baseCost")),
                             "techLevel": p.node.findtext("techLevel"),
                             "prerequisites": [li.text for li in p.node.findall("prerequisites/li")]})
            else:
                info.append({"defName": nme})
        return info

    def common(d):
        n = d.node
        sb = wlib.stat_dict(n)
        cost = wlib.pairs(n, "costList")
        stuff_count = int(wlib.num(n.findtext("costStuffCount"), 0))
        explicit = "MarketValue" in sb
        calc = computed_mv(n, None)
        rec = {
            "defName": d.def_name,
            "label": n.findtext("label"),
            "source": (d.mod_id or "").replace("Ludeon.RimWorld.", "").replace("Ludeon.RimWorld", "Core"),
            "file": d.file,
            "parents": [p.split(":")[-1] for p in d.parents],
            "techLevel": n.findtext("techLevel"),
            "weaponTags": [li.text for li in n.findall("weaponTags/li")],
            "weaponClasses": [li.text for li in n.findall("weaponClasses/li")],
            "thingCategories": [li.text for li in n.findall("thingCategories/li")],
            "tradeTags": [li.text for li in n.findall("tradeTags/li")],
            "statBases": sb,
            "statBaseCount": len(sb),
            "equippedStatOffsets": wlib.pairs(n, "equippedStatOffsets"),
            "cost": {"costList": cost, "costStuffCount": stuff_count,
                     "stuffCategories": [li.text for li in n.findall("stuffCategories/li")]},
            "recipe": {
                "workToMake": sb.get("WorkToMake"),
                "workSkill": n.findtext("recipeMaker/workSkill"),
                "skillRequirements": wlib.pairs(n, "recipeMaker/skillRequirements"),
                "recipeUsers": [li.text for li in n.findall("recipeMaker/recipeUsers/li")],
                "research": research_info(n),
                "craftable": n.find("recipeMaker") is not None,
            },
            "trade": {
                "tradeability": n.findtext("tradeability"),
                "generateCommonality": wlib.num(n.findtext("generateCommonality"), 1.0) if n.find("generateCommonality") is not None else None,
                "generateAllowChance": wlib.num(n.findtext("generateAllowChance"), 1.0) if n.find("generateAllowChance") is not None else None,
                "SellPriceFactor": sb.get("SellPriceFactor"),
                "destroyOnDrop": n.findtext("destroyOnDrop"),
            },
            "marketValue": {
                "explicit": sb.get("MarketValue") if explicit else None,
                "formula_unstuffed": round(calc["value"], 3),
                "formula_parts": {k: round(v, 3) for k, v in calc.items() if k != "value"},
                "effective": sb["MarketValue"] if explicit else round(calc["value"], 3),
            },
            "comps": [li.get("Class") or li.findtext("compClass") for li in n.findall("comps/li")],
            "graphic": {"texPath": n.findtext("graphicData/texPath"), "graphicClass": n.findtext("graphicData/graphicClass")},
            "stuffed": stuff_count > 0 or n.find("stuffCategories") is not None,
        }
        return rec

    def tool_list(n):
        tools = []
        for li in n.findall("tools/li"):
            t = {
                "id": li.findtext("id"),
                "label": li.findtext("label"),
                "capacities": [c.text for c in li.findall("capacities/li")],
                "power": wlib.num(li.findtext("power")),
                "cooldownTime": wlib.num(li.findtext("cooldownTime")),
                "armorPenetration": wlib.num(li.findtext("armorPenetration")) if li.find("armorPenetration") is not None else None,
                "chanceFactor": wlib.num(li.findtext("chanceFactor"), 1.0),
                "linkedBodyPartsGroup": li.findtext("linkedBodyPartsGroup"),
                "extraMeleeDamages": [{"def": e.findtext("def"), "amount": wlib.num(e.findtext("amount")),
                                       "armorPenetration": wlib.num(e.findtext("armorPenetration")) if e.find("armorPenetration") is not None else None}
                                      for e in li.findall("extraMeleeDamages/li")],
                "hediff": li.findtext("hediff"),
                "surpriseAttack": wlib.to_py(li.find("surpriseAttack")) if li.find("surpriseAttack") is not None else None,
            }
            tools.append(t)
        return tools

    ranged, melee, other_tool_items = [], [], []
    for d in sorted(things.defs, key=lambda x: x.def_name):
        n = d.node
        if n.findtext("equipmentType") != "Primary" or n.findtext("category") != "Item":
            continue
        cats = set(li.text for li in n.findall("thingCategories/li"))
        tags = [li.text for li in n.findall("weaponTags/li")]
        has_verbs = n.find("verbs") is not None
        if cats & RANGED_CATS or (has_verbs and ("TurretGun" in tags or "Artillery" in tags)):
            r = common(d)
            v = n.find("verbs/li")
            verb = wlib.to_py(v)
            proj_name = verb.get("defaultProjectile")
            proj = things.by_name.get(proj_name) if proj_name else None
            p = {}
            if proj is not None:
                pn = proj.node.find("projectile")
                pj = wlib.to_py(pn) if pn is not None else {}
                dd = damage_info(pj.get("damageDef"))
                p = {"defName": proj_name, "projectile": pj, "damageDef": dd,
                     "thingClass": proj.node.findtext("thingClass"),
                     "projectileComps": [li.get("Class") for li in proj.node.findall("comps/li")]}
            beam = None
            if verb.get("beamDamageDef"):
                beam = damage_info(verb["beamDamageDef"])
            r["verb"] = verb
            r["verbCount"] = len(n.findall("verbs/li"))
            r["projectile"] = p
            r["beamDamage"] = beam
            r["bashTools"] = tool_list(n)
            # classification used by the analysis
            if "TurretGun" in tags or "Artillery" in tags or d.def_name.endswith("Turret"):
                grp = "turret"
            elif d.def_name.endswith("_Unique"):
                grp = "unique"
            elif any(t.startswith("Mechanoid") or t in ("BeamGraserGun", "HellsphereCannonGun", "ChargeBlasterHeavyGun",
                                                            "InfernoCannonGun", "SentryDroneGunShortRange", "NerveSpiker") for t in tags) \
                    or d.def_name in ("Gun_Needle", "Gun_Scattergun", "NerveSpiker"):
                grp = "mech_or_creature"
            else:
                grp = "standard"
            r["group"] = grp
            ranged.append(r)
        elif cats & MELEE_CATS:
            m = common(d)
            m["tools"] = tool_list(n)
            m["group"] = "bladelink" if "WeaponsMeleeBladelink" in cats else "standard"
            melee.append(m)
        elif n.find("tools") is not None:
            other_tool_items.append({"defName": d.def_name, "categories": sorted(cats), "source": (d.mod_id or "")})

    meta = {
        "game_version": version,
        "date_generated_by": "extract.py (deterministic)",
        "engine_stats": res.stats,
        "licence_note": "Research data derived from Ludeon's proprietary game files; not for redistribution. "
                        "RimStudio reads these values from the user's own install at runtime.",
    }
    (out / "vanilla-ranged.json").write_text(json.dumps({"meta": meta, "weapons": ranged}, indent=1, sort_keys=False))
    (out / "vanilla-melee.json").write_text(json.dumps({"meta": meta, "weapons": melee, "non_weapon_tool_items": other_tool_items}, indent=1))
    dd = {k: damage_info(k) for k in sorted(used_damage)}
    (out / "damage_defs.json").write_text(json.dumps(dd, indent=1))
    print("ranged", len(ranged), "melee", len(melee), "stuffs", len(stuffs), "damage defs", len(dd))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
