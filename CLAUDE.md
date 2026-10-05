# RimStudio

A cross-platform desktop app for RimWorld modders: a fast mod manager (RimSort feature parity, compatible with the RimSort community rules datasets) plus a modding toolkit (def explorer, patch tester, an item designer for weapons and apparel that writes vanilla by default with an optional Combat Extended patch, a Steam Workshop publisher). Rust Cargo workspace, Tauri 2, Vite, Preact, TypeScript, Tailwind CSS, pnpm.

## Current state

The Cargo workspace exists in place under `crates/` (every crate is named `rimstudio-*`) and `xtask/`. The documentation under `docs/` is the specification. The Rust backend of release 0.1.0 is implemented and verified (all gates green, see the status page): the designer slice (a weapons editor for ranged and melee weapons, an optional Combat Extended patch generator, a minimal `rimstudio-manager` for settings and sources) with `rimstudio-cli` as the interim front end. The next step is the UI (Tauri shell and frontend) on top of the app registry. The apparel editor, the full manager, datasets and publishing come later. Scope, exit criteria and deferred items are in `docs/roadmap.md` section 2.1; progress is on `docs/status/0.1.0-backend.md`.

The working folder keeps the name `rimforge-studio` (D-099): the GitHub repository is `rim-studio` and the product is RimStudio; no rename is planned. Read `docs/architecture/workspace-layout.md` before adding crates.

Building tip: the project drive is slow, so set the target directory outside the repository, for example `export CARGO_TARGET_DIR=$HOME/.cache/rimstudio-target`, and never build under `/tmp` (RAM backed). Use package scoped commands (`cargo check -p CRATE`, `cargo test -p CRATE`, `cargo clippy -p CRATE --all-targets -- -D warnings`, `cargo fmt -p CRATE`). Tests that read a real install are `#[ignore]` and use the environment variables `RIMSTUDIO_GAME_DIR`, `RIMSTUDIO_WORKSHOP_DIR`, `RIMSTUDIO_CE_DIR` and `RIMSTUDIO_CUSTOM_DIR`; tests use fictional `RS_` names and numbers. The whole workspace gate is `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo run -p xtask -- check-all`; Windows code is compile checked with `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu`. The target directory grows large because of `debug/incremental`; delete that folder when disk space is short.

## Read first

- `docs/README.md`: the index and the reading order.
- `docs/architecture/overview.md`: the named invariants (I-01 to I-19), layers, process model.
- `docs/architecture/workspace-layout.md`: the project structure, naming, dependency matrix, step 0.
- `docs/roadmap.md`: milestones M0 to M7, spikes, owner decisions.
- `docs/features/*.md`: requirement ids (MM, GD, ST, LO, CD, WS, IT, WP) are stable; implement and test against them.

## Rules that apply to every change

1. **Formats (R10).** App-owned data is JSON; configs are JSONC; XML appears only when reading or writing RimWorld's own files and only inside `rimstudio-xml`. No SQL, TOML, YAML or binary formats for app-owned data; many small records live in the in house JSON document store of `rimstudio-io` (D-083). A dependency that only reads a foreign format is acceptable (read only `rusqlite` behind `aux-db` in `rimstudio-datasets`). Templates for generated XML are JSON node trees rendered by the boundary crate.
2. **Licence hygiene (R11).** `RimSort-main`, `RimCrow-main` and `CombatExtended-Development` are read-only references: describe concepts in your own words, never copy code or data. Vanilla and Combat Extended values are read from the user's install at run time; community datasets are fetched at run time and never bundled. Nothing under `docs/research/data/` ships. Design documents and UI development fixtures may quote real numbers (D-100).
3. **Layers.** Dependencies follow the matrix in `workspace-layout.md`; no crate below the shell depends on `tauri`; business rules (ordering, validation, balance math) live in Rust crates, never in views or stores; commands are declared once in the registry in `rimstudio-app`.
4. **Names.** The product is RimStudio and crates and packages use the prefix `rimstudio-`. Never use "rimforge" as a product name.
5. **Docs (R12).** No em dashes, no en dashes, no emojis, no mention of AI assistance in project content. A merged feature updates its specification; a change to a decision adds or supersedes an ADR. Milestone numbers follow the roadmap.
6. **Evidence.** Research notes in `docs/research/` are evidence and are not edited after a decision; where they differ from architecture or feature documents, the latter win.
7. **Vanilla by default (D-085).** The item designer always writes vanilla definitions. A Combat Extended patch is an optional, per item opt in (an off by default toggle), goes into its own files in a folder gated by `LoadFolders.xml`, and is never automatic, even when CE is installed.
8. **Licence later (D-088).** The project licence is custom and comes later: no `license` field in manifests, no `LICENSE` file and no licence headers until the owner supplies one.
9. **Commits and versions (D-101).** Conventional Commits: `type(scope): summary` with feat, fix, chore, docs, refactor, test, build, ci or perf, the scope being the crate or area, an imperative summary and a body that explains what and why; one logical change per commit. Versions are always `major.minor.patch` and tags are `vMAJOR.MINOR.PATCH`. Commit only when asked, never push unasked, and keep commit messages free of AI mentions and co-author trailers (rule 5).
