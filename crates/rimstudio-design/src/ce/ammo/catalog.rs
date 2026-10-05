//! The catalogue of the ammo sets of the user's Combat Extended: all of them, searchable and paged.
//!
//! Every value is read from the loaded data ([`CeModel`]): the ammo types of a set with their projectile
//! numbers, the caliber group, whether the set is a generic one or similar to one, and how many converted
//! weapons use it. The relevance ranking of the current design is given in by the caller
//! (`suggested`: set name to score), so the catalogue marks the sets the designer would suggest.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ce::reader::{AmmoSetInfo, CeModel};

/// The default number of entries of a page.
pub const DEFAULT_PAGE_SIZE: usize = 25;
/// The largest page the catalogue returns.
pub const MAX_PAGE_SIZE: usize = 200;
/// How many example weapons an entry names.
const EXAMPLES: usize = 3;

/// What the catalogue is asked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogQuery {
    /// Words that must all appear in a set (its names, labels, caliber, ammo, classes, example weapons).
    pub query: Option<String>,
    /// A caliber or family, as the facets name them (compared ignoring case).
    pub caliber: Option<String>,
    /// An ammo class def name: only sets with a type of this class.
    pub class: Option<String>,
    /// The page, counted from zero.
    pub page: usize,
    /// Entries per page; the default is [`DEFAULT_PAGE_SIZE`].
    pub page_size: Option<usize>,
}

/// A secondary damage entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSecondary {
    /// The damage def.
    pub def: String,
    /// The amount.
    pub amount: f64,
}

/// One ammo type of a set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogType {
    /// The ammo item def name.
    pub ammo_def: String,
    /// The label of the ammo item.
    pub ammo_label: String,
    /// The ammo class def name.
    pub ammo_class: String,
    /// The label of the ammo class.
    pub ammo_class_label: String,
    /// The projectile def name.
    pub projectile_def: String,
    /// The damage def of the projectile.
    pub damage_def: Option<String>,
    /// Damage per projectile.
    pub damage: Option<f64>,
    /// Sharp penetration.
    pub armor_penetration_sharp: Option<f64>,
    /// Blunt penetration.
    pub armor_penetration_blunt: Option<f64>,
    /// Speed in cells per second.
    pub speed: Option<f64>,
    /// Pellets per shot.
    pub pellets: Option<f64>,
    /// Explosion radius.
    pub explosion_radius: Option<f64>,
    /// Secondary damage entries.
    pub secondary_damage: Vec<CatalogSecondary>,
}

/// One ammo set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// The def name of the set.
    pub def_name: String,
    /// The label of the set.
    pub label: String,
    /// The caliber group: the label of the thing category of its ammunition, else the label of the set.
    pub caliber: String,
    /// The family of the caliber (pistol, rifle, shotgun, ...), when the categories say.
    pub family: Option<String>,
    /// The set this one is similar to.
    pub similar_to: Option<String>,
    /// True when other sets are similar to this one (a generic set).
    pub generic: bool,
    /// How many sets are similar to this one.
    pub similar_sets: usize,
    /// The ammo types.
    pub types: Vec<CatalogType>,
    /// How many converted weapons use the set.
    pub weapon_count: usize,
    /// A few labels of those weapons.
    pub example_weapons: Vec<String>,
    /// True when the relevance ranking of the current design suggests the set.
    pub suggested: bool,
    /// The relevance score, when suggested.
    pub score: Option<f64>,
}

/// A value of a facet with its count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFacet {
    /// The value to send back as the filter (a caliber, a family or an ammo class def name).
    pub name: String,
    /// What to show.
    pub label: String,
    /// How many sets (of the sets matching the search words) carry it.
    pub count: usize,
}

/// One page of the catalogue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    /// Sets in the install.
    pub total: usize,
    /// Sets that match the query and the filters.
    pub matching: usize,
    /// The page returned, counted from zero.
    pub page: usize,
    /// Entries per page.
    pub page_size: usize,
    /// The entries of the page.
    pub entries: Vec<CatalogEntry>,
    /// The calibers and families of the sets that match the search words.
    pub calibers: Vec<CatalogFacet>,
    /// The ammo classes of the sets that match the search words.
    pub classes: Vec<CatalogFacet>,
}

