# ADR 0009: Runtime def type table and two-tier def data

Status: proposed | Last updated: 2026-10-04 | Register: D-018, D-019

Amended by [ADR 0039](0039-project-records-type-table-and-dto-boundary-as-built.md): spike S-07 passed for vanilla and the DLC; the type table is built at run time by `rimstudio-workspace`.

## Context

Resolving a full 600-mod list is unmeasured, while the streaming def index measured 32 MB for 700 mods. The def type table that says which classes are defs and which fields are lists could be shipped as a schema, but that commits data derived from game assemblies to the repository (R11 spirit).

## Decision

The type table is JSON generated at run time from the user's assemblies and cached by assembly hash; with no table, every `*Def` is treated as a def and a visible warning is shown. No schema is shipped. Def data has two tiers: an always-available streaming def index and a full `DefDatabases` snapshot built on demand as a job. Queries are paged and no tree is sent in bulk to the webview. Spikes S-07 (a Rust metadata reader for assemblies) and S-08 (memory and time for a 600-mod resolve) must pass before the decision is frozen.

## Consequences

- The proposal blocks M4 tools that need exact types; the fallback keeps them usable.
- The def explorer and patch tester work on snapshots, never live trees.
- If S-08 fails, the snapshot is limited to the active project plus vanilla.

## Alternatives rejected

- Shipping a pre-generated schema.
- Resolving everything at open.
- Sending trees to the UI.

## Evidence

- [Def engine semantics](../research/def-engine-semantics.md) implication 9
- [Webview and IPC performance](../research/webview-and-ipc-performance.md) section 2.4
- [Modding toolkit scope](../research/modding-toolkit-scope.md) section 5.2
