//! Defaults for a new ammo type, taken from the user's own ammunition, and copies of existing types.
//!
//! [`suggest_type`] finds the nearest ammunition of the user's install by ammo class and by an energy index
//! (damage times speed; see [`ProjectileFacts::energy`]) and fills a [`CustomAmmoType`] with suggested
//! values: a number is the median of the nearest few, rated by how closely they agree; a name (parent,
//! thing class, damage def, graphic) is the nearest one's. [`type_from_existing`] copies one real type
//! completely, which is also how the checks of the generator are measured against the real definitions.
//! Nothing here is a built in value: with no ammunition in the install nothing is suggested.

use serde::{Deserialize, Serialize};

use super::library::{AmmoFacts, ProjectileFacts, RecipeFacts};
use super::names::identifier;
use crate::ce::classes::Reliability;
use crate::ce::reader::CeModel;
use crate::model::{
    CookOffKind, CustomAmmoItem, CustomAmmoRecipe, CustomAmmoSpec, CustomAmmoType,
    CustomIngredient, CustomProjectile, Sourced, ValueSource,
};

/// How many neighbours a suggestion looks at.
const NEIGHBOURS: usize = 5;
/// Relative spread below which the neighbours count as agreeing (reliable).
const AGREE: f64 = 0.15;
/// Relative spread below which the neighbours count as roughly agreeing.
const ROUGH: f64 = 0.4;

/// What the user knows about the caliber they are making.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AmmoHints {
    /// A caliber name to compare labels with (information; shown, not used for numbers).
    pub caliber: Option<String>,
    /// The damage the user wants; it is used as given and also places the type on the energy scale.
    pub damage: Option<f64>,
    /// The speed the user wants.
    pub speed: Option<f64>,
    /// An existing ammo set whose ammunition is the yardstick (its type of the same class).
    pub similar_set: Option<String>,
}

/// An existing ammo type near the one asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearestAmmo {
    /// The ammo item def name.
    pub ammo_def: String,
    /// Its label.
    pub ammo_label: String,
    /// The ammo set it was found in.
    pub set: Option<String>,
    /// The projectile def name.
    pub projectile_def: String,
    /// Damage.
    pub damage: Option<f64>,
    /// Speed.
    pub speed: Option<f64>,
    /// The energy index.
    pub energy: Option<f64>,
    /// Distance from the target on the energy scale (natural log ratio); zero for a copy.
    pub distance: f64,
}

/// Where a suggested value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AmmoSourceKind {
    /// Copied from the ammunition the user chose to start from.
    Copied,
    /// The median of the nearest ammunition of the same class.
    Nearest,
    /// The user's hint, used as given.
    Hint,
}

/// One suggested field of the new type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedAmmoField {
    /// The JSON pointer of the field inside the type (`/projectile/damage`).
    pub field: String,
    /// What the field is called.
    pub label: String,
    /// The number, for a numeric field.
    pub value: Option<f64>,
    /// The name or text, for a text field.
    pub text: Option<String>,
    /// Where it comes from.
    pub source: AmmoSourceKind,
    /// How far it can be trusted.
    pub rating: Reliability,
    /// How many ammunition types it rests on.
    pub n: usize,
    /// The ammo defs it rests on.
    pub from: Vec<String>,
    /// The smallest and the largest number among the neighbours.
    pub range: Option<(f64, f64)>,
}

/// The answer of [`suggest_type`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoSuggestion {
    /// False when the install has no ammunition data.
    pub available: bool,
    /// Why nothing is suggested.
    pub reason: Option<String>,
    /// The ammo class asked for.
    pub class: String,
    /// The label of the class.
    pub class_label: String,
    /// The ammo def the type was copied from, when a copy was asked for.
    pub copied_from: Option<String>,
    /// The nearest ammunition, nearest first.
    pub nearest: Vec<NearestAmmo>,
    /// The suggested fields with their sources and ratings.
    pub fields: Vec<SuggestedAmmoField>,
    /// A type filled with the suggestions (every value `suggested` or `answered`).
    pub ammo_type: CustomAmmoType,
    /// Remarks, for example that the class has no ammunition in the install.
    pub notes: Vec<String>,
}

