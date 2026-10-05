#!/usr/bin/env python3
"""Anchor-first item calibration prototype (research code, numpy + pandas only).

Reads the committed research datasets (resolved items), simulates a modder's
answers from each held-out item's true attributes, predicts its key stats with
(a) simple mode, (b) quiz mode, and compares with two baselines.
Run:  PYTHONDONTWRITEBYTECODE=1 python3 anchor_first.py   (writes results.json)
The product never reads these CSV files: it reads the user's install at runtime.
"""
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd

DATA = Path(__file__).resolve().parents[2]  # docs/research/data
TECH = {"Neolithic": 0, "Medieval": 1, "Industrial": 2, "Spacer": 3, "Ultra": 4, "Archotech": 5}
W_ROLE, W_TIER, W_CAT = 2.0, 0.35, 0.8     # anchor distance weights (chosen after a sweep, see sweep.json)
SAME_BAND = 0.10                            # "about the same" answer if |ln ratio| below this
BUCKET = math.log(1 / 0.85)                 # lower / same / higher buckets for "vs anchor" questions
PRIOR_SD = 0.6
K_ANCHORS = 2


# ---------------------------------------------------------------- pools
def _pool(df, kind, role, tier, lnp, stats, cat=None, extra=None, names=None):
    Y = {s: pd.to_numeric(df[s], errors="coerce").to_numpy(float) for s in stats}
    for s, v in (extra or {}).items():
        Y[s] = np.asarray(v, float)
    return dict(kind=kind, name=np.asarray(names if names is not None else df.index.astype(str)),
                role=np.asarray(role, str), tier=np.asarray(tier, float), lnP=np.asarray(lnp, float),
                cat=np.asarray(cat if cat is not None else -np.ones(len(df)), int), Y=Y)


def cls_vanilla_burst(b):
    return 0 if b <= 1 else 1 if b <= 4 else 2 if b <= 9 else 3


def cls_ce_burst(b):
    return 0 if b <= 1 else 1 if b <= 6 else 2


