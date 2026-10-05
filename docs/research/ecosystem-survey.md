# RimWorld modding tool ecosystem survey

Scope: a survey of the real tools, documentation and developer switches that surround RimWorld modding in October 2026, so that RimStudio (mod manager plus modding toolkit) knows what already exists, what to link to or encode, and where the gaps are. Part A maps the official and community documentation, Part B verifies the game's developer tools in the decompiled code, Part C catalogues tools by category with a feature matrix. The toolkit design that builds on this note is in `docs/research/modding-toolkit-scope.md`.

Status: research note | Last verified: 2026-10-04

Method and evidence conventions. Repository metadata (stars, licence, last commit, latest release) was read from public GitHub HTML and Atom feeds on 2026-10-04 (script `ghinfo.py` in the scratch folder, generic user agent, no API token). Pages marked "opened" were fetched and read; "search only" means a web search result named the tool but I did not open an authoritative page; those rows are flagged. Evidence pointers: `decompiled:<path>` is the ILSpy output of the game assembly (build 1.6.4871 rev598; Ludeon code, described in my own words, never copied). RimSort, RimCrow and Combat Extended (CE) are described as concepts only. Wiki pages were fetched as wikitext on 2026-10-04 and are secondary to the decompiled code.

## 1. Summary

| Finding | Evidence |
| --- | --- |
| There is no single maintained, cross-platform tool that combines a mod manager with a def-aware modding toolkit. The nearest are RimCrow (manager first, creator tools only planned) and a small assistant-feature workbench for def indexing. | Section 4, matrix in section 5 |
| Editor support exists but is split by editor: one VS Code language server (about 158k installs, last published 2025-10-15) and one Rider plugin (active, release 2026-05-12). Both derive their knowledge from the game's C# classes at run time. Neither understands patches as a first-class view. | Section 4.3 |
| The only widely used texture tool, todds (DDS encoder), was archived on 2026-03-31 and points to a successor that is not yet at feature parity. A native Rust DDS path is an open opportunity. | Section 4.6 |
| Translation tools are mostly one-person projects; the oldest standard (RimTrans) has been idle since 2020, two new ones were released in September 2026. | Section 4.5 |
| No maintained standalone weapon or apparel balance calculator was found. The community convention is CE's own spreadsheet set (four sheets: guns, projectiles and ammo, melee, races). | Section 4.9 |
| The game itself exposes a handful of dev switches RimStudio can drive safely: `devMode` in Prefs.xml, the `-quicktest` and `savedatafolder=` arguments, a log window and about 47 boolean debug settings. | Section 3 |
| A reflection-derived schema is feasible (see toolkit note section 6): it validated 13,808 vanilla defs with 10 unresolved element occurrences out of 168,009. | `docs/research/data/schema-probe/` |

## 2. Documentation map: what the toolkit should link to or encode

The four starting pages were fetched as wikitext (cached under the scratch folder `ecosystem-and-toolkit/wiki/`). The table lists the topics a modder needs, the best source, and what RimStudio should do with it. Wiki text is not copied; URLs are given for linking.

