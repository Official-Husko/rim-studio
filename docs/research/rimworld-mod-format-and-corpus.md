# RimWorld 1.6 mod format, load semantics and mod corpus

Scope: the on-disk format of RimWorld 1.6 mods and the game's load semantics (About.xml, LoadFolders.xml, folder anatomy, mod enumeration, ModsConfig.xml, the Defs and patch pipeline, versions and logs), verified against the decompiled game code, the real install and the real mod library, plus corpus statistics and the parser requirements and scan budget they imply for RimStudio. Every statement carries an evidence pointer; anything not verified is marked "(unverified)".

Status: research note | Last verified: 2026-10-04

Evidence conventions: `decompiled:<path>` is the ILSpy output of Assembly-CSharp.dll for build 1.6.4871 rev598 (Ludeon code, never copied here; behaviour is described in my own words). `corpus:` means a field of `docs/research/data/mod-corpus/mod_corpus_summary.json`, produced by `docs/research/data/mod-corpus/scan_mod_corpus.py` over 764 mod folders (690 Steam workshop, 47 install Mods, 27 owner folders; the 6 official Data folders are reported separately). The scan ran in about 4.6 s wall time on this machine with 16 workers (cache state not controlled, so not a cold-scan figure).

## 1. About.xml

The game reads `About/About.xml` (the `About` directory must match case exactly; only the file name is resolved case-insensitively) into a private class. A missing file yields an all-default record, a file that fails to parse logs an error and also yields defaults ("Loading defaults instead"). Evidence: decompiled:Verse/ModMetaData.cs (Init), decompiled:Verse/DirectXmlLoader.cs (ItemFromXmlFile, ItemFromXmlString), decompiled:Verse/GenFile.cs (ResolveCaseInsensitiveFilePath).

### 1.1 Fields the game reads

Tag names are the C# field names and are matched case-sensitively (corpus: 0 tag-case variants in 756 About.xml files). Root element name is not checked (corpus: all 756 use `ModMetaData`).

| Tag | Type, default | Notes |
|---|---|---|
| `name` | string, "" | Empty becomes the folder name; for workshop items "Workshop mod <folderId>". 756 of 756 mods set it. |
| `shortName` | string | Display fallback only. Corpus: 0 uses. |
| `author` | string, "Anonymous" | Split on " and " and on commas for display. 735 mods. |
| `authors` | list of `li` | Overrides `author` for display when non-empty. 20 mods. |
| `packageId` | string, "" | See 1.3. 756 of 756 set it. |
| `description` | string, "No description provided." | See 1.4 for the comment hazard. 754 mods. |
| `descriptionsByVersion` | element whose children are named after versions | See 1.5. 1 mod. |
| `supportedVersions` | list of `li` | See 1.2. 756 mods. |
| `targetVersion` | string | Obsolete, only produces a warning when present. Corpus: 0. |
| `modVersion` | string | Free text. 131 mods. |
| `url` | string | 284 mods declare it, 210 non-empty. |
| `modIconPath` | string (texture path in the mod) | If non-empty it is resolved through the texture loader, otherwise `About/ModIcon.png` is used if present. 162 mods. |
| `steamAppId` | uint | Only used for official DLC detection. 3 mods. |
| `modDependencies` | list of `li` ModDependency | Children: `packageId`, `displayName`, `steamWorkshopUrl`, `downloadUrl`, `alternativePackageIds` (list of `li`). 564 mods declare it, 535 non-empty. |
| `loadBefore`, `loadAfter` | list of `li` packageId | Soft ordering hints. 55 and 576 mods. |
| `forceLoadBefore`, `forceLoadAfter` | list of `li` packageId | Used only for newly detected official expansions at startup. 0 and 6 mods. |
| `incompatibleWith` | list of `li` packageId | 73 mods. |
| `modDependenciesByVersion`, `loadBeforeByVersion`, `loadAfterByVersion`, `incompatibleWithByVersion` | element with one child per version, each containing the normal list | See 1.5. Corpus: 24, 0, 0 and 3 mods. |
| `modLatestUpdateDate` | not a field | Silenced by an ignore attribute, corpus: 1 file. Any other unknown tag is logged as an error but harmless. |

Evidence: decompiled:Verse/ModMetaData.cs (ModMetaDataInternal), decompiled:Verse/ModDependency.cs, decompiled:Verse/ModRequirement.cs, corpus:all_requested_roots.field_usage. Other tags seen in real files that the game ignores are not catalogued here.

### 1.2 supportedVersions

1. `ludeon.rimworld` (case-insensitive) always supports the current version regardless of its list.
2. A missing list is a warning, an empty list an error; the mod still loads, with an empty compatible set.
3. Each `li` is split on "."; at least two parts, the first two must be non-negative integers; extra parts are dropped with a warning ("1.6.4871" parses as 1.6). A single number or text (for example "v1.6") fails with an error.
4. Compatibility is exact major and minor equality with the running game (`IsCompatible`); build and revision are never compared. Evidence: decompiled:RimWorld/VersionControl.cs.
5. "Made for newer version" means no compatible entry but some entry above the running major.minor.

Corpus (756 About.xml): entries per mod p50 3, p95 7, max 10; mods listing 1.6: 636 (84.1 percent); only older versions: 120; listing 1.5: 615, 1.4: 507; stray values "1.7", "1.8", "1.9" in 1 mod each (a placeholder style), "0.19" in 2; unparsable or malformed entries: 0. corpus:supported_versions.

### 1.3 packageId

