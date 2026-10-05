# ADR 0003: Tauri-free composition root, one command registry, CLI package

Status: accepted | Last updated: 2026-10-04 | Register: D-005, D-006, D-007

## Context

Per-feature Tauri plugin crates cost boilerplate and a drift test, and the behaviour of tauri-specta per plugin is unverified. Business rules inside the shell make a headless client impossible. A previous design (Jan) used a pair of mutually exclusive cargo features for the CLI and needed 15 cfg gates.

## Decision

`rimstudio-app` is a Tauri-free composition root holding `AppContext`, the single command registry macro, dispatch, the `JobRunner`, the tool table, logging initialisation and boot. Commands are declared once with a kind (query, action, stream, job) and plain handlers `fn(ctx, req) -> Result<Resp, ApiError>`. Shell wrappers, CLI routes, the permission list and bindings are generated from that table. `rimstudio-shell` and `rimstudio-cli` depend only on app, core and the contract. The CLI is a separate package that never depends on the shell and has no exclusive features. Plugin crates are reserved for OS capabilities with their own permission scope (none planned).

## Consequences

- The shell cannot name a feature crate, so no business rule can leak into it.
- CLI and GUI run identical handlers and render identical job progress.
- Permissions are coarse: one capability plus a registry-derived command list.
- Distribution of the CLI (separate download or beside the app) is decided in M7.

## Alternatives rejected

- One plugin crate per feature.
- All commands in the shell crate (Cap's 11k line file).
- CLI as a feature of the shell or an extra `[[bin]]` there (Tauri bundles every bin).

## Evidence

- [Reference architectures](../research/reference-architectures.md) section 7 strategies D and E
- [Rust crate research](../research/rust-crate-research.md) section 12
- [IPC and state](../architecture/ipc-and-state.md)
