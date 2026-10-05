# RimStudio workspace layout

This document fixes the project structure that all code follows: the complete repository tree after the restructure, naming and layer-tag conventions, the allowed-dependency matrix, the checklists for adding a tool module, a crate or a command, the platform code and feature-flag policies, the exact step-0 procedure that takes the repository from its cargo-init state to the target layout, and the enforcement toolchain that keeps the structure from eroding. Read it with the [architecture overview](overview.md) (invariants I-01 to I-19), the [crate catalog](crate-catalog.md) (what each crate contains) and the [decision register](decision-register.md).

Status: draft | Last updated: 2026-10-04

## 1. Repository tree after step 0

The product is RimStudio. The repository folder is currently named `rimforge-studio` and the root Cargo package is the cargo-init default; step 0 (section 9) replaces the root manifest with a virtual workspace (done); the folder rename of that step was dropped by the owner (D-099) and the folder keeps its name. Paths below are relative to the repository root.

```text
rimstudio/
  Cargo.toml                    virtual workspace: members, workspace.package, workspace.dependencies, workspace.lints, profiles
  Cargo.lock                    committed
  rust-toolchain.toml           channel pin (1.96) with rustfmt and clippy
  rustfmt.toml                  edition 2024 style
  clippy.toml                   disallowed methods and types (section 10)
  deny.toml                     cargo-deny: bans with wrappers, licences, advisories
  .cargo/config.toml            alias xtask = "run --package xtask --"
  package.json                  root pnpm scripts: check, lint, test, dev, build
  pnpm-workspace.yaml           packages: apps/*, packages/*
  pnpm-lock.yaml                committed
  .npmrc                        exact pins, minimum release age
  .oxlintrc.json                lint config with per-folder no-restricted-imports
  .dependency-cruiser.cjs       graph rules for the frontend
  .prettierrc.json              prettier with the tailwind class sorter
  .editorconfig  .gitattributes  .gitignore
  README.md  CONTRIBUTING.md   (no LICENSE until the owner supplies a custom licence, D-088)
  crates/                       all Rust library crates (one folder per crate, folder name equals package name)
    rimstudio-core/             L0: ids, versions, node tree, diagnostics, load plan, ports, settings types, job types, paths constants
      src/{ids,version,tree,diag,load_plan,ports,settings,jobs,paths,os,redact}.rs
    rimstudio-xml/              L1: the only XML boundary (quick-xml): parse, render, byte-span edit, RimWorld file codecs
      src/{reader,render,edit,about,load_folders,mods_config,save_meta,defs_scan,patches,modes}.rs
    rimstudio-io/               L1: JSONC and JSON documents, the in house JSON document store (`collection`), atomic writes, backups, migrations, schemas, data roots, walking, watching, write fence
      src/{roots,jsonc,store,atomic,backup,migrate,schema,walk,statkey,watch,guard,ignore,fence}.rs
    rimstudio-platform/         L1: every OS specific thing behind core ports (registry, links, processes, launcher, credentials, sandbox, install source)
      src/{lib.rs,windows/,macos/,linux/,unix/,generic/}
    rimstudio-xpath/            L2 engine: XPath 1.0 parser and evaluator over the node tree
      src/{lexer,parser,ast,eval,coerce,index_hint}.rs   tests/corpus.rs
    rimstudio-defs/             L2 engine: merge, patch, inherit, instantiate, provenance, type table, def databases
      src/{merge,patch_ops,apply,inherit,build,provenance,type_table,index,custom_ops}.rs
    rimstudio-rules/            L2 engine: lossless community and user rule formats, layered rule graph, explain, export patches
      src/{format,user,layers,graph,explain,suppress,export}.rs
    rimstudio-sort/             L2 engine: tiers, canonical and game style sort, cycle report, diff preview
      src/{tiers,canonical,game_style,cycles,diff}.rs
    rimstudio-validate/         L2 engine: diagnostic producers (list problems, author lints, publish preflight checks, log classification)
      src/{list,author,preflight,logs,codes}.rs
    rimstudio-design/           L2 engine: item math (stats, armor, DPS, price, fit), calibration, CE module (reader, formulas, patch generator, lint)
      src/{stats,armor,ranged,melee,apparel,price,fit,baseline,quiz,loo}.rs   src/ce/   templates/
    rimstudio-steam/            L2 service: own VDF reader, ACF views, game locator, workshop status; IO only through ports
      src/{vdf,acf,locator,probe,report,workshop,custom}.rs
    rimstudio-library/          L2 service: scanner, stat key cache, mod index, list codecs and history, link farm and ModsConfig deploy
      src/{scan,cache,index,sources,duplicates,lists,thumbs,deploy/}.rs
    rimstudio-workspace/        L2 service: reference sets, project store, def index build and cache, DefDatabases snapshots, scaffolding
      src/{refset,project,defindex,snapshot,scaffold}.rs
    rimstudio-datasets/         L2 service: RemoteDataset pipeline, HTTP fetch, codecs, slim indexes, RimSort import
      src/{descriptor,state,fetch,pipeline,codecs/,slim,rimsort/}.rs
    rimstudio-manager/          L3: manager use cases (library session, list ops with undo, profiles, history, sources, settings, datasets, launch, duplicates, bisect)
      src/{session,listops,sorting,profiles,history,sources,settings,datasets,launch,duplicates,meta,bisect,api}.rs
    rimstudio-toolkit/          L3: tool modules, one folder each, each behind a cargo feature
      src/{defs/,project/,designer/,registry.rs}
    rimstudio-publish/          L3: staging, plan, preflight wiring, publish state machine, history, helper transport
      src/{staging,plan,state,history,transport/,api}.rs
    rimstudio-ipc-types/        contract: DTOs, events, error envelope, helper protocol; specta and ts-rs derives behind features
      src/{lib.rs,mods.rs,library.rs,defs.rs,designer.rs,publish.rs,settings.rs,jobs.rs,error.rs,helper.rs}
    rimstudio-app/              L4 app: AppContext, command registry, tool table, job runner, logging init, dispatch
      src/{context,registry,tools,jobs,logging,dispatch,boot,images}.rs   benches/e2e_budgets.rs
    rimstudio-cli/              L4 CLI binary `rimstudio-cli`
      src/{main,cmds,render}.rs
    rimstudio-steam-helper/     L4 sidecar binary: protocol loop and SteamBackend trait
      src/{main,protocol,backend,lib_path}.rs   backend/{steamworks,gamelib}.rs
    rimstudio-testing/          support: fixture builders, fake ports, recording filesystem, golden helpers (dev-dependency only)
      src/{fixtures,fakes,golden,install_tree}.rs
  apps/
    desktop/                    the one frontend app (npm package rimstudio-desktop) and the Tauri shell
      package.json  index.html  vite.config.ts  tsconfig.json
      src/                      see section 3
      src-tauri/                the shell crate rimstudio-shell (L4)
        Cargo.toml  build.rs  tauri.conf.json
        capabilities/main.json
        icons/
        binaries/               git-ignored; sidecar copies named <name>-<target-triple>, filled by xtask
        src/{main.rs,lib.rs,commands.rs,state.rs,window/,events.rs,protocol_rsimg.rs,updater.rs,menu.rs}
        tests/bindings.rs       exports TypeScript bindings, compares in --check mode
  packages/
    ui/                         npm package rimstudio-ui: design system components, tokens, gallery stories
    ipc-types/                  npm package rimstudio-ipc-types: generated bindings.ts only (no hand written code)
    testkit/                    npm package rimstudio-testkit: mock IPC, fixtures, render helpers
  xtask/                        repository automation binary (support): layer check, bindings, schemas, fixtures, packaging helpers
    src/{main,layers,cfg,deps,licences,docs_tidy,tools,bindings,schemas,fixtures,package,budgets,scaffold}.rs
    layers.jsonc                the allowed-edge matrix and same-layer exceptions as data
  schemas/                      generated JSON Schemas (settings, workspace, project, rules, caches); committed
  tests/
    fixtures/                   small committed fixtures shared by two or more crates (synthetic only)
    vectors/                    language neutral test vectors: defs/ (38 vectors, 13 traces), xpath/, designer/ (fictional numbers)
    golden/                     expected JSON outputs for CLI and crate golden tests
    corpus/                     XPath shape file and golden hashes (never raw patches)
    e2e/                        Playwright (mock IPC) and WebdriverIO smoke suites
  scripts/                      few thin shell or node helpers called by CI only (no logic that belongs in xtask)
  docs/
    architecture/               this folder
    research/                   the evidence base, including reproducible data
    (later stages: design prompt, roadmap, glossary and index)
  .github/workflows/            ci.yml, nightly.yml, release.yml
  reference/                    optional, git-ignored: RimSort, RimCrow and Combat Extended checkouts (read only, concepts only, R11)
```

