#!/usr/bin/env python3
"""scan_mod_corpus.py - deterministic statistics over a RimWorld mod library.

Purpose
    Measure the real-world shape of RimWorld 1.6 mods (About.xml, LoadFolders.xml,
    folder anatomy, encodings, path hygiene, size and scan budget) so that the
    RimStudio parser and scanner can be specified against evidence.

Inputs (all paths come from arguments or environment variables; nothing is hard coded)
    --workshop DIR       Steam workshop content dir for the game (.../workshop/content/294100)
                         env RIMSTUDIO_WORKSHOP_DIR
    --install-mods DIR   the Mods dir inside the game install        env RIMSTUDIO_INSTALL_MODS_DIR
    --owner-mods DIR     a custom mod folder (the owner's mods)       env RIMSTUDIO_OWNER_MODS_DIR
    --official-data DIR  the Data dir inside the game install (Core + DLC). Reported separately
                         and NOT part of the "all" aggregate.         env RIMSTUDIO_OFFICIAL_DATA_DIR
    --mods-config FILE   optional ModsConfig.xml; enables the "active set" scan budget section
                         env RIMSTUDIO_MODS_CONFIG
    --workshop-acf FILE  optional appworkshop_294100.acf (default: derived from --workshop).
                         Only item ids are read. No account data is read or stored.
    --out FILE           summary JSON (sorted keys, no timestamps, no absolute paths)
    --mods-csv FILE      optional per-mod metrics CSV (no file contents)

Determinism
    Directory listings are sorted, results are sorted by (root, folder), percentiles use the
    nearest-rank method, and the JSON is written with sorted keys. Re-running on the same
    library state gives byte-identical output.

Semantics emulated (verified against the decompiled game, see the research note)
    ModMetaData.Init, TryParseSupportedVersions, TryParsePackageId, ValidateDependencies,
    ModLoadFolders.LoadDataFromXmlCustom, ModContentPack.InitLoadFolders, LoadFolder.ShouldLoad,
    File.ReadAllText (BOM sniffing, UTF-8 replacement decoding), the *.xml directory filter.

Requires Python 3.10+ and lxml (libxml2 is used as a strict XML 1.0 reference parser).
"""
from __future__ import annotations

import argparse
import csv
import json
import math
import os
import re
import sys
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor
from typing import Any

try:
    from lxml import etree
except ImportError:  # pragma: no cover
    sys.exit("lxml is required: pip install lxml")

SCHEMA = "rimstudio.mod-corpus.summary/1"

# ---------------------------------------------------------------------------
# constants taken from the decompiled game (Verse/ModMetaData.cs and friends)
# ---------------------------------------------------------------------------

# every instance field of ModMetaData.ModMetaDataInternal (the game reads these tag names)
ABOUT_FIELDS = [
    "packageId", "name", "shortName", "author", "authors", "modIconPath", "modVersion", "url",
    "description", "steamAppId", "supportedVersions", "targetVersion", "modDependencies",
    "loadBefore", "loadAfter", "incompatibleWith", "forceLoadBefore", "forceLoadAfter",
    "descriptionsByVersion", "modDependenciesByVersion", "loadBeforeByVersion",
    "loadAfterByVersion", "incompatibleWithByVersion",
]
ABOUT_FIELDS_LC = {f.lower(): f for f in ABOUT_FIELDS}
LIST_STRING_FIELDS = ["supportedVersions", "authors", "loadBefore", "loadAfter", "incompatibleWith",
                      "forceLoadBefore", "forceLoadAfter"]
BYVERSION_FIELDS = ["descriptionsByVersion", "modDependenciesByVersion", "loadBeforeByVersion",
                    "loadAfterByVersion", "incompatibleWithByVersion"]
# fields typed as plain strings: exactly one text or CDATA child (or none) is accepted
STRING_FIELDS = ["packageId", "name", "shortName", "author", "modIconPath", "modVersion", "url",
                 "description", "targetVersion"]
DEP_FIELDS = ["packageId", "displayName", "steamWorkshopUrl", "downloadUrl", "alternativePackageIds"]
DEP_FIELDS_LC = {f.lower(): f for f in DEP_FIELDS}

# ModMetaDataInternal.PackageIdFormatRegex (decompiled:Verse/ModMetaData.cs)
PACKAGE_ID_RE = re.compile(r"(?=.{1,60}$)^(?!\.)(?=.*?[.])(?!.*([.])\1+)[a-zA-Z0-9.]{1,}[a-zA-Z0-9]{1}$")

CONTENT_AREAS = ["Defs", "Patches", "Assemblies", "Textures", "Sounds", "Languages", "Strings",
                 "AssetBundles", "Materials"]
AREA_KEYS_LC = {a.lower() for a in CONTENT_AREAS} | {"about", "source"}
TEXTURE_EXT = {".png", ".jpg", ".jpeg", ".psd", ".dds"}
AUDIO_EXT = {".wav", ".mp3", ".ogg", ".xm", ".it", ".mod", ".s3m"}

WIN_RESERVED = {"CON", "PRN", "AUX", "NUL"} | {f"COM{i}" for i in range(1, 10)} | {f"LPT{i}" for i in range(1, 10)}
WIN_BAD_CHARS = set('<>:"/\\|?*')

DEFAULT_WIN_WORKSHOP_PREFIX = r"C:\Program Files (x86)\Steam\steamapps\workshop\content\294100"
DEFAULT_WIN_MODS_PREFIX = r"C:\Program Files (x86)\Steam\steamapps\common\RimWorld\Mods"
WIN_MAX_PATH = 260  # includes the terminating NUL, so usable length is 259

EXAMPLE_LIMIT = 8


# ---------------------------------------------------------------------------
# small utilities
# ---------------------------------------------------------------------------

def pct(values: list[float], p: float) -> float | int | None:
    """Nearest-rank percentile (deterministic, no interpolation)."""
    if not values:
        return None
    s = sorted(values)
    k = max(0, math.ceil(p / 100.0 * len(s)) - 1)
    return s[k]


def dist(values: list[float]) -> dict[str, Any]:
    if not values:
        return {"n": 0}
    s = sorted(values)
    n = len(s)
    return {
        "n": n,
        "min": s[0],
        "p50": pct(s, 50),
        "p90": pct(s, 90),
        "p95": pct(s, 95),
        "p99": pct(s, 99),
        "max": s[-1],
        "mean": round(sum(s) / n, 3),
        "sum": sum(s),
    }


def safe_name(entry_name: str) -> str:
    return entry_name.encode("utf-8", "backslashreplace").decode("utf-8")


def sniff_bom(b: bytes) -> str:
    if b.startswith(b"\xff\xfe\x00\x00"):
        return "utf-32-le"
    if b.startswith(b"\x00\x00\xfe\xff"):
        return "utf-32-be"
    if b.startswith(b"\xef\xbb\xbf"):
        return "utf-8-bom"
    if b.startswith(b"\xff\xfe"):
        return "utf-16-le"
    if b.startswith(b"\xfe\xff"):
        return "utf-16-be"
    return "none"


def dotnet_read_all_text(b: bytes) -> tuple[str, str]:
    """Emulates System.IO.File.ReadAllText(path): BOM detection, else UTF-8 with U+FFFD replacement."""
    kind = sniff_bom(b)
    if kind == "utf-8-bom":
        return b[3:].decode("utf-8", "replace"), kind
    if kind == "utf-16-le":
        return b[2:].decode("utf-16-le", "replace"), kind
    if kind == "utf-16-be":
        return b[2:].decode("utf-16-be", "replace"), kind
    if kind == "utf-32-le":
        return b[4:].decode("utf-32-le", "replace"), kind
    if kind == "utf-32-be":
        return b[4:].decode("utf-32-be", "replace"), kind
    return b.decode("utf-8", "replace"), kind


def loadable_xml_asset_text(b: bytes) -> tuple[str, str]:
    """Emulates LoadableXmlAsset(FileInfo): strips exactly one UTF-8 BOM, decodes as UTF-8 always."""
    kind = sniff_bom(b)
    if b[:3] == b"\xef\xbb\xbf":
        b = b[3:]
    return b.decode("utf-8", "replace"), kind


DECL_RE = re.compile(r"<\?xml[^>]*\?>")
DECL_ENC_RE = re.compile(r"""encoding\s*=\s*(["'])(.*?)\1""", re.S)


def declared_encoding(text: str) -> str | None:
    m = DECL_RE.match(text.lstrip("\ufeff"))
    if not m:
        return None
    e = DECL_ENC_RE.search(m.group(0))
    return e.group(2) if e else None


def neutralize_decl(text: str) -> bytes:
    """Re-encode for lxml: the game parses an already decoded string and ignores the declared encoding."""
    if text.startswith("<?xml"):
        m = DECL_RE.match(text)
        if m:
            decl = DECL_ENC_RE.sub('encoding="utf-8"', m.group(0))
            text = decl + text[m.end():]
    return text.encode("utf-8", "replace")


ERR_CATEGORIES = [
    (re.compile(r"Document is empty|Start tag expected"), "empty_or_no_root"),
    (re.compile(r"Opening and ending tag mismatch"), "tag_mismatch"),
    (re.compile(r"Extra content at the end of the document"), "extra_content_after_root"),
    (re.compile(r"xmlParseEntityRef|EntityRef|Entity '|Entity "), "bad_entity_or_ampersand"),
    (re.compile(r"Premature end of data"), "truncated"),
    (re.compile(r"PCDATA invalid Char|invalid Char|Char 0x"), "invalid_char"),
    (re.compile(r"XML declaration allowed only at the start"), "xml_decl_not_first"),
    (re.compile(r"Namespace prefix"), "undeclared_namespace_prefix"),
    (re.compile(r"Attribute .* redefined"), "duplicate_attribute"),
    (re.compile(r"AttValue|attributes construct error|Unescaped '<'"), "bad_attribute_value"),
    (re.compile(r"StartTag: invalid element name"), "invalid_element_name"),
    (re.compile(r"Specification mandates value for attribute"), "attribute_without_value"),
    (re.compile(r"Input is not proper UTF-8|encoding error|Encoding error"), "encoding_error"),
    (re.compile(r"Couldn't find end of Start Tag|expected '>'"), "malformed_tag"),
]


def categorize_error(msg: str) -> str:
    for rx, name in ERR_CATEGORIES:
        if rx.search(msg):
            return name
    return "other"


def make_parser(remove_comments: bool = False) -> "etree.XMLParser":
    return etree.XMLParser(resolve_entities=False, no_network=True, huge_tree=True, recover=False,
                           remove_comments=remove_comments, remove_pis=False)


def parse_bytes_strict(data: bytes, remove_comments: bool = False):
    """Returns (root, error_dict_or_None)."""
    try:
        return etree.fromstring(data, make_parser(remove_comments)), None
    except etree.XMLSyntaxError as e:  # includes lxml error log text
        msg = str(e)
        return None, {"category": categorize_error(msg), "message": msg[:200],
                      "line": getattr(e, "lineno", None)}
    except ValueError as e:  # e.g. unicode strings with declarations
        return None, {"category": "other", "message": str(e)[:200], "line": None}


# ---------------------------------------------------------------------------
# emulation of VersionControl helpers (decompiled:RimWorld/VersionControl.cs)
# ---------------------------------------------------------------------------

_INT_RE = re.compile(r"^\s*[+-]?\d+\s*$")  # int.TryParse(NumberStyles.Integer), ASCII digits only


def dotnet_int(s: str) -> int | None:
    if _INT_RE.match(s):
        v = int(s.strip())
        if -(2 ** 31) <= v < 2 ** 31:
            return v
    return None


def try_parse_version_string(s: str | None) -> tuple[int, int] | None:
    """VersionControl.TryParseVersionString: >=2 dot parts, first two int-parsable and >= 0."""
    if s is None:
        return None
    parts = s.split(".")
    if len(parts) < 2:
        return None
    vals = []
    for p in parts[:2]:
        v = dotnet_int(p)
        if v is None or v < 0:
            return None
        vals.append(v)
    return vals[0], vals[1]


def is_well_formatted_version(s: str) -> bool:
    parts = s.split(".")
    if len(parts) != 2:
        return False
    return all((dotnet_int(p) is not None and dotnet_int(p) >= 0) for p in parts)


def version_from_string(s: str) -> tuple[int, int, int] | None:
    """VersionControl.VersionFromString: 1..3 int parts, throws otherwise (None here)."""
    if not s:
        return None
    parts = s.split(".")
    if len(parts) > 3:
        return None
    vals = []
    for p in parts:
        v = dotnet_int(p)
        if v is None or v < 0:
            return None
        vals.append(v)
    while len(vals) < 3:
        vals.append(0)
    return vals[0], vals[1], vals[2]


# ---------------------------------------------------------------------------
# About.xml analysis
# ---------------------------------------------------------------------------

def element_children(el) -> list:
    return [c for c in el if isinstance(c.tag, str)]


