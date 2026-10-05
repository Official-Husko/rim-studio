//! Compares the full output of every research vector with the prototype's golden file.
//!
//! The golden file holds, per vector, the counters, diagnostic counts, patch results and every
//! database with its defs (resolved node tree, type and provenance). Everything is synthetic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rimstudio_core::tree::Node;
use rimstudio_defs::diag_codes;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenProv {
    file: Option<String>,
    #[serde(rename = "mod")]
    mod_id: Option<String>,
    parents: Vec<String>,
    patched_by: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GoldenDef {
    #[serde(rename = "defName")]
    def_name: String,
    node: Node,
    provenance: GoldenProv,
    #[serde(rename = "type")]
    type_name: String,
}

#[derive(Debug, Deserialize)]
struct Golden {
    name: String,
    doc: String,
    stats: BTreeMap<String, u64>,
    diagnostics: BTreeMap<String, u64>,
    patch_results: Vec<bool>,
    databases: BTreeMap<String, Vec<GoldenDef>>,
}

fn goldens() -> Vec<Golden> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/vector_outputs.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn compare(g: &Golden) {
    let vectors = common::load_vectors();
    let v = vectors.iter().find(|v| v.name == g.name).unwrap();
    let (prepared, out) = common::run_vector(v);
    let doc = &g.doc;
    let stats = [
        ("defs", out.stats.defs),
        ("files", out.stats.files),
        ("abstract", out.stats.abstract_nodes),
        ("may_require_skipped", out.stats.may_require_skipped),
        ("unknown_type", out.stats.unknown_type),
        ("top_level_nodes", out.stats.top_level_nodes),
        ("patch_ops", out.stats.patch_ops),
    ];
    for (k, got) in stats {
        assert_eq!(Some(&u64::from(got)), g.stats.get(k), "stats {k}: {doc}");
    }
    for (legacy, want) in &g.diagnostics {
        let code = diag_codes::from_legacy(legacy).unwrap();
        assert_eq!(out.diagnostics.count(&code), *want, "diag {legacy}: {doc}");
    }
    // the golden counts were taken before the databases were built, so the database diagnostics
    // (duplicates inside one mod) are not part of them
    let golden_total: u64 = g.diagnostics.values().sum();
    assert_eq!(
        out.diagnostics.total() - out.diagnostics.count(&diag_codes::DUPLICATE_IN_MOD),
        golden_total,
        "total diagnostics: {doc}\n{:?}",
        out.diagnostics.counts
    );
    assert_eq!(
        out.patch_report.results(),
        g.patch_results,
        "patch results: {doc}"
    );
    for (db_type, defs) in &g.databases {
        let db = out.databases.database(db_type).unwrap();
        let got: Vec<_> = db.iter().collect();
        assert_eq!(got.len(), defs.len(), "database {db_type} size: {doc}");
        for (have, want) in got.iter().zip(defs) {
            let ctx = format!("{db_type}/{}: {doc}", want.def_name);
            assert_eq!(have.def_name, want.def_name, "{ctx}");
            assert_eq!(have.type_name, want.type_name, "type of {ctx}");
            assert_eq!(have.node, want.node, "node of {ctx}");
            let mod_id = have
                .origin
                .and_then(|o| prepared.package_id(o.mod_idx))
                .map(str::to_owned);
            assert_eq!(mod_id, want.provenance.mod_id, "mod of {ctx}");
            let file = have
                .origin
                .and_then(|o| prepared.file_path(o.file))
                .map(str::to_owned);
            assert_eq!(file, want.provenance.file, "file of {ctx}");
            let parents: Vec<String> = have.parents.iter().map(|p| p.label(&out.order)).collect();
            assert_eq!(parents, want.provenance.parents, "parents of {ctx}");
            let lookup = |f| prepared.file_path(f).map(str::to_owned);
            let patched: Vec<String> = have
                .patched_by
                .iter()
                .map(|p| p.label(&out.order, &lookup))
                .collect();
            assert_eq!(patched, want.provenance.patched_by, "patched_by of {ctx}");
        }
    }
}

#[test]
fn every_vector_matches_the_golden_output() {
    let all = goldens();
    assert_eq!(all.len(), 38);
    for g in &all {
        compare(g);
    }
}
