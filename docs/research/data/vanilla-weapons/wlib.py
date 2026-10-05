"""Shared helpers for the vanilla weapon analysis: XML node to dict conversion and the combat formulas.

The formulas follow the behaviour of the game code verified in docs/research/vanilla-ranged-weapons-analysis.md
(Verse.VerbProperties, ProjectileProperties, ArmorUtility, StatWorker_MeleeAverageDPS, StatWorker_MarketValue).
Nothing here is copied from game code; the rules are re-expressed from observed behaviour.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional

TICKS_PER_SECOND = 60.0
VALUE_PER_WORK = 0.0036          # silver per work unit in the computed market value (StatWorker_MarketValue)
DEFAULT_STUFF_GUESS = 2.0        # silver per stuff unit when the stuff is unknown
AP_PER_DAMAGE = 0.015            # default armor penetration is damage * 0.015 when nothing sets it

QUALITY = ["Awful", "Poor", "Normal", "Good", "Excellent", "Masterwork", "Legendary"]


def num(text: Optional[str], default: float = 0.0) -> float:
    if text is None:
        return default
    try:
        return float(text.strip())
    except ValueError:
        return default


def to_py(el) -> Any:
    """Generic XML node to JSON value: leaf text, list for <li> children, dict otherwise."""
    kids = [k for k in el if isinstance(k.tag, str)]
    if not kids:
        t = (el.text or "").strip()
        return t
    if all(k.tag == "li" for k in kids):
        return [to_py(k) for k in kids]
    out: Dict[str, Any] = {}
    for k in kids:
        out[k.tag] = to_py(k)
    return out


def stat_dict(node) -> Dict[str, float]:
    sb = node.find("statBases")
    out: Dict[str, float] = {}
    if sb is not None:
        for c in sb:
            if isinstance(c.tag, str):
                out[c.tag] = num(c.text)
    return out


def pairs(node, path: str) -> Dict[str, float]:
    el = node.find(path)
    out: Dict[str, float] = {}
    if el is not None:
        for c in el:
            if isinstance(c.tag, str):
                out[c.tag] = num(c.text)
    return out


# ---------------------------------------------------------------------------------------------- ranged formulas

def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * min(max(t, 0.0), 1.0)


def weapon_accuracy(acc: Dict[str, float], dist: float) -> float:
    """Equipment hit factor at a distance in tiles: piecewise linear through the four accuracy stats
    (touch up to 3, short at 12, medium at 25, long at 40 and beyond), clamped to [0.01, 1]."""
    t, s, m, l = acc["touch"], acc["short"], acc["medium"], acc["long"]
    if dist <= 3:
        v = t
    elif dist <= 12:
        v = lerp(t, s, (dist - 3) / 9)
    elif dist <= 25:
        v = lerp(s, m, (dist - 12) / 13)
    elif dist <= 40:
        v = lerp(m, l, (dist - 25) / 15)
    else:
        v = l
    return min(max(v, 0.01), 1.0)


def armor_multiplier(armor: float, ap: float) -> float:
    """Expected fraction of damage that survives one armor layer: with e = max(armor - ap, 0) the hit is
    nullified with probability e/2, halved with probability e/2 and untouched otherwise."""
    e = max(armor - ap, 0.0)
    return 1.0 - 0.75 * e


def ranged_cycle_seconds(warmup: float, cooldown: float, burst: int, ticks_between: float) -> float:
    return warmup + cooldown + (burst - 1) * ticks_between / TICKS_PER_SECOND


def ranged_shot_dps(damage: float, burst: int, cycle: float, hit: float = 1.0) -> float:
    return damage * burst * hit / cycle if cycle > 0 else 0.0


# ---------------------------------------------------------------------------------------------- melee formulas

def melee_tool_entries(tools: List[dict], kind_of: Dict[str, str], stuff_mult: Optional[Dict[str, float]] = None,
                       dmg_mult: float = 1.0, cd_mult: float = 1.0) -> List[dict]:
    """One entry per (tool, capacity), the way the game enumerates verbs from tools.
    kind_of maps a capacity to its damage armor category ("Sharp" or "Blunt") via the maneuver table.
    damage = power * MeleeWeapon_DamageMultiplier * stuff multiplier of that category (only when a stuff is given)
    weight = damage^2 * chanceFactor (maneuver commonality is 1 in vanilla)
    ap     = explicit armorPenetration * MeleeWeapon_DamageMultiplier, else damage * 0.015."""
    out = []
    for t in tools:
        for cap in t.get("capacities") or []:
            kind = kind_of.get(cap, "Blunt")
            sm = stuff_mult.get(kind, 1.0) if stuff_mult else 1.0
            dmg = t["power"] * dmg_mult * sm
            ap_expl = t.get("armorPenetration")
            ap = dmg * AP_PER_DAMAGE if (ap_expl is None or ap_expl < 0) else ap_expl * dmg_mult
            cd = t["cooldownTime"] * cd_mult
            out.append({"tool": t.get("label"), "capacity": cap, "kind": kind, "damage": dmg, "cooldown": cd, "ap": ap,
                        "weight": dmg * dmg * t.get("chanceFactor", 1.0), "chanceFactor": t.get("chanceFactor", 1.0)})
    return out


def melee_averages(entries: List[dict]) -> Dict[str, float]:
    sw = sum(e["weight"] for e in entries)
    if sw <= 0:
        return {"avg_damage": 0.0, "avg_cooldown": 0.0, "dps": 0.0, "avg_ap": 0.0}
    avg_d = sum(e["damage"] * e["weight"] for e in entries) / sw
    avg_c = sum(e["cooldown"] * e["weight"] for e in entries) / sw
    avg_ap = sum(e["ap"] * e["weight"] for e in entries) / sw
    return {"avg_damage": avg_d, "avg_cooldown": avg_c, "dps": avg_d / avg_c if avg_c else 0.0, "avg_ap": avg_ap}


def melee_selection_dps(entries: List[dict]) -> Dict[str, float]:
    """In-fight verb choice (VerbUtility.GetSelectionCategory and FinalSelectionWeight, re-expressed):
    initial weight = damage * (1 + ap) / cooldown * chanceFactor; Best = weight >= 0.95 * max, Worst = weight < 0.25 * max
    (never used), Mid otherwise.  Best entries share 0.75 of the choices, Mid entries share 0.25.
    Returns the expected damage per swing, the expected cooldown, their ratio and the expected AP."""
    if not entries:
        return {"sel_damage": 0.0, "sel_cooldown": 0.0, "dps_select": 0.0, "sel_ap": 0.0}
    w0 = [e["damage"] * (1 + e["ap"]) / e["cooldown"] * e["chanceFactor"] for e in entries]
    hi = max(w0)
    cat = ["best" if w >= 0.95 * hi else ("worst" if w < 0.25 * hi else "mid") for w in w0]
    nb, nm = cat.count("best"), cat.count("mid")
    fw = [0.0 if c == "worst" else (0.75 / nb if c == "best" else 0.25 / nm) for c in cat]
    tw = sum(fw)
    d = sum(e["damage"] * f for e, f in zip(entries, fw)) / tw
    c = sum(e["cooldown"] * f for e, f in zip(entries, fw)) / tw
    a = sum(e["ap"] * f for e, f in zip(entries, fw)) / tw
    return {"sel_damage": d, "sel_cooldown": c, "dps_select": d / c if c else 0.0, "sel_ap": a}