The three reference checkouts (`RimSort-main`, `RimCrow-main`, `CombatExtended-Development`) currently sit at the repository root and are git-ignored. Step 0 moves them into `reference/` (optional) so the root stays readable; no tool reads them and no build step depends on them.

### 1.1 What the tree optimises for

1. One folder per crate, folder name equal to package name, so `crates/*` as a workspace glob never needs editing.
2. The Tauri shell sits at `apps/desktop/src-tauri` because the Tauri CLI expects `src-tauri` beside `package.json`; it is still a workspace member and tagged `l4-shell`.
3. Generated files are few and named: `packages/ipc-types/src/bindings.ts`, everything in `schemas/`, and `apps/desktop/src-tauri/capabilities/main.json` is hand written but checked against the registry.
4. Test data that two crates share lives once under `tests/`; data used by one crate stays in that crate's `tests/data/`.

## 2. Naming and layer-tag conventions

| Subject | Convention | Example |
|---|---|---|
| Product name | RimStudio in prose, `rimstudio` in identifiers; never "rimforge" except when naming the old folder | `rimstudio-core` |
| Rust crate / package | `rimstudio-<noun>`; library crates are nouns (what it contains), bins are `rimstudio-cli` and `rimstudio-steam-helper`; the shell is `rimstudio-shell` with bin name `rimstudio` | `rimstudio-library` |
| Lib name | package name with underscores | `rimstudio_library` |
| npm package | same prefix: `rimstudio-desktop`, `rimstudio-ui`, `rimstudio-ipc-types`, `rimstudio-testkit` | |
| Layer tag | `[package.metadata.rimstudio] layer = "<tag>"` in every manifest; allowed values: `l0-domain`, `l1-infra`, `l2-engine`, `l2-service`, `l3-feature`, `l4-app`, `l4-shell`, `l4-cli`, `l4-sidecar`, `contract`, `support` | `layer = "l2-engine"` |
| Module files | `snake_case.rs`; a module with children is a folder with `mod.rs` only if it has more than one child | `deploy/` |
| Types | `PascalCase`; ids are newtypes (`PackageId`, `ModId`, `WorkshopId`, `ModIdx`); result containers end in `Report` or `Outcome` | `DetectionReport` |
| Traits | nouns for capabilities (`LinkBackend`), `-Probe` for read only OS queries, `-Sink` for outputs | `ProgressSink` |
| Errors | one `thiserror` enum per crate named `<Crate>Error` (for example `XmlError`), with a stable `code()` string; content problems are `Diagnostic`, never `Err` | |
| Diagnostic codes | `<area>.<kebab-name>`, lowercase; areas: `xml`, `defs`, `xpath`, `scan`, `rules`, `sort`, `list`, `author`, `deploy`, `steam`, `dataset`, `publish`, `ce`, `design`, `log`; defs internal golden codes (`patch_failed`) are mapped by rule to `defs.patch-failed` at the API edge. Error codes from crate `code()` methods use the same grammar with the extra areas `io`, `ipc`, `job`, `session`, `settings`, `game`, `app`, `applog` | `list.missing-dependency` |
| Commands | `<area>_<verb>` snake_case in Rust and the registry (`mods_toggle`, `library_scan`); TypeScript names are the camelCase form generated by the binding tool | `modsToggle` |
| DTO types | `<Command>Request`, `<Command>Response` for per command shapes, plain nouns with a `Dto` suffix when they would collide with a domain type; fields camelCase via `rename_all`; enum variants kebab-case strings; 64 bit ids as strings | `ModRowDto` |
| JSON files | `camelCase` keys, a top level `schemaVersion` (documents) or `v` (compact caches) integer | |
| Files on disk | lowercase kebab-case names with `.json` or `.jsonc`; JSONC only where a person edits | `settings.jsonc` |
| Fixtures and vectors | `snake_case` ids; each vector has a `doc` sentence that a failing test prints | |
| Benchmarks | `benches/<subject>.rs`; the budget table in `xtask/budgets.jsonc` names each bench and its limit | |
| Frontend folders | `kebab-case` for folders, `PascalCase.tsx` for components, `camelCase.ts` for modules, feature entry `index.ts`; feature folder names equal the tool id | `features/manager` |
| Milestones | As in the [roadmap](../roadmap.md): M0 skeleton, M1 core engine, M2 manager MVP, M3 manager v1, M4 toolkit foundation, M5 item designer, M6 Workshop publishing, M7 polish and release | |

