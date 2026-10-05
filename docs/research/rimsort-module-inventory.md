# RimSort module inventory

Scope: every Python module under RimSort-main/app with its line count (measured by script), responsibility in our own words, layer, and the decision for RimStudio. Companion to rimsort-core-domain.md. RimSort is GPL-3.0 and is used as a concept reference only.

Status: research note | Last verified: 2026-10-04

## How this table was produced

The table is rendered by docs/research/data/rimsort-core/gen_module_inventory.py (arguments: RimSort root, a template, an output path). LOC is the newline count of each file, the same rule as `wc -l`. The layer and decision columns are our own annotations, made from reading module docstrings, class names and, for the engine modules, the code itself. Layers: domain (engine logic RimStudio must own), ui (Qt widgets and view glue), integration (Steam, git, network, OS, external tools). Decisions: port (port the concept into a Rust crate), crate (replace by an existing Rust crate or a small wrapper), redesign (keep the intent, change the design), skip (do not carry over).

## Summary

| Group | Modules | LOC |
|---|---:|---:|
| dec: crate | 19 | 5857 |
| dec: port | 36 | 11954 |
| dec: redesign | 52 | 34302 |
| dec: skip | 60 | 11297 |
| layer: domain | 43 | 11098 |
| layer: integration | 52 | 17960 |
| layer: ui | 72 | 34352 |
| total | 167 | 63410 |

Reading the summary: the engine proper (models/metadata, sort, mod_list, import and export, history, schema) is a small fraction of the 63k lines. Most of the code is Qt views, and one file (views/mods_panel.py, 5,820 lines) mixes list widgets with the warning and error calculation that belongs in the engine. Three files above 2,000 lines (update_utils.py, main_content_panel.py, main_content_controller.py) are orchestration or self-update code with little reusable domain content.

## Table

