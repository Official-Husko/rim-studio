//! The load output is byte identical whatever the number of worker threads.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_defs::{LoadInput, LoadOutput, load};

fn run_in_pool(threads: usize, input: &LoadInput) -> LoadOutput {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| load(input.clone()))
}

/// Everything of the output that must not depend on scheduling, as JSON text.
fn fingerprint(out: &LoadOutput) -> String {
    let value = serde_json::json!({
        "defs": &*out.defs,
        "stats": out.stats,
        "diagnostics": out.diagnostics,
        "events": out.patch_report.deterministic_events(),
        "order": out.order,
        "dbTypes": out.databases.type_names(),
        "dbThing": out.databases.database("RS_ThingDef").unwrap().names(),
        "overrides": out.databases.overrides().len(),
    });
    serde_json::to_string(&value).unwrap()
}

#[test]
fn one_and_eight_threads_give_identical_output() {
    let input = common::synth(1500, 400);
    let one = run_in_pool(1, &input);
    let eight = run_in_pool(8, &input);
    assert!(one.stats.defs > 1000);
    assert_eq!(one.patch_report.events.len(), 400);
    assert_eq!(fingerprint(&one), fingerprint(&eight));
}

#[test]
fn repeated_runs_give_identical_output() {
    let input = common::synth(300, 60);
    let a = fingerprint(&run_in_pool(4, &input));
    let b = fingerprint(&run_in_pool(4, &input));
    assert_eq!(a, b);
}

#[test]
fn the_synthetic_corpus_loads_without_error_diagnostics() {
    let out = load(common::synth(600, 100));
    assert_eq!(out.diagnostics.errors, 0, "{:?}", out.diagnostics.counts);
    assert_eq!(out.patch_report.failed(), 0);
    assert!(out.stats.abstract_nodes > 0);
    assert_eq!(out.stats.unknown_type, 0);
}
