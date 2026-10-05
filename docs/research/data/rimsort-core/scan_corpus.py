#!/usr/bin/env python3
"""Scan RimWorld mod roots and record About.xml facts for the research notes.

Usage:
    scan_corpus.py <out.json> <root-label>=<path> [<root-label>=<path> ...]

Reads only. For each immediate sub-directory of every root it records whether
About/About.xml exists (exact and case-insensitive), whether the XML parses
strictly, and the raw field values needed to compare RimSort and RimWorld
behaviour. Deterministic: roots and entries are processed in sorted order.
"""
import json
import os
import re
import sys
import time
import xml.etree.ElementTree as ET

from lxml import etree as LET

PACKAGE_ID_RE = re.compile(r"^(?=.{1,60}$)(?!\.)(?=.*?[.])(?!.*([.])\1+)[a-zA-Z0-9.]{1,}[a-zA-Z0-9]{1}$")


def text_of(el):
    if el is None:
        return None
    t = "".join(el.itertext()) if len(el) == 0 else (el.text or "")
    return t.strip()


def li_list(el):
    """Return list of (text, attrib-dict) for the <li> children of el."""
    if el is None:
        return None
    out = []
    for c in el:
        if not isinstance(c.tag, str):
            continue
        out.append((text_of(c), dict(c.attrib)))
    return out


def deps_list(el):
    if el is None:
        return None
    out = []
    for c in el:
        if not isinstance(c.tag, str):
            continue
        d = {}
        for f in c:
            if not isinstance(f.tag, str):
                continue
            if f.tag == "alternativePackageIds":
                d["alternativePackageIds"] = [text_of(x) for x in f if isinstance(x.tag, str)]
            else:
                d[f.tag] = text_of(f)
        d["_attrs"] = dict(c.attrib)
        out.append(d)
    return out


def find_ci(parent, name):
    """Case-insensitive child lookup used only to detect near-miss tag names."""
    for c in parent:
        if isinstance(c.tag, str) and c.tag.lower() == name.lower():
            return c
    return None


