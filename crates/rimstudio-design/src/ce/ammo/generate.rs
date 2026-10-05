//! The definition file of a custom caliber: the node trees of the thing category, the ammo set, the ammo
//! items, the projectiles and the recipes.
//!
//! The generator shapes what the spec decided and computes nothing: a number that is not in the spec is not
//! written (the def inherits it from its parent), and a market value that is not given is left to the game,
//! which prices Combat Extended's own ammunition from the recipe. The output is deterministic: the same spec,
//! prefix, install data and layout give the same trees.
//!
//! Order of the defs: the thing category (when the caliber has its own), the ammo set, then for every type
//! the ammo item, the projectile and the recipe. Each def has a section header `====== <defName> ======`.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::codes::ART_RESERVED;
use super::names::{AMMO_DEF, DerivedNames, FRAGMENTS_COMP, PROJECTILE_PROPS, PairForm, type_key};
use crate::ce::reader::CeModel;
use crate::model::{
    CookOffKind, CustomAmmoItem, CustomAmmoRecipe, CustomAmmoSpec, CustomAmmoType,
    CustomProjectile, Sourced, format_number,
};
use crate::plan::{ProjectLayout, SectionHeader};

/// The folder of the reserved art of an ammo item below `Textures`.
const AMMO_TEXTURE_DIR: &str = "Things/Item/Ammo";

/// The generated definitions of a custom caliber.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedAmmo {
    /// The file stem of the definition file (`<prefix>_<Name>`).
    pub stem: String,
    /// The def name of the ammo set.
    pub set_name: String,
    /// The projectile of the default type (the weapon's default projectile).
    pub default_projectile: Option<String>,
    /// The defs, in file order.
    pub defs: Vec<Node>,
    /// The section headers of the defs.
    pub sections: Vec<SectionHeader>,
    /// The ammo item def names, in type order.
    pub ammo: Vec<String>,
    /// The projectile def names, in type order.
    pub projectiles: Vec<String>,
    /// The recipe def names, in type order.
    pub recipes: Vec<String>,
    /// Info diagnostics: art that is reserved but not imported.
    pub diagnostics: Vec<Diagnostic>,
}

fn leaf(tag: &str, text: impl Into<String>) -> Node {
    Node::with_text(tag, text)
}

fn number(n: &Sourced<f64>) -> String {
    format_number(n.value)
}

fn push_text(node: &mut Node, tag: &str, text: Option<&String>) {
    if let Some(t) = text.filter(|t| !t.trim().is_empty()) {
        node.push_child(leaf(tag, t.trim()));
    }
}

fn push_num(node: &mut Node, tag: &str, n: Option<&Sourced<f64>>) {
    if let Some(n) = n {
        node.push_child(leaf(tag, number(n)));
    }
}

fn push_bool(node: &mut Node, tag: &str, b: Option<bool>) {
    if let Some(b) = b {
        node.push_child(leaf(tag, if b { "true" } else { "false" }));
    }
}

fn list(tag: &str, items: &[String], replace: bool) -> Node {
    let mut n = Node::new(tag);
    if replace {
        n.set_attr("Inherit", "False");
    }
    for i in items {
        n.push_child(leaf("li", i.trim()));
    }
    n
}

fn thing(class: Option<&str>, parent: Option<&String>) -> Node {
    let mut n = Node::new("ThingDef");
    if let Some(c) = class {
        n.set_attr("Class", c);
    }
    if let Some(p) = parent.filter(|p| !p.trim().is_empty()) {
        n.set_attr("ParentName", p.trim());
    }
    n
}

fn graphic(tex: &str, class: Option<&String>, size: Option<&String>, extra: &[Node]) -> Node {
    let mut g = Node::new("graphicData");
    g.push_child(leaf("texPath", tex));
    push_text(&mut g, "graphicClass", class);
    push_text(&mut g, "drawSize", size);
    for e in extra {
        g.push_child(e.clone());
    }
    g
}

/// The label of the ammo item of a type.
#[must_use]
pub fn ammo_label(spec: &CustomAmmoSpec, t: &CustomAmmoType, model: &CeModel) -> String {
    match t.label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_owned(),
        None => {
            let class = model.ammo.class_label(&t.ammo_class);
            format!("{} ({class})", spec.caliber.trim())
        }
    }
}

fn cook_off(t: &CustomAmmoType) -> CookOffKind {
    t.item
        .cook_off
        .unwrap_or(if t.projectile.explosion_radius.is_some() {
            CookOffKind::Detonate
        } else {
            CookOffKind::Projectile
        })
}

