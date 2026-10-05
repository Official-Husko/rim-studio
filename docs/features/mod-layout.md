# Mod layout

Status: as built for 0.1.0 | Last updated: 2026-10-05 | Requirement ids: ML-001 to ML-050 | Decisions: D-104 to D-109, [ADR 0041](../adr/0041-rimstudio-mod-layout-v1.md)

This document specifies the RimStudio mod layout v1: where the files of a mod live, how the tool names and groups them, how it recognises the convention of a mod that already exists, and what it never moves. It covers the scaffold of a new mod, the placement of generated files (weapon and projectile definitions, sound definitions, texture paths, the gated Combat Extended folder, `LoadFolders.xml`), the layout check, the annotated project tree and the file viewer command. The code is in `rimstudio-workspace` (`scaffold`), `rimstudio-design` (`plan::layout`) and `rimstudio-toolkit` (`project`, `shared::projectfs`).

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

1. A weapon without a texture path of its own gets the reserved one, `Things/Item/Equipment/WeaponRanged/<DefName>` (`WeaponMelee` for melee), written into its `graphicData`. The plan carries the info diagnostic `design.texture-reserved` naming the file where the art goes (`Textures/Things/Item/Equipment/WeaponRanged/<DefName>.png`). The tool creates no image. A typed texture path is kept as typed.
2. Projectile textures reserve `Things/Projectile/<DefName>`; sound definitions go to `Defs/SoundDefs/World_Oneshots_Weapons.xml` (shots), `Reload_Oneshots_Weapons.xml` and `World_Oneshots_ProjectileImpacts.xml`, with clips in `Sounds/Weapons/<DefName>_Shot`, `_Reload` and `_Impact`; the `clipFolderPath` is `Weapons/<DefName>_Shot` (relative to `Sounds`). The 0.1.0 designer writes no sound definition; the path rules exist so the next extension places them without a new decision.
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
| Convert flow, update mode | `<CE folder>/Patches/...` and `LoadFolders.xml` only; no definition file of the project is touched |

Examples:

| Project | Weapon `RS_Rifle`, Industrial, ranged | With Combat Extended |
|---|---|---|
| New mod, flat | `Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Rifle.xml` | `Compat/CombatExtended/Patches/<slug>_Weapons_Ranged.xml`, `LoadFolders.xml` |
| Versioned `1.6` | `1.6/Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Rifle.xml` | `1.6/Compat/CombatExtended/Patches/...`, `LoadFolders.xml` with `1.6/Compat/CombatExtended` |
| Game style (Gewehr 41) | `Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml`, two sections appended | the gate and folder as above, unless the mod has an older folder |
| Flat (Lone Wolf) | `Common/Defs/ThingsDef_Misc/Weapons/RS_Rifle.xml` | `Common/Compat/CombatExtended/Patches/...` |
| Older folder `CE` gated | as the profile says | `CE/Patches/...`; `LoadFolders.xml` already gates it and is reported unchanged |

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

Only the creation of a missing folder is automatic. A fix that moves, renames or edits a file is always a suggestion (section 11). At most 500 issues are reported.

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
| `project_scaffold_missing` (`project scaffold-missing`) | action | Section 9 |
| `project_read_file` (`project read`) | query | One text file by relative path for the file viewer: the path goes through the guarded writer's checks, at most 262144 bytes by default (1048576 at most), UTF-8 without the byte order mark, `binary` for a file with a NUL byte or invalid UTF-8, `truncated` and the full size otherwise. A folder, a missing file, a link that leaves the root and an unsafe path are errors (`project.path-outside-root`, `io.not-found`) |

The designer's plan, apply and convert use the same layout and report paths relative to the project root (`designer_export_plan`, `designer_apply_plan`, `designer_convert_scan`).

## 11. What the tool never moves or deletes

1. No command moves, renames, rewrites or deletes an existing file or folder of a mod because of the layout. Findings are suggestions; the user moves files.
2. A plan changes an existing file only by byte span edit of one marked section or by adding a section (the update region mechanism), after a backup outside the mod folder, and only a file named in the plan.
3. The older Combat Extended folder, a flat weapon folder and category files of the game's style stay where they are; new files follow the profile.
4. `project_scaffold_missing` creates folders only. Links are never followed by the scan or the reader.
5. The game install, the Workshop folders and the reference mod folders are never written (the protected paths and the write fence of ADR 0021).

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

## 13. Evidence from the owner's mods and open points

The ignored test `real_layout.rs` ran over temporary copies (without `.git`, `Source` and `Raw Assets`) of the owner's two weapon mods on 2026-10-05. Gewehr 41: profile `core-style`, 5 weapons and 1 projectile in one category file, 10 textures, findings: `layout.ce-outside-gate` for `Patches/ce_patch.xml` (a warning) and two missing standard folders (`Defs/SoundDefs`, `Sounds`, info). Lone Wolf: profile `flat` with the content in `Common`, weapon folder `Common/Defs/ThingsDef_Misc/Weapons`, 9 weapons and 5 projectiles, 44 textures, 6 sounds, findings: `layout.ce-outside-gate` for `Patches/CE_Patch.xml` and one missing standard folder (`Common/Defs/SoundDefs`, because the mod's folder is named `SoundDef`). Neither mod was written to.

Open points:

1. Apparel files: the layout reserves `Defs/ThingDefs_Misc/Apparel` style names for the apparel designer, which follows 0.1.0; the category rule for apparel is not decided here.
2. The sound definition files of section 5.3 are reserved; the designer writes none yet.
3. A command that carries out a suggested move (with a backup and a plan like the designer's) is a possible later action; it would be explicit and per file.
4. Texture and sound files that nothing references are not reported.