| Topic | Primary source (verified) | Secondary source | What RimStudio does with it |
| --- | --- | --- | --- |
| Def types and fields | Game assembly classes (field names equal XML element names) | https://rimworldwiki.com/wiki/Modding_Tutorials/Defs , https://rimworldwiki.com/wiki/Modding_Tutorials/ThingDef | Derive a schema from the assembly; link each node to the wiki page when one exists |
| XML inheritance (`Name`, `ParentName`, `Abstract`) | decompiled:Verse/XmlInheritance.cs (see `docs/research/def-engine-semantics.md`) | https://rimworldwiki.com/wiki/Modding_Tutorials/XML_Defs | Def Explorer inheritance tree and resolved view |
| PatchOperations | decompiled:Verse/PatchOperation*.cs (see `docs/research/xpath-patch-coverage.md`) | https://rimworldwiki.com/wiki/Modding_Tutorials/PatchOperations | Patch tester and "who patched this" |
| `MayRequire` and conditional content | decompiled:Verse/DirectXmlToObject.cs | https://rimworldwiki.com/wiki/Modding_Tutorials/MayRequire | Validator must understand the attribute |
| About.xml, folder structure, LoadFolders | `docs/research/rimworld-mod-format-and-corpus.md` | https://rimworldwiki.com/wiki/Modding_Tutorials/Mod_Folder_Structure | About editor and linter, version-folder manager |
| Compatibility | wiki page `Modding_Tutorials/Compatibility` | Same plus CE compatibility guide (section 4.8) | Conflict detector rules |
| Publishing and distribution | `docs/research/workshop-publishing-research.md` | https://rimworldwiki.com/wiki/Modding_Tutorials/Distribution | Workshop publisher |
| Localization | wiki page `Modding_Tutorials/Localization` | decompiled:Verse/LanguageWorker* (not studied here) | Translation helper |
| Textures and sounds | wiki pages `Modding_Tutorials/Textures`, `Modding_Tutorials/Sounds` | | Texture validator |
| Harmony and C# | https://harmony.pardeike.net (library docs; page not opened) , wiki pages `Modding_Tutorials/Harmony`, `Setting_up_a_solution`, `Decompiling_source_code` | Owner's template (section 4.7) | Out of scope to edit C#; scaffold links and dev launcher only |
| 1.6 update notes | wiki page `Modding_Tutorials/RimWorld_1.6_Mod_Updates` | | Version-bump checklist feature |
| Testing mods and troubleshooting | wiki pages `Testing_mods`, `Troubleshooting`, `Troubleshooting/Finding_Exceptions`, `ConfigErrors` | | Log analyser patterns |
| Debug actions and TweakValue | wiki pages `DebugActions`, `TweakValue` | Section 3 below | Dev launcher cheat sheet |
| Recommended software | wiki page `Modding_Tutorials/Recommended_software` | Section 4 | Source for the category list in this note |

The wiki index page `Modding_Tutorials` links to further topic pages the toolkit may later reference: `XML file structure`, `Linking XML and C#`, `DefModExtension`, `Custom Comp Classes`, `ExposeData`, `GameComponent`, `Quests`, `Rituals`, `Xenotype template`, `Weapons Guns`, `BigAssListOfUsefulClasses`. These are unverified for 1.6 accuracy; the wiki is community written and the date of the last revision per page is in `wiki/revisions.json` in the scratch folder.

## 3. Developer tools and flags RimStudio could integrate

Each row was checked in decompiled code and, where possible, against the real install and the owner's config on this machine.

