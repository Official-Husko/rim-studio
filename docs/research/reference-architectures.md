# Reference architectures for a modular Rust and Tauri codebase

Scope: how mature open-source desktop applications structure a Rust workspace with a Tauri (or comparable) shell, gathered to give the later layout stage of RimStudio evidence and vocabulary. Eight projects are studied in depth (GitButler, Yaak, Modrinth App, Cap, Spacedrive, Jan, Zed, rust-analyzer), three more in passing (Helix, kftray, Hoppscotch), plus the official Tauri documentation on project structure, state, IPC and plugins. The note then synthesises recurring patterns, anti-patterns, a layering vocabulary with an allowed-dependency matrix, a comparison of command-hosting strategies, enforceable rules, frontend layout evidence, and a comparison with RimSort and RimCrow. It does not design the RimStudio layout itself.

Status: research note | Last verified: 2026-10-04

How to read the evidence. Every project was cloned shallowly (blobless, scratch folder) on 2026-10-04 and analysed with small scripts (workspace member counts, internal dependency graph, fan-in and fan-out, longest dependency chain, line counts, workflow files). Numbers marked "measured" come from those scripts; they are counts of the checked-out revision named in section 1, not of later commits. Pointers are written `<repo>:<path>` relative to that repository's root. The licences of the studied projects vary (GPL, AGPL, FSL); only structure and ideas are taken, no code is copied, consistent with requirement R11. Statements that rest on reasoning rather than a measurement are marked "(reasoning)" or "(unverified)".

## 1. Projects studied

All repository facts were fetched on 2026-10-04. "Last commit" is the date of the cloned HEAD; every project below had a push within the 7 days before the access date, except Spacedrive's default branch (see its row).

| Project | URL | Licence (verified from file) | Last commit (cloned HEAD) | Rust workspace | Role in this study |
|---|---|---|---|---|---|
| GitButler | https://github.com/gitbutlerapp/gitbutler | FSL-1.1-MIT (`LICENSE.md`) | 2026-10-03 (`12cd332`) | 63 members | Tauri shell over a many-crate workspace, macro-generated API facade, CLI, HTTP server, Node binding |
| Yaak | https://github.com/mountain-loop/yaak | MIT | 2026-09-26 (`7f30253`) | 34 members | Directory-as-layer, one generic Tauri command, shared router, CLI crate |
| Modrinth App (monorepo `modrinth/code`) | https://github.com/modrinth/code | per package; app crate GPL-3.0-only (`COPYING.md`, `apps/app/Cargo.toml`) | 2026-10-02 (`71f35eb`) | 16 members | Domain analog: a game-mod launcher and manager on Tauri 2 |
| Cap | https://github.com/CapSoftware/Cap | AGPL-3.0 mostly, camera and scap crates MIT | 2026-10-01 (`a2a6bd8`) | 50 members | Capability-named crates, thick app crate, hakari |
| Spacedrive | https://github.com/spacedriveapp/spacedrive | FSL-1.1-ALv2 | main 2026-07-28 (`6dfeccf`); side branches `sources` and `ci-green-sources` committed 2026-10-04 | 24 members | One big core crate with internal layers, daemon plus thin Tauri shell |
| Jan (desktop client for local model inference) | https://github.com/janhq/jan | `LICENSE` file is Apache-2.0, but `Cargo.toml` says MIT (inconsistent metadata) | 2026-10-02 (`14a7206`) | no workspace, 10 manifests | Plugin crate per feature, headless CLI problem |
| Zed | https://github.com/zed-industries/zed | GPL-3.0-or-later for app crates, Apache-2.0 for `gpui` | 2026-10-03 (`a846890`) | 254 members | Non-Tauri: crate boundaries at scale |
| rust-analyzer | https://github.com/rust-lang/rust-analyzer | MIT OR Apache-2.0 | 2026-10-03 (`3332931`) | 45 members | Non-Tauri: written architecture invariants and tidy checks |
| Helix (brief) | https://github.com/helix-editor/helix | MPL-2.0 | 2026-09-29 (`ba40e54`) | 14 members | Small crate set with a responsibility table |
| kftray (brief) | https://github.com/hcavarsan/kftray | GPL-3.0 | 2026-09-30 (`456d946`) | 10 members | Two shells (Tauri and TUI) over shared crates |
| Hoppscotch (brief) | https://github.com/hoppscotch/hoppscotch | MIT | 2026-09-30 (`63273f8`) | none, 8 separate Cargo projects | Frontend kernel pattern, counter-example for Rust layout |

Rejected or demoted candidates: Spacedrive is kept despite its default branch being 68 days old at the access date because active work is visibly landing on side branches (commits dated 2026-10-04); Hoppscotch and Jan are kept as counter-examples because they have no Cargo workspace.

## 2. Per-project findings

### 2.1 GitButler

Evidence: `gitbutler:Cargo.toml`, `crates/AGENTS.md`, `crates/but-api`, `crates/but-api-macros`, `crates/gitbutler-tauri`, `electron-migration-assessment.md`, `.github/workflows/push.yaml`.

| Aspect | Finding |
|---|---|
| Crate count and naming | 63 members, 118 `[workspace.dependencies]` (measured). Two naming generations coexist: new `but-*` crates and legacy `gitbutler-*` crates picked up by a glob in `members`. Manifest comments rate crates as "Exemplary", "Needs work", "Soon obsolete" or "Legacy", a deliberate migration ledger inside `Cargo.toml`. |
| Layering | Longest internal dependency chain 16. Fan-in leaders: `but-core` 35, `but-testsupport` 25, `but-schemars` 24, `but-ctx` 24, `but-db` 19, `but-graph` 18, `but-error` 16. Fan-out leaders are the binaries and the facade: `but` (CLI) 41, `but-api` 40. `crates/AGENTS.md` states that lower-level crates must not depend on `but-api` and that DTOs live at the API boundary in a local `json` module. |
| Where commands live | One Tauri shell crate (`gitbutler-tauri`) with a single hand-maintained `generate_handler!` of roughly 214 commands. The commands are generated by an attribute macro (`#[but_api]` in the `but-api-macros` proc-macro crate): per function it emits the plain function, a `_json` Tauri command (behind a `tauri` feature), an HTTP handler for the server crate, and an optional Node binding. |
| Other transports | `but-server` (axum HTTP and WebSocket, localhost-only middleware), `but-napi` (napi-rs cdylib feeding `packages/but-sdk`), `but` (CLI and TUI). One logical API, four transports. |
| State sharing | A `Context` type (`but-ctx`) per repository: cheap to clone, lazy on-demand resources, not thread-safe, with a documented deadlock warning; exclusive and shared permission tokens (`RepoExclusive`, `RepoShared`) are passed down the call chain to make locking explicit. |
| Typed bindings | `but-ts` converts JSON Schema to TypeScript declarations. Its README explains why ts-rs was not used: ts-rs emits types as a side effect of running tests, which is awkward. CI has a job that verifies the generated SDK types are current. |
| Jobs, progress, cancellation | Not studied in depth here; undo is modelled as oplog snapshots, and frontend cache invalidation is declared on API functions with `provides` and `invalidates` tags. |
| Frontend | pnpm workspace (`apps/*`, `packages/*`, `crates/*`) with turbo. `apps/desktop` is SvelteKit; shared packages include `ui-svelte` (695 files), `ui-react` (561), `shared`, `core`, `but-sdk`. A migration spike (`electron-migration-assessment.md`, dated 2026-07-18) records that no `@tauri-apps` import exists outside `src/lib/backend/` and that the frontend talks to an `IBackend` interface of about 40 methods with a Tauri and a web implementation. |
| Lint and format | `[workspace.lints]` with `clippy::all = deny`; 61 of 63 crates opt in (measured). `rustfmt` width 100, edition 2024, pinned toolchain file ("should match CI"). ESLint with `import-x/no-cycle` as error and two custom rules: no relative import paths, and no cross-domain imports (at most 2 domains per file; `views/**` and `routes/**` are exempt as the composition layer). `knip` for dead code. |
| CI | Change detection, prettier, node lint/check/unit, SDK type freshness, `cargo fmt --check`, `cargo check --workspace --all-targets`, clippy with `-D warnings` in three feature variants, `cargo-machete`, `cargo-deny`, `cargo doc`, `cargo nextest` (excluding the Tauri crate and macro test crate), a Tauri feature-matrix check, a Windows check, and a gate job aggregating them. |
| Boundary enforcement | Convention (`AGENTS.md`) plus lints. `deny.toml` is the stock template: only licences are configured and `bans.deny` is empty (measured), so no dependency edge is machine-enforced. |
| Pain points recorded | The Electron spike states that "route-list drift is real": commands invoked by the frontend but absent from the HTTP server routing return "Command not found" because each shell hand-registers commands; it recommends a macro-generated registry. `apps/desktop/src/lib/LIB_PLAN.md` documents a reorganisation that cut import cycles from 30 to 2 (test-only) by moving domain utilities into feature modules. Dev profile uses `debug = "line-tables-only"` and `incremental = false`, with `opt-level = 3` for the git library crates only. |

