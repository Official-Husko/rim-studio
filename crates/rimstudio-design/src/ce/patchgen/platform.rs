//! Weapon platforms and under barrel units in the Combat Extended patch.
//!
//! Emitted shapes (the research note `ce-platforms-0.1.0`, observed on the converted weapons of the user's
//! install and in the documented merge of the conversion operation):
//!
//! ```text
//! Operation[make gun] > ..., isWeaponPlatform, attachmentLinks, defaultGraphicParts
//! Operation[Conditional: no under barrel comp yet] > nomatch
//!     Operation[Replace comp of the vanilla ability  |  Add into comps]
//!         li[@Class=under barrel] > standardLabel, underBarrelLabel, propsUnderBarrel,
//!                                   verbPropsUnderBarrel, propsFireModesUnderBarrel
//!         li > compClass CompEquippable          (only after a replace)
//! ```
//!
//! - The **platform parameters** ride in the gun conversion operation of a new conversion. For a target that
//!   already carries a conversion they are written as a platform only conversion operation (the operation
//!   applies each of its parts only when given), guarded by the platform type of the def, and as an add of
//!   each missing attachment link.
//! - The **under barrel unit** is a component. A vanilla weapon with an equippable ability component (the
//!   unit of the game's own weapons) has that component replaced, and gets the plain equippable component
//!   back, because the ability component stood in for it. Any other target gets the component added. A
//!   unit with nothing set is the bare slot form (a unique weapon's trait carrier).
//! - Every operation is guarded by what it writes, so applying the patch twice changes nothing (IT-057).
//!
//! No number here is a table of Combat Extended values: the numbers of the unit are the user's (typed,
//! answered, or read from the unit of a converted weapon), and the habits and suggestions below only count
//! what the user's own conversions agree on (R11).

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::{Node, NodeBuilder};

use super::container::{Container, ExistingConversion};
use super::ops::{DefOps, add, class_li_xpath, replace, unless_present, xpath_literal};
use super::update::{changed, set_field};
use crate::ce::lint::codes::DERIVED_VALUE;
use crate::ce::reader::{CeClassNames, CeModel};
use crate::model::{
    CeAttachmentLink, CeGraphicPart, CePatchSpec, CeStatEntry, CeUnderBarrel, format_number,
};
use crate::validation::codes::{REF_UNRESOLVED, REQUIRED_MISSING};

/// The vanilla component class family of a weapon's own switchable unit: an equippable component that
/// carries an ability. Such a component replaces the plain equippable component of the weapon.
pub const EQUIPPABLE_ABILITY_PREFIX: &str = "CompProperties_EquippableAbility";

/// The `compClass` of the plain equippable component every carried weapon needs.
pub const EQUIPPABLE_COMP: &str = "CompEquippable";

/// True when the block asks for a platform conversion: the flag, links or parts.
#[must_use]
pub fn is_platform(ce: &CePatchSpec) -> bool {
    ce.is_weapon_platform || !ce.attachment_links.is_empty() || !ce.default_graphic_parts.is_empty()
}

fn stats_node(tag: &str, entries: &[CeStatEntry]) -> Option<Node> {
    if entries.is_empty() {
        return None;
    }
    let mut node = Node::new(tag);
    for e in entries {
        node.push_child(Node::with_text(&e.stat, format_number(e.value)));
    }
    Some(node)
}

/// The `li` of one attachment link.
#[must_use]
pub fn link_node(link: &CeAttachmentLink) -> Node {
    let mut li = NodeBuilder::new("li")
        .text_elem("attachment", &link.attachment)
        .text_elem_opt("drawScale", link.draw_scale.clone())
        .text_elem_opt("drawOffset", link.draw_offset.clone())
        .build();
    for (tag, entries) in [
        ("statOffsets", &link.stat_offsets),
        ("statMultipliers", &link.stat_multipliers),
        ("statReplacers", &link.stat_replacers),
    ] {
        if let Some(n) = stats_node(tag, entries) {
            li.push_child(n);
        }
    }
    li
}

