# RimStudio research notes

This folder holds the 26 research notes and the data and scripts behind them. The notes were written before the architecture and feature documents and are the evidence those documents rest on; where a note and an architecture or feature document differ, the architecture and feature documents win (D-080 in the [decision register](../architecture/decision-register.md#12-integration-decisions)). This index groups the notes by theme with a one line summary each, points to the data folders and how to reproduce them, and lists the names and decisions that supersede what the notes say.

Status: draft | Last updated: 2026-10-04

Milestone numbering follows the [roadmap, section 1.1](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register) (older mentions of M3 to M6 use the decision register's numbering).

## 1. Reading rule

1. Architecture documents ([overview](../architecture/overview.md), [crate catalog](../architecture/crate-catalog.md), [command catalog](../architecture/command-catalog.md), [data and persistence](../architecture/data-and-persistence.md) and the others) and feature specifications under `docs/features/` are authoritative.
2. Research notes keep their findings, measurements and evidence; the names, file locations and numbering they propose are superseded where section 4 says so.
3. Every note carries "Status: research note" and a verification date; none is edited to follow later decisions, so a stale name in a note is expected and listed below rather than corrected in place.

## 2. The notes by theme

### 2.1 Reference applications and architecture

| Note | Summary |
|---|---|
| [Reference architectures](reference-architectures.md) | How eight mature Rust and Tauri applications structure a workspace: layering, crate counts, dependency graphs, lints and CI, used as vocabulary for the layout. |
| [RimCrow analysis](rimcrow-analysis.md) | The second reference mod manager (MIT, Python and Vue): what it does, how it is built, its mod-creator plan, and which ideas to adopt or avoid. |
| [RimSort feature and UX inventory](rimsort-feature-and-ux-inventory.md) | Every RimSort menu, button, dialog, flow and shortcut with pain points, the capability bar the manager must match. |
| [RimSort module inventory](rimsort-module-inventory.md) | Each Python module of RimSort with measured size, responsibility, layer and the decision for RimStudio. |
| [RimSort core domain](rimsort-core-domain.md) | An own-words description of RimSort's engine (discovery, indexes, sorting, validation, rules, persistence) checked against real game semantics. |
| [RimSort settings catalogue](rimsort-settings-catalog.md) | All persisted RimSort settings with type, default, meaning and tab, used to decide which to keep, rename or drop. |
| [Modding toolkit scope](modding-toolkit-scope.md) | Candidate toolkit modules with user stories, difficulty, dependencies, phases and risks, grounded in corpus evidence and competitor plans. |
| [Ecosystem survey](ecosystem-survey.md) | Existing RimWorld modding tools, documentation and developer switches in October 2026, and the gaps RimStudio fills. |

### 2.2 Game format, engine semantics and performance

| Note | Summary |
|---|---|
| [Mod format and corpus](rimworld-mod-format-and-corpus.md) | On-disk format and load semantics of RimWorld 1.6 mods verified in decompiled code, with corpus statistics and parser requirements. |
| [Def engine semantics](def-engine-semantics.md) | Precise rules of the Def loading pipeline (folder selection, merging, patches, inheritance, MayRequire) and the executable specification prototype. |
| [XPath patch coverage](xpath-patch-coverage.md) | Which XPath 1.0 constructs real patches use and the decision to evaluate them with an in-house engine. |
| [Steam and game detection](steam-and-game-detection.md) | How to find Steam, libraries, the game, Workshop content and config folders per OS, the VDF and ACF formats and the detection report model. |
| [Scan and index performance spike](scan-performance-spike.md) | Measurements with real Rust code on the real library that set the budgets for scanning, parsing, caches and thread scaling. |

### 2.3 Datasets and rules

| Note | Summary |
|---|---|
| [Community datasets analysis](community-datasets-analysis.md) | Schemas, counts, transport, cadence and licence terms of the community rules, SteamDB and the small datasets. |
| [Rules fetch and merge design](rules-fetch-and-merge-design.md) | How to fetch, validate, store, layer and migrate the five datasets under R6, R10 and R11. |

### 2.4 Combat Extended and item balance

| Note | Summary |
|---|---|
| [Combat Extended model](combat-extended-model.md) | What CE changes in combat, the units and formulas of each stat and measured value distributions for weapons, ammunition and apparel. |
| [CE patch conventions](ce-patch-conventions.md) | How CE compatibility patches are written in practice: anatomy per item kind, gating and an idiom survey. |
| [CE auto-patcher formulas](ce-autopatcher-formulas.md) | CE's programmatic conversion as explicit functions and how closely it reproduces the hand tuned values. |
| [CE conversion quality, measured](ce-conversion-eval-0.1.0.md) | Leave one out and held out error of the automatic conversion predictors, before and after the similarity estimator with reliability ratings (aggregates only). |
| [Vanilla ranged weapons](vanilla-ranged-weapons-analysis.md) | Statistical structure of every vanilla ranged weapon and a computed baseline for a new one. |
| [Vanilla melee weapons](vanilla-melee-weapons-analysis.md) | The same for melee weapons, with the verified DPS and penetration formulas and a power index. |
| [Vanilla apparel and materials](vanilla-apparel-analysis.md) | The mathematical structure of armour and apparel balance and an automatic baseline for new apparel. |

### 2.5 Stack, delivery and publishing

| Note | Summary |
|---|---|
| [Frontend stack research](frontend-stack-research.md) | Verified brief for Preact, TypeScript, Vite and Tailwind with version pins, state patterns and heavy components. |
| [Webview and IPC performance](webview-and-ipc-performance.md) | How the frontend talks to Rust in Tauri 2, moving thousands of records, typed bindings, webview differences and a numbered budget. |
| [Rust crate research](rust-crate-research.md) | Crates.io checked evaluation of the backend dependencies in ten categories with pins and rejections. |
| [Cross-platform packaging research](cross-platform-packaging-research.md) | Building, signing, updating and testing on Windows, macOS and Linux and the OS behaviours that change tool logic. |
| [Workshop publishing research](workshop-publishing-research.md) | How the game's uploader and the owner's earlier Parallax manager publish, the Rust options and Steamworks redistribution. |

## 3. Data folders and reproduction

The data and scripts are under [data](data/) in one folder per note family. They are inputs to the notes, not part of the product, and no folder is needed to build RimStudio.

| Folder | Supports | Entry point |
|---|---|---|
| [data/ce-autopatcher](data/ce-autopatcher/README.md) | CE auto-patcher formulas | its README lists each script; tests run with `python3 -m unittest discover -s tests` |
| [data/ce-dataset](data/ce-dataset/) | CE model, patch conventions | `build_dataset.py` and `analyze.py` |
| [data/ce-patch-templates](data/ce-patch-templates/README.md) | CE patch conventions | its README |
| [data/community-datasets](data/community-datasets/) | community datasets analysis | `cdlib.py`, `coverage.py`, `library_scan.py`; results in `results/` |
| [data/def-engine](data/def-engine/README.md) | def engine semantics | its README (executable specification and test vectors) |
| [data/frontend-stack](data/frontend-stack/) | frontend stack research | registry metadata tables |
| [data/item-calibration](data/item-calibration/README.md) | item balance baselines | its README (two prototypes and the verdict) |
| [data/mod-corpus](data/mod-corpus/) | mod format and corpus | `scan_mod_corpus.py`, summary in `mod_corpus_summary.json` |
| [data/perf-spike](data/perf-spike/) | scan performance spike | `run_all.sh`, Cargo project in `src/`, results in `results/` |
| [data/rimsort-core](data/rimsort-core/) | RimSort core domain | analysis scripts |
| [data/rules-fetch-design](data/rules-fetch-design/) | rules fetch and merge design | `analyze_*.py` |
| [data/rust-crates](data/rust-crates/) | Rust crate research | `fetch_crates.py`, summary in `crates_summary.tsv` |
| [data/schema-probe](data/schema-probe/README.md) | def engine semantics | its README (a C# probe and a validation script) |
| [data/steam-and-game-detection](data/steam-and-game-detection/) | Steam and game detection | probe scripts and fixtures |
| [data/vanilla-apparel](data/vanilla-apparel/), [data/vanilla-weapons](data/vanilla-weapons/) | vanilla analyses | analysis scripts and tables |
| [data/workshop-publishing](data/workshop-publishing/) | Workshop publishing research | probe scripts |
| [data/xpath-corpus](data/xpath-corpus/) | XPath patch coverage | `xpath_coverage.py`, summary in `summary.json` |
| [data/gen_rimsort_settings_table.py](data/gen_rimsort_settings_table.py) | RimSort settings catalogue | generates the settings table |

To reproduce a result:

1. Set `PYTHONDONTWRITEBYTECODE=1` before running any script so no `__pycache__` folders appear.
2. Read the folder's README first when it has one; the notes name the script that produced each table in their method sections.
3. Scripts that need the game install, the Steam Workshop folders, Combat Extended or the RimSort and RimCrow checkouts expect those at the paths named in the note's evidence conventions; raw third party dataset copies are deliberately not committed (licence, see the [community datasets analysis](community-datasets-analysis.md)).
4. The Rust spike builds with `cargo` from `data/perf-spike`; its measured tables are committed under `results/`.

## 4. Names and decisions that supersede the notes

| Topic | What the notes say | What applies now | Source |
|---|---|---|---|
| Crate `rimstudio-defdb` | Def index, inheritance, patches, provenance | `rimstudio-defs` (with `rimstudio-xpath` separate) | D-004, [crate catalog](../architecture/crate-catalog.md) |
| Crate `rimstudio-project` | Project model and metadata store | `rimstudio-workspace` (reference sets, projects, def index) | D-004 |
| Crates `rimstudio-tauri-*` (`-mods`, `-toolkit`, `-publish`) | One adapter plugin per feature | One command registry in `rimstudio-app`, wrappers generated into `rimstudio-shell` | D-005, [ADR 0003](../adr/0003-one-command-registry-and-app-root.md) |
| Crate `rimstudio-mods` | Scan, About ingestion, load order graph | `rimstudio-library` (scan), `rimstudio-sort` and `rimstudio-rules` (graph), `rimstudio-manager` (use cases) | crate catalog roster |
| Crate `rimstudio-patch` | Patch operations simulator | `rimstudio-defs` (apply patches) and the `defs` module of `rimstudio-toolkit` (patch tester) | crate catalog |
| Crates `rimstudio-detect`, `rimstudio-steam` | Detection logic separate from the VDF reader | one crate `rimstudio-steam` (VDF, ACF and detection) | D-004 |
| Crate `rimstudio-dataset` | Generic remote dataset | `rimstudio-datasets` | D-004 |
| Crates `rimstudio-dataset-http`, `rimstudio-steamdb-index`, `rimstudio-rimsort-import` | Separate fetcher, slim index and import crates | modules of `rimstudio-datasets` (`net`, slim index, `rimsort`) | crate catalog section 2 |
| Crates `rimstudio-rules-format`, `rimstudio-rule-graph` | Two crates for formats and the layered graph | two modules of `rimstudio-rules` | crate catalog, `rimstudio-rules` entry |
| Crate `rimstudio-ce-calibration` | A calibration module with a name to be decided | the `ce` module of `rimstudio-design`, split later as `rimstudio-ce` | crate catalog section 2 |
| Crates `rimstudio-tool-*` (as a pattern) and `rimstudio-lsp` | One crate per tool; a language server | one crate `rimstudio-toolkit` with modules `defs`, `project`, `designer`; no language server in scope (`rimstudio-tool-texture` only when a texture encoder dependency appears) | OD-14, crate catalog |
| Crates `rimstudio-ui`, `rimstudio-testkit` | Frontend packages | unchanged: npm packages `rimstudio-ui` and `rimstudio-testkit` in `packages/` | [frontend architecture](../architecture/frontend-architecture.md) |
| Command names `list_mods_snapshot`, `subscribe_mods`, `get_mod_detail` | Research spellings | `mods_snapshot`, `mods_subscribe`, `mods_get_detail`; all other aliases in the [command catalog](../architecture/command-catalog.md#3-names-that-were-renamed-or-merged) | D-071, D-072 |
| `paths.jsonc` | A separate path override file | the `paths` section of `workspace.jsonc`, with the shape unchanged | D-030, [data and persistence](../architecture/data-and-persistence.md) section 3.1 |
| A fourth config file | Not in the notes | `launch.jsonc` (graphics environment variables and safe-graphics flag, read before the webview exists) in addition to `settings.jsonc` and `workspace.jsonc` | D-077 |
| `userRules.json` in `<data dir>/dbs/` | RimStudio user rules in the RimSort file and location | `userdata/rules/user-rules.json` in the community `{timestamp, rules}` shape with unknown keys preserved; the RimSort compatible `userRules.json` is produced as an export only | [data and persistence](../architecture/data-and-persistence.md) section 3, [community datasets](../features/community-datasets.md) |
| Suppression sidecar `rule-overrides.jsonc` | One sidecar with an `{ edge, reason, since }` entry | `userdata/rules/suppressions.json`, keyed by (subject, kind, target) with a reason | data and persistence section 3 |
| Dataset policy and location | One `datasets` block; clones under a `dbs/` folder | `datasets.items.<id>` in `workspace.jsonc`; one slot per dataset under `cache/datasets/<id>/` with `current`, `previous`, `state.json`, `index/`, `quarantine/` and `tmp/` | data and persistence sections 3.3 and 6 |
| Layer ranking of rule sources | `user`, `about-force`, `about-soft`, `community`, `derived` (default `rulePriority`) | `core` and `about-force` are hard layers that always rank first and are not configurable; the configurable layers are `user`, `about-soft`, `community`, `derived` through `sorting.layerPriority`; ties break in an order independent way | [load order and validation](../features/load-order-and-validation.md) LO-003 (rule layers) |
| Milestones and phases | Phases P0 to P3 and the register's M0 to M6 | the roadmap's M0 to M7 (manager MVP is M2, manager v1 is M3, toolkit foundation M4, item designer M5, Workshop publishing M6, polish and release M7) | [roadmap](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register), D-078 |
| `rust-version = "1.90"` | Workspace skeleton in the crate research | `rust-version = "1.95"`, toolchain 1.96 | D-074 |
| Folder name | `rimforge-studio` appears in two notes | the repository folder is still misnamed; step 0 of the roadmap renames it to `rimstudio` | [ADR 0001](../adr/0001-workspace-rename-and-naming.md) |

Where a note and a document differ in a way this table does not list, use the document and record the difference in the [decision register](../architecture/decision-register.md).

## Implications for RimStudio

1. A reader who needs a name, file path, milestone or decision starts from the architecture and feature documents and uses the notes only for evidence and measurements.
2. The data folders keep the research reproducible; the regression vectors of the Rust crates are derived from them (for example the def engine vectors for `rimstudio-defs`).
3. Corrections found while reading a note are recorded in the decision register or the owning document, not in the note.

## Open questions

1. Whether the notes should receive a one line banner pointing at section 4, or whether this index is enough, is left to the owner.
