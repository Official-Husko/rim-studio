"""Formula-first baseline engine: simple mode, quiz mode, baselines, fit meter.

Everything is computed from a training table of resolved items (the user's install at runtime).
No constants are fitted outside the training table, except the documented priors:
  RIDGE (rank regression shrinkage), COV_K (coverage exponent from the apparel research note), FLAT,
  MIN_POOL (smallest peer pool), MIN_SPREAD (adaptive question threshold).
"""
import numpy as np
import pandas as pd
from scipy.stats import rankdata

RIDGE = 0.2
COV_K = 0.25
MIN_POOL = 3
MIN_SPREAD = 0.5      # ask a question only if the peer pool spans more than a factor 1.65 (p10 to p90)
FLAT = 0.15           # below this p10 to p90 log range a stat is treated as flat (median)
TERCILES = (1 / 6, 1 / 2, 5 / 6)


def midrank(vals, x):
    v = np.asarray(vals, dtype=float)
    return float(((v < x).sum() + 0.5 * (v == x).sum()) / len(v))


def pool_chain(train, role, tier, group):
    return [("role+tier", (train.role == role) & (train.tier == tier)), ("role", train.role == role),
             ("group+tier", (train.group == group) & (train.tier == tier)), ("tier", train.tier == tier),
             ("group", train.group == group), ("all", train.role.notna())]


def pool_of(train, role, tier, group, col=None):
    """Peer pool by the fallback chain role+tier, role, group+tier, tier, group, all."""
    for name, m in pool_chain(train, role, tier, group):
        sub = train[m]
        if (sub[col].notna().sum() if col else len(sub)) >= MIN_POOL:
            return sub, name
    return train, "all"


def _spread(vals):
    v = np.asarray(vals, dtype=float)
    v = v[~np.isnan(v)]
    if len(v) < 2:
        return 0.0
    if (v > 0).all():
        v = np.log(v)
    return float(np.quantile(v, 0.9) - np.quantile(v, 0.1))


def _ranks(v):
    return (rankdata(v) - 0.5) / len(v)


def _beta(pool, stat, xs):
    y = pool[stat].dropna()
    if len(y) < MIN_POOL or not xs:
        return np.zeros(len(xs))
    uy = _ranks(y.values) - 0.5
    X = np.column_stack([_ranks(pool.loc[y.index, c].values) - 0.5 for c in xs])
    return np.linalg.solve(X.T @ X + RIDGE * np.eye(len(xs)), X.T @ uy)


def _quant(vals, u):
    v = np.sort(np.asarray(vals, dtype=float))
    return float(np.quantile(v, min(max(u, 0.0), 1.0)))


def _interval(q, idx):
    e = [-np.inf] + list(q.edges) + [np.inf]
    return e[idx], e[idx + 1]


def _mid(lo, hi):
    if np.isinf(lo):
        return hi / 1.4
    if np.isinf(hi):
        return lo * 1.4
    return float(np.sqrt(lo * hi)) if lo > 0 else (lo + hi) / 2


def _bin(q, x):
    return int(np.searchsorted(q.edges, x, side="right"))


# ----------------------------------------------------------------------------- answers

def simulate_answers(dom, row, train, arm, rng, ammo=None):
    """Answers a modder who knows the item but not its numbers would give. arm: template, simple, quiz, quiz_noisy."""
    role, tier, group = row.role, row.tier, row.group
    pool0, level = pool_of(train, role, tier, group)
    ans = {"role": role, "tier": tier, "group": group, "asked": 2, "level": level}
    if arm == "template":
        return ans
    noisy = arm == "quiz_noisy"
    s_true = midrank(pool0[dom.strength], row[dom.strength])
    if arm == "simple":
        ans["p_s"] = TERCILES[min(2, int(s_true * 3))]
        ans["asked"] += 1
    else:
        sv = pool0[dom.strength].values
        order = np.argsort(sv)
        pos = int(np.argmin(np.abs(sv[order] - row[dom.strength])))
        jitter = int(rng.integers(-2, 3)) if noisy else int(rng.integers(-1, 2))
        rival = sv[order][min(max(pos + jitter, 0), len(sv) - 1)]
        ans["p_s"] = midrank(sv, rival)       # "name an existing item it should rival"
        ans["asked"] += 1
    if dom.coverage:
        ans["cov"] = float(row[dom.coverage])
        ans["asked"] += 1
    if arm == "simple":
        return ans
    for q in dom.questions:
        if _spread(pool0[q.stat]) < MIN_SPREAD and q.stat != dom.mass:
            continue                           # adaptive: skip questions that cannot move the answer
        if noisy and rng.random() < 0.25:
            continue                           # "not sure"
        b = _bin(q, row[q.stat])
        if noisy and rng.random() < 0.15:
            b = min(max(b + int(rng.choice([-1, 1])), 0), len(q.edges))
        ans[q.qid] = (q.stat, b)
        ans["asked"] += 1
    if dom.ammo and isinstance(row.get("ammoSet"), str) and ammo is not None and row.ammoSet in ammo.index:
        if not (noisy and rng.random() < 0.25):
            ans["caliber"] = row.ammoSet
            ans["asked"] += 1
    return ans