/// The `li` of one default graphic part.
#[must_use]
pub fn part_node(part: &CeGraphicPart) -> Node {
    let mut li = Node::new("li");
    if let Some(g) = &part.part_graphic {
        li.push_child(g.clone());
    }
    if let Some(g) = &part.outline_graphic {
        li.push_child(g.clone());
    }
    if !part.slot_tags.is_empty() {
        li.push_child(
            NodeBuilder::new("slotTags")
                .li_each(part.slot_tags.iter().cloned())
                .build(),
        );
    }
    li
}

/// The platform parameters of the gun conversion operation, in the order the operation lists them. Empty
/// when the block asks for no platform.
#[must_use]
pub fn make_gun_params(ce: &CePatchSpec) -> Vec<Node> {
    if !is_platform(ce) {
        return Vec::new();
    }
    let mut out = vec![Node::with_text("isWeaponPlatform", "true")];
    if !ce.attachment_links.is_empty() {
        let mut links = Node::new("attachmentLinks");
        for l in &ce.attachment_links {
            links.push_child(link_node(l));
        }
        out.push(links);
    }
    if !ce.default_graphic_parts.is_empty() {
        let mut parts = Node::new("defaultGraphicParts");
        for p in &ce.default_graphic_parts {
            parts.push_child(part_node(p));
        }
        out.push(parts);
    }
    out
}

fn num(b: NodeBuilder, tag: &str, v: Option<f64>) -> NodeBuilder {
    b.text_elem_opt(tag, v.map(format_number))
}

fn whole(b: NodeBuilder, tag: &str, v: Option<u32>) -> NodeBuilder {
    b.text_elem_opt(tag, v.map(|v| v.to_string()))
}

fn props_node(ub: &CeUnderBarrel) -> Node {
    let b = NodeBuilder::new("propsUnderBarrel");
    let b = whole(b, "magazineSize", ub.magazine_size.map(|m| m.value));
    let b = num(b, "reloadTime", ub.reload_time.map(|r| r.value));
    let mut node = b.text_elem_opt("ammoSet", ub.ammo_set.clone()).build();
    for extra in &ub.props_extra {
        node.push_child(extra.clone());
    }
    node
}

fn verb_node(classes: &CeClassNames, ub: &CeUnderBarrel) -> Node {
    let b = NodeBuilder::new("verbPropsUnderBarrel");
    let b = num(b, "recoilAmount", ub.recoil_amount.map(|v| v.value))
        .text_elem("verbClass", &classes.shoot_verb)
        .text_elem("hasStandardCommand", "true")
        .text_elem_opt("defaultProjectile", ub.default_projectile.clone());
    let b = num(
        b,
        "ai_AvoidFriendlyFireRadius",
        ub.avoid_friendly_fire_radius,
    );
    let b = num(b, "warmupTime", ub.warmup_time.map(|v| v.value));
    let b = num(b, "range", ub.range.map(|v| v.value));
    let b = num(b, "minRange", ub.min_range);
    let b = whole(b, "ticksBetweenBurstShots", ub.ticks_between_burst_shots);
    let b = whole(b, "burstShotCount", ub.burst_shot_count);
    let b = b.text_elem_opt("soundCast", ub.sound_cast.clone());
    let b = num(b, "muzzleFlashScale", ub.muzzle_flash_scale);
    let b = whole(b, "ammoConsumedPerShotCount", ub.ammo_consumed_per_shot);
    let mut node = b.build();
    for extra in &ub.verb_extra {
        node.push_child(extra.clone());
    }
    node
}

fn fire_modes_node(ub: &CeUnderBarrel) -> Option<Node> {
    let f = &ub.fire_modes;
    if f.is_empty() {
        return None;
    }
    let b = NodeBuilder::new("propsFireModesUnderBarrel")
        .text_elem_opt("aiUseBurstMode", f.ai_use_burst_mode.map(|v| v.to_string()))
        .text_elem_opt("aiAimMode", f.ai_aim_mode.clone());
    let b = whole(b, "aimedBurstShotCount", f.aimed_burst_shot_count);
    let b = b.text_elem_opt("noSingleShot", f.no_single_shot.then(|| "true".to_owned()));
    Some(b.build())
}

