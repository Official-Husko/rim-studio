#!/usr/bin/env python3
"""Reference implementation (Python) of Combat Extended's auto-patcher formulas, written from a reading of the
algorithm, not copied. Pure functions over plain dicts; the preset tables are loaded from JSON that
extract_presets.py reads from the user's CE folder at run time.

Conventions: a "gun" is a dict with label, weaponTags, v{mass,cooldown,warmup,range,damage,speed}.
Curves are lists of [x, y] points; the game sorts points by x on load.
"""
import math


# ---------------------------------------------------------------- SimpleCurve
def curve_eval(points, x):
    """Piecewise-linear curve, clamped to the first and last y outside the x range. One point = constant."""
    pts = sorted(points, key=lambda p: p[0])
    if not pts:
        return 0.0
    if x <= pts[0][0]:
        return pts[0][1]
    if x >= pts[-1][0]:
        return pts[-1][1]
    for i in range(1, len(pts)):
        if x <= pts[i][0]:
            (x0, y0), (x1, y1) = pts[i - 1], pts[i]
            return y0 + (y1 - y0) * ((x - x0) / (x1 - x0))
    return pts[-1][1]


def inc(rng, x):
    """FloatRange.Includes: inclusive on both ends."""
    return rng[0] <= x <= rng[1]


# ---------------------------------------------------------------- guns
def label_tokens(label):
    return label.lower().replace("-", "").split(" ")


def discard_designations_tokens(label):
    # each capital A becomes a space before lower-casing, so "MP5A2" splits into "mp5" and "2"
    return label.replace("A", " ").lower().replace("-", "").split(" ")


def preset_ranges(p):
    """Damage and speed match ranges widened to cover every caliber range of the preset."""
    dmg, spd = list(p["damageRange"]), list(p["projSpeedRange"])
    for c in p["CaliberRanges"]:
        dmg = [min(dmg[0], c["DamageRange"][0]), max(dmg[1], c["DamageRange"][1])]
        spd = [min(spd[0], c["SpeedRange"][0]), max(spd[1], c["SpeedRange"][1])]
    return dmg, spd


def matches_verb_props(g, p):
    v = g["v"]
    dmg, spd = preset_ranges(p)
    return (inc(p["WarmupRange"], v["warmup"] or 0.0) and inc(p["RangeRange"], v["range"] or 0.0)
            and inc(dmg, v["damage"] or 0.0) and inc(spd, v["speed"] or 0.0))


def match_reason(g, p):
    """Why preset p claims gun g, or None. Test order is the game's order."""
    toks = label_tokens(g["label"])
    if any(t in p["names"] for t in toks):
        return "name token"
    if g["label"].lower() in p["names"]:
        return "name"
    if p["DiscardDesignations"] and any(t in p["names"] for t in discard_designations_tokens(g["label"])):
        return "name (designations discarded)"
    if matches_verb_props(g, p):
        return "verb ranges"
    if set(p["tags"]) & set(g["weaponTags"]):
        return "tag"
    for sg in p["specialGuns"]:
        if g["label"] in sg["names"] or set(sg["names"]) & set(toks):
            return "special gun"
    return None


def fallback_preset(presets):
    """Guns no preset claims go to the preset with the largest sum of range averages (first on ties)."""
    def score(p):
        dmg, spd = preset_ranges(p)
        avg = lambda r: (r[0] + r[1]) / 2
        return avg(dmg) + avg(p["RangeRange"]) + avg(spd) + avg(p["WarmupRange"])
    best = presets[0]
    for p in presets[1:]:
        if score(p) > score(best):
            best = p
    return best


def classify_gun(g, presets):
    """Returns (preset, reason). presets in def load order; the first claimant wins."""
    for p in presets:
        r = match_reason(g, p)
        if r:
            return p, r
    return fallback_preset(presets), "fallback"


def determine_caliber(g, p):
    for c in p["CaliberRanges"]:
        if inc(c["DamageRange"], g["v"]["damage"] or 0.0) and inc(c["SpeedRange"], g["v"]["speed"] or 0.0):
            return c["AmmoSet"]
    return p["setCaliber"]