def load_pools():
    P = {}
    # vanilla ranged: the 19 direct-fire fit weapons
    f = pd.read_csv(DATA / "vanilla-weapons/ranged_direct_fit_set.csv")
    f = f.rename(columns={"ticks_between": "tb"})
    P["vanilla/ranged"] = _pool(
        f, "ranged", f.role, f.tier, np.log(f.power),
        ["damage", "burst", "warmup", "cooldown", "range", "mass", "mv"],
        cat=[cls_vanilla_burst(b) for b in f.burst],
        extra=dict(acc_mid=f.acc_mid, armor=f.armor_mult_avg, tb=f.tb, power=f.power), names=f.defName)
    # vanilla melee: standard group (bladelink variants repeat their base weapons)
    m = pd.read_csv(DATA / "vanilla-weapons/melee_table.csv")
    m = m[(m.group == "standard")].reset_index(drop=True)
    role = ["blunt" if "MeleeBlunt" in str(c) else "blade" for c in m.weapon_classes]
    m = m.rename(columns={"avg_damage": "damage", "avg_cooldown": "cooldown", "avg_ap": "ap"})
    P["vanilla/melee"] = _pool(m, "melee", role, m.tech.map(TECH), np.log(m.dps),
                               ["damage", "cooldown", "ap", "mass", "mv"], names=m.defName)
    # vanilla apparel: all 112 items at their default stuff, Normal quality
    a = pd.read_csv(DATA / "vanilla-apparel/apparel-items.csv")
    P["vanilla/apparel"] = _pool(a, "apparel", a.kind, a.tier.map(TECH), np.log(a.api + 0.05),
                                 ["sharp", "blunt", "heat", "ins_cold", "mass", "market_value"], names=a.defName)
    P["vanilla/apparel"]["Y"]["mv"] = P["vanilla/apparel"]["Y"].pop("market_value")
    # CE ranged: hand-tuned guns (no turrets, grenades, launchers, flamers, uniques)
    r = pd.read_csv(DATA / "ce-dataset/ranged.csv")
    r = r[r.cls.isin(["bow", "machine gun", "rifle", "sniper", "shotgun", "pistol", "smg"]) & ~r.unique]
    r = r[r.Mass.notna() & r.range.notna() & r.cooldown.notna() & r.damage.between(1, 999)].reset_index(drop=True)
    burst = r.burst.fillna(1).clip(lower=1)
    cyc = r.warmup.fillna(1) + r.cooldown + (burst - 1) * r.ticksBetween.fillna(5) / 60
    pel = r.pellets.fillna(1)
    pw = r.damage * pel * burst / cyc * np.sqrt(r.range / 25) * (1 + 0.1 * r.apSharp.fillna(0))
    r["burst"] = burst
    vb = r.van_burst.fillna(1).clip(lower=1)
    vcyc = r.van_warmup + r.van_cooldown
    vpw = r.van_damage * vb / vcyc * np.sqrt(r.van_range / 25)
    ce = _pool(r, "ranged", r.cls, r.tech.map(TECH), np.log(pw),
               ["Mass", "Bulk", "cooldown", "range", "warmup", "spread", "sway", "mag", "reload", "burst"],
               cat=[cls_ce_burst(b) for b in burst], names=r.defName)
    ce["vanP"] = np.log(vpw.to_numpy(float))
    ce["van"] = {s: r["van_" + s].to_numpy(float) for s in ["mass", "range", "warmup", "cooldown"]}
    P["ce/ranged"] = ce
    # CE melee: weapon level aggregate of the tools
    c = pd.read_csv(DATA / "ce-dataset/melee.csv")
    c = c[c.defName.str.startswith("MeleeWeapon_") & ~c.defName.str.contains("Bladelink") & c.Mass.notna()]
    g = c.groupby("defName").agg(damage=("power", "mean"), cooldown=("cooldown", "mean"), Mass=("Mass", "first"),
                                 Bulk=("Bulk", "first"), apSharp=("apSharp", "max"), parryC=("parryC", "first"),
                                 tech=("tech", "first"), cap=("cap", lambda s: s.iloc[0])).reset_index()
    g["apSharp"] = g.apSharp.where(g.apSharp > 0)
    top = c.loc[c.groupby("defName").power.idxmax()].set_index("defName").cap
    role = ["blunt" if top[d] in ("Blunt", "Demolish") else "blade" for d in g.defName]
    P["ce/melee"] = _pool(g, "melee", role, g.tech.map(TECH), np.log(g.damage / g.cooldown),
                          ["damage", "cooldown", "Mass", "Bulk", "apSharp", "parryC"], names=g.defName)
    # CE apparel: all hand-tuned apparel with a mass; armor power from direct rating or thickness
    p = pd.read_csv(DATA / "ce-dataset/apparel.csv")
    p = p[p.Mass.notna() & p.Bulk.notna()].reset_index(drop=True)
    eff = p.sharp.fillna(p.thickness).fillna(0) + 0.15 * p.blunt.fillna(0)
    P["ce/apparel"] = _pool(p, "apparel", p.kind, p.tech.map(TECH), np.log(eff + 0.05),
                            ["Mass", "Bulk", "WornBulk", "sharp", "blunt"], names=p.defName)
    return P


