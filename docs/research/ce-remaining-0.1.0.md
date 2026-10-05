# Combat Extended remaining differences, 0.1.0

Evidence note. It records what is left of the differences between the generated Combat Extended patch and Combat Extended's own conversions on the owner's install after the structure fidelity task, and how each one was closed or classified. Only aggregates are recorded: element paths and counts, no value of the install (R11). The specification is in [Combat Extended patching](../features/combat-extended-patching.md) section 14.16 and the decision is D-146 ([ADR 0048](../adr/0048-optional-ce-additions-are-block-choices.md)); the previous round is [the structure fidelity note](ce-structure-fidelity-0.1.0.md), which is not edited.

## 1. Question

Which of the classified differences can be closed without breaking the rule that the designer writes vanilla (D-085), and what is the policy or the unsupported reason of every difference that stays?

## 2. Method

The harness of the previous note (`crates/rimstudio-design/tests/real_ce_fidelity.rs`) runs unchanged in structure. What is new is how the optional block is filled, chosen with `RIMSTUDIO_FIDELITY_PLAN`:

| Mode | The block holds |
| --- | --- |
| `none` | the typed values read from the real conversion and nothing else: the plain conversion |
| `suggested` | the same, plus every suggestion of `suggest_options` accepted (companion tags, recoil pattern, one at a time reload, tool plan) |
| `explicit` | the same, plus what a user would state: the real tools as a tool plan, the real extra tags, and the extra `modExtensions` entries of the real def as raw nodes |

The explicit mode shows that the writer can express the real conversion; it does not show that a habit predicts it, because the real tools are the input. The suggested mode shows what the generator offers on its own. The numbers of the typed values equal the real ones by construction in all modes. Every finding of every kind (missing, not removed, removed extra, extra, value) is classified as policy (a decision of the design), optional (written when the user accepts or states it), unsupported, habit (a choice of the generator that the real conversion does not share) or derived (a number from the estimators or the vanilla design). The strict mode of the harness fails on a missing or not removed path without an explanation, on a finding without a class and on a weapon where applying twice differs from once; it passes in all three modes.

Limit: the habits are learned from all converted weapons including the one under test (no leave one out), as in the previous note.

## 3. Population

The harness reads 37 converted weapons with a vanilla twin (20 guns, among them the bows, and 17 melee weapons) and sets 38 aside: 24 of an unsupported kind (turrets and mechanoid weapons, launchers, one use weapons), 12 that inherit the conversion of a converted parent, and 2 without ammo to convert. Of the 37, 34 generate cleanly in the plain and suggested modes and 35 in the explicit mode: the spear that failed the required field check before (its real tool has no sharp penetration, so the plain block cannot be completed) generates once the tool plan carries the penetration of each tool. Two weapons still fail the required field checks of the block read from the real conversion (a turret style gun without bulk, a gun without an ammo set). Applying the patch twice differs from applying it once for none of them in any mode. In 11 of the real conversions Combat Extended has more tools than the vanilla twin.

## 4. Totals

| Measure | Before | Plain (`none`) | Suggested | Explicit |
| --- | --- | --- | --- | --- |
| Missing | 118 | 184 | 114 | 37 |
| Not removed | 43 | 54 | 37 | 16 |
| Removed extra | 6 | 2 | 6 | 2 |
| Extra | 47 | 12 | 47 | 8 |
| Value differences | 335 | 308 | 335 | 126 |
| Weapons where applying twice differs from once | 0 of 34 | 0 of 34 | 0 of 34 | 0 of 35 |
| Operations that failed to apply | 0 of 191 | 0 of 192 | 0 of 192 | 0 of 227 |
| Findings without a class | not classified | 0 | 0 | 0 |

"Before" is the harness of the start of this task (the muzzle and capacity habits applied silently). The plain mode no longer applies them: the extras fall from 47 to 12 and the missing tool entries rise from 36 to 106, because those entries are now the user's decision. The suggested mode reproduces the old behaviour by the user's acceptance (the same tool findings as before), and the explicit mode closes them.

## 5. Differences by path

Counts are findings, one per weapon and entry. Columns: before, plain, suggested, explicit.

| Group | Kind | Before | Plain | Suggested | Explicit |
| --- | --- | --- | --- | --- | --- |
| Ammo comp extras (`AmmoGenPerMagOverride`, `reloadOneAtATime`) | Missing | 3 | 0 | 0 | 0 |
| Verb `recoilPattern` | Missing | 1 | 0 | 0 | 0 |
| Tool child `labelUsedInLogging` | Not removed | 6 | 0 | 0 | 0 |
| Tool list | Missing | 36 | 106 | 36 | 0 |
| Tool list | Not removed | 22 | 39 | 22 | 0 |
| Tool list | Extra | 40 | 5 | 40 | 1 |
| Tool list | Removed extra | 4 | 0 | 4 | 0 |
| Weapon tags beyond the class tag | Missing | 10 | 10 | 10 | 0 |
| Weapon tags | Not removed | 1 | 1 | 1 | 1 |
| Weapon tags | Removed extra | 2 | 2 | 2 | 2 |
| `GunDrawExtension` entries | Missing | 32 | 32 | 32 | 0 |
| Fire modes and burst | Extra | 6 | 6 | 6 | 6 |
| Economy and art (costs, stuff categories, draw size, `MoveSpeed`) | Missing | 33 | 33 | 33 | 34 |
| Economy and art | Not removed | 14 | 14 | 14 | 15 |
| Verb `soundCastTail` | Missing | 1 | 1 | 1 | 1 |
| Verb `targetParams` | Missing | 2 | 2 | 2 | 2 |
| Verb `recoilAmount` | Extra | 1 | 1 | 1 | 1 |