### 2.2 Yaak

Evidence: `yaak:Cargo.toml`, `crates-tauri/yaak-app-client/src/{lib.rs,rpc_ext.rs}`, `crates/common/yaak-rpc*`, `crates/yaak-commands`, `packages/`, `.github/workflows/ci.yml`.

| Aspect | Finding |
|---|---|
| Crate count and naming | 34 members, 46 workspace dependencies (measured). All crates prefixed `yaak-`. |
| Layering | The directory is the layer: `crates/` ("Shared crates (no Tauri dependency)" per the manifest comment), `crates/common/` (database, rpc, rpc-schema), `crates-tauri/` (8 crates, the only ones depending on `tauri`), `crates-cli/yaak-cli`, `crates-server/` (web, playground), `crates-proxy/`. Dependency depth 0 to 7; fan-in leaders `yaak-models` 16, `yaak-templates` 10, `yaak-common` 7. |
| Where commands live | Exactly one Tauri command, `rpc`, taking an envelope of command name plus JSON payload, dispatched by a host-independent `RpcRouter` (`yaak-rpc`). A schema crate (`yaak-rpc-schema`) declares every command name, request and response once (113 entries counted). Each host provides adapters and the manifest says a missing adapter fails to compile. Command bodies live in `yaak-commands` against a `Host` trait; only window, updater and dialog commands live in the app crate. |
| State sharing | `app.manage(...)` of the router, connection and encryption managers and several `Mutex` wrapped handles (updater, notifier, gRPC, WebSocket manager), assembled in `lib.rs`. Small `*_ext.rs` adapter files per feature. |
| Typed bindings | ts-rs: `cargo test -p yaak-rpc-schema` writes a committed `gen_rpc.ts`. |
| Streaming | Events named `stream_<id>` with a caller-minted stream id so the frontend subscribes before the command starts. |
| Thickness of the shell | `yaak-app-client/src` is 6,857 lines, `lib.rs` 1,583 and `rpc_ext.rs` 1,239 (measured). Moderately thick for a design whose goal is a thin shell. |
| Platform features | Tiny Tauri plugin crates with permissions for OS specifics (`yaak-fonts`, `yaak-mac-window`), listed in `capabilities/default.json` as `yaak-fonts:default`. |
| Frontend | npm workspaces: `packages/{ui, theme, tailwind-config, model-store, common-lib, platform, plugin-runtime, ...}`, `packages/plugins/*` (about 45 TypeScript plugins), `apps/yaak-client` (React with TanStack Router; `components/core` 113 files, `components/<feature>`, `lib/`, `routes/`). `packages/platform` has Tauri and web implementations behind one entry (`index.ts` and `index.web.ts`). |
| Lint and CI | `rustfmt.toml` only; no `[workspace.lints]`, no `deny.toml`, no `clippy.toml` (measured). One CI job: frontend lint and tests, then `cargo test --all` with the wry feature. |
| Notable | Cargo features switch the webview engine (`cef` or `wry`) on the app crate; `[patch.crates-io]` pins tauri to a git revision. |

### 2.3 Modrinth App

Evidence: `modrinth_code:Cargo.toml`, `apps/app/build.rs`, `apps/app/src/{main.rs,api/}`, `packages/app-lib/`, `.cargo/config.toml`, `.github/workflows/`.

| Aspect | Finding |
|---|---|
| Crate count and naming | 16 members (apps and packages mixed); the app crate is `theseus_gui`, the library `theseus` (`packages/app-lib`, 221 Rust files). Names come from the code name, not a shared prefix. |
| Layering | Library (`theseus`) with `api` (106 files), `state` (92), `util`, `install`, `launcher`, `event`; the Tauri crate is a shell. Features on the library: `cli` (progress via indicatif), `tauri`, `export-ts`. |
| Where commands live | About 22 feature modules in `apps/app/src/api/*.rs`, each exposing `init() -> TauriPlugin` via `tauri::plugin::Builder::new("auth").invoke_handler(...)`. `build.rs` declares each as an inlined plugin with an explicit command list (about 281 quoted command names) and a default permission that allows all commands, with a comment that there is no better way until Tauri parses source for command attributes (Tauri issue 10075). So: plugin per feature without separate crates. |
| State sharing | Global singletons: a global `State` and an `EVENT_STATE` once-cell inside the library, not Tauri managed state. |
| Events and progress | A single `AppEvent` tagged enum is sent over one Tauri channel using binary postcard encoding; TypeScript types come from ts-rs plus postcard-bindgen. Progress is a `LoadingBar` record (uuid, message, total, current, kind) in a concurrent map; an RAII id removes the bar and emits "Completed" on drop; with the `cli` feature the same bars map to terminal progress bars. |
| Frontend | pnpm and turbo: `apps/app-frontend` (Vue 3, 460 files), `apps/frontend` (website), `packages/ui` (846 files, shared component library with Storybook, used by app and website), `packages/assets`, `packages/api-client`, `packages/utils`, `packages/tooling-config`. |
| Lint and CI | `[workspace.lints]` (clippy warn-level subset), all 16 crates opt in (measured); `clippy.toml`; `cargo-shear` in CI; turbo runs lint and test. One build workflow with per-channel Tauri config files. |
| Pain points recorded | `.cargo/config.toml` raises the Windows main-thread stack size with a comment that Windows overflows the stack when calling from Tauri. The `build.rs` comment above admits permission boilerplate. |

This project matters most to RimStudio: it is a mod manager for another game, it targets the three desktop platforms, and its library has a CLI mode that reuses the same progress model.

