# ADR 0034: Licence hygiene and reference-project rules

Status: accepted | Last updated: 2026-10-04 | Register: D-067, D-088

## Context

RimSort is GPL-3.0, RimCrow is MIT, Combat Extended is CC BY-NC-SA 4.0 and the community datasets are unlicensed (R11). A single copied file could taint the repository. RimStudio's own licence is custom and will be added later by the owner (D-088).

## Decision

RimSort, RimCrow and Combat Extended are read-only concept references described in our own words; their trees move to `reference/` and are never copied from. `cargo deny` enforces a licence policy for dependencies and `xtask check-licences` plus the invariants no-reference-data and runtime-datasets check that no CE, vanilla or dataset tables enter the repository. The repository licence is custom and comes later: manifests carry no `license` field (the workspace package table has none and sets `publish = false`), there is no `LICENSE` file and no licence header until the owner supplies one, and `xtask check-licences` concerns dependency licences only.

Practical rules: describe reference behaviour in our own words in docs and comments; never paste code, tables or data files from a reference tree; write test vectors with fictional numbers; fetch datasets at run time; and when unsure, ask before adding.

## Consequences

- The owner supplies the licence text before the first public release (M7 gate).
- Contributors must read the reference rules in the contributing guide.
- Whatever licence the owner writes, no dependency choices change.

## Alternatives rejected

- A placeholder permissive licence (MIT OR Apache-2.0).
- A copyleft licence chosen by default.
- Bundling datasets with attribution.

## Evidence

- [Cross-platform packaging](../research/cross-platform-packaging-research.md) implication 12
- [Community datasets analysis](../research/community-datasets-analysis.md)
- [Tooling and conventions](../architecture/tooling-and-conventions.md)