def has_non_text_children(el) -> bool:
    """True if the element has comment / PI / element children (a .NET string field then throws)."""
    for c in el:
        return True
    return False


def inner_text(el) -> str:
    return "".join(el.itertext())


def analyze_list_items(el) -> tuple[list[str], int, int]:
    """Returns (item texts for <li>, non-li element count, raw-text-in-list flag)."""
    items: list[str] = []
    non_li = 0
    for c in el:
        if not isinstance(c.tag, str):
            continue
        if c.tag != "li":
            non_li += 1
            continue
        items.append(inner_text(c))
    raw_text = 1 if (el.text and el.text.strip()) else 0
    return items, non_li, raw_text


def analyze_dependency_li(li) -> dict[str, Any]:
    d: dict[str, Any] = {"children": {}, "unknown": [], "case_variants": []}
    for c in element_children(li):
        canon = c.tag if c.tag in DEP_FIELDS else None
        if canon is None and c.tag.lower() in DEP_FIELDS_LC:
            canon = DEP_FIELDS_LC[c.tag.lower()]
            d["case_variants"].append((canon, c.tag))
        if canon is None:
            d["unknown"].append(c.tag)
            continue
        if canon == "alternativePackageIds":
            d["children"][canon] = [inner_text(x) for x in element_children(c) if x.tag == "li"]
        else:
            d["children"][canon] = None if has_non_text_children(c) else inner_text(c)
    return d


def validate_dependency(dep: dict[str, Any]) -> list[str]:
    """ModMetaDataInternal.ValidateDependencies: returns the list of reasons that make the game drop the entry."""
    reasons: list[str] = []
    pid = dep["children"].get("packageId")
    if pid is None or pid == "":
        reasons.append("missing_packageId")
    elif not PACKAGE_ID_RE.search(pid):
        reasons.append("invalid_packageId_format")
    dn = dep["children"].get("displayName")
    if dn is None or dn == "":
        reasons.append("empty_displayName")
    durl = dep["children"].get("downloadUrl") or ""
    surl = dep["children"].get("steamWorkshopUrl") or ""
    if durl == "" and surl == "":
        if pid is None:
            # the game dereferences packageId.ToLower() here: NullReferenceException when packageId is absent
            reasons.append("no_url_and_null_packageId_would_throw")
        elif "ludeon" not in pid.lower():
            reasons.append("no_downloadUrl_or_steamWorkshopUrl")
    return reasons


def analyze_about(raw: bytes, is_official: bool, game_major: int, game_minor: int) -> dict[str, Any]:
    r: dict[str, Any] = {"bytes": len(raw)}
    text, bom = dotnet_read_all_text(raw)
    r["bom"] = bom
    r["eol"] = ("crlf" if "\r\n" in text and "\n" not in text.replace("\r\n", "") else
                "mixed" if "\r\n" in text and "\n" in text.replace("\r\n", "") else
                "lf" if "\n" in text else "none")
    decl_enc = declared_encoding(text)
    r["declared_encoding"] = (decl_enc.lower() if decl_enc else None)
    r["leading_ws_before_decl"] = bool(text) and text[0] in " \t\r\n" and text.lstrip().startswith("<?xml")
    # strict UTF-8 validity of the raw bytes (only meaningful without a UTF-16/32 BOM)
    utf8_ok = None
    if bom in ("none", "utf-8-bom"):
        try:
            raw[3:].decode("utf-8") if bom == "utf-8-bom" else raw.decode("utf-8")
            utf8_ok = True
        except UnicodeDecodeError:
            utf8_ok = False
    r["utf8_strict_ok"] = utf8_ok
    # parse 1: raw bytes, honours the declared encoding (what a typical strict parser does)
    root_raw, err_raw = parse_bytes_strict(raw)
    r["raw_parse_ok"] = err_raw is None
    # parse 2: game-like (decode first, ignore the declaration)
    root, err = parse_bytes_strict(neutralize_decl(text))
    r["parse_ok"] = err is None
    if err_raw is not None and err is None:
        r["raw_fail_game_ok"] = True
    if err is not None:
        r["parse_error"] = err
        return r
    r["root_tag"] = root.tag
    fields: dict[str, dict[str, Any]] = {}
    unknown_tags: dict[str, int] = defaultdict(int)
    unknown_ignored: dict[str, int] = defaultdict(int)
    case_variants: list[tuple[str, str]] = []
    seen_counts: Counter = Counter()
    dropped_string_fields: list[str] = []
    list_issues: list[str] = []
    for ch in element_children(root):
        tag = ch.tag
        canon = tag if tag in ABOUT_FIELDS else None
        variant = False
        if canon is None and tag.lower() in ABOUT_FIELDS_LC:
            canon = ABOUT_FIELDS_LC[tag.lower()]
            variant = True
            case_variants.append((canon, tag))
        if canon is None:
            ignore_attr = (ch.get("IgnoreIfNoMatchingField") or "").lower() == "true"
            if ignore_attr:
                unknown_ignored[tag] += 1
            else:
                unknown_tags[tag] += 1
            continue
        seen_counts[canon] += 1
        info = fields.setdefault(canon, {"exact": 0, "variant": 0, "nonempty": 0})
        info["variant" if variant else "exact"] += 1
        if canon in STRING_FIELDS:
            if has_non_text_children(ch):
                dropped_string_fields.append(canon)
                continue
            val = inner_text(ch)
            if val.strip() != "" or val != "":
                # note: a whitespace-only value is an empty element for XmlDocument (insignificant whitespace)
                if val.strip() != "":
                    info["nonempty"] += 1
            info["value"] = val if canon in ("packageId", "name", "targetVersion", "modVersion") else None
            info["len"] = len(val)
        elif canon == "steamAppId":
            val = inner_text(ch).strip()
            info["value"] = val
            if val:
                info["nonempty"] += 1
        elif canon in LIST_STRING_FIELDS:
            items, non_li, raw_text = analyze_list_items(ch)
            info["items"] = items
            info["non_li"] = non_li
            if non_li:
                list_issues.append(f"{canon}:non_li_children")
            if raw_text:
                list_issues.append(f"{canon}:raw_text_in_list")
            if items:
                info["nonempty"] += 1
        elif canon == "modDependencies":
            deps = [analyze_dependency_li(li) for li in element_children(ch) if li.tag == "li"]
            non_li = sum(1 for li in element_children(ch) if li.tag != "li")
            info["deps"] = deps
            info["non_li"] = non_li
            if deps:
                info["nonempty"] += 1
        elif canon in BYVERSION_FIELDS:
            vkeys = []
            payload: dict[str, Any] = {}
            for vc in element_children(ch):
                key = vc.tag.lower()
                if key.startswith("v"):
                    key = key[1:]
                vkeys.append(key)
                if canon == "modDependenciesByVersion":
                    payload[key] = [analyze_dependency_li(li) for li in element_children(vc) if li.tag == "li"]
                elif canon == "descriptionsByVersion":
                    payload[key] = None
                else:
                    payload[key] = [inner_text(li) for li in element_children(vc) if li.tag == "li"]
            info["version_keys"] = vkeys
            info["payload"] = payload
            if vkeys:
                info["nonempty"] += 1
        else:  # description handled as string above; anything else
            info["nonempty"] += 1
    r["fields"] = fields
    r["unknown_tags"] = dict(unknown_tags)
    r["unknown_tags_ignored_by_attribute"] = dict(unknown_ignored)
    r["case_variants"] = case_variants
    r["duplicate_fields"] = sorted(k for k, v in seen_counts.items() if v > 1)
    r["dropped_string_fields"] = sorted(dropped_string_fields)
    r["list_issues"] = sorted(set(list_issues))

    # ---- derived: packageId
    pid_info = fields.get("packageId")
    pid = pid_info.get("value") if pid_info and "value" in pid_info else None
    if pid_info and pid_info["exact"] + pid_info["variant"] and "packageId" in dropped_string_fields:
        pid = None
    r["package_id"] = pid
    reasons: list[str] = []
    if pid is None or pid == "" or pid.strip() == "":
        r["package_id_missing"] = True
    else:
        if not PACKAGE_ID_RE.search(pid):
            reasons.append("regex")
            if pid != pid.strip():
                reasons.append("surrounding_whitespace")
            if " " in pid.strip():
                reasons.append("contains_space")
            if "." not in pid:
                reasons.append("no_dot")
            if len(pid) > 60:
                reasons.append("longer_than_60")
            if re.search(r"[^A-Za-z0-9.]", pid.strip()):
                reasons.append("char_outside_alnum_dot")
            if pid.strip().startswith(".") or pid.strip().endswith("."):
                reasons.append("leading_or_trailing_dot")
            if ".." in pid:
                reasons.append("consecutive_dots")
        if (not is_official) and "ludeon" in pid.lower():
            reasons.append("contains_ludeon_reserved")
        low = pid.strip().lower()
        if low.endswith("_steam") or low.endswith(".steam"):
            reasons.append("steam_suffix_declared")
    r["package_id_problems"] = reasons
    # ---- derived: supportedVersions
    sv = fields.get("supportedVersions")
    sv_items = sv["items"] if sv and "items" in sv else None
    r["supported_raw"] = sv_items
    parsed: list[tuple[int, int]] = []
    sv_unparsable: list[str] = []
    sv_malformed: list[str] = []
    if sv_items:
        for it in sv_items:
            pv = try_parse_version_string(it)
            if pv is None:
                sv_unparsable.append(it)
            else:
                parsed.append(pv)
                if not is_well_formatted_version(it):
                    sv_malformed.append(it)
    r["supported_parsed"] = sorted(set(parsed))
    r["supported_unparsable"] = sv_unparsable
    r["supported_malformed"] = sv_malformed
    r["supported_state"] = ("missing_list" if sv_items is None else "empty_list" if len(sv_items) == 0 else "has_entries")
    r["supports_current"] = (game_major, game_minor) in parsed
    r["supports_only_older"] = bool(parsed) and (game_major, game_minor) not in parsed and all(p < (game_major, game_minor) for p in parsed)
    r["supports_only_newer"] = bool(parsed) and (game_major, game_minor) not in parsed and all(p > (game_major, game_minor) for p in parsed)
    # ---- derived: dependencies (before / after the game's validation)
    md = fields.get("modDependencies")
    deps = md["deps"] if md and "deps" in md else []
    dep_reasons: Counter = Counter()
    effective = 0
    for d in deps:
        rs = validate_dependency(d)
        if rs:
            for x in rs:
                dep_reasons[x] += 1
        else:
            effective += 1
    r["dependencies_raw"] = len(deps)
    r["dependencies_effective"] = effective
    r["dependency_drop_reasons"] = dict(dep_reasons)
    r["dependency_dropped"] = len(deps) - effective
    r["dependency_alt_ids_used"] = sum(1 for d in deps if d["children"].get("alternativePackageIds"))
    r["dependency_child_unknown"] = sorted({u for d in deps for u in d["unknown"]})
    r["dependency_child_case_variants"] = sorted({f"{c}->{t}" for d in deps for c, t in d["case_variants"]})
    # ---- derived: references that mention package ids (for suffix and case statistics)
    refs: list[tuple[str, str]] = []
    for fld in ("loadBefore", "loadAfter", "forceLoadBefore", "forceLoadAfter", "incompatibleWith"):
        f = fields.get(fld)
        if f and "items" in f:
            refs.extend((fld, i) for i in f["items"])
    for d in deps:
        p = d["children"].get("packageId")
        if p:
            refs.append(("modDependencies", p))
        for a in d["children"].get("alternativePackageIds") or []:
            refs.append(("alternativePackageIds", a))
    r["refs"] = refs
    for fld in BYVERSION_FIELDS:
        f = fields.get(fld)
        if f:
            r.setdefault("byversion_keys", []).extend(f["version_keys"])
    # list sizes
    for fld in ("loadBefore", "loadAfter", "forceLoadBefore", "forceLoadAfter", "incompatibleWith", "authors"):
        f = fields.get(fld)
        r[f"n_{fld}"] = len(f["items"]) if f and "items" in f else 0
    r["has_authors_list"] = "authors" in fields
    r["name"] = fields.get("name", {}).get("value")
    r["name_dropped"] = "name" in dropped_string_fields
    return r


# ---------------------------------------------------------------------------
# LoadFolders.xml analysis (ModLoadFolders.LoadDataFromXmlCustom semantics)
# ---------------------------------------------------------------------------

