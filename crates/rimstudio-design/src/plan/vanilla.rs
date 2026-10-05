//! Vanilla definition emission: node trees for the templates `vanilla.ranged` and `vanilla.melee`.
//!
//! The functions turn a [`DesignSpec`] into `ThingDef` nodes exactly as written in the spec. They do not
//! validate (the export function does), they never invent values, and they never write a Combat Extended
//! class: the CE block of the spec is not read here at all.
//!
//! Emitted shape of a ranged weapon (elements appear only when the spec has the value):
//!
//! ```text
//! ThingDef[ParentName] > defName, label, description, techLevel, graphicData, uiIconPath, uiIconScale,
//!   statBases, equippedStatOffsets, costList, costStuffCount, stuffCategories, recipeMaker, soundInteract,
//!   verbs, tools, weaponTags, weaponClasses, tradeTags, comps, then the raw extra fields
//! ```
//!
//! A melee weapon has the same head and its tools are the attacks; it has `verbs` only when the spec
//! carries verbs of its own. The tech level is left out when the parent already supplies it. Raw fields
//! (comps, extra fields, extras of verbs, tools and the recipe) are emitted exactly as stored. The
//! projectile, when the spec creates one, is a separate `ThingDef`; the plan puts it in the file of the
//! weapon, before the weapon.

use rimstudio_core::tree::{Node, NodeBuilder};

use crate::model::{
    DEFAULT_DAMAGE_DEF, DEFAULT_GRAPHIC_CLASS, DEFAULT_PROJECTILE_PARENT, DEFAULT_VERB_CLASS,
    DesignSpec, ExtraMeleeDamage, INHERIT_RESETTABLE, ItemKind, ProjectileChoice, RangedInputs,
    Sourced, ToolSpec, ValueSource, format_number,
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
    let b = with_node(b, equipped_offsets(spec));
    let b = cost_and_stuff(b, spec);
    let b = recipe(b, spec);
    let b = b.child(verbs_node(spec, Some(ranged)));
    let b = tools(b, spec);
    tags(b, spec).build()
}

/// The `ThingDef` of a melee weapon (template [`TEMPLATE_VANILLA_MELEE`]).
#[must_use]
pub fn melee_def(spec: &DesignSpec) -> Node {
    let b = with_node(head(spec), stat_bases(spec, None));
    let b = with_node(b, equipped_offsets(spec));
    let b = cost_and_stuff(b, spec);
    let b = recipe(b, spec);
    let b = if spec.other_verbs.is_empty() {
        b
    } else {
        b.child(verbs_node(spec, None))
    };
    let b = tools(b, spec);
    tags(b, spec).build()
}

/// The projectile `ThingDef` when the spec creates one (an inline projectile), else `None`.
#[must_use]
pub fn projectile_def(spec: &DesignSpec) -> Option<Node> {
    let ranged = spec.ranged.as_ref()?;
    let Some(ProjectileChoice::Inline(p)) = &ranged.projectile else {
        return None;
    };
    let omitted = |name: &str| p.omit_defaults.iter().any(|o| o == name);
    let parent = p.parent.as_deref().unwrap_or(DEFAULT_PROJECTILE_PARENT);
    let mut b = NodeBuilder::new("ThingDef")
        .attr("ParentName", parent)
        .text_elem("defName", &p.def_name);
    if !p.label.is_empty() {
        b = b.text_elem("label", &p.label);
    }
    if p.texture_path.is_some() || !p.graphic_extra.is_empty() {
        let class = p.graphic_class.as_deref().unwrap_or(DEFAULT_GRAPHIC_CLASS);
        let mut graphic = Node::new("graphicData");
        if let Some(tex) = &p.texture_path {
            graphic.push_child(Node::with_text("texPath", tex));
            if p.graphic_class.is_some() || !omitted("graphicClass") {
                graphic.push_child(Node::with_text("graphicClass", class));
            }
        }
        for extra in &p.graphic_extra {
            graphic.push_child(extra.clone());
        }
        b = b.child(graphic);
    }
    let damage_def = p.damage_def.as_deref().unwrap_or(DEFAULT_DAMAGE_DEF);
    let mut props = Node::new("projectile");
    if p.damage_def.is_some() || !omitted("damageDef") {
        props.push_child(Node::with_text("damageDef", damage_def));
    }
    let mut push = |tag: &str, value: Option<String>| {
        if let Some(v) = value {
            props.push_child(Node::with_text(tag, v));
        }
    };
    if !omitted("damageAmountBase") {
        push("damageAmountBase", num(ranged.damage));
    }
    push("stoppingPower", num(p.stopping_power));
    if !omitted("armorPenetrationBase") {
        push("armorPenetrationBase", num(ranged.armor_penetration));
    }
    push("speed", num(p.speed));
    for extra in &p.extra {
        props.push_child(extra.clone());
    }
    if props.elements().next().is_some() {
        b = b.child(props);
    }
    for extra in &p.thing_extra {
        b = b.child(extra.clone());
    }
    Some(b.build())
}

