# RimStudio testing strategy

This document fixes how RimStudio is tested: the pyramid per layer, the fixture and vector inventory, the golden JSON policy, property tests, corpus tests on a real library, benchmarks and their CI gates, frontend and end-to-end suites, the CI jobs, the test-data and licence policy, and a table mapping every named invariant to the test that guards it. It implements decision D-058 of the [decision register](decision-register.md) and relies on the layering in the [architecture overview](overview.md) and the [workspace layout](workspace-layout.md). Evidence for tool choices is in the [Rust crate research](../research/rust-crate-research.md) (section 11.2), the [frontend stack research](../research/frontend-stack-research.md) (section 2.6) and the [cross-platform packaging research](../research/cross-platform-packaging-research.md) (section 5).

Status: draft, section 8 updated to the environment variables in use | Last updated: 2026-10-05

## 1. Principles

1. **Test where the logic lives.** Business rules sit in pure crates (L0 and L2 engines), so most tests run without a filesystem, a webview or the network. A test that needs Tauri is a bug in the layering (I-01).
2. **Ports make tests parallel.** No global mutable state in library crates (I-16) and every OS effect behind a port in `rimstudio-core::ports` (D-009) means every test builds its own `AppContext` from fakes and runs under `cargo nextest` in parallel with no shared lock.
3. **Fixtures are generated, never copied.** Trees are built from code at test time (builders in `rimstudio-testing`) so that no RimWorld, Combat Extended (CE) or community dataset content enters the repository (R11, I-06, I-07). The only committed data is small, synthetic and language neutral.
4. **Golden outputs are JSON.** R10 lists golden outputs among the things that are never XML, and I-12 says output is byte identical across thread counts, so a golden test compares JSON bytes.
5. **Real data is opt-in.** Tests that read a real install, a real workshop folder or a real CE are `#[ignore]` and are enabled by environment variables. CI never needs them, the maintainer runs them locally, and the nightly job can run them on a prepared self-hosted machine if one exists (owner decision, section 12).
6. **A failing test explains itself.** Vectors carry a `doc` sentence that the failure message prints; golden diffs print through `similar`; diagnostics are compared by stable code, not by message text.
7. **Performance is a test.** Budgets from the [scan performance spike](../research/scan-performance-spike.md) live in `xtask/budgets.jsonc` and a 2x regression fails the gate.

## 2. Tool set

| Concern | Tool (pinned in the workspace manifest) | Use |
|---|---|---|
| Runner | cargo-nextest 0.9.146 | Every Rust test on every OS leg; per-test timeouts; JUnit output for CI |
| Table tests | rstest 0.27.0 | One test body over a list of fixtures, vectors or layouts |
| Properties | proptest 1.11.0 | Sorting, round trips, cache, XPath equivalences |
| Temp trees | assert_fs 1.1.4 (tempfile underneath) | Mod trees, install trees, link farms |
| Golden files | goldenfile 1.11.0 with similar 3.2.0 for diffs | JSON goldens, regenerated with `UPDATE_GOLDENFILES=1` |
| Benchmarks | criterion 0.8.2 | Crate benches and the end-to-end budget bench |
| Coverage | cargo-llvm-cov 0.9.1 | Optional, reported not gated |
| Frontend unit | Vitest 5.0.3, jsdom 30.1.2, @testing-library/preact 3.2.4 | Stores, formatters, components |
| Frontend integration | Playwright 1.63.0 with `mockIPC` | Whole web layer with a scripted backend, gallery screenshots |
| End to end | WebdriverIO with `@wdio/tauri-service` | Real shell smoke suite, nightly, behind the `e2e` feature |

Rejected: insta file snapshots (a `.snap` file is not JSON, and R10 names golden outputs; inline `assert_json_snapshot!` is allowed for tiny expectations), divan (last release is 17 months old at the time of the research), and tests in Python (the prototypes under `docs/research/data/` stay as research evidence, the Rust suites replace them).

## 3. The pyramid per layer

