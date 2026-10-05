# ADR 0018: Sorting and validation as separate pure crates

Status: accepted | Last updated: 2026-10-04 | Register: D-036

## Context

Sorting and validation were entangled in the reference managers' views. The CLI `check` command and the GUI must produce the same answers, and both should be fast enough to re-run on every list change.

## Decision

`rimstudio-sort` provides deterministic tiers, a canonical sort and a game-style sort, cycle reports and order diffs. `rimstudio-validate` produces diagnostics: incremental list validation over a position array, author lints, publish preflight and Player.log classification, under a code registry. Dependency edges imply order only as an opt-in. Both are L2 engines without IO and output is deterministic (identical at 1 and 8 threads).

The engines take their inputs (rule graph, list positions, definitions) as plain data, so the same call can run in the CLI, in a benchmark or in a property test, and the UI only renders the result.

## Consequences

- Incremental validation re-checks only the affected window of the list.
- Both crates run in the CLI with no GUI.
- A sort preview is a diff of two orders, which the UI renders.

## Alternatives rejected

- Rules evaluated inside views.
- One combined crate.

## Evidence

- [RimSort core domain](../research/rimsort-core-domain.md) implications 7 to 10
- [RimCrow analysis](../research/rimcrow-analysis.md) implication 4
- [Load order and validation spec](../features/load-order-and-validation.md)