fn catalog_type(model: &CeModel, a: &crate::ce::reader::AmmoType) -> CatalogType {
    let lib = &model.ammo;
    let item = lib.ammo.get(&a.ammo);
    let facts = lib.projectiles.get(&a.projectile);
    let ammo_class = item.map(|i| i.ammo_class.clone()).unwrap_or_default();
    CatalogType {
        ammo_def: a.ammo.clone(),
        ammo_label: item.map(|i| i.label.clone()).unwrap_or_default(),
        ammo_class_label: if ammo_class.is_empty() {
            String::new()
        } else {
            lib.class_label(&ammo_class)
        },
        ammo_class,
        projectile_def: a.projectile.clone(),
        damage_def: facts.and_then(|f| f.damage_def.clone()),
        damage: a.info.damage,
        armor_penetration_sharp: a.info.ap_sharp,
        armor_penetration_blunt: a.info.ap_blunt,
        speed: a.info.speed,
        pellets: a.info.pellets,
        explosion_radius: a.info.explosion_radius,
        secondary_damage: facts
            .map(|f| {
                f.secondary
                    .iter()
                    .map(|(def, amount)| CatalogSecondary {
                        def: def.clone(),
                        amount: *amount,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn entry_of(
    model: &CeModel,
    set: &AmmoSetInfo,
    referenced: &BTreeMap<&str, usize>,
    suggested: &BTreeMap<String, f64>,
) -> CatalogEntry {
    let label = model
        .ammo
        .set_label(&set.def_name)
        .unwrap_or_else(|| set.def_name.clone());
    let (caliber, family) = set
        .ammo_types
        .iter()
        .find_map(|a| model.ammo.caliber_of(&a.ammo))
        .unwrap_or_else(|| (label.clone(), None));
    let mut users: Vec<&str> = model
        .guns
        .iter()
        .filter(|g| g.ammo_set.as_deref() == Some(set.def_name.as_str()))
        .map(|g| g.label.as_str())
        .collect();
    users.sort_unstable();
    let weapon_count = users.len();
    let score = suggested.get(&set.def_name).copied();
    CatalogEntry {
        def_name: set.def_name.clone(),
        label,
        caliber,
        family,
        similar_to: set.similar_to.clone(),
        generic: referenced.get(set.def_name.as_str()).copied().unwrap_or(0) > 0,
        similar_sets: referenced.get(set.def_name.as_str()).copied().unwrap_or(0),
        types: set
            .ammo_types
            .iter()
            .map(|a| catalog_type(model, a))
            .collect(),
        weapon_count,
        example_weapons: users
            .into_iter()
            .take(EXAMPLES)
            .map(str::to_owned)
            .collect(),
        suggested: score.is_some(),
        score,
    }
}

fn words(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
}

fn haystack(e: &CatalogEntry, model: &CeModel) -> String {
    let mut h = format!(
        "{} {} {} {}",
        e.def_name,
        e.label,
        e.caliber,
        e.family.as_deref().unwrap_or("")
    );
    for t in &e.types {
        h.push(' ');
        h.push_str(&t.ammo_def);
        h.push(' ');
        h.push_str(&t.ammo_label);
        h.push(' ');
        h.push_str(&t.ammo_class_label);
        h.push(' ');
        if let Some(c) = model.ammo.class(&t.ammo_class) {
            h.push_str(&c.label);
            h.push(' ');
        }
        h.push_str(&t.projectile_def);
    }
    for w in &e.example_weapons {
        h.push(' ');
        h.push_str(w);
    }
    h.to_lowercase()
}

fn facets(entries: &[&CatalogEntry], model: &CeModel) -> (Vec<CatalogFacet>, Vec<CatalogFacet>) {
    let mut calibers: BTreeMap<String, usize> = BTreeMap::new();
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries {
        let mut seen = BTreeSet::new();
        for name in std::iter::once(&e.caliber).chain(e.family.iter()) {
            if seen.insert(name.to_lowercase()) {
                *calibers.entry(name.clone()).or_insert(0) += 1;
            }
        }
        let mut seen = BTreeSet::new();
        for t in &e.types {
            if !t.ammo_class.is_empty() && seen.insert(t.ammo_class.clone()) {
                *classes.entry(t.ammo_class.clone()).or_insert(0) += 1;
            }
        }
    }
    let calibers = calibers
        .into_iter()
        .map(|(name, count)| CatalogFacet {
            label: name.clone(),
            name,
            count,
        })
        .collect();
    let classes = classes
        .into_iter()
        .map(|(name, count)| CatalogFacet {
            label: model.ammo.class_label(&name),
            name,
            count,
        })
        .collect();
    (calibers, classes)
}

/// Answers a catalogue query. The result is deterministic: suggested sets first (best score first), then by
/// caliber, then by def name. A page past the end is empty.
#[must_use]
pub fn catalog(
    model: &CeModel,
    query: &CatalogQuery,
    suggested: &BTreeMap<String, f64>,
) -> CatalogPage {
    let mut referenced: BTreeMap<&str, usize> = BTreeMap::new();
    for s in &model.ammo_sets {
        if let Some(target) = s.similar_to.as_deref() {
            *referenced.entry(target).or_insert(0) += 1;
        }
    }
    let mut all: Vec<CatalogEntry> = model
        .ammo_sets
        .iter()
        .map(|s| entry_of(model, s, &referenced, suggested))
        .collect();
    all.sort_by(|a, b| {
        b.score
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&a.score.unwrap_or(f64::NEG_INFINITY))
            .then_with(|| a.caliber.to_lowercase().cmp(&b.caliber.to_lowercase()))
            .then_with(|| a.def_name.cmp(&b.def_name))
    });
    let total = all.len();
    let wanted = query.query.as_deref().map(words).unwrap_or_default();
    let by_words: Vec<&CatalogEntry> = all
        .iter()
        .filter(|e| {
            let h = haystack(e, model);
            wanted.iter().all(|w| h.contains(w))
        })
        .collect();
    let (calibers, classes) = facets(&by_words, model);
    let caliber = query
        .caliber
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_lowercase);
    let class = query
        .class
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_lowercase);
    let matching: Vec<&CatalogEntry> = by_words
        .into_iter()
        .filter(|e| {
            caliber.as_ref().is_none_or(|c| {
                e.caliber.to_lowercase() == *c
                    || e.family.as_ref().is_some_and(|f| f.to_lowercase() == *c)
            })
        })
        .filter(|e| {
            class
                .as_ref()
                .is_none_or(|c| e.types.iter().any(|t| t.ammo_class.to_lowercase() == *c))
        })
        .collect();
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let entries = matching
        .iter()
        .skip(query.page.saturating_mul(page_size))
        .take(page_size)
        .map(|e| (*e).clone())
        .collect();
    CatalogPage {
        total,
        matching: matching.len(),
        page: query.page,
        page_size,
        entries,
        calibers,
        classes,
    }
}
