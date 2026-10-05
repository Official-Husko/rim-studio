"""Mod metadata, load-folder resolution and asset discovery.

Mirrors ModMetaData.Init, ModLoadFolders, LoadFolder.ShouldLoad,
ModContentPack.InitLoadFolders and DirectXmlLoader.XmlAssetsInModFolder.
"""
from __future__ import annotations

import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set

from .diag import Diagnostics
from .xmlnet import XmlParseError, child_elements, inner_text, parse_xml_bytes

CORE_PACKAGE_ID = "ludeon.rimworld"
STEAM_POSTFIX = "_steam"


# ---------------------------------------------------------------------------- versions

@dataclass(frozen=True, order=True)
class Version:
    """System.Version: components that were not specified are -1."""

    major: int
    minor: int
    build: int = -1
    revision: int = -1

    def __str__(self):
        parts = [self.major, self.minor]
        if self.build >= 0:
            parts.append(self.build)
            if self.revision >= 0:
                parts.append(self.revision)
        return ".".join(str(p) for p in parts)


@dataclass(frozen=True)
class GameVersion:
    """The running game version. RimWorld 1.6.4871 rev598 -> major 1, minor 6, build 4871."""

    major: int = 1
    minor: int = 6
    build: int = 4871
    rev: int = 598

    @property
    def current(self) -> Version:
        return Version(self.major, self.minor, self.build, self.rev)

    @property
    def string(self) -> str:                       # VersionControl.CurrentVersionString
        return "%d.%d.%d" % (self.major, self.minor, self.build)

    @property
    def string_without_build(self) -> str:         # VersionControl.CurrentVersionStringWithoutBuild
        return "%d.%d" % (self.major, self.minor)

    @classmethod
    def parse(cls, text: str) -> "GameVersion":
        m = re.match(r"\s*(\d+)\.(\d+)\.(\d+)(?:\s+rev(\d+))?", text)
        if not m:
            raise ValueError("cannot parse game version %r" % text)
        return cls(int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4) or 0))

    @classmethod
    def from_install(cls, game_dir: os.PathLike) -> "GameVersion":
        return cls.parse(Path(game_dir, "Version.txt").read_text(encoding="utf-8-sig"))


_INT_RE = re.compile(r"^\s*[+-]?\d+\s*$")


def _int_try_parse(s: str) -> Optional[int]:
    return int(s) if _INT_RE.match(s) else None


def try_parse_version_string(s: Optional[str]) -> Optional[Version]:
    """VersionControl.TryParseVersionString: at least two dot separated numbers, only the
    first two are used (so "1.4.3" is Version(1, 4))."""
    if s is None:
        return None
    parts = s.split(".")
    if len(parts) < 2:
        return None
    nums = []
    for p in parts[:2]:
        v = _int_try_parse(p)
        if v is None or v < 0:
            return None
        nums.append(v)
    return Version(nums[0], nums[1])


def version_from_string(s: str) -> Version:
    """VersionControl.VersionFromString: up to three numeric parts, raises ValueError otherwise."""
    if not s:
        raise ValueError("empty")
    parts = s.split(".")
    if len(parts) > 3:
        raise ValueError(s)
    nums = [0, 0, 0]
    for i, p in enumerate(parts):
        v = _int_try_parse(p)
        if v is None or v < 0:
            raise ValueError(s)
        nums[i] = v
    return Version(nums[0], nums[1], nums[2])


# ---------------------------------------------------------------------------- About.xml

def _stable_string_hash(s: str) -> int:
    num = 23
    data = s.encode("utf-16-le")
    for i in range(0, len(data), 2):
        unit = data[i] | (data[i + 1] << 8)
        num = (num * 31 + unit) & 0xFFFFFFFF
    return num - (1 << 32) if num >= (1 << 31) else num


def _convert_to_ascii(part: str) -> str:
    out = []
    for ch in part:
        if not ch.isalnum() or ord(ch) >= 0x80:
            ch = chr(ord(ch) % 25 + 65)
        out.append(ch)
    return "".join(out)


@dataclass
class AboutMeta:
    package_id: str = ""
    name: str = ""
    author: str = "Anonymous"
    description: str = "No description provided."
    mod_version: str = ""
    supported_versions: Optional[List[str]] = None
    load_before: List[str] = field(default_factory=list)
    load_after: List[str] = field(default_factory=list)
    force_load_before: List[str] = field(default_factory=list)
    force_load_after: List[str] = field(default_factory=list)
    incompatible_with: List[str] = field(default_factory=list)
    dependencies: List[str] = field(default_factory=list)       # package ids only
    package_id_generated: bool = False