| Layer | Crates | What is tested | How | Needs |
|---|---|---|---|---|
| L0 domain | `rimstudio-core` | Id newtypes, `GameVersion` parsing and ordering, node tree and arena, load plan, port trait contracts, settings types | Unit tests and proptest; serde round trip of every public type | Nothing |
| L1 infra | `rimstudio-xml`, `rimstudio-io`, `rimstudio-platform` | XML codecs and byte-span edits; atomic writes, backups, migrations, CST edits, `GameWriteFence`, `RootGuard`; OS ports | Fixture files, `RecordingFs`, temp dirs; platform tests per OS leg | Temp dirs; OS legs for platform |
| L2 engines | `rimstudio-xpath`, `-defs`, `-rules`, `-sort`, `-validate`, `-design` | Pure logic: evaluator, merge, patches, inheritance, rule graph, sorting, validators, item math | Vectors, goldens, proptest, corpus gates | Nothing from L1 (an engine cannot link it) |
| L2 services | `rimstudio-steam`, `-library`, `-workspace`, `-datasets` | Detection, scanning, caches, deploy, def index, dataset pipeline | Synthetic trees through `DetectEnv`, `FakeTransport`, `RecordingFs` | Temp dirs |
| L3 features | `rimstudio-manager`, `-toolkit`, `-publish` | Use cases: undo stack, profiles, preview, tool orchestration, staging plans | Scenario tests over `AppContext` built from fakes | `rimstudio-testing` |
| Contract | `rimstudio-ipc-types` | Serialisation shape, camelCase and kebab-case, string ids, error envelope | Golden JSON per DTO, schema drift check | Nothing |
| L4 app | `rimstudio-app` | Registry parity, dispatch, `JobRunner`, boot and logging init | Registry tests; job progress, cancellation and rate-limit tests | Fakes |
| L4 shell | `rimstudio-shell` | Generated wrappers compile; capabilities equal registry; bindings export | `tests/bindings.rs`, capability test; no logic to test by budget | Tauri build |
| L4 cli | `rimstudio-cli` | Exit codes, JSON output, progress rendering, the generic `call` route | Golden CLI output with `assert_cmd` style process tests (unverified crate choice, a plain `std::process` harness is acceptable) | Built binary |
| L4 sidecar | `rimstudio-steam-helper` | Protocol v1: one terminal event, line limit, watchdog; `SteamBackend` fake | Spawn the helper with a fake backend; transport tests in `rimstudio-publish` use `FakeTransport` | Built binary |

Rules that follow from the table:

1. Every public function in an L2 engine has at least one test that does not touch the filesystem.
2. Use cases in L3 are tested through the same `handler(ctx, req)` functions the shell and CLI call, so a use case test is also a CLI test minus the process boundary (I-09).
3. A bug found in production gets a regression test at the lowest layer that can reproduce it, then a vector if the behaviour is a game quirk.
4. The integration tests of a crate live in its `tests/` folder and use only its public API; unit tests stay next to the code.

## 4. Fixtures and vectors

### 4.1 `rimstudio-testing`

A dev-dependency only crate (layer `support`, a normal dependency on it fails `xtask check-deps`). It provides:

| Component | Purpose |
|---|---|
| Install tree builder | Builds a RimWorld install (Data with a one-line Core `About.xml`, `Version.txt` with `1.6.4871 rev598`, platform executable stub, `Mods`) for an OS profile under a temp root |
| Steam tree builder | Builds `libraryfolders.vdf`, `appmanifest_294100.acf`, `workshop/content/294100/<id>` and `appworkshop_294100.acf` from templates |
| Mod folder builder | Fluent builder: package id, name, versions, `LoadFolders`, defs, patches, preview, deliberate damage (BOM, stray `&`, wrong case folder) |
| Library builder | N mods across several roots, with duplicates and dependencies, for sorting and scan tests |
| Fake ports | `FakeClock`, `FakeEnv`, `FakeRegistry`, `FakeFs` probe, `FakeProcesses`, `FakeLinkBackend`, `FakeLauncher`, `FakeCredentialStore` |
| `RecordingFs` | Wraps `RealFs` and records every write, rename and delete, so tests assert the write set (I-05) |
| `ScriptedHttp`, `ScriptedHelper` | Plain-data scripts (ETag, 304, truncated body, wrong encoding, oversize; helper event lines). The `FakeTransport` types that replay them implement the transport traits inside `rimstudio-datasets` and `rimstudio-publish` behind a `test-support` feature, because `support` may not depend on services or features |
| Golden helpers | `golden_json(name, &value)` over goldenfile, canonical key order, trailing newline |
| Corpus path helper | Reads `RIMSTUDIO_GAME_DIR`, `RIMSTUDIO_WORKSHOP_DIR`, `RIMSTUDIO_CE_DIR`, `RIMSTUDIO_CUSTOM_DIR` (as built the tests read them directly with `std::env::var`); returns `None` so the test prints a skip reason |