# ----------------------------------------------------------------------------- prediction

def _tf(c, v):
    if c == "api":
        return np.log(v + 0.05)
    return np.log(v) if c in ("power", "mass", "work") else v


def _fit_log(train, ycol, xcols):
    """OLS of ln(y) on transformed predictors with a tiny ridge; None when the table is too small."""
    d = train.dropna(subset=[ycol] + xcols)
    d = d[d[ycol] > 0]
    if len(d) < len(xcols) + 4:
        return None
    X = np.column_stack([np.ones(len(d))] + [_tf(c, d[c].values) for c in xcols])
    pen = 1e-3 * np.eye(X.shape[1]); pen[0, 0] = 0
    return (xcols, np.linalg.solve(X.T @ X + pen, X.T @ np.log(d[ycol].values)))


def _apply(model, vals):
    xcols, beta = model
    return float(np.exp(beta[0] + sum(b * _tf(c, v) for b, c, v in zip(beta[1:], xcols, vals))))


def predict(dom, train, ans, ammo=None, couple=True, twin=None):
    role, tier, group = ans["role"], ans["tier"], ans["group"]
    pool0, level = pool_of(train, role, tier, group)
    p_s = ans.get("p_s", 0.5)
    intervals = {}
    for q in dom.questions:
        if q.qid in ans:
            intervals[q.stat] = _interval(q, ans[q.qid][1])
    pred = {}
    # size axis first: mass from its interval, then its percentile drives the other stats
    p_m = 0.5
    if dom.mass:
        if dom.mass in intervals:
            lo, hi = intervals[dom.mass]
            inside = pool0[dom.mass].dropna()
            inside = inside[(inside >= lo) & (inside < hi)]
            mass_pred = float(np.median(inside)) if len(inside) else _mid(lo, hi)
            p_m = midrank(pool0[dom.mass].dropna().values, mass_pred)
    stats = list(dict.fromkeys(dom.stats + dom.hidden))
    for st in stats:
        sub, _ = pool_of(train, role, tier, group, col=st)
        xs = [dom.strength, "tier"] + ([dom.mass] if dom.mass and st != dom.mass else [])
        b = _beta(sub, st, xs)
        p_t = midrank(pool0["tier"].values, tier)
        x_in = np.array([p_s - 0.5, p_t - 0.5] + ([p_m - 0.5] if len(xs) == 3 else []))
        u = 0.5 + float(b @ x_in)
        vals = sub[st].dropna().values
        if _spread(vals) < FLAT:
            u = 0.5
        if st in intervals:
            lo, hi = intervals[st]
            ins = vals[(vals >= lo) & (vals < hi)]
            if len(ins) < 2:                       # widen the peer pool until the interval holds items
                for _, m in pool_chain(train, role, tier, group):
                    v2 = train[m][st].dropna().values
                    ins = v2[(v2 >= lo) & (v2 < hi)]
                    if len(ins) >= 2:
                        break
            pred[st] = _quant(ins, u) if len(ins) else _mid(lo, hi)
        else:
            pred[st] = _quant(vals, u)
        if "cov" in ans and st in dom.cov_scaled:
            cv = sub[dom.coverage].replace(0, np.nan).dropna()
            if len(cv) and ans["cov"] > 0:
                pred[st] *= (ans["cov"] / float(np.median(cv))) ** COV_K
    if couple:
        _couple(dom, train, pool0, ans, pred, p_s)
    if ans.get("caliber") and ammo is not None:
        pred["damage"], pred["apSharp"] = float(ammo.loc[ans["caliber"], "damage"]), float(ammo.loc[ans["caliber"], "apSharp"] or 0)
    if twin:
        for st, (col, mode) in dom.twin.items():
            if twin.get(col) is not None and not np.isnan(twin[col]):
                if mode == "identity":
                    pred[st] = float(twin[col])
                else:
                    has = pool0.dropna(subset=[col, st])
                    ratio = float(np.median(has[st] / has[col])) if len(has) >= 2 else 1.0
                    pred[st] = float(twin[col]) * ratio
    for st in dom.integer:
        pred[st] = float(max(1, round(pred[st])))
    pred["_level"] = level
    return pred


