# ADR 0038: Toolkit shared module, guarded writer and project backups outside the mod folder

Status: accepted | Last updated: 2026-10-05 | Register: D-089, D-090

## Context

ADR 0004 says that a tool module of `rimstudio-toolkit` never uses another tool module. The designer and the project tool both need the same project view, the same guarded writer and the same `LoadFolders.xml` merging. Duplicating them in two modules would let the two copies drift, and the write path is the one place where a divergence is dangerous. The designer also replaces existing files in a mod folder, and a backup written beside the file would be uploaded to the Workshop with the mod.

## Decision

1. The toolkit has one neutral module `shared` (submodules `env`, `writer`, `projectfs`, `merge`, `diff`) that no tool owns and that every tool may use. The rule of ADR 0004 stays: a tool module never uses another tool module; the `xtask` grep treats `shared` as allowed. `shared::writer::GuardedWriter` is the only write path of the toolkit: it refuses unsafe paths, paths outside the project root and the protected paths (the game install of Core and the DLC, the reference mod roots, explicit folders; an optional `GameWriteFence` adds to this).
2. Backups of project files that the designer replaces are written to `<data root>/project-backups/<projectId>/<folder of the file>/` under the `user_data` retention policy (the last 10 plus one per day for 14 days, each read back), never into the mod folder. The apply report gives the backup path as an absolute path string.

## Consequences

- The project tool re-exposes only a summary of the shared `LoadFolders.xml` helper (`load_folders_summary`); the merge logic exists once.
- Backups accumulate under the data root until a clean up action exists; the policy is not configurable yet.
- A Workshop upload of a mod folder never contains RimStudio backups.
- Tests of the writer cover refusal of outside root and protected paths before the first write of any plan.

## Alternatives rejected

- Letting the designer use the project module: breaks ADR 0004 and couples the tools.
- A copy of the helpers in each module: two write paths to keep safe.
- Backups beside the file or in a hidden folder of the mod: they would ship with the mod.

## Evidence

- [ADR 0004](0004-no-feature-edges-and-toolkit-registry.md)
- [Items toolkit](../features/items-toolkit.md) section 15
- [Data and persistence](../architecture/data-and-persistence.md) section 8.3
- [Crate catalog](../architecture/crate-catalog.md) rimstudio-toolkit
