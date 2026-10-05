# ADR 0046: Layout fixes with an undo journal

Status: accepted | Last updated: 2026-10-05 | Register: D-123 to D-125

## Context

The layout check of [ADR 0041](0041-rimstudio-mod-layout-v1.md) reports where a mod differs from the layout and suggests a fix, but carries out only the creation of an empty folder. The owner asked for the check to be able to carry out its fixes. The risks are concrete: a Combat Extended patch moved into a gated folder is never loaded when `LoadFolders.xml` does not list that folder, a texture or sound moved away from the path a definition names breaks the definition, a rename can overwrite a file, a link in a path can lead out of the mod, and a user who dislikes the result has to put every file back by hand. The write path review ([security and privacy](../architecture/security-and-privacy.md) section 19) found that the planned writes of the designer needed a stale plan check, a guard on every path and a backup; moves need the same and an undo.

## Decision

1. Carrying out a fix is a plan, then an apply (D-123). `project_layout_fix_plan` lists the changes without writing; each item has an id, a kind, from and to, a reason, a risk, the exact `LoadFolders.xml` diff, the conflict and the references found. The plan id is a hash of the content, and `project_layout_fix_apply` refuses an id that is no longer current, like the designer's plan. Only a plan the caller reviewed and items the caller selected are applied.
2. Only safe items are applied (D-123). Textures and sounds, a move whose old path other files mention, a file that cannot be moved whole, a move between game version folders and anything the path rules refuse are listed as needing review with their references and are never applied. A move never overwrites: a destination that exists is a conflict, refused by default, with a numbered name that is used only when the caller accepts it. A Combat Extended patch moves only together with the item that gates its folder.
3. Every apply writes an undo journal first (D-124). The journal lives in the data root (`project-fix-journal/<projectId>/<applyId>.json`), never in the mod, records every move with the hash of what is moved, every edited `LoadFolders.xml` with its previous text and hashes, and every folder the apply created, and is rewritten before each edit and after each item, so an interruption at any point leaves a journal that an undo can use. The journal has a checksum against damage; it is also checked against the project and the disk, because a checksum is not a secret. `project_layout_fix_undo` checks everything before it changes anything and refuses, naming the path, when a moved path no longer holds the recorded bytes or the original place is taken; `project_layout_fix_history` lists the journals.
4. Moves go through one guarded primitive (D-125). `rimstudio-io::rename::move_path` takes a `RootGuard`, never follows or moves a link, never overwrites (a hard link first, so a competing writer cannot be overwritten), allows a letter case only rename of the same entry, and across volumes copies, verifies byte for byte and only then removes the source. The toolkit adds the writer's checks (case twins, device names, protected folders, the path length) on every source and destination, and the previous `LoadFolders.xml` is backed up in the data root before it is replaced (D-090).
5. A new `LoadFolders.xml` keeps the game's own loading: the root, `Common` and the version folder, plus the gated Combat Extended folder, in the block of the newest version and in `default`. An existing file is edited by span edit with the same helpers as the designer's Combat Extended plan, so every other byte stays.

## Consequences

- Four registry rows (`project_layout_fix_plan` query, `project_layout_fix_apply` job, `project_layout_fix_undo` action, `project_layout_fix_history` query), three error codes (`project.fix-not-found`, `project.fix-journal-damaged`, `project.fix-undo-refused`), additive DTOs and generated types, the CLI `project fix plan|apply|undo|history`.
- Decision D-107 ("no existing file is moved without an explicit action") stands: the explicit action is the reviewed plan with the selected items.
- A file that defines several weapons is not split and a folder is not merged; the user does those by hand. A move whose effect on references cannot be judged is left to the user.
- The journal stays until the user removes it; there is no automatic expiry in 0.1.0.

## Alternatives rejected

- Moving files from the check itself: no review, no id, no stale detection.
- A backup copy of every moved file instead of a journal: a move changes no bytes, so a copy doubles the mod; the hash in the journal proves what the undo reverses.
- Letting the undo force: a file edited since the apply is the user's work; refusing is the only safe answer.
- Rewriting `texPath` and `clipPath` references together with a texture move: the tool does not edit definitions it did not write; it lists them.

## Evidence

- [Mod layout](../features/mod-layout.md) section 14
- [Security and privacy](../architecture/security-and-privacy.md) sections 19 and 20
- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md) section 3
