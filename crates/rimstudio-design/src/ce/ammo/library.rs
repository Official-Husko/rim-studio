//! What the user's Combat Extended says about its ammunition: ammo classes, ammo items, projectiles,
//! recipes and thing categories, read from the resolved defs of a load.
//!
//! The library is data read at run time; the repository holds no value of it (R11). It serves the catalogue
//! ([`super::catalog`]), the suggestions for a new type ([`super::suggest`]) and the checks of a custom
//! caliber ([`super::validate`]).
//!
//! Resolved nodes carry what the abstract parents provide, so the library keeps, besides the modelled
//! fields, the children that are not provided by a base (a child that nearly every def of its kind has comes
//! from a base) as raw nodes: a type copied from existing ammunition then keeps what makes it special.

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, DefRecord};
use serde::{Deserialize, Serialize};

use super::names::{AMMO_CATEGORY_DEF, FRAGMENTS_COMP, PairForm};
use crate::ce::reader::AmmoSetInfo;
use crate::model::{CustomFragment, CustomSecondaryDamage};
use crate::reader::access::{
    child_bool, child_number, child_text, class_attr, list_texts, text_of,
};

/// Fraction of the defs of a kind that must carry a child for it to count as provided by a base.
const COMMON_SHARE: f64 = 0.8;

/// An ammo class (`AmmoCategoryDef`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoClassFacts {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// The short label, when the def has one.
    pub label_short: Option<String>,
    /// The description.
    pub description: Option<String>,
    /// Whether the class is an advanced one (the def's `advanced` flag).
    pub advanced: bool,
}

/// An ammo item (an `AmmoDef`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoFacts {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// The description.
    pub description: Option<String>,
    /// The nearest abstract parent.
    pub parent: Option<String>,
    /// The ammo class def name.
    pub ammo_class: String,
    /// The `Mass` stat.
    pub mass: Option<f64>,
    /// The `Bulk` stat.
    pub bulk: Option<f64>,
    /// The `MarketValue` stat, when the def sets one.
    pub market_value: Option<f64>,
    /// Stats besides mass, bulk and market value that no base provides.
    pub stats: BTreeMap<String, f64>,
    /// The draw size of the graphic, as written.
    pub draw_size: Option<String>,
    /// Other children of the graphic data, as written.
    pub graphic_extra: Vec<Node>,
    /// The stack limit.
    pub stack_limit: Option<f64>,
    /// The thing categories.
    pub thing_categories: Vec<String>,
    /// The trade tags.
    pub trade_tags: Vec<String>,
    /// The texture path.
    pub tex_path: Option<String>,
    /// The graphic class.
    pub graphic_class: Option<String>,
    /// The tech level.
    pub tech_level: Option<String>,
    /// The projectile fired when the item cooks off.
    pub cook_off_projectile: Option<String>,
    /// The projectile that detonates when the item cooks off.
    pub detonate_projectile: Option<String>,
    /// Children that no base provides, as written.
    pub extra: Vec<Node>,
}

/// A projectile def with its Combat Extended properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectileFacts {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: Option<String>,
    /// The nearest abstract parent.
    pub parent: Option<String>,
    /// The `thingClass`.
    pub thing_class: Option<String>,
    /// The damage def.
    pub damage_def: Option<String>,
    /// Damage per projectile.
    pub damage: Option<f64>,
    /// Sharp penetration.
    pub ap_sharp: Option<f64>,
    /// Blunt penetration.
    pub ap_blunt: Option<f64>,
    /// Speed.
    pub speed: Option<f64>,
    /// Pellets per shot.
    pub pellets: Option<f64>,
    /// Spread multiplier.
    pub spread_mult: Option<f64>,
    /// Secondary damage entries.
    pub secondary: Vec<(String, f64)>,
    /// Explosion radius.
    pub explosion_radius: Option<f64>,
    /// Whether the explosion reaches the neighbouring cells.
    pub explosion_neighbors: Option<bool>,
    /// Suppression factor.
    pub suppression: Option<f64>,
    /// Danger factor.
    pub danger: Option<f64>,
    /// Whether a casing drops.
    pub drops_casings: Option<bool>,
    /// The casing fleck.
    pub casing_mote: Option<String>,
    /// The casing filth.
    pub casing_filth: Option<String>,
    /// The AI incendiary flag.
    pub incendiary: Option<bool>,
    /// The fly overhead flag.
    pub fly_overhead: Option<bool>,
    /// The texture path.
    pub tex_path: Option<String>,
    /// The graphic class.
    pub graphic_class: Option<String>,
    /// The draw size as written.
    pub draw_size: Option<String>,
    /// Other children of the graphic data, as written.
    pub graphic_extra: Vec<Node>,
    /// The explosion sound.
    pub sound_explode: Option<String>,
    /// The flight sound.
    pub sound_ambient: Option<String>,
    /// The thick roof impact sound.
    pub sound_hit_thick_roof: Option<String>,
    /// The anticipation sound.
    pub sound_impact_anticipate: Option<String>,
    /// Fragments.
    pub fragments: Vec<CustomFragment>,
    /// Children of the properties that have no typed place, as written.
    pub extra: Vec<Node>,
    /// Other components and children of the def that no base provides, as written.
    pub thing_extra: Vec<Node>,
}

