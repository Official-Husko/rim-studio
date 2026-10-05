#!/usr/bin/env python3
"""Probe how RimSort's own typed schema and save path treat a real communityRules.json.

The script does not copy RimSort code. At run time it reads RimSort's source files
(read only), extracts the schema class definitions and the dict-merge helper with the
`ast` module, executes them in an isolated namespace and feeds them the dataset.

Usage:
    PYTHONPATH=<dir containing msgspec> PYTHONDONTWRITEBYTECODE=1 \
      rimsort_semantics_probe.py --rimsort-root <RimSort checkout> --rules-json <communityRules.json>

Output: one JSON document on stdout. Deterministic.
"""
from __future__ import annotations

import argparse
import ast
import copy
import json
import sys
import types
from pathlib import Path

import msgspec

WANTED_CLASSES = {
    "SubExternalRule",
    "SubExternalBoolRule",
    "ExternalRule",
    "ExternalRulesSchema",
    "SteamDbEntryDependency",
    "SteamDbEntryBlacklist",
    "SteamDbEntry",
    "SteamDbSchema",
}


def load_schema_namespace(rimsort_root: Path) -> dict:
    src = (rimsort_root / "app/models/metadata/metadata_structure.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    body = [n for n in tree.body if isinstance(n, ast.ClassDef) and n.name in WANTED_CLASSES]
    module = ast.Module(body=body, type_ignores=[])
    ast.fix_missing_locations(module)
    # msgspec resolves string annotations through sys.modules[cls.__module__], so the
    # extracted classes need a real (throwaway) module object to live in.
    mod = types.ModuleType("rimsort_schema_probe")
    mod.__dict__["msgspec"] = msgspec
    sys.modules[mod.__name__] = mod
    exec(compile(module, "<rimsort-schema>", "exec"), mod.__dict__)  # noqa: S102 - isolated, read-only probe
    return mod.__dict__


def load_dict_helper(rimsort_root: Path):
    src = (rimsort_root / "app/utils/dict_utils.py").read_text(encoding="utf-8")
    ns: dict = {}
    exec(compile(src, "<rimsort-dict_utils>", "exec"), ns)  # noqa: S102
    return ns["recursively_update_dict"]


def load_constants(rimsort_root: Path) -> dict:
    src = (rimsort_root / "app/utils/constants.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    wanted = {"DB_BUILDER_PRUNE_EXCEPTIONS", "DB_BUILDER_RECURSE_EXCEPTIONS"}
    out = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            t = node.targets[0]
            if isinstance(t, ast.Name) and t.id in wanted:
                out[t.id] = ast.literal_eval(node.value)
    return out


def count_edges(rules: dict, key: str) -> int:
    n = 0
    for entry in rules.values():
        v = entry.get(key)
        if isinstance(v, dict):
            n += len(v)
    return n


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rimsort-root", required=True)
    ap.add_argument("--rules-json", required=True)
    args = ap.parse_args()

    root = Path(args.rimsort_root)
    raw = Path(args.rules_json).read_bytes()
    original = json.loads(raw)
    ns = load_schema_namespace(root)
    update = load_dict_helper(root)
    consts = load_constants(root)

    out: dict = {"msgspec_version": msgspec.__version__}
    out["constants"] = consts

    # 1. typed decode (what RimSort loads)
    ExternalRulesSchema = ns["ExternalRulesSchema"]
    schema = msgspec.json.decode(raw, type=ExternalRulesSchema)
    schema.rules = {k.lower(): v for k, v in schema.rules.items()}  # same normalisation as its reader
    built = msgspec.to_builtins(schema.rules)
    out["typed_decode"] = {
        "entries_in_file": len(original["rules"]),
        "entries_after_lowercase_normalisation": len(schema.rules),
        "incompatibleWith_edges_in_file": count_edges(original["rules"], "incompatibleWith"),
        "incompatibleWith_edges_visible_after_typed_decode": count_edges(built, "incompatibleWith"),
        "loadAfter_edges_in_file": count_edges(original["rules"], "loadAfter"),
        "loadAfter_edges_after_typed_decode": count_edges(built, "loadAfter"),
        "loadBefore_edges_in_file": count_edges(original["rules"], "loadBefore"),
        "loadBefore_edges_after_typed_decode": count_edges(built, "loadBefore"),
        "unknown_field_rejected": False,
    }

    # strictness probe: does the typed decode reject an unknown top-level or rule-level field?
    probe = json.dumps({"timestamp": 1, "extraTop": 1, "rules": {"a.b": {"loadAfter": {}, "extraRule": {"x": 1}}}}).encode()
    try:
        msgspec.json.decode(probe, type=ExternalRulesSchema)
        out["typed_decode"]["unknown_field_rejected"] = False
    except msgspec.DecodeError as e:
        out["typed_decode"]["unknown_field_rejected"] = str(e)
    # type-mismatch probe: name as an object, timestamp as a string
    for label, doc in (
        ("timestamp_is_string", {"timestamp": "1", "rules": {}}),
        ("name_is_object", {"timestamp": 1, "rules": {"a.b": {"loadAfter": {"c.d": {"name": {"x": 1}}}}}}),
        ("rules_is_list", {"timestamp": 1, "rules": []}),
    ):
        try:
            msgspec.json.decode(json.dumps(doc).encode(), type=ExternalRulesSchema)
            out["typed_decode"][label] = "accepted"
        except msgspec.DecodeError as e:
            out["typed_decode"][label] = "rejected: " + str(e)

    # 2. typed re-encode (what RimSort's write_rules_db would emit)
    reencoded = msgspec.json.encode(schema)
    out["typed_reencode"] = {
        "bytes": len(reencoded),
        "original_bytes": len(raw),
        "is_single_line": b"\n" not in reencoded,
    }

    # 3. simulate the rule editor save path against the on-disk JSON
    prune = consts["DB_BUILDER_PRUNE_EXCEPTIONS"]
    recurse = consts["DB_BUILDER_RECURSE_EXCEPTIONS"]

    def simulate(editor_rules: dict) -> dict:
        disk = copy.deepcopy(original)
        incoming = {"timestamp": 1_790_000_000, "rules": editor_rules}
        update(disk, incoming, prune_exceptions=prune, recurse_exceptions=recurse)
        return disk

    # 3a. no-op save: editor holds exactly what the typed decode produced
    noop = simulate(copy.deepcopy(built))
    out["editor_save_noop"] = {
        "entries_before": len(original["rules"]),
        "entries_after": len(noop["rules"]),
        "keys_added": sorted(set(noop["rules"]) - set(original["rules"])),
        "keys_removed": sorted(set(original["rules"]) - set(noop["rules"])),
        "incompatibleWith_edges_after": count_edges(noop["rules"], "incompatibleWith"),
        "timestamp_changed": noop["timestamp"] != original["timestamp"],
    }

    # 3b. editor adds one incompatibleWith edge to a mod that already has some in the file
    victim = next((k for k, v in original["rules"].items() if v.get("incompatibleWith")), None)
    if victim is not None:
        ed = copy.deepcopy(built)
        ed.setdefault(victim.lower(), {}).setdefault("incompatibleWith", {})["new.example.mod"] = {
            "name": "Example",
            "comment": "added in editor",
        }
        res = simulate(ed)
        before = set(original["rules"][victim]["incompatibleWith"].keys())
        after_entry = res["rules"].get(victim) or res["rules"].get(victim.lower()) or {}
        after = set((after_entry.get("incompatibleWith") or {}).keys())
        out["editor_adds_incompat_to_mod_that_already_has_some"] = {
            "mod_had_incompat_targets": len(before),
            "mod_has_after_save": len(after),
            "pre_existing_targets_lost": sorted(before - after),
        }

    # 3c. editor removes the last loadAfter edge of a mod (empty dict pruning)
    sub = next((k for k, v in built.items() if list(v.keys()) == ["loadAfter"] and len(v["loadAfter"]) == 1), None)
    if sub is not None:
        ed = copy.deepcopy(built)
        ed[sub]["loadAfter"] = {}
        res = simulate(ed)
        out["editor_removes_last_edge_of_a_mod"] = {
            "mod": sub,
            "entry_after_save": res["rules"].get(sub, "<entry deleted>"),
        }

    # 4. byte compatibility of the on-disk serialisation
    text = raw.decode("utf-8")
    canon = json.dumps(original, indent=4)  # same defaults as a plain dump with indent=4
    out["serialisation"] = {
        "python_indent4_ascii_equals_file": canon == text,
        "python_indent4_ascii_plus_newline_equals_file": canon + "\n" == text,
        "python_indent4_ensure_ascii_false_equals_file": json.dumps(original, indent=4, ensure_ascii=False) + "\n" == text,
    }

    json.dump(out, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
