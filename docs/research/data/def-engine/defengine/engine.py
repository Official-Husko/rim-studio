"""The Def loading pipeline: mods -> unified document -> patches -> inheritance -> defs.

The stages and their order are those of LoadedModManager.LoadAllActiveMods:

  1. each mod resolves its load folders (ModContentPack.InitLoadFolders);
  2. the Defs/*.xml files of every mod, in mod order, are merged into ONE document whose root is <Defs>
     (CombineIntoUnifiedXML); every imported top-level node remembers its (mod, file);
  3. the Patches of every mod, in mod order, are applied to that document (ApplyPatches);
  4. top-level nodes are registered for inheritance and resolved (ParseAndProcessXML);
  5. each non-abstract node whose MayRequire / MayRequireAnyOf pass becomes a def of the type named by the
     element name or the (resolved) Class attribute (DefFromNodeNew);
  6. defs go into per-type databases (DefDatabase<T>.AddAllInMods).

What is NOT done: C# field parsing (nodes stay XML trees), cross references, PostLoad, ResolveReferences,
Harmony and runtime patches, custom PatchOperation classes (except those registered by the caller).
"""
from __future__ import annotations

import json
import os
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional, Sequence

from lxml import etree

from .diag import Diagnostics
from .inherit import DEFAULT_ALLOW_DUPLICATE_NODES, Inheritance
from .mods import (ActiveSet, AssetFile, GameVersion, ModInfo, init_load_folders, load_mod,
                   xml_assets_in_mod_folder)
from .patches import Ctx, Op, PatchException, load_patch_file
from .xmlnet import (Element, XmlParseError, child_elements, inner_text, is_element, parse_xml_bytes,
                     to_canonical, import_node)

IGNORED_NAMESPACES = ["RimWorld", "Verse", "LudeonTK", "Verse.AI", "Verse.AI.Group", "Verse.Sound", "Verse.Grammar",
                      "RimWorld.Planet", "RimWorld.BaseGen", "RimWorld.QuestGen", "RimWorld.SketchGen", "System"]


# ---------------------------------------------------------------------------- def type table

class TypeTable:
    """Def classes of the loaded assemblies (see build_type_table.py) and GenTypes.GetTypeInAnyAssembly."""

    def __init__(self, data: dict):
        self.root = data.get("root", "Verse.Def")
        self.types: Dict[str, dict] = data["types"]
        self.collisions: Dict[str, List[str]] = data.get("short_name_collisions", {})
        self.by_short: Dict[str, str] = {}
        self.by_lower_full: Dict[str, str] = {}
        for full in self.types:                       # insertion order = assembly load order, later wins
            ns = full.rsplit(".", 1)[0] if "." in full.split("/")[0] else ""
            short = full.rsplit("/", 1)[-1].rsplit(".", 1)[-1]
            if ns == "" or ns in IGNORED_NAMESPACES:
                self.by_short[short] = full
            self.by_lower_full.setdefault(full.lower(), full)    # Assembly.GetType: the first assembly wins
        for short, names in self.collisions.items():
            self.by_short[short] = names[-1]
        self._anc: Dict[str, List[str]] = {}

    @classmethod
    def load(cls, path: os.PathLike) -> "TypeTable":
        return cls(json.loads(Path(path).read_text(encoding="utf-8")))

    def lookup(self, name: str) -> Optional[str]:
        """Full name of the def type for `name`, or None (not found or not a Def type)."""
        full = self.by_short.get(name)
        if full is None:
            full = self.by_lower_full.get(name.lower())
        if full is None:
            for ns in IGNORED_NAMESPACES:
                full = self.by_lower_full.get((ns + "." + name).lower())
                if full is not None:
                    break
        if full is None or full not in self.types:
            return None
        return full

    def ancestors(self, full: str) -> List[str]:
        """full, its base, ... up to and including the root Def type."""
        a = self._anc.get(full)
        if a is None:
            a = []
            cur = full
            while cur is not None and cur not in a:
                a.append(cur)
                if cur == self.root:
                    break
                cur = self.types.get(cur, {}).get("base")
            self._anc[full] = a
        return a


