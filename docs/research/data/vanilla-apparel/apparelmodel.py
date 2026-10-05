"""Stat pipeline and helpers for the resolved apparel and stuff datasets (research prototype).

Implements, in plain Python, what RimWorld 1.6 does for a stat of an apparel Thing:
  decompiled:RimWorld/StatWorker.cs (GetValueUnfinalized, FinalizeValue), RimWorld/StatPart_Stuff.cs,
  RimWorld/StatPart_Quality.cs, RimWorld/StatWorker_MarketValue.cs, Verse/ArmorUtility.cs.
Only formulas (facts about the game) are reimplemented; no game code is copied.
"""
import json, sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
QUALITIES = ["Awful", "Poor", "Normal", "Good", "Excellent", "Masterwork", "Legendary"]
VALUE_PER_WORK = 0.0036          # StatWorker_MarketValue.ValuePerWork
DEFAULT_GUESS_STUFF_COST = 2.0   # price per stuff unit when the item has no stuff chosen


def load(data_dir=HERE):
    a = json.loads((Path(data_dir) / "vanilla-apparel.json").read_text())
    s = json.loads((Path(data_dir) / "vanilla-stuff.json").read_text())
    return a["apparel"], s


class Model:
    def __init__(self, apparel, stuffdoc):
        self.apparel = {a["defName"]: a for a in apparel}
        self.stuff = {s["defName"]: s for s in stuffdoc["stuff"]}
        self.stats = stuffdoc["stats"]
        self.parts = stuffdoc["humanBodyParts"]
        self.extra_values = stuffdoc.get("thingValues", {})

    # ---- body coverage -------------------------------------------------------------------
    def coverage(self, groups, outside_only=False):
        g = set(groups)
        return sum(p["coverageAbs"] for p in self.parts if g & set(p["groups"]) and (not outside_only or p["depth"] == "Outside"))

    # ---- stat pipeline -----------------------------------------------------------------------
    def _quality_part(self, part, v, q):
        if v <= 0 and part.get("applyToNegativeValues") != "true":
            return v
        f = part.get("factor" + q, 1.0)
        gain = min(v * f - v, part.get("maxGain" + q, 9999999.0))
        return v + gain

    def stat(self, item, stat, stuff=None, q="Normal", calc_value=True):
        a = self.apparel[item] if isinstance(item, str) else item
        st = self.stats[stat]
        sd = self.stuff[stuff] if stuff else None
        if stat == "MarketValue" and "MarketValue" not in a["statBases"] and calc_value:
            v = self.calculated_market_value(a, sd)
        else:
            v = a["statBases"].get(stat, st["defaultBaseValue"])
            if sd is not None:
                if v > 0 or st["applyFactorsIfNegative"] == "true":
                    v *= sd["statFactors"].get(stat, 1.0)
                v += sd["statOffsets"].get(stat, 0.0)
        for part in sorted(st["parts"], key=lambda p: -p["priority"]):       # stable, priority descending
            c = part["class"]
            if c == "StatPart_Stuff":
                power = sd["statBases"].get(part["stuffPowerStat"], 0.0) if sd else 0.0
                v += a["statBases"].get(part["multiplierStat"], 0.0) * power
            elif c == "StatPart_Quality":
                v = self._quality_part(part, v, q)
            # StatPart_Health is 1.0 at full hit points; the other parts do not apply to a fresh item
        if abs(v) > st["roundToFiveOver"]:
            v = round(v / 5) * 5
        if st["roundValue"] == "true":
            v = int(v + 0.5)
        return max(st["minValue"], min(st["maxValue"], v))

    def calculated_market_value(self, a, sd):
        """StatWorker_MarketValue.CalculatedBaseMarketValue for an item without its own MarketValue."""
        work = a["statBases"].get("WorkToMake", 0.0)
        if sd is not None:
            work *= sd["statFactors"].get("WorkToMake", 1.0)
        total = 0.0
        for k, n in a["costList"].items():
            total += n * self.thing_base_value(k)
        if a["costStuffCount"] > 0:
            if sd is None:
                total += a["costStuffCount"] * DEFAULT_GUESS_STUFF_COST
            else:
                vol = 0.1 if sd["smallVolume"] == "true" else 1.0
                total += a["costStuffCount"] / vol * sd["statBases"].get("MarketValue", 0.0)
        if work > 2:
            total += work * VALUE_PER_WORK
        return total

    def thing_base_value(self, name):
        if name in self.stuff:
            return self.stuff[name]["statBases"].get("MarketValue", 0.0)
        return self.extra_values.get(name, 0.0)

    extra_values = {}

    def stuff_count(self, a, sd):
        if a["costStuffCount"] <= 0 or sd is None:
            return 0
        vol = 0.1 if sd["smallVolume"] == "true" else 1.0
        return int(a["costStuffCount"] / vol + 0.5)

    def can_make(self, a, sd):
        return bool(set(a["stuffCategories"]) & set(sd["stuffCategories_of_stuff"]))


# ---- combat math (Verse.ArmorUtility.ApplyArmor) -------------------------------------------------
def layer_outcomes(rating, ap=0.0):
    """P(zero), P(half) for one layer: effective rating e = max(rating - ap, 0); a uniform draw below e/2 deflects,
    below e halves (and turns sharp into blunt)."""
    e = max(rating - ap, 0.0)
    p0 = min(e / 2, 1.0)
    ph = max(min(e, 1.0) - p0, 0.0)
    return p0, ph


def layer_mean_factor(rating, ap=0.0):
    p0, ph = layer_outcomes(rating, ap)
    return 1.0 - p0 - ph / 2


def stack_mean_factor(ratings, ap=0.0):
    f = 1.0
    for r in ratings:
        f *= layer_mean_factor(r, ap)
    return f
