# ADR 0002: Layer matrix and crate granularity

Status: accepted | Last updated: 2026-10-04 | Register: D-002, D-003, D-004

## Context

R2 asks for non-monolithic, reusable components. The reference projects show two failure modes: giant crates that hide layers and recompile everything (Cap, Spacedrive) and over-splitting that costs boilerplate. Earlier research notes also used conflicting crate names (defs versus defdb, detect versus steam).

## Decision

The workspace has 24 members arranged in layers L0 domain, L1 infra, L2 pure engines, L2 IO services, L3 features, L4 app, shell, cli and sidecar, plus a contract crate and a support layer. Every manifest carries `[package.metadata.rimstudio] layer = ...` and the allowed edges are data in `xtask/layers.jsonc`, enforced by `cargo xtask check-layers` (guppy). A crate is split off only for dependency enforcement, compile time or testability; everything else stays a module until a named trigger in the crate catalog fires. Canonical names are core, xml, io, platform, xpath, defs, rules, sort, validate, design, steam, library, workspace, datasets, manager, toolkit, publish, ipc-types, app, shell, cli, steam-helper, testing and xtask. Retired names are defdb, detect, rules-format, rule-graph, dataset-http and rimstudio-tauri-*.

## Consequences

- A new crate must choose a layer; an edge outside the matrix needs a register entry.
- Pure engines can be benchmarked and fuzzed without IO.
- Same-layer exceptions are explicit: defs to xpath, sort to rules, validate to rules, defs and xpath, design to defs, workspace to library.

## Alternatives rejected

- Convention-only layering in one library crate (Spacedrive).
- One crate per module, 30 or more crates.
- A few large crates.

## Evidence

- [Reference architectures](../research/reference-architectures.md) sections 6 and 8
- [Crate catalog](../architecture/crate-catalog.md) section 2
- [Overview](../architecture/overview.md)
