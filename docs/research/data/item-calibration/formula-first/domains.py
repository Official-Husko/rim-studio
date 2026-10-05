"""Domain definitions: which resolved items, which axes, which stats, which quiz questions.

Research data only. The product reads the same quantities from the user's install at runtime.
Standard library, numpy and pandas only.
"""
from dataclasses import dataclass, field
from pathlib import Path
import numpy as np
import pandas as pd

DATA = Path(__file__).resolve().parents[2]
TIER = {"Neolithic": 0, "Medieval": 1, "Industrial": 2, "Spacer": 3, "Ultra": 4, "Archotech": 4}
INF = float("inf")


@dataclass
class Question:
    qid: str
    stat: str           # stat constrained by the answer
    edges: list         # inner bin edges (absolute, human readable)
    labels: list        # one label per bin (len(edges) + 1)
    text: str = ""


@dataclass
class Domain:
    name: str
    df: pd.DataFrame
    strength: str                 # column holding the scalar power index
    stats: list                   # scored stats
    questions: list
    hidden: list = field(default_factory=list)  # stats needed by formulas but not scored
    mass: str | None = None       # size axis column
    coverage: str | None = None   # apparel coverage column
    cov_scaled: tuple = ()
    ammo: bool = False            # caliber question available (CE ranged)
    twin: dict = field(default_factory=dict)    # stat -> (twin column, mode) for the conversion arm
    integer: tuple = ()           # stats rounded to integers
    twin_keys: tuple = ()         # stats that identify an identical sibling to drop from training


def _tier(s):
    return s.map(TIER)


def ranged_vanilla():
    d = pd.read_csv(DATA / "vanilla-weapons" / "ranged_direct_fit_set.csv")
    d = d.reset_index(drop=True)
    d["group"] = "all"
    d["gap"] = d.ticks_between.where(d.burst > 1)
    qs = [
        Question("size", "mass", [1.5, 3.0, 6.0], ["pocket or one-handed (up to 1.5 kg)", "handheld (1.5 to 3 kg)", "two-handed (3 to 6 kg)", "heavy (over 6 kg)"]),
        Question("fire", "burst", [1.5, 4.5], ["single shot", "short burst (2 to 4)", "long burst (5 or more)"]),
        Question("reach", "range", [20.0, 27.0, 35.0], ["short (under 20 tiles)", "medium (20 to 27)", "long (27 to 35)", "very long (over 35)"]),
        Question("handling", "warmup", [0.5, 1.2, 2.5], ["instant (under 0.5 s)", "quick (0.5 to 1.2 s)", "deliberate (1.2 to 2.5 s)", "slow (over 2.5 s)"]),
    ]
    return Domain("ranged_vanilla", d, "power",
                  ["damage", "burst", "warmup", "cooldown", "range", "mass", "ap", "dps_nominal", "power", "mv", "work"],
                  qs, hidden=["acc_mid", "gap"], mass="mass", integer=("burst",))


def melee_vanilla():
    d = pd.read_csv(DATA / "vanilla-weapons" / "melee_table.csv").reset_index(drop=True)
    d["role"] = np.where(d.weapon_classes.str.contains("MeleeBlunt"), "blunt", "sharp")
    d["group"] = "all"
    d["strength_idx"] = d.dps_select * (1 + d.sel_ap)
    d["stuff_count"] = d.stuff_count.where(d.stuff_count > 0)
    qs = [
        Question("size", "mass", [0.7, 1.3, 2.5], ["dagger sized (under 0.7 kg)", "one-handed (0.7 to 1.3 kg)", "long (1.3 to 2.5 kg)", "heavy (over 2.5 kg)"]),
        Question("swing", "avg_cooldown", [1.8, 2.3], ["quick swing", "normal swing", "slow swing"]),
    ]
    return Domain("melee_vanilla", d, "strength_idx",
                  ["avg_damage", "avg_cooldown", "dps", "mass", "mv", "work", "stuff_count"], qs, mass="mass")


def apparel_vanilla():
    d = pd.read_csv(DATA / "vanilla-apparel" / "apparel-items.csv")
    d = d[d.has_recipe & ~d.explicit_market_value].reset_index(drop=True)
    d["role"] = d["kind"] + ":" + np.where(d.stuffed, "S", "F")
    d["group"] = np.where(d.stuffed, "S", "F")
    d["tier"] = d["tier_ord"]
    d["mass"] = d["mass"]
    d["stuff_units"] = d.stuff_units.where(d.stuffed)
    d["mv"] = d["market_value"]
    d["sharp"] = d["sharp"].where(d.sharp > 0)
    qs = [Question("size", "mass", [0.3, 1.0, 3.0], ["featherweight (under 0.3 kg)", "light (0.3 to 1 kg)", "medium (1 to 3 kg)", "heavy (over 3 kg)"])]
    return Domain("apparel_vanilla", d, "api", ["work", "mass", "hp", "stuff_units", "sharp", "api", "mv"], qs,
                  mass="mass", coverage="coverage", cov_scaled=("work", "mass", "stuff_units"), twin_keys=("work", "mass", "hp"))