impl ProjectileFacts {
    /// The secondary damage as the model's entries.
    #[must_use]
    pub fn secondary_entries(&self) -> Vec<CustomSecondaryDamage> {
        self.secondary
            .iter()
            .map(|(def, amount)| CustomSecondaryDamage {
                def: def.clone(),
                amount: Some(crate::model::Sourced::suggested(*amount)),
            })
            .collect()
    }

    /// The energy index used to rank ammunition: damage times speed. An implementation choice that
    /// orders calibers well enough to find a neighbour; it is not a physical energy.
    #[must_use]
    pub fn energy(&self) -> Option<f64> {
        let e = self.damage? * self.speed?;
        (e > 0.0).then_some(e)
    }
}

/// One ingredient of a recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngredientFacts {
    /// The thing defs the ingredient accepts.
    pub things: Vec<String>,
    /// The categories it accepts.
    pub categories: Vec<String>,
    /// How many are needed.
    pub count: f64,
}

/// A recipe that makes an ammo item.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFacts {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: Option<String>,
    /// The description.
    pub description: Option<String>,
    /// The job string.
    pub job_string: Option<String>,
    /// The nearest abstract parent.
    pub parent: Option<String>,
    /// The ingredients.
    pub ingredients: Vec<IngredientFacts>,
    /// Items made per craft.
    pub products: Option<f64>,
    /// Work amount.
    pub work_amount: Option<f64>,
    /// The workbenches that offer it.
    pub users: Vec<String>,
    /// The research prerequisite.
    pub research: Option<String>,
    /// The research prerequisites that must all be done.
    pub research_list: Vec<String>,
    /// Children no base provides, as written.
    pub extra: Vec<Node>,
    /// The Crafting level needed.
    pub skill_level: Option<f64>,
}

/// A thing category def.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryFacts {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// The parent category.
    pub parent: Option<String>,
    /// The icon path.
    pub icon_path: Option<String>,
}

/// The ammunition of the user's Combat Extended.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoLibrary {
    /// The ammo classes, in database order.
    pub classes: Vec<AmmoClassFacts>,
    /// The ammo items by def name.
    pub ammo: BTreeMap<String, AmmoFacts>,
    /// The projectiles of the ammo sets by def name.
    pub projectiles: BTreeMap<String, ProjectileFacts>,
    /// The recipes by the def name of the ammo item they make.
    pub recipes: BTreeMap<String, RecipeFacts>,
    /// The thing categories of the ammo items and their ancestors.
    pub categories: BTreeMap<String, CategoryFacts>,
    /// The labels of the ammo sets by def name.
    pub set_labels: BTreeMap<String, String>,
    /// The children of the ammo set defs besides the label, the pairs and `similarTo`, as written.
    pub set_extra: BTreeMap<String, Vec<Node>>,
    /// How the ammo sets of the install write their pairs.
    pub pair_form: PairForm,
    /// The field names of the projectile properties seen in the install, with how many projectiles carry
    /// each (the fields a custom projectile may use).
    pub projectile_fields: BTreeMap<String, u32>,
}

