//! Reading a def together with its own, unresolved node: what a clone carries.
//!
//! A resolved def holds what the parent supplies as well. A clone keeps the parent of its source, so it
//! must write only what the source writes itself: the stats the parent supplies stay inherited (see
//! [`super::spec::spec_from_def_inheriting`]), and so do tags, comps, recipe fields and the tech level the
//! parent provides, otherwise the game would list them twice. [`OwnSource`] hands the reader the def node as
//! its file writes it; [`apply_own`] then reads the fields the designer models from that node and carries
//! every other child as a raw node, so a clone reproduces its source.
//!
//! The same module reads the projectile of a gun the same way ([`projectile_spec_from_def`]), which gives a
//! clone a projectile of its own.

use rimstudio_core::tree::Node;

use super::access::{child_number, child_text, list_items, parse_number, text_of};
use crate::model::{DesignSpec, ItemKind};
use crate::model::{
    ExtraMeleeDamage, INHERIT_RESETTABLE, MODELLED_THING_FIELDS, ProjectileSpec, RecipeSpec,
    Sourced, SurpriseAttackSpec, ToolSpec, ValueSource, extra_damage_from_node,
};

/// The def nodes of a source as their files write them, before inheritance.
#[derive(Debug, Clone, Copy)]
pub struct OwnSource<'a> {
    /// The weapon def as written.
    pub def: &'a Node,
    /// The def of the projectile the shooting verb names, as written (a gun only).
    pub projectile: Option<&'a Node>,
}

/// Children of the shooting verb the spec has a field for.
const VERB_FIELDS: [&str; 11] = [
    "verbClass",
    "defaultProjectile",
    "range",
    "burstShotCount",
    "ticksBetweenBurstShots",
    "warmupTime",
    "soundCast",
    "soundCastTail",
    "muzzleFlashScale",
    "hasStandardCommand",
    "forcedMissRadius",
];
/// Children of a tool the spec has a field for.
const TOOL_FIELDS: [&str; 9] = [
    "label",
    "capacities",
    "power",
    "cooldownTime",
    "armorPenetration",
    "chanceFactor",
    "linkedBodyPartsGroup",
    "extraMeleeDamages",
    "surpriseAttack",
];
/// Children of `recipeMaker` the recipe has a field for (`researchPrerequisite` is on the spec itself).
const RECIPE_FIELDS: [&str; 6] = [
    "researchPrerequisite",
    "skillRequirements",
    "displayPriority",
    "recipeUsers",
    "unfinishedThingDef",
    "workSkill",
];
/// Children of the `projectile` element of a projectile def the spec has a field for.
const PROJECTILE_FIELDS: [&str; 5] = [
    "damageDef",
    "damageAmountBase",
    "stoppingPower",
    "armorPenetrationBase",
    "speed",
];
/// The attributes of the root element the spec writes itself.
const ROOT_ATTRS: [&str; 4] = ["Name", "ParentName", "Abstract", "Inherit"];

fn is_false_text(text: Option<&str>) -> bool {
    text.is_some_and(|t| t.trim().eq_ignore_ascii_case("false"))
}

fn resets(node: &Node) -> bool {
    is_false_text(node.attr("Inherit"))
}

fn remember_reset(spec: &mut DesignSpec, container: &Node) {
    if resets(container)
        && INHERIT_RESETTABLE.contains(&container.tag.as_str())
        && !spec.inherit_reset.contains(&container.tag)
    {
        spec.inherit_reset.push(container.tag.clone());
    }
}

fn li_texts(container: &Node) -> Vec<String> {
    container.children_named("li").filter_map(text_of).collect()
}

fn others(node: &Node, known: &[&str]) -> Vec<Node> {
    node.elements()
        .filter(|c| !known.contains(&c.tag.as_str()))
        .cloned()
        .collect()
}

fn sourced(v: Option<f64>, source: ValueSource) -> Option<Sourced<f64>> {
    v.map(|v| Sourced::new(v, source))
}

/// The shooting verb of a def: the first verb that names a default projectile.
#[must_use]
pub fn shooting_verb(def: &Node) -> Option<&Node> {
    list_items(def, "verbs")
        .into_iter()
        .find(|v| child_text(v, "defaultProjectile").is_some())
}