# ---------------------------------------------------------------------------- results

@dataclass
class PatchEvent:
    mod: str
    file: str
    index: int                      # position inside the mod's patch list
    op_class: str
    description: str
    result: bool
    error: Optional[str] = None
    raw: Optional[Element] = None   # the <Operation> element when the class is unknown to the engine (parameters stay readable)


@dataclass
class DefRecord:
    """One def node that survived MayRequire and the Abstract / type checks."""

    seq: int
    tag: str
    type_name: str                  # full type name from the table, e.g. "Verse.ThingDef", "CombatExtended.AmmoDef"
    def_name: str
    mod: Optional[ModInfo]          # None: the node was created by a patch (no asset)
    file: Optional[str]             # path relative to the load folder, e.g. "Defs/ThingDefs_Misc/Weapons.xml"
    node: Element                   # the resolved node (parent merged in)
    source: Element                 # the node as it stood after patches, before inheritance
    parents: List[str] = field(default_factory=list)         # "mod:Name" chain from nearest parent up
    patched_by: List[str] = field(default_factory=list)      # "mod:file#index ClassName(xpath)"

    @property
    def mod_id(self) -> Optional[str]:
        return self.mod.package_id if self.mod else None

    def provenance(self) -> dict:
        return {"mod": self.mod_id, "file": self.file, "parents": self.parents, "patched_by": self.patched_by}

    def to_json(self, with_node: bool = True) -> dict:
        d = {"type": self.type_name, "defName": self.def_name, "provenance": self.provenance()}
        if with_node:
            d["node"] = to_canonical(self.node)
        return d


@dataclass
class LoadConfig:
    mods: Sequence[os.PathLike]                       # mod root folders in load order (Core first)
    game_dir: Optional[os.PathLike] = None            # install folder holding Version.txt
    game_version: Optional[str] = None                # e.g. "1.6.4871 rev598" (overrides Version.txt)
    types: object = None                              # TypeTable or path of a JSON def type table (build_type_table.py)
    extra_active_ids: Iterable[str] = ()              # package ids that count as active without a mod folder
    custom_ops: Dict[str, Callable] = field(default_factory=dict)
    allow_duplicate_nodes: Iterable[str] = DEFAULT_ALLOW_DUPLICATE_NODES
    sort_files: bool = True


@dataclass
class LoadResult:
    mods: List[ModInfo]
    active: ActiveSet
    diag: Diagnostics
    root: Element                                   # the patched unified <Defs> document
    defs: List[DefRecord]
    patch_events: List[PatchEvent]
    timings: Dict[str, float]
    types: Optional[TypeTable]
    stats: Dict[str, int]
    _db: Dict[str, "Database"] = field(default_factory=dict)

    def database(self, type_name: str) -> "Database":
        """DefDatabase<T> for the type `type_name` (short or full name), with the game's override rules."""
        full = self.types.lookup(type_name) if self.types else type_name
        if full is None:
            raise KeyError(type_name)
        if self.types is not None and full == self.types.root:
            raise ValueError("the game never builds DefDatabase<Def> (only the subclasses of Def)")
        db = self._db.get(full)
        if db is None:
            db = build_database(self, full)
            self._db[full] = db
        return db

    def get(self, type_name: str, def_name: str) -> Optional[DefRecord]:
        return self.database(type_name).by_name.get(def_name)


@dataclass
class Database:
    type_name: str
    defs: List[DefRecord]
    by_name: Dict[str, DefRecord]
    skipped_same_mod: List[DefRecord]
    overridden: List[tuple]                          # (old, new)