| Capability | Verified behaviour | Evidence | RimStudio use |
| --- | --- | --- | --- |
| Dev mode toggle | Stored as `devMode` in Prefs.xml (config folder). The property returns true when preferences are not yet loaded. Turning it off also clears `logVerbose`, re-enables `resetModsConfigOnCrash`, turns god mode off and closes the dev palette. A zero-byte file named `DevModeDisabled` in the config folder permanently disables dev mode. Real file on this machine contains `devMode True`, `logVerbose False`, `resetModsConfigOnCrash True`, `runInBackground True`. | decompiled:Verse/Prefs.cs (DevMode), decompiled:Verse/PrefsData.cs, decompiled:Verse/GenFilePaths.cs (PrefsFilePath, DevModePermanentlyDisabledFilePath), decompiled:RimWorld/DevModePermanentlyDisabledUtility.cs, `/home/pawbeans/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/Prefs.xml` | Dev launcher toggles `devMode` only while the game is not running; refuse if `DevModeDisabled` exists and explain why |
| Reset-on-crash | `resetModsConfigOnCrash` (default true) makes the game reset the active mod list after a crash. Dev mode off forces it back to true. | decompiled:Verse/PrefsData.cs (field), decompiled:Verse/Prefs.cs | Warn modders that a crash may erase a hand-built test list; snapshot ModsConfig.xml before each dev launch |
| Quick test | `-quicktest` (matching is case-insensitive and tolerates a leading dash or none) loads the Play scene directly while in the entry scene, once. Wiki says a crashlanded map with default options is generated. | decompiled:Verse/QuickStarter.cs, decompiled:Verse/GenCommandLine.cs (CommandLineArgPassed), https://rimworldwiki.com/wiki/Modding_Tutorials/Testing_mods | Dev launcher "launch straight into a test map" |
| Isolated save data | `savedatafolder=<path>` is parsed as `key=value`; the argument is split on `=` and accepted only when exactly two parts result, so a path containing `=` is silently ignored. | decompiled:Verse/GenCommandLine.cs (TryGetCommandLineArg), decompiled:Verse/GenFilePaths.cs (SaveDataFolderCommand) | Dev launcher "clean profile" mode; reject paths containing `=` |
| Borderless window | `-popupwindow` is read straight from the process arguments. | decompiled:RimWorld/ResolutionUtility.cs | Optional launch flag |
| Legacy XML deserializer | `-legacy-xml-deserializer` switches the def loader to the older path. | decompiled:Verse/LoadedModManager.cs | Diagnostic toggle for obscure load differences (advanced) |
| Missing-attribute report | In dev mode the game logs classes that probably lack the static-constructor attribute during init. | decompiled:Verse/Root.cs | Mentioned in log analyser rules |
| Log window | `Log.TryOpenLogWindow` opens the in-game log editor window; errors can auto-open it. The same messages go to Unity's `Player.log` (with `Player-prev.log` for the previous run) beside the config folder. Both exist on this machine. | decompiled:Verse/Log.cs, decompiled:LudeonTK/EditWindow_Log.cs, `/home/pawbeans/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Player.log` | Log analyser tails and parses Player.log without the game running a window |
| Debug settings | `DebugSettings` is a static class of about 47 boolean switches (god mode, fast research, fast crafting, pause on error, unlimited power, instant recruit, translation-window-in-English, and more). They are in-memory game state, not command-line arguments. | decompiled:Verse/DebugSettings.cs | Do not try to set them from outside; list them in a cheat sheet only |
| Debug actions menu and TweakValue | In-game menus (wiki documents them). Not scriptable from outside. | wiki pages `DebugActions`, `TweakValue` | Link only |

Command-line surface found in the decompiled code: only `quicktest`, `savedatafolder`, `popupwindow` and `legacy-xml-deserializer` were found by grepping every command-line helper call in the decompiled tree. Unity's own arguments (for example `-batchmode`) are not game code and were not verified. Consequently there is no argument to select the mod folder or to point the game at another Mods folder: custom mod folders must be exposed to the game by the manager through link deployment or by copying, as already concluded in `docs/research/steam-and-game-detection.md`.

## 4. Tool catalogue

Columns: name and URL, last commit or release, licence, platforms, key features, gaps relative to RimStudio. "Not verified" means I could not read it on a page.

### 4.1 Mod managers and sorters

