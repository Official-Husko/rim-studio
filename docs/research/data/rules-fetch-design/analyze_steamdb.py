#!/usr/bin/env python3
"""Profile a RimSort-format steamDB.json (about 49 MB) and size candidate compact encodings.

Usage:
    analyze_steamdb.py <steamDB.json> [--out summary.json] [--local-library DIR]

Reports entry counts, field presence and types, per-field byte share, duplicate package ids,
and the size of a slim projection under gzip, xz and zstd (Python 3.14 `compression.zstd`
when available). Deterministic. No network. All numbers in the research note come from here.
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import gzip
import json
import lzma
import re
import sys
import time
from pathlib import Path


def zstd_size(data: bytes, level: int) -> int | None:
    try:
        from compression import zstd  # Python 3.14+

        return len(zstd.compress(data, level=level))
    except Exception:
        return None


def jsize(v) -> int:
    return len(json.dumps(v, separators=(",", ":"), ensure_ascii=False).encode("utf-8"))


def type_name(v) -> str:
    if isinstance(v, list):
        inner = sorted({type(x).__name__ for x in v})
        return "list[" + "|".join(inner) + "]"
    return type(v).__name__


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("path")
    ap.add_argument("--out", default=None)
    ap.add_argument("--local-library", default=None)
    args = ap.parse_args()

    p = Path(args.path)
    raw = p.read_bytes()
    t0 = time.perf_counter()
    data = json.loads(raw)
    parse_s = time.perf_counter() - t0

    out: dict = {"file_bytes": len(raw)}
    out["python_json_parse_seconds"] = round(parse_s, 2)
    out["ends_with_newline"] = raw.endswith(b"\n")
    out["top_level_keys"] = list(data.keys())
    out["version"] = data.get("version")
    if isinstance(data.get("version"), int):
        out["version_utc"] = dt.datetime.fromtimestamp(data["version"], dt.UTC).isoformat()
    m = re.search(rb"\n( +)\"", raw[:2000])
    out["first_indent_spaces"] = len(m.group(1)) if m else None
    out["raw_non_ascii_bytes"] = sum(1 for b in raw if b > 127)
    out["escaped_unicode_sequences"] = len(re.findall(rb"\\u[0-9a-fA-F]{4}", raw))

    db = data["database"]
    out["entries"] = len(db)
    keys = list(db.keys())
    out["keys_all_digits"] = sum(1 for k in keys if k.isdigit())
    out["keys_not_digits_examples"] = [k for k in keys if not k.isdigit()][:5]

    field_presence = collections.Counter()
    field_types = collections.defaultdict(collections.Counter)
    field_bytes = collections.Counter()
    entry_sizes = []
    only_unpublished = 0
    appid_entries = 0
    pid_variants = collections.Counter()
    pid_to_pfids = collections.defaultdict(list)
    dep_shapes = collections.Counter()
    deps_total = 0
    deps_entries = 0
    deps_dep_counts = []
    tags_entries = 0
    blacklist_true = 0
    unpublished_true = 0
    game_versions_forms = collections.Counter()
    authors_forms = collections.Counter()
    steamname_diff = 0
    names_missing = 0
    name_total_bytes = 0
    url_total_bytes = 0

    for pfid, e in db.items():
        sz = jsize(e)
        entry_sizes.append(sz)
        if not isinstance(e, dict):
            field_presence["<non-object entry>"] += 1
            continue
        if set(e.keys()) == {"unpublished"}:
            only_unpublished += 1
        for k, v in e.items():
            field_presence[k] += 1
            field_types[k][type_name(v)] += 1
            field_bytes[k] += jsize({k: v}) - 2  # key + value + colon, no braces
        if e.get("appid"):
            appid_entries += 1
        has_lower = "packageid" in e
        has_camel = "packageId" in e
        pid_variants[("packageid" if has_lower else "") + ("+" if has_lower and has_camel else "") + ("packageId" if has_camel else "") or "none"] += 1
        pid = e.get("packageId") or e.get("packageid")
        if isinstance(pid, str) and pid:
            pid_to_pfids[pid.lower()].append(pfid)
        if e.get("unpublished") is True:
            unpublished_true += 1
        bl = e.get("blacklist")
        if isinstance(bl, dict) and bl.get("value"):
            blacklist_true += 1
        if "dependencies" in e:
            deps = e["dependencies"]
            if isinstance(deps, dict):
                deps_entries += 1
                deps_total += len(deps)
                deps_dep_counts.append(len(deps))
                for dk, dv in deps.items():
                    dep_shapes[type_name(dv)] += 1
        if e.get("tags"):
            tags_entries += 1
        gv = e.get("gameVersions")
        game_versions_forms[type_name(gv) if gv is not None else "absent"] += 1
        au = e.get("authors")
        authors_forms[type_name(au) if au is not None else "absent"] += 1
        if "name" not in e and "steamName" not in e:
            names_missing += 1
        if e.get("steamName") and e.get("name") and e["steamName"] != e["name"]:
            steamname_diff += 1
        if isinstance(e.get("name"), str):
            name_total_bytes += len(e["name"].encode("utf-8"))
        if isinstance(e.get("url"), str):
            url_total_bytes += len(e["url"].encode("utf-8"))

    out["entries_only_unpublished_flag"] = only_unpublished
    out["entries_unpublished_true"] = unpublished_true
    out["entries_blacklist_true"] = blacklist_true
    out["entries_with_appid_true"] = appid_entries
    out["entries_with_dependencies_object"] = deps_entries
    out["dependency_edges_total"] = deps_total
    out["dependency_value_shapes"] = dict(dep_shapes)
    out["max_dependencies_in_one_entry"] = max(deps_dep_counts, default=0)
    out["entries_with_tags"] = tags_entries
    out["field_presence"] = dict(field_presence.most_common())
    out["field_value_types"] = {k: dict(v) for k, v in field_types.items()}
    out["field_bytes_compact_json"] = dict(field_bytes.most_common())
    total_field_bytes = sum(field_bytes.values())
    out["field_share_percent_of_entry_payload"] = {
        k: round(100 * v / total_field_bytes, 1) for k, v in field_bytes.most_common(12)
    }
    out["packageid_key_variants"] = dict(pid_variants)
    out["game_versions_forms"] = dict(game_versions_forms)
    out["authors_forms"] = dict(authors_forms)
    out["entries_steamName_differs_from_name"] = steamname_diff
    out["entries_without_any_name"] = names_missing
    out["distinct_package_ids"] = len(pid_to_pfids)
    out["package_id_placeholders"] = {
        k: len(pid_to_pfids[k]) for k in ("scenario.rsc", "missing.packageid", "invalid.item") if k in pid_to_pfids
    }
    dups = {k: v for k, v in pid_to_pfids.items() if len(v) > 1}
    out["package_ids_with_multiple_workshop_entries"] = len(dups)
    out["max_workshop_entries_for_one_package_id"] = max((len(v) for v in pid_to_pfids.values()), default=0)
    out["compact_entry_size_bytes"] = {
        "min": min(entry_sizes),
        "median": sorted(entry_sizes)[len(entry_sizes) // 2],
        "mean": round(sum(entry_sizes) / len(entry_sizes), 1),
        "p99": sorted(entry_sizes)[int(len(entry_sizes) * 0.99)],
        "max": max(entry_sizes),
    }

    # slim projection: only what a mod manager needs to look a mod up
    slim = {}
    for pfid, e in db.items():
        if not isinstance(e, dict):
            continue
        pid = e.get("packageId") or e.get("packageid")
        if not pid:
            if e.get("unpublished") is True:
                slim[pfid] = {"u": 1}
            continue
        row = {"p": pid.lower()}
        nm = e.get("steamName") or e.get("name")
        if nm:
            row["n"] = nm
        au = e.get("authors")
        if au:
            row["a"] = au
        gv = e.get("gameVersions")
        if gv:
            row["g"] = gv
        deps = e.get("dependencies")
        if isinstance(deps, dict) and deps:
            row["d"] = sorted(deps.keys())  # dependency keys are workshop ids (or Steam app ids), not package ids
        if e.get("unpublished") is True:
            row["u"] = 1
        bl = e.get("blacklist")
        if isinstance(bl, dict) and bl.get("value"):
            row["b"] = 1
        slim[pfid] = row
    slim_bytes = json.dumps(slim, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    out["slim_projection"] = {
        "description": "pfid -> {p: packageId lowercase, n: steamName or name, a: authors, g: gameVersions, d: dependency workshop ids only, u: unpublished, b: blacklisted}",
        "entries": len(slim),
        "minified_json_bytes": len(slim_bytes),
        "gzip_9_bytes": len(gzip.compress(slim_bytes, 9)),
        "xz_6_bytes": len(lzma.compress(slim_bytes, preset=6)),
        "zstd_19_bytes": zstd_size(slim_bytes, 19),
        "zstd_3_bytes": zstd_size(slim_bytes, 3),
    }
    # reference compressions of the full file
    out["full_file_compressed"] = {
        "gzip_9_bytes": len(gzip.compress(raw, 9)),
        "gzip_6_bytes": len(gzip.compress(raw, 6)),
        "xz_6_bytes": len(lzma.compress(raw, preset=6)),
        "zstd_3_bytes": zstd_size(raw, 3),
        "zstd_19_bytes": zstd_size(raw, 19),
    }

    if args.local_library:
        wd = Path(args.local_library)
        pfids = {x.name for x in wd.iterdir() if x.is_dir() and x.name.isdigit()}
        out["local_library"] = {
            "workshop_folders": len(pfids),
            "folders_present_in_steamdb": sum(1 for x in pfids if x in db),
            "folders_flagged_unpublished_in_steamdb": sum(
                1 for x in pfids if isinstance(db.get(x), dict) and db[x].get("unpublished") is True
            ),
        }

    s = json.dumps(out, indent=2, sort_keys=True, ensure_ascii=True)
    if args.out:
        Path(args.out).write_text(s + "\n", encoding="utf-8")
    print(s)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