def _case_insensitive_file(directory: Path, target: str) -> Path:
    """GenFile.ResolveCaseInsensitiveFilePath."""
    p = directory / target
    if p.is_file():
        return p
    if directory.is_dir():
        for child in sorted(directory.iterdir()):
            if child.is_file() and child.name.lower() == target.lower():
                return child
    return p


def _text_list(el) -> List[str]:
    return [inner_text(li) for li in child_elements(el) if li.tag == "li"]


def read_about(root: Path, diag: Diagnostics, folder_name: str, on_workshop: bool) -> AboutMeta:
    meta = AboutMeta()
    path = _case_insensitive_file(root / "About", "About.xml")
    if path.is_file():
        try:
            xml = parse_xml_bytes(path.read_bytes())
        except XmlParseError as exc:
            diag.add("error", "about_parse_error", "Exception loading About.xml: %s" % exc, file=str(path))
            xml = None
        if xml is not None:
            for child in child_elements(xml):
                tag = child.tag
                low = tag.lower()
                canonical = {
                    "packageId": "package_id", "name": "name", "author": "author", "description": "description",
                    "modVersion": "mod_version", "supportedVersions": "supported_versions",
                    "loadBefore": "load_before", "loadAfter": "load_after", "forceLoadBefore": "force_load_before",
                    "forceLoadAfter": "force_load_after", "incompatibleWith": "incompatible_with",
                    "modDependencies": "dependencies",
                }
                attr = canonical.get(tag)
                if attr is None:
                    # field names are case sensitive since 1.0 but a mismatch is still accepted (with an error)
                    for k, v in canonical.items():
                        if k.lower() == low:
                            attr = v
                            diag.add("error", "case_mismatch_field",
                                     "xml tags are now case-sensitive: %s" % tag, file=str(path))
                            break
                if attr is None:
                    continue
                if attr in ("package_id", "name", "author", "description", "mod_version"):
                    setattr(meta, attr, inner_text(child))
                elif attr == "dependencies":
                    deps = []
                    for li in child_elements(child):
                        pid = None
                        for sub in child_elements(li):
                            if sub.tag == "packageId":
                                pid = inner_text(sub)
                        if pid:
                            deps.append(pid)
                    meta.dependencies = deps
                else:
                    setattr(meta, attr, _text_list(child))
    if not meta.name:
        meta.name = ("Workshop mod " + folder_name) if on_workshop else folder_name
    if not meta.package_id:
        txt = "none"
        if meta.description:
            txt = str(_stable_string_hash(meta.description)).replace("-", "")
            txt = txt[:3]
        meta.package_id = _convert_to_ascii(meta.author + txt) + "." + _convert_to_ascii(meta.name)
        meta.package_id_generated = True
    return meta


# ---------------------------------------------------------------------------- LoadFolders.xml

@dataclass
class LoadFolder:
    folder_name: str
    required_any_of: Optional[List[str]] = None
    required_all_of: Optional[List[str]] = None
    disallowed_any_of: Optional[List[str]] = None

    def should_load(self, mods: "ActiveSet") -> bool:
        """LoadFolder.ShouldLoad: IfModActive (any of) AND IfModActiveAll AND NOT IfModNotActive (any of)."""
        if self.required_any_of and not mods.any_mod_active_no_suffix(self.required_any_of):
            return False
        if self.required_all_of and not mods.all_mods_active_no_suffix(self.required_all_of):
            return False
        if self.disallowed_any_of and mods.any_mod_active_no_suffix(self.disallowed_any_of):
            return False
        return True


@dataclass
class ModLoadFolders:
    versions: Dict[str, List[LoadFolder]] = field(default_factory=dict)

    def defined_versions(self) -> List[str]:
        return list(self.versions.keys())

    def folders_for_version(self, version: str) -> Optional[List[LoadFolder]]:
        return self.versions.get(version)


def _split_ids(value: str) -> List[str]:
    return [s.strip() for s in value.split(",")]