| Tool | Facts | Gaps vs RimStudio |
| --- | --- | --- |
| RimSort, https://github.com/RimSort/RimSort | GPL-3.0, Python and PySide6, Windows, macOS, Linux. Local copy is the reference for the manager (see `docs/research/rimsort-feature-and-ux-inventory.md`). Community datasets: https://github.com/RimSort/Community-Rules-Database (last commit 2026-09-13, 47 stars, licence not shown on the repo page) and https://github.com/RimSort/Steam-Workshop-Database (last commit 2026-09-13, 53 stars, licence not shown). | No creator tooling; Python start-up and list performance; GPL means concepts only. |
| RimCrow, https://github.com/Inky-Feather/RimCrow | MIT, 160 stars, last commit and release v0.24.7 on 2026-07-27. Python backend, Vue 3 frontend in a pywebview window. Manager with backups, texture optimisation, troubleshooting and assistant features; creator tools are a plan only. Deep analysis: `docs/research/rimcrow-analysis.md` (section 5 covers the creator plan). | Creator tools unbuilt; no def engine; no CE-aware item design. |
| RimPy, https://github.com/rimpy-custom/RimPy | 564 stars, 95 open issues, 6 commits; last commit 2022-03-08; latest release 1.2.6.29 on 2022-12-02 (Atom feeds). The project site claims free and open source and a Linux build (search only). Licence and platform detail not verified from the repo. Also distributes a Steam Workshop item "RimPy Mod Manager Database" (id 1847679158, from a search result URL; not opened). | Dormant since 2022; no toolkit; database format is its own. Compatibility target for import is optional. |
| Mod Manager (in game), https://github.com/fluffy-mods/ModManager | MIT, 106 stars, last release v4.8.1119 on 2022-10-14. A mod that replaces the in-game mod screen with a faster one. | Runs inside the game, so it cannot see mods before load; no dataset sync. |
| Circinus Mod Manager, https://github.com/Astryls/CircinusModManager | MIT, v1.8.0 on 2026-10-02, Tauri 2 desktop app with Rust crates and a Svelte frontend (cloned and inspected). | Young; relevant as a Tauri plus Rust peer. Not studied further. |
| RimModManager, https://github.com/MrXploisLite/RimModManager | Cross-platform claim (Windows, macOS, Linux, Steam Deck), v0.6.0 on 2026-07-13, 14 stars. Licence text unclear on the page ("include the original license"). | Young; licence unclear, do not reuse. |
| RimworldModManager, https://github.com/Zeracronius/RimworldModManager | GPL-3.0, v2.1.9 on 2026-05-20, drag and drop and basic load order validation. | Windows desktop style; no dataset sync. |
| rimbisect, https://github.com/Booyaka101/rimbisect | MIT, v0.2.2 on 2026-09-30, Windows only, unattended bisection of a mod list by relaunching the game and reading logs; never writes the real mod list (uses a probe helper mod it removes). | Windows only; separate tool. Shows demand for the bisect flow RimStudio can offer in the manager. |
| rimmods CLI, https://github.com/Stay1444/rimmods | 0.0.3 (2023); downloads workshop mods via SteamCMD. | Niche. |

### 4.2 Reference sites and decompiled source

| Tool | Facts | Gaps |
| --- | --- | --- |
| RimWorld Wiki, https://rimworldwiki.com/wiki/Modding_Tutorials | Community wiki; the modding tutorial hub (opened, section 2). Some pages predate 1.6. | Not structured data; RimStudio should link rather than depend on it. |
| Epicguru Rimworld-Auto-Documentation, https://github.com/Epicguru/Rimworld-Auto-Documentation | 27 stars, release 2021-10-26, last commit 2024-04-12, no licence shown. Generated XML documentation of defs. | Stale for 1.6; shows the demand for generated def docs. |
| Chillu1 RimWorldDecompiled, https://github.com/Chillu1/RimWorldDecompiled | 109 stars, last commit 2026-05-20, repository states personal-use-only; no licence file shown. | Redistributes Ludeon code; RimStudio must not vendor or mirror it. RimStudio reads game metadata from the user's install. |
| Dyyrlysh RimworldDecompile, https://github.com/Dyyrlysh/RimworldDecompile | 1.6 decompile, last commit 2025-08-24, no licence. | Same. |
| Older decompiles (RimWorld-zh/RimWorld-Decompile 2018, josh-m/RW-Decompile 2018, Axinex/RimWorld_CoreDoc 2018) | Last commits 2018. | Obsolete. |

Category note: no first-party searchable "def reference site" for 1.6 was found; the wiki and decompile mirrors are the substitutes. This confirms the value of a local Def Explorer over the user's own install.

### 4.3 Editor and IDE support for RimWorld XML

