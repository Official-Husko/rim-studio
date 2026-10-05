# Item designer baseline, approach A: formula and distribution first

This note proposes how the RimStudio item designer (R7) computes a well-fitting baseline for a new weapon or apparel item from a few user inputs, in vanilla mode and in Combat Extended (CE) mode, with a simple formula mode and an optional quiz-like calibration mode. The approach places the item on a few axes (tier, role, relative strength percentile, size class, and for CE the class and caliber) and lets fitted formulas and peer distributions do the rest. A prototype and a leave-one-out backtest in this folder measure it. Evidence base: [vanilla ranged weapons](../../../vanilla-ranged-weapons-analysis.md), [vanilla melee weapons](../../../vanilla-melee-weapons-analysis.md), [vanilla apparel](../../../vanilla-apparel-analysis.md), [CE model](../../../combat-extended-model.md), [CE auto-patcher formulas](../../../ce-autopatcher-formulas.md) and [CE patch conventions](../../../ce-patch-conventions.md).

Status: draft | Last updated: 2026-10-04

## 1. Summary

1. Every number the designer proposes is either a quantile of a peer distribution read from the user's install, or the output of a small fitted formula (the power index, the price and work log-linear models, the apparel coverage law, the CE blunt to sharp ratio, the caliber lookup). No constant is shipped except five documented priors (section 2.5).
2. Simple mode asks three things (tier, role, a three-position strength slider; apparel adds coverage). Quiz mode asks at most 4 to 8 questions depending on kind, skips questions that cannot move the answer, and replaces the slider with "name an existing item it should rival".
3. Measured on six leave-one-out sets (n = 17 to 126), quiz mode lowers the mean median error to 18 percent (vanilla ranged), 24 (vanilla melee), 27 (vanilla apparel), 16 (CE ranged, excluding the caliber lookup), 32 (CE melee) and 29 (CE apparel). Against a nearest-neighbour baseline that gets the same answers it wins clearly on ranged and melee, and ties on apparel and on the CE ranged median, where copying a close sibling is hard to beat (section 7).
4. Error bands come from the user's own leave-one-out residuals, recomputed at calibration time, and are about 75 to 80 percent reliable at the P80 level (checked in section 4).
5. Graceful degradation is structural: a fixed fallback chain of peer pools, a flat-stat rule, and per-stat "ask, do not guess" flags. Without CE, CE modes are disabled and no CE number exists in the product (R11).

## 2. The model

### 2.1 Axes

| axis | meaning | source |
| --- | --- | --- |
| tier | Neolithic to Ultra, an ordinal | question 1, exact |
| role | ranged: bow, pistol, smg, rifle, shotgun, sniper, heavy (CE: class); melee: sharp or blunt; apparel: kind plus stuffed or fixed | question 2, exact |
| strength percentile s | position of the new item in the power index distribution of its peers | slider, rival item or rank |
| size class | interval on mass | question, optional |
| coverage | sum of the chosen body part group shares, computed from the install's body def | derived, apparel only |
| caliber | an ammo set from the install (CE ranged) | question, optional |