# stats evaluated, which are discrete (copied, not scaled), and which are asked as "vs anchor" questions
CFG = {
    "vanilla/ranged": dict(stats=["damage", "burst", "warmup", "cooldown", "range", "mass", "mv"], discrete=["burst"],
                           asked=["mass", "range"], cat=True, scope="kind", solve="ranged"),
    "vanilla/melee": dict(stats=["damage", "cooldown", "ap", "mass", "mv"], discrete=[], asked=["mass", "cooldown"],
                          cat=False, scope="kind", solve="melee"),
    "vanilla/apparel": dict(stats=["sharp", "blunt", "heat", "ins_cold", "mass", "mv"], discrete=[],
                            asked=["mass", "ins_cold"], cat=False, scope="role", solve=None),
    "ce/ranged": dict(stats=["Mass", "Bulk", "cooldown", "range", "warmup", "spread", "sway", "mag", "reload", "burst"],
                      discrete=["burst", "mag", "reload"], asked=["Mass", "range"], cat=True, scope="kind", solve=None),
    "ce/melee": dict(stats=["damage", "cooldown", "Mass", "Bulk", "apSharp", "parryC"], discrete=[],
                     asked=["Mass", "cooldown"], cat=False, scope="kind", solve="melee"),
    "ce/apparel": dict(stats=["Mass", "Bulk", "WornBulk", "sharp", "blunt"], discrete=[], asked=["Mass", "Bulk"],
                       cat=False, scope="role", solve=None),
}
CONVERT_DRV = {"Mass": "mass", "range": "range", "warmup": "warmup", "cooldown": "cooldown"}


# ---------------------------------------------------------------- model parts
def lg(x):
    with np.errstate(all="ignore"):
        v = np.log(np.asarray(x, float))
    return np.where(np.isfinite(v), v, np.nan)


def wmedian(v, w):
    o = np.argsort(v)
    v, w = v[o], w[o]
    c = np.cumsum(w) / w.sum()
    return v[np.searchsorted(c, 0.5)]


def dist(pool, ti, t_role, t_tier, t_lnp, t_cat, cand, sdp):
    d2 = (W_ROLE * (pool["role"][cand] != t_role)) ** 2
    d2 = d2 + (W_TIER * np.abs(pool["tier"][cand] - t_tier)) ** 2
    d2 = d2 + ((pool["lnP"][cand] - t_lnp) / sdp) ** 2
    if t_cat is not None and t_cat >= 0:
        d2 = d2 + (W_CAT * (pool["cat"][cand] != t_cat)) ** 2
    return np.sqrt(d2)