fn item_def(
    spec: &CustomAmmoSpec,
    t: &CustomAmmoType,
    names: &DerivedNames,
    category: Option<&str>,
    model: &CeModel,
    art: &mut Vec<(String, String)>,
) -> Node {
    let key = type_key(t);
    let item: &CustomAmmoItem = &t.item;
    let mut n = thing(Some(AMMO_DEF), item.parent.as_ref());
    n.push_child(leaf("defName", names.ammo(&key)));
    n.push_child(leaf("label", ammo_label(spec, t, model)));
    push_text(&mut n, "description", t.description.as_ref());
    if item.mass.is_some()
        || item.bulk.is_some()
        || item.market_value.is_some()
        || !item.stat_bases.is_empty()
    {
        let mut stats = Node::new("statBases");
        push_num(&mut stats, "Mass", item.mass.as_ref());
        push_num(&mut stats, "Bulk", item.bulk.as_ref());
        push_num(&mut stats, "MarketValue", item.market_value.as_ref());
        for (stat, value) in &item.stat_bases {
            stats.push_child(leaf(stat, number(value)));
        }
        n.push_child(stats);
    }
    if !item.trade_tags.is_empty() {
        n.push_child(list("tradeTags", &item.trade_tags, true));
    }
    let mut cats: Vec<String> = category.map(str::to_owned).into_iter().collect();
    for c in &item.thing_categories {
        if !cats.contains(c) {
            cats.push(c.clone());
        }
    }
    if !cats.is_empty() {
        n.push_child(list("thingCategories", &cats, true));
    }
    if let Some(s) = &item.stack_limit {
        n.push_child(leaf("stackLimit", s.value.to_string()));
    }
    let tex = match item
        .tex_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(p) => p.to_owned(),
        None => {
            let p = format!("{AMMO_TEXTURE_DIR}/{}", names.ammo(&key));
            art.push((format!("the ammo {}", names.ammo(&key)), p.clone()));
            p
        }
    };
    n.push_child(graphic(
        &tex,
        item.graphic_class.as_ref(),
        item.draw_size.as_ref(),
        &item.graphic_extra,
    ));
    push_text(&mut n, "techLevel", item.tech_level.as_ref());
    n.push_child(leaf("ammoClass", t.ammo_class.trim()));
    let projectile = names.projectile(&key);
    match cook_off(t) {
        CookOffKind::Projectile => n.push_child(leaf("cookOffProjectile", projectile)),
        CookOffKind::Detonate => n.push_child(leaf("detonateProjectile", projectile)),
        CookOffKind::None => {}
    }
    for e in &item.extra {
        n.push_child(e.clone());
    }
    n
}

fn properties(p: &CustomProjectile) -> Node {
    let mut props = Node::new("projectile");
    props.set_attr("Class", PROJECTILE_PROPS);
    push_text(&mut props, "damageDef", p.damage_def.as_ref());
    push_num(&mut props, "damageAmountBase", p.damage.as_ref());
    push_num(&mut props, "pelletCount", p.pellet_count.as_ref());
    push_num(
        &mut props,
        "armorPenetrationSharp",
        p.armor_penetration_sharp.as_ref(),
    );
    push_num(
        &mut props,
        "armorPenetrationBlunt",
        p.armor_penetration_blunt.as_ref(),
    );
    push_num(&mut props, "speed", p.speed.as_ref());
    push_num(&mut props, "spreadMult", p.spread_mult.as_ref());
    if !p.secondary_damage.is_empty() {
        let mut list = Node::new("secondaryDamage");
        for s in &p.secondary_damage {
            let mut li = Node::new("li");
            li.push_child(leaf("def", s.def.trim()));
            push_num(&mut li, "amount", s.amount.as_ref());
            list.push_child(li);
        }
        props.push_child(list);
    }
    push_num(&mut props, "explosionRadius", p.explosion_radius.as_ref());
    push_bool(
        &mut props,
        "applyDamageToExplosionCellsNeighbors",
        p.explosion_neighbors,
    );
    push_num(
        &mut props,
        "suppressionFactor",
        p.suppression_factor.as_ref(),
    );
    push_num(&mut props, "dangerFactor", p.danger_factor.as_ref());
    push_bool(&mut props, "dropsCasings", p.drops_casings);
    push_text(&mut props, "casingMoteDefname", p.casing_mote.as_ref());
    push_text(&mut props, "casingFilthDefname", p.casing_filth.as_ref());
    push_bool(&mut props, "ai_IsIncendiary", p.incendiary);
    push_bool(&mut props, "flyOverhead", p.fly_overhead);
    push_text(&mut props, "soundExplode", p.sound_explode.as_ref());
    push_text(&mut props, "soundAmbient", p.sound_ambient.as_ref());
    push_text(
        &mut props,
        "soundHitThickRoof",
        p.sound_hit_thick_roof.as_ref(),
    );
    push_text(
        &mut props,
        "soundImpactAnticipate",
        p.sound_impact_anticipate.as_ref(),
    );
    for e in &p.extra {
        props.push_child(e.clone());
    }
    props
}

