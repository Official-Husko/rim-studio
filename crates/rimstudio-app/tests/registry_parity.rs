//! Registry parity: every row of the registry has a handler, names are unique, and the registry matches
//! the rows of the command catalog for the 0.1.0 slice.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_app::registry::{self, CommandKind, ROUTES, Runner};

/// Rows the registry has that the catalog does not list yet (the catalog is owned by the docs pass).
const NOT_IN_CATALOG_YET: &[&str] = &["job_status"];

/// Catalog rows of the slice areas that the toolkit or manager do not implement yet.
const DEFERRED: &[&str] = &["designer_material_matrix"];

/// The areas the 0.1.0 slice covers, by command name prefix or exact name.
fn in_slice(name: &str) -> bool {
    const EXACT: &[&str] = &[
        "app_ping",
        "app_get_info",
        "app_list_tools",
        "cancel_job",
        "settings_get",
        "settings_update",
        "library_scan",
        "defs_search",
        "defs_get_resolved",
        "project_open",
        "project_create",
        "project_close",
        "project_tree",
        "project_layout_check",
        "project_scaffold_missing",
        "project_read_file",
    ];
    EXACT.contains(&name)
        || name.starts_with("detect_")
        || name.starts_with("sources_")
        || name.starts_with("designer_")
}

fn catalog() -> BTreeMap<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/architecture/command-catalog.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the catalog is read from {}: {e}", path.display()));
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // | `name` | kind | ... |
        let (Some(name), Some(kind)) = (cells.get(1), cells.get(2)) else {
            continue;
        };
        let Some(name) = name.strip_prefix('`').and_then(|n| n.strip_suffix('`')) else {
            continue;
        };
        if name
            .chars()
            .any(|c| !(c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
        {
            continue;
        }
        let kind = kind.split_whitespace().next().unwrap_or("").to_owned();
        if matches!(
            kind.as_str(),
            "query" | "action" | "job" | "stream" | "family"
        ) {
            rows.insert(name.to_owned(), kind);
        }
    }
    rows
}

#[test]
fn names_are_unique() {
    let names = registry::names();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len());
}

#[test]
fn every_row_has_a_runner_of_its_kind_and_names_its_handler() {
    for route in ROUTES {
        match (route.kind, route.runner) {
            (CommandKind::Job, Runner::Job(_))
            | (CommandKind::Query | CommandKind::Action, Runner::Sync(_)) => {}
            (kind, runner) => panic!("{}: {kind:?} with {runner:?}", route.name),
        }
        assert!(
            route.handler.contains("api"),
            "{} -> {}",
            route.name,
            route.handler
        );
        assert!(!route.request.is_empty() && !route.response.is_empty());
    }
}

#[test]
fn the_catalog_parses_and_has_the_slice_rows() {
    let rows = catalog();
    assert!(rows.len() > 100, "{} rows", rows.len());
    assert_eq!(rows.get("settings_get").map(String::as_str), Some("query"));
    assert_eq!(rows.get("library_scan").map(String::as_str), Some("job"));
}

#[test]
fn every_registry_row_is_in_the_catalog_with_the_same_kind() {
    let rows = catalog();
    for route in ROUTES {
        if NOT_IN_CATALOG_YET.contains(&route.name) {
            continue;
        }
        let kind = rows
            .get(route.name)
            .unwrap_or_else(|| panic!("{} is not a row of the catalog", route.name));
        assert_eq!(kind, route.kind.as_str(), "kind of {}", route.name);
    }
}

#[test]
fn every_slice_row_of_the_catalog_is_registered_or_explicitly_deferred() {
    let rows = catalog();
    let registered: BTreeSet<&str> = registry::names().into_iter().collect();
    for name in rows.keys().filter(|n| in_slice(n)) {
        let ok = registered.contains(name.as_str()) || DEFERRED.contains(&name.as_str());
        // rows the catalog proposes for later milestones are outside the slice even when the area matches
        let later = matches!(name.as_str(), "detect_get_report_history" | "sources_watch");
        assert!(
            ok || later,
            "{name} is a slice row of the catalog and is neither registered nor deferred"
        );
    }
}

#[test]
fn deferred_and_extra_lists_do_not_hide_registered_rows() {
    let registered: BTreeSet<&str> = registry::names().into_iter().collect();
    for name in DEFERRED {
        assert!(
            !registered.contains(name),
            "{name} is registered, remove it from DEFERRED"
        );
    }
    for name in NOT_IN_CATALOG_YET {
        assert!(registered.contains(name), "{name} is not registered");
    }
}

#[test]
fn the_description_lists_every_row() {
    let described = registry::describe();
    assert_eq!(described.as_array().map(Vec::len), Some(ROUTES.len()));
}
