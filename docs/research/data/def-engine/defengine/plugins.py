"""Optional custom PatchOperation classes (mod assemblies in the game). Registered through
LoadConfig.custom_ops = {"combatextended.patchoperationsettingsconditional": factory}.

Only operations whose behaviour is a pure function of the XML and a few settings can be reproduced.
CombatExtended.PatchOperationMakeGunCECompatible (reflection and runtime logic) cannot, and stays unknown.
"""
from __future__ import annotations

from typing import Dict

from .patches import Ctx, Op, parse_operation


def ce_settings_conditional(settings: Dict[str, bool]):
    """CombatExtended.PatchOperationSettingsConditional: a Conditional whose test is a bool field of the
    mod settings (settingName). A missing or non-bool setting logs an error and fails."""

    class SettingsConditional(Op):
        class_name = "CombatExtended.PatchOperationSettingsConditional"
        custom_fields = ["success", "settingName", "match", "nomatch"]

        def __init__(self):
            super().__init__()
            self.setting_name = None
            self.match = None
            self.nomatch = None

        def set_field(self, name, el, ctx: Ctx):
            if name == "settingName":
                self.setting_name = "".join(el.itertext())
            else:
                setattr(self, name, parse_operation(el, ctx))

        def apply_worker(self, root, ctx):
            val = settings.get(self.setting_name)
            if not isinstance(val, bool):
                ctx.diag.add("error", "patch_setting_missing", "Cannot find the bool setting %s" % self.setting_name,
                             ctx.mod, ctx.file)
                return False
            if val:
                if self.match is not None:
                    return self.match.apply(root, ctx)
            elif self.nomatch is not None:
                return self.nomatch.apply(root, ctx)
            if self.match is None:
                return self.nomatch is not None
            return True

    return SettingsConditional


# defaults read from the mod's settings class (CombatExtended Source, ModSettings/Settings.cs, 2026-10-04)
CE_DEFAULT_SETTINGS = {"genericAmmo": False, "realWeaponNames": True}


def ce_custom_ops(settings=None) -> Dict[str, object]:
    return {"combatextended.patchoperationsettingsconditional":
            ce_settings_conditional(dict(CE_DEFAULT_SETTINGS if settings is None else settings))}
