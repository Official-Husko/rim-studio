//! The link commands through the registry, on the fictional install with fake ports (the fake link
//! backend creates no entry on disk, so the real link cycle is in the library tests and in the ignored
//! `real_link` test).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::{fixture, project_folder, select_install};
use rimstudio_app::dispatch_blocking;
use serde_json::{Value, json};

fn open(f: &common::Fixture, path: &camino::Utf8Path) -> String {
    let out: Value =
        dispatch_blocking(&f.app, "project_open", json!({"path": path.as_str()})).unwrap();
    out["projectId"].as_str().unwrap().to_owned()
}

#[test]
fn without_a_selected_install_there_is_nothing_to_link_into() {
    let f = fixture(false);
    let path = project_folder(&f, "RS_Linkable", &[]);
    let id = open(&f, &path);
    let status =
        dispatch_blocking(&f.app, "project_link_status", json!({"projectId": id})).unwrap();
    assert_eq!(status["gameFound"], false, "{status}");
    assert_eq!(status["state"], "unavailable");
    assert_eq!(status["canCreate"], false);
    let made = dispatch_blocking(&f.app, "project_link_create", json!({"projectId": id})).unwrap();
    assert_eq!(made["done"], false);
    assert_eq!(made["refusal"]["code"], "deploy.game-not-found");
}

#[test]
fn with_an_install_the_status_names_the_entry_and_the_manual_command() {
    let f = fixture(false);
    select_install(&f);
    let path = project_folder(&f, "RS_Linkable", &[]);
    let id = open(&f, &path);
    let status =
        dispatch_blocking(&f.app, "project_link_status", json!({"projectId": id})).unwrap();
    assert_eq!(status["gameFound"], true, "{status}");
    assert_eq!(status["linkName"], "RS_Linkable");
    assert!(
        status["entryPath"]
            .as_str()
            .unwrap()
            .ends_with("Mods/RS_Linkable")
    );
    assert_eq!(status["gameRunning"], "not-running");
    assert!(
        status["manualCommand"]
            .as_str()
            .unwrap()
            .starts_with("ln -s ")
    );
}

#[test]
fn an_unknown_project_is_an_error() {
    let f = fixture(false);
    let err = dispatch_blocking(
        &f.app,
        "project_link_status",
        json!({"projectId": "p-00000000"}),
    )
    .unwrap_err();
    assert_eq!(err.code, "project.not-open");
}

#[test]
fn a_request_that_is_not_an_object_is_refused_not_acted_on() {
    let f = fixture(false);
    for name in [
        "project_link_status",
        "project_link_create",
        "project_link_remove",
    ] {
        assert!(
            dispatch_blocking(&f.app, name, json!("RS")).is_err(),
            "{name}"
        );
    }
}
