#!/usr/bin/env python3
"""Fidelity of the auto-patcher formulas against CE's hand-tuned values, plus data-driven baselines.

Usage: fidelity.py DATA_DIR      (reads presets_*.json and pairs_*.json in DATA_DIR, writes fidelity_* files there)
Deterministic (fixed seed). Needs numpy and scipy.
"""
import csv, json, sys, statistics as st
from pathlib import Path
import numpy as np
from scipy.stats import spearmanr

sys.path.insert(0, str(Path(__file__).resolve().parent))
import autopatch as AP

D = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
presets_g = json.loads((D / "presets_gun.json").read_text())
presets_a = json.loads((D / "presets_apparel.json").read_text())
guns_all = json.loads((D / "pairs_guns.json").read_text())
app_all = json.loads((D / "pairs_apparel.json").read_text())
mel_all = json.loads((D / "pairs_melee.json").read_text())
RNG = np.random.default_rng(20261004)
out = {}


# ------------------------------------------------------------------ helpers
def metrics(pred, true):
    pred, true = np.asarray(pred, float), np.asarray(true, float)
    ok = ~(np.isnan(pred) | np.isnan(true))
    p, t = pred[ok], true[ok]
    if len(t) == 0:
        return None
    nz = np.abs(t) > 1e-9
    ape = np.abs(p[nz] - t[nz]) / np.abs(t[nz])
    rho = None
    if len(t) >= 4 and np.ptp(p) > 0 and np.ptp(t) > 0:
        rho = float(spearmanr(p, t)[0])
    return {"n": int(len(t)), "mae": float(np.mean(np.abs(p - t))), "median_ape": float(np.median(ape)) if len(ape) else None,
            "p80_ape": float(np.quantile(ape, 0.8)) if len(ape) else None,
            "within20pct": float(np.mean(ape <= 0.2)) if len(ape) else None, "spearman": rho,
            "pred_is_constant": bool(np.ptp(p) == 0)}


def feats_matrix(rows, keys, logs):
    X = np.array([[(r.get(k) if r.get(k) is not None else np.nan) for k in keys] for r in rows], float)
    med = np.nanmedian(X, axis=0)
    for j in range(X.shape[1]):
        X[np.isnan(X[:, j]), j] = med[j]
        if logs[j]:
            X[:, j] = np.log(np.maximum(X[:, j], 0) + logs[j])
    sd = X.std(axis=0)
    sd[sd == 0] = 1
    return (X - X.mean(axis=0)) / sd


def loo_predict(X, y, method, k=3, lam=1.0):
    """Leave-one-out predictions of log(y) (y > 0) by 'mean', 'knn', 'ridge'."""
    n = len(y)
    EPS = 0.05
    ly = np.log(np.asarray(y, float) + EPS)
    pred = np.zeros(n)
    for i in range(n):
        tr = np.arange(n) != i
        if method == "mean":
            pred[i] = ly[tr].mean()
        elif method == "knn":
            d = np.sqrt(((X[tr] - X[i]) ** 2).sum(axis=1))
            idx = np.argsort(d)[:k]
            w = 1.0 / (d[idx] + 1e-6)
            pred[i] = (w * ly[tr][idx]).sum() / w.sum()
        elif method == "ridge":
            A = np.hstack([X[tr], np.ones((tr.sum(), 1))])
            reg = lam * np.eye(A.shape[1])
            reg[-1, -1] = 0
            beta = np.linalg.solve(A.T @ A + reg, A.T @ ly[tr])
            pred[i] = np.append(X[i], 1.0) @ beta
    return np.exp(pred) - EPS


