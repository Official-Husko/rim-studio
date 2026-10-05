# Item designer baseline, approach B: anchor and comparison first

This note designs how the item designer (R7) computes a well-fitting baseline for a new weapon or apparel item from a few user inputs, in vanilla mode and in Combat Extended (CE) mode, with a simple mode and an optional quiz mode. The baseline comes from real reference items: the user is led to one to three anchors, continuous stats are ratio-scaled from them, and the quiz is a short comparison dialogue ("is yours stronger than the Assault Rifle?") that finds the item's place in a power-sorted reference list. It is one of two competing designs; the measurements come from the prototype in this folder ([README](README.md), [results.json](results.json)).

Status: draft | Last updated: 2026-10-04

## 1. Verdict in brief

1. Where the quiz helps, it helps a lot: with ideal answers the median relative error of mass drops from 0.24 (class median) to 0.14 for vanilla ranged and from 0.37 to 0.03 for vanilla apparel, melee damage drops from 0.33 to 0.07, and CE magazine size from 0.50 to 0.17.
2. With realistic answer noise the gain shrinks by roughly half, and for vanilla ranged weapons it vanishes on the macro measure (0.248 noisy versus 0.244 for a role-and-tier median). The honest claim is "the quiz fixes the stats that depend on strength and scale (mass, melee damage and cooldown, apparel ratings, magazine) and does nothing for class constants".
3. For CE class constants (warmup, spread, sway, cooldown) a median of the same role and tier is as good as anything, and the engine selects that rule per stat by a nested leave-one-out on the user's own reference set.
4. Samples are tiny (14 to 26 items per pool, 112 and 81 for apparel with many near twins). All numbers are indicative, they rank methods and do not give guarantees (section 9.4).
5. The quiz costs a mean of 6 to 9 taps (hard cap 10) of which two are setup taps for tier and role.

## 2. Model

### 2.1 Reference pool

A pool is the set of reference items of one kind (ranged, melee, apparel) in one mode. In vanilla mode the pool is read from the user's resolved vanilla defs. In CE mode it is the user's hand-tuned CE conversions, read through the def engine at runtime ([def engine semantics](../../../def-engine-semantics.md), [CE model](../../../combat-extended-model.md)). No vanilla or CE value is stored in the repository (R11). The user's own CE patches (for example the mods on the external drive) can be added to the pool as extra anchors, per project, by the user's choice ([autopatcher formulas, section 7.3](../../../ce-autopatcher-formulas.md)).

Each reference item has five features: kind (hard filter), role (a small label set, section 4.1), tier (ordinal 0 to 5, Neolithic to Archotech), a strength index P (a positive scalar, section 2.2) and its stats. In CE mode the item additionally carries a fire-pattern class (single, burst or auto 2 to 6, sustained 7 or more) for ranged weapons.

### 2.2 Strength index

Strength is the single axis the quiz measures, so it must be explainable on one card and orderable.

| Kind | Strength index P | Source |
| --- | --- | --- |
| vanilla ranged | the power index P4 (damage, burst, cycle, hit chance, armor, square root of range) | [vanilla ranged weapons](../../../vanilla-ranged-weapons-analysis.md), section "Power index"; Spearman 0.94 with market value |
| vanilla melee | damage per second over the weapon's tools | `dps` column of the melee table |
| vanilla apparel | armor power index (api) plus a floor of 0.05 | [vanilla apparel](../../../vanilla-apparel-analysis.md) |
| CE ranged | damage x pellets x burst / cycle x sqrt(range / 25) x (1 + 0.1 x sharp penetration) | this prototype's own proxy (unverified as a human-perceived order) |
| CE melee | mean tool power / mean tool cooldown | prototype proxy |
| CE apparel | sharp rating (or stuff thickness) + 0.15 x blunt rating, plus a floor of 0.05 | prototype proxy |

