//! Acceptance tests over a real install. They only read, are `#[ignore]` and need environment
//! variables:
//!
//! - `RIMSTUDIO_GAME_DIR`: the RimWorld install folder (holds `Version.txt` and `Data/`);
//! - `RIMSTUDIO_CE_DIR` (optional): a Combat Extended mod folder, for the patch heavy load.
//!
//! Run with
//! `cargo test -p rimstudio-defs --release --test real_data -- --ignored --nocapture --test-threads=1`.
//!
//! The def type table and the hand traces are read at run time from `docs/research/data` when
//! present; nothing from an install or from those files is copied into the repository, and the
//! tests assert structure and print counts.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rimstudio_core::tree::ArenaDoc;
use rimstudio_defs::{LoadOutput, TypeTable, load};
use rimstudio_xpath::{Value, XNode, XPath};
use serde::Deserialize;

const OFFICIAL_PACKS: [&str; 6] = [
    "Core", "Royalty", "Ideology", "Biotech", "Anomaly", "Odyssey",
];

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("RIMSTUDIO_GAME_DIR")?);
    dir.join("Version.txt").is_file().then_some(dir)
}

fn research_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data/def-engine")
        .join(rel)
}

fn type_table() -> Option<Arc<TypeTable>> {
    let text = fs::read_to_string(research_file("data/def_types_vanilla.json")).ok()?;
    Some(Arc::new(TypeTable::from_json_str(&text).unwrap()))
}

