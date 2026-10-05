# RimSort core domain logic, in our own words

Scope: an own-words description of the domain logic of RimSort (the engine a RimWorld mod manager needs: discovery, ingestion, indexes, sorting, validation, rule data, persistence, performance), cross-checked against the real game semantics from the decompiled game code and real files, so that RimStudio can build a faster and more correct Rust engine. RimSort is GPL-3.0 and is a read-only concept reference: nothing here is copied code or data. Dataset analysis and fetch design are covered by a separate note and are not repeated here.

Status: research note | Last verified: 2026-10-04

## Evidence base and method

Sources read: RimSort-main/app/models/**, app/sort/**, app/services/**, app/controllers/metadata_controller.py and sort_controller.py, app/views/main_content_panel.py (refresh, sort, save, import orchestration), app/views/mods_panel.py (validation), app/utils/{xml,schema,files,constants}.py, app/windows/rule_editor_panel.py (concepts only), RimSort-main/tests/**, docs/user-guide/sorting-algorithms.md and rule-editor.md. Game semantics come from the decompiled game (cited as decompiled:Verse/ModsConfig.cs and similar; behaviour described in our own words). Real data: the owner's machine, 785 folders in four roots (install Data 8, install Mods 55 entries, the owner's own mods folder 32, Steam workshop 690) and the active list of 610 mods in ModsConfig.xml, with the community rules file (631 rule entries) and the Steam database (57,679 entries) fetched earlier into scratch.

All measurements in this note come from our own replicas of the approach (Python scripts that mimic RimSort's algorithms and a small Rust benchmark), not from running RimSort itself. They are scratch material (not committed except where stated): scratchpad/rimsort-core-domain/analyze5.py (sorting replica), bench_parse.py, bench_resolve.py, bench_cs.py, bench_save2.py, bench_steamdb.py and rs_bench/src/main.rs. The committed scripts are docs/research/data/rimsort-core/scan_corpus.py (About.xml facts per mod folder) and gen_module_inventory.py (module table). Treat every timing as order of magnitude on one machine.

## 1. Discovery and ingestion

### 1.1 Sources scanned

RimSort builds one flat dictionary of mods, keyed by absolute mod path, from exactly three roots (evidence: app/models/metadata/metadata_mediator.py, method refresh_metadata):

1. the instance's workshop folder (Steam workshop content for app 294100),
2. the instance's local mods folder (defaults to the game's Mods folder when unset),
3. the game's Data folder (Core and official DLC).

Each root is listed one level deep; every immediate subdirectory is a candidate mod, files are ignored, nothing is scanned recursively. If the local mods folder or the game folder is missing or not a directory, the whole refresh is skipped (a single guard at the top). The workshop folder is optional. The game version string is read from Version.txt in the game folder and kept verbatim (here `1.6.4871 rev598`); it is "Unknown" when the file is missing.

Game cross-check: the game also uses exactly three sources (install Data folder, install Mods folder, Steam-subscribed workshop items; decompiled:Verse/ModLister.cs, RebuildModList). So RimSort matches the game. Consequence for requirement R4: a mod in an arbitrary custom folder is invisible to the game, and the game silently drops active entries it cannot find (decompiled:Verse/ModsConfig.cs, DeactivateNotInstalledMods). A manager can show custom-folder mods, but to make the game load one it must materialise it inside the install Mods folder (link or copy). RimSort has no custom-folder feature; its only extra root is the local folder setting.

Steam discovery: RimSort derives the Steam ACF path from the workshop path (two directories above content/294100 holds appworkshop_294100.acf). Path guessing for game, workshop and config lives in services/path_autodetect_service.py (fixed candidate lists per OS plus GOG and Heroic lookups); a helper in utils/generic.py reads libraryfolders.vdf. We did not trace every call site of that helper (see open questions). That is the R3 researcher's topic.

### 1.2 About.xml location and parsing

Per mod directory:

1. Find About/About.xml. A setting `case_insensitive_about_xml_lookup` (default on only when the platform is Linux) makes the lookup case-insensitive for both the folder and the file name. In the real corpus, 5 of 762 workshop mods have a lowercase `about.xml` inside `About`, which fails on a case-sensitive file system with a strict lookup. The game resolves the path case-insensitively (decompiled:Verse/ModMetaData.cs, Init, via GenFile.ResolveCaseInsensitiveFilePath), so strict lookup would be a bug.
2. If there is no About.xml: look for `*.rsc` files (scenario files). Exactly one gives a ScenarioMod; two or more abort with an invalid placeholder; none gives an invalid placeholder ("valid = false", path set, no package id).
3. The XML is converted wholesale into a nested dictionary (utils/xml.py), the root key is lowercased, and the mod data is the `modmetadata` entry. Inside it, tag names are matched case-sensitively in camelCase (`packageId`, `supportedVersions`, ...). A scalar-only element is unwrapped to a stripped string; a list element becomes `{"li": [...]}`, and a helper keeps unwrapping single-key dictionaries until a string or a multi-key structure remains.

Fields read and defaults (evidence: metadata_factory.py, `_parse_basic`, `_parse_optional`, `create_base_rules`):

| Field | How read | Default or fallback |
|---|---|---|
| packageId | string, stripped, lowercased by the case-insensitive wrapper | missing or blank: the sentinel `missing.packageid`, with a log warning (all such mods share one id) |
| steamAppId | digits only | else DLC table lookup by package id, else -1 |
| name | string | official DLC: fixed display name; else the package id |
| description | string | official DLC: fixed text; else a long "considered invalid" text |
| author, authors | `author` appended first, then `authors/li` items | empty list |
| supportedVersions | list or single string, stored as a set of raw strings | empty set |
| modVersion, url, modIconPath | strings | empty |
| descriptionsByVersion | matched with the version rule below | base description |
| modDependencies | list of dictionaries with packageId, displayName, workshopUrl, alternativePackageIds | empty; duplicates by id are skipped with a warning; null or empty `li` entries skipped |
| loadBefore, loadAfter | strings in `li` | empty set |
| forceLoadBefore, forceLoadAfter | merged into loadBefore and loadAfter (the distinction is lost) | empty |
| incompatibleWith | strings | empty |
| loadBeforeByVersion, loadAfterByVersion, incompatibleWithByVersion, modDependenciesByVersion | the entry for the target version replaces the base list entirely (not additive) when the setting `prefer_versioned_about_tags` is on (default on) | base list |

Not read at all: LoadFolders.xml (a text search of the app tree finds no reference), version-specific mod subfolders (1.5/, 1.6/, Common/), `targetVersion`, `modLatestUpdateDate`. In the corpus 291 of 785 folders (37%) carry a LoadFolders.xml. This is acceptable for ordering metadata (About.xml sits in one place), but means RimSort cannot tell which Defs a mod really loads for a game version and cannot see CE-style "loads only if mod X is active" folders.

Version matching differs from the game. The game lowercases each ByVersion child tag, strips a leading `v`, and looks up the exact string `major.minor` of the running version (decompiled:Verse/ModMetaData.cs, InitVersionedData and the VersionedData helper class); the first value for a duplicate key wins. RimSort tries `v1.6` and `1.6` as exact keys, then falls back to a regular-expression prefix match against `v1.6` (unescaped dot), so `v1.60` or `v1x6` would also match and `1.6.4871` matches. Practically harmless on real data (no mod in the corpus uses loadAfterByVersion or loadBeforeByVersion; modDependenciesByVersion appears in 24, incompatibleWithByVersion in 3, descriptionsByVersion in 1), but RimStudio should copy the game's exact rule.

packageId normalisation: lowercased everywhere (the wrapper type). 586 of 762 real package ids contain an uppercase letter, so normalisation is load-bearing. RimSort does not validate the id format. The game validates against a regular expression (letters, digits, dots, at most 60 characters, at least one dot, no empty segments), warns on the word "Ludeon" outside official content, and synthesises an id from author, a hash of the description and the name when the id is missing (decompiled:Verse/ModMetaData.cs, TryParsePackageId). In the corpus all 762 ids satisfy the game's pattern.

Core and DLC: Core and the five DLC are ordinary mods read from Data/<name>/About/About.xml with type `Ludeon`. A static table (utils/constants.py, RIMWORLD_DLC_METADATA) maps Steam app id, package id, name and description for the six official packages (ludeon.rimworld and .royalty, .ideology, .biotech, .anomaly, .odyssey) because the DLC About.xml files omit name and description. Real files: Core declares forceLoadBefore for Ideology and Royalty; each DLC declares forceLoadAfter for the earlier ones and forceLoadBefore for the later ones, so release order Core, Royalty, Ideology, Biotech, Anomaly, Odyssey is encoded in the files (Data/*/About/About.xml). The game additionally enforces it in code (decompiled:Verse/ModsConfig.cs, ReorderConflict: Core before expansions, official mods after Core) and removes DLC that the Steam client reports as not owned (decompiled:Verse/ModLister.cs, TryAddMod).

