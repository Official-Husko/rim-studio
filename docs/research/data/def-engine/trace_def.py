#!/usr/bin/env python3
"""Print the raw inheritance chain of defs (own children of every level) next to the engine's resolved values.

Usage: trace_def.py --game DIR --types FILE [--mod PATH ...] ThingDef/Gun_AssaultRifle ...
It is the aid used for the hand traces in the semantics note: read the 'own' lines top-down (root parent
first) and apply the merge rules by hand, then compare with the 'resolved' lines.
"""
import argparse
import os
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from defengine.engine import LoadConfig, load_game          # noqa: E402
from defengine.xmlnet import child_elements, inner_text      # noqa: E402


def brief(el, depth=0):
    kids = child_elements(el)
    if not kids:
        return "<%s>%s" % (el.tag, inner_text(el).strip()[:40])
    if el.tag in ("statBases", "equippedStatOffsets", "costList", "apparel", "tools", "verbs", "comps", "weaponTags") or depth == 0:
        return "<%s>{%s}" % (el.tag, ", ".join(brief(k, depth + 1) for k in kids))
    return "<%s>(%d children)" % (el.tag, len(kids))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True)
    ap.add_argument("--types", required=True)
    ap.add_argument("--mod", action="append", default=[])
    ap.add_argument("--core-dirs", default="Core,Royalty,Ideology,Biotech,Anomaly,Odyssey")
    ap.add_argument("defs", nargs="+")
    a = ap.parse_args()
    g = Path(a.game)
    mods = [g / "Data" / x for x in a.core_dirs.split(",")] + [Path(m) for m in a.mod]
    res = load_game(LoadConfig(mods=mods, game_dir=g, types=a.types))
    for key in a.defs:
        t, n = key.split("/", 1)
        d = res.get(t, n)
        print("=== %s  (%s : %s)" % (key, d.mod_id, d.file))
        chain = []
        src = d.source
        chain.append(("own", src))
        for pn in d.parents:
            pass
        print("own   :", brief(d.source))
        print("parents:", " <- ".join(d.parents))
        print("resolved:", brief(d.node))


if __name__ == "__main__":
    main()
