# Weapon archetypes: validation on the owner's install (0.1.0)

Scope: how well the archetype solver (item balance math section 15) reproduces the real weapons of one install, measured by a leave one out harness, with the tuning that was done and the limits of the result.

Status: research note | Last verified: 2026-10-05

## Summary

- The harness derives the archetype and the descriptors of every gun and melee weapon of the install by the rules of the taxonomy, builds the proposal for the typical balance target against the pools WITHOUT that weapon, and compares the proposal with the real numbers. 37 weapons: 20 guns and bows, 17 melee weapons (Core and the five official expansions of RimWorld 1.6.4871).
- Typical target, after tuning: median error 17 percent for damage, 0 for range, 15 for cooldown and 6 for mass over the guns and bows, and 12, 8 and 10 percent for swing damage, cooldown and mass over the melee weapons. The gate of the specification (median error under 25 percent for damage, range, cooldown and mass in every family of at least 3 members) holds in all families.
- 35 of 37 proposals are typical or plausible on the fit meter (95 percent); the meter says the same of 36 of 37 real weapons. The two failures are the minigun and the beam repeater, both derived as the heavy machine gun, which the meter (it knows only the tier class) calls unusual for their burst, ticks and warmup. No proposal misses its strength target by more than 15 percent, and the orderings hold (sniper range above rifle range above SMG range, the same for warmup, SMG cooldown below rifle cooldown).
- Told the real strength of the weapon (`strength` in the request), the proposals are much closer: median damage error 10 percent, swing damage 7 percent, strength index error 3 and 2 percent. The remaining error of the typical run is therefore mostly the real weapons not being typical, not the shape.
- With Combat Extended (79 converted guns, 19 with vanilla twins, 297 ammo sets as calibres) the proposals for the 13 guns that have a twin, with the gun's own ammo set as the calibre and the numbers of the block from the existing predictors, give a median error of 26 percent for bulk, 9 for cooldown, 15 for mass, 26 for range, 3 for reload time, 11 for sway and 29 for warmup, and the proposed AI class tag equals the real one for 12 of 13 guns.

## Method

`crates/rimstudio-design/tests/archetype_real.rs` (ignored tests that need `RIMSTUDIO_GAME_DIR`, and `RIMSTUDIO_CE_DIR` for the Combat Extended part). For each weapon of the install:

1. The pool, the baseline model and the reference weapons are rebuilt without the weapon (leave one out), so the medians, the class and the cost lists never include it.
2. `derive_ranged` or `derive_melee` names the archetype by the ordered rules of the taxonomy (conditions on burst, tier, the ratios of damage, range, warmup, cooldown and mass against the medians, and on tool capacities) and the descriptors from the numbers: the action from the burst and the tags, the calibre from the damage class (the real calibre of a vanilla gun is unknown), the handling from the mass, the rate of fire from the cycle time.
3. The proposal is made for the typical target and compared per stat: relative error `abs(proposal - real) / real`, reported as the median and the 80th percentile per family and over all weapons. A weapon the game prices explicitly (work to make of 1) is left out of the work comparison.
4. The fit meter scores the proposal with the documented default bands (no calibration) and the verdict is typical, plausible or unusual (item balance math section 15.3); the stats the designer asks for instead of predicting (work to make, market value) are shown but do not decide the verdict. The same meter scores the real weapon at its own strength, which is the ceiling a proposal can reach.
5. The orderings of the canonical archetypes are checked on the full pool.

## Before and after tuning

| Step | What changed | Effect |
| --- | --- | --- |
| 0, first run | the strength shift of an archetype applied only when no pool role matched; the calibre ladder was absolute and the default handling was compact or heavy for some archetypes; the derived action chose full automatic for every burst of three | guns: damage 35 percent median error (P80 42), warmup 32, cooldown 12, mass 11; melee: swing damage 15, mass 24; 29 of 37 typical or plausible (78 percent); 9 proposals outside the P10 to P90 of the class strength |
| 1, strength shift always applied | each archetype states how strong it typically is against the middle of its class (a great bow against the other bows, an assault rifle against the other guns of the tier), 25 constants | guns: damage 17 percent (from 35), bows 24 percent (from 41), machine guns 20 (from 40) |
| 2, handling and calibre relative to the archetype | the default handling is standard for every archetype and the factors of a calibre class are relative to the archetype's default calibre, so the shape is the shape of the default weapon | guns: mass 6 percent (from 11), melee mass 10 (from 24) |
| 3, derive: nearest burst count | the action of a burst weapon is the one whose burst count is nearest | rifle damage 10 percent (from 19) |
| 4, verdict excludes asked stats | work to make and market value do not decide the verdict | 35 of 37 typical or plausible (from 30) |
| 5, median work of made weapons | weapons with a work of 1 or less (priced explicitly, nobody crafts them) are left out of the median work | melee work 20 percent median error (from 96) |

Every tuning constant is generic: the exponents (damage 0.65, cooldown 0.2, warmup 0.15, power 0.7, tempo 0.3, rate of fire on cooldown 0.8, on burst 0.7, on ticks 0.5, mass 0.25, work 0.35, cost 0.5), the 25 strength shifts, the shape ratios and the thresholds of the derive rules are in `data/archetypes.json` and are ratios or exponents, never a game value. They were chosen with these 37 weapons in view.