def build_database(res: LoadResult, full: str) -> Database:
    """DefDatabase<T>.AddAllInMods: Core first, other mods in load order, then defs created by patches."""
    tbl = res.types
    members = [d for d in res.defs if tbl is None or full in tbl.ancestors(d.type_name)]
    by_mod: Dict[int, List[DefRecord]] = {}
    patched: List[DefRecord] = []
    for d in members:
        if d.mod is None:
            patched.append(d)
        else:
            by_mod.setdefault(d.mod.index, []).append(d)
    order = sorted(res.mods, key=lambda m: (m.overwrite_priority, m.index))
    out: List[DefRecord] = []
    by_name: Dict[str, DefRecord] = {}
    skipped, overridden = [], []

    def add(d: DefRecord):
        old = by_name.get(d.def_name)
        if old is not None:
            out.remove(old)
            overridden.append((old, d))
        out.append(d)
        by_name[d.def_name] = d

    for m in order:
        seen = set()
        for d in by_mod.get(m.index, []):
            if d.def_name in seen:
                skipped.append(d)
                res.diag.add("error", "def_duplicate_in_mod", "Mod %s has multiple %ss named %s. Skipping."
                             % (m.package_id, full, d.def_name), m.package_id, d.file)
                continue
            seen.add(d.def_name)
            add(d)
    for d in patched:
        add(d)
    return Database(full, out, by_name, skipped, overridden)


# ---------------------------------------------------------------------------- the pipeline

def _may_require_ok(node: Element, active: ActiveSet) -> bool:
    """The checks of ParseAndProcessXML (note: the attribute values are lower-cased first)."""
    mr = node.get("MayRequire")
    if mr is not None:
        if not active.all_mods_active_no_suffix(mr.lower().split(",")):
            return False
    any_of = node.get("MayRequireAnyOf")
    if any_of is not None:
        parts = any_of.lower().split(",")
        if parts and not active.any_mod_active_no_suffix(parts):    # Array NullOrEmpty never holds after Split
            return False
    return True


