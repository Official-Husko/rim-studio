# Item calibration prototype: formula-first (approach A)

This folder holds a small research prototype and a leave-one-out backtest for the RimStudio item designer baseline (R7), approach A: fitted formulas and peer distributions first, a short quiz that places the new item on a few axes. The design note is [DESIGN.md](DESIGN.md). The data it reads are the committed research datasets only; the product reads the same quantities from the user's install at runtime (R11).

Status: draft | Last updated: 2026-10-04

## Run

Requires Python 3 with numpy, scipy and pandas. No other packages. From this folder:

```
PYTHONDONTWRITEBYTECODE=1 python3 run_backtest.py
```

It takes about one minute, is deterministic (fixed seeds, the second run produced an identical `results.json`), and writes `results.json` and `results_table.md` next to the scripts. No `__pycache__` is left when the variable above is set.

## Files

| file | content |
| --- | --- |
| `domains.py` | the six item sets (vanilla ranged, melee, apparel; CE ranged, melee, apparel), their stats, axes and quiz questions with bin edges |
| `engine.py` | peer pools, rank regression, formula layer, simulated answers, baselines, fit meter |
| `run_backtest.py` | leave-one-out loop, metrics, band calibration check, tables |
| `results.json` | median and p80 relative error per set, arm and stat; pool levels; questions asked; fit meter summary |
| `results_table.md` | the same as tables, cells are median / p80 relative error in percent |

Inputs (read only): `../../vanilla-weapons/ranged_direct_fit_set.csv`, `melee_table.csv`; `../../vanilla-apparel/apparel-items.csv`; `../../ce-dataset/ranged.csv`, `melee.csv`, `apparel.csv`, `ammo_calibers.csv`.

## Arms in the backtest

| arm | inputs given |
| --- | --- |
| `tier_median` | baseline: median of the items of the same tier |
| `nn_simple`, `nn_quiz` | baseline: copy the nearest training item given the same answers as simple or quiz mode (quiz also counts interval answers) |
| `template` | tier and role only |
| `simple` | adds a three-position strength answer (apparel: and coverage) |
| `quiz` | strength by naming a rival item, coverage, interval answers, caliber (CE ranged); questions with no spread in the pool are skipped |
| `quiz_noisy` | quiz with careless answers: 25 percent "not sure", 15 percent one bin off, rival off by up to 2 ranks |
| `quiz_nocouple` | quiz without the formula layer (distribution only) |
| `quiz_twin` | quiz plus identity mass and ratio range from an existing vanilla design (CE only) |
| `quiz_thin_half` | quiz with a random half of the training items, three repeats |

Apparel sets drop identical siblings from the training set so that a held-out item looks like a new one. Error is the absolute relative error; stats whose true value is zero or missing are skipped.

## Limits

Sample sizes are 17 to 126 items; see section 8 of the design note before quoting any number. The CE strength index is the prototype's own definition.
