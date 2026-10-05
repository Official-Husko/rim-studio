//! Class names of the Combat Extended assembly and the detector for converted defs.
//!
//! These are names of classes and fields, not values. The defaults are the spellings of the surveyed Combat
//! Extended; [`crate::ce::reader::read_conversions`] overwrites the verb class with the one observed on the
//! user's converted weapons. The Combat Extended class strings live only in this module of the crate
//! (`xtask check-source`, rule `source.ce-class`).

use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

use crate::reader::access::{class_attr, list_items};

/// The package id of Combat Extended in the form used by gates: lower case.
pub const CE_PACKAGE_ID: &str = "ceteam.combatextended";

/// Class and field names used to find and write Combat Extended content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeClassNames {
    /// Class attribute of the verb properties entry of a converted gun.
    pub verb_properties: String,
    /// The `verbClass` of a converted gun's shoot verb.
    pub shoot_verb: String,
    /// Class attribute of a converted tool.
    pub tool: String,
    /// Class attribute of the ammo user component.
    pub ammo_user: String,
    /// Class attribute of the fire modes component.
    pub fire_modes: String,
    /// Class of the gun conversion patch operation.
    pub make_gun_op: String,
    /// Class of the settings conditional patch operation.
    pub settings_conditional_op: String,
    /// Full type name of an ammo set def.
    pub ammo_set_def: String,
    /// Full type name of a gun patcher preset def.
    pub gun_preset_def: String,
    /// Full type name of an apparel patcher preset def.
    pub apparel_preset_def: String,
    /// Prefix of every Combat Extended weapon tag.
    pub tag_prefix: String,
    /// Prefix of the weapon tags that name an AI class.
    pub ai_tag_prefix: String,
}

impl Default for CeClassNames {
    fn default() -> Self {
        Self {
            verb_properties: "CombatExtended.VerbPropertiesCE".into(),
            shoot_verb: "CombatExtended.Verb_ShootCE".into(),
            tool: "CombatExtended.ToolCE".into(),
            ammo_user: "CombatExtended.CompProperties_AmmoUser".into(),
            fire_modes: "CombatExtended.CompProperties_FireModes".into(),
            make_gun_op: "CombatExtended.PatchOperationMakeGunCECompatible".into(),
            settings_conditional_op: "CombatExtended.PatchOperationSettingsConditional".into(),
            ammo_set_def: "CombatExtended.AmmoSetDef".into(),
            gun_preset_def: "CombatExtended.GunPatcherPresetDef".into(),
            apparel_preset_def: "CombatExtended.ApparelPatcherPresetDef".into(),
            tag_prefix: "CE_".into(),
            ai_tag_prefix: "CE_AI_".into(),
        }
    }
}

/// What marks a def as a Combat Extended conversion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeMarkers {
    /// A verb entry with the verb properties class.
    pub verb: bool,
    /// A tool entry with the tool class.
    pub tool: bool,
    /// A component with the ammo user class.
    pub ammo_comp: bool,
    /// A component with the fire modes class.
    pub fire_modes: bool,
}

impl CeMarkers {
    /// True when any of the three conversion markers is present (verb, tool or ammo component), the
    /// detector of update mode and of the pools.
    #[must_use]
    pub fn is_conversion(&self) -> bool {
        self.verb || self.tool || self.ammo_comp
    }
}

fn has_class(node: &Node, list: &str, class: &str) -> bool {
    list_items(node, list)
        .iter()
        .any(|li| class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(class)))
}

/// Looks for the conversion markers in a resolved `ThingDef` node.
#[must_use]
pub fn detect_markers(node: &Node, classes: &CeClassNames) -> CeMarkers {
    CeMarkers {
        verb: has_class(node, "verbs", &classes.verb_properties),
        tool: has_class(node, "tools", &classes.tool),
        ammo_comp: has_class(node, "comps", &classes.ammo_user),
        fire_modes: has_class(node, "comps", &classes.fire_modes),
    }
}

/// True when the node carries a Combat Extended conversion (default class names).
#[must_use]
pub fn is_conversion(node: &Node) -> bool {
    detect_markers(node, &CeClassNames::default()).is_conversion()
}