The CE indices are proxies defined here because no research note fixes one. The product shows the index components on each anchor card so the user compares like with like, and the index is replaceable data (a JSON definition per kind and mode, R10).

### 2.3 Anchor selection

For a target with role r, tier t, strength estimate P and (ranged) fire class c, each reference item a has a distance

`d = sqrt( (2.0 [role differs])^2 + (0.35 |tier difference|)^2 + ((ln P - ln P_a) / sd)^2 + (0.8 [fire class differs])^2 )`

where sd is the standard deviation of ln P in the pool. The K = 2 nearest items that have the stat in question are the anchors, weighted by 1 / (d + 0.3). The weights and K were chosen after a sweep of 18 settings ([sweep.json](sweep.json)): the macro error varies only between 0.150 and 0.195, a spread below the noise of the data, and K = 1 is best partly because near twins flatter it (section 9.3). The product exposes the anchors to the user and lets the user swap or add one (up to three); it does not tune the weights per user.

### 2.4 Ratio scaling

For a continuous stat y the prediction is a weighted geometric mean over the anchors, shifted along strength:

`ln y = sum_j w_j ( ln y_j + b_y ( ln P - ln P_j ) ) / sum_j w_j`

b_y is the elasticity of the stat with respect to strength, fitted per pool as a within-role regression slope of ln y on ln P with a ridge term of 0.5 and clipped to [-0.5, 1.5]. Measured elasticities on all items: mass 0.51 (vanilla ranged), 0.57 (vanilla apparel), 0.34 (CE ranged); range 0.11 and 0.14; cooldown close to 0 (-0.04 and -0.09); armor ratings of apparel 0.7 to 0.8 in vanilla. A stat with b near 0 is effectively copied from the anchors, which is what the CE conventions look like.

Discrete stats (burst, magazine, reload time) are not scaled: the prediction is the anchors' weighted median, and burst is replaced by the median of the answered fire-pattern class.

CE conversion variant: when the item already has vanilla numbers (the typical "make a CE patch for this mod weapon" case) the strength index and the scaling driver of mass, range, warmup and cooldown are the item's own vanilla values, so the ratio scaling becomes "CE value of the anchor times the vanilla ratio". That is the mass rule the research note found best ([autopatcher formulas, section 6.4](../../../ce-autopatcher-formulas.md)), generalised.

### 2.5 Class constants and structural solves

In a tight class (the CE ranged cooldown, warmup and spread conventions) copying a class median beats scaling. For each stat the engine runs a nested leave-one-out inside the training set and compares the anchor rule with the median of the same role and tier (fallback: same role); it uses the better rule for that stat. Stats that are asked in the quiz always use the answer.

Two structural identities are used where the game provides them. Melee damage is solved as strength times predicted cooldown (strength is damage per second, so this is exact). Ranged damage could be solved from the P4 definition; measured, that solve is worse than ratio scaling (median 0.34 versus 0.29), so the designer uses ratio scaling for the suggestion and offers the solve only as a "pin the other stats, show the damage that keeps the strength" helper. Market value is not scaled at all: the product computes it from the cost list and work with the identity measured in the vanilla note (price is ingredient value plus a work term), because ratio scaling gave 0.38 to 0.43, no better than the baselines.

## 3. Simple mode

Inputs: kind, tier, role, and an optional strength slider that defaults to the middle of the role and tier peers. The prior strength is the mean ln P of the same role and tier (fallback role, then all). The anchors and the ratio scaling are as above. The user may also type any numbers they know (damage, range, mass); each typed number replaces the corresponding prediction and, for mass, range, warmup and cooldown in CE conversion, becomes the scaling driver. Simple mode labels itself "estimate", shows the per-stat band (section 6) and never silently falls back to a fixed preset.

Measured, simple mode behaves like a class median: for ranged and melee its macro error equals the role-and-tier median, and for apparel it is worse (0.41 and 0.74 against 0.29 and 0.33) because strength drives the ratings there (section 9.1). Its value is that it needs no questions and that its bands are honest.

