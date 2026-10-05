# Combat Extended structure fidelity, 0.1.0

Evidence note. It records the round trip of the Combat Extended patch generator against the user's own Combat Extended on the owner's install, before and after the changes of the task `ce-fidelity`. Only aggregates are recorded: element paths and counts, no value of the install (R11). The specification is in [Combat Extended patching](../features/combat-extended-patching.md) section 14.12; this note is evidence and is not edited after the decision.

## 1. Question

Does the patch that the generator writes for a weapon produce the same definition structure as Combat Extended's own conversion of that weapon? Every element and list entry that the real conversion adds must exist in the generated result, and every extra element of the generated result must be explainable.

## 2. Method

The harness is `crates/rimstudio-design/tests/real_ce_fidelity.rs` (ignored, needs `RIMSTUDIO_GAME_DIR` and `RIMSTUDIO_CE_DIR`, writes nothing).

1. Two loads with the shared def engine: the official packs alone (the vanilla twins and their raw nodes), and the official packs plus the installed Combat Extended with the gun conversion operation registered (the real conversions as the game resolves them).
2. For every weapon in the model's converted guns and converted melee weapons: the scan must list it as convertible. Turrets, launchers, one use weapons, explosive weapons and bows are unsupported kinds and are listed, not converted. A weapon whose parent is converted inherits the conversion and is set aside.
3. The design of the vanilla twin is read from the raw def (`spec_from_def_own`). Its Combat Extended block is read from the real conversion (`ce_block_from_def`, typed values), so the numbers the block carries are the real ones and only the structure is under test. Penetration of the real tools is mapped onto the twin's tools by label, else by position.
4. The patch is generated (`gun_patch`, `melee_patch`, container read from the raw def) and applied by loading the vanilla defs with the patch as an extra mod. A second load applies every patch twice.
5. The two resolved defs are compared with the vanilla def as the base, path by path. List entries are keyed by class and label; tools are keyed by position because Combat Extended relabels some tools and the class of a tool entry is compared too.
   - Missing: the real def has an entry that the generated result lacks.
   - Not removed: the real conversion removed an entry of the vanilla def that the generated result keeps.
   - Removed extra: the generated result removed an entry of the vanilla def that the real def keeps.
   - Extra: the generated result has an entry that neither the real def nor the vanilla def has.
   - Value: both have the element with a different value.
6. The class names of the generated operations are checked against the classes that Combat Extended's own data files use, and the patches are linted with the gate of the compat folder.

Limits: the habits (tool conventions, fire modes) are learned from all converted weapons of the install including the weapon under test (no leave one out), so the habit results are an upper bound for a weapon outside the install; typed values equal the real ones by construction.

## 3. Population

76 weapons are converted by Combat Extended and have a vanilla twin (55 guns, 21 melee weapons); the scan lists 75 of them as weapons. Of those:

- 36 are converted by the harness and compared (17 guns, 19 melee weapons); 31 generate without error. The other five fail the required field checks on the block read from the real conversion: a turret style gun that has no bulk, a gun without an ammo set, two beam or flame weapons that have no projectile and are read as melee weapons, and a spear whose tool has no sharp penetration in Combat Extended. They are listed by the harness and left out of the comparison.
- 28 are unsupported kinds by the reason of the scan: turrets and mechanoid weapons 19, bows 4, explosive launchers 3, one use weapons 2.
- 11 inherit the conversion of their parent (the unique variants of converted guns).

## 4. Totals

| Measure | Before | After |
| --- | --- | --- |
| Missing | 319 | 139 |
| Not removed | 56 | 45 |
| Removed extra | 8 | 6 |
| Extra | 6 | 47 |
| Value differences | 277 | 320 |
| Weapons where applying twice differs from once | 31 of 31 | 0 of 31 |
| Operations that failed to apply | 0 of 125 | 0 of 185 |
| Class names unknown to Combat Extended's data | none | none |
| Lint findings of the generated patches | none | none |