def patch_gun(g, p):
    """CE stats the auto-patcher would write for gun g given preset p (keys as in the paired dataset's 'ce')."""
    v = g["v"]
    out = {}
    out["range"] = curve_eval(p["rangeCurve"], v["range"]) if p.get("rangeCurve") else float(p["gunStats"].get("range", 0))
    out["warmup"] = curve_eval(p["warmupCurve"], v["warmup"] or 0.0) if p.get("warmupCurve") else float(p["gunStats"].get("warmupTime", 0))
    out["mass"] = curve_eval(p["MassCurve"], v["mass"]) if p.get("MassCurve") and v["mass"] is not None else p["Mass"]
    out["cooldown"] = curve_eval(p["cooldownCurve"], v["cooldown"] or 0.0) if p.get("cooldownCurve") else p.get("CooldownTime", 0.0)
    out["bulk"] = p["Bulk"]
    out["spread"] = p["Spread"]
    out["sway"] = p["Sway"]
    out["sights"] = p["MiscOtherStats"].get("SightsEfficiency")
    out["recoil"] = float(p["gunStats"]["recoilAmount"]) if "recoilAmount" in p["gunStats"] else None
    out["burst"] = float(p["gunStats"].get("burstShotCount", 1))
    out["ticksBetween"] = float(p["gunStats"]["ticksBetweenBurstShots"]) if "ticksBetweenBurstShots" in p["gunStats"] else None
    out["magazine"] = float(p["AmmoCapacity"])
    out["reload"] = float(p["ReloadTime"])
    toks = label_tokens(g["label"])
    special = next((sg for sg in p["specialGuns"] if set(sg["names"]) & set(toks)), None)
    base_set = special["caliber"] if special and special.get("caliber") else p["setCaliber"]
    out["ammoSet"] = base_set
    if p["DetermineCaliber"]:
        for c in p["CaliberRanges"]:
            if inc(c["DamageRange"], v["damage"] or 0.0) and inc(c["SpeedRange"], v["speed"] or 0.0):
                out["ammoSet"] = c["AmmoSet"]
                break
    out["preset"] = p["defName"]
    # a special gun entry (named model) overrides reload, magazine, mass, bulk and may add stats
    for sg in [special] if special else []:
        if True:
            out["magazine"] = sg.get("magCap", out["magazine"])
            out["reload"] = sg.get("reloadTime", out["reload"])
            out["mass"] = sg.get("mass", out["mass"])
            out["bulk"] = sg.get("bulk", out["bulk"])
            for k, val in sg.get("stats", {}).items():
                out.setdefault("extra", {})[k] = val
            break
    return out


# ---------------------------------------------------------------- apparel
def apparel_matches(a, p):
    """a: dict with layers, groups, v{sharp, sema}. Missing stats read as 0 (the game reads statBases only)."""
    if not all(l in p["neededLayers"] for l in a["layers"]):
        return False
    if not all(x in p["neededGroups"] for x in a["groups"]):
        return False
    sharp = a["v"].get("sharp") or 0.0
    sema = a["v"].get("sema") or 0.0
    return inc(p["vanillaArmorRatingRange"], sharp) or inc(p["vanillaArmorRatingRange"], sema)


def classify_apparel(a, presets):
    for p in presets:
        if apparel_matches(a, p):
            return p
    return None


def _final(p, key, static_key, x):
    pts = p.get(key)
    return curve_eval(pts, x) if pts else p.get(static_key, 0.0)


def patch_apparel(a, p):
    sharp_stat = a["v"].get("sharp")
    if sharp_stat is not None:
        sharp = _final(p, "ArmorCurveSharp", "ArmorStaticSharp", sharp_stat)
        blunt = _final(p, "ArmorCurveBlunt", "ArmorStaticBlunt", a["v"].get("blunt") or 0.0)
    else:  # no explicit sharp stat: both curves are fed the stuff multiplier
        x = a["v"].get("sema") or 0.0
        sharp = _final(p, "ArmorCurveSharp", "ArmorStaticSharp", x)
        blunt = _final(p, "ArmorCurveBlunt", "ArmorStaticBlunt", x)
    return {"sharp": sharp, "blunt": blunt, "bulk": p["Bulk"], "wornBulk": p["BulkWorn"],
            "mass": p.get("Mass", 0.0), "partial": bool(p["partialStats"]), "preset": p["defName"]}


# ---------------------------------------------------------------- melee tools, toughness, races
def convert_tool(t):
    """Vanilla tool dict (power, cooldown, ap) to CE tool values: AP copied to both kinds, with two defaults."""
    cd = t["cooldown"] if t["cooldown"] and t["cooldown"] > 0 else 2.0
    ap = t["ap"] or 0.0
    if ap <= 0:
        return {"power": t["power"], "cooldown": cd, "apSharp": 0.5, "apBlunt": 2.0}
    return {"power": t["power"], "cooldown": cd, "apSharp": ap, "apBlunt": ap}


TECH_THICKNESS = {"Spacer": 2.0, "Ultra": 4.0, "Archotech": 8.0}
SHARP_CAPACITIES = {"Cut", "Stab", "Scratch", "Bite", "ScratchToxic", "ToxicSting"}


def stuff_toughness_multiplier(bulk, tech, ranged, capacities):
    """StuffEffectMultiplierToughness for a stuffable weapon: sqrt(bulk), tech multiplier, doubled for blunt-only melee."""
    if not bulk:
        return None
    th = math.sqrt(bulk) * TECH_THICKNESS.get(tech, 1.0)
    if not ranged and not (set(capacities) & SHARP_CAPACITIES):
        th *= 2.0
    return th


def toughness_rating_fixed(bulk, tech, ranged, capacities, ingredient_sharp_power, toughness_mult=1.0):
    """ToughnessRating for a non-stuffable weapon: thickness times the strongest main ingredient's sharp power."""
    th = stuff_toughness_multiplier(bulk, tech, ranged, capacities)
    return None if th is None else th * ingredient_sharp_power * toughness_mult


RACE_SHARP_CURVE = [[0.2, 1.0], [2.0, 20.0]]
RACE_BLUNT_CURVE = [[0.2, 2.0], [2.0, 40.0]]


def race_armor(sharp, blunt):
    """Animal and alien race armor conversion; absent ratings get the defaults 0.125 sharp and 1 blunt."""
    s = curve_eval(RACE_SHARP_CURVE, sharp) if sharp is not None else 0.125
    b = curve_eval(RACE_BLUNT_CURVE, blunt) if blunt is not None else 1.0
    return s, b
