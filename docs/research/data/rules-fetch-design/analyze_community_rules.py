#!/usr/bin/env python3
"""Profile a RimSort-format communityRules.json (or userRules.json) file.

Usage:
    analyze_community_rules.py <rules.json> [--workshop-dir DIR] [--out summary.json]

Prints a JSON summary: shape of every level, unknown keys, case collisions,
formatting facts (indent, trailing newline, escaping), cycles and contradictions.
All numbers in the research note come from this script. Deterministic, no network.
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import json
import re
import sys
from pathlib import Path

KNOWN_RULE_KEYS = {"loadAfter", "loadBefore", "loadTop", "loadBottom"}
EDGE_KEYS = ("loadAfter", "loadBefore", "incompatibleWith")
BOOL_KEYS = ("loadTop", "loadBottom")


def shape_of(v):
    if isinstance(v, list):
        return "list[" + ",".join(sorted({type(x).__name__ for x in v})) + "]"
    return type(v).__name__


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("path")
    ap.add_argument("--workshop-dir", default=None,
                    help="optional folder of workshop mods to compute coverage (About/About.xml)")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    raw = Path(args.path).read_bytes()
    text = raw.decode("utf-8")
    data = json.loads(text)

    out: dict = {}
    out["file_bytes"] = len(raw)
    out["has_bom"] = raw.startswith(b"\xef\xbb\xbf")
    out["crlf_count"] = raw.count(b"\r\n")
    out["ends_with_newline"] = raw.endswith(b"\n")
    out["first_indent_spaces"] = None
    m = re.search(r"\n( +)\"", text)
    if m:
        out["first_indent_spaces"] = len(m.group(1))
    out["raw_non_ascii_chars"] = sum(1 for ch in text if ord(ch) > 127)
    out["escaped_unicode_sequences"] = len(re.findall(r"\\u[0-9a-fA-F]{4}", text))
    out["top_level_keys"] = list(data.keys())
    ts = data.get("timestamp")
    out["timestamp"] = ts
    if isinstance(ts, int):
        out["timestamp_utc"] = dt.datetime.fromtimestamp(ts, dt.UTC).isoformat()

    rules = data.get("rules", {})
    out["rule_entries"] = len(rules)
    keys = list(rules.keys())
    out["rules_keys_sorted_case_sensitive"] = keys == sorted(keys)
    out["rules_keys_sorted_case_insensitive"] = keys == sorted(keys, key=str.lower)
    out["keys_with_uppercase"] = sum(1 for k in keys if k != k.lower())
    out["keys_with_uppercase_examples"] = [k for k in keys if k != k.lower()][:5]
    lowered = collections.Counter(k.lower() for k in keys)
    out["lowercase_collisions"] = {k: c for k, c in lowered.items() if c > 1}
    out["keys_with_whitespace"] = sum(1 for k in keys if k != k.strip())

    rule_key_counter = collections.Counter()
    unknown_rule_keys = collections.Counter()
    edge_counts = collections.Counter()
    inner_key_counter = collections.defaultdict(collections.Counter)
    inner_shape_counter = collections.defaultdict(collections.Counter)
    bool_value_counter = collections.defaultdict(collections.Counter)
    targets_upper = 0
    target_lower_collisions = 0
    self_edges = 0
    edges_by_type = {k: set() for k in EDGE_KEYS}
    all_targets = set()
    comment_total = 0
    name_missing = 0
    name_empty_list = 0
    multi_name_lists = 0
    rule_type_combos = collections.Counter()

    for pid, entry in rules.items():
        if not isinstance(entry, dict):
            unknown_rule_keys["<entry not an object>"] += 1
            continue
        rule_type_combos[tuple(sorted(entry.keys()))] += 1
        for rk, rv in entry.items():
            rule_key_counter[rk] += 1
            if rk not in KNOWN_RULE_KEYS and rk != "incompatibleWith":
                unknown_rule_keys[rk] += 1
            if rk in EDGE_KEYS:
                if not isinstance(rv, dict):
                    unknown_rule_keys[f"<{rk} not an object>"] += 1
                    continue
                for tgt, meta in rv.items():
                    edge_counts[rk] += 1
                    edges_by_type[rk].add((pid.lower(), tgt.lower()))
                    all_targets.add(tgt.lower())
                    if tgt != tgt.lower():
                        targets_upper += 1
                    if tgt.lower() == pid.lower():
                        self_edges += 1
                    if not isinstance(meta, dict):
                        inner_shape_counter[rk]["<non-object>"] += 1
                        continue
                    for ik, iv in meta.items():
                        inner_key_counter[rk][ik] += 1
                        inner_shape_counter[rk][f"{ik}:{shape_of(iv)}"] += 1
                    if "comment" in meta and meta["comment"] not in ("", [], None):
                        comment_total += 1
                    nm = meta.get("name")
                    if nm is None:
                        name_missing += 1
                    elif isinstance(nm, list):
                        if not nm:
                            name_empty_list += 1
                        if len(nm) > 1:
                            multi_name_lists += 1
            elif rk in BOOL_KEYS:
                if isinstance(rv, dict):
                    for ik, iv in rv.items():
                        inner_key_counter[rk][ik] += 1
                        inner_shape_counter[rk][f"{ik}:{shape_of(iv)}"] += 1
                    bool_value_counter[rk][str(rv.get("value"))] += 1
                else:
                    unknown_rule_keys[f"<{rk} not an object>"] += 1

    out["rule_key_counts"] = dict(rule_key_counter)
    out["unknown_rule_keys"] = dict(unknown_rule_keys)
    out["edge_counts"] = dict(edge_counts)
    out["edge_total"] = sum(edge_counts.values())
    out["inner_keys"] = {k: dict(v) for k, v in inner_key_counter.items()}
    out["inner_shapes"] = {k: dict(v) for k, v in inner_shape_counter.items()}
    out["bool_rule_values"] = {k: dict(v) for k, v in bool_value_counter.items()}
    out["target_keys_with_uppercase"] = targets_upper
    out["self_edges"] = self_edges
    out["edges_with_comment"] = comment_total
    out["edges_missing_name"] = name_missing
    out["edges_empty_name_list"] = name_empty_list
    out["edges_multi_name_lists"] = multi_name_lists
    out["distinct_rule_key_combinations"] = len(rule_type_combos)
    out["top_rule_key_combinations"] = [
        [list(k), c] for k, c in rule_type_combos.most_common(8)
    ]
    out["distinct_target_package_ids"] = len(all_targets)
    out["distinct_subject_package_ids"] = len({k.lower() for k in keys})
    out["distinct_package_ids_total"] = len(all_targets | {k.lower() for k in keys})

    # Normalise every rule to a directed "must come before" edge (before -> after)
    before_edges = set()
    for (pid, tgt) in edges_by_type["loadAfter"]:
        before_edges.add((tgt, pid))  # tgt loads before pid
    for (pid, tgt) in edges_by_type["loadBefore"]:
        before_edges.add((pid, tgt))  # pid loads before tgt
    out["directed_order_edges_after_normalisation"] = len(before_edges)
    two_cycles = {tuple(sorted((a, b))) for (a, b) in before_edges if (b, a) in before_edges}
    out["two_cycles_in_dataset_alone"] = len(two_cycles)
    out["two_cycle_examples"] = [list(c) for c in sorted(two_cycles)[:5]]
    # contradiction between loadAfter and loadBefore declared for the same subject/target pair
    contra = set()
    for (pid, tgt) in edges_by_type["loadAfter"]:
        if (pid, tgt) in edges_by_type["loadBefore"]:
            contra.add((pid, tgt))
    out["same_subject_same_target_after_and_before"] = len(contra)

    # reduce to find longer cycles via Tarjan SCC
    adj = collections.defaultdict(list)
    for a, b in before_edges:
        adj[a].append(b)
    index = {}
    low = {}
    onstack = set()
    stack = []
    sccs = []
    counter = [0]
    sys.setrecursionlimit(100000)

    def strong(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        onstack.add(v)
        for w in adj.get(v, []):
            if w not in index:
                strong(w)
                low[v] = min(low[v], low[w])
            elif w in onstack:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = stack.pop()
                onstack.discard(w)
                comp.append(w)
                if w == v:
                    break
            if len(comp) > 1:
                sccs.append(sorted(comp))

    for v in list(adj.keys()):
        if v not in index:
            strong(v)
    out["cyclic_components_in_dataset_alone"] = len(sccs)
    out["largest_cyclic_component"] = max((len(c) for c in sccs), default=0)

    # in/out degree stats
    outdeg = collections.Counter(a for a, _ in before_edges)
    indeg = collections.Counter(b for _, b in before_edges)
    out["max_out_degree"] = max(outdeg.values(), default=0)
    out["max_in_degree"] = max(indeg.values(), default=0)
    if outdeg:
        top = outdeg.most_common(3)
        out["top_out_degree"] = [[k, v] for k, v in top]
    if indeg:
        out["top_in_degree"] = [[k, v] for k, v in indeg.most_common(3)]

    # coverage against a local workshop folder, if provided
    if args.workshop_dir:
        wd = Path(args.workshop_dir)
        pids = set()
        folders = 0
        for p in sorted(wd.iterdir()):
            if not p.is_dir():
                continue
            folders += 1
            about = None
            for c in p.iterdir():
                if c.name.lower() == "about" and c.is_dir():
                    for f in c.iterdir():
                        if f.name.lower() == "about.xml":
                            about = f
            if about is None:
                continue
            try:
                t = about.read_text(encoding="utf-8-sig", errors="replace")
            except OSError:
                continue
            mm = re.search(r"<packageId>\s*([^<]+?)\s*</packageId>", t, re.I)
            if mm:
                pids.add(mm.group(1).strip().lower())
        subj = {k.lower() for k in keys}
        out["local_library"] = {
            "workshop_folders": folders,
            "distinct_package_ids": len(pids),
            "installed_ids_that_are_rule_subjects": len(pids & subj),
            "installed_ids_that_are_rule_targets": len(pids & all_targets),
            "rule_subjects_not_installed": len(subj - pids),
            "rule_targets_not_installed": len(all_targets - pids),
            "rule_edges_with_both_ends_installed": sum(
                1 for (a, b) in before_edges if a in pids and b in pids
            ),
            "rule_edges_with_subject_installed_target_missing": sum(
                1 for (a, b) in before_edges if (a in pids) != (b in pids)
            ),
        }

    s = json.dumps(out, indent=2, sort_keys=True, ensure_ascii=True)
    if args.out:
        Path(args.out).write_text(s + "\n", encoding="utf-8")
    print(s)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
