//! Vanilla definition emission: node trees for the templates `vanilla.ranged` and `vanilla.melee`.
//!
//! The functions turn a [`DesignSpec`] into `ThingDef` nodes exactly as written in the spec. They do not
//! validate (the export function does), they never invent values, and they never write a Combat Extended
//! class: the CE block of the spec is not read here at all.
//!
//! Emitted shape of a ranged weapon (elements appear only when the spec has the value):
//!
//! ```text
//! ThingDef[ParentName] > defName, label, description, techLevel, graphicData, statBases, costList,
//!   costStuffCount, stuffCategories, recipeMaker, verbs, tools, weaponTags, weaponClasses, tradeTags
//! ```
//!
//! A melee weapon has the same head, no `verbs`, and its tools are the attacks. The projectile, when the
//! spec creates one, is a separate `ThingDef` in its own file.

use rimstudio_core::tree::{Node, NodeBuilder};

use crate::model::{
    DEFAULT_DAMAGE_DEF, DEFAULT_GRAPHIC_CLASS, DEFAULT_PROJECTILE_PARENT, DEFAULT_VERB_CLASS,
    DesignSpec, ItemKind, ProjectileChoice, RangedInputs, Sourced, ToolSpec, format_number,
};

/// The id of the template that writes a vanilla ranged weapon.
pub const TEMPLATE_VANILLA_RANGED: &str = "vanilla.ranged";
/// The id of the template that writes a vanilla melee weapon.
pub const TEMPLATE_VANILLA_MELEE: &str = "vanilla.melee";

fn num(slot: Option<Sourced<f64>>) -> Option<String> {
    slot.map(|s| format_number(s.value))
}

/// The weapon definition for the kind of the spec.
#[must_use]
pub fn weapon_def(spec: &DesignSpec) -> Node {
    match spec.kind {
        ItemKind::Ranged => ranged_def(spec),
        ItemKind::Melee => melee_def(spec),
    }
}

/// The `ThingDef` of a ranged weapon (template [`TEMPLATE_VANILLA_RANGED`]).
#[must_use]
pub fn ranged_def(spec: &DesignSpec) -> Node {
    let empty = RangedInputs::default();
    let ranged = spec.ranged.as_ref().unwrap_or(&empty);
    let b = with_node(head(spec), stat_bases(spec, Some(ranged)));
    let b = cost_and_stuff(b, spec);
    let b = recipe(b, spec);
    let b = b.elem("verbs", |v| v.elem("li", |li| verb(li, ranged)));
    let b = tools(b, &spec.tools);
    tags(b, spec).build()
}

/// The `ThingDef` of a melee weapon (template [`TEMPLATE_VANILLA_MELEE`]).
#[must_use]
pub fn melee_def(spec: &DesignSpec) -> Node {
    let b = with_node(head(spec), stat_bases(spec, None));
    let b = cost_and_stuff(b, spec);
    let b = recipe(b, spec);
    let b = tools(b, &spec.tools);
    tags(b, spec).build()
}

/// The projectile `ThingDef` when the spec creates one (an inline projectile), else `None`.
#[must_use]
pub fn projectile_def(spec: &DesignSpec) -> Option<Node> {
    let ranged = spec.ranged.as_ref()?;
    let Some(ProjectileChoice::Inline(p)) = &ranged.projectile else {
        return None;
    };
    let parent = p.parent.as_deref().unwrap_or(DEFAULT_PROJECTILE_PARENT);
    let mut b = NodeBuilder::new("ThingDef")
        .attr("ParentName", parent)
        .text_elem("defName", &p.def_name);
    if !p.label.is_empty() {
        b = b.text_elem("label", &p.label);
    }
    if let Some(tex) = &p.texture_path {
        let class = p.graphic_class.as_deref().unwrap_or(DEFAULT_GRAPHIC_CLASS);
        b = b.elem("graphicData", |g| {
            g.text_elem("texPath", tex).text_elem("graphicClass", class)
        });
    }
    let damage_def = p.damage_def.as_deref().unwrap_or(DEFAULT_DAMAGE_DEF);
    b = b.elem("projectile", |pr| {
        pr.text_elem("damageDef", damage_def)
            .text_elem_opt("damageAmountBase", num(ranged.damage))
            .text_elem_opt("stoppingPower", num(p.stopping_power))
            .text_elem_opt("armorPenetrationBase", num(ranged.armor_penetration))
            .text_elem_opt("speed", num(p.speed))
    });
    Some(b.build())
}

fn with_node(b: NodeBuilder, node: Option<Node>) -> NodeBuilder {
    match node {
        Some(n) => b.child(n),
        None => b,
    }
}