### 2.4 Cap

Evidence: `cap:Cargo.toml`, `.config/hakari.toml`, `apps/desktop/src-tauri/src/lib.rs`, `.github/workflows/ci.yml`.

| Aspect | Finding |
|---|---|
| Crate count and naming | 50 members: 49 under `crates/` (including a hakari "workspace-hack" crate) plus two apps. Crates are named by capability or platform (`camera-avfoundation`, `camera-directshow`, `enc-ffmpeg`, `rendering`, `recording`, `editor`, `export`, `project`). Depth up to 8. |
| Where commands live | Everything in one Tauri crate (`cap-desktop`), the only crate depending on `tauri`. Measured: 68 Rust files, 73,769 lines, `lib.rs` 11,091 lines, `recording.rs` 10,408, 175 `#[tauri::command]` functions (70 in `lib.rs`). This is the clearest "giant app crate" in the sample. |
| Typed bindings | tauri-specta with `specta` pinned to an exact release-candidate version as a workspace dependency; bindings exported at debug startup; events through a derive. |
| Concurrency model | The recording crate uses an actor framework plus channels. |
| Build-time lessons (from manifest comments) | Fat LTO "cost about 60 minutes" of the Windows release job (18 minutes CLI link, 41 minutes desktop link), so release uses thin LTO, `opt-level = "s"`, one codegen unit and line-tables-only debug info. `hakari.toml` explains that leaving a crate out of the unification would make the CLI sidecar and the desktop app resolve different feature sets and recompile about 500 shared crates in one job. |
| Lint and CI | 12 clippy lints at deny level in `[workspace.lints]`, 48 of 50 crates opt in. CI has change detection, typecheck, Biome format, `cargo fmt`, clippy over an OS matrix with `--workspace --all-features --locked -D warnings`, plus dedicated workflows for recording reliability, performance regressions, Windows targets, and migration journal validation. |
| Frontend | bun and turbo: `apps/{web (Next.js), desktop (SolidJS, 632 files), desktop-gpui (experimental), mobile, chrome-extension, media-server, render-farm, cli}`, `packages/{ui-solid, ui, database, web-backend, web-domain, recorder-core, utils, config, ...}`. |

### 2.5 Spacedrive

Evidence: `spacedrive:Cargo.toml`, `core/src/{lib.rs,ops,infra,service,domain}`, `crates/task-system`, `apps/tauri/src-tauri`, `.github/workflows/`.

| Aspect | Finding |
|---|---|
| Crate count and naming | 24 members and 33 manifests (measured). The member list contains commented-out legacy entries (old desktop, cloud and mobile apps), visible rewrite history. |
| Layering | One big core crate (`sd-core`, 871 Rust files) with layers as modules: `ops` (476 files, CQRS style operations), `infra` (155: db 75, sync, job, event, wire, api), `service` (80), `domain` (22), `volume`, `library`, `crypto`. A `Core` struct composes the managers (device, library, volume, event bus, log bus, services). Leaf crates exist for reusable parts (`task-system`, `fs-watcher`, `crypto`, `ffmpeg`, `images`, `actors`). |
| Where commands live | The Tauri shell (`apps/tauri/src-tauri`, 3,472 lines in total, `main.rs` 2,251) exposes few generic commands (`daemon_request`, `subscribe_to_events`, `unsubscribe_from_events`, daemon start and install). The core runs as a separate daemon speaking JSON-RPC 2.0 over a socket. |
| Jobs | `infra/job` (manager, registry, executor, progress, handle, context) on top of the `task-system` crate: a work-stealing scheduler with pause, cancel and abort, which on shutdown returns pending tasks to their dispatchers so they can be persisted. A derive macro (`job-derive`) removes registration boilerplate. |
| Typed bindings | A forked specta generating TypeScript and Swift clients (`packages/ts-client`, `packages/swift-client`). |
| Frontend | bun workspaces: `packages/{assets, interface (280 files), ts-client, swift-client, config}`, `apps/{tauri, mobile, cli, server, web}`. |
| Build conventions | `default-members` excludes the Tauri apps ("it requires the frontend to be built first"), so plain `cargo build` and `cargo test` skip the shell. An `xtask` crate (setup, mobile builds, test-core, bump). |
| Pain points visible in code | `core/src/lib.rs` begins with a crate-wide `#![allow(warnings)]`; no `[workspace.lints]`; `cargo test --workspace` is commented out in CI and replaced by an xtask running the core tests; a very large single crate whose internal layering is convention only. |

### 2.6 Jan

Evidence: `jan:src-tauri/{Cargo.toml,src/lib.rs}`, `plugins/tauri-plugin-llamacpp/{build.rs,src/lib.rs}`, `jan-cli`, `.github/workflows/rust-check.yml`.

| Aspect | Finding |
|---|---|
| Crate count and naming | No Cargo workspace: 10 manifests and 6 separate lockfiles. The app crate depends by path on `tauri-plugin-{agent-tools, hardware, llamacpp, mlx, rag, vector-db, websearch}` and a utility crate. CI tests the crates one by one with `--manifest-path`. |
| Where commands live | Two levels: about 96 app commands hand-registered in a macro in `lib.rs` (138 files under `src/core/`), plus plugin commands. Each plugin has `build.rs` calling `tauri_plugin::Builder::new(COMMANDS).build()` (the llamacpp plugin lists 19 names) and `init()` building a plugin with `invoke_handler` and a `setup` closure that does `app.manage(Arc::new(State::new()))`. |
| Plugin gotcha, documented in a test | A unit test named `every_registered_command_has_a_permission` exists because a command listed in `generate_handler!` but missing from the `build.rs` list compiles, passes tests, and fails at runtime with "not allowed. Command not found". The same drift class as GitButler's route lists, one level lower. |
| Tauri-free core | Plugins compiled with default features off form a "tauri-free core"; the `tauri` feature adds the command surface. Engine C++ build is behind a feature so a CLI-only check does not need cmake. |
| Headless CLI problem | `jan-cli` is a standalone crate outside any workspace. Reasons recorded in the repo: the Tauri CLI bundles every `[[bin]]` regardless of required features; the `cli` and `tauri-app` features of the app crate are mutually exclusive (`compile_error!`) with 15 `cfg(not(feature = "cli"))` gates in a 566-line `lib.rs`; GTK enters through tauri's menu dependency so the dependency must be absent, not merely unused; the keyring must use the async secret-service backend so a headless host without a session bus can run. CI builds desktop, cli and e2e configurations separately because they are mutually exclusive configurations of one crate. This is the strongest evidence for keeping the CLI in its own crate that never depends on the Tauri shell. |
| Capabilities | Seven capability files, one per window or platform. |
| Frontend | Yarn workspaces: `core` (TypeScript SDK and event bus), `web-app` (React, TanStack Router; folders by kind: `containers`, `hooks`, `services`, `components`, `stores`, `routes`), `extensions/*` (frontend plugin system), `packages/*`. |
| Build | Release profile is size first (`opt-level = "z"`, fat LTO, one codegen unit). Toolchain file pins Rust 1.98.0 (the other studied repos pin 1.88 to 1.96). |

### 2.7 Zed (non-Tauri, boundaries only)

Evidence: `zed:Cargo.toml`, `clippy.toml`, `tooling/xtask`, `tooling/lints`, `script/crate-dep-graph`.

