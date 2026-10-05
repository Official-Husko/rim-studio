//! Weapon platforms and under barrel units, read from the defs of the user's Combat Extended.
//!
//! Two things live here:
//!
//! - the **read side**: [`read_platform`] reads a resolved weapon def into the platform part of a Combat
//!   Extended block ([`PlatformRead`]): whether the def is a platform, its attachment links and default
//!   graphic parts, and its under barrel unit. [`links_from_nodes`] and [`parts_from_nodes`] read the raw
//!   children of the conversion operation's parameters the same way;
//! - the **merge side**: [`merge_platform`] applies the platform parameters of the gun conversion operation
//!   (the platform flag, the attachment links and the default graphic parts) to an owned `ThingDef` tree,
//!   the way the game does, so a load that registers the operation resolves platform weapons.
//!
//! The merge of the platform parameters, as documented by the reference and observed: a def becomes a
//! platform when the flag is set or when links or parts are given; it then takes the platform def class
//! attribute, a platform `thingClass` (kept when it already names a Combat Extended class) and the
//! real time drawer. The links and parts are appended to the lists of the def, creating them when missing.
//! The merge is not idempotent (a second run appends the links again), like the rest of the operation.

use rimstudio_core::tree::Node;

use super::names::CeClassNames;
use crate::model::{
    CeAttachmentLink, CeGraphicPart, CeStatEntry, CeUnderBarrel, CeUnderBarrelFireModes, Sourced,
    ValueSource,
};
use crate::reader::access::{
    child_bool, child_number, child_text, class_attr, list_items, list_texts, parse_number,
};

/// The `drawerType` a platform takes: attachments are drawn at run time.
const PLATFORM_DRAWER: &str = "RealtimeOnly";

/// The children of an under barrel verb that the model types; every other child is carried as written.
const TYPED_VERB_FIELDS: [&str; 13] = [
    "recoilAmount",
    "verbClass",
    "hasStandardCommand",
    "defaultProjectile",
    "ai_AvoidFriendlyFireRadius",
    "warmupTime",
    "range",
    "minRange",
    "ticksBetweenBurstShots",
    "burstShotCount",
    "soundCast",
    "muzzleFlashScale",
    "ammoConsumedPerShotCount",
];

/// The children of the ammo block that the model types.
const TYPED_PROPS_FIELDS: [&str; 3] = ["magazineSize", "reloadTime", "ammoSet"];

/// The children of the component entry that the model types.
const TYPED_COMP_FIELDS: [&str; 7] = [
    "standardLabel",
    "underBarrelLabel",
    "oneAmmoHolder",
    "requiresReload",
    "propsUnderBarrel",
    "verbPropsUnderBarrel",
    "propsFireModesUnderBarrel",
];

/// The platform and under barrel part of a Combat Extended block, as read from one def.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlatformRead {
    /// The def is a weapon platform.
    pub is_platform: bool,
    /// The attachment links.
    pub links: Vec<CeAttachmentLink>,
    /// The default graphic parts.
    pub parts: Vec<CeGraphicPart>,
    /// The under barrel unit, when the def carries the component.
    pub under_barrel: Option<CeUnderBarrel>,
}

fn whole(v: Option<f64>) -> Option<u32> {
    let v = v?;
    (v >= 0.0 && v <= f64::from(u32::MAX) && v.fract() == 0.0).then_some(v as u32)
}

