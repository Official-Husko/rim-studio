# Mod layout

Status: as built for 0.1.0 | Last updated: 2026-10-05 | Requirement ids: ML-001 to ML-097 | Decisions: D-104 to D-109, D-123 to D-125, D-180 to D-184 and D-210, [ADR 0041](../adr/0041-rimstudio-mod-layout-v1.md), [ADR 0046](../adr/0046-layout-fixes-with-an-undo-journal.md) and [ADR 0052](../adr/0052-mod-basics-editing.md)

This document specifies the RimStudio mod layout v1: where the files of a mod live, how the tool names and groups them, how it recognises the convention of a mod that already exists, and what it never moves. It covers the scaffold of a new mod, the placement of generated files (weapon and projectile definitions, sound definitions, texture paths, the gated Combat Extended folder, `LoadFolders.xml`), the layout check, the layout fixes (plan, apply, undo), the annotated project tree and the file viewer command. The code is in `rimstudio-workspace` (`scaffold`), `rimstudio-design` (`plan::layout`) and `rimstudio-toolkit` (`project`, `shared::projectfs`).

## Contents

1. Purpose and scope
2. Survey: how RimWorld and the owner's mods organise a mod
3. The decision and its rationale
4. The layout
5. Weapon definitions: grouping and naming
6. Conventions of existing projects and how they are recognised
7. Placement of generated files
8. The layout check
9. The scaffold
10. Commands
11. What the tool never moves or deletes
12. Requirements and acceptance
13. Evidence from the owner's mods and open points
14. Layout fixes: plan, apply, undo
15. Mod basics: About.xml, preview image, LoadFolders.xml, version folders

## 1. Purpose and scope

The owner asked for "proper mod folder setups in a clean way organized, based on how RimWorld does it", with room to "change it a bit to make it more organized". The layout therefore keeps every name the game itself uses, and improves organisation only where the game's own data is loose: one very long file per category, projectiles mixed in with guns, a patch folder that mixes unrelated mods.

In scope: the standard skeleton of a new mod, the path of every file the designer writes, the recognition of an existing mod's convention, the layout check and the project tree. Out of scope for 0.1.0: apparel files (the apparel designer follows 0.1.0), translation tooling, the Workshop publisher, and any automatic reorganisation of an existing mod (section 11).

## 2. Survey: how RimWorld and the owner's mods organise a mod

Read from the installed game (`Data/Core` and the four expansions), the owner's own mods, the mod corpus ([mod format and corpus](../research/rimworld-mod-format-and-corpus.md)) and the wiki conventions for About, Defs, Patches, Textures, Sounds, Languages, Assemblies, `LoadFolders.xml`, version folders and `Common`.

| Subject | What the game and the community do | Evidence |
|---|---|---|
| Weapon definitions | Every pack keeps them in `Defs/ThingDefs_Misc/Weapons/`, one file per category named by kind and tech level. Core: `BaseWeapons.xml` (abstract bases), `RangedNeolithic.xml`, `RangedIndustrial.xml` (with `RangedIndustrialConsumable.xml` and `RangedIndustrialGrenades.xml`), `RangedSpacer.xml`, `RangedSpecial.xml`, `RangedMechanoid.xml`, `MeleeNeolithic.xml`, `MeleeMedieval.xml`, `Breach.xml`. Royalty adds `MeleeUltratech.xml`, `MeleeBladelink.xml`, `PsychicWeapons.xml`, `OrbitalWeapons.xml`; Biotech `RangedMechanoid_Light.xml` and siblings; Anomaly `Weapons_Ranged.xml`; Odyssey `Ranged_Spacer.xml` | install listing |
| Projectiles | In the same file as the gun that fires them, the projectile first (`Bullet_Revolver` then `Gun_Revolver` in `RangedIndustrial.xml`) | `RangedIndustrial.xml` |
| Sound definitions | `Defs/SoundDefs/`, one file per area and kind: `World_Oneshots_Weapons.xml`, `Reload_Oneshots_Misc.xml`, `Interact_Oneshots_Violent.xml`, 35 files in Core | install listing |
| Textures | Referenced by `texPath` without extension below `Textures/`: `Things/Item/Equipment/WeaponRanged/<Name>`, `.../WeaponMelee/<Name>`, `Things/Projectile/<Name>`. The art itself ships in the game's asset bundles, so the convention comes from the `texPath` values | `texPath` values in the weapon files |
| Sounds | Clips below `Sounds/`, referenced by `clipFolderPath` relative to it | the owner's Lone Wolf mod |
| Patches | `Patches/`, any depth; Combat Extended itself keeps per mod patches in `ModPatches/` | corpus, CE source layout |
| Version folders | A folder named `major.minor` (`1.6`), plus `Common`; selected by the game's fallback order (version folder, then `Common`, then the root) or by `LoadFolders.xml`. 422 of about 690 mods in the workshop root of the corpus have a `1.6` folder, 72 have `Common`, 38 percent of all mods (291) have a `LoadFolders.xml` | corpus section 2 |
| Combat Extended folders | 39 mods name a top level folder `CE`; the others use `ModPatches`, `Patches/CE`, `1.6/CE` and similar | corpus top level folder names |
| Author material | `Source/` (63 mods, code and art), `Raw Assets/`, `.git` (71), `.vscode` (22) | corpus top level folder names |
| Metadata | `About/About.xml` is required; `Preview.png`, `ModIcon.png`, `PublishedFileId.txt` are optional; `Manifest.xml` (87 mods) is read by mod managers, not by the game | corpus |
| Owner: `[OH] Gewehr 41` | The game's own style: `Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml` (bullet then five guns, section comments), `Patches/ce_patch.xml` (uses `CombatExtended.PatchOperationFindMod` outside any gated folder), `Textures/Things/Item/Equipment/WeaponRanged/oh_g41*.dds`, `Source/` ignored by `.gitignore`, `Credits.txt`, `About/Manifest.xml` and `PublishedFileId.txt`, one version (1.4) | the mod folder |
| Owner: `[OH] The Lone Wolf Weapon Package` | `Common/Defs/ThingsDef_Misc/Weapons/Weapons_Ranged.xml` (a misspelt folder), `Common/Defs/SoundDef/Weapon_Sounds.xml`, `Patches/CE_Patch.xml` and `Patches/Patches_For_CE_And_Others/`, `Sounds/Weapons/*.wav`, `Textures/Things/Weapon/Ranged/<class>/` and `Things/Projectile/`, `Raw Assets/` ignored by `.gitignore`, `README.md`, `LICENSE.md` | the mod folder |
| Owner: `rimworld-mod-template` | `1.5/{Assemblies, Defs, Languages/English/Data.xml, Patches}`, `About`, `Source`, `.vscode`; no `LoadFolders.xml` | the template folder |

What this shows. The game's names are stable and used by every pack, so they are the right names. The game's grouping (one file per category holding many weapons) is fine for a team that edits a file by hand and poor for a tool that adds one weapon at a time: a new weapon becomes a long diff in a shared file. The owner's mods show two older habits (the game's own style, and loosely named folders), so the tool must recognise a convention instead of assuming one. Combat Extended content has no common folder name, which is why this layout names one and detects the others.

## 3. The decision and its rationale

1. **Follow the game's names wherever they exist.** `About`, `Defs/ThingDefs_Misc/Weapons`, `Defs/SoundDefs`, `Patches`, `Textures/Things/Item/Equipment/WeaponRanged` and `WeaponMelee`, `Textures/Things/Projectile`, `Sounds`, `Languages/English/Keyed`, `Assemblies`, `Source`, `Common` and version folders. A modder who knows Core finds everything where Core keeps it.
2. **Improve organisation in three places.** (a) One file per weapon, holding the weapon and its own projectile, in a folder named like the game's category file (`RangedIndustrial/OH_G41m.xml`). The category stays visible, a new weapon is a new file, and a second weapon never edits the first. (b) The gated Combat Extended content has its own folder `Compat/CombatExtended` with its own `Patches` (and, later, `Defs`), so the gate in `LoadFolders.xml` selects one clean folder. (c) Art that the game never reads goes to `Source/Art`, not into `Textures`.
3. **Recognise, never assume.** A mod that already follows another convention keeps it. A mod in the game's own style gets its new weapon appended to the matching category file as a marked section through the existing update region mechanism; a flat mod gets one file per weapon in its own weapon folder; an older Combat Extended folder is used where it is. Nothing existing is reorganised without an explicit action by the user.
4. **Vanilla by default.** The vanilla files never contain Combat Extended text. The Combat Extended folder, its patches and its `LoadFolders.xml` entry appear only when the item's toggle is on, or in the convert flow (D-085, D-087).
5. **No silent extras.** `README.md`, `Credits.txt`, `.gitignore`, `About/Preview.png` and `About/Manifest.xml` are never created without the user asking.

