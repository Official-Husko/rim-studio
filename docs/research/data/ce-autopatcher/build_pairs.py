#!/usr/bin/env python3
"""Build the paired dataset: every vanilla (Core + DLC) weapon and apparel next to the value CE gives it.

Usage: build_pairs.py OUT_DIR      (env: RIMWORLD_DIR, CE_DIR, ENGINE_DIR, TYPES_JSON, see ce_load.py)
Writes OUT_DIR/pairs_guns.json, pairs_apparel.json, pairs_melee.json
Vanilla side = resolved defs of a vanilla-only load. CE side = resolved defs of a vanilla + CE load (CE's gun
operation re-implemented in ce_load.py). Research artefacts: CE values stay out of the product (see README).
"""
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import ce_load as L


def f(x, default=None):
    try:
        return float(x)
    except (TypeError, ValueError):
        return default


def stat(d, k):
    return f((d.get("statBases") or {}).get(k))


def first_verb(d):
    v = d.get("verbs")
    return v[0] if isinstance(v, list) and v and isinstance(v[0], dict) else None


def is_gun_like(d):
    v = first_verb(d)
    return bool(d.get("equipmentType")) and v is not None and v.get("defaultProjectile") is not None


def tools_of(d):
    out = []
    for t in d.get("tools") or []:
        if isinstance(t, dict):
            out.append(t)
    return out


