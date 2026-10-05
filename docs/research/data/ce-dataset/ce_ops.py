"""Re-implementation of CombatExtended.PatchOperationMakeGunCECompatible for the def-engine prototype.

Behaviour (from the C# class in CE Source, described in our own words): for every ThingDef with the given defName it
optionally turns the def into a weapon platform, then merges the supplied containers into the vanilla def:
statBases (vanilla Accuracy* stats removed, listed stats replace same-named ones), costList (cleared, then replaced),
Properties (appended as a CombatExtended.VerbPropertiesCE verb after removing vanilla shoot verbs), AmmoUser and
FireModes (appended as comps), weaponTags / weaponClasses (appended), researchPrerequisite (replaces in recipeMaker).
Graphics, attachment and RunAndGun parts are irrelevant for stats and skipped.
"""
from lxml import etree

from defengine.patches import Ctx, Op
from defengine.xmlnet import child_elements
from defengine import xpath_net


def _get_or_create(parent, name):
    found = [c for c in child_elements(parent) if c.tag == name]
    if found:
        return found[0]
    return etree.SubElement(parent, name)


def _copy_into(dest, src_container, override=False):
    for c in child_elements(src_container):
        if override:
            for old in [x for x in child_elements(dest) if x.tag == c.tag]:
                dest.remove(old)
        dest.append(_clone(c))


def _clone(el):
    import copy
    return copy.deepcopy(el)


class MakeGunCECompatible(Op):
    class_name = "CombatExtended.PatchOperationMakeGunCECompatible"
    custom_fields = ["success", "defName", "texPath", "isWeaponPlatform", "AllowWithRunAndGun", "statBases", "Properties",
                     "AmmoUser", "FireModes", "weaponTags", "weaponClasses", "costList", "researchPrerequisite",
                     "attachmentLinks", "defaultGraphicParts"]

    def __init__(self):
        super().__init__()
        self.defName = None
        self.c = {}

    def set_field(self, name, el, ctx):
        if name == "defName":
            self.defName = "".join(el.itertext()).strip()
        elif name in ("texPath", "isWeaponPlatform", "AllowWithRunAndGun"):
            pass
        else:
            self.c[name] = el

    def apply_worker(self, root, ctx: Ctx):
        if not self.defName:
            return False
        hit = False
        for d in xpath_net.select_nodes(root, 'Defs/ThingDef[defName="%s"]' % self.defName):
            hit = True
            c = self.c
            if "statBases" in c and len(c["statBases"]):
                sb = _get_or_create(d, "statBases")
                for x in list(child_elements(sb)):
                    if x.tag in ("AccuracyTouch", "AccuracyShort", "AccuracyMedium", "AccuracyLong"):
                        sb.remove(x)
                _copy_into(sb, c["statBases"], override=True)
            if "costList" in c and len(c["costList"]):
                cl = _get_or_create(d, "costList")
                for x in list(cl):
                    cl.remove(x)
                _copy_into(cl, c["costList"])
            if "Properties" in c and len(c["Properties"]):
                verbs = _get_or_create(d, "verbs")
                for li in list(child_elements(verbs)):
                    vc = [x for x in child_elements(li) if x.tag == "verbClass"]
                    if vc and "".join(vc[0].itertext()).strip() in ("Verb_Shoot", "Verb_ShootOneUse", "Verb_LaunchProjectile"):
                        verbs.remove(li)
                li = etree.SubElement(verbs, "li")
                li.set("Class", "CombatExtended.VerbPropertiesCE")
                _copy_into(li, c["Properties"])
            if "AmmoUser" in c or "FireModes" in c:
                comps = _get_or_create(d, "comps")
                for key, cls in (("AmmoUser", "CombatExtended.CompProperties_AmmoUser"), ("FireModes", "CombatExtended.CompProperties_FireModes")):
                    if key in c:
                        li = etree.SubElement(comps, "li")
                        li.set("Class", cls)
                        _copy_into(li, c[key])
            for key, tag in (("weaponClasses", "weaponClasses"), ("weaponTags", "weaponTags")):
                if key in c and len(c[key]):
                    _copy_into(_get_or_create(d, tag), c[key])
            if "researchPrerequisite" in c:
                rm = _get_or_create(d, "recipeMaker")
                for old in [x for x in child_elements(rm) if x.tag == c["researchPrerequisite"].tag]:
                    rm.remove(old)
                rm.append(_clone(c["researchPrerequisite"]))
        if not hit:
            ctx.diag.add("warning", "ce_makegun_missing_def", "MakeGunCECompatible: def %s not found" % self.defName, ctx.mod, ctx.file)
        return hit


def custom_ops():
    return {MakeGunCECompatible.class_name.lower(): MakeGunCECompatible}