Why per weapon files and not category files: the category file model needs a merge on every write (the update region mechanism handles it, but the diff of a second weapon touches a file the first weapon lives in); per weapon files make the plan a plain create. Why the projectile lives in the weapon file: a projectile defined for one gun belongs to it, the game does the same, and the two defs then travel together when a file is moved or shared. A projectile that several guns share is referenced by name and written once, in the file of the weapon that introduced it.

## 4. The layout

All paths are relative to the mod root. `<content>` is the mod root, or a version folder (`1.6`) or `Common` when the mod keeps its content there.

```
<mod>/
  About/
    About.xml                       created by the scaffold, with a description placeholder
  LoadFolders.xml                   only for a versioned layout or a gated Combat Extended folder
  <content>/
    Defs/
      ThingDefs_Misc/Weapons/
        RangedIndustrial/
          OH_G41m.xml               the weapon and its own projectile
        MeleeMedieval/
      SoundDefs/
        World_Oneshots_Weapons.xml  reserved name for shot sounds
    Patches/                        patches for vanilla and other mods (no Combat Extended class)
    Compat/
      CombatExtended/               gated in LoadFolders.xml by IfModActive="ceteam.combatextended"
        Patches/
          <slug>_Weapons_Ranged.xml
        Defs/Ammo/
          <prefix>_<Name>.xml       only for a custom caliber: ammo set, ammo items, projectiles, recipes
    Textures/Things/
      Item/Equipment/WeaponRanged/<DefName>.png     reserved, the tool never creates the image
      Item/Equipment/WeaponMelee/<DefName>.png
      Projectile/<DefName>.png
    Sounds/Weapons/<DefName>_Shot/  reserved clip folder
    Languages/English/Keyed/        optional
    Assemblies/                     optional
  Source/Art/                       optional, never shipped
```

| Folder or file | Role id (`project_tree`) | Created by the scaffold | Notes |
|---|---|---|---|
| `About/` and its files | `about` | yes (`About.xml` only) | `Preview.png`, `ModIcon.png`, `Manifest.xml`, `PublishedFileId.txt` are never created silently |
| `LoadFolders.xml` | `load-folders` | when versioned or with the Combat Extended folder | edited only by byte span splice |
| `Defs/` | `defs` | yes | the folder of definitions |
| `Defs/ThingDefs_Misc/Weapons` | `defs-weapons` | yes | the weapon folder of the RimStudio layout |
| `Defs/SoundDefs` | `defs-sounds` | yes | sound definitions |
| `Patches/` | `patches` | yes | no Combat Extended class (section 8) |
| `Compat/CombatExtended/` | `ce-compat` | only on request | gated; the older `CE` is also recognised |
| `Textures/` | `textures` | yes (three folders) | the tool names the file where art goes and never makes an image |
| `Sounds/` | `sounds` | yes (`Weapons`) | clips are the user's |
| `Languages/`, `Assemblies/` | `languages`, `assemblies` | only on request | |
| `Source/`, `Raw Assets/` | `source` | `Source/Art` only on request | author material the game never reads |
| a version folder or `Common` | `content-root` | for the versioned layout | holds the content folders above |

### 4.1 Version folders, `Common` and `LoadFolders.xml`