### 1.3 Mod type, published file id, git

Type is decided from the mod's parent folder (metadata_factory.py, `_set_mod_type`): parent equals game Data gives Ludeon; parent equals the workshop folder gives Steam Workshop; parent equals the local folder gives one of three: SteamCMD (a PublishedFileId.txt exists whose number equals the folder name), Git (a .git directory directly inside the folder), or, if only a PublishedFileId.txt exists, SteamCMD, else Local. A folder not directly under one of the three roots gets type Unknown. The order matters: a workshop-style download that also ships a .git folder must stay SteamCMD.

Published file id (ListedMod.published_file_id): read About/PublishedFileId.txt (UTF-8 with BOM tolerated) and accept only a positive integer; if the file is absent and the folder name is numeric, use the folder name. In the corpus 706 mods carry the file and in the workshop root all values equal the folder name. The game reads the same file with a plain unsigned parse (decompiled:Verse/ModMetaData.cs, Init) and keeps a separate "Steam workshop item" notion for items it enumerates from Steam.

Git: 132 of 785 corpus folders contain a .git directory (73 workshop, 46 install Mods, 13 owner). Detection uses pygit2 repository discovery plus a check that the repository root equals the mod folder; discovery cost was negligible on 27 folders (0 ms median in our replica).

Dependency fields: `modDependencies` entries carry packageId, displayName, steamWorkshopUrl or downloadUrl, and optional alternativePackageIds (the game supports alternatives: a dependency is satisfied if any listed id is active; decompiled:Verse/ModDependency.cs, IsSatisfied). The corpus has no mod using alternativePackageIds, so it is rare in practice, but a correct engine supports it. RimSort reads `workshopUrl` as the URL key, while the real tag is `steamWorkshopUrl` (the dependency resolver reads `steamWorkshopUrl` separately from raw XML, the factory does not), so the model's `workshop_url` field is effectively always empty. See pitfalls.

### 1.4 Duplicate package ids