## Final tables (typical target, leave one out)

Guns and bows, median error / P80 error:

| family | n | damage | range | warmup | cooldown | mass | dps |
| --- | --- | --- | --- | --- | --- | --- | --- |
| bow | 6 | 24 / 45 | 2 / 23 | 19 / 25 | 15 / 24 | 7 / 12 | 27 / 53 |
| machine gun | 3 | 20 / 152 | 0 / 25 | 33 / 34 | 16 / 40 | 6 / 17 | 10 / 271 |
| pistol | 3 | 17 / 17 | 0 / 0 | 0 / 12 | 10 / 14 | 4 / 6 | 10 / 10 |
| rifle | 5 | 10 / 13 | 0 / 11 | 26 / 28 | 13 / 16 | 6 / 6 | 5 / 18 |
| shotgun | 2 | 11 / 18 | 0 / 0 | 21 / 23 | 17 / 19 | 5 / 6 | 4 / 5 |
| SMG | 1 | 17 / 17 | 0 / 0 | 22 / 22 | 15 / 15 | 9 / 9 | 10 / 10 |
| all | 20 | 17 / 23 | 0 / 11 | 25 / 30 | 15 / 21 | 6 / 8 | 11 / 31 |

Melee weapons:

| family | n | swing damage | cooldown | dps | armor penetration | mass |
| --- | --- | --- | --- | --- | --- | --- |
| axe | 1 | 7 / 7 | 5 / 5 | 2 / 2 | 7 / 7 | 0 / 0 |
| club and staff | 3 | 25 / 45 | 15 / 25 | 25 / 58 | 25 / 45 | 13 / 51 |
| knife | 3 | 12 / 14 | 12 / 15 | 24 / 25 | 12 / 14 | 6 / 19 |
| mace and hammer | 4 | 11 / 22 | 6 / 9 | 16 / 26 | 11 / 22 | 7 / 78 |
| spear | 1 | 9 / 9 | 4 / 4 | 12 / 12 | 9 / 9 | 5 / 5 |
| sword | 5 | 23 / 41 | 4 / 9 | 28 / 52 | 59 / 68 | 18 / 20 |
| all | 17 | 12 / 29 | 8 / 14 | 19 / 32 | 14 / 55 | 10 / 22 |

The canonical proposals on this install at the typical target (defaults of each archetype) land on the targets within 5 percent: an assault rifle of damage 12, range 30.9, warmup 1.15, cooldown 1.65, a burst of 3 and mass 3.7; a sniper rifle of damage 29, range 44.9, warmup 4.35, cooldown 1.6 and mass 4.4; a light SMG of damage 8, range 20.9, warmup 0.65, cooldown 0.95, a burst of 3 and mass 2.2; a revolver of damage 11, range 25.9, warmup 0.35, cooldown 1.45 and mass 1.4.

## Limits

- The shapes and strength shifts were tuned on the same 37 weapons the harness measures; the leave one out removes the weapon from the medians, the class and the materials, but not from the choice of the constants. The numbers above are therefore an in sample result for the shapes and an out of sample result for the medians. A modded install with many more weapons is the real test; the Combat Extended run is a small independent check (the shapes were not tuned on it).
- Families of one or two members (axe, spear, SMG, shotgun) say little; the machine gun family is carried by three weapons of very different size (a light machine gun, a minigun and a beam repeater with a work of 60000 and a burst of 30), so its P80 error is large.
- A vanilla weapon with a special mechanic (the flame bow, the nerve spiker, the pilum, the beam repeater) is derived as the nearest ordinary archetype and shows the largest errors.
- Work to make stays the least predictable stat (guns 25 percent median error, knives and clubs 54 to 68): it is an input in the designer, not a prediction, and the verdict ignores it.
- The fit meter knows only the role and tier class, so a sniper rifle at the typical target is called plausible and an anti materiel rifle unusual (the canonical proposals, not part of the 37): the meter is right that they are outliers among the guns of the tier, and the proposal is right to make them so.
- The strength index of the Combat Extended guns is not compared (the Combat Extended proxy is a prototype, math specification section 4); the comparison is per stat.

## Implications for RimStudio

- A first complete weapon can start from a description (family, action, rate of fire, calibre, balance target) instead of a class median: the proposals land within the error above, every number has a reason, and the strength is where the target says.
- The balance target is only as meaningful as the class pool: with fewer than 3 weapons of the role and tier the class widens, and the proposal says so. The designer should show the class label and the class size next to the target.
- The archetype shapes belong in data (`archetypes.json`), so a modder or the owner can tune a family without code; the validation harness is the way to check a change.
- The fit meter and the archetypes agree on typical weapons and disagree on outliers by design; the verdict ignores asked stats (work, market value).

## Open questions

- Do the shapes hold on a heavily modded install? The ignored harness runs against any install named by `RIMSTUDIO_GAME_DIR` and a Combat Extended folder, so the owner's mod folders can be added as reference mods and measured the same way.
- Should the strength shift of an archetype be learned from the install when the role holds enough weapons (at least 3 per archetype) instead of being fixed in the data?
- Combat Extended mode proposes the calibre as an ammo set but the vanilla numbers still come from a class ladder scaled by the ratio of the set. A direct comparison with the converted values of the user's own weapons (per ammo set) would show whether the exponents of the ratio are right.
