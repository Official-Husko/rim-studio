# Combat Extended patch conventions

Scope: how Combat Extended (CE) compatibility patches are written in practice, measured on CE's own core patches, CE's 759 integrated third-party patch folders, 75 Steam workshop mods that ship their own CE support and the owner's mods, so that RimStudio can generate correct CE patches for new and existing mods. It covers the anatomy of a conversion per item kind, how patches are gated, a quantitative idiom survey with the most common mistakes, the input-to-output specification of a patch generator, and annotated templates. CE is CC BY-NC-SA 4.0 and third-party patches belong to their authors: this note describes patterns in its own words and copies no files; the templates were written from verified patterns.

Status: research note | Last verified: 2026-10-04

Related notes: `docs/research/ce-autopatcher-formulas.md` (numeric formulas for stat values), `docs/research/def-engine-semantics.md` (patch engine semantics), `docs/research/xpath-patch-coverage.md`.

## 1. Method and sources

| Source | What it gave | Pointer |
|---|---|---|
| CE repository checkout (mod version 16.7.3.0, supports 1.6) | Core patches, 760 ModPatches folders, `LoadFolders.xml`, C# source of `PatchOperationMakeGunCECompatible`, `PatchOperationFindMod` and the def classes | `CombatExtended-Development/Patches/Core/**`, `CombatExtended-Development/ModPatches/**`, `CombatExtended-Development/Source/CombatExtended/CombatExtended/` |
| Decompiled game (1.6.4871) | Exact semantics of `PatchOperationFindMod`, `Conditional`, `Sequence`, `MayRequire`, load folders, unknown classes | decompiled:Verse/PatchOperationFindMod.cs, decompiled:Verse/ModLister.cs (HasActiveModWithName, AnyModActiveNoSuffix), decompiled:Verse/PatchOperation.cs, decompiled:Verse/ModContentPack.cs (LoadPatches, InitLoadFolders), decompiled:Verse/DirectXmlToObject.cs, decompiled:Verse/LoadedModManager.cs |
| Survey script (static analysis, lxml, deterministic) | All counts below | `docs/research/data/ce-patch-templates/survey_ce_patches.py`, output `docs/research/data/ce-patch-templates/survey_summary.json` |
| Value distribution script | Medians and percentiles of CE patch numbers | `docs/research/data/ce-patch-templates/value_stats.py`, output `value_stats.json` |
| Owner mods | Real hand-written patches (7 mods mention CE) | `/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods/` |
| CE GitHub wiki, Compatibility Patch Guide (fetched 2026-10-04) | The maintainers' own checklist of what needs patching | https://github.com/CombatExtended-Continued/CombatExtended/wiki/Compatibility-Patch-Guide (and the older pages "Making gun mods CE compatible" and "Creating an xpath patch" on the same wiki) |
| RimWorld wiki, PatchOperations (fetched 2026-10-04) | Secondary reference for operation names | https://rimworldwiki.com/wiki/Modding_Tutorials/PatchOperations |

Corpora in the survey (patch files only, no code executed; zero parse errors in all four):

| Corpus | Patch files | Mods or folders | Top-level operations | Nested operations |
|---|---|---|---|---|
| CE core (`Patches/**`) | 100 | 1 | 1,475 | 254 |
| CE ModPatches (`ModPatches/*/Patches/**`) | 3,574 | 759 | 36,795 | 5,993 |
| Workshop mods that ship CE patches (CE itself excluded) | 355 | 75 | 18,677 | 36,393 |
| Owner mods | 12 | 7 | 12 | 80 |

