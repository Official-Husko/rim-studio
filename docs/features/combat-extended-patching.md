# Combat Extended patching

This document specifies the Combat Extended (CE) patch generator of the RimStudio item designer (requirement R7): the pure function that turns a typed item description and a snapshot of the user's environment into JSON node trees for CE compatibility patches, per item kind, with required fields, gating and folder output, an update mode for items that are already converted, a lint mode with the stable rule ids CEP001 to CEP022, the handling of `PatchOperationMakeGunCECompatible`, and validation against the user's own CE data. It fixes the shape of the output; the numbers come from the formulas and calibration modes of [item balance math](item-balance-math.md). The product behaviour around it is in [items toolkit](items-toolkit.md). The evidence is [CE patch conventions](../research/ce-patch-conventions.md), [Combat Extended model](../research/combat-extended-model.md), [CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) and [def engine semantics](../research/def-engine-semantics.md). CE is CC BY-NC-SA 4.0 and third-party patches belong to their authors: this document describes patterns in its own words and nothing here is copied from CE (R11).

Status: draft, section 14 records the 0.1.0 backend as built | Last updated: 2026-10-06

## Contents

1. [Scope and constraints](#1-scope-and-constraints)
2. [Inputs: the environment snapshot and the CE reader](#2-inputs-the-environment-snapshot-and-the-ce-reader)
3. [The generator contract](#3-the-generator-contract)
4. [Per item kind](#4-per-item-kind)
5. [Emission rules](#5-emission-rules)
6. [Update mode](#6-update-mode)
7. [Handling of MakeGunCECompatible](#7-handling-of-makeguncecompatible)
8. [Lint mode](#8-lint-mode)
9. [Gating, folders and versions](#9-gating-folders-and-versions)
10. [Validation against the user's CE data](#10-validation-against-the-users-ce-data)
11. [Architecture mapping](#11-architecture-mapping)
12. [Requirements and tests](#12-requirements-and-tests)
13. [Owner decisions, conflicts and open points](#13-owner-decisions-conflicts-and-open-points)
14. [As built in the 0.1.0 backend](#14-as-built-in-the-010-backend)

## 1. Scope and constraints

CE replaces vanilla combat with its own units and classes: armor in millimetres of steel equivalent and MPa, items with Bulk, guns with ammo sets, melee tools with penetration. A CE patch is RimWorld `PatchOperation` XML (root `Patch`, children `Operation`). Weapons that fire projectiles are converted with CE's own operation `CombatExtended.PatchOperationMakeGunCECompatible`; everything else is converted with plain Add, Replace, Remove and AddModExtension operations addressed by xpath, 95 percent of them by `defName` ([CE patch conventions](../research/ce-patch-conventions.md) section 2).

| Rule | Consequence |
| --- | --- |
| R10, I-02, I-03 | The generator builds JSON node trees; XML text is produced only by `rimstudio-xml::render` (new files) and byte-span edits (existing files, D-013). Templates, presets and plans are JSON. |
| R11, I-06, I-07 | No CE value, preset, ammo table or tag list is stored in the repository. The generator reads the user's installed CE at runtime. XML skeletons in the research folder are documentation and golden references only. Test vectors use fictional numbers. |
| D-085 | The generator only produces optional patches. The designer always writes vanilla definitions; a CE patch is generated only when the user turns on the per item toggle (or runs the Convert flow), is never automatic even when CE is installed, goes into its own files in a gated folder and is never mixed into a vanilla definition. |
| D-065 | The generator lives in `rimstudio-design::ce::patchgen`; orchestration and write plans in `rimstudio-toolkit::designer`. |
| I-10 | Findings are diagnostics (`ce.*`), never errors, except structural impossibilities that block writing. |
| I-05 | Nothing is written under the game install or config folders; output goes only into the open mod project. |

Out of scope for v1 (P2): grenades and throwables, turrets, mechanoid weapons with race and pawn kind layers, creation of a new calibre (ammo set, ammo, projectile), weapon platforms and attachments (built after 0.1.0 for the weapon platform flag, attachment links and the under barrel unit, section 14.14; attachment defs themselves are still not written). Section 4 specifies them in short so the node tree shape is fixed, but only guns, bows, melee weapons and apparel ship first.

## 2. Inputs: the environment snapshot and the CE reader

The generator is a pure function of a `DesignSpec` and an environment snapshot. Nothing is read inside it.

| Input | Source | Used for |
| --- | --- | --- |
| Target def, parsed with inheritance and patches resolved | the workspace's `DefDatabases` snapshot ([modding workspace](modding-workspace.md)) | existence checks for containers, detection of an existing CE conversion, vanilla values (V) |
| CE install facts: `ceName`, `cePackageId`, the set of `AmmoSetDef` with their projectile members, projectile defs, weapon tags, ammo categories, body part groups, research projects | a scan of the user's installed CE at runtime through `design::ce::reader` | defRef pickers and validation |
| CE conversions of vanilla items (the user's hand-tuned CE guns, tools and apparel) | `design::ce::reader::read_conversions` over the snapshot | reference pools, class statistics, twin pairs |
| Game version, the mod's `supportedVersions`, its existing `LoadFolders.xml` and About | the mod project | folder placement, version block, `loadAfter` advice |
| Position of CE relative to the mod in the load order | the manager's active list | `loadAfter` suggestion, update mode ordering |
| Class statistics and error bands | `design::ce::classes::ClassStats::derive` and the calibration cache | value flags and suggestions |

How CE data is read:

1. `ceName` and `cePackageId` come from the installed CE's `About.xml` (`Combat Extended` and `CETeam.CombatExtended` in the surveyed version; read, never assumed). The generator keeps them as two separate fields and uses the lowercase package id in gates.
2. CE's gun conversions are applied through CE's own patch operation, which the shared def engine deliberately leaves as an unknown class (D-020). `design::ce::reader` holds a typed reader of the operation's documented merge (section 7), so guns converted by CE's `ModPatches` are evaluated as the game will see them. The merge is about 100 lines of logic.
3. A def is a CE conversion when its resolved form has a CE verb class in `verbs`, a CE tool class in `tools`, or the CE ammo user comp in `comps` ([CE patch conventions](../research/ce-patch-conventions.md) implication 5). The same detector drives update mode, pools and lint.
4. Weapon tags are the union of `weaponTags` found on the user's resolved CE defs; the AI class tags are those with CE's class prefix. This was derived by a survey script in the research ([CE patch conventions](../research/ce-patch-conventions.md) open question 1) and is unverified for CE versions other than 16.7.3.0 (unverified).
5. The CE version hash (CE `About.xml` version plus a hash of its patch folders in load order) keys all derived caches.

## 3. The generator contract

```text
gun_patch, melee_patch, apparel_patch (DesignSpec, CeModel, Container) -> PatchgenResult<GeneratedPatch> (operation node trees plus mode, diagnostics and derived values; see section 14)
```

Fields have one of three sources: V (taken from the target def, parsed with inheritance resolved), U (typed or chosen by the user in the designer) and C (computed by the formulas or the calibrated baseline). The generator receives the values already decided (the designer resolved V, U and C and recorded the source of each); it never computes numbers itself, it shapes them.

A template is a JSON document with the form `{template, version, params, operations[]}`; `params` declare each placeholder with its source (`vanilla`, `user`, `computed`), type and constraints (number range, defRef target type); `operations` is a list of nodes `{name, @attrs, #text, children}` with an optional `requires` object (the package id that gates the output folder) and an `output` object (folder, file name, gate kind). The research example is `docs/research/data/ce-patch-templates/node-tree.example.json`. The template set is RimStudio-authored and versioned: `ce.ranged-gun`, `ce.bow`, `ce.melee`, `ce.apparel` for v1; `ce.grenade`, `ce.turret`, `ce.mech-weapon`, `ce.ammo` for P2. A renderer in `rimstudio-xml` walks the tree, substitutes parameters and emits XML; it is the only place XML is produced.

Properties of the function: deterministic (identical JSON for identical input), total (every invalid combination is a diagnostic, not a panic), idempotent in effect (generating twice from the same state yields the same plan and the second plan detects the first output as already converted), and side-effect free.

## 4. Per item kind

"REQ" means the item misbehaves or errors without the field; "OPT" means a sensible default exists. The failure modes are from the conventions note; the numbers named here come from [item balance math](item-balance-math.md).

### 4.1 Ranged gun

Operations in order:

1. One `MakeGunCECompatible` with `defName`, `statBases`, optional `costList`, `Properties`, `AmmoUser`, `FireModes`, `weaponTags`, optional `researchPrerequisite` and `AllowWithRunAndGun`.
2. One Replace of `tools` with one to three `ToolCE` entries (stock, barrel, muzzle).

| Field | Level | Source | Notes |
| --- | --- | --- | --- |
| `defName` | REQ | V | concrete def only; the operation cannot target an abstract parent |
| `statBases`: `Mass`, `Bulk`, `SwayFactor`, `ShotSpread`, `SightsEfficiency`, `RangedWeapon_Cooldown`, optional `WorkToMake` | REQ | V for unchanged vanilla values (mass identity, cooldown), C or U for the rest | the operation deletes vanilla accuracy stats and replaces same-named stats |
| `costList` | OPT | V or U | replaces the whole list (cleared first); never used to add one ingredient |
| `Properties`: `verbClass` = CE shoot verb, `hasStandardCommand` true, `defaultProjectile`, `warmupTime`, `range`, `recoilAmount`, `soundCast` | REQ | constant, U, V, C | without `hasStandardCommand` there is no draft command; a vanilla verb left in place ignores CE ballistics |
| `Properties`: `burstShotCount`, `ticksBetweenBurstShots`, `muzzleFlashScale`, `soundCastTail`, `targetParams`, `minRange` | OPT | V, then C | burst from the vanilla verb |
| `AmmoUser`: `ammoSet`, `magazineSize`, `reloadTime` | REQ | U (ammo set, magazine), C default for reload | an unknown ammo set or projectile is a cross-reference error at load |
| `FireModes` element | REQ in practice | U, C | may be empty; `aimedBurstShotCount`, `aiAimMode`, `aiUseBurstMode` optional |
| `weaponTags` | REQ for AI classification | U with C default | one AI class tag from the installed CE data; appended, never replaced |
| `researchPrerequisite` | OPT | V | needs a recipe-capable def |
| `tools` | REQ | C | `ToolCE` class attribute required; `armorPenetrationBlunt` from power times the median ratio of the class |

Damage and penetration are not fields of the gun: the chosen ammo set's projectile carries them. The designer shows them as lookups and as the meets-armor table.

Structure rules learned from the round trip against the user's own conversions (section 14.12):

- The conversion replaces the whole shooting verb, so every field of the vanilla verb that the designer has no field for (`minRange`, `targetParams`, `aimingChargeMote`, `ai_*` fields, `forcedMissRadius`) is written again into `Properties`; nothing the vanilla verb carries is lost.
- `FireModes` holds `aiAimMode`, `aiUseBurstMode` and `aimedBurstShotCount`. The aim mode is the user's belt fed flag first (`SuppressFire`), else the mode that most converted guns of the same class tag use in the model, else `AimedShot`; `aiUseBurstMode` follows the class the same way and otherwise is `true` for a burst weapon; `aimedBurstShotCount` is the vanilla burst of a burst weapon (the aimed mode keeps the vanilla burst while `Properties.burstShotCount` is the full automatic burst).
- A `ToolCE` takes its label, capacities, power, cooldown, extra melee damages, surprise attack and other children from the vanilla tool. A tool without a body part group or a pick weight gets the group and the weight that the converted tools with the same label share in the model, and the capacities that converted weapons always give a tool with that label (a handle that pokes), and the muzzle tool that takes a capacity away from a multi capacity tool, are not applied on their own: the patch lists them as hints and the user turns them on with a tool plan (section 14.16). A habit needs enough agreeing examples (section 14.12) and never overrules a value of the design.
- The conversion is wrapped in a `PatchOperationConditional` whose `nomatch` branch is the `MakeGun` operation, tested on the ammo user component of the def: applied twice, or on a def another patch already converted, the second application does nothing.
- A def that takes its `verbs` from a parent has no list of its own to remove the shoot verb from: the list is first started with `Inherit="False"` (guarded the same way), so the converted verb replaces the inherited ones.

### 4.2 Bow and crossbow

As the gun with: an arrow or bolt `ammoSet`, an empty `FireModes` element, the tag `CE_Bow` (read from the install, not assumed), `AllowWithRunAndGun` false, optional `AmmoGenPerMagOverride` instead of a real magazine, and a single blunt tool for `tools`. Built in `ce::patchgen::bow`; the as built description, the habits it learns and the measured fidelity are in section 14.13.

### 4.3 Melee weapon

Operations in order (plain operations, no CE-specific operation):

1. Replace `tools` with a list of `ToolCE` entries.
2. Add `statBases` entries (`Bulk`, `MeleeCounterParryBonus`).
3. Add `equippedStatOffsets` entries (`MeleeCritChance`, `MeleeParryChance`, `MeleeDodgeChance`), with the container ensured first.
4. Replace the stuff category entry used for crafting when the CE material set differs.

| Field | Level | Source |
| --- | --- | --- |
| every tool is `Class="CombatExtended.ToolCE"` with `power`, `cooldownTime`, `capacities`, `armorPenetrationBlunt`; plus `armorPenetrationSharp` for Cut, Stab, Scratch, Bite | REQ | V for label, capacities, linked body part group; C or U for power, cooldown, penetration |
| `chanceFactor`, `linkedBodyPartsGroup`, `surpriseAttack`, `extraMeleeDamages` | OPT | V |
| `Bulk`, `MeleeCounterParryBonus` | REQ | C or U |
| crit, parry, dodge offsets | REQ | U from class ranges |

Weapon tags of a melee conversion: the class tag the user chose (`weaponTagClass`, a Combat Extended tag such as the sidearm marks of the install) and the one handed mark (`oneHanded`) are added to `weaponTags` entry by entry, each only when the list does not hold it yet. Neither is invented: without an answer no tag is written. The stat and offset entries are set with a conditional (replace when the entry is there by then, add when it is not), so a second application leaves one entry.

A tool without the class loads as a vanilla tool with no penetration and is blocked by all CE armor; a tool with the class but no penetration field is legal but flagged (CEP011). The designer warns when sharp penetration is under the lowest common armor rating (blades in CE usually sit under 1 mm RHA), and when blunt penetration is far above the class p90 ratio.

### 4.4 Apparel

Operations (plain operations):

1. Stuffed apparel: Replace or Add `statBases/StuffEffectMultiplierArmor` (thickness in mm). Fixed apparel: Replace `ArmorRating_Sharp` (mm RHA), `ArmorRating_Blunt` (MPa) and, where the item has a heat rating, `ArmorRating_Heat`; the sharp and blunt replacements are emitted adjacent (the commonest pair in practice).
2. Add `Bulk` and `WornBulk` to `statBases`.
3. Replace `MaxHitPoints` and `Mass` where needed.
4. Optional `PartialArmorExt` via AddModExtension, optional `equippedStatOffsets`, optional durability comp.

| Field | Level | Source |
| --- | --- | --- |
| armor thickness or ratings in CE units | REQ | C or U; armor for stuffable apparel is asked because CE restructures it into multipliers |
| `Bulk`, `WornBulk` | REQ | C with ask flag |
| `MaxHitPoints`, `Mass` | OPT | V (mass identity) |
| partial armor | OPT | U |

Vanilla fractional ratings (0 to 2) read in CE units give almost no protection, so a CE patch whose ratings are below 1 is flagged as vanilla-scale (`design.unit-mismatch`). A missing `Bulk` is expected to leave the item without inventory volume (not verified in game) (unverified).

### 4.5 P2 kinds (shape only)

| Kind | Operations |
| --- | --- |
| Grenade or throwable | `MakeGun` with the one-use CE verb, no `AmmoUser` or `FireModes`; Add `tools` and `tradeTags`; projectile fixes (`thingClass` to the CE explosive projectile class, the `projectile` class attribute, then CE fields; setting a CE-only field before the class attribute fails to load) |
| Turret | `MakeGun` for the gun with mounted recoil; Replace the building `thingClass` with the CE turret class; Add `AimingAccuracy`, Replace `ShootingAccuracyTurret`, Mass and Bulk; Remove vanilla comps that CE replaces |
| Mechanoid weapon | gun conversion; race extension (body shape), pawn stat bases, `ToolCE` tools; pawn kind loadout extension for ammo counts |
| Ammo and projectile | a Defs file (not a patch) with `AmmoSetDef`, ammo defs and projectile defs; defNames `Ammo_<Calibre>_<Type>`, `Bullet_<Calibre>_<Type>`, `AmmoSet_<Calibre>`; reuse an installed ammo set first and create a calibre only when asked |

## 5. Emission rules

1. Operation order per item: ensure containers, then adds, then replaces, then removes; `MakeGun` before `Replace tools`.
2. One `MakeGun` per weapon. A `Replace tools` list with `defName="A" or defName="B"` may serve weapons that share a tool set.
3. The generator checks the resolved target: if a container exists it uses Replace or Add into it; if it does not, it emits the ensure-container idiom (a `Conditional` on the container whose `nomatch` Adds an empty container, then the Add of the children); it never emits Replace on a path that does not exist. Adds into optional containers (`statBases`, `equippedStatOffsets`, `weaponTags`, `comps`, `costList`) are the case where 89 percent of third-party operations had no existence check, so the check is mandatory here.
4. A target that already carries a CE conversion is never given `MakeGun` again (section 6).
5. De-duplicate by (class, normalised xpath, canonical value) before writing, within a file and across the project's CE folder (CEP021).
6. Never emit `MayRequire` on an `<Operation>` (the game ignores it there) and never use `<success>Always</success>` to hide a failure; design patches so nothing fails.
7. Operation classes come from an enumerated list; the spelling is case-sensitive and checked (CEP002 style misspellings of the class names are the generator's own bug if they occur).
8. File names follow the category: `Weapons_Ranged.xml`, `Weapons_Melee.xml`, `Apparel.xml`, `Ammo.xml`; `<!-- ====== Name ====== -->` section comments; stable defName order.
9. Static validation runs before write and maps to lint ids (section 8); after writing, a dry apply in the app's own patch engine reports any operation the game would log as failed (section 10).
10. Comments in the generated file mark it as RimStudio output with the date and the draft id, so update mode can recognise its own files (section 6).
11. Every patch is safe to apply twice. The gun conversion sits in the `nomatch` branch of a conditional on the ammo user component; a stat or offset entry the def lacks is set with a conditional (replace when present, add when absent); list entries (weapon tags) are added only when absent; a list the def does not hold is replaced when it exists by then and added with `Inherit="False"` otherwise; update mode sets a missing field the same way. A containers ensure is already idempotent.
12. The operation form is `PatchOperationMakeGunCECompatible` for guns, not a sequence of explicit operations. It is Combat Extended's own tool for the job, it knows the accuracy stats to delete, the shoot verbs to remove and the component classes to append, and its parameter names are the stable surface of the class. An explicit form would repeat that knowledge in about ten operations that break when Combat Extended changes a class. The cost is that the engine keeps the class as unknown (D-020) and the designer simulates it (section 7); a lint run (CEP023) flags a hand written conversion that nothing guards.

Probe defs: gates that probe for a CE def (a `Conditional` on a def that exists only with CE) take the probe from the user's installed CE data at runtime, not from a constant in the template.

## 6. Update mode

Trigger: the target is already a CE conversion (detector in section 2). Reasons it can be: CE's own `ModPatches` entry for the mod, the mod's own hand-written patch, or an earlier RimStudio output. Running `MakeGun` again appends a second verb, a second ammo comp and duplicate tags (CEP007), so it is never emitted.

Procedure:

1. Read the provenance of the target def from the def engine (which operation in which file converted it, three-level provenance, D-017). Classify the source: RimStudio-owned file, file of the open project, or file of another mod (including CE).
2. Compute the field diff between the resolved CE def and the design: each field that differs becomes one candidate operation.
3. Emit by source:
   - RimStudio-owned file: byte-span edit of the changed values only.
   - A file of the open project written by hand: byte-span splice limited to the changed values when the operation is recognised (same defName, same field), keeping every other byte; the diff is shown first. When the operation is not recognised, add new Replace operations in a RimStudio-owned file in the same CE folder.
   - A file of another mod (CE's `ModPatches` or a third-party patch): never touched. New Replace operations go into a RimStudio-owned file in the project's CE folder, and the designer checks that those operations run after the conversion: `loadAfter` CE (or the other mod) in About, and a file name that sorts after the original in the same mod.
4. Operation shapes: Replace on `statBases/<Stat>` (preceded by Add when the stat is absent), Replace on a field of the CE verb entry, the ammo comp or the fire modes entry, and whole-list Replace for `tools`. These xpath shapes are verified by the dry apply against the resolved def, not by a corpus of game runs (unverified in game).
5. Show everything as an editable diff. Nothing is written without the user's confirmation.

This resolves the research note's open question 5 (how to treat patches by another author in the same file) conservatively: foreign files are never rewritten, and the user's own files are changed minimally.

## 7. Handling of MakeGunCECompatible

The operation is a CE class: it parses whether or not CE is active, and in the shared def engine it is carried raw as an unknown class marked "not simulated" (D-020). RimStudio needs it in three places and handles each by one typed implementation in `design::ce::reader`:

| Need | How |
| --- | --- |
| Read CE guns as the game sees them (pools, class statistics, twins) | `read_conversions` reads the raw parameters of every `MakeGun` operation the engine retained and applies the documented merge to a copy of the target, producing a `CeModel`; the shared document is not changed |
| Dry-run a generated patch | the same merge, registered as a custom patch operation in a scratch `PatchContext` (the extension point of `rimstudio-defs`), applied to a scratch copy of the resolved defs; the shared engine keeps D-020 |
| Lint existing patches | the parameters of each operation are parsed into a typed `MakeGunSpec` for CEP007 to CEP016 |

The merge, in words (from the operation's behaviour, [CE patch conventions](../research/ce-patch-conventions.md) section 2.1):

1. Select the ThingDef by exact `defName`; an empty name or no match gives a warning and false.
2. `statBases`: create the container if absent, delete vanilla `AccuracyTouch`, `AccuracyShort`, `AccuracyMedium`, `AccuracyLong`, then replace same-named stats and append new ones.
3. `costList`: clear the existing list, then write the new one.
4. `Properties`: remove vanilla shoot verbs (`Verb_Shoot`, `Verb_ShootOneUse`, `Verb_LaunchProjectile`) from `verbs`, append one CE verb properties entry.
5. `AmmoUser` and `FireModes`: append the CE ammo user comp and the fire modes comp to `comps` (created if absent); an absent `AmmoUser` means a gun without ammo comp.
6. `weaponTags` and `weaponClasses`: append the children; never replace.
7. `researchPrerequisite`: write into the recipe maker; `texPath`: set the graphic path and force the single graphic class; platform fields convert the def to CE's platform type: the `Class` attribute of the def, the platform `thingClass` (kept when it already names a Combat Extended class), the real time drawer, and the links and parts appended to their lists (section 14.14).
8. Not idempotent: running twice appends twice.

The reference prototype of this merge exists in the research (`docs/research/data/ce-dataset/ce_ops.py`); the test vector is a CE conversion read from the user's install, with a fictional-number fixture for unit tests. Behaviour with a def whose `verbs` already contain a CE verb plus other verbs (alternate fire modes) was not tested; the source suggests a second verb entry is appended (unverified).

## 8. Lint mode

Lint checks existing patch files ("check my CE patch") with the rule ids CEP001 to CEP022 (and CEP023, CEP024, the bow rules CEP030 to CEP033 of section 14.13 and the platform rules CEP040 to CEP046 of section 14.14) as stable identifiers in the UI, in JSON reports and in diagnostic codes. A rule id is embedded in the code as `ce.cep<nnn>-<kebab-name>` (for example `ce.cep013-ammoset-unresolved`), which keeps the architecture's `<area>.<kebab-name>` form and the stable id together. CEP006 is not defined in the research script and stays unassigned. Severities are proposals, set by consequence: error when the patch silently never applies or breaks loading, warning when the effect is log noise or a missing nicety.

| id | name | check | severity | needs CE data |
| --- | --- | --- | --- | --- |
| CEP001 | findmod-looks-like-packageid | a `FindMod` entry looks like a package id; `FindMod` compares the About name, so it never matches | error | no |
| CEP002 | findmod-name-spelling | a `FindMod` entry spells Combat Extended differently from the exact name | error | no (compares with the installed name when known) |
| CEP003 | findmod-whitespace | leading or trailing whitespace; names are compared without trimming | error | no |
| CEP004 | ce-class-ungated | a CE class in a patch file that loads even when CE is absent (load-time errors without CE) | error | no |
| CEP005 | mayrequire-on-operation | `MayRequire` on a top-level operation; honoured only on list items | warning | no |
| CEP007 | makegun-repeated | the same def converted more than once in one mod | error | no |
| CEP008 | makegun-incomplete | missing section or key field (Properties, AmmoUser, FireModes, verb class, default projectile, ammo set, magazine size) | error | no |
| CEP009 | verb-class-not-ce | `verbClass` is not a CE verb class | warning | no |
| CEP010 | unknown-ce-type | a class attribute names a CE type that does not exist | error | yes (type table of the user's CE assemblies, D-018) |
| CEP011 | toolce-no-penetration | a CE tool entry without sharp and blunt penetration | warning | no |
| CEP012 | tool-without-ce-class | tools replaced with entries that lack the class attribute | warning | no |
| CEP013 | ammoset-unresolved | ammo set not defined in CE, the project or the patched mod | error | yes |
| CEP014 | projectile-unresolved | default projectile not defined in CE, vanilla, the project or the patched mod | error | yes |
| CEP015 | unknown-field | a field name in Properties, AmmoUser, FireModes or a CE tool is not a known field (typo, wrong case, obsolete) | warning | yes (type table) |
| CEP016 | unknown-ce-tag | a CE weapon tag unknown to the installed CE | warning | yes |
| CEP017 | patch-file-shape | file does not parse, root is not `Patch`, or a child is not `Operation` | error | no |
| CEP018 | folder-not-loaded | the patch file sits in a folder that `LoadFolders.xml` never loads for the selected version | error | no |
| CEP019 | ifmodactive-id-variant | an `IfModActive` id that cannot match (a `_copy` or `_steam` suffix) | error | no |
| CEP020 | case-mismatch | `Patches` folder or `.xml` extension in a different case (breaks case-sensitive file systems) | warning | no |
| CEP021 | duplicate-operation | exact duplicate operation (class, xpath, value) in one mod | warning | no |
| CEP022 | xpath-malformed | the xpath is statically malformed (root not `Defs`, unbalanced brackets, bare literal or name inside a predicate) | error | no (uses the in-house XPath parser) |
| CEP040 | under-barrel-incomplete | an under barrel unit with data that names no ammo set (unless it shares the main gun's ammo holder) or no default projectile | warning | no |
| CEP041 | under-barrel-ammoset-unresolved | the ammo set of an under barrel unit is not defined in CE, the project or the patched mod | error | yes |
| CEP042 | under-barrel-projectile-not-in-set | the default projectile of an under barrel unit is not a member of its ammo set | warning | yes |
| CEP043 | equippable-comp-missing | the replacement of an ability component does not add the plain equippable component back | warning | no |
| CEP044 | attachment-link-empty | an attachment link that names no attachment | error | no |
| CEP045 | attachment-unknown | an attachment link to an attachment def that CE, the project and the patched mod do not define (checked only when something is known to compare with) | warning | yes |
| CEP046 | platform-empty | a weapon platform with no attachment links and no default graphic parts | hint | no |

Measured prevalence in the research corpora (rule hits, from the survey): exact duplicates 182 operations in 105 CE ModPatches mods and 1,004 in 29 workshop mods; incomplete `MakeGun` 321 operations in 79 ModPatches mods; CE classes in an ungated file 22 operations in 3 owner mods and 95 in 16 workshop mods; patch files in unloaded folders 49 in 12 workshop mods; unmatchable `IfModActive` ids 86 entries in 33 workshop mods; double conversion 8 in ModPatches and 10 in workshop. These are the reasons the generator's own rules (sections 5 and 9) exist.

Without CE installed, the rules marked "needs CE data" report "not checked: CE not installed"; all others run. Lint is available three ways: the designer's "check my CE patch" action on a project, the manager's validator for a mod in the active list, and `rimstudio-cli lint` with JSON output and exit codes. The JSON report is `{ruleId, code, severity, mod, file, line, message, fix?}` per finding. Automatic fixes are offered only where the fix is mechanical and local: CEP001 and CEP002 (replace the entry with the exact name), CEP003 (trim), CEP019 (canonical id), CEP021 (remove the duplicate), CEP017 (none), the rest by explanation only. Every fix is a byte-span edit shown as a diff.

## 9. Gating, folders and versions

CE detection mechanisms and their limits ([CE patch conventions](../research/ce-patch-conventions.md) section 3):

| Mechanism | Matches | Use by the generator |
| --- | --- | --- |
| `LoadFolders.xml` with `IfModActive` | package id, lowercased on both sides, ignoring a platform suffix of the active mod's id but not of the written string | the primary gate: CE classes only in a selected folder |
| `FindMod` by name | the About name, exact, case-sensitive | fallback for vanilla-class-only files; the entry equals `ceName` exactly |
| `Conditional` probing a CE-only def | presence of a node in the combined defs | optional probe for vanilla-class-only files; the probe def comes from the installed CE |
| `MayRequire` | package id on list items and top-level def nodes, not on operations | never used on an operation |
| `loadAfter` in About | orders the mod after CE | advice, not a gate; needed when a patch edits something CE's patches also edit |

Rules:

1. CE classes (`CombatExtended.*` operations and class attributes) are written only into a folder selected by `LoadFolders.xml` with `IfModActive="ceteam.combatextended"`. A test over every generated plan asserts that no file outside that folder contains a CE class.
2. The CE folder is `Compat/CombatExtended` by default (D-105, [mod layout](mod-layout.md) section 6) with the patch files in its `Patches` folder, so definitions can follow beside them later. A project that already has another Combat Extended folder keeps it: the generator uses the folder that `LoadFolders.xml` gates on `ceteam.combatextended`, else the standard folder, else `CE` or `CombatExtended` as spelled on disk, and never moves it (update mode and the convert flow work the same on such a project). Folder names are case-sensitive on Linux and macOS.
3. `LoadFolders.xml` is created when absent (root folder plus the CE folder under the current version block) or edited by byte-span splice when present (see [items toolkit](items-toolkit.md) section 8.3). The `v1.6` block is added when the mod's supported versions lack it by copying the previous block; the game uses the exact version block, else the highest older, else `default`. The default block is kept equal to the newest block.
4. Id suffixes (`_copy`, `_steam`) found in existing entries are flagged and offered a canonical replacement, never copied.
5. A mod that must not use `LoadFolders.xml` gets a fallback file with only vanilla classes under a `FindMod` with `ceName`; `MakeGun` cannot be emitted there because CE classes in a file that loads without CE log errors at load time, and the generator tells the user so.
6. After a patch from the mod's own CE folder, the About editor is asked to add `loadAfter` CE when patches touch values that CE's own patches also set.
7. Another mod's CE patch folder can double-convert a def (the mod's own conversion plus a `ModPatches` entry); the detector sees both and update mode is used (CEP007 logic).

## 10. Validation against the user's CE data

Static validation runs before every write and is the same code as lint.

1. defRefs: every ammo set, default projectile, weapon tag, body part group, research project and parent base is resolved against the user's CE data (scanned at runtime), the target mod's defs, the project and vanilla; unresolved references block writing (`design.ref-unresolved`, CEP013 and CEP014). A projectile must be a member of the chosen ammo set's projectiles.
2. Required keys per kind (section 4) are enforced by the form: no gun without verb class, projectile, range, ammo set, magazine size and reload time; no melee tool without the CE tool class and penetration.
3. Value ranges: inputs are compared with percentiles computed at run time from the user's CE conversions and flagged, not clamped: tool penetration far above the class p90 ratio, apparel ratings below 1 on a CE patch, range jumping without warmup, Bulk out of class (`design.*`). The percentiles that the research measured (for example median gun range 44 cells, median magazine 20, median Bulk 8.35) are evidence, not shipped defaults.
4. Tags: only tags found in the installed CE data are offered; an unknown tag is CEP016.
5. Dry apply: after rendering, the plan is applied to a scratch copy of the resolved defs by the app's own patch engine (including the `MakeGun` merge of section 7). Each top-level operation must succeed; a failure is `ce.dry-run-failed` and blocks the confirmation of the write. This catches the "Replace on a path that does not exist" and "container missing" classes of failure before the game does. A test with a target def that lacks `statBases` must produce a patch that applies without error.
6. After writing, the project is resolved again and the dry apply is repeated on the written files, so the user sees the real result.

## 11. Architecture mapping

| piece | place |
| --- | --- |
| reader, class statistics, formulas, patch generator, lint | `rimstudio-design::ce::{reader, classes, formulas, patchgen, lint}` (depends on core and defs only) |
| write plan, orchestration, apply | `rimstudio-toolkit::designer` |
| render, byte-span edit | `rimstudio-xml` |
| code registry for `ce.*` | `rimstudio-design::ce::lint::codes` (the `validate` crate hosts only list and author codes) |
| commands | `designer_export_plan`, `designer_apply_plan`, `designer_convert_scan` ([items toolkit](items-toolkit.md) section 10); lint through the validator and `rimstudio-cli lint` |
| separate crate | `rimstudio-ce` is split off only when a second consumer appears (ADR 0033) |

The generator and lint have no IO, no XML dependency and no Tauri dependency; the render and edit steps happen in the toolkit through the boundary crate.

## 12. Requirements and tests

| id | requirement | acceptance |
| --- | --- | --- |
| CP-001 | The generator is a pure function of the design and an environment snapshot; identical inputs give identical JSON | proptest on determinism |
| CP-002 | `MakeGun` is emitted for every projectile weapon and plain operations for every other kind | golden test per kind |
| CP-003 | Template `ce.ranged-gun` generated from the example node tree equals its golden XML | golden test |
| CP-004 | CE classes appear only in a folder selected by `LoadFolders.xml` gated by the lowercase package id | test over every plan |
| CP-005 | `ceName` and `cePackageId` are separate fields; a `FindMod` that looks like a package id fails validation | validator test |
| CP-006 | Containers are checked on the resolved target; a missing container yields the ensure-container idiom and never a Replace on a missing path | test with a def lacking `statBases` and dry apply |
| CP-007 | An existing CE conversion switches to update mode; a second `MakeGun` is never emitted | tests for CE-owned, project-owned and RimStudio-owned sources |
| CP-008 | Foreign files are never rewritten; project files change only in the edited values | byte-identity test on untouched spans |
| CP-009 | Every defRef is validated against CE data, the mod, the project and vanilla; unresolved blocks writing | test per reference kind |
| CP-010 | Required keys per kind are enforced | one test per removed key |
| CP-011 | Value ranges are flagged from the user's data and never clamped | property test |
| CP-012 | `LoadFolders.xml` is created or edited with the gated folder and a `v1.6` block when needed; suffix ids are flagged | golden tests |
| CP-013 | The dry apply runs in the app's engine with the typed `MakeGun` merge and reports failing operations | test with a failing fixture |
| CP-014 | The typed merge matches the documented behaviour: stats replaced, vanilla accuracy deleted, cost list cleared, shoot verbs removed, comps and tags appended, not idempotent | unit tests on fictional fixtures plus an `#[ignore]` real-CE test |
| CP-015 | Lint implements CEP001 to CEP024 (CEP006 unassigned) with stable ids, JSON reports and exit codes through the CLI | fixture files per rule; each rule has a positive and a negative case |
| CP-016 | Data-dependent lint rules report "not checked" without CE | test with CE removed |
| CP-017 | Automatic fixes are mechanical byte-span edits shown as diffs | golden diff tests |
| CP-018 | With CE absent no CE data exists in the product and CE generation is unavailable | repository scan plus absent-CE fixture test |
| CP-019 | Grenade, turret, mech and ammo templates have JSON shapes and golden XML but are not offered in the UI until P2 | template tests; UI gating test |
| CP-021 | A generated patch applied twice leaves the def as applied once, for guns, melee weapons and update mode | dry apply of the patch once and twice in `ce_fidelity` and over every convertible weapon of the install in `real_ce_fidelity` |
| CP-022 | The structure of a generated conversion matches Combat Extended's own conversion of the same weapon: every element that the real conversion adds is added, and every difference is classified in the research note | `real_ce_fidelity` round trip (ignored, needs the install), counts in [the fidelity note](../research/ce-structure-fidelity-0.1.0.md) |
| CP-023 | Habits of converted tools and fire modes are learned from the model at run time, trusted only with enough agreeing examples, reported as derived values and never overrule the design | `ce_fidelity` tests with a fictional model |
| CP-020 | The generator is invoked only for an item whose CE patch toggle is on or by the Convert flow; with the toggle off no patch node is produced even when CE is installed, and no vanilla definition node ever contains a CE class | test: plans for the same draft with the toggle off and on, with CE present |
| CP-030 | The catalogue lists every ammo set of the user's Combat Extended (300 or more on the owner's install) with its ammo types, the numbers of each projectile, its caliber group, the generic or similar to relation, the converted weapons that use it and a suggested flag from the relevance ranking of the design, searchable, filterable and paged; every value is read from the loaded data | `ce_ammo` tests on a fictional install, the ignored `real_ce_ammo` prints the size on the owner's install |
| CP-031 | A custom caliber (`ce.customAmmo`) produces one definition file in the gated folder (`Compat/CombatExtended/Defs/Ammo/<prefix>_<Name>.xml`) with the ammo set, the ammo items, the projectiles and the recipes, one marked section per def; nothing is written when the Combat Extended switch is off, applying twice changes nothing and a changed spec replaces its section | `ce_ammo` plan tests, `designer_ce_ammo` apply test, CLI plan test |
| CP-032 | Suggestions for a new ammo type come from the nearest of the user's own ammunition by ammo class and energy index, carry a source and a rating per field, never overwrite a typed value and never invent a number when the install has no ammunition; a copy from an existing type takes every value | `ce_ammo` suggestion tests |
| CP-033 | The custom ammo spec is checked: required fields, usable numbers, plausibility against the user's own ammunition of the class (reported, never clamped), duplicate def names against the loaded data and the project, class and caliber consistency, ingredients that exist, XML safe strings; the generated defs pass the lint rules CEP050 to CEP057 and a dry load through the def engine with the Combat Extended type table | `ce_ammo` validation, lint and dry load tests |
| CP-034 | Rebuilding every ammo set of the install as a custom spec from its real values and generating the files reproduces the real definitions: every element of the real set, ammo item, projectile and recipe is present, extras are explained | ignored `real_ce_ammo`, counts in [the ammunition note](../research/ce-ammo-0.1.0.md) |
| CP-035 | For ammunition the user sees a list of every available ammo set (searchable, filterable by caliber and ammo class and by use in weapons, with the numbers of each type and the weapons that use it) with a Select button and a prominent Create custom ammo button; a window with every field of the projectile, the ammo item and the recipe, the set grouping, the art paths and a review of the files creates a new caliber, checked by the backend as the user types; the custom ammunition is offered only where the Combat Extended switch is on or a conversion writes into an open project | desktop tests of the browser, the field and the window on the recorded catalogue, suggestion and plan; `ammo.spec.ts` on the real bridge |
| CP-036 | A weapon with custom ammunition is not asked for an ammo set or a default projectile, in the designer and in the conversion of a project weapon; the conversion writes the ammunition file and reports the ammunition checks while other questions are open | toolkit and e2e tests (`ammo.spec.ts`) |

Fixtures use fictional numbers and fictional defNames. Install-backed tests (`#[ignore]`, `RIMSTUDIO_GAME_DIR` and a CE folder) cover reading real CE conversions through the typed merge, the CE meets-armor examples, and a lint run over a sample of the owner's mods that mention CE (7 mods in the research corpus).

## 13. Owner decisions, conflicts and open points

Owner decisions:

1. Whether update mode may splice into hand-written files of the open project (proposed: yes, minimal and shown as a diff) or must always write a RimStudio-owned override file.
2. Whether to add the `v1.6` block automatically or ask (the default folder name is decided: `Compat/CombatExtended`, D-105).
3. Whether to offer a CE auto-patcher compatible export (writing the preset-matching tags so CE's own auto-patcher handles the item at game start) as an extra option; the research recommends against shipping the preset approach and this document does not include it ([CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) open question 7).
4. Which CE versions older than 16.7 (supporting game 1.5) the generator targets; the templates were verified only against CE 16.7.3.0 and game 1.6.4871.

Conflicts and notes:

1. D-020 keeps `MakeGunCECompatible` unknown in the shared engine, while the designer needs its effect. The resolution here (typed reader in `design::ce`, registered as a custom operation only in scratch dry-run contexts) respects D-020 and the design-depends-on-defs layer rule, but it means the manager's reference view shows these operations as not simulated while the designer sees them applied.
2. The conventions note and the architecture both name lint ids CEP001 to CEP022; the research script defines no CEP006, so there are 21 rules.
3. The research templates are XML in the docs folder; they remain documentation and golden references, while the product templates are JSON (R10).
4. Severity levels in section 8 are proposals; the research reports only counts.

Open points: where CE's default ammo set assignments and tag to loadout mappings are stored in the user's CE data and whether every AI class tag can be listed without parsing C# (research open question 1); the default ammo count of a pawn kind with a CE gun but no loadout extension (needs an in-game test); behaviour of `MakeGun` on a def with several verbs; attachment defs and weapon platforms against a real conversion (the owner's install has none); whether CE ships an apparel auto-patcher whose output the app should mirror ([CE patch conventions](../research/ce-patch-conventions.md) open question 6).

## 14. As built in the 0.1.0 backend

The generator, the reader, the lint and the conversion of existing weapons are implemented in `rimstudio-design::ce` (pure, no file access, no XML dependency) and orchestrated by `rimstudio-toolkit::designer`. Apparel conversion is not built (`apparel_patch` returns `design.deferred`), and launchers and grenades are listed as unsupported; bows and crossbows are converted in their own style (section 14.13). Owner rules hold: the generator runs only when the item's `ce` block is present or in the Convert flow (CP-020), never mixes a CE class into a vanilla file, and every CE class is written under the folder gated by `LoadFolders.xml` (IT-052 is enforced by `gate_violations` and the designer error `designer.ce-outside-gate`).

### 14.1 The reader

1. `read_conversions(&DefDatabases)` and `read_conversions_with(&DefDatabases, &CeReadOptions)` build a `CeModel { absent, names, classes, ammo_sets, weapon_tags, ai_class_tags, gun_presets, apparel_presets, stat_rules, guns, melee, probe_def, diagnostics }`. The plain form cannot see the load order or the vanilla twins; the `_with` form takes an optional mod order (to find the CE mod by package id and to give the exact `ceName`) and an optional second `DefDatabases` from a vanilla load, and twins exist only when that second load is supplied. A model without CE data is `absent` with a reason (`ce.absent`, an info) and the toggle is disabled (section 12).
2. The shared def engine drops defs of unknown types and the vanilla type table lacks the CE assembly classes (spike S-07), so the caller builds the type table with `with_ce_types(&TypeTable, &CeClassNames)` and registers `custom_registry(&CeClassNames, settings)` in the load input so that the CE settings conditional and `MakeGunCECompatible` are applied during load (the shared engine itself keeps the class unknown, D-020). The settings map comes from the user's CE settings and the caller supplies it; a missing setting makes that conditional fail with `defs.patch-setting-missing`. `MakeGunOp` rewrites the children of the target def in place; the def keeps its position, mod and file, the rewritten children carry the operation as their origin and the operation appears in `patched_by`. Ammo defs of CE (the ammo `ThingDef` subclass) and other assembly classes still report unknown types until a full CE type table exists.
3. Weapon platform fields of `MakeGunCECompatible` (`isWeaponPlatform`, `attachmentLinks`, `defaultGraphicParts`) are parsed into `MakeGunSpec` and applied by the merge (`ce::reader::platform::merge_platform`, which also sets the `Class` attribute of the def); `MakeGunSpec.unsupported` is empty at present and stays as the hook for a parameter that is parsed but not merged. Diagnostics of the reader: `ce.absent`, `ce.twin-missing`, `ce.ammo-set-unresolved`, `ce.makegun-missing-def`, `reader.armor-fallback`.

### 14.2 The generator

1. `gun_patch` and `melee_patch(&DesignSpec, &CeModel, &Container) -> PatchgenResult<GeneratedPatch>` return a struct, not a bare node: `GeneratedPatch { mode: PatchMode::{New, Update, Off}, category, def_name, operations, diagnostics, derived, source }` (invalid input gives diagnostics and no operations). A gun patch is the MakeGun operation plus a tools Replace; a melee patch is stat and offset entries (the ensure idiom, Replace for existing entries) followed by Replace or Add of the tools.
2. `Container` is the state of the target def's child containers (name to child names) plus the existing conversion (`ExistingConversion { markers, block, source, verb_fields, ammo_fields, weapon_tags }` with `ConversionSource::{RimStudio, Project, Foreign, Unknown}`). The operations act on the raw (unresolved) def, so the container must be built from the raw node (`Container::from_node`); `from_def` falls back to the resolved node and can only over report containers, which the dry run then catches. A missing list container is never replaced: `tools` is added to the def with `Inherit="False"` when the def has none of its own, so it replaces an inherited list instead of merging with it.
3. `export_ce_plan(&DesignSpec, &CeModel, &ProjectLayout)` and `export_ce_plan_with(.., &CeProjectState { load_folders, supported_versions, game_version, container })` return the plan: the CE patch file (`FileKind::CePatch`, in the CE folder, with the category name and `_Update` in update mode) plus `LoadFolders.xml` (`FileKind::LoadFolders`); the plan is empty when the spec has no `ce` block. `load_folders_plan(..)` returns `Create`, `UpdateRegion` or `Unchanged`; an `UpdateRegion` carries the complete new tree and no edits, and the toolkit computes the byte spans (`ensure_block` and `add_entry` of `rimstudio-xml`). The toolkit must supply the raw project nodes, the parsed `LoadFolders.xml`, the About `supportedVersions` and the `ConversionSource` of an existing conversion from def provenance.
4. Values not in the block (mass, range, warmup, cooldown, sights, recoil) come from the estimator of section 14.9 when its rating allows writing the number, and otherwise fall back to the vanilla design (identity) with the rejected rating recorded; mass is always written explicitly when known; costs, research and work to make stay in the vanilla definition and are not repeated. Tool power and cooldown of melee weapons are scaled by the predicted class change only when it differs from 1 by more than 1 percent; gun bash tools keep their vanilla power and cooldown. The AI aim mode is `SuppressFire` when `belt_fed` and `AimedShot` otherwise; `one_handed` adds the first installed CE tag whose name contains `onehanded` (a `ce.tag-not-found` hint when none exists). These are field values and a search over the model's tags, not tables.
6. Structure fidelity (section 14.12): `ce::patchgen::conventions` holds the habits learned from the model (`label_habit`, `fire_mode_habit`, `companion_tags`, `tool_habits`, `apply_tool_habits`; the optional additions of section 14.16 live in `ce::patchgen::extras`); `GeneratedPatch::gun_conversion(&CeClassNames)` finds the conversion operation inside its guard; `ops::{unless_present, branch}` build the guard and the replace-or-add conditional, and `DefOps::{set_entries, add_unique_li, replace_list}` use them. The reader keeps the tools, `aiUseBurstMode` and `aimedBurstShotCount` of converted guns (`CeGun.tools`, `use_burst_mode`, `aimed_burst`) and the body part group of every converted tool (`CeToolRow.linked_body_parts_group`); `ce_block_from_def` reads the class tag of a melee conversion as its first Combat Extended tag that does not say one handed. Flow D reads the weapon with `spec_from_def_own` when the raw def is at hand, so verb and tool extras are carried.
5. The file comment of a generated patch says it was generated by RimStudio for the draft, without a date, because the generator has no clock; the plan has one section header per file. `find_mod_fallback` exists as a helper but is not wired into the plan (the CE class names inside its values would break IT-052).

### 14.3 Update mode

When `Container::existing` is set the generator emits no `MakeGun`; it compares only the fields the block names, skips the required field checks and produces Replace operations. As built the operations always go into an override file `Weapons_<Kind>_Update.xml` (RimStudio owned, merged by section like any generated file), also for a conversion that lives in a hand written project file; splicing by value into existing files is a limit of the design crate (the generator has no file text), and `GeneratedPatch.source` tells the toolkit where the conversion lives. Foreign files are never rewritten. `ce.update-load-after` and `ce.about-suggestion` hints suggest the About `loadAfter` entry; `ce.update-nothing` says that nothing differs.

### 14.4 Convert and scan

`scan(raw project nodes, &DefDatabases, &CeModel) -> Vec<ConvertCandidate { def, label, kind, status, reason, family }>` takes the raw nodes as well because `TargetNotFound` (abstract, or missing from the resolved set) cannot be told from resolved records alone; status is `NotConverted`, `AlreadyCe`, `UnsupportedKind` or `TargetNotFound`. `convert(&ConvertCandidate, &ConvertAnswers, &ConvertEnv) -> ConvertOutcome { plan, asks, spec, derived, update }` derives the CE block (`derive_ce_block`) from the vanilla def through the predictors, lists what it cannot derive as asks, and never changes the vanilla definition. The numbers come from the estimator of section 14.9: a number whose measured accuracy is too low is asked, with the reason and the rejected estimate as a reference, and is never written. A bow or crossbow takes the bow derivation of section 14.13 (an arrow or bolt set, the bow tag, estimates from the converted bows alone); only 18 of 67 converted guns find a vanilla twin, and gun bash penetration is estimated from the melee conversions and is rated unreliable on the owner's install, so it is asked for each tool.

### 14.5 Dry apply

`dry_apply(defs, patch_files, &CeModel) -> DryRun { diagnostics, defs, applied, failed }` runs the generated patch in the app's engine on the scratch defs (the new vanilla definitions of a design or the project's raw definitions for a conversion, not the whole reference snapshot) with `MakeGunCeSimulation` registered for the gun conversion; it reports top level operation failures (`ce.dry-run-failed`) plus the simulation warnings (`ce.simulation-unsupported`), not the engine's own diagnostics. `DryRun::is_clean()` is the success test of IT-056. MakeGun on a def that inherits its verbs from an abstract parent leaves the inherited vanilla verb in place because the merge only sees the def's own verbs; this is not detected.

### 14.6 Lint and the code list

`ce::lint::run(&[Node], &CeModel, &LintContext) -> Vec<Diagnostic>` and `run_files(&[LintFile], ..)` implement CEP001 to CEP024 (CEP006 is not assigned), the bow rules CEP030 to CEP033 and the platform rules CEP040 to CEP046 (section 14.14). CEP023 (warning) flags a gun conversion that no conditional encloses, because a second application appends a second verb, ammo component and set of tags; CEP024 (hint) flags a burst weapon whose fire modes name no `aimedBurstShotCount`. Besides the rules, the generator reports `ce.companion-tags` (a hint naming the tags that converted weapons of the same class carry and the design does not: they are the user's choice and are never written) and `ce.derived-value` (info) for every number or name taken from a habit of the converted weapons. `LintContext { paths, load_folders, game_version, known_defs, ce_types, known_fields }`: CEP013 and CEP014 use the model plus `known_defs`; CEP010 and CEP015 are checked only when `ce_types` or `known_fields` is supplied (reader work, spike S-07) and otherwise report `ce.not-checked`; CEP022 covers root, brackets, quotes and bare quoted literals but not bare names inside a predicate, because `[defName]` is a legal existence test. CEP007 to CEP016 use the typed `MakeGunSpec::parse` and `merge_into_def` of the reader. Lint only reports: automatic fixes as byte span edits (CP-017) are not implemented. The code list: `ce.cep001-findmod-looks-like-packageid`, `ce.cep002-findmod-name-spelling`, `ce.cep003-findmod-whitespace`, `ce.cep004-ce-class-ungated`, `ce.cep005-mayrequire-on-operation`, `ce.cep007-makegun-repeated`, `ce.cep008-makegun-incomplete`, `ce.cep009-verb-class-not-ce`, `ce.cep010-unknown-ce-type`, `ce.cep011-toolce-no-penetration`, `ce.cep012-tool-without-ce-class`, `ce.cep013-ammoset-unresolved`, `ce.cep014-projectile-unresolved`, `ce.cep015-unknown-field`, `ce.cep016-unknown-ce-tag`, `ce.cep017-patch-file-shape`, `ce.cep018-folder-not-loaded`, `ce.cep019-ifmodactive-id-variant`, `ce.cep020-case-mismatch`, `ce.cep021-duplicate-operation`, `ce.cep022-xpath-malformed`; and `ce.not-checked` (info), `ce.update-load-after` (hint), `ce.about-suggestion` (hint), `ce.update-nothing` (info), `ce.tag-not-found` (hint), `ce.already-converted`, `ce.dry-run-failed`, `ce.absent`, `ce.twin-missing`, `ce.ammo-set-unresolved`, `ce.makegun-missing-def`, `ce.simulation-unsupported`. The lint registry is `ce::lint::codes::REGISTRY`; the design codes are in `validation::codes::REGISTRY`.

### 14.7 Toolkit orchestration

The toolkit (items toolkit section 15) builds the project session (reference set plus the project as last pack, CE session preferred), keeps the CE conversion state, runs `scan`, `convert` and the lint with the model and the project's known def names, writes through the guarded writer and verifies with `dry_apply`. A conversion with open questions is reported as the error diagnostic `designer.convert-needs-answer` (field pointer in the `field` arg) of an empty plan; with the CE block on but no CE data the plan stays vanilla with the warning `designer.ce-unavailable`.

Limits: the patch operations rely on XPath attribute predicates (`li[@Class=...]`, `li[label=...]`, `li[.=...]`) that are verified only by the in engine dry run on fictional defs, not against game runs; the CE settings read by the conditionals are supplied by the caller; the Combat Extended class strings exist only inside `rimstudio-design::ce` (`xtask check-source` rule `source.ce-class`).

### 14.8 Integration findings (2026-10-05)

A run on real mods found three things that are now fixed and one that is open.

- Fixed: hand written conversions normally start a sequence with Combat Extended's own mod gate (`CombatExtended.PatchOperationFindMod`, which succeeds when an active mod has exactly the display name in `modName`). The engine kept it as an unknown class, so such a mod looked unconverted and the scan offered a second conversion, which the non idempotent gun merge would apply twice. `ce::reader::FindModOp` (class constant `FIND_MOD_OP`) now implements the gate; it is part of `custom_registry` and of the dry run registry.
- Fixed: inside one top level operation (a sequence) a def replaced by an earlier step could not be found by a later step, because the `defName` index was only refreshed at the end of the top level operation. A sequence now flushes the index notes after every step (`PatchCtx::flush_index_notes`). A mod that replaces a whole def twice in one sequence aborted its sequence in RimStudio but not in the game.
- Added: every number of a generated conversion that the block did not give is reported as an info diagnostic `ce.derived-value` (field, value, predictor and the number of converted weapons behind it). A predicted range or mass outside 0.6 to 2.5 times the vanilla number carries the warning `ce.derived-far-from-vanilla` (the window is an implementation choice, not a game value). The conversion flow also reports the block values it derived as `designer.convert-derived`.
- Closed in the quality round: the class median that ignored the vanilla number is replaced by the estimator of section 14.9 (a bolt action rifle now gets a range within a few percent of the owner's own number), and magazine size, spread and the tool penetration are asked instead of guessed. Sights follow the pool. The tool details that no number can supply (`chanceFactor`, `linkedBodyPartsGroup`) are still not derived. See `docs/status/0.1.0-backend.md` for the comparison against a hand written patch and [the evaluation note](../research/ce-conversion-eval-0.1.0.md) for the measurements.

### 14.9 The conversion estimator and reliability gating

`rimstudio-design::ce::classes::estimate` replaces the class median predictor of `predict_conversion` (kept as the pool based predecessor, used by the evaluation harness as the "before"). The measurements are in [Combat Extended conversion quality, measured](../research/ce-conversion-eval-0.1.0.md).

1. **Examples.** `ExampleSet::from_model(&CeModel, ItemKind)` builds one `Example` per converted weapon without an exclusion reason: tier, vanilla weapon tags (those of the twin, `CeGun.twin_tags` and `CeMelee.twin_tags`, so that class assignment never sees tags the conversion added; without a twin the converted tags minus the CE prefix), the tag class, the converted numbers and the twin's numbers. A `Profile` is what a designer knows before converting: tier, tags, the answered tag class (`ce.weaponTagClass`), the vanilla numbers.
2. **Class by similarity.** The distance of a design to a conversion adds: the rarity weighted tag dissimilarity, a penalty when both sides name a different tag class, a penalty for a different verb shape (burst or single), a penalty per tier step and a small penalty per step of the strength proxy tercile (a tie break only). The class of a stat is its 4 nearest conversions that carry it. A stat that fewer than 3 conversions carry is withheld (`ConversionPrediction.withheld` holds the reason); a class below the minimum is never used.
3. **Forms.** Identity (stats that the vanilla number measures in the same unit, such as mass), ratio (the vanilla number times the class's median converted over vanilla ratio, with the log ratio shrunk toward the ratio of all conversions with strength 2) and class median. The form is chosen by leave one out error with a 20 percent switch margin (order identity, ratio, median). The estimator does not predict the numbers that the chosen ammo supplies (`damage`, `speed`, `ap_sharp`, `ap_blunt`, `pellets`, `ticks_between`; `EstimateOptions::skip`).
4. **Reliability.** `StatPrediction.reliability` is `Reliable` (median leave one out error below 15 percent), `Rough` (below 35 percent), `Unreliable` (35 percent or more, or an 80th percentile error of 80 percent or more) or `Unmeasured` (fewer than 5 measurable conversions). The band comes from the same residuals. Every constant is in `EstimateOptions` and is generic: class size, minimum class, shrinkage, distance weights, switch margin and thresholds.
5. **Gating.** Only `Reliable` and `Rough` numbers are written. In `derive_ce_block` an unusable estimate becomes an `AskItem` with `reason` (the accuracy in plain words) and `suggestion` (the rejected estimate, for reference; it is written only if the user answers with it, through `overrides`); an optional stat (sights, recoil, counter parry bonus) is simply left out. In `resolve_gun` an unusable range, warmup, mass, cooldown, sights or recoil falls back to the vanilla number (identity) and the `DerivedValue` carries `rating` and `error`; the `ce.derived-value` info diagnostic says "rated unreliable ... and is not used". A tool penetration without a usable ratio is asked per tool (`/ce/toolPenetration/<tool>/blunt`), which includes the gun bash tools on an install where the melee conversions cannot predict a ratio; a plan with a gun tool and neither a block entry nor a usable estimate is the error `design.required-missing` naming the tool. Usable numbers are listed as `ce.derived-value` with their predictor, rating and typical error.
6. **Reports.** `DerivedValue { rating: Option<Reliability>, error: Option<f64> }` and `AskItem { reason, suggestion }` are additive and serialised only when set; the toolkit DTO for an ask does not carry them yet.

Limits: the ratings describe the pool, not the next author's style (a hand written conversion with a very different spread or magazine is outside any estimate), launchers stay out of scope (bows are estimated from the converted bows alone, section 14.13), and with few converted weapons every stat is `Unmeasured` and therefore asked.

### 14.10 Suggestions for a designed weapon

`rimstudio-design::ce::suggest` reuses the estimator of 14.9 for a weapon that the user designs (flow A) and does not convert. `suggest_block(&DesignSpec, &CeModel) -> CeSuggestion` takes the vanilla design as the twin, calls `derive_ce_block` (so the numbers equal those of the convert flow) and reports per field the suggested value, its source (identity, predictor with the number of weapons, vanilla, answered), the band of the estimator, the rating of the stat, whether the number is derived or asked and, for an ask, the rejected estimate and the reason. It also ranks the candidates for the ammo set and the weapon tag class by the conversions of the nearest weapons, and lists the required fields that are still missing. `accept_suggestions(&DesignSpec, &CeSuggestion, &Accept) -> AcceptOutcome` fills the empty fields (never a typed, answered or anchor value) and returns one `ce.derived-value` diagnostic per field; it returns a spec without a CE block unchanged. Neither function turns the toggle on, and the ammo set and the weapon tag class are never chosen for the user. The toolkit command is `designer_ce_suggest`, and the plan option `acceptSuggestions` is described in [Items toolkit](items-toolkit.md) section 15.9.

### 14.11 Ask reasons, tagless melee weapons and answer groups

- **Asks.** `AskItem` (and its DTO) carries `reason` and `suggestion` for a number that is asked although an estimate exists (section 14.9): the text says what the estimate was and how it was rated, and the rejected number is shown for reference only. `convert scan` and the plan text show both.
- **Melee scan.** A melee weapon is listed when it has `tools` and satisfies `is_weapon_def` (weapon tags, or primary equipment under a weapon category), the same rule the reference pools use, so a modded blade without weapon tags is converted like any other. Items that only carry tools are not weapons.
- **Family.** `family_key(node, kind)` gives the weapon family: `ranged/<first weapon tag or untagged>/<default projectile>` for a gun and `melee/<first weapon tag or untagged>` for a blade. The scan stores it in `ConvertCandidate.family` (empty for an abstract base or an unresolved def).
- **Groups.** One answer set can cover a family (or an explicit list of def names) so that a mod of many similar guns needs one answer set; the merge rules and the files are in [Items toolkit](items-toolkit.md) section 15.11. Group answers follow the same rules as any answer: they win over derived values, and an estimate that is unreliable stays an ask until a group, the weapon's own answers or `--set` gives the number.

### 14.12 Structure fidelity against the user's Combat Extended

The ignored test `tests/real_ce_fidelity.rs` of `rimstudio-design` is the round trip harness (needs `RIMSTUDIO_GAME_DIR` and `RIMSTUDIO_CE_DIR`). For every weapon that Combat Extended converts and that has a vanilla twin and a supported kind, it reads the real conversion as the Combat Extended block of the design of the twin (typed values, so only the structure is tested), generates the patch, loads the vanilla defs with the patch as an extra mod in the app's own engine (with the gun conversion registered) and compares the resolved def with the one of the Combat Extended load, path by path against the vanilla def. It reports missing elements, elements Combat Extended removes and the patch keeps, removed extras, extras and value differences grouped by path with counts, whether every class name the patches use appears in Combat Extended's own data, what the lint says, and whether applying the patch twice differs from once. With `RIMSTUDIO_FIDELITY_STRICT` set the test fails on any missing or not removed path that has no explanation in the harness (the classes of the note) and on any weapon where applying twice differs from once; `RIMSTUDIO_FIDELITY_DUMP=<defName,...>` prints the raw def, the operations and the three resolved defs of a weapon, and `RIMSTUDIO_FIDELITY_TABLE` and `RIMSTUDIO_FIDELITY_HABITS` print the evidence tables.

Result on the owner's install at the time of that task (36 weapons, 17 guns and 19 melee weapons, after the weapons that inherit a conversion from their parent, turrets, launchers and bows are set aside): missing elements 319 before and 139 after the changes of this section, applying twice differed for 31 of 31 weapons before and for none after, and the generated patches use only classes of Combat Extended's own data and lint clean. A habit is trusted when at least 2 converted tools share the label and at least 60 percent of them agree (capacities need 3 examples and 75 percent, because they change what the tool does; a fire mode habit needs 2 guns of the class). The classified remainder is in [the fidelity note](../research/ce-structure-fidelity-0.1.0.md). In short:

- **Left to the vanilla design on purpose (D-085):** costs, work to make, research, stuff categories, graphic draw size and the `GunDrawExtension` casing and offset fields. Combat Extended's own conversions change them in the same operation; the designer writes the weapon in vanilla terms and the patch carries what the conversion needs to function.
- **Habits, not rules:** tool groups, pick weights, capacities and fire modes follow what the user's converted weapons do and are reported as derived values; the weapon tags of a class that are not the class tag (loadout and sidearm marks) are only suggested (section 14.16).
- **Not expressible, listed as unsupported:** turrets, launchers and one use weapons, a vanilla tag that a conversion removes, the verb field `targetParams` that Combat Extended adds, and the pairing of a conversion with other mods' patches. A restructured tool list, `recoilPattern`, `reloadOneAtATime`, `AmmoGenPerMagOverride`, companion tags and art nodes are no longer in this list: they are optional additions of section 14.16.
- **Child defs:** a weapon whose parent is converted inherits the conversion; the generator converts the parent and does not convert the child again. A concrete def that only inherits its verbs starts its own list with `Inherit="False"`. Inherited vanilla accuracy stats cannot be deleted by the operation and remain.

### 14.13 Bows and crossbows

Combat Extended converts a bow in a style of its own, and `ce::patchgen::bow` generates it (decision D-140, which supersedes D-097). A weapon is a bow when its shape says so, never by its def name: a weapon tag or weapon class that names a bow, a shooting verb whose projectile is an arrow or a bolt (`Bolt` as a whole word, so a bolt action rifle is a gun), the role `bow` of a design, or `ce.bow` set to true in the block (`ce.bow` false keeps a bow shaped weapon on the gun conversion). The test is `ce::reader::bows::is_bow_shape`; the reader marks converted bows with `CeGun.bow`, the scan lists vanilla bows as `NotConverted` with their asks, and `gun_patch` calls `bow_patch` for a bow spec, so every caller (export, convert, update) takes the bow path.

The patch is the gun operation with these differences, all observed on the converted bows of the user's install:

- **Properties:** the verb class, the standard command flag, the default projectile, warmup, range, the shot sound and the carried vanilla verb fields; no recoil and no burst unless the block gives a recoil.
- **AmmoUser:** the arrow or bolt set, and `AmmoGenPerMagOverride` (the number of arrows a pawn spawns with per magazine, which replaces a real magazine). A magazine size and a reload time are written only when the block gives them: Combat Extended has no draw time and no draw strength, so the pull of a bow lives in the range, the warmup and the projectile of the set.
- **FireModes:** an empty element (the component exists, no mode is set).
- **Tags and flags:** the bow tag, plus the one handed mark when asked; `AllowWithRunAndGun` false unless `ce.allowWithRunAndGun` is true.
- **Mass:** a bow keeps the mass of the vanilla design. `Mass` is written only when `ce.mass` differs from it.
- **Tools:** the gun bash conversion of a gun (replace, with the penetration asked or taken from the block).
- The conversion is guarded by the ammo component like a gun, so applying it twice changes nothing, and a bow that already carries a conversion goes through update mode (the same operations as a gun, plus `AmmoGenPerMagOverride` and `Mass`).

New optional fields of the Combat Extended block: `bow`, `ammoGenPerMag`, `allowWithRunAndGun`, `mass`. Validation does not require a magazine size or a reload time for a bow; it still requires the ammo set, the default projectile, bulk, sway and spread.

**Numbers and choices (convert flow and suggestions).** The estimates for a bow come from the converted bows of the library only (`bow_prediction`: the estimator of section 14.9 over the bows, not the guns), with the same rating and the same gating: a number rated unreliable or unmeasured is asked, with the reason and the rejected estimate. On the owner's install the library holds 4 converted bows (the unique variant is excluded as a duplicate), fewer than the 5 conversions that a rating needs, so bulk, sway and spread are asked and the plan says why; the same is true of any install with few bows. The arrow set is asked among the sets that converted bows use first, then the sets whose name or ammo reads as an arrow or a bolt. The default projectile is the first member of the set. The bow tag and the spawn count are taken from the converted bows only when at least 2 of them carry the same one and at least 60 percent agree (the thresholds of section 14.12), and are asked or left out otherwise; a derived spawn count is reported as a derived value. A bow is never asked for a magazine, a reload time, recoil, one handed or belt fed.

**Lint.** The bow rules run on a gun conversion whose tags or projectile say bow, and CEP008 does not ask a bow for a magazine size. CEP030 (warning): the ammo set is not used by any converted bow and holds no arrows or bolts. CEP031 (hint): the fire modes are not empty. CEP032 (hint): `AllowWithRunAndGun` is not false. CEP033 (hint): neither `AmmoGenPerMagOverride` nor a magazine size is set, so pawns spawn with one arrow per magazine. Codes: `ce.cep030-bow-ammo-not-arrows`, `ce.cep031-bow-fire-modes`, `ce.cep032-bow-run-and-gun`, `ce.cep033-bow-spawn-count`.

**Fidelity.** The harness of section 14.12 compares the bows too (aggregates in [the bow note](../research/ce-bows-0.1.0.md)): the 3 bows that have a vanilla twin and no converted parent show no missing or not removed element that is not classified (graphic draw size and the draw offset extension stay in the vanilla design, the tool list is restructured by Combat Extended), applying the patch twice changes none of them, and the variant that inherits a bow's conversion resolves the same way except for the components that it resets itself, which Combat Extended patches in a separate file. The convert flow on the three real bows asks the arrow set, bulk, sway, spread and the tool penetration, and produces a plan without errors once they are answered. Limits: with fewer than 5 converted bows no bow number is estimated; a hand converted crossbow that keeps aim modes or a magazine (the nerve spiker of Anomaly) is not a bow shape in its vanilla form and is converted as a gun; ranges, warmups and the tool power of the real conversions differ from the vanilla values and are not predicted (they are values, not structure).

### 14.14 Weapon platforms and under barrel units

Combat Extended has two features that the designer's one gun cannot say: a **weapon platform** (the def becomes a platform type that accepts attachments) and an **under barrel unit** (a second gun with its own ammo, verb and fire modes that the wielder switches to). `ce::patchgen::platform` generates both (decision D-142); the evidence is in [the platform note](../research/ce-platforms-0.1.0.md). Both are choices of the optional Combat Extended block and never appear in the vanilla definition (D-085): a spec without the toggle writes nothing.

New optional fields of the Combat Extended block, all absent by default:

- `isWeaponPlatform` (flag), `attachmentLinks` (each: `attachment`, `drawScale`, `drawOffset`, `statOffsets`, `statMultipliers`, `statReplacers`) and `defaultGraphicParts` (each: `partGraphic`, `outlineGraphic`, `slotTags`). A platform is asked for by the flag, by links or by parts.
- `underBarrel`: `standardLabel`, `underBarrelLabel`, `oneAmmoHolder`, `requiresReload`, `ammoSet`, `defaultProjectile`, `magazineSize`, `reloadTime`, `recoilAmount`, `warmupTime`, `range`, `minRange`, `burstShotCount`, `ticksBetweenBurstShots`, `ammoConsumedPerShot`, `avoidFriendlyFireRadius`, `soundCast`, `muzzleFlashScale`, `verbExtra`, `propsExtra`, `extra`, `fireModes` (`aiUseBurstMode`, `aiAimMode`, `aimedBurstShotCount`, `noSingleShot`) and `replacesComp`. A unit with nothing set is the bare slot form, written as the component without data.

What the generator writes:

- **Platform parameters** ride in the gun conversion operation of a new conversion (`isWeaponPlatform`, then `attachmentLinks` and `defaultGraphicParts`). The operation applies each of its parts only when given, so for a def that already carries a conversion the update writes a platform only operation guarded by the platform type of the def (`Defs/ThingDef[defName=".."][@Class=".."]`), and an add per missing attachment link, guarded by the link's attachment. The lint does not report a platform only operation as an incomplete gun conversion (CEP008) or a repeated one (CEP007).
- **The unit** is one operation under a conditional on the component, so a second application does nothing. A target whose own `comps` list a vanilla ability component (the class family `CompProperties_EquippableAbility*`, which stands in for the plain equippable component) gets that component replaced by the unit and the plain `CompEquippable` entry added back, unless it already lists one. Any other target gets the unit added to `comps`. `replacesComp` names the class to replace when the block knows it. The conversion operation still runs first and guards itself by the ammo component.
- **Fire modes of a unit that gives none** are taken from the units of the user's converted weapons when at least 2 agree and at least 60 percent share the value (`ce::patchgen::platform::derive::unit_fire_habit`), reported as `ce.derived-value`. The unit's numbers (magazine, reload, range, warmup, recoil) are never predicted: the library holds too few units to rate an estimate, so they are asked.
- **Validation** (`platform::validate`): a unit with data needs a default projectile, a range and, unless it shares the main gun's ammo holder, an ammo set, a magazine size and a reload time; the ammo set must exist and the projectile must be one of its members; an attachment link needs an attachment. Errors give no operations.
- **Reader and model:** `ce_block_from_def` reads the platform and the unit of a converted def into the block; `CeModel.platform` (`PlatformLibrary`) lists the install's platforms, units and attachment defs; `CeClassNames` gains `underBarrelComp`, `weaponPlatformDef`, `weaponPlatformThing` and `attachmentDef`; `Container.compClasses` lists the target's own components. `merge_into_def` applies the platform parameters (the dry run and the load that registers the operation resolve platform weapons).
- **Scan and convert:** a vanilla weapon with an ability component is `NotConverted` with a reason that names the unit, and `convert` asks `/ce/underBarrel/ammoSet`, `/magazineSize`, `/reloadTime` and `/range` (the ask carries a reference: the median of the library's units when at least 2 exist, else the range of the vanilla ability def; a reference is never written). The user answers through `ConvertAnswers.overrides` (`underBarrel`, `isWeaponPlatform`, `attachmentLinks`, `defaultGraphicParts`); `ConvertAnswers.skipUnderBarrel` converts the weapon without its own unit. A weapon whose verb names no projectile (a beam or a spray) is listed as `UnsupportedKind` with that reason instead of being read as a melee weapon.
- **Lint:** CEP040 to CEP046 (section 8).

Result on the owner's install (aggregates in the note): the one weapon with an under barrel unit and a vanilla twin had 32 findings on the unit and its components before (22 unit fields, 7 fields of the removed ability component, 3 component entries) and has 0 after; applying the patch twice changes nothing; overall missing elements went from 142 to 118 and not removed from 51 to 43. The install has no weapon platform, so the platform half is covered by fictional tests and goldens only. Not done: attachment defs (a separate def type), hand written explicit conversions of the unit, predicted unit numbers and the TypeScript bindings of the new block fields.

### 14.15 Lint of existing files and the scan facts

`designer_lint_files` (items toolkit section 15.14) feeds the lint of section 8 with the patch files of a project as they are on disk, hand written or generated, so the Lint tab of the Patches page and `lint ce` no longer have to re-plan the conversions the generator can read. The rules, the codes and the order of the findings are those of `ce::lint::run_files`; the call adds the file, the operation index, the xpath and a plain explanation of the rule to each finding, and reports the rules that did not run (`notChecked`). The scan of `designer_convert_scan` carries the weapon tags, the weapon classes and a few numbers per candidate (items toolkit section 15.13) for the Patches table.

### 14.16 Optional additions beyond the numbers

Combat Extended's own conversions differ from the plain conversion of a design in ways the design has no field for. Each of them is a field of the optional Combat Extended block, written only when the block holds it (D-085, D-146); the generator never fills one in on its own. The evidence is in [the remaining differences note](../research/ce-remaining-0.1.0.md).

| Block field | What the patch writes | Where it comes from |
| --- | --- | --- |
| `reloadOneAtATime` | `reloadOneAtATime` true in the ammo comp (only `true` is written) | typed, read from a conversion, or the suggestion `reload-one-at-a-time` |
| `ammoGenPerMag` | `AmmoGenPerMagOverride` in the ammo comp, for a gun as for a bow | typed or read from a conversion |
| `recoilPattern` | `recoilPattern` in the converted verb | typed, read from a conversion, or the suggestion `recoil-pattern` |
| `toolPlan` | the converted tools are exactly the entries of the plan, in order | typed, or the suggestion `tool-plan` |
| `keepToolFields` | the tool children that the conversion would remove stay | typed |
| `extraTags` | weapon tags next to the class tag and the one handed mark, each once | typed, or the suggestion `extra-tags` |
| `rawExtras` | raw nodes appended to the converted def | typed |

**Tool plan.** An entry of `toolPlan` (`CeToolPlan`) names a label and starts from a vanilla tool: the one named by `from`, else the one with the same label. It takes over that tool's numbers, penetration, body part group and carried children, then applies the fields it sets (capacities, power, cooldown, pick weight, sharp and blunt penetration, body part group). An entry with no vanilla tool to start from must give capacities, power, cooldown and a blunt penetration. An empty label is refused (`ce.tool-plan-invalid`) unless the entry names `from`, and then the tool is written without a label, as Combat Extended's own bow tool is; a repeated label is allowed, because Combat Extended's own conversions repeat one for tools with other capacities. With a plan, the per vanilla tool penetration of the block is no longer required, and the habits only fill the body part group and pick weight an entry leaves open.

**The muzzle habit is a suggestion.** When the converted tools with a label agree on capacities (3 examples and 75 percent), a design whose tool has other capacities used to be changed silently, and a capacity that the tool gave up moved to a new tool (the muzzle of a pistol that Combat Extended merged by hand). That is now a restructuring: the patch keeps the tools of the design and reports a hint `ce.tool-restructure-suggested` per change, and `suggest_tool_plan` returns the plan that writes it. Accepting the suggestion sets `toolPlan`; the written tools are then the same as before the change, now by the user's choice.

**Tool children that conversions drop.** The reader records, for every converted weapon, the names of the tool children it carries and those its vanilla twin carries (`CeModel.extras`). A child (not one of the modelled fields) that at least 2 twins carry and that at least 75 percent of those weapons no longer carry after conversion is removed from the carried children of the converted tools, and the patch reports it as a derived value (`ce.derived-value`, "tools labelUsedInLogging removed"). `keepToolFields` keeps a child.

**Suggestions.** `ce::suggest::options::suggest_options(spec, model, held)` lists the habits as `CeOption { id, field, label, value, examples, of, why }` (also `CeSuggestion.options`): the companion tags that at least 60 percent of the converted guns of the class carry (with at least 2 guns), the recoil pattern and the one at a time reload that at least 60 percent of the class share, and the tool plan. `accept_options(spec, model, ids)` writes the chosen ones, never overwrites a field the block holds, and leaves a design without a block unchanged. Companion tags are class level habits; on the owner's install none of them reaches the bar, so the tag suggestion stays empty there and the tags are given by the user.

**Raw nodes.** `rawExtras` are checked with the tree validation of the shared node type (names, attributes, characters, depth). A node whose name the conversion writes itself (`defName`, `statBases`, `verbs`, `comps`, `tools`, `weaponTags`, `equippedStatOffsets`), a second node of the same name, and a list entry that has neither a `Class` attribute nor plain text are refused with `ce.raw-extra-invalid`. A list (every child is an `li`) is merged entry by entry, each entry only when the def does not hold it yet (matched by class, or by text); another node replaces the def's node of that name or is added. Applied twice the patch changes nothing. The generator does not know what the nodes mean: they are the way to write art and platform specific additions such as a gun draw extension.

**Economy and art by policy.** Costs, work to make, stuff categories, the graphic draw size and the `MoveSpeed` offset stay what the design says. A new conversion carries one info diagnostic `ce.economy-by-design` that says so, once per patch; the harness classifies these paths as policy and never as a defect.

**Update mode** writes the new ammo and verb fields when they differ from the existing conversion, adds accepted tags with a guard, and applies the raw nodes with the same guards; the tool plan is a matter of new conversions (a plan for an existing conversion is not diffed).

**Harness.** `RIMSTUDIO_FIDELITY_PLAN` selects how the additions are filled: `none` (the plain conversion), `suggested` (the default: every suggestion accepted) and `explicit` (the real conversion's tools as a tool plan, the real extra tags and the extra `modExtensions` entries as raw nodes). The harness classifies every finding of every kind as policy, optional, unsupported, habit or derived and fails in strict mode on a missing or not removed path without an explanation, on a finding without a class and on a weapon where applying twice differs from once. The decision is D-146; the habits follow the evidence gate of D-095 (agreeing examples, shown as derived values or hints, never a rating).

### 14.17 Ammunition: the catalogue and custom calibers

A weapon converted to Combat Extended fires an ammo set. The designer shows the user's own ammo sets as a list and, when the user wants a caliber that does not exist, a window for a new one. Both are optional parts of the Combat Extended block (D-085, D-152) and never turn the switch on.

**The library.** `read_conversions_with` also reads the ammunition of the load into `CeModel.ammo` (`ce::ammo::AmmoLibrary`): the ammo classes (`AmmoCategoryDef`), the ammo items (`AmmoDef`, a thing def subclass), the projectiles of the ammo sets with their Combat Extended properties, the recipes that make the ammo items, the thing categories of the ammo with their ancestors, the labels of the sets and how the install writes the pairs of a set (`<AmmoDef>Projectile</AmmoDef>` or `li` entries). The reader needs the two ammunition types in the type table (`ce::ammo::names::type_entries`, added by `with_ce_types`). A child that most defs of a kind share unchanged (80 percent, measured among the defs under the same parent when there are 3 or more) is taken to come from a base and is not copied; what is left is kept as raw nodes. On the owner's install the library is read in about 70 ms.

**The catalogue.** `designer_ce_ammo_catalog { draft?, query?, caliber?, class?, page?, pageSize? }` returns `CeAmmoCatalogDto`: the number of sets in the install, the number that match, the entries of the page and two facet lists (calibers and families, ammo classes) over the sets that match the search words. An entry has the def name and label of the set, the caliber group (the label of the thing category of its ammunition) and family (the parent category), `similarTo`, whether other sets are similar to it (`generic`, `similarSets`), the ammo types and the number of converted weapons that use the set with up to three example labels. A type has the ammo item and its class, the projectile and its damage def, damage, sharp and blunt penetration, speed, pellets, explosion radius and secondary damage. With a draft in the request the sets that the relevance ranking of that design suggests (the candidates of `/ce/ammoSet` of `designer_ce_suggest`) carry `suggested` and `score` and come first. The order is deterministic. Without Combat Extended data the answer has `available: false` and the plain reason.

**The custom ammo spec.** `CePatchSpec.customAmmo` (`CustomAmmoSpec`) holds the name from which the def names derive (`<prefix>_AmmoSet_<Name>`, `<prefix>_Ammo_<Name>_<Key>`, `<prefix>_Bullet_<Name>_<Key>`, `<prefix>_MakeAmmo_<Name>_<Key>`, with the project prefix of the design), the caliber label, the optional set label, `similarTo` (so guns that accept a generic set can use the new one), the parent thing category and icon of the caliber's own category, the default type and a list of types. A type has a key, an ammo class chosen from the install's ammo classes, a label and description, the projectile, the ammo item and the recipe. Every number is a sourced value. The projectile carries the damage def and amount, sharp and blunt penetration, speed, pellet count and spread multiplier, secondary damage, explosion radius and the neighbour flag, suppression and danger factors, casing drop and the casing fleck and filth, the incendiary flag, fly overhead, the graphic (path, class, size, extra children), four sounds, fragments and raw children of the properties and of the def. The ammo item carries its parent, mass, bulk, market value (absent means the game prices it from the recipe), stack limit, thing categories, trade tags, graphic, tech level, stats beyond mass and bulk, the cook off behaviour and raw children. The recipe carries its parent, texts, the ingredients (a thing, alternatives, categories, a count), the products per craft, the work amount, the workbenches, the research prerequisite or prerequisites and the Crafting level. A parent is the abstract def of the user's own ammunition that the type copies from; a type without one gets the warning `ce.ammo-no-parent`.

**Suggestions.** `designer_ce_ammo_suggest { class, hints { caliber?, damage?, speed?, similarSet? }, copyFrom? }` returns `CeAmmoSuggestionDto`. Without `copyFrom` the nearest ammunition of the class by the energy index (damage times speed, an implementation choice that orders calibers well enough to find a neighbour) is found; the target energy comes from the hints (damage and speed given, or the type of `similarSet` of the same class), else it is the class median. A number is the median of the nearest five and carries its source (`nearest`, `hint`, `copied`), the number of types behind it, the smallest and largest value among them and a rating (reliable when the five agree within 15 percent, rough within 40, unreliable beyond, unmeasured with fewer than three or when the class has no ammunition and the nearest of any class was used, which is noted). A name (parent, thing class, damage def, recipe parent, research) is the nearest one's. The suggested type is ready to use: its values are `suggested`, the hints are `answered`, and a value the user typed is never replaced. With `copyFrom` every value, the raw children included, is copied from that ammunition, except what names its own caliber (labels, description, thing categories). `set_from_existing` rebuilds a whole install set as a spec.

**Files.** When the switch is on and `customAmmo` is present, `export_ce_plan` adds the file `[<version>/]<CE folder>/Defs/Ammo/<prefix>_<Name>.xml` (kind `ce-defs`) next to the weapon patch and `LoadFolders.xml`. The defs are, in order: the thing category of the caliber (when a parent category is given), the ammo set, then per type the ammo item, the projectile and the recipe. Each def has a section header `====== <defName> ======`, so the update region mechanism replaces a changed def in place and keeps every other byte. The weapon's conversion then uses the custom set as its ammo set and the projectile of the default type as its default projectile (a typed ammo set that differs gets `ce.ammo-set-overridden`). Lists that merge with the parent's (trade tags, thing categories, recipe users, raw component lists, mod extensions) are written as replacements (`Inherit="False"`) so nothing is written twice. Textures and sounds are not imported yet: an ammo item without a texture path gets the reserved path `Things/Item/Ammo/<DefName>` and the info `ce.ammo-art-reserved` names the file where the art goes; a projectile without art draws like its parent. Importing art is left to the asset import of the designer (items toolkit section 8.6). The gate holds: `gate_violations` accepts the file only under the gated folder, and a failing plan carries no ammunition file.

**Checks.** `validate_custom_ammo` (pure) and `validate_custom_ammo_refs` (with the def lookup) report: a missing name, caliber or type, a name that is no identifier, an ammo class that is missing or not an ammo class of the install, repeated keys, a missing damage, penetration or speed (unless the projectile explodes), numbers that are not finite or are out of range, pellets that are not a whole number, an unknown `similarTo` or default type, text that cannot be written as XML, derived def names that already exist in the install or the project, and ingredients, workbenches, research projects, damage defs and sounds that do not exist. Plausibility is a measurement: a damage, penetration, speed, mass, bulk or penetration per damage that lies below half of the smallest or above twice the largest value among the installed ammunition of the class (with at least 5 types) is reported with the range found as `ce.ammo-implausible`, never refused and never clamped. The lint rules for the generated definitions are CEP050 (a parent that nothing defines), CEP051 (an ammo set pair that names an undefined ammo item or projectile), CEP052 (a cook off projectile that is not defined), CEP053 (a recipe that makes nothing, makes an undefined def or has no ingredient), CEP054 (an ammo item without its class attribute), CEP055 (projectile properties without the Combat Extended class), CEP056 (a projectile without damage or penetration) and CEP057 (a def name repeated in the file); they run in `ce::ammo::lint_defs` over `Defs` roots, which the patch lint of section 8 does not read. `dry_load` loads the generated defs through the def engine with the Combat Extended type table (an empty abstract parent stands in for each named parent) and confirms that every def resolves as the type the game reads it as: the ammo item as the ammo def type, the set as the ammo set def type, the projectile as a thing def and the recipe as a recipe def.

**Fidelity.** The ignored test `real_ce_ammo` rebuilds every ammo set of the owner's Combat Extended as a custom spec of typed values, generates the files, loads them as one more mod after the real defs and compares each generated def with the real one leaf by leaf on the resolved nodes; the counts are in [the ammunition note](../research/ce-ammo-0.1.0.md).

**With custom ammunition nothing is asked for the set.** A weapon whose block holds `ce.customAmmo` gets its ammo set and default projectile from the custom caliber, so `designer.ce-needs-answer` (Weapons page) and the ask `/ce/ammoSet` (conversion of a project weapon) are not raised for them. The conversion flow takes `customAmmo` from the `overrides` of the answers into the derived block, so the plan of a conversion writes the same ammunition file as a designed weapon (`Compat/CombatExtended/Defs/Ammo/<prefix>_<Name>.xml`), and the lint of the patch treats the def names the plan writes as defined (`custom_known_defs`). While other questions of a conversion are still open the conversion stops before the files are planned; the checks of the custom ammunition (`ce.ammo-*`, CEP050 to CEP057) are still reported in that plan, so the custom ammo window can show them while the user answers the rest. The window is described in [frontend architecture](../architecture/frontend-architecture.md) section 18.15.

Code: `rimstudio-design::ce::ammo` (`library`, `catalog`, `suggest`, `generate`, `validate`, `lint`, `dryload`, `plan`, `codes`, `names`) and the types in `rimstudio-design::model::ce_ammo`; `rimstudio-toolkit::designer::ammo` for the two queries and the reference checks of the plan; the DTOs in `rimstudio-ipc-types::designer::ce_ammo`; the CLI `designer ammo catalog` and `designer ammo suggest`, and `--set ce.customAmmo.*` on the draft commands.