impl AmmoLibrary {
    /// True when the install holds no ammunition data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ammo.is_empty() && self.projectiles.is_empty() && self.classes.is_empty()
    }

    /// The label of an ammo set.
    #[must_use]
    pub fn set_label(&self, def_name: &str) -> Option<String> {
        self.set_labels.get(def_name).cloned()
    }

    /// The ammo class with the def name.
    #[must_use]
    pub fn class(&self, def_name: &str) -> Option<&AmmoClassFacts> {
        self.classes.iter().find(|c| c.def_name == def_name)
    }

    /// The label of an ammo class: its short label, else its label, else the def name.
    #[must_use]
    pub fn class_label(&self, def_name: &str) -> String {
        self.class(def_name).map_or_else(
            || def_name.to_owned(),
            |c| c.label_short.clone().unwrap_or_else(|| c.label.clone()),
        )
    }

    /// The caliber and the family of an ammo item: the label of its first thing category and the label of
    /// that category's parent.
    #[must_use]
    pub fn caliber_of(&self, ammo: &str) -> Option<(String, Option<String>)> {
        let item = self.ammo.get(ammo)?;
        let cat = item
            .thing_categories
            .iter()
            .find_map(|c| self.categories.get(c))?;
        let family = cat
            .parent
            .as_ref()
            .and_then(|p| self.categories.get(p))
            .map(|p| p.label.clone());
        Some((cat.label.clone(), family))
    }
}

/// The nearest parent of a def.
fn parent_of(def: &DefRecord) -> Option<String> {
    def.parents.first().map(|p| p.name.clone())
}

fn graphic(node: &Node) -> (Option<String>, Option<String>, Option<String>) {
    match node.child("graphicData") {
        Some(g) => (
            child_text(g, "texPath"),
            child_text(g, "graphicClass"),
            child_text(g, "drawSize"),
        ),
        None => (None, None, None),
    }
}

/// The children that most defs of a kind share unchanged: the content (as JSON) of each child name that
/// at least [`COMMON_SHARE`] of the defs carry with the same content. Such a child comes from a base.
fn modal(nodes: &[&Node]) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<(&str, String), usize> = BTreeMap::new();
    for n in nodes {
        for c in n.elements() {
            *counts
                .entry((c.tag.as_str(), c.to_json_string()))
                .or_insert(0) += 1;
        }
    }
    let need = ((nodes.len() as f64 * COMMON_SHARE).ceil() as usize).max(2);
    counts
        .into_iter()
        .filter(|(_, c)| *c >= need)
        .map(|((t, json), _)| (t.to_owned(), json))
        .collect()
}

fn is_common(modal: &BTreeMap<String, String>, node: &Node) -> bool {
    modal
        .get(&node.tag)
        .is_some_and(|j| *j == node.to_json_string())
}

/// The children of `children` that are not modelled and not provided by a base. A list that merges with
/// its parent's (`modExtensions`, a list of `li` entries) is written as a replacement.
fn leftovers<'a>(
    children: impl Iterator<Item = &'a Node>,
    modelled: &[&str],
    common: &BTreeMap<String, String>,
) -> Vec<Node> {
    children
        .filter(|c| !modelled.contains(&c.tag.as_str()) && !is_common(common, c))
        .map(|c| {
            let mut c = c.clone();
            // A list merges with its parent's; a copy replaces it so nothing is written twice.
            if c.tag == "modExtensions"
                || (c.elements().next().is_some() && c.elements().all(|e| e.tag == "li"))
            {
                c.set_attr("Inherit", "False");
            }
            c
        })
        .collect()
}

/// The share of the defs of a kind that carry each component entry, by the entry's JSON.
fn common_comps(nodes: &[&Node]) -> BTreeSet<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for n in nodes {
        for li in n
            .child("comps")
            .into_iter()
            .flat_map(|c| c.children_named("li"))
        {
            *counts.entry(li.to_json_string()).or_insert(0) += 1;
        }
    }
    let need = ((nodes.len() as f64 * COMMON_SHARE).ceil() as usize).max(2);
    counts
        .into_iter()
        .filter(|(_, c)| *c >= need)
        .map(|(j, _)| j)
        .collect()
}

/// The modal value of each stat of `statBases` that at least [`COMMON_SHARE`] of the defs carry unchanged.
fn common_stats(nodes: &[&Node]) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for n in nodes {
        for st in n.child("statBases").into_iter().flat_map(Node::elements) {
            *counts
                .entry((st.tag.clone(), st.text_content().trim().to_owned()))
                .or_insert(0) += 1;
        }
    }
    let need = ((nodes.len() as f64 * COMMON_SHARE).ceil() as usize).max(2);
    counts
        .into_iter()
        .filter(|(_, c)| *c >= need)
        .map(|((t, v), _)| (t, v))
        .collect()
}