def analyze_loadfolders(raw: bytes) -> dict[str, Any]:
    r: dict[str, Any] = {"bytes": len(raw)}
    text, bom = dotnet_read_all_text(raw)
    r["bom"] = bom
    root, err = parse_bytes_strict(neutralize_decl(text))
    if err is not None:
        r["parse_ok"] = False
        r["parse_error"] = err
        return r
    r["parse_ok"] = True
    r["root_tag"] = root.tag
    versions: dict[str, list[dict[str, Any]]] = {}
    issues: Counter = Counter()
    attr_use: Counter = Counter()
    li_names: Counter = Counter()
    for vc in element_children(root):
        key = vc.tag.lower()
        orig_key = vc.tag
        if key.startswith("v"):
            key = key[1:]
        entries = versions.setdefault(key, [])
        if orig_key != orig_key.lower() and not orig_key.startswith("v"):
            issues["version_tag_uppercase_V_or_other_case"] += 1
        for li in element_children(vc):
            li_names[li.tag] += 1
            conds = {}
            for an in ("IfModActive", "IfModActiveAll", "IfModNotActive"):
                if li.get(an) is not None:
                    conds[an] = [s.strip() for s in li.get(an).split(",")]
                    attr_use[an] += 1
            # case variants of the attribute names are NOT honoured by the game (attributes are case-sensitive)
            for an in li.attrib:
                if an.lower() in ("ifmodactive", "ifmodactiveall", "ifmodnotactive") and an not in ("IfModActive", "IfModActiveAll", "IfModNotActive"):
                    issues["condition_attribute_wrong_case_ignored_by_game"] += 1
                elif an not in ("IfModActive", "IfModActiveAll", "IfModNotActive"):
                    issues[f"unknown_li_attribute:{an}"] += 1
            raw_text = "".join(li.itertext())
            if raw_text in ("/", "\\"):
                path = ""
                issues["root_slash"] += 1
            else:
                path = raw_text
                if path != path.strip():
                    issues["path_has_surrounding_whitespace"] += 1
                if "\\" in path:
                    issues["path_has_backslash_(windows_only)"] += 1
                if path.startswith("/") and len(path) > 1:
                    issues["path_leading_slash_(absolute_on_unix)"] += 1
                if path.startswith("./") or path.startswith(".\\"):
                    issues["path_dot_prefix"] += 1
                if path.endswith("/") or path.endswith("\\"):
                    issues["path_trailing_slash"] += 1
                if path == "":
                    issues["empty_path"] += 1
            entries.append({"path": path, "conds": conds})
        if not entries:
            issues["empty_version_block"] += 1
    r["versions"] = versions
    r["issues"] = dict(issues)
    r["attr_use"] = dict(attr_use)
    r["li_names"] = dict(li_names)
    r["has_default_key"] = "default" in versions
    return r


def loadfolders_signature(lf: dict[str, Any]) -> str:
    """Structural signature: version keys -> ordered roles of the entries (names abstracted)."""
    parts = []
    for key in sorted(lf["versions"].keys(), key=lambda k: (version_from_string(k) or (9999, 0, 0), k)):
        roles = []
        for e in lf["versions"][key]:
            p = e["path"].replace("\\", "/").strip("/")
            if e["path"] in ("",) :
                role = "/"
            elif p == "Common" or p.lower() == "common":
                role = "Common"
            elif try_parse_version_string(p.split("/")[0]) is not None and "/" not in p:
                role = "<ver>"
            elif "/" in p and try_parse_version_string(p.split("/")[-1]) is not None:
                role = "<dir>/<ver>"
            elif "/" in p and try_parse_version_string(p.split("/")[0]) is not None:
                role = "<ver>/<dir>"
            else:
                role = "<dir>"
            c = ""
            if e["conds"]:
                c = "?" + "".join(sorted({"A" if k == "IfModActive" else "L" if k == "IfModActiveAll" else "N" for k in e["conds"]}))
            roles.append(role + c)
        parts.append(("default" if key == "default" else "v" + key) + ":[" + ",".join(roles) + "]")
    return "|".join(parts)


# ---------------------------------------------------------------------------
# effective load folders (ModContentPack.InitLoadFolders), for the active-set budget
# ---------------------------------------------------------------------------

def should_load(entry: dict[str, Any], active: set[str]) -> bool:
    c = entry["conds"]
    def norm(x: str) -> str:
        return x.strip().lower()
    if "IfModActive" in c and c["IfModActive"] and not any(norm(i) in active for i in c["IfModActive"]):
        return False
    if "IfModActiveAll" in c and c["IfModActiveAll"] and not all(norm(i) in active for i in c["IfModActiveAll"]):
        return False
    if "IfModNotActive" in c and c["IfModNotActive"] and any(norm(i) in active for i in c["IfModNotActive"]):
        return False
    return True


def resolve_load_folders(top_dirs: list[str], lf: dict[str, Any] | None, cur: tuple[int, int, int, int],
                         active: set[str]) -> tuple[str, list[str]]:
    """Returns (source, folders in DESCENDING priority order; '' is the mod root)."""
    cur_str_full = f"{cur[0]}.{cur[1]}.{cur[2]}"
    cur_str = f"{cur[0]}.{cur[1]}"
    versions = lf["versions"] if lf and lf.get("parse_ok") else {}
    if versions:
        lst = versions.get(cur_str_full)
        if lst:
            return "explicit_exact_build", _descend(lst, active)
        cands = []
        for k in versions:
            if k == "default" or not k or "." not in k:
                continue
            v = version_from_string(k)
            if v is None:
                continue
            if (v[0], v[1], v[2], -1) <= cur:
                cands.append(k)
        best = sorted(cands, reverse=True)[0] if cands else None  # string ordering, as the game does
        if best is not None:
            return ("explicit_exact_minor" if best == cur_str else "explicit_lower_version"), _descend(versions[best], active)
        if "default" in versions:
            return "explicit_default", _descend(versions["default"], active)
    folders: list[str] = []
    if cur_str in top_dirs:
        folders.append(cur_str)
        source = "implicit_exact_version_dir"
    else:
        parsed = []
        for d in top_dirs:
            pv = try_parse_version_string(d)
            if pv is not None:
                parsed.append(pv)
        parsed.sort()
        chosen = (0, 0)
        for item in parsed:
            if (item > chosen or chosen > (cur[0], cur[1])) and (item <= (cur[0], cur[1]) or chosen[0] == 0):
                chosen = item
        if chosen[0] > 0:
            # directory name as found on disk (any spelling that parses to the chosen version)
            name = next((d for d in top_dirs if try_parse_version_string(d) == chosen), f"{chosen[0]}.{chosen[1]}")
            folders.append(name)
            source = "implicit_nearest_lower" if chosen <= (cur[0], cur[1]) else "implicit_nearest_higher"
        else:
            source = "implicit_root_only"
    if "Common" in top_dirs:
        folders.append("Common")
    folders.append("")
    return source, folders


def _descend(entries: list[dict[str, Any]], active: set[str]) -> list[str]:
    out = []
    for e in reversed(entries):
        if should_load(e, active):
            out.append(e["path"])
    return out


# ---------------------------------------------------------------------------
# filesystem walk
# ---------------------------------------------------------------------------

def win_name_issues(name: str) -> list[str]:
    issues = []
    if any(c in WIN_BAD_CHARS for c in name):
        issues.append("reserved_char")
    if any(ord(c) < 32 for c in name):
        issues.append("control_char")
    if name not in (".", "..") and (name.endswith(" ") or name.endswith(".")):
        issues.append("trailing_space_or_dot")
    stem = name.split(".")[0].rstrip(" ").upper()
    if stem in WIN_RESERVED:
        issues.append("reserved_device_name")
    if len(name.encode("utf-16-le")) // 2 > 255:
        issues.append("name_longer_than_255")
    return issues


def walk_mod(mod_path: str, win_base_len: int) -> dict[str, Any]:
    """Iterative scandir walk. Symlinks are never followed. `.git` content is counted separately."""
    out: dict[str, Any] = {
        "files": 0, "dirs": 0, "bytes": 0, "xml_files": 0, "xml_bytes": 0,
        "git_files": 0, "git_dirs": 0, "git_bytes": 0,
        "symlink_files": 0, "symlink_dirs": 0, "symlink_examples": [],
        "max_rel_len": 0, "max_rel_len_bytes": 0, "max_rel_path": "", "win_over_259": 0, "win_over_259_examples": [],
        "win_issue_counts": Counter(), "win_issue_examples": [],
        "case_collisions": [], "non_ascii_names": 0, "hidden_files": 0,
        "ext_by_area": defaultdict(Counter), "files_list": [], "top_entries": [],
        "xml_ext_case_variants": Counter(), "tar_in_languages": 0, "unreadable": 0,
    }
    stack: list[tuple[str, str]] = [("", mod_path)]
    first = True
    while stack:
        rel_dir, abs_dir = stack.pop()
        try:
            with os.scandir(abs_dir) as it:
                entries = sorted(it, key=lambda e: e.name)
        except OSError:
            out["unreadable"] += 1
            continue
        names_lc: dict[str, list[str]] = defaultdict(list)
        for e in entries:
            name = e.name
            names_lc[name.lower()].append(name)
            rel = f"{rel_dir}/{name}" if rel_dir else name
            is_link = e.is_symlink()
            try:
                is_dir = e.is_dir(follow_symlinks=False)
                is_file = e.is_file(follow_symlinks=False)
            except OSError:
                out["unreadable"] += 1
                continue
            if first:
                kind = "dir" if is_dir else "file" if is_file else "link" if is_link else "other"
                out["top_entries"].append((safe_name(name), kind))
            in_git = rel.startswith(".git/") or rel == ".git" or "/.git/" in ("/" + rel + "/")
            # name hygiene
            iss = win_name_issues(name)
            if iss and not in_git:
                for x in iss:
                    out["win_issue_counts"][x] += 1
                if len(out["win_issue_examples"]) < EXAMPLE_LIMIT:
                    out["win_issue_examples"].append((safe_name(rel), iss))
            if any(ord(c) > 127 for c in name):
                out["non_ascii_names"] += 1
            if is_link:
                if os.path.isdir(e.path):
                    out["symlink_dirs"] += 1
                else:
                    out["symlink_files"] += 1
                if len(out["symlink_examples"]) < EXAMPLE_LIMIT:
                    out["symlink_examples"].append(safe_name(rel))
                continue
            rl = len(rel)
            if not in_git:
                if rl > out["max_rel_len"]:
                    out["max_rel_len"] = rl
                    out["max_rel_path"] = safe_name(rel)
                    out["max_rel_len_bytes"] = len(rel.encode("utf-8", "replace"))
                if win_base_len + 1 + rl >= WIN_MAX_PATH:
                    out["win_over_259"] += 1
                    if len(out["win_over_259_examples"]) < 3:
                        out["win_over_259_examples"].append((win_base_len + 1 + rl, safe_name(rel)))
            if is_dir:
                if name == ".git":
                    out["git_dirs"] += 1
                    stack.append((rel, e.path))
                    continue
                if in_git:
                    out["git_dirs"] += 1
                else:
                    out["dirs"] += 1
                stack.append((rel, e.path))
            elif is_file:
                try:
                    size = e.stat(follow_symlinks=False).st_size
                except OSError:
                    out["unreadable"] += 1
                    continue
                if in_git:
                    out["git_files"] += 1
                    out["git_bytes"] += size
                    continue
                out["files"] += 1
                out["bytes"] += size
                if name.startswith("."):
                    out["hidden_files"] += 1
                ext = os.path.splitext(name)[1]
                if ext.lower() == ".xml":
                    out["xml_files"] += 1
                    out["xml_bytes"] += size
                    if ext != ".xml":
                        out["xml_ext_case_variants"][ext] += 1
                parts = rel.split("/")
                area = None
                for comp in parts[:-1]:
                    if comp.lower() in AREA_KEYS_LC:
                        area = comp.lower()
                        break
                out["ext_by_area"][area or "other"][ext.lower() or "(none)"] += 1
                if area == "languages" and ext.lower() == ".tar":
                    out["tar_in_languages"] += 1
                out["files_list"].append((rel, size))
            else:
                pass
        for lc, group in names_lc.items():
            if len(group) > 1 and len(out["case_collisions"]) < EXAMPLE_LIMIT:
                out["case_collisions"].append((safe_name(rel_dir or "."), [safe_name(g) for g in group]))
            elif len(group) > 1:
                out["case_collisions"].append(None)  # keep the count only
        first = False
    out["case_collisions_count"] = len(out["case_collisions"])
    out["case_collisions"] = [c for c in out["case_collisions"] if c]
    return out


# ---------------------------------------------------------------------------
# Defs / Patches / Languages XML hygiene
# ---------------------------------------------------------------------------

def xml_area(rel: str) -> str:
    parts = rel.split("/")
    for comp in parts[:-1]:
        lc = comp.lower()
        if lc in AREA_KEYS_LC:
            return lc
    return "other"