## 3. Frontend structure

One Vite app with feature folders plus three small packages, created because each has a second consumer or a different test profile: `rimstudio-ui` (gallery and screenshot tests), `rimstudio-ipc-types` (generated), `rimstudio-testkit` (shared by app and package tests). Evidence: [frontend stack research](../research/frontend-stack-research.md) section 5.

```text
apps/desktop/src/
  main.tsx                      entry: theme bootstrap, webview feature probe, mount
  app/                          shell frame, providers, route signal, command palette, task centre, toasts, tools.ts (tool table), error boundary
  features/
    manager/                    mod lists, sorting preview, conflicts, profiles, history, duplicates
    settings/                   paths, custom mod folders, datasets panel, appearance, shortcuts, search
    logs/                       Player.log viewer and analysis
    workspace/                  Def Explorer, patch tester, conflict view, reference validator
    project/                    scaffolder, About editor, LoadFolders manager, dev launcher
    designer/                   item designer, fit meter, calibration quiz
    workshop/                   staging plan, preflight, publish and update
    onboarding/                 first run detection wizard
    (each feature: index.ts, <Name>Page.tsx, store.ts, api.ts, components/, optional model/, tests)
  shared/
    ipc/                        client, query, stream, snapshot plus delta, jobs, cancellation (no component calls invoke directly)
    platform/                   the only module importing @tauri-apps/*; previewUrl(id), dialogs, openers, window controls
    lists/                      windowing hook over @tanstack/virtual-core
    dnd/                        pointer based drag and drop facade
    charts/  graph/  editor/    wrappers for Chart.js, Cytoscape, CodeMirror (loaded lazily)
    rich-text/                  BBCode, Unity rich text and markdown to vnodes
    i18n/  keys/  forms/        ICU messages, shortcut registry, valibot helpers
  locales/en.json               flat semantic keys; other languages optional and lazy
  styles/{tokens.css,theme.css,layers.css}   CSS variables, @theme inline mapping, @layer user last
  gallery/                      dev only route rendering every rimstudio-ui component
```

