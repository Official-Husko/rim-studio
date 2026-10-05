# RimStudio tooling and conventions

This document collects the working rules of the project: Rust and TypeScript conventions, the `xtask` commands, the local development workflow, commit and pull request conventions, the architecture decision record (ADR) process, the documentation discipline, versioning and release, and the definition of done. It builds on the [workspace layout](workspace-layout.md) (naming, layers, checklists), the [decision register](decision-register.md) (D-001, D-057, D-058, D-068) and the invariants of the [architecture overview](overview.md). Evidence: [Rust crate research](../research/rust-crate-research.md), [frontend stack research](../research/frontend-stack-research.md) and [packaging research](../research/cross-platform-packaging-research.md). The testing rules are in the [testing strategy](testing-strategy.md) and the error rules in [error handling and logging](error-handling-and-logging.md).

Status: draft | Last updated: 2026-10-04

## 1. Rust conventions

### 1.1 Toolchain and workspace

| Item | Rule |
|---|---|
| Edition and resolver | Edition 2024, resolver 3, one virtual workspace (D-001) |
| Toolchain | `rust-toolchain.toml` pins channel 1.96 with `rustfmt` and `clippy`; `rust-version = "1.95"` in the workspace package table is the declared minimum |
| Shared settings | `[workspace.package]`, `[workspace.dependencies]` (every external dependency is declared once, members use `dep.workspace = true`) and `[workspace.lints]`; `xtask check-deps` and `xtask check-layers` fail a member that opts out (cargo-deny `bans.workspace-dependencies` reports only duplicate and unused workspace entries) |
| Layer tag | Each manifest has `[package.metadata.rimstudio] layer = "<tag>"`; the matrix is data in `xtask/layers.jsonc` |
| Features | Additive only; a feature never changes behaviour of existing items. Feature names are kebab-case (`tool-defs`, `aux-db`). `e2e` and `diagnostics` never reach release builds |
| Dependencies | Exact pins for the platform-sensitive ones (Tauri 2.12.1, `tauri-specta =2.0.0-rc.25`, `quick-xml 0.42.0`), caret ranges elsewhere, all resolved by the committed `Cargo.lock`. A new dependency needs a line in the pull request: why, size, licence, maintenance, alternatives |
| New crate | Only by the checklist in the [workspace layout](workspace-layout.md) section 6.2 and `cargo xtask new-crate`; the default is a module, a crate only for enforcement, compile time or testability (D-003) |

### 1.2 Formatting and lints

1. `rustfmt.toml`: edition 2024, `max_width = 100`, `use_small_heuristics = "Default"`, imports grouped `std`, external, workspace, crate (`group_imports = "StdExternalCrate"` needs nightly rustfmt; do not use it on stable, and let `cargo fmt` decide). `cargo fmt --check` is a CI gate.
2. Workspace lints (root manifest): `unsafe_code = "forbid"` (a documented crate level override only in `rimstudio-steam-helper` if the FFI route needs it, D-062), `unreachable_pub` warn (error in CI), clippy `dbg_macro`, `todo`, `unwrap_used` and `expect_used` warn in library crates, `print_stdout` and `print_stderr` warn outside the CLI. CI runs with `RUSTFLAGS=-D warnings`.
3. `clippy.toml` `disallowed-methods` (the clippy policy of the [workspace layout](workspace-layout.md) section 10): `std::process::Command::new` (use `Launcher`), `std::fs::write`, `File::create`, `rename`, `remove_file`, `remove_dir_all` (allowed only in `rimstudio-io` through a crate-level `allow` with a rationale), `SystemTime::now` (use `Clock`), `std::env::var` (use `EnvProbe`). `disallowed_types` blocks `OnceLock`, `LazyLock`, `once_cell` and `lazy_static` outside the logging init (I-16); `static mut` is covered by `unsafe_code = "forbid"`. `rimstudio-cli` carries no exception: it writes `--out` files through `rimstudio-app::dispatch_to_file`, which calls `rimstudio-io` (D-075). `rimstudio-testing` and `xtask` carry a crate level `allow` with a reason for the write functions, and `xtask` also for `Command::new`.
4. `#[allow(...)]` always carries `reason = "..."`. A lint is fixed or justified, never silenced in bulk.
5. Public items have doc comments; each crate has a `//!` crate doc stating layer, purpose, and what it must never depend on (`xtask check-docs` enforces presence).

### 1.3 Naming and module layout