## 4. Quiz mode

### 4.1 Setup taps (2)

| Id | Question | Options |
| --- | --- | --- |
| S1 | "Which tech level is it?" | Neolithic, Medieval, Industrial, Spacer, Ultra, Archotech (only levels present in the pool are enabled) |
| S2 | "What is it?" | ranged: bow, pistol, smg, rifle, shotgun, sniper, heavy or machine gun; melee: blunt, blade or piercing; vanilla apparel: helmet, torso vest, torso outer, base clothing, legwear, full body, utility; CE apparel: headgear, torso, full body, legs, other |

The role list is derived from the pool, not hard-coded.

### 4.2 Comparison dialogue (at most 5 questions)

Each question shows an anchor card (name, picture where available, the index components) and asks "Is your item weaker, about the same, or stronger?". The reference list is the pool of the same kind (for apparel, of the same role, because strength is not comparable across a vest and a hat), sorted by P.

Adaptive logic:

1. Maintain a bracket (lo, hi) of list positions known to hold the item; start with the whole list.
2. Prior: a normal distribution on ln P centred on the mean of the same role and tier peers with standard deviation 0.6.
3. Pivot: the item in the bracket whose position splits the remaining prior mass in half. This is a prior-weighted bisection, which needs fewer questions than plain bisection when the role and tier are known.
4. Answer "stronger" moves lo, "weaker" moves hi, "about the same" (a difference under about 10 percent of power) ends the dialogue with P equal to the pivot.
5. When two neighbours remain, one extra question asks "closer to {lower} or {upper}?", which places the item at a quarter or three quarters of the gap (halfway without it).
6. Stop at the cap of 5 questions or when the bracket is a single gap. The estimate is the interpolation of ln P inside the bracket; at the list edges it extrapolates by one median gap.

Measured on a pool of 19 weapons the dialogue averaged 2.0 comparisons (maximum 5) because "about the same" ends it early; it located the true strength within 0.02 ln units at the median with ideal answers. With noisy answers the stop is less often reached and the cost is about one question more.

### 4.3 Stat questions (at most 3)

| Kind and mode | Question | Answer set | Effect |
| --- | --- | --- | --- |
| ranged (vanilla and CE) | "How does it fire?" | vanilla: single, short burst 2 to 4, long burst 5 to 9, belt 10 or more; CE: single, burst or auto 2 to 6, sustained 7 or more | sets burst to the class median; adds 0.8 to the distance of anchors of another class |
| ranged | "Is its mass lighter than, similar to, or heavier than {anchor 1}?" | lighter (below 85 percent), similar, heavier (above 118 percent) | mass = anchor mass times the learned median ratio of that bucket |
| ranged | same for range | same | same for range |
| melee (both) | same for mass, and for cooldown ("slower, similar, quicker swing") | same | mass and cooldown from the bucket; damage then follows from strength times cooldown |
| vanilla apparel | same for mass, and for insulation ("colder, similar, warmer") | same | mass and cold insulation from the bucket |
| CE apparel | same for mass and for Bulk | same | mass and Bulk from the bucket |

The bucket multipliers are learned on the pool: each reference item is compared with its own nearest anchor and the median ratio per bucket is stored. The anchor shown is the nearest of the estimated strength, so the user sees real numbers to compare with. A stat that is never derivable stays a plain input, not a question: CE caliber and ammo set, CE armor in millimetres for stuffable apparel, and weapon tags ([autopatcher formulas, section 7.5](../../../ce-autopatcher-formulas.md)).

### 4.4 Counts

Maximum 2 + 5 + 3 = 10 taps (9 for melee and apparel, which have two stat questions and no fire-pattern question). Measured means with ideal answers: 7.0 (vanilla ranged), 6.2 (melee), 6.2 (vanilla apparel), 8.6 (CE ranged), 7.4 (CE apparel). The UI shows progress as "question n of about m", with m recomputed from the bracket, and offers "skip to result" after any question: the estimate is available after every answer, and the fit meter shows how much each answer tightened it (section 6).