1. A new mod is flat (content in the mod root) unless the user asks for the versioned layout. The game then needs no `LoadFolders.xml`.
2. The versioned layout creates one folder per supported version (`1.6`) and `Common`, and a `LoadFolders.xml` with a block per version listing `Common` and the version folder (the game's own fallback order).
3. `LoadFolders.xml` is created, as well, when the Combat Extended folder is requested: the block lists the root (or the version folder) and the gated `Compat/CombatExtended` entry.
4. An existing mod keeps what it has. A mod with a `1.6` folder and no `Defs` at its root is a versioned mod, its content folder is the last listed supported version that has a folder, and every planned file goes inside it; `LoadFolders.xml`, `About` and `Source` stay in the root. A mod with no version folder but a `Common` that holds `Defs` uses `Common` the same way.
5. Standard folders found at the root count as present for a mod whose content lives in `Common` (the game loads the root as well), so the check does not ask for a duplicate.

## 5. Weapon definitions: grouping and naming

### 5.1 Category

The category names are the names of the game's own category files. The designer chooses one from the kind and the tech level of the item:

| Kind | Tech level | Category |
|---|---|---|
| ranged | Neolithic, Medieval | `RangedNeolithic` |
| ranged | Industrial | `RangedIndustrial` |
| ranged | Spacer, Ultra | `RangedSpacer` |
| ranged | Archotech | `RangedSpecial` |
| melee | Neolithic | `MeleeNeolithic` |
| melee | Medieval, Industrial | `MeleeMedieval` |
| melee | Spacer, Ultra, Archotech | `MeleeUltratech` |

`RangedMechanoid` is recognised in existing mods and never chosen for a design. An absent tech level counts as Industrial for ranged and Medieval for melee weapons; the validator asks for the level, so this is only a fallback. The mapping is a naming rule and holds no game value.

### 5.2 One file per weapon

`<weapon folder>/<Category>/<DefName>.xml` holds, in this order, the projectile (when the item creates one) and the weapon. Each def has a section header `<!-- ====== <defName> ====== -->`, which the update region mechanism uses to find the def again. The file name is the def name with every character other than letters, digits, `_` and `-` replaced by `_`.

Example, a new ranged weapon `OH_G41m` of tech level Industrial with its own projectile:

```
Defs/ThingDefs_Misc/Weapons/RangedIndustrial/OH_G41m.xml

<Defs>
  <!-- ====== OH_Bullet_G41m ====== -->
  <ThingDef ParentName="BaseBullet"> ... </ThingDef>
  <!-- ====== OH_G41m ====== -->
  <ThingDef ParentName="BaseHumanMakeableGun"> ... </ThingDef>
</Defs>
```

### 5.3 Textures, sounds and art

1. A weapon without a texture path of its own gets the reserved one, `Things/Item/Equipment/WeaponRanged/<DefName>` (`WeaponMelee` for melee), written into its `graphicData`. The plan carries the info diagnostic `design.texture-reserved` naming the file where the art goes (`Textures/Things/Item/Equipment/WeaponRanged/<DefName>.png`). The tool creates no image; the art can be imported (item 4). A typed texture path is kept as typed, and so is the texture path a clone takes over from its source (a vanilla texture that works in game): the plan then carries the info `design.texture-shared` naming the same file for the art of the new weapon (and `Textures/Things/Projectile/<DefName>.png` for an own projectile that keeps a texture). A clone whose parent supplies the texture writes none and gets no reserved path. Art is copied only when the user imports a PNG (item 4); a texture a clone takes over from its source is never copied ([generation fidelity](items-toolkit.md#1512-generation-fidelity-of-a-clone)).
2. Projectile textures reserve `Things/Projectile/<DefName>`; sound definitions go to `Defs/SoundDefs/World_Oneshots_Weapons.xml` (shots), `Reload_Oneshots_Weapons.xml` and `World_Oneshots_ProjectileImpacts.xml`, with clips in `Sounds/Weapons/<DefName>_Shot`, `_Reload` and `_Impact`; the `clipFolderPath` is `Weapons/<DefName>_Shot` (relative to `Sounds`). The designer writes the shot sound definition and its clips when the user brings clip files (item 5); reload and impact sounds keep the reserved paths and are chosen by name.
4. An imported texture is copied to `[<version>/]Textures/<texPath>.png` where `<texPath>` is the reserved path of item 1 (weapon) or item 2 (own projectile), and `graphicData.texPath` points at it. The source is a PNG chosen by the user, at most 8 MiB and 4096 pixels on a side ([items toolkit](items-toolkit.md#86-imported-assets-textures-and-custom-sounds)). A texture already at the target is replaced after a backup in the data root.
5. An imported shot sound is written as a marked section `====== <SoundDefName> ======` in `[<version>/]Defs/SoundDefs/World_Oneshots_Weapons.xml` (one file per mod, one section per sound, merged like a weapon in a project that keeps one file per category), with its clips copied to `[<version>/]Sounds/Weapons/<DefName>_Shot/<clip>.wav` or `.ogg`. The `clipPath` of each `AudioGrain_Clip` is `Weapons/<DefName>_Shot/<clip>` (relative to `Sounds`, no extension). The sound def name is `<DefName>_Shot`, with the mod prefix in front when the weapon's name lacks it.
3. The design art (layered files, reference sheets) goes to `Source/Art`, at the mod root even for a versioned mod. The optional `.gitignore` can keep it out of the repository.

## 6. Conventions of existing projects and how they are recognised

A project has one convention profile for weapon files, decided from the files on disk each time a project is read (nothing is stored).

| Profile id | How it is recognised | Where a new weapon goes |
|---|---|---|
| `rimstudio` | A weapon folder (`Weapons`, any case, up to three levels below `Defs`) that holds a sub folder named like a category; also a mod with no weapon files at all (a new mod) | `<weapon folder>/<Category>/<DefName>.xml`; the weapon folder is `Defs/ThingDefs_Misc/Weapons` for a new mod |
| `core-style` | A weapon folder whose XML files are named like category files (`RangedIndustrial.xml`) | The matching category file; when it exists the weapon and its projectile are appended as marked sections (update region), when it does not the file is created beside the others |
| `flat` | A weapon folder with XML files that do not follow the category names (the Lone Wolf mod's `ThingsDef_Misc/Weapons/Weapons_Ranged.xml`), or definition files lying directly in `Defs` | `<folder>/<DefName>.xml` in the folder the mod already uses; the existing files are not touched |

Rules of the detection (ML-020 to ML-026):

1. The content folder is decided first (section 4.1 item 4); the weapon folder is searched below its `Defs`.
2. When several weapon folders qualify the profile of the best one wins in the order `rimstudio`, `core-style`, `flat`, then the shallowest path, then the name.
3. The Combat Extended folder is, in this order: the folder `LoadFolders.xml` gates on `ceteam.combatextended` (any case, relative to the content folder), `Compat/CombatExtended`, `CE`, `CombatExtended` as spelled on disk; when none exists, `Compat/CombatExtended`. A folder other than the standard one is reported as an older name (`ce-folder-legacy`) and used where it is. Moving it needs an explicit action by the user.
4. The mod slug of patch file names is the last segment of the package id (`oh.weapons.gewehr41` gives `gewehr41`); file names are `<slug>_Weapons_Ranged.xml`, `<slug>_Weapons_Melee.xml`, `<slug>_Apparel.xml` and, for update mode, `<slug>_Weapons_Ranged_Update.xml`.
5. Detection reads folder and file names only; it never opens a weapon file to decide.

## 7. Placement of generated files

| Request | Files (relative to the mod root; inside the content folder when there is one) |
|---|---|
| Vanilla design (always) | the weapon file of the profile (section 6); one file, projectile first |
| Combat Extended toggle on for the item | the above unchanged, plus `<CE folder>/Patches/<slug>_Weapons_Ranged.xml` or `Weapons_Melee`, plus `LoadFolders.xml` (created, or edited by splice) |
| Combat Extended toggle on and a custom caliber (`ce.customAmmo`) | the above, plus `<CE folder>/Defs/Ammo/<prefix>_<Name>.xml`: the thing category, the ammo set, and per ammo type the ammo item, the projectile and the recipe, each with a section header; the gate of `LoadFolders.xml` already covers the folder, so no further edit is needed |
| Convert flow, update mode | `<CE folder>/Patches/...` and `LoadFolders.xml` only; no definition file of the project is touched |

Examples:

| Project | Weapon `RS_Rifle`, Industrial, ranged | With Combat Extended |
|---|---|---|
| New mod, flat | `Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Rifle.xml` | `Compat/CombatExtended/Patches/<slug>_Weapons_Ranged.xml`, `LoadFolders.xml` |
| Versioned `1.6` | `1.6/Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Rifle.xml` | `1.6/Compat/CombatExtended/Patches/...`, `LoadFolders.xml` with `1.6/Compat/CombatExtended` |
| Game style (Gewehr 41) | `Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml`, two sections appended | the gate and folder as above, unless the mod has an older folder |
| Flat (Lone Wolf) | `Common/Defs/ThingsDef_Misc/Weapons/RS_Rifle.xml` | `Common/Compat/CombatExtended/Patches/...` |
| Older folder `CE` gated | as the profile says | `CE/Patches/...`; `LoadFolders.xml` already gates it and is reported unchanged |
| New mod, custom caliber `Mine` (prefix `RS`) | as above | `Compat/CombatExtended/Defs/Ammo/RS_Mine.xml` next to the patch; `1.6/Compat/CombatExtended/Defs/Ammo/RS_Mine.xml` for a versioned mod |

All paths in plans and reports are relative to the project root with `/` separators. A plan never contains a path outside the root; the guarded writer refuses `..`, absolute paths, drive letters, reserved device names, links out of the root and the protected game folders.

## 8. The layout check

`project_layout_check` reads the project (no write) and lists issues, most serious first, each with a code, a severity, the path, a plain message and a suggested fix.

| Code | Severity | Meaning | Fix | Automatic |
|---|---|---|---|---|
| `layout.missing-about` | error | no `About/About.xml` | create it (the new project dialog writes one) | no |
| `layout.missing-folder` | info | a standard folder does not exist (`Defs`, the weapon folder, `Defs/SoundDefs`, `Patches`, `Textures`, `Sounds`) | create the empty folder | yes, and safe |
| `layout.folder-case` | warning | `defs` for `Defs` and similar: invisible to the game on Linux and macOS | rename the folder | no |
| `layout.weapon-misplaced` | warning | a file defines weapons outside the weapon folder | move the file to the named path | no |
| `layout.def-wrong-category` | warning, info | a ranged weapon in a melee category or the reverse (warning); an explicit tech level that fits another category (info) | move the definition to the named path | no |
| `layout.ce-outside-gate` | warning | a definition or patch file mentions `CombatExtended.` outside the gated folder and the About file does not list Combat Extended as required | move it to `<CE folder>/Patches` and gate the folder | no |
| `layout.ce-folder-ungated` | warning | the Combat Extended folder has files but `LoadFolders.xml` has no entry conditional on Combat Extended for it | add the entry (the designer plan does) | no |
| `layout.ce-legacy-folder` | info | the Combat Extended folder is `CE` or `CombatExtended`; it works and stays | none | n/a |
| `layout.load-folders-missing-folder` | warning | `LoadFolders.xml` lists a folder that does not exist | create it or remove the entry | no |
| `layout.wrong-root` | warning | a file in `Defs` has a `Patch` root or the reverse; the game ignores it | move it | no |
| `layout.unparsable-file` | warning | a definition or patch file is not valid XML | fix it | no |
| `layout.texture-missing` | info | the `texPath` of a weapon has no file in the project (fine for a vanilla texture) | add the art at the named path | no |

Only the creation of a missing folder is applied by `project_scaffold_missing`. The other fixes are carried out by the layout fix commands of section 14, item by item, after a plan and with an undo; they are never applied silently (section 11). At most 500 issues are reported.

## 9. The scaffold

`project_create` (CLI `project create`) writes the plan of `rimstudio_workspace::scaffold::plan` through the guarded writer: it never overwrites (a planned file that exists must hold exactly the planned text, which completes a stopped run) and writes `About/About.xml` last.

| Option | Default | Creates |
|---|---|---|
| name, package id, author, supported versions | 1.6 | `About/About.xml`; an empty description gets the placeholder "Describe what this mod adds. This text is shown in the mod list and on the Steam Workshop page." |
| definitions | on | `Defs/ThingDefs_Misc/Weapons`, `Defs/SoundDefs` |
| patches | on | `Patches` |
| textures | on | `Textures/Things/Item/Equipment/WeaponRanged`, `WeaponMelee`, `Textures/Things/Projectile` |
| sounds | on | `Sounds/Weapons` |
| versioned folders | off | one folder per version, `Common`, `LoadFolders.xml` |
| Combat Extended folder | off | `Compat/CombatExtended/Patches`, the gated entry in `LoadFolders.xml` |
| languages | off | `Languages/English/Keyed` |
| assemblies | off | `Assemblies` |
| source folder | off | `Source/Art` (in the root, also for a versioned mod) |
| `.gitignore` | off | operating system files, build output of `Source`; with the Source art option also `Source/Art/` and `Raw Assets/` |
| `README.md`, `Credits.txt` | off | the name and description; the name and the author |
| placeholder files | off | `.gitkeep` in every empty leaf folder, for a repository |

The optional files offered are exactly `README.md`, `Credits.txt` and `.gitignore`. `About/Preview.png` (it needs an image) and `About/Manifest.xml` (it belongs to a mod manager and to the Workshop publisher) are not offered by the scaffold. A new mod is flat, with the default folders above, and holds Combat Extended content only when asked.

`project_scaffold_missing` creates only the missing standard folders of an existing mod (the `layout.missing-folder` issues), in the content folder, never a file, never over an existing entry; a second run does nothing; `dryRun` lists what would be created.

## 10. Commands

| Command (CLI) | Kind | Does |
|---|---|---|
| `project_tree` (`project tree`) | query | The annotated tree: every folder and file with its role, size, files below it and the number of issues on or below it; totals (folders, files, bytes, definition files, weapons, projectiles, patch files, textures, sounds); the profile, the weapon folder, the Combat Extended folder (exists, older name, gated) and the issue list. At most 4000 entries are listed by default; counts stay complete and `truncated` says so. Links are listed as empty files and never followed; `.git`, `.vs`, `node_modules` and `target` are listed and not entered |
| `project_layout_check` (`project check`) | query | The issues of section 8 with counts per severity and the number that `project_scaffold_missing` fixes |
| `project_scaffold_preview` | query | Section 9.1: what `project_create` would write, and the findings about the values |
| `project_scaffold_missing` (`project scaffold-missing`) | action | Section 9 |
| `project_layout_fix_plan` (`project fix plan`) | query | Section 14: the fixable findings as a list of items, read only |
| `project_layout_fix_apply` (`project fix apply`) | job | Section 14: carries out the selected items of a reviewed plan, writes an undo journal first, runs the check again |
| `project_layout_fix_undo` (`project fix undo`) | action | Section 14: reverses an apply from its journal, or refuses and changes nothing |
| `project_layout_fix_history` (`project fix history`) | query | Section 14: the journals of a project and whether each can still be undone |
| `project_read_file` (`project read`) | query | One text file by relative path for the file viewer: the path goes through the guarded writer's checks, at most 262144 bytes by default (1048576 at most), UTF-8 without the byte order mark, `binary` for a file with a NUL byte or invalid UTF-8, `truncated` and the full size otherwise. A folder, a missing file, a link that leaves the root and an unsafe path are errors (`project.path-outside-root`, `io.not-found`) |
| `project_about_get` (`project about show`) | query | Section 15: every basic of the mod with the findings, the file hash and the raw text |
| `project_about_preview` (`project about set`, without `--yes`) | query | Section 15: the diff and the resulting model of a list of changes; nothing is written |
| `project_about_update` (`project about set --yes`) | action | Section 15: the changes as byte span edits, with a backup and a read back |
| `project_about_set_preview`, `project_about_remove_preview` (`project about preview`) | action | Section 15: copy a PNG to `About/Preview.png`, or remove it after a backup |
| `project_load_folders_get`, `project_load_folders_update` (`project load-folders show`, `set`) | query, action | Section 15: the blocks and entries of `LoadFolders.xml`, and byte span edits of them |
| `project_version_add` (`project version add`) | action | Section 15: the folder of another game version, its standard sub folders and its block |
| `library_mod_search` (`library search`) | query | Section 15: mods of the last scan by name, package id or author, for the dependency picker |

The designer's plan, apply and convert use the same layout and report paths relative to the project root (`designer_export_plan`, `designer_apply_plan`, `designer_convert_scan`).

## 11. What the tool never moves or deletes

1. No command moves, renames, rewrites or deletes an existing file or folder of a mod because of the layout, except `project_layout_fix_apply` for the items of a plan the caller reviewed and selected (section 14), which never overwrites, never deletes content, never touches a texture or a sound and can be undone. The layout check itself only reports.
2. A plan changes an existing file only by byte span edit of one marked section or by adding a section (the update region mechanism), after a backup outside the mod folder, and only a file named in the plan.
3. The older Combat Extended folder, a flat weapon folder and category files of the game's style stay where they are; new files follow the profile.
4. `project_scaffold_missing` creates folders only. Links are never followed by the scan, the reader or a fix.
5. The game install, the Workshop folders and the reference mod folders are never written (the protected paths and the write fence of ADR 0021).
6. A fix never moves a texture or a sound (definitions refer to them by path), never merges two folders and never splits a file; those are listed as needing review.

## 12. Requirements and acceptance

| Id | Requirement | Acceptance |
|---|---|---|
| ML-001 | A new project has the standard skeleton and nothing optional | scaffold golden trees, CLI `a_new_mod_has_the_standard_skeleton_and_nothing_optional` |
| ML-002 | Every optional file is created only when asked; `Preview.png` and `Manifest.xml` never | scaffold unit and property tests |
| ML-003 | `About.xml` has the supported version and a description placeholder when none is given | scaffold tests |
| ML-004 | The versioned layout has a folder per version, `Common` and a `LoadFolders.xml` that lists them | scaffold golden tree |
| ML-005 | The Combat Extended folder is `Compat/CombatExtended`, gated by `IfModActive="ceteam.combatextended"`, off unless asked | scaffold and plan tests, property test of the gate |
| ML-010 | The vanilla plan is one file per weapon in the category folder, projectile before weapon, section headers per def | `plan_vanilla.rs` (design and toolkit), goldens |
| ML-011 | The category follows kind and tech level (section 5.1) | `layout.rs` unit tests |
| ML-012 | A weapon without a texture path gets the reserved path and a hint; a typed path is kept; no image is created | design and toolkit plan tests |
| ML-013 | Sound definition, sound clip and art paths follow section 5.3 | `layout.rs` unit tests |
| ML-020 | The three profiles are recognised from folder and file names | `plan_profiles.rs`, `project_layout.rs` |
| ML-021 | A game style project gets a marked section appended to the category file and no new file | `a_project_in_the_games_own_style_gets_a_marked_section_appended_to_the_category_file` |
| ML-022 | A flat project gets one file per weapon in its own folder | `a_flat_project_keeps_its_folder_and_gets_one_file_per_weapon` |
| ML-023 | A missing category file is created beside the existing ones | `a_missing_category_file_is_created_next_to_the_existing_ones` |
| ML-024 | A content folder (`1.6`, `Common`) holds every planned file; `LoadFolders.xml` stays at the root | `a_version_folder_holds_the_files_and_load_folders_stays_at_the_root` |
| ML-025 | An older Combat Extended folder is detected from the gate or from its name, used for patches and update mode, never moved | `an_older_ce_folder_is_detected_used_and_never_moved`, `a_project_with_the_older_ce_folder_keeps_it_for_a_conversion` |
| ML-026 | The vanilla plan never names the Combat Extended folder | `the_vanilla_plan_never_names_the_ce_folder_even_in_a_project_that_has_one` |
| ML-030 | `project_tree` annotates every entry with a role, sizes and issue counts, deterministically, folders first | `project_layout.rs` |
| ML-031 | The tree never follows a link and survives hostile files (a DTD bomb, a 2 MB file, deep folders, invalid bytes) | `links_out_of_the_project_are_neither_followed_nor_read`, `hostile_xml_and_huge_trees_do_not_break_the_scan` |
| ML-032 | The layout check reports the issues of section 8 with a fix each; only a missing folder is automatic | `project_layout.rs`, CLI `check_lists_the_issues...` |
| ML-033 | `project_scaffold_missing` creates folders only, is idempotent, supports `dryRun` and refuses a protected project | `scaffolding_missing_folders_creates_folders_only...`, `scaffolding_refuses_a_project_inside_a_protected_folder` |
| ML-034 | `project_read_file` limits the size, marks binary files and refuses every path attack | `path_attacks_on_the_file_reader_are_refused`, `reading_a_file_is_limited_and_marks_binary_files` |
| ML-040 | The DTOs have golden JSON and generated TypeScript types that match it | `golden_project_tree`, `golden_project_layout_check`, `golden_project_file`, `pnpm --filter rimstudio-ipc-types bindings:check` |
| ML-050 | The tree and the check run over copies of the owner's weapon mods and print their findings | `real_layout.rs` (ignored; needs `RIMSTUDIO_CUSTOM_DIR`) |
| ML-051 | `project_layout_fix_plan` lists the fixable findings with id, kind, from, to, why, risk, the `LoadFolders.xml` diff, the conflict and the references, and a content hash as plan id; it is read only and deterministic | `gewehr_41_plan_is_golden`, `the_lone_wolf_plan_is_golden`, `the_plan_is_deterministic_and_read_only` (golden `fix_plan_*.json`) |
| ML-052 | A Combat Extended patch outside the gate moves to `<CE folder>/Patches` and the folder is gated (an entry added by span edit, or a new `LoadFolders.xml` that keeps loading what the game loaded without one) | `applying_the_gewehr_41_plan_gates_and_moves_the_patch`, `applying_the_lone_wolf_plan_keeps_the_content_in_common_loaded`, `a_backup_of_the_edited_load_folders_is_kept_in_the_data_root`, `an_entry_for_the_folder_without_a_condition_is_gated_in_place` |
| ML-053 | An older Combat Extended folder is renamed to `Compat/CombatExtended` together with its `LoadFolders.xml` entries, as two items that apply together | `a_legacy_folder_is_renamed_with_the_patches_inside_and_undone`, `a_legacy_combat_extended_folder_is_renamed_with_its_entry` |
| ML-054 | A weapon file moves to the place of its category or file name rule when one file holds one weapon (or one category); a file with weapons of several categories is not split | `a_single_weapon_file_in_the_wrong_category_moves`, `a_file_with_several_weapons_is_not_split_by_a_move` |
| ML-055 | Textures and sounds, a move whose old path other files mention, a patch with operations that do not use Combat Extended, a move between game version folders and a destination the path rules refuse are listed as needing review with the references found, and are never applied | `textures_and_sounds_are_listed_for_review_only`, `a_move_whose_old_path_other_files_mention_needs_review_with_the_references`, `a_patch_with_a_vanilla_operation_needs_review`, `a_move_may_not_change_the_game_versions_a_file_loads_for`, `items_that_need_review_unknown_ids_and_orphaned_items_are_skipped_with_reasons` |
| ML-056 | A destination that exists is a conflict: refused by default, a numbered name offered and used only when the caller accepts it; nothing is overwritten | `an_existing_destination_is_a_conflict_with_a_numbered_name`, `a_destination_that_exists_is_never_overwritten` |
| ML-057 | An apply is refused when the project changed since the plan (plan id), and no journal is written | `a_file_changed_between_the_plan_and_the_apply_refuses_the_apply`, `a_plan_id_that_is_not_the_current_one_refuses_the_apply` |
| ML-058 | Every path goes through the guarded writer (inside the root, no link, no case twin, safe names); a link in a path is refused and nothing leaves the project | `a_link_in_the_destination_path_is_refused_and_nothing_leaves_the_project`, `a_linked_source_file_is_not_a_candidate`, io `rename` tests |
| ML-059 | The undo journal is written to the data root before the first change and updated after each item; a crash at any point leaves a journal an undo can use | `a_crash_between_items_leaves_a_consistent_journal_that_undoes_cleanly`, `a_crash_at_any_point_of_a_rename_with_an_entry_edit_can_be_undone` |
| ML-060 | An undo restores every byte and every folder of the owner's two weapon mods | `undo_restores_every_byte_and_every_folder` |
| ML-061 | An undo is refused, changing nothing, when a moved file changed, the original place is taken, or `LoadFolders.xml` was edited again | `an_undo_is_refused_when_the_moved_file_was_changed_afterwards`, `an_undo_is_refused_when_the_original_place_is_taken_or_the_file_is_gone`, `an_undo_is_refused_when_load_folders_was_edited_again` |
| ML-062 | A damaged or forged journal is refused (checksum, project, paths, hashes); a journal can only restore `LoadFolders.xml` among edited files | the `a_journal_*` and `a_forged_journal_*` tests of `project_fix_apply.rs` |
| ML-063 | Cancellation is observed between items; the items done stay and can be undone; the history lists the journals | `a_cancelled_job_keeps_what_was_done_and_it_can_be_undone`, `the_history_lists_the_journals_with_their_state` |
| ML-064 | Plan, apply and undo run over temporary copies of every mod of the owner's folder and restore every byte | `real_fix.rs` (ignored; needs `RIMSTUDIO_CUSTOM_DIR`), CLI `cli_fix.rs` |
| ML-065 | The Layout tab opens the plan from a finding or from Fix all safe, shows items, risks, the diff, conflicts and the references of review items, and applies only what is ticked after a confirmation | `FixDialog.test.tsx`, `FixPlanView.test.tsx`, `FixItemRow.test.tsx`, `fixStore.test.ts` |
| ML-066 | After an apply the result lists what was done, edited and skipped with reasons; the project tree and the check are read again; the last apply can be undone from the result | `FixResultView.test.tsx`, `fixStore.test.ts` |
| ML-067 | History lists the journals with Undo where possible and the reason where not; a stale plan and a refused undo are explained | `FixHistoryDialog.test.tsx`, `FixDialog.test.tsx` |
| ML-070 | An imported weapon or projectile texture is copied to the path of section 5.3 and the weapon's `texPath` points at it, in a plain, a versioned and a game style project | `plan_assets.rs`, `assets_flow.rs` |
| ML-071 | An imported shot sound writes a marked section in the sound file, one copy per clip below `Sounds/Weapons/<DefName>_Shot` and `soundCast`; a second weapon adds a section to the same file | `assets_flow.rs` (golden `assets_sound_def_two.xml`) |
| ML-072 | After an import the project tree counts the textures and sounds, and the layout check reports no `layout.texture-missing` for the weapon | `imported_assets_show_in_the_project_tree_and_the_layout_check_is_quiet_about_them` |

## 13. Evidence from the owner's mods and open points

The ignored test `real_layout.rs` ran over temporary copies (without `.git`, `Source` and `Raw Assets`) of the owner's two weapon mods on 2026-10-05. Gewehr 41: profile `core-style`, 5 weapons and 1 projectile in one category file, 10 textures, findings: `layout.ce-outside-gate` for `Patches/ce_patch.xml` (a warning) and two missing standard folders (`Defs/SoundDefs`, `Sounds`, info). Lone Wolf: profile `flat` with the content in `Common`, weapon folder `Common/Defs/ThingsDef_Misc/Weapons`, 9 weapons and 5 projectiles, 44 textures, 6 sounds, findings: `layout.ce-outside-gate` for `Patches/CE_Patch.xml` and one missing standard folder (`Common/Defs/SoundDefs`, because the mod's folder is named `SoundDef`). Neither mod was written to.

Open points:

1. Apparel files: the layout reserves `Defs/ThingDefs_Misc/Apparel` style names for the apparel designer, which follows 0.1.0; the category rule for apparel is not decided here.
2. The reload and impact sound definition files of section 5.3 are reserved; the designer writes only the shot sound (item 5 of section 5.3). A project whose own sound folder has another name (the owner's Lone Wolf mod keeps `Defs/SoundDef`) gets the standard `Defs/SoundDefs` file beside it; the layout check reports the missing standard folder as before.
3. The layout fixes of section 14 carry out the suggested moves. Splitting a file and merging two folders are not done; they stay listed as needing review.
4. Texture and sound files that nothing references are not reported.

## 14. Layout fixes: plan, apply, undo

The layout check only reports. Four commands carry out what it suggests: `project_layout_fix_plan` lists the changes, `project_layout_fix_apply` carries out the selected ones, `project_layout_fix_undo` reverses an apply and `project_layout_fix_history` lists the applies ([ADR 0046](../adr/0046-layout-fixes-with-an-undo-journal.md)). The code is `rimstudio-toolkit::project::{fix, fix_apply, undo, history, journal}` and `rimstudio-io::rename`.

### 14.1 The plan

`project_layout_fix_plan { projectId, fixes? }` builds a list of items from the issues of the check; `fixes` limits it to the given issue codes (and the items they need). Nothing is written. An item has an id (`fix-<number>-<hash>`), a kind, the issue code it carries out, `from` and `to` (relative to the project root), a plain `why`, a risk, `applicable`, a `reviewReason` when it is not, the exact edit as a unified `diff` for `LoadFolders.xml`, a `conflict` (the destination exists, with a numbered name next to it), the `references` found and `requires` (ids of items that apply together with it). The plan id is a hash of every item, the state of every file the items touch and the text of `LoadFolders.xml`; it does not depend on `fixes`.

| Kind | Issue | What the item does |
|---|---|---|
| `create-folder` | `layout.missing-folder` | create the empty standard folder |
| `create-load-folders` / `edit-load-folders` | `layout.ce-outside-gate`, `layout.ce-folder-ungated` | gate the Combat Extended folder: an entry with `IfModActive="ceteam.combatextended"` is added to the block of the game version (and to `default` when that block is the newest) by span edit; an existing entry for the folder without a condition gets the condition in place. A project without the file gets one that keeps loading the root, `Common` and the version folder the game loaded without it, plus the gated folder |
| `move-file` | `layout.ce-outside-gate` | the patch moves to `<CE folder>/Patches/<name>`; the item requires the gate item |
| `move-file` | `layout.weapon-misplaced`, `layout.def-wrong-category` | the file moves to the path of its category (`rimstudio`: `<Category>/<DefName>.xml`; `core-style`: the category file; `flat`: the same name in the weapon folder) when one file holds one weapon (or, for `core-style`, weapons of one category) |
| `move-folder` and `edit-load-folders` | `layout.ce-legacy-folder` | `CE` (or `CombatExtended`) becomes `Compat/CombatExtended` and the `LoadFolders.xml` entries that name it follow; the two items require each other. Patches moved into the older folder by the same apply travel with it |
| `move-folder` | `layout.folder-case` | `defs` becomes `Defs`, a case only rename of the same entry |

Items are carried out in this order: folders to create, the gate, weapon moves, patch moves, the folder rename and its entry, case renames. A **safe** item can be applied and undone. An item that **needs review** is listed with a reason and never applied:

- a texture or sound folder (definitions name those files by path); the references found in the project's definition and patch files are listed;
- a move whose old path other definition or patch files mention (`references`);
- a file that cannot be moved whole: several weapons that belong to different places, a patch that also holds operations that do not use Combat Extended (moving it would load them only with Combat Extended);
- a move that changes the game versions a file loads for (the root and `Common` load for every version, a version folder for that version only);
- a destination that the path rules refuse (a link, a case twin, a protected folder, an unsafe name), a `LoadFolders.xml` that cannot be read or edited or that does not list the destination folder, and a standard folder that already exists where an older one would be renamed to.

### 14.2 The apply

`project_layout_fix_apply { projectId, planId, items: [{ id, renameOnConflict }] }` is a job. It builds the plan again and refuses a plan id that no longer matches (`designer.plan-stale`), so a file edited by another program since the review is never moved. Then:

1. Each selected item that is not in the plan, not applicable, in conflict (unless `renameOnConflict` accepts the numbered name) or whose required item is not selected is skipped with a reason.
2. The undo journal is written to the data root (`project-fix-journal/<projectId>/<applyId>.json`) before the first change: every item with its from, to, the hash of what is moved and the folders the move will create. It is rewritten before each `LoadFolders.xml` edit (with the previous text) and after each item.
3. Every source and destination goes through the guarded writer (inside the root, no link that leaves it, no case twin, names safe on every platform, outside the protected folders). A move is a rename inside the project root that never overwrites (`rimstudio-io::rename::move_path`: a hard link first where the file system has them, so two writers cannot overwrite each other); across volumes it is copy, verify byte for byte, remove. A hash taken just before the move must still match.
4. The previous `LoadFolders.xml` is backed up in the data root (`project-backups`, D-090) before it is replaced; the new text is written atomically and read back.
5. Cancellation is observed between items. The items done stay done and appear in the journal.
6. An item that fails is reported as skipped and the rest continue; an item whose required item was not applied is skipped.
7. The layout check runs again and its result is part of the response, with the moved paths, the edited files and the skipped items.

### 14.3 The undo and the history

`project_layout_fix_undo { projectId, applyId }` reads the journal, checks its checksum, its project and every path, and checks the disk **before changing anything**: a moved path must still hold the recorded bytes (a folder: the same manifest of names and file hashes) and the original place must be free; `LoadFolders.xml` must hold the text the apply wrote. If any check fails the call is refused (`project.fix-undo-refused`, naming the path) and nothing changes. Otherwise the items are reversed from the last to the first; folders the apply created are removed again while they are empty (a folder that holds a file added since stays). An apply that was interrupted is handled from the disk: an item the journal still lists as pending counts as done when its destination holds the recorded bytes and its source is gone. A journal that is damaged, tampered with or from another project is refused (`project.fix-journal-damaged`), an unknown id is `project.fix-not-found`.

`project_layout_fix_history { projectId }` lists the journals newest first with the time, the number of items done and skipped, whether the apply was undone and whether an undo would succeed now (`undoPossible`, `undoBlocker`).

### 14.4 Commands

`project fix plan PATH [--only CODE]` prints the items and writes nothing. `project fix apply PATH (--all | --item ID...) [--rename-on-conflict] [--plan-id ID] [--yes]` prints what it would do and changes nothing without `--yes`. `project fix undo PATH APPLY_ID` and `project fix history PATH`. `--json` prints the responses.

### 14.5 The Project page

The Layout tab of the Project page carries out the plan (`apps/desktop/src/features/project`, `fix*` and `Fix*` files). A finding whose fix is a move, a `LoadFolders.xml` edit or the older Combat Extended folder rename has a Fix button; Fix all safe plans every finding. One dialog goes through four steps: the review (the applicable items with checkboxes, from and to, why, the risk, the `LoadFolders.xml` edit as a diff, the conflict with the choice of the numbered name, and the items for review only with the references found), the confirmation (what moves where, and that an undo journal and copies of edited files are written to the data folder), the run (the job progress) and the result (done, edited, skipped with reasons, the check afterwards, Undo). A ticked item brings the items it requires; an item whose destination exists stays unticked until the numbered name is chosen. History lists the journals with Undo where `undoPossible` and the reason where not. A plan id that no longer matches (`designer.plan-stale`) shows that nothing was applied and offers Review again. The page keeps no rule: the plan, its order, the risk and the conflicts come from the commands.

### 14.6 Evidence

On 2026-10-05 `real_fix.rs` planned, applied and undid the fixes over temporary copies of 18 mods of the owner (mods above 40 MB skipped, nothing written to the originals): 78 items done, every undo restored every byte. For Gewehr 41 and The Lone Wolf Weapon Package the plan is the creation of the missing folders, a new `LoadFolders.xml` that gates `Compat/CombatExtended` (for Lone Wolf as `Common/Compat/CombatExtended`, next to `/` and `Common`) and the move of the Combat Extended patch; after the apply the check reports no warning. In Ratchet & Clank Lombax Tech the two Combat Extended patches in the `1.2` folder are listed for review because the layout names the `1.4` folder, and one definition file is not split.

## 15. Mod basics: About.xml, preview image, LoadFolders.xml, version folders

A mod starts from the scaffold of section 9 (`project_create`, the new mod dialog: the standard folder tree, an `About/About.xml` with the name, author, package id, supported versions and a description placeholder). Everything the scaffold wrote can then be changed from the app without opening an editor: the About form, the preview image, the load folders and the version folders. The commands change the existing files as byte span edits, so a hand written file keeps its comments, its element order, its unknown elements, its line endings, its byte order mark and every field the person did not touch ([ADR 0052](../adr/0052-mod-basics-editing.md), D-180 to D-184).

### 15.1 The About model

`project_about_get` returns every basic: `name`, `shortName`, `author`, `authors`, `packageId`, `description`, `url`, `modVersion`, `modIconPath`, `supportedVersions`, `modDependencies` (package id, display name, Workshop URL, download URL, alternatives), `loadBefore`, `loadAfter`, `forceLoadBefore`, `forceLoadAfter`, `incompatibleWith`, `steamAppId`, and the `ByVersion` blocks (`descriptionsByVersion` and the relation lists of one game version) marked `advanced`. For every field it says whether the file has the element (`present`), so a form can tell a value from a game default. It also returns the SHA-256 of the file (`fileHash`), the text of the file (read only, cut at 262144 bytes), the elements the game does not read (never touched), the facts of `About/Preview.png` (with the image itself as a data URL on request, up to 2 MiB), the installed game version the findings compared against, and the findings.

A file that is not UTF-8 (UTF-16), not well formed, or larger than 1 MiB is shown (the reader follows the game: a file that is not well formed gives the defaults) and is `editable: false` with the reason; no edit rewrites a byte of it. A document type declaration is never expanded; the edits are byte splices.

### 15.2 Changes, preview and update

A change is one small operation: `set` and `clear` for a text field, `list-set`, `list-add`, `list-remove` and `list-move` for a list, `dependency-add`, `dependency-update`, `dependency-remove` and `dependency-move` for the dependencies (by package id, ignoring case), `by-version-list-set` and `by-version-description-set` for the advanced blocks. A request holds a list of them, applied in order. `project_about_preview` returns the unified diff and the complete model of the resulting text, with its findings, and writes nothing. `project_about_update` applies the same changes and writes.

Rules of the edits:

1. A text field is updated inside its element; a missing element is inserted at the conventional place: after the last element that conventionally comes before it (`name`, `shortName`, `author`, `authors`, `packageId`, `url`, `modVersion`, `steamAppId`, `supportedVersions`, `modDependencies`, `loadBefore`, `loadAfter`, `forceLoadBefore`, `forceLoadAfter`, `incompatibleWith`, `modIconPath`, `descriptionsByVersion`, `description`), else before the first that comes after it. The new line uses the indentation and the line ending of the file.
2. Setting an empty text clears the field: the element is removed, with every copy of it (the game reads the first one, so a duplicate left behind would take over).
3. A list that is emptied loses its element, except a list the file already had empty, which stays as it was. A list entry is added at the end or at a position, not twice (ignoring case); removing the last entry removes the element. Moving an entry swaps text spans and leaves the neighbours and their comments where they are.
4. A change that changes no byte writes nothing and makes no backup.
5. Values are checked: a single line field holds no line break or control character, a package id is not empty, a version key is `major.minor`, a field is at most 100000 characters, a request has at most 500 changes. A change that does not apply (an entry that is not there) is refused with `project.edit-invalid` and nothing is written.

The write goes through the guarded writer: the replaced file is copied to `<data root>/project-backups/<project id>/About/` first (never into the mod folder, D-090), the bytes go down atomically, and the file is read back and compared. `expectedHash` is the `fileHash` the form read; when the file differs now the call is refused with `project.file-stale` (and the writer checks the old text again right before the bytes go down). The answer of an update carries the fresh model, the diff, the backup path and `verified`. The project record (name, package id) is refreshed from the written file.

### 15.3 The findings

Findings never block a save; the only things that do are the three refusals of 15.1 (`project.file-not-editable`). Each has a stable `about.*` code and a `field` pointer (`/packageId`, `/supportedVersions/1`, `/modDependencies/0/steamWorkshopUrl`).

| Code | Severity | Finding |
|---|---|---|
| `about.name-missing`, `about.author-missing` | warning | the name or the author is missing |
| `about.package-id-missing` | error | the package id is empty |
| `about.package-id-format` | error | the game's rule: at most 60 characters, ASCII letters, digits and dots, no leading dot, at least one dot, no two dots in a row, an alphanumeric last character; the reason is named |
| `about.package-id-case` | hint | capital letters (the game compares ignoring case) |
| `about.package-id-steam-suffix` | warning | the id ends in `_steam` or `.steam` |
| `about.package-id-ludeon` | warning | the word `ludeon` in an unofficial id |
| `about.package-id-collision`, `about.package-id-case-conflict` | warning | another mod of the last scan outside the project has the same id (or the same id in other letter case); the message names the mod and its folder, the arguments carry both |
| `about.supported-versions-empty` | warning | the list is missing or empty |
| `about.supported-versions-malformed` | error | an entry the game does not read (`v1.6`, `1`) |
| `about.supported-versions-build` | info | `1.6.4871` counts as `1.6` |
| `about.supported-versions-missing-game` | warning | the installed game version (`1.6`) is not named; only when a scan found the game version |
| `about.dependency-package-id-format`, `about.dependency-no-display-name`, `about.dependency-no-url` | warning | a dependency the game drops: a malformed id, no display name, or neither a Workshop URL nor a download URL (official ids need neither) |
| `about.dependency-url-malformed`, `about.url-malformed` | warning | not an `http`, `https` or `steam` address with a host |
| `about.dependency-unknown` | info | no mod with that id (or any alternative) in the last scan |
| `about.self-reference` | warning | `loadAfter`, `loadBefore`, `incompatibleWith` or a dependency names the mod itself |
| `about.load-order-contradiction` | warning | an id in both `loadBefore` and `loadAfter`, or a dependency that is also in `loadBefore` or `incompatibleWith` |
| `about.icon-missing` | warning | `modIconPath` names no texture below `Textures` of the mod, `Common` or a version folder |
| `about.preview-missing` | warning | no `About/Preview.png` |
| `about.preview-invalid` | info | the file is not a PNG by its header |
| `about.preview-too-large` | warning | above 1 MB (Steam refuses it) |
| `about.preview-dimensions` | warning, info | not 640 by 360: a warning, only a note when the picture has the 16 to 9 ratio |
| `about.preview-case` | warning | spelled other than `Preview.png` |
| `about.description-empty`, `about.description-long`, `about.description-size-tag` | warning, warning, info | empty; above 8000 characters (Steam keeps 8000); a `<size=` tag |
| `about.unparseable`, `about.not-utf8`, `about.too-large`, `about.leading-whitespace` | error, warning, error, error | the file cannot be read (the game falls back to defaults), is not UTF-8, is above 1 MiB, or has white space before `<?xml` |

The reader's own findings (`about.tag-case`, `about.unknown-tag`, `about.field-dropped`, `about.duplicate-field`, `about.duplicate-version-key`, `about.dependency-no-id`) are part of the list. The comparison with other mods needs a library scan; `project_about_get` never starts one (with no scan the comparing findings are left out).

### 15.4 The preview image

`project_about_set_preview {projectId, sourcePath}` copies a PNG to `About/Preview.png` (the spelling of an existing file is kept, so a case twin is never created). The source must be an absolute path to a regular file, never a link, at most 8 MiB, a complete PNG with a readable header and at most 4096 pixels on a side. The copy has the guarantees of the asset import (ADR 0047): a different file already there is backed up first, the copy is hash checked and read back. The answer carries the findings of the image (not 640 by 360, above 1 MB). `project_about_remove_preview` copies the file to the backup folder, then removes it. `About/Manifest.xml` and `About/PublishedFileId.txt` are never created, changed or removed by any of these commands.

### 15.5 LoadFolders.xml and version folders

`project_load_folders_get` lists the blocks and entries by position (the game merges repeated blocks, so a name is not an address) with `IfModActive`, `IfModActiveAll` and `IfModNotActive` as lists, the attributes the game does not read (`IfModActiveAny`), and whether each folder exists; with the raw text, the supported versions and the version folders of the root. Findings (never blocking): `loadfolders.folder-missing`, `loadfolders.unsupported-version` (a block for a version `supportedVersions` does not list), `loadfolders.ungated-compat-folder` (a `Compat`, `CE`, `CombatExtended` or `ModPatches` folder without an `IfModActive` condition), `loadfolders.ignored-attribute`, `loadfolders.empty-block` (the game then loads nothing), `loadfolders.duplicate-block`, `loadfolders.unparseable`, `loadfolders.too-large`, `loadfolders.not-utf8`.

`project_load_folders_update` applies `add-block`, `remove-block`, `add-entry`, `remove-entry`, `move-entry` and `set-entry` (folder, the three conditions, and removal of the attributes the game ignores) as byte span edits with the same write path, hash check and backup as the About update; `dryRun` returns the diff and the result without writing; `create` makes the file when the mod has none (an empty `loadFolders` root the edits fill).

`project_version_add {projectId, version, standardFolders, addBlock, dryRun}` creates the folder `major.minor` and, with `standardFolders`, the standard sub folders of a version folder (the ones the scaffold plan of a versioned mod lists for a version: the definition, patch, texture and sound folders). It copies and moves no file. When the mod has a `LoadFolders.xml` (or `addBlock` asks for one) it adds a block for the version: the plain root and `Common` entries of the nearest lower block, else `Common` when the folder exists, then the new folder. The block is written last, so a stop leaves folders only. The answer says when the version is missing from `supportedVersions`; the call does not edit About.xml. A folder or a block that exists is left as it is and reported.

### 15.6 Library search

`library_mod_search {query, limit}` searches the last scan by name, package id, author and folder name (every word must match; best match first; at most 100 rows) and returns, per mod: package id, name, authors, the kind of source, the Workshop item id and page (from the folder name or `PublishedFileId.txt`), the folder, whether the game can load it and its own `url`. A dependency row of the form is filled from it (package id, display name, Workshop URL). It never scans: with no scan the answer is `scanned: false`, empty, with a hint. The command line scans first, because each run starts without a scan (`library search`).

### 15.7 Requirements and acceptance

| Id | Requirement | Acceptance |
|---|---|---|
| ML-080 | Every basic of the mod is returned with whether the file has it, the hash and the findings | `get_returns_every_basic_with_its_place_in_the_file` |
| ML-081 | An edit changes only the span of the field: comments, order, unknown elements, line endings and a byte order mark stay; a missing field is inserted at its conventional place | `about_edit.rs` (xml), `update_writes_a_span_edit_backs_up_and_verifies` |
| ML-082 | Random edit sequences over five file styles keep the file readable and equal to a model of the edits, keep comments and the encoding frame, and an edit that sets what is there is byte identical | `random_edit_sequences_*`, `an_edit_that_sets_what_is_already_there_is_byte_identical` |
| ML-083 | A file that changed since it was read is refused; a change that does not apply is refused; neither writes | `a_file_that_changed_since_it_was_read_is_refused`, `a_change_that_does_not_apply_is_refused_and_writes_nothing` |
| ML-084 | Hostile files (not XML, UTF-16, a document type declaration with entities, above 1 MiB) are shown and never rewritten | `hostile_files_are_shown_but_never_edited` |
| ML-085 | The findings carry the codes and pointers of 15.3, name the colliding mod and its folder, and compare against the installed game version | `findings_name_the_colliding_mod_and_the_installed_game_version`, `about_lint.rs` unit tests |
| ML-086 | The preview image is copied only when it is a complete PNG, backed up when replaced, removable, and warns when not 640 by 360 | `the_preview_image_is_copied_checked_replaced_and_removed`, `a_linked_preview_source_is_refused` |
| ML-087 | `LoadFolders.xml` is read by position with findings and edited in place; created only when asked; a stale file is refused | `load_folders_are_read_with_their_findings_and_edited_in_place`, `load_folders_are_created_only_when_asked` |
| ML-088 | A version folder is created with its standard folders and its block, folders only, idempotent, `dryRun` writes nothing | `a_version_folder_gets_its_standard_folders_and_its_block` |
| ML-089 | The library search finds mods by name, package id and author and works without a scan | `query.rs` unit tests, `library_search_scans_first_and_finds_mods_by_name` |
| ML-090 | The DTOs have golden JSON and generated TypeScript types that match it | `golden_about.rs`, `pnpm --filter rimstudio-ipc-types bindings:check` |
| ML-091 | A no-op update of ten real About files keeps every byte; the findings run over all of the owner's About files | `real_about.rs` (ignored; needs `RIMSTUDIO_GAME_DIR`, `RIMSTUDIO_WORKSHOP_DIR`, `RIMSTUDIO_CUSTOM_DIR`) |

### 15.8 Evidence

On 2026-10-05 `real_about.rs` read 766 About files of the owner's install, Workshop and own mods (all parsed) and ran the findings with the scan as the neighbour list: 589 mods have a capital letter in the package id (hint), 485 have a preview image that is not 640 by 360 (most are 16 to 9 at other sizes), 471 dependencies carry the Workshop link form `steam://url/CommunityFilePage/<id>` (accepted, as is a placeholder such as `https://nope`: the game takes any text there), 186 have no `About/Preview.png`, 123 do not name game 1.6, 31 descriptions have a size tag, 24 have an empty description, 19 previews are above 1 MB, 16 package ids collide with another folder (mostly the owner's own backup copies), 14 previews are not PNG by their header, 4 dependencies are not in the scanned library, 3 dependencies have no link (the game drops them), 2 ids differ from another mod only by case, and one id ends in `.steam`. A no-op update (every field set to what it says) of ten of the files, copied to temporary folders, left every byte identical.

### 15.9 The Mod page

The Project page is the Mod page (the route is still `#/project`; the rail says Mod). Its parts:

1. **The hub.** With no mod open it shows two cards: Create a new mod (opens the creation window) and Open a mod (the folder picker, the mod folders of Setup as shortcuts into the picker), and the recent mods below.
2. **The header and the tabs.** With a mod open: the name, package id, folder, supported versions and status chips (layout convention, `LoadFolders.xml`, layout issues, counts), then the tabs Basics, Versions and folders, Files, Layout and Test in game. A mod opens on Basics. The Basics and Versions and folders tabs show the number of unsaved edits, which stay when another tab is shown.
3. **The creation window** has three steps. Identity: the folder (the picker and the mod folders of Setup as quick choices), name, author, package id (suggested from author and name until typed), the game versions as checkboxes (the installed version and the three before it), the folder name and the description; the findings of `project_scaffold_preview` show as you type (a malformed id, a collision with a mod of the last scan, an empty name) and Next waits for a valid answer. Structure: the switches of section 9 and a tree of the entries the backend would write, each with a sentence about what it is for. Review: the summary, the findings, and the exact list of folders and files; Create writes them (`project_create`) and opens the mod on Basics.
4. **Basics** is the About form in sections (Identity, Description, Game versions, Dependencies, Load order and conflicts, Images, Advanced). The form keeps a draft; after a pause the draft goes to `project_about_preview` and the findings of the result show next to their fields (an error under the field, the others as lines with their severity in words), with the totals on top. Dependencies are added from the library search, by hand (a row with no package id yet is not sent) or with a one click Combat Extended entry (dependency and load after) when the last scan has it. A bar at the bottom shows the unsaved count with Review changes (the unified diff of the preview), Save (`project_about_update` with the file hash) and Discard, and says that the old file is copied to the backup folder first; after a save it names the backup. A refusal for a stale file shows a Reload button that reads the file again and keeps the draft. The preview image is chosen with the platform file picker and saved at once through `project_about_set_preview` (a notice says the old image is backed up); removing it asks first. The Advanced section shows the per version blocks read only, and the text of `About.xml`.
5. **Versions and folders** shows, per supported version, whether its folder exists and whether `LoadFolders.xml` has a block; the blocks as cards of folders in load order with the folder check, the three conditions (with the library picker), move and remove, and the attributes the game ignores with a button that drops them. Every edit is checked with a dry run of `project_load_folders_update` and the tab shows the answer; the bar has the same Review changes, Save and Discard. A mod with no `LoadFolders.xml` shows a button that creates it (saved with the first edit). Add a version folder opens a window that shows, from a dry run of `project_version_add`, which folders and block it would create.

#### `project_scaffold_preview`

The request is the one of `project_create`. The answer holds `root`, `valid`, `diagnostics` (findings with a `field` pointer: `/name`, `/packageId`, `/supportedVersions`, `/target`), `entries` (path and kind, sorted, empty when not valid), `targetExists` and `conflicts` (planned files that exist with other content). The package id findings are those of section 15.3 (the format rule in the game's words, the hints, the collision with the last scan); nothing is written and no scan is started.

| Id | Requirement | Acceptance |
|---|---|---|
| ML-092 | The scaffold preview lists the entries of the plan, reports the findings of the values and the files that exist, and writes nothing | `a_default_spec_lists_its_entries_and_has_no_findings`, `a_bad_package_id_is_an_error_and_lists_no_entries`, `capital_letters_are_a_hint_and_existing_files_are_conflicts` |
| ML-093 | The creation window asks the backend while the person types and keeps Next off until the values are valid | `NewModDialog.test.tsx`, `createStore.test.ts` |
| ML-094 | The Basics tab edits only what the person changed; the saved file equals the old file with those spans replaced | `mod-basics.spec.ts` (one field, the owner mod copies), `aboutModel.test.ts` |
| ML-095 | A stale file is refused with an explanation, a reload keeps the draft, and the save then succeeds | `mod-basics.spec.ts`, `aboutStore.test.ts` |
| ML-096 | The Versions and folders tab edits `LoadFolders.xml`, creates it only when asked, and adds a version folder with its block | `FoldersTab.test.tsx`, `mod-basics.spec.ts` |
| ML-097 | The new views have no axe violations at 1440 by 900 and fit 1024 by 700 | `mod-basics.spec.ts` |