def scan_dir(path):
    rec = {"folder": os.path.basename(path)}
    try:
        entries = os.listdir(path)
    except OSError as e:
        rec["error"] = str(e)
        return rec
    lower = {e.lower(): e for e in entries}
    rec["has_about_dir_exact"] = "About" in entries
    about_dir = lower.get("about")
    rec["has_about_dir_ci"] = about_dir is not None
    rec["about_dir_name"] = about_dir
    rec["has_rsc"] = any(e.lower().endswith(".rsc") for e in entries)
    rec["has_loadfolders"] = "loadfolders.xml" in lower
    rec["loadfolders_name"] = lower.get("loadfolders.xml")
    rec["has_git"] = ".git" in entries
    rec["subdirs"] = sorted(e for e in entries if os.path.isdir(os.path.join(path, e)))[:40]
    pf = None
    if about_dir:
        try:
            ad = os.listdir(os.path.join(path, about_dir))
        except OSError:
            ad = []
        al = {e.lower(): e for e in ad}
        rec["about_xml_exact"] = "About.xml" in ad
        rec["about_xml_ci"] = al.get("about.xml")
        rec["has_preview"] = "Preview.png" in ad
        rec["has_pfid_file"] = "PublishedFileId.txt" in ad
        if rec["has_pfid_file"]:
            try:
                with open(os.path.join(path, about_dir, "PublishedFileId.txt"), "rb") as f:
                    pf = f.read(64)
                rec["pfid_raw"] = pf.decode("utf-8", "replace")
            except OSError:
                pass
    if not rec.get("about_xml_ci"):
        return rec
    ap = os.path.join(path, about_dir, rec["about_xml_ci"])
    raw = open(ap, "rb").read()
    rec["about_bytes"] = len(raw)
    rec["bom"] = raw.startswith(b"\xef\xbb\xbf")
    rec["utf16_bom"] = raw[:2] in (b"\xff\xfe", b"\xfe\xff")
    # strict parse as bytes (what RimSort's ET.parse does)
    root = None
    try:
        root = ET.fromstring(raw)
        rec["strict_bytes_ok"] = True
    except Exception as e:
        rec["strict_bytes_ok"] = False
        rec["strict_bytes_err"] = str(e)[:120]
    # strict parse as decoded text (what the game's XmlDocument.LoadXml does)
    try:
        txt = raw.decode("utf-8-sig")
        # strip declaration so encoding mismatch does not matter (string load ignores it)
        txt2 = re.sub(r"^\s*<\?xml[^>]*\?>", "", txt)
        root_text = ET.fromstring(txt2)
        rec["strict_text_ok"] = True
        if root is None:
            root = root_text
    except Exception as e:
        rec["strict_text_ok"] = False
        rec["strict_text_err"] = str(e)[:120]
    if root is None:
        try:
            lroot = LET.fromstring(raw, parser=LET.XMLParser(recover=True))
            rec["lenient_ok"] = lroot is not None
            # Convert lxml tree to a stdlib-like tree minimally
            root = ET.fromstring(LET.tostring(lroot)) if lroot is not None else None
        except Exception as e:
            rec["lenient_ok"] = False
    if root is None:
        return rec
    rec["root_tag"] = root.tag
    rec["root_is_ModMetaData"] = root.tag == "ModMetaData"
    rec["root_is_modmetadata_ci"] = root.tag.lower() == "modmetadata"
    # direct child tag names
    rec["child_tags"] = [c.tag for c in root if isinstance(c.tag, str)]
    seen = {}
    for c in root:
        if isinstance(c.tag, str):
            seen[c.tag] = seen.get(c.tag, 0) + 1
    rec["dup_child_tags"] = sorted(k for k, v in seen.items() if v > 1)
    # near-miss casing of known fields
    known = ["packageId", "name", "author", "authors", "description", "supportedVersions", "modVersion",
             "modIconPath", "url", "steamAppId", "targetVersion", "shortName", "modDependencies",
             "loadBefore", "loadAfter", "forceLoadBefore", "forceLoadAfter", "incompatibleWith",
             "descriptionsByVersion", "modDependenciesByVersion", "loadBeforeByVersion",
             "loadAfterByVersion", "incompatibleWithByVersion"]
    miscased = []
    for k in known:
        if root.find(k) is None and find_ci(root, k) is not None:
            miscased.append(k)
    rec["miscased_fields"] = miscased
    pid = root.find("packageId")
    rec["packageId"] = text_of(pid)
    rec["name"] = text_of(root.find("name"))
    rec["author"] = text_of(root.find("author"))
    rec["authors"] = [t for t, a in (li_list(root.find("authors")) or [])]
    rec["modVersion"] = text_of(root.find("modVersion"))
    rec["url"] = text_of(root.find("url"))
    rec["steamAppId"] = text_of(root.find("steamAppId"))
    rec["has_description"] = root.find("description") is not None
    sv = root.find("supportedVersions")
    rec["supportedVersions_present"] = sv is not None
    rec["supportedVersions"] = [t for t, a in (li_list(sv) or [])] if sv is not None else None
    rec["targetVersion"] = text_of(root.find("targetVersion"))
    for tag in ["loadBefore", "loadAfter", "forceLoadBefore", "forceLoadAfter", "incompatibleWith"]:
        el = root.find(tag)
        rec[tag] = li_list(el)
        bv = root.find(tag + "ByVersion")
        if bv is not None:
            rec[tag + "ByVersion"] = {c.tag: li_list(c) for c in bv if isinstance(c.tag, str)}
    md = root.find("modDependencies")
    rec["modDependencies"] = deps_list(md)
    mdb = root.find("modDependenciesByVersion")
    if mdb is not None:
        rec["modDependenciesByVersion"] = {c.tag: deps_list(c) for c in mdb if isinstance(c.tag, str)}
    dbv = root.find("descriptionsByVersion")
    if dbv is not None:
        rec["descriptionsByVersion_keys"] = [c.tag for c in dbv if isinstance(c.tag, str)]
    return rec


def main():
    out = sys.argv[1]
    roots = {}
    for a in sys.argv[2:]:
        label, p = a.split("=", 1)
        roots[label] = p
    result = {}
    t0 = time.time()
    for label in sorted(roots):
        p = roots[label]
        recs = []
        if not os.path.isdir(p):
            result[label] = {"path_exists": False}
            continue
        names = sorted(os.listdir(p))
        for n in names:
            d = os.path.join(p, n)
            if os.path.isdir(d):
                r = scan_dir(d)
                r["root"] = label
                recs.append(r)
            else:
                recs.append({"folder": n, "root": label, "is_file": True})
        result[label] = {"path_exists": True, "entries": recs}
    result["_elapsed_s"] = round(time.time() - t0, 2)
    with open(out, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False)
    for label in sorted(roots):
        r = result[label]
        if r.get("path_exists"):
            ents = r["entries"]
            print(label, "entries:", len(ents), "dirs:", sum(1 for e in ents if not e.get("is_file")))
    print("elapsed", result["_elapsed_s"])


if __name__ == "__main__":
    main()