| Tool | Facts | Gaps |
| --- | --- | --- |
| RWXML Language Server (VS Code), https://marketplace.visualstudio.com/items?itemName=madeline.rwxml-lang-serv (opened) | Version 0.42.0, updated 2025-10-15, 158,108 installs; features: syntax highlighting, analysis, completion, go to definition and references, symbol rename, inheritance attribute suggestions, AlienRace support; needs configuration of the game folder, Core and mod folders; works from dynamic analysis of C# classes. Source: https://github.com/zzzz465/rwxml-language-server, Apache-2.0, last commit 2025-12-28, last tagged release 2022-10-15. An "Insider" build exists (search only). | Editor extension, not a manager; no patch simulation; configuration burden; no CE or item math. |
| Rider RimWorld plugin, https://github.com/Garethp/Rider-RimworldDevelopment (opened) | MIT, release 2025.1.11 on 2026-05-12, last commit 2026-09-21, 19 stars. Completion of def types, properties, defNames and enum values; navigation between XML and C#; find usages; code generation; run configuration with Doorstop; project templates. Author states no VS Code version is planned. | JetBrains-only; C# oriented. |
| RimWorld Extension Pack (VS Code), HikageWorks.rimworld-extension-pack (search only) | Bundles XML extensions and the language server. Not opened. | Not verified. |
| RimWorld XML catalogs, https://github.com/Trollam/RimWorld-XML-catalogs | GPL-3.0, last commit 2024-01-09, 0 stars. Purpose not verified beyond the repository name; probably schema catalogs for editors. | Stale; GPL. |
| SchemaGenerator, https://github.com/TanmanG/SchemaGenerator | MIT, 2023-06-18, infers an XSD from sample XML folders (generic, not RimWorld specific). | Sample-based inference is weaker than reflection (section 6 of the toolkit note). |

No JetBrains plugin other than Rider's was found; no XSD published by Ludeon exists (the game deserialises by reflection).

### 4.4 Source-level aids (C#, debugging)

| Tool | Facts |
| --- | --- |
| Harmony, https://github.com/pardeike/Harmony | MIT, release 2.4.2 on 2025-11-13, last commit 2026-09-08; the patching library nearly all C# mods use. |
| RimWorld4Debugging, https://github.com/pardeike/RimWorld4Debugging | MIT, 2025-10-03; debugging helper. |
| Rimworld-Doorstop, https://github.com/pardeike/Rimworld-Doorstop | Release 2026-03-04, no licence shown; Doorstop that also changes mod loading for debugging. |

These support C# modders; RimStudio's toolkit stays on the XML and asset side and only scaffolds links to them.

### 4.5 Translation tools

| Tool | Facts | Gaps |
| --- | --- | --- |
| RimTrans (RimWorld-zh), https://github.com/RimWorld-zh/RimTrans | MIT, 83 stars, release 0.18.2.6 on 2020-01-27, last commit 2020-01-19. | Idle for 6 years; targets old versions. |
| RimTrans (Aironsoft), https://github.com/Aironsoft/RimTrans | v0.21.9.13 on 2021-09-13; extracts language files for editing; no licence shown. | Idle. |
| Mod Translation Toolkit, https://github.com/DrizztGaming/Mod-Translation-Toolkit | MIT, v0.10.26 on 2026-09-11, 3 stars; a translation tool "currently focused on RimWorld". | Young; not studied in depth. |
| Translation Forge, https://github.com/Momaomao8787/Translation-Forge | MIT, v0.8.7 on 2026-09-26, last commit 2026-10-03, 2 stars. | Young. |
| RimWorld-translator (burukinsd), https://github.com/burukinsd/RimWorld-translator | Last commit 2026-09-22, no licence shown. | Not verified in detail. |

Pattern: translation extraction from `DefInjected` and `Keyed` is repeatedly rebuilt; none is integrated with a def engine that knows which fields are translatable (the `[MustTranslate]` attribute in the assembly is a reflection feature, see the toolkit note).

### 4.6 Texture tools

| Tool | Facts | Gaps |
| --- | --- | --- |
| todds, https://github.com/todds-encoder/todds (opened; the old URL joseasoler/todds redirects to it) | MPL-2.0, repository archived on 2026-03-31 (read-only), last release 0.4.1 on 2023-11-19. CPU DDS encoder for batch conversion with BC1 and BC7, mipmaps, up to 32 threads, quality levels 0 to 7, regex file filters; builds for Windows, Linux, macOS with CMake. Author points to a successor, imutate (https://codeberg.org/joseasoler/imutate), that is planned to extend the feature set. | Archived; CLI only; RimStudio must decide to shell out to todds (MPL-2.0 is file-level copyleft, bundling a binary is allowed) or write its own encoder. |
| RimCrow texture optimisation, RimPy texture optimisation | Both managers advertise converting mod textures to DDS to cut load time and memory (RimCrow: see `docs/research/rimcrow-analysis.md` section 2.5; RimPy: search only). | Optimise installed mods, not author-side validation. |

