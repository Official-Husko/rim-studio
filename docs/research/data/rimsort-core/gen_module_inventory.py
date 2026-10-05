#!/usr/bin/env python3
"""Render the RimSort module inventory table (own annotations, measured LOC).

Usage: gen_module_inventory.py <rimsort_root> <template.md> <out.md>

Walks <rimsort_root>/app for *.py, counts lines (newline count, same rule as
`wc -l`), joins each module with the annotation table below (first matching
rule wins; exact paths before directory prefixes) and replaces the markers
{{TABLE}} and {{SUMMARY}} in the template. Reads only; deterministic; run it
with PYTHONDONTWRITEBYTECODE=1.

Layers: domain = engine logic RimStudio must own; ui = Qt widgets and view
glue; integration = talks to Steam, git, network, OS or external tools.
Decisions: port = port the concept; crate = replace with an existing crate;
redesign = keep the intent, change the design; skip = do not carry over.
"""
import os
import sys
from collections import defaultdict

# (path or prefix ending in '/', layer, decision, responsibility)
RULES = [
    ("app/__main__.py", "ui", "redesign", "process entry: crash hook, single-instance guard, Qt bootstrap, CLI dispatch"),
    ("app/cli/build_db.py", "integration", "redesign", "headless command that builds the Steam Workshop metadata database"),
    ("app/cli/main.py", "integration", "redesign", "click entry point for headless subcommands"),
    ("app/cli/", "integration", "redesign", "package marker for the headless CLI"),
    ("app/controllers/app_controller.py", "ui", "skip", "wires controllers and the main window at startup"),
    ("app/controllers/file_search_controller.py", "ui", "redesign", "drives the file search dialog and its search worker"),
    ("app/controllers/instance_controller.py", "domain", "port", "create, clone, backup and restore game instances (UI-side orchestration)"),
    ("app/controllers/language_controller.py", "ui", "skip", "switches the UI translation"),
    ("app/controllers/main_content_controller.py", "integration", "redesign", "git clone and update flows, database download, GitHub mod install, database upload to a repo"),
    ("app/controllers/main_window_controller.py", "ui", "skip", "main window state and theme hooks"),
    ("app/controllers/menu_bar_controller.py", "ui", "skip", "menu bar actions routed to the main content"),
    ("app/controllers/metadata_controller.py", "domain", "redesign", "singleton facade over metadata: refresh, indexes, compile, version mismatch, replacements, list resolution, aux DB sync"),
    ("app/controllers/metadata_db_controller.py", "domain", "redesign", "SQLAlchemy sessions for the per-mod auxiliary database"),
    ("app/controllers/mods_panel_controller.py", "ui", "skip", "mods panel signal wiring and filter state"),
    ("app/controllers/settings_controller.py", "ui", "redesign", "settings dialog glue, load and save, reset"),
    ("app/controllers/settings_tabs/", "ui", "skip", "one controller per settings tab"),
    ("app/controllers/sort_controller.py", "domain", "port", "Sorter: builds the four tier subgraphs and runs the chosen algorithm per tier"),
    ("app/controllers/theme_controller.py", "ui", "skip", "theme loading and application"),
    ("app/controllers/todds_controller.py", "integration", "redesign", "runs the external texture optimiser"),
    ("app/controllers/troubleshooting_controller.py", "integration", "redesign", "backup, restore and cleanup actions (saves, config, mods, caches)"),
    ("app/models/animations.py", "ui", "skip", "loading animation labels"),
    ("app/models/divider.py", "ui", "port", "divider rows inside the active list (data only)"),
    ("app/models/filter_state.py", "ui", "port", "filter flags for the mod lists"),
    ("app/models/image_label.py", "ui", "skip", "image label widget"),
    ("app/models/instance.py", "domain", "port", "Instance record: game, config, local, workshop folders, run args, steamcmd options"),
    ("app/models/metadata/__init__.py", "domain", "skip", "package exports"),
    ("app/models/metadata/metadata_db.py", "domain", "redesign", "SQLAlchemy tables for notes, colour, tags, ignore flag, timestamps keyed by mod path"),
    ("app/models/metadata/metadata_factory.py", "domain", "port", "About.xml to mod model: field extraction, ByVersion, dependencies, mod type, rules DB readers and writers, mods config reader"),
    ("app/models/metadata/metadata_mediator.py", "domain", "redesign", "refresh driver: reads rules and DBs, scans three roots, parser threads, attaches community and user rules"),
    ("app/models/metadata/metadata_structure.py", "domain", "port", "mod types, case-insensitive ids, rule merge, CompiledDependencyData graph builder, DB schemas"),
    ("app/models/mod_list.py", "domain", "port", "ordered list with path and package id indexes, diff, resolve; not imported by app code yet"),
    ("app/models/operation_mode.py", "ui", "skip", "enum for list operation mode"),
    ("app/models/search_result.py", "ui", "skip", "file search result row"),
    ("app/models/settings.py", "domain", "redesign", "settings struct (about 100 fields), load, migrate, save; catalogue is another note"),
    ("app/services/dependency_resolver.py", "domain", "port", "classifies dependencies as satisfied, local or downloadable and resolves workshop ids"),
    ("app/services/http_download_service.py", "integration", "crate", "background HTTP download of database archives"),
    ("app/services/import_export_service.py", "domain", "port", "collects active list, writes ModsConfig, clipboard and Rentry reports, imports lists"),
    ("app/services/instance_service.py", "domain", "port", "clone, copy, back up, restore and delete instances"),
    ("app/services/mod_list_parser.py", "domain", "port", "detects and parses ModsConfig XML, RimSort JSON, rml, rws and save files into package ids"),
    ("app/services/mod_path_service.py", "domain", "port", "derives mod folders from an Instance"),
    ("app/services/modlist_history_service.py", "domain", "port", "snapshots of the active list with ids, diff and pruning"),
    ("app/services/path_autodetect_service.py", "domain", "redesign", "per-OS game, workshop and config path guesses incl. GOG and Heroic"),
    ("app/services/version_data_service.py", "domain", "port", "loads the RimWorld versions list for downloads"),
    ("app/services/window_manager.py", "ui", "skip", "tracks child windows"),
    ("app/sort/alphabetical_sort.py", "domain", "skip", "deprecated insertion sort with recursive dependency injection"),
    ("app/sort/dependencies.py", "domain", "port", "tier subgraph extraction and recursive closure helpers"),
    ("app/sort/mod_sorting.py", "domain", "redesign", "keys for sorting the inactive list (name, author, size, tags, colour) plus folder size workers"),
    ("app/sort/topo_sort.py", "domain", "redesign", "level-based topological sort with alphabetical order inside a level and cycle reporting"),
    ("app/utils/acf_utils.py", "integration", "crate", "reads appworkshop ACF files, merges sources, purges steamcmd entries"),
    ("app/utils/app_info.py", "integration", "redesign", "app folders, version, bundled resources"),
    ("app/utils/aux_db_utils.py", "domain", "redesign", "helpers over the auxiliary DB: colour, tags, notes, ignore"),
    ("app/utils/button_factory.py", "ui", "skip", "standard button builders"),
    ("app/utils/constants.py", "domain", "port", "tier zero and one id lists, DLC table, sentinel package id, enums"),
    ("app/utils/csv_export_utils.py", "ui", "port", "CSV export of the visible mod table"),
    ("app/utils/custom_list_widget_item.py", "ui", "skip", "list item subclass"),
    ("app/utils/custom_list_widget_item_metadata.py", "ui", "skip", "per-row display data for the list widget"),
    ("app/utils/custom_qlabels.py", "ui", "skip", "clickable labels"),
    ("app/utils/db_builder.py", "integration", "redesign", "Qt wrapper over the Workshop database builder"),
    ("app/utils/db_builder_core.py", "domain", "port", "Qt-free core of the Workshop database builder"),
    ("app/utils/dds_utility.py", "integration", "skip", "DDS texture file helpers"),
    ("app/utils/dict_utils.py", "domain", "skip", "recursive dict update"),
    ("app/utils/event_bus.py", "ui", "skip", "global Qt signal hub"),
    ("app/utils/file_search.py", "integration", "crate", "walks mod folders for text and file name matches"),
    ("app/utils/files.py", "domain", "port", "C# and patch probes, saves backup, backup pruning"),
    ("app/utils/generic.py", "integration", "crate", "clipboard, rmtree, scandir wrappers, open folder, misc"),
    ("app/utils/git_utils.py", "integration", "crate", "pygit2 clone, pull, push, status and token helpers"),
    ("app/utils/git_worker.py", "integration", "crate", "Qt workers running git batches"),
    ("app/utils/github/", "integration", "redesign", "GitHub provider, installer, updater and workers for installing mods from repos"),
    ("app/utils/globals.py", "ui", "skip", "global flags"),
    ("app/utils/gui_info.py", "ui", "skip", "fonts and sizes"),
    ("app/utils/http.py", "integration", "crate", "HTTP with retries"),
    ("app/utils/http_downloader.py", "integration", "crate", "conditional HTTP downloads of datasets"),
    ("app/utils/ignore_extensions.py", "domain", "port", "default ignore patterns for file operations"),
    ("app/utils/ignore_manager.py", "domain", "port", "persisted set of package ids whose warnings are ignored"),
    ("app/utils/json_utils.py", "domain", "crate", "atomic JSON write with retry"),
    ("app/utils/launch_command_parser.py", "integration", "port", "parses Steam style %command% launch options"),
    ("app/utils/log_setup.py", "integration", "crate", "loguru configuration"),
    ("app/utils/mod_info.py", "ui", "skip", "view model for the mod info panel"),
    ("app/utils/mod_utils.py", "domain", "port", "resolves Steam update and touch timestamps for display and sort"),
    ("app/utils/platform/", "integration", "crate", "Windows directory entry helpers for reparse points"),
    ("app/utils/privatebin.py", "integration", "skip", "PrivateBin client for sharing lists"),
    ("app/utils/pygit2_loader.py", "integration", "skip", "SSL-safe pygit2 import"),
    ("app/utils/rentry/", "integration", "redesign", "Rentry.co upload client for list reports"),
    ("app/utils/schema.py", "domain", "port", "builds and validates ModsConfig dictionaries"),
    ("app/utils/single_instance.py", "integration", "crate", "single instance lock"),
    ("app/utils/startup_impact.py", "domain", "port", "parses the game's Loading Progress report for per-mod startup cost"),
    ("app/utils/steam/availability.py", "integration", "redesign", "detects the Steam executable and whether Steam runs"),
    ("app/utils/steam/db_builder_thread.py", "integration", "redesign", "thread that builds the Workshop database via the web API"),
    ("app/utils/steam/steambrowser/", "ui", "skip", "embedded Steam Workshop browser (web view)"),
    ("app/utils/steam/steamcmd/", "integration", "redesign", "SteamCMD install, login-free downloads, depot cache cleanup"),
    ("app/utils/steam/steamfiles/", "integration", "crate", "ACF to dict and back"),
    ("app/utils/steam/steamworks/", "integration", "redesign", "Steamworks client API wrapper for subscribe, unsubscribe, launch"),
    ("app/utils/steam/webapi/", "integration", "redesign", "Steam Web API queries for Workshop items and collections"),
    ("app/utils/steam/workshop_urls.py", "integration", "port", "Workshop search URL builder"),
    ("app/utils/steam/workshop_utils.py", "integration", "port", "Workshop update check result model and helpers"),
    ("app/utils/symlink.py", "integration", "crate", "symlink and junction creation"),
    ("app/utils/system_info.py", "integration", "crate", "OS and hardware info"),
    ("app/utils/todds/", "integration", "redesign", "texture optimiser runner"),
    ("app/utils/update_utils.py", "integration", "redesign", "application self-update download and swap, per OS"),
    ("app/utils/watchdog.py", "integration", "crate", "file system watcher raising create, delete and modify events for mod folders"),
    ("app/utils/win_find_steam.py", "integration", "redesign", "Windows registry lookup of the Steam folder"),
    ("app/utils/window_launch_state.py", "ui", "skip", "window geometry restore"),
    ("app/utils/xml.py", "domain", "redesign", "XML to nested dict conversion (ElementTree and bs4), dict to XML writer, gzip and zstd helpers"),
    ("app/utils/zip_extractor.py", "integration", "crate", "ZIP validation, extraction and backup"),
    ("app/views/acf_log_reader.py", "ui", "skip", "viewer for ACF workshop items"),
    ("app/views/deletion_menu.py", "ui", "port", "mod deletion choices: delete, unsubscribe, keep data"),
    ("app/views/mod_info_panel.py", "ui", "redesign", "details pane for the selected mod"),
    ("app/views/mods_panel.py", "ui", "redesign", "list widgets plus the warning and error calculation (domain logic buried in the widget)"),
    ("app/views/main_content_panel.py", "ui", "redesign", "central panel: refresh, sort, import, export, save, restore orchestration"),
    ("app/views/player_log_tab.py", "ui", "port", "game log viewer with pattern highlighting"),
    ("app/views/settings_dialog.py", "ui", "redesign", "settings dialog widgets"),
    ("app/views/", "ui", "skip", "Qt widget or dialog"),
    ("app/windows/rule_editor_panel.py", "ui", "redesign", "editor for community and user rules, drag and drop rule creation"),
    ("app/windows/missing_dependencies_dialog.py", "ui", "port", "dialog to add or download missing dependencies before sorting"),
    ("app/windows/use_this_instead_panel.py", "ui", "port", "panel listing replacement recommendations with install status"),
    ("app/windows/modlist_history_panel.py", "ui", "port", "browse and diff history snapshots"),
    ("app/windows/duplicate_mods_panel.py", "ui", "port", "choose which duplicate package id copy to keep"),
    ("app/windows/missing_mods_panel.py", "ui", "port", "prompt for mods listed in a list but not installed"),
    ("app/windows/missing_mod_properties_panel.py", "ui", "port", "list mods missing package id or published file id"),
    ("app/windows/", "ui", "redesign", "secondary window"),
]