The rise of extras and value differences is the price of carrying more: the extra tools are muzzle tools that a gun with a two capacity barrel gets where Combat Extended merged barrel and muzzle by hand (pistols), and the value differences are numbers that were absent before (a pick weight the design keeps over the habit, a fire mode count) now compared. Typed block values (bulk, sway, spread, sights, recoil, reload, magazine, parry bonus, crit, parry and dodge offsets, tool penetration) show no value difference in either run.

## 5. Differences by path

Paths of tools are by position (`li@N`). Counts are findings, one per weapon and entry.

#### tool list restructured by Combat Extended or tool habits

Before 234 findings, after 96.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/tools/li@N/linkedBodyPartsGroup` | 72 | 12 |
| NotRemoved | `/tools/li@N/capacities/li` | 32 | 15 |
| Missing | `/tools/li@N/chanceFactor` | 29 | 2 |
| Missing | `/tools/li@N/capacities/li` | 28 | 12 |
| Missing | `/tools/li@N/@Class` | 11 | 2 |
| Missing | `/tools/li@N/armorPenetrationBlunt` | 11 | 2 |
| Missing | `/tools/li@N/cooldownTime` | 11 | 2 |
| Missing | `/tools/li@N/label` | 11 | 2 |
| Missing | `/tools/li@N/power` | 11 | 2 |
| Missing | `/tools/li@N/extraMeleeDamages/...` | 10 | 0 |
| RemovedExtra | `/tools/li@N/extraMeleeDamages/...` | 6 | 0 |
| Extra | `/tools/li@N/armorPenetrationSharp` | 1 | 1 |
| NotRemoved | `/tools/li@N/label` | 1 | 1 |
| Extra | `/tools/li@N/@Class` | 0 | 5 |
| Extra | `/tools/li@N/armorPenetrationBlunt` | 0 | 5 |
| Extra | `/tools/li@N/capacities/li` | 0 | 5 |
| RemovedExtra | `/tools/li@N/capacities/li` | 0 | 4 |
| Extra | `/tools/li@N/cooldownTime` | 0 | 5 |
| Extra | `/tools/li@N/label` | 0 | 5 |
| Extra | `/tools/li@N/linkedBodyPartsGroup` | 0 | 9 |
| Extra | `/tools/li@N/power` | 0 | 5 |

#### fire mode habits

Before 14 findings, after 6.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/comps/li[@Class=CE.CompProperties_FireModes]/aimedBurstShotCount` | 8 | 0 |
| Extra | `/comps/li[@Class=CE.CompProperties_FireModes]/aiUseBurstMode` | 3 | 4 |
| Missing | `/comps/li[@Class=CE.CompProperties_FireModes]/aiUseBurstMode` | 2 | 0 |
| Extra | `/verbs/li[@Class=CE.VerbPropertiesCE]/burstShotCount` | 1 | 1 |
| Extra | `/comps/li[@Class=CE.CompProperties_FireModes]/aimedBurstShotCount` | 0 | 1 |

#### weapon tags beyond the class tag

Before 24 findings, after 14.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/weaponTags/li` | 21 | 11 |
| RemovedExtra | `/weaponTags/li` | 2 | 2 |
| NotRemoved | `/weaponTags/li` | 1 | 1 |

#### verb fields Combat Extended adds

Before 7 findings, after 5.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/verbs/li[@Class=CE.VerbPropertiesCE]/targetParams/canTargetLocations` | 2 | 2 |
| Missing | `/verbs/li[@Class=CE.VerbPropertiesCE]/aimingChargeMote` | 1 | 0 |
| Missing | `/verbs/li[@Class=CE.VerbPropertiesCE]/aimingChargeMoteOffset` | 1 | 0 |
| Extra | `/verbs/li[@Class=CE.VerbPropertiesCE]/recoilAmount` | 1 | 1 |
| Missing | `/verbs/li[@Class=CE.VerbPropertiesCE]/recoilPattern` | 1 | 1 |
| Missing | `/verbs/li[@Class=CE.VerbPropertiesCE]/soundCastTail` | 1 | 1 |

#### ammo comp extras

