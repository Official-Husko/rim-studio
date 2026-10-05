//! Micro benchmarks as ignored tests; run them in release mode:
//!
//! `cargo test -p rimstudio-defs --release --test bench -- --ignored --nocapture --test-threads=1`
//!
//! The corpus is generated (fictional names), about 13,000 defs in files of nine nodes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::time::Instant;

use rimstudio_defs::{LoadOutput, load};

fn report(label: &str, out: &LoadOutput, wall: std::time::Duration) {
    let t = &out.timings;
    println!(
        "{label}: files {} nodes {} defs {} patch ops {} (failed {}) | parse patches {} us, merge {} us, patch {} us, inherit {} us, defs {} us, databases {} us, load total {} us, wall {:?}",
        out.stats.files,
        out.stats.top_level_nodes,
        out.stats.defs,
        out.stats.patch_ops,
        out.patch_report.failed(),
        t.parse_patches,
        t.merge,
        t.patch,
        t.inherit,
        t.defs,
        t.databases,
        t.total,
        wall,
    );
}

#[test]
#[ignore = "benchmark: run with --release"]
fn vanilla_scale_load_stays_under_one_second() {
    let input = common::synth(13_000, 29);
    let copy = input.clone();
    let start = Instant::now();
    let out = load(copy);
    let wall = start.elapsed();
    report("vanilla scale", &out, wall);
    assert!(out.stats.defs >= 13_000);
    assert_eq!(out.diagnostics.errors, 0);
    if !cfg!(debug_assertions) {
        assert!(wall.as_millis() < 1000, "budget is 1 s, took {wall:?}");
    }
}

#[test]
#[ignore = "benchmark: run with --release"]
fn patch_heavy_load_stays_under_two_seconds() {
    let input = common::synth(13_000, 2_800);
    let copy = input.clone();
    let start = Instant::now();
    let out = load(copy);
    let wall = start.elapsed();
    report("patch heavy", &out, wall);
    assert_eq!(out.patch_report.events.len(), 2_800);
    assert_eq!(out.patch_report.failed(), 0);
    if !cfg!(debug_assertions) {
        assert!(wall.as_millis() < 2000, "budget is 2 s, took {wall:?}");
    }
}

#[test]
#[ignore = "benchmark: run with --release"]
fn thread_scaling_of_the_vanilla_scale_load() {
    let input = common::synth(13_000, 29);
    for threads in [1usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let copy = input.clone();
        let start = Instant::now();
        let out = pool.install(|| load(copy));
        report(&format!("{threads} thread(s)"), &out, start.elapsed());
    }
}