/// The component entry of an under barrel unit.
#[must_use]
pub fn under_barrel_li(classes: &CeClassNames, ub: &CeUnderBarrel) -> Node {
    let mut li = NodeBuilder::new("li")
        .attr("Class", &classes.under_barrel_comp)
        .text_elem_opt("standardLabel", ub.standard_label.clone())
        .text_elem_opt("underBarrelLabel", ub.under_barrel_label.clone())
        .text_elem_opt(
            "oneAmmoHolder",
            ub.one_ammo_holder.then(|| "true".to_owned()),
        )
        .text_elem_opt(
            "requiresReload",
            ub.requires_reload.then(|| "true".to_owned()),
        )
        .build();
    if ub.is_bare() {
        return li;
    }
    li.push_child(props_node(ub));
    li.push_child(verb_node(classes, ub));
    if let Some(f) = fire_modes_node(ub) {
        li.push_child(f);
    }
    for extra in &ub.extra {
        li.push_child(extra.clone());
    }
    li
}

/// The plain equippable component entry.
fn equippable_li() -> Node {
    NodeBuilder::new("li")
        .text_elem("compClass", EQUIPPABLE_COMP)
        .build()
}

/// The vanilla component the unit replaces on this target: the one the block names when the target has it,
/// else the first component of the ability family, else none (the unit is added).
#[must_use]
pub fn replaced_comp(ub: &CeUnderBarrel, container: &Container) -> Option<String> {
    if let Some(named) = ub
        .replaces_comp
        .as_deref()
        .filter(|c| container.has_comp(c))
    {
        return Some(named.to_owned());
    }
    container
        .comp_classes
        .iter()
        .find(|c| c.starts_with(EQUIPPABLE_ABILITY_PREFIX))
        .cloned()
}

/// The vanilla ability component of a def, when it has one: the sign that the weapon carries a unit of its
/// own that Combat Extended converts into an under barrel unit.
#[must_use]
pub fn vanilla_unit_comp(node: &Node) -> Option<String> {
    crate::ce::reader::platform::comp_classes(node)
        .into_iter()
        .find(|c| c.starts_with(EQUIPPABLE_ABILITY_PREFIX))
}

/// True when a def has a verb with a verb class and no projectile: a beam or spray weapon. Melee weapons
/// have no verbs at all, and a gun's verb names its projectile.
#[must_use]
pub fn has_projectile_less_verb(node: &Node) -> bool {
    crate::reader::access::list_items(node, "verbs")
        .into_iter()
        .any(|v| {
            crate::reader::access::child_text(v, "verbClass").is_some()
                && crate::reader::access::child_text(v, "defaultProjectile").is_none()
        })
}

/// The reason text of a convertible weapon: names the under barrel unit when the vanilla definition has one,
/// because that part of the conversion needs the unit's own ammo and numbers.
#[must_use]
pub fn convert_reason(node: &Node) -> &'static str {
    if vanilla_unit_comp(node).is_some() {
        "can be converted; its own unit becomes an under barrel unit that needs an ammo set and numbers of its own"
    } else {
        "can be converted"
    }
}

/// The unit as it is written: a unit that gives no fire modes takes the ones the user's converted weapons
/// agree on ([`derive::unit_fire_habit`]), reported as a derived value. A unit with fire modes of its own,
/// and the bare slot form, are left as they are.
#[must_use]
pub fn with_habit(
    unit: &CeUnderBarrel,
    model: &CeModel,
    diagnostics: &mut Vec<Diagnostic>,
) -> CeUnderBarrel {
    let mut out = unit.clone();
    if unit.is_bare() || !unit.fire_modes.is_empty() {
        return out;
    }
    if let Some(habit) = derive::unit_fire_habit(model) {
        let aim = habit.ai_aim_mode.clone().unwrap_or_default();
        diagnostics.push(DERIVED_VALUE.diagnostic(
            "/ce/underBarrel/fireModes",
            &[
                ("field", "the fire modes of the under barrel unit"),
                (
                    "value",
                    &format!("{aim}, no single shot {}", habit.no_single_shot),
                ),
                ("how", "taken from the units of your converted weapons"),
            ],
        ));
        out.fire_modes = habit;
    }
    out
}