fn projectile_def(
    spec: &CustomAmmoSpec,
    t: &CustomAmmoType,
    names: &DerivedNames,
    model: &CeModel,
) -> Node {
    let key = type_key(t);
    let p = &t.projectile;
    let mut n = thing(None, p.parent.as_ref());
    n.push_child(leaf("defName", names.projectile(&key)));
    let label = p
        .label
        .clone()
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| ammo_label(spec, t, model));
    n.push_child(leaf("label", label.trim()));
    push_text(&mut n, "thingClass", p.thing_class.as_ref());
    // A projectile without art of its own draws like its parent's (the user's projectiles share a few
    // pictures), so nothing is reserved for it.
    if let Some(tex) = p
        .tex_path
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        n.push_child(graphic(
            tex,
            p.graphic_class.as_ref(),
            p.draw_size.as_ref(),
            &p.graphic_extra,
        ));
    }
    n.push_child(properties(p));
    // The components: a raw `comps` node (a copied list that replaces the parent's) and loose `li` entries
    // are kept as they are; the typed fragments join them or make a list of their own that adds to the
    // parent's.
    let mut comps = Node::new("comps");
    let mut others: Vec<Node> = Vec::new();
    for e in &p.thing_extra {
        match e.tag.as_str() {
            "comps" => {
                for (k, v) in &e.attrs {
                    comps.set_attr(k.clone(), v.clone());
                }
                for li in e.elements() {
                    comps.push_child(li.clone());
                }
            }
            "li" => comps.push_child(e.clone()),
            _ => others.push(e.clone()),
        }
    }
    if !p.fragments.is_empty() {
        let mut li = Node::new("li");
        li.set_attr("Class", FRAGMENTS_COMP);
        let mut frags = Node::new("fragments");
        for f in &p.fragments {
            frags.push_child(leaf(f.def.trim(), f.count.to_string()));
        }
        li.push_child(frags);
        comps.push_child(li);
    }
    if comps.elements().next().is_some() {
        n.push_child(comps);
    }
    for o in others {
        n.push_child(o);
    }
    n
}

fn recipe_def(
    spec: &CustomAmmoSpec,
    t: &CustomAmmoType,
    names: &DerivedNames,
    model: &CeModel,
) -> Node {
    let key = type_key(t);
    let r: &CustomAmmoRecipe = &t.recipe;
    let label = ammo_label(spec, t, model);
    let count = r.products.map_or(1, |p| p.value);
    let mut n = Node::new("RecipeDef");
    if let Some(p) = r.parent.as_ref().filter(|p| !p.trim().is_empty()) {
        n.set_attr("ParentName", p.trim());
    }
    n.push_child(leaf("defName", names.recipe(&key)));
    let text = |own: &Option<String>, derived: String| {
        own.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map_or(derived, str::to_owned)
    };
    n.push_child(leaf(
        "label",
        text(&r.label, format!("make {label} x{count}")),
    ));
    n.push_child(leaf(
        "description",
        text(&r.description, format!("Craft {count} {label}.")),
    ));
    n.push_child(leaf(
        "jobString",
        text(&r.job_string, format!("Making {label}.")),
    ));
    let mut ing = Node::new("ingredients");
    let mut fixed_things: Vec<String> = Vec::new();
    let mut fixed_cats: Vec<String> = Vec::new();
    for i in &r.ingredients {
        let things: Vec<String> = std::iter::once(&i.thing)
            .chain(i.alternatives.iter())
            .filter(|t| !t.trim().is_empty())
            .cloned()
            .collect();
        let mut li = Node::new("li");
        let mut filter = Node::new("filter");
        if !things.is_empty() {
            filter.push_child(list("thingDefs", &things, false));
        }
        if !i.categories.is_empty() {
            filter.push_child(list("categories", &i.categories, false));
        }
        li.push_child(filter);
        push_num(&mut li, "count", i.count.as_ref());
        ing.push_child(li);
        for t in things {
            if !fixed_things.contains(&t) {
                fixed_things.push(t);
            }
        }
        for c in &i.categories {
            if !fixed_cats.contains(c) {
                fixed_cats.push(c.clone());
            }
        }
    }
    n.push_child(ing);
    if !r.extra.iter().any(|e| e.tag == "fixedIngredientFilter") {
        let mut ff = Node::new("fixedIngredientFilter");
        if !fixed_things.is_empty() {
            ff.push_child(list("thingDefs", &fixed_things, false));
        }
        if !fixed_cats.is_empty() {
            ff.push_child(list("categories", &fixed_cats, false));
        }
        n.push_child(ff);
    }
    let mut products = Node::new("products");
    products.push_child(leaf(&names.ammo(&key), count.to_string()));
    n.push_child(products);
    push_num(&mut n, "workAmount", r.work_amount.as_ref());
    if !r.users.is_empty() {
        n.push_child(list("recipeUsers", &r.users, true));
    }
    match r.research_prerequisite.as_deref().map(str::trim) {
        Some("") => {
            let mut clear = Node::new("researchPrerequisite");
            clear.set_attr("Inherit", "False");
            n.push_child(clear);
        }
        Some(text) => n.push_child(leaf("researchPrerequisite", text)),
        None => {}
    }
    if !r.research_prerequisites.is_empty() {
        n.push_child(list(
            "researchPrerequisites",
            &r.research_prerequisites,
            false,
        ));
    }
    if let Some(level) = &r.skill_level {
        let mut s = Node::new("skillRequirements");
        s.push_child(leaf("Crafting", level.value.to_string()));
        n.push_child(s);
    }
    for e in &r.extra {
        n.push_child(e.clone());
    }
    n
}