Caveat: the workshop corpus is the set of mods that happen to be subscribed on this machine, not a random sample. CE ModPatches is written by the CE team and is the most uniform and trustworthy style guide. Where a count is an upper bound (for example dangling references, because the patched mod's defs are not available) the text says so.

## 2. Anatomy of a CE conversion per item kind

General rules that apply to every kind:

1. CE patches are RimWorld `PatchOperation` XML (root `<Patch>`, children `<Operation Class=...>`). Only files under a loaded `Patches/` folder are read (decompiled:Verse/ModContentPack.cs, LoadPatches), the root must be `Patch` and every child must be `Operation`, otherwise the game logs an error and skips it.
2. Weapons that fire projectiles are converted with the CE-specific operation `CombatExtended.PatchOperationMakeGunCECompatible` (3,006 top-level uses in ModPatches, 8.17 percent of operations; 30 in CE core). Everything else is converted with plain Add, Replace, Remove, AddModExtension and Conditional operations addressed by xpath, 95.3 percent of them by `defName` (37,094 of 38,923 xpath operations in ModPatches; 88.05 percent in core, where 10.69 percent address abstract parents with `@Name`).
3. Numbers are CE-scale, not vanilla-scale: armour is in millimetres of steel equivalent, melee tools carry armour penetration, items carry `Bulk`. Section 4 and `ce-autopatcher-formulas.md` give the value sources.
4. Ordering inside a file matters only where one operation creates what a later one needs (container before child, `Class` attribute before CE-only fields). Operations run in file order, mods in load order, and the combined Defs document exists before any patch runs (decompiled:Verse/LoadedModManager.cs, ApplyPatches), so xpath can already see defs from mods that load later; a mod's own patch can therefore target CE defs regardless of load order, but must still run after another mod's patch if it depends on that patch's output.

### 2.1 What `PatchOperationMakeGunCECompatible` does (read from the CE C# source)

`CombatExtended-Development/Source/CombatExtended/CombatExtended/PatchOperationMakeGunCECompatible.cs`. Behaviour in my own words:

| Field in the operation | Effect on `Defs/ThingDef[defName=...]` | Failure or trap |
|---|---|---|
| `defName` | Selects the def by exact defName among ThingDefs; empty gives false; no match gives a logged warning and false | Cannot target abstract parents (`Name=`), only concrete defNames |
| `statBases` | Creates the container if absent, deletes vanilla `AccuracyTouch/Short/Medium/Long`, then replaces same-named stats and appends new ones | Other vanilla stats stay as they were |
| `costList` | Clears the whole existing list, then writes the new one | Cannot add one ingredient; it is replace-all |
| `Properties` | Removes vanilla shoot verbs (`Verb_Shoot`, `Verb_ShootOneUse`, `Verb_LaunchProjectile`) from `verbs`, appends one `VerbPropertiesCE` entry | Other verbs stay; running twice appends twice |
| `AmmoUser`, `FireModes` | Appends a `CompProperties_AmmoUser` and/or `CompProperties_FireModes` entry to `comps` (created if absent) | Not idempotent; an absent `AmmoUser` element means a gun without ammo comp (grenades, bows with spawn count only) |
| `weaponTags`, `weaponClasses` | Appends the children | Appends, never replaces: a second run duplicates tags |
| `researchPrerequisite` | Writes into `recipeMaker` | Needs a recipe-capable def |
| `texPath` | Sets `graphicData/texPath` and forces `Graphic_Single` | Overrides a different graphic class |
| `isWeaponPlatform`, `attachmentLinks`, `defaultGraphicParts` | Converts the def to CE's weapon-platform type for attachments | Out of scope for the first generator |
| `AllowWithRunAndGun` | If false and a mod named RunAndGun is active, adds a modExtension forbidding it | No effect otherwise |

Comments in the operation's child blocks are skipped. The class and all its children are parsed when the file loads, whether or not CE is active (see section 3.3).

### 2.2 Per kind: fields, order, requirement level, failure mode when missing

"REQ" means the weapon or item misbehaves or errors without it; "OPT" means a sensible default exists. Frequencies are the share of `MakeGun` operations in CE ModPatches (3,130 operations on 3,029 distinct defNames) unless stated.

| Kind | Operations emitted, in order | REQ fields | OPT fields | If a REQ field is missing |
|---|---|---|---|---|
| Ranged gun | 1 `MakeGun` (statBases, costList, Properties, AmmoUser, FireModes, weaponTags, researchPrerequisite); 2 Replace `tools` with `ToolCE` entries | `statBases` (99.04 pct), `Properties` with `verbClass`, `defaultProjectile`, `range`, `hasStandardCommand`, `soundCast`, `warmupTime`; `AmmoUser` with `ammoSet`, `magazineSize`, `reloadTime` (90.26 pct have AmmoUser); `FireModes` element (94.63 pct) | `burstShotCount` + `ticksBetweenBurstShots` (52 and 50 pct), `recoilAmount` (63 pct), `muzzleFlashScale`, `soundCastTail`, `targetParams`, `minRange`, `weaponTags` (60.3 pct), `costList` (43.5 pct), `researchPrerequisite` (7.9 pct) | No `hasStandardCommand`: no draft command. No `AmmoUser`: gun has no ammo comp and does not fire as CE gun. Unknown `ammoSet` or `defaultProjectile`: cross-reference error at def load. Vanilla `Verb_Shoot` left: gun ignores CE ballistics |
| Grenade or throwable | 1 `MakeGun` with `Verb_ShootCEOneUse`, no AmmoUser or FireModes; 2 Add `tools`, `tradeTags`; 3 projectile fixes (`thingClass` to `ProjectileCE_Explosive`, `projectile` Class attribute to `ProjectilePropertiesCE`, then CE fields) | `verbClass` OneUse (131 of 3,130 ops; 59 in workshop), `defaultProjectile`, `range`, projectile `thingClass` | `minRange`, `noiseRadius`, `onlyManualCast`, `ai_AvoidFriendlyFireRadius`, `ai_IsBuildingDestroyer` | Vanilla projectile class does not use CE explosion model; setting a CE-only projectile field before the `Class` attribute is set fails to load |
| Bow, crossbow | 1 `MakeGun` with arrow `ammoSet`, empty `<FireModes />`, tag `CE_Bow`, `AllowWithRunAndGun` false; 2 Replace `tools` with one blunt tool | as gun; `ammoSet` | `AmmoGenPerMagOverride` instead of a real magazine (131 uses in ModPatches) | As gun |
| Melee weapon | 1 Replace `tools`; 2 Add `statBases` (Bulk, MeleeCounterParryBonus); 3 Add `equippedStatOffsets` (MeleeCritChance, MeleeParryChance, MeleeDodgeChance); 4 Replace `stuffCategories` entry for crafting | every tool is `Class="CombatExtended.ToolCE"` with `power`, `cooldownTime`, `capacities`, `armorPenetrationBlunt`, plus `armorPenetrationSharp` for Cut, Stab, Scratch and Bite tools | `chanceFactor` (37 pct of tools), `linkedBodyPartsGroup` (92 pct), `surpriseAttack`, `extraMeleeDamages` | A tool without the class loads as vanilla `Tool` (no penetration: always blocked by CE armour). A tool with `Class` but no penetration fields is legal; 74 such entries in ModPatches |
| Apparel | 1 Replace or Add `statBases` (`StuffEffectMultiplierArmor` for stuffable cloth; `ArmorRating_Sharp`, `ArmorRating_Blunt`, `ArmorRating_Heat` for fixed armour); 2 Add `Bulk`, `WornBulk`; 3 Replace `MaxHitPoints`, `Mass` where needed; 4 optional `PartialArmorExt`, `equippedStatOffsets`, `CompProperties_ArmorDurability` | `Bulk` and `WornBulk` (Add Bulk 2,705, Add WornBulk 1,449 operations), armour ratings re-scaled | `PartialArmorExt` (350 uses), carry stats, night vision | Vanilla fractional ratings (0.0 to 2.0) would be read in CE units, so the item is expected to give almost no protection; missing `Bulk` is expected to leave the item without inventory volume (not verified in game) |
| Ammo and projectile | New Defs file (not a patch): `AmmoSetDef`, `ThingCategoryDef`, abstract ammo base, concrete `AmmoDef`s, projectile base from `BaseBulletCE`, concrete projectiles | AmmoSet maps each ammo item to a projectile; each concrete projectile sets `damageAmountBase`, `armorPenetrationSharp`, `armorPenetrationBlunt` and (via base) `speed` and `damageDef` | `similarTo`, `generateAllowChance`, `cookOffProjectile`, `dropsCasings` | Dangling ammo or projectile reference: error at def load; ammo without trade and crafting tags is never traded or crafted automatically |
| Turret | 1 `MakeGun` for the gun with `recoilPattern` Mounted; 2 Replace `thingClass` with `CombatExtended.Building_TurretGunCE`; 3 Add `AimingAccuracy`, Replace `ShootingAccuracyTurret`, Mass plus Bulk; 4 Remove vanilla comps CE replaces | building `thingClass`, gun conversion | `turretBurstCooldownTime`, power and cost adjustments | Vanilla building class cannot use CE ammo and reloading |
| Mech weapon | Gun conversion as above; race: `RacePropertiesExtensionCE` (bodyShape), pawn `statBases` (AimingAccuracy, ShootingAccuracyPawn, melee chances, CarryWeight, CarryBulk), `tools` as `ToolCE`; pawn kind: `LoadoutPropertiesExtension` (ammo counts) | gun conversion; race extension | loadout counts | Spawned pawn with a CE gun but no loadout may carry little or no spare ammo (the CE guide notes this for pawn kinds; the exact default was not verified in game) |

Operation shapes seen in numbers (ModPatches top-level signatures): `Add:statBases` 4,301, `Replace:tools` 3,313, `MakeGun` 3,006, `Replace:statBases/ArmorRating_Sharp` 1,891, `Replace:statBases/ArmorRating_Blunt` 1,843, `Add:<def>` 1,606, `AddModExt:RacePropertiesExtensionCE` 1,140, `Replace:statBases/StuffEffectMultiplierArmor` 918, `Add:equippedStatOffsets` 887, `AddModExt:LoadoutPropertiesExtension` 839 (file `survey_summary.json`, key `idioms`). The two most frequent adjacent pairs are `Replace ArmorRating_Sharp` then `Replace ArmorRating_Blunt` (1,321) and `Replace tools` with `Add statBases` in either order (916 and 880).

### 2.3 Projectiles and ammo sets in CE's own definitions

- Projectile `thingClass` over 2,594 concrete CE projectile defs: `BulletCE` 2,011, `ProjectileCE_Explosive` 359, laser beam 107, inherited or none 60, `ProjectileCE_Bursting` 17. The commonest abstract parents are calibre bases (for example `Base6x24mmChargedBullet` 68, `Base556x45mmNATOBullet` 46) and the generic `BaseBulletCE` (51 direct children). The generic base sets the thing class to CE's bullet class and the projectile properties class to CE's, so a child def only restates `projectile` fields it changes.
- Ammo sets: 932 `AmmoSetDef`s, 424 hold a single ammo type, 159 hold three, 87 hold six; 291 carry `similarTo`. 1,557 concrete `AmmoDef`s. Ammo items carry trade tags such as `CE_AmmoInjector` (1,385 defs), `CE_AutoEnableTrade` (1,273) and `CE_Ammo` (861); the names indicate that CE injects trade and recipes from the tags without per-item patches (injector internals not read).
- CE ships data that a third-party patch should reuse: the most used ammo sets in workshop gun patches are `AmmoSet_556x45mmNATO` (620 weapons), `AmmoSet_9x19mmPara` (470) and `AmmoSet_762x51mmNATO` (438). A generator offers these first and only creates a new calibre when asked.

Value ranges (all CE patch corpora, `value_stats.json`; includes animals, mechs and outliers, so use medians): gun `range` median 44 cells (p10 12, p90 75), `magazineSize` median 20 (p10 3, p90 100), `reloadTime` median 4.0 s, `RangedWeapon_Cooldown` median 0.40, `SwayFactor` median 1.28, `ShotSpread` median 0.10, `SightsEfficiency` median 1.0, `recoilAmount` median 1.46, `Bulk` median 8.35, `Mass` median 3.85; melee tool `power` median 8, penetration-to-power ratio median 0.26 blunt and 0.05 sharp (p90 0.60 and 0.67); apparel `StuffEffectMultiplierArmor` median 3, `WornBulk` median 2, `Bulk` median 5. These seed validation warnings, not defaults.

## 3. Gating idioms

### 3.1 How CE is detected (verified in the decompiled game)

| Mechanism | Matches against | Case | Notes | Evidence |
|---|---|---|---|---|
| `PatchOperationFindMod` (vanilla) with `<mods><li>NAME</li></mods>` | `About.xml` `<name>` of an active mod, exact string equality | Case-sensitive, no trimming | A packageId never matches. Match branch runs if any listed name is active; a missing branch counts as success; with no match and no `nomatch` the result is also success | decompiled:Verse/PatchOperationFindMod.cs, decompiled:Verse/ModLister.cs (HasActiveModWithName compares the name field) |
| `CombatExtended.PatchOperationFindMod` with `<modName>` | `Name` of an active mod, via CE's own class | Case-sensitive | Returns false when not active, true when active; has no match or nomatch branches, so it is only useful as the first step of a `Sequence` | `CombatExtended-Development/Source/CombatExtended/CombatExtended/PatchOperationFindMod.cs`; the owner's Gewehr 41 mod uses it |
| `PatchOperationConditional` with an xpath for a CE-only def | Presence of a node in the combined Defs | n/a | Independent of mod names; survives renames. When the xpath matches and there is no `match` branch, the result is success only if a `nomatch` branch exists (a quirk) | decompiled:Verse/PatchOperationConditional.cs |
| `LoadFolders.xml` `IfModActive="..."`, also `IfModActiveAll` and `IfModNotActive` | packageId list, comma separated | Lowercased on both sides; matching ignores a platform suffix on the active mod's id (`_steam`, `_copy`) but not on the string you write | Selects whole folders. Version block selection: exact `v1.6`, else the highest older version, else `default` | decompiled:Verse/ModLoadFolders.cs, decompiled:Verse/LoadFolder.cs (ShouldLoad), decompiled:Verse/ModContentPack.cs (InitLoadFolders), decompiled:Verse/ModLister.cs (GetActiveModWithIdentifier) |
| `MayRequire="packageId"` and `MayRequireAnyOf` | packageId, comma separated | Lowercased | Honoured on `<li>` list items and on top-level def nodes. Not honoured on an `<Operation>` element: patch operations are deserialized without that check | decompiled:Verse/DirectXmlToObject.cs (list loading), decompiled:Verse/LoadedModManager.cs (def loading), decompiled:Verse/ModContentPack.cs (LoadPatches) |
| `loadAfter` in `About.xml` | packageId of CE (`ceteam.combatextended`) | n/a | Not a gate. It orders the mod after CE so the mod's patches run after CE's, which matters when a patch edits something CE's patches also edit | CE `About/About.xml` lists packageId `CETeam.CombatExtended` |

CE's own package id is `CETeam.CombatExtended` and its name is `Combat Extended`; the `FindMod` string `Combat Extended` appears 195 times in workshop CE patches and 8 times in owner mods. Both vanilla `FindMod` string and packageId mechanisms exist, and mixing them up is the cause of mistakes 6 and 10 below. A generator must keep two separate fields: `ceName` (About name) and `cePackageId`.

### 3.2 Where CE patches live

| Placement | Share | How it is gated | Notes |
|---|---|---|---|
| CE `ModPatches/<Mod>/Patches/...` and `ModPatches/<Mod>/Defs/...` | 3,574 files in 759 mods | CE's root `LoadFolders.xml` has 769 `IfModActive` entries in the `v1.6` block, one per patched mod (40 list several package ids, 5 ModPatches folders on disk are not referenced); inside the folder operations are unconditional (93.98 pct direct) | Authors of a patched mod never write a gate; CE does. Only 2 of 3,574 files are never loaded |
| A mod's own `CE/` (or similar) folder selected by its `LoadFolders.xml` | 44 of 75 surveyed workshop mods | `IfModActive` on the mod's own folder; id spellings in the wild: `CETeam.CombatExtended` 72, `_copy` 50, `_steam` 7, lowercase 7, `_Development` 2 | The `_copy` spellings were pasted from local copies; they only work when the active mod id carries that suffix |
| A mod's own `Patches/` with `FindMod` by name | 37 of 75 workshop mods; all 7 owner mods | `PatchOperationFindMod` or `Conditional` per operation, often inside a `Sequence` | Simple but CE classes still parse when CE is absent (3.3) |
| Always loaded, no gate | 5 of 75 | none | Plain vanilla-class patches that are harmless without CE |
| Hard dependency on CE | 1 of 75 | About.xml `modDependencies` | Not a patch, a different product |

`loadAfter` CE is declared by 43 of 75 workshop mods and 5 of 7 owner mods.

### 3.3 Behaviour when CE is inactive, loads later, or classes are missing

1. A CE class (`CombatExtended.PatchOperationMakeGunCECompatible`, `CombatExtended.PatchOperationFindMod`) inside a patch file that loads without CE cannot resolve: the game logs "Could not find type named ..." and falls back to the base operation, which logs that the patch will always fail (decompiled:Verse/DirectXmlToObject.cs, decompiled:Verse/PatchOperation.cs). Deserialization happens at load, so wrapping CE classes in a vanilla `FindMod` or `Conditional` does not help. The survey flags 22 operations in 3 owner mods and 95 operations in 16 workshop mods (lint CEP004). The only safe forms are a gated folder (`LoadFolders`) or restricting a file to vanilla classes.
2. An operation that never succeeds is reported as an error after patching (the `Complete` step logs "Patch operation ... failed" with the file) (decompiled:Verse/PatchOperation.cs). A `Sequence` stops at the first failing step and returns false without undoing earlier steps; `<success>Always</success>` hides the failure, which the owner's mods use. Generated patches should be designed so that nothing fails, not hidden with `Always`.
3. Because the combined Defs document is built before patching, load order between CE and the patched mod does not change whether an xpath finds its def. Load order changes only the order of patches, so a mod patching a def that CE's ModPatches also patch needs `loadAfter` CE and should use Replace on the CE-written value.
4. Another mod's `ModPatches` entry for the same target can cause double conversion. If a mod ships its own CE patch and CE also has a ModPatches folder for it, both run; `MakeGun` run twice duplicates comps, tags and verbs (lint CEP007: 8 cases in ModPatches, 10 in workshop). The generator must detect an existing CE conversion before emitting (section 5.4).
5. CE has two kinds of mod-specific data: packageId-gated `ModPatches` and the global `Defs/GunPatcherDefs` folder, which appears to hold CE's own auto-conversion rules (not read in detail) (see `ce-autopatcher-formulas.md`). A mod author can rely on neither for unknown mods, so an explicit patch is needed.

### 3.4 Version folders

CE's root `LoadFolders.xml` defines only `v1.6` (the CE version surveyed supports only 1.6). Mod authors usually use version folders (`1.5/`, `1.6/`) or one `CE/` folder and list it under every `v1.x` block; the owner's Ratchet and Clank weapons mod does this for 1.0 to 1.2 with `IfModActive="CETeam.CombatExtended"`. The game picks the block for the running version, else the newest older block, else `default` (decompiled:Verse/ModContentPack.cs). None of the 7 owner mods lists 1.6 in `supportedVersions` (survey field `supports_1_6`), so CE generation must also offer to add a `v1.6` block.

## 4. Quantitative survey of idioms

All figures from `survey_summary.json` (`corpus_overview`, `operation_classes`, `gating`, `idioms`, `weapons_make_gun`, `tools`, `stats_and_extensions`).

### 4.1 Operation classes

| Class | CE core top-level (1,475) | CE ModPatches top-level (36,795) | Workshop top-level (18,677) |
|---|---|---|---|
| `PatchOperationReplace` | 35.19 pct | 45.16 pct | 31.94 pct |
| `PatchOperationAdd` | 32.20 pct | 30.18 pct | 4.42 pct |
| `PatchOperationConditional` | 14.03 pct | 4.86 pct | 51.62 pct |
| `MakeGunCECompatible` | 2.03 pct | 8.17 pct | 0.69 pct |
| `PatchOperationAddModExtension` | 8.54 pct | 6.88 pct | 3.68 pct |
| `PatchOperationRemove` | 4.47 pct | 3.00 pct | 5.46 pct |
| `PatchOperationSequence` | 0.14 pct | 0.23 pct | 0.67 pct |
| `PatchOperationFindMod` | 0.07 pct | 0.79 pct | 1.24 pct |
| `PatchOperationAttributeSet` | 1.42 pct | 0.54 pct | 0.24 pct |

Workshop is dominated by Conditional because a few large animal and race mods emit hundreds of identical upsert blocks (for example race fields 340 to 430 times each). The class names are case-sensitive: ModPatches contains 9 operations spelled `PatchOperationreplace` and 1 `PatchOperationadd`, which resolve to no class (mistake 10).

### 4.2 Selectors, containers and the upsert idiom

- xpath selectors in ModPatches: by `defName` 95.30 pct, by `@Name` (abstract parent) 4.33 pct, mixed predicates 0.28 pct; 252 (0.65 pct) start with a slash, 337 use the descendant axis. A selector with several defNames joined by `or`: 1 name 33,750, 2 to 3 names 2,205, 4 to 9 names 921, 10 or more 218.
- Adds into containers that a def may not have (statBases, equippedStatOffsets, weaponTags, comps, costList): 6,181 in ModPatches, of which 5,505 (89.1 pct) have no preceding existence check in the same file. CE authors know the vanilla XML; a generator working on arbitrary mods does not, so it must check the loaded def (statBases 4,073, equippedStatOffsets 797, weaponTags 289, comps 167 unguarded).
- The two guard idioms: ensure-container (`Conditional` with `nomatch` Add of an empty container, then Add children): 1,461 in ModPatches (`comps` 612, `weaponTags` 184, `equippedStatOffsets` 67); and upsert (`Conditional` with `match` Replace, `nomatch` Add): 7,075 in workshop (the dominant workshop shape), 12 in ModPatches. A third shape, `match` Remove with no `nomatch` (6,426 in workshop), typically strips a vanilla value when present.
- Replace `tools` as a whole list is the standard for tools (3,313 top-level in ModPatches); Replace of one stat uses `statBases/StatName` and fails if the stat is absent, hence `Add statBases` first for new CE stats (Bulk, WornBulk).

### 4.3 Weapons

| Measure | CE core (30 MakeGun) | ModPatches (3,130) | Workshop (3,385) |
|---|---|---|---|
| `verbClass` `Verb_ShootCE` | 25 | 2,929 | 3,320 |
| `Verb_ShootCEOneUse` | 5 | 131 | 59 |
| `Verb_ShootMortarCE` | 0 | 40 | 4 |
| `verbClass` missing | 0 | 29 | 0 |
| has `AmmoUser` | 83.3 pct | 90.26 pct | 98.2 pct |
| has `FireModes` | 90.0 pct | 94.63 pct | 98.79 pct |
| has `weaponTags` | 86.7 pct | 60.32 pct | 61.92 pct |
| has `costList` | 60.0 pct | 43.45 pct | 3.6 pct |
| has `researchPrerequisite` | 53.3 pct | 7.86 pct | 0.44 pct |
| `AllowWithRunAndGun` present | 30.0 pct | 13.07 pct | 6.38 pct |

Observations: third-party authors rarely touch `costList` and research (3.6 and 0.44 pct) because the base mod already defines them; the CE team often re-balances them. Tool coverage in ModPatches: 10,149 tool entries, all `ToolCE`; armour penetration present as blunt-only 54.2 pct, both 44.88 pct, none 0.73 pct, sharp-only 0.19 pct. In workshop patches only 2,842 of 4,095 tool entries carry the `ToolCE` class (1,238 are plain vanilla `Tool`, 30.2 pct), and 30.72 pct have no penetration at all.

Mod extensions added in ModPatches: `RacePropertiesExtensionCE` 1,266, `LoadoutPropertiesExtension` 867, `PartialArmorExt` 350, `GunDrawExtension` 92, `DamageDefExtensionCE` 26, `ShieldDefExtension` 15.

### 4.4 Integration styles per author

Workshop (75 mods): LoadFolders-gated CE folder 44, FindMod by name 37 (some use both), ungated 5, hard dependency 1. Owner (7 mods): all FindMod by name, none with a LoadFolders CE folder. Style choice follows the author's habit and era (older mods use FindMod; newer ones use a CE folder).

## 5. The 10 most common mistakes found

Source: lint rules CEP001 to CEP022 in `survey_ce_patches.py` (`python3 survey_ce_patches.py lint FILE_OR_DIR` runs the same checks on any patch), plus survey counts. Rule hits are counts of operations or entries; "mods" is the number of distinct mods affected.

| # | Mistake | Count | Consequence | Generator rule |
|---|---|---|---|---|
| 1 | Exact duplicate operation (same class, xpath, value) in one mod (CEP021) | ModPatches 182 ops in 105 mods; workshop 1,004 ops in 29 mods; owner 8 in 4 mods | Appending containers (weaponTags, comps, parts) double up; silent bloat | De-duplicate by (class, normalized xpath, canonical value) before writing |
| 2 | Wrong ammo set or projectile reference (CEP013, CEP014; upper bounds because the patched mod's own defs are not scanned) | ammoSet 5 in ModPatches, 309 in workshop; projectile 154 and 273 | Cross-reference error at def load, weapon unusable | Validate every defRef against CE defs, the target mod and vanilla |
| 3 | `MakeGun` operation missing a section or key field (CEP008) | 321 ops in 79 ModPatches mods; 2 workshop | No ammo comp, no projectile, or vanilla verb retained | Emit the required set per kind (table 2.2) and refuse to save otherwise |
| 4 | CE-only classes in a patch file loaded without CE (CEP004) | owner 22 ops in 3 mods; workshop 95 in 16 | Error log spam when CE is absent | Put CE classes only in a LoadFolders-gated folder |
| 5 | Patch file in a folder that `LoadFolders.xml` never loads (CEP018) | ModPatches 2 files in 1 mod; owner 4 in 1; workshop 49 in 12 | The patch silently never applies | Cross-check emitted folder against the selected version block |
| 6 | `IfModActive` id variant that cannot match (`_copy`, `_copy_copy`) (CEP019) | workshop 86 entries in 33 mods; ModPatches 5 | The CE folder never loads | Write the canonical id `ceteam.combatextended`; warn on suffixes |
| 7 | Unknown or invented `CE_` weapon tag (CEP016) | ModPatches 44 in 20 mods; owner 10 in 2 | Weapon not classified for AI loadouts | Offer only tags found in the installed CE data |
| 8 | `ToolCE` with no penetration fields (CEP011), or tools without the CE class | ModPatches 74 in 18 mods; workshop 5; plus 1,238 plain tools in workshop | Melee blocked by all armour | Always add `Class` and at least blunt penetration |
| 9 | Same def converted twice with `MakeGun` (CEP007) | ModPatches 8 in 7 mods; workshop 10 in 7 | Duplicated comps, verbs, tags | Detect an existing CE conversion before generating; skip or switch to update mode |
| 10 | Misspelled operation class or `FindMod` name; `MayRequire` on an `<Operation>` (CEP002, CEP005; class spelling in 4.1) | CEP002 1 ModPatches and 10 workshop; CEP005 7 in 3 mods; 10 misspelled class names | Operation never runs; error log | Use enumerated classes; keep `ceName` exact; never emit MayRequire on Operation |

Additional smaller findings: malformed xpath such as a bare name inside a predicate (CEP022, 9 operations in 4 mods), a `.XML` extension in upper case that breaks on case-sensitive file systems (CEP020, 1), a non-CE verb class inside `Properties` (CEP009, 1), an unknown field name such as `AmmoGenPerMagOverride` placed in `FireModes` instead of `AmmoUser` (CEP015, 1), a BOM at file start (56 of 3,574 ModPatches files; the game handles it but diff tools may not).

## 6. Generator specification (input to output)

The patch generator is a pure function from a typed item description plus an environment snapshot to a node tree. Fields have one of three sources: V from the vanilla (or base-mod) def, U from user input in the item designer, C computed (formula or calibration, requirement R7).

### 6.1 Environment inputs

| Input | Source | Used for |
|---|---|---|
| Target def (parsed, with inheritance resolved) | the def engine reading the mod's XML | existence checks for containers, existing CE conversion detection, V values |
| CE install facts: `ceName`, `cePackageId`, set of `AmmoSetDef`, projectile defs, weapon tags, ammo categories, body part groups | scan of the user's installed CE at runtime (no CE data is shipped, R11) | defRef validation and pickers |
| Game version and mod `supportedVersions`, existing `LoadFolders.xml` | the mod project | folder placement, version block |
| Load order position of CE relative to the mod | mod manager | `loadAfter` suggestion |

### 6.2 Per kind: fields and sources

| Kind | Field | Src | Rule |
|---|---|---|---|
| Gun | `defName` | V | Concrete def only |
| | `Mass`, `RangedWeapon_Cooldown`, `warmupTime`, `range`, `soundCast`, `WorkToMake` | V | Keep vanilla value unless the user overrides; CE range is usually vanilla range |
| | `Bulk`, `SwayFactor`, `ShotSpread`, `recoilAmount`, `SightsEfficiency` | C or U | Formula mode from role and mass, or calibration from a reference gun; user may type values |
| | `defaultProjectile`, `ammoSet` | U | Picker over CE and mod defs; `defaultProjectile` must be a member of the chosen set's projectiles |
| | `magazineSize`, `reloadTime` | U with C default | Default from the reference weapon of the same class |
| | `burstShotCount`, `ticksBetweenBurstShots`, `aimedBurstShotCount`, `aiAimMode`, `aiUseBurstMode` | V then C | Burst from vanilla verb; aimed burst usually a fraction of the burst |
| | `weaponTags` | U with C default | One `CE_AI_*` class tag chosen from the installed CE data |
| | `verbClass` (always `CombatExtended.Verb_ShootCE`, OneUse for thrown), `hasStandardCommand` true | constant | |
| | `tools` | C | One to three `ToolCE` entries from the gun's role (stock, barrel, muzzle); `armorPenetrationBlunt` from `power` times the median ratio |
| Grenade | `verbClass` OneUse, no AmmoUser | constant | Projectile edits: `thingClass`, `projectile` Class attribute, explosion fields, then CE penetration and suppression |
| Bow | `ammoSet` arrow or bolt, empty `FireModes`, `CE_Bow`, `AllowWithRunAndGun` false | U or constant | |
| Melee | tools | V + C | Keep labels, capacities, `linkedBodyPartsGroup` from vanilla; compute `power` scaling, `armorPenetrationBlunt`, `armorPenetrationSharp` from capacity type |
| | `Bulk`, `MeleeCounterParryBonus`, `equippedStatOffsets` | C | |
| Apparel | `ArmorRating_*` | V + C | Convert vanilla fractions to CE millimetre scale by body-coverage and layer; user may override |
| | `StuffEffectMultiplierArmor` or fixed ratings | V | Stuffable gear gets a multiplier, fixed gear gets ratings |
| | `Bulk`, `WornBulk`, `MaxHitPoints` | C | |
| Ammo and projectile | `AmmoSetDef`, ammo defs, projectile defs | U + C | Only when no CE set fits; damage, penetration and speed from calibre class |
| Turret | gun fields as Gun; building thingClass constant | | `recoilPattern` Mounted |
| Mech | race and pawnkind extensions | U + C | body shape picker, loadout counts |

### 6.3 Emission rules

1. Operation order per item: ensure-containers, then adds, then replaces, then removes; `MakeGun` before `Replace tools`.
2. Prefer one `MakeGun` per weapon for gun-like items, and a `Replace tools` list with `defName="A" or defName="B"` for weapons that share a tool set.
3. Check the loaded target def: if a container exists use Replace or Add into it; if not emit the ensure-container idiom; never emit Replace on a path that does not exist.
4. Do not emit CE operations when the target already carries a CE conversion (detected by `CombatExtended.VerbPropertiesCE` in `verbs`, a `ToolCE` tool, or `CombatExtended.CompProperties_AmmoUser`); offer update mode instead, which uses Replace on individual fields.
5. Files go into a CE folder selected by `LoadFolders.xml`; when the mod has no `LoadFolders.xml`, create one with the root folder plus the CE folder under the current version block. Fallback for mods that must not use LoadFolders: a file with only vanilla classes under a `FindMod` using the exact `ceName`.
6. Naming: `Patches/` file per category (`Weapons_Ranged.xml`, `Weapons_Melee.xml`, `Apparel.xml`, `Ammo.xml`); keep the defName order stable; use `<!-- ====== Name ====== -->` section comments; new ammo defNames follow `Ammo_<Calibre>_<Type>`, `Bullet_<Calibre>_<Type>`, `AmmoSet_<Calibre>`, abstract bases `Base<Calibre>Bullet`.
7. Static validation (run before write, mapped to lint ids): well-formed XML with `Patch` root (CEP017); class names from an enumerated list; `FindMod` string equals `ceName` exactly (CEP001 to 003); no CE class outside a gated folder (CEP004); no MayRequire on Operation (CEP005); no duplicate operations (CEP021); no repeated `MakeGun` per def (CEP007); required keys present per kind (CEP008); verb class is a CE verb (CEP009); defRefs resolve (CEP013, CEP014); known fields only (CEP015); known `CE_` tags (CEP016); folder is loaded (CEP018); id has no suffix (CEP019); file extension and folder case match (CEP020); xpath well-formed with no leading slash needed, `Defs` root (CEP022).
8. After writing, run a dry apply in the app's own patch engine on the def tree and report any operation that fails (the game would log it as an error).

### 6.4 Holding templates as JSON node trees (R10)

Each template is a JSON document: `{template, version, params, operations[]}`. `params` declare each placeholder with source (`vanilla`, `user`, `computed`), type and constraints (number range, defRef target type). `operations` is a list of nodes `{name, @attrs, #text, children}` plus an optional `requires` object (for example the mod packageId that gates the output folder) and an `output` object (folder, file name, gate kind). A renderer in the XML boundary crate walks the tree, substitutes parameters and emits XML; it is the only place XML is produced. See `docs/research/data/ce-patch-templates/node-tree.example.json` for template 01 in this form. Presets and calibration results (R7) are JSON; the XML skeletons in the templates folder are documentation of expected output only and are used as golden-output comparisons through the renderer, never parsed by the app for its own use.

## 7. Templates

Index at `docs/research/data/ce-patch-templates/README.md`. Files:

| File | Content |
|---|---|
| `01-ranged-gun.xml` | MakeGun conversion with tools, every field annotated REQ or OPT and with its source |
| `02-grenade-throwable.xml` | One-use verb, projectile class fixes, tool and trade tag |
| `03-bow.xml` | Arrow set, empty FireModes, spawn count |
| `04-melee-weapon.xml` | Tools, statBases, equippedStatOffsets, stuff category |
| `05-apparel.xml` | Stuffable, fixed armour, partial armour |
| `06-ammo-and-projectile.xml` | New calibre: set, ammo base, ammo, projectile base, projectile (Defs file) |
| `07-turret.xml` | Building class and stats plus the gun |
| `08-mech-weapon.xml` | Gun, race, pawn kind layers |
| `09-gating-and-folders.xml` | LoadFolders, FindMod, Conditional probes, ensure-container, Sequence |
| `node-tree.example.json` | The JSON node-tree form of template 01 |

## Implications for RimStudio

1. The generator emits `CombatExtended.PatchOperationMakeGunCECompatible` for every projectile weapon and plain operations for everything else; a unit test generates template 01 from the example node tree and compares with a golden XML.
2. CE classes are written only into a folder selected by `LoadFolders.xml` with `IfModActive="ceteam.combatextended"`; a test asserts that no emitted file outside that folder contains a `CombatExtended.` class.
3. The app keeps `ceName` ("Combat Extended", compared exactly, case-sensitive, by `FindMod`) and `cePackageId` as distinct fields; a validator fails any `FindMod` entry that looks like a packageId.
4. Before emitting, the app resolves the target def and picks Add, Replace or the ensure-container idiom from the actual container state; a test with a def lacking `statBases` must produce a patch that applies without error in the app's dry-run engine.
5. Existing CE conversions are detected (VerbPropertiesCE, ToolCE, AmmoUser comp) and the app switches to update mode instead of running `MakeGun` twice (mistakes 1 and 9).
6. Every defRef (ammoSet, defaultProjectile, weapon tag, body part group, research project) is validated against CE data scanned at runtime plus the mod's own defs and vanilla; unresolved references block saving.
7. Required keys per kind (table 2.2) are enforced by the item designer form: no gun without verb class, projectile, range, ammo set, magazine size and reload time; no melee tool without `ToolCE` class and penetration.
8. Value inputs are range-checked against the survey percentiles (`value_stats.json`) and flagged, not clamped: for example melee tool penetration far above the p90 ratio or apparel ratings below 1 (vanilla fractions) on a CE patch.
9. The generator ships a `lint` mode compatible with CEP001 to CEP022 for existing mods ("check my CE patch"), with the rule ids as stable identifiers in the UI and in JSON reports.
10. LoadFolders handling is part of the output: the app creates or edits `LoadFolders.xml`, adds a `v1.6` block when the mod's supported versions lack it, and warns on id suffixes (`_copy`, `_steam`) and on patch files in unselected folders.
11. Templates and presets are JSON node trees with typed parameters; XML is produced only by the boundary crate renderer, and the XML files in the templates folder remain documentation and golden references.
12. The numbers for `Bulk`, `SwayFactor`, `ShotSpread`, tool penetration and armour conversion come from the R7 formula or calibration modes (see `ce-autopatcher-formulas.md`); this note fixes only the shape of the output.

## Open questions

1. Where exactly are CE's default vanilla-weapon `AmmoSet` assignments and tag-to-loadout mappings read from at runtime, and can RimStudio list all `CE_AI_*` classes from the installed CE data without parsing C# (the weapon tag list was derived from Defs and the CE source by the survey script)?
2. What does a pawn kind with a CE gun but no `LoadoutPropertiesExtension` get in game (ammo count default)? Not verified; needs an in-game test.
3. Is `PatchOperationMakeGunCECompatible` safe to run on a def whose `verbs` already contain a CE verb plus other verbs (alternate fire modes)? The source suggests a second `VerbPropertiesCE` is appended; behaviour with multiple verbs was not tested.
4. Should the generator support weapon platforms (attachments) in a later release? The operation supports it; the corpus shows only 1 use of `weaponClasses` and few platform defs.
5. How should update mode treat patches written by another author in the same file (preserve comments and formatting, or regenerate the whole file)? A round-trip XML writer decision is needed.
6. Numeric armour conversion for the vanilla-to-CE scale of modded apparel is in the formulas note; does CE ship an apparel auto-patcher for mods that skip the patch (source `ApparelAutoPatcher.cs`), and should the app mirror its output as a default?
7. Which CE versions older than 16.7 (supporting 1.5) must the generator target, given that the templates were verified only against 16.7.3.0 and game 1.6.4871?
