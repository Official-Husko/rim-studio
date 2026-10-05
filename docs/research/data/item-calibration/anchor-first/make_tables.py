#!/usr/bin/env python3
"""Print the per-stat result tables of results.json as markdown (used to write DESIGN.md)."""
import json
from pathlib import Path

R = json.load(open(Path(__file__).parent / "results.json"))
COLS = [("simple", "simple"), ("quiz_full", "quiz"), ("quiz_full_noisy", "quiz noisy"), ("tier_median", "tier med"),
        ("nn_kind_tier(oracle power)", "NN kind+tier"), ("role_tier_median", "role+tier med")]
for key, r in R["results"].items():
    sim = "simple_convert" if "convert" in key else "simple"
    cols = [(sim, "simple")] + COLS[1:] + ([("identity(vanilla value)", "identity")] if "convert" in key else [])
    if "convert" in key:
        cols = [c for c in cols if c[0] in (sim, "tier_median", "nn_kind_tier(oracle power)", "role_tier_median", "identity(vanilla value)")]
    print(f"\n### {key} (n = {r['n_items']}; questions: {r['questions']})\n")
    print("| stat | " + " | ".join(c[1] for c in cols) + " | band cover p50/p80 |")
    print("|" + "---|" * (len(cols) + 2))
    stats = [s for s in r["methods"][sim] if r["methods"][sim][s]]
    for s in stats:
        cells = []
        for m, _ in cols:
            v = r["methods"].get(m, {}).get(s)
            cells.append(f"{v['median']:.2f} / {v['p80']:.2f}" if v else "-")
        c = r["band_coverage"].get(s)
        print(f"| {s} (n={r['methods'][sim][s]['n']}) | " + " | ".join(cells) + f" | {c['cover_p50']:.2f}/{c['cover_p80']:.2f} |" if c else f"| {s} | " + " | ".join(cells) + " | - |")
print("\nMACRO", json.dumps(R["macro_median_of_stat_medians"], indent=0))
print("TWIN-FREE", json.dumps(R["twin_free"]["macro_median_of_stat_medians"], indent=0))
for k, r in R["results"].items():
    if r.get("elasticity_all_items"):
        print(k, r["elasticity_all_items"])