Rules enforced by oxlint and dependency-cruiser (section 10): a feature imports only `shared`, `rimstudio-ui` and `rimstudio-ipc-types`, never another feature and never another feature's internals; `shared` imports no feature; `rimstudio-ui` imports nothing from the app; only `shared/platform` imports `@tauri-apps/*`; no XML library anywhere; every long list uses `shared/lists`.

## 4. Allowed-dependency matrix

Row depends on column. Y allowed, N forbidden, E allowed only for the edges listed in `xtask/layers.jsonc` (acyclic, reviewed). Dev-dependencies are checked separately: only `rimstudio-testing` may be a dev-dependency of any crate, plus the external test crates.

| Dependent \ Dependency | core | l1-infra | l2-engine | l2-service | l3-feature | app | shell | cli | sidecar | contract | testing |
|---|---|---|---|---|---|---|---|---|---|---|---|
| l0-domain (core) | n/a | N | N | N | N | N | N | N | N | N | N |
| l1-infra | Y | E (none today) | N | N | N | N | N | N | N | N | N |
| l2-engine | Y | N | E | N | N | N | N | N | N | N | N |
| l2-service | Y | Y | Y | E | N | N | N | N | N | N | N |
| l3-feature | Y | Y | Y | Y | N | N | N | N | N | Y | N |
| l4-app | Y | Y | Y | Y | Y | n/a | N | N | N | Y | N |
| l4-shell | Y | N | N | N | N | Y | n/a | N | N | Y | N |
| l4-cli | Y | N | N | N | N | Y | N | n/a | N | Y | N |
| l4-sidecar | Y | N | N | N | N | N | N | N | n/a | Y | N |
| contract | Y | N | N | N | N | N | N | N | N | E (none) | N |
| support (testing) | Y | Y | N | N | N | N | N | N | N | Y | E (none) |

Refinements over the research vocabulary ([reference architectures](../research/reference-architectures.md) section 6):

1. L2 is split into pure engines (no IO, no async, no platform) and services (IO through L1 crates and core ports). Engines cannot reach L1 at all, which keeps them benchmarkable on in-memory data.
2. The shell and the CLI depend on the composition root `rimstudio-app`, not on features. This makes "the shell has no business rules" a graph property (the shell cannot even name a feature crate) and gives the CLI exactly the same handlers as the UI (I-09).
3. The sidecar sees only core and the contract crate; it cannot reach the library, the XML crate or the filesystem layer.
4. `rimstudio-testing` is a dev-dependency only and never depends on engines, services or features, so test helpers cannot pull production logic in circles.

Same-layer exceptions (`allowedSameLayer` in `xtask/layers.jsonc`):

| From | To | Reason |
|---|---|---|
| rimstudio-defs | rimstudio-xpath | patch operations evaluate XPath |
| rimstudio-sort | rimstudio-rules | sorting consumes the merged rule graph |
| rimstudio-validate | rimstudio-rules, rimstudio-defs, rimstudio-xpath | list problems use rules; author lints use patch and def data |
| rimstudio-design | rimstudio-defs | reads resolved defs for reference items |
| rimstudio-workspace | rimstudio-library | reference sets are built from library mod indexes |

Every addition to this table needs a decision-register entry.

## 5. Third party confinement

`deny.toml` bans a crate everywhere except through listed wrappers. Final wrapper lists are completed from `cargo tree -i <crate> -e normal` once the Tauri tree exists ([rust crate research](../research/rust-crate-research.md) section 11.3).

| Crate (version) | Allowed direct dependents | Why |
|---|---|---|
| quick-xml (0.42.0) | `rimstudio-xml`, `plist` (via Tauri) | R10 and I-02 |
| roxmltree, xml-rs, xmltree, xmlparser, sxd-document, sxd-xpath, serde-xml-rs, minidom, libxml | none | second XML stacks; oracle data is precomputed JSON, so no oracle crate is linked |
| tauri (2.12.1) and tauri plugins | `rimstudio-shell` and the official plugin crates | I-01 |
| reqwest (0.13.5) | `rimstudio-datasets` (plus Tauri updater transitively) | one HTTP client behind one fetch adapter |
| rusqlite (0.40.2) | `rimstudio-datasets` with feature `aux-db` only | reading RimSort's foreign database; accepted read only (D-031, D-083) |
| rusqlite elsewhere, sqlx, any other SQL or embedded database engine, diesel, serde_yaml, serde_norway, bincode, rkyv, postcard | none | I-03 |
| notify, notify-debouncer-mini, atomic-write-file, jsonc-parser, ignore, same-file | `rimstudio-io` | storage and watching live in one crate |
| winreg (0.56.0), junction (2.1.0), sysinfo (0.39.6), keyring (4.2.0) | `rimstudio-platform` | I-11 |
| steamworks (0.13.1) | `rimstudio-steam-helper` | I-15 |
| petgraph (0.8.3) | `rimstudio-rules`, `rimstudio-sort` | graph algorithms |
| steamlocate (2.1.1), keyvalues-parser (0.2.4) | dev-dependencies of `rimstudio-steam` only | differential tests of the own VDF reader; `xtask check-deps` rejects them in normal dependencies |
| guppy (0.19.1) | `xtask` | layer check |
| tauri-plugin-fs, tauri-plugin-store, tauri-plugin-sql, tauri-plugin-log, tauri-plugin-shell, git2, self_update, sentry | none | security, R10 or duplication ([decision D-047](decision-register.md), D-061) |

