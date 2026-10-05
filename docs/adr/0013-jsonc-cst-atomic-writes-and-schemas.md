# ADR 0013: JSONC with CST edits, atomic writes, migrations and generated schemas

Status: accepted | Last updated: 2026-10-04 | Register: D-025, D-026, D-027

Update 2026-10-05: spike S-02 passed; the CST supports every needed edit and the serde rewrite fallback is not used by the app (details in [data and persistence](../architecture/data-and-persistence.md) section 9).

## Context

R10 says app data is JSON and configs are JSONC. Users will hand-edit settings and themes and expect their comments to survive. RimCrow's persistence work showed the value of backups and forward-only migrations.

## Decision

Machine-written data is JSON; user-edited files (settings.jsonc, workspace.jsonc, projects/*.jsonc, themes) are JSONC and are changed only through CST edits with `jsonc-parser` 0.34.0 so comments survive (spike S-02; the fallback is a serde rewrite with a comment-loss warning). Writes are atomic (temp then rename), each save keeps a verified backup under a retention policy, documents carry `schemaVersion` (compact caches carry `v`), migrations are pure and forward-only, and a file newer than the app opens read-only. JSON Schemas are generated from Rust types with schemars through `xtask schemas`, committed and drift-checked.

## Consequences

- S-02 is an M0 gate: insert key in order, remove key, replace array element.
- Downgrades are safe because newer files are never rewritten.
- Schemas allow editor completion for hand-edited files.

## Alternatives rejected

- json5 or strip-and-rewrite (loses comments).
- In-place writes; silent downgrade.
- Hand-written schemas.

## Evidence

- [Rust crate research](../research/rust-crate-research.md) section 6.2 and implication 5
- [RimCrow analysis](../research/rimcrow-analysis.md) implications 3 and 6
- [Data and persistence](../architecture/data-and-persistence.md)