### 4.2 Synthetic Steam and install trees per OS

Detection is tested with the builders and `DetectEnv`, so every OS layout runs on every CI leg, and the real Windows and macOS legs additionally run the same assertions against a temp dir. The layouts are those of the [Steam and game detection note](../research/steam-and-game-detection.md) section 7.4 and the packaging note section 5.2.

| Id | Layout | Key assertion |
|---|---|---|
| L1 | Linux native Steam with `.steam/steam` symlinks | One root after symlink dedup; user dir native |
| L2 | Two libraries, game in the first | Install found through the VDF path; workshop dir in the right library |
| L3 | Flatpak Steam, with and without the dot variant of the user dir | Both user dir candidates probed |
| L4 | Snap Steam | Snap candidate found |
| L5 | Proton prefix with `CompatToolMapping` | Both user dirs listed, newer mtime selected |
| L6 | Empty `compatdata/294100` | `proton` false, no warning |
| W1 to W3 | Injected registry values, HKLM only, no registry with default path | Registry order, escaped `D:\\SteamLibrary` decoded, case-insensitive dedup, fallback confidence medium |
| M1 | macOS app bundle with `Mods` inside | Bundle detected |
| N1 to N3 | GOG on Windows and Linux, manual app | Kind gog or manual, no workshop |
| P1 | Pathological: zero-byte temp manifest, BOM VDF, manifest without folder, dangling symlink, offline library, Steam plus GOG | Documented warning each, never a panic, other candidates still returned |

Edge cases added for the library scanner: lowercase `about/About.xml`, BOM, self-referencing symlink, path over 240 characters, Windows reserved names (creatable only on Linux), NFD and NFC names, duplicate package id across roots, an offline custom folder. Each is one `rstest` case.

### 4.3 Mod folder fixtures from corpus signatures

The [mod format and corpus note](../research/rimworld-mod-format-and-corpus.md) records the signatures of real mods (damage classes, field shapes, folder conventions). The builder generates one fixture per signature class with invented names (for example `fixture.alpha`), never a copy of a real mod. The parser checklist in that note becomes a table test: each row is a fixture and an expected set of diagnostic codes (`xml.*`, `scan.*`). The vanilla load fixture is the only place that needs real data and is `#[ignore]` (section 8).

### 4.4 RimWorld file samples

Small hand-written samples of `About.xml`, `LoadFolders.xml`, `ModsConfig.xml`, save meta and `Player.log` excerpts live in `tests/fixtures/rimworld/`. They exercise the codecs in `rimstudio-xml` (round trip with unknown elements preserved, byte-span edits that leave every other byte untouched, BOM and CRLF preserved, I-18). They are written by hand with fictional values. Real player logs are never committed (they contain paths and user names); the `log.*` classifier is tested with hand-written lines and a redaction test (see the [error handling document](error-handling-and-logging.md)).

### 4.5 Def engine vectors and traces

The research prototype holds 38 language neutral vectors (merge and databases 5, inheritance 15, patches 16, MayRequire 2) and 13 hand traces, with a resolved-output golden of 64 KB ([def engine semantics](../research/def-engine-semantics.md) sections 9.5 and Implications 7). They move to `tests/vectors/defs/` and are run unchanged by an `rstest` over the directory in `rimstudio-defs/tests/vectors.rs`:

