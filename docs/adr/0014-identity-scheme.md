# ADR 0014: Identity scheme for mods

Status: accepted | Last updated: 2026-10-04 | Register: D-029

## Context

Mods are keyed differently across features: packageId in rules, workshop id on Steam, folder path on disk. Using the path as identity loses notes when a mod moves, and packageId alone is not unique because duplicates across sources are a core manager concern. Sorting and deltas over thousands of mods also need compact handles that cost little to serialise.

## Decision

`PackageId` is a lowercased newtype that keeps the original text for display and export. `ModId` is a stable string: `w<workshopId>` or `<source>:<packageId>`. `ModIdx(u32)` and `FileId(u32)` are session handles used on the hot path (deltas, sort results, validation positions) and are never persisted. WorkshopId and every other 64-bit id cross IPC as strings because they exceed the safe integer range of JavaScript.

## Consequences

- Moving a mod across sources changes its `ModId`; the UI offers re-linking of notes and profile entries.
- Sort results travel as compact `ModIdx` arrays, which keeps messages small.
- Stored files never contain session handles, so a restart cannot corrupt references.
- Duplicate detection works on packageId while identity stays unambiguous.

## Alternatives rejected

- Folder path as identity: notes lost on move.
- Stable string ids on the hot path: slower, larger messages.

## Evidence

- [RimSort core domain](../research/rimsort-core-domain.md) implications 2 and 14
- [IPC and state](../architecture/ipc-and-state.md)
- [Webview and IPC performance](../research/webview-and-ipc-performance.md) section 2.3