def parse_load_folders(xml_root) -> ModLoadFolders:
    """ModLoadFolders.LoadDataFromXmlCustom."""
    lf = ModLoadFolders()
    for version_node in child_elements(xml_root):
        key = version_node.tag.lower()
        if key.startswith("v"):
            key = key[1:]
        entries = lf.versions.setdefault(key, [])
        for li in child_elements(version_node):
            any_of = _split_ids(li.get("IfModActive")) if li.get("IfModActive") is not None else None
            all_of = _split_ids(li.get("IfModActiveAll")) if li.get("IfModActiveAll") is not None else None
            not_of = _split_ids(li.get("IfModNotActive")) if li.get("IfModNotActive") is not None else None
            text = inner_text(li)
            if text in ("/", "\\"):
                entries.append(LoadFolder("", any_of, all_of, not_of))
            else:
                entries.append(LoadFolder(text.replace("\\", "/"), any_of, all_of, not_of))
    return lf


# ---------------------------------------------------------------------------- active set

class ActiveSet:
    """The set of active package ids and mod names (ModLister.AnyModActiveNoSuffix and friends).

    Identifiers are trimmed and compared case-insensitively. The "no suffix" variants ignore
    the `_steam` postfix the game appends to duplicate workshop copies of the SAME package id;
    the lookup keys are the declared (un-suffixed) package ids, so a requested id that itself
    ends in `_steam` never matches.
    """

    def __init__(self, ids: Iterable[str], names: Iterable[str] = ()):
        self.ids: Set[str] = {i.strip().lower() for i in ids}
        self.names: Set[str] = set(names)

    def is_active(self, ident: str) -> bool:
        return ident.strip().lower() in self.ids

    def all_mods_active_no_suffix(self, ids: Iterable[str]) -> bool:
        return all(self.is_active(i) for i in ids)

    def any_mod_active_no_suffix(self, ids: Iterable[str]) -> bool:
        return any(self.is_active(i) for i in ids)

    def has_active_mod_with_name(self, name: str) -> bool:
        return name in self.names


# ---------------------------------------------------------------------------- mod + folders

@dataclass
class FolderEntry:
    path: Path
    reason: str


@dataclass
class ModInfo:
    index: int
    root: Path
    meta: AboutMeta
    on_workshop: bool = False
    load_folders: Optional[ModLoadFolders] = None
    folders: List[FolderEntry] = field(default_factory=list)      # descending priority
    folder_mode: str = ""

    @property
    def folder_name(self) -> str:
        return self.root.name

    @property
    def package_id(self) -> str:
        return self.meta.package_id

    @property
    def package_id_lc(self) -> str:
        return self.meta.package_id.lower()

    @property
    def name(self) -> str:
        return self.meta.name

    @property
    def is_core(self) -> bool:
        return self.package_id_lc == CORE_PACKAGE_ID

    @property
    def overwrite_priority(self) -> int:
        return 0 if self.is_core else 1

    def __repr__(self):
        return "ModInfo(%d, %s, %s)" % (self.index, self.package_id, self.root)


def init_load_folders(mod: ModInfo, game: GameVersion, active: ActiveSet) -> None:
    """ModContentPack.InitLoadFolders: fills mod.folders (highest priority first) and folder_mode."""
    folders: List[FolderEntry] = []
    root = mod.root

    def add_folders(lst: List[LoadFolder], why: str) -> None:
        # the list is walked from the end: the LAST entry of LoadFolders.xml has the highest priority
        for lf in reversed(lst):
            if lf.should_load(active):
                folders.append(FolderEntry(root / lf.folder_name if lf.folder_name else root,
                                           "%s entry %r" % (why, lf.folder_name or "/")))

    lf = mod.load_folders
    if lf is not None and len(lf.defined_versions()) > 0:
        lst = lf.folders_for_version(game.string)
        if lst:
            add_folders(lst, "LoadFolders v%s" % game.string)
            mod.folders, mod.folder_mode = folders, "loadfolders:exact-version"
            return
        best = None
        cands = []
        for key in lf.defined_versions():
            if key == "default" or not key or "." not in key:
                continue
            try:
                if version_from_string(key) <= game.current:
                    cands.append(key)
            except ValueError:
                continue
        if cands:
            best = sorted(cands, reverse=True)[0]          # OrderByDescending on the raw strings
        if best is not None:
            lst2 = lf.folders_for_version(best)
            if lst2 is not None:
                add_folders(lst2, "LoadFolders v%s" % best)
                mod.folders, mod.folder_mode = folders, "loadfolders:best-older-version(%s)" % best
                return
        lst3 = lf.folders_for_version("default")
        if lst3 is not None:
            add_folders(lst3, "LoadFolders default")
            mod.folders, mod.folder_mode = folders, "loadfolders:default"
            return
    # no usable LoadFolders.xml: version folder, Common, root
    mode = "version-folders"
    exact = root / game.string_without_build
    if exact.is_dir():
        folders.append(FolderEntry(exact, "version folder %s" % game.string_without_build))
    else:
        found: List[Version] = []
        if root.is_dir():
            for d in sorted(root.iterdir()):
                if d.is_dir():
                    v = try_parse_version_string(d.name)
                    if v is not None:
                        found.append(v)
        found.sort()
        chosen = Version(0, 0)
        for item in found:
            if (item > chosen or chosen > game.current) and (item <= game.current or chosen.major == 0):
                chosen = item
        if chosen.major > 0:
            # note: the folder name is rebuilt from the parsed version ("1.5" even for a "1.5.1" folder)
            folders.append(FolderEntry(root / str(chosen), "closest version folder %s" % chosen))
    common = root / "Common"
    if common.is_dir():
        folders.append(FolderEntry(common, "Common"))
    folders.append(FolderEntry(root, "mod root"))
    mod.folders, mod.folder_mode = folders, mode