fn with_node(b: NodeBuilder, node: Option<Node>) -> NodeBuilder {
    match node {
        Some(n) => b.child(n),
        None => b,
    }
}

fn omitted(spec: &DesignSpec, name: &str) -> bool {
    spec.omit_defaults.iter().any(|o| o == name)
}

/// Marks a list container that the spec says replaces the parent's list (`Inherit="False"`).
fn reset_if_listed(spec: &DesignSpec, mut node: Node) -> Node {
    if INHERIT_RESETTABLE.contains(&node.tag.as_str()) && spec.inherit_reset.contains(&node.tag) {
        node.attrs.push(("Inherit".to_owned(), "False".to_owned()));
    }
    node
}

fn list_node(spec: &DesignSpec, tag: &str, items: &[String]) -> Node {
    let mut node = Node::new(tag);
    for item in items {
        node.push_child(Node::with_text("li", item));
    }
    reset_if_listed(spec, node)
}

fn head(spec: &DesignSpec) -> NodeBuilder {
    let mut b = NodeBuilder::new("ThingDef");
    if let Some(p) = &spec.parent
        && !p.def_name.is_empty()
    {
        b = b.attr("ParentName", &p.def_name);
    }
    for (name, value) in &spec.extra_attrs {
        b = b.attr(name, value);
    }
    b = b
        .text_elem("defName", &spec.identity.def_name)
        .text_elem("label", &spec.identity.label);
    if !spec.identity.description.is_empty() {
        b = b.text_elem("description", &spec.identity.description);
    }
    if let Some(level) = spec.tech_level
        && spec.parent.as_ref().and_then(|p| p.inherited_tech_level) != Some(level)
    {
        b = b.text_elem("techLevel", level.xml_name());
    }
    if spec.texture_path.is_some()
        || spec.draw_size.is_some()
        || spec.graphic_color.is_some()
        || !spec.graphic_extra.is_empty()
    {
        let mut graphic = Node::new("graphicData");
        if let Some(tex) = &spec.texture_path {
            graphic.push_child(Node::with_text("texPath", tex));
            if spec.graphic_class.is_some() || !omitted(spec, "graphicClass") {
                let class = spec
                    .graphic_class
                    .as_deref()
                    .unwrap_or(DEFAULT_GRAPHIC_CLASS);
                graphic.push_child(Node::with_text("graphicClass", class));
            }
        }
        if let Some(size) = &spec.draw_size {
            graphic.push_child(Node::with_text("drawSize", size));
        }
        if let Some(color) = &spec.graphic_color {
            graphic.push_child(Node::with_text("color", color));
        }
        for extra in &spec.graphic_extra {
            graphic.push_child(extra.clone());
        }
        b = b.child(graphic);
    }
    b.text_elem_opt("uiIconPath", spec.ui_icon_path.clone())
        .text_elem_opt("uiIconScale", spec.ui_icon_scale.map(format_number))
}

