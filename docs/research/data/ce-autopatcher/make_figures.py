#!/usr/bin/env python3
"""Figures from fidelity_summary.json and fidelity_guns.csv. Usage: make_figures.py DATA_DIR"""
import csv, json, sys
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

D = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
o = json.loads((D / "fidelity_summary.json").read_text())
rows = [r for r in csv.DictReader(open(D / "fidelity_guns.csv")) if r["firearm_subset"] == "True"]
PAL = ["#4477AA", "#EE6677", "#228833", "#CCBB44", "#66CCEE", "#AA3377"]
plt.rcParams.update({"font.size": 9, "axes.spines.top": False, "axes.spines.right": False, "figure.dpi": 130})

def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return np.nan

# 1 scatter, formula as shipped
stats = ["mass", "range", "cooldown", "magazine", "spread", "sway"]
fig, axs = plt.subplots(2, 3, figsize=(9, 5.6))
presets = sorted({r["preset"] for r in rows})
for ax, s in zip(axs.ravel(), stats):
    for i, p in enumerate(presets):
        sub = [r for r in rows if r["preset"] == p]
        ax.scatter([num(r["ce_" + s]) for r in sub], [num(r["auto_" + s]) for r in sub], s=22, color=PAL[i], label=p, alpha=.9)
    lo = min(num(r["ce_" + s]) for r in rows); hi = max(num(r["ce_" + s]) for r in rows)
    ax.plot([lo, hi], [lo, hi], color="#888", lw=1, ls="--")
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_title(s); ax.set_xlabel("CE hand-tuned"); ax.set_ylabel("formula")
axs[0, 0].legend(fontsize=7, frameon=False, title="preset chosen")
fig.suptitle("Auto-patcher output vs hand-tuned CE values, 21 vanilla firearms (dashed: perfect)")
fig.tight_layout(); fig.savefig(D / "fig1_gun_formula_vs_hand_tuned.png"); plt.close(fig)

# 2 error by method
g = o["guns"]["firearms"]
keys = ["mass", "bulk", "range", "warmup", "cooldown", "spread", "sway", "magazine", "reload"]
meth = [("formula", "formula as shipped", PAL[1]), ("formula_with_joint_best_preset", "formula, right preset", PAL[2]),
        ("global_mean", "class-blind mean", "#BBBBBB"), ("knn3", "3 nearest items", PAL[0]), ("ridge", "ridge regression", PAL[3])]
fig, ax = plt.subplots(figsize=(9, 3.8))
w = 0.16
for j, (k, lab, col) in enumerate(meth):
    vals = [min(g[s][k]["median_ape"], 2.0) if g[s].get(k) and g[s][k]["median_ape"] is not None else 0 for s in keys]
    ax.bar(np.arange(len(keys)) + (j - 2) * w, vals, w, label=lab, color=col)
ax.set_xticks(range(len(keys))); ax.set_xticklabels(keys)
ax.set_ylabel("median relative error (capped at 2)"); ax.legend(fontsize=7, frameon=False, ncol=5, loc="upper center")
ax.set_title("Error against hand-tuned CE values, leave-one-out for data methods, n=21 firearms")
fig.tight_layout(); fig.savefig(D / "fig2_gun_error_by_method.png"); plt.close(fig)

# 3 learning curve
fig, ax = plt.subplots(figsize=(6, 3.8))
for i, s in enumerate(["mass", "bulk", "range", "warmup", "cooldown", "spread", "magazine"]):
    lc = g[s].get("learning_curve_nearest_of_m")
    if lc:
        ms = sorted(int(m) for m in lc)
        ax.plot(ms, [lc[str(m)] for m in ms], marker="o", ms=3, color=PAL[i % 6], ls="-" if i < 6 else "--", label=s)
ax.set_xlabel("hand-tuned reference items available (m)"); ax.set_ylabel("median relative error")
ax.set_title("Calibrated mode: nearest of m random references"); ax.legend(fontsize=7, frameon=False, ncol=2)
fig.tight_layout(); fig.savefig(D / "fig3_calibration_learning_curve.png"); plt.close(fig)

# 4 apparel
ar = list(csv.DictReader(open(D / "fidelity_apparel.csv")))
fig, axs = plt.subplots(1, 2, figsize=(9, 3.6))
matched = [r for r in ar if r["preset"]]
cnt = {"matched by a preset": len(matched), "no preset claims it": len(ar) - len(matched)}
axs[0].bar(list(cnt), list(cnt.values()), color=[PAL[0], "#BBBBBB"])
axs[0].set_title("67 vanilla apparel that CE converts by hand"); axs[0].set_ylabel("items")
for k in ("auto_bulk",):
    x = [num(r["ce_bulk"]) for r in matched]; y = [num(r[k]) for r in matched]
    axs[1].scatter(x, y, color=PAL[1], s=24)
    axs[1].plot([0, 12], [0, 12], color="#888", ls="--", lw=1)
axs[1].set_xlabel("CE hand-tuned Bulk"); axs[1].set_ylabel("formula Bulk"); axs[1].set_title("Bulk, 16 matched items")
fig.tight_layout(); fig.savefig(D / "fig4_apparel_coverage_and_bulk.png"); plt.close(fig)
print("figures written")