| Aspect | Finding |
|---|---|
| Size | 254 members, 519 workspace dependencies, longest chain 25, 38 crates with no internal dependencies (measured). Fan-in leaders: `util` 126, `settings` 102, `collections` 97, `language` 77, `project` 75, `workspace` 74, `ui` 71. Binaries (fan-in 0): `zed` (fan-out 140), `collab`, `remote_server`, `cli`, `xtask`. |
| Naming and split | snake_case feature names; 13 `*_ui` crates separate view code from logic (`git`/`git_ui`, `agent`/`agent_ui`, `settings`/`settings_ui`, `debugger`/`debugger_ui`). The split leaks: a few non-ui crates (`vim`, `project_panel`) depend on `*_ui` crates (measured). |
| Lints | `[workspace.lints]` clippy with `dbg_macro`, `todo`, `redundant_clone` and `disallowed_methods` at deny; the style group is allowed with a comment that the full clippy run "can take several minutes". 249 of 254 crates opt in. |
| Disallowed APIs | `clippy.toml` bans `std::process::Command` spawn calls (use the async runtime's process type), a timer type (test non-determinism) and `serde_json::from_reader` (performance). A dylint library with a pinned nightly (kept outside the workspace) adds domain lints such as blocking IO on the foreground thread. |
| Tooling | `xtask` crates: package conformity (reports crates not inheriting workspace lints or dependencies; reports, does not fail), gpui validation built on the `guppy` crate graph (licence and unique publish names), licence checks; GitHub workflow YAML is generated from Rust code. `script/crate-dep-graph` runs `cargo depgraph --workspace-only` with root filters. |

### 2.8 rust-analyzer (non-Tauri, written invariants)

Evidence: `rust-analyzer:docs/book/src/contributing/architecture.md`, `xtask/src/tidy.rs` area, `.github/workflows/ci.yaml`, `clippy.toml`.

| Aspect | Finding |
|---|---|
| Size | 45 members (`crates/*`, `lib/*`, `xtask`), 87 workspace dependencies, depth up to 14, 44 of 45 crates opt into workspace lints (measured). Fan-in: `stdx` 24, `syntax` 17, `span` 16, `intern` 16, `test-utils` 14. |
| Architecture document | About 25 explicit "Architecture Invariant" statements, for example: the `syntax` crate is independent of everything else; the database crate does not know about the file system or paths; only the top crate knows the language-server protocol and JSON serialization; the public `ide` API is built of plain-old-data types with public fields; if a type is serializable it is part of some IPC boundary; two crates "will never be an API boundary". Three real API boundaries are named (syntax, hir, ide, plus the protocol layer). |
| Lesson recorded | The document warns against waiting for first users before stabilising a boundary. |
| Enforcement | `xtask tidy` checks manifest rules (internal dependency version rules, licence set, test attributes, module docs present, trailing whitespace), a `codegen --check`, CI with warnings denied plus `unreachable-pub`, `cargo machete`, a miri job on one crate, cross-target checks, and a dependency-edge guard: a CI step that fails if `cargo tree -i salsa` shows the incremental-computation crate reachable from the proc-macro server crates. `clippy.toml` bans std hash maps (use the faster hasher) and raw process creation (forces a chosen working directory). Lints are warn locally and raised to deny in CI. |

### 2.9 Brief notes: Helix, kftray, Hoppscotch

| Project | Finding | Evidence |
|---|---|---|
| Helix | 14 crates under `helix-*` plus `xtask`; `default-members` is only the terminal crate; `docs/architecture.md` is a table from crate to responsibility; it admits the view layer "was supposed to be" frontend-agnostic but is now tied to the terminal UI (boundary erosion). The event crate bundles debounced async hooks and cancellation. CI runs `cargo test --workspace`, an integration test alias, fmt, clippy `-D warnings`, `cargo doc` with private items, and xtask checks of generated documentation. | `helix:docs/architecture.md`, `.github/workflows/build.yml` |
| kftray | 10 crates under `crates/*`: commons (models, db; fan-in 7), port-forward, network monitor, http logs, shortcuts, helper binary, server binary, MCP, TUI binary, Tauri shell. The shell has 15 command files and 106 registered commands in 12,371 lines. The TUI and the Tauri shell share the same crates: two shells over one core. Tests expose the global-state tax: global lazy DB pools plus a global async mutex to serialise tests, because a pool outliving its runtime leaks connections. | `kftray:crates/`, `kftray-commons` tests |
| Hoppscotch | No Rust workspace (8 separate Cargo projects; the desktop shell takes two plugin crates as git dependencies pinned by revision while also carrying copies in `plugin-workspace/`, a duplication risk). Frontend is the interesting part: `hoppscotch-common` (1,031 files) is shared by web, self-hosted and desktop shells, and `hoppscotch-kernel` defines versioned platform ports (io, relay, store, log) with web and desktop implementations behind `window.__KERNEL__`. | `hoppscotch:packages/` |

### 2.10 Official Tauri documentation

Sources (accessed 2026-10-04): https://v2.tauri.app/start/project-structure/ , https://v2.tauri.app/develop/plugins/ , https://v2.tauri.app/develop/calling-rust/ , https://v2.tauri.app/develop/state-management/ , https://v2.tauri.app/security/capabilities/ (the first, third, fourth and fifth also read as raw files from the `tauri-docs` repository, `v2` branch). Cross-reference: `docs/research/webview-and-ipc-performance.md` for measured IPC behaviour and `docs/research/rust-crate-research.md` section 3 for crate versions.

| Topic | What the docs say (own words) |
|---|---|
| Project structure | The JavaScript project is at the top level and the Rust project is `src-tauri/`; the doc says that for a Rust-only focus `src-tauri` can be used as the top level or as a member of a Rust workspace. `lib.rs` holds the app code and the mobile entry point, `main.rs` only calls into it, because mobile builds compile the app as a library. |
| Commands | Arguments are a JSON object with camelCase keys; return values and errors must implement `Serialize`; the docs recommend a custom serialisable error type. Commands may live in separate modules but must be `pub`. `generate_handler!` takes all commands in one call; registering twice replaces the earlier list. Channels are the recommended way to stream data; commands can also read the raw request body. |
| State | Managed state is shared through `State`; mutation needs interior mutability; a plain `std::sync::Mutex` is usually fine in async code unless a guard is held across an await point, in which case an async mutex is needed. |
| Plugin anatomy | The CLI scaffolds `tauri-plugin-<name>` with `src/{commands.rs, desktop.rs, mobile.rs, error.rs, lib.rs, models.rs}`, `build.rs`, a `permissions/` folder, optional `android/` and `ios/`, and optional `guest-js` API bindings. `build.rs` calls `tauri_plugin::Builder::new(COMMANDS)` with the command names in snake_case, and each name auto-generates an `allow-<name>` and a `deny-<name>` permission. Permission sets group permissions; a plugin's `setup` hook can register state with `manage`. |
| Capabilities | App commands registered with `invoke_handler` are allowed for every window by default; `AppManifest::commands` in the app's `build.rs` restricts them. Plugin commands require explicit capability grants per window and platform. |
| Workspace in the Tauri repo itself | The Tauri repo is a workspace of `tauri`, `tauri-build`, `tauri-codegen`, `tauri-macros`, `tauri-runtime`, `tauri-runtime-wry`, `tauri-utils`, `tauri-plugin` and tooling crates, and `ARCHITECTURE.md` describes each in one paragraph. The official plugins live in a separate workspace (`plugins-workspace`, 30 plugin folders at the cloned revision). |
| Version facts | From the registry metadata fetched on 2026-10-04: `tauri` 2.12.1, `tauri-build` 2.7.1, `tauri-plugin` 2.7.1; `tauri-specta` and `specta` stable releases are 1.x for Tauri 1 and the Tauri 2 line is `2.0.0-rc.25` (published 2026-05-08). |

## 3. Cross-project comparison

| Dimension | GitButler | Yaak | Modrinth | Cap | Spacedrive | Jan | Zed |
|---|---|---|---|---|---|---|---|
| Members | 63 | 34 | 16 | 50 | 24 | 0 (no workspace) | 254 |
| Shell command style | one big `generate_handler!` of macro-generated commands | one `rpc` command with envelope | inlined plugin per feature module | 175 commands in one app crate | few generic commands to a daemon | app commands plus plugin crates | not applicable |
| Crates depending on tauri | 1 shell (plus facade feature) | 8, all under `crates-tauri/` | 1 shell plus lib feature | 1 | 1 shell | app plus plugins | 0 |
| Bindings | JSON Schema to TS (`but-ts`) | ts-rs, committed output | ts-rs plus postcard-bindgen | tauri-specta, pinned rc | forked specta | not determined | not applicable |
| Workspace lints | yes (61 of 63) | no | yes (16 of 16) | yes (48 of 50) | no | not applicable | yes (249 of 254) |
| Dependency policy file | stock deny.toml, no bans | none | none found | none found (hakari) | none found | none found | custom xtask, dylint |
| Headless CLI crate | `but` | `yaak-cli` | library `cli` feature | `apps/cli` | `apps/cli` | `jan-cli`, outside workspace | `cli` |
| Frontend packages | many (ui-svelte, shared, sdk) | `ui`, `theme`, `platform`, `model-store` | `ui`, `assets`, `api-client`, `utils` | `ui`, `ui-solid`, `utils` | `interface`, `ts-client`, `assets` | `core`, `web-app`, `extensions` | not applicable |

## 4. Recurring structural patterns

1. **A Tauri-free core and a thin shell.** Yaak states it in a manifest comment and puts all `tauri` dependents under one directory; GitButler gates Tauri behind a feature of the facade; Modrinth gates it behind a library feature; Jan builds plugin crates as a "tauri-free core" by default; Spacedrive moves the whole core into a daemon; kftray shares crates between a Tauri shell and a TUI. Evidence: sections 2.1 to 2.6, 2.9. This is the single most consistent finding.
2. **One logical API, many transports.** GitButler generates Tauri, HTTP and Node entry points from one annotated function; Yaak dispatches one envelope through a router that any host can adapt; Spacedrive speaks JSON-RPC to a daemon. The consequence is that the CLI, a future server mode and tests call the same functions as the UI.
3. **A schema or registry as the single source of command names.** Yaak's `with_commands!` schema and GitButler's `#[but_api]` macro exist because hand-listing commands per shell drifts (GitButler's electron spike, Jan's permission-drift test, Modrinth's build.rs comment). Registries that fail compilation on a missing adapter are preferred over runtime failures.
4. **Directory or naming as layer.** Yaak's directories (`crates`, `crates-tauri`, `crates-cli`, `crates-server`), Zed's `*_ui` suffix, Helix's responsibility table, GitButler's `but-` versus `gitbutler-` and manifest ratings. Layers are visible without opening a manifest.
5. **A small set of very high fan-in leaf crates.** `but-core`, `yaak-models`, `util`, `stdx`, `cap-media-info`. Leaves hold types and utilities and depend on nothing internal; the churn-prone crates sit at the top. Zed's `util` (126 dependents) and `collections` (97) show the stable base staying small and boring.
6. **Workspace-wide lint inheritance.** `[workspace.lints]` with opt-in per crate is used by 5 of the 8 deep-studied projects, and in each case almost every member opts in (61/63, 16/16, 48/50, 249/254, 44/45). Policy beyond clippy defaults goes into `clippy.toml` disallowed lists (Zed, rust-analyzer).
7. **Long-running work is a first-class model, not an ad hoc thread.** Spacedrive's registry, executor and task-system; Modrinth's loading-bar records with RAII completion and a CLI mapping; Cap's actors; Helix's event crate with cancellation; Yaak's caller-minted stream ids. The shared elements: an id minted by the caller, a registry keyed by id, a channel back to the UI, an explicit cancel path.
8. **Frontend isolated behind a platform adapter.** GitButler `IBackend` (zero Tauri imports elsewhere), Yaak `packages/platform`, Hoppscotch `hoppscotch-kernel`. It enables a web build, tests with a fake backend and an eventual shell swap.
9. **CI as a gate job over change detection.** GitButler, Cap and Jan split work per area, run per-OS matrices for clippy, and build the shell separately from the core (GitButler excludes the Tauri crate from nextest; Spacedrive excludes shells from `default-members`).

## 5. Anti-patterns seen

| Anti-pattern | Where | Cost observed |
|---|---|---|
| Giant app crate holding every command | Cap (73,769 lines, 175 commands, 11k-line `lib.rs`) | Every feature change recompiles one huge crate; no feature is testable without the shell. The release link took about 60 minutes with fat LTO before being tuned. |
| Hand-registered command lists per shell | GitButler (214 commands, drift between Tauri and HTTP), Jan (96 plus plugin lists) | Runtime "Command not found" failures that compile and pass tests. |
| Permission list duplicated from handler list | Jan and Modrinth plugin `build.rs` lists; Modrinth's comment links an upstream issue | Boilerplate and a silent drift class; Jan added a test to catch it. |
| Mutually exclusive features on one crate to get a CLI | Jan (`cli` versus `tauri-app`, 15 cfg gates in one file) | Neither configuration proves the other compiles; CI must build each; the CLI had to leave the workspace. |
| No workspace | Jan, Hoppscotch | Six lockfiles, per-crate CI invocations, duplicated plugin copies pinned by git revision. |
| One enormous library crate with convention-only layering | Spacedrive `sd-core` (871 files, `#![allow(warnings)]`) | Layers inside a crate cannot be enforced by Cargo; lints are suppressed wholesale. |
| UI split that leaks | Zed (`vim`, `project_panel` depending on `git_ui`), Helix (view layer tied to terminal) | The boundary exists in names but not in the graph; erosion is documented by maintainers. |
| Global singletons for state | Modrinth (`State`, `EVENT_STATE`), kftray (lazy DB pools with a global test mutex) | Tests must serialise; hidden coupling; a Windows stack-size workaround appears in Modrinth's cargo config. |
| Import cycles in the frontend | GitButler `lib/` (30 cycles before reorganisation) | A reorganisation project of its own; fixed by feature modules and a lint rule. |
| Policy by comment only | GitButler `AGENTS.md`, `deny.toml` stock with empty bans | Rules are not machine-checked; compare rust-analyzer, where invariants are mostly checked by tidy. |
| Business logic inside views or stores | RimSort `views/mods_panel.py` (5,820 lines with warning calculation), RimCrow `issues.js` store (645 lines of rules in the frontend) | Rules cannot be tested or reused outside the UI. See section 10. |

## 6. Recommended layering vocabulary for RimStudio

This is vocabulary and a matrix for the later layout stage, not a crate list. Names in the first column are the proposed terms; crate prefix `rimstudio-` applies. Counts of crates per layer are intentionally absent.

| Layer | Meaning | Examples of content (illustrative) | May use platform IO? | May depend on Tauri? |
|---|---|---|---|---|
| L0 domain | Plain data types and pure functions: ids, version ranges, mod metadata, rule types, DTOs, error codes. No IO, no async runtime. Serializable types here are part of an IPC boundary (rust-analyzer's invariant). | mod identity and version types, def reference types, rule edge types | no | no |
| L1 infrastructure | Adapters to the outside world, each behind one crate: XML boundary (requirement R10: one parser crate), Steam VDF and ACF reading, filesystem scanning, HTTP fetching, SQLite or JSON stores, JSONC configuration. | xml boundary crate, vdf reader, http client wrapper, config store | yes | no |
| L2 services | Engines that combine domain types and infrastructure without knowing the UI: sorting, rule merging, dataset fetch and update, game and Steam detection, patch application, calibration maths. Pure where possible. | sorter, rule merger, detector | via L1 only | no |
| L3 features | Use cases a user performs, one per module of the product: mod manager, def explorer, patch tooling, item designer, workshop uploader, project workspace. Each exposes a typed API (request and response DTOs, job starters) and no transport. Features never depend on each other; they compose in the shell, or through L2. | the 12 to 20 product modules | via L1 only | no |
| L4 app shell | Tauri application: window setup, command adapters, state wiring, capabilities, updater, event bridging. No business rules. | the Tauri crate (and any OS-specific plugin crates for window effects) | yes | yes |
| L4 CLI | Headless binary over L3 and L2 for scripting, tests and CI. Never depends on L4 shell or Tauri. | the CLI crate | yes | no |
| Support | `testing` (fixtures, fake backends, temp installs), `contract` (command registry and generated bindings), `xtask` (repo automation). Not a layer; allowed per matrix below. | | | |

Allowed dependency matrix (row depends on column; Y allowed, N forbidden, S same-layer allowed only as an explicit acyclic exception recorded in the manifest):

| Dependent \ Dependency | L0 domain | L1 infra | L2 services | L3 features | L4 shell | L4 CLI | contract | testing |
|---|---|---|---|---|---|---|---|---|
| L0 domain | S | N | N | N | N | N | N | N |
| L1 infra | Y | S | N | N | N | N | N | N |
| L2 services | Y | Y | S | N | N | N | N | N |
| L3 features | Y | Y | Y | N | N | N | Y | N |
| L4 shell | Y | Y | Y | Y | N | N | Y | N |
| L4 CLI | Y | Y | Y | Y | N | N | Y | N |
| contract | Y | N | N | N | N | N | S | N |
| testing (dev-dependency only) | Y | Y | Y | Y | N | N | Y | S |

Rationale per row, tied to evidence: L0 empty of internal edges mirrors `syntax`, `stdx` and `but-core` as the high fan-in base; L1 never depends upward (Zed's `util` and `collections`); L3 features not depending on each other follows GitButler's no-cross-domain rule on the frontend and avoids the `vim` to `git_ui` leak; the shell and CLI as siblings that never depend on each other follows Yaak (`crates-tauri` versus `crates-cli`) and Jan's headless-CLI lessons; the contract crate depends only on L0 so transports can use it without pulling services.

## 7. Command hosting strategies for a 12 to 20 module app

Strategies as seen in the sample: A) all commands in the app crate (Cap, kftray); B) inlined plugin per feature inside the app crate (Modrinth); C) one plugin crate per feature (Jan, official plugins); D) one generic `rpc` command over a Tauri-free router (Yaak); E) macro-generated facade with per-transport expansion (GitButler). Compile time claims are reasoning from structure, not measurements; a generator for a synthetic comparison workspace was prepared in the scratch folder but not run, in keeping with the rule against large builds (see Open questions).