/// A tool read from its own node, with its extras. The numbers are read as [`ValueSource`] says.
#[must_use]
pub fn tool_spec_own(node: &Node, source: ValueSource) -> ToolSpec {
    let mut tool = ToolSpec {
        label: child_text(node, "label").unwrap_or_default(),
        capacities: node.child("capacities").map(li_texts).unwrap_or_default(),
        power: sourced(child_number(node, "power"), source),
        cooldown_time: sourced(child_number(node, "cooldownTime"), source),
        armor_penetration: sourced(child_number(node, "armorPenetration"), source),
        chance_factor: sourced(child_number(node, "chanceFactor"), source),
        linked_body_parts_group: child_text(node, "linkedBodyPartsGroup"),
        extra_melee_damages: Vec::new(),
        surprise_attack: None,
        extra: others(node, &TOOL_FIELDS),
    };
    // extraMeleeDamages: typed when every entry is a plain damage, raw otherwise
    if let Some(list) = node.child("extraMeleeDamages") {
        match typed_damages(list) {
            Some(d) => tool.extra_melee_damages = d,
            None => tool.extra.push(list.clone()),
        }
    }
    if let Some(surprise) = node.child("surpriseAttack") {
        let typed = surprise.attrs.is_empty()
            && surprise.elements().all(|c| c.tag == "extraMeleeDamages")
            && surprise.elements().count() <= 1;
        let damages = surprise.child("extraMeleeDamages").map(typed_damages);
        match (typed, damages) {
            (true, Some(Some(d))) => {
                tool.surprise_attack = Some(SurpriseAttackSpec {
                    extra_melee_damages: d,
                });
            }
            (true, None) => tool.surprise_attack = Some(SurpriseAttackSpec::default()),
            _ => tool.extra.push(surprise.clone()),
        }
    }
    tool
}

fn typed_damages(list: &Node) -> Option<Vec<ExtraMeleeDamage>> {
    if !list.attrs.is_empty() {
        return None;
    }
    list.elements().map(extra_damage_from_node).collect()
}

/// Reads the recipe of a `recipeMaker` node: the typed fields and the raw rest. The research prerequisite
/// is returned apart because the spec keeps it in its own field.
fn recipe_from(node: &Node, spec: &mut DesignSpec) -> (Option<String>, RecipeSpec) {
    let mut recipe = RecipeSpec {
        extra: others(node, &RECIPE_FIELDS),
        attrs: node.attrs.iter().cloned().collect(),
        ..RecipeSpec::default()
    };
    if let Some(skills) = node.child("skillRequirements") {
        let parsed: Option<Vec<(String, u32)>> = skills
            .elements()
            .map(|e| Some((e.tag.clone(), e.text_content().trim().parse::<u32>().ok()?)))
            .collect();
        match parsed {
            Some(list) if skills.attrs.iter().all(|(n, _)| n == "Inherit") => {
                remember_reset(spec, skills);
                recipe.skill_requirements = list.into_iter().collect();
            }
            _ => recipe.extra.push(skills.clone()),
        }
    }
    recipe.display_priority = child_number(node, "displayPriority");
    if let Some(users) = node.child("recipeUsers") {
        remember_reset(spec, users);
        recipe.recipe_users = li_texts(users);
    }
    recipe.unfinished_thing_def = child_text(node, "unfinishedThingDef");
    recipe.work_skill = child_text(node, "workSkill");
    (child_text(node, "researchPrerequisite"), recipe)
}

/// Replaces the fields of the spec that the def only inherits, or that the designer carries raw, with what
/// the own node of the def says. `source` marks the numbers read here.
///
/// The numbers of the verb, the cost list and the stats stay as the resolved reading made them (they are
/// the same for a def that writes them itself); what changes is which fields the clone writes and which
/// raw fields it carries.
pub fn apply_own(spec: &mut DesignSpec, resolved: &Node, own: &Node, source: ValueSource) {
    for (name, value) in &own.attrs {
        if !ROOT_ATTRS.contains(&name.as_str()) {
            spec.extra_attrs.insert(name.clone(), value.clone());
        }
    }
    own_head(spec, own);
    own_lists(spec, own);
    own_recipe(spec, own);
    own_graphics(spec, own);
    own_tools_and_verbs(spec, resolved, own, source);
    // Every other child is carried raw. The comps and the offsets are handled above.
    for child in own.elements() {
        if !MODELLED_THING_FIELDS.contains(&child.tag.as_str()) {
            spec.extra_fields.push(child.clone());
        }
    }
}