Before 3 findings, after 3.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/comps/li[@Class=CE.CompProperties_AmmoUser]/AmmoGenPerMagOverride` | 2 | 2 |
| Missing | `/comps/li[@Class=CE.CompProperties_AmmoUser]/reloadOneAtATime` | 1 | 1 |

#### vanilla tool field carried over that Combat Extended dropped

Before 0 findings, after 6.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| NotRemoved | `/tools/li@N/labelUsedInLogging` | 0 | 6 |

#### under barrel comp (weapon platform style)

Before 22 findings, after 22.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/comps/li[@Class=CE.CompProperties_UnderBarrel]/...` | 22 | 22 |

#### left to the vanilla design (economy, art, platform specifics)

Before 85 findings, after 85.

| Kind | Path | Before | After |
| --- | --- | --- | --- |
| Missing | `/modExtensions/li[@Class=CE.GunDrawExtension]/...` | 30 | 30 |
| NotRemoved | `/stuffCategories/li` | 13 | 13 |
| Missing | `/graphicData/drawSize` | 10 | 10 |
| Missing | `/stuffCategories/li` | 9 | 9 |
| NotRemoved | `/comps/li[@Class=CompProperties_EquippableAbilityReloadable]/...` | 7 | 7 |
| Missing | `/costList/Chemfuel` | 5 | 5 |
| Missing | `/costList/WoodLog` | 4 | 4 |
| Missing | `/comps/li/compClass` | 2 | 2 |
| Missing | `/costList/ComponentIndustrial` | 2 | 2 |
| NotRemoved | `/comps/li/compClass` | 1 | 1 |
| Missing | `/costList/Steel` | 1 | 1 |
| NotRemoved | `/equippedStatOffsets/MoveSpeed` | 1 | 1 |

## 6. Value differences

Numbers that the block does not carry come from the estimators or from the vanilla design and are reported as derived values; they differ from Combat Extended's hand tuned numbers by nature. None of them is a structure difference.

| Path | Before | After |
| --- | --- | --- |
| `/tools/li@N/cooldownTime` | 66 | 75 |
| `/tools/li@N/power` | 66 | 75 |
| `/tools/li@N/armorPenetrationBlunt` | 8 | 17 |
| `/tools/li@N/label` | 16 | 16 |
| `/statBases/Mass` | 15 | 15 |
| `/verbs/li[@Class=CE.VerbPropertiesCE]/range` | 15 | 15 |
| `/verbs/li[@Class=CE.VerbPropertiesCE]/warmupTime` | 15 | 15 |
| `/statBases/WorkToMake` | 12 | 12 |
| `/costList/Steel` | 11 | 11 |
| `/tools/li@N/extraMeleeDamages/...` | 0 | 10 |
| `/costList/ComponentIndustrial` | 8 | 8 |
| `/tools/li@N/armorPenetrationSharp` | 8 | 8 |
| `/verbs/li[@Class=CE.VerbPropertiesCE]/burstShotCount` | 8 | 8 |
| `/verbs/li[@Class=CE.VerbPropertiesCE]/ticksBetweenBurstShots` | 8 | 8 |
| `/tools/li@N/linkedBodyPartsGroup` | 0 | 7 |
| `/tools/li@N/chanceFactor` | 0 | 5 |
| `/verbs/li[@Class=CE.VerbPropertiesCE]/soundCast` | 3 | 3 |
| `/comps/li[@Class=CE.CompProperties_FireModes]/aiAimMode` | 8 | 2 |
| `/comps/li[@Class=CE.CompProperties_FireModes]/aimedBurstShotCount` | 0 | 2 |
| `/costList/Plasteel` | 2 | 2 |
| `/description` | 2 | 2 |
| `/comps/li[@Class=CE.CompProperties_FireModes]/aiUseBurstMode` | 3 | 1 |
| `/costList/ComponentSpacer` | 1 | 1 |
| `/costStuffCount` | 1 | 1 |
| `/recipeMaker/researchPrerequisite` | 1 | 1 |

## 7. Gaps closed

