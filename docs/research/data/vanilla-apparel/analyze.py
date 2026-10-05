#!/usr/bin/env python3
"""Statistics, models, worked examples and the baseline backtest for vanilla apparel.

Usage: analyze.py [--data DIR] [--out DIR] [--figs]
Reads vanilla-apparel.json and vanilla-stuff.json (written by extract.py) and writes CSV/JSON/PNG next to them.
Deterministic (fixed seeds).  Needs numpy, scipy and (for --figs) matplotlib.
"""
import argparse, csv, json, math, sys
from pathlib import Path
import numpy as np

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import apparelmodel as M                                    # noqa: E402

TIER = {"Neolithic": 0, "Medieval": 1, "Industrial": 2, "Spacer": 3, "Ultra": 4, "Archotech": 5, "Undefined": -1}
AP_GRID = (0.0, 0.15, 0.30)          # armor penetration values the sharp index is averaged over
W = {"sharp": 0.6, "blunt": 0.25, "heat": 0.15}   # protection index weights (an assumption, tested in the sensitivity table)


def ref_stuff(a):
    c = a["stuffCategories"]
    if not a["stuffed"]:
        return None
    for cat, s in (("Metallic", "Steel"), ("Fabric", "Cloth"), ("Leathery", "Leather_Plain"), ("Woody", "WoodLog"), ("Biofferous", "Bioferrite")):
        if cat in c:
            return s
    return "Bioferrite"


def kind_of(a):
    L, G = set(a["layers"]), set(a["bodyPartGroups"])
    if L == {"Belt"}:
        return "utility"
    if L & {"Overhead", "EyeCover"}:
        return "helmet"
    if {"Torso", "Legs", "Arms"} <= G:
        return "full body"
    if G and G <= {"Legs"}:
        return "legwear"
    if "Shell" in L:
        return "torso outer"
    if "Middle" in L:
        return "torso vest"
    return "base clothing"


def protection(r_sharp, r_blunt, r_heat):
    """Mean damage prevented (0..1) by one layer set, averaged over the AP grid for sharp (Verse.ArmorUtility semantics)."""
    s = np.mean([1 - M.layer_mean_factor(r_sharp, ap) for ap in AP_GRID])
    return W["sharp"] * s + W["blunt"] * (1 - M.layer_mean_factor(r_blunt)) + W["heat"] * (1 - M.layer_mean_factor(r_heat))


def item_row(m, a, stuff=None, q="Normal"):
    st = lambda n: m.stat(a, n, stuff, q)
    sh, bl, ht = st("ArmorRating_Sharp"), st("ArmorRating_Blunt"), st("ArmorRating_Heat")
    cov = m.coverage(a["bodyPartGroups"])
    P = protection(sh, bl, ht)
    sd = m.stuff.get(stuff) if stuff else None
    return {"defName": a["defName"], "stuff": stuff or "", "quality": q, "kind": kind_of(a), "tier": a["techLevel"], "coverage": round(cov, 4),
            "sharp": round(sh, 4), "blunt": round(bl, 4), "heat": round(ht, 4), "ins_cold": round(st("Insulation_Cold"), 2), "ins_heat": round(st("Insulation_Heat"), 2),
            "hp": st("MaxHitPoints"), "mass": round(st("Mass"), 3), "flammability": round(st("Flammability"), 3), "work": round(st("WorkToMake"), 1),
            "market_value": round(st("MarketValue"), 2), "stuff_units": m.stuff_count(a, sd), "protection": round(P, 4), "api": round(100 * cov * P, 3)}


def ols(X, y, ridge=1e-6):
    X = np.asarray(X, float); y = np.asarray(y, float)
    beta = np.linalg.solve(X.T @ X + ridge * np.eye(X.shape[1]), X.T @ y)
    res = y - X @ beta
    ss = float(((y - y.mean()) ** 2).sum())
    return beta, res, (1 - float((res ** 2).sum()) / ss if ss > 0 else float("nan"))


def write_csv(path, rows):
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)


