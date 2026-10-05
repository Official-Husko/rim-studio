# Item calibration prototypes: which approach won

This folder holds two competing research prototypes for the RimStudio item designer baseline (requirement R7) and the verdict that merges them. Approach A is "formula and distribution first" ([formula-first](formula-first/DESIGN.md)); approach B is "anchor and comparison first" ([anchor-first](anchor-first/DESIGN.md)). Both prototypes are kept unchanged as evidence. The product specification that follows from them is in [item balance math](../../../features/item-balance-math.md), [items toolkit](../../../features/items-toolkit.md) and [Combat Extended patching](../../../features/combat-extended-patching.md).

Status: draft | Last updated: 2026-10-04

## Verdict

Neither approach wins outright. The product takes a hybrid in which each design owns the part where it is measurably or structurally stronger.

| Part of the designer | Winner | Why |
| --- | --- | --- |
| Exact derived stats (cycle time, DPS, armor mitigation, price identity, apparel stat pipeline, CE meets-armor) | Both agree, shared | Both prototypes found these are rules of the game, not estimates; B additionally showed that scaling market value from anchors (error 0.38 to 0.43) is no better than baselines, and A showed price is best derived from the identity |
| Simple mode (no questions beyond tier and role, optional strength slider) | A | Its pool fallback chain, rank regression and formula coupling beat the tier median in every set (for example apparel 27 against 54 percent mean median error); B's simple mode equals a role-and-tier median and is worse for apparel |
| Calibrated mode, strength placement | B | A named anchor with real numbers on a card is easier to explain and to correct than a quantile, the adaptive bisection needs 2.0 comparisons on average, and an anchor survives unusual items |
| Calibrated mode, stats the user knows in real units (warmup, range, cooldown, magazine) | A | Interval answers cut warmup error from 0.33 to 0.10 where B's relative buckets did nothing |
| Calibrated mode, strength-driven stats (apparel ratings, Bulk, mass, melee damage) | B | Ratio scaling from anchors is better where strength drives the value (apparel sharp rating 0.06 against 0.36 simple) |
| Per-stat predictor choice | B's nested leave-one-out rule, extended to three predictors | Class constants need a class median, scale stats need anchors, quantiles sit between; the choice is data driven per stat |
| Error bands and fit meter | Both | Bands from leave-one-out residuals of a noisy-answer run (B); typicality score against peers (A); both were checked at about 0.7 to 0.84 coverage at P80 |

The hybrid itself is not measured. Section "Harmonized comparison" below shows why that is acceptable for a specification (the two designs are within noise of each other on the stats they share) and the acceptance gate that the Rust harness must pass before the hybrid is shipped.

## Re-run of both backtests (2026-10-04)

Both prototypes were re-run on a copy in scratch space. `results.json` of both and `results_table.md` of approach A came out byte-identical to the committed files, so the numbers quoted in the specification are reproducible. Commands, from each prototype folder:

```
PYTHONDONTWRITEBYTECODE=1 python3 run_backtest.py     # formula-first, about one minute
PYTHONDONTWRITEBYTECODE=1 python3 anchor_first.py     # anchor-first, about 20 seconds
```

## Harmonized comparison

The two prototypes aggregate differently (A: mean over stats of the per-stat median error; B: median over stats), so their headline tables cannot be compared. This comparison takes the per-stat median relative errors of both `results.json` files, keeps only the stats both predict under the same name, drops derived stats (market value, work, power, nominal DPS, AP, burst) and the CE caliber lookup stats, and averages. Cells are percent. "A quiz" is A's `quiz` arm, "B quiz" is B's `quiz_full`; "noisy" is A's `quiz_noisy` and B's `quiz_full_noisy` (the two noise models differ). The tier median baselines of the two prototypes agree within 1.5 points, which supports the comparison.