/// The operations that write the unit into a def, guarded so a second application does nothing.
pub fn under_barrel_ops(
    ops: &mut DefOps<'_>,
    classes: &CeClassNames,
    ub: &CeUnderBarrel,
    container: &Container,
) {
    let li = under_barrel_li(classes, ub);
    let guard = ops.path(&format!(
        "comps/{}",
        class_li_xpath(&classes.under_barrel_comp)
    ));
    let op = match replaced_comp(ub, container) {
        Some(class) => {
            let mut value = vec![li];
            if !container.has_comp(EQUIPPABLE_COMP) {
                value.push(equippable_li());
            }
            replace(
                &ops.path(&format!("comps/{}", class_li_xpath(&class))),
                value,
            )
        }
        None => {
            ops.ensure("comps");
            add(&ops.path("comps"), vec![li])
        }
    };
    ops.push(unless_present(&guard, op));
}

/// Carries the platform and unit choices of `with` into `out` (the update mode overlay): the flag when set,
/// links and parts by attachment and position, the unit when given.
pub fn overlay(out: &mut CePatchSpec, with: &CePatchSpec) {
    out.is_weapon_platform |= with.is_weapon_platform;
    for link in &with.attachment_links {
        match out
            .attachment_links
            .iter_mut()
            .find(|l| l.attachment == link.attachment)
        {
            Some(existing) => *existing = link.clone(),
            None => out.attachment_links.push(link.clone()),
        }
    }
    for part in &with.default_graphic_parts {
        if !out.default_graphic_parts.contains(part) {
            out.default_graphic_parts.push(part.clone());
        }
    }
    if with.under_barrel.is_some() {
        out.under_barrel.clone_from(&with.under_barrel);
    }
}

/// The checks of the platform and unit choices of a block: required numbers and references. Pointers are
/// `/ce/underBarrel/...` and `/ce/attachmentLinks/<n>/...`.
#[must_use]
pub fn validate(ce: &CePatchSpec, model: &CeModel) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(ub) = ce.under_barrel.as_ref().filter(|u| !u.is_bare()) {
        let required = |out: &mut Vec<Diagnostic>, missing: bool, pointer: &str, label: &str| {
            if missing {
                out.push(REQUIRED_MISSING.diagnostic(pointer, &[("label", label)]));
            }
        };
        // A unit that shares the main gun's ammo holder has no ammo of its own to describe.
        let own_ammo = !ub.one_ammo_holder;
        required(
            &mut out,
            own_ammo && ub.ammo_set.as_deref().is_none_or(str::is_empty),
            "/ce/underBarrel/ammoSet",
            "the ammo set of the under barrel unit",
        );
        required(
            &mut out,
            ub.default_projectile.as_deref().is_none_or(str::is_empty),
            "/ce/underBarrel/defaultProjectile",
            "the default projectile of the under barrel unit",
        );
        required(
            &mut out,
            own_ammo && ub.magazine_size.is_none(),
            "/ce/underBarrel/magazineSize",
            "the magazine size of the under barrel unit",
        );
        required(
            &mut out,
            own_ammo && ub.reload_time.is_none(),
            "/ce/underBarrel/reloadTime",
            "the reload time of the under barrel unit",
        );
        required(
            &mut out,
            ub.range.is_none(),
            "/ce/underBarrel/range",
            "the range of the under barrel unit",
        );
        if let Some(set) = ub.ammo_set.as_deref().filter(|s| !s.is_empty()) {
            match model.ammo_set(set) {
                None => out.push(REF_UNRESOLVED.diagnostic(
                    "/ce/underBarrel/ammoSet",
                    &[
                        ("label", "CE ammo set of the under barrel unit"),
                        ("value", set),
                    ],
                )),
                Some(info) => {
                    if let Some(p) = ub.default_projectile.as_deref().filter(|s| !s.is_empty())
                        && !info.has_projectile(p)
                    {
                        out.push(REF_UNRESOLVED.diagnostic(
                            "/ce/underBarrel/defaultProjectile",
                            &[
                                (
                                    "label",
                                    "default projectile of the under barrel unit (not a member of its ammo set)",
                                ),
                                ("value", p),
                            ],
                        ));
                    }
                }
            }
        }
    }
    for (i, link) in ce.attachment_links.iter().enumerate() {
        if link.attachment.trim().is_empty() {
            out.push(REQUIRED_MISSING.diagnostic(
                &format!("/ce/attachmentLinks/{i}/attachment"),
                &[("label", "the attachment of an attachment link")],
            ));
        }
    }
    out
}