fn head(spec: &DesignSpec) -> NodeBuilder {
    let mut b = NodeBuilder::new("ThingDef");
    if let Some(p) = &spec.parent
        && !p.def_name.is_empty()
    {
        b = b.attr("ParentName", &p.def_name);
    }
    b = b
        .text_elem("defName", &spec.identity.def_name)
        .text_elem("label", &spec.identity.label);
    if !spec.identity.description.is_empty() {
        b = b.text_elem("description", &spec.identity.description);
    }
    if let Some(level) = spec.tech_level {
        b = b.text_elem("techLevel", level.xml_name());
    }
    if let Some(tex) = &spec.texture_path {
        let class = spec
            .graphic_class
            .as_deref()
            .unwrap_or(DEFAULT_GRAPHIC_CLASS);
        b = b.elem("graphicData", |g| {
            g.text_elem("texPath", tex).text_elem("graphicClass", class)
        });
    }
    b
}

/// The `statBases` element: explicit market value, work, mass, ranged accuracy and cooldown, then the extra
/// stats in name order. Inherited stats are never written (IT-032). A stat named twice is written once.
fn stat_bases(spec: &DesignSpec, ranged: Option<&RangedInputs>) -> Option<Node> {
    let mut stats: Vec<(String, String)> = Vec::new();
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value
            && !stats.iter().any(|(n, _)| n == name)
        {
            stats.push((name.to_owned(), v));
        }
    };
    push("MarketValue", num(spec.market_value));
    push("WorkToMake", num(spec.work_to_make));
    push("Mass", num(spec.mass));
    if let Some(r) = ranged {
        push("AccuracyTouch", num(r.accuracy.touch));
        push("AccuracyShort", num(r.accuracy.short));
        push("AccuracyMedium", num(r.accuracy.medium));
        push("AccuracyLong", num(r.accuracy.long));
        push("RangedWeapon_Cooldown", num(r.cooldown));
    }
    for (name, value) in &spec.extra_stats {
        push(name, Some(format_number(value.value)));
    }
    if stats.is_empty() {
        return None;
    }
    let mut node = Node::new("statBases");
    for (name, value) in stats {
        node.push_child(Node::with_text(name, value));
    }
    Some(node)
}

fn cost_and_stuff(mut b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    if !spec.cost_list.is_empty() {
        b = b.elem("costList", |c| {
            spec.cost_list
                .iter()
                .fold(c, |c, e| c.text_elem(&e.def_name, format_number(e.count)))
        });
    }
    if let Some(stuff) = &spec.stuff {
        b = b.text_elem_opt("costStuffCount", num(stuff.count));
        if !stuff.categories.is_empty() {
            b = b.elem("stuffCategories", |s| s.li_each(&stuff.categories));
        }
    }
    b
}

fn recipe(b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    match &spec.research_prerequisite {
        Some(r) => b.elem("recipeMaker", |m| m.text_elem("researchPrerequisite", r)),
        None => b,
    }
}

fn verb(li: NodeBuilder, r: &RangedInputs) -> NodeBuilder {
    let class = r.verb_class.as_deref().unwrap_or(DEFAULT_VERB_CLASS);
    li.text_elem("verbClass", class)
        .text_elem("hasStandardCommand", "true")
        .text_elem_opt(
            "defaultProjectile",
            r.projectile.as_ref().map(|p| p.def_name().to_owned()),
        )
        .text_elem_opt("warmupTime", num(r.warmup))
        .text_elem_opt("range", num(r.range))
        .text_elem_opt("burstShotCount", r.burst_count.map(|b| b.value.to_string()))
        .text_elem_opt("ticksBetweenBurstShots", num(r.ticks_between_burst_shots))
        .text_elem_opt("soundCast", r.sound_cast.clone())
        .text_elem_opt("soundCastTail", r.sound_cast_tail.clone())
        .text_elem_opt("muzzleFlashScale", r.muzzle_flash_scale.map(format_number))
}

fn tools(b: NodeBuilder, tools: &[ToolSpec]) -> NodeBuilder {
    if tools.is_empty() {
        return b;
    }
    b.elem("tools", |t| {
        tools.iter().fold(t, |t, tool| {
            t.elem("li", |li| {
                li.text_elem("label", &tool.label)
                    .elem("capacities", |c| c.li_each(&tool.capacities))
                    .text_elem_opt("power", num(tool.power))
                    .text_elem_opt("cooldownTime", num(tool.cooldown_time))
                    .text_elem_opt("armorPenetration", num(tool.armor_penetration))
                    .text_elem_opt("linkedBodyPartsGroup", tool.linked_body_parts_group.clone())
                    .text_elem_opt("chanceFactor", num(tool.chance_factor))
            })
        })
    })
}

fn tags(mut b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    for (tag, list) in [
        ("weaponTags", &spec.weapon_tags),
        ("weaponClasses", &spec.weapon_classes),
        ("tradeTags", &spec.trade_tags),
    ] {
        if !list.is_empty() {
            b = b.elem(tag, |e| e.li_each(list));
        }
    }
    b
}