fn sourced(value: f64, source: ValueSource) -> Option<Sourced<f64>> {
    Some(Sourced::new(value, source))
}

fn sourced_u32(value: f64, source: ValueSource) -> Option<Sourced<u32>> {
    (value.is_finite() && value >= 0.0).then(|| Sourced::new(value.round() as u32, source))
}

/// The projectile an ammo item is measured by: the first pair of an ammo set that names it, else its cook
/// off or detonation projectile.
fn projectile_for<'a>(
    model: &'a CeModel,
    ammo: &AmmoFacts,
) -> Option<(&'a ProjectileFacts, Option<&'a str>)> {
    for set in &model.ammo_sets {
        if let Some(pair) = set.ammo_types.iter().find(|a| a.ammo == ammo.def_name)
            && let Some(p) = model.ammo.projectiles.get(&pair.projectile)
        {
            return Some((p, Some(set.def_name.as_str())));
        }
    }
    let name = ammo
        .cook_off_projectile
        .as_deref()
        .or(ammo.detonate_projectile.as_deref())?;
    model.ammo.projectiles.get(name).map(|p| (p, None))
}

fn recipe_type(r: &RecipeFacts, source: ValueSource, with_extra: bool) -> CustomAmmoRecipe {
    CustomAmmoRecipe {
        label: r.label.clone(),
        description: r.description.clone(),
        job_string: r.job_string.clone(),
        parent: r.parent.clone(),
        ingredients: r
            .ingredients
            .iter()
            .map(|i| CustomIngredient {
                thing: i.things.first().cloned().unwrap_or_default(),
                alternatives: i.things.iter().skip(1).cloned().collect(),
                categories: i.categories.clone(),
                count: sourced(i.count, source),
            })
            .collect(),
        products: r.products.and_then(|p| sourced_u32(p, source)),
        work_amount: r.work_amount.and_then(|w| sourced(w, source)),
        users: r.users.clone(),
        research_prerequisite: r.research.clone(),
        research_prerequisites: r.research_list.clone(),
        extra: if with_extra {
            r.extra.clone()
        } else {
            Vec::new()
        },
        skill_level: r.skill_level.and_then(|s| sourced_u32(s, source)),
    }
}

fn projectile_type(p: &ProjectileFacts, source: ValueSource, with_extra: bool) -> CustomProjectile {
    let n = |v: Option<f64>| v.and_then(|v| sourced(v, source));
    CustomProjectile {
        label: p.label.clone(),
        parent: p.parent.clone(),
        thing_class: p.thing_class.clone(),
        damage_def: p.damage_def.clone(),
        damage: n(p.damage),
        armor_penetration_sharp: n(p.ap_sharp),
        armor_penetration_blunt: n(p.ap_blunt),
        speed: n(p.speed),
        pellet_count: n(p.pellets),
        spread_mult: n(p.spread_mult),
        secondary_damage: p
            .secondary
            .iter()
            .map(|(def, amount)| crate::model::CustomSecondaryDamage {
                def: def.clone(),
                amount: sourced(*amount, source),
            })
            .collect(),
        explosion_radius: n(p.explosion_radius),
        explosion_neighbors: p.explosion_neighbors,
        suppression_factor: n(p.suppression),
        danger_factor: n(p.danger),
        drops_casings: p.drops_casings,
        casing_mote: p.casing_mote.clone(),
        casing_filth: p.casing_filth.clone(),
        incendiary: p.incendiary,
        fly_overhead: p.fly_overhead,
        tex_path: p.tex_path.clone(),
        graphic_class: p.graphic_class.clone(),
        draw_size: p.draw_size.clone(),
        graphic_extra: if with_extra {
            p.graphic_extra.clone()
        } else {
            Vec::new()
        },
        sound_explode: p.sound_explode.clone(),
        sound_ambient: p.sound_ambient.clone(),
        sound_hit_thick_roof: p.sound_hit_thick_roof.clone(),
        sound_impact_anticipate: p.sound_impact_anticipate.clone(),
        fragments: p.fragments.clone(),
        extra: if with_extra {
            p.extra.clone()
        } else {
            Vec::new()
        },
        thing_extra: if with_extra {
            p.thing_extra.clone()
        } else {
            Vec::new()
        },
    }
}

