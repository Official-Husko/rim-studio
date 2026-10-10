# ADR 0061: Editing a weapon of the project where it stands

Status: accepted | Last updated: 2026-10-11 | Register: D-214

## Context

A weapon the open mod already defines could not be loaded into the designer. Clone and adjust ([ADR 0042](0042-generation-fidelity-of-a-clone.md)) reads its source from the reference session, which holds the game and its expansions only, so a weapon of the project was refused with "no reference data". The Weapons page showed the drafts of the project and the reference weapons, but nothing about the weapons the loaded mod actually contains. The owner asked on 2026-10-10 for the ability to load an already made weapon and for the details of a loaded mod in the Weapons tab.

## Decision

1. A weapon the open project defines can be listed and opened as a draft that edits the definition where it stands (D-214). `designer_project_weapons` lists the weapons with their numbers; `designer_open_weapon` stores a draft that carries `editsFile`, the project file of the definition.
2. The project is read through a session over the reference set without Combat Extended, even when Combat Extended is loaded, so the numbers are the ones the base game loads and a conversion patch does not leak into what the editor shows.
3. The draft reads the definition as its file writes it (the reader of the clone flow), so fields the designer does not model stay as raw fields and are written back unchanged. The role comes from the nearest reference weapon by the numbers.
4. Planning such a draft points the file that holds the definition at `editsFile`, and the merge brings the definition to the new meaning by small edits: a changed value is replaced where it stands, a new member is appended, a dropped member is removed, comments, the order of the members and the number format stay, and a definition that already means the design is not touched. The weapon is defined once and every other byte of the file stays.
5. A weapon that exists keeps its name. A draft whose name is not defined in `editsFile` plans with an error (`designer.edits-name-changed`) and the Weapons page shows the name field read only; a draft whose file is gone plans with `designer.edits-file-missing`, and reading the weapon again brings the same draft to its new file.
6. A projectile defined in the same file becomes the weapon's own projectile (same name, not a copy), so its damage is edited; a projectile defined elsewhere stays a reference and a note says that its damage is not written from here.

## Consequences

- A second cached project session exists next to the Combat Extended one while both pages are used.
- The in place merge also applies to every other design whose definition already stands in the target file without a section comment of RimStudio: comments and order are kept there too.
- The command list gains `designer_project_weapons` and `designer_open_weapon`; drafts gain the optional `editsFile` (additive, the draft schema version stays 2).
- The optional Combat Extended block of such a draft stays off until the modder turns it on (D-085).

## Alternatives rejected

- Opening a project weapon as a clone with a new name: that makes a second weapon instead of editing the first.
- Reading the project through the Combat Extended session: inherited stats of vanilla parents are patched by Combat Extended, so a vanilla draft would carry converted values.
- Writing an edited weapon to the layout's path: the mod would define the weapon twice and the game reports duplicate definitions.
- Replacing the whole element in the file: it moves members, rewrites number formats and drops the comments of the person, so an untouched weapon would show a diff.

## Evidence

- [Items toolkit](../features/items-toolkit.md) section 15.15, [decision register](../architecture/decision-register.md) D-214.
- Tests: `crates/rimstudio-toolkit/tests/project_weapons.rs`, the unit tests of `shared::merge_element`, `tests/e2e/specs/18-project-weapons.spec.ts`. Checked on a copy of the owner's Gewehr 41 mod with the owner's install: an unedited weapon plans as unchanged and a changed damage and range give a two line diff.
