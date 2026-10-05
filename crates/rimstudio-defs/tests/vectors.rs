//! The research vectors as integration tests: one test per case, named after the case.
//!
//! The vectors are language neutral JSON (mods as files in strings, expectations as XML strings
//! and counts). Each test builds the mods in a temporary folder, reads them with the game's folder
//! rules through the support crate, runs `rimstudio_defs::load` and checks every expectation; a
//! failure prints the vector's `doc` sentence.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::Vector;
use rimstudio_defs::{LoadOutput, diag_codes};

fn split_key(key: &str) -> (&str, &str) {
    key.split_once('/').unwrap()
}

fn check(vector: &Vector) {
    let (prepared, out): (common::Prepared, LoadOutput) = common::run_vector(vector);
    let doc = &vector.doc;
    let exp = &vector.expect;
    for (key, xml) in &exp.defs {
        let (t, n) = split_key(key);
        let got = out
            .get(t, n)
            .unwrap_or_else(|| panic!("missing def {key}: {doc}"));
        let want = common::parse_fragment(xml);
        assert_eq!(got.node, want, "def {key} differs: {doc}");
    }
    for key in &exp.absent {
        let (t, n) = split_key(key);
        assert!(out.get(t, n).is_none(), "def {key} should be absent: {doc}");
    }
    for (key, want) in &exp.provenance {
        let (t, n) = split_key(key);
        let d = out
            .get(t, n)
            .unwrap_or_else(|| panic!("provenance: missing def {key}: {doc}"));
        if let Some(m) = &want.mod_id {
            let got = d
                .origin
                .and_then(|o| prepared.package_id(o.mod_idx))
                .map(str::to_owned);
            assert_eq!(&got, m, "provenance {key}.mod: {doc}");
        }
        if let Some(f) = &want.file {
            let got = d
                .origin
                .and_then(|o| prepared.file_path(o.file))
                .map(str::to_owned);
            assert_eq!(&got, f, "provenance {key}.file: {doc}");
        }
        if let Some(p) = &want.parents {
            let got: Vec<String> = d.parents.iter().map(|x| x.label(&out.order)).collect();
            assert_eq!(&got, p, "provenance {key}.parents: {doc}");
        }
    }
    if let Some(want) = &exp.patch_results {
        assert_eq!(&out.patch_report.results(), want, "patch_results: {doc}");
    }
    for (t, names) in &exp.db_order {
        let db = out.databases.database(t).unwrap();
        let got: Vec<String> = db.names().into_iter().map(str::to_owned).collect();
        assert_eq!(&got, names, "db_order {t}: {doc}");
    }
    for (legacy, n) in &exp.diag {
        let code = diag_codes::from_legacy(legacy).unwrap();
        assert_eq!(
            out.diagnostics.count(&code),
            *n,
            "diag {legacy}: {doc}\nall diagnostics: {:?}",
            out.diagnostics.counts
        );
    }
    for (k, want) in &exp.stats {
        let got = match k.as_str() {
            "defs" => out.stats.defs,
            "files" => out.stats.files,
            "abstract" => out.stats.abstract_nodes,
            "may_require_skipped" => out.stats.may_require_skipped,
            "unknown_type" => out.stats.unknown_type,
            "top_level_nodes" => out.stats.top_level_nodes,
            "patch_ops" => out.stats.patch_ops,
            other => panic!("unknown stat {other}"),
        };
        assert_eq!(u64::from(got), *want, "stats {k}: {doc}");
    }
}

fn run(name: &str) {
    let vectors = common::load_vectors();
    let v = vectors
        .iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("no vector named {name}"));
    check(v);
}

macro_rules! vector_tests {
    ($($fn_name:ident: $case:literal,)*) => {
        $(
            #[test]
            fn $fn_name() {
                run($case);
            }
        )*

        const COVERED: &[&str] = &[$($case),*];
    };
}

vector_tests! {
    merge_load_order_and_override: "merge-load-order-and-override",
    duplicate_in_same_mod: "duplicate-in-same-mod",
    load_folders_and_version_folders: "load-folders-and-version-folders",
    file_dedup_between_folders: "file-dedup-between-folders",
    defs_bad_root_parse_error_unknown_type: "defs-bad-root-parse-error-unknown-type",
    inherit_basic_merge: "inherit-basic-merge",
    inherit_allow_duplicate_nodes: "inherit-allow-duplicate-nodes",
    inherit_false_nested: "inherit-false-nested",
    inherit_false_root_keeps_parent_attrs: "inherit-false-root-keeps-parent-attrs",
    inherit_empty_element: "inherit-empty-element",
    inherit_text_over_multichild_quirk: "inherit-text-over-multichild-quirk",
    inherit_chain_three_levels: "inherit-chain-three-levels",
    inherit_cycle: "inherit-cycle",
    inherit_missing_parent: "inherit-missing-parent",
    inherit_parent_by_load_order: "inherit-parent-by-load-order",
    inherit_duplicate_name_same_mod: "inherit-duplicate-name-same-mod",
    inherit_duplicate_node_name_error: "inherit-duplicate-node-name-error",
    abstract_not_created: "abstract-not-created",
    class_attribute_and_type_names: "class-attribute-and-type-names",
    duplicates_across_types: "duplicates-across-types",
    mayrequire_top_level: "mayrequire-top-level",
    mayrequire_in_patch_lists: "mayrequire-in-patch-lists",
    patch_add_order: "patch-add-order",
    patch_insert_order: "patch-insert-order",
    patch_replace_remove: "patch-replace-remove",
    patch_replace_whole_def: "patch-replace-whole-def",
    patch_attribute_ops: "patch-attribute-ops",
    patch_setname: "patch-setname",
    patch_addmodextension: "patch-addmodextension",
    patch_conditional_test_sequence: "patch-conditional-test-sequence",
    patch_success_modes: "patch-success-modes",
    patch_findmod: "patch-findmod",
    patch_unknown_class: "patch-unknown-class",
    patch_xpath_forms: "patch-xpath-forms",
    patch_order_across_mods: "patch-order-across-mods",
    patch_created_nodes_have_no_mod: "patch-created-nodes-have-no-mod",
    patch_exceptions: "patch-exceptions",
    patch_class_case_tolerance: "patch-class-case-tolerance",
}

#[test]
fn every_vector_of_the_files_has_a_test() {
    let names: Vec<String> = common::load_vectors().into_iter().map(|v| v.name).collect();
    assert_eq!(
        names.len(),
        COVERED.len(),
        "the vector files and the test list differ"
    );
    for n in &names {
        assert!(COVERED.contains(&n.as_str()), "vector {n} has no test");
    }
}