fn item_type(a: &AmmoFacts, source: ValueSource, with_extra: bool) -> CustomAmmoItem {
    CustomAmmoItem {
        parent: a.parent.clone(),
        mass: a.mass.and_then(|v| sourced(v, source)),
        bulk: a.bulk.and_then(|v| sourced(v, source)),
        market_value: a.market_value.and_then(|v| sourced(v, source)),
        draw_size: a.draw_size.clone(),
        graphic_extra: if with_extra {
            a.graphic_extra.clone()
        } else {
            Vec::new()
        },
        stat_bases: if with_extra {
            a.stats
                .iter()
                .map(|(k, v)| (k.clone(), Sourced::new(*v, source)))
                .collect()
        } else {
            std::collections::BTreeMap::new()
        },
        stack_limit: a.stack_limit.and_then(|v| sourced_u32(v, source)),
        thing_categories: if with_extra {
            a.thing_categories.clone()
        } else {
            Vec::new()
        },
        trade_tags: a.trade_tags.clone(),
        tex_path: a.tex_path.clone(),
        graphic_class: a.graphic_class.clone(),
        tech_level: a.tech_level.clone(),
        cook_off: Some(if a.detonate_projectile.is_some() {
            CookOffKind::Detonate
        } else if a.cook_off_projectile.is_some() {
            CookOffKind::Projectile
        } else {
            CookOffKind::None
        }),
        extra: if with_extra {
            a.extra.clone()
        } else {
            Vec::new()
        },
    }
}

/// A type copied completely from the ammo item `ammo` and the projectile `projectile` (the pair of an ammo
/// set). Every number carries `source`. `None` when the ammo item is unknown.
#[must_use]
pub fn type_from_pair(
    model: &CeModel,
    ammo: &str,
    projectile: &str,
    source: ValueSource,
) -> Option<CustomAmmoType> {
    let item = model.ammo.ammo.get(ammo)?;
    let facts = model.ammo.projectiles.get(projectile)?;
    Some(CustomAmmoType {
        key: if item.ammo_class.is_empty() {
            String::new()
        } else {
            identifier(&model.ammo.class_label(&item.ammo_class))
        },
        ammo_class: item.ammo_class.clone(),
        label: Some(item.label.clone()),
        description: item.description.clone(),
        copied_from: Some(item.def_name.clone()),
        projectile: projectile_type(facts, source, true),
        item: item_type(item, source, true),
        recipe: model
            .ammo
            .recipes
            .get(ammo)
            .map(|r| recipe_type(r, source, true))
            .unwrap_or_default(),
    })
}

/// A type copied completely from an ammo item, with the projectile of its first ammo set (else its cook
/// off projectile). `None` when the item or its projectile is unknown.
#[must_use]
pub fn type_from_existing(
    model: &CeModel,
    ammo: &str,
    source: ValueSource,
) -> Option<CustomAmmoType> {
    let item = model.ammo.ammo.get(ammo)?;
    let (facts, _) = projectile_for(model, item)?;
    type_from_pair(model, ammo, &facts.def_name, source)
}