### 4.7 Mod templates and scaffolding

| Tool | Facts |
| --- | --- |
| Rimworld-Mods/Template, https://github.com/Rimworld-Mods/Template | VS Code template, 64 stars, last commit 2026-07-03, no licence shown. |
| truemogician RimWorld-Mod-Template, https://github.com/truemogician/RimWorld-Mod-Template | Visual Studio template with CI/CD pipelines, MIT, last commit 2025-12-23. |
| Zeta-of-the-rim Rimwold-Dotnet-Template, https://github.com/Zeta-of-the-rim/Rimwold-Dotnet-Template | `dotnet new` template, 31 stars, last commit 2023-07-09. |
| Lakuna RimWorld-Mod-Template, https://github.com/Lakuna/RimWorld-Mod-Template | MIT, last commit 2026-04-25; Harmony oriented. |
| krafs/RimCI, https://github.com/krafs/RimCI | MIT, example mod built with GitHub Actions, last commit 2024-09-04. |
| RimWorks/mod-ci, https://github.com/RimWorks/mod-ci | MIT, v3.3.0 on 2026-10-03; shared release plumbing (publish stamps, Workshop version bumps). |
| Owner's template, `/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods/rimworld-mod-template` | Folder contains `About/About.xml`, `1.5/Defs/LetterDef.xml`, `1.5/Patches/Patches.xml`, `1.5/Languages/English/Data.xml`, `Source/Main.cs`, a solution file and `.vscode/` with Linux and Windows `mod.csproj` plus build scripts and launch configs. The `.vscode/` folder keeps all intermediate build output; `.gitignore` excludes obj folders, the solution and built DLLs. The README says to clone it into the game's `Mods` folder and press F5 to build and launch. The template is still on 1.5 folders. |

Gap: every template is a static copy; none generates a version-aware, load-folder-aware project, and none keeps tool metadata out of the shipped folder.

### 4.8 Combat Extended patching guides and auto-patchers

| Tool | Facts | Gaps |
| --- | --- | --- |
| CE Compatibility Patch Guide, https://github.com/CombatExtended-Continued/CombatExtended/wiki/Compatibility-Patch-Guide | Official guide (CC BY-NC-SA 4.0 content; concepts only). Fetched copy in the scratch folder `ce/compat-guide.md`. Analysed in `docs/research/ce-patch-conventions.md`. | Prose; no generator. |
| CE API reference page and FAQ | Fetched copies in `ce/`; the API page documents callbacks for stopping CE projectiles. | Not relevant to RimStudio's first release. |
| CE in-game auto-patch operation | CE ships a patch operation that converts a vanilla-style gun at game load time (class `PatchOperationMakeGunCECompatible`, file in the CE source checkout). It is a runtime convenience: the result is invisible and not editable. Formula analysis: `docs/research/ce-autopatcher-formulas.md`. | RimStudio's generator writes explicit, reviewable patch files. |
| CE integrated patches | CE's checkout contains 760 `ModPatches` folders written by hand for third-party mods (`docs/research/ce-patch-conventions.md`). | Per-mod human work: the dominant cost the generator reduces. |
| RimWorldForge, https://github.com/ShugokiFable/RimWorldForge | MIT, v0.2.1 released 2026-08-29, last commit 2026-09-14, Python 3.10 or newer, RimWorld 1.6. "Assistant features" workbench: deterministic tools for def indexing over game and installed mods, generation, validation, C# scaffolding, packaging and log analysis, plus a skill document for agents. Opened README. | Command-line and agent oriented; no GUI; no mod manager; CE not a stated focus. Shows the same def-index idea. |

No GUI patch generator for CE was found other than the in-game operation above.

### 4.9 Weapon and apparel balance