/// A def path guarded by the platform type of the def: `Defs/ThingDef[defName="X"][@Class="..."]`.
fn platform_def_xpath(def_xpath: &str, classes: &CeClassNames) -> String {
    format!(
        "{def_xpath}[@Class={}]",
        xpath_literal(&classes.weapon_platform_def)
    )
}

/// Update mode for the platform and unit choices of a block, appended to the operations of `ops`.
pub fn update_ops(
    ops: &mut DefOps<'_>,
    def_name: &str,
    ce: &CePatchSpec,
    cur: &ExistingConversion,
    classes: &CeClassNames,
    container: &Container,
) {
    let have = &cur.block;
    if is_platform(ce) && !have.is_weapon_platform {
        // The conversion operation applies each parameter only when given: this one converts the def into
        // a platform and writes nothing else. The guard makes it safe to repeat.
        let mut op = NodeBuilder::new("Operation")
            .attr("Class", &classes.make_gun_op)
            .text_elem("defName", def_name)
            .build();
        for p in make_gun_params(ce) {
            op.push_child(p);
        }
        let guard = platform_def_xpath(ops.def_xpath(), classes);
        ops.push(unless_present(&guard, op));
    } else if is_platform(ce) {
        for link in &ce.attachment_links {
            if have
                .attachment_links
                .iter()
                .any(|l| l.attachment == link.attachment)
            {
                continue;
            }
            let guard = ops.path(&format!(
                "attachmentLinks/li[attachment={}]",
                xpath_literal(&link.attachment)
            ));
            ops.ensure("attachmentLinks");
            ops.push(unless_present(
                &guard,
                add(&ops.path("attachmentLinks"), vec![link_node(link)]),
            ));
        }
    }
    let Some(ub) = ce.under_barrel.as_ref() else {
        return;
    };
    match have.under_barrel.as_ref() {
        None => under_barrel_ops(ops, classes, ub, container),
        Some(cur_ub) => update_unit_fields(ops, ub, cur_ub, classes),
    }
}

/// Sets the numbers and references of an existing unit that differ from the block.
fn update_unit_fields(
    ops: &mut DefOps<'_>,
    ub: &CeUnderBarrel,
    cur: &CeUnderBarrel,
    classes: &CeClassNames,
) {
    let comp = format!("comps/{}", class_li_xpath(&classes.under_barrel_comp));
    let props = format!("{comp}/propsUnderBarrel");
    let verb = format!("{comp}/verbPropsUnderBarrel");
    if let Some(s) = ub.ammo_set.as_deref().filter(|s| !s.is_empty())
        && cur.ammo_set.as_deref() != Some(s)
    {
        set_field(ops, &props, "ammoSet", s.to_owned(), cur.ammo_set.is_some());
    }
    if let Some(m) = ub.magazine_size
        && changed(
            f64::from(m.value),
            cur.magazine_size.map(|c| f64::from(c.value)),
        )
    {
        set_field(
            ops,
            &props,
            "magazineSize",
            m.value.to_string(),
            cur.magazine_size.is_some(),
        );
    }
    if let Some(r) = ub.reload_time
        && changed(r.value, cur.reload_time.map(|c| c.value))
    {
        set_field(
            ops,
            &props,
            "reloadTime",
            format_number(r.value),
            cur.reload_time.is_some(),
        );
    }
    if let Some(p) = ub.default_projectile.as_deref().filter(|s| !s.is_empty())
        && cur.default_projectile.as_deref() != Some(p)
    {
        set_field(
            ops,
            &verb,
            "defaultProjectile",
            p.to_owned(),
            cur.default_projectile.is_some(),
        );
    }
    for (field, want, have) in [
        (
            "range",
            ub.range.map(|v| v.value),
            cur.range.map(|v| v.value),
        ),
        (
            "warmupTime",
            ub.warmup_time.map(|v| v.value),
            cur.warmup_time.map(|v| v.value),
        ),
        (
            "recoilAmount",
            ub.recoil_amount.map(|v| v.value),
            cur.recoil_amount.map(|v| v.value),
        ),
    ] {
        if let Some(w) = want
            && changed(w, have)
        {
            set_field(ops, &verb, field, format_number(w), have.is_some());
        }
    }
}

pub mod derive;

#[cfg(test)]
mod tests;