The table of names (crates, lib names, types, traits, commands, DTOs, diagnostic codes, files, benchmarks, folders, milestones) is in [workspace layout](workspace-layout.md) section 2 and is not repeated. Additional rules:

1. Module files are `snake_case.rs`. A module becomes a folder only when it has more than one child. `lib.rs` contains the crate doc, `mod` lines and the curated public re-exports, nothing else.
2. A crate's public API is what `lib.rs` re-exports. Everything else is `pub(crate)`. Tests that need internals sit next to the code in `#[cfg(test)]` modules.
3. Files stay under 1,500 lines (warning, `xtask check-size`); the shell crate stays under 3,000 lines with no file over 400.
4. Handlers are plain functions `pub fn name(ctx: &AppContext, req: Req) -> Result<Resp, ApiError>` (D-005); no handler holds state, and no service reaches for globals (I-16).
5. Constructors take ports and configuration explicitly. A function that needs the time, environment, filesystem, processes or network takes the port, never the standard library call (clippy enforces the common ones).
6. Result containers end in `Report` or `Outcome`; ports end in `Probe` (read only) or `Sink` (output); capability traits are nouns (`LinkBackend`).
7. Collections that reach output are ordered: `BTreeMap`, `IndexMap` or sorted vectors. Iteration order of `HashMap` must not influence JSON, diagnostics or caches (I-12).
8. Paths: `PathBuf` in domain code is allowed, but anything persisted or sent over IPC uses the forward-slash relative form against a named root; UTF-8 is assumed only after an explicit lossy conversion with a diagnostic.

### 1.4 Errors, DTOs and serialisation

1. One `thiserror` enum per crate named `<Crate>Error` with a stable `code()` (D-046). Binaries, xtask and tests may use `anyhow`. Content problems are `Diagnostic`s, never `Err` (I-10). Details are in [error handling and logging](error-handling-and-logging.md).
2. DTOs live in `rimstudio-ipc-types`: `<Command>Request`, `<Command>Response`, or a `Dto` suffix on collision. Serde attributes: `#[serde(rename_all = "camelCase")]` on structs, `#[serde(rename_all = "kebab-case")]` on enums. 64-bit ids cross IPC as strings (`#[serde(with = ...)]` helper in the contract crate). List rows carry list columns only; detail comes from a second query (D-043).
3. Domain types keep their own serde shape and are never exposed to IPC directly. Conversions are `From` impls in the feature or app crate, not in the contract crate.
4. Persisted JSON documents have a top-level `schemaVersion` (compact caches use `v`) and a migration; the schema is generated by `schemars` into `schemas/` and drift-checked (D-027). New fields are optional with defaults so older files load.
5. XML text appears only in `rimstudio-xml` (I-02). Any other crate that needs RimWorld XML calls its codecs or the render function.

### 1.5 Unsafe policy

`unsafe_code` is forbidden in every crate. Exceptions need an ADR, a single module that contains all the unsafe code, a `// SAFETY:` comment per block stating the invariant, a test that exercises the boundary, and a Miri run where the code is pure Rust. The only expected candidate is the Steam helper's FFI (D-061 and D-062); the Windows registry and junction calls go through safe crates in `rimstudio-platform` and must stay that way unless measured otherwise. Third-party crates with `unsafe` are fine; our own code stays safe.

### 1.6 Concurrency and cancellation

Work over 1 ms is a job (I-13). Parallel scans use rayon with at most 8 workers and one core kept free (D-069). Every loop over units of work checks the `CancelToken`. No async in engine crates or in `rimstudio-io`, `-library`, `-workspace` or the L3 features' pure logic; async is confined to `rimstudio-datasets` (reqwest), `rimstudio-publish` (helper process I/O), `rimstudio-app` (job runner, dispatch) and the shell. Blocking work never runs on the Tauri async runtime thread; query and action handlers are synchronous functions that the shell wrappers run through `spawn_blocking`, and job handlers run on the job runner.

## 2. TypeScript conventions

### 2.1 Project settings

