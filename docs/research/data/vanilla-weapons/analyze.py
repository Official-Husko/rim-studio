#!/usr/bin/env python3
"""Statistics, fits, power index and backtests for the vanilla weapon datasets.

Usage:  PYTHONDONTWRITEBYTECODE=1 python3 analyze.py [--data DIR]   (DIR holds the JSON files written by extract.py)
Writes (into DIR): ranged_table.csv, melee_table.csv, melee_stuff_table.csv, stats_ranged.csv, stats_melee.csv,
fits.json, backtest.json and a few PNG figures.  Needs numpy, pandas, scipy, matplotlib.  Deterministic.
"""
from __future__ import annotations

import argparse
import itertools
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from scipy import stats as sps

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE))
import wlib  # noqa: E402

TIER = {"Neolithic": 0, "Medieval": 1, "Industrial": 2, "Spacer": 3, "Ultra": 4}

ROLE = {
    "Bow_Short": "bow", "Bow_Recurve": "bow", "Bow_Great": "bow", "Pila": "bow", "Flamebow": "bow",
    "Gun_Revolver": "pistol", "Gun_Autopistol": "pistol", "Gun_MachinePistol": "pistol",
    "Gun_HeavySMG": "smg",
    "Gun_AssaultRifle": "rifle", "Gun_HellcatRifle": "rifle", "Gun_ChargeRifle": "rifle",
    "Gun_BoltActionRifle": "sniper", "Gun_SniperRifle": "sniper",
    "Gun_PumpShotgun": "shotgun", "Gun_ChainShotgun": "shotgun",
    "Gun_LMG": "heavy", "Gun_Minigun": "heavy", "Gun_BeamRepeater": "heavy", "Gun_ChargeLance": "heavy",
    "Gun_IncendiaryLauncher": "launcher", "Gun_EmpLauncher": "launcher", "Gun_SmokeLauncher": "launcher",
    "Gun_ToxbombLauncher": "launcher",
    "Weapon_GrenadeFrag": "grenade", "Weapon_GrenadeMolotov": "grenade", "Weapon_GrenadeEMP": "grenade",
    "Weapon_GrenadeTox": "grenade",
    "Gun_TripleRocket": "rocket", "Gun_DoomsdayRocket": "rocket",
    "Gun_Incinerator": "flame",
}
DIRECT_ROLES = ["bow", "pistol", "smg", "rifle", "sniper", "shotgun", "heavy"]
ARMOR_REFS = {"none": 0.0, "flak_jacket": 0.55, "marine_armor": 0.92, "flak_vest": 1.0}  # sharp ratings read from the Apparel defs
RANGES = {"touch": 3, "short": 12, "medium": 25, "long": 40}


def fnum(x, d=None):
    try:
        return float(x)
    except (TypeError, ValueError):
        return d


