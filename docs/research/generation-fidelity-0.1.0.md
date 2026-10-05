# Generation fidelity of a clone, 0.1.0

Status: evidence note, aggregates only | Date: 2026-10-05 | Decisions: D-110 to D-114 ([ADR 0042](../adr/0042-generation-fidelity-of-a-clone.md))

This note records the measurement behind the clone fidelity work. It holds counts, no definitions and no values of the game (R11). Research notes are not edited after a decision.

## Method

The ignored test `crates/rimstudio-toolkit/tests/real_fidelity.rs` reads the installed game (Core and the four expansions). For every weapon definition (`is_weapon_def`, 86 definitions) it clones the weapon through `designer_clone`, plans the clone, takes the file as the plan rendered it through the XML boundary, parses it back and compares:

1. the OWN comparison: every own child element, attribute and list entry of the source (the def as its file writes it) against the written clone;
2. the RESOLVED comparison: both definitions after inheritance, in a session that holds the game and a project with all clones;
3. both comparisons again for the projectile of a clone that has its own projectile.

Entries are compared as `(path, value)` pairs, numbers by value and `true` and `false` without regard to case, list entries by position when they hold elements. Missing and extra entries are reported grouped by element path. The explained differences are `defName`, `label`, the `Name` attribute, the projectile name in the shooting verb of a clone with its own projectile, and, on the extra side only, the verbs and tools of a source that inherits them, which the clone writes in full with `Inherit="False"`.

## Before

The first run (clone as built before this work, shared projectile, no raw fields):

| Measure | Value |
|---|---|
| Weapons cloned and planned | 29 of 86 |
| Refused with `design.required-missing` | 57 (diagnostics by field: cost list 25, work to make 33, accuracy 13 per band, damage 14, role 49, tier 9, stuff 9, mass 1) |
| Unexplained differences, own, of the 29 | 274 |
| Unexplained differences, resolved, of the 29 | 265 |

The largest groups by element path: `recipeMaker/skillRequirements` (27 missing), `recipeMaker/displayPriority` (29), `soundInteract` (19), `recipeMaker/recipeUsers` (10), `equippedAngleOffset` (10), `thingSetMakerTags` (16), `uiIconScale` (8), `comps` (entries of the weapons with charges and art), and, as extras, `techLevel` (15), `tradeTags` (28), `weaponClasses` (35) and `weaponTags` (17) which the clone wrote although the parent supplies them (the game appends them, so the resolved definition listed them twice).

## After

| Measure | Value |
|---|---|
| Weapons cloned and planned | 86 of 86 |
| Unexplained differences, own, weapon | 0 |
| Unexplained differences, own, projectile | 0 |
| Unexplained differences, resolved, weapon | 0 |
| Unexplained differences, resolved, projectile | 0 |
| Clones with an own projectile | 64 |
| Clones with a recipe carried | 53 |
| Clones with an interaction sound | 36 |
| Clones with comps | 23 |
| Clones with raw extra fields | 68 |
| Clones with raw verb extras | 25 |
| Clones with tool extras | 6 |
| Clones with equipped stat offsets | 3 |
| Clones whose tech level is not written (the parent supplies it) | 48 |
| Clones with a list that replaces the parent's (`Inherit="False"`) | 29 |
| Clones with a required field accepted as missing | 57 |
| Sources changed by a patch | 0 |

## What the gaps were

- Modelled fields missing: interaction sound, recipe details (skill requirements, display priority, workbenches, the unfinished thing, the work skill), comps, equipped stat offsets, the menu icon scale, the forced miss radius, extra damages of a tool.
- Fields the designer does not understand (`relicChance`, `smeltable`, `thingSetMakerTags`, `equippedAngleOffset`, `possessionCount`, aiming effects of a verb): now carried as raw nodes.
- Inheritance: tags, the tech level, mapped stats, cost lists and recipe fields the parent supplies were written again. A recipe that a source nulls (`IsNull`) or replaces (`Inherit`) was lost.
- Weapons that inherit their verbs and tools from another weapon (the unique variants) lost the verb and the tools numbers when the clone had no way to write them; they are written in full now.
- A weapon without a shooting verb that names a projectile (a flamethrower verb) lost its accuracy and cooldown stats.
- Required fields the source lacks (non craftable weapons, mechanoid weapons) refused the clone.
- The projectile: a clone shared the projectile of its source, so a damage edit never reached the file.

## Implications for RimStudio

- The clone is faithful to its source for every vanilla weapon: the written definition holds every own field of the source, and the definition after inheritance equals the source's except for the new names and the own projectile name.
- Because the fields the designer does not understand are carried raw, a modded weapon with fields unknown to the designer clones the same way, with no table of fields to maintain.
- The designer must treat a missing required field of a clone as the source's own state, not as an error (D-113).
- The harness is the regression test for every later change of the reader or the writer; it needs the game folder and is ignored by default.

## Open questions

- A source changed by a patch has no faithful file form. None of the 86 vanilla weapons is patched, so the case is untested on real data; it is read the plain way with a note.
- Weapons of installed mods are not measured here (the harness reads the game and its expansions). The same harness can be pointed at a mod folder.
- The raw fields are shown as XML text in the UI; whether the user needs a guided editor for the common ones (`relicChance`, `thingSetMakerTags`) is a UI decision.
