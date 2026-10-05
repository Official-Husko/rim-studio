# ADR 0001: Virtual Cargo workspace, rename and naming

Status: accepted | Last updated: 2026-10-04 | Register: D-001, D-011, D-068, D-070

## Context

The repository folder is misnamed `rimforge-studio` and the root is the cargo-init default package with edition 2024 and a hello world main. Requirement R1 fixes the product name RimStudio and R2 demands modular code. Tauri 2.12 is itself edition 2024 with a minimum Rust of 1.90, and Tauri 3 is only at an alpha (3.0.0-alpha.4, 2026-10-01).

## Decision

Step 0 of the roadmap renames the folder to `rimstudio`, replaces the root package with a virtual workspace (edition 2024, resolver 3, `rust-version` 1.95, toolchain channel 1.96, shared `[workspace.dependencies]` and `[workspace.lints]`) and moves the read-only reference checkouts into `reference/`. The product is RimStudio; every crate, npm package and binary uses the prefix `rimstudio-` (library names use underscores). The word rimforge is never a product name; only the old folder name may be mentioned factually. Docs and code are linted for the old name. We stay on the Tauri 2.12 line and keep every crate below the shell free of Tauri types, so a later Tauri 3 migration touches only the shell.

## Consequences

- All paths in documents written before step 0 use the old folder name until the rename lands.
- A `cargo xtask` alias, one lockfile and inherited lints become the baseline for every crate.
- MSRV drift against Tauri is watched in CI.
- The shell crate has a line budget (3000 lines) that makes a Tauri 3 move cheap.

## Alternatives rejected

- Keep a single package with modules: no enforcement of boundaries.
- No workspace, one lockfile per component (the reference tool Jan ended with six lockfiles).
- Adopt the Tauri 3 alpha now: unstable, unneeded.

## Evidence

- [Rust crate research](../research/rust-crate-research.md) section 12.2 and section 3.1
- [Reference architectures](../research/reference-architectures.md) section 5
- [Workspace layout](../architecture/workspace-layout.md)
- [Decision register](../architecture/decision-register.md) section 2
