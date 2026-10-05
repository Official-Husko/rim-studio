//! Architecture guards for the design crate: no XML dependency (IT-050) and no CE value table.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

fn manifest() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap()
}

#[test]
fn the_design_crate_has_no_xml_dependency() {
    let text = manifest();
    for banned in [
        "quick-xml",
        "rimstudio-xml",
        "xmltree",
        "roxmltree",
        "xml-rs",
    ] {
        assert!(
            !text.contains(banned),
            "the design crate must not depend on {banned}"
        );
    }
}

#[test]
fn the_design_crate_does_not_depend_on_tauri_or_the_shell() {
    let text = manifest();
    for banned in ["tauri", "rimstudio-app", "rimstudio-toolkit"] {
        assert!(!text.contains(banned), "{banned}");
    }
}