def ranged_row(w):
    v = w["verb"]
    sb = w["statBases"]
    pj = w["projectile"].get("projectile") or {}
    dd = w["projectile"].get("damageDef") or {}
    beam = w.get("beamDamage")
    burst = int(fnum(v.get("burstShotCount"), 1))
    ticks = fnum(v.get("ticksBetweenBurstShots"), 15)
    warm = fnum(v.get("warmupTime"), 0.0)
    cd = sb.get("RangedWeapon_Cooldown", 0.0)
    cycle = wlib.ranged_cycle_seconds(warm, cd, burst, ticks)
    explosive = False
    kind = "projectile"
    if beam:
        dmg = beam["defaultDamage"]
        ap = max(beam["defaultArmorPenetration"], 0.0)
        kind = "beam"
    else:
        base = fnum(pj.get("damageAmountBase"), -1)
        dmg = base if base != -1 else dd.get("defaultDamage", -1)
        # armor penetration rule of ProjectileProperties: explicit base wins; when the damage amount is explicit and
        # no base is given the damage def default is skipped; a negative result becomes damage * 0.015
        apb = fnum(pj.get("armorPenetrationBase"), -1)
        if base != -1 or apb >= 0:
            ap = apb
        else:
            ap = dd.get("defaultArmorPenetration", -1)
        if ap < 0:
            ap = dmg * wlib.AP_PER_DAMAGE
        if dd.get("armorCategory") is None:
            ap = 0.0
        explosive = fnum(pj.get("explosionRadius"), 0) > 0
        if explosive:
            kind = "explosive"
    acc = {k: sb.get("Accuracy" + k.capitalize()) for k in RANGES}
    have_acc = all(a is not None for a in acc.values())
    row = {
        "defName": w["defName"], "label": w["label"], "source": w["source"], "group": w["group"],
        "role": ROLE.get(w["defName"], w["group"]), "tech": w["techLevel"], "tier": TIER.get(w["techLevel"], np.nan),
        "kind": kind, "verbClass": v.get("verbClass"),
        "damage": dmg, "ap": ap, "burst": burst, "ticks_between": ticks if burst > 1 else 0, "warmup": warm,
        "cooldown": cd, "cycle": cycle, "range": fnum(v.get("range")), "min_range": fnum(v.get("minRange"), 0),
        "radius": fnum(pj.get("explosionRadius"), 0), "speed": fnum(pj.get("speed")),
        "stopping_power": fnum(pj.get("stoppingPower")),
        "mass": sb.get("Mass"), "work": sb.get("WorkToMake"), "mv": w["marketValue"]["effective"],
        "mv_explicit": w["marketValue"]["explicit"] is not None,
        "mv_formula": w["marketValue"]["formula_unstuffed"],
        "ingredients": w["marketValue"]["formula_parts"]["ingredients"],
        "n_statbases": w["statBaseCount"], "hp": sb.get("MaxHitPoints"), "sell_factor": sb.get("SellPriceFactor"),
        "research": ";".join(r["defName"] for r in w["recipe"]["research"]),
        "research_cost": sum((r.get("baseCost") or 0) for r in w["recipe"]["research"]),
        "craftable": w["recipe"]["craftable"],
        "weapon_classes": ";".join(w["weaponClasses"]),
    }
    for k, a in acc.items():
        row["acc_" + k] = a
    row["has_acc"] = have_acc
    dps_nom = dmg * burst / cycle if cycle > 0 and dmg and dmg > 0 else 0.0
    row["dps_nominal"] = dps_nom
    if have_acc:
        for name, r in (("3", 3), ("12", 12), ("25", 25), ("40", 40)):
            row["dps_at_" + name] = dps_nom * wlib.weapon_accuracy(acc, r)
        row["acc_mid"] = (wlib.weapon_accuracy(acc, 12) + wlib.weapon_accuracy(acc, 25)) / 2
        row["acc_avg_0_range"] = float(np.mean([wlib.weapon_accuracy(acc, r) for r in np.linspace(3, min(row["range"], 40), 12)]))
    else:
        row["acc_mid"] = 1.0
        row["acc_avg_0_range"] = 1.0
        for name in ("3", "12", "25", "40"):
            row["dps_at_" + name] = dps_nom
    for name, a in ARMOR_REFS.items():
        row["armor_mult_" + name] = wlib.armor_multiplier(a, ap)
    row["armor_mult_avg"] = float(np.mean([row["armor_mult_" + n] for n in ("none", "flak_jacket", "flak_vest")]))
    row["dps_eff_mid"] = dps_nom * row["acc_mid"]
    row["dps_armored"] = row["dps_eff_mid"] * row["armor_mult_avg"]
    row["power"] = row["dps_armored"] * math.sqrt(max(row["range"], 1) / 25.0)  # candidate P4, see fits.json
    return row


def melee_rows(M, man, stuffs):
    kind_of = {cap: v[0]["armorCategory"] for cap, v in man.items()}
    rows, srows = [], []
    for w in M["weapons"]:
        base = {"defName": w["defName"], "label": w["label"], "source": w["source"], "group": w["group"],
                "tech": w["techLevel"], "tier": TIER.get(w["techLevel"], np.nan),
                "weapon_classes": ";".join(w["weaponClasses"]), "mass": w["statBases"].get("Mass"),
                "n_statbases": w["statBaseCount"], "work_base": w["statBases"].get("WorkToMake"),
                "stuff_count": w["cost"]["costStuffCount"], "stuffed": w["cost"]["costStuffCount"] > 0,
                "n_tools": len(w["tools"]), "equip_offsets": ";".join(w["equippedStatOffsets"]),
                "research": ";".join(r["defName"] for r in w["recipe"]["research"]),
                "research_cost": sum((r.get("baseCost") or 0) for r in w["recipe"]["research"]),
                "mv_explicit": w["marketValue"]["explicit"] is not None}
        cats = w["cost"]["stuffCategories"]
        options = [None]
        if w["cost"]["costStuffCount"] > 0:
            options = ["Steel"] + sorted(s for s, d in stuffs.items() if set(d["categories"]) & set(cats) and s != "Steel")
        for s in options:
            sd = stuffs[s] if s else None
            mults = {"Sharp": sd["sharp_damage_mult"], "Blunt": sd["blunt_damage_mult"]} if sd else None
            cdm = w["statBases"].get("MeleeWeapon_CooldownMultiplier", 1.0) * (sd["stat_factors"].get("MeleeWeapon_CooldownMultiplier", 1.0) if sd else 1.0)
            ents = wlib.melee_tool_entries(w["tools"], kind_of, mults, 1.0, cdm)
            av = wlib.melee_averages(ents)
            best = max(ents, key=lambda e: e["damage"] / e["cooldown"])
            row = dict(base)
            sel = wlib.melee_selection_dps(ents)
            row.update(sel)
            row.update({"stuff": s or "", "avg_damage": av["avg_damage"], "avg_cooldown": av["avg_cooldown"],
                        "dps": av["dps"], "avg_ap": av["avg_ap"],
                        "best_tool": best["tool"] + "/" + best["capacity"], "best_dps": best["damage"] / best["cooldown"],
                        "best_damage": best["damage"], "best_ap": best["ap"],
                        "max_damage": max(e["damage"] for e in ents), "min_cooldown": min(e["cooldown"] for e in ents)})
            # unweighted (chanceFactor only) alternative, to show how much the dmg^2 weight matters
            sw = sum(e["chanceFactor"] for e in ents)
            row["dps_chance_only"] = (sum(e["damage"] * e["chanceFactor"] for e in ents) / sw) / (sum(e["cooldown"] * e["chanceFactor"] for e in ents) / sw)
            # market value
            sb = w["statBases"]
            if w["marketValue"]["explicit"] is not None:
                mv = w["marketValue"]["explicit"]
                work = sb.get("WorkToMake")
            else:
                ing = w["marketValue"]["formula_parts"]["ingredients"]
                work = (sb.get("WorkToMake") or 0) * (sd["stat_factors"].get("WorkToMake", 1.0) if sd else 1.0)
                if sd:
                    stuff_part = w["cost"]["costStuffCount"] / sd["volume_per_unit"] * sd["market_value"]
                else:
                    stuff_part = 0.0
                mv = ing + stuff_part + (work * wlib.VALUE_PER_WORK if work > 2 else 0)
            row["work"] = work
            row["mv"] = mv
            (rows if s in (None, "Steel") else srows).append(row)
            if s == "Steel":
                srows.append(dict(row))
    return pd.DataFrame(rows), pd.DataFrame(srows)