## 6. Checklists

### 6.1 Add a tool module

1. Decide placement: a module under `crates/rimstudio-toolkit/src/<tool>/` behind Cargo feature `tool-<tool>` (default on). Make it its own `l3-feature` crate `rimstudio-tool-<tool>` only if it brings a heavy external dependency or a different test or release profile; record the split in the decision register.
2. Put reusable logic in an L2 crate (engine if pure, service if IO). The tool module holds orchestration only; it must not parse XML (I-02) and must read defs through `rimstudio-workspace` snapshots.
3. Add DTOs to `crates/rimstudio-ipc-types/src/<tool>.rs` and export them from `lib.rs`.
4. Append commands to `crates/rimstudio-app/src/registry.rs` with their kind (`query`, `action`, `stream`, `job`) and add the `ToolDescriptor` to `crates/rimstudio-app/src/tools.rs` (id, `titleKey`, icon name, required `Capability` list, phase).
5. Run `cargo xtask bindings` and `cargo xtask schemas`; commit the output.
6. Add the frontend feature folder `apps/desktop/src/features/<tool>/` and register its descriptor in `apps/desktop/src/app/tools.ts` with the same id; add locale keys.
7. Add a fixture under `tests/fixtures/` or the crate's `tests/data/`, unit tests for the logic crate, a test of the tool module with `rimstudio-testing`, and a benchmark plus a row in `xtask/budgets.jsonc` if the tool states a performance target.
8. Add a row to [crate-catalog.md](crate-catalog.md) (or update the toolkit module list) and run `cargo xtask check`; the check fails when the tool id sets of Rust and TypeScript differ or when the catalogue and workspace member lists differ.

### 6.2 Add a crate

1. `cargo xtask new-crate <name> --layer <tag> --description "<one line>"` (after step 0; before that use the template in section 9).
2. Choose the layer from the matrix; if the new crate needs an edge the matrix forbids, change the design, not the matrix.
3. Add `lints.workspace = true`, a `//!` crate doc, a `<Crate>Error` if it can fail, and a row in the crate catalog.
4. Declare every external dependency in the root `[workspace.dependencies]` and use `dep.workspace = true`; `xtask check-deps` rejects a member that declares its own version, and `cargo deny` `bans.workspace-dependencies` reports duplicate and unused workspace entries (it does not police members, checked against the cargo-deny configuration docs).

### 6.3 Add a command

1. Add the handler as a plain function in the owning feature crate (`pub fn name(ctx: &Ctx, req: Request) -> Result<Response, ApiError>`, async for jobs and streams).
2. Add one row to the registry table; the shell wrapper, CLI route, capability permission and TypeScript binding are generated or fail to compile if missing.
3. Add a registry test row: a CLI route or the marker `ui-only`.

## 7. Platform code policy

1. `cfg(target_os)`, `cfg(windows)`, `cfg(unix)` and OS specific dependencies are allowed only in: `crates/rimstudio-platform/src/{windows,macos,linux,unix}/`, `apps/desktop/src-tauri/src/window/` and its `build.rs` and `tauri.conf.json` platform sections, `crates/rimstudio-steam-helper/src/lib_path.rs`, and `xtask/src/package.rs`.
2. Everything else reaches the OS through the traits in `rimstudio-core::ports`: `Clock`, `EnvProbe`, `RegistryProbe` (returns `None` off Windows), `FsProbe`, `ProcessProbe`, `LinkBackend`, `Launcher`, `CredentialStore`, `SandboxProbe`, `InstallSourceProbe`. Each port has a default or fake implementation in `rimstudio-testing`.
3. Behavioural differences that are data, not code, use the runtime value `Os::current()` (from `std::env::consts::OS`, no cfg), for example case sensitivity of `About` resolution or the candidate list of Steam roots.
4. Path handling uses `camino::Utf8PathBuf` and `dunce` for Windows verbatim prefixes; non-UTF-8 paths become `scan.non-utf8-path` diagnostics. No hand written separators.
5. Tests for OS specific code run on all three operating systems in CI; tests of everything else run identically on all three and must not use `cfg` (use the fakes).
6. A new OS specific feature follows scenario S6: new file in `rimstudio-platform`, a port method with a default, and ordinary data flowing out.