def hygiene_for_file(abs_path: str, area: str) -> dict[str, Any]:
    """Parse one XML file the way LoadableXmlAsset does and classify problems."""
    r: dict[str, Any] = {}
    try:
        with open(abs_path, "rb") as f:
            raw = f.read()
    except OSError:
        return {"unreadable": True}
    text, bom = loadable_xml_asset_text(raw)
    r["bom"] = bom
    r["size"] = len(raw)
    if bom in ("none", "utf-8-bom"):
        try:
            (raw[3:] if bom == "utf-8-bom" else raw).decode("utf-8")
            r["utf8_ok"] = True
        except UnicodeDecodeError:
            r["utf8_ok"] = False
    else:
        r["utf8_ok"] = None
    de = declared_encoding(text)
    r["declared"] = de.lower() if de else None
    r["leading_ws_before_decl"] = bool(text) and text[0] in " \t\r\n" and text.lstrip().startswith("<?xml")
    r["doctype"] = "<!DOCTYPE" in text[:4096]
    root, err = parse_bytes_strict(neutralize_decl(text), remove_comments=True)
    if err is not None:
        r["parse_ok"] = False
        r["error"] = err
        return r
    r["parse_ok"] = True
    r["root"] = root.tag
    expected = {"defs": "Defs", "patches": "Patch"}.get(area)
    if expected and root.tag != expected:
        r["wrong_root"] = root.tag
    if area == "defs":
        kids = element_children(root)
        r["def_nodes"] = len(kids)
        tags = Counter(k.tag for k in kids)
        r["def_tags"] = dict(tags)
        r["mayrequire"] = sum(1 for k in kids if k.get("MayRequire") is not None)
        r["mayrequireanyof"] = sum(1 for k in kids if k.get("MayRequireAnyOf") is not None)
        r["abstract"] = sum(1 for k in kids if (k.get("Abstract") or "").lower() == "true")
        r["parentname"] = sum(1 for k in kids if k.get("ParentName") is not None)
        r["named"] = sum(1 for k in kids if k.get("Name") is not None)
        r["with_defname"] = sum(1 for k in kids if k.find("defName") is not None)
    elif area == "patches":
        ops = [k for k in element_children(root) if k.tag == "Operation"]
        r["operations"] = len(ops)
        r["non_operation_children"] = sum(1 for k in element_children(root) if k.tag != "Operation")
        r["toplevel_mayrequire"] = sum(1 for k in ops if k.get("MayRequire") is not None or k.get("MayRequireAnyOf") is not None)
        classes: Counter = Counter()
        mr_nested = 0

        def walk(el, depth=0):
            nonlocal mr_nested
            for c in element_children(el):
                if c.tag == "value":
                    continue  # payload, not an operation
                cls = c.get("Class")
                if cls is not None and (c.tag in ("Operation", "match", "nomatch") or (c.tag == "li" and el.tag == "operations")):
                    classes[cls] += 1
                if c.tag == "li" and el.tag == "operations" and (c.get("MayRequire") is not None or c.get("MayRequireAnyOf") is not None):
                    mr_nested += 1
                walk(c, depth + 1)

        for o in ops:
            c = o.get("Class")
            if c is not None:
                classes[c] += 1
            walk(o)
        r["classes"] = dict(classes)
        r["nested_mayrequire"] = mr_nested
    return r


# ---------------------------------------------------------------------------
# per-mod worker
# ---------------------------------------------------------------------------

def find_ci(entries: list[tuple[str, str]], want_lc: str, kind: str) -> list[str]:
    return [n for n, k in entries if n.lower() == want_lc and k == kind]


def parse_published_file_id(raw: bytes) -> dict[str, Any]:
    text, bom = dotnet_read_all_text(raw)
    stripped = text.strip()
    cls = "empty" if stripped == "" else "digits" if stripped.isascii() and stripped.isdigit() else "other"
    shape = []
    if bom != "none":
        shape.append("bom")
    if text.endswith("\n"):
        shape.append("trailing_newline")
    if text != text.strip() and not text.endswith("\n"):
        shape.append("surrounding_space")
    if "\n" in stripped:
        shape.append("multiline")
    value = int(stripped) if cls == "digits" and len(stripped) < 20 and int(stripped) < 2 ** 64 else None
    return {"class": cls, "shape": shape, "value": value}


def scan_one(task: dict[str, Any]) -> dict[str, Any]:
    root_label = task["root"]
    folder = task["folder"]
    mod_path = task["path"]
    game_major, game_minor = task["game_major"], task["game_minor"]
    cur_full = tuple(task["game_full"])
    win_prefix = task["win_prefix"]
    win_base_len = (len(win_prefix) + 1 + len(folder)) if win_prefix else 0
    rec: dict[str, Any] = {"root": root_label, "folder": safe_name(folder), "is_symlink_root": task["is_symlink"]}
    w = walk_mod(mod_path, win_base_len if win_prefix else 0)
    top = w["top_entries"]
    rec["top_dirs"] = sorted(n for n, k in top if k == "dir")
    rec["top_files"] = sorted(n for n, k in top if k == "file")
    # ------------------------------------------------------------ About
    about_dirs = find_ci(top, "about", "dir")
    rec["about_dir_variants"] = about_dirs
    about_dir_exact = "About" in about_dirs
    rec["about_dir_exact"] = about_dir_exact
    about_xml_name = None
    about_names: list[str] = []
    if about_dirs:
        ad = "About" if about_dir_exact else about_dirs[0]
        try:
            with os.scandir(os.path.join(mod_path, ad)) as it:
                about_entries = sorted(((e.name, "dir" if e.is_dir(follow_symlinks=False) else "file") for e in it))
        except OSError:
            about_entries = []
        about_names = [n for n, k in about_entries]
        cands = find_ci(about_entries, "about.xml", "file")
        about_xml_name = "About.xml" if "About.xml" in cands else (cands[0] if cands else None)
        rec["about_xml_variants"] = cands
        # sibling files the game or tools care about
        rec["preview_variants"] = sorted(n for n in about_names if n.lower().startswith("preview."))
        rec["preview_exact_png"] = "Preview.png" in about_names
        rec["modicon_variants"] = sorted(n for n in about_names if n.lower().startswith("modicon."))
        rec["modicon_exact_png"] = "ModIcon.png" in about_names
        rec["manifest_xml"] = any(n.lower() == "manifest.xml" for n in about_names)
        rec["pfid_variants"] = sorted(n for n in about_names if n.lower() == "publishedfileid.txt")
        rec["pfid_exact"] = "PublishedFileId.txt" in about_names
        rec["about_other_files"] = sorted(n for n in about_names if n.lower() not in ("about.xml", "publishedfileid.txt") and not n.lower().startswith(("preview.", "modicon.")) and n.lower() != "manifest.xml")
    rec["has_about_xml"] = about_xml_name is not None
    rec["about_path_actual"] = (f"{about_dirs[0] if about_dirs else '?'}/{about_xml_name}" if about_xml_name else None)
    about: dict[str, Any] | None = None
    if about_xml_name:
        ad = "About" if about_dir_exact else about_dirs[0]
        try:
            with open(os.path.join(mod_path, ad, about_xml_name), "rb") as f:
                raw = f.read(8 * 1024 * 1024)
            about = analyze_about(raw, task["is_official"], game_major, game_minor)
        except OSError:
            about = None
    rec["about"] = about
    # ------------------------------------------------------------ PublishedFileId.txt (exact name only is read by the game)
    pf = None
    if rec.get("pfid_exact"):
        try:
            with open(os.path.join(mod_path, "About", "PublishedFileId.txt"), "rb") as f:
                pf = parse_published_file_id(f.read(4096))
        except OSError:
            pf = None
    rec["pfid"] = pf
    # ------------------------------------------------------------ LoadFolders.xml
    lf_names = find_ci(top, "loadfolders.xml", "file")
    rec["loadfolders_variants"] = lf_names
    lf = None
    if lf_names:
        nm = "LoadFolders.xml" if "LoadFolders.xml" in lf_names else lf_names[0]
        try:
            with open(os.path.join(mod_path, nm), "rb") as f:
                lf = analyze_loadfolders(f.read(4 * 1024 * 1024))
        except OSError:
            lf = None
        if lf is not None and lf.get("parse_ok"):
            lf["signature"] = loadfolders_signature(lf)
            # dangling references (folder does not exist, exact-case check as on Linux)
            top_dirs = set(rec["top_dirs"])
            dangling = 0
            total_refs = 0
            for key, entries in lf["versions"].items():
                for e in entries:
                    total_refs += 1
                    p = e["path"]
                    if p == "":
                        continue
                    first_comp = p.replace("\\", "/").strip("/").split("/")[0]
                    if first_comp not in top_dirs:
                        dangling += 1
            lf["dangling_first_component"] = dangling
            lf["total_refs"] = total_refs
    rec["loadfolders"] = lf
    # ------------------------------------------------------------ top-level folder anatomy
    td = rec["top_dirs"]
    rec["area_dirs_exact"] = sorted(a for a in CONTENT_AREAS if a in td)
    rec["area_dirs_case_variants"] = sorted(d for d in td if d.lower() in {a.lower() for a in CONTENT_AREAS} and d not in CONTENT_AREAS)
    rec["common_variants"] = sorted(d for d in td if d.lower() == "common")
    vdirs_all = [d for d in td if re.match(r"^[vV]?\d+[._]\d+", d)]
    rec["version_like_dirs"] = sorted(vdirs_all)
    rec["version_dirs_game_recognized"] = sorted(d for d in td if try_parse_version_string(d) is not None)
    # content folders found inside version-like / Common / LoadFolders directories (casing variants)
    inner_variants: Counter = Counter()
    sub_dirs: set[str] = set(vdirs_all) | set(rec["common_variants"])
    if lf and lf.get("parse_ok"):
        for entries in lf["versions"].values():
            for e in entries:
                p = e["path"].replace("\\", "/").strip("/")
                if p:
                    sub_dirs.add(p.split("/")[0])
    for sd in sorted(sub_dirs):
        sp = os.path.join(mod_path, sd)
        if not os.path.isdir(sp):
            continue
        try:
            with os.scandir(sp) as it:
                for e in it:
                    if e.is_dir(follow_symlinks=False) and e.name.lower() in {a.lower() for a in CONTENT_AREAS} and e.name not in CONTENT_AREAS:
                        inner_variants[e.name] += 1
        except OSError:
            pass
    rec["inner_area_case_variants"] = dict(inner_variants)
    # ------------------------------------------------------------ walk results
    for k in ("files", "dirs", "bytes", "xml_files", "xml_bytes", "git_files", "git_dirs", "git_bytes", "symlink_files",
              "symlink_dirs", "max_rel_len", "max_rel_len_bytes", "max_rel_path", "win_over_259", "non_ascii_names",
              "hidden_files", "tar_in_languages", "unreadable", "case_collisions_count"):
        rec[k] = w[k]
    rec["symlink_examples"] = w["symlink_examples"]
    rec["win_over_259_examples"] = w["win_over_259_examples"]
    rec["win_issue_counts"] = dict(w["win_issue_counts"])
    rec["win_issue_examples"] = w["win_issue_examples"]
    rec["case_collisions"] = w["case_collisions"]
    rec["xml_ext_case_variants"] = dict(w["xml_ext_case_variants"])
    rec["ext_by_area"] = {a: dict(c) for a, c in w["ext_by_area"].items()}
    # nested About.xml deeper than the mod root (never read by the game)
    nested = [rel for rel, _ in w["files_list"] if rel.lower().endswith("/about/about.xml")]
    rec["nested_about_xml"] = sorted(safe_name(n) for n in nested)[:EXAMPLE_LIMIT]
    rec["nested_about_xml_count"] = len(nested)
    # ------------------------------------------------------------ XML hygiene over Defs / Patches / Languages
    hyg = {"defs": [], "patches": [], "languages": []}
    area_files: dict[str, list[tuple[str, int]]] = defaultdict(list)
    for rel, size in w["files_list"]:
        if rel.lower().endswith(".xml"):
            a = xml_area(rel)
            if a in hyg:
                area_files[a].append((rel, size))
    agg: dict[str, Any] = {}
    for a in ("defs", "patches", "languages"):
        s: dict[str, Any] = {"files": 0, "bytes": 0, "bom": Counter(), "utf8_invalid": 0, "declared": Counter(),
                             "parse_fail": 0, "fail_categories": Counter(), "fail_examples": [], "wrong_root": Counter(),
                             "doctype": 0, "leading_ws_before_decl": 0, "hidden_names": 0, "unreadable": 0,
                             "def_nodes": 0, "def_tags": Counter(), "mayrequire": 0, "mayrequireanyof": 0, "abstract": 0,
                             "parentname": 0, "named": 0, "with_defname": 0, "operations": 0, "classes": Counter(),
                             "toplevel_mayrequire": 0, "nested_mayrequire": 0, "non_operation_children": 0,
                             "utf16_or_32": 0, "declared_not_utf8_but_utf8_valid": 0}
        for rel, size in area_files[a]:
            name = rel.rsplit("/", 1)[-1]
            if name.startswith("."):
                s["hidden_names"] += 1
            h = hygiene_for_file(os.path.join(mod_path, *rel.split("/")), a)
            if h.get("unreadable"):
                s["unreadable"] += 1
                continue
            s["files"] += 1
            s["bytes"] += h["size"]
            s["bom"][h["bom"]] += 1
            if h["bom"] in ("utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be"):
                s["utf16_or_32"] += 1
            if h.get("utf8_ok") is False:
                s["utf8_invalid"] += 1
            s["declared"][h["declared"] or "(none)"] += 1
            if h["declared"] and h["declared"] not in ("utf-8", "utf8") and h.get("utf8_ok"):
                s["declared_not_utf8_but_utf8_valid"] += 1
            if h["doctype"]:
                s["doctype"] += 1
            if h["leading_ws_before_decl"]:
                s["leading_ws_before_decl"] += 1
            if not h["parse_ok"]:
                s["parse_fail"] += 1
                s["fail_categories"][h["error"]["category"]] += 1
                if len(s["fail_examples"]) < 3:
                    s["fail_examples"].append({"file": safe_name(rel), "category": h["error"]["category"], "message": h["error"]["message"], "line": h["error"]["line"]})
                continue
            if "wrong_root" in h:
                s["wrong_root"][h["wrong_root"]] += 1
            for kk in ("def_nodes", "mayrequire", "mayrequireanyof", "abstract", "parentname", "named", "with_defname",
                       "operations", "toplevel_mayrequire", "nested_mayrequire", "non_operation_children"):
                if kk in h:
                    s[kk] += h[kk]
            if "def_tags" in h:
                s["def_tags"].update(h["def_tags"])
            if "classes" in h:
                s["classes"].update(h["classes"])
        for kk in ("bom", "declared", "fail_categories", "wrong_root", "def_tags", "classes"):
            s[kk] = dict(s[kk])
        agg[a] = s
    rec["hygiene"] = agg
    # ------------------------------------------------------------ effective set for the active-set budget
    rec["files_for_effective"] = None
    if task.get("compute_effective") is not None:
        active = set(task["compute_effective"])
        top_dirs = rec["top_dirs"]
        source, folders = resolve_load_folders(top_dirs, lf, cur_full, active)
        eff = {"source": source, "folders": folders, "defs_files": 0, "defs_bytes": 0, "patches_files": 0, "patches_bytes": 0,
               "dll_files": 0, "dll_bytes": 0, "tex_files": 0, "tex_bytes": 0, "snd_files": 0, "snd_bytes": 0,
               "overshadowed_xml": 0, "ignored_hidden_xml": 0, "ignored_nonlower_ext": 0}
        seen: dict[str, set[str]] = defaultdict(set)
        # index files by (folder prefix, area) lazily
        flist = w["files_list"]
        for fold in folders:
            pref = (fold.replace("\\", "/").strip("/") + "/") if fold else ""
            for rel, size in flist:
                if not rel.startswith(pref):
                    continue
                sub = rel[len(pref):]
                head = sub.split("/", 1)[0]
                if head not in ("Defs", "Patches", "Assemblies", "Textures", "Sounds"):
                    continue
                name = sub.rsplit("/", 1)[-1]
                ext = os.path.splitext(name)[1]
                if head in ("Defs", "Patches"):
                    if ext.lower() != ".xml":
                        continue
                    if ext != ".xml":
                        eff["ignored_nonlower_ext"] += 1
                        continue
                    if name.startswith("."):
                        eff["ignored_hidden_xml"] += 1
                        continue
                    if sub in seen[head]:
                        eff["overshadowed_xml"] += 1
                        continue
                    seen[head].add(sub)
                    if head == "Defs":
                        eff["defs_files"] += 1
                        eff["defs_bytes"] += size
                    else:
                        eff["patches_files"] += 1
                        eff["patches_bytes"] += size
                elif head == "Assemblies" and ext.lower() == ".dll":
                    if sub in seen["Assemblies"]:
                        continue
                    seen["Assemblies"].add(sub)
                    eff["dll_files"] += 1
                    eff["dll_bytes"] += size
                elif head == "Textures" and ext.lower() in TEXTURE_EXT:
                    if sub in seen["Textures"]:
                        continue
                    seen["Textures"].add(sub)
                    eff["tex_files"] += 1
                    eff["tex_bytes"] += size
                elif head == "Sounds" and ext.lower() in AUDIO_EXT:
                    if sub in seen["Sounds"]:
                        continue
                    seen["Sounds"].add(sub)
                    eff["snd_files"] += 1
                    eff["snd_bytes"] += size
        rec["effective"] = eff
    # resolved folder source for every mod (unconditional view: all IfMod* conditions evaluated as inactive)
    src_inactive, folders_inactive = resolve_load_folders(rec["top_dirs"], lf, cur_full, set())
    rec["load_source_no_mods"] = src_inactive
    rec["load_folders_no_mods"] = folders_inactive
    return rec


