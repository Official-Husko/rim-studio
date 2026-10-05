# ADR 0041: RimStudio mod layout v1

Status: accepted | Last updated: 2026-10-05 | Register: D-104 to D-109

## Context

The first backend wrote weapons to `Defs/Weapons/<DefName>.xml`, projectiles to `Defs/Projectiles/<DefName>.xml` and Combat Extended patches to `CE/Patches/`. Those names were chosen for the first plan builder and follow nothing in the game. The owner asked for "proper mod folder setups in a clean way organized, based on how RimWorld does it", with room to improve the organisation. A survey of the game (Core and four expansions), the owner's mods and the mod corpus ([mod layout](../features/mod-layout.md) section 2) shows that the game keeps weapons in `Defs/ThingDefs_Misc/Weapons` in one file per category (`RangedIndustrial.xml`), keeps each projectile beside its gun, keeps sound definitions in `Defs/SoundDefs`, and references textures by `Things/Item/Equipment/WeaponRanged/<Name>`. Mods in the wild use many other conventions, among them the game's own style and loosely named folders, and the Combat Extended folder has no common name (39 mods use `CE`, the others vary).

## Decision

1. The layout follows the game's names: `About`, `Defs/ThingDefs_Misc/Weapons`, `Defs/SoundDefs`, `Patches`, `Textures/Things/Item/Equipment/WeaponRanged` and `WeaponMelee`, `Textures/Things/Projectile`, `Sounds`, `Languages/English/Keyed`, `Assemblies`, `Source`, `Common` and version folders (D-104).
2. A new mod gets one file per weapon, holding the weapon and its own projectile, in a folder named like the game's category file: `Defs/ThingDefs_Misc/Weapons/RangedIndustrial/<DefName>.xml`. The category comes from the kind and the tech level (D-104).
3. The gated Combat Extended content lives in `Compat/CombatExtended` with its own `Patches` folder, selected by `LoadFolders.xml` with `IfModActive="ceteam.combatextended"`. A mod that has the folder `CE` or `CombatExtended`, or whose `LoadFolders.xml` already gates another folder, keeps it; the tool detects it and never moves it (D-105).
4. A weapon without a texture path gets the reserved `texPath` that mirrors the game's convention, with an info hint naming the PNG file; the tool creates no image. Sound definition, clip and art source paths are reserved by the layout (`Source/Art` for art that is never shipped) (D-106).
5. The convention of an existing mod is recognised from folder and file names: `rimstudio` (category folders), `core-style` (one file per category) or `flat` (one file per weapon in the folder the mod already uses). A new weapon follows the profile; for `core-style` it is appended to the matching category file as a marked section through the update region mechanism. Nothing existing is reorganised without an explicit action (D-107).
6. The scaffold writes the standard skeleton (About, definition, patch, texture and sound folders) and only what the user asks beyond it. `README.md`, `Credits.txt` and `.gitignore` are offered; `About/Preview.png` and `About/Manifest.xml` are not created silently (D-108).
7. Four commands serve the layout: `project_tree`, `project_layout_check`, `project_scaffold_missing` (folders only) and `project_read_file` (D-109).

## Consequences

- The plan of a new weapon is one file per design (the first backend wrote two); goldens, the CLI tests and the status page follow.
- The Combat Extended folder name appears in `rimstudio-core::paths` (`CE_COMPAT_DIR`) and in `ProjectLayout`; the older name is a detected value, not a constant of the generator.
- Detection reads names only, so it is cheap and cannot be fooled by file content, but a mod that follows no convention is treated as `rimstudio` when it has no weapon folder and as `flat` when it has one.
- The check is advice. A user who wants an old mod reorganised moves the files; a later explicit, per file move command is possible and out of scope.
- `rimstudio-ipc-types` and the generated TypeScript gain the layout DTOs and four registry rows (42 commands).

## Alternatives rejected

- Appending every weapon to Core style category files for all projects: a second weapon edits a file the first one lives in, and every write becomes a merge.
- Keeping `Defs/Weapons` and `Defs/Projectiles`: names that no game file uses; a projectile separated from its gun.
- Keeping `CE` as the default: it is the commonest name but it says nothing and leaves no room for Combat Extended definitions beside patches; the detection keeps every existing `CE` folder working.
- Creating a `Preview.png` placeholder or a `Manifest.xml`: the first needs an image, the second belongs to the mod manager and the publisher, and both would be silent extras.
- Moving existing files to the layout automatically: it can break references in another mod's patches and in `LoadFolders.xml`, and the owner's mods follow conventions the tool should respect.

## Evidence

- [Mod layout](../features/mod-layout.md)
- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md) sections 2 and 3
- [Items toolkit](../features/items-toolkit.md) section 8
- [Combat Extended patching](../features/combat-extended-patching.md) section 9
- [ADR 0037](0037-vanilla-default-optional-ce-patch.md) and [ADR 0038](0038-toolkit-shared-module-and-project-backups.md)