| Criterion | A: app crate | B: inline plugins | C: plugin crate per feature | D: single rpc envelope | E: macro facade |
|---|---|---|---|---|---|
| Compile time, incremental (reasoning) | Worst: every wrapper change recompiles the whole shell, and Tauri macro expansion sits in one crate (Cap) | Same crate as A, modules only | Better: wrappers compile in parallel and only the changed plugin and the shell relink | Best for logic: handlers live in Tauri-free crates, the shell changes rarely | Good: wrappers are generated beside the function in the feature crate, but proc-macro cost is paid per function |
| Cold build | One large crate | Same | Many small crates, each pulling `tauri`; extra manifests and build scripts | Small shell | Proc-macro crate must build first |
| Testing without a webview | Poor (needs Tauri test feature or mock) | Poor | Command wrappers testable per crate with mock runtime; logic needs the "tauri-free default" convention (Jan) | Excellent: router is called directly | Excellent: plain function is the unit under test, wrappers trivial |
| Boilerplate per command | Low (attribute plus one list line) | Medium (list in `build.rs` and in handler) | Highest (COMMANDS list, handler list, permission files, plugin init, optional guest bindings) | Low per command (one schema entry and one handler) but adapters per host | Lowest per command after the macro exists; macro itself is a maintained crate with UI tests |
| Permission granularity | App-wide by default, restrictable via `AppManifest::commands` | Per inlined plugin, usually allow-all | Native: per-plugin permissions and capabilities | Coarse: one command, so Tauri cannot restrict by feature; a router-level policy is needed | Per shell; capability file lists generated names |
| Drift risk | List versus attribute | list versus `build.rs` (Jan test exists for this) | permission list versus handler list | Low if the schema is the only list and adapters must compile | Low if the registry is macro-generated; GitButler still hand-lists in the shell |
| Typed bindings | specta or ts-rs over commands | same | per plugin guest JS or specta | Schema crate drives generation (Yaak: ts-rs) | JSON Schema path (GitButler) |
| Fits a CLI sharing the same API | Poor | Poor | Medium (core crates are shareable, wrappers are not) | Excellent | Excellent |
| Performance note | Native Tauri argument decoding | same | same | Extra envelope decode and string dispatch per call (small; unmeasured); channels still apply for streams | same as A for the Tauri path |
| Fit for 12 to 20 modules | Acceptable only if the shell stays thin; Cap shows the failure mode | Reasonable middle path if permissions per feature matter | Heavy at this size; justified only for OS-specific capabilities | Strong if one generic permission posture is acceptable (RimStudio's security design already avoids `fs`, `shell` and `http` plugins on the frontend, per `docs/research/webview-and-ipc-performance.md`) | Strong, at the cost of owning a macro crate |

Reading of the evidence: strategies D and E both solve the same problem from different ends, and both put logic in Tauri-free crates first. Plugin crates per feature (C) are the Tauri-official recommendation for reusable, permissioned units but, in the sample, only Jan uses them for features and it pays with boilerplate and a drift test; Modrinth chose inlined plugins to keep permissions without separate crates and documents the boilerplate as a limitation.

## 8. Enforceable rules

The table lists mechanisms with verification status. Tools were checked against registry metadata on 2026-10-04 (versions below).

| Mechanism | What it enforces | Verification | Source |
|---|---|---|---|
| cargo-deny `[bans] deny` with `wrappers` | Banned crate may be depended on directly only by listed crates; all other transitive reach is denied. Example uses: allow `quick-xml` only through the xml boundary crate (R10), allow `tauri` only through the shell and OS plugin crates. | Semantics verified in the cargo-deny docs (`docs/src/checks/bans/cfg.md`, accessed 2026-10-04); not executed here because the tool is not installed. cargo-deny 0.20.2 (2026-07-09). | https://github.com/EmbarkStudios/cargo-deny |
| cargo-deny `[bans.workspace-dependencies]` | Duplicate declarations not using `workspace = true` and unused workspace dependencies are errors by default. | Verified in the same doc. | same |
| Layer tag in each manifest plus a metadata check | Every workspace member declares its layer in `[package.metadata.rimstudio]`; a script rejects edges not allowed by the matrix and rejects untagged members. | Executed on a toy workspace in the scratch folder (5 crates: types, xml, core, cli, app; layers domain, infra, service, cli, shell): the jq filter below printed nothing for a conforming graph and reported an untagged member (a stub of tauri placed inside the workspace) as an error, which confirms detection of unlayered members. A real `tauri` from the registry is a non-member and is skipped by the filter. | scratch `toy/` |
| `cargo tree -i <crate> -p <member>` guard | Fails CI if a banned crate is reachable from given members (rust-analyzer's check against `salsa`). Use for "CLI must not reach tauri or gtk". | Pattern verified in rust-analyzer CI; command semantics standard cargo. | `rust-analyzer:.github/workflows/ci.yaml` |
| `[workspace.lints]` plus `lints.workspace = true` | Shared rustc and clippy policy; combine with a check that every member opts in (Zed's conformity task reports, does not fail; a failing variant is a better default for a new project). | Pattern verified (5 projects). | sections 2.1, 2.3, 2.4, 2.7, 2.8 |
| `clippy.toml` `disallowed-methods` and `disallowed-types` | Project API policy, for example blocking raw `std::process::Command` (rust-analyzer and Zed both do), `std::fs` calls outside the IO crate, or the second XML parser. | Pattern verified; the toy workspace sets `disallowed_methods` and `disallowed_types` to deny. | Zed, rust-analyzer `clippy.toml` |
| `cargo-modules` | Inspects module trees and orphans inside a crate; useful for the Spacedrive-style "layers as modules" case. | Latest 0.27.0 (2026-08-03); flags used for checks not verified (unverified). | https://crates.io/crates/cargo-modules |
| `cargo-depgraph` | Renders the workspace graph (`--workspace-only`), as Zed's script does. | Registry shows the last release 1.6.0 on 2023-12-07, so treat as stable but unmaintained; the same graph can be produced from `cargo metadata`. | https://crates.io/crates/cargo-depgraph |
| `guppy` | Rust API over `cargo metadata` for custom graph rules (Zed's xtask); the better base for an `xtask check-layers` than shell scripts. | 0.19.1 (2026-09-24). | https://crates.io/crates/guppy |
| `cargo-shear` or `cargo-machete` | Unused dependencies, so removed code drops its edges. | Used by Modrinth (shear) and GitButler (machete); shear 1.14.0, machete 0.9.2. | sections 2.1, 2.3 |
| `cargo-hakari` | Unifies features so shell and CLI builds share compiled dependencies; costs a generated crate and care with dev-dependency features. | Cap's comments document both the benefit (avoid recompiling about 500 crates) and the trap. hakari 0.9.39. | `cap:.config/hakari.toml` |
| `cargo-nextest` | Fast test runner; used by GitButler, rust-analyzer, kftray with llvm-cov. | 0.9.146. | sections 2.1, 2.8, 2.9 |
| `-D warnings` and `unreachable-pub` in CI only | Local warnings, CI errors (rust-analyzer policy). | Pattern verified. | rust-analyzer CI |
| Generated-output freshness job | CI regenerates bindings and fails on a diff (GitButler SDK check, Yaak committed `gen_rpc.ts`, Helix docs check). | Pattern verified. | sections 2.1, 2.2, 2.9 |
| Tidy-style test for architecture documents | A test requires module docs and keeps the architecture document's invariants checkable. | rust-analyzer tidy verified. | section 2.8 |

The layer check filter used in the toy experiment (about 15 lines of jq, input is `cargo metadata --format-version 1 --no-deps`) is a fair starting point: it builds a map of workspace members to their declared layer, loops over non-dev dependencies that point to other workspace members, and prints one line per edge not present in an allowed-edges object. Dev dependencies are skipped so `testing` can be used from tests. The fuller version belongs in an `xtask` using `guppy`, which also checks that the CLI crate has no path to `tauri` (the Jan lesson).

Frontend equivalents: `import-x/no-cycle` as an error and a custom cross-feature rule (GitButler); `knip` for dead exports; a CI job that fails on a stale generated bindings file. Specific plugin names beyond those found in GitButler's config are not verified here.

## 9. Frontend layout evidence

| Question | Evidence |
|---|---|
| Feature-sliced design (FSD) used? | Not by any studied project, by name. Feature-folder organisation is common without the formal FSD layers: RimCrow `frontend/src/features/*`, Yaak `components/<feature>`, GitButler feature modules under `lib/` after its cycle clean-up, Modrinth and Cap by app. Jan uses folders by kind (`containers`, `hooks`, `services`). Whether FSD would fit is a design decision for the layout stage; the evidence only shows that the lighter "feature folder plus enforced import rules" form is what is working in production. |
| Packages versus one app | The shell apps with a UI kit have packages: Yaak (8 core packages), Modrinth (`ui` with Storybook shared with a website), Cap and GitButler (two or three UI kits). Single-package frontends exist for products with one UI: kftray (one `frontend` package, 118 components), Jan (`web-app` plus an SDK package). Packages were created in response to a second consumer (web or marketing site, second shell), not up front. |
| What is usually extracted | A UI component kit; design tokens or theme; the generated IPC client; the platform adapter; sometimes shared domain utilities (`shared`, `common-lib`, `utils`). |
| Platform adapter | GitButler `IBackend`, Yaak `platform`, Hoppscotch `kernel`: one module owns every import of the Tauri API. |
| Routing and state | TanStack Router in Yaak and Jan (generated route tree in Jan); stores per domain (Yaak `model-store`). |
| Frontend boundary enforcement | GitButler: relative-import ban, cross-domain rule with a composition-layer exemption, `no-cycle`, knip. Others: not found. |
| Tooling | Biome (Cap, kftray), ESLint plus prettier (GitButler), turbo for task orchestration (GitButler, Modrinth, Cap, Spacedrive); pnpm used by GitButler, Modrinth, kftray; Yaak npm, Cap and Spacedrive bun, Jan yarn. A supply-chain setting appears in two pnpm workspaces (minimum release age 2880 and 10080 minutes). |

## 10. Comparison with the two reference mod managers

| Aspect | RimSort (Python, PySide6) | RimCrow (Python, pywebview, Vue 3) | What the reference architectures suggest |
|---|---|---|---|
| Size and shape | 63,410 lines under `app/` (measured with `wc -l`); layout `controllers/`, `models/`, `views/`, `windows/`, `utils/`, `sort/`, `services/`, `cli/` | Backend 111 Python files, 68,101 lines; frontend about 65,100 lines (from `docs/research/rimcrow-analysis.md`) | Both are single-process monoliths by package; neither enforces layers. |
| Largest files | `views/mods_panel.py` 5,820; `views/main_content_panel.py` 3,275; `utils/update_utils.py` 2,658; `controllers/main_content_controller.py` 2,134; `utils/git_utils.py` 1,867 | `backend/api.py` 9,004 lines with about 391 methods as the only bridge class; `mgr_steam.py` 3,664; `mgr_texture_opt.py` 3,472; frontend `workspaceStore.js` 2,699 | Large files concentrate in the layer that mixes concerns: the view (RimSort) and the single facade (RimCrow). Compare Cap's 11k-line `lib.rs`. |
| Domain core | About 3,900 lines in six files plus `sort/*`; `models/mod_list.py` (413 lines, newer pure model) is imported by nothing else (from `docs/research/rimsort-module-inventory.md`) | Sorter and rules in `managers/mgr_sorter.py` (1,200) and `mgr_rules.py` (1,335); issue detection lives in a frontend store (`issues.js`, 645 lines) | Domain logic is a small fraction of the code in both. In RimStudio it belongs in L0 and L2 crates with the CLI as its first consumer, so it is testable without a UI. |
| Boundary | Qt widgets call controllers and utilities directly; global event bus and globals module | One pywebview bridge, JSON-like calls, events through `evaluate_js` | A typed contract crate and a platform adapter on the frontend replace both. |
| Integration code | About 18,000 lines across 52 integration modules (steam, git, github, updater) | Per-integration managers | Integrations are L1 adapters, each behind one crate and one trait, replaceable by small wrappers over crates. |
| Tests and CI | Present for sort and models; no layer checks | 67 Python test files, no CI workflow in the checkout | The reference projects show tests live beside the crate they test; CI gates boundaries. |

Note: both comparisons use RimSort and RimCrow as concept references only (R11); line counts and file names are facts, not copied content.

## Implications for RimStudio

1. The workspace has a Tauri-free core: no crate below the app shell may list `tauri` (or `gtk`, `webkit2gtk`) in its dependency tree. Test: `cargo tree -p <cli crate> -i tauri` returns no path, as a CI step (rust-analyzer's `salsa` guard pattern).
2. The headless CLI is its own binary crate that depends on L2 and L3 crates but never on the shell, and never relies on mutually exclusive features of the shell crate (Jan's cost). Test: CLI builds on a machine without GTK development packages.
3. Every workspace member carries a layer tag in its manifest metadata and an `xtask check-layers` (guppy-based, starting from the jq prototype) fails CI on any edge outside the matrix in section 6 and on any untagged member.
4. A cargo-deny `bans` policy with `wrappers`: the XML parser crates only through the single XML boundary crate (R10), `tauri` only through the shell and OS plugin crates, and one HTTP client crate wrapped by the fetch adapter. Test: adding a second direct XML parser dependency to a feature crate fails `cargo deny check bans`.
5. Features do not depend on other features; composition happens in the shell or in L2. Test: the matrix has no L3 to L3 edge and the check enforces it.
6. A single command registry: the command name, request and response types are declared once (Yaak's schema or GitButler's macro idea), handlers are written as plain functions in feature crates, and the shell adapter is generated or fails to compile when one is missing. The CLI uses the same functions. Test: removing a registry entry breaks the shell build, not a runtime call.
7. Add a CI test that every command in the registry has a matching capability entry or an explicit statement that it relies on the default app-command allowance (Jan's `every_registered_command_has_a_permission` lesson), if any plugin crates are used.
8. Typed bindings are generated and committed, and CI regenerates them and fails on a diff (GitButler, Yaak). The choice of generator (ts-rs, tauri-specta at `2.0.0-rc.25`, JSON Schema route) is left to the layout stage; avoid pinning an exact release-candidate version without a reason recorded (Cap pins one).
9. Jobs follow one model: caller-minted job id, a registry keyed by id, a progress channel, an explicit cancel call and cancellation on channel drop, with the same progress records usable by the CLI renderer (Modrinth's loading bars with a CLI mapping; Spacedrive's registry; cross-reference section 4 of `docs/research/webview-and-ipc-performance.md`).
10. Workspace lints are inherited by every member, verified by the layer check (fail, not report). Policy lints in `clippy.toml`: no raw process spawning outside one launcher crate, no direct filesystem writes outside the storage adapters, no second XML parser.
11. Keep the shell thin by measurable budget: for example, no file in the shell crate over a set line count and no business rule in it; Cap's 73,769-line shell and Yaak's 6,857-line shell bracket the range. The threshold itself is for the layout stage.
12. Do not use a plugin crate per feature for the 12 to 20 modules; reserve plugin crates for OS-specific capabilities with permissions (window effects, native dialogs). Test: count of plugin crates stays small and each has a platform reason in its manifest description.
13. A written architecture document with named invariants (rust-analyzer style) lives in the repo and a tidy-style check keeps it from rotting (module docs required, crate list matches the workspace).
14. Frontend: one app plus a small number of packages created on demand (UI kit, generated IPC client, platform adapter), feature folders with an enforced cross-feature import rule and a no-cycle rule, and a single module that owns all Tauri API imports (GitButler `IBackend`, Yaak `platform`). Test: a lint rule fails any `@tauri-apps` import outside the adapter.
15. Logic that decides warnings, errors, ordering and conflicts lives in Rust crates, never in views or stores (the RimSort `mods_panel.py` and RimCrow `issues.js` lessons). Test: the sorting and issue-detection suites run through the CLI without a webview.
16. Avoid global singletons for state in services (Modrinth, kftray); pass a context object explicitly (GitButler's `Context` with permission tokens) so tests can run in parallel without a global mutex.

## Open questions

1. Compile-time numbers for strategies A to E (section 7) are reasoned, not measured. Should a throwaway spike (synthetic workspace with 16 modules and 10 commands each, no Tauri build beyond a mock runtime) be run before the layout stage freezes? A generator for this exists only in the scratch folder.
2. Does cargo-deny `wrappers` behave as needed when the wrapper is a workspace member and the banned crate is also a transitive dependency of a third-party crate? The docs were read but the tool was not run here (it is not installed).
3. `cargo-modules` capabilities for failing CI on module-level orphans or cycles were not verified.
4. Which binding generator fits: tauri-specta and specta are still `2.0.0-rc.25`, GitButler built its own JSON Schema route, Yaak uses ts-rs with a schema crate. This overlaps `docs/research/rust-crate-research.md`.
5. How were GitButler's typical build times and incremental rebuild costs experienced by its maintainers? No maintainer statements about compile times were found in the repositories read; only Cap's link-time comment and Jan's CI notes exist.
6. Spacedrive's default branch has been quiet since July 2026 while side branches are active; whether its architecture was abandoned or is being rebuilt was not established.
7. Jan's licensing is inconsistent between the `LICENSE` file and `Cargo.toml`; irrelevant to RimStudio's use (structure only) but worth noting if the project is cited publicly.
8. Whether feature-sliced design as a named methodology brings value over feature folders with enforced imports was not evidenced by any studied project and is left to the frontend layout stage.
9. Packaging implications of a separate CLI binary (shipping it beside the app, name collision with the app binary, sidecar versus separate download) were not studied here; Jan's note that the Tauri CLI bundles every `[[bin]]` in the app crate suggests keeping the CLI in a separate package.
