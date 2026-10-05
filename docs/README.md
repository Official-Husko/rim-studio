# RimStudio documentation

RimStudio is a planned cross-platform desktop application for RimWorld modders: a fast, intuitive mod manager (modelled on RimSort's feature set, compatible with the RimSort community rules datasets) and a modding toolkit (def explorer, patch tester, an item designer for weapons and apparel that writes vanilla definitions and can optionally generate a Combat Extended patch, and a Steam Workshop publisher), built as a Rust workspace with a Tauri 2, Preact, TypeScript and Tailwind frontend. This folder is the plan and its evidence: research notes with reproducible data, the architecture and project structure, decision records, feature specifications, and the written design brief for the visual design tool. The Cargo workspace skeleton exists under `crates/` and `xtask/` and the Rust backend of release 0.1.0 (the designer slice) is being implemented; progress is on the [0.1.0 backend status page](status/0.1.0-backend.md). The folder is still named `rimforge-studio`: renaming it to `rimstudio` and moving the reference checkouts into `reference/` are pending owner action.

Status: draft | Last updated: 2026-10-04

## Start here

Read in this order; each document links to the next level of detail.

1. [Architecture overview](architecture/overview.md): the three pillars, the named invariants (I-01 to I-19), the layered architecture, the process model and ten scenario walkthroughs.
2. [Glossary](glossary.md): the terms used everywhere else.
3. [Workspace layout](architecture/workspace-layout.md): **the project structure**: the full repository tree, naming rules, the allowed-dependency matrix, how to add a tool module, and the exact step-0 commands from the current cargo-init state.
4. [Roadmap](roadmap.md): milestones M0 to M7 and the 0.1.0 designer slice with testable exit criteria, the spikes that gate each, the risk register and the **owner decisions** that are waiting.
   Progress of the first release: [0.1.0 backend status](status/0.1.0-backend.md).
5. [Design brief](design/design-prompt.md): the in-depth prompt to paste, in parts, into the visual design tool; [UI inventory](design/ui-inventory.md) is its source (53 screens, states, components).
6. [Decision register](architecture/decision-register.md) and the [ADR index](adr/README.md): why each choice was made, what was rejected, and what is still open.

## Owner requirements and where each is covered

| Req | Requirement | Main documents |
|---|---|---|
| R1 | Rust backend, Tauri 2, Vite, Preact, TypeScript, Tailwind; Windows, macOS, Linux | [overview](architecture/overview.md), [workspace layout](architecture/workspace-layout.md), [cross-platform](architecture/cross-platform.md), [frontend stack research](research/frontend-stack-research.md) |
| R2 | Modular, non-monolithic, reusable components; docs folder | [workspace layout](architecture/workspace-layout.md), [crate catalog](architecture/crate-catalog.md), [frontend architecture](architecture/frontend-architecture.md), [ADR 0002 to 0005](adr/README.md) |
| R3 | Auto-detect the game, Steam libraries and Workshop through VDF/ACF files | [game and mod discovery](features/game-and-mod-discovery.md), [detection research](research/steam-and-game-detection.md), [ADR 0019](adr/0019-own-vdf-reader-and-detection-report.md) |
| R4 | Any number of custom mod folders in the settings | [game and mod discovery](features/game-and-mod-discovery.md), [settings](features/settings.md), [ADR 0020](adr/0020-managed-link-farm.md), [ADR 0021](adr/0021-game-folder-write-fence.md) |
| R5 | One app: mod manager (RimSort-based) plus toolkit; performance and intuitive design | [mod manager](features/mod-manager.md), [load order and validation](features/load-order-and-validation.md), [modding workspace](features/modding-workspace.md), [performance strategy](architecture/performance-strategy.md) |
| R6 | RimSort community rules dataset compatibility with auto-fetching | [community datasets](features/community-datasets.md), [datasets analysis](research/community-datasets-analysis.md), [fetch and merge design](research/rules-fetch-and-merge-design.md), [ADR 0016](adr/0016-remote-datasets-runtime-only.md), [ADR 0017](adr/0017-lossless-layered-rules.md) |
| R7 | Item designer: vanilla reference, vanilla definitions by default, optional Combat Extended patches, math, optional quiz | [items toolkit](features/items-toolkit.md), [item balance math](features/item-balance-math.md), [CE patching](features/combat-extended-patching.md), [ADR 0033](adr/0033-item-math-and-ce-generator.md), the vanilla and CE research notes |
| R8 | Blueprint-inspired modern UI; an in-depth design prompt | [design brief](design/design-prompt.md), [UI inventory](design/ui-inventory.md) |
| R9 | Workshop upload and update, based on the owner's Parallax project | [workshop publishing](features/workshop-publishing.md), [research](research/workshop-publishing-research.md), [ADR 0032](adr/0032-publish-sidecar-and-steam-library.md) |
| R10 | App data JSON, configs JSONC, XML only at the RimWorld file boundary | invariants I-02 and I-03 in the [overview](architecture/overview.md), [data and persistence](architecture/data-and-persistence.md), [ADR 0006](adr/0006-xml-boundary-and-byte-span-editing.md), [ADR 0007](adr/0007-node-tree-and-xml-free-defs.md), [ADR 0011](adr/0011-json-caches-and-cache-keys.md) |
| R11 | Licence hygiene for RimSort, RimCrow, Combat Extended and datasets | [ADR 0034](adr/0034-licence-hygiene.md), [security and privacy](architecture/security-and-privacy.md) |
| R12 | Docs conventions | this page, [tooling and conventions](architecture/tooling-and-conventions.md) |

## Map of the documentation

### Architecture ([architecture/](architecture/))

| Document | What it fixes |
|---|---|
| [overview](architecture/overview.md) | Scope, invariants, layers, process model, data flow, storage map, scenarios |
| [workspace layout](architecture/workspace-layout.md) | The repository tree, naming, dependency matrix, step 0, enforcement toolchain |
| [crate catalog](architecture/crate-catalog.md) | All 24 workspace members: purpose, API sketch, dependencies, tests |
| [decision register](architecture/decision-register.md) | Every decision D-001 and up with status, evidence and open design issues |
| [ipc and state](architecture/ipc-and-state.md) and [command catalog](architecture/command-catalog.md) | The command registry, DTO conventions, snapshot plus delta, jobs; every command in one table |
| [frontend architecture](architecture/frontend-architecture.md) | Folders, state, theming, virtual lists, drag and drop, component library |
| [data and persistence](architecture/data-and-persistence.md) | Every file the app reads or writes: format, owner, migration, backup |
| [performance strategy](architecture/performance-strategy.md) | Measured budgets and the scan pipeline |
| [cross-platform](architecture/cross-platform.md) | OS matrix, link farm per OS, packaging and CI decisions |
| [security and privacy](architecture/security-and-privacy.md) | Threat model, capabilities, secrets, dataset trust, licence enforcement |
| [testing strategy](architecture/testing-strategy.md), [error handling and logging](architecture/error-handling-and-logging.md), [tooling and conventions](architecture/tooling-and-conventions.md) | How quality is enforced |

### Architecture decision records ([adr/](adr/))

43 short records (0001 to 0043) in the [ADR index](adr/README.md), each with context, decision, consequences, rejected alternatives and evidence.

### Feature specifications ([features/](features/))

Requirement ids are stable and testable: MM (manager), GD (discovery), ST (settings), LO (load order and validation), CD (community datasets), WS (workspace), IT (items), WP (publishing).

| Document | Covers |
|---|---|
| [mod manager](features/mod-manager.md) | The manager end to end: lists, search, undo, sort preview, groups, profiles, import and export, launch, task centre |
| [game and mod discovery](features/game-and-mod-discovery.md) | Detection report, sources, custom folders, duplicates, the link farm and pre-launch checks |
| [settings](features/settings.md) | The settings model, files with examples, the screens |
| [load order and validation](features/load-order-and-validation.md) | Rule layers, sorting, the validation catalogue, explain |
| [community datasets](features/community-datasets.md) | The five datasets, fetching, user rules, RimSort import and export |
| [modding workspace](features/modding-workspace.md) | The toolkit foundation: projects, Def Explorer, patch tester, validators, log and save tools |
| [mod layout](features/mod-layout.md) | The RimStudio mod layout v1: folder names taken from the game, the scaffold, placement of generated files, recognition of an existing mod's convention, the layout check and the project tree |
| [items toolkit](features/items-toolkit.md), [item balance math](features/item-balance-math.md), [CE patching](features/combat-extended-patching.md) | The item designer (vanilla by default), its formulas and quiz, and the optional Combat Extended patch generation |
| [workshop publishing](features/workshop-publishing.md) | Staging, preflight, the Steam helper protocol, history |

### Status ([status/](status/))

| Document | Covers |
|---|---|
| [0.1.0 backend status](status/0.1.0-backend.md) | A living page: the requirement groups of release 0.1.0 with their state (planned, in progress, done, deferred) |

### Design ([design/](design/))

[design brief](design/design-prompt.md) (paste in parts; Parts H and the "How to use" section are for you) and [UI inventory](design/ui-inventory.md).

### Research ([research/](research/))

30 evidence notes with reproducible scripts and data under `research/data/`. Start at the [research index](research/README.md), which also lists the names and decisions that supersede the notes. Architecture and feature documents win where they differ from a research note.

## Decisions waiting for you

The full list with recommendations and deadlines is [roadmap section 12](roadmap.md#12-owner-decisions). The ones that change what gets built first:

1. The **app identifier** (placeholder `app.rimstudio.desktop`; it fixes data folders, so choose before real users keep data).
2. **Link deployment default per operating system**, decided after spike S-03 (an in-game test of linked mods).
3. The **macOS notarization path** (long lead time), and the **permission request** to the RimSort maintainers about dataset licences.
4. Spike S-04 needs your go-ahead: it creates and deletes a real Private Workshop item in your account through your installed Steam.

Decided on 2026-10-04 and no longer waiting: the in house JSON database with no SQL and the read-only RimSort import ([D-083](architecture/decision-register.md)), the 0.1.0 scope ([D-084](architecture/decision-register.md)), vanilla by default with an optional CE patch ([D-085](architecture/decision-register.md)), the installed Steam used directly ([D-086](architecture/decision-register.md)), no Windows signing ([D-087](architecture/decision-register.md)) and a custom licence added later ([D-088](architecture/decision-register.md)).

## Conventions for these documents

1. **Evidence first.** Research notes cite files, decompiled game code (described, never copied) and URLs; claims that could not be verified are marked "(unverified)". Research notes are evidence and are not edited after a decision; the decision register and ADRs record what was decided.
2. **Licence hygiene (R11).** RimSort (GPL-3.0), RimCrow (MIT) and Combat Extended (CC BY-NC-SA 4.0) are read-only references: concepts are described in our own words and no code or data file is copied. Vanilla and Combat Extended values are read from the user's own install at run time; the datasets under `research/data/` are research artefacts and are never shipped. The community datasets are fetched at run time.
3. **Formats (R10).** All app-owned data is JSON; configs are JSONC; XML appears only when reading or writing RimWorld's own files, through one boundary crate; there is no SQL and the app keeps its records in the in house JSON document store ([D-083](architecture/decision-register.md)). Documents say so wherever it matters.
4. **Vanilla by default (D-085).** The item designer always writes vanilla definitions; a Combat Extended patch is an optional, per item opt in, never automatic.
5. **Licence later (D-088).** The project licence is custom and comes later: no `license` field, no `LICENSE` file and no licence headers until the owner supplies one.
6. **Style (R12).** No em dashes, no en dashes, no emojis, no mention of AI assistance in project content. Names: product RimStudio, crate prefix `rimstudio-`; the working folder keeps the name `rimforge-studio` and the GitHub repository is `rim-studio` (D-099).
7. **Milestone numbering follows the [roadmap](roadmap.md#11-mapping-to-the-milestone-names-in-the-register)** (M0 to M7); older mentions in some documents use the register's names and are mapped there.
8. **Updating.** A merged feature updates its specification and, if it changes a decision, adds or supersedes an ADR (see [tooling and conventions](architecture/tooling-and-conventions.md)). Reference checkouts named in evidence pointers (`RimSort-main`, `RimCrow-main`, `CombatExtended-Development`) are local and not part of the repository; step 0 moves them into a git-ignored `reference/` folder.