def main():
    ap_ = argparse.ArgumentParser()
    ap_.add_argument("--data", default=str(HERE))
    ap_.add_argument("--out", default=str(HERE))
    ap_.add_argument("--figs", action="store_true")
    args = ap_.parse_args()
    out = Path(args.out)
    A, S = M.load(args.data)
    m = M.Model(A, S)
    R = {}                                                     # everything that goes to models.json

    # ---- 1. item table at the reference material ----------------------------------------------------
    items = []
    for a in A:
        r = item_row(m, a, ref_stuff(a))
        r.update({"mod": a["mod"].replace("Ludeon.RimWorld", "Core").replace("Core.", ""), "stuffed": a["stuffed"], "cost_stuff_count": a["costStuffCount"],
                  "explicit_market_value": "MarketValue" in a["statBases"], "has_recipe": a["recipeMaker"] is not None or bool(a["recipeDefs"]),
                  "move_speed_offset": a["equippedStatOffsets"].get("MoveSpeed", 0), "layers": "/".join(a["layers"]), "groups": "/".join(a["bodyPartGroups"]),
                  "armor_mult": a["statBases"].get("StuffEffectMultiplierArmor", ""), "ins_cold_mult": a["statBases"].get("StuffEffectMultiplierInsulation_Cold", ""),
                  "ins_heat_mult": a["statBases"].get("StuffEffectMultiplierInsulation_Heat", ""), "tier_ord": TIER.get(a["techLevel"], -1)})
        items.append(r)
    write_csv(out / "apparel-items.csv", items)

    # ---- 2. every item x every legal material, Normal quality ---------------------------------------
    bystuff = []
    mats = [s for s in S["stuff"] if s["stuffCategories_of_stuff"] and s["defName"] in m.stuff and s["statBases"].get("StuffPower_Armor_Sharp") is not None]
    for a in A:
        if not a["stuffed"]:
            continue
        for s in mats:
            if m.can_make(a, s):
                r = item_row(m, a, s["defName"])
                bystuff.append({k: r[k] for k in ("defName", "stuff", "kind", "coverage", "sharp", "blunt", "heat", "ins_cold", "ins_heat", "hp", "mass", "flammability", "work", "market_value", "stuff_units", "api")})
    write_csv(out / "apparel-by-stuff.csv", bystuff)
    R["counts"] = {"apparel": len(A), "stuffed": sum(a["stuffed"] for a in A), "stuff_total": len(S["stuff"]), "stuff_for_apparel": len(mats), "item_x_material_rows": len(bystuff)}

    # ---- 3. stuff table -------------------------------------------------------------------------------
    st_rows = []
    for s in mats:
        sb = s["statBases"]
        vol = 0.1 if s["smallVolume"] == "true" else 1.0
        st_rows.append({"defName": s["defName"], "categories": "/".join(s["stuffCategories_of_stuff"]), "commonality": s["commonality"], "market_value": sb.get("MarketValue", ""),
                        "small_volume": s["smallVolume"], "mass": sb.get("Mass", ""), "sharp": sb.get("StuffPower_Armor_Sharp"), "blunt": sb.get("StuffPower_Armor_Blunt"),
                        "heat": sb.get("StuffPower_Armor_Heat"), "ins_cold": sb.get("StuffPower_Insulation_Cold"), "ins_heat": sb.get("StuffPower_Insulation_Heat"),
                        "f_hp": s["statFactors"].get("MaxHitPoints", 1), "f_flammability": s["statFactors"].get("Flammability", 1), "f_work": s["statFactors"].get("WorkToMake", 1),
                        "f_beauty": s["statFactors"].get("Beauty", 1), "price_per_unit": round(sb.get("MarketValue", 0) / vol if False else sb.get("MarketValue", 0) / vol, 3)})
    write_csv(out / "stuff-table.csv", st_rows)

    # ---- 4. quality table (from the StatDefs) ---------------------------------------------------------
    qt = {}
    for stat in ("ArmorRating_Sharp", "Insulation_Cold", "MarketValue"):
        p = [x for x in S["stats"][stat]["parts"] if x["class"] == "StatPart_Quality"][0]
        qt[stat] = {q: {"factor": p.get("factor" + q), "maxGain": p.get("maxGain" + q)} for q in M.QUALITIES}
    R["quality"] = qt

    # ---- 5. distributions per kind and tier (reference material) ----------------------------------------
    def agg(rows, key):
        v = np.array([r[key] for r in rows], float)
        return {"n": len(v), "min": float(v.min()), "median": float(np.median(v)), "max": float(v.max())} if len(v) else None
    dist = {}
    for r in items:
        if r["kind"] == "utility" and not r["has_recipe"]:
            continue
        dist.setdefault(r["kind"], []).append(r)
    R["dist_by_kind"] = {k: {key: agg(v, key) for key in ("coverage", "sharp", "blunt", "heat", "ins_cold", "mass", "hp", "work", "market_value", "api")} | {"n": len(v)} for k, v in dist.items()}
    bt = {}
    for r in items:
        if r["kind"] != "utility":
            bt.setdefault((r["kind"], r["tier"]), []).append(r)
    R["dist_by_kind_tier"] = {"%s|%s" % k: {"n": len(v), "sharp_median": float(np.median([x["sharp"] for x in v])), "work_median": float(np.median([x["work"] for x in v])),
                                              "mv_median": float(np.median([x["market_value"] for x in v]))} for k, v in sorted(bt.items())}

    # ---- 6. market value formation ----------------------------------------------------------------------
    # (a) the formula itself: share of value that is labour vs materials, per item (reference material)
    share = []
    for a in A:
        if "MarketValue" in a["statBases"] or a["statBases"].get("WorkToMake") is None:
            continue
        sd = m.stuff.get(ref_stuff(a)) if a["stuffed"] else None
        w = a["statBases"]["WorkToMake"] * (sd["statFactors"].get("WorkToMake", 1) if sd else 1)
        labour = w * M.VALUE_PER_WORK if w > 2 else 0
        total = m.calculated_market_value(a, sd)
        share.append({"defName": a["defName"], "labour_share": labour / total, "total": total})
    R["labour_share"] = {"median": float(np.median([x["labour_share"] for x in share])), "min": min(x["labour_share"] for x in share), "max": max(x["labour_share"] for x in share), "n": len(share)}
    R["explicit_market_value_items"] = [r["defName"] for r in items if r["explicit_market_value"]]

    # (b) models: ln(MV) against the armor power index, coverage and tier
    mod = [r for r in items if not r["explicit_market_value"] and r["kind"] != "utility" and r["tier_ord"] >= 0 and r["has_recipe"]]
    armored = [r for r in mod if r["sharp"] >= 0.3 or r["blunt"] >= 0.2]
    clothes = [r for r in mod if r not in armored]
    R["model_sets"] = {"modelled": len(mod), "armored": len(armored), "clothing": len(clothes)}
    lnmv = lambda rs: np.log([r["market_value"] for r in rs])
    def design(rs, spec):
        cols = [np.ones(len(rs))]
        if "api" in spec: cols.append(np.log([r["api"] + 0.05 for r in rs]))
        if "q" in spec: cols.append(np.log([r["protection"] + 0.01 for r in rs]))
        if "cov" in spec: cols.append(np.log([r["coverage"] + 0.01 for r in rs]))
        if "sharp" in spec: cols.append(np.log([r["sharp"] + 0.05 for r in rs]))
        if "tier" in spec: cols.append(np.array([r["tier_ord"] for r in rs], float))
        if "work" in spec: cols.append(np.log([r["work"] for r in rs]))
        return np.column_stack(cols)
    specs = {"A: ln API": ["api"], "B: ln API + tier": ["api", "tier"], "C: ln Q + ln coverage + tier": ["q", "cov", "tier"], "D: ln sharp rating + tier": ["sharp", "tier"],
             "E: ln Q + ln coverage": ["q", "cov"], "F: tier only": ["tier"], "G: ln work (the formula's own driver)": ["work"]}
    R["mv_models"] = {}
    for name, sp in specs.items():
        for setname, rs in (("armored", armored), ("all modelled", mod)):
            b, res, r2 = ols(design(rs, sp), lnmv(rs))
            R["mv_models"][name + " | " + setname] = {"n": len(rs), "r2": round(r2, 3), "beta": [round(float(x), 3) for x in b], "rmse_ln": round(float(np.sqrt((res ** 2).mean())), 3)}
    # sensitivity of the index weights
    sens = {}
    for wname, w in {"sharp only": (1, 0, 0), "default 0.6/0.25/0.15": (0.6, 0.25, 0.15), "equal": (1 / 3, 1 / 3, 1 / 3), "sharp+blunt": (0.7, 0.3, 0)}.items():
        rs = armored
        for r in rs:
            a = m.apparel[r["defName"]]
            pr = w[0] * np.mean([1 - M.layer_mean_factor(r["sharp"], ap) for ap in AP_GRID]) + w[1] * (1 - M.layer_mean_factor(r["blunt"])) + w[2] * (1 - M.layer_mean_factor(r["heat"]))
            r["_api_w"] = 100 * r["coverage"] * pr
        X = np.column_stack([np.ones(len(rs)), np.log([r["_api_w"] + 0.05 for r in rs]), [r["tier_ord"] for r in rs]])
        sens[wname] = round(ols(X, lnmv(rs))[2], 3)
    R["index_weight_sensitivity_r2_armored_B"] = sens
    b, res, r2 = ols(design(armored, ["api", "tier"]), lnmv(armored))
    R["outliers_model_B"] = sorted([{"defName": r["defName"], "resid_ln": round(float(e), 3), "ratio": round(math.exp(float(e)), 2)} for r, e in zip(armored, res)], key=lambda x: -abs(x["resid_ln"]))[:8]

    # (c) work and cost models
    nonstuff = [r for r in mod if not r["stuffed"] and r["work"] > 0]
    stuffed = [r for r in mod if r["stuffed"] and r["work"] > 0]
    R["work_models"] = {}
    for label, rs in (("stuffed: ln work ~ ln stuff_units", stuffed), ("non-stuffed: ln work ~ ln material value", nonstuff)):
        if label.startswith("stuffed"):
            x = np.log([max(r["stuff_units"], 1) for r in rs])
        else:
            x = np.log([max(sum(n * m.thing_base_value(k) for k, n in m.apparel[r["defName"]]["costList"].items()), 1) for r in rs])
        X = np.column_stack([np.ones(len(rs)), x])
        b, res, r2 = ols(X, np.log([r["work"] for r in rs]))
        R["work_models"][label] = {"n": len(rs), "r2": round(r2, 3), "beta": [round(float(v), 3) for v in b]}
    ratio = [r["work"] / max(r["stuff_units"], 1) for r in stuffed]
    R["stuffed_work_per_unit"] = {"median": float(np.median(ratio)), "p10": float(np.percentile(ratio, 10)), "p90": float(np.percentile(ratio, 90))}
    nsratio = [(r["defName"], r["work"] / max(sum(n * m.thing_base_value(k) for k, n in m.apparel[r["defName"]]["costList"].items()), 1)) for r in nonstuff]
    R["nonstuffed_work_per_material_value"] = {"median": float(np.median([v for _, v in nsratio])), "p10": float(np.percentile([v for _, v in nsratio], 10)), "p90": float(np.percentile([v for _, v in nsratio], 90))}

    # ---- 7. armor-per-price of materials for reference items ------------------------------------------------
    refitems = ["Apparel_PlateArmor", "Apparel_AdvancedHelmet", "Apparel_Duster"]
    R["reference_items"] = {}
    for n in refitems:
        a = m.apparel[n]
        rows = []
        for s in mats:
            if m.can_make(a, s):
                r = item_row(m, a, s["defName"])
                rows.append({k: r[k] for k in ("stuff", "sharp", "blunt", "heat", "ins_cold", "hp", "work", "market_value", "stuff_units", "api")})
        R["reference_items"][n] = rows
    mult = {}
    for s in mats:
        sb = s["statBases"]
        mult[s["defName"]] = {"sharp_power": sb.get("StuffPower_Armor_Sharp"), "blunt_power": sb.get("StuffPower_Armor_Blunt"), "heat_power": sb.get("StuffPower_Armor_Heat"),
                              "ins_cold_power": sb.get("StuffPower_Insulation_Cold"), "f_hp": s["statFactors"].get("MaxHitPoints", 1), "f_work": s["statFactors"].get("WorkToMake", 1),
                              "market_value": sb.get("MarketValue")}
    R["material_multipliers"] = mult

    # ---- 8. worked examples ---------------------------------------------------------------------------------
    ex_items = ["Apparel_PlateArmor", "Apparel_AdvancedHelmet", "Apparel_SimpleHelmet", "Apparel_Duster", "Apparel_Parka", "Apparel_Jacket"]
    ex_mats = {"Apparel_PlateArmor": ["Steel", "Plasteel", "WoodLog"], "Apparel_AdvancedHelmet": ["Steel", "Plasteel", "Uranium"], "Apparel_SimpleHelmet": ["Steel", "Silver", "Gold"],
               "Apparel_Duster": ["Cloth", "Synthread", "Leather_Thrumbo"], "Apparel_Parka": ["Cloth", "WoolMegasloth", "Leather_Heavy"], "Apparel_Jacket": ["Cloth", "Hyperweave", "Leather_Plain"]}
    R["worked"] = []
    for n in ex_items:
        for s in ex_mats[n]:
            for q in ("Normal", "Legendary") if s in ("Steel", "Cloth") else ("Normal",):
                r = item_row(m, m.apparel[n], s, q)
                R["worked"].append({k: r[k] for k in ("defName", "stuff", "quality", "sharp", "blunt", "heat", "ins_cold", "hp", "work", "market_value", "stuff_units")})
    # worked quality ladder for one item
    R["quality_ladder"] = [{"quality": q, **{k: v for k, v in item_row(m, m.apparel["Apparel_PlateArmor"], "Steel", q).items() if k in ("sharp", "blunt", "heat", "ins_cold", "hp", "market_value")}} for q in M.QUALITIES]

    # ---- 9. pawn-level example: layered apparel, closed form + Monte Carlo of the game's rule -----------------
    outfit = [("Apparel_BasicShirt", "Cloth"), ("Apparel_FlakVest", None), ("Apparel_Jacket", "Leather_Heavy"), ("Apparel_Pants", "Cloth"), ("Apparel_AdvancedHelmet", "Steel")]
    order = {"OnSkin": 0, "Middle": 100, "Shell": 200, "Belt": 300, "Overhead": 400, "EyeCover": 500}
    worn = sorted(outfit, key=lambda x: order[m.apparel[x[0]]["layers"][-1]])          # ascending draw order; damage runs from the end
    rng = np.random.default_rng(7)
    def sim(ratings, ap, n=200000):
        amt = np.ones(n)
        for r in reversed(ratings):
            e = max(r - ap, 0.0)
            u = rng.random(n)
            amt = np.where(amt <= 0, 0, np.where(u < e / 2, 0, np.where(u < e, amt / 2, amt)))
        return float(amt.mean())
    peritem = {n: {"sharp": m.stat(n, "ArmorRating_Sharp", s)} for n, s in worn}
    parts_res = []
    total_w = 0.0; exp_total = {ap: 0.0 for ap in AP_GRID}
    for p in m.parts:
        if p["coverageAbs"] <= 0:
            continue
        cover = [(n, s) for n, s in worn if set(p["groups"]) & set(m.apparel[n]["bodyPartGroups"])]
        rat = [peritem[n]["sharp"] for n, _ in cover]
        for ap in AP_GRID:
            exp_total[ap] += p["coverageAbs"] * M.stack_mean_factor(rat, ap)
        total_w += p["coverageAbs"]
        parts_res.append({"part": p["path"].split("/")[-1], "weight": p["coverageAbs"], "layers": [n.replace("Apparel_", "") for n, _ in cover], "ratings": [round(x, 3) for x in rat]})
    R["pawn_example"] = {"outfit": [(n, s) for n, s in worn], "ratings_sharp": {n: round(v["sharp"], 3) for n, v in peritem.items()},
                         "expected_damage_fraction_random_hit": {str(ap): round(exp_total[ap] / total_w, 4) for ap in AP_GRID}, "total_weight": round(total_w, 4)}
    torso = [p for p in parts_res if p["part"] == "Torso"][0]
    head = [p for p in parts_res if p["part"] == "Head"]
    R["pawn_example"]["torso"] = {**torso, "closed_form_ap0.15": round(M.stack_mean_factor(torso["ratings"], 0.15), 4), "monte_carlo_ap0.15": round(sim(torso["ratings"], 0.15), 4)}
    if head:
        R["pawn_example"]["head"] = {**head[0], "closed_form_ap0.15": round(M.stack_mean_factor(head[0]["ratings"], 0.15), 4)}
    R["layer_stacking"] = [{"ratings": rs, "ap": ap, "mean_factor": round(M.stack_mean_factor(rs, ap), 4)} for rs in ([0.5], [0.5, 0.5], [1.0], [1.0, 1.0], [0.9, 0.36], [1.2]) for ap in (0.0, 0.3)]

    # ---- 10. archetypes (k-means on standardised features, fixed seed) -----------------------------------------------
    from scipy.cluster.vq import kmeans2
    feats = np.array([[math.log(r["api"] + 0.05), r["coverage"], math.log(max(r["work"], 100)), r["tier_ord"], r["move_speed_offset"], math.log(r["market_value"] + 1)] for r in mod], float)
    z = (feats - feats.mean(0)) / feats.std(0)
    np.random.seed(11)
    cent, lab = kmeans2(z, 6, minit="++", seed=11)
    arche = {}
    for r, l in zip(mod, lab):
        arche.setdefault(int(l), []).append(r)
    R["archetypes"] = [{"cluster": k, "n": len(v), "median_coverage": round(float(np.median([x["coverage"] for x in v])), 3), "median_sharp": round(float(np.median([x["sharp"] for x in v])), 2),
                        "median_work": float(np.median([x["work"] for x in v])), "median_mv": round(float(np.median([x["market_value"] for x in v])), 1), "members": [x["defName"].replace("Apparel_", "") for x in v][:14]}
                       for k, v in sorted(arche.items(), key=lambda kv: np.median([x["market_value"] for x in kv[1]]))]

    # ---- 11. baseline generator and backtests ---------------------------------------------------------------------------
    # Baseline = anchor (median of the same stuffed/fixed group, kind and tier) scaled by coverage^k, for work, mass, hit points
    # and material cost; market value is then DERIVED with the game's own formula. Compared with a plain group median and an OLS
    # fit. Two backtests: plain leave-one-out and "twin-excluded" leave-one-out (identical items in the training set are dropped,
    # which mimics designing a genuinely new item instead of a recoloured sibling).
    pool = [r for r in mod if r["work"] > 0 and r["mass"] > 0]
    for r in pool:
        a_ = m.apparel[r["defName"]]
        r["cost_total"] = r["stuff_units"] if r["stuffed"] else sum(n * m.thing_base_value(k) for k, n in a_["costList"].items())
    twin = lambda r: (r["work"], r["mass"], r["hp"], r["cost_total"])
    gkeys = (lambda r: (r["stuffed"], r["kind"], r["tier"]), lambda r: (r["stuffed"], r["kind"]), lambda r: (r["stuffed"],))
    lcov = lambda r: math.log(r["coverage"] + 0.01)
    def anchor_pred(train, r, t, k):
        for g in gkeys:
            same = [x for x in train if g(x) == g(r)]
            if same:
                return math.exp(float(np.median([math.log(x[t]) - k * lcov(x) for x in same])) + k * lcov(r))
    def ols_pred(train, r, t):
        X = np.array([[1, lcov(x), x["tier_ord"], 1.0 if x["stuffed"] else 0.0] for x in train]); y = np.log([x[t] for x in train])
        b, _, _ = ols(X, y, 1e-3)
        return math.exp(float(np.array([1, lcov(r), r["tier_ord"], 1.0 if r["stuffed"] else 0.0]) @ b))
    def derive_mv(r, work, cost):
        sd = m.stuff.get(ref_stuff(m.apparel[r["defName"]])) if r["stuffed"] else None
        w = work * (sd["statFactors"].get("WorkToMake", 1) if sd else 1)
        base = (cost / (0.1 if sd["smallVolume"] == "true" else 1.0) * sd["statBases"].get("MarketValue", 0) if False else (cost * sd["statBases"].get("MarketValue", 0) if sd else cost))
        return base + (w * M.VALUE_PER_WORK if w > 2 else 0)
    def run(exclude_twins, k):
        res = {t: {"anchor": [], "naive": [], "ols": []} for t in ("work", "mass", "hp", "cost_total", "market_value")}
        rows = []
        for i, r in enumerate(pool):
            train = [x for j, x in enumerate(pool) if j != i and not (exclude_twins and twin(x) == twin(r))]
            preds = {}
            for t in ("work", "mass", "hp", "cost_total"):
                pa = anchor_pred(train, r, t, k)
                pn = anchor_pred(train, r, t, 0.0)
                po = ols_pred(train, r, t)
                preds[t] = (pa, pn, po)
                for name, p in zip(("anchor", "naive", "ols"), (pa, pn, po)):
                    res[t][name].append(abs(p / r[t] - 1))
            for idx, name in enumerate(("anchor", "naive", "ols")):
                mvp = derive_mv(r, preds["work"][idx], preds["cost_total"][idx])
                res["market_value"][name].append(abs(mvp / r["market_value"] - 1))
            rows.append({"defName": r["defName"], "twin_excluded": exclude_twins, **{t + "_actual": round(r[t], 2) for t in ("work", "mass", "hp", "cost_total", "market_value")},
                         **{t + "_baseline": round(preds[t][0], 2) for t in ("work", "mass", "hp", "cost_total")}, "market_value_baseline": round(derive_mv(r, preds["work"][0], preds["cost_total"][0]), 2)})
        summ = {t: {n: {"median_abs_pct_err": round(100 * float(np.median(v)), 1), "p80_abs_pct_err": round(100 * float(np.percentile(v, 80)), 1)} for n, v in d.items()} for t, d in res.items()}
        return summ, rows
    # choose k on the twin-excluded backtest (grid, global)
    grid = {k: run(True, k)[0] for k in (0.0, 0.25, 0.5, 0.75, 1.0)}
    bestk = min(grid, key=lambda k: sum(grid[k][t]["anchor"]["median_abs_pct_err"] for t in ("work", "mass", "hp", "cost_total")))
    s_loo, rows_loo = run(False, bestk)
    s_tw, rows_tw = run(True, bestk)
    R["baseline_backtest"] = {"n": len(pool), "k_grid_twin_excluded": {str(k): {t: v[t]["anchor"]["median_abs_pct_err"] for t in v} for k, v in grid.items()}, "best_k": bestk,
                              "plain_loo": s_loo, "twin_excluded_loo": s_tw, "twin_groups_dropped_median": float(np.median([sum(1 for x in pool if twin(x) == twin(r)) - 1 for r in pool]))}
    write_csv(out / "backtest-loo.csv", rows_loo + rows_tw)
    # strength bands: per (stuffed, kind) the observed quantiles of armor numbers, for the design guidance
    bands = {}
    for r in mod:
        bands.setdefault("%s|%s" % ("stuffed" if r["stuffed"] else "fixed", r["kind"]), []).append(r)
    R["bands"] = {k: {key: {q: round(float(np.percentile([x[key] for x in v], p_)), 3) for q, p_ in (("p10", 10), ("p50", 50), ("p90", 90))} | {"n": len(v)} for key in ("coverage", "sharp", "blunt", "heat", "work", "mass", "hp", "market_value")} for k, v in sorted(bands.items())}
    R["armor_mult_by_kind"] = {k: sorted({float(r["armor_mult"]) for r in items if r["kind"] == k and r["stuffed"] and r["armor_mult"] != ""}) for k in sorted({r["kind"] for r in items})}

    (out / "models.json").write_text(json.dumps(R, indent=1, sort_keys=True))
    print(json.dumps({k: R[k] for k in ("counts", "model_sets", "labour_share", "mv_models", "index_weight_sensitivity_r2_armored_B", "work_models", "stuffed_work_per_unit",
                                       "nonstuffed_work_per_material_value", "baseline_backtest", "pawn_example")}, indent=1)[:9000])

    if args.figs:
        figs(out, items, armored, mod, R, m, mats)