| Path | LOC | Layer | Responsibility | RimStudio decision |
|---|---:|---|---|---|
| __init__.py | 0 | domain | package marker | skip |
| __main__.py | 276 | ui | process entry: crash hook, single-instance guard, Qt bootstrap, CLI dispatch | redesign |
| cli/__init__.py | 1 | integration | package marker for the headless CLI | redesign |
| cli/build_db.py | 207 | integration | headless command that builds the Steam Workshop metadata database | redesign |
| cli/main.py | 30 | integration | click entry point for headless subcommands | redesign |
| controllers/__init__.py | 0 | domain | package marker | skip |
| controllers/app_controller.py | 170 | ui | wires controllers and the main window at startup | skip |
| controllers/file_search_controller.py | 1145 | ui | drives the file search dialog and its search worker | redesign |
| controllers/instance_controller.py | 321 | domain | create, clone, backup and restore game instances (UI-side orchestration) | port |
| controllers/language_controller.py | 126 | ui | switches the UI translation | skip |
| controllers/main_content_controller.py | 2134 | integration | git clone and update flows, database download, GitHub mod install, database upload to a repo | redesign |
| controllers/main_window_controller.py | 204 | ui | main window state and theme hooks | skip |
| controllers/menu_bar_controller.py | 316 | ui | menu bar actions routed to the main content | skip |
| controllers/metadata_controller.py | 773 | domain | singleton facade over metadata: refresh, indexes, compile, version mismatch, replacements, list resolution, aux DB sync | redesign |
| controllers/metadata_db_controller.py | 327 | domain | SQLAlchemy sessions for the per-mod auxiliary database | redesign |
| controllers/mods_panel_controller.py | 378 | ui | mods panel signal wiring and filter state | skip |
| controllers/settings_controller.py | 517 | ui | settings dialog glue, load and save, reset | redesign |
| controllers/settings_tabs/__init__.py | 39 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/advanced_tab_controller.py | 158 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/appearance_tab_controller.py | 200 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/base_tab_controller.py | 39 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/database_builder_tab_controller.py | 132 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/databases_tab_controller.py | 262 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/external_tools_tab_controller.py | 42 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/game_launch_tab_controller.py | 49 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/internal_tools_tab_controller.py | 142 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/locations_tab_controller.py | 273 | ui | one controller per settings tab | skip |
| controllers/settings_tabs/sorting_tab_controller.py | 144 | ui | one controller per settings tab | skip |
| controllers/sort_controller.py | 165 | domain | Sorter: builds the four tier subgraphs and runs the chosen algorithm per tier | port |
| controllers/theme_controller.py | 248 | ui | theme loading and application | skip |
| controllers/todds_controller.py | 142 | integration | runs the external texture optimiser | redesign |
| controllers/troubleshooting_controller.py | 840 | integration | backup, restore and cleanup actions (saves, config, mods, caches) | redesign |
| models/__init__.py | 0 | domain | package marker | skip |
| models/animations.py | 154 | ui | loading animation labels | skip |
| models/divider.py | 41 | ui | divider rows inside the active list (data only) | port |
| models/filter_state.py | 87 | ui | filter flags for the mod lists | port |
| models/image_label.py | 56 | ui | image label widget | skip |
| models/instance.py | 36 | domain | Instance record: game, config, local, workshop folders, run args, steamcmd options | port |
| models/metadata/__init__.py | 43 | domain | package exports | skip |
| models/metadata/metadata_db.py | 92 | domain | SQLAlchemy tables for notes, colour, tags, ignore flag, timestamps keyed by mod path | redesign |
| models/metadata/metadata_factory.py | 940 | domain | About.xml to mod model: field extraction, ByVersion, dependencies, mod type, rules DB readers and writers, mods config reader | port |
| models/metadata/metadata_mediator.py | 404 | domain | refresh driver: reads rules and DBs, scans three roots, parser threads, attaches community and user rules | redesign |
| models/metadata/metadata_structure.py | 767 | domain | mod types, case-insensitive ids, rule merge, CompiledDependencyData graph builder, DB schemas | port |
| models/mod_list.py | 413 | domain | ordered list with path and package id indexes, diff, resolve; not imported by app code yet | port |
| models/operation_mode.py | 10 | ui | enum for list operation mode | skip |
| models/search_result.py | 101 | ui | file search result row | skip |
| models/settings.py | 707 | domain | settings struct (about 100 fields), load, migrate, save; catalogue is another note | redesign |
| services/__init__.py | 0 | domain | package marker | skip |
| services/dependency_resolver.py | 235 | domain | classifies dependencies as satisfied, local or downloadable and resolves workshop ids | port |
| services/http_download_service.py | 106 | integration | background HTTP download of database archives | crate |
| services/import_export_service.py | 328 | domain | collects active list, writes ModsConfig, clipboard and Rentry reports, imports lists | port |
| services/instance_service.py | 937 | domain | clone, copy, back up, restore and delete instances | port |
| services/mod_list_parser.py | 154 | domain | detects and parses ModsConfig XML, RimSort JSON, rml, rws and save files into package ids | port |
| services/mod_path_service.py | 39 | domain | derives mod folders from an Instance | port |
| services/modlist_history_service.py | 462 | domain | snapshots of the active list with ids, diff and pruning | port |
| services/path_autodetect_service.py | 698 | domain | per-OS game, workshop and config path guesses incl. GOG and Heroic | redesign |
| services/version_data_service.py | 85 | domain | loads the RimWorld versions list for downloads | port |
| services/window_manager.py | 84 | ui | tracks child windows | skip |
| sort/__init__.py | 0 | domain | package marker | skip |
| sort/alphabetical_sort.py | 108 | domain | deprecated insertion sort with recursive dependency injection | skip |
| sort/dependencies.py | 48 | domain | tier subgraph extraction and recursive closure helpers | port |
| sort/mod_sorting.py | 582 | domain | keys for sorting the inactive list (name, author, size, tags, colour) plus folder size workers | redesign |
| sort/topo_sort.py | 95 | domain | level-based topological sort with alphabetical order inside a level and cycle reporting | redesign |
| utils/__init__.py | 0 | domain | package marker | skip |
| utils/acf_utils.py | 571 | integration | reads appworkshop ACF files, merges sources, purges steamcmd entries | crate |
| utils/app_info.py | 407 | integration | app folders, version, bundled resources | redesign |
| utils/aux_db_utils.py | 402 | domain | helpers over the auxiliary DB: colour, tags, notes, ignore | redesign |
| utils/button_factory.py | 191 | ui | standard button builders | skip |
| utils/constants.py | 117 | domain | tier zero and one id lists, DLC table, sentinel package id, enums | port |
| utils/csv_export_utils.py | 322 | ui | CSV export of the visible mod table | port |
| utils/custom_list_widget_item.py | 43 | ui | list item subclass | skip |
| utils/custom_list_widget_item_metadata.py | 231 | ui | per-row display data for the list widget | skip |
| utils/custom_qlabels.py | 79 | ui | clickable labels | skip |
| utils/db_builder.py | 537 | integration | Qt wrapper over the Workshop database builder | redesign |
| utils/db_builder_core.py | 220 | domain | Qt-free core of the Workshop database builder | port |
| utils/dds_utility.py | 54 | integration | DDS texture file helpers | skip |
| utils/dict_utils.py | 45 | domain | recursive dict update | skip |
| utils/event_bus.py | 165 | ui | global Qt signal hub | skip |
| utils/file_search.py | 297 | integration | walks mod folders for text and file name matches | crate |
| utils/files.py | 189 | domain | C# and patch probes, saves backup, backup pruning | port |
| utils/generic.py | 861 | integration | clipboard, rmtree, scandir wrappers, open folder, misc | crate |
| utils/git_utils.py | 1867 | integration | pygit2 clone, pull, push, status and token helpers | crate |
| utils/git_worker.py | 582 | integration | Qt workers running git batches | crate |
| utils/github/__init__.py | 37 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/github/installer.py | 247 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/github/models.py | 60 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/github/provider.py | 308 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/github/updater.py | 120 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/github/worker.py | 153 | integration | GitHub provider, installer, updater and workers for installing mods from repos | redesign |
| utils/globals.py | 12 | ui | global flags | skip |
| utils/gui_info.py | 213 | ui | fonts and sizes | skip |
| utils/http.py | 72 | integration | HTTP with retries | crate |
| utils/http_downloader.py | 285 | integration | conditional HTTP downloads of datasets | crate |
| utils/ignore_extensions.py | 104 | domain | default ignore patterns for file operations | port |
| utils/ignore_manager.py | 210 | domain | persisted set of package ids whose warnings are ignored | port |
| utils/json_utils.py | 76 | domain | atomic JSON write with retry | crate |
| utils/launch_command_parser.py | 106 | integration | parses Steam style %command% launch options | port |
| utils/log_setup.py | 129 | integration | loguru configuration | crate |
| utils/mod_info.py | 298 | ui | view model for the mod info panel | skip |
| utils/mod_utils.py | 338 | domain | resolves Steam update and touch timestamps for display and sort | port |
| utils/platform/__init__.py | 0 | integration | Windows directory entry helpers for reparse points | crate |
| utils/platform/windows.py | 98 | integration | Windows directory entry helpers for reparse points | crate |
| utils/privatebin.py | 142 | integration | PrivateBin client for sharing lists | skip |
| utils/pygit2_loader.py | 48 | integration | SSL-safe pygit2 import | skip |
| utils/rentry/__init__.py | 0 | integration | Rentry.co upload client for list reports | redesign |
| utils/rentry/wrapper.py | 386 | integration | Rentry.co upload client for list reports | redesign |
| utils/schema.py | 103 | domain | builds and validates ModsConfig dictionaries | port |
| utils/single_instance.py | 50 | integration | single instance lock | crate |
| utils/startup_impact.py | 243 | domain | parses the game's Loading Progress report for per-mod startup cost | port |
| utils/steam/__init__.py | 0 | domain | package marker | skip |
| utils/steam/availability.py | 292 | integration | detects the Steam executable and whether Steam runs | redesign |
| utils/steam/db_builder_thread.py | 202 | integration | thread that builds the Workshop database via the web API | redesign |
| utils/steam/steambrowser/__init__.py | 0 | ui | embedded Steam Workshop browser (web view) | skip |
| utils/steam/steambrowser/browser.py | 1233 | ui | embedded Steam Workshop browser (web view) | skip |
| utils/steam/steambrowser/js_bridge.py | 40 | ui | embedded Steam Workshop browser (web view) | skip |
| utils/steam/steamcmd/__init__.py | 0 | integration | SteamCMD install, login-free downloads, depot cache cleanup | redesign |
| utils/steam/steamcmd/wrapper.py | 880 | integration | SteamCMD install, login-free downloads, depot cache cleanup | redesign |
| utils/steam/steamfiles/__init__.py | 0 | integration | ACF to dict and back | crate |
| utils/steam/steamfiles/wrapper.py | 23 | integration | ACF to dict and back | crate |
| utils/steam/steamworks/__init__.py | 0 | integration | Steamworks client API wrapper for subscribe, unsubscribe, launch | redesign |
| utils/steam/steamworks/wrapper.py | 744 | integration | Steamworks client API wrapper for subscribe, unsubscribe, launch | redesign |
| utils/steam/webapi/__init__.py | 0 | integration | Steam Web API queries for Workshop items and collections | redesign |
| utils/steam/webapi/wrapper.py | 991 | integration | Steam Web API queries for Workshop items and collections | redesign |
| utils/steam/workshop_urls.py | 16 | integration | Workshop search URL builder | port |
| utils/steam/workshop_utils.py | 235 | integration | Workshop update check result model and helpers | port |
| utils/symlink.py | 177 | integration | symlink and junction creation | crate |
| utils/system_info.py | 124 | integration | OS and hardware info | crate |
| utils/todds/__init__.py | 0 | integration | texture optimiser runner | redesign |
| utils/todds/wrapper.py | 158 | integration | texture optimiser runner | redesign |
| utils/update_utils.py | 2658 | integration | application self-update download and swap, per OS | redesign |
| utils/watchdog.py | 292 | integration | file system watcher raising create, delete and modify events for mod folders | crate |
| utils/win_find_steam.py | 44 | integration | Windows registry lookup of the Steam folder | redesign |
| utils/window_launch_state.py | 32 | ui | window geometry restore | skip |
| utils/xml.py | 292 | domain | XML to nested dict conversion (ElementTree and bs4), dict to XML writer, gzip and zstd helpers | redesign |
| utils/zip_extractor.py | 247 | integration | ZIP validation, extraction and backup | crate |
| views/__init__.py | 0 | ui | Qt widget or dialog | skip |
| views/acf_log_reader.py | 709 | ui | viewer for ACF workshop items | skip |
| views/deletion_menu.py | 820 | ui | mod deletion choices: delete, unsubscribe, keep data | port |
| views/description_widget.py | 128 | ui | Qt widget or dialog | skip |
| views/dialogue.py | 847 | ui | Qt widget or dialog | skip |
| views/divider_widget.py | 89 | ui | Qt widget or dialog | skip |
| views/download_rimworld_dialog.py | 156 | ui | Qt widget or dialog | skip |
| views/file_search_dialog.py | 862 | ui | Qt widget or dialog | skip |
| views/filter_panel.py | 650 | ui | Qt widget or dialog | skip |
| views/main_content_panel.py | 3275 | ui | central panel: refresh, sort, import, export, save, restore orchestration | redesign |
| views/main_window.py | 417 | ui | Qt widget or dialog | skip |
| views/menu_bar.py | 431 | ui | Qt widget or dialog | skip |
| views/mod_info_panel.py | 1168 | ui | details pane for the selected mod | redesign |
| views/mods_panel.py | 5820 | ui | list widgets plus the warning and error calculation (domain logic buried in the widget) | redesign |
| views/player_log_tab.py | 1587 | ui | game log viewer with pattern highlighting | port |
| views/settings_dialog.py | 1812 | ui | settings dialog widgets | redesign |
| views/status_panel.py | 146 | ui | Qt widget or dialog | skip |
| views/task_progress_window.py | 130 | ui | Qt widget or dialog | skip |
| views/troubleshooting_dialog.py | 428 | ui | Qt widget or dialog | skip |
| windows/__init__.py | 0 | ui | secondary window | redesign |
| windows/base_mods_panel.py | 1266 | ui | secondary window | redesign |
| windows/duplicate_mods_panel.py | 86 | ui | choose which duplicate package id copy to keep | port |
| windows/github_mods_panel.py | 501 | ui | secondary window | redesign |
| windows/ignore_json_editor.py | 190 | ui | secondary window | redesign |
| windows/missing_dependencies_dialog.py | 410 | ui | dialog to add or download missing dependencies before sorting | port |
| windows/missing_mod_properties_panel.py | 334 | ui | list mods missing package id or published file id | port |
| windows/missing_mods_panel.py | 431 | ui | prompt for mods listed in a list but not installed | port |
| windows/modlist_history_panel.py | 323 | ui | browse and diff history snapshots | port |
| windows/rule_editor_panel.py | 1435 | ui | editor for community and user rules, drag and drop rule creation | redesign |
| windows/runner_panel.py | 815 | ui | secondary window | redesign |
| windows/use_this_instead_panel.py | 702 | ui | panel listing replacement recommendations with install status | port |
| windows/workshop_mod_updater_panel.py | 132 | ui | secondary window | redesign |

