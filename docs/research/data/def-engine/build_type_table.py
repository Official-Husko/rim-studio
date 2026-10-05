#!/usr/bin/env python3
"""Build the Def class table used by engine.py from compiled .NET assemblies.

The game resolves the XML element name (or the Class attribute) of every top-level
node to a C# type, and only types that derive from Verse.Def are accepted as defs.
engine.py has no way to run reflection, so this tool extracts the same facts
(type name, namespace, base type, abstract flag) from assembly metadata using the
`monodis` disassembler that ships with Mono. Only the metadata tables are read
(`--typedef` and `--typeref`), which takes well under a second per assembly.

Usage:
    build_type_table.py --dll PATH [--dll PATH ...] [--out FILE] [--root Verse.Def]

DLL order matters: it mirrors assembly load order (the game assembly first, then
mod assemblies in load order); for types whose short name is looked up in the
"ignored namespaces" index the later assembly wins, exactly like the game.

Output JSON (format 1):
    {
      "format": 1, "root": "Verse.Def",
      "assemblies": ["Assembly-CSharp", "CombatExtended"],
      "types": {"Verse.ThingDef": {"base": "Verse.BuildableDef", "abstract": false,
                                   "assembly": "Assembly-CSharp"}, ...},
      "short_name_collisions": {"Name": ["Ns1.Name", "Ns2.Name"], ...}
    }
`types` holds every type that is the root or derives from it. A base type that
lives in an assembly which was not scanned is recorded by name only.
"""
import argparse
import json
import os
import re
import subprocess
import sys

IGNORED_NAMESPACES = [
    "RimWorld", "Verse", "LudeonTK", "Verse.AI", "Verse.AI.Group", "Verse.Sound",
    "Verse.Grammar", "RimWorld.Planet", "RimWorld.BaseGen", "RimWorld.QuestGen",
    "RimWorld.SketchGen", "System",
]

TYPEDEF_RE = re.compile(r"^(\d+): (.*?) \(flist=\d+, mlist=\d+, flags=0x([0-9a-fA-F]+), extends=0x([0-9a-fA-F]+)\)$")
TYPEREF_RE = re.compile(r"^(\d+): (?:\[(?:'([^']*)'|([^\]]*))\])?(.*)$")


def run_monodis(flag, dll):
    out = subprocess.run(["monodis", flag, dll], capture_output=True, text=True, check=False)
    if out.returncode != 0 or not out.stdout:
        raise SystemExit("monodis %s failed for %s: %s" % (flag, dll, out.stderr.strip()))
    return out.stdout.splitlines()


def read_assembly(dll):
    typedefs = {}
    for line in run_monodis("--typedef", dll):
        m = TYPEDEF_RE.match(line.strip())
        if m:
            idx = int(m.group(1))
            typedefs[idx] = {"name": m.group(2), "flags": int(m.group(3), 16), "extends": int(m.group(4), 16)}
    typerefs = {}
    for line in run_monodis("--typeref", dll):
        m = TYPEREF_RE.match(line.strip())
        if m and m.group(4):
            typerefs[int(m.group(1))] = m.group(4)
    asm = os.path.splitext(os.path.basename(dll))[0]
    return asm, typedefs, typerefs


def base_name(entry, typedefs, typerefs):
    code = entry["extends"]
    tag, idx = code & 3, code >> 2
    if idx == 0:
        return None
    if tag == 0:
        t = typedefs.get(idx)
        return t["name"] if t else None
    if tag == 1:
        return typerefs.get(idx)
    return None  # TypeSpec (generic base): not a def base


def short_name(full):
    return full.rsplit("/", 1)[-1].rsplit(".", 1)[-1]


def namespace_of(full):
    outer = full.split("/", 1)[0]
    return outer.rsplit(".", 1)[0] if "." in outer else ""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dll", action="append", required=True, help="assembly path (repeat in load order)")
    ap.add_argument("--out", default="-", help="output JSON path (default stdout)")
    ap.add_argument("--root", default="Verse.Def")
    args = ap.parse_args()

    all_types = {}      # full name -> dict
    order = []          # insertion order for "later wins" semantics
    assemblies = []
    for dll in args.dll:
        asm, typedefs, typerefs = read_assembly(dll)
        assemblies.append(asm)
        for idx in sorted(typedefs):
            t = typedefs[idx]
            if not t["name"] or t["name"] == "(null)":
                continue
            all_types[t["name"]] = {
                "base": base_name(t, typedefs, typerefs),
                "abstract": bool(t["flags"] & 0x80),
                "assembly": asm,
            }
            order.append(t["name"])

    def derives(name):
        seen = set()
        while name and name not in seen:
            if name == args.root:
                return True
            seen.add(name)
            info = all_types.get(name)
            name = info["base"] if info else None
        return False

    def_types = {n: all_types[n] for n in order if derives(n)}
    # Short-name collisions in the ignored-namespace index, restricted to those that involve a def type.
    by_short = {}
    for n in order:
        ns = namespace_of(n)
        if ns == "" or ns in IGNORED_NAMESPACES:
            by_short.setdefault(short_name(n), []).append(n)
    collisions = {k: v for k, v in by_short.items() if len(v) > 1 and any(x in def_types for x in v)}

    doc = {
        "format": 1,
        "root": args.root,
        "assemblies": assemblies,
        "types": def_types,
        "short_name_collisions": collisions,
    }
    text = json.dumps(doc, indent=1, sort_keys=False)
    if args.out == "-":
        sys.stdout.write(text + "\n")
    else:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(text + "\n")
        print("wrote %s: %d def types from %d assemblies, %d short-name collisions involving defs"
              % (args.out, len(def_types), len(assemblies), len(collisions)), file=sys.stderr)


if __name__ == "__main__":
    main()