const AMMO_MODELLED: [&str; 13] = [
    "defName",
    "label",
    "description",
    "statBases",
    "stackLimit",
    "thingCategories",
    "tradeTags",
    "graphicData",
    "ammoClass",
    "techLevel",
    "cookOffProjectile",
    "detonateProjectile",
    "comps",
];

const PROJECTILE_MODELLED: [&str; 7] = [
    "defName",
    "label",
    "description",
    "graphicData",
    "thingClass",
    "projectile",
    "comps",
];

const PROPS_MODELLED: [&str; 21] = [
    "damageDef",
    "damageAmountBase",
    "armorPenetrationSharp",
    "armorPenetrationBlunt",
    "speed",
    "pelletCount",
    "spreadMult",
    "secondaryDamage",
    "explosionRadius",
    "applyDamageToExplosionCellsNeighbors",
    "suppressionFactor",
    "dangerFactor",
    "dropsCasings",
    "casingMoteDefname",
    "casingFilthDefname",
    "ai_IsIncendiary",
    "flyOverhead",
    "soundExplode",
    "soundAmbient",
    "soundHitThickRoof",
    "soundImpactAnticipate",
];

/// What a kind of def shares: the children, the component entries and the stats that come from a base.
struct Shared {
    children: BTreeMap<String, String>,
    comps: BTreeSet<String>,
    stats: BTreeMap<String, String>,
    graphic: BTreeMap<String, String>,
    props: BTreeMap<String, String>,
}

fn shared(nodes: &[&Node]) -> Shared {
    let graphics: Vec<&Node> = nodes
        .iter()
        .filter_map(|n| n.child("graphicData"))
        .collect();
    let props: Vec<&Node> = nodes.iter().filter_map(|n| n.child("projectile")).collect();
    Shared {
        children: modal(nodes),
        comps: common_comps(nodes),
        stats: common_stats(nodes),
        graphic: modal(&graphics),
        props: modal(&props),
    }
}

/// The smallest group of defs under one parent whose own shared content is trusted.
const MIN_GROUP: usize = 3;

/// What is shared by all defs of a kind, and by the defs under each parent: a def under a parent with
/// enough siblings is measured against its siblings (a value that differs from the siblings' is its own),
/// any other def against the whole kind.
struct SharedSet {
    global: Shared,
    groups: BTreeMap<String, Shared>,
}

impl SharedSet {
    fn new(defs: &[&DefRecord]) -> Self {
        let nodes: Vec<&Node> = defs.iter().map(|d| &d.node).collect();
        let mut by_parent: BTreeMap<String, Vec<&Node>> = BTreeMap::new();
        for d in defs {
            if let Some(p) = parent_of(d) {
                by_parent.entry(p).or_default().push(&d.node);
            }
        }
        Self {
            global: shared(&nodes),
            groups: by_parent
                .into_iter()
                .filter(|(_, v)| v.len() >= MIN_GROUP)
                .map(|(p, v)| (p, shared(&v)))
                .collect(),
        }
    }

    fn of(&self, def: &DefRecord) -> &Shared {
        parent_of(def)
            .and_then(|p| self.groups.get(&p))
            .unwrap_or(&self.global)
    }
}

fn graphic_extra(node: &Node, shared: &Shared) -> Vec<Node> {
    node.child("graphicData")
        .map(|g| {
            leftovers(
                g.elements(),
                &["texPath", "graphicClass", "drawSize"],
                &shared.graphic,
            )
        })
        .unwrap_or_default()
}