/// The `statBases` element: explicit market value, work, mass, ranged accuracy and cooldown, then the extra
/// stats in name order. Inherited stats are never written (IT-032). A stat named twice is written once.
fn stat_bases(spec: &DesignSpec, ranged: Option<&RangedInputs>) -> Option<Node> {
    let mut stats: Vec<(String, String)> = Vec::new();
    let inherited = |name: &str, slot: Option<Sourced<f64>>| {
        let parent_value = spec
            .parent
            .as_ref()
            .and_then(|p| p.inherited_stats.get(name));
        matches!((parent_value, slot), (Some(v), Some(s)) if *v == s.value && s.source != ValueSource::Typed)
    };
    let mut push = |name: &str, slot: Option<Sourced<f64>>| {
        if !inherited(name, slot)
            && let Some(v) = num(slot)
            && !stats.iter().any(|(n, _)| n == name)
        {
            stats.push((name.to_owned(), v));
        }
    };
    push("MarketValue", spec.market_value);
    push("WorkToMake", spec.work_to_make);
    push("Mass", spec.mass);
    if let Some(r) = ranged {
        push("AccuracyTouch", r.accuracy.touch);
        push("AccuracyShort", r.accuracy.short);
        push("AccuracyMedium", r.accuracy.medium);
        push("AccuracyLong", r.accuracy.long);
        push("RangedWeapon_Cooldown", r.cooldown);
    }
    for (name, value) in &spec.extra_stats {
        push(name, Some(*value));
    }
    if stats.is_empty() {
        return None;
    }
    let mut node = Node::new("statBases");
    for (name, value) in stats {
        node.push_child(Node::with_text(name, value));
    }
    Some(reset_if_listed(spec, node))
}

fn equipped_offsets(spec: &DesignSpec) -> Option<Node> {
    if spec.equipped_stat_offsets.is_empty() {
        return None;
    }
    let mut node = Node::new("equippedStatOffsets");
    for (name, value) in &spec.equipped_stat_offsets {
        node.push_child(Node::with_text(name, format_number(*value)));
    }
    Some(reset_if_listed(spec, node))
}

fn cost_and_stuff(mut b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    if !spec.cost_list.is_empty() {
        let mut cost = Node::new("costList");
        for e in &spec.cost_list {
            cost.push_child(Node::with_text(&e.def_name, format_number(e.count)));
        }
        b = b.child(reset_if_listed(spec, cost));
    }
    if let Some(stuff) = &spec.stuff {
        b = b.text_elem_opt("costStuffCount", num(stuff.count));
        if !stuff.categories.is_empty() {
            b = b.child(list_node(spec, "stuffCategories", &stuff.categories));
        }
    }
    b
}

/// The `recipeMaker` element: the research prerequisite first, then the recipe fields, then the raw extras.
fn recipe_node(spec: &DesignSpec) -> Option<Node> {
    let recipe = spec.recipe.as_ref();
    if spec.research_prerequisite.is_none() && recipe.is_none() {
        return None;
    }
    let mut node = Node::new("recipeMaker");
    if let Some(recipe) = recipe {
        for (name, value) in &recipe.attrs {
            node.attrs.push((name.clone(), value.clone()));
        }
    }
    if let Some(r) = &spec.research_prerequisite {
        node.push_child(Node::with_text("researchPrerequisite", r));
    }
    if let Some(recipe) = recipe {
        if !recipe.skill_requirements.is_empty() {
            let mut skills = Node::new("skillRequirements");
            for (skill, level) in &recipe.skill_requirements {
                skills.push_child(Node::with_text(skill, level.to_string()));
            }
            node.push_child(reset_if_listed(spec, skills));
        }
        if let Some(p) = recipe.display_priority {
            node.push_child(Node::with_text("displayPriority", format_number(p)));
        }
        if !recipe.recipe_users.is_empty() {
            node.push_child(list_node(spec, "recipeUsers", &recipe.recipe_users));
        }
        if let Some(t) = &recipe.unfinished_thing_def {
            node.push_child(Node::with_text("unfinishedThingDef", t));
        }
        if let Some(w) = &recipe.work_skill {
            node.push_child(Node::with_text("workSkill", w));
        }
        for extra in &recipe.extra {
            node.push_child(extra.clone());
        }
    }
    Some(node)
}

fn recipe(b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    with_node(b, recipe_node(spec)).text_elem_opt("soundInteract", spec.sound_interact.clone())
}

/// The `verbs` element: the shooting verb of a ranged weapon first, then the other verbs as stored.
fn verbs_node(spec: &DesignSpec, ranged: Option<&RangedInputs>) -> Node {
    let mut node = Node::new("verbs");
    if let Some(r) = ranged {
        node.push_child(verb(NodeBuilder::new("li"), r, spec).build());
    }
    for other in &spec.other_verbs {
        node.push_child(other.clone());
    }
    reset_if_listed(spec, node)
}