/// A whole custom caliber copied from an existing ammo set: one type per pair of the set. The def names of
/// the copy derive from the set's name with the prefix of the project. `None` when the set is unknown.
#[must_use]
pub fn set_from_existing(
    model: &CeModel,
    set: &str,
    source: ValueSource,
) -> Option<CustomAmmoSpec> {
    let info = model.ammo_set(set)?;
    let mut types: Vec<CustomAmmoType> = Vec::new();
    for pair in &info.ammo_types {
        let Some(mut t) = type_from_pair(model, &pair.ammo, &pair.projectile, source) else {
            continue;
        };
        let base = if t.key.is_empty() {
            "Type".to_owned()
        } else {
            t.key.clone()
        };
        let mut key = base.clone();
        let mut n = 2;
        while types.iter().any(|o| o.key == key) {
            key = format!("{base}{n}");
            n += 1;
        }
        t.key = key;
        types.push(t);
    }
    Some(CustomAmmoSpec {
        name: identifier(set.rsplit_once("AmmoSet_").map_or(set, |(_, n)| n)),
        caliber: model.ammo.set_label(set).unwrap_or_else(|| set.to_owned()),
        set_label: None,
        similar_to: info.similar_to.clone(),
        category_parent: None,
        category_icon: None,
        default_type: None,
        set_extra: model.ammo.set_extra.get(set).cloned().unwrap_or_default(),
        types,
    })
}

struct Candidate<'a> {
    ammo: &'a AmmoFacts,
    projectile: &'a ProjectileFacts,
    set: Option<&'a str>,
    energy: f64,
}

/// Every ammo item of the install with the projectile it is measured by.
pub(crate) fn ammo_pairs(model: &CeModel) -> Vec<(&AmmoFacts, &ProjectileFacts)> {
    model
        .ammo
        .ammo
        .values()
        .filter_map(|ammo| Some((ammo, projectile_for(model, ammo)?.0)))
        .collect()
}

fn candidates(model: &CeModel) -> Vec<Candidate<'_>> {
    model
        .ammo
        .ammo
        .values()
        .filter_map(|ammo| {
            let (projectile, set) = projectile_for(model, ammo)?;
            Some(Candidate {
                ammo,
                projectile,
                set,
                energy: projectile.energy()?,
            })
        })
        .collect()
}

fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        return None;
    }
    let mid = n / 2;
    if n % 2 == 1 {
        values.get(mid).copied()
    } else {
        Some((values.get(mid.checked_sub(1)?).copied()? + values.get(mid).copied()?) / 2.0)
    }
}

