# ADR 0042: Generation fidelity of a clone

Status: accepted | Last updated: 2026-10-05 | Register: D-110 to D-114

## Context

The first clone flow copied the fields the design spec models and listed the rest as left behind. A clone of `Gun_BoltActionRifle` written into a new mod lost the interaction sound, the crafting skill requirement and the display priority of its recipe, wrote a `techLevel` that its parent already supplies, listed the parent's tags a second time (the game appends the lists of a child to those of its parent), and, for a gun, pointed at the projectile of its source, so a change of damage never reached the written file. A clone that silently loses a crafting requirement is wrong.

## Decision

1. A clone carries every field its source defines itself and nothing its parent supplies. The reader takes the def as its file writes it (before inheritance) next to the resolved def. Fields the designer models are first class (sound, recipe, comps, equipped stat offsets, icon, graphic data, verb and tool extras); every other own field travels as a raw node tree in the draft, is emitted verbatim after the modelled fields and can be removed by the user (D-110).
2. What the parent supplies is recorded and not written: the tech level (`parent.inheritedTechLevel`), the mapped stats (`parent.inheritedStats`, written again only when the value changes or is typed), tags, comps and recipe fields (not read at all). A list the parent supplies that the designer needs to edit (the verbs and tools of a weapon that inherits them) is written in full with `Inherit="False"`, which keeps the resolved definition equal to the source's (D-111).
3. A clone of a gun has a projectile of its own by default: the source projectile is copied into an inline projectile spec named after the weapon (`<prefix>_Bullet_<rest>`), written in the weapon file before the weapon (the layout of [ADR 0041](0041-rimstudio-mod-layout-v1.md)). `designer_projectile_own` switches it on or off for any gun draft; off points back at the projectile it was copied from. This supersedes the 0.1.0 rule that a clone shares the projectile of its source (D-112).
4. A required field that the source does not set either (a relic without a cost list, a weapon without work to make) is not an error for a clone: the spec lists its pointer in `acceptedMissing` and the plan reports an info `design.accepted-missing`. Raw fields that repeat a modelled field, or are not valid XML, are errors (`design.extra-field-conflict`, `design.extra-field-invalid`); a texture other than the reserved path is an info (`design.texture-shared`): the tool never copies art (D-113).
5. The draft schema version is 2. The change is additive, so a version 1 draft reads unchanged; the stored record has a migration step and an older draft sent by a client is upgraded when read. A draft of a newer version is read as far as it is understood and never written back (D-114).

## Consequences

- `rimstudio-ipc-types` and the generated TypeScript gain optional fields (all additive) and one registry row (`designer_projectile_own`, 43 commands).
- The fidelity harness (`crates/rimstudio-toolkit/tests/real_fidelity.rs`, ignored, needs the game folder) clones, plans, renders and compares every vanilla weapon; the evidence is in [generation fidelity](../research/generation-fidelity-0.1.0.md).
- A clone of a weapon that inherits its verbs or tools writes them in full. The written file is longer than the source's, and equal after inheritance.
- Combat Extended is unchanged: the vanilla files never contain it (D-085).

## Alternatives rejected

- Copying the resolved def: it writes the parent's tags, comps and stats a second time.
- Writing the raw own node of the source: it would not follow the designer's rules for names, numbers and validation, and the user could not edit what the file holds.
- Keeping the shared projectile as the default: a damage edit would not change the file, the failure the owner reported.
