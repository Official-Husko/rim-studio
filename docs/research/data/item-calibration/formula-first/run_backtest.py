"""Leave-one-out backtest of the formula-first baseline. Deterministic. Writes results.json and results_table.md.

Run: PYTHONDONTWRITEBYTECODE=1 python3 run_backtest.py
"""
import json
from collections import Counter
from pathlib import Path
import numpy as np
import pandas as pd
import domains as D
import engine as E

OUT = Path(__file__).resolve().parent
ARMS = ["tier_median", "nn_simple", "nn_quiz", "template", "simple", "quiz", "quiz_noisy", "quiz_nocouple", "quiz_twin", "quiz_thin_half"]


def rel(p, a):
    if a is None or np.isnan(a) or a == 0 or p is None or np.isnan(p):
        return None
    return abs(p - a) / abs(a)


def summarize(errs):
    out = {}
    for st, v in errs.items():
        v = [x for x in v if x is not None]
        if len(v) >= 3:
            out[st] = {"n": len(v), "median": round(float(np.median(v)), 3), "p80": round(float(np.quantile(v, 0.8)), 3)}
    return out


def run_domain(dom, mode, ammo, di):
    df = dom.df
    errs = {a: {s: [] for s in dom.stats} for a in ARMS}
    per_item = {s: [] for s in dom.stats}      # quiz arm errors for the band coverage check
    levels, asked, meters = Counter(), {"simple": [], "quiz": [], "quiz_noisy": []}, []
    for i in range(len(df)):
        row = df.iloc[i]
        train = df.drop(df.index[i])
        if dom.twin_keys:                      # a new item has no identical sibling in the install
            same = np.ones(len(train), dtype=bool)
            for k in dom.twin_keys:
                a, b = train[k].values.astype(float), float(row[k])
                same &= (np.isclose(a, b, equal_nan=True))
            train = train[~same]
        def add(arm, pred):
            for s in dom.stats:
                errs[arm][s].append(rel(pred.get(s), row[s]))
        seeds = {a: np.random.default_rng(10_000 * di + 31 * i + k) for k, a in enumerate(["simple", "quiz", "quiz_noisy", "thin"])}
        a_t = E.simulate_answers(dom, row, train, "template", seeds["simple"], ammo)
        a_s = E.simulate_answers(dom, row, train, "simple", seeds["simple"], ammo)
        a_q = E.simulate_answers(dom, row, train, "quiz", seeds["quiz"], ammo)
        a_n = E.simulate_answers(dom, row, train, "quiz_noisy", seeds["quiz_noisy"], ammo)
        add("tier_median", E.tier_median(dom, train, row.tier))
        add("nn_simple", E.nearest_neighbour(dom, train, a_s))
        add("nn_quiz", E.nearest_neighbour(dom, train, a_q, quiz=True))
        add("template", E.predict(dom, train, a_t, ammo))
        add("simple", E.predict(dom, train, a_s, ammo))
        pq = E.predict(dom, train, a_q, ammo)
        add("quiz", pq)
        add("quiz_noisy", E.predict(dom, train, a_n, ammo))
        add("quiz_nocouple", E.predict(dom, train, a_q, ammo, couple=False))
        if dom.twin:
            add("quiz_twin", E.predict(dom, train, a_q, ammo, twin={c: row.get(c) for c, _ in dom.twin.values()}))
        else:
            add("quiz_twin", pq)
        # thin install: half of the remaining items, three repeats, errors pooled
        for rep in range(3):
            idx = seeds["thin"].permutation(len(train))[: max(E.MIN_POOL + 1, len(train) // 2)]
            thin = train.iloc[idx]
            a_h = E.simulate_answers(dom, row, thin, "quiz", seeds["thin"], ammo)
            add("quiz_thin_half", E.predict(dom, thin, a_h, ammo))
        levels[pq["_level"]] += 1
        asked["simple"].append(a_s["asked"]); asked["quiz"].append(a_q["asked"]); asked["quiz_noisy"].append(a_n["asked"])
        for s in dom.stats:
            per_item[s].append(rel(pq.get(s), row[s]))
        peers, _ = E.pool_of(train, row.role, row.tier, row.group)
        meters.append((row.defName if "defName" in row else str(i), E.fit_meter(dom, peers, row)))
    res = {"n_items": len(df), "arms": {a: summarize(errs[a]) for a in ARMS}}
    res["summary"] = {a: {"mean_median_err": round(float(np.mean([v["median"] for v in res["arms"][a].values()])), 3),
                          "mean_p80_err": round(float(np.mean([v["p80"] for v in res["arms"][a].values()])), 3)} for a in ARMS}
    res["pool_levels_quiz"] = dict(levels)
    res["mean_questions"] = {k: round(float(np.mean(v)), 2) for k, v in asked.items()}
    res["max_questions"] = {k: int(max(v)) for k, v in asked.items()}
    # band calibration: is an item inside the p80 band estimated from the other items' errors?
    cov = {}
    for s, v in per_item.items():
        w = np.array([x if x is not None else np.nan for x in v], dtype=float)
        ok = ~np.isnan(w)
        if ok.sum() >= 8:
            hits = [w[j] <= np.quantile(np.delete(w[ok], np.where(np.where(ok)[0] == j)[0]), 0.8) for j in np.where(ok)[0]]
            cov[s] = round(float(np.mean(hits)), 2)
    res["band_p80_coverage_quiz"] = cov
    meters.sort(key=lambda t: (np.isnan(t[1]), t[1]))
    vals = np.array([m for _, m in meters if not np.isnan(m)])
    res["fit_meter_existing_items"] = {"median": round(float(np.median(vals)), 1), "p10": round(float(np.quantile(vals, 0.1)), 1),
                                       "lowest": [(n, round(m, 1)) for n, m in meters[:3]]}
    return res


def table(results):
    lines = []
    for key, r in results.items():
        lines.append(f"### {key} (n = {r['n_items']})\n")
        lines.append("| stat | " + " | ".join(ARMS[:8]) + " |")
        lines.append("| --- | " + " | ".join(["---"] * 8) + " |")
        stats = list(r["arms"]["quiz"].keys())
        for s in stats:
            cells = []
            for a in ARMS[:8]:
                v = r["arms"][a].get(s)
                cells.append(f"{v['median']*100:.0f} / {v['p80']*100:.0f}" if v else "-")
            lines.append(f"| {s} | " + " | ".join(cells) + " |")
        lines.append("| mean | " + " | ".join(f"{r['summary'][a]['mean_median_err']*100:.0f} / {r['summary'][a]['mean_p80_err']*100:.0f}" for a in ARMS[:8]) + " |\n")
    return "\n".join(lines)


def main():
    ammo = D.ammo_table()
    results = {}
    di = 0
    for mode, makers in D.ALL.items():
        for mk in makers:
            di += 1
            dom = mk()
            results[dom.name] = run_domain(dom, mode, ammo, di)
            results[dom.name]["mode"] = mode
    meta = {"method": "leave-one-out, cells are median / p80 relative error in percent in results_table.md",
            "arms": ARMS, "ridge": E.RIDGE, "cov_k": E.COV_K, "min_pool": E.MIN_POOL, "min_spread": E.MIN_SPREAD}
    (OUT / "results.json").write_text(json.dumps({"meta": meta, "domains": results}, indent=1))
    (OUT / "results_table.md").write_text(table(results))
    for k, r in results.items():
        print(k, r["n_items"], {a: (r["summary"][a]["mean_median_err"], r["summary"][a]["mean_p80_err"]) for a in ARMS})


if __name__ == "__main__":
    main()