fn rating(values: &[f64], med: f64) -> Reliability {
    if values.len() < 3 {
        return Reliability::Unmeasured;
    }
    let (lo, hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let spread = if med.abs() > f64::EPSILON {
        (hi - lo) / med.abs()
    } else if hi == lo {
        0.0
    } else {
        f64::MAX
    };
    if spread <= AGREE {
        Reliability::Reliable
    } else if spread <= ROUGH {
        Reliability::Rough
    } else {
        Reliability::Unreliable
    }
}

/// Suggests the values of a new ammo type of ammo class `class`.
///
/// With `copy_from` (an ammo def name) the type is a complete copy of that ammunition. Otherwise the
/// nearest ammunition of the class on the energy scale decides: the target energy comes from the hints (the
/// damage and speed given, or the type of `similar_set` of the same class), else it is the class median.
/// With no ammunition of the class the nearest over all classes is used and the ratings say `unmeasured`.
#[must_use]
pub fn suggest_type(
    model: &CeModel,
    class: &str,
    hints: &AmmoHints,
    copy_from: Option<&str>,
) -> AmmoSuggestion {
    let class_label = model.ammo.class_label(class);
    let mut out = AmmoSuggestion {
        available: true,
        reason: None,
        class: class.to_owned(),
        class_label,
        copied_from: None,
        nearest: Vec::new(),
        fields: Vec::new(),
        ammo_type: CustomAmmoType {
            ammo_class: class.to_owned(),
            key: identifier(&model.ammo.class_label(class)),
            ..CustomAmmoType::default()
        },
        notes: Vec::new(),
    };
    if model.ammo.is_empty() || !model.is_present() {
        out.available = false;
        out.reason = Some("no Combat Extended ammunition is loaded".to_owned());
        return out;
    }
    if let Some(from) = copy_from {
        let Some(mut copy) = type_from_existing(model, from, ValueSource::Suggested) else {
            out.available = false;
            out.reason = Some(format!("the ammunition {from} is not loaded"));
            return out;
        };
        if !class.is_empty() && class != copy.ammo_class {
            out.notes.push(format!(
                "the copy keeps the ammo class of {from}; the class asked for was {class}"
            ));
        }
        out.fields = copied_fields(&copy, from);
        // The copy is a starting point for another caliber: what names the source's own caliber (labels,
        // description, its thing category) is left for the user to give.
        let source_label = copy.label.take().unwrap_or_default();
        copy.description = None;
        copy.projectile.label = None;
        copy.item.thing_categories.clear();
        copy.recipe.label = None;
        copy.recipe.description = None;
        copy.recipe.job_string = None;
        out.class.clone_from(&copy.ammo_class);
        out.class_label = model.ammo.class_label(&copy.ammo_class);
        out.copied_from = Some(from.to_owned());
        out.nearest = vec![NearestAmmo {
            ammo_def: from.to_owned(),
            ammo_label: source_label,
            set: None,
            projectile_def: String::new(),
            damage: copy.projectile.damage.map(|d| d.value),
            speed: copy.projectile.speed.map(|d| d.value),
            energy: None,
            distance: 0.0,
        }];
        out.ammo_type = copy;
        return out;
    }
    let all = candidates(model);
    let mut pool: Vec<&Candidate<'_>> = all.iter().filter(|c| c.ammo.ammo_class == class).collect();
    if pool.is_empty() {
        out.notes.push(format!(
            "the install has no ammunition of the class {class}; the nearest ammunition of any class is used"
        ));
        pool = all.iter().collect();
    }
    if pool.is_empty() {
        out.available = false;
        out.reason = Some("no ammunition with a damage and a speed is loaded".to_owned());
        return out;
    }
    let mut speeds: Vec<f64> = pool.iter().filter_map(|c| c.projectile.speed).collect();
    let mut energies: Vec<f64> = pool.iter().map(|c| c.energy).collect();
    let from_set = hints.similar_set.as_deref().and_then(|s| {
        let info = model.ammo_set(s)?;
        let in_class = info.ammo_types.iter().find(|a| {
            model
                .ammo
                .ammo
                .get(&a.ammo)
                .is_some_and(|i| i.ammo_class == class)
        });
        let pair = in_class.or_else(|| info.ammo_types.first())?;
        model.ammo.projectiles.get(&pair.projectile)?.energy()
    });
    let target = from_set
        .or_else(|| match (hints.damage, hints.speed) {
            (Some(d), Some(s)) => Some(d * s),
            (Some(d), None) => median(&mut speeds).map(|s| d * s),
            (None, Some(s)) => {
                let mut dmg: Vec<f64> = pool.iter().filter_map(|c| c.projectile.damage).collect();
                median(&mut dmg).map(|d| d * s)
            }
            (None, None) => None,
        })
        .or_else(|| median(&mut energies))
        .unwrap_or(1.0);
    let mut ranked: Vec<(f64, &Candidate<'_>)> = pool
        .iter()
        .map(|c| ((c.energy / target).ln().abs(), *c))
        .collect();
    ranked.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.ammo.def_name.cmp(&b.1.ammo.def_name))
    });
    let near: Vec<&(f64, &Candidate<'_>)> = ranked.iter().take(NEIGHBOURS).collect();
    out.nearest = near
        .iter()
        .map(|(d, c)| NearestAmmo {
            ammo_def: c.ammo.def_name.clone(),
            ammo_label: c.ammo.label.clone(),
            set: c.set.map(str::to_owned),
            projectile_def: c.projectile.def_name.clone(),
            damage: c.projectile.damage,
            speed: c.projectile.speed,
            energy: Some(c.energy),
            distance: *d,
        })
        .collect();
    let Some(first) = near.first().map(|(_, c)| *c) else {
        return out;
    };
    let base = type_from_pair(
        model,
        &first.ammo.def_name,
        &first.projectile.def_name,
        ValueSource::Suggested,
    );
    let mut t = base.unwrap_or_default();
    // A new type starts from the nearest one's structure but is its own ammunition: no copy mark, no
    // special children of the neighbour, no categories of its caliber.
    t.copied_from = None;
    t.label = None;
    t.description = None;
    t.projectile.label = None;
    t.projectile.extra.clear();
    t.projectile.thing_extra.clear();
    t.item.extra.clear();
    t.item.thing_categories.clear();
    t.recipe.label = None;
    t.recipe.description = None;
    t.recipe.job_string = None;
    t.recipe.extra.clear();
    t.item.graphic_extra.clear();
    t.projectile.graphic_extra.clear();
    t.item.stat_bases.clear();
    t.ammo_class = class.to_owned();
    t.key = identifier(&model.ammo.class_label(class));
    let from: Vec<String> = near.iter().map(|(_, c)| c.ammo.def_name.clone()).collect();
    let class_pool = pool_is_class(&all, class);
    let mut numeric = |field: &str,
                       label: &str,
                       get: &dyn Fn(&Candidate<'_>) -> Option<f64>,
                       set: &mut dyn FnMut(&mut CustomAmmoType, f64)| {
        let mut values: Vec<f64> = near.iter().filter_map(|(_, c)| get(c)).collect();
        if values.len() * 2 <= near.len() {
            return;
        }
        let Some(med) = median(&mut values.clone()) else {
            return;
        };
        let r = if class_pool {
            rating(&values, med)
        } else {
            Reliability::Unmeasured
        };
        let (lo, hi) = values
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        values.sort_by(f64::total_cmp);
        set(&mut t, med);
        out.fields.push(SuggestedAmmoField {
            field: field.to_owned(),
            label: label.to_owned(),
            value: Some(med),
            text: None,
            source: AmmoSourceKind::Nearest,
            rating: r,
            n: values.len(),
            from: from.clone(),
            range: Some((lo, hi)),
        });
    };
    numeric(
        "/projectile/damage",
        "damage",
        &|c| c.projectile.damage,
        &mut |t, v| t.projectile.damage = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/projectile/armorPenetrationSharp",
        "sharp penetration",
        &|c| c.projectile.ap_sharp,
        &mut |t, v| t.projectile.armor_penetration_sharp = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/projectile/armorPenetrationBlunt",
        "blunt penetration",
        &|c| c.projectile.ap_blunt,
        &mut |t, v| t.projectile.armor_penetration_blunt = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/projectile/speed",
        "speed",
        &|c| c.projectile.speed,
        &mut |t, v| t.projectile.speed = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/projectile/pelletCount",
        "pellets",
        &|c| c.projectile.pellets,
        &mut |t, v| t.projectile.pellet_count = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/projectile/spreadMult",
        "spread multiplier",
        &|c| c.projectile.spread_mult,
        &mut |t, v| t.projectile.spread_mult = sourced(v, ValueSource::Suggested),
    );
    numeric("/item/mass", "mass", &|c| c.ammo.mass, &mut |t, v| {
        t.item.mass = sourced(v, ValueSource::Suggested)
    });
    numeric("/item/bulk", "bulk", &|c| c.ammo.bulk, &mut |t, v| {
        t.item.bulk = sourced(v, ValueSource::Suggested)
    });
    numeric(
        "/recipe/workAmount",
        "work amount",
        &|c| {
            model
                .ammo
                .recipes
                .get(&c.ammo.def_name)
                .and_then(|r| r.work_amount)
        },
        &mut |t, v| t.recipe.work_amount = sourced(v, ValueSource::Suggested),
    );
    numeric(
        "/recipe/products",
        "products per craft",
        &|c| {
            model
                .ammo
                .recipes
                .get(&c.ammo.def_name)
                .and_then(|r| r.products)
        },
        &mut |t, v| t.recipe.products = sourced_u32(v, ValueSource::Suggested),
    );
    for (field, label, text) in [
        (
            "/projectile/parent",
            "projectile parent",
            t.projectile.parent.clone(),
        ),
        (
            "/projectile/thingClass",
            "projectile class",
            t.projectile.thing_class.clone(),
        ),
        (
            "/projectile/damageDef",
            "damage def",
            t.projectile.damage_def.clone(),
        ),
        ("/item/parent", "ammo parent", t.item.parent.clone()),
        ("/recipe/parent", "recipe parent", t.recipe.parent.clone()),
        (
            "/recipe/researchPrerequisite",
            "research",
            t.recipe.research_prerequisite.clone(),
        ),
    ] {
        if let Some(text) = text {
            out.fields.push(SuggestedAmmoField {
                field: field.to_owned(),
                label: label.to_owned(),
                value: None,
                text: Some(text),
                source: AmmoSourceKind::Nearest,
                rating: Reliability::Rough,
                n: 1,
                from: vec![first.ammo.def_name.clone()],
                range: None,
            });
        }
    }
    if let Some(d) = hints.damage {
        t.projectile.damage = sourced(d, ValueSource::Answered);
        set_field(&mut out.fields, "/projectile/damage", "damage", d);
    }
    if let Some(s) = hints.speed {
        t.projectile.speed = sourced(s, ValueSource::Answered);
        set_field(&mut out.fields, "/projectile/speed", "speed", s);
    }
    out.ammo_type = t;
    out
}

fn pool_is_class(all: &[Candidate<'_>], class: &str) -> bool {
    all.iter().any(|c| c.ammo.ammo_class == class)
}

fn set_field(fields: &mut Vec<SuggestedAmmoField>, pointer: &str, label: &str, value: f64) {
    fields.retain(|f| f.field != pointer);
    fields.push(SuggestedAmmoField {
        field: pointer.to_owned(),
        label: label.to_owned(),
        value: Some(value),
        text: None,
        source: AmmoSourceKind::Hint,
        rating: Reliability::Reliable,
        n: 0,
        from: Vec::new(),
        range: None,
    });
}

fn copied_fields(t: &CustomAmmoType, from: &str) -> Vec<SuggestedAmmoField> {
    let mut out = Vec::new();
    let mut push = |field: &str, label: &str, value: Option<f64>, text: Option<String>| {
        if value.is_some() || text.is_some() {
            out.push(SuggestedAmmoField {
                field: field.to_owned(),
                label: label.to_owned(),
                value,
                text,
                source: AmmoSourceKind::Copied,
                rating: Reliability::Reliable,
                n: 1,
                from: vec![from.to_owned()],
                range: None,
            });
        }
    };
    let p = &t.projectile;
    push(
        "/projectile/damage",
        "damage",
        p.damage.map(|v| v.value),
        None,
    );
    push(
        "/projectile/armorPenetrationSharp",
        "sharp penetration",
        p.armor_penetration_sharp.map(|v| v.value),
        None,
    );
    push(
        "/projectile/armorPenetrationBlunt",
        "blunt penetration",
        p.armor_penetration_blunt.map(|v| v.value),
        None,
    );
    push("/projectile/speed", "speed", p.speed.map(|v| v.value), None);
    push(
        "/projectile/pelletCount",
        "pellets",
        p.pellet_count.map(|v| v.value),
        None,
    );
    push(
        "/projectile/damageDef",
        "damage def",
        None,
        p.damage_def.clone(),
    );
    push("/item/mass", "mass", t.item.mass.map(|v| v.value), None);
    push("/item/bulk", "bulk", t.item.bulk.map(|v| v.value), None);
    push(
        "/recipe/workAmount",
        "work amount",
        t.recipe.work_amount.map(|v| v.value),
        None,
    );
    out
}