fn own_head(spec: &mut DesignSpec, own: &Node) {
    if own.child("description").is_none() {
        spec.identity.description.clear();
    }
    if own.child("techLevel").is_none()
        && let Some(level) = spec.tech_level
        && let Some(parent) = spec.parent.as_mut()
    {
        parent.inherited_tech_level = Some(level);
    }
    spec.sound_interact = child_text(own, "soundInteract");
    spec.ui_icon_path = child_text(own, "uiIconPath");
    spec.ui_icon_scale = child_number(own, "uiIconScale");
}

fn own_lists(spec: &mut DesignSpec, own: &Node) {
    // Lists: only what the def lists itself; a list the parent supplies stays the parent's.
    let take = |spec: &mut DesignSpec, tag: &str| -> Vec<String> {
        own.child(tag)
            .map(|c| {
                remember_reset(spec, c);
                li_texts(c)
            })
            .unwrap_or_default()
    };
    spec.weapon_tags = take(spec, "weaponTags");
    spec.trade_tags = take(spec, "tradeTags");
    spec.weapon_classes = take(spec, "weaponClasses");
    // Cost list and stuff: own only.
    match own.child("costList") {
        Some(c) => remember_reset(spec, c),
        None => spec.cost_list.clear(),
    }
    let categories = own.child("stuffCategories");
    if let Some(c) = categories {
        remember_reset(spec, c);
    }
    let count = child_number(own, "costStuffCount");
    if categories.is_none() && count.is_none() {
        spec.stuff = None;
    } else if let Some(stuff) = spec.stuff.as_mut() {
        if categories.is_none() {
            stuff.categories.clear();
        }
        if count.is_none() {
            stuff.count = None;
        }
    }
    if let Some(stats) = own.child("statBases") {
        remember_reset(spec, stats);
    }
    // Equipped stat offsets: typed when every entry is a number, raw otherwise.
    spec.equipped_stat_offsets.clear();
    if let Some(offsets) = own.child("equippedStatOffsets") {
        let parsed: Option<Vec<(String, f64)>> = offsets
            .elements()
            .map(|e| Some((e.tag.clone(), parse_number(&e.text_content())?)))
            .collect();
        let plain = offsets.attrs.iter().all(|(n, _)| n == "Inherit");
        match parsed {
            Some(list) if plain => {
                remember_reset(spec, offsets);
                spec.equipped_stat_offsets = list.into_iter().collect();
            }
            _ => spec.extra_fields.push(offsets.clone()),
        }
    }
    spec.comps = own
        .child("comps")
        .map(|c| {
            remember_reset(spec, c);
            c.elements().cloned().collect()
        })
        .unwrap_or_default();
}

fn own_recipe(spec: &mut DesignSpec, own: &Node) {
    spec.research_prerequisite = None;
    spec.recipe = None;
    if let Some(maker) = own.child("recipeMaker") {
        let (research, recipe) = recipe_from(maker, spec);
        spec.research_prerequisite = research;
        spec.recipe = Some(recipe);
    }
}

fn own_graphics(spec: &mut DesignSpec, own: &Node) {
    spec.texture_path = None;
    spec.graphic_class = None;
    spec.draw_size = None;
    spec.graphic_color = None;
    spec.graphic_extra.clear();
    let Some(graphic) = own.child("graphicData") else {
        spec.omit_defaults.push("texPath".to_owned());
        return;
    };
    spec.texture_path = child_text(graphic, "texPath");
    if spec.texture_path.is_none() {
        spec.omit_defaults.push("texPath".to_owned());
    }
    spec.graphic_class = child_text(graphic, "graphicClass");
    spec.draw_size = child_text(graphic, "drawSize");
    spec.graphic_color = child_text(graphic, "color");
    spec.graphic_extra = others(graphic, &["texPath", "graphicClass", "drawSize", "color"]);
    if spec.texture_path.is_some() && spec.graphic_class.is_none() {
        spec.omit_defaults.push("graphicClass".to_owned());
    }
}