## 5. CE mode

CE mode is the same engine with the user's hand-tuned CE conversions as the pool. Anchor cards show CE values only. Class constants (warmup, spread, sway, cooldown) dominate, so the per-stat rule selection of section 2.5 matters most; measured, the engine lands near the role-and-tier median for those and wins on mass, magazine and burst. The conversion variant (section 2.4) is the default when the item has vanilla numbers. With fewer than 3 reference items in the chosen role the pool widens to the kind and says so; with no CE installed the mode is disabled with a reason and vanilla mode is unaffected (R11: no CE number exists in the source tree). Patch output follows [CE patch conventions](../../../ce-patch-conventions.md); this design fixes only the numbers.

## 6. Fit meter and error bands

Bands. At calibration time (first use, and whenever the CE version hash or the mod set changes) the engine runs the whole procedure leave-one-out over the pool with ideal answers and stores, per stat, the residuals |ln(predicted / true)|. The band of a stat is the multiplicative factor at the 50th and 80th percentile of those residuals. Examples from the prototype (factor at P50 and P80): vanilla ranged range x1.10 and x1.15, mass x1.17 and x2.68; CE ranged range x1.14 and x1.33, Bulk x1.25 and x1.46, magazine x1.18 and x2.0; vanilla apparel sharp rating x1.06 and x1.35.

Meter. For each stat the value in the editor sits on a bar with the prediction and the two bands. It is green inside the P50 band, amber inside the P80 band, red outside. Beside it the meter shows the rank ("heavier than 12 of 19 reference rifles") because ranks are more robust than values. A summary at the top shows the share of stats that are green or amber and the number of reference items behind the bands; below about 15 items in the chosen role it states that the bands are rough.

Calibration check. Using the other items' residuals to set the bands of a held-out item, coverage was 0.47 to 0.57 at P50 and 0.71 to 0.82 at P80 across pools (burst, a discrete stat, aside), close to nominal, so the bands are not over-confident on these data. Because answers are noisy in practice, the stored bands should come from a run with simulated answer noise (the `quiz_full_noisy` variant), which widens them.

## 7. Degradation

| Missing | Behaviour |
| --- | --- |
| CE not installed | CE mode disabled with a reason; vanilla mode complete |
| CE installed, few hand-tuned items in the role | anchors widen to the kind; bands widen with the residuals; the meter shows the count |
| a DLC missing in vanilla | the pool shrinks; roles with under 2 items fall back to the kind; the comparison list shortens (fewer questions) |
| no reference item with a stat (for example CE melee penetration, 8 of 14 items) | that stat is a plain input with the pool range shown, not a prediction |
| user answers "not sure" | the question is skipped and its stat falls back to the anchor rule; the meter widens |

## 8. UX implications

1. The quiz is a dialogue of cards: one anchor card and three buttons per question, with item names the modder already knows. The anchors stay visible and swappable; "start from the Revolver" is the same engine with the comparison step skipped.
2. The cost is announced ("about 6 questions"), the estimate is live after every answer and "skip to result" is always available.
3. Every value shows its source (anchor, class median, answer, user input) and its band; damage per shot, penetration, CE spread and sway get wide bands because they are design choices, not functions of strength.
4. The leave-one-out calibration runs in the background and is cached as JSON keyed by the CE version hash and the mod set hash (R10), so the designer opens instantly.

## 9. Measured results

### 9.1 Protocol

Leave-one-out over every item of each pool. For each held-out item the simulated modder answers from its true attributes: tier and role exactly; "stronger, same, weaker" by comparing the true strength with the anchor's (a difference under 10 percent counts as "about the same"); fire pattern exactly; "vs anchor" questions by the true ratio against the first anchor, bucketed at 15 percent. Noisy variants add a normal error with standard deviation 0.2 in ln units to every comparison and 0.1 to every "vs anchor" answer, 10 replicates per item. Error is |predicted - true| / true, over items where the true value is positive. Training sees only the other items, including the bucket multipliers, elasticities and the per-stat rule selection. The simulated modder knows strength through the same index the model uses, which flatters the quiz; the noisy variants are the more realistic reading.

