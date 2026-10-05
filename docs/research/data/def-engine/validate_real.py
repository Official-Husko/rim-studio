#!/usr/bin/env python3
"""Validate the engine on real data: vanilla (Core + 5 DLC) and vanilla + Combat Extended.

Usage (all paths can also come from the environment):
  validate_real.py --game DIR --ce DIR [--ce-dll FILE] [--out golden/real_data_summary.json] [--timings FILE]
  RIMWORLD_DIR, CE_DIR, CE_DLL are the environment fallbacks.

Prints and optionally writes a deterministic summary (counts, content hashes). Timings go to a separate
file because they vary per machine. The CE def type table is built from the compiled CE assembly with
build_type_table.py (needs `monodis`); the table is written to a temporary folder and not kept.
Nothing from the game or from Combat Extended is stored: only counts, def names and SHA-256 hashes.
"""
import argparse
import collections
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from defengine.engine import LoadConfig, load_game        # noqa: E402
from defengine.plugins import ce_custom_ops               # noqa: E402
from defengine.xmlnet import child_elements, inner_text, to_canonical  # noqa: E402

DLC = "Core Royalty Ideology Biotech Anomaly Odyssey".split()


def digest(res, type_name="ThingDef"):
    h = hashlib.sha256()
    for d in res.database(type_name).defs:
        h.update(json.dumps([d.type_name, d.def_name, d.mod_id, d.file, to_canonical(d.node)], sort_keys=True).encode())
    return h.hexdigest()


def summarize(res, ce_mod_id=None):
    out = {"mods": [m.package_id for m in res.mods], "stats": res.stats,
           "diagnostics": {k: v["count"] for k, v in res.diag.summary().items()},
           "databases": {}, "patch_ops": {}}
    for t in ("ThingDef", "Verse.BuildableDef"):
        db = res.database(t)
        out["databases"][t] = {"defs": len(db.defs), "overridden": len(db.overridden), "duplicates_skipped": len(db.skipped_same_mod),
                               "sha256": digest(res, t) if t == "ThingDef" else None}
    out["thingdefs_by_mod"] = dict(sorted(collections.Counter(d.mod_id for d in res.database("ThingDef").defs).items()))
    by = collections.Counter()
    for e in res.patch_events:
        by[(e.mod, e.op_class, "applied" if e.result else ("exception" if e.error else "failed"))] += 1
    out["patch_ops"] = {"|".join(k): v for k, v in sorted(by.items())}
    out["patch_ops_total"] = len(res.patch_events)
    out["patch_ops_applied"] = sum(1 for e in res.patch_events if e.result)
    return out


def make_types_table(dll_game, dll_ce, tmp):
    out = Path(tmp) / "types.json"
    cmd = [sys.executable, str(HERE / "build_type_table.py"), "--dll", str(dll_game)]
    if dll_ce:
        cmd += ["--dll", str(dll_ce)]
    cmd += ["--out", str(out)]
    subprocess.run(cmd, check=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"), stderr=subprocess.DEVNULL)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--game", default=os.environ.get("RIMWORLD_DIR"))
    ap.add_argument("--ce", default=os.environ.get("CE_DIR"))
    ap.add_argument("--ce-dll", default=os.environ.get("CE_DLL"))
    ap.add_argument("--out")
    ap.add_argument("--timings")
    a = ap.parse_args()
    game = Path(a.game)
    core = [game / "Data" / d for d in DLC]
    result, timings = {}, {}
    with tempfile.TemporaryDirectory() as tmp:
        van_types = HERE / "data" / "def_types_vanilla.json"
        r = load_game(LoadConfig(mods=core, game_dir=game, types=van_types))
        result["vanilla"] = summarize(r)
        timings["vanilla"] = {k: round(v, 3) for k, v in r.timings.items()}
        if a.ce:
            dll_game = next(game.glob("*_Data/Managed/Assembly-CSharp.dll"))
            types = make_types_table(dll_game, a.ce_dll, tmp) if a.ce_dll else van_types
            r = load_game(LoadConfig(mods=core + [Path(a.ce)], game_dir=game, types=types, custom_ops=ce_custom_ops()))
            s = summarize(r)
            ce_id = next(m.package_id for m in r.mods if m.package_id.lower() == "ceteam.combatextended")
            ce_ops = [e for e in r.patch_events if e.mod == ce_id]
            s["ce"] = {"package_id": ce_id, "load_folders": [str(f.path.relative_to(a.ce)) for f in next(m for m in r.mods if m.package_id == ce_id).folders],
                       "patch_ops": len(ce_ops), "applied": sum(e.result for e in ce_ops), "not_applied": sum(not e.result for e in ce_ops),
                       "not_applied_classes": dict(collections.Counter(e.op_class for e in ce_ops if not e.result)),
                       "make_gun_ops_distinct_defnames": len({inner_text(c) for e in ce_ops if e.raw is not None
                                                              for c in child_elements(e.raw) if c.tag == "defName"})}
            thing_vanilla = {d.def_name for d in r.database("ThingDef").defs if d.mod_id != ce_id}
            s["ce"]["ce_defs_by_type"] = dict(collections.Counter(d.type_name for d in r.defs if d.mod_id == ce_id).most_common(8))
            s["ce"]["ammo_defs"] = len(r.database("CombatExtended.AmmoDef").defs) if "CombatExtended.AmmoDef" in (r.types.types if r.types else {}) else None
            result["vanilla_plus_ce"] = s
            timings["vanilla_plus_ce"] = {k: round(v, 3) for k, v in r.timings.items()}
    text = json.dumps(result, indent=1, sort_keys=True)
    print(text)
    if a.out:
        Path(a.out).write_text(text + "\n", encoding="utf-8")
    if a.timings:
        Path(a.timings).write_text(json.dumps(timings, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