# ---------------------------------------------------------------------------
# minimal VDF/ACF reader (only used for the set of installed workshop item ids)
# ---------------------------------------------------------------------------

def read_acf_item_ids(path: str) -> dict[str, Any] | None:
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            txt = f.read()
    except OSError:
        return None
    tokens = re.findall(r'"((?:[^"\\]|\\.)*)"|([{}])', txt)
    pos = 0

    def parse_obj() -> dict[str, Any]:
        nonlocal pos
        obj: dict[str, Any] = {}
        while pos < len(tokens):
            s, brace = tokens[pos]
            if brace == "}":
                pos += 1
                return obj
            key = s
            pos += 1
            if pos >= len(tokens):
                break
            s2, brace2 = tokens[pos]
            if brace2 == "{":
                pos += 1
                obj[key] = parse_obj()
            else:
                obj[key] = s2
                pos += 1
        return obj

    data = parse_obj()
    top = data.get("AppWorkshop") or {}
    inst = top.get("WorkshopItemsInstalled") or {}
    # keep ONLY ids and sizes; the details section contains account data and is never read
    return {"installed": {k: int(v.get("size", 0)) for k, v in inst.items() if isinstance(v, dict)}}


# ---------------------------------------------------------------------------
# aggregation
# ---------------------------------------------------------------------------

def add_example(lst: list, item: Any, limit: int = EXAMPLE_LIMIT) -> None:
    if len(lst) < limit:
        lst.append(item)


def mod_key(r: dict[str, Any]) -> str:
    return f"{r['root']}:{r['folder']}"