def match(rel):
    for pat, layer, dec, text in RULES:
        if pat.endswith("/"):
            if rel.startswith(pat):
                return layer, dec, text
        elif rel == pat:
            return layer, dec, text
    if rel.endswith("__init__.py"):
        return "domain", "skip", "package marker"
    return "ui", "skip", "(unclassified)"


def main():
    root, tmpl, out = sys.argv[1], sys.argv[2], sys.argv[3]
    rows = []
    for dp, dn, fn in os.walk(os.path.join(root, "app")):
        dn.sort()
        for f in sorted(fn):
            if not f.endswith(".py"):
                continue
            p = os.path.join(dp, f)
            rel = os.path.relpath(p, root).replace(os.sep, "/")
            data = open(p, "rb").read()
            loc = data.count(b"\n")
            rows.append((rel, loc) + match(rel))
    rows.sort(key=lambda r: r[0])
    lines = ["| Path | LOC | Layer | Responsibility | RimStudio decision |", "|---|---:|---|---|---|"]
    for rel, loc, layer, dec, text in rows:
        lines.append(f"| {rel[4:]} | {loc} | {layer} | {text} | {dec} |")
    agg = defaultdict(lambda: [0, 0])
    for rel, loc, layer, dec, text in rows:
        agg[("layer", layer)][0] += 1
        agg[("layer", layer)][1] += loc
        agg[("dec", dec)][0] += 1
        agg[("dec", dec)][1] += loc
    s = ["| Group | Modules | LOC |", "|---|---:|---:|"]
    for k in sorted(agg):
        s.append(f"| {k[0]}: {k[1]} | {agg[k][0]} | {agg[k][1]} |")
    s.append(f"| total | {len(rows)} | {sum(r[1] for r in rows)} |")
    txt = open(tmpl, encoding="utf-8").read()
    txt = txt.replace("{{TABLE}}", "\n".join(lines)).replace("{{SUMMARY}}", "\n".join(s))
    open(out, "w", encoding="utf-8").write(txt)
    unclassified = [r[0] for r in rows if r[4] == "(unclassified)"]
    print(len(rows), "modules", sum(r[1] for r in rows), "LOC; unclassified:", unclassified)


if __name__ == "__main__":
    main()
