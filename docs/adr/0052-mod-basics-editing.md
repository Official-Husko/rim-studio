# ADR 0052: Editing the basics of a mod

Status: accepted | Last updated: 2026-10-05 | Register: D-180 to D-184

## Context

The owner asked that the tool allows a new mod setup with the recommended file tree, and has the tab to edit the basics of the mod directly or to make a new mod. The scaffold ([ADR 0041](0041-rimstudio-mod-layout-v1.md)) already makes the tree and a first `About.xml`. What was missing is changing the basics afterwards without leaving the app: name, authors, package id, description, supported versions, dependencies, load order, the preview image, `LoadFolders.xml` and the folder of another game version. Mod files are often written by hand (31 percent carry a byte order mark, many have comments and unknown elements), so a form that regenerates the file would destroy the author's work.

## Decision

1. The About file is edited by byte span edits of the existing text through `AboutEditor` (a layer over the span editor of `rimstudio-xml`). A form sends small changes (set, clear, list add, remove, move, dependency operations); the backend returns the unified diff and the resulting model without writing (`project_about_preview`), or writes (`project_about_update`) through the guarded writer: backup in the data root, atomic write, read back, refusal of a file that changed since the hash the form read, refusal of a change that does not apply. A change that changes no byte writes nothing (D-180).
2. The model says, for every field, whether the file has it. The `ByVersion` blocks are returned structured and marked advanced, and can be edited with their own operations. A file that is not UTF-8, not well formed or above 1 MiB is shown and never rewritten (D-180).
3. Findings are values with stable `about.*` and `loadfolders.*` codes and field pointers, never blocking. The comparison with other mods uses the last library scan as an input; the commands never scan. The package id rule is the game's (at most 60 characters, ASCII letters, digits and dots, a dot, no empty segment). A dependency the game drops (no display name, no link) is a warning (D-181).
4. The preview image is a guarded copy with the checks of the asset import: a complete PNG, a regular file, a size limit, a backup of a different file, a read back; not 640 by 360 is a warning (a note for the 16 to 9 ratio). Removing it keeps a copy in the backup folder. `Manifest.xml` and `PublishedFileId.txt` are never touched (D-182).
5. `LoadFolders.xml` is read by position (the game merges repeated blocks) and edited by positional span edits, created only when asked. A version folder is created with its standard sub folders (from the scaffold plan) and its block, folders only, the block written last, nothing copied or moved (D-183).
6. `library_mod_search` searches the last scan by name, package id, author and folder name and returns what a dependency row needs, including the Workshop page built from the item id. The command line scans first because each run starts without a scan (D-184).

## Consequences

- `rimstudio-xml` gains `about_edit` and `load_folders_edit`; `rimstudio-library` gains `query`; `rimstudio-ipc-types` gains `project_about` and three error codes (`project.edit-invalid`, `project.file-not-editable`, `project.file-stale`); `rimstudio-toolkit` gains `project::{about, about_apply, about_lint, about_preview, load_folders, versions}` and `GuardedWriter::remove_file`; `rimstudio-app` registers nine rows; `rimstudio-cli` gains `project about`, `project load-folders`, `project version` and `library search`.
- `SpanEditor::insert_child` no longer drops the comments of an element that holds only comments when its first child is added (found by the property tests of the editors).
- The writer can now remove a file, only after a verified backup, and only on request (the preview image).
- The scaffold is unchanged. A later page for a new mod and the basics tab use these commands.

## Alternatives rejected

- Regenerating `About.xml` from the model on every save: loses comments, ordering and unknown elements, and changes line endings and encodings.
- Failing a save on any finding: a mod that is half set up is the normal state of a mod being made.
- Scanning the library from `project_about_get`: a scan takes seconds and the Setup page owns it; the findings that need it are left out when there is none.
- Editing `PublishedFileId.txt` or `Manifest.xml`: the publisher owns the first, a mod manager the second.