fn read_ammo(def: &DefRecord, shared: &Shared) -> AmmoFacts {
    let n = &def.node;
    let stat = |name: &str| n.child("statBases").and_then(|s| child_number(s, name));
    let (tex_path, graphic_class, draw_size) = graphic(n);
    let mut extra = leftovers(n.elements(), &AMMO_MODELLED, &shared.children);
    if let Some(comps) = n.child("comps")
        && comps
            .children_named("li")
            .any(|li| !shared.comps.contains(&li.to_json_string()))
    {
        let mut c = comps.clone();
        c.set_attr("Inherit", "False");
        extra.push(c);
    }
    let stats = n
        .child("statBases")
        .into_iter()
        .flat_map(Node::elements)
        .filter(|st| !matches!(st.tag.as_str(), "Mass" | "Bulk" | "MarketValue"))
        .filter(|st| {
            shared
                .stats
                .get(&st.tag)
                .is_none_or(|v| *v != st.text_content().trim())
        })
        .filter_map(|st| Some((st.tag.clone(), parse(&st.text_content())?)))
        .collect();
    AmmoFacts {
        def_name: def.def_name.clone(),
        label: child_text(n, "label").unwrap_or_default(),
        description: child_text(n, "description"),
        parent: parent_of(def),
        ammo_class: child_text(n, "ammoClass").unwrap_or_default(),
        mass: stat("Mass"),
        bulk: stat("Bulk"),
        market_value: stat("MarketValue"),
        stats,
        draw_size,
        graphic_extra: graphic_extra(n, shared),
        stack_limit: child_number(n, "stackLimit"),
        thing_categories: list_texts(n, "thingCategories"),
        trade_tags: list_texts(n, "tradeTags"),
        tex_path,
        graphic_class,
        tech_level: child_text(n, "techLevel"),
        cook_off_projectile: child_text(n, "cookOffProjectile"),
        detonate_projectile: child_text(n, "detonateProjectile"),
        extra,
    }
}

fn parse(text: &str) -> Option<f64> {
    crate::reader::access::parse_number(text)
}

fn read_projectile(def: &DefRecord, shared: &Shared) -> ProjectileFacts {
    let n = &def.node;
    let props = n.child("projectile");
    let num = |name: &str| props.and_then(|p| child_number(p, name));
    let flag = |name: &str| props.and_then(|p| child_bool(p, name));
    let text = |name: &str| props.and_then(|p| child_text(p, name));
    let (tex_path, graphic_class, draw_size) = graphic(n);
    let secondary_node = props.and_then(|p| p.child("secondaryDamage"));
    let secondary: Vec<(String, f64)> = secondary_node
        .map(|s| {
            s.children_named("li")
                .filter_map(|li| Some((child_text(li, "def")?, child_number(li, "amount")?)))
                .collect()
        })
        .unwrap_or_default();
    // A secondary damage entry with children besides its def and amount (a chance) stays raw.
    let secondary_plain = secondary_node.is_none_or(|s| {
        s.children_named("li").all(|li| {
            li.elements()
                .all(|e| matches!(e.tag.as_str(), "def" | "amount"))
        })
    });
    let mut fragments = Vec::new();
    let mut thing_extra: Vec<Node> = Vec::new();
    if let Some(comps) = n.child("comps") {
        let mut rest = Node::new("comps");
        rest.set_attr("Inherit", "False");
        let mut unusual = false;
        for li in comps.children_named("li") {
            let only_fragments = class_attr(li) == Some(FRAGMENTS_COMP)
                && li.elements().all(|e| e.tag == "fragments");
            if only_fragments {
                for f in li.elements().flat_map(Node::elements) {
                    if let Some(count) = text_of(f).and_then(|t| t.parse::<f64>().ok()) {
                        fragments.push(CustomFragment {
                            def: f.tag.clone(),
                            count: count.max(0.0) as u32,
                        });
                    }
                }
            } else {
                unusual |= !shared.comps.contains(&li.to_json_string());
                rest.push_child(li.clone());
            }
        }
        if unusual {
            thing_extra.push(rest);
        }
    }
    thing_extra.extend(leftovers(
        n.elements(),
        &PROJECTILE_MODELLED,
        &shared.children,
    ));
    let mut extra = props
        .map(|p| leftovers(p.elements(), &PROPS_MODELLED, &shared.props))
        .unwrap_or_default();
    if !secondary_plain && let Some(s) = secondary_node {
        extra.push(s.clone());
    }
    ProjectileFacts {
        def_name: def.def_name.clone(),
        label: child_text(n, "label"),
        parent: parent_of(def),
        thing_class: child_text(n, "thingClass"),
        damage_def: text("damageDef"),
        damage: num("damageAmountBase"),
        ap_sharp: num("armorPenetrationSharp"),
        ap_blunt: num("armorPenetrationBlunt"),
        speed: num("speed"),
        pellets: num("pelletCount"),
        spread_mult: num("spreadMult"),
        secondary: if secondary_plain {
            secondary
        } else {
            Vec::new()
        },
        explosion_radius: num("explosionRadius"),
        explosion_neighbors: flag("applyDamageToExplosionCellsNeighbors"),
        suppression: num("suppressionFactor"),
        danger: num("dangerFactor"),
        drops_casings: flag("dropsCasings"),
        casing_mote: text("casingMoteDefname"),
        casing_filth: text("casingFilthDefname"),
        incendiary: flag("ai_IsIncendiary"),
        fly_overhead: flag("flyOverhead"),
        tex_path,
        graphic_class,
        draw_size,
        graphic_extra: graphic_extra(n, shared),
        sound_explode: text("soundExplode"),
        sound_ambient: text("soundAmbient"),
        sound_hit_thick_roof: text("soundHitThickRoof"),
        sound_impact_anticipate: text("soundImpactAnticipate"),
        fragments,
        extra,
        thing_extra,
    }
}

