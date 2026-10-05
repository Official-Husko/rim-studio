#!/bin/bash
# Usage: run_all.sh <results dir>   (builds into $CARGO_TARGET_DIR, default /tmp/perf-spike-target)
# Roots default to the owner's machine; override with SPIKE_ROOT_WORKSHOP, SPIKE_ROOT_DATA, SPIKE_ROOT_LOCALMODS,
# SPIKE_ROOT_OWNER, SPIKE_ROOT_CE. Set SPIKE_GATE_MS=0 to skip the "wait for a quiet machine" gate.
set -e
export PYTHONDONTWRITEBYTECODE=1 CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/tmp/perf-spike-target}
HERE=$(cd "$(dirname "$0")" && pwd); R=${1:?results dir}; mkdir -p "$R"
cargo build --release --manifest-path "$HERE/Cargo.toml"
B=$CARGO_TARGET_DIR/release/rimstudio-perf-spike
$B s1 --roots workshop,data,localmods,owner,ce --threads 1,4,16 --runs 5 --out $R/s1.jsonl > $R/s1.txt
$B s2 --runs 5 --out $R/s2.jsonl > $R/s2.txt
$B s2-robust > $R/s2robust.txt
$B s3 --runs 5 --threads 1,16 --out $R/s3.jsonl > $R/s3.txt
$B s4 --dir /tmp/spike-cache --runs 5 --out $R/s4.jsonl > $R/s4.txt
$B s5 --runs 3 > $R/s5.txt
$B s6 --runs 5 --out $R/s6.jsonl > $R/s6.txt
$B s6-about --threads 1 --runs 7 > $R/s6about.txt; $B s6-about --threads 16 --runs 7 >> $R/s6about.txt
python3 "$HERE/pitfalls.py" "${SPIKE_ROOT_WORKSHOP:-$HOME/.steam/steam/steamapps/workshop/content/294100}" > $R/pitfalls.txt 2>&1 || true
rm -rf /tmp/spike-cache