def aggregate(recs: list[dict[str, Any]], label: str, game_major: int, game_minor: int) -> dict[str, Any]:
    """Statistics over a set of mod records."""
    A: dict[str, Any] = {"label": label, "mods": len(recs)}
    with_dir = [r for r in recs if r["about_dir_variants"]]
    with_xml = [r for r in recs if r["has_about_xml"]]
    abouts = [r["about"] for r in recs if r["about"] is not None]
    ok = [a for a in abouts if a["parse_ok"]]
    bad = [(r, r["about"]) for r in recs if r["about"] is not None and not r["about"]["parse_ok"]]
    A["about"] = {
        "mod_dirs": len(recs),
        "with_about_dir_any_case": len(with_dir),
        "with_about_dir_exact_case": sum(1 for r in recs if r["about_dir_exact"]),
        "with_about_xml_any_case": len(with_xml),
        "without_about_xml": len(recs) - len(with_xml),
        "about_xml_parse_ok_gamelike": len(ok),
        "about_xml_parse_fail": len(bad),
        "about_xml_raw_fail_but_game_like_ok": sum(1 for a in abouts if a.get("raw_fail_game_ok")),
        "path_variants": dict(Counter(r["about_path_actual"] for r in recs if r["about_path_actual"])),
        "root_tag": dict(Counter(a["root_tag"] for a in ok)),
    }
    ghost = [mod_key(r) for r in recs if not r["has_about_xml"]]
    A["about"]["ghost_mod_examples"] = ghost[:EXAMPLE_LIMIT]
    # ---------------- field usage
    usage: dict[str, dict[str, int]] = {f: {"mods": 0, "exact": 0, "case_variant": 0, "nonempty": 0} for f in ABOUT_FIELDS}
    unknown: Counter = Counter()
    unknown_ignored: Counter = Counter()
    variants: dict[str, Counter] = defaultdict(Counter)
    for a in ok:
        for f, info in a["fields"].items():
            u = usage[f]
            u["mods"] += 1
            if info["exact"]:
                u["exact"] += 1
            if info["variant"]:
                u["case_variant"] += 1
            if info["nonempty"]:
                u["nonempty"] += 1
        for t in a["unknown_tags"]:
            unknown[t] += 1
        for t in a["unknown_tags_ignored_by_attribute"]:
            unknown_ignored[t] += 1
        for canon, spelled in a["case_variants"]:
            variants[canon][spelled] += 1
    A["field_usage"] = usage
    A["unknown_root_tags_logged_as_error"] = dict(sorted(unknown.items(), key=lambda kv: (-kv[1], kv[0])))
    A["unknown_root_tags_silenced_by_IgnoreIfNoMatchingField"] = dict(sorted(unknown_ignored.items(), key=lambda kv: (-kv[1], kv[0])))
    A["tag_case_variants"] = {k: dict(sorted(v.items())) for k, v in sorted(variants.items())}
    dep_child_var: Counter = Counter()
    dep_child_unknown: Counter = Counter()
    for a in ok:
        for x in a["dependency_child_case_variants"]:
            dep_child_var[x] += 1
        for x in a["dependency_child_unknown"]:
            dep_child_unknown[x] += 1
    A["dependency_child_tag_case_variants"] = dict(sorted(dep_child_var.items()))
    A["dependency_child_unknown_tags"] = dict(sorted(dep_child_unknown.items()))
    A["duplicate_field_in_one_file"] = dict(Counter(f for a in ok for f in a["duplicate_fields"]))
    A["string_fields_dropped_by_game_due_to_comment_or_child_nodes"] = dict(Counter(f for a in ok for f in a["dropped_string_fields"]))
    A["list_structure_issues"] = dict(Counter(f for a in ok for f in a["list_issues"]))
    # ---------------- encodings
    A["encodings"] = {
        "bom": dict(Counter(a["bom"] for a in abouts)),
        "declared_encoding": dict(Counter((a["declared_encoding"] or "(none)") for a in abouts)),
        "utf8_strict_invalid": sum(1 for a in abouts if a["utf8_strict_ok"] is False),
        "utf8_strict_invalid_examples": [mod_key(r) for r in recs if r["about"] and r["about"]["utf8_strict_ok"] is False][:EXAMPLE_LIMIT],
        "declared_non_utf8_but_bytes_valid_utf8": sum(1 for a in abouts if a["declared_encoding"] and a["declared_encoding"] not in ("utf-8", "utf8") and a["utf8_strict_ok"]),
        "eol": dict(Counter(a["eol"] for a in abouts)),
        "leading_whitespace_before_declaration": sum(1 for a in abouts if a["leading_ws_before_decl"]),
        "utf16_or_utf32_bom": sum(1 for a in abouts if a["bom"] in ("utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be")),
    }
    # ---------------- malformed
    cats = Counter(a["parse_error"]["category"] for r, a in bad)
    ex = []
    for r, a in bad[:EXAMPLE_LIMIT]:
        ex.append({"mod": mod_key(r), "category": a["parse_error"]["category"], "line": a["parse_error"]["line"], "message": a["parse_error"]["message"]})
    A["malformed_about_xml"] = {"count": len(bad), "by_category": dict(cats), "examples": ex}
    # ---------------- package ids
    pids: dict[str, list[tuple[str, str]]] = defaultdict(list)
    missing = []
    problems: Counter = Counter()
    invalid_examples: dict[str, list[str]] = defaultdict(list)
    for r in recs:
        a = r["about"]
        if a is None or not a["parse_ok"]:
            continue
        if a.get("package_id_missing"):
            missing.append(mod_key(r))
            continue
        pid = a["package_id"]
        pids[pid.strip().lower()].append((mod_key(r), pid))
        for pr in a["package_id_problems"]:
            problems[pr] += 1
            if len(invalid_examples[pr]) < 4:
                invalid_examples[pr].append(f"{mod_key(r)} -> {pid!r}")
    dups = {k: v for k, v in pids.items() if len(v) > 1}
    case_conf = {k: v for k, v in pids.items() if len({orig for _, orig in v}) > 1}
    A["package_ids"] = {
        "declared_unique_lowercase": len(pids),
        "missing_or_empty_count": len(missing),
        "missing_examples": missing[:EXAMPLE_LIMIT],
        "problems": dict(problems),
        "problem_examples": {k: v for k, v in sorted(invalid_examples.items())},
        "duplicate_ids": len(dups),
        "duplicates": [{"id": k, "entries": [m for m, _ in v]} for k, v in sorted(dups.items())],
        "case_conflict_ids": len(case_conf),
        "case_conflicts": [{"id": k, "spellings": sorted({o for _, o in v}), "mods": [m for m, _ in v]} for k, v in sorted(case_conf.items())],
        "mods_not_in_any_duplicate_group": sum(1 for k, v in pids.items() if len(v) == 1),
    }
    # references: suffix and case statistics
    ref_total = 0
    ref_steam_underscore = Counter()
    ref_steam_dot = Counter()
    ref_by_field = Counter()
    ref_with_ws = 0
    ref_case_mismatch_vs_declared = 0
    declared_spelling: dict[str, set[str]] = defaultdict(set)
    for k, v in pids.items():
        for _, o in v:
            declared_spelling[k].add(o)
    for a in ok:
        for fld, ref in a["refs"]:
            ref_total += 1
            ref_by_field[fld] += 1
            if ref != ref.strip():
                ref_with_ws += 1
            low = ref.strip().lower()
            if low.endswith("_steam"):
                ref_steam_underscore[fld] += 1
            if low.endswith(".steam"):
                ref_steam_dot[fld] += 1
            if low in declared_spelling and ref.strip() not in declared_spelling[low]:
                ref_case_mismatch_vs_declared += 1
    A["package_id_references"] = {
        "total": ref_total, "by_field": dict(ref_by_field),
        "ending_with__steam": dict(ref_steam_underscore), "ending_with_dot_steam": dict(ref_steam_dot),
        "with_surrounding_whitespace": ref_with_ws,
        "spelling_differs_from_declared_spelling_of_target": ref_case_mismatch_vs_declared,
    }
    # ---------------- supportedVersions
    sv_count: Counter = Counter()
    sv_raw_odd: Counter = Counter()
    sv_entries = []
    states: Counter = Counter()
    for a in ok:
        states[a["supported_state"]] += 1
        sv_entries.append(len(a["supported_raw"]) if a["supported_raw"] is not None else 0)
        for p in a["supported_parsed"]:
            sv_count[f"{p[0]}.{p[1]}"] += 1
        for it in a["supported_unparsable"]:
            sv_raw_odd[f"unparsable:{it}"] += 1
        for it in a["supported_malformed"]:
            sv_raw_odd[f"malformed:{it}"] += 1
    A["supported_versions"] = {
        "state": dict(states),
        "mods_per_version": dict(sorted(sv_count.items(), key=lambda kv: (tuple(int(x) for x in kv[0].split(".")), kv[0]))),
        "mods_supporting_current": sum(1 for a in ok if a["supports_current"]),
        "mods_only_older": sum(1 for a in ok if a["supports_only_older"]),
        "mods_only_newer": sum(1 for a in ok if a["supports_only_newer"]),
        "mods_with_any_unparsable_entry": sum(1 for a in ok if a["supported_unparsable"]),
        "mods_with_any_malformed_entry": sum(1 for a in ok if a["supported_malformed"]),
        "odd_raw_entries": dict(sorted(sv_raw_odd.items(), key=lambda kv: (-kv[1], kv[0]))[:40]),
        "entries_per_mod": dist(sv_entries),
        "mods_with_targetVersion_field": sum(1 for a in ok if "targetVersion" in a["fields"]),
    }
    # ---------------- dependencies
    raw_counts = [a["dependencies_raw"] for a in ok]
    eff_counts = [a["dependencies_effective"] for a in ok]
    drop: Counter = Counter()
    for a in ok:
        drop.update(a["dependency_drop_reasons"])
    A["dependencies"] = {
        "entries_per_mod_raw": dist(raw_counts),
        "entries_per_mod_effective_after_game_validation": dist(eff_counts),
        "mods_with_any_dependency": sum(1 for c in raw_counts if c),
        "total_entries_raw": sum(raw_counts),
        "total_entries_dropped_by_game": sum(a["dependency_dropped"] for a in ok),
        "mods_with_any_dropped_entry": sum(1 for a in ok if a["dependency_dropped"]),
        "drop_reasons": dict(drop),
        "mods_using_alternativePackageIds": sum(1 for a in ok if a["dependency_alt_ids_used"]),
        "mods_using_byVersion_fields": sum(1 for a in ok if a.get("byversion_keys")),
        "byVersion_keys": dict(Counter(k for a in ok for k in a.get("byversion_keys", []))),
        "loadBefore_per_mod": dist([a["n_loadBefore"] for a in ok]),
        "loadAfter_per_mod": dist([a["n_loadAfter"] for a in ok]),
        "forceLoadBefore_per_mod": dist([a["n_forceLoadBefore"] for a in ok]),
        "forceLoadAfter_per_mod": dist([a["n_forceLoadAfter"] for a in ok]),
        "incompatibleWith_per_mod": dist([a["n_incompatibleWith"] for a in ok]),
        "mods_using_authors_list": sum(1 for a in ok if a["has_authors_list"]),
    }
    # ---------------- sizes
    A["about_xml_bytes"] = dist([a["bytes"] for a in abouts])
    A["mod_size"] = {
        "files": dist([r["files"] for r in recs]),
        "dirs": dist([r["dirs"] for r in recs]),
        "bytes": dist([r["bytes"] for r in recs]),
        "xml_files": dist([r["xml_files"] for r in recs]),
        "xml_bytes": dist([r["xml_bytes"] for r in recs]),
        "git_files_total": sum(r["git_files"] for r in recs),
        "mods_with_git_dir": sum(1 for r in recs if r["git_dirs"] or r["git_files"]),
    }
    # ---------------- LoadFolders
    lfr = [(r, r["loadfolders"]) for r in recs if r["loadfolders"] is not None]
    lf_ok = [(r, lf) for r, lf in lfr if lf.get("parse_ok")]
    sigs = Counter(lf["signature"] for _, lf in lf_ok)
    sig_ex: dict[str, str] = {}
    for r, lf in lf_ok:
        sig_ex.setdefault(lf["signature"], mod_key(r))
    vkeys: Counter = Counter()
    iss: Counter = Counter()
    attrs: Counter = Counter()
    lif: Counter = Counter()
    for r, lf in lf_ok:
        for k in lf["versions"]:
            vkeys[k] += 1
        iss.update(lf["issues"])
        attrs.update(lf["attr_use"])
        lif.update(lf["li_names"])
    A["load_folders"] = {
        "present_any_case": len(lfr),
        "name_variants": dict(Counter(n for r in recs for n in r["loadfolders_variants"])),
        "parse_ok": len(lf_ok),
        "parse_fail": len(lfr) - len(lf_ok),
        "parse_fail_examples": [{"mod": mod_key(r), "category": lf["parse_error"]["category"], "message": lf["parse_error"]["message"]} for r, lf in lfr if not lf.get("parse_ok")][:EXAMPLE_LIMIT],
        "version_keys": dict(sorted(vkeys.items(), key=lambda kv: (-kv[1], kv[0]))),
        "mods_with_default_key": sum(1 for _, lf in lf_ok if lf["has_default_key"]),
        "mods_defining_current_minor": sum(1 for _, lf in lf_ok if f"{game_major}.{game_minor}" in lf["versions"]),
        "condition_attribute_usage_entries": dict(attrs),
        "li_element_names": dict(lif),
        "entry_issues_total": dict(iss),
        "mods_with_dangling_first_component": sum(1 for _, lf in lf_ok if lf["dangling_first_component"]),
        "mods_with_backslash_paths": sum(1 for _, lf in lf_ok if lf["issues"].get("path_has_backslash_(windows_only)")),
        "mods_with_whitespace_padded_paths": sum(1 for _, lf in lf_ok if lf["issues"].get("path_has_surrounding_whitespace")),
        "structural_variants_top": [{"signature": s, "mods": n, "example": sig_ex[s]} for s, n in sorted(sigs.items(), key=lambda kv: (-kv[1], kv[0]))[:25]],
        "structural_variants_distinct": len(sigs),
        "root_tag": dict(Counter(lf["root_tag"] for _, lf in lf_ok)),
    }
    # ---------------- folder anatomy
    area_present = Counter()
    for r in recs:
        for a in r["area_dirs_exact"]:
            area_present[a] += 1
    top_dir_names = Counter()
    for r in recs:
        for d in r["top_dirs"]:
            top_dir_names[d] += 1
    vdirs = Counter()
    vlike_odd = Counter()
    recognized = Counter()
    for r in recs:
        for d in r["version_like_dirs"]:
            vdirs[d] += 1
            if not re.match(r"^\d+\.\d+$", d):
                vlike_odd[d] += 1
        for d in r["version_dirs_game_recognized"]:
            recognized[d] += 1
    src = Counter(r["load_source_no_mods"] for r in recs if r["has_about_xml"])
    A["folder_anatomy"] = {
        "area_dirs_exact_case": dict(sorted(area_present.items(), key=lambda kv: (-kv[1], kv[0]))),
        "mods_with_common_dir_exact": sum(1 for r in recs if "Common" in r["top_dirs"]),
        "common_dir_case_variants": dict(Counter(d for r in recs for d in r["common_variants"])),
        "area_dir_case_variants_toplevel": dict(Counter(d for r in recs for d in r["area_dirs_case_variants"])),
        "area_dir_case_variants_inside_version_common_loadfolders_dirs": dict(Counter(k for r in recs for k, v in r["inner_area_case_variants"].items() for _ in range(v))),
        "mods_with_version_like_dirs": sum(1 for r in recs if r["version_like_dirs"]),
        "version_like_dir_names": dict(sorted(vdirs.items(), key=lambda kv: (-kv[1], kv[0]))),
        "version_like_dir_names_not_plain_major_minor": dict(sorted(vlike_odd.items(), key=lambda kv: (-kv[1], kv[0]))),
        "version_dirs_recognized_by_game_implicit_rule": dict(sorted(recognized.items(), key=lambda kv: (-kv[1], kv[0]))),
        "top_level_dir_names_top40": dict(sorted(top_dir_names.items(), key=lambda kv: (-kv[1], kv[0]))[:40]),
        "load_folder_source_when_no_other_mod_active": dict(sorted(src.items(), key=lambda kv: (-kv[1], kv[0]))),
        "mods_with_manifest_xml": sum(1 for r in recs if r.get("manifest_xml")),
        "mods_with_modicon_png_exact": sum(1 for r in recs if r.get("modicon_exact_png")),
        "mods_with_preview_png_exact": sum(1 for r in recs if r.get("preview_exact_png")),
        "preview_variants": dict(Counter(v for r in recs for v in r.get("preview_variants", []))),
        "modicon_variants": dict(Counter(v for r in recs for v in r.get("modicon_variants", []))),
        "mods_with_preview_not_exact_png": sum(1 for r in recs if r["about_dir_variants"] and not r.get("preview_exact_png")),
        "about_other_files_top": dict(sorted(Counter(v for r in recs for v in r.get("about_other_files", [])).items(), key=lambda kv: (-kv[1], kv[0]))[:20]),
        "mods_with_language_tar": sum(1 for r in recs if r["tar_in_languages"]),
        "xml_extension_case_variants_files": dict(Counter({k: v for r in recs for k, v in r["xml_ext_case_variants"].items()})),
        "mods_with_nested_about_xml_below_root": sum(1 for r in recs if r["nested_about_xml_count"]),
        "nested_about_xml_examples": [f"{mod_key(r)} -> {r['nested_about_xml'][0]}" for r in recs if r["nested_about_xml_count"]][:EXAMPLE_LIMIT],
    }
    # ---------------- extension histogram by area
    ext_area: dict[str, Counter] = defaultdict(Counter)
    for r in recs:
        for a, c in r["ext_by_area"].items():
            ext_area[a].update(c)
    A["extension_histogram_by_area_top"] = {a: dict(sorted(c.items(), key=lambda kv: (-kv[1], kv[0]))[:8]) for a, c in sorted(ext_area.items())}
    # ---------------- PublishedFileId.txt
    pf_present = [r for r in recs if r.get("pfid_variants")]
    pf_exact = [r for r in recs if r["pfid"] is not None]
    pf_cls = Counter(r["pfid"]["class"] for r in pf_exact)
    pf_shape = Counter(s for r in pf_exact for s in (r["pfid"]["shape"] or ["clean"]))
    mism = []
    match = 0
    for r in pf_exact:
        if r["root"] == "workshop" and r["pfid"]["value"] is not None and r["folder"].isdigit():
            if r["pfid"]["value"] == int(r["folder"]):
                match += 1
            else:
                mism.append({"mod": mod_key(r), "file_value": r["pfid"]["value"]})
    A["published_file_id"] = {
        "files_present_any_case": len(pf_present),
        "files_present_exact_case": len(pf_exact),
        "name_variants": dict(Counter(v for r in recs for v in r.get("pfid_variants", []))),
        "content_class": dict(pf_cls),
        "content_shape": dict(pf_shape),
        "workshop_folder_equals_file_value": match,
        "workshop_folder_differs_from_file_value": len(mism),
        "workshop_mismatch_examples": mism[:EXAMPLE_LIMIT],
        "workshop_mods_without_file": sum(1 for r in recs if r["root"] == "workshop" and r["pfid"] is None),
    }
    # ---------------- symlinks, windows names, path length
    A["symlinks"] = {
        "mod_roots_that_are_symlinks": sum(1 for r in recs if r["is_symlink_root"]),
        "symlinked_files_inside_mods": sum(r["symlink_files"] for r in recs),
        "symlinked_dirs_inside_mods": sum(r["symlink_dirs"] for r in recs),
        "examples": [f"{mod_key(r)} -> {r['symlink_examples'][0]}" for r in recs if r["symlink_examples"]][:EXAMPLE_LIMIT],
    }
    wi: Counter = Counter()
    wex: list = []
    for r in recs:
        wi.update(r["win_issue_counts"])
        for rel, iss_ in r["win_issue_examples"]:
            add_example(wex, f"{mod_key(r)} -> {rel} {iss_}")
    # folder-name level issues (the mod folder itself)
    root_issues = Counter()
    for r in recs:
        for x in win_name_issues(r["folder"]):
            root_issues[x] += 1
    A["windows_invalid_names"] = {
        "entries_by_issue_inside_mods": dict(wi),
        "mods_with_any_issue": sum(1 for r in recs if r["win_issue_counts"]),
        "mod_folder_names_by_issue": dict(root_issues),
        "examples": wex,
        "mods_with_case_only_name_collisions_in_a_dir": sum(1 for r in recs if r["case_collisions_count"]),
        "case_collision_examples": [f"{mod_key(r)} -> {r['case_collisions'][0]}" for r in recs if r["case_collisions"]][:EXAMPLE_LIMIT],
        "mods_with_non_ascii_names": sum(1 for r in recs if r["non_ascii_names"]),
        "non_ascii_name_entries": sum(r["non_ascii_names"] for r in recs),
        "hidden_files_total": sum(r["hidden_files"] for r in recs),
    }
    mx = max(recs, key=lambda r: (r["max_rel_len"], r["folder"])) if recs else None
    A["path_length"] = {
        "max_relative_path_chars_inside_mod": mx["max_rel_len"] if mx else 0,
        "max_relative_path_bytes_utf8_inside_mod": mx["max_rel_len_bytes"] if mx else 0,
        "max_relative_path_example": f"{mod_key(mx)} -> {mx['max_rel_path']}" if mx else None,
        "relative_path_len_per_mod_max": dist([r["max_rel_len"] for r in recs]),
        "entries_reaching_windows_max_path_with_default_prefix": sum(r["win_over_259"] for r in recs),
        "mods_with_such_entries": sum(1 for r in recs if r["win_over_259"]),
        "examples": [f"{mod_key(r)} -> len {r['win_over_259_examples'][0][0]}: {r['win_over_259_examples'][0][1]}" for r in recs if r["win_over_259_examples"]][:EXAMPLE_LIMIT],
    }
    # ---------------- XML hygiene over Defs/Patches/Languages
    H: dict[str, Any] = {}
    for a in ("defs", "patches", "languages"):
        s_files = sum(r["hygiene"][a]["files"] for r in recs)
        s_bytes = sum(r["hygiene"][a]["bytes"] for r in recs)
        bom: Counter = Counter()
        declared: Counter = Counter()
        fcat: Counter = Counter()
        wrong: Counter = Counter()
        for r in recs:
            h = r["hygiene"][a]
            bom.update(h["bom"])
            declared.update(h["declared"])
            fcat.update(h["fail_categories"])
            wrong.update(h["wrong_root"])
        H[a] = {
            "files": s_files, "bytes": s_bytes,
            "files_per_mod": dist([r["hygiene"][a]["files"] for r in recs]),
            "bytes_per_mod": dist([r["hygiene"][a]["bytes"] for r in recs]),
            "bom": dict(bom),
            "declared_encoding": dict(sorted(declared.items(), key=lambda kv: (-kv[1], kv[0]))[:12]),
            "utf16_or_utf32_files": sum(r["hygiene"][a]["utf16_or_32"] for r in recs),
            "invalid_utf8_files": sum(r["hygiene"][a]["utf8_invalid"] for r in recs),
            "declared_non_utf8_but_valid_utf8": sum(r["hygiene"][a]["declared_not_utf8_but_utf8_valid"] for r in recs),
            "doctype_files": sum(r["hygiene"][a]["doctype"] for r in recs),
            "leading_whitespace_before_declaration": sum(r["hygiene"][a]["leading_ws_before_decl"] for r in recs),
            "hidden_name_files_ignored_by_game": sum(r["hygiene"][a]["hidden_names"] for r in recs),
            "parse_fail_files": sum(r["hygiene"][a]["parse_fail"] for r in recs),
            "parse_fail_mods": sum(1 for r in recs if r["hygiene"][a]["parse_fail"]),
            "parse_fail_categories": dict(fcat),
            "parse_fail_examples": [dict(e, mod=mod_key(r)) for r in recs for e in r["hygiene"][a]["fail_examples"]][:EXAMPLE_LIMIT],
            "wrong_root_element": dict(wrong),
            "unreadable_files": sum(r["hygiene"][a]["unreadable"] for r in recs),
        }
        if a == "defs":
            tags: Counter = Counter()
            for r in recs:
                tags.update(r["hygiene"][a]["def_tags"])
            H[a].update({
                "top_level_def_nodes": sum(r["hygiene"][a]["def_nodes"] for r in recs),
                "def_nodes_per_mod": dist([r["hygiene"][a]["def_nodes"] for r in recs]),
                "nodes_with_defName_child": sum(r["hygiene"][a]["with_defname"] for r in recs),
                "nodes_with_MayRequire": sum(r["hygiene"][a]["mayrequire"] for r in recs),
                "nodes_with_MayRequireAnyOf": sum(r["hygiene"][a]["mayrequireanyof"] for r in recs),
                "nodes_Abstract_true": sum(r["hygiene"][a]["abstract"] for r in recs),
                "nodes_with_ParentName": sum(r["hygiene"][a]["parentname"] for r in recs),
                "nodes_with_Name": sum(r["hygiene"][a]["named"] for r in recs),
                "distinct_def_element_names": len(tags),
                "def_element_names_top30": dict(sorted(tags.items(), key=lambda kv: (-kv[1], kv[0]))[:30]),
            })
        if a == "patches":
            cls: Counter = Counter()
            for r in recs:
                cls.update(r["hygiene"][a]["classes"])
            H[a].update({
                "operations_top_level": sum(r["hygiene"][a]["operations"] for r in recs),
                "root_children_not_named_Operation": sum(r["hygiene"][a]["non_operation_children"] for r in recs),
                "top_level_Operation_with_MayRequire_ignored_by_game": sum(r["hygiene"][a]["toplevel_mayrequire"] for r in recs),
                "operations_list_li_with_MayRequire_honoured": sum(r["hygiene"][a]["nested_mayrequire"] for r in recs),
                "operation_class_usage_top40": dict(sorted(cls.items(), key=lambda kv: (-kv[1], kv[0]))[:40]),
                "distinct_operation_classes": len(cls),
            })
    A["xml_hygiene"] = H
    return A