## 8. Feature-flag policy

1. Cargo features are additive and never mutually exclusive. There is no `cli` versus `tauri-app` feature pair; the CLI and the shell are separate packages ([decision D-007](decision-register.md)).
2. Defined features: `tool-<name>` (toolkit modules, default on), `aux-db` (datasets, reads RimSort's foreign database, default off until the datasets crate exists; the read only use is accepted, D-083), `bindings` (ipc-types derives `specta::Type`, on in the shell and xtask only), `ts-fallback` (ts-rs derives, off), `schema` (schemars derives in core and ipc-types, on in xtask), `diagnostics` (shell: DevTools and the diagnostics page), `e2e` (shell: embedded WebDriver server), `backend-gamelib` (helper: second Steam backend), `test-support` (datasets and publish: the fake transports that replay `rimstudio-testing` scripts; off in release builds).
3. `e2e` and `diagnostics` must be absent from release builds; `cargo xtask check-release-features` inspects resolved features of `rimstudio-shell` and fails the release job otherwise.
4. Library crates are tested with default features and with `--no-default-features`; all-features runs exclude the shell. The shell is built with default features in CI and with `diagnostics` once.
5. No feature changes behaviour of the data formats on disk.

## 9. Step 0: from cargo-init to the target layout

Prerequisites: Rust 1.96, Node 26, pnpm 11. The machine's global `cargo tauri` reports 2.2.2, which is older than the pinned 2.12.1; do not use it. The project-local `@tauri-apps/cli` is used through `pnpm tauri`. Install tool binaries once: `cargo install --locked cargo-deny@0.20.2 cargo-nextest@0.9.146 cargo-machete@0.9.2` (versions from [rust crate research](../research/rust-crate-research.md) section 13).

1. Commit the current documentation so the move is a clean diff: `git add docs && git commit -m "docs: research and architecture"`.
2. Rename the folder and move the reference checkouts:
   ```bash
   cd /run/media/pawbeans/project_drive/pawbeans/Projects/Rust
   mv rimforge-studio rimstudio && cd rimstudio
   mkdir -p reference && mv RimSort-main RimCrow-main CombatExtended-Development reference/
   ```
   Update `.gitignore` to ignore `/reference/`, `node_modules/`, `/apps/desktop/dist/`, `/apps/desktop/src-tauri/binaries/` and `/apps/desktop/src-tauri/gen/`.
3. Replace the cargo-init package with the virtual workspace and remove the stub:
   ```bash
   git rm -r src
   cat > rust-toolchain.toml <<'EOF'
   [toolchain]
   channel = "1.96"
   components = ["rustfmt", "clippy"]
   EOF
   mkdir -p .cargo && printf '[alias]\nxtask = "run --package xtask --"\n' > .cargo/config.toml
   ```
   Write the root `Cargo.toml` from the skeleton in [rust crate research](../research/rust-crate-research.md) section 12.2 (resolver 3, edition 2024, `rust-version = "1.95"`), with `members = ["crates/*", "apps/desktop/src-tauri", "xtask"]`, the `[workspace.dependencies]` table limited to dependencies that crates use at M0, `[workspace.lints]` (section 10) and the dev and release profiles from the same note. There is no `license` field: the workspace package table has none and sets `publish = false`, and no `LICENSE` file or licence header is added until the owner supplies a custom licence (D-067, D-088).
4. Generate the library crate skeletons with one template (the layer tags are the contract for `xtask check-layers`):
   ```bash
   mk() { d=crates/$1; mkdir -p $d/src
     printf '[package]\nname = "%s"\ndescription = "%s"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\npublish = false\n\n[package.metadata.rimstudio]\nlayer = "%s"\n\n[lints]\nworkspace = true\n\n[dependencies]\n' "$1" "$3" "$2" > $d/Cargo.toml
     printf '//! %s\n' "$3" > $d/src/lib.rs; }
   mk rimstudio-core l0-domain "Ids, node tree, diagnostics, ports and settings types"
   mk rimstudio-xml l1-infra "The only XML boundary"
   mk rimstudio-io l1-infra "JSON and JSONC storage, document collections, atomic writes, walking and watching"
   mk rimstudio-platform l1-infra "Operating system adapters behind core ports"
   mk rimstudio-xpath l2-engine "XPath 1.0 over the node tree"
   mk rimstudio-defs l2-engine "Def merge, patch, inherit and provenance"
   mk rimstudio-rules l2-engine "Rule formats and layered rule graph"
   mk rimstudio-sort l2-engine "Tiers and sorting"
   mk rimstudio-validate l2-engine "Diagnostic producers"
   mk rimstudio-design l2-engine "Item math, calibration and CE module"
   mk rimstudio-steam l2-service "VDF reader and game detection"
   mk rimstudio-library l2-service "Scanner, cache, lists and deploy"
   mk rimstudio-workspace l2-service "Reference sets, projects and def databases"
   mk rimstudio-datasets l2-service "Remote datasets and RimSort import"
   mk rimstudio-manager l3-feature "Mod manager use cases"
   mk rimstudio-toolkit l3-feature "Toolkit tool modules"
   mk rimstudio-publish l3-feature "Staging and publishing"
   mk rimstudio-ipc-types contract "Process boundary DTOs"
   mk rimstudio-app l4-app "Composition root and command registry"
   mk rimstudio-testing support "Fixtures and fakes"
   ```
   Create the two binaries by adding `src/main.rs` (`fn main() {}`) and the tag `l4-cli` for `rimstudio-cli` and `l4-sidecar` for `rimstudio-steam-helper`, using the same manifest template with `[[bin]]` entries.
5. Create `xtask`: `cargo new xtask --bin --vcs none`, add the `support` tag, and implement `check-layers` first (section 10). Until it is complete, the jq filter from [reference architectures](../research/reference-architectures.md) section 8 is an acceptable stopgap.
6. Scaffold the frontend with the pinned versions (template names and flags from the tools' help, to be re-checked at run time):
   ```bash
   printf 'packages:\n  - "apps/*"\n  - "packages/*"\n' > pnpm-workspace.yaml
   pnpm init
   pnpm create vite apps/desktop --template preact-ts
   pnpm --filter ./apps/desktop pkg set name=rimstudio-desktop
   pnpm --filter rimstudio-desktop add -E preact@10.29.8 @preact/signals@2.11.3 @tauri-apps/api@2.12.1
   pnpm --filter rimstudio-desktop add -DE vite@8.3.2 @preact/preset-vite@2.10.6 typescript@6.0.3 \
     tailwindcss@4.3.3 @tailwindcss/vite@4.3.3 @tauri-apps/cli@2.12.1
   ```
7. Scaffold the shell with the local CLI (the `init` flags exist in the installed 2.2.2 CLI; confirm with `pnpm --filter rimstudio-desktop tauri init --help`):
   ```bash
   cd apps/desktop
   pnpm tauri init --ci --app-name RimStudio --window-title RimStudio \
     --frontend-dist ../dist --dev-url http://localhost:1420 \
     --before-dev-command "pnpm dev" --before-build-command "pnpm build"
   cd ../..
   ```
   Then edit `apps/desktop/src-tauri/Cargo.toml`: package `rimstudio-shell`, workspace inheritance, `[package.metadata.rimstudio] layer = "l4-shell"`, `lints.workspace = true`, dependencies `tauri 2.12.1`, `tauri-build 2.7.1` and the plugins of D-047. In `tauri.conf.json` set `identifier` to the placeholder `app.rimstudio.desktop` (owner decision D-049), `bundle.macOS.minimumSystemVersion` to `13.3`, a restrictive CSP and `dragDropEnabled` true.
8. Add `packages/ui`, `packages/ipc-types` and `packages/testkit` as empty packages with the prefixed names; `ipc-types` contains only a placeholder `bindings.ts` until the first `cargo xtask bindings` run.
9. Add `deny.toml`, `clippy.toml`, `.oxlintrc.json`, `.dependency-cruiser.cjs`, and the CI workflow from section 10 with every job allowed to fail except `check-layers`.
10. Verify and commit:
    ```bash
    cargo check --workspace && cargo xtask check-layers
    pnpm install && pnpm --filter rimstudio-desktop build
    pnpm --filter rimstudio-desktop tauri dev      # window opens, no data
    git add -A && git commit -m "chore: restructure into a virtual workspace"
    ```
    Exit criteria for M0: the workspace builds on the three operating systems in CI, the layer check passes with 24 tagged members, `cargo tree -p rimstudio-cli -i tauri` prints nothing, and the empty app window opens.

## 10. Enforcement toolchain

Everything runs locally as `cargo xtask check` plus `pnpm check`, and in CI on every pull request.

| Check | Mechanism | Fails when | Source |
|---|---|---|---|
| Layer matrix | `xtask check-layers` using guppy over `cargo metadata`, data in `xtask/layers.jsonc` | an edge outside the matrix and the same-layer list, an untagged member, a feature to feature edge, or a path from `rimstudio-cli` to `tauri` | [reference architectures](../research/reference-architectures.md) section 8 |
| Third party bans | `cargo deny check bans` with wrappers (section 5); `bans.workspace-dependencies` | a second XML crate, SQLite, YAML, a duplicate or unused workspace dependency entry | [rust crate research](../research/rust-crate-research.md) section 11.3 |
| Dev-only crates | `xtask check-deps` | a member that declares its own dependency version instead of `workspace = true`; `steamlocate` or `keyvalues-parser` outside `[dev-dependencies]` of `rimstudio-steam`; an XML crate in any manifest but `rimstudio-xml`; a normal dependency on `rimstudio-testing` | D-037 |
| Platform quarantine | `xtask check-cfg` greps `target_os`, `cfg(windows)`, `cfg(unix)` outside the allow list | any hit | I-11 |
| Licences and R11 | `cargo deny check licenses` (dependency licences only); `xtask check-licences` scans for dataset extensions and value tables | a dataset or CE/vanilla table in the tree, an incompatible licence | I-06, I-07 |
| Advisories | `cargo deny check advisories` (and `cargo audit`) | open advisory | [packaging research](../research/cross-platform-packaging-research.md) implication 12 |
| Unused dependencies | `cargo machete` (or `cargo shear`) | unused dependency | |
| Workspace lints | `[workspace.lints]` inherited by every member; `xtask check-layers` also fails a member without `lints.workspace = true` | missing opt in | |
| Clippy policy | `clippy.toml` `disallowed-methods`: `std::process::Command::new` (use `Launcher`), `std::fs::write`, `std::fs::File::create`, `std::fs::rename`, `std::fs::remove_file`, `std::fs::remove_dir_all` (allowed only in `rimstudio-io` through a crate level `allow` with a rationale; the CLI writes its `--out` files through `rimstudio-app::dispatch_to_file`, which calls `rimstudio-io`, so it needs no exception (D-075); `rimstudio-testing` and `xtask` carry their own crate level `allow`, and `std::process::Command::new` is also allowed in `xtask`), `std::time::SystemTime::now` and `std::env::var` (use `Clock` and `EnvProbe`) | a call outside the owning crate | reference architectures implication 10 |
| Warnings | `RUSTFLAGS=-D warnings`, `unreachable_pub` as error in CI only | any warning | rust-analyzer policy |
| Generated files | `cargo xtask bindings --check`, `schemas --check` | a diff after regeneration | I-19 |
| Registry parity | test in `rimstudio-app`: every registry name has a CLI route or `ui-only`; permission file equals registry; `xtask check-tools` compares Rust tool ids with `app/tools.ts` | drift | I-09 |
| Shell thinness | `xtask check-size`: shell crate under 3,000 lines excluding generated code, no file over 400 lines; any crate file over 1,500 lines warns | breach | reference architectures implication 11 |
| Architecture docs | `xtask check-docs`: every crate has a `//!` doc, the first column of the crate catalog equals the workspace members, invariants I-xx in this folder are all referenced by an enforcement entry | mismatch | rust-analyzer tidy |
| Tests | `cargo nextest run --workspace` on three operating systems; golden tests at 1 and 8 threads | failure | |
| Benchmarks | `xtask bench-budgets` runs `rimstudio-app/benches/e2e_budgets.rs` and crate benches against `xtask/budgets.jsonc` | more than 2x regression (warning first, failure once calibrated) | [scan performance spike](../research/scan-performance-spike.md) implication 12 |
| Frontend lint | oxlint `no-restricted-imports` per folder, `import/no-cycle`, type check (tsc), Prettier | banned import, cycle | [frontend research](../research/frontend-stack-research.md) section 5.2 |
| Frontend graph | dependency-cruiser `forbidden` rules mirroring section 3, `no-circular`, `rimstudio-ui` must not import the app; knip as warning | violation | |
| Frontend tests | Vitest 5.0.3, Playwright 1.63.0 screenshot tests of the gallery in both themes, locale completeness script, Preact 11 matrix leg | failure | D-050 |
| Release features | `xtask check-release-features` | `e2e` or `diagnostics` in a release build | section 8 |

Workspace lints (root `Cargo.toml`): `unsafe_code = "forbid"` (a documented override only in `rimstudio-steam-helper` if the FFI route needs it), `unreachable_pub` warn, clippy `dbg_macro`, `todo`, `unwrap_used` and `expect_used` warn in library crates, `print_stdout` and `print_stderr` warn outside the CLI.

oxlint sketch (`.oxlintrc.json`, per-folder overrides; exact rule options to be verified against oxlint 1.86.0 at M0, open question 3 of the frontend note): in `apps/desktop/src/features/**` forbid patterns `**/features/*/**` (cross feature and deep imports), `@tauri-apps/*`, any XML parser package; in `apps/desktop/src/shared/**` forbid `**/features/**` and `@tauri-apps/*` except in `shared/platform/**`; in `packages/ui/**` forbid imports of the app.

CI jobs (`.github/workflows/ci.yml`, matrix windows-latest, macos-latest, ubuntu-22.04 with WebKitGTK 2.50 or newer for the shell): `rust` (fmt, clippy, nextest, deny, machete, xtask check), `frontend` (pnpm check on Preact 10 and a second leg on 11), `shell-build` (tauri build without bundling), `e2e` (Playwright with `mockIPC`). `nightly.yml` runs the WebdriverIO smoke suite on all three operating systems and the benchmark budgets. `release.yml` runs `tauri-action`, writes `SHA256SUMS` and attestations; action versions are pinned by hash ([packaging research](../research/cross-platform-packaging-research.md) implications 12 and 14).