The scalar power index per kind is the one the research notes define: P4 for direct-fire ranged ([vanilla ranged](../../../vanilla-ranged-weapons-analysis.md), section Power index), in-fight DPS times (1 + AP) for vanilla melee, the armor power index for apparel. For CE the notes define no index, so the prototype uses damage times the square root of (1 + sharp penetration) for guns, power over cooldown for melee, and the sharp rating or thickness for apparel (these CE definitions are the prototype's own and unverified against CE design intent).

### 2.2 Peer pools

The pool for a new item is the first of: role and tier, role, group and tier, tier, group, all, that holds at least three items with the stat in question. The pool level is reported to the UI ("based on 5 Industrial rifles"). A new item is never compared against items of another role without saying so.

### 2.3 Distribution layer (every stat)

Each stat is predicted as a quantile of its pool distribution at a position u. The position starts at the median and moves with the answered axes by a ridge-shrunk rank regression fitted inside the pool: u = 0.5 + b1 (s - 0.5) + b2 (tier rank - 0.5) + b3 (size rank - 0.5). The coefficients are learned per stat from the user's install, so the model discovers, for example, that in vanilla ranged weapons mass follows strength and that cooldown does not. A stat whose pool spans less than a factor 1.16 (p10 to p90) is treated as flat and takes the median. An interval answer (for example "two-handed, 3 to 6 kg") restricts the quantile to pool items inside the interval, widening the pool until two items qualify, and falls back to the interval's geometric centre.

### 2.4 Formula layer (where mechanics tie stats together)

| kind | formula |
| --- | --- |
| vanilla ranged | burst, warmup, cooldown, range and accuracy come from the distribution layer; damage is then solved as the integer that makes P4 hit the target from s (AP follows 0.015 x damage inside the solve, clamped to the pool's damage range); nominal DPS follows exactly; price from ln MV = a + b ln P4 + c tier + d ln mass, work from ln work = a + b ln P4 + c tier, both refit on the install |
| vanilla melee | swing damage = predicted DPS x predicted cooldown (this makes the damage and cooldown trade-off exact instead of two independent guesses) |
| vanilla apparel | work, mass and material units are scaled by (coverage / pool median coverage)^0.25 (exponent from the apparel note backtest); market value from ln MV on ln work, tier and ln(API + 0.05); armor mechanics stay exact and are not estimated |
| CE ranged | damage and sharp penetration are a lookup on the chosen ammo set, as in CE itself; without a caliber they are quantiles on s |
| CE apparel | blunt rating = predicted sharp rating x the pool's median blunt to sharp ratio |
| CE conversion | when the user already has a vanilla design: mass is copied (identity), range is scaled by the pool's median CE to vanilla ratio, following [CE auto-patcher formulas](../../../ce-autopatcher-formulas.md) implication 3 |

### 2.5 Priors and what is learned

Learned at runtime from the install and cached as JSON with the install fingerprint: pools, rank coefficients, price and work coefficients, error bands. Fixed priors: ridge 0.2, coverage exponent 0.25, minimum pool 3, ask threshold (p10 to p90 factor 1.65), flat threshold (factor 1.16). These were set once and not tuned per kind. Explosive, mechanoid, turret and unique weapons are excluded from the pools and get no strength percentile ([vanilla ranged](../../../vanilla-ranged-weapons-analysis.md) implication 9).

## 3. The quiz

Answers constrain the model in exactly three ways: a pool selector (tier, role), a percentile (strength), or an interval on one stat (everything else). Nothing else is asked.

### 3.1 Questions

| id | question | options | constrains |
| --- | --- | --- | --- |
| tier | What era is it from? | Neolithic, Medieval, Industrial, Spacer, Ultra | pool |
| role | What kind of item is it? | role list of the kind (ranged weapons: bow, pistol, smg, rifle, shotgun, sniper, heavy) | pool |
| strength (simple) | Compared with other items of this role and tier, it should be | weaker than most, typical, stronger than most | s = 1/6, 1/2, 5/6 |
| strength (quiz) | Pick the existing item it should rival in power (or "not sure": falls back to the slider, and to the median if skipped) | the pool's items, listed with the strength meter hidden | s = percentile of the chosen item |
| coverage (apparel) | Which body parts does it cover? | body part groups | exact coverage |
| size (all kinds) | How big is it? | ranged: up to 1.5, 1.5 to 3, 3 to 6, over 6 kg; melee: under 0.7, 0.7 to 1.3, 1.3 to 2.5, over 2.5 kg; apparel: under 0.3, 0.3 to 1, 1 to 3, over 3 kg; CE ranged: under 1.5, 1.5 to 3.5, 3.5 to 7, 7 to 15, over 15 kg | mass interval |
| fire (ranged) | How does it fire? | vanilla: single shot, short burst (2 to 4), long burst (5 or more); CE: automatic or semi-automatic, pump or lever, bolt or single shot | burst or cooldown interval |
| reach (ranged) | How far does it shoot? | vanilla: under 20, 20 to 27, 27 to 35, over 35 tiles; CE: under 20, 20 to 40, 40 to 60, over 60 cells | range interval |
| handling (vanilla ranged) | How fast does it get on target? | under 0.5, 0.5 to 1.2, 1.2 to 2.5, over 2.5 s | warmup interval |
| swing (melee) | How fast is the swing? | quick, normal, slow (vanilla under 1.8, 1.8 to 2.3, over 2.3 s; CE under 1.3, 1.3 to 2.0, over 2.0 s) | cooldown interval |
| mag (CE ranged) | How many rounds does it hold? | up to 8, 9 to 20, 21 to 40, over 40 | magazine interval |
| caliber (CE ranged) | Which caliber? | the install's ammo sets, or "not sure" | damage and penetration by lookup |

The bin edges are round numbers chosen by the author after looking at the vanilla and CE distributions; the labels are what the user sees, so the thresholds must be reviewed by the owner (section 9).

### 3.2 Counts and adaptive logic

Maximum questions: vanilla ranged 7 (tier, role, strength, size, fire, reach, handling), vanilla melee 5, vanilla apparel 5 (adds coverage and size), CE ranged 8, CE melee 5, CE apparel 4. Measured mean questions asked in the backtest: 6.4, 4.0, 5.0, 6.9, 4.4 and 4.0. Simple mode is 3 questions (4 for apparel).

Adaptive rules: (1) skip a question if the pool's p10 to p90 range for the stat it constrains is under a factor 1.65 (bows are all single shot, so "fire" is skipped; CE rifles still get asked because of bolt actions); (2) once strength is known by rival item, the three-position slider is not shown; (3) "not sure" is always allowed and means the pool value; (4) the CE caliber question is shown only when CE is detected. A user can stop at any point; the baseline is recomputed after every answer, so the quiz is an incremental refinement, not a gate.

## 4. Fit meter and error bands

Fit meter (0 to 100): for each stat of the finished item, a robust z against the pool (median and 1.4826 MAD floored at 0.1 in log units); the stat scores exp(-0.5 max(|z| - 1.5, 0)^2); the meter is the mean over stats. It reads only the install's distributions. On the existing items (each scored against the others) the median is 81 to 91 across kinds, and the lowest scorers are the items the research notes already call outliers: Pila (65), Beam repeater (62 vanilla, 43 CE), children's vacsuit (20), Knife (60), Advanced helmet (25). That is the intended behaviour: low means "unusual", not "wrong".

Error bands: at calibration time the app runs the leave-one-out loop of `run_backtest.py` on the install's items, stores per stat the median and 80th percentile of the relative error (for the answer set the user gives, simple or quiz), and shows the predicted value with a P50 band and a P80 band: value / (1 + e) to value x (1 + e). A stat is green inside P50, amber inside P80, red outside. Check of the idea: for each item, the P80 band estimated from the other items' errors contained the item 70 to 84 percent of the time per stat (mean 76 to 80 percent per kind, ignoring the caliber lookup stats), so "80 percent band" is honest to within a few points at these sample sizes. Stats whose P50 exceeds 50 percent are labelled "ask, do not trust the estimate" (see 7.3).

## 5. Degradation

| situation | behaviour | measured |
| --- | --- | --- |
| CE not installed | CE modes disabled with an explanation; vanilla mode unaffected; no CE numbers anywhere | by design (R11) |
| CE installed but a class has fewer than 3 items | pool falls to group and tier, then all; the UI says so | CE ranged: 14 of 25 held-out items used the role pool, 4 role and tier, 7 group and tier |
| half of the reference items missing | same code, thinner pools | quiz mean median error rises from 18 to 23 (vanilla ranged), 24 to 30 (melee), 27 to 34 (apparel), 16 to 23 (CE ranged), 32 to 36 (CE melee), 29 to 34 (CE apparel) |
| user answers carelessly | each optional answer is dropped with probability 0.25 or moved one bin with 0.15, the rival is off by up to 2 ranks | mean median error rises by 0 to 5 points; still below the tier median baseline everywhere |
| fewer than about 8 items of a kind | quiz is hidden; simple mode only, with bands; formulas that need a fit (price, work) are hidden | not measured |
| mod with unknown tier or role | answered by the user; "any" pool | not measured |

Nothing in the model needs a minimum number of questions: with no answers beyond tier and role it returns role and tier medians (mean median error 29, 31, 32, 31, 41 and 45 percent), no worse than the tier median baseline and better in apparel and CE, and it carries a pool label.

## 6. UX implications

1. Show every proposed value as value plus P50 and P80 band, the pool label and its size, and the three nearest items; never a bare number.
2. Strength is the central control. Offer the rival item picker first because it needs no number knowledge; keep the three-position slider as the fast path.
3. Show derived stats (DPS, P4, price, API) as live formula results, not as estimates, once the primitive stats are fixed; they are exact then (the estimate applies to the primitives).
4. Flag stats the model cannot predict (market value of melee, CE Bulk, parry, crit, penetration of melee tools, WornBulk) as inputs, with the pool range beside them.
5. After the user edits any value, update the fit meter, never the other values.
6. Recalculate the bands when the install fingerprint changes (mods added, CE version change) and show the date.
7. The quiz can end early with "use what I have"; the progress label counts questions left.

## 7. Measured results

Method: leave-one-out over the resolved items of the research datasets (`data/vanilla-weapons`, `data/vanilla-apparel`, `data/ce-dataset`). For each item the prototype simulates the answers a modder would give from the item's true attributes (tier and role exact; strength as a tercile in simple mode and as a rival item chosen within 1 rank of the truth in quiz mode; bins by the table in 3.1; caliber exact), predicts the stats, and records the relative error. Apparel sets drop identical siblings (same work, mass, hit points; CE: same mass, bulk, armor) from training, as the apparel note does for its twin-excluded run. Baselines: tier median, and nearest neighbour given the same answers (nn simple: tier, role, strength; nn quiz: also the interval answers). Cells are median and p80 relative error in percent. Full per-stat tables: `results_table.md`; raw numbers: `results.json`.

### 7.1 Mean over scored stats (median / p80)

| set | n | tier median | nn simple | formula simple | nn quiz | formula quiz | quiz, careless answers | quiz, half the install |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged | 19 | 28 / 89 | 29 / 66 | 27 / 72 | 24 / 52 | 18 / 38 | 22 / 50 | 23 / 54 |
| vanilla melee | 17 | 33 / 60 | 40 / 71 | 26 / 60 | 32 / 63 | 24 / 49 | 24 / 52 | 30 / 76 |
| vanilla apparel | 96 | 54 / 118 | 34 / 81 | 27 / 65 | 27 / 90 | 27 / 70 | 29 / 76 | 34 / 78 |
| CE ranged (without caliber stats) | 25 | 39 / 102 | 23 / 69 | 27 / 95 | 13 / 52 | 16 / 42 | 17 / 67 | 23 / 51 |
| CE melee | 17 | 46 / 80 | 43 / 80 | 40 / 90 | 43 / 94 | 32 / 75 | 35 / 96 | 36 / 85 |
| CE apparel (without blunt) | 126 | 68 / 188 | 33 / 106 | 43 / 137 | 27 / 73 | 29 / 91 | 32 / 106 | 34 / 101 |

The CE ranged damage and sharp penetration stats are excluded because the caliber lookup returns them with zero error, which is the correct behaviour (in CE they are a property of the ammo set) but not a prediction. The CE apparel blunt rating has a heavy tail (p80 above 500 percent) and is excluded from the mean.

### 7.2 Headline stats (median / p80, quiz mode against tier median)

| stat | tier median | formula quiz |
| --- | --- | --- |
| ranged vanilla: nominal DPS | 23 / 74 | 22 / 36 |
| ranged vanilla: market value | 47 / 86 | 16 / 39 |
| ranged vanilla: mass | 54 / 130 | 13 / 28 |
| ranged vanilla: warmup | 41 / 93 | 10 / 37 |
| melee vanilla: DPS | 13 / 31 | 13 / 33 |
| melee vanilla: mass | 40 / 62 | 0 / 10 |
| apparel vanilla: work | 68 / 150 | 46 / 127 |
| apparel vanilla: mass | 83 / 200 | 14 / 39 |
| CE ranged: mass | 65 / 167 | 19 / 29 |
| CE ranged: range | 52 / 159 | 13 / 26 |
| CE ranged: magazine | 93 / 200 | 18 / 52 |
| CE melee: cooldown | 48 / 72 | 14 / 30 |
| CE apparel: mass | 82 / 400 | 30 / 57 |

### 7.3 Reading the results honestly

1. Where the formula layer matters: vanilla ranged, where solving damage from the target P4 and pricing from P4, tier and mass drops market value error from 47 to 16 percent against the tier median and from 37 to 16 against nearest neighbour with the same answers; removing the coupling (ablation `quiz_nocouple`) costs 17 points of market value error and 6 points of the p80 mean.
2. Where the distribution layer matters: size, warmup, range and magazine answers (interval constraints) reduce the errors of the stats they constrain by 2 to 5 times; they do nothing for the others.
3. What stays unpredictable from these inputs: work to make (about 50 percent median), melee market value (about 45), CE Bulk (26 to 53), parry and critical chances (about 54 and 24 to 35), apparel work (46) and armor ratings of unusual items. Those get an "ask" flag.
4. Apparel: simple mode (27 / 65) is as good as quiz mode (27 / 70). The quiz adds nothing there beyond the coverage the app already computes, so the apparel quiz should be 3 questions: kind and material mode, tier, strength.
5. Nearest-neighbour copying ties or beats the formula on the median for CE ranged (13 against 16) and for CE apparel (27 against 29), because CE patches of vanilla items are conversions with strongly sibling-like values. The formula keeps a better p80 in CE ranged (42 against 52). A hybrid (copy the rival item's values when the user names one and a close sibling exists, use the formula otherwise) is the natural next step and is where approach B and this one should be merged.
6. Converting an existing vanilla design to CE with identity mass and ratio range (`quiz_twin`) improves the CE apparel mean median error from 31 to 25 percent (blunt included) and CE ranged from 13.4 to 12.7, consistent with the auto-patcher note.

## 8. Limits of the evidence

1. Sample sizes are 17 to 25 for weapons and about 100 for apparel (with many near-identical siblings). A p80 from 17 items is the 14th ranked error; it moves by 10 points if one item changes. Differences below about 5 points between arms are noise; no significance tests were run.
2. Leave-one-out is optimistic: the author chose the design after seeing the same data. Choices made after the first run: the tier rank regressor, widening an interval pool, the flat-stat rule, the p10 to p90 spread rule, dropping the AP rule in favour of a pool quantile, dropping CE melee blunt penetration (heavy tail), and the CE blunt ratio. The first run (before the sibling exclusion for apparel, so apparel is not comparable) had quiz mean median errors of 20.5 (vanilla ranged), 27.4 (melee) and 15.9 (CE ranged, with caliber zeros); the final run has 17.6, 24.1 and 13.4, so tuning bought about 3 points and fresh data will probably be worse than the table.
3. Answers are simulated from true attributes, so they assume a user who knows the real item; the careless arm only partly models fiction-inspired items with no real counterpart.
4. CE data are the CE-converted vanilla items plus a few CE-native ones from the development tree, so CE results mostly measure how CE converts vanilla items. The caliber lookup is circular in the backtest (the item's own ammo set exists in the table) but it is exactly what a user selecting an installed caliber gets. The CE strength index is the prototype's own definition.
5. The bins and the strength by rival item were not user tested; whether modders can name a rival is a UX hypothesis. The five-level strength fallback was not measured.
6. Third-party modded items (the owner's own mods, CE ModPatches) are the right second dataset and are not used.

## 9. Owner decisions

1. Approve or change the bin edges in 3.1 (they become visible labels).
2. Decide whether the rival item picker is the default strength control (recommended) or the slider.
3. Decide whether the apparel quiz is cut to three questions (recommended by 7.3).
4. Decide whether to merge with the nearest-sibling approach as a hybrid (recommended).
5. Decide whether the bands are shown as P50 and P80 (recommended) or one band.

Reproduction: see [README](README.md).