- Format regex (decompiled:Verse/ModMetaData.cs, PackageIdFormatRegex): at most 60 characters, ASCII letters, digits and dots only, no leading dot, at least one dot, no two dots in a row, last character alphanumeric. Underscores and hyphens are invalid.
- Violations only log a warning and set a flag; the id is kept as written. The word "ludeon" anywhere in a non-official id is a warning too.
- If empty, the game fabricates an id from author, a short numeric hash of the description and the name, mapping any non-alphanumeric or non-ASCII character to a letter by arithmetic (ConvertToASCII). The hash is the game's own string hash, so reproducing generated ids exactly needs that function (open question). Corpus: 0 mods lack a packageId, so this path is rare.
- Identity is the lower-cased id (`packageIdLowerCase`, lower-cased with the current culture). Lookup dictionaries use a culture-ignore-case comparer. Active-mod lookups also trim whitespace and lower-case with the invariant culture. Evidence: decompiled:Verse/ModLister.cs (static constructor, GetActiveModWithIdentifier).
- Postfix handling: two dictionaries exist, `modsByPackageId` keyed by the effective id (which may end in `_steam`) and `modsByPackageIdIgnorePostfix` keyed by the plain lower-cased id. When a workshop copy and a non-workshop copy share an id, the workshop copy is renamed by appending `_steam` and added; two copies from the same kind of source are rejected with an error and the later one is ignored. Enumeration order decides who is first (section 4). `ignorePostfix` lookups are what dependencies, `LoadFolders` conditions and `MayRequire` use, so they match either copy. Evidence: decompiled:Verse/ModLister.cs (TryAddMod), decompiled:Verse/ModMetaData.cs (SamePackageId, PackageId).
- Corpus: 750 distinct lower-cased ids among 756 declared; 6 duplicate groups (3 are owner-folder backup copies, 2 are install Mods vs workshop copies of the same mod, 1 is owner vs workshop); 1 case-only conflict (`Mlie.ResearchTree` vs `Mlie.researchtree`, which the game treats as the same id); 1 mod declares an id ending in `.steam` (`Dubwise.DubsPerformanceAnalyzer.steam`); 4,560 packageId references of which 228 differ in spelling from the target's declared spelling (so case-folding is mandatory); 0 references with surrounding whitespace. corpus:package_ids, corpus:package_id_references.

### 1.4 What malformed input does

| Input | Game behaviour | Evidence |
|---|---|---|
| About.xml absent | Mod still listed, all defaults, name = folder name, generated packageId, no supported versions (warning) | decompiled:Verse/DirectXmlLoader.cs |
| About.xml not well-formed | Error logged, defaults used (same as absent) | same |
| UTF-8, UTF-16 or UTF-32 BOM | Removed by the text reader before parsing (About.xml and LoadFolders.xml use ReadAllText) | decompiled:Verse/DirectXmlLoader.cs; Mono check in 7.3 |
| `encoding=` declaration disagrees with the bytes | Ignored, because the document is parsed from an in-memory string | local Mono experiment, 7.3 |
| Whitespace or newline before `<?xml` | Parse failure | local Mono experiment, 7.3 |
| `description` containing a comment or child elements | The string field is dropped (empty or default). Corpus: 1 mod | corpus:string_fields_dropped_by_game_due_to_comment_or_child_nodes |
| Dependency with empty packageId, invalid packageId, empty displayName, or neither `downloadUrl` nor `steamWorkshopUrl` (non-Ludeon) | Entry silently removed from the dependency list (warning only; workshop mods are not logged). Corpus: 3 of 765 entries dropped, in 2 mods, all for missing URLs | decompiled:Verse/ModMetaData.cs (ValidateDependencies), corpus:dependencies |
| Duplicate tag in one file | Error logged for most types. Corpus: 0 | decompiled:Verse/DirectXmlToObject.cs |
| `loadAfter` entries that are not installed | Ignored | by construction (they are only ordering hints) |

PublishedFileId.txt (`About/PublishedFileId.txt`) is read with an unsigned-integer parse, which tolerates surrounding whitespace; a parse failure just leaves the id unset. It is written without a newline when the game publishes. Evidence: decompiled:Verse/ModMetaData.cs (Init, SetPublishedFileId).

### 1.5 ByVersion elements

`descriptionsByVersion`, `modDependenciesByVersion`, `loadBeforeByVersion`, `loadAfterByVersion` and `incompatibleWithByVersion` have children named after versions. Child names are lower-cased and a leading `v` removed (`v1.6` and `1.6` are the same key); the first occurrence of a key wins and later ones log a warning. At init the game looks up the running major.minor ("1.6", no build) and, if found, replaces the corresponding base field entirely (not merged). Comments between children are skipped. Corpus: `modDependenciesByVersion` keys 1.1 to 1.6 (4, 7, 12, 16, 21, 16 mods). Evidence: decompiled:Verse/ModMetaData.cs (VersionedData, InitVersionedData). Note that validation of dependencies runs after this replacement.

## 2. LoadFolders.xml

Location: mod root, file name matched case-insensitively (corpus: `LoadFolders.xml` 238 mods, `loadFolders.xml` 50, `loadfolders.xml` 3). The root element name is not checked (284 use `loadFolders`, 7 use `LoadFolders`). Evidence: decompiled:Verse/ModLoadFolders.cs, decompiled:Verse/ModMetaData.cs (Init).

### 2.1 Syntax as the game parses it

