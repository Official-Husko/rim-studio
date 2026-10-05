#!/usr/bin/env python3
"""Profile the three small RimSort companion datasets.

Usage:
    analyze_small_datasets.py --uti replacements.json.gz --nvw-dir <NoVersionWarning checkout> \
        --versions rimworld_versions.json [--out summary.json]

* Use This Instead: gzip JSON, BOM, {"rules": [...]} (counts, key sets, id coverage)
* No Version Warning: one ModIdsToFix.xml per game version folder (counts, comments, case)
* RimWorld versions: depots and per-platform version lists

Deterministic, no network.
"""
from __future__ import annotations

import argparse
import collections
import gzip
import json
import re
import sys
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--uti")
    ap.add_argument("--nvw-dir")
    ap.add_argument("--versions")
    ap.add_argument("--workshop-dir", default=None)
    ap.add_argument("--out", default=None)
    a = ap.parse_args()
    out: dict = {}

    if a.uti:
        gz = Path(a.uti).read_bytes()
        raw = gzip.decompress(gz)
        out["use_this_instead"] = u = {
            "gz_bytes": len(gz),
            "decompressed_bytes": len(raw),
            "starts_with_bom": raw.startswith(b"\xef\xbb\xbf"),
            "gzip_header_mtime_field_nonzero": gz[4:8] != b"\x00\x00\x00\x00",
        }
        data = json.loads(raw.decode("utf-8-sig"))
        u["top_level_keys"] = list(data.keys())
        rules = data["rules"]
        u["rules"] = len(rules)
        kc = collections.Counter()
        for r in rules:
            kc[tuple(sorted(r.keys()))] += 1
        u["distinct_key_sets"] = len(kc)
        u["key_sets"] = [[list(k), c] for k, c in kc.most_common(3)]
        u["old_workshop_ids_unique"] = len({r.get("oldWorkshopId") for r in rules})
        u["old_workshop_id_duplicates"] = len(rules) - u["old_workshop_ids_unique"]
        u["rules_with_empty_old_package_id"] = sum(1 for r in rules if not r.get("oldPackageId"))
        u["rules_with_empty_new_package_id"] = sum(1 for r in rules if not r.get("newPackageId"))
        u["rules_where_new_id_equals_old_id"] = sum(1 for r in rules if r.get("newWorkshopId") == r.get("oldWorkshopId"))
        ver = collections.Counter(v for r in rules for v in r.get("newVersions", []))
        u["newVersions_histogram"] = dict(sorted(ver.items()))
        u["workshop_id_types"] = dict(collections.Counter(type(r.get("oldWorkshopId")).__name__ for r in rules))
        if a.workshop_dir:
            wd = Path(a.workshop_dir)
            have = {p.name for p in wd.iterdir() if p.is_dir() and p.name.isdigit()}
            u["local_library_mods_with_a_replacement_rule"] = sorted(
                have & {str(r["oldWorkshopId"]) for r in rules}
            ).__len__()

    if a.nvw_dir:
        base = Path(a.nvw_dir)
        out["no_version_warning"] = n = {}
        for d in sorted(p for p in base.iterdir() if p.is_dir() and re.fullmatch(r"\d+\.\d+", p.name)):
            f = d / "ModIdsToFix.xml"
            if not f.exists():
                continue
            raw = f.read_bytes()
            text = raw.decode("utf-8-sig")
            ids = re.findall(r"<li>\s*([^<]+?)\s*</li>", text)
            comments = re.findall(r"<!--(.*?)-->", text, re.S)
            n[d.name] = {
                "bytes": len(raw),
                "starts_with_bom": raw.startswith(b"\xef\xbb\xbf"),
                "li_entries": len(ids),
                "distinct_lowercase": len({i.lower() for i in ids}),
                "entries_with_uppercase": sum(1 for i in ids if i != i.lower()),
                "comment_blocks": len(comments),
                "crlf": raw.count(b"\r\n"),
            }

    if a.versions:
        raw = Path(a.versions).read_bytes()
        data = json.loads(raw)
        out["rimworld_versions"] = r = {
            "bytes": len(raw),
            "top_level_keys": list(data.keys()),
            "depot_groups": list(data["depots"].keys()),
            "platforms": list(data["versions"].keys()),
            "versions_per_platform": {k: len(v) for k, v in data["versions"].items()},
            "status_values": dict(collections.Counter(x["status"] for v in data["versions"].values() for x in v)),
            "version_entry_keys": sorted({k for v in data["versions"].values() for x in v for k in x.keys()}),
        }
        last = data["versions"].get("linux") or next(iter(data["versions"].values()))
        r["newest_linux_version_label"] = last[-1]["version"]

    s = json.dumps(out, indent=2, sort_keys=True)
    if a.out:
        Path(a.out).write_text(s + "\n", encoding="utf-8")
    print(s)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