/// The key of the default type: the typed key when it names a type, else the first type's key.
#[must_use]
pub fn default_key(spec: &CustomAmmoSpec) -> Option<String> {
    let typed = spec
        .default_type
        .as_deref()
        .map(super::names::identifier)
        .filter(|k| spec.types.iter().any(|t| type_key(t) == *k));
    typed.or_else(|| spec.types.first().map(type_key))
}

/// Generates the definitions of a custom caliber under the project prefix `prefix` (without underscore).
#[must_use]
pub fn generate(
    spec: &CustomAmmoSpec,
    prefix: &str,
    model: &CeModel,
    layout: &ProjectLayout,
) -> GeneratedAmmo {
    let names = DerivedNames::new(spec, prefix);
    let mut defs: Vec<Node> = Vec::new();
    let mut sections: Vec<SectionHeader> = Vec::new();
    let mut add = |defs: &mut Vec<Node>, node: Node| {
        let name = node.child_text("defName").unwrap_or_default().to_owned();
        sections.push(SectionHeader::banner(defs.len(), &name));
        defs.push(node);
    };
    let category = spec
        .category_parent
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());
    if let Some(parent) = category {
        let mut c = Node::new("ThingCategoryDef");
        c.push_child(leaf("defName", names.category()));
        c.push_child(leaf("label", spec.caliber.trim()));
        c.push_child(leaf("parent", parent));
        push_text(&mut c, "iconPath", spec.category_icon.as_ref());
        add(&mut defs, c);
    }
    let set_tag = model.classes.ammo_set_def.clone();
    let mut set = Node::new(set_tag);
    set.push_child(leaf("defName", names.set()));
    let set_label = spec
        .set_label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| spec.caliber.trim());
    set.push_child(leaf("label", set_label));
    let mut pairs = Node::new("ammoTypes");
    for t in &spec.types {
        let key = type_key(t);
        match model.ammo.pair_form {
            PairForm::Element => pairs.push_child(leaf(&names.ammo(&key), names.projectile(&key))),
            PairForm::Li => {
                let mut li = Node::new("li");
                li.push_child(leaf("ammo", names.ammo(&key)));
                li.push_child(leaf("projectile", names.projectile(&key)));
                pairs.push_child(li);
            }
        }
    }
    set.push_child(pairs);
    push_text(&mut set, "similarTo", spec.similar_to.as_ref());
    for e in &spec.set_extra {
        set.push_child(e.clone());
    }
    add(&mut defs, set);
    let category_name = category.map(|_| names.category());
    let mut art: Vec<(String, String)> = Vec::new();
    let (mut ammo, mut projectiles, mut recipes) = (Vec::new(), Vec::new(), Vec::new());
    for t in &spec.types {
        let key = type_key(t);
        add(
            &mut defs,
            item_def(spec, t, &names, category_name.as_deref(), model, &mut art),
        );
        add(&mut defs, projectile_def(spec, t, &names, model));
        add(&mut defs, recipe_def(spec, t, &names, model));
        ammo.push(names.ammo(&key));
        projectiles.push(names.projectile(&key));
        recipes.push(names.recipe(&key));
    }
    let diagnostics = art
        .iter()
        .map(|(what, path)| {
            ART_RESERVED.diagnostic(
                "",
                &[
                    ("what", what),
                    ("path", path),
                    ("file", &layout.texture_file(path)),
                ],
            )
        })
        .collect();
    GeneratedAmmo {
        stem: names.file_stem(),
        set_name: names.set(),
        default_projectile: default_key(spec).map(|k| names.projectile(&k)),
        defs,
        sections,
        ammo,
        projectiles,
        recipes,
        diagnostics,
    }
}