class Fold:
    """Everything learned from the training items (all but the held-out one)."""

    def __init__(self, pool, cfg, i, drv=None, drop=()):
        self.pool, self.cfg, self.i = pool, cfg, i
        n = len(pool["name"])
        self.tr = np.array([j for j in range(n) if j != i and j not in drop])
        self.sdp = max(float(np.std(pool["lnP"][self.tr])), 0.3)
        self.drv = drv or {s: pool["lnP"] for s in cfg["stats"]}
        self.b = {}
        tr = self.tr
        for s in cfg["stats"]:
            if s in cfg["discrete"]:
                self.b[s] = 0.0
                continue
            ly = lg(pool["Y"][s][tr])
            dv = self.drv[s][tr]
            rl = pool["role"][tr]
            ok = np.isfinite(ly) & np.isfinite(dv)
            x, y = np.zeros(len(tr)), np.zeros(len(tr))
            used = 0
            for r in np.unique(rl):                  # within-role (fixed effect) regression of ln y on ln strength
                sel = ok & (rl == r)
                if sel.sum() >= 2:
                    x[sel] = dv[sel] - dv[sel].mean()
                    y[sel] = ly[sel] - ly[sel].mean()
                    used += int(sel.sum())
            if used < 8:                             # too few role peers: pool all items of the kind
                sel = ok
                x[sel], y[sel] = dv[sel] - dv[sel].mean(), ly[sel] - ly[sel].mean()
            self.b[s] = float(np.clip(np.sum(x * y) / (np.sum(x * x) + 0.5), -0.5, 1.5))
        # multipliers for "vs anchor" buckets: each training item against its own nearest other training item
        self.mult = {}
        rat = {s: [] for s in cfg["asked"]}
        err_a = {s: [] for s in cfg["stats"]}   # nested leave-one-out inside the training set: anchor rule
        err_r = {s: [] for s in cfg["stats"]}   # versus class-median rule
        for t in tr:
            cand = tr[tr != t]
            d = dist(pool, t, pool["role"][t], pool["tier"][t], pool["lnP"][t], pool["cat"][t] if cfg["cat"] else None,
                     cand, self.sdp)
            a = cand[int(np.argmin(d))]
            for s in cfg["asked"]:
                r = lg(pool["Y"][s][t]) - lg(pool["Y"][s][a])
                if np.isfinite(r):
                    rat[s].append(r)
            for s in cfg["stats"]:
                if s in cfg["discrete"]:
                    continue
                yt, ya = pool["Y"][s][t], pool["Y"][s][a]
                if not (np.isfinite(yt) and yt > 0 and np.isfinite(ya) and ya > 0):
                    continue
                rm = role_peers(self, s, pool["role"][t], pool["tier"][t], exclude=t)
                if rm is None:
                    continue
                err_a[s].append(abs(math.log(yt / ya) - self.b[s] * (pool["lnP"][t] - pool["lnP"][a])))
                err_r[s].append(abs(math.log(yt / rm)))
        self.use_class = {s: len(err_a[s]) >= 5 and np.median(err_r[s]) < np.median(err_a[s]) for s in cfg["stats"]}
        for s in cfg["asked"]:
            r = np.array(rat[s])
            m = []
            for k, sel in enumerate([r < -BUCKET, (r >= -BUCKET) & (r <= BUCKET), r > BUCKET]):
                m.append(float(np.exp(np.median(r[sel]))) if sel.sum() >= 2 else math.exp((k - 1) * 0.4))
            self.mult[s] = m
        self.cat_burst = {}
        if cfg["cat"] and "burst" in pool["Y"]:
            for c in np.unique(pool["cat"][tr]):
                self.cat_burst[int(c)] = float(np.median(pool["Y"]["burst"][tr][pool["cat"][tr] == c]))

    # prior strength for simple mode: peers of the same role and tier, else role, else all
    def prior(self, role, tier):
        p, tr = self.pool, self.tr
        for sel in ((p["role"][tr] == role) & (p["tier"][tr] == tier), p["role"][tr] == role, p["tier"][tr] == tier):
            if sel.sum() >= 1:
                return float(np.mean(p["lnP"][tr][sel]))
        return float(np.mean(p["lnP"][tr]))


def comparison_dialogue(fold, role, tier, true_lnp, bmax, sigma, rng, scope_role, same_band=SAME_BAND):
    """Prior-weighted bisection over the power-sorted reference list. Returns (lnP estimate, n questions, pivots)."""
    p, tr = fold.pool, fold.tr
    refs = tr[p["role"][tr] == role] if scope_role and (p["role"][tr] == role).sum() >= 4 else tr
    refs = refs[np.argsort(p["lnP"][refs], kind="stable")]
    L = p["lnP"][refs]
    m = len(L)
    mu = fold.prior(role, tier)
    cdf = lambda x: 0.5 * (1 + math.erf((x - mu) / (PRIOR_SD * math.sqrt(2))))
    lo, hi, nq, same = -1, m, 0, None
    gap = float(np.median(np.diff(L))) if m > 1 else 0.2
    while hi - lo > 1 and nq < bmax:
        flo = cdf(L[lo]) if lo >= 0 else 0.0
        fhi = cdf(L[hi]) if hi < m else 1.0
        target = 0.5 * (flo + fhi)
        k = min(range(lo + 1, hi), key=lambda q: abs(cdf(L[q]) - target))
        perceived = true_lnp + (rng.normal(0, sigma) if sigma > 0 else 0.0)
        nq += 1
        if abs(perceived - L[k]) < same_band:
            same = L[k]
            break
        if perceived > L[k]:
            lo = k
        else:
            hi = k
    if same is not None:
        return float(same), nq
    t = 0.5
    if hi - lo == 1 and 0 <= lo and hi < m and nq < bmax:           # closer-to question
        nq += 1
        perceived = true_lnp + (rng.normal(0, sigma) if sigma > 0 else 0.0)
        t = 0.25 if perceived - L[lo] < L[hi] - perceived else 0.75
    a = L[lo] if lo >= 0 else L[0] - gap
    b = L[hi] if hi < m else L[-1] + gap
    if hi - lo > 1:                                                  # budget ran out: centre of the bracket
        a = L[lo + 1] if lo + 1 < m else a
        b = L[hi - 1] if hi - 1 >= 0 else b
        t = 0.5
    return float(a + t * (b - a)), nq