def load_game(cfg: LoadConfig) -> LoadResult:
    t0 = time.perf_counter()
    timings: Dict[str, float] = {}
    diag = Diagnostics()
    if cfg.game_version:
        game = GameVersion.parse(cfg.game_version)
    elif cfg.game_dir:
        game = GameVersion.from_install(cfg.game_dir)
    else:
        raise ValueError("game_version or game_dir is required")
    types = (cfg.types if isinstance(cfg.types, TypeTable) else TypeTable.load(cfg.types)) if cfg.types else None

    # 1. mods and their folders
    mods: List[ModInfo] = []
    for i, root in enumerate(cfg.mods):
        mods.append(load_mod(i, root, game, diag))
    active = ActiveSet([m.package_id for m in mods] + list(cfg.extra_active_ids), [m.name for m in mods])
    for m in mods:
        init_load_folders(m, game, active)
    timings["mods"] = time.perf_counter() - t0

    # 2. unified document
    t1 = time.perf_counter()
    unified = etree.Element("Defs")
    prov: Dict[Element, AssetFile] = {}
    n_files = 0
    for m in mods:
        for asset in xml_assets_in_mod_folder(m, "Defs/"):
            n_files += 1
            try:
                root = parse_xml_bytes(asset.path.read_bytes())
            except XmlParseError as exc:
                diag.add("warning", "xml_parse_error", "Exception reading %s as XML: %s" % (asset.name, exc),
                         m.package_id, asset.rel)
                diag.add("error", "defs_unknown_parse_failure", "%s: unknown parse failure" % asset.rel, m.package_id,
                         asset.rel)
                continue
            if root.tag != "Defs":
                diag.add("error", "defs_bad_root", "%s: root element named %s; should be named Defs" % (asset.rel, root.tag),
                         m.package_id, asset.rel)
            for node in child_elements(root):
                node = import_node(node)
                unified.append(node)
                prov[node] = asset
    timings["combine"] = time.perf_counter() - t1

    # 3. patches
    t2 = time.perf_counter()
    events: List[PatchEvent] = []
    touched_by: Dict[Element, List[str]] = {}
    n_patch_ops = 0
    for m in mods:
        ctx = Ctx(active=active, diag=diag, custom=dict(cfg.custom_ops), mod=m.package_id)
        ops: List[tuple] = []
        for asset in xml_assets_in_mod_folder(m, "Patches/"):
            ctx.file = asset.rel
            try:
                root = parse_xml_bytes(asset.path.read_bytes())
            except XmlParseError as exc:
                diag.add("warning", "xml_parse_error", "Exception reading %s as XML: %s" % (asset.name, exc),
                         m.package_id, asset.rel)
                diag.add("error", "patch_file_unreadable", "patch file has no document (the game would throw): %s" % asset.rel,
                         m.package_id, asset.rel)
                continue
            for op in load_patch_file(root, ctx, str(asset.path)):
                ops.append((asset.rel, op))
        for idx, (rel, op) in enumerate(ops):
            n_patch_ops += 1
            ctx.file = rel
            ctx.touched = []
            err = None
            try:
                ok = op.apply(unified, ctx)
            except PatchException as exc:
                ok, err = False, str(exc)
                diag.add("error", "patch_exception", "Error in patch.Apply(): %s" % exc, m.package_id, rel)
            if not ok and err is None:
                diag.add("error", "patch_failed", "Patch operation %s failed" % op.describe(), m.package_id, rel)
            events.append(PatchEvent(m.package_id, rel, idx, op.declared_class or op.class_name, op.describe(), ok, err,
                                     op.raw if op.unknown else None))
            if ok or ctx.touched:
                label = "%s:%s#%d %s" % (m.package_id, rel, idx, op.describe())
                seen = set()
                for t in ctx.touched:
                    top = _top_level(t, unified)
                    if top is not None and id(top) not in seen:
                        seen.add(id(top))
                        touched_by.setdefault(top, []).append(label)
    timings["patch"] = time.perf_counter() - t2

    # 4. inheritance
    t3 = time.perf_counter()
    inh = Inheritance(diag, active, frozenset(cfg.allow_duplicate_nodes))
    top_nodes = [n for n in unified if is_element(n)]
    for n in top_nodes:
        a = prov.get(n)
        inh.try_register(n, a.mod if a else None)
    inh.resolve()
    timings["inherit"] = time.perf_counter() - t3

    # 5. defs
    t4 = time.perf_counter()
    defs: List[DefRecord] = []
    stats = {"top_level_nodes": len(top_nodes), "abstract": 0, "may_require_skipped": 0, "unknown_type": 0,
             "defs": 0, "files": n_files, "patch_ops": n_patch_ops}
    for n in top_nodes:
        if not _may_require_ok(n, active):
            stats["may_require_skipped"] += 1
            continue
        ab = n.get("Abstract")
        if ab is not None and ab.lower() == "true":                 # original node, not the resolved one
            stats["abstract"] += 1
            continue
        resolved = inh.resolved_for(n)
        tname = n.tag
        cls = resolved.get("Class")
        if cls is not None:
            tname = cls
        full = types.lookup(tname) if types else tname
        if full is None:
            stats["unknown_type"] += 1
            diag.add("error", "def_unknown_type", "Type %s is not a Def type or could not be found" % tname,
                     getattr(getattr(prov.get(n), "mod", None), "package_id", None))
            continue
        dn = "UnnamedDef"
        for c in child_elements(resolved):                          # later duplicates overwrite earlier ones
            if c.tag == "defName":
                dn = inner_text(c)
        a = prov.get(n)
        chain = ["%s:%s" % (p.mod.package_id if p.mod else "patch", p.xml.get("Name")) for p in inh.parent_chain(n)]
        defs.append(DefRecord(len(defs), n.tag, full, dn, a.mod if a else None, a.rel if a else None, resolved, n,
                              chain, touched_by.get(n, [])))
    stats["defs"] = len(defs)
    timings["defs"] = time.perf_counter() - t4
    timings["total"] = time.perf_counter() - t0
    return LoadResult(mods, active, diag, unified, defs, events, timings, types, stats)


def _top_level(node, unified: Element) -> Optional[Element]:
    """The ancestor of `node` that is a direct child of the unified document (None if detached)."""
    cur = node
    while cur is not None:
        p = cur.getparent()
        if p is unified:
            return cur
        cur = p
    return None
