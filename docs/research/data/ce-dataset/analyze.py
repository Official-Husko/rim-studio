#!/usr/bin/env python3
"""Analyse the resolved CE datasets: flat CSVs, markdown tables (for the research note) and figures.

Usage: analyze.py [--data DIR] [--tables DIR] [--figs DIR]
Reads ce-ranged/ce-melee/ce-apparel/ce-ammo/ce-presets .json (from build_dataset.py); writes ranged.csv, melee.csv,
apparel.csv, ammo_calibers.csv in --data, one markdown table per file in --tables, PNG figures in --figs.
Weapon class names below are OUR OWN grouping (not CE's); CE's own grouping is its weaponTags and GunPatcher presets.
"""
import argparse
import csv
import json
import statistics as st
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.dont_write_bytecode = True

CLASS = {  # personal ranged weapons, our grouping (defName -> class)
    "Gun_Revolver": "pistol", "Gun_Autopistol": "pistol",
    "Gun_MachinePistol": "smg", "Gun_HeavySMG": "smg",
    "Gun_PumpShotgun": "shotgun", "Gun_ChainShotgun": "shotgun", "Gun_MiniShotgun": "shotgun",
    "Gun_AssaultRifle": "rifle", "Gun_HellcatRifle": "rifle", "Gun_ChargeRifle": "rifle", "Gun_BoltActionRifle": "rifle", "Gun_Slugthrower": "rifle",
    "Gun_SniperRifle": "sniper", "Gun_Needle": "sniper", "Gun_ToxicNeedle": "sniper", "Gun_ChargeLance": "sniper",
    "Gun_LMG": "machine gun", "Gun_Minigun": "machine gun", "Gun_ChargeBlasterHeavy": "machine gun", "CE_MechanoidMinigun": "machine gun", "Gun_BeamRepeater": "machine gun",
    "Gun_IncendiaryLauncher": "launcher", "Gun_InfernoCannon": "launcher", "Gun_ThumpCannon": "launcher", "CE_GrenadeLauncher": "launcher",
    "CE_DisposableRocketLauncher": "launcher", "Gun_DoomsdayRocket": "launcher", "Gun_TripleRocket": "launcher", "CE_ThermalBoltProjector": "launcher",
    "Gun_Incinerator": "flamer", "Gun_MiniFlameblaster": "flamer",
    "Bow_Short": "bow", "Bow_Recurve": "bow", "Bow_Great": "bow", "NerveSpiker": "bow", "Pila": "bow",
}
PERSONAL_FIREARMS = ["pistol", "smg", "shotgun", "rifle", "sniper", "machine gun"]


def med(v):
    v = [x for x in v if isinstance(x, (int, float))]
    return st.median(v) if v else None


def fmt(x, nd=2):
    if x is None:
        return "-"
    if isinstance(x, float):
        s = ("%." + str(nd) + "f") % x
        return s.rstrip("0").rstrip(".") if "." in s else s
    return str(x)


def rng(v, nd=2):
    v = [x for x in v if isinstance(x, (int, float))]
    if not v:
        return "-"
    return "%s (%s-%s)" % (fmt(st.median(v), nd), fmt(min(v), nd), fmt(max(v), nd))


def md(headers, rows):
    out = ["| " + " | ".join(headers) + " |", "|" + "|".join(["---"] * len(headers)) + "|"]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out) + "\n"


def load(p):
    return json.loads(Path(p).read_text())