fn stat_entries(node: &Node, tag: &str) -> Vec<CeStatEntry> {
    node.child(tag)
        .map(|c| {
            c.elements()
                .filter_map(|e| {
                    let value = parse_number(&e.text_content())?;
                    Some(CeStatEntry {
                        stat: e.tag.clone(),
                        value,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reads attachment link entries (`li` nodes) into links. An entry without an attachment is kept with an
/// empty name so the lint can name it.
#[must_use]
pub fn links_from_nodes(items: &[&Node]) -> Vec<CeAttachmentLink> {
    items
        .iter()
        .map(|li| CeAttachmentLink {
            attachment: child_text(li, "attachment").unwrap_or_default(),
            draw_scale: child_text(li, "drawScale"),
            draw_offset: child_text(li, "drawOffset"),
            stat_offsets: stat_entries(li, "statOffsets"),
            stat_multipliers: stat_entries(li, "statMultipliers"),
            stat_replacers: stat_entries(li, "statReplacers"),
        })
        .collect()
}

/// Reads default graphic part entries (`li` nodes) into parts.
#[must_use]
pub fn parts_from_nodes(items: &[&Node]) -> Vec<CeGraphicPart> {
    items
        .iter()
        .map(|li| CeGraphicPart {
            part_graphic: li.child("partGraphicData").cloned(),
            outline_graphic: li.child("outlineGraphicData").cloned(),
            slot_tags: list_texts(li, "slotTags"),
        })
        .collect()
}

fn fire_modes_of(node: &Node) -> CeUnderBarrelFireModes {
    CeUnderBarrelFireModes {
        ai_use_burst_mode: child_bool(node, "aiUseBurstMode"),
        ai_aim_mode: child_text(node, "aiAimMode"),
        aimed_burst_shot_count: whole(child_number(node, "aimedBurstShotCount")),
        no_single_shot: child_bool(node, "noSingleShot").unwrap_or(false),
    }
}

/// Reads the under barrel component entry (`li`) into a unit. Every number is tagged with `source`.
#[must_use]
pub fn read_under_barrel(comp: &Node, source: ValueSource) -> CeUnderBarrel {
    let props = comp.child("propsUnderBarrel");
    let verb = comp.child("verbPropsUnderBarrel");
    let modes = comp.child("propsFireModesUnderBarrel");
    let s = |v: Option<f64>| v.map(|v| Sourced::new(v, source));
    CeUnderBarrel {
        standard_label: child_text(comp, "standardLabel"),
        under_barrel_label: child_text(comp, "underBarrelLabel"),
        one_ammo_holder: child_bool(comp, "oneAmmoHolder").unwrap_or(false),
        requires_reload: child_bool(comp, "requiresReload").unwrap_or(false),
        ammo_set: props.and_then(|p| child_text(p, "ammoSet")),
        default_projectile: verb.and_then(|v| child_text(v, "defaultProjectile")),
        magazine_size: props
            .and_then(|p| whole(child_number(p, "magazineSize")))
            .map(|m| Sourced::new(m, source)),
        reload_time: s(props.and_then(|p| child_number(p, "reloadTime"))),
        recoil_amount: s(verb.and_then(|v| child_number(v, "recoilAmount"))),
        warmup_time: s(verb.and_then(|v| child_number(v, "warmupTime"))),
        range: s(verb.and_then(|v| child_number(v, "range"))),
        min_range: verb.and_then(|v| child_number(v, "minRange")),
        burst_shot_count: verb.and_then(|v| whole(child_number(v, "burstShotCount"))),
        ticks_between_burst_shots: verb
            .and_then(|v| whole(child_number(v, "ticksBetweenBurstShots"))),
        ammo_consumed_per_shot: verb
            .and_then(|v| whole(child_number(v, "ammoConsumedPerShotCount"))),
        avoid_friendly_fire_radius: verb
            .and_then(|v| child_number(v, "ai_AvoidFriendlyFireRadius")),
        sound_cast: verb.and_then(|v| child_text(v, "soundCast")),
        muzzle_flash_scale: verb.and_then(|v| child_number(v, "muzzleFlashScale")),
        verb_extra: verb
            .map(|v| {
                v.elements()
                    .filter(|e| !TYPED_VERB_FIELDS.contains(&e.tag.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        fire_modes: modes.map(fire_modes_of).unwrap_or_default(),
        props_extra: props
            .map(|p| {
                p.elements()
                    .filter(|e| !TYPED_PROPS_FIELDS.contains(&e.tag.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        extra: comp
            .elements()
            .filter(|e| !TYPED_COMP_FIELDS.contains(&e.tag.as_str()))
            .cloned()
            .collect(),
        replaces_comp: None,
    }
}

/// True when the def node is a weapon platform: it has the platform def class attribute.
#[must_use]
pub fn is_platform_node(node: &Node, classes: &CeClassNames) -> bool {
    class_attr(node).is_some_and(|c| c.eq_ignore_ascii_case(&classes.weapon_platform_def))
}

/// The under barrel component entry of a def node, when it has one.
#[must_use]
pub fn under_barrel_comp<'a>(node: &'a Node, classes: &CeClassNames) -> Option<&'a Node> {
    list_items(node, "comps").into_iter().find(|li| {
        class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(&classes.under_barrel_comp))
    })
}

/// Reads the platform and under barrel part of a resolved weapon def.
#[must_use]
pub fn read_platform(node: &Node, classes: &CeClassNames, source: ValueSource) -> PlatformRead {
    PlatformRead {
        is_platform: is_platform_node(node, classes),
        links: links_from_nodes(&list_items(node, "attachmentLinks")),
        parts: parts_from_nodes(&list_items(node, "defaultGraphicParts")),
        under_barrel: under_barrel_comp(node, classes).map(|c| read_under_barrel(c, source)),
    }
}

/// The comp class names of a def node, in list order: the `Class` attribute of an entry, else its
/// `compClass` text.
#[must_use]
pub fn comp_classes(node: &Node) -> Vec<String> {
    list_items(node, "comps")
        .into_iter()
        .filter_map(|li| {
            class_attr(li)
                .map(str::to_owned)
                .or_else(|| child_text(li, "compClass"))
        })
        .collect()
}

fn append_to_list(def: &mut Node, tag: &str, items: &[Node]) {
    if def.child(tag).is_none() {
        def.push_child(Node::new(tag));
    }
    if let Some(list) = def.elements_mut().find(|e| e.tag == tag) {
        for item in items {
            list.push_child(item.clone());
        }
    }
}

fn set_text_child(def: &mut Node, tag: &str, text: &str, only_if_missing: bool) {
    if def.child(tag).is_some() {
        if !only_if_missing {
            def.set_child_text(tag, text);
        }
    } else {
        def.push_child(Node::with_text(tag, text));
    }
}

/// Applies the platform parameters of the conversion operation to a `ThingDef` tree. Returns true when the
/// def was made a platform (so the caller can carry the class attribute over).
pub fn merge_platform(
    def: &mut Node,
    is_platform: bool,
    links: &[Node],
    parts: &[Node],
    classes: &CeClassNames,
) -> bool {
    if !(is_platform || !links.is_empty() || !parts.is_empty()) {
        return false;
    }
    if !is_platform_node(def, classes) {
        def.set_attr("Class", classes.weapon_platform_def.clone());
    }
    let own_ce_class = def
        .child_text("thingClass")
        .is_some_and(|t| t.trim().starts_with("CombatExtended"));
    if !own_ce_class {
        set_text_child(def, "thingClass", &classes.weapon_platform_thing, false);
    }
    set_text_child(def, "drawerType", PLATFORM_DRAWER, false);
    for (tag, items) in [("attachmentLinks", links), ("defaultGraphicParts", parts)] {
        if items.is_empty() {
            continue;
        }
        append_to_list(def, tag, items);
    }
    true
}

/// One under barrel unit of the user's Combat Extended, with the weapon that carries it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnderBarrelExample {
    /// The def name of the weapon.
    pub def_name: String,
    /// The ammo set of the weapon's main gun, when it has one.
    pub main_ammo_set: Option<String>,
    /// The unit as the def holds it.
    pub unit: CeUnderBarrel,
}

/// One weapon platform of the user's Combat Extended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformExample {
    /// The def name of the platform weapon.
    pub def_name: String,
    /// How many attachment links it has.
    pub links: usize,
    /// How many default graphic parts it has.
    pub parts: usize,
}

/// What the user's Combat Extended has of platforms and under barrel units: the examples the suggestions
/// learn from and the attachment defs a link may name.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlatformLibrary {
    /// Weapons that carry an under barrel unit with data (a bare slot is not an example), by def name.
    pub under_barrels: Vec<UnderBarrelExample>,
    /// Weapon platforms, by def name.
    pub platforms: Vec<PlatformExample>,
    /// The def names of the attachment defs, sorted.
    pub attachments: Vec<String>,
}

impl PlatformLibrary {
    /// True when the library knows the attachment def.
    #[must_use]
    pub fn knows_attachment(&self, name: &str) -> bool {
        self.attachments.iter().any(|a| a == name)
    }
}

/// Reads the platform library from the defs of a load.
#[must_use]
pub fn read_library<'a>(
    defs: impl Iterator<Item = &'a rimstudio_defs::DefRecord>,
    classes: &CeClassNames,
) -> PlatformLibrary {
    let mut out = PlatformLibrary::default();
    for def in defs {
        let node = &def.node;
        if node
            .attr("Class")
            .is_some_and(|c| c.eq_ignore_ascii_case(&classes.attachment_def))
        {
            out.attachments.push(def.def_name.clone());
        }
        if is_platform_node(node, classes) {
            out.platforms.push(PlatformExample {
                def_name: def.def_name.clone(),
                links: list_items(node, "attachmentLinks").len(),
                parts: list_items(node, "defaultGraphicParts").len(),
            });
        }
        if let Some(comp) = under_barrel_comp(node, classes) {
            let unit = read_under_barrel(comp, ValueSource::Anchor);
            if !unit.is_bare() {
                let main_ammo_set = list_items(node, "comps")
                    .into_iter()
                    .find(|li| {
                        class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(&classes.ammo_user))
                    })
                    .and_then(|a| child_text(a, "ammoSet"));
                out.under_barrels.push(UnderBarrelExample {
                    def_name: def.def_name.clone(),
                    main_ammo_set,
                    unit,
                });
            }
        }
    }
    out.attachments.sort();
    out
}
