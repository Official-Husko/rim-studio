"""Load vanilla and vanilla+Combat Extended with the def-engine prototype and return resolved ThingDefs as plain dicts.

Environment: RIMWORLD_DIR (folder with Data/), CE_DIR (CE source or mod folder), ENGINE_DIR (def-engine folder),
TYPES_JSON (def type table that includes CombatExtended.* classes, built with build_type_table.py).
CE's own custom patch operation CombatExtended.PatchOperationMakeGunCECompatible is re-implemented here in a
simplified way (statBases replaced per stat, Properties becomes verbs[0], AmmoUser and FireModes become comps).
Nothing from CE is copied: the module reads the user's CE folder at run time.
"""
import copy, os, sys
from pathlib import Path

ENGINE = os.environ.get("ENGINE_DIR", str(Path(__file__).resolve().parents[1] / "def-engine"))
sys.path.insert(0, ENGINE)
from defengine import LoadConfig, load_game          # noqa: E402
from defengine.plugins import ce_custom_ops          # noqa: E402
from defengine.patches import Op                     # noqa: E402
from defengine.xpath_net import select_nodes         # noqa: E402
from lxml import etree                               # noqa: E402

DLC = "Core Royalty Ideology Biotech Anomaly Odyssey".split()


def make_gun_op():
    class MakeGunCE(Op):
        class_name = "CombatExtended.PatchOperationMakeGunCECompatible"
        custom_fields = ["defName", "statBases", "Properties", "AmmoUser", "FireModes", "weaponTags", "weaponClasses",
                         "costList", "researchPrerequisite", "isWeaponPlatform", "AllowWithRunAndGun", "texPath",
                         "attachmentLinks", "defaultGraphicParts"]

        def __init__(self):
            super().__init__()
            self.f = {}

        def set_field(self, name, el, ctx):
            self.f[name] = el

        def apply_worker(self, root, ctx):
            dn = "".join(self.f["defName"].itertext()).strip() if "defName" in self.f else ""
            hits = select_nodes(root, 'Defs/ThingDef[defName="%s"]' % dn) if dn else []
            for d in hits:
                self._apply(d)
            return bool(hits)

        @staticmethod
        def _get(d, name):
            n = d.find(name)
            if n is None:
                n = etree.SubElement(d, name)
            return n

        def _apply(self, d):
            if "statBases" in self.f:
                sb = self._get(d, "statBases")
                for c in self.f["statBases"]:
                    if not isinstance(c.tag, str):
                        continue
                    old = sb.find(c.tag)
                    if old is not None:
                        sb.replace(old, copy.deepcopy(c))
                    else:
                        sb.append(copy.deepcopy(c))
            if "Properties" in self.f and len(self.f["Properties"]):
                v = self._get(d, "verbs")
                for k in list(v):
                    v.remove(k)
                li = etree.SubElement(v, "li", Class="CombatExtended.VerbPropertiesCE")
                for c in self.f["Properties"]:
                    if isinstance(c.tag, str):
                        li.append(copy.deepcopy(c))
            for key, cls in (("AmmoUser", "CombatExtended.CompProperties_AmmoUser"),
                             ("FireModes", "CombatExtended.CompProperties_FireModes")):
                if key in self.f:
                    comps = self._get(d, "comps")
                    li = etree.SubElement(comps, "li", Class=cls)
                    for c in self.f[key]:
                        if isinstance(c.tag, str):
                            li.append(copy.deepcopy(c))
            if "weaponTags" in self.f:
                wt = self._get(d, "weaponTags")
                for c in self.f["weaponTags"]:
                    if isinstance(c.tag, str):
                        wt.append(copy.deepcopy(c))
    return MakeGunCE


def to_py(el):
    """XML element to dict/list/str. li children become lists; Class attribute kept as '@Class'."""
    kids = [k for k in el if isinstance(k.tag, str)]
    if not kids:
        return (el.text or "").strip()
    if all(k.tag == "li" for k in kids):
        out = []
        for k in kids:
            v = to_py(k)
            if isinstance(v, dict) and k.get("Class"):
                v["@Class"] = k.get("Class")
            out.append(v)
        return out
    out = {}
    for k in kids:
        v = to_py(k)
        if k.tag in out:                      # repeated tag: keep first (matches how stat lists never repeat)
            continue
        out[k.tag] = v
    return out


def load(with_ce):
    g = Path(os.environ.get("RIMWORLD_DIR", "/home/pawbeans/.steam/steam/steamapps/common/RimWorld"))
    ce = Path(os.environ["CE_DIR"])
    types = os.environ["TYPES_JSON"]
    mods = [g / "Data" / x for x in DLC]
    ops = {}
    if with_ce:
        mods.append(ce)
        ops = ce_custom_ops()
        ops["combatextended.patchoperationmakeguncecompatible"] = make_gun_op()
    return load_game(LoadConfig(mods=mods, game_dir=g, types=types, custom_ops=ops))


def db_dicts(res, type_name):
    """defName -> {'d': plain dict of the resolved node, 'mod': package id, 'parents': chain} for one def type."""
    out = {}
    for d in res.database(type_name).defs:
        out[d.def_name] = {"d": to_py(d.node), "mod": d.mod_id, "file": d.file, "parents": d.parents}
    return out