The ammo extras, the recoil pattern and the dropped tool child are zero in the plain mode too. The first two because the block read from the real conversion holds them (they are block fields now, written when present); the third because the habit below removes the child on its own.

## 6. The six items

1. **Ammo comp extras and the recoil pattern** are optional block fields (`reloadOneAtATime`, `ammoGenPerMag` for guns, `recoilPattern`), read by `ce_block_from_def`, written into the conversion operation, diffed in update mode. Suggestions come from the converted guns of the weapon class: a pattern or the one at a time reload is offered when at least 2 guns do it and at least 60 percent of the class agree. On this install no class reaches that bar for any of the three, so nothing is suggested; the fields are typed or read from a conversion.
2. **Tool lists.** The explicit `toolPlan` writes the real tool lists exactly: 36 missing, 22 not removed and 40 extra tool findings become 0, 0 and 1 (one sharp penetration that the plan carries and the real tool lacks). The muzzle habit is no longer silent: the plain conversion keeps the tools of the design and lists each restructuring as a hint, and the suggestion `tool-plan` writes it when accepted.
3. **A tool child that conversions drop.** `labelUsedInLogging` is carried by the vanilla tools of 6 converted melee weapons and removed by Combat Extended in all 6. The reader now records the tool children of every converted weapon and its twin; a child that at least 2 twins carry and at least 75 percent of those weapons drop is removed, with a derived value note. Not removed findings of this child: 6 to 0. No gun shows such a child.
4. **Weapon tags beyond the class tag.** They are written through `extraTags`. The suggestion needs the tag on at least 2 guns of the class and on at least 60 percent of the class; on this install no class reaches that bar, so the generator suggests none (the tags follow the weapon: a sidearm mark on pistols, the simple and advanced gun marks by tier), and the findings stay in the suggested mode and go to 0 in the explicit mode. Three tag differences stay: one vanilla tag that Combat Extended removes and the patch keeps, and two vanilla tags of one weapon that the conversion operation of the patch removes and Combat Extended keeps. The block has no way to say either, and they stay.
5. **Raw nodes.** `rawExtras` carries the `GunDrawExtension` entries of the real defs: 32 missing findings become 0 and the patch stays idempotent. Shape checks refuse a node the conversion owns, an invalid node, a second node of the same name, and a list entry that cannot be matched.
6. **Economy and art** (costs, stuff categories, draw size, the `MoveSpeed` offset) stay decisions of the design. They are classified as policy in the harness (47 findings in the plain and suggested modes, 49 in the explicit mode with its extra weapon) and every new conversion carries one info diagnostic `ce.economy-by-design`. They were never a defect.

## 7. What stays, with its class

| Class | Finding | Why |
| --- | --- | --- |
| Policy | costs, stuff categories, draw size, `MoveSpeed` (49 in the explicit mode) | the design decides economy and art (D-085) |
| Optional | the tool list, the tags and the draw extension in the plain and suggested modes | written when the user accepts the tool plan or states the tags and nodes |
| Unsupported | 1 `soundCastTail`, 2 `targetParams`, 1 vanilla tag that Combat Extended removes and the patch keeps, 2 vanilla tags that the conversion operation removes and Combat Extended keeps | a verb field that Combat Extended adds and tag removals or merges that the block cannot say; weapon sounds are a separate task |
| Habit | 6 fire mode and burst extras, 1 recoil extra | the fire mode habit and the class estimate write what the real conversion leaves out |
| Derived | 126 value differences in the explicit mode | numbers from the estimators or the vanilla design, not structure |

## Implications for RimStudio

- The patch is idempotent in every mode, and the plain conversion no longer changes the tools of a design without being asked.
- A user who wants the conversion to look like Combat Extended's own can state the tool plan, the tags and the art nodes, and the generator writes them; the generator cannot yet predict them from habits on this install, because the conversions per class are too few and too individual.
- The suggestion engine for tags, recoil and reload would need more converted guns per class (or a grouping by the vanilla tags of the twin) to clear its 60 percent bar.

## Open questions

- A grouping of the companion tag habit by the vanilla tags of the twin (the simple and advanced gun marks follow the tier) could clear the bar where the class grouping does not.
- The bow tool of Combat Extended has no label; the plan writes a tool without a label only for an entry that starts from a vanilla tool.
- The IPC DTO and the TypeScript bindings of the block do not carry the new fields, so the UI cannot set them yet.