fn own_tools_and_verbs(spec: &mut DesignSpec, resolved: &Node, own: &Node, source: ValueSource) {
    spec.tools = match own.child("tools") {
        Some(tools) => {
            remember_reset(spec, tools);
            tools
                .children_named("li")
                .map(|t| tool_spec_own(t, source))
                .collect()
        }
        None => {
            // The parent supplies the tools. They are written in full (so that they can be edited) and
            // replace the parent's list, which keeps the resolved definition equal to the source's.
            let inherited: Vec<ToolSpec> = list_items(resolved, "tools")
                .into_iter()
                .map(|t| tool_spec_own(t, source))
                .collect();
            if !inherited.is_empty() {
                push_reset(spec, "tools");
            }
            inherited
        }
    };
    spec.other_verbs.clear();
    let Some(verbs) = own.child("verbs") else {
        if shooting_verb(resolved).is_some() {
            // The same for the shooting verb the parent supplies.
            push_reset(spec, "verbs");
            if let Some(verb) = shooting_verb(resolved) {
                carry_verb(spec, verb);
            }
            spec.other_verbs = list_items(resolved, "verbs")
                .into_iter()
                .filter(|v| shooting_verb(resolved).is_none_or(|s| !std::ptr::eq(s, *v)))
                .cloned()
                .collect();
        }
        return;
    };
    remember_reset(spec, verbs);
    let shooting = shooting_verb(own);
    for verb in verbs.children_named("li") {
        let is_shooting = shooting.is_some_and(|s| std::ptr::eq(s, verb));
        if !is_shooting {
            spec.other_verbs.push(verb.clone());
        }
    }
    if let Some(verb) = shooting {
        carry_verb(spec, verb);
    }
}

fn push_reset(spec: &mut DesignSpec, name: &str) {
    if !spec.inherit_reset.iter().any(|n| n == name) {
        spec.inherit_reset.push(name.to_owned());
    }
}

/// Carries what the shooting verb holds beyond the modelled fields, and the defaults it does not set.
fn carry_verb(spec: &mut DesignSpec, verb: &Node) {
    if spec.kind != ItemKind::Ranged {
        return;
    }
    let Some(ranged) = spec.ranged.as_mut() else {
        return;
    };
    ranged.forced_miss_radius = child_number(verb, "forcedMissRadius");
    ranged.verb_extra = others(verb, &VERB_FIELDS);
    if verb.child("verbClass").is_none() {
        spec.omit_defaults.push("verbClass".to_owned());
    }
    if verb.child("hasStandardCommand").is_none() {
        spec.omit_defaults.push("hasStandardCommand".to_owned());
    }
}

/// Reads a projectile def, as its file writes it, into the inline projectile spec of a gun that owns it.
///
/// `new_def_name` is the name of the copy; `copied_from` is the name of the source projectile. Returns
/// `None` when the def has no `ParentName` (a def without a parent cannot be written with the default
/// parent). The damage and the armor penetration are not read here: they are the weapon's `damage` and
/// `armorPenetration` inputs, and the spec lists them in `omit_defaults` when the source leaves them to its
/// parent.
#[must_use]
pub fn projectile_spec_from_def(
    own: &Node,
    new_def_name: &str,
    copied_from: &str,
    source: ValueSource,
) -> Option<ProjectileSpec> {
    let parent = own.attr("ParentName")?.to_owned();
    let props = own.child("projectile");
    let number = |tag: &str| props.and_then(|p| child_number(p, tag));
    let graphic = own.child("graphicData");
    let mut spec = ProjectileSpec {
        def_name: new_def_name.to_owned(),
        label: child_text(own, "label").unwrap_or_default(),
        parent: Some(parent),
        damage_def: props.and_then(|p| child_text(p, "damageDef")),
        texture_path: graphic.and_then(|g| child_text(g, "texPath")),
        graphic_class: graphic.and_then(|g| child_text(g, "graphicClass")),
        speed: sourced(number("speed"), source),
        stopping_power: sourced(number("stoppingPower"), source),
        extra: props
            .map(|p| others(p, &PROJECTILE_FIELDS))
            .unwrap_or_default(),
        graphic_extra: graphic
            .map(|g| others(g, &["texPath", "graphicClass"]))
            .unwrap_or_default(),
        thing_extra: others(own, &["defName", "label", "graphicData", "projectile"]),
        omit_defaults: Vec::new(),
        copied_from: Some(copied_from.to_owned()),
    };
    let present = |tag: &str| props.is_some_and(|p| p.child(tag).is_some());
    for (field, tag) in [
        ("damageDef", "damageDef"),
        ("damageAmountBase", "damageAmountBase"),
        ("armorPenetrationBase", "armorPenetrationBase"),
    ] {
        if !present(tag) {
            spec.omit_defaults.push(field.to_owned());
        }
    }
    if spec.texture_path.is_some() && spec.graphic_class.is_none() {
        spec.omit_defaults.push("graphicClass".to_owned());
    }
    Some(spec)
}
