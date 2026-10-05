//! The def explorer: search and resolve over a workspace session.
#![cfg(all(feature = "tool-defs", feature = "tool-designer"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use rimstudio_ipc_types::defs::{DefSearchRequest, DefsGetResolvedRequest};
use rimstudio_toolkit::defs::{resolve, search};

#[test]
fn search_pages_and_filters_by_type_and_mod() {
    let f = common_project::fixture(true);
    let all = search(
        &f.session,
        &DefSearchRequest {
            query: "RS_Gun".into(),
            query_id: "q1".into(),
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    assert_eq!(all.query_id, "q1");
    assert_eq!(all.total, 14);
    assert_eq!(all.items.len(), 14);
    let page = search(
        &f.session,
        &DefSearchRequest {
            query: "RS_Gun".into(),
            offset: 10,
            limit: 3,
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.offset, 10);
    // two types and the abstract bases
    let multi = search(
        &f.session,
        &DefSearchRequest {
            query: String::new(),
            def_types: vec!["DamageDef".into(), "StatDef".into()],
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    assert!(
        multi
            .items
            .iter()
            .all(|r| r.def_type == "DamageDef" || r.def_type == "StatDef")
    );
    assert!(multi.total >= 3);
    let bases = search(
        &f.session,
        &DefSearchRequest {
            query: "RS_Base".into(),
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    let hidden = search(
        &f.session,
        &DefSearchRequest {
            query: "RS_Base".into(),
            hide_abstract: true,
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    assert!(bases.total > hidden.total || bases.total == 0);
    let by_mod = search(
        &f.session,
        &DefSearchRequest {
            query: "RS_ModGun".into(),
            mod_ids: vec!["rs.extra".into()],
            ..DefSearchRequest::default()
        },
    )
    .unwrap();
    assert_eq!(by_mod.total, 1);
    assert_eq!(by_mod.items[0].mod_id, "rs.extra");
    assert!(
        by_mod.items[0].file.starts_with("Defs/"),
        "{}",
        by_mod.items[0].file
    );
}

#[test]
fn resolve_returns_the_def_tree_and_reports_a_missing_def() {
    let f = common_project::fixture(true);
    let def = resolve(
        &f.session,
        &DefsGetResolvedRequest {
            session_id: "s".into(),
            def_type: "ThingDef".into(),
            def_name: "RS_Gun00".into(),
        },
    )
    .unwrap();
    assert_eq!(def.def_name, "RS_Gun00");
    assert_eq!(def.tree["tag"], "ThingDef");
    assert!(def.patch_events.is_empty());
    let missing = resolve(
        &f.session,
        &DefsGetResolvedRequest {
            session_id: "s".into(),
            def_type: "ThingDef".into(),
            def_name: "RS_Nope".into(),
        },
    );
    assert!(missing.is_err());
}