fn verb(li: NodeBuilder, r: &RangedInputs, spec: &DesignSpec) -> NodeBuilder {
    let mut li = li;
    if r.verb_class.is_some() || !omitted(spec, "verbClass") {
        li = li.text_elem(
            "verbClass",
            r.verb_class.as_deref().unwrap_or(DEFAULT_VERB_CLASS),
        );
    }
    if !omitted(spec, "hasStandardCommand") {
        li = li.text_elem("hasStandardCommand", "true");
    }
    let mut li = li
        .text_elem_opt(
            "defaultProjectile",
            r.projectile.as_ref().map(|p| p.def_name().to_owned()),
        )
        .text_elem_opt("warmupTime", num(r.warmup))
        .text_elem_opt("range", num(r.range))
        .text_elem_opt("burstShotCount", r.burst_count.map(|b| b.value.to_string()))
        .text_elem_opt("ticksBetweenBurstShots", num(r.ticks_between_burst_shots))
        .text_elem_opt("forcedMissRadius", r.forced_miss_radius.map(format_number))
        .text_elem_opt("soundCast", r.sound_cast.clone())
        .text_elem_opt("soundCastTail", r.sound_cast_tail.clone())
        .text_elem_opt("muzzleFlashScale", r.muzzle_flash_scale.map(format_number));
    for extra in &r.verb_extra {
        li = li.child(extra.clone());
    }
    li
}

fn extra_damages_node(damages: &[ExtraMeleeDamage]) -> Node {
    let mut list = Node::new("extraMeleeDamages");
    for d in damages {
        let mut li = Node::new("li");
        li.push_child(Node::with_text("def", &d.def));
        if let Some(a) = d.amount {
            li.push_child(Node::with_text("amount", format_number(a)));
        }
        if let Some(c) = d.chance {
            li.push_child(Node::with_text("chance", format_number(c)));
        }
        list.push_child(li);
    }
    list
}

fn tool_node(tool: &ToolSpec) -> Node {
    let mut li = Node::new("li");
    li.push_child(Node::with_text("label", &tool.label));
    let mut capacities = Node::new("capacities");
    for c in &tool.capacities {
        capacities.push_child(Node::with_text("li", c));
    }
    li.push_child(capacities);
    for (tag, value) in [
        ("power", num(tool.power)),
        ("cooldownTime", num(tool.cooldown_time)),
        ("armorPenetration", num(tool.armor_penetration)),
        ("linkedBodyPartsGroup", tool.linked_body_parts_group.clone()),
        ("chanceFactor", num(tool.chance_factor)),
    ] {
        if let Some(v) = value {
            li.push_child(Node::with_text(tag, v));
        }
    }
    if !tool.extra_melee_damages.is_empty() {
        li.push_child(extra_damages_node(&tool.extra_melee_damages));
    }
    if let Some(surprise) = &tool.surprise_attack {
        let mut node = Node::new("surpriseAttack");
        if !surprise.extra_melee_damages.is_empty() {
            node.push_child(extra_damages_node(&surprise.extra_melee_damages));
        }
        li.push_child(node);
    }
    for extra in &tool.extra {
        li.push_child(extra.clone());
    }
    li
}

fn tools(b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    if spec.tools.is_empty() {
        return b;
    }
    let mut node = Node::new("tools");
    for tool in &spec.tools {
        node.push_child(tool_node(tool));
    }
    b.child(reset_if_listed(spec, node))
}

fn tags(mut b: NodeBuilder, spec: &DesignSpec) -> NodeBuilder {
    for (tag, list) in [
        ("weaponTags", &spec.weapon_tags),
        ("weaponClasses", &spec.weapon_classes),
        ("tradeTags", &spec.trade_tags),
    ] {
        if !list.is_empty() {
            b = b.child(list_node(spec, tag, list));
        }
    }
    if !spec.comps.is_empty() {
        let mut comps = Node::new("comps");
        for comp in &spec.comps {
            comps.push_child(comp.clone());
        }
        b = b.child(reset_if_listed(spec, comps));
    }
    for extra in &spec.extra_fields {
        b = b.child(extra.clone());
    }
    b
}