def figs(out, items, armored, mod, R, m, mats):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    pal = {0: "#2a9d8f", 1: "#e9c46a", 2: "#e76f51", 3: "#264653", 4: "#7b2cbf", 5: "#555555"}
    names = {0: "Neolithic", 1: "Medieval", 2: "Industrial", 3: "Spacer", 4: "Ultra", 5: "Archotech"}
    plt.rcParams.update({"font.size": 9, "axes.spines.top": False, "axes.spines.right": False})
    # fig 1: MV vs API
    fig, ax = plt.subplots(figsize=(6.4, 4.2))
    for t in sorted({r["tier_ord"] for r in armored}):
        rs = [r for r in armored if r["tier_ord"] == t]
        ax.scatter([r["api"] + 0.05 for r in rs], [r["market_value"] for r in rs], c=pal[t], label=names[t], s=22)
    b = R["mv_models"]["B: ln API + tier | armored"]["beta"]
    xs = np.logspace(-1, 1.7, 50)
    for t in sorted({r["tier_ord"] for r in armored}):
        ax.plot(xs, np.exp(b[0] + b[1] * np.log(xs) + b[2] * t), c=pal[t], lw=1, alpha=.7)
    ax.set_xscale("log"); ax.set_yscale("log"); ax.set_xlabel("armor power index (API + 0.05)"); ax.set_ylabel("market value (silver)")
    ax.set_title("Market value against armor power index, by tier"); ax.legend(frameon=False)
    fig.tight_layout(); fig.savefig(out / "fig1_mv_vs_api.png", dpi=100); plt.close(fig)
    # fig 2: sharp armor per material for reference items
    fig, axs = plt.subplots(1, 3, figsize=(10, 5.4), sharey=False)
    for ax, (n, rows) in zip(axs, R["reference_items"].items()):
        rows = sorted(rows, key=lambda r: r["sharp"])
        ax.barh([r["stuff"] for r in rows], [r["sharp"] * 100 for r in rows], color="#264653")
        ax.set_title(n.replace("Apparel_", ""), fontsize=9); ax.set_xlabel("sharp armor, % (Normal)"); ax.tick_params(axis="y", labelsize=5.5)
    fig.tight_layout(); fig.savefig(out / "fig2_armor_by_material.png", dpi=100); plt.close(fig)
    # fig 3: layer stacking
    fig, ax = plt.subplots(figsize=(5.6, 3.8))
    xs = np.linspace(0, 2, 101)
    for ap, ls in ((0.0, "-"), (0.3, "--")):
        ax.plot(xs, [1 - M.layer_mean_factor(x, ap) for x in xs], ls, c="#264653", label="one layer, AP %.2f" % ap)
        ax.plot(xs, [1 - M.stack_mean_factor([x, x], ap) for x in xs], ls, c="#e76f51", label="two equal layers, AP %.2f" % ap)
    ax.set_xlabel("armor rating per layer (1.0 = 100%)"); ax.set_ylabel("expected damage prevented"); ax.legend(frameon=False, fontsize=7); ax.set_title("Expected damage prevented by layers")
    fig.tight_layout(); fig.savefig(out / "fig3_layer_stacking.png", dpi=100); plt.close(fig)
    # fig 4: work vs material cost
    fig, ax = plt.subplots(figsize=(6, 4))
    st = [r for r in mod if r["stuffed"] and r["work"] > 0]; ns = [r for r in mod if not r["stuffed"] and r["work"] > 0]
    ax.scatter([max(r["stuff_units"], 1) for r in st], [r["work"] for r in st], c="#2a9d8f", s=18, label="stuffed (x: material units)")
    ax2 = ax.twiny()
    ax2.scatter([max(sum(n * m.thing_base_value(k) for k, n in m.apparel[r["defName"]]["costList"].items()), 1) for r in ns], [r["work"] for r in ns], c="#e76f51", s=18, marker="s", label="fixed cost (top x: material value)")
    ax.set_xscale("log"); ax2.set_xscale("log"); ax.set_yscale("log"); ax.set_ylabel("WorkToMake"); ax.set_xlabel("material units (stuffed)"); ax2.set_xlabel("material value (fixed-cost items)")
    ax.set_title("Work to make against material cost", y=1.12)
    fig.tight_layout(); fig.savefig(out / "fig4_work_vs_cost.png", dpi=100); plt.close(fig)
    # fig 5: backtest (twin-excluded)
    rows = [r for r in csv.DictReader(open(out / "backtest-loo.csv")) if r["twin_excluded"] == "True"]
    fig, axs = plt.subplots(1, 5, figsize=(11, 2.6))
    for ax, tg in zip(axs, ("work", "mass", "hp", "cost_total", "market_value")):
        xs_ = [float(r[tg + "_actual"]) for r in rows]; ys_ = [float(r[tg + "_baseline"]) for r in rows]
        ax.scatter(xs_, ys_, s=9, c="#264653"); lo, hi = min(xs_), max(xs_)
        ax.plot([lo, hi], [lo, hi], c="#e76f51", lw=1); ax.set_xscale("log"); ax.set_yscale("log"); ax.set_title(tg, fontsize=8); ax.set_xlabel("actual"); ax.set_ylabel("baseline")
    fig.tight_layout(); fig.savefig(out / "fig5_backtest.png", dpi=100); plt.close(fig)


if __name__ == "__main__":
    main()