def scan_budget(recs: list[dict[str, Any]], label: str) -> dict[str, Any]:
    """What a cold scan of this library touches, by scan level."""
    n = len(recs)
    top_entries = sum(len(r["top_dirs"]) + len(r["top_files"]) for r in recs)
    about_bytes = sum(r["about"]["bytes"] for r in recs if r["about"] is not None)
    lf_bytes = sum(r["loadfolders"]["bytes"] for r in recs if r["loadfolders"] is not None)
    pf_files = sum(1 for r in recs if r["pfid"] is not None)
    h_files = sum(r["hygiene"][a]["files"] for r in recs for a in ("defs", "patches"))
    h_bytes = sum(r["hygiene"][a]["bytes"] for r in recs for a in ("defs", "patches"))
    lang_files = sum(r["hygiene"]["languages"]["files"] for r in recs)
    lang_bytes = sum(r["hygiene"]["languages"]["bytes"] for r in recs)
    dirs = sum(r["dirs"] for r in recs)
    files = sum(r["files"] for r in recs)
    bytes_ = sum(r["bytes"] for r in recs)
    git_files = sum(r["git_files"] for r in recs)
    level0 = {
        "what": "list each mod root, read About/About.xml + LoadFolders.xml + PublishedFileId.txt, stat Preview.png/ModIcon.png",
        "readdir_calls": n + sum(1 for r in recs if r["about_dir_variants"]),
        "directory_entries_listed": top_entries + sum(len(r.get("about_other_files", [])) + 4 for r in recs if r["about_dir_variants"]),
        "files_opened": sum(1 for r in recs if r["has_about_xml"]) + sum(1 for r in recs if r["loadfolders"] is not None) + pf_files,
        "bytes_read": about_bytes + lf_bytes + (pf_files * 12),
    }
    level1 = {
        "what": "level 0 + a def index over every Defs/** and Patches/** XML file in any folder (no load-order logic)",
        "files_opened": level0["files_opened"] + h_files,
        "bytes_read": level0["bytes_read"] + h_bytes,
        "xml_files_defs_and_patches": h_files,
        "xml_bytes_defs_and_patches": h_bytes,
    }
    level2 = {
        "what": "level 1 + Languages/** XML (translation index)",
        "files_opened": level1["files_opened"] + lang_files,
        "bytes_read": level1["bytes_read"] + lang_bytes,
    }
    level3 = {
        "what": "full recursive walk with stat of every entry (size accounting, path checks), no content reads beyond level 0",
        "files_stat": files,
        "dirs_listed": dirs + n,
        "total_bytes_on_disk_apparent": bytes_,
        "extra_files_if_git_dirs_are_not_skipped": git_files,
    }
    return {"label": label, "mods": n, "level0_metadata_only": level0, "level1_defs_patches_index": level1,
            "level2_plus_languages": level2, "level3_full_walk": level3}


# ---------------------------------------------------------------------------
# ModsConfig.xml (active set) support
# ---------------------------------------------------------------------------

def read_mods_config(path: str) -> dict[str, Any] | None:
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError:
        return None
    text, _ = dotnet_read_all_text(raw)
    root, err = parse_bytes_strict(neutralize_decl(text))
    if err is not None:
        return None
    version = None
    active: list[str] = []
    known: list[str] = []
    for ch in element_children(root):
        if ch.tag in ("version", "buildNumber"):
            version = inner_text(ch)
        elif ch.tag == "activeMods":
            active = [inner_text(li) for li in element_children(ch) if li.tag == "li"]
        elif ch.tag == "knownExpansions":
            known = [inner_text(li) for li in element_children(ch) if li.tag == "li"]
    return {"version": version, "active": active, "known": known}


def active_set_budget(recs: list[dict[str, Any]], mc: dict[str, Any]) -> dict[str, Any]:
    """Resolve ModsConfig active ids against the library like ModLister.TryAddMod, then sum effective files."""
    order = {"official": 0, "install_mods": 1, "workshop": 2}
    cands = [r for r in recs if r["root"] in order and r["has_about_xml"] and r["about"] and r["about"]["parse_ok"] and not r["about"].get("package_id_missing")]
    cands.sort(key=lambda r: (order[r["root"]], int(r["folder"]) if r["root"] == "workshop" and r["folder"].isdigit() else 0, r["folder"]))
    registered: dict[str, dict[str, Any]] = {}
    postfixed: set[str] = set()
    dup_errors = 0
    for r in cands:
        pid = r["about"]["package_id"].strip().lower()
        key = pid
        if key in registered:
            other = registered[key]
            if (r["root"] == "workshop") != (other["root"] == "workshop"):
                key = pid + "_steam"
                postfixed.add(mod_key(r))
                if key in registered:
                    dup_errors += 1
                    continue
            else:
                dup_errors += 1
                continue
        registered[key] = r
    active = [a.strip().lower() for a in mc["active"]]
    matched = []
    unmatched = []
    for a in active:
        r = registered.get(a)
        if r is None and a.endswith("_steam"):
            r = registered.get(a[: -len("_steam")])
        if r is None:
            unmatched.append(a)
        else:
            matched.append((a, r))
    tot = Counter()
    sources = Counter()
    per_mod_defs = []
    for a, r in matched:
        eff = r.get("effective")
        if not eff:
            continue
        sources[eff["source"]] += 1
        for k in ("defs_files", "defs_bytes", "patches_files", "patches_bytes", "dll_files", "dll_bytes", "tex_files", "tex_bytes",
                  "snd_files", "snd_bytes", "overshadowed_xml", "ignored_hidden_xml", "ignored_nonlower_ext"):
            tot[k] += eff[k]
        per_mod_defs.append(eff["defs_files"] + eff["patches_files"])
    by_root = Counter(r["root"] for _, r in matched)
    return {
        "modsconfig_version_string": mc["version"],
        "active_entries": len(active),
        "matched_to_installed_mods": len(matched),
        "unmatched_entries": len(unmatched),
        "unmatched_examples": unmatched[:EXAMPLE_LIMIT],
        "matched_by_root": dict(by_root),
        "duplicate_package_id_errors_in_library": dup_errors,
        "workshop_copies_that_get_the_steam_postfix": len(postfixed),
        "effective_load_folder_source": dict(sources),
        "effective_totals": dict(tot),
        "effective_defs_plus_patches_files_per_active_mod": dist(per_mod_defs),
    }


# ---------------------------------------------------------------------------
# driver
# ---------------------------------------------------------------------------

