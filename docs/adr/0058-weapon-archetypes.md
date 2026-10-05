# ADR 0058: Weapon archetypes

Status: accepted | Last updated: 2026-10-05 | Register: D-200 to D-204

## Context

The owner asked that the numbers of a weapon can be calculated roughly from its category (rifle), its kind (sniper, bolt action), its speed of fire and its calibre, for balancing and for automatic numbers that fit the game, and wants to try a first mod within the hour. The designer already had exact formulas, class pools by tier and role, a baseline model and a fit meter. What was missing was a way to start from a description of the weapon instead of from a class median.

## Decision

1. The taxonomy is RimStudio's own JSON embedded in the binary (`data/archetypes.json`): families, archetypes, the allowed actions, rate of fire classes, calibre classes and handling classes, and the SHAPE of each archetype as ratios to the medians of the user's install. It holds names, counts and ratios and no game value (D-200).
2. The solver is two steps. The shape (medians times ratios) holds the character of the weapon. One strength scale, found by bisection against the strength index of the install's own pool, moves damage, cooldown, warmup and mass together until the strength lands on the percentile the balance target names (D-201). The strength shift of an archetype against the middle of its class is data, so that a great bow is typically stronger than a short bow.
3. Every number carries a plain reason and the multipliers behind it, and the source chip `archetype`. In a draft the numbers are stored as `suggested`, so the offer rule keeps every typed, answered or anchored value; no new source is added to the spec (D-202).
4. The choice is stored in the draft as an optional `archetype` member, so older drafts load unchanged and the numbers can be proposed again when the target changes. Applying also completes the empty structure from the nearest reference weapon (D-203).
5. Combat Extended stays optional (D-085). In Combat Extended mode the calibre is one of the user's ammo sets and the block is filled only on request, by the existing predictors. Vanilla mode needs no Combat Extended (D-204).
6. The solver is validated by a harness on the real install (leave one out): archetype and descriptors are derived by rules from tags, verbs and tools, the typical proposal is compared with the real weapon per stat, and the gates are checked.

## Consequences

- `rimstudio-design` gains the `archetype` module, `model::archetype` and the optional `archetype` member of `Draft`; `rimstudio-ipc-types` gains `designer::archetype` and the optional `archetype` member of the draft DTO; `rimstudio-toolkit` gains `designer::archetype` and a cached `Engine::archetype_data`; `rimstudio-app` registers three rows; `rimstudio-cli` gains `designer archetypes`, `designer propose` and `designer new --archetype`.
- The shape ratios and the strength shifts are tuning constants chosen with the vanilla weapons of one install in view; a leave one out harness reports the error, and the limits are stated in the validation note. They can be changed in the JSON without code.
- A weapon of a kind the taxonomy does not know is still handled by the class baseline; the archetype is an additional way to start, not a replacement.

## Alternatives rejected

- A fixed table of stats per weapon type: it would be a game value table (R11) and would be wrong for any modded install.
- Scaling every stat by the strength ratio: the character of a weapon would change with its power; one scale on damage and tempo keeps range and accuracy as the archetype says.
- A new value source for archetype numbers in the spec: every match on the source enum would change for a label the proposal can carry itself.
- Writing the market value: the price follows from the cost list and the work (section 3.4 of the math); a stored value would drift from them.