Baselines: tier median (same tier), nearest neighbour of the same kind and tier (told the true strength, so strong), and role-and-tier median (told the true role).

Macro summary (median over stats of the per-stat median relative error; lower is better):

| Pool (n) | simple | quiz | quiz noisy | noisy, wide "same" | tier median | NN kind+tier | role+tier median | quiz, twin-free |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged (19) | 0.24 | 0.14 | 0.25 | 0.30 | 0.33 | 0.50 | 0.24 | 0.14 |
| vanilla melee (14) | 0.38 | 0.17 | 0.25 | 0.27 | 0.26 | 0.33 | 0.38 | 0.17 |
| vanilla apparel (112) | 0.41 | 0.07 | 0.14 | 0.16 | 0.50 | 0.06 | 0.29 | 0.14 |
| CE ranged, from scratch (26) | 0.17 | 0.14 | 0.15 | 0.15 | 0.34 | 0.35 | 0.12 | 0.14 |
| CE ranged, conversion (25) | 0.12 | | | | 0.43 | 0.50 | 0.12 | |
| CE melee (14) | 0.51 | 0.34 | 0.33 | 0.34 | 0.38 | 0.36 | 0.50 | 0.34 |
| CE apparel (81) | 0.74 | 0.17 | 0.26 | 0.25 | 0.67 | 0.20 | 0.33 | 0.20 |

### 9.2 Per-stat medians (selected)

Entries are median relative error; the quiz column also gives the p80 in brackets. "Noisy" is the noisy quiz.

| Pool and stat | simple | quiz (p80) | noisy | tier median | NN | role+tier |
| --- | --- | --- | --- | --- | --- | --- |
| vanilla ranged mass | 0.24 | 0.14 (0.66) | 0.25 | 0.54 | 0.52 | 0.24 |
| vanilla ranged warmup | 0.33 | 0.33 (0.47) | 0.33 | 0.41 | 0.55 | 0.30 |
| vanilla ranged damage | 0.34 | 0.34 (0.57) | 0.37 | 0.33 | 0.52 | 0.27 |
| vanilla melee damage | 0.30 | 0.07 (0.16) | 0.15 | 0.19 | 0.25 | 0.33 |
| vanilla melee cooldown | 0.20 | 0.03 (0.14) | 0.09 | 0.14 | 0.18 | 0.18 |
| vanilla apparel mass | 0.33 | 0.03 (0.61) | 0.20 | 0.83 | 0.29 | 0.37 |
| vanilla apparel sharp rating (n = 77) | 0.36 | 0.06 (0.27) | 0.12 | 0.20 | 0.00 | 0.29 |
| CE ranged Mass | 0.49 | 0.24 (0.46) | 0.27 | 0.63 | 0.38 | 0.48 |
| CE ranged magazine (n = 22) | 0.55 | 0.17 (0.79) | 0.17 | 0.93 | 0.70 | 0.50 |
| CE ranged warmup | 0.10 | 0.15 (0.26) | 0.15 | 0.35 | 0.39 | 0.10 |
| CE ranged spread | 0.10 | 0.10 (0.80) | 0.10 | 0.34 | 0.37 | 0.10 |
| CE ranged conversion Mass (identity: 0.08) | 0.26 | | | 0.64 | 0.68 | 0.54 |
| CE apparel Bulk | 0.46 | 0.09 (0.73) | 0.25 | 0.67 | 0.20 | 0.25 |

The full tables for all stats are produced by `make_tables.py`.

### 9.3 Reading the results