def role_peers(fold, s, role, tier, exclude=None):
    p, tr = fold.pool, fold.tr
    tr = tr[tr != exclude] if exclude is not None else tr
    v = p["Y"][s][tr]
    for sel in ((p["role"][tr] == role) & (p["tier"][tr] == tier), p["role"][tr] == role):
        vv = v[sel & np.isfinite(v) & (v > 0)]
        if len(vv):
            return float(np.median(vv))
    return None


def predict(fold, pool_cfg_name, role, tier, lnp, cat, ans_bucket, anchors_k, drv_t=None, cat_known=True):
    pool, cfg, tr = fold.pool, fold.cfg, fold.tr
    d_all = dist(pool, None, role, tier, lnp, cat if cat_known else None, tr, fold.sdp)
    out, info = {}, {}
    first = None
    for s in cfg["stats"]:
        Ys = pool["Y"][s][tr]
        ok = np.isfinite(Ys) & (Ys > 0 if s not in ("sharp", "blunt", "heat", "ap", "apSharp") else np.isfinite(Ys))
        if ok.sum() == 0:
            out[s] = np.nan
            continue
        idx = np.where(ok)[0]
        order = idx[np.argsort(d_all[idx], kind="stable")][:anchors_k]
        d = d_all[order]
        w = 1.0 / (d + 0.3)
        y = Ys[order]
        if first is None:
            first = tr[order[0]]
        if s == "burst" and cat in fold.cat_burst and cat_known and cfg["cat"]:
            out[s] = fold.cat_burst[int(cat)]
        elif s in cfg["asked"] and s in ans_bucket and np.isfinite(y[0]) and y[0] > 0:
            out[s] = float(y[0] * fold.mult[s][ans_bucket[s]])
        elif s in cfg["discrete"]:
            out[s] = float(wmedian(y, w))
        elif fold.use_class.get(s) and s not in cfg["asked"] and role_peers(fold, s, role, tier) is not None:
            out[s] = role_peers(fold, s, role, tier)      # a tight class convention: copy the class median
        else:
            dvt = (drv_t[s] if drv_t else lnp)
            dva = (fold.drv[s][tr][order])
            if np.any(y <= 0):                                       # zero ratings: weighted mean in linear space
                out[s] = float(np.sum(w * y) / w.sum())
            else:
                out[s] = float(np.exp(np.sum(w * (np.log(y) + fold.b[s] * (dvt - dva))) / w.sum()))
    # structural solves
    if cfg["solve"] == "ranged":
        o = np.argsort(d_all, kind="stable")[:anchors_k]
        w = 1 / (d_all[o] + 0.3)
        hit = float(np.sum(w * pool["Y"]["acc_mid"][tr][o]) / w.sum())
        arm = float(np.sum(w * pool["Y"]["armor"][tr][o]) / w.sum())
        tb = float(np.sum(w * pool["Y"]["tb"][tr][o]) / w.sum())
        bst = max(out["burst"], 1.0)
        cyc = out["warmup"] + out["cooldown"] + (bst - 1) * tb / 60
        out["damage_ratio"] = out["damage"]
        out["damage"] = float(math.exp(lnp) * cyc / (bst * hit * arm * math.sqrt(out["range"] / 25)))
    elif cfg["solve"] == "melee":
        out["damage_ratio"] = out["damage"]
        out["damage"] = float(math.exp(lnp) * out["cooldown"])
    out["_anchor"] = pool["name"][first] if first is not None else None
    return out