def _armor_mean(ap):
    return np.mean([1.0, 1 - 0.75 * max(0.55 - ap, 0), 1 - 0.75 * max(1.0 - ap, 0)])


def _couple(dom, train, pool0, ans, pred, p_s):
    n = dom.name
    if n == "ranged_vanilla":
        burst = max(1, round(pred["burst"]))
        gap = pred["gap"] if burst > 1 else 0.0
        if np.isnan(gap):
            gap = float(np.nanmedian(train.gap)) if train.gap.notna().any() else 6.0
        cycle = pred["warmup"] + pred["cooldown"] + (burst - 1) * gap / 60
        lp = np.log(pool0["power"].values)
        target = float(np.exp(_quant(lp, p_s)))
        base = pred["acc_mid"] * np.sqrt(pred["range"] / 25) * burst / cycle
        dmg = np.arange(1, 61)
        p4 = np.array([d * base * _armor_mean(0.015 * d) for d in dmg])
        d = int(dmg[np.argmin(np.abs(np.log(p4) - np.log(target)))])
        lo, hi = pool0.damage.min(), pool0.damage.max()
        d = int(min(max(d, lo), hi))
        pred.update(burst=float(burst), damage=float(d), power=float(d * base * _armor_mean(0.015 * d)),
                    dps_nominal=d * burst / cycle)
        bm = _fit_log(train, "mv", ["power", "tier", "mass"])
        if bm is not None:
            pred["mv"] = _apply(bm, [pred["power"], ans["tier"], pred["mass"]])
        bw = _fit_log(train, "work", ["power", "tier"])
        if bw is not None:
            pred["work"] = _apply(bw, [pred["power"], ans["tier"]])
    elif n == "melee_vanilla":
        pred["avg_damage"] = pred["dps"] * pred["avg_cooldown"]
    elif n == "ce_apparel":
        has = pool0.dropna(subset=["blunt", "sharp"])
        has = has[has.sharp > 0]
        if len(has) >= MIN_POOL and pred.get("armor"):
            pred["blunt"] = pred["armor"] * float(np.median(has.blunt / has.sharp))
    elif n == "apparel_vanilla":
        bm = _fit_log(train, "mv", ["work", "tier", "api"])
        if bm is not None:
            pred["mv"] = _apply(bm, [pred["work"], ans["tier"], pred["api"]])


# ----------------------------------------------------------------------------- baselines

def tier_median(dom, train, tier):
    m = train[train.tier == tier]
    m = m if len(m) else train
    out = {}
    for st in dom.stats:
        v = m[st].dropna()
        out[st] = float(v.median()) if len(v) else float(train[st].median())
    return out


def nearest_neighbour(dom, train, ans, quiz=False):
    pool0, _ = pool_of(train, ans["role"], ans["tier"], ans["group"])
    p_s = ans.get("p_s", 0.5)
    sv = pool0[dom.strength].values
    best, bd = None, 1e9
    for i, r in train.iterrows():
        d = abs(r.tier - ans["tier"]) + (0 if r.role == ans["role"] else 1) + abs(midrank(sv, r[dom.strength]) - p_s)
        if quiz:
            for q in dom.questions:
                if q.qid in ans:
                    lo, hi = _interval(q, ans[q.qid][1])
                    v = r[q.stat]
                    d += 0 if (not np.isnan(v) and lo <= v < hi) else 1
        if d < bd:
            best, bd = r, d
    return {st: float(best[st]) for st in dom.stats}


# ----------------------------------------------------------------------------- fit meter

def fit_meter(dom, peers, item):
    """0 to 100. Mean over stats of a soft penalty for sitting outside the robust peer range."""
    scores = []
    for st in dom.stats:
        v = peers[st].dropna().values
        if len(v) < MIN_POOL or np.isnan(item[st]) or item[st] <= 0:
            continue
        lv = np.log(v[v > 0]); x = np.log(item[st])
        z = (x - np.median(lv)) / max(1.4826 * np.median(np.abs(lv - np.median(lv))), 0.1)
        scores.append(np.exp(-0.5 * max(abs(z) - 1.5, 0) ** 2))
    return 100 * float(np.mean(scores)) if scores else float("nan")