## Observations that matter for the rewrite

1. The domain core is concentrated in six files: models/metadata/metadata_structure.py, metadata_factory.py, metadata_mediator.py, controllers/metadata_controller.py, controllers/sort_controller.py and sort/*. Together they are about 3,900 lines. A Rust engine can cover the same ground in a few crates (see the first document for the proposed split).
2. views/mods_panel.py contains `recalculate_internal_errors_warnings` and the `_check_*` helpers. These are pure functions of the list order and the metadata and must move into an engine crate with unit tests; the view should only render their result.
3. models/mod_list.py (413 lines) is a newer pure model with indexes, diff and resolution logic and its own tests, but no other module imports it (checked by searching the app tree). It shows the direction the maintainers were heading and is a good template for the RimStudio list model.
4. Three modules duplicate the game's own behaviour in Python: schema.py (ModsConfig shape), the About.xml reader in metadata_factory.py, and the duplicate resolution in metadata_controller.py. RimStudio should implement each once, in one crate, with the game's semantics as the reference.
5. Everything under utils/steam, utils/git*, utils/github, utils/update_utils.py, utils/rentry and utils/privatebin is integration code. It is large (about 18,000 lines across 52 modules, per the summary) and mostly replaceable by small wrappers over crates or by calling the system tools.

## Implications for RimStudio

1. Create the engine crates around the six domain files first and test them headless; no Qt-equivalent dependency may appear in them (testable: the engine crates build with no GUI dependency in their dependency tree).
2. Move all list validation (errors, warnings, ignore handling) out of the view layer into one crate whose functions take an ordered list and an index and return structured problems.
3. Model the active list as an ordered collection with a path index and a package id index maintained incrementally (as models/mod_list.py intends), so move, insert and remove are O(log n) or O(1) per item.
4. Keep integration code behind narrow traits (Steam, git, HTTP, self-update) so each can be replaced or skipped per platform.
5. Do not port the embedded Workshop browser, the ACF log viewer or PrivateBin sharing without an explicit product decision (marked skip).

## Open questions

1. The layer and decision columns for UI modules were assigned from docstrings and class names without reading every widget; a second pass is advisable before planning UI work.
2. Whether RimStudio should keep an equivalent of the Workshop database builder (db_builder_core.py, steam/webapi) depends on the dataset fetch design in the separate research note.
3. self-update (utils/update_utils.py, 2,658 lines) may be replaced by Tauri's updater; this needs a decision outside this note.