def true_bucket(pool, i, s, anchor_name, sigma, rng):
    a = int(np.where(pool["name"] == anchor_name)[0][0])
    r = lg(pool["Y"][s][i]) - lg(pool["Y"][s][a])
    if not np.isfinite(r):
        return None
    if sigma > 0:
        r += rng.normal(0, sigma * 0.5)
    return 0 if r < -BUCKET else 2 if r > BUCKET else 1


# ---------------------------------------------------------------- methods
def run_method(name, pool, cfg, i, fold, variant, convert=False):
    role, tier = pool["role"][i], pool["tier"][i]
    rng = np.random.default_rng(1000 + i + (variant.get("rep", 0) * 7919))
    sigma = variant.get("sigma", 0.0)
    cat = int(pool["cat"][i]) if cfg["cat"] else -1
    nq = 2  # tier and role are asked as setup taps
    drv_t = None
    if convert:
        lnp = float(pool["vanP"][i])
        drv_t = {s: (math.log(pool["van"][CONVERT_DRV[s]][i]) if s in CONVERT_DRV else lnp) for s in cfg["stats"]}
        nq = 2
    elif variant["bmax"] > 0:
        lnp, k = comparison_dialogue(fold, role, tier, float(pool["lnP"][i]), variant["bmax"], sigma, rng,
                                     cfg["scope"] == "role", variant.get("same", SAME_BAND))
        nq += k
    else:
        lnp = fold.prior(role, tier)
    ans = {}
    if variant.get("asked") and not convert:
        pre = predict(fold, name, role, tier, lnp, cat, {}, variant.get("k", K_ANCHORS), None, variant.get("cat", True))
        for s in cfg["asked"]:
            b = true_bucket(pool, i, s, pre["_anchor"], sigma, rng)
            if b is not None:
                ans[s] = b
        nq += len(ans) + (1 if cfg["cat"] and variant.get("cat", True) else 0)
    use_cat = cfg["cat"] and (variant.get("asked") or convert) and variant.get("cat", True)
    out = predict(fold, name, role, tier, lnp, cat, ans, variant.get("k", K_ANCHORS), drv_t, use_cat)
    out["_nq"] = nq
    out["_lnp"] = lnp
    return out


def baselines(pool, cfg, i, fold, convert=False):
    tr, Y = fold.tr, pool["Y"]
    role, tier = pool["role"][i], pool["tier"][i]
    res = {"tier_median": {}, "nn_kind_tier(oracle power)": {}, "role_tier_median": {}}
    same_t = tr[pool["tier"][tr] == tier]
    if len(same_t) == 0:
        dt = np.abs(pool["tier"][tr] - tier)
        same_t = tr[dt == dt.min()]
    nn = same_t[int(np.argmin(np.abs(pool["lnP"][same_t] - pool["lnP"][i])))]
    rt = tr[(pool["role"][tr] == role) & (pool["tier"][tr] == tier)]
    if len(rt) == 0:
        rt = tr[pool["role"][tr] == role]
    if len(rt) == 0:
        rt = same_t
    for s in cfg["stats"]:
        v = Y[s][same_t]
        res["tier_median"][s] = float(np.nanmedian(v)) if np.isfinite(v).any() else np.nan
        res["nn_kind_tier(oracle power)"][s] = float(Y[s][nn])
        v = Y[s][rt]
        res["role_tier_median"][s] = float(np.nanmedian(v)) if np.isfinite(v).any() else np.nan
    if convert:
        res["identity(vanilla value)"] = {s: (float(pool["van"][CONVERT_DRV[s]][i]) if s in CONVERT_DRV else np.nan)
                                          for s in cfg["stats"]}
    return res


def rel_err(pred, true, s):
    if not (np.isfinite(pred) and np.isfinite(true)) or true <= 0:
        return None
    return abs(pred - true) / true


def summarize(errs):
    a = np.array(errs, float)
    return dict(n=int(len(a)), median=round(float(np.median(a)), 3), p80=round(float(np.quantile(a, 0.8)), 3),
                within20=round(float(np.mean(a <= 0.2)), 2)) if len(a) else None