- Each child of the root is a version block. Its name is lower-cased and a leading `v` stripped (`v1.6`, `V1.6`, `1.6` identical); duplicate blocks are merged into one list. The name `default` is a legacy fallback.
- Each child of a block is an entry whose text is a folder path relative to the mod root. The element name is irrelevant (corpus: all 5,600 are `li`). The text `/` or `\` means the mod root itself. Backslashes are converted to the OS separator only when the OS separator differs (so a Windows-only mod with `a\b` works on Windows and breaks on Linux; corpus: 2 entries in 1 mod). Text is not trimmed.
- Conditions are XML attributes on the entry: `IfModActive` (any of), `IfModActiveAll` (all of), `IfModNotActive` (none of); values are comma separated, each part trimmed, matched ignoring case and ignoring the `_steam` postfix. Conditions combine with AND. Any other attribute is ignored, so the folder loads unconditionally: corpus shows 9 entries with a non-existent `IfModActiveAny`. Usage counts: `IfModActive` 3,832, `IfModNotActive` 75, `IfModActiveAll` 10.
- Entry issues in the corpus: 671 root entries `/`, 18 empty paths, 11 trailing slashes, 2 backslash paths; 6 mods reference a first path component that does not exist; 0 default blocks; 195 distinct structural signatures across 291 files. corpus:load_folders.

### 2.2 Which block is chosen (InitLoadFolders)

Evidence: decompiled:Verse/ModContentPack.cs (InitLoadFolders), decompiled:Verse/LoadFolder.cs (ShouldLoad).

1. If the file defines at least one block, look for a block named exactly like the running version with build (`1.6.4871`). This almost never matches real mods, which use `1.6`.
2. Otherwise take, among defined keys that contain a dot and parse as a version less than or equal to the running version, the greatest key by plain string ordering (descending string sort, which is wrong for `1.10` vs `1.9`: unverified in practice because no such key exists yet). A `1.6` block is therefore found here, and a `1.5`-only mod falls back to its `1.5` block on a 1.6 game.
3. Otherwise use the `default` block if present.
4. If a block was selected but every entry fails its conditions, the mod loads nothing (no fallback to root). If no block was selected, continue with the implicit rules.

The selected entries are visited from last to first and entries whose conditions fail are skipped. The result is a descending-priority list of folders. Priority matters because Defs and Patches files are keyed by their path relative to the load folder and the first file with a given relative path wins; a same-named file in a lower-priority folder is shadowed (corpus: 86 overshadowed XML files in the active set).

### 2.3 Fallback when LoadFolders.xml is absent or selects nothing

1. If a directory named after the running major.minor (`1.6`) exists in the mod root, it is the version folder.
2. Otherwise every immediate subdirectory whose name parses as `A.B` (first two dot-separated parts are integers) is a candidate. The game picks the greatest candidate not above the running version; if every candidate is newer, it picks the smallest newer one (a mod with only `1.7` loads `1.7` on 1.6). The path used is rebuilt from the parsed version, so a directory named `1.3.3311` maps to a non-existent `1.3` (derived from the code, 1 such folder in the corpus, not run in the game).
3. Then `Common` (exact name `Common`) if present, then the mod root. Order of priority: version folder, Common, root.

Names that do not parse are not version folders: `v1.6` (13 mods, used only inside LoadFolders blocks where `v` is stripped), `1.6NotOdyssey` (5), `1.5 Content` (1), `1.5_And_1.6_Shared` (1).

Real structural variants (corpus:load_folders.structural_variants_top, 291 mods with a LoadFolders file):

| Signature (1.6 block unless stated) | Example mods |
|---|---|
| root plus one or two conditional folders, e.g. `/`, `<dir>` with `IfModActive` | 12 mods (example workshop 2949339248) |
| `/` plus the version directory per version (1.4, 1.5, 1.6) | 11 mods |
| only `1.6` block `[/, <ver>]` | 9 mods |
| blocks for every version 1.1 to 1.6 each listing `/` and a subdirectory | 7 mods |
| `<ver>` and `Common` only (no root) | 5 mods |
| `1.6` block `/` while older blocks list folders | 2 mods |

Effective source for the 610 active mods in the owner's list: LoadFolders block `1.6` 221, lower block 11, implicit version directory 256, implicit nearest-lower directory 10, root only 112. corpus:active_set_budget.effective_load_folder_source.

## 3. Folder anatomy and what the game reads

| Path in a load folder | Read by the game as | Corpus (mods with it at top level) |
|---|---|---|
| `About/About.xml` | metadata, section 1 (always from the mod root, never from a version folder) | 756 |
| `About/Preview.png` | Workshop preview and in-game mod list image, exact name `Preview.png` | 570 exact case; 186 mods have another spelling or format (`preview.jpg`, `Preview.dds`, ...) |
| `About/ModIcon.png` | list icon unless `modIconPath` is set | 178 exact, 15 other spellings |
| `About/PublishedFileId.txt` | Workshop id, 1.4 | 706 |
| `About/Manifest.xml` | nothing: no reference found anywhere in the decompiled code (third-party use unverified) | 121 |
| other `About/*` (Changelog.txt 49, ModSync.xml 44, Version.xml 7, Credits...) | not read | many |
| `Defs/**/*.xml` | merged Defs, section 5. Only `*.xml`; names starting with `.` skipped; recursive | 168 top-level `Defs` dirs |
| `Patches/**/*.xml` | patch operations, root must be `Patch` | 150 |
| `Assemblies/*.dll` | managed assemblies (extension compared lower-case), loaded from low to high priority folder, files sorted by name | 77 |
| `Textures/**`, `Sounds/**`, `AssetBundles/**` | content loaded by path relative to the folder | 486, 94, 5 |
| `Languages/<Lang>/{Keyed,DefInjected,Strings}/**` | translation XML (also supported as a `.tar` archive in the game; corpus 0) | 306 |

Evidence: decompiled:Verse/ModContentPack.cs, decompiled:Verse/DirectXmlLoader.cs (XmlAssetsInModFolder), decompiled:Verse/ModAssemblyHandler.cs, decompiled:Verse/ModMetaData.cs (PreviewImagePath, ModIconImagePath), corpus:folder_anatomy.

Details that matter for a scanner:

- Directory names are matched with the OS rules; on Linux `defs` or `Textures ` with a different case is invisible to the game. Corpus: 0 case variants of area directories at top level; `Common` always exact (84 mods); `.xml` extension case variants: 1 file named `.XML` (the game's `*.xml` filter is OS dependent, so on Windows it loads and on Linux the filter is case-sensitive, unverified).
- Nested mods exist inside real mods (6 mods contain an `About/About.xml` below the root, such as `1.3/Ideology/About/About.xml`); the game never treats nested folders as mods unless a LoadFolders entry points at them.
- Version directory names found: `1.0` to `1.6` plus `0.19` and variants; 542 mods have version-like directories. `.git` directories exist in 134 of 764 mods (24,069 files in total) and must be skipped by the scanner.
- Defs and Patches order inside one mod follows the file enumeration of the OS (the directory listing is not sorted in the code I read), so order between files in one mod is unspecified; order between mods is the load order (section 5).

## 4. Mod enumeration, ModsConfig.xml and config directories

### 4.1 Enumeration

`ModLister.RebuildModList` builds the list from exactly three sources, in this order: every subdirectory of the install `Data` folder (official: Core and DLC, a DLC with a Steam app id is skipped when Steam is up and the DLC is not owned), every subdirectory of the install `Mods` folder, and Steam-subscribed workshop items (only when Steam is initialised; items with a `.rsc` file are scenarios, not mods). Nothing else is scanned. Every subdirectory counts, with or without `About.xml` (the 8 owner folders without About.xml would appear as nameless defaults if they sat in Mods). Evidence: decompiled:Verse/ModLister.cs, decompiled:Verse.Steam/WorkshopItem.cs (MakeFrom), decompiled:Verse.Steam/WorkshopItems.cs.

The install directories come from the executable location (`GetOrCreateModsFolder` uses the parent of the Unity data folder), not from settings. The only command-line argument that touches paths is `savedatafolder`, which moves the user data directory (Config, Saves, logs excluded) but not Mods or Data. Evidence: decompiled:Verse/GenFilePaths.cs.

Duplicate handling: first added wins; workshop and non-workshop copies of the same id coexist through the `_steam` postfix (section 1.3); other duplicates are dropped with an error. If no mod is active, Core is activated. Install Mods in this machine: 47 folders, of which 33 are active.

### 4.2 ModsConfig.xml

Path: `<user data dir>/Config/ModsConfig.xml`. Real file head (no account data in this file):

    <ModsConfigData>
      <version>1.6.4633 rev1270</version>
      <activeMods> <li>zetrith.prepatcher</li> ... </activeMods>
      <knownExpansions> ... </knownExpansions>

| Element | Meaning |
|---|---|
| `version` (alias `buildNumber`) | version string at last save, with revision |
| `activeMods` | ordered list of effective packageIds (lower case, may carry `_steam`); load order is list order, Core and DLC are ordinary entries |
| `knownExpansions` | official expansion ids the player has already been shown |

Real file: 615 `li` lines = 610 active mods + 5 known expansions; all 610 matched an installed mod (571 workshop, 33 install Mods, 6 official, 0 owner folders). corpus:active_set_budget.

When the game reads and rewrites it (decompiled:Verse/ModsConfig.cs):

1. If the stored `version` has a different major or minor than the running game, the whole file is discarded: only Core and the compatible official DLC remain, then `TrySortMods` and a save (the user's whole list is lost on every major.minor game update). A pure numeric legacy version up to 2009 also resets. Build differences (4633 vs 4871 here) do not reset.
2. If an entry matches no installed id but matches a folder name, it is rewritten to the mod's id; an entry ending in `_steam` falls back to the plain id.
3. A duplicate active instance of one id is deactivated with a warning.
4. A newly detected official expansion is activated, sorted using `forceLoadBefore/After`, and marked known, then saved.
5. Missing file triggers a reset and save; a user change in the in-game Mods screen saves; the version is stamped with the full string including `rev`.

### 4.3 DeactivateNotInstalledMods: the silent drop

During `RebuildModList` (every start and every list rebuild) each `activeMods` entry that matches no mod in the list is removed from memory with only a log line ("Deactivating <id>", visible in Player.log with verbose logging). The file on disk is rewritten at the next `Save`, so the loss is not immediate but is normally permanent. Consequences for RimStudio: a mod folder that is a broken symlink, an unmounted external drive, a workshop item while Steam is not initialised, or an owner folder that is not inside `Mods` simply vanishes from the user's list the next time the game saves. The game only sees the three enumeration roots, so RimStudio's custom mod folders (R4) are invisible to the game unless RimStudio links or copies them into the install `Mods` folder; mod folders in Mods are matched by their packageId only. Writing a ModsConfig.xml that contains ids from a custom folder is not enough. Evidence: decompiled:Verse/ModsConfig.cs (DeactivateNotInstalledMods), decompiled:Verse/ModLister.cs (RebuildModList), decompiled:Verse/LoadedModManager.cs (InitializeMods deactivates an active mod whose directory vanished at load time with a warning).

### 4.4 User data directory per OS

| OS | Directory | Evidence |
|---|---|---|
| Linux | `~/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/` (honours `$XDG_CONFIG_HOME`) | verified live: Config/, Saves/, Player.log present; Unity docs https://docs.unity3d.com/ScriptReference/Application-persistentDataPath.html (accessed 2026-10-04) |
| Windows | `%USERPROFILE%\AppData\LocalLow\Ludeon Studios\RimWorld by Ludeon Studios\` | Unity docs above; RimWorld wiki Development_mode page (mentions LocalLow, Ludeon Studios, Config) |
| macOS | the game uses the parent of Unity's persistentDataPath plus `RimWorld`, so `~/Library/Application Support/RimWorld/` (unverified on a real Mac) | decompiled:Verse/GenFilePaths.cs (SaveDataFolderPath) |

Subfolders created under it: `Config` (ModsConfig.xml, Prefs.xml, KeyPrefs.xml, Knowledge.xml, LastPlayedVersion.txt, `Mod_<id>_<Name>.xml` per-mod settings files, `DevModeDisabled` marker), `Saves`, `ModLists`, `Ideos`, `Xenotypes`, `Screenshots`, `DevOutput` and others. Evidence: decompiled:Verse/GenFilePaths.cs, live listing. Per-mod settings and extra folders written by mods (HugsLib, RealRuins) share this directory.

## 5. Loading pipeline and patch operations

Evidence: decompiled:Verse/LoadedModManager.cs (LoadAllActiveMods, CombineIntoUnifiedXML, ApplyPatches, ParseAndProcessXML), decompiled:Verse/DefDatabase.cs, decompiled:Verse/XmlInheritance.cs.

```mermaid
flowchart LR
  A[Active mods in load order] --> B[Resolve load folders]
  B --> C[Read Defs files in parallel]
  C --> D[Merge into one Defs document]
  D --> E[Read Patches, apply all in mod order on raw XML]
  E --> F[Register Name and ParentName nodes]
  F --> G[Resolve inheritance]
  G --> H[MayRequire filter, instantiate defs, DefDatabase.Add]
  H --> I[Resolve cross references]
```

1. Mods are initialised in `activeMods` order, each with its resolved load folders (section 2). Content, assemblies and mod classes load before XML.
2. Defs: all `Defs/**.xml` of all mods are parsed (two helper threads plus the caller), comments and whitespace ignored, then every child of every file's root is imported into one `Defs` document in mod order. A root element not named `Defs` is an error but its children are still imported. A file that fails to parse is skipped with a warning and costs only that file.
3. Patches are loaded per mod from `Patches/**.xml`: root must be `Patch`, children must be `Operation`, each becomes an object whose `Class` attribute names the type. All operations of all mods run in mod order on the merged document before inheritance exists, so patches see raw XML of every def of every mod (including mods later in the order) and `ParentName` has not been resolved yet. An exception in one operation is caught and logged; the next operation continues.
4. After all patches, `Complete` runs per operation: an operation that returned success at least once is silent, one that never succeeded logs "Patch operation X failed" with its file (this is the dominant error line in real logs, 98,391-line Player.log sample).
5. Inheritance: nodes with `Name` or `ParentName` are registered (a node whose `MayRequire` is not met is skipped; `MayRequireAnyOf` is not consulted here); a repeated `Name` inside the same mod is an error and the second is dropped, the same name in different mods is allowed and resolution prefers a parent from the same mod or earlier ones. Then `Resolve` merges parent into child.
6. Def instantiation: a def node with unmet `MayRequire` (all of, comma separated) or unmet `MayRequireAnyOf` is skipped. The same attributes are honoured on list `li` items and fields (decompiled:Verse/DirectXmlToObjectNew.cs). Values are lower-cased and compared ignoring case and the `_steam` postfix. Corpus: 3,258 def nodes use `MayRequire`, 170 use `MayRequireAnyOf`, 600 patch-operation list items use `MayRequire`.
7. `DefDatabase.Add`: a def whose `defName` already exists in that def type is an error ("Adding duplicate ... name") and the new def is renamed by appending a random number from 0 to 1000, so a duplicate is not an override and its final name is nondeterministic. Overriding is done with patches, not with a same-named def.
8. Cross references (`<defName>` text in fields) are resolved last and only produce errors if missing.

### 5.1 Vanilla PatchOperation classes (all in namespace Verse)

Every operation has an optional `<success>` element with values `Normal` (default), `Invert`, `Always`, `Never`, applied after the worker runs. `xpath` is evaluated with .NET XPath 1.0 against the whole merged document (`Defs/ThingDef[defName="X"]`). `value` content is a container whose children are copied in.

| Class | Parameters | Success (before the success override) |
|---|---|---|
| `PatchOperationAdd` | `xpath`, `value`, `order` (Append default, Prepend) | true if at least one node matched; appends or prepends the children of `value` into every matched node |
| `PatchOperationInsert` | `xpath`, `value`, `order` (Prepend default, Append) | true if matched; inserts as siblings before or after each match |
| `PatchOperationReplace` | `xpath`, `value` | true if matched; replaces each matched node by the children of `value` |
| `PatchOperationRemove` | `xpath` | true if matched; removes matched nodes |
| `PatchOperationAddModExtension` | `xpath`, `value` | true if matched; creates `modExtensions` when absent, then appends |
| `PatchOperationSetName` | `xpath`, `name` | true if matched; replaces the element by one with the new name keeping the inner XML only (attributes are lost) |
| `PatchOperationAttributeAdd` | `xpath`, `attribute`, `value` | true only if some node lacked the attribute (existing attributes untouched) |
| `PatchOperationAttributeSet` | `xpath`, `attribute`, `value` | true if matched; sets or creates |
| `PatchOperationAttributeRemove` | `xpath`, `attribute` | true only if some node had the attribute |
| `PatchOperationTest` | `xpath` | true if the path selects at least one node |
| `PatchOperationConditional` | `xpath`, `match`, `nomatch` | evaluates the first selected node: runs `match` if found or `nomatch` otherwise and returns that result; absent-branch rules explained below |
| `PatchOperationSequence` | `operations` (list) | runs in order and stops at the first failure, returning false (earlier changes are kept, there is no rollback) |
| `PatchOperationFindMod` | `mods` (list of mod display names), `match`, `nomatch` | true unless the chosen branch fails; matches the mod `name`, not the packageId, with exact case-sensitive text |

`PatchOperationConditional` in full: it tests whether the xpath selects a node. If yes and `match` exists, the result is the result of `match`; if no and `nomatch` exists, the result of `nomatch`. When the needed branch is absent, the result is true if `match` exists (even when the xpath did not match), and otherwise true only if `nomatch` exists. So with neither branch it is false, and a missing xpath with only `match` is true. Evidence: decompiled:Verse/PatchOperationConditional.cs.

Corpus usage (11,087 patch files, 129 distinct operation classes): `PatchOperationReplace` 50,434; `PatchOperationAdd` 50,340; `PatchOperationConditional` 25,185; `PatchOperationRemove` 11,513; `PatchOperationAddModExtension` 7,651; `PatchOperationSequence` 5,744; `PatchOperationFindMod` 4,679; `PatchOperationAttributeSet` 414; `PatchOperationInsert` 282; `PatchOperationTest` 331. Non-vanilla classes are common: `CombatExtended.PatchOperationMakeGunCECompatible` 6,554, `XmlExtensions.*` (OptionalPatch 764, SafeAdd 620 and others), `NQualityOfLife.XML.*`, `ModSettingsFramework.PatchOperationModOption`. An external tool cannot evaluate those; it must preserve them as opaque nodes. corpus:xml_hygiene.patches.

## 6. Saves, logs, Prefs and version strings

- Save header: a save file's `<savegame><meta>` holds `gameVersion` (for example `1.6.4633 rev1273`), `modIds` (list of packageIds in load order), `modNames`, and `modSteamIds` (0 for non-workshop mods in a real save). The first bytes of a real save are a UTF-8 BOM. On load the game compares the lists with the active set and warns on differences. Evidence: decompiled:Verse/ScribeMetaHeaderUtility.cs, a real autosave (head only).
- Player.log: Linux `~/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Player.log` (previous run `Player-prev.log`, verified live: 12 MB and 20 MB); Windows `%USERPROFILE%\AppData\LocalLow\Ludeon Studios\RimWorld by Ludeon Studios\Player.log`; macOS `~/Library/Logs/Ludeon Studios/RimWorld by Ludeon Studios/Player.log` (pattern from https://docs.unity3d.com/Manual/LogFiles.html, accessed 2026-10-04, unverified for the product folder).
- Error line format in the real log: there is no severity prefix; a message is a plain line, followed by `[Ref xxxxxxxx]`-style ids and indented C# stack lines (`  at Verse.X (...) [0x0005a] in <hash>:0`). Examples seen: `XML error: Could not register node named "X" because this name is already used.`, `Exception loading def from file <name>.xml: System.ArgumentException: Could not find type named ...`, `[<ModName>] Patch operation Verse.PatchOperationReplace(<xpath>) failed` (xpath may span several lines) and `Mod <Name> dependency (<id>) needs to have <downloadUrl> and/or <steamWorkshopUrl> specified.` Classification therefore needs message patterns, not a level token. Mods inject their own lines (for example a profiler line with a duration). Evidence: live Player.log (grep only, 98,391 lines, line count taken from the file).
- Prefs.xml (`Config/Prefs.xml`): booleans are `True` or `False`; keys include `devMode`, `logVerbose`, `testMapSizes` and `resetModsConfigOnCrash` (default True). The owner's file has devMode True, logVerbose False. A file `Config/DevModeDisabled` blocks dev mode permanently. Evidence: decompiled:Verse/PrefsData.cs, live Prefs.xml.
- Version.txt (install root) holds `1.6.4871 rev598`. The game derives its own version from the assembly version: build is the assembly build minus 4805, revision is the assembly revision times 2 divided by 60 (decompiled:RimWorld/VersionControl.cs), so Version.txt and the game agree only if shipped consistently (the file is the practical source for an external tool).
- `Config/LastPlayedVersion.txt` holds the version without revision (`1.6.4633` on this machine, older than the installed 4871, since the game has been updated but not started since). It is rewritten at start only when it differs from the running version. Evidence: decompiled:RimWorld/LastPlayedVersion.cs.
- Parsing rules: `VersionFromString` accepts one to three dot-separated non-negative integers and throws on text or four parts, so `1.6.4871 rev598` must be split on the space first (`VersionStringWithoutRev`); mod-facing parsing (`TryParseVersionString`) keeps only major and minor; version comparison for folders uses the three-part value with build; compatibility is major.minor equality; a ModsConfig reset triggers on major.minor difference only. Evidence: decompiled:RimWorld/VersionControl.cs.

## 7. Corpus statistics

Scope: 764 mod folders (690 workshop, 47 install Mods, 27 owner), plus 6 official folders reported separately. Evidence for every number: corpus:all_requested_roots unless stated. The scanner emulates the game semantics above and uses libxml2 as a strict XML reference, so it flags files the game's reader might also reject.

### 7.1 About.xml

| Measure | Value |
|---|---|
| mods with About/About.xml | 756 of 764 (8 owner folders have none; 6 mods carry nested About.xml deeper) |
| `About/about.xml` lower-case path | 5 mods |
| About.xml size bytes | p50 1,148, p90 2,996, p95 4,178, p99 6,848, max 37,144, total 1.21 MB |
| encoding | UTF-8 BOM 239 (31.6 percent), no BOM 517; declared `utf-8` 726, no declaration 30; UTF-16/32 BOM 0; invalid UTF-8 0 |
| line endings | CRLF 557, LF 151, mixed 48 |
| malformed About.xml | 0 |
| `modDependencies` per mod after validation | p50 1, p90 2, p95 3, max 4, mean 1.0; 535 mods have at least one |
| `loadAfter` per mod | p50 1, p90 7, p95 10, p99 25, max 979 (an aggregator mod); total 3,511 references |
| `loadBefore` per mod | p95 1, max 15; `incompatibleWith` p95 1, max 22; `forceLoadAfter` max 7 |
| authors list (`authors`) | 20 mods; ByVersion fields 27 mods; `alternativePackageIds` 0 mods |

### 7.2 Folder structure

| Measure | Value |
|---|---|
| mods with a LoadFolders file | 291 (38 percent) |
| mods with `Common` | 84 |
| mods with version-like directories | 542; with Defs 168; Patches 150; Assemblies 77; Languages 306; Textures 486 |
| top-level directory name noise | `.git` 130, `.github` 16, `.vscode` 25, `.vs` 8, `Source` 84, `Mods` 12 |
| PublishedFileId.txt | 706 present, all digits, 4 with a trailing newline; workshop folder id equals file value in 689 of 690, 1 workshop mod has no file, 0 mismatches |
| ACF cross check | 692 installed ids in `appworkshop_294100.acf`, 690 folders; 2 ids without folder (`2971930101`, `3309790710`); only ids and sizes were read, no account data |
| symlinks | 0 (roots or inside mods) |
| Windows-invalid names | 0 in mod folder or entry names; 3 mods have case-only collisions inside a directory (`Log.DDS` and `Log.dds`), which break on Windows and macOS default file systems; 659 hidden files; 45 mods use non-ASCII names (418 entries) |
| longest relative path inside a mod | 160 characters (`ModPatches/<long name>/Patches/<long name>/Weapons.xml`); with the default Steam Windows prefix the longest full path stays below the 260 limit (0 entries over) |

### 7.3 XML hygiene

| Area | Files | With UTF-8 BOM | Not parseable | Notes |
|---|---|---|---|---|
| Defs | 30,499 | 11,667 (38.3 percent) | 2 | 8 declare `utf-16` yet are UTF-8 bytes; 1 invalid UTF-8 file |
| Patches | 11,087 | 531 | 6 (empty or no root) | |
| Languages | 21,015 | 15,929 | 71 (tag mismatch 62, XML declaration not first 6, invalid element name 2, malformed tag 1) | 89 hidden-name files ignored by the game |

Failure examples (all from real mods): a `Defs` file with a malformed tag at line 83 (`canines-animations`, game: warning, file skipped); an unescaped `&` in a research def (`Ratchet & Clank` owner mod, entity-reference error, file skipped); mismatched tags in translation files such as `Arthropleura.lifeStages.0.labelMale` closed by `...label` (workshop 1055485938, only that file is lost); patch files with nothing but comments (workshop 2244594116).

Local Mono experiment (docs/research/data scratch, not committed; the game uses Unity's Mono so treat as indicative): a raw control character U+0001 fails, `&#1;` is accepted, an encoding declaration of `utf-16` or an unknown encoding inside a string is ignored, a leading U+FEFF character or a newline before `<?xml` fails, a leading space before a root without declaration is accepted, unescaped `&`, `&nbsp;`, duplicate attributes, undeclared namespace prefixes and multiple roots all fail. The game decodes Defs files as UTF-8 after removing only a UTF-8 BOM, so a real UTF-16 file is garbage and fails (0 in the corpus).

Defs shape: 145,631 def nodes in 764 mods per-mod p50 11, p95 760, max 11,249; 541 distinct def element names; `ThingDef` 38,342 nodes; 6,970 nodes with `Name`, 73,349 with `ParentName`, 6,103 abstract.

### 7.4 Sizes and cold-scan budget

| Per mod | p50 | p95 | p99 | max |
|---|---|---|---|---|
| files | 78 | 1,840 | 5,645 | 22,030 |
| directories | 28 | 266 | 807 | 4,366 |
| bytes | 3.3 MB | 176 MB | 825 MB | 2.8 GB |
| XML files | 11 | 280 | 1,058 | 9,074 |
| XML bytes | 43.6 KB | 2.2 MB | 11.3 MB | 46.7 MB |

Totals: 329,539 files, 60,384 directories, 34.3 GB apparent (31.2 GB workshop; the ACF declares only 6.8 GB for the same items, unexplained: open question), 466.8 MB XML in 64,840 files, of which Defs 294.6 MB, Patches 62.0 MB, Languages 42.3 MB.

Scan levels (all roots together, corpus:scan_budget):

| Level | Work | Files opened | Bytes read |
|---|---|---|---|
| 0 metadata | list each mod root, read About.xml, LoadFolders.xml, PublishedFileId.txt, stat Preview and icon | 1,753 | 1.6 MB (8,858 directory entries, 1,520 readdir calls) |
| 1 def and patch index | level 0 plus every Defs and Patches XML | 43,339 | 358 MB |
| 2 plus Languages | adds translation XML | 64,354 | 400 MB |
| 3 full walk | stat of every entry, no extra reads | 329,539 stats | 0 content (24,069 extra files if `.git` is not skipped) |

For the active set (610 mods): effective Defs files 14,829 (147 MB), patch files 2,974 (26 MB), 579 DLLs (123 MB), 1,966 sound files (330 MB), 145,353 textures (16.6 GB); per active mod Defs plus Patches files p50 3, p95 132, max 2,200. Textures dominate bytes, so a scanner must never read textures except on demand.

Budget proposal (targets derived from the numbers, not measurements): level 0 must be responsive in under 300 ms warm because it opens fewer than 2,000 small files; level 1 reads about 360 MB, so it needs a persistent cache keyed by (path, size, mtime), parallel reads and a streaming XML reader, with a first cold index under 15 s on an SSD and well under 2 s on re-open when 99 percent of files are unchanged; never walk textures; skip `.git`, `.vs` and `.vscode`. The 4.6 s measured wall time for level 0 plus statistics includes full-walk statistics on a warm cache.

## Parser requirements checklist

1. Resolve `About/About.xml` with exact `About` directory and case-insensitive file name; treat a missing or unparsable file as an all-default mod with a recorded diagnostic, never as an exclusion.
2. Strip UTF-8, UTF-16 and UTF-32 BOMs for About.xml and LoadFolders.xml; strip only the UTF-8 BOM for Defs and Patches; ignore the `encoding` declaration; reject leading whitespace before the declaration.
3. Match About tags case-sensitively against the field table in 1.1; keep unknown tags as diagnostics; drop string fields that contain comments or child elements exactly as the game does, and report it.
4. Parse supportedVersions per 1.2 (first two integer parts, keep the raw string, warn on extra parts or `v` prefix) and expose `compatible = any major.minor equals running`.
5. Normalise packageIds with ASCII lowercase plus trim for lookup, keep the original spelling for display, validate with the documented regex, and implement the `_steam` postfix rule and duplicate resolution in enumeration order.
6. Implement ByVersion replacement semantics (lower-case, strip `v`, first wins, replace not merge) before dependency validation, and drop invalid dependencies with the same four reasons.
7. Parse LoadFolders per 2.1 including condition attributes (unknown attributes are ignored), `/` and `\` as root, backslash conversion by OS, and the block selection algorithm 2.2 including "selected but all conditions fail loads nothing".
8. Implement the implicit fallback of 2.3 (exact major.minor directory, nearest lower, smallest newer, `Common`, root) and the first-file-wins shadowing by relative path.
9. Enumerate mods only from official Data, Mods and Steam items for the "what the game sees" view; keep custom folders as a separate source with an explicit link, copy or junction step before launch.
10. Treat any directory as a mod candidate, skip `.git`, `.vs`, `.vscode` inside mods, and read `PublishedFileId.txt` with a whitespace-tolerant unsigned parse.
11. Read ModsConfig.xml into an ordered id list plus known expansions and version; never write it without a timestamped backup, and warn when an active id is not resolvable (the game would drop it).
12. Strip the revision before parsing game versions; support one to three numeric parts; compare folders with three parts and compatibility with two.
13. Preserve unknown patch operation classes as opaque nodes; evaluate vanilla operations per 5.1 with golden tests derived from real mods; apply patches over the merged document in mod order before inheritance.
14. Honour `MayRequire` (all of) and `MayRequireAnyOf` with case-insensitive postfix-agnostic matching, including on list items; treat inheritance registration as `MayRequire` only.
15. Report duplicate defNames as diagnostics; never emulate the random rename.
16. Handle per-file XML failures locally: record file, line, category; continue with the remaining files; run on files up to 47 MB with a streaming reader.
17. Use a persistent index keyed by path, size and mtime; read textures never; handle 22,030 files in one mod and relative paths of 160 characters.
18. Detect case-only name collisions, non-ASCII names, Windows-invalid names and over-long paths as warnings (3, 45, 0 and 0 mods in this corpus).

## Implications for RimStudio

1. The mod model has three sources (official, install Mods, Steam) that mirror the game, plus user custom folders (R4) as a fourth source that the game cannot see: RimStudio must offer an explicit "make visible to the game" action (junction on Windows, symlink on Linux and macOS) and must show which mods are visible.
2. Before any launch or ModsConfig write, RimStudio must verify that every active id resolves to a folder reachable by the game; otherwise show a blocking warning that names the DeactivateNotInstalledMods consequence.
3. ModsConfig.xml writes: back up first, keep `version` equal to the installed `Version.txt` string, write ids lower-case with the `_steam` postfix where the workshop copy is meant, and keep `knownExpansions`; a mismatch in major.minor must never be created by RimStudio.
4. The mod scanner is two-phase: level 0 metadata for the whole library (fewer than 2,000 files, 1.6 MB) then lazy level 1 indexing with a persistent cache; budget numbers in 7.4 are acceptance targets for benchmarks.
5. Dependency, loadAfter and incompatibleWith edges must be resolved by case-insensitive postfix-agnostic id matching; a test must cover the 228 differently spelled references in the corpus.
6. The sort engine must treat `loadAfter` lists of up to 979 entries and cycles gracefully; `forceLoad*` is only meaningful for official expansions.
7. LoadFolders and version-folder resolution is a shared pure function used by the manager, the def explorer and the project workspace; its tests use the corpus signatures of 2.3 (the 195 distinct structural variants).
8. The patch engine and def merger must reproduce the order merge, then patch, then inherit, then instantiate, and expose per-operation success so the toolkit can show "this operation never succeeded" like the game's log.
9. Log import must classify lines by pattern (no severity token), group by mod via the `[ModName]` prefix, and parse the paths for Linux, Windows and macOS from 4.4 and section 6.
10. Diagnostics for authors: warn on bad packageId format, missing supportedVersions entry for 1.6, missing dependency URLs, `_steam`-suffixed or case-conflicting ids, mixed encodings, files with leading whitespace before `<?xml`, and case-only filename collisions.
11. The Workshop update tool must read and write `About/PublishedFileId.txt` exactly (digits only, no newline needed) and use the folder id when both exist and differ.

## Open questions

1. How is the game's `StableStringHash` computed, so that generated packageIds for mods without one can be reproduced exactly? (0 corpus mods need it.)
2. Cold-cache timings of the scan levels on spinning disks and network drives were not measured; only warm wall time (4.6 s) exists.
3. The 31.2 GB apparent workshop size versus 6.8 GB in the ACF: compressed depots, hard links, or stale ACF sizes?
4. Does the OS-level `*.xml` filter match `.XML` on Linux in the game's Mono runtime, and in what order does it list files (affects patch order inside a mod)? Decompiled code shows no sort.
5. Exact macOS paths for the data folder and for `Mods` inside the app bundle were taken from code and the wiki, not from a Mac.
6. When exactly does the game call `ModsConfig.Save` after DeactivateNotInstalledMods (quit, Mods screen, never)? The removal is in memory immediately; persistence timing was not traced.
7. Does the Player.log have a distinguishable warning format (the sampled log has no level tokens)?
8. The `1.3.3311` style directory (path rebuilt as `1.3`) is a code-derived edge case, not exercised in the game.