def learning_curve(X, y, ms, trials=60):
    """Calibrated-mode simulation: predict item i from the nearest of m randomly chosen other items."""
    n = len(y)
    res = {}
    for m in ms:
        apes = []
        for i in range(n):
            others = np.array([j for j in range(n) if j != i])
            for _ in range(max(1, trials // 6)):
                pick = RNG.choice(others, size=min(m, len(others)), replace=False)
                d = np.sqrt(((X[pick] - X[i]) ** 2).sum(axis=1))
                p = y[pick[np.argmin(d)]]
                apes.append(abs(p - y[i]) / abs(y[i]))
        res[m] = float(np.median(apes))
    return res


# ------------------------------------------------------------------ guns
def eligible_gun(g):
    """The auto-patcher's own eligibility test on the vanilla def."""
    return (g["thingClass"] in ("ThingWithComps", "Thing") and g["projectileClass"] in ("Bullet", "Projectile_Explosive")
            and g["verbClass"] in ("Verb_ShootOneUse", "Verb_Shoot", "Verb_LaunchProjectile", "Verb_LaunchProjectileStatic")
            and g["nVerbs"] >= 1)


def fmj_damage(ammo):
    return ammo["damage"] if ammo else None


guns = [g for g in guns_all if not g["defName"].endswith("_Unique")]
elig = [g for g in guns if eligible_gun(g)]
paired = [g for g in elig if g["ce"] and g["ce"]["magazine"] is not None]
firearms = [g for g in paired if g["projectileClass"] == "Bullet" and "Neolithic" not in g["weaponTags"]
            and "TurretGun" not in g["weaponTags"] and g["v"]["damage"]]
out["guns_counts"] = {"vanilla_gun_like_defs": len(guns_all), "unique_variants_dropped": len(guns_all) - len(guns),
                      "eligible_for_autopatcher": len(elig), "eligible_with_ce_hand_tuned": len(paired),
                      "firearm_subset": len(firearms),
                      "eligible_without_ce_counterpart": [g["defName"] for g in elig if not g["ce"]]}

STATS = [("mass", "mass"), ("bulk", "bulk"), ("range", "range"), ("warmup", "warmup"), ("cooldown", "cooldown"),
         ("spread", "spread"), ("sway", "sway"), ("recoil", "recoil"), ("magazine", "magazine"), ("reload", "reload"),
         ("burst", "burst"), ("sights", "sights")]

rows = []
for g in paired:
    p, why = AP.classify_gun(g, presets_g)
    a = AP.patch_gun(g, p)
    preds_all = {q["defName"]: AP.patch_gun(g, q) for q in presets_g}
    ce_amm = g["ce"]["ammo"]
    pred_amm = None
    # damage of the first projectile of the predicted ammo set, looked up among CE hand-tuned guns that use that set
    rows.append({"g": g, "preset": p["defName"], "why": why, "a": a, "all": preds_all})

# FMJ damage lookup by ammo set from the hand-tuned data itself
set_dmg = {}
for g in guns:
    if g["ce"] and g["ce"]["ammo"] and g["ce"]["ammoSet"]:
        set_dmg[g["ce"]["ammoSet"]] = g["ce"]["ammo"]["damage"]

JOINT = ["mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "magazine", "reload"]
for r in rows:
    def err(q):
        tot = 0.0
        for k in JOINT:
            t = r["g"]["ce"].get(k)
            if t is None or q.get(k) is None:
                continue
            tot += abs(q[k] - t) / max(abs(t), 1e-9)
        return tot
    r["joint_best"] = min(r["all"], key=lambda n: err(r["all"][n]))
    r["joint_a"] = r["all"][r["joint_best"]]
gun_csv = []
for r in rows:
    g, a, c = r["g"], r["a"], r["g"]["ce"]
    gun_csv.append({"defName": g["defName"], "label": g["label"], "joint_best_preset": r.get("joint_best"), "firearm_subset": g in firearms, "preset": r["preset"],
                    "matched_by": r["why"], **{"vanilla_" + k: g["v"].get(k) for k in ("mass", "range", "warmup", "cooldown", "burst", "damage", "speed")},
                    **{"ce_" + k: c.get(k) for k in ("mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "recoil", "magazine", "reload", "burst", "ammoSet")},
                    **{"auto_" + k: a.get(k) for k in ("mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "recoil", "magazine", "reload", "burst", "ammoSet")}})
with open(D / "fidelity_guns.csv", "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=list(gun_csv[0].keys()))
    w.writeheader()
    w.writerows(gun_csv)

FEATS = ["mass", "range", "warmup", "cooldown", "burst", "damage", "speed"]
LOGS = [0.01, 1, 0.1, 0.05, 1, 1, 1]
gun_res = {}
for subset_name, subset in (("firearms", firearms), ("all_eligible_paired", paired)):
    sel = [r for r in rows if r["g"] in subset]
    X = feats_matrix([r["g"]["v"] for r in sel], FEATS, LOGS)
    res = {}
    for name, key in STATS:
        idx = [i for i, r in enumerate(sel) if r["g"]["ce"].get(key) is not None and r["a"].get(key) is not None]
        if len(idx) < 6:
            continue
        t = np.array([sel[i]["g"]["ce"][key] for i in idx], float)
        f = np.array([sel[i]["a"][key] for i in idx], float)
        oracle = np.array([min(abs(q[key] - tv) for q in sel[i]["all"].values() if q.get(key) is not None) for i, tv in zip(idx, t)])
        fj = np.array([sel[i]["joint_a"][key] for i in idx], float)
        entry = {"formula": metrics(f, t), "formula_with_joint_best_preset": metrics(fj, t),
                 "oracle_preset_median_ape": float(np.median(oracle / np.maximum(np.abs(t), 1e-9)))}
        if key in ("mass", "warmup", "cooldown", "burst", "range"):
            vv = np.array([(sel[i]["g"]["v"].get(key) or 0.0) for i in idx], float)
            entry["identity_ce_equals_vanilla"] = metrics(vv, t)
        if True:
            Xs = X[idx]
            for m, kw in (("global_mean", dict(method="mean")), ("knn1", dict(method="knn", k=1)), ("knn3", dict(method="knn", k=3)),
                          ("ridge", dict(method="ridge"))):
                entry[m] = metrics(loo_predict(Xs, t, **kw), t)
            entry["learning_curve_nearest_of_m"] = learning_curve(Xs, t, [1, 2, 3, 5, 8]) if subset_name == "firearms" else None
        res[name] = entry
    # categorical: caliber
    cal_ok = [r for r in sel if r["g"]["ce"]["ammoSet"]]
    exact = np.mean([r["a"]["ammoSet"] == r["g"]["ce"]["ammoSet"] for r in cal_ok])
    dm_t = np.array([set_dmg.get(r["g"]["ce"]["ammoSet"], np.nan) for r in cal_ok], float)
    dm_p = np.array([set_dmg.get(r["a"]["ammoSet"], np.nan) for r in cal_ok], float)
    res["caliber_exact_match_rate"] = float(exact)
    res["caliber_fmj_damage"] = metrics(dm_p, dm_t)
    pre = {}
    for r in sel:
        pre.setdefault(r["preset"], []).append(r["g"]["defName"])
    res["preset_assignment"] = pre
    res["joint_best_assignment"] = {k: sum(1 for r in sel if r["joint_best"] == k) for k in sorted({r["joint_best"] for r in sel})}
    res["classifier_agrees_with_joint_best"] = float(np.mean([r["preset"] == r["joint_best"] for r in sel]))
    res["matched_by"] = {k: sum(1 for r in sel if r["why"] == k) for k in sorted({r["why"] for r in sel})}
    gun_res[subset_name] = res
out["guns"] = gun_res

# ------------------------------------------------------------------ apparel
app = [a for a in app_all if a["ce"] and a["ce"]["bulk"] is not None]
arows = []
for a in app:
    p = AP.classify_apparel(a, presets_a)
    pr = AP.patch_apparel(a, p) if p else None
    ce = a["ce"]
    ce_sharp = ce["sharp"] if ce["sharp"] is not None else ce["sema"]
    ce_blunt = ce["blunt"] if ce["blunt"] is not None else ce["sema"]
    arows.append({"a": a, "p": p["defName"] if p else None, "pr": pr, "ce_sharp": ce_sharp, "ce_blunt": ce_blunt,
                  "regime": "direct" if (a["v"]["sharp"] is not None and ce["sharp"] is not None) else
                            ("stuff_to_stuff" if (ce["sema"] is not None and a["v"]["sema"] is not None) else "mixed")})
out["apparel_counts"] = {"vanilla_apparel": len(app_all), "ce_converted_with_bulk": len(app),
                         "formula_matches_a_preset": sum(1 for r in arows if r["p"]),
                         "regimes": {k: sum(1 for r in arows if r["regime"] == k) for k in ("direct", "stuff_to_stuff", "mixed")},
                         "unmatched": [r["a"]["defName"] for r in arows if not r["p"]]}
with open(D / "fidelity_apparel.csv", "w", newline="") as fh:
    cols = ["defName", "layers", "regime", "preset", "v_sharp", "v_blunt", "v_sema", "v_mass", "ce_sharp", "ce_blunt", "ce_bulk",
            "ce_wornBulk", "ce_mass", "auto_sharp", "auto_blunt", "auto_bulk", "auto_wornBulk", "auto_mass"]
    w = csv.writer(fh)
    w.writerow(cols)
    for r in arows:
        a, ce, pr = r["a"], r["a"]["ce"], r["pr"] or {}
        w.writerow([a["defName"], "+".join(a["layers"]), r["regime"], r["p"], a["v"]["sharp"], a["v"]["blunt"], a["v"]["sema"],
                    a["v"]["mass"], r["ce_sharp"], r["ce_blunt"], ce["bulk"], ce["wornBulk"], ce["mass"], pr.get("sharp"),
                    pr.get("blunt"), pr.get("bulk"), pr.get("wornBulk"), pr.get("mass")])

AF = ["mass", "sharp", "blunt", "sema"]
ap_res = {}
for regime in ("direct", "stuff_to_stuff", "all"):
    sel = [r for r in arows if r["pr"] and (regime == "all" or r["regime"] == regime)]
    if len(sel) < 6:
        continue
    res = {}
    pairs = {"sharp": ("ce_sharp", "sharp"), "blunt": ("ce_blunt", "blunt"), "bulk": (None, "bulk"), "wornBulk": (None, "wornBulk"),
             "mass": (None, "mass")}
    for name, (ck, pk) in pairs.items():
        t = np.array([(r[ck] if ck else r["a"]["ce"][name]) for r in sel], float)
        f = np.array([r["pr"][pk] for r in sel], float)
        res[name] = {"formula": metrics(f, t)}
        if name == "mass":
            res[name]["identity_ce_equals_vanilla"] = metrics([r["a"]["v"]["mass"] or 0.0 for r in sel], t)
    # data-driven baselines for bulk, wornBulk, mass, sharp, blunt using vanilla features
    vrows = []
    for r in sel:
        v = r["a"]["v"]
        vrows.append({"mass": v["mass"], "sharp": v["sharp"] or 0.0, "blunt": v["blunt"] or 0.0, "sema": v["sema"] or 0.0,
                      "nlayers": len(r["a"]["layers"]), "ngroups": len(r["a"]["groups"]),
                      "overhead": 1.0 if "Overhead" in r["a"]["layers"] else 0.0, "shell": 1.0 if "Shell" in r["a"]["layers"] else 0.0,
                      "middle": 1.0 if "Middle" in r["a"]["layers"] else 0.0})
    X = feats_matrix(vrows, ["mass", "sharp", "blunt", "sema", "nlayers", "ngroups", "overhead", "shell", "middle"],
                     [0.05, 0.01, 0.01, 0.01, 1, 1, 1, 1, 1])
    for name, (ck, pk) in pairs.items():
        t = np.array([(r[ck] if ck else r["a"]["ce"][name]) for r in sel], float)
        if True:
            for m, kw in (("global_mean", dict(method="mean")), ("knn1", dict(method="knn", k=1)), ("knn3", dict(method="knn", k=3)),
                          ("ridge", dict(method="ridge", lam=3.0))):
                res[name][m] = metrics(loo_predict(X, t, **kw), t)
    ap_res[regime] = {"n": len(sel), "stats": res}
vr = []
for a in app:
    v = a["v"]
    vr.append({"mass": v["mass"], "sharp": v["sharp"] or 0.0, "blunt": v["blunt"] or 0.0, "sema": v["sema"] or 0.0,
               "nlayers": len(a["layers"]), "ngroups": len(a["groups"]), "overhead": 1.0 if "Overhead" in a["layers"] else 0.0,
               "shell": 1.0 if "Shell" in a["layers"] else 0.0, "middle": 1.0 if "Middle" in a["layers"] else 0.0})
XA = feats_matrix(vr, ["mass", "sharp", "blunt", "sema", "nlayers", "ngroups", "overhead", "shell", "middle"],
                  [0.05, 0.01, 0.01, 0.01, 1, 1, 1, 1, 1])
allres = {}
for name in ("bulk", "wornBulk", "mass"):
    t = np.array([a["ce"][name] for a in app], float)
    allres[name] = {"identity_ce_equals_vanilla": metrics([(a["v"]["mass"] or 0.0) for a in app], t)} if name == "mass" else {}
    allres[name].update({m: metrics(loo_predict(XA, t, **kw), t) for m, kw in (("global_mean", dict(method="mean")),
                    ("knn1", dict(method="knn", k=1)), ("knn3", dict(method="knn", k=3)), ("ridge", dict(method="ridge", lam=3.0)))})
    lc = learning_curve(XA, t, [1, 2, 3, 5, 8, 12])
    allres[name]["learning_curve_nearest_of_m"] = lc
ap_res["all_converted_baselines_only"] = {"n": len(app), "stats": allres}
out["apparel"] = ap_res

# ------------------------------------------------------------------ melee tools and toughness
trows = []
for m in mel_all:
    if not m["ceTools"]:
        continue
    cet = {t["label"]: t for t in m["ceTools"]}
    for vt in m["vTools"]:
        c = cet.get(vt["label"])
        if not c:
            continue
        a = AP.convert_tool(vt)
        trows.append({"weapon": m["defName"], "tool": vt["label"], "v": vt, "ce": c, "auto": a})
mt = {}
for key, vk, ck in (("power", "power", "power"), ("cooldown", "cooldown", "cooldown"), ("apSharp", "apSharp", "apSharp"), ("apBlunt", "apBlunt", "apBlunt")):
    t = [r["ce"][ck] for r in trows]
    f = [r["auto"][vk] for r in trows]
    mt[key] = metrics([x if x is not None else np.nan for x in f], [x if x is not None else np.nan for x in t])
out["melee_tools"] = {"n_tools": len(trows), "convert_tool_vs_hand_tuned": mt,
                      "power_changed_by_ce_share": float(np.mean([abs((r["ce"]["power"] or 0) - (r["v"]["power"] or 0)) > 1e-9 for r in trows])),
                      "cooldown_changed_by_ce_share": float(np.mean([abs((r["ce"]["cooldown"] or 0) - (r["v"]["cooldown"] or 0)) > 1e-9 for r in trows]))}
with open(D / "fidelity_melee_tools.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["weapon", "tool", "v_power", "v_cooldown", "v_ap", "ce_power", "ce_cooldown", "ce_apSharp", "ce_apBlunt", "auto_apSharp", "auto_apBlunt"])
    for r in trows:
        w.writerow([r["weapon"], r["tool"], r["v"]["power"], r["v"]["cooldown"], r["v"]["ap"], r["ce"]["power"], r["ce"]["cooldown"],
                    r["ce"]["apSharp"], r["ce"]["apBlunt"], r["auto"]["apSharp"], r["auto"]["apBlunt"]])

tough = []
for m in mel_all:
    ce = m["ce"] or {}
    if ce.get("stuffToughness") is None or ce.get("bulk") is None:
        continue
    caps = [c for t in m["vTools"] for c in t["capacities"]]
    f = AP.stuff_toughness_multiplier(ce["bulk"], m["techLevel"], m["ranged"], caps)
    tough.append({"weapon": m["defName"], "bulk": ce["bulk"], "tech": m["techLevel"], "formula": f, "ce": ce["stuffToughness"]})
out["toughness"] = {"n": len(tough), "formula_vs_hand_tuned": metrics([x["formula"] for x in tough], [x["ce"] for x in tough]) if tough else None,
                    "rows": tough}
(D / "fidelity_summary.json").write_text(json.dumps(out, indent=1, sort_keys=True, default=float) + "\n")
print(json.dumps(out["guns_counts"], indent=0)[:900])
print(json.dumps(out["apparel_counts"], indent=0)[:900])
print("melee tools", out["melee_tools"]["n_tools"], "toughness n", out["toughness"]["n"])
