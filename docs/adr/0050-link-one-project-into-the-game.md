# ADR 0050: Link one project into the game for testing

Status: accepted | Last updated: 2026-10-05 | Register: D-170, D-171, D-039, D-040

## Context

The game reads only its own `Mods` folder, `Data` and the Steam Workshop. A mod made in a custom folder is invisible until something puts it in `Mods`, and the write fence of ADR 0021 forbids writing under the install except owned link entries and `ModsConfig.xml`. The full link farm (a plan for a whole load order, dry runs, the `ModsConfig.xml` write) is deferred, but the owner wants to test a mod in the game within minutes of making it. One link for one project needs only the part of the fence that already exists.

## Decision

1. Three commands: `project_link_status` (read only), `project_link_create` and `project_link_remove`. The entry is `<game install>/Mods/<project folder name>`, with the name made safe as one path component (D-170).
2. Nothing that exists is replaced. A real folder, a link RimStudio did not make, or a link that RimStudio made and that now points elsewhere is left alone and reported. Removal is only for an entry recorded in the ownership manifest that is a link pointing at the recorded target, or a marked copy whose marker the fence verified. A link is removed with the link primitive, never recursively; the project is never touched (D-170).
3. The ownership manifest is one JSON document per entry in the data root (`game-links`), written before the entry is created. A crash leaves nothing, or a record without an entry (dropped by the next create), never an entry that is not recorded (D-170).
4. A running game refuses `project_link_create` unless the request says the person will restart the game; `project_link_remove` is never refused for that reason. A hidden process table (a sandbox) reports `unknown` and does not block, because a new entry is only read at start up (D-170).
5. The mode is a symbolic link by default, a junction where the platform backend offers one, and a copy with the `.rimstudio.json` marker only on request, with a visible warning that a copy does not follow edits (D-170).
6. Refusals are values in the answer (code, message, and the state after), never errors, so the page shows the reason and the manual command (`ln -s` on Linux and macOS, `mklink /J` on Windows). A missing or read only `Mods` folder is such a refusal.
7. The fence needs two small additions: a link or copy source may not be inside the game folder (the folder that holds `Mods`) and may not contain it, and the ownership set of a shared fence can be replaced from the manifest (`set_owned`). Tests prove that every other write under the install is still refused and that no name can leave `Mods` (D-171).
8. The status also reads `ModsConfig.xml` through the existing codec, read only, to tell whether the project and Combat Extended are in the active list.

## Consequences

- `rimstudio-library` gains `deploy::{inspect, ops, manifest, command, codes}`; the rest of the link farm stays deferred and `deploy.rs` says so.
- `rimstudio-toolkit` gains `project::link`; `rimstudio-app` gains three registry rows and the `WorkspaceHub::link_machine` read; the CLI gains `project link status|create|remove`.
- The Project page gains the card "Test in RimWorld".
- Under the install, the app now writes one more kind of thing than before the card: a link named after a project. It is the same entry kind the fence already allowed for the link farm.
- A project whose folder is itself a link is linked by its real folder; one that resolves inside the game folder is refused.

## Alternatives rejected

- Writing a `ModsConfig.xml` entry as well: the person enables the mod in the game's list, which keeps the fence at one kind of write for this feature.
- A junction or symbolic link created without the fence: the fence is the only place allowed to write under the install.
- Creating the `Mods` folder when it is missing: it is the game's folder; the person creates it or runs the shown command.
- Copy by default: a copy goes stale and doubles the files.

## Evidence

- [Security and privacy](../architecture/security-and-privacy.md) section 9.6, [game and mod discovery](../features/game-and-mod-discovery.md) section 17.