VARIANTS = {
    "simple": dict(bmax=0, asked=False),
    "quiz_compare_only": dict(bmax=5, asked=False),
    "quiz_full": dict(bmax=5, asked=True),
    "quiz_full_noisy": dict(bmax=5, asked=True, sigma=0.2, reps=10),
    "quiz_full_noisy_wide": dict(bmax=5, asked=True, sigma=0.2, same=0.25, reps=10),
    "quiz_budget3": dict(bmax=3, asked=True),
    "quiz_full_k1": dict(bmax=5, asked=True, k=1),
    "quiz_full_k3": dict(bmax=5, asked=True, k=3),
}


def subset_convert(pool):
    """Convert scenario: only guns with complete vanilla numbers; strength is the vanilla power index."""
    ok = np.array([all(np.isfinite(pool["van"][k][i]) for k in pool["van"]) and np.isfinite(pool["vanP"][i])
                   for i in range(len(pool["name"]))])
    q = dict(pool)
    for k in ("name", "role", "tier", "cat", "vanP"):
        q[k] = pool[k][ok]
    q["Y"] = {s: v[ok] for s, v in pool["Y"].items()}
    q["van"] = {s: v[ok] for s, v in pool["van"].items()}
    q["lnP"] = q["vanP"]
    return q


def twins(pool, i):
    """Training items that are near copies of item i (same role and tier, strength within 5 percent, mass within 5 percent)."""
    mk = "mass" if "mass" in pool["Y"] else "Mass"
    m = pool["Y"][mk]
    same = (pool["role"] == pool["role"][i]) & (pool["tier"] == pool["tier"][i])
    near = np.abs(pool["lnP"] - pool["lnP"][i]) < 0.05
    mm = np.abs(m - m[i]) <= 0.05 * np.abs(m[i])
    return {int(j) for j in np.where(same & near & mm)[0] if j != i}


def evaluate(P, VARIANTS=VARIANTS, only=None, twin_free=False):
    R = {}
    for key, pool in P.items():
        if only and key not in only:
            continue
        cfg = CFG[key]
        n = len(pool["name"])
        scen = [("scratch", False)] + ([("convert", True)] if key == "ce/ranged" else [])
        base_pool = pool
        for sname, conv in scen:
            pool = base_pool
            if conv:
                pool = subset_convert(base_pool)
            n = len(pool["name"])
            ids = list(range(n))
            folds = {}
            for i in ids:
                drv = None
                if conv:
                    drv = {s: pool["vanP"] if s not in CONVERT_DRV else lg(pool["van"][CONVERT_DRV[s]]) for s in cfg["stats"]}
                folds[i] = Fold(pool, cfg, i, drv, twins(pool, i) if twin_free else ())
            methods = {}
            logres = {}
            nqs = {}
            vlist = {"simple_convert": dict(bmax=0, asked=False)} if conv else VARIANTS
            for vn, var in vlist.items():
                reps = var.get("reps", 1)
                for i in ids:
                    for rp in range(reps):
                        v = dict(var, rep=rp)
                        o = run_method("m", pool, cfg, i, folds[i], v, conv)
                        nqs.setdefault(vn, []).append(o["_nq"])
                        for s in list(cfg["stats"]) + (["damage_ratio"] if "damage_ratio" in o else []):
                            t = pool["Y"][s if s != "damage_ratio" else "damage"][i]
                            e = rel_err(o.get(s, np.nan), t, s)
                            if e is not None:
                                methods.setdefault(vn, {}).setdefault(s, []).append(e)
                                if vn == "quiz_full":
                                    logres.setdefault(s, []).append((i, abs(math.log(max(o[s], 1e-9) / t))))
            for i in ids:
                for bn, vals in baselines(pool, cfg, i, folds[i], conv).items():
                    for s, pv in vals.items():
                        e = rel_err(pv, pool["Y"][s][i], s)
                        if e is not None:
                            methods.setdefault(bn, {}).setdefault(s, []).append(e)
            # band coverage of quiz_full: P50 / P80 of the other items' |ln ratio| residuals
            cov = {}
            for s, lst in logres.items():
                arr = np.array([x[1] for x in lst])
                c50 = c80 = 0
                for k in range(len(arr)):
                    oth = np.delete(arr, k)
                    c50 += arr[k] <= np.quantile(oth, 0.5)
                    c80 += arr[k] <= np.quantile(oth, 0.8)
                cov[s] = dict(cover_p50=round(c50 / len(arr), 2), cover_p80=round(c80 / len(arr), 2),
                              band_p50_ratio=round(float(np.exp(np.quantile(arr, 0.5))), 2),
                              band_p80_ratio=round(float(np.exp(np.quantile(arr, 0.8))), 2))
            tab = {m: {s: summarize(v) for s, v in d.items()} for m, d in methods.items()}
            R[f"{key}|{sname}"] = dict(
                n_items=len(ids), methods=tab, band_coverage=cov,
                questions={vn: dict(mean=round(float(np.mean(q)), 2), max=int(np.max(q))) for vn, q in nqs.items()},
                elasticity_all_items={s: round(Fold(pool, cfg, -1, None).b[s], 2) for s in cfg["stats"]} if not conv else None)
    return R


