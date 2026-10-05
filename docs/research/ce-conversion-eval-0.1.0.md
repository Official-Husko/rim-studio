# Combat Extended conversion quality, measured (0.1.0)

Status: evidence note | Date: 2026-10-05 | Harness: `crates/rimstudio-design/tests/real_eval.rs` | Code: `rimstudio-design::ce::classes::estimate`

This note records how well the automatic Combat Extended (CE) conversion predicts the numbers of a conversion from the vanilla weapon alone, before and after the estimator of this round, and where the predictions are only guesses. It holds aggregate error figures only. It contains no per weapon CE values: the install data is read at run time by the harness and nothing from it is copied here (R11).

## 1. Question

The automatic flow converts a vanilla weapon to a CE patch. Ammo set, tag class, one handed and belt fed are asked. Everything else (mass, range, warmup, cooldown, bulk, sway, spread, sights, recoil, magazine, reload and the tool numbers of melee weapons) is derived from the user's own CE conversions. The first end to end run gave a range of 16 for a vanilla range of 44 where a hand written conversion had 55 to 68. The question: how good is each derived number, can that be measured honestly, and what should happen to a number that is not good enough.

## 2. Method

**Data.** The Combat Extended workshop mod loaded on top of the six official packs, and a second load of the official packs alone (the vanilla twins). A converted weapon has a twin when the same def name exists in the vanilla load. On the owner's install this gave 18 converted guns and 17 converted melee weapons with a twin and without an exclusion reason (turret, launcher, grenade, unique). Bows and one creature gun are among the guns, because CE converts them with the gun machinery.

**Leave one out.** Each target is predicted from the vanilla twin alone: the twin's numbers (mass, range, warmup, cooldown, burst for guns; mass, tool power and tool cooldown for melee weapons) and, for the new estimator, the twin's weapon tags and tier. The target and its own conversion are left out of every class, ratio, median and reliability measurement. Two variants run: the tag class (the AI class) answered, as the convert flow always asks it, and not answered.

**Held out.** The hand written CE patches of the mods in the owner's custom folder (16 mods loaded) were read through the engine on top of CE, with a vanilla load of the same mods for the twins. Only the owner's rifle mod has weapons that CE converts by hand and that have a twin: 5 guns, no melee weapon. They are five variants of one rifle (two sights, two magazines), so the held out set is effectively one design. The predictions for them come from the full CE pool, which does not contain them. No constant was chosen with them, but their figures were looked at while the estimator was developed, so treat them as indicative, not as a clean test (section 7).

**Metrics.** Per stat: n (targets that carry the stat), median and 80th percentile of the absolute relative error `|predicted - actual| / actual`, the share of targets whose actual value lies inside the claimed P50 and P80 bands, the share of predictions that carry a rating that lets a generator write them ("written"), and the median error of those. A macro figure averages the per stat medians over the stats a patch uses (11 for guns, 10 for melee weapons). The old predictor also returned damage, speed, penetration, pellets and the ticks between shots; the chosen ammo supplies those, so they are not compared.

## 3. The old predictor (before)

Class by exact keys (tag class, tier, burst or single) with a fallback chain by size 3; per stat the choice of identity, class median, class ratio or an elastic fit by leave one out inside the class; the band from those residuals. Aggregates, tag class answered:

| Set | Macro median error, patch stats | Written |
| --- | --- | --- |
| Guns, leave one out | 23.1% | all |
| Melee, leave one out | 43.5% | all |
| Guns, held out | 32.2% | all |

The failure modes were visible in the tables. Range was predicted from a class median that ignored the vanilla number in small classes (held out median error 72%). Magazine, spread, bulk and the melee penetration ratios had median errors of 37% to 83% and were written like any other number. Nothing told the user which numbers were reliable.

## 4. What changed

All rules are generic: a class size, a shrinkage strength, margins and thresholds. No stat table, tag table or game value is involved.