| Source | Facts |
| --- | --- |
| CE Stat-a-Balance spreadsheets | A page lists four Google Sheets: Guns, Projectiles and Ammunition, Melee Attacks (Weapons and Races), Races and Creatures. Fetched xlsx exports are in the scratch folder `ce/` (about 1.8 MB total, not committed). They hold CE's derivation of new stats; they are CC BY-NC-SA material and stay a read-only reference. |
| Community conventions | CE's compatibility guide gives ranges and formulas by weapon category; summarised in `docs/research/ce-patch-conventions.md` and `docs/research/ce-autopatcher-formulas.md`. |
| Standalone calculators | None found that is maintained and covers 1.6 (search was limited; absence not proven). |

Opportunity: an offline calculator in the item designer that reads vanilla and CE values from the user's install (R7, R11).

### 4.10 Log analysis and troubleshooting

| Tool | Facts |
| --- | --- |
| rimbisect | Section 4.1. |
| rw-log-check, https://github.com/orionfive/rw-log-check | Last commit 2025-08-16, 0 stars, no licence shown; log checking helper, details not verified. |
| RimCrow log tooling | See `docs/research/rimcrow-analysis.md` section 2.6. |
| Game log window and wiki guide `Troubleshooting/Finding_Exceptions` | Section 2, 3. |

### 4.11 Workshop upload tools

| Tool | Facts | Gaps |
| --- | --- | --- |
| In-game uploader | Built into the game; full analysis in `docs/research/workshop-publishing-research.md` sections 1 to 4. It uploads the whole mod folder; excluded items and pitfalls are in section 1.4 and section 4 of that note. | No staging, no ignore file, limited metadata editing. |
| Publisher Plus fork, https://github.com/NightmareCorporation/PublisherPlusMultiVersionFork | Last commit 2025-10-06, no licence shown, "extended options for publishing mods". | Fork of an in-game mod; needs the running game. |
| steamworks-rs, https://github.com/Noxime/steamworks-rs | Apache-2.0, 484 stars, v0.13.1 on 2026-05-05, last commit 2026-08-27. Rust bindings to the Steamworks SDK. Relevant library, evaluated in `docs/research/workshop-publishing-research.md` section 5. | Library, not a tool. |
| Parallax companion (owner), `/run/media/pawbeans/project_drive/pawbeans/Projects/Go/parallax-mod-manager/companions/` | Owner's earlier Steam publish helper; analysed in the publishing note section 3. | Go sidecar. |

### 4.12 Framework mods relevant to a def engine