| Gap | Change | Evidence |
| --- | --- | --- |
| Applying a patch twice duplicated verbs, ammo comps, tags, stats | the gun conversion is guarded by a conditional on the ammo comp; stat and offset entries are set with replace-or-add conditionals; tags and list entries are added only when absent; a missing list is replaced or added with `Inherit="False"` | 31 of 31 weapons differed, 0 of 31 now |
| Tools had no body part group and no pick weight | learned per label from converted tools, only with agreeing examples, reported as derived values | missing groups 72 to 12, pick weights 29 to 2 |
| Tool capacities and the muzzle of a gun | capacities learned per label (a handle that pokes); a capacity taken from a multi capacity tool moves to a new tool when converted weapons name such a tool | missing capacities 28 to 12 |
| Extra melee damages, surprise attack and other tool children were lost | carried from the vanilla tool | missing 10 to 0, removed extra 6 to 0 |
| Fields of the vanilla verb were lost by the conversion (charge motes, target rules, minimum range, forced miss radius) | carried into `Properties` unless the properties write them themselves | missing aiming charge mote 2 to 0 |
| Fire modes had one shape for every weapon | aim mode and burst flag from the class of the converted guns, the belt fed flag first; `aimedBurstShotCount` is the vanilla burst | missing aimed bursts 8 to 0 |
| Melee conversions wrote no tags | class tag and one handed mark when the user gave them, each added once; update mode adds a missing mark and the class tag | tags of melee weapons |
| A def that only inherits its verbs kept the vanilla shoot verb | its own `verbs` list is started with `Inherit="False"` before the conversion | dry run on fictional defs |
| No rule caught an unguarded conversion or a burst without aimed burst | CEP023, CEP024 | lint tests |

## 8. Not expressible, listed as unsupported

- Weapon platform conversions: `isWeaponPlatform`, attachment links, default graphic parts, and the under barrel comp with its own ammo, verb and fire modes (one weapon family of the install).
- Bows and crossbows: their own conversion style (empty fire modes, an ammo count override, the bow tag).
- Turrets and mechanoid weapons, launchers, one use weapons and explosive weapons: listed with the reason by the scan.
- Tool lists restructured by hand: a tool that Combat Extended adds, relabels or reorders (a pistol whose barrel became a muzzle, a spear and a staff with other labels), and a vanilla tool field that Combat Extended dropped.
- Ammo comp extras `reloadOneAtATime` and `AmmoGenPerMagOverride`, and the verb field `recoilPattern`, which no field of the design maps to.
- Weapon tags that are not the class tag (sidearm, submachine gun, bipod, no switch, simple and advanced gun marks): the generator suggests them as a hint (`ce.companion-tags`) and never writes them.
- A def that inherits its conversion from a converted parent: converting the parent is enough.

## 9. Explained differences that stay

- Economy and art: costs, work to make, stuff categories, the graphic draw size and the gun draw extension are changed by Combat Extended in the same operation. The designer writes vanilla values (D-085); the user changes them in the design when the weapon should differ.
- Vanilla accuracy stats that a def only inherits from its parent are not deleted by the operation (it deletes them in the def's own list); they stay and do nothing.
- A tool field carried from the design that Combat Extended's own conversion dropped (`labelUsedInLogging`) is kept.

## Implications for RimStudio

- The generated patch is safe to apply twice and keeps what the vanilla weapon carries; the economy of a weapon (costs, work, materials) stays a decision of the design, not of the patch.
- Habits of the user's own conversions are a legitimate source for names and counts (groups, pick weights, fire modes) as long as they need agreeing examples and are shown as derived values.
- A conversion style the generator cannot express is listed as unsupported with its reason instead of being approximated.
- The harness is the regression guard for the structure of the patch; it should be run on the owner's install before each release and its totals compared with section 4.

## Open questions

- A leave one out run (habits learned without the weapon under test) would measure the habits fairly; the install has too few converted weapons per class for it to be informative.
- A two capacity tool that Combat Extended merges into one (pistols) is split by the muzzle habit; the user removes the extra tool.
- The melee scan asks nothing about tags; a one handed or class tag is written only when the user answers (`ConvertAnswers`).