1. **Class by similarity.** A design is compared with every conversion by its vanilla weapon tags (weighted by rarity, so a tag that every weapon carries counts for nothing), the answered tag class, the verb shape (burst or single shot), the tier and, as a tie break only, the tercile of a strength proxy computed from the vanilla numbers. A class is the 4 nearest conversions that carry the stat; a stat that fewer than 3 conversions carry is not predicted at all, and the reason is recorded.
2. **Forms with the vanilla number kept.** A stat is estimated as the vanilla number times the class's median ratio, with the log ratio shrunk toward the ratio of all conversions (strength 2: a class of 4 counts 4 against 2 pseudo members of the pool), as the class median of the converted number (not shrunk), or as the vanilla number itself (identity, for mass and for the stats that CE carries over). The form is chosen by leave one out error; a later form (order identity, ratio, median) replaces an earlier one only when it is at least 20 percent better. Without 5 or more measurable conversions the stat is unmeasured.
3. **Reliability per stat.** The median leave one out error rates a stat reliable (below 15%), rough (below 35%) or unreliable; a stat whose 80th percentile error is 80% or more is unreliable whatever its median; fewer than 5 measurable conversions is unmeasured.
4. **Gating.** Reliable and rough numbers are written and labelled with their rating and typical error in the plan. Unreliable and unmeasured numbers are never written: the convert flow asks them (the question carries the reason and the rejected estimate as a reference), the range, warmup, mass, cooldown, sights and recoil that no block field can hold fall back to the vanilla number (identity) with the rating recorded in the plan.

Constants were chosen on a coarse grid over leave one out error of the guns and melee weapons (class size 3 to 8, tag, tier and shrinkage weights), preferring the flat middle of the good region; the top configurations were within a few hundredths of each other in macro error. That makes the leave one out figures below slightly optimistic. A fourth form (the median of the whole pool) and a larger weight for the answered tag class were tried and dropped: both made the leave one out figures worse overall.

## 5. After: leave one out over the CE conversions

Median absolute relative error (P80 in brackets), tag class answered. "Written" is the share of predictions that carry a usable rating.

Guns (18 targets):

| Stat | Before | After | In P80 band | Written | Rating |
| --- | --- | --- | --- | --- | --- |
| mass | 19% (41%) | 12% (43%) | 78% | 100% | reliable, identity |
| range | 25% (65%) | 31% (61%) | 83% | 100% | rough, ratio |
| warmup | 5% (31%) | 15% (37%) | 78% | 100% | reliable or rough |
| cooldown | 3% (45%) | 6% (38%) | 78% | 100% | reliable |
| bulk | 37% (54%) | 33% (45%) | 72% | 44% | ratio, rough or unreliable by case |
| sway | 20% (33%) | 19% (32%) | 83% | 100% | rough |
| sights | 25% (38%) | 10% (21%) | 83% | 100% | reliable |
| recoil | 22% (56%) | 19% (42%) | 79% | 100% | rough |
| reload | 0% (14%) | 23% (25%) | 80% | 100% | rough |
| spread | 59% (120%) | 28% (120%) | 72% | 0% | unreliable |
| magazine | 40% (79%) | 66% (87%) | 80% | 0% | unreliable |

Macro median error 23.1% before, 23.6% after over all predictions; 19.0% over the 77% that are written.

Melee weapons (17 targets):

| Stat | Before | After | In P80 band | Written | Rating |
| --- | --- | --- | --- | --- | --- |
| mass | 0% | 0% | 94% | 100% | reliable, identity |
| tool_power | 27% (44%) | 33% (54%) | 71% | 100% | rough |
| tool_cooldown | 20% (38%) | 15% (36%) | 76% | 100% | reliable or rough |
| bulk | 53% (94%) | 33% (104%) | 76% | 0% | unreliable |
| parry | 44% (53%) | 27% (53%) | 82% | 76% | rough |
| dodge | 33% (49%) | 33% (54%) | 82% | 41% | rough or unreliable |
| crit | 38% (83%) | 38% (83%) | 71% | 0% | unreliable |
| counter_parry | 66% (111%) | 61% (103%) | 76% | 0% | unreliable |
| ap_blunt_ratio | 83% (151%) | 33% (95%) | 88% | 0% | unreliable |
| ap_sharp_ratio | 71% (305%) | 29% (89%) | 70% | 20% | mostly unreliable |

Macro median error 43.5% before, 30.2% after over all predictions. Only 45% of the melee predictions are written; the written ones are mass, power, cooldown and parry.

Without the answered tag class the guns come out at 20.8% (before 26.0%) and the same melee figures (melee weapons have no tag class). The bands are honest: the P80 band holds 70% to 83% of targets for almost every stat (claimed 80%) and the P50 band holds 40% to 60% (claimed 50%).

