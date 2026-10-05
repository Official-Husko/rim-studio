#!/usr/bin/env python3
"""Scan a RimWorld mod library (About.xml of every mod) into one JSON file.

Reads, for every mod folder found in the given roots, only the fields that
matter for community-dataset compatibility: packageId, name, authors,
supportedVersions, modDependencies, loadBefore, loadAfter, incompatibleWith,
forceLoadBefore, forceLoadAfter and the four *ByVersion variants.  It also
applies the game's own ByVersion rule for the requested game version
(an exact, non-additive replacement; see decompiled:Verse/ModMetaData.cs,
nested class VersionedData) and stores the result as "effective_*" fields.

The output is a working file for the other scripts.  It lists the user's mods,
so keep it out of the repository (write it to a scratch directory).

Usage:
  library_scan.py --workshop DIR [--install DIR] [--local DIR ...]
                  --game-version 1.6 --out library.json

Every path comes from the command line; nothing is hard coded.  The script
is deterministic: mods are sorted by (source, folder name).
"""

import argparse
import json
import os
import sys
from pathlib import Path

from lxml import etree

# Fields that hold a flat list of packageIds in About.xml.
ID_LIST_FIELDS = (
    "loadBefore",
    "loadAfter",
    "incompatibleWith",
    "forceLoadBefore",
    "forceLoadAfter",
)
BYVERSION_ID_LIST_FIELDS = ("loadBeforeByVersion", "loadAfterByVersion", "incompatibleWithByVersion")


def local(tag):
    """Local tag name, lower-cased (the scan is deliberately case tolerant)."""
    if not isinstance(tag, str):
        return ""
    return tag.split("}")[-1].lower()


def child(el, name):
    """First child element whose lower-cased local name equals `name`."""
    name = name.lower()
    for c in el:
        if local(c.tag) == name:
            return c
    return None


def text_of(el):
    if el is None:
        return None
    t = "".join(el.itertext()).strip()
    return t


def li_texts(el):
    """Texts of the <li> children of `el` (empty list when `el` is None)."""
    if el is None:
        return []
    out = []
    for c in el:
        if local(c.tag) == "li":
            t = text_of(c)
            if t is not None:
                out.append(t)
    return out


def parse_dependency(li):
    """One <li> of modDependencies."""
    dep = {
        "packageId": text_of(child(li, "packageId")) or "",
        "displayName": text_of(child(li, "displayName")) or "",
        "steamWorkshopUrl": text_of(child(li, "steamWorkshopUrl")) or "",
        "downloadUrl": text_of(child(li, "downloadUrl")) or "",
        "alternativePackageIds": li_texts(child(li, "alternativePackageIds")),
    }
    return dep


def parse_dependencies(el):
    if el is None:
        return []
    return [parse_dependency(c) for c in el if local(c.tag) == "li"]


def parse_byversion(el, kind):
    """Parse a *ByVersion element into {version: payload}.

    The game lower-cases the child element name and strips one leading "v";
    the first entry for a version wins (decompiled:Verse/ModMetaData.cs).
    """
    out = {}
    if el is None:
        return out
    for c in el:
        if not isinstance(c.tag, str):
            continue
        key = local(c.tag)
        if key.startswith("v"):
            key = key[1:]
        if key in out:
            continue
        out[key] = parse_dependencies(c) if kind == "deps" else li_texts(c)
    return out


def find_about_xml(mod_dir: Path):
    """Case-insensitive About/About.xml lookup.  Returns (path, exact_case)."""
    try:
        entries = list(os.scandir(mod_dir))
    except OSError:
        return None, False
    for e in entries:
        if e.name.lower() == "about" and e.is_dir():
            try:
                for f in os.scandir(e.path):
                    if f.name.lower() == "about.xml" and f.is_file():
                        return Path(f.path), (e.name == "About" and f.name == "About.xml")
            except OSError:
                return None, False
            return None, False
    return None, False


