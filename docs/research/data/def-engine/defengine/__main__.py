"""Command line: python -m defengine {load,def,patches} ...   (run from the def-engine folder or set PYTHONPATH)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .engine import LoadConfig, load_game
from .xmlnet import serialize


def _config(a) -> LoadConfig:
    game = Path(a.game)
    mods = []
    for sub in a.core_dirs.split(","):
        p = game / "Data" / sub
        if p.is_dir():
            mods.append(p)
    mods += [Path(m) for m in a.mod]
    return LoadConfig(mods=mods, game_dir=game, types=a.types, extra_active_ids=a.active or [])


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="defengine", description=__doc__)
    ap.add_argument("cmd", choices=["load", "def", "patches"])
    ap.add_argument("--game", required=True, help="RimWorld install folder (Version.txt, Data/)")
    ap.add_argument("--core-dirs", default="Core,Royalty,Ideology,Biotech,Anomaly,Odyssey",
                    help="Data/<name> folders loaded first, in this order (missing ones are skipped)")
    ap.add_argument("--mod", action="append", default=[], help="extra mod root folder, repeat in load order")
    ap.add_argument("--types", required=True, help="def type table JSON (build_type_table.py output)")
    ap.add_argument("--active", action="append", help="extra package id that counts as active")
    ap.add_argument("--def", dest="defname", help="TypeName/defName for the 'def' command")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    res = load_game(_config(a))
    if a.cmd == "load":
        out = {"mods": [m.package_id for m in res.mods], "stats": res.stats,
               "timings_s": {k: round(v, 3) for k, v in res.timings.items()}, "diagnostics": res.diag.summary()}
        print(json.dumps(out, indent=1))
    elif a.cmd == "def":
        t, n = a.defname.split("/", 1)
        d = res.get(t, n)
        if d is None:
            print("not found", file=sys.stderr)
            return 1
        if a.json:
            print(json.dumps(d.to_json(), indent=1))
        else:
            print(json.dumps(d.provenance(), indent=1))
            print(serialize(d.node))
    else:
        for e in res.patch_events:
            print("%s %s #%d %s -> %s%s" % (e.mod, e.file, e.index, e.description, e.result, (" [%s]" % e.error) if e.error else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