1. Strength-and-scale stats improve with the quiz: mass, melee damage and cooldown, apparel ratings, CE magazine and Bulk. These are the stats a modder can judge by comparison but cannot produce from nothing.
2. Class-constant stats do not improve and sometimes get slightly worse (CE warmup 0.10 simple versus 0.15 quiz; vanilla ranged damage 0.27 for the role median versus 0.34). Damage per shot is quantised by design in vanilla and chosen by caliber in CE; no strength answer predicts it, which confirms that damage and AP should not be calibrated by the quiz.
3. Noise matters. Answer noise plus a wider "about the same" band lifts vanilla ranged from 0.14 to 0.30, worse than the role median. The stored bands should therefore come from a noisy run, and the stat questions stay because they show real numbers.
4. CE conversion from known vanilla numbers is the best CE case (spread 0.07, warmup 0.10, cooldown 0.06, range 0.14, burst exact). Mass is better handled by the identity rule (0.08 against 0.26); the product should copy vanilla mass when a vanilla value exists, as the autopatcher note found.
5. The nearest neighbour of the same kind and tier is told the true strength and still loses or ties everywhere except vanilla apparel, where it matches the quiz (0.06 against 0.07) only because the table holds near twins (kid and adult parkas, helmet variants, sets sharing ratings). With twins removed from training it rises to 0.23 against 0.14 for the quiz. K = 1 looks best for the same reason; in the twin-free rerun K = 1 and K = 2 are within 0.03 of each other everywhere.

### 9.4 Honest assessment of sample size

1. The pools have 14 to 26 items (112 and 81 for apparel). One item moves a median by several points and the 80th percentile is set by the two or three worst items, so differences of 0.03 to 0.05 between methods are not significant. Some roles have one or two members (vanilla smg, CE pistol and smg), where every role-based method falls back to wider groups.
2. Several choices were made after seeing the same data (anchor weights and K from a sweep, the nested per-stat rule, the 15 percent buckets). The sweep is flat, but the figures are optimistic by an unknown amount. A fair test needs a held-out pool, best the owner's own CE patches or the 760 third-party CE patch folders counted in the CE notes.
3. The simulated modder knows strength through the index the model uses, and the CE indices are proxies. Real modders will disagree on unusual weapons (area damage, incendiary and special projectiles are outside the vanilla index); the noisy variants only approximate that.
4. CE melee has 14 weapons and only 8 with sharp penetration; the quiz does not help penetration, Bulk or parry chance there, and the engine should label them "no estimate".
5. The question counts include early "about the same" stops that depend on a dense list; a sparse pool gives a shorter dialogue and a coarser estimate.

## 10. Owner decisions and open questions

1. Accept a hard cap of 10 taps (two setup, up to five comparisons, up to three stat questions), or a tighter cap of 7 (measured, going from 5 to 3 comparisons changes the macro error by 0.00 to 0.03, except vanilla apparel with 0.10 against 0.07)?
2. Define the CE strength index in a short design review; the proxy here is untested against how CE modders judge weapons.
3. Decide whether apparel armor for stuffable CE items gets a quiz at all: the prototype predicts only direct ratings (32 of 81 items), in line with the earlier recommendation.
4. Provide a held-out pool (the owner's CE patches) to replace the in-sample choices of section 9.4.
5. Whether the price identity (cost list plus work) should be shown next to the strength estimate as the default market value, instead of any scaled value.

## 11. Reproduction

See [README](README.md): `python3 anchor_first.py` regenerates [results.json](results.json) and `--sweep` regenerates [sweep.json](sweep.json). Evidence notes: [vanilla ranged weapons](../../../vanilla-ranged-weapons-analysis.md), [vanilla melee weapons](../../../vanilla-melee-weapons-analysis.md), [vanilla apparel](../../../vanilla-apparel-analysis.md), [Combat Extended model](../../../combat-extended-model.md), [autopatcher formulas](../../../ce-autopatcher-formulas.md).
