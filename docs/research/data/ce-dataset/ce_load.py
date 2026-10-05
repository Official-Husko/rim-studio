#!/usr/bin/env python3
"""Load RimWorld (Core + DLC) and Combat Extended with the def-engine prototype and return resolved defs.

Needs: the def-engine folder (DEF_ENGINE_DIR or --engine), the RimWorld install (RIMWORLD_DIR), the Combat Extended
source tree (CE_DIR). Nothing is read from the network. The CE def class table is built here from the C# sources
(there is no compiled CE assembly in the source tree), and CombatExtended.PatchOperationMakeGunCECompatible, a runtime
operation the engine cannot run by itself, is re-implemented in ce_ops.py from its documented behaviour.
Nothing from CE is copied into this repository: only derived numbers (see build_dataset.py).
"""
import json
import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ENGINE = Path(os.environ.get("DEF_ENGINE_DIR", HERE.parent / "def-engine"))
sys.path.insert(0, str(ENGINE))
sys.path.insert(0, str(HERE))

from defengine.engine import LoadConfig, load_game          # noqa: E402
from defengine.plugins import ce_custom_ops                  # noqa: E402
import ce_ops                                                # noqa: E402

DLC = "Core Royalty Ideology Biotech Anomaly Odyssey".split()


def ce_type_table(ce_dir: Path, out: Path):
    """Vanilla def type table plus every CE class deriving from a Def (found by scanning the C# sources)."""
    tbl = json.loads((ENGINE / "data" / "def_types_vanilla.json").read_text())
    found = {}
    for p in (ce_dir / "Source" / "CombatExtended").rglob("*.cs"):
        t = p.read_text(encoding="utf-8-sig", errors="replace")
        ns = re.search(r"^namespace\s+([\w.]+)", t, re.M)
        for m in re.finditer(r"class\s+(\w+)\s*(?:<[^>]*>)?\s*:\s*([\w.]+)", t):
            found[(ns.group(1) if ns else "", m.group(1))] = m.group(2)
    # resolve bases iteratively: vanilla names, or other CE classes
    known = {full: v for full, v in tbl["types"].items()}
    short = {}
    for full in known:
        short.setdefault(full.split(".")[-1], full)
    added = True
    while added:
        added = False
        for (ns, name), base in sorted(found.items()):
            full = f"{ns}.{name}" if ns else name
            if full in known:
                continue
            b = base.split(".")[-1]
            bfull = short.get(b)
            if bfull is None:
                for (ns2, n2) in found:
                    if n2 == b and (f"{ns2}.{n2}" in known):
                        bfull = f"{ns2}.{n2}"
            if bfull is not None:
                known[full] = {"base": bfull, "abstract": False, "assembly": "CombatExtended"}
                short.setdefault(name, full)
                added = True
    tbl["types"] = known
    tbl["assemblies"] = tbl["assemblies"] + ["CombatExtended"]
    out.write_text(json.dumps(tbl))
    return out


def load(game: Path, ce: Path, tmp: Path, with_ce=True):
    core = [game / "Data" / d for d in DLC]
    if not with_ce:
        return load_game(LoadConfig(mods=core, game_dir=game, types=ENGINE / "data" / "def_types_vanilla.json"))
    types = ce_type_table(ce, tmp / "types_ce.json")
    ops = ce_custom_ops()
    ops.update(ce_ops.custom_ops())
    return load_game(LoadConfig(mods=core + [ce], game_dir=game, types=types, custom_ops=ops))


if __name__ == "__main__":
    import argparse
    import tempfile
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default=os.environ.get("RIMWORLD_DIR"))
    ap.add_argument("--ce", default=os.environ.get("CE_DIR"))
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as t:
        r = load(Path(a.game), Path(a.ce), Path(t))
    print(r.stats)
    print(r.diag.summary() if hasattr(r.diag, "summary") else "")
    print(sum(1 for e in r.patch_events if e.result), "of", len(r.patch_events), "ops applied")