def parse_about(path: Path, game_version: str):
    parser = etree.XMLParser(recover=True, remove_comments=True, resolve_entities=False, huge_tree=True)
    tree = etree.parse(str(path), parser)
    root = tree.getroot()
    errors = [str(e) for e in parser.error_log][:3]
    if root is None or local(root.tag) != "modmetadata":
        return None, errors + ["root element is not ModMetaData"]

    m = {}
    m["packageId"] = text_of(child(root, "packageId")) or ""
    m["name"] = text_of(child(root, "name")) or ""
    authors = li_texts(child(root, "authors"))
    if not authors:
        a = text_of(child(root, "author"))
        authors = [a] if a else []
    m["authors"] = authors
    m["supportedVersions"] = li_texts(child(root, "supportedVersions"))
    m["modDependencies"] = parse_dependencies(child(root, "modDependencies"))
    for f in ID_LIST_FIELDS:
        m[f] = li_texts(child(root, f))
    m["modDependenciesByVersion"] = parse_byversion(child(root, "modDependenciesByVersion"), "deps")
    for f in BYVERSION_ID_LIST_FIELDS:
        m[f] = parse_byversion(child(root, f), "ids")

    # Game semantics: exact version key replaces the base list entirely.
    gv = game_version
    eff = {}
    eff["modDependencies"] = m["modDependenciesByVersion"].get(gv, m["modDependencies"])
    eff["loadBefore"] = m["loadBeforeByVersion"].get(gv, m["loadBefore"])
    eff["loadAfter"] = m["loadAfterByVersion"].get(gv, m["loadAfter"])
    eff["incompatibleWith"] = m["incompatibleWithByVersion"].get(gv, m["incompatibleWith"])
    eff["forceLoadBefore"] = m["forceLoadBefore"]
    eff["forceLoadAfter"] = m["forceLoadAfter"]
    m["effective"] = eff
    m["usesByVersion"] = any(
        bool(m[k]) for k in ("modDependenciesByVersion",) + BYVERSION_ID_LIST_FIELDS
    )
    return m, errors


def scan_root(root: Path, source: str, game_version: str):
    mods = []
    if not root.is_dir():
        return mods
    for entry in sorted(root.iterdir(), key=lambda p: p.name):
        if not entry.is_dir():
            continue
        rec = {
            "source": source,
            "folder": entry.name,
            "workshopId": entry.name if (source == "workshop" and entry.name.isdigit()) else None,
            "hasAbout": False,
            "aboutExactCase": None,
            "parseErrors": [],
        }
        about, exact = find_about_xml(entry)
        if about is not None:
            rec["hasAbout"] = True
            rec["aboutExactCase"] = exact
            try:
                meta, errs = parse_about(about, game_version)
            except Exception as ex:  # noqa: BLE001 - report and continue
                meta, errs = None, [f"exception: {ex}"]
            rec["parseErrors"] = errs
            if meta is not None:
                rec.update(meta)
        # PublishedFileId.txt (local mods that were published keep their id here)
        pf = None
        for sub in ("About", "about"):
            p = entry / sub / "PublishedFileId.txt"
            if p.is_file():
                try:
                    pf = p.read_text(encoding="utf-8-sig").strip()
                except OSError:
                    pf = None
                break
        rec["publishedFileIdTxt"] = pf if (pf and pf.isdigit()) else None
        mods.append(rec)
    return mods


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--workshop", required=True, help="Steam workshop content folder (content/294100)")
    ap.add_argument("--install", help="RimWorld install folder (reads Data/* as official content, Mods/* as local)")
    ap.add_argument("--local", action="append", default=[], help="extra local mod roots (repeatable)")
    ap.add_argument("--game-version", required=True, help="major.minor used for *ByVersion, e.g. 1.6")
    ap.add_argument("--out", required=True, help="output JSON path")
    args = ap.parse_args()

    mods = []
    mods += scan_root(Path(args.workshop), "workshop", args.game_version)
    if args.install:
        inst = Path(args.install)
        mods += scan_root(inst / "Data", "official", args.game_version)
        mods += scan_root(inst / "Mods", "local", args.game_version)
    for extra in args.local:
        mods += scan_root(Path(extra), "local-extra", args.game_version)

    out = {"gameVersion": args.game_version, "mods": mods}
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1, sort_keys=True)
    by_src = {}
    for m in mods:
        by_src.setdefault(m["source"], [0, 0])
        by_src[m["source"]][0] += 1
        by_src[m["source"]][1] += 1 if m["hasAbout"] else 0
    print(json.dumps({"mods": len(mods), "bySource(total,withAbout)": by_src}, indent=1), file=sys.stderr)


if __name__ == "__main__":
    main()