const RECIPE_MODELLED: [&str; 11] = [
    "defName",
    "label",
    "description",
    "jobString",
    "ingredients",
    "fixedIngredientFilter",
    "products",
    "workAmount",
    "recipeUsers",
    "researchPrerequisite",
    "researchPrerequisites",
];

fn read_recipe(def: &DefRecord, common: &Shared) -> RecipeFacts {
    let n = &def.node;
    let ingredients: Vec<IngredientFacts> = n
        .child("ingredients")
        .map(|list| {
            list.children_named("li")
                .filter_map(|li| {
                    let filter = li.child("filter")?;
                    Some(IngredientFacts {
                        things: list_texts(filter, "thingDefs"),
                        categories: list_texts(filter, "categories"),
                        count: child_number(li, "count").unwrap_or(1.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut extra = leftovers(
        n.elements().filter(|c| c.tag != "skillRequirements"),
        &RECIPE_MODELLED,
        &common.children,
    );
    if let Some(fixed) = n.child("fixedIngredientFilter") {
        let (mut things, mut cats): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for i in &ingredients {
            for t in &i.things {
                if !things.contains(t) {
                    things.push(t.clone());
                }
            }
            for c in &i.categories {
                if !cats.contains(c) {
                    cats.push(c.clone());
                }
            }
        }
        if list_texts(fixed, "thingDefs") != things || list_texts(fixed, "categories") != cats {
            extra.push(fixed.clone());
        }
    }
    let skill = n.child("skillRequirements");
    let plain_skill = skill.is_none_or(|s| s.elements().all(|e| e.tag == "Crafting"));
    if !plain_skill && let Some(s) = skill {
        extra.push(s.clone());
    }
    RecipeFacts {
        def_name: def.def_name.clone(),
        label: child_text(n, "label"),
        description: child_text(n, "description"),
        job_string: child_text(n, "jobString"),
        parent: parent_of(def),
        ingredients,
        products: None,
        work_amount: child_number(n, "workAmount"),
        users: list_texts(n, "recipeUsers"),
        research: n
            .child("researchPrerequisite")
            .map(|c| text_of(c).unwrap_or_default()),
        research_list: list_texts(n, "researchPrerequisites"),
        extra,
        skill_level: if plain_skill {
            skill.and_then(|s| child_number(s, "Crafting"))
        } else {
            None
        },
    }
}

/// The products of a recipe as `(ammo def name, count)`: both the element form and the list form.
fn products_of(def: &DefRecord) -> Vec<(String, f64)> {
    let Some(list) = def.node.child("products") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in list.elements() {
        if e.tag == "li" {
            if let (Some(t), Some(c)) = (child_text(e, "thingDef"), child_number(e, "count")) {
                out.push((t, c));
            }
        } else if let Some(c) = text_of(e).and_then(|t| t.parse::<f64>().ok()) {
            out.push((e.tag.clone(), c));
        }
    }
    out
}

fn pair_form(dbs: &DefDatabases, set_type: &str) -> PairForm {
    let Ok(view) = dbs.database(set_type) else {
        return PairForm::Element;
    };
    let li = view.iter().any(|d| {
        d.node
            .child("ammoTypes")
            .is_some_and(|t| t.elements().any(|e| e.tag == "li"))
    });
    if li { PairForm::Li } else { PairForm::Element }
}

/// Reads the ammunition of a load. `db_type` is the thing database, `set_type` the ammo set def type and
/// `sets` the sets already read from it (their projectiles are the ones kept).
#[must_use]
pub fn read_library(
    dbs: &DefDatabases,
    db_type: &str,
    set_type: &str,
    sets: &[AmmoSetInfo],
) -> AmmoLibrary {
    let mut lib = AmmoLibrary {
        pair_form: pair_form(dbs, set_type),
        ..AmmoLibrary::default()
    };
    if let Ok(view) = dbs.database(set_type) {
        for d in view.iter() {
            if let Some(label) = child_text(&d.node, "label") {
                lib.set_labels.insert(d.def_name.clone(), label);
            }
            let extra: Vec<Node> = d
                .node
                .elements()
                .filter(|c| {
                    !matches!(
                        c.tag.as_str(),
                        "defName" | "label" | "ammoTypes" | "similarTo"
                    )
                })
                .cloned()
                .collect();
            if !extra.is_empty() {
                lib.set_extra.insert(d.def_name.clone(), extra);
            }
        }
    }
    if let Ok(view) = dbs.database(AMMO_CATEGORY_DEF) {
        for d in view.iter() {
            lib.classes.push(AmmoClassFacts {
                def_name: d.def_name.clone(),
                label: child_text(&d.node, "label").unwrap_or_default(),
                label_short: child_text(&d.node, "labelShort"),
                description: child_text(&d.node, "description"),
                advanced: child_bool(&d.node, "advanced").unwrap_or(false),
            });
        }
    }
    let Ok(things) = dbs.database(db_type) else {
        return lib;
    };
    let ammo_defs: Vec<&DefRecord> = things
        .iter()
        .filter(|d| d.node.child("ammoClass").is_some())
        .collect();
    let ammo_shared = SharedSet::new(&ammo_defs);
    for d in &ammo_defs {
        lib.ammo
            .insert(d.def_name.clone(), read_ammo(d, ammo_shared.of(d)));
    }
    let mut wanted: BTreeSet<&str> = BTreeSet::new();
    for s in sets {
        wanted.extend(s.ammo_types.iter().map(|a| a.projectile.as_str()));
    }
    for a in lib.ammo.values() {
        wanted.extend(a.cook_off_projectile.as_deref());
        wanted.extend(a.detonate_projectile.as_deref());
    }
    let projectile_defs: Vec<&DefRecord> = wanted
        .iter()
        .filter_map(|name| things.get(name))
        .filter(|d| d.node.child("projectile").is_some())
        .collect();
    let projectile_shared = SharedSet::new(&projectile_defs);
    for d in &projectile_defs {
        for e in d
            .node
            .child("projectile")
            .into_iter()
            .flat_map(Node::elements)
        {
            *lib.projectile_fields.entry(e.tag.clone()).or_insert(0) += 1;
        }
        lib.projectiles.insert(
            d.def_name.clone(),
            read_projectile(d, projectile_shared.of(d)),
        );
    }
    if let Ok(recipes) = dbs.database("RecipeDef") {
        let mine: Vec<&DefRecord> = recipes
            .iter()
            .filter(|d| products_of(d).iter().any(|(p, _)| lib.ammo.contains_key(p)))
            .collect();
        let recipe_shared = SharedSet::new(&mine);
        for d in mine {
            for (product, count) in products_of(d) {
                if lib.ammo.contains_key(&product) && !lib.recipes.contains_key(&product) {
                    let mut facts = read_recipe(d, recipe_shared.of(d));
                    facts.products = Some(count);
                    lib.recipes.insert(product, facts);
                }
            }
        }
    }
    if let Ok(cats) = dbs.database("ThingCategoryDef") {
        let mut want: Vec<String> = lib
            .ammo
            .values()
            .flat_map(|a| a.thing_categories.iter().cloned())
            .collect();
        while let Some(name) = want.pop() {
            if lib.categories.contains_key(&name) {
                continue;
            }
            let Some(d) = cats.get(&name) else { continue };
            let parent = child_text(&d.node, "parent");
            want.extend(parent.clone());
            lib.categories.insert(
                name.clone(),
                CategoryFacts {
                    def_name: name,
                    label: child_text(&d.node, "label").unwrap_or_default(),
                    parent,
                    icon_path: child_text(&d.node, "iconPath"),
                },
            );
        }
    }
    lib
}
