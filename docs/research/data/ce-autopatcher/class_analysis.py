#!/usr/bin/env python3
"""Does knowing the weapon archetype fix the simple formula? Reads fidelity_guns.csv, writes class_analysis.json.
Usage: class_analysis.py DATA_DIR"""
import csv, json, sys
from pathlib import Path
import numpy as np

D = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
rows = [r for r in csv.DictReader(open(D / "fidelity_guns.csv")) if r["firearm_subset"] == "True"]
n = len(rows)
num = lambda x: float(x) if x not in ("", None, "None") else np.nan
FE = ["mass", "range", "warmup", "cooldown", "burst", "damage", "speed"]
LG = [0.01, 1, 0.1, 0.05, 1, 1, 1]
X = np.array([[num(r["vanilla_" + k]) for k in FE] for r in rows])
for j in range(X.shape[1]):
    X[np.isnan(X[:, j]), j] = np.nanmedian(X[:, j])
    X[:, j] = np.log(X[:, j] + LG[j])
sd = X.std(0); sd[sd == 0] = 1
Z = (X - X.mean(0)) / sd
cls = [r["joint_best_preset"] for r in rows]
TARGETS = ["mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "magazine", "reload", "burst"]
Y = {t: np.array([num(r["ce_" + t]) for r in rows]) for t in TARGETS}
out = {"n": n, "class_counts": {c: cls.count(c) for c in sorted(set(cls))}}

def ape(p, t):
    ok = ~(np.isnan(p) | np.isnan(t)) & (np.abs(t) > 1e-9)
    return np.abs(p[ok] - t[ok]) / np.abs(t[ok])

# (1) class known: leave-one-out class median of hand-tuned CE values
res = {}
for t in TARGETS:
    p = np.full(n, np.nan)
    for i in range(n):
        same = [j for j in range(n) if j != i and cls[j] == cls[i] and not np.isnan(Y[t][j])]
        if same:
            p[i] = np.median(Y[t][same])
    a = ape(p, Y[t])
    res[t] = {"median_ape": float(np.median(a)), "within20pct": float(np.mean(a <= .2)), "n_with_peers": int(len(a))}
out["class_known_loo_class_median"] = res

# (2) class inferred: leave-one-out nearest neighbour classification of the archetype from vanilla features
acc = []
for k in (1, 3):
    ok = 0
    for i in range(n):
        d = np.sqrt(((Z - Z[i]) ** 2).sum(1)); d[i] = 1e9
        idx = np.argsort(d)[:k]
        votes = {}
        for j in idx:
            votes[cls[j]] = votes.get(cls[j], 0) + 1.0 / (d[j] + 1e-6)
        ok += max(votes, key=votes.get) == cls[i]
    acc.append(ok / n)
out["archetype_inference_loo_accuracy"] = {"knn1": acc[0], "knn3": acc[1], "shipped_classifier_agreement": float(np.mean([r["preset"] == r["joint_best_preset"] for r in rows]))}

# (3) feature ablation for 3-NN regression: mean over targets of median relative error when one vanilla feature is removed
def knn_err(cols):
    errs = []
    for t in TARGETS:
        y = Y[t]; p = np.full(n, np.nan)
        for i in range(n):
            if np.isnan(y[i]): continue
            d = np.sqrt(((Z[:, cols] - Z[i, cols]) ** 2).sum(1)); d[i] = 1e9
            order = [j for j in np.argsort(d) if not np.isnan(y[j])][:3]
            w = 1 / (d[order] + 1e-6)
            p[i] = (w * y[order]).sum() / w.sum()
        errs.append(np.median(ape(p, y)))
    return float(np.mean(errs))
base = knn_err(list(range(len(FE))))
out["feature_ablation_3nn"] = {"all_features_mean_median_ape": base,
                               "without": {FE[j]: knn_err([c for c in range(len(FE)) if c != j]) for j in range(len(FE))}}
# (4) a one-number rule: CE mass equals vanilla mass; ratio statistics
ratio = np.array([num(r["ce_mass"]) / num(r["vanilla_mass"]) for r in rows])
out["mass_ratio_ce_over_vanilla"] = {"median": float(np.nanmedian(ratio)), "p10": float(np.nanquantile(ratio, .1)), "p90": float(np.nanquantile(ratio, .9))}
# (5) user picks ONE reference gun among the other hand-tuned guns. Oracle pick = the reference whose copied stats are jointly closest
# to the truth (an upper bound on what a perfect human choice gives); nearest pick = closest in vanilla feature space (what an
# automatic suggestion gives). Prediction = copy of the reference's CE value, or for mass and warmup the value scaled by the ratio
# of the vanilla values.
JT = ["mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "magazine", "reload"]
vm = np.array([num(r["vanilla_mass"]) for r in rows]); vw = np.array([num(r["vanilla_warmup"]) for r in rows])
def joint(i, j):
    e = []
    for t in JT:
        a_, b_ = Y[t][i], Y[t][j]
        if not (np.isnan(a_) or np.isnan(b_)) and abs(a_) > 1e-9:
            e.append(abs(b_ - a_) / abs(a_))
    return np.mean(e) if e else 9
ref = {"oracle": {}, "nearest": {}}
for mode in ref:
    pred = {t: np.full(n, np.nan) for t in TARGETS}
    pred_scaled = {"mass": np.full(n, np.nan), "warmup": np.full(n, np.nan)}
    for i in range(n):
        others = [j for j in range(n) if j != i]
        if mode == "oracle":
            j = min(others, key=lambda j: joint(i, j))
        else:
            d = np.sqrt(((Z - Z[i]) ** 2).sum(1)); d[i] = 1e9
            j = int(np.argmin(d))
        for t in TARGETS:
            pred[t][i] = Y[t][j]
        pred_scaled["mass"][i] = Y["mass"][j] * vm[i] / vm[j]
        if vw[j] > 0 and not np.isnan(vw[i]):
            pred_scaled["warmup"][i] = Y["warmup"][j] * max(vw[i], 0.05) / vw[j]
    ref[mode] = {t: {"median_ape": float(np.median(ape(pred[t], Y[t]))), "within20pct": float(np.mean(ape(pred[t], Y[t]) <= .2))} for t in TARGETS}
    ref[mode]["mass_scaled_by_vanilla_ratio"] = {"median_ape": float(np.median(ape(pred_scaled["mass"], Y["mass"])))}
    ref[mode]["warmup_scaled_by_vanilla_ratio"] = {"median_ape": float(np.nanmedian(ape(pred_scaled["warmup"], Y["warmup"])))}
out["single_reference_copy"] = ref
(D / "class_analysis.json").write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
print(json.dumps(out, indent=0)[:2600])
