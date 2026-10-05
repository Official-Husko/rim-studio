# RimStudio architecture decision records

This folder holds one record per architecture decision that deserves a durable explanation. Each record condenses one or more entries of the [decision register](../architecture/decision-register.md), adds the context, the consequences and the alternatives that were rejected, and points to the research evidence. The [roadmap](../roadmap.md) says when each decision is built and which spikes confirm it.

Status: draft | Last updated: 2026-10-05

## 1. Index

Status values: accepted (decided, build on it), proposed (accepted for planning, needs a spike or measurement), needs-owner (a human decision is required; a placeholder is in use). The register column lists the D-numbers a record covers. Register entries with no record of their own stay in the register. A record that covers entries of mixed status carries the weakest of them (needs-owner, then proposed, then accepted) and its text says which parts are firm: 0010 (allocator D-069 is proposed), 0016 (D-035 proposed), 0021 (D-041 proposed), and 0031 (identifier D-049 and macOS notarisation need the owner; the Windows signing part is decided by D-087).

| ADR | Title | Status | Register |
|---|---|---|---|
| 0001 | [Virtual Cargo workspace, rename and naming](0001-workspace-rename-and-naming.md) | accepted | D-001, D-011, D-068, D-070 |
| 0002 | [Layer matrix and crate granularity](0002-layers-and-crate-granularity.md) | accepted | D-002, D-003, D-004 |
| 0003 | [Tauri-free composition root, one command registry, CLI package](0003-one-command-registry-and-app-root.md) | accepted | D-005, D-006, D-007 |
| 0004 | [No feature-to-feature edges and a compile-time toolkit registry](0004-no-feature-edges-and-toolkit-registry.md) | accepted | D-008, D-066 |
| 0005 | [Ports in core and a cfg quarantine in rimstudio-platform](0005-ports-and-platform-quarantine.md) | accepted | D-009, D-010 |
| 0006 | [XML boundary crate and byte-span editing](0006-xml-boundary-and-byte-span-editing.md) | accepted | D-012, D-013 |
| 0007 | [Node tree in core and an XML-free defs engine](0007-node-tree-and-xml-free-defs.md) | accepted | D-014, D-016, D-017, D-020 |
| 0008 | [In-house XPath 1.0 engine](0008-in-house-xpath.md) | accepted | D-015 |
| 0009 | [Runtime def type table and two-tier def data](0009-def-type-table-and-two-tier-def-data.md) | proposed | D-018, D-019 |
| 0010 | [Two-phase scan, file watching and allocator policy](0010-scanner-watching-and-allocator.md) | proposed | D-021, D-028, D-069 |
| 0011 | [JSON-only caches and the cache key](0011-json-caches-and-cache-keys.md) | accepted | D-022, D-023 |
| 0012 | [Data roots, portable mode, settings files and secrets](0012-data-roots-settings-and-secrets.md) | proposed | D-024, D-030 |
| 0013 | [JSONC with CST edits, atomic writes, migrations and generated schemas](0013-jsonc-cst-atomic-writes-and-schemas.md) | accepted | D-025, D-026, D-027 |
| 0014 | [Identity scheme for mods](0014-identity-scheme.md) | accepted | D-029 |
| 0015 | [Read-only import of the RimSort aux database](0015-rimsort-aux-database-import.md) | accepted | D-031, D-083 |
| 0016 | [RemoteDataset pipeline and runtime-only community datasets](0016-remote-datasets-runtime-only.md) | proposed | D-032, D-034, D-035 |
| 0017 | [Lossless layered rules model](0017-lossless-layered-rules.md) | accepted | D-033 |
| 0018 | [Sorting and validation as separate pure crates](0018-sort-and-validate-pure-crates.md) | accepted | D-036 |
| 0019 | [Own VDF reader and the DetectionReport](0019-own-vdf-reader-and-detection-report.md) | accepted | D-037, D-038 |
| 0020 | [Managed link farm for custom mod folders](0020-managed-link-farm.md) | accepted | D-039 |
| 0021 | [Game-folder write fence and ModsConfig protocol](0021-game-folder-write-fence.md) | proposed | D-040, D-041 |
| 0022 | [Typed bindings generator and DTO conventions](0022-typed-bindings-and-dto-conventions.md) | accepted | D-042, D-043 |
| 0023 | [Snapshot plus delta streaming and the jobs model](0023-snapshot-delta-streaming-and-jobs.md) | accepted | D-044, D-045 |
| 0024 | [Error envelope, diagnostic codes and logging](0024-errors-diagnostics-and-logging.md) | accepted | D-046, D-060 |
| 0025 | [Webview security posture and image serving](0025-webview-security-and-image-serving.md) | accepted | D-047, D-048 |
| 0026 | [Preact 10 now, Preact 11 behind a gate](0026-preact-10-now-11-gated.md) | accepted | D-050 |
| 0027 | [Frontend layout, state layer and enforcement](0027-frontend-layout-state-and-enforcement.md) | accepted | D-051, D-052, D-057 |
| 0028 | [Virtualised lists and drag and drop](0028-virtual-lists-and-drag-and-drop.md) | accepted | D-053, D-054 |
| 0029 | [Theming and internationalisation](0029-theming-and-i18n.md) | accepted | D-055, D-056 |
| 0030 | [Testing strategy](0030-testing-strategy.md) | accepted | D-058 |
| 0031 | [CI, packaging, updates and the app identifier](0031-ci-packaging-updates-and-identifier.md) | needs-owner | D-059, D-049, D-087 |
| 0032 | [Publish sidecar protocol and the user's Steam library](0032-publish-sidecar-and-steam-library.md) | accepted | D-061, D-062, D-063, D-086 |
| 0033 | [Item math and Combat Extended generator placement](0033-item-math-and-ce-generator.md) | accepted | D-064, D-065 |
| 0034 | [Licence hygiene and reference-project rules](0034-licence-hygiene.md) | accepted | D-067, D-088 |
| 0035 | [In house JSON database and the aux-db exception](0035-in-house-json-database.md) | accepted | D-083, D-031 |
| 0036 | [Release 0.1.0 scope, backend first, and the minimal manager crate](0036-release-scope-and-backend-first.md) | accepted | D-084 |
| 0037 | [Vanilla by default, Combat Extended as an optional patch](0037-vanilla-default-optional-ce-patch.md) | accepted | D-085, D-064, D-065 |
| 0038 | [Toolkit shared module, guarded writer and project backups outside the mod folder](0038-toolkit-shared-module-and-project-backups.md) | accepted | D-089, D-090 |
| 0041 | [RimStudio mod layout v1](0041-rimstudio-mod-layout-v1.md) | accepted | D-104 to D-109 |
| 0042 | [Generation fidelity of a clone](0042-generation-fidelity-of-a-clone.md) | accepted | D-110 to D-114 |
| 0039 | [Project records as documents, the run time type table and the DTO boundary, as built](0039-project-records-type-table-and-dto-boundary-as-built.md) | accepted | D-091, D-092, D-093 |
| 0040 | [Development bridge for the browser test UI](0040-development-bridge.md) | accepted | D-103 |

## 2. Template

Copy this structure for a new record. Name the file `NNNN-short-title.md` with the next free number. A superseded record keeps its file and gets the line `Superseded by ADR NNNN` under the status line; it is never deleted or silently edited.

```markdown
# ADR NNNN: Title in sentence case

Status: accepted | proposed | needs-owner | superseded | Last updated: YYYY-MM-DD | Register: D-nnn

## Context

The forces at play: the requirement (R1 to R12), the measured facts, the constraint.
Two to six sentences.

## Decision

What is decided, in the active voice, with the names of crates, files and versions.

## Consequences

- What becomes easier, what becomes harder, what must be measured or enforced.

## Alternatives rejected

- Each alternative in one line with the reason.

## Evidence

- Relative links to the research note sections or architecture documents.
```

## 3. Conventions

1. One decision per record; closely coupled register entries may share one record.
2. Evidence links are relative and must resolve; `cargo xtask check-docs` verifies them.
3. Records describe RimSort, RimCrow, Combat Extended and game behaviour in the project's own words ([ADR 0034](0034-licence-hygiene.md)).
4. Changing a decision means adding a record that supersedes the old one and updating the register and the roadmap in the same change.
5. Documents use no em dashes, no en dashes and no emojis.