| Tool | Version pin | Rule |
|---|---|---|
| TypeScript | 6.0.3 | `strict` plus `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `noUnusedLocals`, `noUnusedParameters`, `verbatimModuleSyntax`, `isolatedModules`, `moduleResolution: "bundler"`, `jsx: "react-jsx"` with `jsxImportSource: "preact"`; target the lowest webview (the startup feature probe tells the user when it fails, D-055) |
| Preact and signals | 10.29.8, `@preact/signals` 2.11.3 | Write Preact 11-clean code (D-050): no `preact/compat` in new code, no legacy context, no `forceUpdate`, function components only |
| Vite | 8.3.2 | One config, `vite-plugin-checker` for type and lint in dev |
| Tailwind | 4.3.3 | CSS-first config, tokens in `styles/tokens.css` |
| Package manager | pnpm 11 | `pnpm-workspace.yaml` with `apps/*` and `packages/*`; `packageManager` field set; the lockfile is committed and CI uses `--frozen-lockfile` |
| Lint | oxlint 1.86.0 | `.oxlintrc.json` with the `react`, `typescript`, `oxc` and import plugins and per-folder `no-restricted-imports` overrides (D-057, spike S-12) |
| Format | Prettier 3.9.9 with `prettier-plugin-tailwindcss` 0.8.1 | `.prettierrc.json` at the root |

### 2.2 Imports and boundaries

1. A feature imports only `shared`, `rimstudio-ui` and `rimstudio-ipc-types`, never another feature. Cross-feature needs move to `shared` or to the backend.
2. Each feature has one public entry, `index.ts`; nobody imports from `features/<name>/components/...` from outside.
3. Only `shared/platform` imports `@tauri-apps/*`; only `shared/ipc` calls `invoke` (I-14 spirit, I-09). Components never call IPC directly; they use the feature's `api.ts` which wraps `shared/ipc` with typed commands from `rimstudio-ipc-types`.
4. `rimstudio-ui` imports nothing from the app. `packages/ipc-types/src/bindings.ts` is generated and never edited (I-19).
5. No XML library, no business rules (I-04): sorting, validation, scoring and path policy live in Rust; the frontend formats, filters and orders what the backend returns.
6. Boundaries are enforced by oxlint overrides and dependency-cruiser (`forbidden` rules, `no-circular`); knip reports unused exports as a warning.

### 2.3 Components, hooks and state

1. One component per file, `PascalCase.tsx`; modules `camelCase.ts`; folders `kebab-case`. Props are a named `interface <Name>Props`. No default exports except lazy route pages.
2. State is signals. A feature's `store.ts` holds signals and pure action functions (testable without rendering). Component-local state uses `useSignal`. Derived state is `computed`, never effect-synchronised copies.
3. Rows of large lists live in a map of signals so an update touches one row (D-052). Every list over 100 rows uses `shared/lists` (D-053); fixed row heights; DOM under 3,000 nodes.
4. Server data goes through `shared/ipc`: `query` for request and response, `stream` for channels, `snapshot` plus `delta` for revisioned lists, `jobs` for long work with cancellation, rAF batching of updates. Hooks named `useX` wrap a signal or store and must not hide network calls.
5. Effects: avoid `useEffect` for data flow; if needed, return the cleanup and keep the dependency list exact. Event handlers are named `onX` props and `handleX` internally.
6. Accessibility is a requirement: roles and labels from the gallery components, focus order, visible focus, every drag has a keyboard or menu equivalent (D-054), colour is never the only signal, contrast checked against the tokens in both themes.
7. Text is a key from `en.json` through the i18n layer; no string literals in JSX except test ids. Keys are flat and semantic (`manager.sort.apply`). Game text is never translated (D-056).
8. Errors: a code from the envelope selects the message key; components never show `message` directly except in Details. An error boundary at the app shell and one per feature page.
9. No `any`; `unknown` plus narrowing or valibot schemas at trust boundaries (user theme files, locale files). No non-null assertions without a comment.

### 2.4 Tailwind and styling

1. Colours, spacing, radii, fonts and shadows are CSS variables defined in `tokens.css` and mapped through Tailwind `@theme inline` (D-055). No hex colours or raw pixel values in components; use token utilities.
2. Light, dark and system themes via a `data-theme` attribute and variable sets; one accent variable. The Blueprint-inspired look (R8) is expressed in tokens and `rimstudio-ui` components, so the later design prompt changes tokens, not feature code.
3. Layers: `@layer` order is base, components, utilities, user (last), so user theme files win without `!important`.
4. Class strings are sorted by the Prettier plugin; long repeated groups become a `rimstudio-ui` component, not an `@apply` rule. `@apply` is allowed only in `layers.css` for base elements.
5. No backdrop filters on large panels (performance, IPC note implication 9), fonts are self hosted, motion respects `prefers-reduced-motion`.
6. Every new `rimstudio-ui` component gets a gallery entry and screenshots in both themes before merging.

## 3. xtask commands

The alias `cargo xtask` is defined in `.cargo/config.toml`; xtask needs only cargo (D-011 workspace, crate research section 11.2). An optional two-line `justfile` may alias the common ones; no logic lives there.

| Command | Purpose | Notes |
|---|---|---|
| `cargo xtask check` | Runs every static check below plus `bindings --check` and `schemas --check` | The local equivalent of the `rust` and `bindings` CI jobs |
| `check-layers` | Layer matrix over `cargo metadata` with guppy, tags, same-layer exceptions, feature-edge ban, path from `rimstudio-cli` to `tauri` | Data in `xtask/layers.jsonc` |
| `check-cfg` | Greps `target_os`, `cfg(windows)`, `cfg(unix)` outside the allow list (I-11) | |
| `check-deps` | Dev-only crates (`steamlocate`, `keyvalues-parser`), XML crates only in `rimstudio-xml`, no normal dependency on `rimstudio-testing`, banned telemetry crates | Complements `cargo deny` |
| `check-licences` | Dataset extensions, value tables, reference project paths, old folder name used as product name | I-06, I-07, R11, R12 |
| `check-docs` | Crate `//!` docs; first column of the crate catalog equals workspace members; every invariant is referenced by an enforcement entry; internal markdown links resolve; one H1 and the status line per document; no em dash, en dash or emoji in `docs/` | |
| `check-tools` | Rust `ToolDescriptor` ids equal `apps/desktop/src/app/tools.ts` | D-066 |
| `check-size` | Shell and file size limits; golden size cap | |
| `check-release-features` | `e2e` or `diagnostics` enabled in a release build fails | |
| `bindings [--check]` | Exports TypeScript bindings through the shell's export test into `packages/ipc-types/src/bindings.ts`; `--check` fails on a diff | I-19, D-042 |
| `schemas [--check]` | Generates JSON Schemas with schemars into `schemas/` | D-027 |
| `fixtures` | Regenerates small committed fixtures and vector listings that are generated (the XPath shape file is regenerated only by the maintainer script, never in CI) | |
| `corpus` | Runs the ignored real-data suite after checking `RIMSTUDIO_TEST_*` variables | [testing strategy](testing-strategy.md) section 8 |
| `bench-budgets [--compare <rev>]` | Runs the budget benches and compares ratios with `xtask/budgets.jsonc` | 2x fails, 1.25x warns |
| `package` | Builds the release artifacts: copies the helper per target triple, runs `tauri build`, writes `SHA256SUMS` | Release job only |
| `release-prep <version>` | Bumps versions in all manifests and `package.json` files, updates the changelog, runs `check` and the release checklist, creates the release commit and an unsigned local tag | Does not push |
| `new-crate <name> <layer>` | Scaffolds a crate by the workspace layout checklist | |
| `new-tool <id>` | Scaffolds a toolkit tool module, its `ToolDescriptor`, the frontend feature folder and the tests | |

Every xtask command exits non-zero on failure with one line naming the rule and the offending file; there are tests for xtask itself using small fixture workspaces (a deliberately violating manifest must fail the matching check).

## 4. Local development workflow

1. **Prerequisites.** Rust from `rust-toolchain.toml` (installed by rustup on first run), Node 26, pnpm 11, `cargo-nextest`, `cargo-deny`, `cargo-machete`, and `tauri-cli` 2.12.1 (`cargo install tauri-cli --version 2.12.1 --locked` or the `@tauri-apps/cli` package already in the app). Linux also needs the WebKitGTK development packages: on Arch `webkit2gtk-4.1 base-devel curl wget file openssl libappindicator-gtk3 librsvg xdotool`; on Debian and Ubuntu `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`; on Fedora `webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel` (packaging research section 2; some package lines there are marked unverified). Windows needs WebView2 (present on current systems) and the MSVC build tools; macOS needs the Xcode command line tools.
2. **First run.** `pnpm install`, then `cargo xtask check` to prove the tree is healthy. `cargo xtask fixtures` is only needed after changing vector generators.
3. **Develop the app.** `pnpm --filter rimstudio-desktop tauri dev` (or `cargo tauri dev` from `apps/desktop`), which starts Vite with hot module reload and rebuilds the shell on Rust changes. Backend logic can be developed without the shell: `cargo run -p rimstudio-cli -- <command>` calls the same handlers and needs no GTK (I-01). Prefer the CLI and `cargo nextest` for engine work; the shell is for UI integration.
4. **Develop UI without Rust.** `pnpm --filter rimstudio-desktop dev` serves the web layer with `mockIPC` from `rimstudio-testkit` (a query flag selects the mock backend), and the dev-only `/gallery` route renders every `rimstudio-ui` component in both themes.
5. **Fast loops.** `cargo nextest run -p <crate>`; `cargo check --workspace` for type errors; `pnpm vitest --watch`; `cargo bench -p <crate> --bench <name>` for one benchmark. `bacon` is a fine developer tool but is never a dependency (licence).
6. **Portable data during development.** Create a file named `rimstudio.portable` next to the debug binary (or set `RIMSTUDIO_DATA` if supported, unverified until the boot code lands) so development settings do not touch the real profile.
7. **Real data.** Export `RIMSTUDIO_GAME_DIR`, `RIMSTUDIO_WORKSHOP_DIR`, `RIMSTUDIO_CE_DIR` and `RIMSTUDIO_CUSTOM_DIR` in a shell profile and run the `#[ignore]` tests per crate with `cargo test -p <crate> --release -- --ignored` (a `cargo xtask corpus` wrapper is planned); all access is read-only (see the [testing strategy](testing-strategy.md) section 8).
8. **Linux webview notes.** WebKitGTK 2.50 or newer is the target (D-059). If the window is blank or rendering is slow on a particular GPU or Wayland compositor, known workarounds are the environment variables `WEBKIT_DISABLE_DMABUF_RENDERER=1` and `WEBKIT_DISABLE_COMPOSITING_MODE=1` (commonly reported for Tauri on Linux, unverified for this app until S-10); record any that is needed in the diagnostics page and not in code. Under xvfb the e2e suite needs `webkit2gtk-driver` for `tauri-driver` (package name unverified on non-Debian systems). The inotify limits matter for large libraries; the watcher falls back to polling on ENOSPC (D-028) and logs a warning with the current limit.
9. **Editor setup.** rust-analyzer with `check.command = "clippy"`, the `rimstudio` workspace opened at the repository root; the oxc VS Code extension and Tailwind IntelliSense for the frontend. Editor configuration files are not committed beyond `.editorconfig` (UTF-8, LF, final newline, 4 spaces for Rust, 2 for TS, JSON and Markdown).

## 5. Commit and pull request conventions

The conventions follow the owner's other repositories.

1. **One commit per finished task**, staging only the files that task touched. Do not sweep unrelated working-tree changes into a commit.
2. **Subject**: `<type>: <what changed>`, short, lower case after the prefix, no trailing full stop, at most 72 characters. Types: `feat`, `fix`, `chore`, `remove`, `docs`, `refactor`, `test`, plus `perf` and `build` where they clarify. An optional scope in parentheses names the crate or folder: `feat(library): add two-level scanner`.
3. **Body**: a blank line, then a short `-` bullet list of concrete changes; explain why when it is not obvious. Breaking changes start a line with `BREAKING:` and name the code, DTO, schema or file affected.
4. **Messages describe the product.** Content is about the product only: no tooling credits in code comments, documentation, commit messages or pull request text, and no co-author or attribution trailer or footer on commits or pull requests (R12; the owner's rule for repositories applies here). This overrides tool defaults.
5. **Branches**: `main` is always releasable and protected; work happens on short-lived branches named `<type>/<short-topic>`. Rebase before merge; squash only when the branch has noise, otherwise keep the one-commit-per-task history.
6. **Pull request description**: what and why in two or three sentences; the invariants touched (I-xx) and decisions (D-xxx); how it was tested; screenshots for UI in both themes; benchmark delta if a budget row is affected; fixture origin statement if a fixture was added (testing strategy section 13); the checklist of section 9.
7. **Reviews check**: layer and boundary rules, diagnostics versus errors, determinism, redaction of anything logged, locale keys, accessibility, and docs updated in the same pull request.
8. **Generated files** (bindings, schemas, command table help, layer graph picture) are committed in the same commit as their source change, and never edited by hand (I-19).
9. **Large files**: nothing over 1.5 MB under `docs/`, no binary assets beyond icons, and no real game or mod content in any commit.

## 6. Architecture decision records

The [decision register](decision-register.md) holds decisions D-001 to D-070. After the initial architecture, a new decision follows this process:

1. **When.** A change that affects a layer rule, an invariant, a persisted format, the IPC contract, a pinned dependency family, the dependency policy or a security posture needs an ADR. Pure implementation choices inside one crate do not.
2. **Where.** One file per decision in `docs/adr/NNNN-kebab-title.md` (four digits, next free number, never reused). The register keeps the D-xxx ids; a new ADR also adds the next `D-0xx` row to the register with a link to the ADR file, so the register stays the index.
3. **Template** (copy the one in `docs/adr/README.md` section 2): title; status line `Status: proposed | accepted | needs-owner | superseded by NNNN | rejected`; date; context with evidence links to research notes or measurements; decision in one paragraph; alternatives considered each with one line on why rejected; consequences (what becomes easier, harder, what must be enforced and by which check); owner decisions needed; review date if the decision is time gated.
4. **Status flow.** `proposed` in the pull request that introduces it; `accepted` when merged by the owner; a later change creates a new ADR that supersedes the old one, and the old file only changes its status line and gets a link forward. Accepted ADRs are not rewritten.
5. **Spikes.** A spike (S-xx in the register) records its question, method, result and the decision it feeds in its ADR or in `docs/research/`. A decision marked "needs spike" stays `proposed` until the result is linked.
6. **Owner decisions** are listed in the ADR and in the register's owner decisions section until the owner records an answer; the answer is recorded as a status change, with the date.
7. **Enforcement link.** Every ADR that adds a rule names the check that enforces it (xtask check, deny ban, clippy rule, test). A rule without a check is a guideline and says so.

## 7. Documentation discipline

1. **Structure.** `docs/README.md` is the index (same style as the owner's Parallax docs), with a glossary (`docs/glossary.md`) of every project term; `docs/research/` holds evidence notes and `docs/research/data/` the reproducible data; `docs/architecture/` holds the authoritative architecture documents (overview, workspace layout, crate catalog, decision register, and the documents named in the index); `docs/adr/` holds later ADRs; `docs/features/` holds the per-feature specifications.
2. **Research notes are immutable once a decision uses them.** A note carries its date and evidence. If new facts change a conclusion, add a dated "Update" section at the end and record the consequence as an ADR; do not rewrite history. Typos and broken links are fixable.
3. **Specs follow the code.** A feature has a spec page that describes behaviour in the present tense. A feature is not done until its spec, `features.md` and progress entry are current (adapted from the owner's rules): the spec page explains the concept and edge cases; `features.md` is the short list of what the app can do today, one line per feature grouped by area with a Security and privacy section and a Performance section; `PROGRESS.md` moves items from "Not yet built" to "Done" with one line item and a short write-up, and links the ADR and tests. The `features.md` change goes in the same commit as the feature.
4. **README** stays a short, user-facing pitch; it changes only when a feature affects its highlights or roadmap.
5. **Style rules (R12)**: GitHub-flavoured Markdown; exactly one H1; a one paragraph scope statement; the line `Status: draft | Last updated: <date>`; tables for catalogues, numbered lists for procedures, small mermaid diagrams only where they clarify; no em dashes, no en dashes (use a hyphen, a colon or restructure; write numeric ranges with the word "to"), no emojis, no tooling credits; internal links are relative and must resolve. `xtask check-docs` and `check-licences` enforce the mechanical parts.
6. **Own words (R11).** Describe RimSort, RimCrow, Combat Extended and game behaviour in our own words and cite the research note, never paste their text, code or data.
7. **Naming.** The product is RimStudio. The old folder name is mentioned only as a fact about the misnamed folder, never as a product name; the check flags other uses.
8. **Keeping documents true.** When code and a document disagree, the pull request that changes one changes the other. The crate catalog's first column equals the workspace members; every invariant has an enforcement row; both are checked.
9. **Diagrams and figures** are text (mermaid) or generated by a script kept in the repository; images stay small and are never screenshots of third-party content.

## 8. Versioning and release process

1. **Versioning.** Semantic versioning for the application (`MAJOR.MINOR.PATCH`, pre-releases `-beta.N`). All workspace crates and npm packages share the application version through `[workspace.package]` and one source script (`release-prep`); crates are not published to crates.io in the first releases. The persisted schemas have their own integers (`schemaVersion`, `v`); an app release that raises one includes a forward-only migration and a test with the previous fixture (D-026).
2. **Compatibility promises.** CLI command names and exit codes, the JSON output shapes, the sidecar protocol (version 1), the settings and project file schemas and diagnostic codes are interfaces. Breaking one is a minor version before 1.0 and a major after, with a changelog entry and a migration where data is involved.
3. **Changelog.** `CHANGELOG.md` in Keep a Changelog style, generated by `release-prep` from commit subjects (`feat`, `fix`, `perf`, `remove`), edited by hand for clarity, written for users.
4. **Channels.** Stable and beta update manifests on GitHub Releases through `tauri-action` (D-059). Beta tags are `vX.Y.Z-beta.N`. The updater is hidden where a package manager owns updates.
5. **Release checklist.**
   1. `main` is green on all CI jobs, including `perf` and `nightly` for the last three days.
   2. `cargo xtask corpus` run on the maintainer's real library, install and CE, results attached to the release issue.
   3. Manual matrix of the packaging note (install, detect, external drive, scan 700 mods, update from the previous release, running game warning, uninstall leaves user data) on at least Windows 11, a current macOS and one Linux; the link farm spike S-03 result is current for the shipped behaviour.
   4. `cargo xtask check-release-features`, licence check, `cargo deny check`, audit clean; third party notices regenerated.
   5. `cargo xtask release-prep X.Y.Z`; review the diff; merge; tag.
   6. The release workflow builds NSIS (`currentUser`) and portable zip (both unsigned, D-087), notarised dmg per architecture, AppImage plus deb and rpm; writes `SHA256SUMS` and build attestations; uploads the manifests.
   7. Install the produced artifacts on a clean machine per OS and run the smoke script by hand.
   8. Publish release notes with known issues; announce only after the update manifest is verified.
6. **Signing** and the Windows path are owner decisions (D-059); unsigned pre-releases are acceptable until the owner chooses, and the release notes say so.
7. **Rollback.** A bad release is withdrawn by marking it pre-release and pointing the stable manifest at the previous version; user data stays valid because migrations are forward-only and newer files open read-only in older apps (D-026).

## 9. Definition of done

A task is done when all of these hold; the pull request checklist repeats them.

1. The behaviour works as specified and the acceptance criteria in the spec or issue are met, including the error and empty states.
2. Tests exist at the lowest sensible layer, regressions have vectors where the game quirk is the cause, and `cargo nextest run --workspace` and `pnpm check` pass locally.
3. `cargo xtask check` passes: layers, cfg quarantine, dependency bans, licence hygiene, docs, tools, size, bindings and schemas in sync.
4. Problems in user content are diagnostics with registered codes and locale keys; failures are typed errors with stable codes; nothing logs secrets, usernames or raw paths; a long operation is a job with progress and cancel (I-10, I-13).
5. Output is deterministic: golden tests pass at 1 and 8 threads (I-12).
6. A performance budget row is added or still met when the change is on a measured path; the bench delta is in the pull request.
7. UI work: keyboard operable, accessible names, both themes, a gallery entry and screenshots, locale keys in `en.json`, no business rule in TypeScript (I-04).
8. App-owned data is JSON or JSONC through `rimstudio-io`, with a schema, a migration if a shape changed, and CST edits for user-edited files; the game-folder fence is untouched or tested (I-03, I-05, I-18).
9. No game, CE or community dataset content in the change; fixtures are synthetic with their origin stated (R11).
10. Documentation is current: the spec page, `features.md`, `PROGRESS.md`, the crate catalog or decision register if a decision changed, and an ADR if the change needs one.
11. The commit follows section 5: one task, conventional subject, bullet body, no attribution trailer, generated files included.

## 10. Open items

| Item | Needed from |
|---|---|
| Repository licence: custom, no `license` field and no `LICENSE` file until supplied (D-067, D-088) | Owner, before the first public release |
| App identifier `app.rimstudio.desktop` (D-049) | Owner, before the first public build |
| macOS notarisation path (Windows builds are unsigned, D-087) | Owner |
| Oxlint per-folder override syntax and `import/no-cycle` support in 1.86.0 | Spike S-12 in M0 |
| Whether `RIMSTUDIO_DATA` exists as a development override of the data roots | Boot code in M0; the portable marker is the decided mechanism |
| Linux webview environment variable workarounds | Spike S-10 |
| The first `docs/README.md` and `docs/glossary.md` files | Written with the docs index task; this document assumes their paths (the ADR template lives in `docs/adr/README.md`) |