fn game_version(dir: &std::path::Path) -> String {
    fs::read_to_string(dir.join("Version.txt"))
        .unwrap()
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn official_roots(dir: &std::path::Path) -> Vec<PathBuf> {
    OFFICIAL_PACKS
        .iter()
        .map(|p| dir.join("Data").join(p))
        .filter(|p| p.is_dir())
        .collect()
}

fn load_vanilla() -> Option<(common::Prepared, LoadOutput)> {
    let Some(dir) = game_dir() else {
        println!("RIMSTUDIO_GAME_DIR is not set: skipped");
        return None;
    };
    let Some(types) = type_table() else {
        println!("docs/research/data/def-engine/data/def_types_vanilla.json is missing: skipped");
        return None;
    };
    let roots = official_roots(&dir);
    let t = Instant::now();
    let prepared = common::prepare(&roots, &game_version(&dir), &[], types);
    println!(
        "read and parsed {} defs files and {} patch files in {:?}",
        prepared.input.def_files.len(),
        prepared.input.patch_files.len(),
        t.elapsed()
    );
    let t = Instant::now();
    let out = load(prepared.input.clone());
    println!("load took {:?}: {:?}", t.elapsed(), out.timings);
    Some((prepared, out))
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn the_official_packs_load_without_error_diagnostics() {
    let Some((_, out)) = load_vanilla() else {
        return;
    };
    println!(
        "stats {:?}\nthing defs {}\nproblems {:?}",
        out.stats,
        out.databases.database("ThingDef").map_or(0, |d| d.len()),
        out.diagnostics.counts
    );
    assert!(out.stats.defs > 0);
    assert!(out.stats.files > 0);
    assert!(!out.databases.database("ThingDef").unwrap().is_empty());
    assert_eq!(out.diagnostics.errors, 0, "{:?}", out.diagnostics.samples);
    assert_eq!(out.stats.unknown_type, 0);
    assert_eq!(
        out.patch_report.applied(),
        out.patch_report.events.len(),
        "every vanilla patch operation finds its target"
    );
    let things: Vec<_> = out
        .databases
        .overrides()
        .into_iter()
        .filter(|o| o.db_type.ends_with("ThingDef"))
        .collect();
    assert!(things.is_empty(), "vanilla redefines no ThingDef");
    println!(
        "overrides over all databases: {}",
        out.databases.overrides().len()
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn the_counts_match_the_research_summary_for_the_same_game_version() {
    let Some((_, out)) = load_vanilla() else {
        return;
    };
    let Ok(text) = fs::read_to_string(research_file("golden/real_data_summary.json")) else {
        println!("no research summary: skipped");
        return;
    };
    let golden: serde_json::Value = serde_json::from_str(&text).unwrap();
    let stats = &golden["vanilla"]["stats"];
    let want = |k: &str| stats[k].as_u64().unwrap_or(u64::MAX);
    println!("research summary: {stats}");
    println!("this load: {:?}", out.stats);
    let dir = game_dir().unwrap();
    if !game_version(&dir).starts_with("1.6.4871") {
        println!("another game version: counts printed, not compared");
        return;
    }
    assert_eq!(u64::from(out.stats.files), want("files"));
    assert_eq!(
        u64::from(out.stats.top_level_nodes),
        want("top_level_nodes")
    );
    assert_eq!(u64::from(out.stats.abstract_nodes), want("abstract"));
    assert_eq!(u64::from(out.stats.defs), want("defs"));
    assert_eq!(u64::from(out.stats.patch_ops), want("patch_ops"));
}

#[derive(Debug, Deserialize)]
struct Check {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    count: Option<String>,
    #[serde(default)]
    texts: Option<String>,
    #[serde(default)]
    names: Option<String>,
    #[serde(default)]
    attrs: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    expect: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Trace {
    def: String,
    #[serde(rename = "mod")]
    mod_id: String,
    #[serde(default)]
    parents: Option<Vec<String>>,
    checks: Vec<Check>,
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and docs/research/data/def-engine/tests/handtrace_vanilla.json"]
fn the_hand_traces_of_vanilla_defs_hold() {
    let Some((prepared, out)) = load_vanilla() else {
        return;
    };
    let Ok(text) = fs::read_to_string(research_file("tests/handtrace_vanilla.json")) else {
        println!("no hand trace file: skipped");
        return;
    };
    let traces: Vec<Trace> = serde_json::from_str(&text).unwrap();
    assert!(traces.len() >= 12);
    for t in &traces {
        let (ty, name) = t.def.split_once('/').unwrap();
        let d = out
            .get(ty, name)
            .unwrap_or_else(|| panic!("missing {}", t.def));
        let owner = d.origin.and_then(|o| prepared.package_id(o.mod_idx));
        assert_eq!(owner, Some(t.mod_id.as_str()), "{}", t.def);
        if let Some(p) = &t.parents {
            let got: Vec<String> = d.parents.iter().map(|x| x.label(&out.order)).collect();
            assert_eq!(&got, p, "{}", t.def);
        }
        let doc = ArenaDoc::from_node(&d.node, rimstudio_core::tree::OriginId::NONE).unwrap();
        let el = doc.document_element().unwrap();
        let text_of = |x: XNode| match x {
            XNode::Node(id) => doc
                .string_value(id)
                .map(|s| s.into_owned())
                .unwrap_or_default(),
            XNode::Attr { .. } => String::new(),
        };
        for c in &t.checks {
            let ctx = format!("{} {c:?}", t.def);
            if let Some(path) = &c.text {
                let got = d
                    .node
                    .find(path)
                    .map(rimstudio_core::tree::Node::text_content);
                assert_eq!(got.as_deref(), c.expect.as_str(), "{ctx}");
            } else if let Some(expr) = &c.count {
                let v = XPath::parse(&format!("count({expr})"))
                    .unwrap()
                    .eval(&doc, el)
                    .unwrap();
                let n = match v {
                    Value::Number(n) => n,
                    other => panic!("{ctx}: {other:?}"),
                };
                assert_eq!(n, c.expect.as_f64().unwrap(), "{ctx}");
            } else if let Some(expr) = &c.texts {
                let nodes = XPath::parse(expr).unwrap().select_nodes(&doc, el).unwrap();
                let got: Vec<String> = nodes.into_iter().map(text_of).collect();
                let want: Vec<String> = serde_json::from_value(c.expect.clone()).unwrap();
                assert_eq!(got, want, "{ctx}");
            } else if let Some(expr) = &c.names {
                let nodes = XPath::parse(expr).unwrap().select_nodes(&doc, el).unwrap();
                let got: Vec<String> = nodes
                    .into_iter()
                    .filter_map(XNode::node_id)
                    .filter_map(|id| doc.name(id).map(str::to_owned))
                    .collect();
                let want: Vec<String> = serde_json::from_value(c.expect.clone()).unwrap();
                assert_eq!(got, want, "{ctx}");
            } else if let Some(attrs) = &c.attrs {
                for (k, v) in attrs {
                    assert_eq!(d.node.attr(k), Some(v.as_str()), "{ctx}");
                }
            }
        }
    }
    println!("{} hand traces hold", traces.len());
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn the_official_packs_plus_combat_extended_apply_their_patches_quickly() {
    let (Some(dir), Some(ce)) = (
        game_dir(),
        std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let Some(types) = type_table() else {
        println!("no def type table: skipped");
        return;
    };
    let mut roots = official_roots(&dir);
    roots.push(ce);
    let prepared = common::prepare(&roots, &game_version(&dir), &[], types);
    let t = Instant::now();
    let out = load(prepared.input.clone());
    let wall = t.elapsed();
    println!(
        "defs files {} patch files {} | load {:?} | {:?}\nstats {:?}\napplied {} of {} patch operations\nproblems {:?}",
        prepared.input.def_files.len(),
        prepared.input.patch_files.len(),
        wall,
        out.timings,
        out.stats,
        out.patch_report.applied(),
        out.patch_report.events.len(),
        out.diagnostics.counts
    );
    // the type table lacks the mod's own def classes, so those defs are reported, not created
    assert!(out.patch_report.events.len() > 1000);
    assert!(out.stats.defs > 0);
    let unknown: Vec<_> = out
        .patch_report
        .events
        .iter()
        .filter(|e| e.raw.is_some())
        .map(|e| e.class.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    println!("operation classes kept as unknown: {unknown:?}");
    let mut slow: Vec<(u64, &str)> = out
        .patch_report
        .op_micros
        .iter()
        .copied()
        .zip(
            out.patch_report
                .events
                .iter()
                .map(|e| e.description.as_str()),
        )
        .collect();
    slow.sort_by_key(|s| std::cmp::Reverse(s.0));
    for (us, what) in slow.iter().take(8) {
        println!("slow patch operation: {us} us {what}");
    }
    let total: u64 = out.patch_report.op_micros.iter().sum();
    let over_1ms = out
        .patch_report
        .op_micros
        .iter()
        .filter(|&&u| u > 1000)
        .count();
    println!("patch operations over 1 ms: {over_1ms}, summed {total} us");
}