def macro(res):
    """Median over stats of the per-stat median error, per method (only stats common to all methods)."""
    out = {}
    for key, r in res.items():
        stats = None
        for m, d in r["methods"].items():
            ss = {s for s, v in d.items() if v and s != "damage_ratio"}
            stats = ss if stats is None else stats & ss
        out[key] = {m: round(float(np.median([r["methods"][m][s]["median"] for s in sorted(stats)])), 3)
                    for m in r["methods"]}
    return out


def sweep(pools):
    """Sensitivity of the a-priori weights. Chosen on the same data, so the best cell is optimistic."""
    sv = {k: VARIANTS[k] for k in ("simple", "quiz_full")}
    out = {}
    for wr in (1.0, 2.0, 4.0):
        for wt in (0.35, 0.7):
            for k in (1, 2, 3):
                globals().update(W_ROLE=wr, W_TIER=wt, K_ANCHORS=k)
                v = {n: dict(d, k=k) for n, d in sv.items()}
                m = macro(evaluate(pools, v))
                out[f"role{wr}_tier{wt}_k{k}"] = {p: {q: m[p][q] for q in ("simple", "quiz_full")} for p in m if "convert" not in p}
    return out


if __name__ == "__main__":
    pools = load_pools()
    if "--sweep" in sys.argv:
        sw = sweep(pools)
        (Path(__file__).parent / "sweep.json").write_text(json.dumps(sw, indent=0))
        for k, v in sw.items():
            print(k, {p: v[p]["quiz_full"] for p in v}, "avg", round(sum(v[p]["quiz_full"] for p in v) / len(v), 3))
        sys.exit(0)
    res = evaluate(pools)
    tf = evaluate(pools, {k: VARIANTS[k] for k in ("simple", "quiz_full", "quiz_full_k1")}, twin_free=True)
    out = dict(meta=dict(note="research prototype; see README.md", weights=dict(role=W_ROLE, tier=W_TIER, cat=W_CAT),
                         k_anchors=K_ANCHORS, same_band=SAME_BAND, noise_sigma_ln=0.2, noisy_reps=10),
               results=res, macro_median_of_stat_medians=macro(res),
               twin_free=dict(results=tf, macro_median_of_stat_medians=macro(tf)))
    (Path(__file__).parent / "results.json").write_text(json.dumps(out, indent=1))
    for k, v in out["macro_median_of_stat_medians"].items():
        print(k, res[k]["n_items"], v)
    print("twin-free")
    for k, v in out["twin_free"]["macro_median_of_stat_medians"].items():
        print(k, v)
