# Anchor-first item calibration prototype

Research prototype for the item designer baseline (R7), approach B: anchor and comparison first. It simulates the answers of a modder who knows a weapon or apparel item but not its numbers, predicts the item's key stats from a few reference items ("anchors"), and measures the error with a leave-one-out backtest in vanilla mode and in Combat Extended (CE) mode. The design that follows from the measurements is in [DESIGN.md](DESIGN.md).

Status: draft | Last updated: 2026-10-04

## Run

Requires Python 3 with numpy and pandas (pandas only reads the CSV files; scipy is not needed). From this folder:

```
PYTHONDONTWRITEBYTECODE=1 python3 anchor_first.py            # about 20 s, writes results.json
PYTHONDONTWRITEBYTECODE=1 python3 anchor_first.py --sweep    # about 2 min, writes sweep.json
PYTHONDONTWRITEBYTECODE=1 python3 make_tables.py             # prints the per-stat markdown tables
```

Everything is deterministic: item `i` uses the random generator seed `1000 + i` (plus a replicate offset in the noisy variants).

## Inputs (research data only)

The scripts read the committed resolved datasets and nothing else. The product never reads these files: it reads the user's install at runtime (R11).

| Pool | Source file under `docs/research/data/` | Items |
| --- | --- | --- |
| vanilla ranged | `vanilla-weapons/ranged_direct_fit_set.csv` (the 19 direct-fire weapons) | 19 |
| vanilla melee | `vanilla-weapons/melee_table.csv`, group `standard` | 14 |
| vanilla apparel | `vanilla-apparel/apparel-items.csv` (Normal quality, default stuff) | 112 |
| CE ranged | `ce-dataset/ranged.csv` (bow, pistol, smg, rifle, shotgun, sniper, machine gun; no turrets, grenades, launchers, flamers or uniques) | 26 (25 with vanilla twin values) |
| CE melee | `ce-dataset/melee.csv`, tools aggregated per weapon, no bladelink variants | 14 |
| CE apparel | `ce-dataset/apparel.csv` with Mass and Bulk | 81 |

## Methods compared

| Name in results.json | Meaning |
| --- | --- |
| `simple` | tier and role only, strength at the role and tier prior (no comparison questions) |
| `simple_convert` | CE only: the item's vanilla numbers are known (conversion of a vanilla-style item) |
| `quiz_compare_only` | tier, role and up to 5 comparison questions |
| `quiz_full` | the above plus the stat questions (fire pattern, "vs anchor" questions); the product default |
| `quiz_full_noisy` | `quiz_full` with answer noise (standard deviation 0.2 in ln units per answer, 10 replicates) |
| `quiz_full_noisy_wide` | as above and an "about the same" band of 25 percent instead of 10 percent |
| `quiz_budget3`, `quiz_full_k1`, `quiz_full_k3` | budget of 3 comparisons; 1 anchor; 3 anchors |
| `tier_median` | baseline 1: median of the same tier |
| `nn_kind_tier(oracle power)` | baseline 2: nearest neighbour of the same kind and tier, nearest in true power (it is told the true strength, so it is a strong baseline) |
| `role_tier_median` | baseline 3: median of the same role and tier (it is told the true role) |
| `identity(vanilla value)` | CE conversion only: copy the vanilla value |

## Outputs

`results.json`: per pool and scenario the median, p80 and share within 20 percent of the relative error per stat and method, questions asked (mean, max), coverage of the error bands, the learned elasticities, a macro summary (median over stats of the per-stat median) and a twin-free rerun that removes near copies of the held-out item from the training set. `sweep.json`: macro results over the anchor weights and the number of anchors. Both are well below 500 KB.