XmlExtensions (https://github.com/15adhami/XmlExtensions, GPL-3.0, v1.9.3 on 2026-05-20, 37 stars) adds custom patch operations to XML. Any patch tester that wants to simulate mods using such frameworks must either implement or flag their custom operations; the corpus numbers for custom operation usage are in `docs/research/xpath-patch-coverage.md`.

## 5. Feature comparison matrix

Legend: Y = yes (verified from page or README), P = partial or planned, N = no, ? = not verified. RimStudio column shows the plan from the toolkit note ("MVP" or "later"). Rows are product features; columns are representative tools per category.

| Feature | RimSort | RimCrow | RimPy | RWXML LS | Rider plugin | RimWorldForge | todds | rimbisect | In-game uploader | RimStudio plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Mod list and load order | Y | Y | Y | N | N | N | N | P | N | MVP |
| Community sort rules and datasets | Y | P | Y (own DB) | N | N | N | N | N | N | MVP |
| Auto-detect game and Steam | Y | Y | Y | N | N | Y | N | Y | n/a | MVP |
| Custom mod folders | Y | ? | ? | N | N | ? | N | N | N | MVP |
| Bisect to find faulty mod | N | P (planned) | N | N | N | N | N | Y | N | later |
| Def index over game and mods | N | N | N | Y | Y | Y | N | N | N | MVP (Def Explorer) |
| Schema-aware XML completion | N | N | N | Y | Y | N | N | N | N | later (embedded editor) |
| Inheritance tree and resolved view | N | N | N | P | P | ? | N | N | N | MVP |
| Patch simulation, "who patched this" | N | N | N | N | N | ? | N | N | N | MVP |
| Conflict and missing-reference check | P | P (planned) | P | P | P | Y (validation) | N | N | N | MVP |
| Texture to DDS | N | Y | Y | N | N | N | Y | N | N | later |
| Translation extraction | N | P | N | N | N | ? | N | N | N | later |
| Item designer with balance math | N | N | N | N | N | N | N | N | N | MVP (weapons, apparel) |
| CE patch generation | N | N | N | N | N | ? | N | N | N | MVP |
| Log analysis | P | Y | P | N | N | Y | N | Y | N | MVP |
| Workshop upload and update | N | P (planned) | N | N | N | ? | N | N | Y | later |
| Staged packaging with ignore rules | N | N | N | N | N | Y (packaging) | N | N | N | later (with upload) |
| Cross-platform desktop | Y | Y | P | Y (editor) | Y | Y (CLI) | Y | N (Windows) | Y | MVP |

RimSort column facts rest on `docs/research/rimsort-feature-and-ux-inventory.md`; RimCrow on `docs/research/rimcrow-analysis.md`. RimPy cells marked from search-level evidence are weaker.

## 6. Gaps and opportunities (what a fast Rust def and patch engine does uniquely well)

1. One engine that serves manager and toolkit: load order, def index and patch effects over the user's actual active list. Nobody ships this combination.
2. Speed: a single-threaded Python prototype loaded vanilla plus DLC in 2.0 s but needed 19.8 s for vanilla plus CE, of which 16.5 s was 2849 patch operations evaluated by linear XPath scans (`docs/research/def-engine-semantics.md` section 9.4). A Rust engine with defName and Name indexes (that note's implication 5) targets sub-second results, which is what makes live "resolved after patches" views possible while typing (a target, not a measurement).
3. Patch provenance: recording which operation touched which node enables the "who patched this" view that the language servers lack.
4. Reflection-derived schema shipped as compact JSON, independent of any editor.
5. Reviewable CE patches instead of runtime-invisible auto-conversion.
6. Native, dependency-free DDS and packaging paths, because the leading DDS tool is archived.
7. Safe launch tooling (isolated save folder, snapshot of ModsConfig.xml, reset-on-crash warning) that no manager exposes today (unverified for RimPy).

## Implications for RimStudio

1. The Def Explorer, patch tester and validator are the unique differentiators; they must be built on the same def database the manager uses, not a separate index.
2. The dev launcher module must expose exactly four verified switches (dev mode in Prefs.xml, `-quicktest`, `savedatafolder=`, `-popupwindow`), refuse a `savedatafolder` path containing `=`, and refuse to change Prefs.xml while the game runs.
3. Before every dev launch, copy ModsConfig.xml to a backup, because `resetModsConfigOnCrash` defaults to true.
4. If `DevModeDisabled` exists in the config folder, the toolkit must show dev mode as locked rather than rewriting Prefs.xml.
5. Link out to the wiki pages in section 2 instead of mirroring text; store URLs as data in a JSONC link catalogue so they can be updated without a release.
6. Do not vendor, mirror or fetch decompiled game code; derive metadata at dev time from the assembly and runtime from the user's install.
7. Texture conversion ships as an optional module that can call an external encoder (todds or its successor) or an in-house encoder, decided by an ADR; MPL-2.0 permits bundling an unmodified binary with attribution.
8. Provide a mod bisect flow in the manager (the demand is evidenced by rimbisect and RimCrow's plan); it must never write the user's real ModsConfig.xml without a snapshot.
9. Parse and show custom patch operation classes from frameworks (for example XmlExtensions) as "unknown operation: simulation skipped" rather than failing.
10. Keep the owner's template as the default scaffold source but update it to 1.6 folder layout and move build scaffolding out of the shipped tree.

## Open questions

1. RimPy licence, platform support and current maintenance state (repo shows 2022 activity; website claims newer builds).
2. Whether the imutate successor to todds supports BC7 batch conversion today (not opened).
3. Exact purpose and contents of Trollam/RimWorld-XML-catalogs, HikageWorks Extension Pack and rw-log-check (names only).
4. Whether any maintained balance calculator exists outside GitHub (forums, Discord); searched only GitHub and the CE wiki.
5. Which Unity or game command-line arguments exist beyond the four found (Unity player arguments were not verified).
6. Steam Workshop item pages (RimPy database, Publisher Plus original) were not opened because the Workshop requires a browser session; their metadata is search-level.