def main():
    ap = argparse.ArgumentParser()
    here = Path(__file__).resolve().parent
    ap.add_argument("--data", default=str(here))
    ap.add_argument("--tables", default=str(here / "tables"))
    ap.add_argument("--figs", default=str(here / "figures"))
    a = ap.parse_args()
    data, tdir, fdir = Path(a.data), Path(a.tables), Path(a.figs)
    tdir.mkdir(parents=True, exist_ok=True)
    fdir.mkdir(parents=True, exist_ok=True)
    ranged = load(data / "ce-ranged.json")["weapons"]
    melee = load(data / "ce-melee.json")["weapons"]
    app = load(data / "ce-apparel.json")
    ammo = load(data / "ce-ammo.json")
    presets = load(data / "ce-presets.json")
    T = {}

    # ------------------------------------------------------------ ranged flat rows
    rows = []
    for w in ranged:
        s, v, au, fm = w["stats"], w["verb"], w.get("ammoUser") or {}, w.get("fireModes") or {}
        p = (w.get("defaultProjectile") or {}).get("props", {})
        cls = CLASS.get(w["defName"], "turret" if "TurretGun" in w["weaponTags"] else ("grenade" if "grenade" in w["defName"].lower() else "other"))
        tick = v.get("ticksBetweenBurstShots")
        van = w.get("vanilla") or {}
        vv, vs, vp = van.get("verb") or {}, van.get("stats") or {}, ((van.get("defaultProjectile") or {}).get("props") or {})
        sec = p.get("secondaryDamage")
        rows.append(dict(defName=w["defName"], label=w["label"], source=w["source"].split(".")[-1], tech=w["techLevel"], cls=cls,
                         unique=w["defName"].endswith("_Unique"), ceConverted=w["isCEConverted"],
                         ammoSet=au.get("ammoSet"), Mass=s.get("Mass"), Bulk=s.get("Bulk"), cooldown=s.get("RangedWeapon_Cooldown"),
                         sights=s.get("SightsEfficiency"), spread=s.get("ShotSpread"), sway=s.get("SwayFactor"), recoil=v.get("recoilAmount"),
                         range=v.get("range"), warmup=v.get("warmupTime"), burst=v.get("burstShotCount"), ticksBetween=tick,
                         rpm=round(3600 / tick) if isinstance(tick, (int, float)) and tick else None,
                         mag=au.get("magazineSize"), reload=au.get("reloadTime"), oneAtATime=au.get("reloadOneAtATime"),
                         aimedBurst=fm.get("aimedBurstShotCount"), noSnapshot=fm.get("noSnapshot"), noSingle=fm.get("noSingleShot"), aiAim=fm.get("aiAimMode"),
                         projectile=(w.get("defaultProjectile") or {}).get("defName"), damage=p.get("damageAmountBase"), damageDef=p.get("damageDef"),
                         apSharp=p.get("armorPenetrationSharp"), apBlunt=p.get("armorPenetrationBlunt"), speed=p.get("speed"),
                         pellets=p.get("pelletCount"), spreadMult=p.get("spreadMult"), hasSecondary=bool(sec),
                         van_range=vv.get("range"), van_warmup=vv.get("warmupTime"), van_cooldown=vs.get("RangedWeapon_Cooldown"), van_mass=vs.get("Mass"),
                         van_damage=vp.get("damageAmountBase"), van_speed=vp.get("speed"), van_burst=vv.get("burstShotCount"),
                         van_accShort=vs.get("AccuracyShort"), van_accLong=vs.get("AccuracyLong")))
    with open(data / "ranged.csv", "w", newline="") as f:
        wr = csv.DictWriter(f, fieldnames=list(rows[0]))
        wr.writeheader()
        wr.writerows(rows)
    base = [r for r in rows if not r["unique"]]

    # class summary (personal firearms and others)
    order = PERSONAL_FIREARMS + ["launcher", "flamer", "bow", "grenade", "turret"]
    t = []
    for c in order:
        g = [r for r in base if r["cls"] == c and r["ammoSet"] is not None or (r["cls"] == c and c in ("grenade",))]
        g = [r for r in base if r["cls"] == c]
        if not g:
            continue
        t.append([c, len(g), rng([r["Mass"] for r in g]), rng([r["Bulk"] for r in g]), rng([r["cooldown"] for r in g]), rng([r["spread"] for r in g]),
                  rng([r["sway"] for r in g]), rng([r["sights"] for r in g]), rng([r["recoil"] for r in g])])
    T["ranged_class_a"] = md(["class", "n", "Mass kg", "Bulk", "cooldown s", "ShotSpread", "SwayFactor", "SightsEff", "recoilAmount"], t)
    t = []
    for c in order:
        g = [r for r in base if r["cls"] == c]
        if not g:
            continue
        t.append([c, len(g), rng([r["range"] for r in g], 1), rng([r["warmup"] for r in g]), rng([r["mag"] for r in g], 0), rng([r["reload"] for r in g]),
                  rng([r["rpm"] for r in g], 0), rng([r["damage"] for r in g], 1), rng([r["apSharp"] for r in g], 1), rng([r["speed"] for r in g], 0)])
    T["ranged_class_b"] = md(["class", "n", "range cells", "warmup s", "mag", "reload s", "rpm (burst)", "damage", "AP sharp mm", "speed"], t)

    # personal weapons catalogue
    cat = [r for r in base if r["cls"] in PERSONAL_FIREARMS]
    cat.sort(key=lambda r: (PERSONAL_FIREARMS.index(r["cls"]), r["Mass"] or 0))
    T["ranged_catalogue"] = md(["defName", "class", "Mass", "Bulk", "cooldown", "spread", "sway", "sights", "recoil", "range", "warmup", "mag", "reload", "burst/rpm", "caliber (ammoSet)"],
                               [[r["defName"].replace("Gun_", ""), r["cls"], fmt(r["Mass"]), fmt(r["Bulk"]), fmt(r["cooldown"]), fmt(r["spread"]), fmt(r["sway"]), fmt(r["sights"]), fmt(r["recoil"]),
                                 fmt(r["range"], 1), fmt(r["warmup"]), fmt(r["mag"], 0), fmt(r["reload"]), "%s/%s" % (fmt(r["burst"], 0), fmt(r["rpm"], 0)),
                                 (r["ammoSet"] or "-").replace("AmmoSet_", "")] for r in cat])

    # CE vs vanilla
    t = []
    for r in base:
        if r["cls"] in PERSONAL_FIREARMS and r["van_range"] is not None:
            t.append([r["defName"].replace("Gun_", ""), "%s -> %s" % (fmt(r["van_range"], 1), fmt(r["range"], 1)), "%s -> %s" % (fmt(r["van_warmup"]), fmt(r["warmup"])),
                      "%s -> %s" % (fmt(r["van_cooldown"]), fmt(r["cooldown"])), "%s -> %s" % (fmt(r["van_damage"], 0), fmt(r["damage"], 0)),
                      "%s -> %s" % (fmt(r["van_speed"], 0), fmt(r["speed"], 0)), "%s -> %s" % (fmt(r["van_mass"]), fmt(r["Mass"])), fmt(r["apSharp"], 1), fmt(r["apBlunt"], 1)])
    T["ce_vs_vanilla_guns"] = md(["gun", "range (vanilla -> CE)", "warmup s", "cooldown s", "damage", "speed", "Mass kg", "AP sharp", "AP blunt"], t)

    # fire modes
    fmc = Counter()
    for r in base:
        if r["cls"] in PERSONAL_FIREARMS:
            fmc[(r["cls"], "burst>1" if (r["burst"] or 1) > 1 else "single", "aimedBurst" if r["aimedBurst"] else "-", "noSnapshot" if r["noSnapshot"] else "-", r["aiAim"] or "-")] += 1
    T["firemodes"] = md(["class", "verb burst", "aimedBurstShotCount", "snapshot", "aiAimMode", "n"], [list(k) + [v] for k, v in sorted(fmc.items())])

    # ------------------------------------------------------------ ammo
    sets = {s["defName"]: s for s in ammo["ammoSets"]}
    ammo_by = {a_["defName"]: a_ for a_ in ammo["ammo"]}
    users = defaultdict(list)
    for r in rows:
        if r["ammoSet"] and not r["unique"]:
            users[r["ammoSet"]].append(r["defName"])

    def proj_of(ammo_def, set_name):
        am = ammo_by.get(ammo_def)
        if not am:
            return None
        pr = sets[set_name]["ammoTypes"].get(ammo_def)
        for p in am["projectiles"]:
            if p and p["defName"] == pr:
                return p.get("props") or {}
        return None

    cal_rows = []
    for sn, s in sorted(sets.items()):
        types = s["ammoTypes"]
        fmj = next((k for k in types if k.endswith("_FMJ")), None) or next(iter(types), None)
        p = proj_of(fmj, sn) if fmj else None
        p = p or {}
        wd = [r for r in rows if r["ammoSet"] == sn and not r["unique"] and r["projectile"]]
        if wd:                                   # the round the weapon is defined to fire by default
            dp = next((x for x in ranged if x["defName"] == wd[0]["defName"]), None)
            p = ((dp or {}).get("defaultProjectile") or {}).get("props") or p
        cal_rows.append(dict(ammoSet=sn, label=s["label"], similarTo=s["similarTo"], source=s["source"].split(".")[-1], nAmmo=len(types), baseAmmo=fmj,
                             damage=p.get("damageAmountBase"), damageDef=p.get("damageDef"), apSharp=p.get("armorPenetrationSharp"), apBlunt=p.get("armorPenetrationBlunt"),
                             speed=p.get("speed"), pellets=p.get("pelletCount"), spreadMult=p.get("spreadMult"), nWeapons=len(users.get(sn, [])), weapons=";".join(sorted(users.get(sn, [])))))
    with open(data / "ammo_calibers.csv", "w", newline="") as f:
        wr = csv.DictWriter(f, fieldnames=list(cal_rows[0]))
        wr.writeheader()
        wr.writerows(cal_rows)
    used = [c for c in cal_rows if c["nWeapons"] and c["damage"] is not None]
    used.sort(key=lambda c: (c["damage"] or 0))
    T["ammo_calibers_used"] = md(["ammoSet", "ammo types", "default round: damage", "AP sharp", "AP blunt", "speed", "pellets", "weapons"],
                                 [[c["ammoSet"].replace("AmmoSet_", ""), c["nAmmo"], "%s %s" % (fmt(c["damage"], 1), c["damageDef"] if c["damageDef"] != "Bullet" else ""), fmt(c["apSharp"], 1), fmt(c["apBlunt"], 1),
                                   fmt(c["speed"], 0), fmt(c["pellets"], 0), c["nWeapons"]] for c in used])
    sc = Counter(c["source"] for c in cal_rows)
    ac = Counter(a_["source"].split(".")[-1] for a_ in ammo["ammo"])
    cls_counter = Counter(a_["ammoClass"] for a_ in ammo["ammo"])
    T["ammo_counts"] = md(["measure", "value"], [["AmmoDef count", len(ammo["ammo"])], ["AmmoSetDef count", len(sets)], ["ammo sets by source", dict(sc)], ["AmmoDef by source", dict(ac)],
                                                   ["distinct ammoClass values", len(cls_counter)], ["ammo sets used by a vanilla-or-CE personal weapon in this load", len(users)],
                                                   ["ammo types per set (median, max)", "%s, %s" % (med([c["nAmmo"] for c in cal_rows]), max(c["nAmmo"] for c in cal_rows))]])
    # ratios vs base round inside the same set, by ammoClass
    ratios = defaultdict(lambda: defaultdict(list))
    for sn, s in sets.items():
        types = s["ammoTypes"]
        fmj = next((k for k in types if k.endswith("_FMJ")), None)
        if not fmj:
            continue
        b = proj_of(fmj, sn)
        if not b or not b.get("damageAmountBase"):
            continue
        for am, pr in types.items():
            if am == fmj or am not in ammo_by:
                continue
            p = proj_of(am, sn)
            if not p or p.get("damageDef") != "Bullet" or b.get("damageDef") != "Bullet":
                continue
            ac_ = ammo_by[am]["ammoClass"]
            ratios[ac_]["dmg"].append(p.get("damageAmountBase", 0) / b["damageAmountBase"])
            if b.get("armorPenetrationSharp"):
                ratios[ac_]["sharp"].append(p.get("armorPenetrationSharp", 0) / b["armorPenetrationSharp"])
            if b.get("armorPenetrationBlunt"):
                ratios[ac_]["blunt"].append(p.get("armorPenetrationBlunt", 0) / b["armorPenetrationBlunt"])
            if b.get("speed") and p.get("speed"):
                ratios[ac_]["speed"].append(p["speed"] / b["speed"])
    t = []
    for k, d in sorted(ratios.items(), key=lambda kv: -len(kv[1]["dmg"])):
        if len(d["dmg"]) >= 3 and k != "FullMetalJacket":
            t.append([k, len(d["dmg"]), fmt(med(d["dmg"])), fmt(med(d["sharp"])), fmt(med(d["blunt"])), fmt(med(d["speed"]))])
    T["ammo_class_ratios"] = md(["ammoClass (bullet damage only)", "n sets", "damage / FMJ", "AP sharp / FMJ", "AP blunt / FMJ", "speed / FMJ"], t)

    # ------------------------------------------------------------ apparel and stuff
    apr = []
    for x in app["apparel"]:
        s = x["stats"]
        grp = set(x["bodyPartGroups"])
        lay = set(x["layers"])
        if grp & {"FullHead", "UpperHead"} or lay & {"Overhead"}:
            kind = "headgear"
        elif "Torso" in grp and "Legs" in grp:
            kind = "full body"
        elif "Torso" in grp:
            kind = "torso"
        elif "Legs" in grp:
            kind = "legs"
        else:
            kind = "other"
        apr.append(dict(defName=x["defName"], label=x["label"], source=x["source"].split(".")[-1], tech=x["techLevel"], kind=kind, layers="/".join(x["layers"]),
                        groups="/".join(x["bodyPartGroups"]), stuffed=bool(x["stuffCategories"]), stuffCats="/".join(x["stuffCategories"]), thickness=s.get("StuffEffectMultiplierArmor"),
                        sharp=s.get("ArmorRating_Sharp"), blunt=s.get("ArmorRating_Blunt"), heat=s.get("ArmorRating_Heat"), Mass=s.get("Mass"), Bulk=s.get("Bulk"), WornBulk=s.get("WornBulk"),
                        insCold=s.get("Insulation_Cold"), partial=len(x["partialArmor"]),
                        van_sharp=(x.get("vanilla") or {}).get("stats", {}).get("ArmorRating_Sharp"), van_thickness=(x.get("vanilla") or {}).get("stats", {}).get("StuffEffectMultiplierArmor"),
                        van_mass=(x.get("vanilla") or {}).get("stats", {}).get("Mass")))
    with open(data / "apparel.csv", "w", newline="") as f:
        wr = csv.DictWriter(f, fieldnames=list(apr[0]))
        wr.writeheader()
        wr.writerows(apr)
    stuffs = {s["defName"]: s for s in app["stuffs"]}
    mats = ["Steel", "Plasteel", "Cloth", "Synthread", "Hyperweave", "DevilstrandCloth", "Leather_Plain", "Leather_Heavy", "Leather_Thrumbo", "Leather_Bear", "WoodLog", "Gold", "Silver", "Jade", "Uranium", "Bioferrite", "Obsidian"]
    t = []
    for m in mats:
        s = stuffs.get(m)
        if not s:
            continue
        c, v = s["stats"], (s.get("vanilla") or {}).get("stats", {})
        t.append([m, "/".join(s["categories"]), "%s / %s / %s" % (fmt(c.get("StuffPower_Armor_Sharp")), fmt(c.get("StuffPower_Armor_Blunt")), fmt(c.get("StuffPower_Armor_Heat"))),
                  "%s / %s / %s" % (fmt(v.get("StuffPower_Armor_Sharp")), fmt(v.get("StuffPower_Armor_Blunt")), fmt(v.get("StuffPower_Armor_Heat"))),
                  "%s / %s" % (fmt(c.get("SharpDamageMultiplier")), fmt(c.get("BluntDamageMultiplier"))), fmt(c.get("Mass")), fmt(c.get("Bulk"), 3)])
    T["stuffs"] = md(["material", "stuff categories", "CE armor power sharp / blunt / heat", "vanilla power", "melee dmg mult sharp / blunt", "Mass per unit", "Bulk per unit"], t)
    t = []
    for k in ["headgear", "torso", "legs", "full body", "other"]:
        for stuffed in (False, True):
            g = [r for r in apr if r["kind"] == k and r["stuffed"] == stuffed]
            if not g:
                continue
            t.append([k, "stuff-based" if stuffed else "fixed rating", len(g), rng([r["Mass"] for r in g]), rng([r["Bulk"] for r in g]), rng([r["WornBulk"] for r in g]),
                      rng([r["thickness"] for r in g]) if stuffed else rng([r["sharp"] for r in g]), "-" if stuffed else rng([r["blunt"] for r in g])])
    T["apparel_types"] = md(["kind", "rating source", "n", "Mass kg", "Bulk", "WornBulk", "thickness mm (stuffed) or sharp mm RHA (fixed)", "blunt MPa (fixed)"], t)
    # a few concrete pieces: computed rating in steel/plasteel/synthread
    def computed(x, m):
        s = stuffs[m]["stats"]
        th = x["stats"].get("StuffEffectMultiplierArmor")
        return (th * s["StuffPower_Armor_Sharp"], th * s["StuffPower_Armor_Blunt"]) if th is not None else None
    pieces = [x for x in app["apparel"] if x["stuffCategories"] and x["stats"].get("StuffEffectMultiplierArmor") is not None]
    pieces.sort(key=lambda x: -x["stats"]["StuffEffectMultiplierArmor"])
    t = []
    for x in pieces[:12]:
        c = {m: computed(x, m) for m in ("Steel", "Plasteel", "Synthread", "Leather_Plain") if m in stuffs}
        t.append([x["defName"].replace("Apparel_", ""), "/".join(x["stuffCategories"]), fmt(x["stats"]["StuffEffectMultiplierArmor"], 1), fmt(x["stats"].get("Mass")), fmt(x["stats"].get("Bulk")), fmt(x["stats"].get("WornBulk"))] +
                 ["%s / %s" % (fmt(c[m][0]), fmt(c[m][1])) if m in c else "-" for m in ("Steel", "Plasteel", "Synthread", "Leather_Plain")])
    T["apparel_computed"] = md(["apparel", "stuff categories", "thickness mm", "Mass", "Bulk", "WornBulk", "steel sharp/blunt", "plasteel", "synthread", "plainleather"], t)
    t = []
    for p in presets["apparelPresets"]:
        t.append([p["defName"], fmt(p.get("Bulk")), fmt(p.get("BulkWorn")), fmt(p.get("Mass")), p.get("vanillaArmorRatingRange", "-"),
                  json.dumps((p.get("ArmorCurveSharp") or {}).get("points", p.get("ArmorStaticSharp", "-"))).replace('"', ""), json.dumps((p.get("ArmorCurveBlunt") or {}).get("points", p.get("ArmorStaticBlunt", "-"))).replace('"', ""),
                  "/".join(p.get("neededLayers", []) if isinstance(p.get("neededLayers"), list) else [])])
    T["apparel_presets"] = md(["preset", "Bulk", "WornBulk", "Mass", "vanilla rating range matched", "sharp (vanilla, mm) points", "blunt points", "needed layers"], t)
    t = []
    for g in presets["gunPresets"]:
        sp = g.get("specialGuns")
        t.append([g["defName"], fmt(g.get("Mass")), fmt(g.get("Bulk")), fmt(g.get("Spread")), fmt(g.get("Sway")), fmt((g.get("MiscOtherStats") or {}).get("SightsEfficiency")), fmt(g.get("AmmoCapacity"), 0), fmt(g.get("ReloadTime")),
                  g.get("RangeRange", "-"), g.get("WarmupRange", "-"), g.get("damageRange", "-"), g.get("projSpeedRange", "-"), len(g.get("CaliberRanges", {}).get("_li", []) if isinstance(g.get("CaliberRanges"), dict) else (g.get("CaliberRanges") or [])),
                  len(sp.get("_li", []) if isinstance(sp, dict) else (sp or []))])
    T["gun_presets"] = md(["preset", "Mass", "Bulk", "Spread", "Sway", "Sights", "mag", "reload", "matches vanilla range", "vanilla warmup", "vanilla dmg", "vanilla proj speed", "caliber rules", "special guns"], t)

    # ------------------------------------------------------------ melee
    mrows = []
    for w in melee:
        if w["hasRangedVerb"] or not w["ceTools"]:
            continue
        s = w["stats"]
        eo = w.get("equippedStatOffsets") if isinstance(w.get("equippedStatOffsets"), dict) else {}
        tools = [x for x in w["tools"] if isinstance(x, dict)]
        vt = [x for x in ((w.get("vanilla") or {}).get("tools") or []) if isinstance(x, dict)]
        for i, tl in enumerate(tools):
            caps = tl.get("capacities") or []
            vtl = vt[i] if i < len(vt) else {}
            mrows.append(dict(defName=w["defName"], label=w["label"], source=w["source"].split(".")[-1], tech=w["techLevel"], tool=tl.get("label"), cap="/".join(caps if isinstance(caps, list) else [caps]),
                              power=tl.get("power"), cooldown=tl.get("cooldownTime"), apSharp=tl.get("armorPenetrationSharp"), apBlunt=tl.get("armorPenetrationBlunt"), chance=tl.get("chanceFactor"),
                              Mass=s.get("Mass"), Bulk=s.get("Bulk"), critC=eo.get("MeleeCritChance"), parryC=eo.get("MeleeParryChance"), dodgeC=eo.get("MeleeDodgeChance"), toughness=s.get("ToughnessRating"),
                              counterParry=s.get("MeleeCounterParryBonus"), van_power=vtl.get("power"), van_cooldown=vtl.get("cooldownTime")))
    with open(data / "melee.csv", "w", newline="") as f:
        wr = csv.DictWriter(f, fieldnames=list(mrows[0]))
        wr.writeheader()
        wr.writerows(mrows)
    byw = defaultdict(list)
    for r in mrows:
        byw[r["defName"]].append(r)
    t = []
    for dn, g in sorted(byw.items(), key=lambda kv: -(max((r["power"] or 0) for r in kv[1]))):
        top = max(g, key=lambda r: r["power"] or 0)
        if dn.startswith("MeleeWeapon_") and g[0]["source"] in ("RimWorld", "CombatEx"):
            t.append([dn.replace("MeleeWeapon_", ""), top["tool"], top["cap"], fmt(top["power"]), fmt(top["van_power"]), fmt(top["cooldown"]), fmt(top["van_cooldown"]), fmt(top["apSharp"]), fmt(top["apBlunt"]),
                      fmt(g[0]["Mass"]), fmt(g[0]["Bulk"]), fmt(g[0]["critC"]), fmt(g[0]["parryC"]), fmt(g[0]["dodgeC"])])
    T["melee_weapons"] = md(["weapon", "best tool", "capacity", "power (CE)", "power (vanilla)", "cooldown s (CE)", "cooldown (vanilla)", "AP sharp mm", "AP blunt MPa", "Mass", "Bulk", "crit", "parry", "dodge"], t)
    sharp = [r for r in mrows if r["apSharp"] and r["power"] and r["cap"] in ("Cut", "Stab") and r["source"] in ("RimWorld", "CombatEx")]
    blunt = [r for r in mrows if r["apBlunt"] and r["power"] and r["cap"] == "Blunt" and r["source"] in ("RimWorld", "CombatEx")]
    T["melee_ratios"] = md(["tool group", "n", "AP sharp per power", "AP blunt per power", "power", "cooldown s"],
                           [["Cut/Stab with AP sharp", len(sharp), rng([r["apSharp"] / r["power"] for r in sharp]), rng([r["apBlunt"] / r["power"] for r in sharp]), rng([r["power"] for r in sharp], 1), rng([r["cooldown"] for r in sharp])],
                            ["Blunt", len(blunt), "-", rng([r["apBlunt"] / r["power"] for r in blunt]), rng([r["power"] for r in blunt], 1), rng([r["cooldown"] for r in blunt])]])


    # ------------------------------------------------------------ tech tiers
    t = []
    for tl in ["Neolithic", "Medieval", "Industrial", "Spacer"]:
        g = [r for r in base if r["tech"] == tl and r["cls"] in PERSONAL_FIREARMS + ["bow"]]
        h = [r for r in apr if r["tech"] == tl and r["kind"] == "headgear" and not r["stuffed"] and r["sharp"]]
        b = [r for r in apr if r["tech"] == tl and r["kind"] in ("torso", "full body") and not r["stuffed"] and r["sharp"] and r["sharp"] > 1]
        s_ = [r for r in apr if r["tech"] == tl and r["stuffed"] and r["thickness"]]
        t.append([tl, len(g), rng([r["damage"] for r in g], 1), rng([r["apSharp"] for r in g], 1), rng([r["range"] for r in g], 1), rng([r["Mass"] for r in g]),
                  rng([r["sharp"] for r in h], 1) if h else "-", rng([r["sharp"] for r in b], 1) if b else "-", rng([r["thickness"] for r in s_], 1) if s_ else "-"])
    T["tech_tiers"] = md(["tech level", "firearms+bows n", "damage", "AP sharp mm", "range cells", "Mass kg", "helmet sharp mm RHA (fixed)", "torso/body sharp (fixed, >1)", "thickness mm of stuffed apparel"], t)

    # ------------------------------------------------------------ reference hit simulation (our re-statement of the documented sharp/blunt layer rules)
    def layer(kind, armor, pen, dmg):
        """One armor layer for one damage kind; returns (stopped_fully, new_pen, new_dmg)."""
        deflected = kind == "sharp" and armor > pen
        mult = 0.0 if pen == 0 and False else (1.0 if pen == 0 else min(1.0, max(0.0, (pen - armor) / pen)))
        deflected = deflected or mult == 0
        nd, npn = dmg * mult, pen - armor
        if kind == "sharp" and deflected:
            return True, pen, dmg
        return deflected, max(0, npn), max(0, nd)

    def hit(proj, layers):
        """proj: props dict; layers: list of (sharp_mm, blunt_MPa), outermost first. Returns (health damage of the main hit, kind)."""
        pen, dmg, kind = proj.get("armorPenetrationSharp", 0), float(proj.get("damageAmountBase", 0)), "sharp"
        if proj.get("damageDef") != "Bullet":
            return None, "n/a"
        base_dmg, pen0 = dmg, pen
        for sharp, blunt in layers:
            if kind == "sharp":
                stopped, pen_n, dmg_n = layer("sharp", sharp, pen, dmg)
                if stopped:
                    mult_pen = pen / pen0 if pen0 else 1
                    pen = proj.get("armorPenetrationBlunt", 0) * mult_pen
                    dmg = (pen * 10000) ** (1 / 3) / 10 * (dmg / base_dmg if base_dmg else 1)
                    kind = "blunt"
                    stopped, pen, dmg = layer("blunt", blunt, pen, dmg)
                    if stopped or dmg <= 0:
                        return 0.0, "blunt (stopped)"
                else:
                    pen, dmg = pen_n, dmg_n
            else:
                stopped, pen, dmg = layer("blunt", blunt, pen, dmg)
                if stopped or dmg <= 0:
                    return 0.0, "blunt (stopped)"
        return dmg, kind

    def props_of(set_name, suffix):
        s = sets.get(set_name)
        if not s:
            return None
        for am in s["ammoTypes"]:
            if am.endswith(suffix):
                return proj_of(am, set_name)
    rounds = [("45 ACP FMJ", props_of("AmmoSet_45ACP", "_FMJ")), ("5.56 NATO FMJ", props_of("AmmoSet_556x45mmNATO", "_FMJ")), ("5.56 NATO AP", props_of("AmmoSet_556x45mmNATO", "_AP")),
              ("7.62 NATO FMJ", props_of("AmmoSet_762x51mmNATO", "_FMJ")), ("7.62 NATO AP", props_of("AmmoSet_762x51mmNATO", "_AP")), ("14.5x114 FMJ", props_of("AmmoSet_145x114mm", "_FMJ"))]
    rounds = [r for r in rounds if r[1]]
    armors = [("synthread flak jacket (0.8 mm / 0.2 MPa)", [(0.8, 0.2)]), ("steel vest 8 mm / 12 MPa", [(8, 12)]), ("plasteel vest 16 mm / 24 MPa", [(16, 24)]),
              ("plasteel vest over synthread jacket", [(0.8, 0.2), (16, 24)])]
    t = []
    for nm, p in rounds:
        row = [nm, "%s dmg, %s mm, %s MPa" % (fmt(p.get("damageAmountBase"), 1), fmt(p.get("armorPenetrationSharp"), 1), fmt(p.get("armorPenetrationBlunt"), 1))]
        for an, ly in armors:
            d_, k_ = hit(p, ly)
            row.append("%s (%s)" % (fmt(d_, 1), k_.split(" ")[0]) if d_ is not None else "-")
        t.append(row)
    T["hit_matrix"] = md(["round", "damage, AP sharp, AP blunt"] + [a_[0] for a_ in armors], t)

    for k, v in T.items():
        (tdir / (k + ".md")).write_text(v)

    # ------------------------------------------------------------ figures
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    cols = {"pistol": "#1b6ca8", "smg": "#e08a00", "shotgun": "#7a4fa3", "rifle": "#2a9d4b", "sniper": "#c0392b", "machine gun": "#555555", "launcher": "#a0522d", "bow": "#8c8c3c"}
    plt.rcParams.update({"font.size": 9, "axes.spines.top": False, "axes.spines.right": False})
    fig, ax = plt.subplots(1, 2, figsize=(9, 3.8))
    for c, col in cols.items():
        g = [r for r in base if r["cls"] == c and r["Mass"] is not None and r["Bulk"] is not None]
        ax[0].scatter([r["Mass"] for r in g], [r["Bulk"] for r in g], c=col, label=c, s=22)
        g = [r for r in base if r["cls"] == c and r["sway"] is not None and r["spread"] is not None]
        ax[1].scatter([r["sway"] for r in g], [r["spread"] for r in g], c=col, s=22)
    ax[0].set_xscale("log")
    ax[0].set_xlabel("Mass (kg, log)")
    ax[0].set_ylabel("Bulk")
    ax[0].set_title("Mass vs Bulk")
    ax[1].set_xlabel("SwayFactor")
    ax[1].set_ylabel("ShotSpread (deg)")
    ax[1].set_title("Sway vs spread")
    ax[0].legend(frameon=False, fontsize=7, ncol=2)
    fig.tight_layout()
    fig.savefig(fdir / "ranged_mass_bulk_accuracy.png", dpi=130)
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(6.5, 4.2))
    cu = [c for c in cal_rows if c["damage"] and c["apSharp"] is not None and c["damageDef"] == "Bullet" and (c["pellets"] or 1) == 1]
    ax.scatter([c["apSharp"] for c in cu], [c["damage"] for c in cu], s=14, c="#1b6ca8", alpha=0.7)
    ax.set_xscale("symlog", linthresh=1)
    ax.set_yscale("log")
    for c in cu:
        if c["nWeapons"]:
            ax.annotate(c["ammoSet"].replace("AmmoSet_", ""), (c["apSharp"], c["damage"]), fontsize=6, xytext=(3, 2), textcoords="offset points")
    ax.set_xlabel("armorPenetrationSharp of base round (mm RHA, symlog)")
    ax.set_ylabel("damageAmountBase (log)")
    ax.set_title("Single-projectile base rounds per ammo set (labelled: used by a weapon)")
    fig.tight_layout()
    fig.savefig(fdir / "ammo_damage_vs_penetration.png", dpi=130)
    plt.close(fig)

    fig, ax = plt.subplots(1, 2, figsize=(9, 3.8))
    g = [r for r in base if r["cls"] in PERSONAL_FIREARMS and r["van_range"] is not None]
    for c, col in cols.items():
        gg = [r for r in g if r["cls"] == c]
        ax[0].scatter([r["van_range"] for r in gg], [r["range"] for r in gg], c=col, s=22, label=c)
        gg = [r for r in gg if r["van_damage"] and r["damage"]]
        ax[1].scatter([r["van_damage"] for r in gg], [r["damage"] for r in gg], c=col, s=22)
    mx = max(r["van_range"] for r in g)
    ax[0].plot([0, mx], [0, mx], c="#999", lw=0.8)
    ax[0].set_xlabel("vanilla verb range (cells)")
    ax[0].set_ylabel("CE verb range (cells)")
    ax[0].set_title("Range")
    ax[1].plot([0, 30], [0, 30], c="#999", lw=0.8)
    ax[1].set_xlabel("vanilla damage per hit")
    ax[1].set_ylabel("CE damageAmountBase of default round")
    ax[1].set_title("Damage per bullet")
    ax[0].legend(frameon=False, fontsize=7)
    fig.tight_layout()
    fig.savefig(fdir / "ce_vs_vanilla_guns.png", dpi=130)
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(7.5, 3.8))
    ms = [m for m in ["Steel", "Plasteel", "Hyperweave", "DevilstrandCloth", "Synthread", "Leather_Heavy", "Leather_Plain", "Cloth", "WoodLog"] if m in stuffs]
    x = range(len(ms))
    ax.bar([i - 0.2 for i in x], [stuffs[m]["stats"].get("StuffPower_Armor_Sharp", 0) for m in ms], 0.4, label="CE sharp", color="#1b6ca8")
    ax.bar([i + 0.2 for i in x], [(stuffs[m].get("vanilla") or {}).get("stats", {}).get("StuffPower_Armor_Sharp", 0) for m in ms], 0.4, label="vanilla sharp", color="#bbbbbb")
    ax.set_xticks(list(x))
    ax.set_xticklabels(ms, rotation=30, ha="right")
    ax.set_ylabel("StuffPower_Armor_Sharp")
    ax.set_title("Armor power per mm of thickness (CE: mm RHA; vanilla: fraction)")
    ax.legend(frameon=False)
    fig.tight_layout()
    fig.savefig(fdir / "stuff_sharp_power.png", dpi=130)
    plt.close(fig)
    print("tables:", sorted(T), "| rows: ranged", len(rows), "melee tools", len(mrows), "apparel", len(apr), "calibers", len(cal_rows))


if __name__ == "__main__":
    main()