def list_children(path: str) -> list[tuple[str, str, bool]]:
    """Direct child directories of a library root: (name, abs path, is_symlink), sorted by name.
    Mirrors the game: every directory counts as a mod candidate, files are ignored."""
    out = []
    with os.scandir(path) as it:
        for e in it:
            try:
                if e.is_dir():  # follows symlinks like DirectoryInfo.GetDirectories + Directory.Exists
                    out.append((e.name, e.path, e.is_symlink()))
            except OSError:
                pass
    out.sort(key=lambda t: t[0])
    return out


def parse_game_version(s: str) -> tuple[int, int, int, int]:
    m = re.match(r"^\s*(\d+)\.(\d+)(?:\.(\d+))?(?:\s+rev(\d+))?\s*$", s)
    if not m:
        raise SystemExit(f"bad --game-version {s!r}")
    return int(m.group(1)), int(m.group(2)), int(m.group(3) or 0), int(m.group(4) or 0)


def selftest() -> None:
    assert PACKAGE_ID_RE.search("author.mod")
    assert PACKAGE_ID_RE.search("Ludeon.RimWorld.Royalty")
    assert not PACKAGE_ID_RE.search("nodot")
    assert not PACKAGE_ID_RE.search(".lead.dot")
    assert not PACKAGE_ID_RE.search("trail.dot.")
    assert not PACKAGE_ID_RE.search("two..dots")
    assert not PACKAGE_ID_RE.search("has space.mod")
    assert not PACKAGE_ID_RE.search("under_score.mod")
    assert not PACKAGE_ID_RE.search("hy-phen.mod")
    assert not PACKAGE_ID_RE.search("a." + "b" * 60)
    assert try_parse_version_string("1.6") == (1, 6)
    assert try_parse_version_string("1.6.4871") == (1, 6)
    assert try_parse_version_string(" 1. 6 ") == (1, 6)
    assert try_parse_version_string("v1.6") is None
    assert try_parse_version_string("1") is None
    assert try_parse_version_string("1.x") is None
    assert is_well_formatted_version("1.6") and not is_well_formatted_version("1.6.4871")
    assert version_from_string("1.6") == (1, 6, 0)
    assert version_from_string("1.6.4871") == (1, 6, 4871)
    assert version_from_string("1.6.4871.1") is None
    assert pct([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 95) == 10
    assert pct([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 50) == 5
    cur = (1, 6, 4871, 598)
    assert resolve_load_folders(["1.6", "Common"], None, cur, set()) == ("implicit_exact_version_dir", ["1.6", "Common", ""])
    assert resolve_load_folders(["1.4", "1.5"], None, cur, set())[1] == ["1.5", ""]
    assert resolve_load_folders(["1.7"], None, cur, set())[1] == ["1.7", ""]
    assert resolve_load_folders([], None, cur, set()) == ("implicit_root_only", [""])
    lf = {"parse_ok": True, "versions": {"1.5": [{"path": "", "conds": {}}, {"path": "1.5", "conds": {}}]}}
    assert resolve_load_folders(["1.5"], lf, cur, set()) == ("explicit_lower_version", ["1.5", ""])
    lf2 = {"parse_ok": True, "versions": {"1.6": [{"path": "", "conds": {}}, {"path": "1.6", "conds": {}},
                                                   {"path": "Odyssey", "conds": {"IfModActive": ["Ludeon.RimWorld.Odyssey"]}}]}}
    assert resolve_load_folders([], lf2, cur, set())[1] == ["1.6", ""]
    assert resolve_load_folders([], lf2, cur, {"ludeon.rimworld.odyssey"})[1] == ["Odyssey", "1.6", ""]
    d = analyze_dependency_li(etree.fromstring("<li><packageId>a.b</packageId><displayName>A</displayName></li>"))
    assert validate_dependency(d) == ["no_downloadUrl_or_steamWorkshopUrl"]
    d2 = analyze_dependency_li(etree.fromstring("<li><packageId>Ludeon.RimWorld.Biotech</packageId><displayName>B</displayName></li>"))
    assert validate_dependency(d2) == []
    print("selftest ok")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--workshop", default=os.environ.get("RIMSTUDIO_WORKSHOP_DIR"))
    ap.add_argument("--install-mods", default=os.environ.get("RIMSTUDIO_INSTALL_MODS_DIR"))
    ap.add_argument("--owner-mods", default=os.environ.get("RIMSTUDIO_OWNER_MODS_DIR"))
    ap.add_argument("--official-data", default=os.environ.get("RIMSTUDIO_OFFICIAL_DATA_DIR"))
    ap.add_argument("--mods-config", default=os.environ.get("RIMSTUDIO_MODS_CONFIG"))
    ap.add_argument("--workshop-acf", default=os.environ.get("RIMSTUDIO_WORKSHOP_ACF"))
    ap.add_argument("--game-version", default="1.6.4871 rev598", help="running game version (Version.txt content)")
    ap.add_argument("--win-workshop-prefix", default=DEFAULT_WIN_WORKSHOP_PREFIX)
    ap.add_argument("--win-mods-prefix", default=DEFAULT_WIN_MODS_PREFIX)
    ap.add_argument("--out", required=False)
    ap.add_argument("--mods-csv", required=False)
    ap.add_argument("--workers", type=int, default=min(16, os.cpu_count() or 4))
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        selftest()
        return 0
    if not args.out:
        ap.error("--out is required")
    gv = parse_game_version(args.game_version)
    cur_full = gv
    roots = [("workshop", args.workshop, args.win_workshop_prefix, False),
             ("install_mods", args.install_mods, args.win_mods_prefix, False),
             ("owner_mods", args.owner_mods, None, False),
             ("official", args.official_data, None, True)]

    mc = read_mods_config(args.mods_config) if args.mods_config else None
    active_norm: set[str] | None = None
    if mc:
        active_norm = set()
        for a in mc["active"]:
            a = a.strip().lower()
            active_norm.add(a)
            if a.endswith("_steam"):
                active_norm.add(a[: -len("_steam")])

    tasks: list[dict[str, Any]] = []
    root_info: dict[str, Any] = {}
    for label, path, win_prefix, is_official in roots:
        if not path or not os.path.isdir(path):
            root_info[label] = {"present": False}
            continue
        kids = list_children(path)
        root_info[label] = {"present": True, "mod_dirs": len(kids)}
        for name, p, is_link in kids:
            tasks.append({"root": label, "folder": name, "path": p, "is_symlink": is_link, "is_official": is_official,
                          "game_major": gv[0], "game_minor": gv[1], "game_full": list(cur_full), "win_prefix": win_prefix,
                          "compute_effective": (sorted(active_norm) if (active_norm is not None and label != "owner_mods") else None)})
    print(f"scanning {len(tasks)} mod directories with {args.workers} workers", file=sys.stderr)
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        recs = list(ex.map(scan_one, tasks, chunksize=4))
    order = {"workshop": 0, "install_mods": 1, "owner_mods": 2, "official": 3}
    recs.sort(key=lambda r: (order[r["root"]], r["folder"]))

    requested = [r for r in recs if r["root"] in ("workshop", "install_mods", "owner_mods")]
    summary: dict[str, Any] = {"schema": SCHEMA, "game_version_assumed": args.game_version, "roots": root_info}
    summary["all_requested_roots"] = aggregate(requested, "workshop+install_mods+owner_mods", gv[0], gv[1])
    summary["per_root"] = {}
    for label in ("workshop", "install_mods", "owner_mods", "official"):
        rs = [r for r in recs if r["root"] == label]
        if rs:
            summary["per_root"][label] = aggregate(rs, label, gv[0], gv[1])
    summary["scan_budget"] = {"all_requested_roots": scan_budget(requested, "workshop+install_mods+owner_mods")}
    for label in ("workshop", "install_mods", "owner_mods", "official"):
        rs = [r for r in recs if r["root"] == label]
        if rs:
            summary["scan_budget"][label] = scan_budget(rs, label)
    # cross-root duplicates (same package id in different roots): local copy vs workshop copy
    pid_map: dict[str, list[str]] = defaultdict(list)
    for r in requested:
        a = r["about"]
        if a and a["parse_ok"] and not a.get("package_id_missing"):
            pid_map[a["package_id"].strip().lower()].append(r["root"])
    cross = Counter()
    for k, roots_ in pid_map.items():
        if len(roots_) > 1:
            cross["+".join(sorted(roots_))] += 1
    summary["duplicate_package_ids_by_root_combination"] = dict(sorted(cross.items()))
    # workshop acf vs folders
    wdir = args.workshop
    acf_path = args.workshop_acf or (os.path.join(os.path.dirname(os.path.dirname(wdir)), "appworkshop_294100.acf") if wdir else None)
    acf = read_acf_item_ids(acf_path) if acf_path else None
    if acf is not None:
        folder_ids = {r["folder"] for r in recs if r["root"] == "workshop"}
        installed = set(acf["installed"].keys())
        summary["workshop_acf_cross_check"] = {
            "installed_item_ids_in_acf": len(installed),
            "folders_on_disk": len(folder_ids),
            "ids_in_acf_without_folder": sorted(installed - folder_ids),
            "folders_without_acf_entry": sorted(folder_ids - installed),
            "acf_declared_bytes_total": sum(acf["installed"].values()),
            "note": "only item ids and sizes are read; the details section (account data) is never parsed",
        }
    # owner folder structure: container folders that hold nested mods
    owner = [r for r in recs if r["root"] == "owner_mods"]
    summary["owner_mods_structure"] = {
        "direct_children": len(owner),
        "direct_children_with_about_xml": sum(1 for r in owner if r["has_about_xml"]),
        "direct_children_without_about_xml": [r["folder"] for r in owner if not r["has_about_xml"]],
        "children_without_about_that_contain_nested_about_xml": [{"folder": r["folder"], "nested": r["nested_about_xml"]} for r in owner if not r["has_about_xml"] and r["nested_about_xml_count"]],
        "children_with_git_dir": sum(1 for r in owner if r["git_dirs"] or r["git_files"]),
        "children_with_rimstudio_project_json": sum(1 for r in owner if r["has_about_xml"] and os.path.exists(os.path.join(next(t["path"] for t in tasks if t["root"] == "owner_mods" and t["folder"] == r["folder"]), "Config", "rimstudio.project.json"))),
    }
    # active set from ModsConfig.xml
    if mc:
        summary["active_set_budget"] = active_set_budget(recs, mc)
    # top-level totals
    summary["totals_all_requested_roots"] = {
        "mod_dirs": len(requested),
        "files": sum(r["files"] for r in requested),
        "dirs": sum(r["dirs"] for r in requested),
        "bytes": sum(r["bytes"] for r in requested),
        "xml_files": sum(r["xml_files"] for r in requested),
        "xml_bytes": sum(r["xml_bytes"] for r in requested),
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=1, sort_keys=True, ensure_ascii=False)
        f.write("\n")
    if args.mods_csv:
        cols = ["root", "folder", "has_about_xml", "about_path_actual", "about_bytes", "about_bom", "about_declared_encoding",
                "about_parse_ok", "package_id", "package_id_problems", "name", "supported_versions", "supports_current",
                "deps_raw", "deps_effective", "has_loadfolders", "loadfolders_signature", "version_like_dirs", "common_dir",
                "files", "dirs", "bytes", "xml_files", "xml_bytes", "defs_files", "patches_files", "git_files",
                "symlinks", "max_rel_path_chars", "win_name_issues", "published_file_id", "load_source_no_other_mods"]
        with open(args.mods_csv, "w", encoding="utf-8", newline="") as f:
            wr = csv.writer(f, lineterminator="\n")
            wr.writerow(cols)
            for r in recs:
                a = r["about"] or {}
                lf = r["loadfolders"] or {}
                wr.writerow([
                    r["root"], r["folder"], int(r["has_about_xml"]), r["about_path_actual"] or "", a.get("bytes", ""),
                    a.get("bom", ""), a.get("declared_encoding") or "", int(bool(a.get("parse_ok"))) if a else "",
                    a.get("package_id") or "", "|".join(a.get("package_id_problems", [])), (a.get("name") or "")[:80],
                    "|".join(a.get("supported_raw") or []), int(bool(a.get("supports_current"))) if a else "",
                    a.get("dependencies_raw", ""), a.get("dependencies_effective", ""), int(r["loadfolders"] is not None),
                    lf.get("signature", ""), "|".join(r["version_like_dirs"]), int("Common" in r["top_dirs"]),
                    r["files"], r["dirs"], r["bytes"], r["xml_files"], r["xml_bytes"], r["hygiene"]["defs"]["files"],
                    r["hygiene"]["patches"]["files"], r["git_files"], r["symlink_files"] + r["symlink_dirs"],
                    r["max_rel_len"], sum(r["win_issue_counts"].values()),
                    (r["pfid"]["value"] if r["pfid"] and r["pfid"]["value"] is not None else ""), r["load_source_no_mods"],
                ])
    print(f"wrote {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
