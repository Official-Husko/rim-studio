//! The weapon platform and under barrel choices of the optional Combat Extended block.
//!
//! Combat Extended knows two ways for one weapon to carry more than its own gun:
//!
//! - a **weapon platform**: the weapon def becomes a platform type that accepts attachments. The conversion
//!   operation takes a platform flag, a list of attachment links (which attachment def may sit on the weapon
//!   and what it changes) and a list of default graphic parts (what is drawn while a slot is empty);
//! - an **under barrel unit**: a second gun with its own ammo set, magazine, verb and fire modes that the
//!   wielder switches to, as a component of the weapon.
//!
//! Everything here is plain data with `serde` support and optional by default: a spec without these fields
//! converts exactly as before. The values arrive decided (typed, answered or read from a conversion); the
//! generator in [`crate::ce::patchgen`] shapes them and computes nothing.

use serde::{Deserialize, Serialize};

use rimstudio_core::tree::Node;

use super::source::Sourced;

/// A stat entry of an attachment link: a stat def name and the number it offsets, multiplies or replaces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeStatEntry {
    /// The stat def name.
    pub stat: String,
    /// The number.
    pub value: f64,
}

/// One attachment link of a weapon platform: which attachment def may be fitted and what it does to the
/// weapon.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeAttachmentLink {
    /// The def name of the attachment. REQ.
    pub attachment: String,
    /// The draw scale of the attachment on this weapon, as written (`(1.2,1.2)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_scale: Option<String>,
    /// The draw offset of the attachment on this weapon, as written (`(0.1,0)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_offset: Option<String>,
    /// Stats the attachment adds to.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stat_offsets: Vec<CeStatEntry>,
    /// Stats the attachment multiplies.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stat_multipliers: Vec<CeStatEntry>,
    /// Stats the attachment replaces.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stat_replacers: Vec<CeStatEntry>,
}

/// One default graphic part of a weapon platform: what is drawn while the slots it names are empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeGraphicPart {
    /// The `partGraphicData` element as written (texture path, graphic class, draw size), when the part has
    /// a picture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_graphic: Option<Node>,
    /// The `outlineGraphicData` element as written, when the part has an outline picture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_graphic: Option<Node>,
    /// The slot tags: the part is drawn only while all of these slots are free.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slot_tags: Vec<String>,
}

/// The fire modes of an under barrel unit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeUnderBarrelFireModes {
    /// `aiUseBurstMode`, when written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_use_burst_mode: Option<bool>,
    /// `aiAimMode`, when written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_aim_mode: Option<String>,
    /// `aimedBurstShotCount`, when written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aimed_burst_shot_count: Option<u32>,
    /// `noSingleShot`: the unit has no single shot mode.
    pub no_single_shot: bool,
}

impl CeUnderBarrelFireModes {
    /// True when nothing is written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// An under barrel unit: a second gun on the weapon with its own ammo, verb and fire modes.
///
/// A value of this type with nothing set is meaningful: it writes the component with no unit data, the form
/// Combat Extended gives a unique weapon that only carries the slot for a weapon trait.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeUnderBarrel {
    /// The text of the switch back to the main gun (OPT; Combat Extended has a default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_label: Option<String>,
    /// The text of the switch to the unit (OPT; Combat Extended has a default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under_barrel_label: Option<String>,
    /// The unit and the main gun draw from one ammo holder (one magazine, one reload).
    pub one_ammo_holder: bool,
    /// The unit must be reloaded after switching.
    pub requires_reload: bool,
    /// The ammo set of the unit. REQ unless the unit shares the main gun's holder (`oneAmmoHolder`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// The default projectile of the unit, a member of its ammo set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_projectile: Option<String>,
    /// The magazine size of the unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magazine_size: Option<Sourced<u32>>,
    /// The reload time of the unit in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reload_time: Option<Sourced<f64>>,
    /// The recoil of the unit's verb.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoil_amount: Option<Sourced<f64>>,
    /// The warmup of the unit's verb in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warmup_time: Option<Sourced<f64>>,
    /// The range of the unit's verb in cells.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Sourced<f64>>,
    /// The minimum range of the unit's verb in cells.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_range: Option<f64>,
    /// Shots per burst of the unit's verb.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_shot_count: Option<u32>,
    /// Ticks between burst shots of the unit's verb.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks_between_burst_shots: Option<u32>,
    /// Rounds one shot of the unit uses up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_consumed_per_shot: Option<u32>,
    /// The radius around friendly pawns that the AI keeps clear of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avoid_friendly_fire_radius: Option<f64>,
    /// The sound def of the unit's shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_cast: Option<String>,
    /// The muzzle flash scale of the unit's shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muzzle_flash_scale: Option<f64>,
    /// Other children of the unit's verb as written (target rules, aiming effects).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub verb_extra: Vec<Node>,
    /// The fire modes of the unit.
    #[serde(skip_serializing_if = "CeUnderBarrelFireModes::is_empty")]
    pub fire_modes: CeUnderBarrelFireModes,
    /// Other children of the ammo block as written (a one at a time reload, an ammo spawn override).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub props_extra: Vec<Node>,
    /// Other children of the component as written (target tags, icon paths).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
    /// The class of the vanilla component the unit replaces (the vanilla under barrel ability component).
    /// `None` lets the generator find it on the target: an add when the target has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaces_comp: Option<String>,
}

impl CeUnderBarrel {
    /// True when the unit carries no data at all (the bare slot form).
    #[must_use]
    pub fn is_bare(&self) -> bool {
        let bare = Self {
            replaces_comp: self.replaces_comp.clone(),
            ..Self::default()
        };
        *self == bare
    }
}