Detection: group all valid About mods by lowercase id; groups with more than one path are "duplicates" (6 groups in the corpus: three inside the owner's folder, two install-Mods versus workshop, one owner versus workshop). Resolution happens only when turning a list of ids into paths (`get_mods_from_list` in controllers/metadata_controller.py):

1. For each wanted id, strip any `_steam` text (a plain substring replace over the whole id, so an id merely containing `_steam` in the middle is corrupted; see pitfalls) and remember whether the suffix was present.
2. Non-duplicate ids take their single path.
3. For duplicates, walk a priority list and take the first source type that has a copy; if several copies of the same type exist, take the first in natural sort order of the path. Plain id: Ludeon, Local, SteamCMD, Git, Workshop. Id with `_steam` suffix: Workshop, Local, SteamCMD, Git.
4. Ids that matched nothing are "missing".

Game cross-check: the game gives the Workshop copy the id suffix `_steam` only when the same id exists once in Workshop and once outside it; otherwise a second copy with the same id is an error and ignored (decompiled:Verse/ModLister.cs, TryAddMod). Which copy wins in the game is decided by scan order, not by a priority list, so RimSort's priority list is an improvement (local first) but differs. On saving, RimSort writes `<id>_steam` for the Workshop copy of any duplicated id (services/import_export_service.py), which matches the game's naming.

## 2. Data structures and indexes

Core structures (metadata_structure.py):

| Structure | Contents | Notes |
|---|---|---|
| CaseInsensitiveStr, CaseInsensitiveSet | lowercase-on-construction string and set | the whole engine relies on it |
| ListedMod | name, valid, supported_versions, mod_path (set once, also the uuid), mod_type (set once), description, plus lazily computed published_file_id, C# flag (any dll in Assemblies), XML-patch flag (any xml in Patches) | cached properties are never invalidated on refresh |
| AboutXmlMod | package_id, authors, mod_version, url, steam_app_id, three rule objects: about, community, user | `overall_rules` merges them lazily |
| ScenarioMod | summary | for .rsc folders |
| Rules | load_after, load_before, incompatible_with (case-insensitive sets), dependencies (dict by id), load_first, load_last | |
| CompiledDependencyData | deps_graph (id to set of ids it must load after), rev_deps_graph, tier_zero, tier_one, tier_three, incompatibilities (symmetric), declared_incompatibilities | rebuilt on every sort |
| ModsConfig | version, activeMods, knownExpansions | copy-on-read lists |

Merge rule (`overall_rules`): load_before, load_after and incompatible_with are unions of About, Community and User rules; dependencies are merged as a dictionary, later source overriding earlier (About, then Community, then User), though only About ever fills it in practice; load_first and load_last are true if community or user says so (About cannot).

Indexes and where they live:

1. `mods_metadata`: dict path to mod (the primary store, replaced wholesale on refresh).
2. `packageid_to_paths` (lazy, in MetadataController): id to set of paths; invalidated on any refresh, creation, deletion or update.
3. `steamdb_packageid_to_name` (lazy): lowercase id to Workshop name, built by scanning all 57,679 SteamDB entries when first needed.
4. Rules DBs: community and user rules dictionaries keyed by lowercase id; SteamDB keyed by published file id; Use This Instead keyed by old Workshop id.
5. Auxiliary SQLite database (SQLAlchemy): one row per mod path with type, published file id, ACF touched and updated times, external created and updated times, notes, colour, ignore flag, outdated flag, tags (many-to-many). The identity key is the folder path, so moving a mod folder loses its notes and tags.
6. The list widgets keep a Python list of paths (`paths`) for the active and inactive lists; positions are found with list.index (linear).
7. models/mod_list.py is a cleaner design (path index, package id index, diff, resolve) with tests, but nothing imports it yet.

Refresh (MetadataController.refresh_metadata): reset paths from the instance and settings; the mediator reloads user rules, community rules, SteamDB, No Version Warning and Use This Instead from disk; scans the three roots in batches on a Qt thread pool; attaches user and community rules to each About mod by id; then for every mod the aux DB gets a get-or-create plus type and published file id update, ACF timestamps are merged from the Steam and SteamCMD ACF files, caches are invalidated and a signal is emitted. Single-mod create, delete and update events (from a file watcher) call the same parser for one path and invalidate caches. Rules are attached at parse time, so editing user rules or downloading new community rules needs a refresh to take effect (inference from the structure: attachment happens inside the parser worker).

## 3. Sorting

### 3.1 Pipeline

```mermaid
flowchart LR
  A[Active paths from list widget] --> B[compile edges for ALL mods]
  B --> C[filter to active ids]
  C --> D[derive tiers 0,1,3 by closure]
  D --> E[tier 2 = the rest]
  E --> F[per tier: toposort levels, name order inside level]
  F --> G[concatenate 0,1,2,3 and dedupe]
  G --> H[replace active list]
```

Edge construction (CompiledDependencyData.build). Graph edges mean "this id must load after those ids". For every About mod (active or not) with merged rules:

1. loadAfter entries X of mod M give edge M after X (only if X is the id of some known mod, active or not).
2. loadBefore entries X of M give edge X after M.
3. forceLoadAfter and forceLoadBefore were merged into the same sets at parse time, so they are ordinary edges.
4. Community and user rules contribute loadAfter and loadBefore through the same merge; loadTop marks the mod "load first", loadBottom marks it "load last".
5. incompatibleWith builds a symmetric incompatibility map (not used for sorting).
6. Optional setting `use_moddependencies_as_loadTheseBefore` (default off): every declared dependency becomes an extra loadAfter edge, except when the mod or the dependency is a tier-1 or tier-3 mod, and except when an explicit rule already says the opposite order (the inferred edge is dropped to avoid a cycle). With `use_alternative_package_ids` the first installed alternative id replaces a missing dependency id.

By default modDependencies are therefore not ordering constraints, which agrees with the game: the game uses dependencies only for the "unsatisfied dependency" warning (decompiled:Verse/ModsConfig.cs, GetModWarnings), and builds its own sort graph from loadBefore, loadAfter and their force variants only (TrySortMods). Real-data check on the owner's 610 active mods: the dependencies-as-edges option changes the largest topological level from 150 to 106 mods and leaves violations at 0, so it mostly adds ordering that authors did not request.

Tier derivation (controllers/sort_controller.py):

1. Filter the compiled graph and its reverse to active ids.
2. Tier 0 seeds are a fixed list of nine ids: the six official ids plus brrainz.harmony, brrainz.visualexceptions and zetrith.prepatcher. Tier 1 seeds are a fixed list of thirteen framework ids (HugsLib, XML Extensions, Vanilla Factions Expanded Core and others, utils/constants.py) plus every mod with the load-first flag unless it is a tier-0 seed. Tier 3 seeds are mods with the load-last flag.
3. Tier 0 and tier 1 sets are the active seeds plus the transitive closure of what they must load after (their recursive loadAfter predecessors). Tier 3 is the active load-last mods plus the closure of everything that must load after them (reverse edges). The user guide describes tier 0 as "everything that recursively depends on" the seeds; the code does the opposite for tiers 0 and 1 (it pulls in predecessors, which is the sensible direction since predecessors must be loaded earlier). The guide text is inaccurate.
4. Tier 2 is every active mod in none of the other sets. Each tier's graph is the induced subgraph on its members; edges that cross tiers are discarded, never checked.
5. Per tier, run the selected algorithm; concatenate tier 0, 1, 2, 3; remove duplicates keeping the first occurrence (so a mod in both tier 1 and tier 3 stays in tier 1).

Real result on the owner's list (replica, Python, About plus community rules): tier sizes 8, 15, 560, 35 (sum 618 for 610 mods because overlaps are removed at the end), 0 violations, about 3 ms. The 610 current positions differ in 600 places from the topological result; the game's own TrySortMods run on the current order changes 506 places (mean displacement 4.55, maximum 504) and also gives 0 violations.

### 3.2 Algorithms

Topological (default). Python's `toposort` library groups nodes into levels: level 0 has no predecessors, each next level has all predecessors in earlier levels (longest-path layering). Inside a level, nodes are ordered by lowercase mod name (stable sort). Complexity O(V + E) plus O(L log L) per level. A cycle raises an error; the code enumerates all simple cycles with networkx (can be exponential on dense cyclic graphs), shows them in a dialog, and the whole sort is abandoned (return failure, list untouched). Nodes that are mentioned only as predecessors and are not active still appear in levels but are dropped when mapping ids to paths.

Alphabetical (deprecated, still selectable, logs a warning). Sort active mods by name, then walk that list; for each mod not yet placed, append it, then recursively insert its not-yet-placed predecessors (in name order) right before it, nudged to sit after already-placed predecessors of their own. This is an insertion heuristic with an index-based placement that does not look at already-placed mods that must come after the new insertion. Our replica on the real list gives 4 violated constraints among 610 mods (for example a mod that must load after Regrowth 2 placed before it) where topological gives 0. Complexity is quadratic in the worst case (`index` and `insert` on a list).

Tie-breaking and stability: only the lowercase name is used. Equal names, or the fallback string for a non-string name, fall back to iteration order of a Python set, which depends on string hash randomisation, so the order of two mods with identical names can change between runs (inferred from the code: levels are sets and are sorted with a name-only key). The sort is deterministic for distinct names. Compared with the game: the game's TrySortMods is a depth-first postorder over the current list, stable with respect to the existing order (it keeps most mods where they are); RimSort's result is a canonical alphabetical layering that moves most mods (600 of 610 in our test). Both satisfy the constraints.

### 3.3 Visible bugs and TODOs

1. `_do_sort` compares the new ordered list with the old active set (a set versus a list), so "order unchanged" is never detected and the lists are always rebuilt (views/main_content_panel.py).
2. Active mods that the sorter cannot map are silently dropped from the active list and appear as inactive after a sort: scenario or invalid entries (they are skipped), and any second active mod sharing a package id (the id-to-path dictionary keeps only one).
3. The compile step runs over all installed mods at each sort, and `compile` is called twice in `_do_sort` (the first result is discarded).
4. Alphabetical sort is marked deprecated in logs; code comment in `Sorter.__init__` records a TODO to precompute shared lookups.
5. Cross-tier constraints are dropped without any report; a loadBefore pointing from a tier-3 mod at a tier-2 mod, for example, is silently ignored.
6. `incompatibleWith` is never read from community or user rules (see 5.2).

### 3.4 Agreement and differences with the game

| Topic | Game (decompiled) | RimSort |
|---|---|---|
| Edge sources | loadBefore, loadAfter, forceLoadBefore, forceLoadAfter of active mods only | same, plus community and user rules, and optional dependency edges |
| Matching a target id | first active mod whose id matches, ignoring the `_steam` postfix | exact lowercase id; any installed mod (then filtered to active) |
| Cycle | reports the first mod found on a cycle in a dialog, does not sort | lists all simple cycles in a dialog, does not sort |
| Result style | depth-first postorder from the current list order | tiered levels, alphabetical in level |
| Core and DLC | enforced by reorder checks and by the DLC About files | enforced by tier 0 plus the same About rules |
| Manual move | refused when it violates a force rule (ReorderConflict) | allowed; violation shown as a warning |
| In-game ordering flag | only loadBefore and loadAfter (ModHasAnyOrderingIssues), not the force variants | all merged rules |
| dependencies | warning only | warning, optional order edges |
| load top and bottom | no such concept | tier routing |

## 4. Validation and warnings

Where: `recalculate_internal_errors_warnings` in views/mods_panel.py runs after every change of a list (insert, remove, reorder). It is domain logic inside a widget class. Problems are computed only for the Active list except version mismatch and replacement hints, which are computed for both.

| Problem | Rule | Class |
|---|---|---|
| Missing dependency | each merged dependency id (and, if the setting is on, its alternatives) must be an active package id; an active Use This Instead replacement also satisfies it (from the helper's name and call site; its body was not read) | error |
| Incompatibility declared by this mod | an id from the mod's own About incompatibleWith is active | error |
| Incompatibility declared only by the other mod | merged incompatible set minus the mod's own About set | error (shown as "per other mod's rules") |
| loadBefore violated | an active target appears at or before this mod's index | warning |
| loadAfter violated | an active target appears at or after this mod's index | warning |
| Version mismatch | the game's major.minor string is not in the mod's supportedVersions set, unless the mod has an empty set, or its id is in the No Version Warning list | warning |
| Replacement available | the mod's published file id is in Use This Instead | warning |

Severity and surfacing: errors and warnings are counted per list for a status bar, each row gets a tooltip assembled from the problem texts, and the totals open a dialog with per-mod text. Per-mod ignore: a package id in the ignore list (persisted by IgnoreManager as JSON) or a per-row toggle suppresses all of the above, including the version-mismatch flag. All checks look up targets by package id, so with duplicate active ids only the last path in the list answers.

Not checked by RimSort but checked by the game (decompiled:Verse/ModsConfig.cs, GetModWarnings): another mod with the same id already active (the game reports it per mod), the Core-first and DLC order rules (as hard reorder refusals), and a mod missing from disk (silently deactivated). Also absent: duplicate ids in the active list, unmet Core requirement, a mod listed twice, `Harmony` before everything (covered only by tier 0 at sort time). The version test compares raw strings: a mod declaring `v1.6` or `1.6.4871` would show a mismatch while the game parses versions (the game's parse routine was not located in the decompiled output, so its exact leniency is unverified). In the corpus all supportedVersions values are plain major.minor strings.

Real-data check (replica): in the owner's current order, the game-style rules (About only, force and soft) have 1,426 constraint pairs among active mods with 7 violated and 4 mods flagged; adding community rules gives 1,502 pairs, 12 violated, 9 mods flagged. One active dependency is unmet (one mod, one missing id). No declared incompatibility is active.

Cost: for each mod in the list, each constraint target calls `list.index` on the path list (O(n) each), so one recalculation is O(n times constraints times n), roughly 600 x 2.5 x 600 comparisons here, small in absolute terms but repeated for every row insertion unless batched.

## 5. Rule databases as consumed by the engine

### 5.1 Schemas

Community rules (communityRules.json, 394,716 bytes, 631 entries, timestamp 1789239946; user rules userRules.json uses the same schema with `{"timestamp": 0, "rules": {}}` as the default):

```json
{"timestamp": 1789239946, "rules": {"3tes.cgtwaa": {"loadAfter": {"gt.sam.glittertech": {"name": ["Glitter Tech"]}}}}}
```

Per package id (lowercased on load): `loadAfter` and `loadBefore` map an id to `{name: string or list, comment: string or list}`; `loadTop` and `loadBottom` are `{value: bool, comment}`. Field counts in the real file: loadAfter 481, loadBefore 220, loadBottom 12, loadTop 1, incompatibleWith 5.

Steam database (steamDB.json, 49.3 MB, 57,679 entries, `{"version": int, "database": {pfid: entry}}`): entry fields unpublished, url, packageId (also a lowercase `packageid` duplicate in some entries), gameVersions (list, string or null), steamName, name, authors (string, list or null), dependencies (id to list or `{name, url}`), blacklist `{value, comment}`, tags. Keys are lowercased on load. Consumers in the engine: package id to name map (Steam name preferred) for tooltips; dependency id to workshop id resolution; blacklist flag. No ordering input.

Use This Instead (replacements.json.gz, BOM-prefixed JSON, `{"version":..., "rules":[...]}`): each rule has oldWorkshopId, oldName, oldAuthor, oldPackageId, newWorkshopId, newName, newAuthor, newPackageId, oldVersions, newVersions. Indexed by old Workshop id (string). Matching is on the installed mod's published file id.

No Version Warning (ModIdsToFix.xml): XML root with `li` package ids and comments; loaded lowercased; may live in a per-game-version subfolder. This is the one external dataset that is XML; RimStudio must convert it to JSON at the boundary (requirement R10).

Rimworld versions list (rimworld_versions.json): depots per platform for the downloader; not part of ordering.

### 5.2 Merge precedence and gaps

Order rules are unions: About, community and user rules all add edges; there is no override or removal, so a user cannot cancel a community rule (a conflicting pair becomes a cycle). Dependencies come only from About. loadTop and loadBottom come only from community and user rules (the game has no such concept). Settings toggles: dependency edges optional; ByVersion tags optional.

Gap found by static reading: the engine's rule type for external rules has no `incompatibleWith` field, so incompatibilities in community and user rules (5 in the real community file) are ignored when decoding, and the converter that builds a Rules object never sets them. The rule editor and the user guide still describe incompatibleWith for community and user rules, and the validation code distinguishes "declared by this mod" from "declared by the other mod". In effect only About-declared incompatibilities act (not confirmed by running RimSort).

### 5.3 Proposed Rust serde models (sketch)

```rust
// All ids are normalised to lowercase at the boundary (newtype PackageId).
#[derive(Deserialize, Serialize)] struct ExternalRulesFile { #[serde(default)] timestamp: i64, #[serde(default)] rules: BTreeMap<PackageId, ExternalRule> }
#[derive(Deserialize, Serialize, Default)] struct ExternalRule {
  #[serde(default, rename = "loadAfter")] load_after: BTreeMap<PackageId, RuleNote>,
  #[serde(default, rename = "loadBefore")] load_before: BTreeMap<PackageId, RuleNote>,
  #[serde(default, rename = "incompatibleWith")] incompatible_with: BTreeMap<PackageId, RuleNote>,
  #[serde(default, rename = "loadTop")] load_top: Option<BoolRule>,
  #[serde(default, rename = "loadBottom")] load_bottom: Option<BoolRule> }
struct RuleNote { name: OneOrMany<String>, comment: OneOrMany<String> }   // string or array on the wire
struct BoolRule { value: bool, comment: OneOrMany<String> }
struct SteamDbFile { version: i64, database: HashMap<String, SteamDbEntry> }  // key = published file id
struct UseThisInsteadFile { version: String, rules: Vec<Replacement> }        // camelCase fields, BOM tolerated
```

Design notes: unknown fields must be preserved on round trip for user rules (so a newer RimSort or RimStudio does not lose data); `OneOrMany` handles the string-or-list variants seen in the data; load the 49 MB SteamDB lazily or from a compact cache rather than at start-up (see section 7). App-owned persistence of user rules is JSON, and the internal form may be JSONC where comments are useful (R10).

## 6. Persistence, instances, import and export

Settings at the model level: one Settings struct (about 100 fields, models/settings.py) stored as JSON; relevant to the engine are: sorting algorithm (default topological), dependency-edge flag (off), alternative-id flag (on), check-dependencies-on-sort (on), prefer ByVersion tags (on), case-insensitive About lookup (on only on Linux), history enabled (on) and retention (100), and the instances dictionary. The full catalogue is another note.

Instances: an Instance record holds name, game folder, config folder, local folder, workshop folder, run arguments, SteamCMD install path and flags, Steam protocol launch flag and an optional instance folder override. The active instance chooses which three roots are scanned. InstanceService can copy game, config, local and workshop folders into a new instance, back an instance up to an archive, restore from an archive, clone or delete.

ModsConfig.xml (game file in the config folder): `ModsConfigData` with `version`, `activeMods` (list of ids, lowercase, `_steam` suffix for the Workshop copy of a duplicated id) and `knownExpansions`. RimSort writes it through a generic dictionary-to-XML routine and a pretty printer, in a plain non-atomic write (JSON files use an atomic write with retry, ModsConfig.xml does not). When saving, knownExpansions is regenerated as every official DLC id except Core, installed or not, which may suppress the game's "new expansion" notice (effect unverified). The game itself writes the same file and prunes missing mods on start.

Import and export formats (services/mod_list_parser.py, utils/schema.py, import_export_service.py):

| Format | Detection | Shape |
|---|---|---|
| RimWorld ModsConfig.xml | text starts with `<`, root ModsConfigData | version, activeMods/li, knownExpansions/li |
| RimWorld save (.rws) | root savegame | savegame/meta/modIds/li (and modNames, modSteamIds, gameVersion in meta) |
| RimWorld mod list (.rml) | root savedModList | savedModList/meta/modIds/li |
| RimSort JSON list | text starts with `{` | version, activeMods (list), knownExpansions (list) |
| History snapshot | JSON with key rimsortSnapshot | same list fields plus metadata (below) |
| Clipboard report | text | numbered mod list with names, ids, links |
| Rentry report | Markdown uploaded to rentry.co with a per-upload size limit | list plus tables |
| Workshop collection import | Steam web API | published file ids resolved to local mods |

A broken or unreadable list falls back to a default list containing only the base game and shows a dialog.

Save-file parsing: the whole save is parsed into a dictionary to read the meta block. On the owner's 82.7 MB Autosave the Python approach takes about 5.0 s (1.6 s XML parse plus 3.4 s dictionary conversion) and peaks near 1 GB of memory (replica). The `meta` block is at the start of the file, so a streaming reader that stops after `</meta>` is enough: our Rust streaming replica reads the 610 ids in 0.11 ms.

List comparison and history: a snapshot (services/modlist_history_service.py) is a JSON file named by timestamp and a 12-character prefix of a SHA-256 over the lowercased ordered id list (order matters), containing the plain list fields plus a `rimsortSnapshot` block: id, short id, timestamp, version, game version, instance, previous id, note, active and inactive entries with package id, name, published file id and source. A snapshot is skipped when its id equals the newest one; old snapshots are pruned to the retention count; a text log line records counts and deltas. Diff produces active added, removed, reordered, and inactive added and removed. The in-list save comparison indicator marks active mods that are not in the latest save and inactive mods that are.

Backups: a saves backup (a number of recent save files compressed into a backup folder with old backups pruned, optional before game launch) and instance backups (archive of folders). Aux DB and user rules are plain files that users back up by hand.

Restore and revert: "restore" resets the lists to the last saved state kept in memory.

## 7. Performance

Measured on this machine with replicas (743 mod folders from workshop, install Mods and Data; 610 active mods):

| Step | RimSort-style (Python replica) | Rust replica or alternative |
|---|---|---|
| Parse 743 About.xml files, single thread | 66 ms | 21 ms |
| same with threads | 102 ms with 16 threads (slower than single thread because of the interpreter lock) | 3.5 ms with a rayon pool |
| Probe C# and XML-patch flags (two directory walks per mod, an executor per call) | 950 ms; a single-pass no-thread variant 123 ms | one directory walk per mod, lazy, a few ms |
| Resolve 610 ids to paths | 51 ms with a scan of all mods per id and list membership | 0.3 ms with a hash index |
| Load SteamDB (49 MB, 57,679 entries) | 0.19 s typed decode plus lowercase rebuild; about 300 MB peak; 0.39 s with the standard json module | 138 ms typed parse with serde_json; a compact on-disk cache or lazy per-key lookup removes it from start-up |
| Read modIds from an 82.7 MB save | 5.0 s, about 1 GB peak | 0.11 ms streaming |
| Sort 610 active mods | about 3 to 5 ms | microseconds; not a bottleneck |
| Aux DB per mod get-or-create (27 mods) | 25 ms first, 11 ms later (about 0.4 ms per mod, so 600 mods about 250 ms) | one batched upsert or a plain JSON map |

What is slow and why:

1. XML to nested dictionary conversion of every About.xml, then repeated dictionary probing with `value_extractor`: allocation heavy; and the parser threads are pure Python so the interpreter lock serialises them (threaded parse measured slower than single thread).
2. The C#-mod and patch probes (cached properties, each walks directories); most visible when the UI asks for column data for every row.
3. Per-mod SQL round trips during refresh, and a SteamDB scan to build the name map on first use.
4. Linear `list.index` calls and widget-level data updates on every list change; validation rebuilt for all rows after each change.
5. Whole-file XML parse of saves, and full recompilation of the dependency graph for all installed mods (not just active) each sort.
6. Everything refreshes as a unit: no incremental refresh keyed by mod folder mtime; a file watcher exists for single-mod events only.

Where Rust can be dramatically better: parallel streaming parse into typed structs, cached parse results keyed by (path, mtime, size), an in-memory graph with integer ids and CSR adjacency, incremental recompilation of only changed mods, mmap or streaming for saves, lazy memory-mapped SteamDB index, and a validation function that is incremental (only constraints touching moved items re-evaluated) with a position array instead of searches. For the typical 600 to 700 mod library the whole cold start can be well under 100 ms, and a warm start (cache hit on unchanged folders) a few milliseconds.

## 8. Test scenarios worth re-creating

Scenarios only (RimSort tests/ was read for scenario names; no test code is reused):

1. Graph sort: single mod; linear chain; diamond; wide graph with alphabetical tie-break; multiple roots with a shared dependency; disconnected subgraphs; graph entry for an id that is not active; active path missing in metadata; circular dependency reports failure; non-string or missing name.
2. Tiering: tier zero first, tier one between, tier three last; transitive closure into tier one and into tier three; a mod claimed by tier one and tier three; a tier-two mod depending on a tier-one mod; inactive mods and inactive cycles ignored; output has no duplicates; the partition is disjoint and exhaustive; constants not mutated.
3. Rules: two mods loadAfter; two mods loadBefore; chain of three; user loadAfter overriding About loadBefore; user load-first promotes to tier one; user load-last demotes to tier three; explicit rule wins over an inferred dependency edge and the sort stays valid.
4. List resolution: duplicate ids with each source mix (local versus workshop, git versus workshop); `_steam` suffix; id missing from disk; case differences in ids; natural-sort tie among copies of one type.
5. About.xml ingestion: BOM; lowercase file name on a case-sensitive file system; missing packageId (sentinel versus the game's synthesised id); `authors` list versus `author`; dependency with alternatives; ByVersion exact, prefix and absent; supportedVersions as a single string; zero or several .rsc files; empty About directory; broken XML.
6. Validation: missing dependency satisfied by an alternative or a replacement; incompatibility declared by one side only; loadBefore and loadAfter at equal and adjacent indices; ignore list; version mismatch with No Version Warning; the game version string with and without build suffix.
7. Import and export: ModsConfig, RimSort JSON, rml and rws inputs; empty activeMods; single `li` string versus list; unknown format; round trip of `_steam` ids; history snapshot id stability and pruning; diff of reorder-only changes.
8. Large inputs: a save file of tens of megabytes read for the meta block only; a 50 MB rules database; 700 mods with 1,500 constraints (the real owner library is a ready fixture for private runs, never committed).

## Known pitfalls

1. A shared dataclass default: the base mod class assigns one random uuid string at class definition, so every mod without a path shares the same uuid (placeholder objects collide).
2. `_steam` handling uses a substring replace over the whole id, so an id with `_steam` inside it is altered; the game only strips a trailing suffix (decompiled:Verse/ModsConfig.cs, TryGetPackageIdWithoutExtraSteamPostfix).
3. The dependency URL is read from the key `workshopUrl`, but real About.xml files use `steamWorkshopUrl`; the model field is therefore empty and only the separate resolver reads the right tag from raw XML.
4. forceLoadBefore and forceLoadAfter are merged into the soft sets, losing the "hard rule" distinction that the game uses to refuse manual reorders.
5. Community and user `incompatibleWith` are dropped by the decoder (see 5.2).
6. Sorting drops, without notice, active mods that are not About mods or that share a package id with another active mod, and the "order unchanged" check can never be true.
7. Name-only tie-breaking depends on set iteration for equal names, which varies between process runs.
8. Alphabetical sort can violate constraints on real data (4 of 1,502 pairs on the owner's list).
9. Cycles across tiers cannot be detected because cross-tier edges are discarded.
10. LoadFolders.xml and version subfolders are ignored, so version support, C# detection and Defs detection read the mod root only.
11. Version matching of ByVersion keys uses an unanchored regular-expression prefix with an unescaped dot.
12. ModsConfig.xml is written non-atomically and knownExpansions is rewritten for all DLC, installed or not.
13. The auxiliary database keys rows by path and declares the tag link column as integer although the key is a path string; moving a mod loses its notes and tags.
14. A packageId that is missing becomes the same sentinel for every such mod, so they all look like duplicates of one another.
15. The game's own checks (same id active twice, hard-rule refusal on reorder, silent deactivation of missing mods) are not mirrored, so RimSort can write a list that the game rewrites on start.
16. Rules are attached at parse time, so rules changes take effect only after a refresh.

## Implications for RimStudio

1. Build the engine as headless crates (for example rimstudio-mods for scan and About ingestion, rimstudio-xml as the single XML boundary, rimstudio-rules for datasets and user rules, rimstudio-sort for graph and tiers, rimstudio-validate for problems). Test: the engine builds and its tests pass with no GUI dependency.
2. Normalise every package id to lowercase in one newtype at the boundary; reject nothing silently. Test: a corpus run reports counts of ids with uppercase, missing, or invalid-format ids, and all comparisons are case-insensitive.
3. Read About.xml exactly like the game: case-insensitive path resolution on every OS, BOM tolerant, ByVersion lookup by exact `major.minor` after stripping `v` and lowercasing, ByVersion replaces the base list, first duplicate key wins. Test: fixtures for the 5 lowercase `about.xml` files and for each ByVersion variant.
4. Keep the game's hard-rule semantics in the model (hard force flags distinct from soft hints) and expose both in validation and manual reorder (refuse or warn). Test: reorder against a forceLoadAfter rule is flagged as a hard violation.
5. Support three scan sources like the game plus user-added custom folders (R4): scan and display custom folders directly, and materialise a custom-folder mod into the game's Mods folder (link or copy, per platform) before it can be activated; never write an activeMods entry the game cannot resolve. Test: a mod only in a custom folder is flagged "not loadable until linked" and the game list never contains it unlinked.
6. Resolve duplicate ids with an explicit, user-visible policy and keep `_steam` semantics exactly (trailing suffix only, applied only when a workshop copy and a non-workshop copy coexist). Test: duplicate fixtures for each source mix.
7. Sorting: keep the tier idea (core, frameworks, normal, load-bottom) as a documented rule, make cross-tier edges an explicit check that reports conflicts, use a deterministic total order (name, then package id) inside levels, report all cycles with the involved rule sources, and never drop active mods silently (keep or report them). Test: running the sort twice, and with shuffled input, gives identical output; 0 violations on the owner's 610-mod fixture; unmappable mods listed in the result.
8. Offer the game-style sort (stable depth-first from the current order, minimal movement) as an alternative to canonical layering. Test: on the owner's list it changes far fewer positions than 600 of 610.
9. Treat dependencies as warnings by default and make "dependencies imply order" an opt-in with the same conflict-suppression rule. Test: with the option off, the graph contains no dependency edges.
10. Validation as pure functions over (ordered ids, index, rules): missing dependency (with alternatives and replacements), incompatibility (both directions), hard and soft order violations, version mismatch (with the No Version Warning list converted to JSON), duplicate id active, replacement available. Use an id-to-position array; one recalculation of 610 mods with 1,500 constraints must run in under 1 ms (target, to be benchmarked).
11. Parse in parallel with a typed streaming reader and a persistent cache keyed by path, mtime and size; cold scan of the owner's 743 folders under 50 ms and warm under 10 ms (targets; the replica measured 3.5 ms parallel parse only).
12. Read saves with a streaming reader that stops after the meta block, never a full parse (82.7 MB save in well under 10 ms; replica 0.11 ms).
13. Keep app-owned data JSON or JSONC only (rules, user rules, history, instances, caches); convert ModIdsToFix.xml to JSON at fetch time; write ModsConfig.xml atomically (temp file then rename) and write knownExpansions only for installed DLC.
14. Use mod folder identity that survives moves where possible (published file id or package id plus source) rather than the path alone for notes, tags and colour.
15. Merge rule precedence explicitly: About, community, user as separate layers with provenance, and let user rules both add and suppress community rules, so conflicts are resolvable instead of becoming cycles.

## Open questions

1. How does the game treat a mod folder reached through a symlink or junction inside the Mods folder, on each OS? Needed to decide the link strategy for custom folders (R4). Not tested here.
2. The game's version comparison routine (VersionControl) was not located in the decompiled output; whether `v1.6` or full build strings in supportedVersions are accepted is unverified.
3. Where RimSort uses the libraryfolders.vdf helper in utils/generic.py (call sites not traced); relevant to R3 and handled by another note.
4. Whether the knownExpansions rewrite changes game behaviour (new-expansion notice) was not tested.
5. Does the rule editor really write incompatibleWith into userRules.json that is then ignored? The reading is static; confirming needs a running RimSort.
6. Should RimStudio read LoadFolders.xml to compute which version folders and conditional folders apply to a mod? The corpus shows 291 of 785 folders carry one; the answer affects C# detection, Defs ownership and the def explorer.
7. How should a user cancel a community rule (an override layer) without editing the shared dataset?
8. The sort comparisons used About plus community rules only; user rules were empty in this test. A fixture with user rules is still needed.
