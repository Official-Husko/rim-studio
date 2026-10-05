#!/usr/bin/env python3
"""Runs every test vector and writes golden/vector_outputs.json: the engine's resolved defs (canonical JSON tree with
provenance), database orders, patch results and diagnostic counts for the synthetic vectors. Deterministic; synthetic data only.
The Rust defs crate should reproduce this file (after mapping the JSON node shape) as a regression test.

Usage: make_golden.py [--out golden/vector_outputs.json]
"""
import argparse
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "tests"))
import harness  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(HERE / "golden" / "vector_outputs.json"))
    a = ap.parse_args()
    out = []
    for v in harness.load_vectors():
        res = harness.run_vector(v)
        types = sorted({d.type_name for d in res.defs})
        item = {"name": v["name"], "doc": v["doc"], "stats": res.stats, "diagnostics": {k: x["count"] for k, x in res.diag.summary().items()},
                "patch_results": [e.result for e in res.patch_events], "databases": {}}
        for t in types:
            if t == "Verse.Def":
                continue
            db = res.database(t)
            item["databases"][t] = [d.to_json() for d in db.defs]
        out.append(item)
    Path(a.out).write_text(json.dumps(out, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    print("wrote %s (%d vectors)" % (a.out, len(out)))


if __name__ == "__main__":
    main()