| set | common stats | tier median (A / B) | simple (A / B) | quiz (A / B) | noisy quiz (A / B) | best of the two per stat (quiz / noisy) |
| --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged | damage, warmup, cooldown, range, mass | 30.9 / 30.9 | 29.0 / 22.4 | 14.2 / 19.7 | 20.1 / 22.6 | 13.4 / 18.8 |
| vanilla melee | mass | 40.0 / 40.9 | 20.6 / 52.0 | 0.0 / 16.8 | 2.0 / 24.9 | 0.0 / 2.0 |
| vanilla apparel | mass, sharp rating | 48.2 / 51.6 | 19.6 / 34.7 | 13.0 / 4.5 | 18.1 / 16.2 | 4.5 / 15.6 |
| CE ranged | Mass, Bulk, cooldown, range, warmup, magazine, reload, spread | 39.9 / 38.4 | 26.5 / 24.0 | 14.7 / 14.2 | 16.1 / 14.8 | 12.2 / 13.8 |
| CE melee | cooldown, sharp penetration, Mass, Bulk, parry chance | 45.0 / 46.4 | 41.6 / 48.5 | 34.6 / 42.3 | 36.7 / 41.2 | 33.3 / 33.0 |
| CE apparel | Mass, Bulk, WornBulk, blunt rating | 70.0 / 70.1 | 45.2 / 53.0 | 30.7 / 17.2 | 33.1 / 27.2 | 17.1 / 26.5 |

Reading:

1. Under noisy answers the two quizzes are within 6 points of each other in every set (2.5 in vanilla ranged, 1.9 in vanilla apparel, 1.3 in CE ranged); with ideal answers the gaps are larger and go both ways, which is what different information granularity (absolute intervals against anchor-relative buckets) would produce.
2. Picking the better of the two predictors per stat would gain at most 1 to 3 points over the better design alone (the right-hand column), and that is an optimistic bound because the pick is made with hindsight. The per-stat choice is therefore worth keeping only because the nested leave-one-out rule makes it free, not because it promises a large gain.
3. CE melee has 14 to 17 weapons; every number there is close to the tier median and should be shown as "rough".
4. A's CE apparel set contains the blunt rating, which has a heavy tail; it inflates A's figure in that row.

Everything in the tables is in-sample in the sense that both authors tuned choices on the same data (see section 8 of each design note and section 9.4 of approach B). A held-out pool is needed: the owner's own CE patches and the third-party CE patch folders counted in [Combat Extended patch conventions](../../../research/ce-patch-conventions.md).

Script used for the table (reads both `results.json` files, writes nothing):

```python
import json, statistics as st
base = "docs/research/data/item-calibration/"
a = json.load(open(base + "formula-first/results.json"))["domains"]
b = json.load(open(base + "anchor-first/results.json"))["results"]
pairs = [("ranged_vanilla", "vanilla/ranged|scratch"), ("melee_vanilla", "vanilla/melee|scratch"),
         ("apparel_vanilla", "vanilla/apparel|scratch"), ("ce_ranged", "ce/ranged|scratch"),
         ("ce_melee", "ce/melee|scratch"), ("ce_apparel", "ce/apparel|scratch")]
skip = {"mv", "work", "damage_ratio", "power", "dps_nominal", "ap", "burst"}
lc = lambda s: s.lower().replace("_", "")
for ka, kb in pairs:
    A, B = a[ka]["arms"], b[kb]["methods"]
    sa, sb = {lc(s): s for s in A["quiz"]}, {lc(s): s for s in B["quiz_full"]}
    common = [k for k in sa if k in sb and sa[k] not in skip]
    if ka == "ce_ranged":  # caliber lookup stats are exact by construction
        common = [k for k in common if "penetration" not in k and k != "damage"]
    mean = lambda M, arm, names: round(100 * st.mean(M[arm][names[k]]["median"] for k in common), 1)
    print(ka, len(common), mean(A, "quiz", sa), mean(B, "quiz_full", sb),
          mean(A, "quiz_noisy", sa), mean(B, "quiz_full_noisy", sb))
```

## Files

| path | content |
| --- | --- |
| [formula-first/](formula-first/README.md) | approach A: design note, engine, backtest, results |
| [anchor-first/](anchor-first/README.md) | approach B: design note, prototype, sweep, results |

The product never reads these folders. It reads the user's install at runtime (R11); the datasets under `docs/research/data/` are research evidence only.