def fit_ols(X, y):
    X = np.column_stack([np.ones(len(X)), X])
    beta, *_ = np.linalg.lstsq(X, y, rcond=None)
    pred = X @ beta
    ss_res = float(((y - pred) ** 2).sum())
    ss_tot = float(((y - y.mean()) ** 2).sum())
    r2 = 1 - ss_res / ss_tot if ss_tot > 0 else float("nan")
    n, k = X.shape
    adj = 1 - (1 - r2) * (n - 1) / (n - k) if n > k else float("nan")
    # leave-one-out via hat matrix
    H = X @ np.linalg.pinv(X.T @ X) @ X.T
    res = y - pred
    loo = res / (1 - np.diag(H))
    return {"beta": beta.tolist(), "r2": r2, "adj_r2": adj, "rmse": math.sqrt(ss_res / n), "n": int(n),
            "loo_rmse": float(np.sqrt(np.mean(loo ** 2))), "pred": pred, "resid": res}


def summ(s):
    s = pd.Series(s).dropna()
    if len(s) == 0:
        return {}
    q1, q3 = s.quantile(0.25), s.quantile(0.75)
    return {"n": int(len(s)), "min": s.min(), "q1": q1, "median": s.median(), "q3": q3, "max": s.max(), "iqr": q3 - q1}


def stats_table(df, keys, cols):
    out = []
    for key, g in df.groupby(keys):
        key = key if isinstance(key, tuple) else (key,)
        for c in cols:
            d = summ(g[c])
            if d:
                out.append({**{k: v for k, v in zip(keys, key)}, "stat": c, **d})
    return pd.DataFrame(out)