def ce_ranged():
    d = pd.read_csv(DATA / "ce-dataset" / "ranged.csv")
    d = d[(~d.unique) & d.ceConverted & d.cls.isin(["rifle", "pistol", "smg", "shotgun", "sniper", "machine gun", "bow"])]
    d = d.reset_index(drop=True)
    d["role"] = d.cls
    d["group"] = "all"
    d["tier"] = _tier(d.tech)
    d["burst"] = d.burst.fillna(1)
    d["strength_idx"] = d.damage * np.sqrt(1 + d.apSharp.fillna(0))
    qs = [
        Question("size", "Mass", [1.5, 3.5, 7.0, 15.0], ["pocket (under 1.5 kg)", "handheld (1.5 to 3.5 kg)", "rifle sized (3.5 to 7 kg)", "heavy (7 to 15 kg)", "crew weapon (over 15 kg)"]),
        Question("fire", "cooldown", [0.45, 0.8], ["automatic or semi-automatic", "pump or lever action", "bolt or single shot"]),
        Question("reach", "range", [20.0, 40.0, 60.0], ["short (under 20 cells)", "medium (20 to 40)", "long (40 to 60)", "very long (over 60)"]),
        Question("mag", "mag", [8.0, 20.0, 40.0], ["small (up to 8 rounds)", "medium (9 to 20)", "large (21 to 40)", "drum or belt (over 40)"]),
    ]
    return Domain("ce_ranged", d, "strength_idx",
                  ["Mass", "Bulk", "cooldown", "range", "warmup", "mag", "reload", "damage", "apSharp", "spread", "recoil"],
                  qs, mass="Mass", ammo=True, integer=("mag",), twin={"Mass": ("van_mass", "identity"), "range": ("van_range", "ratio")})


def ce_melee():
    m = pd.read_csv(DATA / "ce-dataset" / "melee.csv")
    m = m[m.defName.str.startswith("MeleeWeapon_")]
    d = m.sort_values("power").groupby("defName").tail(1).sort_values("defName").reset_index(drop=True)
    d["role"] = np.where(d.cap.isin(["Cut", "Stab"]), "sharp", "blunt")
    d["group"] = "all"
    d["tier"] = _tier(d.tech)
    d["strength_idx"] = d.power / d.cooldown
    qs = [
        Question("size", "Mass", [1.0, 1.8], ["light (under 1 kg)", "medium (1 to 1.8 kg)", "heavy (over 1.8 kg)"]),
        Question("swing", "cooldown", [1.3, 2.0], ["quick swing", "normal swing", "slow swing"]),
    ]
    return Domain("ce_melee", d, "strength_idx", ["power", "cooldown", "apSharp", "Mass", "Bulk", "parryC", "critC"], qs, mass="Mass")


def ce_apparel():
    d = pd.read_csv(DATA / "ce-dataset" / "apparel.csv")
    d = d[d.Mass.notna()].reset_index(drop=True)
    d["role"] = d.kind + ":" + np.where(d.stuffed, "S", "F")
    d["group"] = np.where(d.stuffed, "S", "F")
    d["tier"] = _tier(d.tech).fillna(2)
    d["armor"] = d.sharp.fillna(d.thickness)
    d["strength_idx"] = d.armor.fillna(0)
    d["armor"] = d.armor.where(d.armor > 0)
    d["WornBulk"] = d.WornBulk.where(d.WornBulk > 0)
    qs = [Question("size", "Mass", [0.5, 2.0, 8.0], ["featherweight (under 0.5 kg)", "light (0.5 to 2 kg)", "medium (2 to 8 kg)", "heavy (over 8 kg)"])]
    return Domain("ce_apparel", d, "strength_idx", ["Mass", "Bulk", "WornBulk", "armor", "blunt"], qs, mass="Mass",
                  twin={"Mass": ("van_mass", "identity")}, twin_keys=("Mass", "Bulk", "armor"))


def ammo_table():
    a = pd.read_csv(DATA / "ce-dataset" / "ammo_calibers.csv")
    return a.drop_duplicates("ammoSet").set_index("ammoSet")[["damage", "apSharp"]]


ALL = {"vanilla": [ranged_vanilla, melee_vanilla, apparel_vanilla], "ce": [ce_ranged, ce_melee, ce_apparel]}