1. Each vector file is `{name, doc, files, patches, active_set, expect}` with XML only inside strings (the game's boundary format). The test uses `rimstudio-xml` to parse those strings into node trees, then calls `rimstudio-defs`. The engine crate itself never sees XML (D-016).
2. A failing vector prints its `doc` sentence and a JSON diff.
3. The expected resolved output is compared structurally with `golden/vector_outputs.json`; the diagnostics are compared by code after the documented mapping from the prototype's snake_case codes to `defs.*` codes.
4. The behaviours that look like bugs but must be copied (Insert reverses multiple values, Sequence has no rollback, `success=Never` keeps the change, and the rest of the list in Implications 8) each have a vector whose name is the test name. Removing one is a reviewed decision, not a cleanup.
5. The vectors are reviewed once at import for any real game values; they are synthetic by construction, but the import step records that review in the pull request.

The real-data acceptance test (vanilla Core plus five DLC: 13,808 nodes, 596 abstract, 13,212 defs, 29 operations all true, 1,811 ThingDefs, 0 diagnostics; with CE: 18,177 nodes, 17,080 defs, 2,820 CE operations, 12 ThingDef overrides) is `#[ignore]` and reads `RIMSTUDIO_GAME_DIR` and `RIMSTUDIO_CE_DIR`. The expected numbers are counts only, not content, so the file `tests/golden/real_data_summary.json` may be committed.

### 4.6 XPath fixtures

Following the [XPath patch coverage note](../research/xpath-patch-coverage.md) section 8:

| Fixture | Content | Gate |
|---|---|---|
| `tests/corpus/xpath/shapes.json` | About 2,000 shapes with literals collapsed, one representative expression, count and tier each; no raw patches | Parse every shape; share handled at least 97 percent (tier B) then 99.8 percent (tier C) of occurrences; unsupported shapes in a checked-in allowlist |
| Synthetic document | 30 to 50 hand-written defs described as JSON, with duplicates, missing children, mixed `Class` | Known node counts authored by hand |
| Golden answers | Node count and node signature hash per shape from Mono and libxml2, regenerated only by an explicit maintainer script | CI compares the Rust evaluator to the stored hashes |
| Truthiness suite | Every row of the coercion table plus the string, number and boolean comparison matrix of XPath 1.0 | One fixture per row |
| Property tests | Random predicates: tier B expressions equal themselves with redundant parentheses and whitespace; results in document order | proptest |
| Differential job | Full sample against the user's real unified document, comparing with `sxd-xpath` as a dev-dependency | Local and optional nightly; lists new disagreements |

Whitespace and newlines inside expressions (6,420 real ones contain newlines) are covered by property tests that insert them at random token boundaries.

### 4.7 CE operation vectors and designer vectors

`tests/vectors/designer/` holds vectors with fictional numbers only (I-07): a made-up gun class with invented medians, an invented armor curve, an invented price function. They cover the item math (stats, DPS, price, fit score), the baseline and quiz calibration, and the leave-one-out harness on synthetic pairs. CE operation vectors cover the reader and generator in `rimstudio-design::ce`: a synthetic `MakeGunCECompatible` operation with fictional parameters is read into a typed struct and written back; the generated patch tree for a synthetic weapon equals a hand-authored golden JSON node tree; update mode switches when a conversion exists; CEP001 to CEP022 lint ids each have one positive and one negative case. Rendering to XML for the golden is done in `rimstudio-xml::render` inside the test and compared as a parsed tree, never as text, so the golden stays JSON.

With CE absent the designer must degrade with an explanation (D-064). A test runs the designer scenarios with an empty CE source and asserts the `design.ce-missing` diagnostic and that no table is invented.

### 4.8 Rules, datasets and deploy fixtures

| Area | Fixtures |
|---|---|
| Rules | Hand-written rules files in each of the five dataset shapes with invented package ids; unknown keys, comments and key order for the lossless round trip (I-18); a layered graph with a conflicting suppression; cycle cases |
| Datasets | `FakeTransport` scripts for first fetch, conditional 304, ETag change, body over cap, wrong content type, partial download, quarantine on parse failure with last good kept, offline start |
| RimSort import | Hand-written user rules and list exports in the formats described in the [RimSort core domain note](../research/rimsort-core-domain.md); the aux database read is tested behind feature `aux-db` with a database created in the test (decision D-031 accepted, D-083) |
| Deploy | Link farm planner with `FakeLinkBackend`; ownership manifest, unlink-only cleanup, stale link repair, pre-launch check blocking when the game would deactivate an id; real link tests per OS leg using symlinks (Linux, macOS) and junctions (Windows) |

## 5. Golden JSON policy

1. **Location.** Crate-specific goldens live in the crate's `tests/data/`; goldens used by two crates or by the CLI live in `tests/golden/`. Names are lowercase kebab-case `.json`.
2. **Format.** Pretty printed with two spaces, keys in canonical order (struct field order or sorted maps), a trailing newline, LF line endings on every OS (`.gitattributes` marks `tests/**` as `eol=lf`).
3. **Regeneration.** Only with `UPDATE_GOLDENFILES=1 cargo nextest run -p <crate>`, then the diff is reviewed in the pull request (as built the DTO goldens of `rimstudio-ipc-types` use `RIMSTUDIO_UPDATE_GOLDEN=1` instead). A CI job fails if a golden changes during a normal run.
4. **Determinism.** Golden tests for sorting, merging, patching, cache writing and export run at 1 and 8 threads and require identical bytes (I-12). `HashMap` iteration never reaches an output; the lint that forbids it is part of review, and the threads test catches regressions.
5. **Size.** A golden above 200 KB is split or summarised; a file above 1.5 MB under `docs/` or any golden over 1 MB fails `xtask check-size`.
6. **Content.** Goldens contain synthetic data only. The licence hygiene scan (section 10) checks goldens for dataset-shaped content.
7. **Schemas.** Every DTO, settings document and cache has a generated JSON Schema in `schemas/`; a test validates each committed golden that claims a schema against it.

## 6. Property tests

| Subject | Property |
|---|---|
| Sorting (`rimstudio-sort`) | Output is a permutation of the input; every satisfied `loadAfter` or `loadBefore` constraint holds when no cycle exists; sorting is idempotent; shuffling the input does not change a canonical sort; cycle reports list a real cycle |
| Rule merge (`rimstudio-rules`) | Merging layers is associative where documented; user layer wins; unknown keys survive; export then import is the identity on the foreign fields |
| Round trips | `About`, `LoadFolders` and `ModsConfig` codecs: parse then render then parse is stable; a byte-span edit changes only the edited span; JSONC CST edits keep comments and order |
| Node tree | JSON serde round trip; ordered attributes keep order |
| Cache (`rimstudio-library`) | Scan, persist manifest, mutate random files, rescan: the incremental result equals a cold scan; the cache key changes on size, mtime or file id change; FAT mtime rounding falls back to hashing; a corrupt manifest is discarded, never fatal |
| Versions | `GameVersion` ordering is a total order and parse of display is identity |
| XPath | Section 4.6 properties |
| Atomic write | A simulated crash at any step leaves either the old or the new file, never a mix |
| Diagnostics | Counting and capping per code never loses the total count |

Proptest regressions are committed (`proptest-regressions/` directories) so a once-found failure stays a test. The default case count is 256 locally and 1,024 in the nightly job.

## 7. Platform tests

`rimstudio-platform` is the only crate with `cfg(target_os)` (I-11), so it is also the only place with OS-gated tests. Each port has a contract test written once against the trait and instantiated by each implementation: registry probe (Windows only), link backend (symlink or junction, copy fallback, removal does not follow links), process probe, launcher, credential store (with the restricted-file fallback), sandbox probe, install source probe, locale. The three-OS CI matrix runs them; Steam Deck and sandbox cases (Flatpak grants) belong to the manual matrix in section 5.5 of the packaging note and gate the link farm through spike S-03.

## 8. Corpus tests on a real library

Real-data tests are `#[ignore]` with `reason` strings and read these variables:

| Variable | Meaning |
|---|---|
| `RIMSTUDIO_GAME_DIR` | The RimWorld install folder (the one that holds `Data` and `Version.txt`), used read-only |
| `RIMSTUDIO_WORKSHOP_DIR` | The Steam Workshop content folder of app 294100, used read-only |
| `RIMSTUDIO_CE_DIR` | A Combat Extended mod folder, used read-only |
| `RIMSTUDIO_CUSTOM_DIR` | The maintainer's custom mod folder (a folder of mod folders), used read-only |

The names above replace the earlier `RIMSTUDIO_TEST_LIBRARY`, `RIMSTUDIO_TEST_INSTALL` and `RIMSTUDIO_TEST_CE`. A test that needs a variable that is not set returns early and says so; no test writes into any of these folders or anywhere outside a temporary directory and the cargo target directory, and the output of a real data run is never committed. Example: `RIMSTUDIO_GAME_DIR=... RIMSTUDIO_WORKSHOP_DIR=... cargo test -p rimstudio-workspace --release -- --ignored --nocapture`. Fixtures and tests use fictional `RS_` names and numbers; the repository holds no RimWorld or Combat Extended value table.

As built, `cargo xtask corpus` does not exist yet (the `xtask` commands are `check-layers`, `check-source`, `check-docs` and `check-all`); the ignored tests are run per crate as in the example.

Planned: `cargo xtask corpus` (a wrapper over `cargo nextest run --run-ignored only` that checks the variables and prints what is missing). The corpus suite asserts: the scan completes with zero panics and records diagnostics counts per code; the same scan at 1 and 8 threads gives identical JSON; the manifest round trips; the vanilla load yields zero diagnostics; the CE load matches the real data summary counts; every About file the scanner reads re-renders through the byte-span editor unchanged when no edit is requested; XPath differential over the real unified document. Output of corpus runs goes to the scratch directory, never into the repository.

A nightly job with these variables set requires a machine that holds a RimWorld install, which GitHub-hosted runners do not. Whether to register a self-hosted runner is an owner decision (section 12); without it, the corpus suite stays a maintainer pre-release step recorded in the release checklist.

## 9. Benchmarks and regression gates

Benches use criterion and write one JSON result per run. `xtask bench-budgets` runs `rimstudio-app/benches/e2e_budgets.rs` plus crate benches and compares with `xtask/budgets.jsonc`.

| Bench | Budget source (SSD, 8 or more threads) |
|---|---|
| Cold list: parse every About.xml of a generated 700-mod tree | 100 ms to a usable list ([scan performance spike](../research/scan-performance-spike.md) budget table) |
| Warm start with manifest | 150 ms to a verified index |
| Refresh after one mod changed | 50 ms including index update |
| Def index build, warm cache | 300 ms; cold SSD 600 ms |
| Memory of the def index | under 100 MB for 700 mods |
| Manifest size | under 15 MB, rewritten at most once per second |
| Vanilla def load | under 1 s; CE load with 2,849 operations under 2 s ([def engine note](../research/def-engine-semantics.md) Implications 5) |
| Snapshot of 5,000 rows through the command layer | the IPC budgets of the [webview and IPC note](../research/webview-and-ipc-performance.md) |
| `query` command inline work | under 1 ms (I-13) |

Rules:

1. Benchmarks generate their tree (700 mods, 300,000 files, as in the packaging note section 5.4) instead of reading a real library; a variant reads `RIMSTUDIO_WORKSHOP_DIR` for the maintainer.
2. CI runners are noisy, so each result is divided by a reference micro-benchmark run in the same job (a fixed hashing loop) and the ratio is compared with the stored baseline ratio. A 2x regression fails; 1.25x warns. During calibration (the first milestones) the gate only warns, and `budgets.jsonc` records the date it turns blocking.
3. Peak RSS is recorded for the scan bench through the platform probe.
4. Startup time to the first frame event is measured by launching the release binary with an environment variable that exits after the first frame, once per OS, nightly.
5. Memory or time changes of more than 10 percent in a pull request are called out in its description by `xtask bench-budgets --compare main`.

## 10. Frontend tests

| Level | Scope | Rules |
|---|---|---|
| Unit (Vitest + jsdom) | `store.ts` pure actions, formatters, `shared/ipc` snapshot plus delta logic, `shared/lists` windowing maths, i18n layer, rich-text conversion, keyboard registry | Stores are tested without rendering; time is faked |
| Component | `packages/ui` components in the gallery, feature components with `@testing-library/preact` | Query by role and label; a component without keyboard parity fails review |
| Integration (Playwright + `mockIPC`) | Whole web layer on the Vite preview, scripted backend from `rimstudio-testkit` | Every feature has a happy path and an error path (envelope with a code); `clearMocks()` after each test |
| Gallery screenshots | Every `rimstudio-ui` component in light and dark themes at a fixed viewport | Screenshots committed under `tests/e2e/`; pixel tolerance fixed; they run on one Linux leg only so fonts match |
| Contract | Typed bindings drift (`cargo xtask bindings --check`) and a test that the mock backend implements every registry command it uses | Fails when a command is renamed |
| Locale | `en.json` key union generated; completeness script for other locales | Missing keys in `en.json` fail the build; other locales warn |
| Preact 11 leg | The same suites with Preact 11.0.0 (D-050) | Allowed to fail until the promotion review (2026-11-01), then blocking |

`rimstudio-testkit` provides the mock IPC, fixtures (fictional mods), render helpers and a script runner for streams (snapshot, then deltas with revisions). The frontend contains no business rule (I-04), so frontend tests assert wiring and behaviour (virtual list renders at most about 3,000 nodes for a 5,000 row snapshot, drag has a keyboard equivalent, cancellation sends `cancel_job`), never sorting or validation results; those are Rust tests exercised also through the CLI.

## 11. End to end smoke tests

The WebdriverIO suite (`tests/e2e/wdio/`) drives the real shell built with the `e2e` feature, which compiles in the embedded WebDriver server (a surface that must never ship). It runs nightly on Windows, Linux (under xvfb) and macOS, and on demand before a release. The script is: start, run the first-run wizard against a fixture tree, open settings, add a custom mod folder, scan, toggle a mod, sort, quit. `xtask check-release-features` fails the release build if `e2e` or `diagnostics` is enabled. A separate test starts the app with the Steam native library absent and asserts that every screen except Publish works (I-15).

The manual matrix of the packaging note (section 5.5: install, detect, external drive, scan 700 mods, update, running game warning, uninstall leaves user data) stays a release checklist item, since CI cannot run Windows 10, Fedora or a Steam Deck.

## 12. CI jobs and gates

Defined in `.github/workflows/ci.yml` (matrix windows-latest, macos-latest, ubuntu-22.04 with WebKitGTK 2.50 or newer for the shell) and `nightly.yml`; action versions pinned by hash.

| Job | Runs | Blocks merge |
|---|---|---|
| `rust` | `cargo fmt --check`, clippy with `-D warnings`, `cargo nextest run --workspace`, golden tests at 1 and 8 threads, `cargo deny check`, `cargo machete`, `cargo xtask check` (layers, cfg, deps, licences, docs, tools, size) | Yes |
| `bindings` | `cargo xtask bindings --check`, `schemas --check` | Yes (I-19) |
| `frontend` | `pnpm check` (tsc, oxlint, Prettier, dependency-cruiser), Vitest, locale script; second leg on Preact 11 | Yes, 11 leg after 2026-11-01 |
| `shell-build` | `tauri build --no-bundle` per OS; capability equals registry test | Yes |
| `e2e` | Playwright with `mockIPC` plus gallery screenshots | Yes |
| `perf` | `xtask bench-budgets` ratio gate | Warn, then blocking after calibration |
| `audit` | `cargo deny check advisories` (covers cargo audit), `pnpm audit` | Advisories block; scheduled daily too |
| `licence-hygiene` | `xtask check-licences`: no dataset file extensions under `crates/` or `apps/`, no value tables or real names from CE or vanilla in the tree, no files of reference projects outside `reference/`, no forbidden dash characters and no emoji in `docs/`, no use of the old folder name as a product name | Yes |
| `nightly` | WebdriverIO smoke on three OS, startup-time bench, proptest at 1,024 cases, optional corpus run on a self-hosted runner | No, files an issue on failure |

Owner decision: whether to run a self-hosted runner holding a RimWorld install and workshop folder for the nightly corpus job.

## 13. Test data and licence policy

1. No RimWorld file, vanilla value table, CE file or value, community dataset or real mod content is committed (R11, I-06, I-07). Fixtures are generated from code or are hand-written with invented names and numbers.
2. Reference projects (RimSort, RimCrow, CombatExtended) are read-only concept references and live outside the build; no test reads them, and no fixture is derived from their files.
3. Real-data tests read the user's files at run time, write nothing to them, and write results only to scratch. Their expected values are counts and hashes, never content.
4. The XPath shape file holds expression shapes with literals collapsed; the research script that derived it is kept, the raw patches are not.
5. `xtask check-licences` implements the scan listed in section 12 and its own tests: a deliberately bad fixture folder must make it fail (a test of the test).
6. Test names, fixtures and docs describe the games and other tools in our own words; fixtures use fictional package ids with the prefix `fixture.`.
7. Contributions that add a fixture state its origin in the pull request template (hand-written, generated, or derived and from what).

## 14. Invariant to test map

| Id | Invariant | Test or gate |
|---|---|---|
| I-01 | tauri-free-core | `xtask check-layers`; CI step `cargo tree -p rimstudio-cli -i tauri` prints nothing; the CLI builds in a container without GTK headers (nightly) |
| I-02 | xml-boundary | `cargo deny check bans` with wrappers; `xtask check-deps`; grep guard test with a deliberately bad fixture |
| I-03 | json-only-app-data | deny bans on SQLite, YAML and binary serialisers; test that every schema in `schemas/` has a writer in `rimstudio-io`; `rimstudio-io` store tests refuse non-JSON writes |
| I-04 | rules-in-rust | oxlint restriction on rule-shaped modules; sorting and validation suites run through `rimstudio-cli` with no webview |
| I-05 | game-folder-fence | `RecordingFs` write-set tests over deploy, launch and ModsConfig scenarios; `GameWriteFence` refusal tests for every path outside the two allow lists; mod folders in fixtures hash-compared before and after |
| I-06 | runtime-datasets | `xtask check-licences` extension scan; `FakeTransport` pipeline tests; test that a fresh checkout has no dataset under `crates/` or `apps/` |
| I-07 | no-reference-data | `xtask check-licences` value-table scan; designer scenarios with CE absent; vectors reviewed for fictional numbers |
| I-08 | no-feature-edges | `xtask check-layers` matrix; a fixture manifest with a feature edge must fail the check |
| I-09 | one-registry | Registry parity test (CLI route or `ui-only` marker); capability file equals registry; shell compile; bindings drift |
| I-10 | diagnostics-not-errors | Vanilla load zero-diagnostic test; damaged-fixture table (every damage yields diagnostics and a result); clippy `unwrap_used` and `expect_used` in libraries |
| I-11 | platform-quarantine | `xtask check-cfg`; platform contract tests per OS |
| I-12 | deterministic-output | Golden tests at 1 and 8 threads; proptest shuffle invariance; hash seed varied by running the suite twice |
| I-13 | jobs-for-long-work | Registry kind test (commands marked `query` have a bench under 1 ms); `JobRunner` tests for progress cap of 20 messages per second, cancel, cancel on channel drop |
| I-14 | no-ambient-webview | Capability equals registry test; `RootGuard` unit and property tests (traversal, symlink escape, case folding); CSP string test |
| I-15 | steam-native-in-sidecar | Dependency check that only the helper links the native crate; start-without-library e2e test |
| I-16 | explicit-context | clippy `disallowed_types` and `disallowed_methods`; whole suite runs in parallel under nextest without a serial lock |
| I-17 | user-vs-derived | Test that deleting the cache root leaves settings, rules, profiles and projects intact and the next start rebuilds; store API takes a root kind |
| I-18 | lossless-foreign | Round trip tests on synthetic fixtures; CST edit tests; byte-span tests; corpus round trip on a real library (ignored) |
| I-19 | generated-in-sync | `bindings --check`, `schemas --check` in CI |

## 15. Open items

| Item | Owner or spike |
|---|---|
| Whether to use a self-hosted runner for the corpus job | Owner |
| Pixel tolerance and the fixed screenshot platform for gallery tests | Decide during the S-12 spike in M0 |
| Playwright WebKit as a stand-in for WKWebView and WebKitGTK is unverified; treat as smoke only | S-10 |
| A process test crate for the CLI (`assert_cmd`) is unverified; a plain `std::process` harness is the default | M0 |
| Calibration date after which perf ratio gates block | After the first full month of CI data |
