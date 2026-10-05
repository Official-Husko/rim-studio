# ADR 0017: Lossless layered rules model

Status: accepted | Last updated: 2026-10-04 | Register: D-033

## Context

RimSort rules files carry unknown keys, varying shapes and an `incompatibleWith` field; users also expect their own rules to be exportable back as a patch. A typed lossy model would silently drop data on export.

## Decision

`rimstudio-rules` models community and user rules losslessly (unknown keys, key order, shape variants and `incompatibleWith` are kept) and merges them in five layers: About force, About soft, community, user and derived. Every edge carries provenance, insertion is cycle-safe, a suppression sidecar lets the user silence a rule they disagree with, and `explain` answers why two mods are ordered as they are. User rules can add and suppress. Export is a surgical patch against the cached upstream bytes, so a pull request touches only what changed.

## Consequences

- Memory is slightly higher than a lossy model.
- The rule editor in M3 builds on explain and suppressions rather than new data.
- Import of RimSort user rules (R6) is a codec, not a migration.

## Alternatives rejected

- A typed lossy model.
- User rules that can only add.

## Evidence

- [Community datasets analysis](../research/community-datasets-analysis.md) implications 1 to 4 and 10
- [Rules fetch and merge design](../research/rules-fetch-and-merge-design.md) implication 8
- [Load order and validation spec](../features/load-order-and-validation.md)