def main():
    out = Path(sys.argv[1])
    van = L.load(False)
    ce = L.load(True)
    vt = L.db_dicts(van, "ThingDef")
    ct = L.db_dicts(ce, "ThingDef")
    sets = L.db_dicts(ce, "CombatExtended.AmmoSetDef")

    def ce_ammo(setname):
        s = sets.get(setname)
        if not s:
            return None
        types = s["d"].get("ammoTypes") or {}
        items = list(types.items())
        if not items:
            return None
        ammo, proj = items[0]
        p = (ct.get(proj) or {}).get("d", {}).get("projectile", {})
        return {"first_ammo": ammo, "first_projectile": proj, "damage": f(p.get("damageAmountBase")),
                "ap_sharp": f(p.get("armorPenetrationSharp")), "ap_blunt": f(p.get("armorPenetrationBlunt")),
                "speed": f(p.get("speed")), "pellets": f(p.get("pelletCount")), "similarTo": s["d"].get("similarTo")}

    guns = []
    for name, rec in vt.items():
        d = rec["d"]
        if not is_gun_like(d):
            continue
        v = first_verb(d)
        proj = (vt.get(v["defaultProjectile"]) or {}).get("d", {})
        pp = proj.get("projectile") or {}
        g = {"defName": name, "mod": rec["mod"], "label": d.get("label"), "techLevel": d.get("techLevel"),
             "thingClass": d.get("thingClass"), "weaponTags": d.get("weaponTags") or [],
             "verbClass": v.get("verbClass"), "nVerbs": len(d["verbs"]),
             "projectile": v["defaultProjectile"], "projectileClass": proj.get("thingClass"),
             "v": {"mass": stat(d, "Mass"), "cooldown": stat(d, "RangedWeapon_Cooldown"),
                   "accTouch": stat(d, "AccuracyTouch"), "accShort": stat(d, "AccuracyShort"),
                   "accMedium": stat(d, "AccuracyMedium"), "accLong": stat(d, "AccuracyLong"),
                   "warmup": f(v.get("warmupTime")), "range": f(v.get("range")),
                   "burst": f(v.get("burstShotCount"), 1.0), "ticksBetween": f(v.get("ticksBetweenBurstShots")),
                   "damage": f(pp.get("damageAmountBase")), "speed": f(pp.get("speed")),
                   "stopping": f(pp.get("stoppingPower")), "marketValue": stat(d, "MarketValue"),
                   "damageDef": pp.get("damageDef"), "explosionRadius": f(pp.get("explosionRadius"))}}
        c = (ct.get(name) or {}).get("d")
        g["ce"] = None
        if c:
            cv = first_verb(c)
            comps = c.get("comps") or []
            au = next((x for x in comps if isinstance(x, dict) and x.get("@Class", "").endswith("CompProperties_AmmoUser")), None)
            fm = next((x for x in comps if isinstance(x, dict) and x.get("@Class", "").endswith("CompProperties_FireModes")), None)
            if cv and cv.get("@Class", "").endswith("VerbPropertiesCE"):
                aset = au.get("ammoSet") if au else None
                g["ce"] = {"mass": stat(c, "Mass"), "bulk": stat(c, "Bulk"), "spread": stat(c, "ShotSpread"),
                           "sway": stat(c, "SwayFactor"), "sights": stat(c, "SightsEfficiency"),
                           "cooldown": stat(c, "RangedWeapon_Cooldown"), "recoil": f(cv.get("recoilAmount")),
                           "warmup": f(cv.get("warmupTime")), "range": f(cv.get("range")),
                           "burst": f(cv.get("burstShotCount"), 1.0), "ticksBetween": f(cv.get("ticksBetweenBurstShots")),
                           "magazine": f(au.get("magazineSize")) if au else None,
                           "reload": f(au.get("reloadTime")) if au else None,
                           "reloadOneAtATime": (au.get("reloadOneAtATime") == "true") if au else None,
                           "ammoSet": aset, "ammo": ce_ammo(aset) if aset else None,
                           "aimMode": fm.get("aiAimMode") if fm else None,
                           "ceTags": [t for t in (c.get("weaponTags") or []) if t.startswith("CE_")],
                           "defaultProjectile": cv.get("defaultProjectile")}
        guns.append(g)

    apparel = []
    for name, rec in vt.items():
        d = rec["d"]
        a = d.get("apparel")
        if not isinstance(a, dict):
            continue
        row = {"defName": name, "mod": rec["mod"], "label": d.get("label"), "techLevel": d.get("techLevel"),
               "layers": a.get("layers") or [], "groups": a.get("bodyPartGroups") or [],
               "stuffed": bool(d.get("stuffCategories")),
               "v": {"mass": stat(d, "Mass"), "sharp": stat(d, "ArmorRating_Sharp"), "blunt": stat(d, "ArmorRating_Blunt"),
                     "sema": stat(d, "StuffEffectMultiplierArmor"), "heat": stat(d, "ArmorRating_Heat"),
                     "marketValue": stat(d, "MarketValue")}, "ce": None}
        c = (ct.get(name) or {}).get("d")
        if c:
            ca = c.get("apparel") or {}
            ext = [x for x in (c.get("modExtensions") or []) if isinstance(x, dict) and "PartialArmorExt" in x.get("@Class", "")]
            row["ce"] = {"mass": stat(c, "Mass"), "bulk": stat(c, "Bulk"), "wornBulk": stat(c, "WornBulk"),
                         "sharp": stat(c, "ArmorRating_Sharp"), "blunt": stat(c, "ArmorRating_Blunt"),
                         "sema": stat(c, "StuffEffectMultiplierArmor"), "stuffed": bool(c.get("stuffCategories")),
                         "partial": bool(ext), "layers": ca.get("layers") or [], "groups": ca.get("bodyPartGroups") or []}
        apparel.append(row)

    melee = []
    for name, rec in vt.items():
        d = rec["d"]
        if is_gun_like(d) or not d.get("equipmentType") and not d.get("weaponTags"):
            continue
        ts = tools_of(d)
        if not ts or not d.get("equipmentType"):
            continue
        c = (ct.get(name) or {}).get("d") or {}
        row = {"defName": name, "mod": rec["mod"], "label": d.get("label"), "techLevel": d.get("techLevel"),
               "stuffed": bool(d.get("stuffCategories")), "ranged": False,
               "vTools": [{"label": t.get("label"), "power": f(t.get("power")), "cooldown": f(t.get("cooldownTime")),
                           "ap": f(t.get("armorPenetration")), "chance": f(t.get("chanceFactor")),
                           "capacities": t.get("capacities") or []} for t in ts],
               "vMass": stat(d, "Mass"),
               "ceTools": [{"label": t.get("label"), "power": f(t.get("power")), "cooldown": f(t.get("cooldownTime")),
                            "apSharp": f(t.get("armorPenetrationSharp")), "apBlunt": f(t.get("armorPenetrationBlunt")),
                            "chance": f(t.get("chanceFactor")), "class": t.get("@Class")} for t in tools_of(c)],
               "ce": {"mass": stat(c, "Mass"), "bulk": stat(c, "Bulk"), "toughnessRating": stat(c, "ToughnessRating"),
                      "stuffToughness": stat(c, "StuffEffectMultiplierToughness")} if c else None,
               "ceStuffed": bool(c.get("stuffCategories")) if c else None}
        melee.append(row)

    out.mkdir(parents=True, exist_ok=True)
    for nm, obj in (("pairs_guns", guns), ("pairs_apparel", apparel), ("pairs_melee", melee)):
        (out / (nm + ".json")).write_text(json.dumps(obj, indent=1, sort_keys=True) + "\n")
    print("guns", len(guns), "with CE verb", sum(1 for g in guns if g["ce"]), "| apparel", len(apparel),
          "with CE bulk", sum(1 for a in apparel if a["ce"] and a["ce"]["bulk"] is not None), "| melee", len(melee))


if __name__ == "__main__":
    main()
