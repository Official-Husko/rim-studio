# Item balance math

This document specifies the mathematics behind the RimStudio item designer (requirement R7): the exact formulas for weapons and apparel in vanilla and Combat Extended (CE) terms, the power indices, how reference pools are generated from the user's install, the simple formula mode, the calibrated quiz mode, the fit meter with its error bands, how the designer degrades, and the measured results of the two competing calibration prototypes that it merges. It is the contract for the pure crate `rimstudio-design` ([crate catalog](../architecture/crate-catalog.md), decisions D-064 and D-065, [ADR 0033](../adr/0033-item-math-and-ce-generator.md)). The product behaviour built on it is in [items toolkit](items-toolkit.md); the patch output is in [Combat Extended patching](combat-extended-patching.md). RimSort, RimCrow and CE are described in our own words as concept references only (R11).

Status: draft, section 14 records the 0.1.0 backend as built | Last updated: 2026-10-05

## Contents

1. [Decision summary](#1-decision-summary)
2. [Conventions, data sources and test vectors](#2-conventions-data-sources-and-test-vectors)
3. [Exact formulas](#3-exact-formulas)
4. [Power indices](#4-power-indices)
5. [Reference pools, roles and tiers generated at runtime](#5-reference-pools-roles-and-tiers-generated-at-runtime)
6. [The baseline model](#6-the-baseline-model)
7. [Simple mode](#7-simple-mode)
8. [Calibrated mode: the quiz](#8-calibrated-mode-the-quiz)
9. [Fit meter and error bands](#9-fit-meter-and-error-bands)
10. [Degradation rules](#10-degradation-rules)
11. [Measured results](#11-measured-results)
12. [Acceptance gates](#12-acceptance-gates)
13. [Owner decisions, conflicts and open points](#13-owner-decisions-conflicts-and-open-points)
14. [As built in the 0.1.0 backend](#14-as-built-in-the-010-backend)
15. [Weapon archetypes](#15-weapon-archetypes)

## 1. Decision summary

Two prototypes were built and backtested ([formula-first](../research/data/item-calibration/formula-first/DESIGN.md), approach A; [anchor-first](../research/data/item-calibration/anchor-first/DESIGN.md), approach B). Both backtests were re-run on 2026-10-04 and reproduce their committed `results.json` byte for byte. The verdict, with the evidence in [the calibration README](../research/data/item-calibration/README.md), is a hybrid:

| Layer | What it does | Taken from | Evidence |
| --- | --- | --- | --- |
| Exact | Cycle time, DPS, armor mitigation, price identity, apparel stat pipeline, CE meets-armor. No estimation. | shared by both, and the research notes | Section 3; values reproduced from the game's rules |
| Simple mode | Tier and role (plus an optional 3-position strength choice and any typed numbers) give a baseline with a pool label, three named neighbours and error bands. | A, with B's neighbour display | A simple mode beats the tier median in all six sets; B's simple mode equals a role-and-tier median |
| Calibrated mode | A short anchor dialogue places the item in a power-sorted list of real items; optional stat questions pin the stats the modder knows in real units. Each stat is predicted by whichever of three predictors a nested leave-one-out picks. | B's dialogue and rule selection, A's interval answers and quantile predictor | Under noisy answers the two quizzes are within 6 points on shared stats; they win in different places (section 11.3) |
| Price | Never predicted from strength. Computed from cost list and work with the game's identity. A model price is only a sanity band. | both | B: scaled market value 0.38 to 0.43 error, no better than baselines; vanilla notes: identity verified |
| Fit meter | Per-stat prediction bands (P50 and P80 of leave-one-out residuals from a noisy-answer run) plus a typicality score against peers. | B bands, A typicality | Band coverage at P80 measured 0.70 to 0.84 (A) and 0.71 to 0.82 (B) |
| CE conversion (optional patch, D-085) | When a vanilla design exists: mass by identity, range by ratio, other class constants by the predictor menu; damage and penetration by ammo set lookup. | both, ce-autopatcher note | Mass error 0.08 by identity against 0.26 scaled |

The hybrid is a specification derived from two measured designs. It has not itself been backtested. Section 12 states the gate: the Rust leave-one-out harness (`loo::validate`) must show the hybrid within 2 points of the better prototype on the shared stats of each set before the calibrated mode ships as the default.

## 2. Conventions, data sources and test vectors

1. Units follow the game: seconds, tiles (vanilla) or cells (CE), kilograms, silver, ticks (60 per second). CE units are mm of rolled homogeneous armor (mm RHA) for sharp ratings and MPa for blunt ratings ([Combat Extended model](../research/combat-extended-model.md) section 5).
2. Every number about the game comes from the user's install through the def engine at runtime (R11, invariant I-07): stat bases, stuff powers, quality factors, StatDef parts, body parts, ingredient prices, and for CE the user's resolved conversions and ammo sets. The repository holds no vanilla or CE value table. Quality factors, armor ratings of reference layers (the three used by the power index) and coverage numbers are read, not compiled in.
3. Test vectors use fictional numbers (a conformance rule of the architecture). Each vector below was computed with a small script during writing and cross-checked against the closed form; the install-backed checks (the revolver and assault rifle cycle, plate armor in steel, longsword and mace DPS from the vanilla notes) are `#[ignore]` tests that need `RIMSTUDIO_GAME_DIR` ([vanilla ranged](../research/vanilla-ranged-weapons-analysis.md), [vanilla melee](../research/vanilla-melee-weapons-analysis.md), [vanilla apparel](../research/vanilla-apparel-analysis.md)).
4. Results are deterministic: the same inputs give byte-identical JSON at any thread count (invariant I-12). Medians and percentiles are in house: percentile uses linear interpolation between order statistics (position q x (n minus 1)).
5. Errors are relative: abs(predicted minus true) divided by true, over items where the true value is positive.

## 3. Exact formulas

These functions are rules of the game. They are never labelled estimates and never carry error bands.

### 3.1 Ranged weapons (vanilla)

Inputs: projectile damage D, optional explicit armor penetration AP, burst count b (default 1), ticks between burst shots t (default 15), warmup w, cooldown c (the weapon stat), range R, four accuracy values (touch, short, medium, long).

1. Damage per shot is D rounded after the quality multiplier (1 at Normal).
2. Implied AP: if the projectile sets an explicit `armorPenetrationBase` it is used; if it sets only `damageAmountBase`, AP = 0.015 x D (the damage def default is not inherited); if it sets neither, the damage def default applies, and a negative default also becomes 0.015 x D. The designer shows the implied AP and warns that the damage def default is not used in that case.
3. Cycle time = w + c + (b minus 1) x t / 60. Warmup is paid once per burst, cooldown after the burst.
4. Nominal DPS = D x b / cycle.
5. Weapon hit factor at distance d: linear through (3, touch), (12, short), (25, medium), (40, long), constant outside, clamped to [0.01, 1]. Shooter skill, weather and cover multiply every weapon alike and are excluded.
6. Hit-adjusted DPS at d = nominal DPS x hit factor(d).
7. Armor step for one layer with rating r against penetration AP: e = max(r minus AP, 0); surviving damage fraction is 1 minus 0.75 e for e up to 1 (section 3.2 for e above 1).
8. Beam, explosive, mechanoid, turret and unique weapons are out of the DPS model (no accuracy curve, flat values, or random traits); the designer gives them fixed-frame rules and no strength percentile.

Test vectors (fictional):

| id | inputs | expected |
| --- | --- | --- |
| RG-1 | D 10, b 3, t 10, w 0.8, c 1.5 | AP 0.15, cycle 2.6333 s, nominal DPS 11.3924 |
| RG-2 | accuracy 0.6, 0.7, 0.65, 0.55 | hit factor at 18 tiles 0.676923 (0.7 at 12 falling linearly to 0.65 at 25) |
| RG-3 | D 20, b 1, w 2.0, c 1.2, explicit AP 0.30 | cycle 3.2 s, nominal DPS 6.25, AP kept 0.30 (not 0.015 x D) |

### 3.2 Armor mitigation (vanilla)

One layer: with e = max(rating minus AP, 0), expected surviving fraction is 1 minus 0.75 e for e up to 1, 0.5 minus e / 4 for e between 1 and 2, and 0 at 2 (ratings are clamped to 2). Layers combine by multiplication in expectation (each layer's outcome is an independent factor of 0, 0.5 or 1). The damage type picks the armor stat once; a layer that converts sharp to blunt does not change the stat used by later layers. Two thin layers are weaker than one thick layer ([vanilla apparel](../research/vanilla-apparel-analysis.md) section 2.2).

| id | inputs | expected |
| --- | --- | --- |
| AR-1 | layer 0.5, AP 0 | 0.625 |
| AR-2 | layers 1.0 and 1.0, AP 0 | 0.0625 |
| AR-3 | layers 0.4 and 0.9, AP 0.10 | 0.3100 (a Monte Carlo of the game's roll rule with 200,000 draws gave 0.3092) |
| AR-4 | layer rating 1.5 (e = 1.5) | 0.125 |
| AR-5 | layers 0.072, 1.0, 0.372, AP 0.15 | 0.30214 (install-backed vector from the apparel note, matches its Monte Carlo 0.3021) |

### 3.3 Melee weapons (vanilla)

Per attack (one per tool and capacity pair): damage = tool power x quality multiplier x stuff multiplier (sharp multiplier for Cut and Stab, blunt multiplier for Blunt, Poke and Demolish); cooldown = tool cooldown x the weapon's cooldown multiplier (the stuff factor); AP = explicit AP x damage multiplier, or 0.015 x adjusted damage when the tool sets none.

Two DPS numbers are shown and labelled, because the game uses two:

1. Stat-panel DPS: weights w_i = damage_i squared x chanceFactor_i; DPS = (sum w_i damage_i / sum w_i) / (sum w_i cooldown_i / sum w_i).
2. In-fight DPS: score_i = damage_i x (1 + AP_i) / cooldown_i x chanceFactor_i; Best if score is at least 0.95 of the maximum, Worst if below 0.25 of the maximum, otherwise Mid. Best attacks share 0.75 of the swings equally, Mid attacks share 0.25 equally, Worst attacks get none; shares are normalised to sum to 1. Swing damage, cooldown and AP are the share-weighted means; in-fight DPS = swing damage / swing cooldown. The additional-hediff term of the game's score is not modelled.

The melee strength scalar is P_M = in-fight DPS x (1 + mean in-fight AP).

| id | tools (power, cooldown, explicit AP) | expected |
| --- | --- | --- |
| ML-1 | handle 9 / 2.0, blade 18 / 2.4, blade 18 / 2.4 (AP implied) | stat DPS: damage 17.0, cooldown 2.3556, DPS 7.2170; in-fight: handle Mid, blades Best, swing damage 15.75, cooldown 2.3, DPS 6.8478, AP 0.23625 |
| ML-2 | handle 9 / 2.0, blade 12 / 1.5, point 13 / 2.0 | stat DPS 6.5000 (damage 11.812, cooldown 1.8173); in-fight 7.2308 (damage 11.75, cooldown 1.625, AP 0.17625) |

ML-2 reproduces the vanilla knife shape from the melee note (stat 6.5, in-fight 7.23) and is an exact algebraic check, not a game value.

### 3.4 Market value

If the def sets no `MarketValue`, base value = sum over the cost list of count x ingredient value, plus (stuffed items) stuff count / volume x stuff value, plus WorkToMake x work factor x 0.0036 when WorkToMake exceeds 2. Volume is 1, or 0.1 for small-volume stuffs (silver, gold). The quality factor, hit-point curve and rounding to the nearest 5 above 200 are applied by the stat pipeline. Price is therefore an output of the cost list and work. The designer never asks for a market value; an explicit `MarketValue` override is allowed with a warning (14 vanilla items set one and all are special items).

| id | inputs (fictional prices) | expected |
| --- | --- | --- |
| MV-1 | 2 of ingredient A at 30, 80 of stuff at 2.0 (volume 1), work 20,000 | 60 + 160 + 72 = 292.0, displayed 290 (rounded to 5 above 200) |
| MV-2 | stuff silver-like: count 40, volume 0.1, price 1.0, work 3,200 | 400 + 11.52 = 411.52, displayed 410 |

### 3.5 Apparel stat pipeline and coverage (vanilla)

Order of evaluation, verified in the apparel note: (1) base from `statBases` or the StatDef default; (2) stuff factor then stuff offset; (3) StatDef parts by priority, highest first: the stuff part adds (item multiplier stat) x (stuff power stat), then the quality part scales the whole sum; (4) finalize: rounding, clamp to the StatDef limits (armor to 0 through 2). Material changes armor only through stuff power; quality scales the stuff contribution too.

Coverage = sum of `coverageAbs` over the body parts the item covers, from the install's BodyDef (human parts sum to 1). It equals the chance that a random hit meets the layer, apart from height and hit-chance factors.

Armor power index: API = 100 x coverage x P with P = 0.6 S + 0.25 B + 0.15 H, where S is the mean prevented fraction of the sharp rating over AP 0, 0.15 and 0.30, and B and H are the prevented fractions of the blunt and heat ratings (prevented = 0.75 e). The weights are an assumption and are stored as replaceable JSON.

| id | inputs | expected |
| --- | --- | --- |
| AP-1 | multiplier 0.5, stuff sharp power 0.8, quality factor 1.15 | (0.5 x 0.8) x 1.15 = 0.46 |
| AP-2 | multiplier 1.5, stuff power 1.4, quality 1.8 | 3.78 clamped to 2.0 |
| AP-3 | sharp 0.6, blunt 0.3, heat 0.4, coverage 0.442 | S 0.3375, B 0.225, H 0.30, P 0.30375, API 13.4257 |

### 3.6 Combat Extended units, armor and the meets-armor preview

CE stats are not vanilla fractions: Mass in kg, Bulk and WornBulk in volume units, `ShotSpread` and `recoilAmount` in degrees, `SwayFactor` and `SightsEfficiency` as factors, warmup and reload in seconds, range in cells. A stuffed apparel item has `StuffEffectMultiplierArmor` in mm and the material's power per mm; fixed apparel carries ratings in `ArmorRating_Sharp` (mm RHA) and `ArmorRating_Blunt` (MPa). Stuffed rating = thickness x stuff power (sharp and blunt powers differ). The designer reads these from the user's CE install and validates units (a CE patch with vanilla-style ratings below 1 is flagged).

Meets-armor preview, for a round with damage d, sharp penetration ps (mm RHA) and blunt penetration pb (MPa) against layers ordered outermost first, each (rs, rb):

1. While the attack is sharp: if rs is greater than the remaining sharp penetration p, the attack is deflected and becomes blunt at this layer. Otherwise damage is multiplied by (p minus rs) / p and p becomes p minus rs.
2. On deflection the blunt penetration becomes pb x (p / ps) (the fraction of sharp penetration that remained) and the blunt damage becomes cube root of (blunt penetration x 10,000) / 10, times the damage share (current damage / original damage). Then the blunt rule applies at the same layer.
3. Blunt rule: if rb is at least the blunt penetration the attack stops (damage 0); otherwise damage is multiplied by (pb minus rb) / pb and pb becomes pb minus rb.
4. The result is health damage after the layers and whether it is sharp or blunt. Body part density, armor damage and partial armor (`PartialArmorExt`) are not modelled in v1.

| id | inputs (fictional) | expected |
| --- | --- | --- |
| CE-1 | round 10 / 5 mm / 20 MPa vs layer 4 mm / 6 MPa | 2.0 sharp (10 x 1/5) |
| CE-2 | same round vs layer 6 mm / 6 MPa | deflected (6 is above 5); blunt damage 5.85 (cube root of 200,000, divided by 10, share 1.0), then x 14/20 at the 6 MPa layer: 4.09 blunt |
| CE-3 | same round vs layers 4 mm / 6 MPa then 8 mm / 12 MPa | 2.0 sharp after the first layer (remaining penetration 1 mm), deflected by the second; blunt penetration 20 x 1/5 = 4 MPa is below 12 MPa, so 0 |

The function reproduces the install-backed worked examples of the CE note within rounding (a 5.56-class round against a steel-like vest 4.54 against the note's 4.5; a jacket followed by a plasteel-like vest 1.10 against 1.1; an armor-piercing round against the steel-like vest 3.0 sharp; a 14.5 mm round 29.44 sharp against 29.4), so the implementation is also pinned by `#[ignore]` conformance tests on the user's CE ([Combat Extended model](../research/combat-extended-model.md) section 5).

Further CE rules that the designer uses: ammo type multipliers (armor-piercing about 0.62 damage and 2.0 sharp penetration, hollow point about 1.26 and 0.5, sabot 0.52, 3.5 and speed 1.36) are read from the user's CE ammo sets when it generates a new ammo set, not stored; melee penetration per hit is tool penetration x (damage factor to the power 0.75) x the skill term x the weapon's penetration factor; blades in CE usually sit under 1 mm RHA and need a deliberate penetration choice, which the designer warns about.

A RimStudio display metric for CE guns, not a CE formula: sustained DPS = D x magazine / (magazine time + reload), where magazine time = bursts x (w + c) + (magazine minus bursts) x t / 60 and bursts = ceil(magazine / b). Vector CE-4: D 12, magazine 30, b 6, t 5, w 1.0, c 0.4, reload 4.0 gives bursts 5, magazine time 9.0833 s and sustained DPS 27.52. CE's hit chance function was not read ([Combat Extended model](../research/combat-extended-model.md) open question 1), so the designer shows the error components, not a hit chance.

## 4. Power indices

A power index is one positive number per item, in units a modder can read, used to order items for the quiz and as a regressor.

| Kind and mode | Index | Definition | Status |
| --- | --- | --- | --- |
| Vanilla ranged | P4 | nominal DPS x mean hit factor at 12 and 25 tiles x mean armor multiplier over unarmored, flak-like 0.55 and heavy 1.00 reference layers (read from the user's apparel) x square root of (range / 25) | Spearman 0.94 with market value on 19 weapons; AssaultRifle 5.674, Revolver 2.935 in the research data |
| Vanilla melee | P_M | in-fight DPS x (1 + in-fight AP) | Spearman 0.72 with price; for placement inside a band, not for pricing |
| Vanilla apparel | API (plus a floor of 0.05 for ratio use) | section 3.5 | R2 0.71 to 0.77 with tier for log price |
| CE ranged | proxy | damage x pellets x burst / cycle x square root of (range / 25) x (1 + 0.1 x sharp penetration) | prototype proxy (approach B), unverified against modder judgement |
| CE melee | proxy | mean tool power / mean tool cooldown | prototype proxy |
| CE apparel | proxy | sharp rating (or stuff thickness) + 0.15 x blunt rating, floor 0.05 | prototype proxy |

Vector PI-1 (fictional gun of RG-1 with range 28 and the accuracy of RG-2, AP 0.15): hit mid 0.675, mean armor multiplier (1, 0.7, 0.3625) = 0.6875, P4 = 11.3924 x 0.675 x 0.6875 x square root of 1.12 = 5.5950. PI-2 (RG-3 with range 40): hit mid 0.775, armor mean 0.7625, P4 = 4.6718.

Indices are replaceable data: a JSON definition per kind and mode names its components and weights, so the owner can change the CE definitions without code (R10). Explosive, mechanoid, turret and unique weapons get no index. The CE proxies need a short design review (owner decision 2).

## 5. Reference pools, roles and tiers generated at runtime

A pool is the set of reference items of one kind (ranged, melee, apparel) in one mode (vanilla or CE), built from the user's resolved defs. Nothing is stored in the repository.

Procedure (job `designer_calibrate`, see [items toolkit](items-toolkit.md)):

1. Take the workspace's reference set (the resolved `DefDatabases` snapshot of the user's install and chosen mods, [modding workspace](modding-workspace.md)).
2. Select candidates by rule: category Item, primary equipment, in the weapon or apparel thing categories; exclude explosive and launcher, mechanoid and creature, turret and unique weapons from direct-fire pools.
3. When the optional CE patch is requested (or in the Convert flow) a candidate is a CE conversion when it carries the CE verb class, a CE tool class or the ammo comp (the same detector the patch generator uses). The raw `MakeGunCECompatible` parameters of the user's CE patches are read through the typed reader in `design::ce::reader`, so converted guns are evaluated as the game will see them.
4. Resolve each item: statBases, verb or tool lists, projectile, cost list, work, tech level, coverage and layers (apparel), tags.
5. Assign the tier from the tech level (Neolithic 0, Medieval 1, Industrial 2, Spacer 3, Ultra 4, Archotech 5; only levels present in the pool are offered).
6. Assign the role by a small decision list over resolved fields, authored by RimStudio as JSON (R10) and editable. Apparel roles come from layers and body part groups (helmet, torso vest, torso outer, base clothing, legwear, full body, utility); melee roles from tool capacities (blunt, blade, piercing); ranged roles from burst, range, warmup, weapon tags and classes (bow, pistol, smg, rifle, shotgun, sniper, heavy). The research prototypes used a visible hand-made role table; the rule-based assigner has not been built. Gate: it must agree with that table on at least 90 percent of the research items, and the user can relabel any reference item (stored in the project JSONC).
7. Add the user's own items on request: a project's items (for example the owner's hand-written CE patches) can be added to a pool per project, as extra references ([approach B design note](../research/data/item-calibration/anchor-first/DESIGN.md) section 2.1).
8. Compute the strength index (section 4) and cache the pool, the fitted coefficients and the error bands as JSON in the cache root, keyed by (game version, CE version hash, hash of the reference set, hash of the index definitions, harness version). The cache is deletable; a stale key triggers a background recalibration while the previous result stays usable with the date shown.

Pool sizes in the research data: 19 direct-fire ranged, 14 to 17 melee, 81 to 126 apparel (many near twins), 25 to 26 CE ranged. A role needs at least 3 items with the stat; otherwise the fallback chain in section 6 applies.

## 6. The baseline model

For each stat the designer holds three predictors and a rule picks one.

| Predictor | Definition | Source |
| --- | --- | --- |
| M, class median | median of the stat over the first pool in the chain that has at least 3 items with the stat | both |
| Q, pool quantile | quantile of the pool distribution at position u = 0.5 + b . (x minus 0.5); x are the mid-ranks of the new item on strength, tier and (when asked) mass within the pool; b is a ridge regression (lambda 0.2) of the pool's mid-ranked stat on the same mid-ranks, fitted per stat on the user's install; a stat whose p10 to p90 range is under a factor 1.16 is flat and takes the median; an interval answer restricts the quantile to pool items inside the interval, widening the pool until two items qualify | A |
| R, anchor ratio | the K = 2 nearest items by normalised distance weighted 1 / (d + 0.3); ln y = weighted mean of (ln y_j + e (ln P minus ln P_j)), with elasticity e fitted within role (ridge 0.5, clipped to [minus 0.5, 1.5]); discrete stats take the anchors' weighted median | B |

Fallback chain for pools: role and tier, role, group and tier, tier, group, all, each with at least 3 items; the chosen level is shown ("based on 5 Industrial rifles"). Distance for anchors: sqrt of (2.0 if role differs)^2 + (0.35 x tier difference)^2 + ((ln P minus ln P_a) / sd)^2 + (0.8 if fire class differs)^2.

Rule selection: for each stat a nested leave-one-out inside the training set computes the median relative error of M, Q and R and uses the lowest, ties resolved in the order M, Q, R (prefer the simpler predictor). Stats the user answered use the answer; typed numbers replace predictions and, for mass, range, warmup and cooldown in a CE conversion, become the scaling driver. The prototypes found that class constants (CE warmup, spread, sway, cooldown) end up at M, and mass, magazine, Bulk and apparel ratings at R.

Structural identities applied after prediction:

1. Melee swing damage = predicted strength (in-fight DPS) x predicted swing cooldown. Measured error 0.07 for damage.
2. Vanilla ranged: burst, warmup, cooldown, range and accuracy profile come from the predictors; damage is the predictor value (B's ratio scaling, error 0.29, was better than the P4 solve at 0.34); the designer offers "keep strength, show the damage that holds P4" as a helper, not a default. AP follows 0.015 x damage unless the role needs more. Market value and work are not predicted from strength.
3. Apparel: work, mass and material units scale with (coverage / pool median coverage) to the power 0.25 (exponent chosen by backtest); armor mechanics stay exact; market value follows the identity.
4. CE ranged: damage and sharp and blunt penetration are a lookup on the chosen ammo set; without a caliber they are shown as a quantile with a wide band.
5. CE apparel: blunt rating = predicted sharp rating x the pool's median blunt to sharp ratio; excluded from headline means (heavy tail).
6. CE conversion (a vanilla design exists): mass is copied (identity, error 0.08), range scaled by the pool's median CE to vanilla ratio, spread, warmup, cooldown by M or R with the vanilla values as the scaling driver (errors 0.07, 0.10, 0.06, range 0.14).

Priors (fixed, documented, not tuned per user): ridge 0.2 (Q) and 0.5 (R), coverage exponent 0.25, minimum pool 3, K = 2, ask threshold factor 1.65, flat threshold factor 1.16, "about the same" band 10 percent, noise sigma 0.2 in ln units. They were chosen on the research data; the sweep over anchor weights and K was flat (macro error 0.150 to 0.195), so they are unlikely to matter more than the noise of the data.

Stats not predictable from these inputs carry an "ask" flag and are plain inputs with the pool range beside them: work (about 50 percent median error), melee market value (about 45), CE Bulk for weapons (26 to 53), parry and crit chances, melee tool penetration (CE melee blunt penetration was dropped for a heavy tail), WornBulk, CE armor in mm for stuffable apparel, caliber and ammo set, weapon tags.

## 7. Simple mode

Simple mode needs no quiz and never uses a fixed preset (the CE auto-patcher's silent fallback to its sniper preset is the anti-pattern, [CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) implication 2).

Questions: tier, role, and a 3-position strength choice (weaker than most, typical, stronger than most of this role and tier, mapped to percentiles 1/6, 1/2 and 5/6); apparel adds the body part groups (coverage is computed). The user may also type any known number. Output: for each stat the predicted value from the rule-selected predictor with the strength percentile, the pool label and size, P50 and P80 bands, and the three nearest items. A typed number replaces the prediction and is not re-banded.

Simple mode is the calibrated mode with no answers beyond the setup taps, so the two share one code path (`baseline::simple_estimate` and `baseline::anchor_baseline`, ADR 0033). With only tier and role known the output is the role and tier median with a pool label (mean median error 29, 31, 32, 31, 41 and 45 percent in A's six sets), no worse than the tier median and better in apparel and CE. Fewer than about 8 reference items of a kind: simple mode only, bands shown, fitted formulas (price and work models) hidden.

## 8. Calibrated mode: the quiz

The quiz has two phases. Phase 1, "find the anchors", is at most five taps (two setup and up to three comparisons; apparel allows five comparisons), which satisfies the CE auto-patcher note's cap of five role questions ([CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) section 7.3). Phase 2, "refine", is optional stat questions. The user can stop at any point with "use what I have"; the estimate is recomputed after every answer, so the quiz is incremental refinement, never a gate.

Every answer is exactly one of three kinds of constraint: a pool selector (tier, role, fire class), a strength position (the interval of ln P found by the dialogue), or a constraint on one stat (an interval or a ratio bucket). Nothing else is asked.

### 8.1 Setup taps (2)

| id | question | options |
| --- | --- | --- |
| S1 | Which tech level is it? | levels present in the pool (Neolithic to Archotech) |
| S2 | What is it? | the role list of the kind, derived from the pool (section 5) |

### 8.2 Comparison dialogue (strength)

Each question shows an anchor card (name, picture where available, the index components with real numbers) and asks "Is your item weaker, about the same, or stronger?" The list is the pool of the same kind, sorted by P (for apparel, the pool of the same role, because strength is not comparable across a vest and a hat).

1. Keep a bracket (lo, hi) of list positions known to hold the item; start with the whole list.
2. Prior: normal on ln P centred on the mean of same role and tier peers with sd 0.6 (fallback role, then all).
3. Pivot: the item in the bracket that splits the remaining prior mass in half (prior-weighted bisection).
4. "Stronger" moves lo, "weaker" moves hi, "about the same" (within about 10 percent of power) ends the dialogue with P equal to the pivot.
5. When two neighbours remain, one extra question asks "closer to {lower} or {upper}?", which places the item at a quarter or three quarters of the gap (halfway when skipped).
6. Stop at the cap (3 comparisons for weapons, 5 for apparel; the budget study found weapons lose 0.00 to 0.03 macro error going from 5 to 3, apparel 0.03) or when the bracket is one gap. The estimate interpolates ln P inside the bracket; at the list edges it extrapolates by one median gap.
7. "Start from {item}" (the rival shortcut of approach A) makes that item the first pivot and replaces the dialogue by the one "weaker, same, stronger than it" question plus the "closer to" refinement.

Measured on 19 vanilla weapons: 2.0 comparisons on average (maximum 5), strength located within 0.02 ln units at the median with ideal answers. With noisy answers the dialogue is about one question longer.

### 8.3 Refine questions (stat constraints)

Questions are skipped when the pool's p10 to p90 range for the stat is under a factor 1.65 (they cannot move the answer; for example bows are all single shot) and are ordered by that spread. Every question allows "not sure" (the stat falls back to the rule-selected predictor and the band widens) and "type a value".

| kind and reference set | question | answer options | effect |
| --- | --- | --- | --- |
| ranged, vanilla and CE reference sets | How does it fire? | vanilla: single, short burst 2 to 4, long burst 5 to 9, belt 10 or more; CE: single, burst or auto 2 to 6, sustained 7 or more | burst = median of that class; anchors of another class get +0.8 distance |
| ranged, vanilla | How fast does it get on target? | under 0.5, 0.5 to 1.2, 1.2 to 2.5, over 2.5 s | warmup interval |
| ranged | How far does it shoot? | vanilla: under 20, 20 to 27, 27 to 35, over 35 tiles; CE: under 20, 20 to 40, 40 to 60, over 60 cells | range interval |
| ranged, CE | Which caliber? | the install's ammo sets, or not sure | damage and penetration by lookup (always first) |
| ranged, CE | How many rounds does it hold? | up to 8, 9 to 20, 21 to 40, over 40 | magazine interval |
| all kinds | Compared with {anchor}, is its mass lighter, similar or heavier? | lighter (below 85 percent), similar, heavier (above 118 percent) of the anchor's shown mass | mass = anchor mass x the learned median ratio of that bucket |
| melee | How fast is the swing? | vanilla: quick under 1.8, normal 1.8 to 2.3, slow over 2.3 s; CE: under 1.3, 1.3 to 2.0, over 2.0 s | cooldown interval; damage follows from strength x cooldown |
| vanilla apparel | Is it colder, similar or warmer than {anchor}? (only when insulation varies in the role) | three buckets | cold insulation |
| CE apparel | Is its Bulk smaller, similar or larger than {anchor}? | three buckets | Bulk |

The bucket multipliers are learned on the pool: each reference item is compared with its nearest anchor and the median ratio per bucket is stored. The bin edges are round numbers chosen after looking at vanilla and CE distributions; they become visible labels, so the owner reviews them (owner decision 1).

### 8.4 Counts and examples

| kind | setup | comparisons | refine (at most) | cap | measured mean (A / B, ideal answers) |
| --- | --- | --- | --- | --- | --- |
| ranged, vanilla | 2 | 3 | 4 (fire, handling, reach, mass) | 9 | 6.4 / 7.0 |
| ranged, CE | 2 | 3 | 4 (caliber, mass, fire, magazine; reach when spread allows) | 9 | 6.9 / 8.6 |
| melee, both | 2 | 3 | 2 (swing, mass) | 7 | 4.0 / 6.2 |
| apparel, vanilla | 2 | 5 | 2 (mass, insulation) | 9 | 5.0 / 6.2 |
| apparel, CE | 2 | 5 | 2 (mass, Bulk) | 9 | 4.0 / 7.4 |

The caps are the hybrid's own; only the comparison budgets are measured. The UI shows "question n of about m" with m recomputed from the bracket, announces the cost up front, and offers "skip to result" after any answer.

Worked example, fictional: the user names a new two-handed industrial rifle. S1 Industrial, S2 rifle. The pivot is the median same-role item; the user says "stronger", then "about the same" as the next pivot: two comparisons, strength fixed. Fire: short burst. Reach: 27 to 35 tiles. Mass: "heavier than {anchor}, 3.5 kg". The app now has the role pool, a strength position, burst 3 (class median), a range interval, and a mass bucket; each remaining stat comes from its rule-selected predictor, and the result screen lists, per stat, the value, band and source (anchor, class median, answer, typed).

## 9. Fit meter and error bands

### 9.1 Calibration harness (leave-one-out)

At calibration time (first use, and whenever the cache key changes) a job runs `loo::validate`: for each reference item in a pool, hold it out, simulate the answers a modder who knows the item would give (tier and role exact; comparisons by true strength with a 10 percent same band; fire class exact; stat buckets from true ratios), predict every stat from the other items, and record the relative error. Three variants run: ideal answers, noisy answers (normal error with sd 0.2 ln on every comparison and 0.1 on every bucket answer, 10 replicates per item, a wider 25 percent same band as a stress case) and the simple-mode answers. The stored bands come from the noisy run, because ideal-answer bands are over-confident (the vanilla ranged macro error went from 0.14 ideal to 0.25 noisy). Each item uses the random generator seed 1000 + index plus the replicate offset, so the job is deterministic.

The harness also reruns with a twin-free training set (near copies of the held-out item removed) to detect flattering by sibling items; a pool whose twin-free error is more than 0.05 above the plain error shows "bands optimistic: many near twins".

### 9.2 Bands and levels

Per stat the cache stores the P50 and P80 multiplicative residual factors (examples from the prototype: vanilla ranged range x1.10 and x1.15, mass x1.17 and x2.68; CE ranged range x1.14 and x1.33, Bulk x1.25 and x1.46, magazine x1.18 and x2.0; vanilla apparel sharp rating x1.06 and x1.35). The editor shows each predicted value with its band: value / factor to value x factor. A stat is green ("typical") inside P50, blue ("plausible") inside P80 and amber ("unusual") outside; red is never used for fit. Beside it a rank check ("heavier than 12 of 19 reference rifles") because ranks are more robust than values (rank correlations 0.4 to 0.8 in the CE note). The summary line gives the share of stats inside P80 (green or blue) and the number of reference items behind the bands, and says "bands are rough" below about 15 items in the chosen role.

Stats a user typed are compared with the band, never overwritten. After the user edits any value the meter updates and other values stay put.

Calibration of the bands themselves was checked: building each held-out item's P80 band from the other items' residuals, the item fell inside 70 to 84 percent of the time per stat in A (mean 76 to 80 percent per kind) and 0.71 to 0.82 in B, with P50 coverage 0.47 to 0.57. The bands are therefore not over-confident at these sample sizes.

### 9.3 Typicality score

Separately from the bands, a typicality score from 0 to 100 answers "does this item look like its peers?". For each stat of the finished item: robust z = (ln x minus median of ln peers) / max(1.4826 x MAD, 0.1); the stat scores exp(minus 0.5 x max(abs z minus 1.5, 0)^2); the score is the mean over stats. Vector FM-1 (fictional peers 8, 9, 10, 11, 12, 14): median of ln 2.3502, robust scale 0.2133 (above the 0.1 floor), so x = 13 gives z 1.01 and score 1.0; x = 30 gives z 4.93 and score 0.003. On the research items the median score is 81 to 91 and the lowest are the outliers the notes already name (Pila, Beam repeater, children's vacsuit, Knife, Advanced helmet). Low means unusual, not wrong; the UI says so.

## 10. Degradation rules

| situation | behaviour | measured |
| --- | --- | --- |
| CE not installed | the optional CE patch toggle is disabled with a reason; the vanilla designer is complete; no CE number exists anywhere in the product | by design (R11, D-085) |
| CE installed, fewer than 3 items in a class | pool widens along the chain and the label says so | CE ranged: 14 of 25 held-out items used the role pool, 4 role and tier, 7 group and tier |
| half of the reference items missing | same code, thinner pools, wider bands | quiz mean median error 18 to 23 (vanilla ranged), 24 to 30 (melee), 27 to 34 (apparel), 16 to 23 (CE ranged), 32 to 36 (CE melee), 29 to 34 (CE apparel) |
| careless answers (25 percent dropped, 15 percent one bin off, rival off by up to 2 ranks) | stored bands come from a noisy run | mean median error rises 0 to 5 points, still below the tier median |
| fewer than about 8 items of a kind | quiz hidden, simple mode only, price and work formulas hidden | not measured |
| role or tier unknown for a mod item | user answers; pool "any" | not measured |
| a DLC missing | pool shrinks; roles under 2 items fall back to the kind; fewer comparison questions | not measured |
| a stat with no reference values | plain input with no estimate | CE melee penetration (8 of 14 items) |
| the user answers "not sure" | question skipped, predictor falls back, band widens | by design |
| the game or CE updates | cache key changes, recalibration job, old result usable with its date | by design |

Nothing in the model needs a minimum number of questions.

## 11. Measured results

### 11.1 Approach A (formula and distribution first)

Leave-one-out over resolved research items. Cells are the mean over scored stats of the median / p80 relative error, percent. "nn" is a nearest neighbour given the same answers.

| set | n | tier median | nn simple | simple | nn quiz | quiz | quiz, careless | quiz, half the install |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged | 19 | 28 / 89 | 29 / 66 | 27 / 72 | 24 / 52 | 18 / 38 | 22 / 50 | 23 / 54 |
| vanilla melee | 17 | 33 / 60 | 40 / 71 | 26 / 60 | 32 / 63 | 24 / 49 | 24 / 52 | 30 / 76 |
| vanilla apparel | 96 | 54 / 118 | 34 / 81 | 27 / 65 | 27 / 90 | 27 / 70 | 29 / 76 | 34 / 78 |
| CE ranged (no caliber stats) | 25 | 39 / 102 | 23 / 69 | 27 / 95 | 13 / 52 | 16 / 42 | 17 / 67 | 23 / 51 |
| CE melee | 17 | 46 / 80 | 43 / 80 | 40 / 90 | 43 / 94 | 32 / 75 | 35 / 96 | 36 / 85 |
| CE apparel (no blunt) | 126 | 68 / 188 | 33 / 106 | 43 / 137 | 27 / 73 | 29 / 91 | 32 / 106 | 34 / 101 |

Findings: the formula coupling (ranged damage solved to a target P4, pricing from P4, tier and mass) cut vanilla ranged market value error from 47 to 16 percent against the tier median and costs 17 points when removed, but market value is now an identity output, so the coupling survives only as the cost-list proposal and the price sanity band. Interval answers cut the errors of the stats they constrain by 2 to 5 times. Apparel simple mode (27 / 65) was as good as the apparel quiz (27 / 70) in A. Nearest-neighbour copying ties or beats the formula on the CE ranged median (13 against 16) and CE apparel (27 against 29) because CE conversions of vanilla items are sibling-like.

### 11.2 Approach B (anchor and comparison first)

Macro summary: median over stats of the per-stat median relative error (fractions).

| pool (n) | simple | quiz | quiz noisy | tier median | NN kind and tier | role and tier median |
| --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged (19) | 0.24 | 0.14 | 0.25 | 0.33 | 0.50 | 0.24 |
| vanilla melee (14) | 0.38 | 0.17 | 0.25 | 0.26 | 0.33 | 0.38 |
| vanilla apparel (112) | 0.41 | 0.07 | 0.14 | 0.50 | 0.06 | 0.29 |
| CE ranged, from scratch (26) | 0.17 | 0.14 | 0.15 | 0.34 | 0.35 | 0.12 |
| CE ranged, conversion (25) | 0.12 | | | 0.43 | 0.50 | 0.12 |
| CE melee (14) | 0.51 | 0.34 | 0.33 | 0.38 | 0.36 | 0.50 |
| CE apparel (81) | 0.74 | 0.17 | 0.26 | 0.67 | 0.20 | 0.33 |

Findings: the quiz fixes strength-and-scale stats (mass, melee damage and cooldown, apparel ratings, CE magazine and Bulk) and does nothing for class constants; with realistic noise the gain roughly halves and for vanilla ranged it vanishes on the macro measure (0.248 against 0.244 for a role and tier median); the nearest neighbour wins vanilla apparel only because of near twins (twin-free quiz 0.14 against the neighbour's 0.23). Selected per-stat quiz medians: vanilla ranged mass 0.14, vanilla melee damage 0.07 and cooldown 0.03, vanilla apparel mass 0.03 and sharp rating 0.06, CE ranged magazine 0.17 and CE apparel Bulk 0.09.

### 11.3 Harmonized comparison and the verdict

On the stats both prototypes predict (full method and script in [the calibration README](../research/data/item-calibration/README.md)), mean median error in percent:

| set | quiz A / B | noisy quiz A / B | simple A / B |
| --- | --- | --- | --- |
| vanilla ranged (5 stats) | 14.2 / 19.7 | 20.1 / 22.6 | 29.0 / 22.4 |
| vanilla apparel (mass, sharp) | 13.0 / 4.5 | 18.1 / 16.2 | 19.6 / 34.7 |
| CE ranged (8 stats) | 14.7 / 14.2 | 16.1 / 14.8 | 26.5 / 24.0 |
| CE apparel (4 stats) | 30.7 / 17.2 | 33.1 / 27.2 | 45.2 / 53.0 |
| CE melee (5 stats) | 34.6 / 42.3 | 36.7 / 41.2 | 41.6 / 48.5 |

Under noise no gap exceeds 6 points, so on measured error neither is decisive. The choice therefore rests on the other criteria:

| criterion | A | B | consequence |
| --- | --- | --- | --- |
| explainability | pool label and quantile | named anchors with real numbers, swappable | anchors in the UI for simple and quiz calibration |
| quiz length | 4 to 8, skips idle questions | 6 to 9, cap 10 | adaptive skipping from A, bisection from B, cap 9 |
| degradation without CE | explicit chain, thinning test | pool widening, noisy test | chain from A, noisy bands from B |
| implementation cost | quantile, ridge, intervals, couplings | distance, elasticity, buckets, bisection, rule selection | one engine with three predictors; the rule selector makes the choice free per stat |
| unusual items | quantile can mislead | anchor survives | keep anchors |

### 11.4 Limits of the evidence

1. Pools are 14 to 26 items (81 to 126 for apparel with near twins). A p80 from 17 items is the 14th ranked error and moves by 10 points when one item changes. Differences below about 5 points are noise; no significance tests were run.
2. Both authors made choices after seeing the same data (A: tier rank regressor, interval widening, flat rule, AP rule dropped, CE melee blunt penetration dropped; B: anchor weights, K, nested rule, buckets). Fresh data will probably be worse than the tables: A's tuning bought about 3 points.
3. The simulated modder knows the strength through the index the model uses, which flatters the quiz; CE indices are the prototypes' own proxies; CE results mostly measure how CE converts vanilla items.
4. Whether modders can name a rival or judge "lighter than the anchor" is a UX hypothesis, not user tested.
5. The right second dataset is the owner's own CE patches and the third-party CE patch folders counted in [Combat Extended patch conventions](../research/ce-patch-conventions.md); neither was used.

## 12. Acceptance gates

1. Formula vectors RG-1 to RG-3, AR-1 to AR-5, ML-1, ML-2, MV-1, MV-2, AP-1 to AP-3, CE-1 to CE-4, PI-1, PI-2, FM-1 pass to 1e-4 (values in sections 3 and 4). Install-backed vectors (revolver cycle 1.9 s and nominal DPS 6.32, assault rifle cycle 3.033 s and 10.88, AP 0.165; plate armor in steel sharp 0.81 and value 460; longsword stat 8.60 and in-fight 7.96; mace 7.02 and 7.01) pass as `#[ignore]` tests.
2. The meets-armor function reproduces the six worked examples of the CE note within rounding on the user's CE.
3. `loo::validate` over the research datasets (loaded through the def engine from an install, not from the repository) reproduces approach A's and B's simple and quiz numbers within 2 points when configured as those prototypes, which validates the port, then the hybrid is run: on each set's shared stats its mean median error must be at most 2 points above the better of the two prototypes, in ideal and in noisy mode. If it is not, the shipped default for that kind falls back to the better prototype's predictor set.
4. Determinism: byte-identical calibration cache at 1 and 8 threads.
5. The role assigner agrees with the research role table on at least 90 percent of items.
6. With CE absent, a repository scan finds no CE value table, and the disabled CE patch toggle reports its reason (test: `design.ce-absent`).
7. Held-out check when available: run the hybrid on the owner's own CE patches as a second pool; report the gap to the in-sample figures in the calibration README and tighten the stored bands if coverage at P80 drops below 0.70.

## 13. Owner decisions, conflicts and open points

Owner decisions:

1. Approve or change the bin edges in section 8.3 (they become visible labels).
2. Review the three CE strength index proxies (section 4) in a short design review; they decide what "stronger than" means in the CE quiz.
3. Decide the quiz caps (9, 9, 7, 9, 9) and the comparison budgets (3 weapons, 5 apparel); the budgets are measured, the totals are not.
4. Decide whether the owner's own CE patches (external drive) may be used as a held-out pool and as optional reference items.
5. Decide whether the typicality score is shown by default or only on request.

Conflicts recorded:

1. The two designs disagree on the apparel quiz: A recommended cutting it to 3 questions (simple mode as good as quiz), B shows the largest gains there (0.41 simple to 0.07 quiz, 0.14 noisy). The hybrid keeps anchor comparisons for apparel because under noise the gap to A is 2 points and the anchor is the explainable interface, but this is the weakest-supported choice and the first one the held-out pool should test.
2. The designs disagree on ranged damage: A solves damage from the P4 target, B found ratio scaling better (0.29 against 0.34). The hybrid follows B and keeps the solve as a helper.
3. The architecture's `fit::score` returns per-stat bands; the typicality score is an addition to that return type (`FitReport`), to be reflected in the crate catalog when M5 starts.
4. The cache key in the architecture is the CE version hash; calibration also depends on the reference set and the index definitions, so the key in section 5 is wider.
5. The CE auto-patcher note caps the quiz at five questions; the hybrid's phase 1 meets that, phase 2 is optional.

Open points: the rule-based role assigner and the hybrid are unbuilt and unmeasured; CE hit chance is not modelled; weapon quality, attachments, bipods and unique-weapon traits are base values only; Odyssey unique weapon traits are not modelled; the 0.6, 0.25, 0.15 armor weights are assumptions.

## 14. As built in the 0.1.0 backend

The math of weapons (ranged and melee), price, stats, armor, the baseline, the quiz, the harness and the fit meter is implemented in `rimstudio-design` and verified on fictional data; apparel math (`apparel`, coverage and the material matrix) is not built. Where this section differs from sections 5 to 9, this section is what the code does.

### 14.1 The model and the rule selection

1. The engine is `baseline::Model::fit(Pool, BaselineConfig)`. It owns the pool, the per stat elasticity, the stat traits (discrete, integer, ask), the bucket ratios, the nested rule selection scores and the internal bands, and offers `estimate(&BaselineInput) -> Estimate`, `estimate_with_bands`, `class_stats`, `anchors` and `quiz_available()` (a pool of at least 8 items, `MIN_QUIZ_ITEMS`). The pure forms `simple_estimate(&ClassStats, StrengthChoice, typed values, &BaselineConfig, Option<&BandTable>)` and `anchor_baseline(&[Anchor], &ClassStats, &AnchorInput, &BaselineConfig, Option<&BandTable>)` take the configuration and optional calibrated bands in addition to the sketch of the crate catalog. Without anchors the anchor predictor falls back to the quantile predictor.
2. The three predictors are `Median`, `Quantile` and `Anchor`; the quantile predictor uses mass as a regressor only when the mass stat is known (answered, typed or from an anchor bucket), and the nested rule selection runs without mass, so its choice reflects the simple mode situation. Monotonicity in strength is exact for the quantile predictor and for `simple_estimate` (property test); the anchor predictor is only statistically monotone (asserted within 3 percent on a noise free power law pool), because the nearest anchor set can switch.
3. Defaults of `BaselineConfig`: two anchors, role weight 2.0, tier weight 0.35, group weight 0.8, anchor epsilon 0.3, prior spread 0.6, same band 0.10, bucket edge ln(1/0.85), ridge 0.2 for quantile and 0.5 for elasticity, flat factor 1.16, minimum pool 3, rough below 15 items. The strength choices Weaker, Typical and Stronger map to the percentiles 1/6, 1/2 and 5/6.
4. The only structural identity implemented is melee swing damage = strength x cooldown (`Identity::StrengthTimes`); the apparel coverage law, the CE blunt to sharp ratio and the CE conversion identities are not implemented (CE conversions use the predictors of section 14.4).

### 14.2 Proposals, predictions and the fit meter

`fit::score(&Proposal, &ClassStats, &CalibrationMetrics) -> FitReport` (and `score_with(.., CalibrationMode)`) takes the predictions through `Proposal.predicted`: a stat without a prediction is centred on the class median. The bands come from `CalibrationMetrics`: the noisy run for the quiz and the simple run for simple mode. `StatFit` adds `stat`, `predicted`, `nearest_reference` and `default_band` to the fields of the crate catalog sketch; `FitReport` adds `typicality` (0 to 100), `summary` and `notices` (calibration time, pool size, class label, class n, rough, optimistic twins). Levels are Typical, Plausible and Unusual; there is no red. The typicality of a peer set is `fit::typicality_of` and vector FM-1 holds.

### 14.3 Pools, roles and tiers

Roles and tiers follow the rule in the module documentation of `classes`, not a role table: tier is the explicit field, else the stat tier or tech level, else terciles of the strength rank; role is the explicit field, else the most common shared tag, else `any`. The weapon reader (`reader`) decides roles through the editable `RoleRules` decision list. Melee roles come from tool capacities (piercing, blunt, blade). Ranged roles come from the weapon classes and tags the game uses to group guns plus two plain thresholds: a tag containing `NeolithicRanged` is a bow, the class `LongShots` a sniper, `ShortShots` with a range below 16 a shotgun, `ShortShots` with `RangedHeavy` a submachine gun, `RangedHeavy` with a mass of at least 8 a heavy gun, `RangedLight` with a mass below 3 a pistol, and any other gun tag a rifle; a weapon no rule claims takes the most common shared tag. On the owner's install this agrees with the research role table on all 19 shared direct fire weapons, so gate 5 of section 12 holds for ranged weapons (the prototype has no melee roles).

A weapon candidate is a def that is no race or building and either has non empty `weaponTags` or is primary equipment (`equipmentType` Primary) filed under a thing category whose name contains `Weapons` (`reader::is_weapon_def`; a wieldable log, drug or horn has neither, so it is not a weapon, while a modded weapon without tags is). It is ranged when a verb has a default projectile (a weapon with several verbs is read from the first such verb) and melee when it has tools and no verbs; a weapon whose verbs have no projectile (beams and flames) is left out silently. Exclusions (hidden, destroy on drop, turret and mechanoid tags, `_Unique`, one use and launcher verb classes, explosive projectiles, custom verb or tool classes) are editable data and every excluded candidate is listed with its reason. The damage kind of a melee capacity (which material multiplier applies) is read from the install's maneuver definitions (capacity, maneuver, damage definition, armor category) and falls back to the names `Cut` and `Stab` as sharp, anything else blunt, when the install has no maneuvers. A pool item's `market_value` stat is the explicit `MarketValue` or the formula price (`reader::market_value_of`, read for the reference material when the weapon is stuffed). The reference armor of the P4 index is derived as (0, median, 90th percentile) of the positive sharp ratings of fixed apparel, falling back to 0, 0.5 and 1.0 with the diagnostic `reader.armor-fallback` when fewer than 3 such items exist; on the owner's install the derived layers are 0, 0.92 and 1.2 (30 fixed armor items), which scales every ranged index by about 0.8 against the prototype's layers 0, 0.55 and 1.0 and leaves the ranking unchanged (Spearman 0.9985).

### 14.4 CE pools and conversion prediction

CE pools carry `vanilla.<stat>` and `ratio.<stat>` stats per converted weapon. The numbers of the convert flow no longer come from `predict_conversion` but from the similarity estimator with reliability ratings of [Combat Extended patching](combat-extended-patching.md) section 14.9; `predict_conversion` is kept as the pool based predecessor for comparison. The paragraph below describes that predecessor. The CE strength proxy follows section 4 (damage x pellets x burst / cycle x sqrt(range / 25) x (1 + 0.1 x sharp penetration); melee mean tool power over mean tool cooldown), with the weight and the range reference as data (`StrengthDefinition`). `predict_conversion(&Pool, &ClassKey, vanilla values, &CeClassOptions)` gives per stat a value with predictor `Identity`, `Median`, `Ratio` or `Elastic`, its driver, band, n, level and leave one out error. On the owner's install the CE ranged pool has 17 items (turret, unique and launcher guns are excluded structurally) and only 18 of 67 converted guns find a vanilla twin, so the ratio and elastic predictors rest on small classes; predictions carry their n and band. CE weapons are left out of a pool when they carry an exclusion reason.

### 14.5 The quiz

1. Interval question bin edges are the quartiles of the class values rounded to two significant digits instead of the round number edges of section 8.3; the labels (under X, X to Y, over Z) are produced from the edges. This is table free, and the edges are no longer an owner decision (decision 1 of section 13 now concerns only the labels).
2. Question ids embed the shown item ids or the bin edges, so a changed pool gives `quiz.stale` and `truncate_stale` drops the stale tail. Answers are tier, role, group, weaker, same, stronger, closer to lower or upper, bin, bucket, typed, not sure, skip and use what I have; `back()` returns the last answer. The comparison budget is 3 for weapons and 5 for apparel; the default refine lists use the conventional stat names (mass, range, warmup, cooldown, insulationCold) and are data.
3. Answer profiles of the harness: the profiles are `ideal`, `noisy` (standard deviation 0.2 and 0.1, 10 replicates), `noisy_wide`, `dropped(rate)`, `simple` and `class_median`. The dropped profile applies its drop rate to the comparison and refine questions, so a dropped comparison ends the dialogue; section 9.1 and the degradation table describe dropping optional answers only plus noise on comparisons, and the code is stricter.

### 14.6 The harness

`loo::validate(&Pool, &BaselineConfig, &ValidateOptions) -> CalibrationMetrics` runs the nested leave one out with seeds `1000 + index + replicate * 7919` and a thread count in the options; the output is byte identical for any thread count. `CalibrationMetrics` holds per variant and per stat `median_error`, `p80_error`, `factor_p50`, `factor_p80`, `shift`, coverage and rank correlation, plus the twin check (`plain_error`, `twin_free_error`, `gap`, `optimistic`). `HARNESS_VERSION` belongs to the cache key; the toolkit stores the result in the cache root (see the items toolkit section 15.6). The install backed numbers of gate 3 (the hybrid against the Python prototypes) are not measured; all behaviour is verified on fictional synthetic pools (`classes::synthetic`).

### 14.7 Exact math as built

Presets and rules are plain structs the caller fills; the crate embeds no value table. Armor survival uses the piecewise formula (1 minus 0.75 e, 0.5 minus e/4, 0 above 2, rating clamped to 2). Inputs that are not finite or are negative return `DesignError::InvalidInput` and never panic. The `ce::formulas` module offers single point constants, inclusive float ranges, clamped interpolation (`Curve`) and the toughness and race armor rules as functions over caller supplied rules. Vectors use fictional numbers; install backed checks are `#[ignore]` tests that need `RIMSTUDIO_GAME_DIR`.

### 14.8 Numeric parity with the research prototypes

The ignored test `crates/rimstudio-design/tests/real_parity.rs` (environment `RIMSTUDIO_GAME_DIR`, only reads) loads the owner's install through the def engine, reads every weapon with `design::reader` and compares the Rust numbers with the prototype tables of `docs/research/data/vanilla-weapons` and `vanilla-apparel`, which it reads at run time. Tolerance: 2 percent relative. Result on the owner's install (RimWorld 1.6, the six official packs): every quantity agrees to within 0.05 percent, which is the four significant digits of the prototype CSV files.

| Quantity | Rows | Max deviation |
| --- | --- | --- |
| Ranged: cycle, nominal DPS, hit adjusted DPS at 3, 12, 25 and 40 cells, implied AP, damage, range, warmup, cooldown, P4 strength, market value | 20 weapons | 0.05 percent |
| Melee: stat panel damage, cooldown, DPS and AP; in fight damage, cooldown, DPS and AP; P_M strength | 92 (weapon, material) rows over 16 materials | 0.05 percent |
| Melee market value | 72 rows (the 20 silver and gold rows are listed in the errata) | 0.05 percent |
| Worked examples of the notes (revolver, assault rifle and the other ranged examples; longsword 8.60 and 7.96, mace 7.02 and 7.01 and the other melee examples in steel, plasteel and Legendary steel) | 132 numbers | 1e-6 relative |
| The 18 Normal quality worked apparel variants of the apparel note (plate armor in steel, plasteel and wood and the helmets, jackets and others): sharp, blunt and heat rating through the stat pipeline, displayed price, layer survival at AP 0, 0.15, 0.165 and 0.30; the stacked layer table; the torso example of 0.3021 | 157 numbers | 0.02 percent (the notes round to four digits) |

Pools: the ranged pool holds 20 weapons and the prototype's direct fire set 19; the extra one is the nerve spiker, which the prototype set aside as a creature weapon through a hard coded name although it is craftable by the player. The melee pool holds 17 weapons, the same 17 as the prototype (bladelink variants included), with identical tiers; the 19 shared ranged weapons have identical tiers and roles. What is left out, and why: 22 turret, mechanoid and creature guns (`destroyOnDrop`), 13 `_Unique` duplicates, 4 launchers (explosive projectile), 2 one use rocket launchers and 4 grenades (verb classes `Verb_ShootOneUse` and `Verb_LaunchProjectile`), the charge lance (tag `MechanoidGunMedium`, a mechanoid weapon the prototype also excludes), and the beam and flame weapons whose verbs have no projectile (`Gun_BeamGraser`, `Gun_Incinerator`, `Gun_MiniFlameblaster`); the turret mortar and rocket swarm have no weapon tags and no weapon category. Logs, a drink, horns and tusks carry tools and are wieldable but are not weapons. No vanilla weapon has several verbs, and all five bows and the thrown spear are in the ranged pool.

Defects the parity check found, all fixed in the Rust:

1. The sharp and blunt damage multipliers of a stuffed melee weapon were read as stuff factors in `stuffProps`. The game reads them from the material's own stats (`Tool.AdjustedBaseMeleeDamageAmount` multiplies by `stuff.GetStatValueAbstract(armorCategory.multStat)`), so every material except steel (multiplier 1) gave wrong melee damage; plasteel was 10 percent off.
2. A damage definition without an armor category has no armor penetration in the game (`ProjectileProperties.GetArmorPenetration` returns 0); the reader gave it 0.015 per damage point.
3. Ranged roles came from the first shared tag (Gun, RangedLight), so no ranged role agreed with the research table; the decision list of section 14.3 replaced this.
4. The pool stat `market_value` held only an explicit `MarketValue` (2 of 20 weapons); it now holds the formula price for all.
5. Gaps closed on the way: the stat definitions' `roundToFiveOver` and `roundValue` were not read; a melee capacity's damage kind came from its name only; a weapon without `weaponTags` was invisible even when it is primary equipment in a weapon category.

Errata of the research prototypes and of this specification (the research notes are evidence and are not edited):

* The weapon extractor (`extract.py`) prices a stuffed weapon with a stuff volume of 1 for every material because it looks for a `volumePerUnit` element that does not exist. The game divides the stuff count by 0.1 for a `smallVolume` material, so the 20 silver and gold rows of `melee_stuff_table.csv` understate the stuff part of the price by a factor of ten (axe in gold: 5,022.68 against the prototype's 522.7). The apparel extractor applies the rule and is right. Section 3.4 is correct.
* The prototype's `power` column and the P4 values quoted in section 4 (assault rifle 5.674, revolver 2.935) hold for the reference layers 0, 0.55 and 1.0 (flak jacket and flak vest), not for the layers derived from the install; with the derived layers the index is about 0.8 times as large.
* The prototype classes the nerve spiker as a creature weapon, so its direct fire pool has 19 weapons and the Rust pool 20 (section 5 and the spec text "19 direct-fire ranged" describe the prototype).
* The in fight selection of the game adds 0.1 to a verb's chance factor for each additional hediff of its damage definition (`VerbUtility.AdditionalSelectionFactor`). Only creature damages (toxic claws and bites) have additional hediffs, so no vanilla weapon is affected and both implementations omit it (section 3.3 already says so). The game computes in single precision; a selection score exactly on the 0.95 or 0.25 boundary can round differently, and no vanilla weapon sits on a boundary.

## 15. Weapon archetypes

An archetype describes a weapon the way a modder talks about it: an assault rifle, fast, medium calibre, typical for its class. The solver turns that description into every number of the vanilla specification, so a first complete weapon exists before any number is typed. It is implemented in `rimstudio-design::archetype` and reached through `designer_archetype_catalog`, `designer_archetype_propose` and `designer_archetype_apply` ([items toolkit](items-toolkit.md) section 16, [ADR 0058](../adr/0058-weapon-archetypes.md)). The measured accuracy is in [the archetype validation note](../research/archetypes-0.1.0.md).

### 15.1 The taxonomy

The taxonomy is RimStudio's own JSON (`crates/rimstudio-design/data/archetypes.json`, embedded in the binary, R10). It holds names, unit free counts and RATIOS to medians that the solver reads from the user's install; it holds no game value (R11). Ranged families: pistol (light, revolver, heavy, machine pistol), submachine gun (light, heavy, personal defence weapon), shotgun (pump, semi automatic, automatic), rifle (carbine, assault, battle, bolt action hunting, designated marksman, sniper, anti materiel), machine gun (light, general purpose, heavy) and bow (short, recurve, great, crossbow). Melee families: knife, sword (short, long), axe, mace and hammer, spear and polearm, club and staff. Each archetype names its allowed actions (bolt, lever, pump, semi automatic, burst, full automatic, draw), rate of fire classes, calibre classes and handling classes, the role names of the pool it is compared with, a tech level hint, its shape and its strength shift.

The descriptors are: action, rate of fire (a class `slow`, `medium`, `fast` or rounds per minute read against the typical rounds per minute of the archetype), calibre (a class from `tiny` to `huge`, or in Combat Extended mode a real ammo set), handling (`compact`, `standard`, `heavy`), tier (the install's tech levels) and the balance target (`weaker`, `typical`, `stronger`, a percentile, or an exact strength index). A descriptor the archetype does not offer is refused with its name; a gun descriptor sent for a melee weapon is ignored with a note.

### 15.2 The solver

1. **Medians.** The solver reads, at run time, the medians over the reference pool of the kind: damage, range, warmup, cooldown, mass, work, ticks between burst shots (burst weapons only) and the four accuracy values for guns; swing damage, cooldown, mass and work for melee weapons. Each median carries the number of weapons behind it.
2. **Shape.** Every stat is its median times the ratios of the archetype, the action, the calibre (relative to the archetype's default calibre), the handling and the rate of fire: cooldown divided by the rate to the power 0.8, burst count scaled by the rate to the power 0.7 (rounded, inside the limits of the action), ticks between shots divided by the rate to the power 0.5. A melee shape lists tools whose power is a ratio of the median swing damage and whose cooldown is a ratio of the median cooldown. Armor penetration is the implied 0.015 per damage point times the ratio of the archetype (a ratio of 1 leaves the field unset and the game derives it).
3. **Scale.** The strength index of the shape (`ranged::strength_p4` with the reference armor of the install, `melee::strength` for tools) is compared with the strength to aim at, and one scale `s` is found by bisection (60 steps, limits e^-3 to e^3). Damage (tool power) is multiplied by `s^0.65` (`s^0.7`), the cooldown by `s^-0.2` (`s^-0.3` for melee tempo), the warmup by `s^-0.15` and the mass by `s^0.25`. Range and the accuracy profile never change with the scale, so the shape of the archetype holds. The exponents are generic tuning constants stored in the taxonomy file. A faster rate of fire never raises the cooldown and a heavier calibre never lowers the damage, because the scale only grows or shrinks damage and tempo together.
4. **Rounding.** The values are rounded the way the install writes them: ranges keep the fractional part that at least 70 percent of the reference ranges share, warmup and cooldown go to 0.05, accuracy to 0.01, damage to a whole point (the solver tries the floor and the ceiling and keeps the one whose strength is nearer the target), mass to 0.05 below 2 kg and 0.1 below 10, work to two significant digits, tool power to a whole point and tool cooldown to 0.1. The strength of the rounded numbers is the one reported.
5. **The strength to aim at.** `strength_target` asks the baseline model for percentile `p` (1/6, 1/2 or 5/6 for the three named targets) of the class of the archetype's pool role and tier through the calibration fallback chain (role and tier, role, tier, all), so a thin pool widens and the proposal says so ("based on 3 items of role pistol, widened from role and tier"). When the chain widened past the tier and the pool holds several tiers, the target moves along the fitted trend of the logarithm of the strength with the tier. The strength shift of the archetype (how strong the archetype typically is against the middle of its class, in natural log units) is then added: it lets a great bow be typically stronger than a short bow although both belong to the role `bow`. An exact strength index (`strength` in the request) replaces the whole target.
6. **The rest of the weapon.** The bash tools of a gun are the medians of the matching tools of the install's guns. The weapon tags are the tags at least 40 percent of the comparable reference weapons carry (same role and tier, else same tier, else same role, else all). The weapon classes are the hints of the archetype that some reference weapon carries. The cost list is the ingredients that at least half of the tier's reference weapons with a cost list need, at the median count scaled by the square root of the size (mass against the median mass) times the strength ratio to the power 0.5. A melee weapon gets the stuff categories of its archetype that the install's weapons accept and a stuff count in the same way. The market value is the price math (section 3.4) of that cost list, the stuff count and the work; it is shown, not written.
7. **Combat Extended mode.** The calibre is one of the ammo sets of the user's install (the catalogue of `design::ce::ammo`): the damage per shot of the first ammo type (pellets included) against the median set of its caliber family gives a ratio, and the vanilla damage factor is that ratio to the power 0.7 (range 0.1, mass 0.25, penetration 0.15, cooldown 0.12; ratio limited to 0.4 to 3). The proposal names the ammo set, its caliber, the first projectile and the AI class tag of the install whose name contains one of the archetype's hints. The numbers of the optional block are filled only when the apply request says so, by the existing predictors from the vanilla numbers. The vanilla definition is never changed by any of this (D-085).

### 15.3 Explanations, sources and typed values

Every proposed number carries a plain reason ("range 38.9: the install's median is 25.9 (20 reference weapons). Bolt action hunting rifle x1.42, large calibre x1.06"), the median and its count, and the list of multipliers; the proposal as a whole carries the class label, the strength target and the strength reached, the notes (widened class, tier trend, ignored descriptors, missing data) and the fit meter's report with a verdict (typical when at most a tenth of the stats that are not asked are unusual and at least half are typical, plausible when at most a quarter are unusual). The source chip is `archetype`; in a draft the numbers are stored as `suggested`, so the offer rule of IT-003 holds: a typed, answered or anchored value is never replaced and a value an earlier proposal wrote is replaced by the new proposal. A single shot proposal removes the burst count and ticks that an earlier burst proposal wrote. The choice (archetype, descriptors, target, mode) is recorded in the draft (`archetype`, additive: older drafts load unchanged) and, in simple mode, the target is recorded as the strength choice the fit meter compares with.

### 15.4 Existing weapons

`archetype::derive` names the archetype of an existing weapon from the ordered rules of the taxonomy (`deriveRules`: conditions on the burst, the tier and the ratios of damage, range, warmup, cooldown and mass against the install's medians, and on the capacities of melee tools) and the descriptors from its numbers: the action from the burst and the tags, the calibre from the damage class (the real calibre of a vanilla gun is unknown), the handling from the mass and the rate of fire class whose shape cycle is nearest to the real cycle. The validation harness uses it.

### 15.5 Measured results

On the owner's install (20 guns and bows, 17 melee weapons; leave one out pools and medians) the proposals for the typical target have a median error of 17 percent for damage, 0 for range, 15 for cooldown and 6 for mass over the guns and bows, and 12 for swing damage, 8 for cooldown and 10 for mass over the melee weapons; 35 of 37 proposals are typical or plausible on the fit meter (the meter says the same of 36 of 37 real weapons), none misses its strength target by more than 15 percent, and the orderings of the families hold. With the real strength of the weapon given, the damage error is 10 percent. In Combat Extended mode the proposed AI class tag equals the real one for 12 of 13 converted guns that have a twin. The full tables, the tuning steps and the limits are in [the archetype validation note](../research/archetypes-0.1.0.md).