Reading: the new estimator is better where a class by tags helps (sights, bulk, parry, the penetration ratios) and equal or worse where the old exact role classes were constant (reload, warmup, cooldown went from near exact to 5% to 25%). Its main gain is not the average error but that it knows which numbers it cannot predict (spread, magazine, bulk and the melee numbers other than mass, power, cooldown and parry) and stops writing them.

## 6. Held out: the owner's hand written conversions

Five variants of one rifle, tag class answered, predicted from the full CE pool.

| Stat | Before | After | Rating |
| --- | --- | --- | --- |
| mass | 0% | 0% | reliable, identity |
| range | 72% | 7% | rough, ratio |
| recoil | 37% | 8% | rough |
| bulk | 13% | 15% | rough |
| sway | 23% | 18% | rough |
| reload | 14% | 14% | rough |
| warmup | 33% | 22% | reliable |
| cooldown | 53% | 36% | reliable |
| sights | 0% | 0% | reliable |
| spread | 79% | 84% | unreliable, asked |
| magazine | 30% | 70% | unreliable, asked |

Macro median error 32.2% before, 24.9% after over all predictions, 13.4% over the written ones (82% of the predictions). The range case that started this round is fixed by keeping the vanilla number in the estimate. Spread and magazine are the stats the owner chose differently from the CE pool (a much larger spread, 10 and 25 round magazines); the estimator now asks for them instead of writing a number. Cooldown and warmup are rated reliable on the CE pool and still miss by 22% to 36% here: a class rating describes the pool, not the next author's style, which is the main limit of any prediction from one mod's conversions.

## 7. Limits

1. Small samples. The guns pool has 18 targets and the melee pool 17. A median of 17 errors has a wide interval, and the ratings are borderline for several stats (bulk flips between rough and unreliable between folds). The numbers are a measurement of this install, not of CE in general.
2. Selection effects. The constants and the choice of forms used the same leave one out error that the tables report. The held out set was only five variants of one design and its figures were inspected during development.
3. Style. The held out conversions follow a different authoring style than CE's own (larger spread, different magazines). No estimator trained on one set of conversions can know that; the gating and the asks are the answer, not a better average.
4. Tags. The similarity uses the vanilla twin's weapon tags. A weapon with no distinctive tag falls back to tier, shape and strength tercile, and the class is then a mix.
5. Bows, crossbows, grenades and launchers stay out of scope; bows sit in the gun pool as targets and as examples, which lowers the gun figures. No melee weapon of the owner's custom folder has a hand written CE patch, so the melee held out set is empty.
6. Reliability is a statement about the typical error of the stat over the pool, not about one weapon.

## 8. Reproducing

```text
RIMSTUDIO_GAME_DIR=<RimWorld install> RIMSTUDIO_CE_DIR=<Combat Extended folder> \
RIMSTUDIO_CUSTOM_DIR=<mods folder, optional> \
cargo test -p rimstudio-design --release --test real_eval -- --ignored --nocapture --test-threads=1
```

The test only reads, prints aggregates, and takes about 100 seconds. `RIMSTUDIO_EVAL_DETAIL=1` also lists the held out targets one by one for local reading; that output must not be committed.

## Implications for RimStudio

1. The automatic conversion is a starting point, and the plan must say which numbers are which: written with a rating and a typical error, or asked. This is implemented (combat-extended-patching section 14.9).
2. A pool of about 18 conversions supports mass, range, warmup, cooldown, sights, recoil, sway, reload and the melee power, cooldown and parry offsets at a typical error of 5% to 30%, and does not support magazine, spread, bulk, crit, counter parry or the penetration ratios. A user who converts many weapons answers those once per design, so the UI should offer to apply an answer to a group of weapons.
3. Estimates should never be compared by the average error alone: the estimator that knows what it cannot predict is worth more than a slightly lower mean.
4. The harness is part of the repository, so a change to the estimator is measured on any install before it ships.

## Open questions

1. Does a larger pool (CE plus other conversion mods that the user loaded) lower the errors enough to move bulk, magazine or the penetration ratios into the written group? The harness answers it for any install.
2. Should the gun bash tools of a gun, which no estimate predicts, get one question for the whole mod instead of one per weapon?
3. Would a held out set with melee weapons and with a second author's style change the ratings? The owner's folder has no hand written melee conversion.
4. Is a rating of rough at 35% median error the right line? It is an implementation choice; the thresholds are options of the estimator.
