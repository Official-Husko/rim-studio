# RimStudio glossary

The terms used across the RimStudio documentation, with the meaning they have in this project. Terms that name code (types, crates, files) are defined precisely in the [crate catalog](architecture/crate-catalog.md) and the [command catalog](architecture/command-catalog.md); terms that name product concepts are defined here first. When a word has two meanings in the wider RimWorld community (for example "workspace" or "mod"), this glossary states the one RimStudio uses.

Status: draft | Last updated: 2026-10-04

| Term | Meaning in RimStudio |
|---|---|
| Mod | A folder with `About/About.xml` that the game can load, plus the official Core and DLC content packs. |
| Package id | The `packageId` from `About.xml`; the `PackageId` newtype lowercases it for lookups and keeps the original spelling for display and write back. |
| ModId | Stable app identity of one mod copy: `w<workshopId>` for Workshop items, `<source>:<packageId>` for others. Survives moves inside a source. |
| Mod source | One place mods come from: game Data, install Mods, a Steam library workshop folder, or a custom folder. |
| Custom folder | A user added mod folder (R4) with layout, depth, priority, link mode and read-only flag; invisible to the game until deployed. |
| Library | Everything scanned from all enabled mod sources, as a `ModIndex`. |
| Installation | One detected or user pinned RimWorld install (game folder, `Mods` dir, user config dir, game version, workshop folder). |
| Detection report | The complete result of game and Steam detection: all candidates with how they were found and a confidence, never only the first hit. |
| Active list | The ordered list of enabled mods that becomes `ModsConfig.xml`. |
| Profile | A named active list with metadata, kept in the data root; history snapshots are automatic profiles. |
| Load plan | The folders of one mod that the game would read for a given version and active set (`LoadFolders.xml`, version folders, Common). |
| Content pack | One mod's resolved load plan as a unit that the def engine consumes. |
| Link farm | The set of links (junctions or symlinks) or copies RimStudio creates in `<install>/Mods` so the game sees custom folder mods. |
| Ownership manifest | The JSON record of every link or copy RimStudio created; only entries in it may ever be removed. |
| Pre-launch check | The verification that every active id resolves to a folder the game will find, run before any write of `ModsConfig.xml` or launch. |
| Reference set | The game install plus chosen mods in load order, used as read-only context by the toolkit. |
| Project | A mod folder the user authors, with metadata kept outside the shipped folders. |
| Workspace | An open project plus a reference set and one def database. |
| Node tree | RimStudio's own XML-free tree (`tag`, ordered `attrs`, `children`) used by patches, defs, templates and golden files. |
| Def database | The merged, patched and inherited defs of a load order, with provenance, queried by the toolkit. |
| Def index | The cheap streaming summary of defs (type, name, parent, file, offset) that powers search without full resolution. |
| Type table | JSON map of def class names to base class and abstractness, generated from assemblies; needed to type defs. |
| Patch operation | One RimWorld `PatchOperation` (XPath plus action); unknown custom classes are carried as raw nodes and marked "not simulated". |
| Provenance | Where a def or a node came from: file, mod, parents, and the patch operations that touched it. |
| Dataset | A fetched community data file (community rules, SteamDB, Use This Instead, No Version Warning, RimWorld versions) with state and an index. |
| Rule layer | One source of ordering rules (About force, About soft, community, user, derived) with a priority and per edge provenance. |
| Hard rule, soft rule | Hard: the game enforces it (`forceLoad*`, official expansions). Soft: a hint such as `loadAfter` or a community rule. |
| Tier | A coarse sorting band (core, frameworks, normal, load bottom) used before the graph order. |
| Diagnostic | A structured finding with code, severity, mod, file and message; the only way content problems are reported. |
| Job | A long running cancellable operation with a caller minted id and a progress channel. |
| Session | A mutable, revisioned in-memory state held by the backend: `LibrarySession` or `WorkspaceSession`. |
| Snapshot, delta | A full list payload with a revision, and an incremental change set (`upserts`, `removes`, `order`) against it. |
| Command registry | The single table in `rimstudio-app` from which wrappers, CLI routes, permissions and bindings are generated. |
| DTO | A serde type in `rimstudio-ipc-types` that crosses a process boundary; camelCase JSON, 64 bit ids as strings. |
| Tool module | A toolkit unit (Def Explorer, Patch tester, Designer and so on): a backend module or crate, a frontend feature, a descriptor and required capabilities. |
| Capability | A runtime fact a tool may require, such as a loaded install, CE present or a reachable Steam helper. |
| Port | A trait in `rimstudio-core::ports` through which a crate reaches the operating system or a clock, so tests can fake it. |
| Staging copy | The mandatory filtered copy of a project that the publisher uploads, built from ignore rules. |
| Publish plan | The listed set of files, sizes and checks shown before anything is sent to Steam. |
| Steam helper | The sidecar process that holds the Steam native library and speaks the newline JSON protocol. |
| Calibration | The optional item designer step (a short role quiz or reference picks) that adjusts the baseline the fit is scored against. |
| Fit meter | The designer display of predicted value, bands and rank among reference items per stat. |
| Document store | The in house JSON database in `rimstudio-io`: a directory of JSON documents, one file per id, each in the envelope `{kind, v, data}`, with atomic writes, forward only migrations, a compact index and quarantine of unreadable files. Used for designer drafts, projects and calibration caches; no SQL (D-083). |
| CE patch (optional) | A Combat Extended compatibility patch the user chooses to generate for one item through the off by default toggle "Add a Combat Extended patch (optional)" or the Convert flow; it goes into its own files in a folder gated by `LoadFolders.xml`, is never automatic and never changes the vanilla definition (D-085). |
| Vanilla by default | The owner rule that the designer always writes vanilla definitions; Combat Extended is only an optional patch. |
| 0.1.0 | The first release: the designer slice (weapons editor, optional CE patch generator, minimal manager) built backend first with the CLI as the interim front end (D-084). |
| Slim index | A compact JSON projection of a large dataset (for example SteamDB) built locally for fast lookup. |
| Golden file | A committed JSON expected output compared structurally by a test. |
| Data root | One of the config, data, cache or logs directories resolved by `DataRoots`. |

## Words with more than one meaning

| Word | Meanings in these documents | Rule |
|---|---|---|
| Workspace | The Cargo workspace (repository level), the crate `rimstudio-workspace`, the `WorkspaceSession` and the frontend feature folder `workspace/` | Say "Cargo workspace" for the build unit; the toolkit's Workspace (a project plus a reference set) is capitalised |
| Source | A mod source (a place mods come from) versus a dataset source (where a dataset is fetched from) | Always qualify: "mod source" or "dataset source" |
| List | The active list (what the game loads) versus a list file (an import or export) versus a UI list (a virtualised component) | "Active list" for the first, "list file" for the second |
| Rule | A load order rule (an ordering edge from About.xml, a community dataset or the user) versus a clippy or lint rule | "Load order rule" or "rule layer"; "lint rule" for tooling |
| Patch | A RimWorld patch operation (XML applied to defs) versus a Combat Extended patch (a whole generated patch file) versus a JSON patch of a user rules export | Qualify when ambiguous |
| Def | A RimWorld definition (an XML element such as a ThingDef) | Never "definition file" for the app's own data, which is JSON |