# ---------------------------------------------------------------------------- assets

@dataclass
class AssetFile:
    """A LoadableXmlAsset candidate."""

    mod: ModInfo
    path: Path                 # absolute path
    rel: str                   # path relative to its load folder (the de-duplication key)
    folder: FolderEntry

    @property
    def name(self) -> str:
        return self.path.name


def _walk_xml(directory: Path, ext_case_sensitive: bool) -> Iterable[Path]:
    """Files of `directory` first (sorted), then each sub directory (sorted), recursively."""
    try:
        entries = sorted(os.scandir(directory), key=lambda e: e.name.casefold())
    except OSError:
        return
    dirs = []
    for e in entries:
        if e.is_dir(follow_symlinks=True):
            dirs.append(e)
        elif e.is_file(follow_symlinks=True):
            nm = e.name
            ok = nm.endswith(".xml") if ext_case_sensitive else nm.lower().endswith(".xml")
            if ok:
                yield Path(e.path)
    for d in dirs:
        yield from _walk_xml(Path(d.path), ext_case_sensitive)


def xml_assets_in_mod_folder(mod: ModInfo, sub: str, ext_case_sensitive: bool = False) -> List[AssetFile]:
    """DirectXmlLoader.XmlAssetsInModFolder(mod, "Defs/" | "Patches/").

    Folders are visited from the highest priority; files are keyed by their path relative to
    the folder and the first occurrence wins (TryAdd), so a file in a higher priority folder
    hides the same relative path in a lower one. Names that start with "." are skipped. The
    result keeps insertion order. Enumeration order inside one folder is OS dependent in the
    game; here it is: files of a directory sorted by case-folded name, then sub directories.
    """
    seen: Dict[str, AssetFile] = {}
    for fe in mod.folders:
        d = fe.path / sub
        if not d.is_dir():
            continue
        for f in _walk_xml(d, ext_case_sensitive):
            if f.name.startswith("."):
                continue
            rel = f.relative_to(fe.path).as_posix()
            if rel not in seen:
                seen[rel] = AssetFile(mod, f, rel, fe)
    return list(seen.values())


# ---------------------------------------------------------------------------- loading mods

def load_mod(index: int, root: os.PathLike, game: GameVersion, diag: Diagnostics,
             on_workshop: Optional[bool] = None) -> ModInfo:
    rootp = Path(root)
    if on_workshop is None:
        on_workshop = "workshop/content/" in rootp.as_posix()
    meta = read_about(rootp, diag, rootp.name, on_workshop)
    mod = ModInfo(index=index, root=rootp, meta=meta, on_workshop=on_workshop)
    lf_path = _case_insensitive_file(rootp, "LoadFolders.xml")
    if lf_path.is_file():
        try:
            mod.load_folders = parse_load_folders(parse_xml_bytes(lf_path.read_bytes()))
        except XmlParseError as exc:
            diag.add("error", "loadfolders_parse_error", "Exception loading LoadFolders.xml: %s" % exc,
                     file=str(lf_path))
    return mod