def backtest(df, targets, group_col, tier_col="tier", power_col="power", min_group=3):
    """Leave-one-out prediction of each target from (tier, class, relative strength percentile) only.
    Relative strength = percentile rank of the weapon's power index within its class peers (the quiz-style input).
    Three predictors: global median; class median; log-linear model log(y) = a + b*tier + c*pct + class offset
    (class offsets shrunk with ridge).  Errors are absolute percentage errors."""
    df = df.reset_index(drop=True)
    classes = sorted(df[group_col].unique())
    results = {t: {"global_median": [], "class_median": [], "model": []} for t in targets}
    for i in range(len(df)):
        tr = df.drop(index=i)
        te = df.loc[i]
        peers = tr[tr[group_col] == te[group_col]]
        pool = peers if len(peers) >= min_group else tr
        pct = float((pool[power_col] < te[power_col]).mean() + 0.5 * (pool[power_col] == te[power_col]).mean())
        # percentile of every training weapon within its own class (leave-one-out safe: uses training peers only)
        tr_pct = []
        for j, r in tr.iterrows():
            pj = tr[tr[group_col] == r[group_col]]
            pj = pj if len(pj) >= min_group else tr
            tr_pct.append(float((pj[power_col] < r[power_col]).mean() + 0.5 * (pj[power_col] == r[power_col]).mean()))
        tr_pct = np.array(tr_pct)

        def design(frame, pcts):
            cols = [frame[tier_col].to_numpy(float), np.asarray(pcts, float)]
            for c in classes:
                cols.append((frame[group_col] == c).to_numpy(float))
            return np.column_stack(cols)

        Xtr = design(tr, tr_pct)
        Xte = design(te.to_frame().T, [pct])
        for t in targets:
            ok = tr[t].notna() & (tr[t] > 0)
            if not (pd.notna(te[t]) and te[t] > 0) or ok.sum() < 6:
                continue
            ytr = np.log(tr.loc[ok, t].to_numpy(float))
            truth = float(te[t])
            gm = float(tr.loc[ok, t].median())
            cm = float(peers[t].median()) if peers[t].notna().sum() >= 1 else gm
            A = np.column_stack([np.ones(ok.sum()), Xtr[ok.to_numpy()]])
            lam = np.diag([0, 1e-6, 1e-6] + [0.5] * len(classes))
            beta = np.linalg.solve(A.T @ A + lam, A.T @ ytr)
            pm = float(np.exp(np.concatenate([[1.0], Xte[0]]) @ beta))
            for name, p in (("global_median", gm), ("class_median", cm), ("model", pm)):
                results[t][name].append(abs(p - truth) / truth)
    out = {}
    for t, d in results.items():
        out[t] = {k: {"n": len(v), "mape": float(np.mean(v)) if v else None, "median_ape": float(np.median(v)) if v else None}
                  for k, v in d.items()}
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default=str(HERE))
    a = ap.parse_args()
    D = Path(a.data)
    R = json.loads((D / "vanilla-ranged.json").read_text())["weapons"]
    M = json.loads((D / "vanilla-melee.json").read_text())
    man = json.loads((D / "maneuvers.json").read_text())
    stuffs = json.loads((D / "stuffs.json").read_text())

    rd = pd.DataFrame([ranged_row(w) for w in R]).sort_values("defName")
    md, ms = melee_rows(M, man, stuffs)
    rd.to_csv(D / "ranged_table.csv", index=False, float_format="%.4g")
    md.to_csv(D / "melee_table.csv", index=False, float_format="%.4g")
    ms.to_csv(D / "melee_stuff_table.csv", index=False, float_format="%.4g")

    fits = {}
    std = rd[(rd.group == "standard") & rd.role.isin(DIRECT_ROLES)].copy()
    fits["ranged_direct_n"] = int(len(std))

    # ---- power index candidates for direct fire weapons
    cands = {
        "P1_dps_nominal": std.dps_nominal,
        "P2_dps_mid_accuracy": std.dps_eff_mid,
        "P3_dps_mid_armor": std.dps_armored,
        "P4_dps_mid_armor_range": std.dps_armored * np.sqrt(std.range / 25.0),
        "P5_dps_mid_armor_range_lin": std.dps_armored * std.range / 25.0,
    }
    ly = np.log(std.mv.to_numpy(float))
    cand_out = {}
    for name, p in cands.items():
        lp = np.log(p.to_numpy(float))
        f1 = fit_ols(lp.reshape(-1, 1), ly)
        f2 = fit_ols(np.column_stack([lp, std.tier.to_numpy(float)]), ly)
        sp = sps.spearmanr(p, std.mv)
        cand_out[name] = {"r2_logmv_logp": f1["r2"], "r2_with_tier": f2["r2"], "adj_r2_with_tier": f2["adj_r2"],
                          "loo_rmse_log_with_tier": f2["loo_rmse"], "spearman_mv": float(sp.correlation),
                          "beta_with_tier": f2["beta"]}
    fits["ranged_power_candidates"] = cand_out

    lm = np.log(std.mass.to_numpy(float))
    lw = np.log(std.work.fillna(std.work.median()).to_numpy(float))
    lp4 = np.log(cands["P4_dps_mid_armor_range"].to_numpy(float))
    tr_ = std.tier.to_numpy(float)
    cmp_ = {}
    for name, X in (("tier_only", tr_.reshape(-1, 1)), ("mass_only", lm.reshape(-1, 1)), ("mass_tier", np.column_stack([lm, tr_])),
                    ("P4_only", lp4.reshape(-1, 1)), ("P4_tier", np.column_stack([lp4, tr_])), ("P4_mass", np.column_stack([lp4, lm])),
                    ("P4_mass_tier", np.column_stack([lp4, lm, tr_]))):
        f_ = fit_ols(X, ly)
        cmp_[name] = {"r2": f_["r2"], "adj_r2": f_["adj_r2"], "loo_rmse_log": f_["loo_rmse"], "beta": f_["beta"]}
    fits["ranged_model_compare_logmv"] = cmp_
    ing = std.ingredients.replace(0, np.nan)
    okk = ing.notna() & std.work.notna()
    sub = std[okk]
    for tname, yv in (("ln_work", np.log(sub.work.to_numpy(float))), ("ln_ingredients", np.log(sub.ingredients.to_numpy(float)))):
        X = np.column_stack([np.log(sub.power.to_numpy(float)), sub.tier.to_numpy(float)])
        f_ = fit_ols(X, yv)
        fits["ranged_" + tname + "_on_P4_tier"] = {"beta": f_["beta"], "r2": f_["r2"], "n": f_["n"], "loo_rmse_log": f_["loo_rmse"]}
    # chosen model: log(MV) ~ log(P4) + tier
    lp = np.log(cands["P4_dps_mid_armor_range"].to_numpy(float))
    ch = fit_ols(np.column_stack([lp, std.tier.to_numpy(float)]), ly)
    std["mv_pred"] = np.exp(ch["pred"])
    std["mv_resid_log"] = ch["resid"]
    fits["ranged_chosen"] = {"form": "ln(MarketValue) = b0 + b1*ln(P4) + b2*tier", "beta": ch["beta"], "r2": ch["r2"],
                             "adj_r2": ch["adj_r2"], "rmse_log": ch["rmse"], "loo_rmse_log": ch["loo_rmse"], "n": ch["n"],
                             "outliers_abs_resid_gt_0p4": std.loc[np.abs(ch["resid"]) > 0.4, "defName"].tolist(),
                             "residuals": {r.defName: round(float(x), 3) for r, x in zip(std.itertuples(), ch["resid"])}}
    # MV from work and ingredients (identity check) over all craftable standard ranged with computed MV
    comp = rd[(rd.group == "standard") & (~rd.mv_explicit) & rd.craftable]
    fits["ranged_mv_identity"] = {"n": int(len(comp)),
                                  "max_abs_diff_formula_vs_effective": float((comp.mv - comp.mv_formula).abs().max()),
                                  "work_share_of_mv_median": float((comp.work * wlib.VALUE_PER_WORK / comp.mv).median()),
                                  "work_share_min_max": [float((comp.work * wlib.VALUE_PER_WORK / comp.mv).min()),
                                                         float((comp.work * wlib.VALUE_PER_WORK / comp.mv).max())]}
    # WorkToMake vs MV and ingredient value vs MV (log-log) for craftable weapons with both
    cw = comp[(comp.work > 0) & (comp.ingredients > 0)]
    if len(cw) > 4:
        f = fit_ols(np.log(cw.work.to_numpy(float)).reshape(-1, 1), np.log(cw.mv.to_numpy(float)))
        g = fit_ols(np.log(cw.ingredients.to_numpy(float)).reshape(-1, 1), np.log(cw.mv.to_numpy(float)))
        h = fit_ols(np.log(cw.work.to_numpy(float)).reshape(-1, 1), np.log(cw.ingredients.to_numpy(float)))
        fits["ranged_work_cost_mv"] = {"n": int(len(cw)), "logmv_on_logwork": {"beta": f["beta"], "r2": f["r2"]},
                                       "logmv_on_logingredients": {"beta": g["beta"], "r2": g["r2"]},
                                       "logingredients_on_logwork": {"beta": h["beta"], "r2": h["r2"]}}
    # mass vs role
    mass_by_role = {r: summ(g.mass) for r, g in rd[rd.group == "standard"].groupby("role")}
    fits["ranged_mass_by_role"] = mass_by_role
    ss_between = 0.0
    g = rd[(rd.group == "standard") & rd.mass.notna()]
    mean = g.mass.mean()
    ss_tot = ((g.mass - mean) ** 2).sum()
    for r, gg in g.groupby("role"):
        ss_between += len(gg) * (gg.mass.mean() - mean) ** 2
    fits["ranged_mass_role_eta2"] = float(ss_between / ss_tot)

    # direct-fire interpretation: partial effects
    fits["ranged_dps_components"] = {c: float(np.log(std[c].replace(0, np.nan)).corr(np.log(std.mv))) for c in
                                     ["damage", "dps_nominal", "range", "ap", "acc_mid", "cooldown", "warmup", "burst", "mass", "work"] if (std[c] > 0).all()}

    # ---- melee
    m_steel = md.copy()
    m_steel_craft = m_steel[m_steel.stuffed].copy()
    mc = {}
    m_steel["PM"] = m_steel.dps_select * (1 + m_steel.sel_ap)
    m_steel_craft = m_steel[m_steel.stuffed].copy()
    ms["PM"] = ms.dps_select * (1 + ms.sel_ap)
    for name, col in (("dps_game", "dps"), ("dps_select", "dps_select"), ("PM_select_x_ap", "PM"), ("dps_chance_only", "dps_chance_only"), ("best_dps", "best_dps"),
                      ("avg_damage", "avg_damage"), ("dps_x_ap", None)):
        p = m_steel[col] if col else m_steel.dps * (1 + m_steel.avg_ap)
        d = m_steel_craft
        pp = d[col] if col else d.dps * (1 + d.avg_ap)
        yy = np.log(d.mv.to_numpy(float))
        f1 = fit_ols(np.log(pp.to_numpy(float)).reshape(-1, 1), yy)
        mc[name] = {"r2_craftable_steel": f1["r2"], "beta": f1["beta"], "n": f1["n"],
                    "spearman_all17": float(sps.spearmanr(p, m_steel.mv).correlation)}
    fits["melee_power_candidates"] = mc
    # material count vs dps among craftable
    f = fit_ols(np.log(m_steel_craft.dps.to_numpy(float)).reshape(-1, 1), np.log(m_steel_craft.stuff_count.to_numpy(float)))
    fits["melee_stuffcount_on_dps"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}
    f = fit_ols(np.log(m_steel_craft.dps.to_numpy(float)).reshape(-1, 1), np.log(m_steel_craft.work.to_numpy(float)))
    fits["melee_work_on_dps"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}
    f = fit_ols(np.column_stack([np.log(m_steel_craft.stuff_count.to_numpy(float)), np.log(m_steel_craft.work.to_numpy(float))]),
                np.log(m_steel_craft.dps.to_numpy(float)))
    fits["melee_dps_on_cost_and_work"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"], "loo_rmse_log": f["loo_rmse"]}
    f = fit_ols(np.column_stack([np.log(m_steel_craft.dps.to_numpy(float))]), np.log(m_steel_craft.mv.to_numpy(float)))
    fits["melee_mv_on_dps_steel"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"], "loo_rmse_log": f["loo_rmse"],
                                     "residuals": {n: round(float(x), 3) for n, x in zip(m_steel_craft.defName, f["resid"])}}
    # stuff effect: dps ratio vs steel per stuff (median over weapons)
    piv = ms.pivot_table(index="defName", columns="stuff", values="dps")
    ratio = piv.div(piv["Steel"], axis=0)
    fits["melee_stuff_dps_ratio_median"] = ratio.median().round(3).dropna().to_dict()
    fits["melee_stuff_dps_ratio_range"] = {s: [round(float(ratio[s].min()), 3), round(float(ratio[s].max()), 3)] for s in ratio.columns if ratio[s].notna().any()}
    # all weapon x stuff rows: log(mv) ~ log(dps) + log(stuff_count)
    allm = ms[ms.stuffed & (ms.mv > 0)]
    f = fit_ols(np.column_stack([np.log(allm.dps.to_numpy(float)), np.log(allm.stuff_count.to_numpy(float))]), np.log(allm.mv.to_numpy(float)))
    fits["melee_mv_on_dps_count_allstuffs"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}

    f = fit_ols(np.log(m_steel_craft.sel_cooldown.to_numpy(float)).reshape(-1, 1), np.log(m_steel_craft.sel_damage.to_numpy(float)))
    fits["melee_seldamage_on_selcooldown_craftable"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"], "note": "ln(damage per swing) = b0 + b1 ln(cooldown)"}
    f = fit_ols(m_steel_craft.sel_cooldown.to_numpy(float).reshape(-1, 1), m_steel_craft.sel_damage.to_numpy(float))
    fits["melee_seldamage_linear_on_selcooldown_craftable"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}
    f = fit_ols(np.log(m_steel_craft.stuff_count.to_numpy(float)).reshape(-1, 1), np.log(m_steel_craft.sel_damage.to_numpy(float)))
    fits["melee_seldamage_on_stuffcount_craftable"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}
    f = fit_ols(np.log(m_steel_craft.mass.to_numpy(float)).reshape(-1, 1), np.log(m_steel_craft.stuff_count.to_numpy(float)))
    fits["melee_stuffcount_on_mass_craftable"] = {"beta": f["beta"], "r2": f["r2"], "n": f["n"]}
    ult = m_steel[~m_steel.stuffed]
    fits["melee_ultra_vs_craftable_dps_select"] = {"craftable_median": float(m_steel_craft.dps_select.median()), "craftable_min_max": [float(m_steel_craft.dps_select.min()), float(m_steel_craft.dps_select.max())],
                                                   "ultra_median": float(ult.dps_select.median()), "ultra_min_max": [float(ult.dps_select.min()), float(ult.dps_select.max())]}

    # ---- group statistics
    rcols = ["damage", "ap", "dps_nominal", "dps_eff_mid", "power", "cooldown", "warmup", "range", "mass", "work", "mv", "burst", "acc_mid"]
    rs = stats_table(std, ["tech"], rcols)
    rs2 = stats_table(std, ["role"], rcols)
    rs["by"] = "tech"
    rs2["by"] = "role"
    rs = pd.concat([rs.rename(columns={"tech": "key"}), rs2.rename(columns={"role": "key"})])
    rs.to_csv(D / "stats_ranged.csv", index=False, float_format="%.4g")
    mcols = ["avg_damage", "dps", "dps_select", "avg_ap", "sel_ap", "best_dps", "avg_cooldown", "mass", "work", "mv", "stuff_count"]
    ms_stats = stats_table(m_steel, ["tech"], mcols).rename(columns={"tech": "key"})
    ms_stats.to_csv(D / "stats_melee.csv", index=False, float_format="%.4g")

    # ---- backtest
    bt = {}
    std_bt = std.copy()
    bt["ranged_direct"] = {"n": int(len(std_bt)), "class_column": "role",
                           "targets": backtest(std_bt, ["damage", "dps_nominal", "cooldown", "range", "mass", "mv", "work", "ap"], "role")}
    mb = m_steel[m_steel.stuffed].copy()
    mb["power"] = mb.PM
    mb["cls"] = np.where(mb.weapon_classes.str.contains("MeleeBlunt"), "blunt", "sharp")
    bt["melee_craftable_steel"] = {"n": int(len(mb)), "class_column": "cls",
                                   "targets": backtest(mb, ["avg_damage", "avg_cooldown", "dps", "mass", "mv", "work", "stuff_count"], "cls", min_group=3)}
    (D / "backtest.json").write_text(json.dumps(bt, indent=1, default=float))

    # ---- archetype clustering (ranged direct fire): k-means with numpy on standardized shape features
    feats = std[["damage", "cycle", "range", "acc_mid", "burst", "ap"]].copy()
    feats["damage"] = np.log(feats.damage)
    feats["cycle"] = np.log(feats.cycle)
    feats["burst"] = np.log(feats.burst)
    Z = ((feats - feats.mean()) / feats.std()).to_numpy(float)
    best = None
    for seed in range(20):
        rng = np.random.default_rng(seed)
        k = 4
        cent = Z[rng.choice(len(Z), k, replace=False)]
        for _ in range(60):
            lab = np.argmin(((Z[:, None, :] - cent[None]) ** 2).sum(-1), axis=1)
            cent = np.array([Z[lab == j].mean(0) if (lab == j).any() else cent[j] for j in range(k)])
        inertia = float(((Z - cent[lab]) ** 2).sum())
        if best is None or inertia < best[0]:
            best = (inertia, lab.copy())
    std["cluster"] = best[1]
    fits["ranged_clusters_k4"] = {int(c): g.defName.tolist() for c, g in std.groupby("cluster")}
    std.to_csv(D / "ranged_direct_fit_set.csv", index=False, float_format="%.4g")

    ex = {"ranged": {}, "melee": {}}
    qual = json.loads((D / "quality_factors.json").read_text())
    for n in ["Gun_Revolver", "Gun_AssaultRifle", "Gun_PumpShotgun", "Gun_SniperRifle", "Gun_Minigun", "Bow_Recurve", "Gun_ChargeRifle", "Weapon_GrenadeFrag"]:
        r = rd[rd.defName == n].iloc[0].to_dict()
        acc = {k: r.get("acc_" + k) for k in RANGES}
        e = {k: r[k] for k in ("damage", "ap", "burst", "warmup", "cooldown", "ticks_between", "cycle", "range", "dps_nominal", "mv", "mv_formula", "radius")}
        if r["has_acc"]:
            e["hit"] = {str(d): wlib.weapon_accuracy(acc, d) for d in (3, 8, 12, 18, 25, 33, 40)}
            e["dps_at"] = {str(d): r["dps_nominal"] * wlib.weapon_accuracy(acc, d) for d in (3, 12, 25, 40)}
        e["armor_mult"] = {k: wlib.armor_multiplier(a, r["ap"]) for k, a in ARMOR_REFS.items()}
        e["dps_vs_armor_at_12"] = {k: r["dps_nominal"] * (wlib.weapon_accuracy(acc, 12) if r["has_acc"] else 1) * wlib.armor_multiplier(a, r["ap"]) for k, a in ARMOR_REFS.items()}
        qd = qual["RangedWeapon_DamageMultiplier"]; qa = qual["RangedWeapon_ArmorPenetrationMultiplier"]
        e["legendary"] = {"damage": int(round(r["damage"] * qd["factorLegendary"] + 1e-9)), "ap": r["ap"] * qa["factorLegendary"],
                          "acc_touch_short_medium_long": [min(1.0, (acc[k] or 0) * qual["AccuracyTouch"]["factorLegendary"]) for k in RANGES]}
        ex["ranged"][n] = e
    for n in ["MeleeWeapon_Knife", "MeleeWeapon_Club", "MeleeWeapon_Mace", "MeleeWeapon_LongSword", "MeleeWeapon_Spear", "MeleeWeapon_Warhammer", "MeleeWeapon_MonoSword", "MeleeWeapon_PlasmaSword"]:
        w = [x for x in M["weapons"] if x["defName"] == n][0]
        kind_of = {cap: v[0]["armorCategory"] for cap, v in man.items()}
        out_ = {}
        for label, st, dm in (("steel", "Steel", 1.0), ("plasteel", "Plasteel", 1.0), ("steel_legendary", "Steel", qual["MeleeWeapon_DamageMultiplier"]["factorLegendary"])):
            if not w["cost"]["costStuffCount"] and st != "Steel":
                continue
            sd = stuffs[st] if w["cost"]["costStuffCount"] else None
            mults = {"Sharp": sd["sharp_damage_mult"], "Blunt": sd["blunt_damage_mult"]} if sd else None
            cdm = (sd["stat_factors"].get("MeleeWeapon_CooldownMultiplier", 1.0) if sd else 1.0)
            ents = wlib.melee_tool_entries(w["tools"], kind_of, mults, dm, cdm)
            out_[label] = {"entries": ents, **wlib.melee_averages(ents), **wlib.melee_selection_dps(ents)}
        ex["melee"][n] = out_
    (D / "worked_examples.json").write_text(json.dumps(ex, indent=1, default=float))
    (D / "fits.json").write_text(json.dumps(fits, indent=1, default=float))
    print("ranged rows", len(rd), "direct fit set", len(std), "melee", len(md), "stuff rows", len(ms))
    figures(D, rd, std, md, ms, fits)


def figures(D, rd, std, md, ms, fits):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    col = {0: "#8c6d31", 2: "#3b6ea5", 3: "#a23b72", 4: "#2a9d8f", 1: "#999999"}
    # 1 ranged: MV vs power
    fig, ax = plt.subplots(figsize=(6.4, 4.4))
    for t, g in std.groupby("tier"):
        ax.scatter(g.power, g.mv, c=col[int(t)], label=g.tech.iloc[0], s=36)
    for r in std.itertuples():
        ax.annotate(r.defName.replace("Gun_", ""), (r.power, r.mv), fontsize=6, xytext=(3, 2), textcoords="offset points")
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_xlabel("power index P4 (armor and accuracy adjusted DPS x sqrt(range/25))"); ax.set_ylabel("market value (silver)")
    ax.legend(title="tech"); ax.set_title("Ranged direct-fire weapons: value vs power index")
    fig.tight_layout(); fig.savefig(D / "fig_ranged_value_vs_power.png", dpi=100); plt.close(fig)
    # 2 ranged: dps vs distance
    fig, ax = plt.subplots(figsize=(6.4, 4.2))
    dist = np.linspace(1, 45, 90)
    for n in ["Gun_Revolver", "Gun_AssaultRifle", "Gun_PumpShotgun", "Gun_SniperRifle", "Gun_Minigun", "Gun_LMG", "Bow_Recurve", "Gun_ChargeRifle"]:
        r = rd[rd.defName == n].iloc[0]
        acc = {k: r["acc_" + k] for k in RANGES}
        ax.plot(dist, [r.dps_nominal * wlib.weapon_accuracy(acc, d) if d <= r.range else np.nan for d in dist], label=n.replace("Gun_", ""))
    ax.set_xlabel("distance (tiles)"); ax.set_ylabel("expected damage per second (weapon accuracy only)")
    ax.legend(fontsize=7, ncol=2); ax.set_title("Hit-adjusted DPS against distance, up to weapon range")
    fig.tight_layout(); fig.savefig(D / "fig_ranged_dps_vs_distance.png", dpi=100); plt.close(fig)
    # 3 melee: mv vs dps (steel)
    m = md[md.stuffed]
    fig, ax = plt.subplots(figsize=(6.0, 4.2))
    ax.scatter(m.dps, m.mv, c=[col[int(t)] for t in m.tier], s=40)
    for r in m.itertuples():
        ax.annotate(r.defName.replace("MeleeWeapon_", ""), (r.dps, r.mv), fontsize=7, xytext=(3, 2), textcoords="offset points")
    ax.set_xlabel("game melee DPS (steel, normal quality)"); ax.set_ylabel("market value with steel (silver)")
    ax.set_title("Craftable melee weapons: value vs DPS")
    fig.tight_layout(); fig.savefig(D / "fig_melee_value_vs_dps.png", dpi=100); plt.close(fig)
    # 4 melee: stuff effect
    fig, ax = plt.subplots(figsize=(6.4, 4.2))
    piv = ms.pivot_table(index="defName", columns="stuff", values="dps")
    cols = [c for c in ["WoodLog", "Steel", "Gold", "Silver", "Uranium", "Plasteel", "Obsidian", "Bioferrite", "Jade", "BlocksGranite"] if c in piv.columns]
    for n in ["MeleeWeapon_Knife", "MeleeWeapon_Mace", "MeleeWeapon_LongSword", "MeleeWeapon_Spear", "MeleeWeapon_Warhammer"]:
        if n in piv.index:
            ax.plot(cols, [piv.loc[n, c] for c in cols], marker="o", label=n.replace("MeleeWeapon_", ""))
    ax.set_ylabel("game melee DPS"); ax.legend(fontsize=7); ax.tick_params(axis="x", rotation=40)
    ax.set_title("Effect of the weapon material on melee DPS")
    fig.tight_layout(); fig.savefig(D / "fig_melee_stuff_effect.png", dpi=100); plt.close(fig)


if __name__ == "__main__":
    main()
