//! Checks of a custom caliber: required fields, usable numbers, plausibility against the user's own
//! ammunition, duplicate def names, references, and strings that can be written as XML.
//!
//! Plausibility is a measurement, not a limit: a number far outside what the installed ammunition of the
//! same ammo class has is reported with the range found, and never clamped or refused. The checks that need
//! the loaded defs of the game (ingredients, research, damage defs, sounds) go through [`DefLookup`]; an
//! unknown answer produces nothing.

use std::collections::BTreeSet;

use rimstudio_core::diag::Diagnostic;

use super::codes::{
    CALIBER_MISSING, CLASS_MISSING, CLASS_UNKNOWN, DEFAULT_UNKNOWN, DUPLICATE_DEF, FIELD_REQUIRED,
    IMPLAUSIBLE, KEY_DUPLICATE, NAME_INVALID, NAME_MISSING, NO_DATA, NO_PARENT, NO_TYPES,
    NUMBER_INVALID, RECIPE_EMPTY, REF_DOUBTFUL, REF_UNRESOLVED, TEXT_INVALID,
};
use super::generate::default_key;
use super::names::{DerivedNames, identifier, type_key};
use super::suggest::ammo_pairs;
use crate::ce::reader::CeModel;
use crate::model::{CustomAmmoSpec, CustomAmmoType, Sourced, format_number};
use crate::validation::{DefLookup, RefKind};

/// A population smaller than this gives no plausibility finding.
const MIN_POPULATION: usize = 5;
/// A value below this share of the smallest installed value, or above this multiple of the largest, is
/// reported.
const BELOW: f64 = 0.5;
const ABOVE: f64 = 2.0;

fn ptr(index: usize, rest: &str) -> String {
    format!("/ce/customAmmo/types/{index}{rest}")
}

fn is_ref(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn xml_safe(text: &str) -> Option<&'static str> {
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
    {
        Some("it contains a control character")
    } else if text.chars().any(|c| matches!(c, '\u{fffe}' | '\u{ffff}')) {
        Some("it contains a character that XML does not allow")
    } else {
        None
    }
}

struct Check<'a> {
    spec: &'a CustomAmmoSpec,
    model: &'a CeModel,
    out: Vec<Diagnostic>,
}

impl Check<'_> {
    // One check per numeric member: the position (type, index, pointer, field), the value and its bound.
    #[allow(clippy::too_many_arguments)]
    fn number(
        &mut self,
        ty: &str,
        index: usize,
        pointer: &str,
        field: &str,
        n: Option<&Sourced<f64>>,
        min: f64,
        strict: bool,
    ) {
        let Some(n) = n else { return };
        let v = n.value;
        let reason = if !v.is_finite() {
            Some("it is not a finite number")
        } else if (strict && v <= min) || v < min {
            Some(if strict {
                "it must be above the minimum"
            } else {
                "it is below the minimum"
            })
        } else {
            None
        };
        if let Some(r) = reason {
            self.out.push(NUMBER_INVALID.diagnostic(
                &ptr(index, pointer),
                &[
                    ("type", ty),
                    ("what", field),
                    ("value", &format_number(v)),
                    ("reason", &format!("{r} ({})", format_number(min))),
                ],
            ));
        }
    }

    fn text(&mut self, index: usize, pointer: &str, field: &str, text: &str) {
        if let Some(reason) = xml_safe(text) {
            self.out.push(
                TEXT_INVALID
                    .diagnostic(&ptr(index, pointer), &[("what", field), ("reason", reason)]),
            );
        }
    }

    fn reference(&mut self, index: usize, pointer: &str, what: &str, value: &str) {
        if !value.is_empty() && !is_ref(value) {
            self.out.push(TEXT_INVALID.diagnostic(
                &ptr(index, pointer),
                &[
                    ("what", what),
                    (
                        "reason",
                        "it is not a def name (letters, digits, underscore, hyphen and dot)",
                    ),
                ],
            ));
        }
    }
}

fn plausibility(c: &mut Check<'_>, index: usize, t: &CustomAmmoType, label: &str) {
    let pop: Vec<_> = ammo_pairs(c.model)
        .into_iter()
        .filter(|(a, _)| a.ammo_class == t.ammo_class)
        .collect();
    if pop.len() < MIN_POPULATION {
        return;
    }
    let p = &t.projectile;
    let mut measure = |field: &str,
                       pointer: &str,
                       value: Option<f64>,
                       get: &dyn Fn(
        &super::library::AmmoFacts,
        &super::library::ProjectileFacts,
    ) -> Option<f64>| {
        let Some(value) = value else { return };
        let vals: Vec<f64> = pop.iter().filter_map(|(a, pr)| get(a, pr)).collect();
        if vals.len() < MIN_POPULATION {
            return;
        }
        let lo = vals.iter().copied().fold(f64::MAX, f64::min);
        let hi = vals.iter().copied().fold(f64::MIN, f64::max);
        if value < lo * BELOW || value > hi * ABOVE {
            c.out.push(IMPLAUSIBLE.diagnostic(
                &ptr(index, pointer),
                &[
                    ("type", label),
                    ("what", field),
                    ("value", &format_number(value)),
                    ("n", &vals.len().to_string()),
                    ("class", &t.ammo_class),
                    ("low", &format_number(lo)),
                    ("high", &format_number(hi)),
                ],
            ));
        }
    };
    measure(
        "damage",
        "/projectile/damage",
        p.damage.map(|v| v.value),
        &|_, pr| pr.damage,
    );
    measure(
        "sharp penetration",
        "/projectile/armorPenetrationSharp",
        p.armor_penetration_sharp.map(|v| v.value),
        &|_, pr| pr.ap_sharp,
    );
    measure(
        "blunt penetration",
        "/projectile/armorPenetrationBlunt",
        p.armor_penetration_blunt.map(|v| v.value),
        &|_, pr| pr.ap_blunt,
    );
    measure(
        "speed",
        "/projectile/speed",
        p.speed.map(|v| v.value),
        &|_, pr| pr.speed,
    );
    measure(
        "mass",
        "/item/mass",
        t.item.mass.map(|v| v.value),
        &|a, _| a.mass,
    );
    measure(
        "bulk",
        "/item/bulk",
        t.item.bulk.map(|v| v.value),
        &|a, _| a.bulk,
    );
    let ratio = match (p.armor_penetration_sharp, p.damage) {
        (Some(ap), Some(d)) if d.value > 0.0 => Some(ap.value / d.value),
        _ => None,
    };
    measure(
        "penetration per damage",
        "/projectile/armorPenetrationSharp",
        ratio,
        &|_, pr| match (pr.ap_sharp, pr.damage) {
            (Some(ap), Some(d)) if d > 0.0 => Some(ap / d),
            _ => None,
        },
    );
}

fn type_checks(c: &mut Check<'_>, index: usize, t: &CustomAmmoType) {
    let key = type_key(t);
    let label = if key.is_empty() {
        format!("#{}", index + 1)
    } else {
        key.clone()
    };
    if t.ammo_class.trim().is_empty() {
        c.out
            .push(CLASS_MISSING.diagnostic(&ptr(index, "/ammoClass"), &[("type", &label)]));
    } else {
        c.reference(index, "/ammoClass", "the ammo class", t.ammo_class.trim());
        let lib = &c.model.ammo;
        if !lib.classes.is_empty() && lib.class(t.ammo_class.trim()).is_none() {
            c.out.push(CLASS_UNKNOWN.diagnostic(
                &ptr(index, "/ammoClass"),
                &[("type", &label), ("class", &t.ammo_class)],
            ));
        }
    }
    if key.is_empty() {
        c.out.push(FIELD_REQUIRED.diagnostic(
            &ptr(index, "/key"),
            &[("type", &label), ("what", "a key or an ammo class")],
        ));
    }
    for (what, text, pointer) in [
        ("label", t.label.as_deref(), "/label"),
        ("description", t.description.as_deref(), "/description"),
        (
            "projectile label",
            t.projectile.label.as_deref(),
            "/projectile/label",
        ),
    ] {
        if let Some(text) = text {
            c.text(index, pointer, what, text);
        }
    }
    let p = &t.projectile;
    let explosive = p.explosion_radius.is_some();
    for (field, pointer, value) in [
        ("damage", "/projectile/damage", p.damage.is_some()),
        (
            "sharp armor penetration",
            "/projectile/armorPenetrationSharp",
            p.armor_penetration_sharp.is_some(),
        ),
        (
            "blunt armor penetration",
            "/projectile/armorPenetrationBlunt",
            p.armor_penetration_blunt.is_some(),
        ),
        ("speed", "/projectile/speed", p.speed.is_some()),
    ] {
        if !value && !explosive {
            c.out.push(
                FIELD_REQUIRED
                    .diagnostic(&ptr(index, pointer), &[("type", &label), ("what", field)]),
            );
        }
    }
    c.number(
        &label,
        index,
        "/projectile/damage",
        "damage",
        p.damage.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/armorPenetrationSharp",
        "sharp armor penetration",
        p.armor_penetration_sharp.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/armorPenetrationBlunt",
        "blunt armor penetration",
        p.armor_penetration_blunt.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/speed",
        "speed",
        p.speed.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/pelletCount",
        "pellet count",
        p.pellet_count.as_ref(),
        1.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/spreadMult",
        "spread multiplier",
        p.spread_mult.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/explosionRadius",
        "explosion radius",
        p.explosion_radius.as_ref(),
        0.0,
        true,
    );
    c.number(
        &label,
        index,
        "/projectile/suppressionFactor",
        "suppression factor",
        p.suppression_factor.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/projectile/dangerFactor",
        "danger factor",
        p.danger_factor.as_ref(),
        0.0,
        false,
    );
    for (i, s) in p.secondary_damage.iter().enumerate() {
        c.number(
            &label,
            index,
            &format!("/projectile/secondaryDamage/{i}/amount"),
            "secondary damage",
            s.amount.as_ref(),
            0.0,
            false,
        );
        c.reference(
            index,
            &format!("/projectile/secondaryDamage/{i}/def"),
            "the secondary damage def",
            s.def.trim(),
        );
    }
    if let Some(n) = p.pellet_count
        && n.value.is_finite()
        && n.value.fract() != 0.0
    {
        c.out.push(NUMBER_INVALID.diagnostic(
            &ptr(index, "/projectile/pelletCount"),
            &[
                ("type", &label),
                ("what", "pellet count"),
                ("value", &format_number(n.value)),
                ("reason", "it must be a whole number"),
            ],
        ));
    }
    for (what, value, pointer) in [
        (
            "the damage def",
            p.damage_def.as_deref(),
            "/projectile/damageDef",
        ),
        (
            "the projectile parent",
            p.parent.as_deref(),
            "/projectile/parent",
        ),
        (
            "the projectile class",
            p.thing_class.as_deref(),
            "/projectile/thingClass",
        ),
        (
            "the casing mote",
            p.casing_mote.as_deref(),
            "/projectile/casingMote",
        ),
        (
            "the casing filth",
            p.casing_filth.as_deref(),
            "/projectile/casingFilth",
        ),
        ("the ammo parent", t.item.parent.as_deref(), "/item/parent"),
        (
            "the recipe parent",
            t.recipe.parent.as_deref(),
            "/recipe/parent",
        ),
    ] {
        if let Some(v) = value {
            c.reference(index, pointer, what, v.trim());
        }
    }
    if t.item.parent.as_deref().is_none_or(|p| p.trim().is_empty()) {
        c.out.push(NO_PARENT.diagnostic(
            &ptr(index, "/item/parent"),
            &[("type", &label), ("what", "ammo item")],
        ));
    }
    if p.parent.as_deref().is_none_or(|p| p.trim().is_empty()) {
        c.out.push(NO_PARENT.diagnostic(
            &ptr(index, "/projectile/parent"),
            &[("type", &label), ("what", "projectile")],
        ));
    }
    for f in &p.fragments {
        c.reference(
            index,
            "/projectile/fragments",
            "the fragment def",
            f.def.trim(),
        );
    }
    let item = &t.item;
    c.number(
        &label,
        index,
        "/item/mass",
        "mass",
        item.mass.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/item/bulk",
        "bulk",
        item.bulk.as_ref(),
        0.0,
        false,
    );
    c.number(
        &label,
        index,
        "/item/marketValue",
        "market value",
        item.market_value.as_ref(),
        0.0,
        false,
    );
    if let Some(s) = item.stack_limit
        && s.value == 0
    {
        c.out.push(NUMBER_INVALID.diagnostic(
            &ptr(index, "/item/stackLimit"),
            &[
                ("type", &label),
                ("what", "stack limit"),
                ("value", "0"),
                ("reason", "it must be at least 1"),
            ],
        ));
    }
    for (i, cat) in item.thing_categories.iter().enumerate() {
        c.reference(
            index,
            &format!("/item/thingCategories/{i}"),
            "the thing category",
            cat.trim(),
        );
    }
    for (i, tag) in item.trade_tags.iter().enumerate() {
        c.reference(
            index,
            &format!("/item/tradeTags/{i}"),
            "the trade tag",
            tag.trim(),
        );
    }
    let r = &t.recipe;
    if r.ingredients.is_empty() {
        c.out
            .push(RECIPE_EMPTY.diagnostic(&ptr(index, "/recipe/ingredients"), &[("type", &label)]));
    }
    for (i, ing) in r.ingredients.iter().enumerate() {
        c.reference(
            index,
            &format!("/recipe/ingredients/{i}/thing"),
            "the ingredient",
            ing.thing.trim(),
        );
        if ing.thing.trim().is_empty() && ing.categories.is_empty() {
            c.out.push(FIELD_REQUIRED.diagnostic(
                &ptr(index, &format!("/recipe/ingredients/{i}/thing")),
                &[("type", &label), ("what", "the ingredient")],
            ));
        }
        match &ing.count {
            None => c.out.push(FIELD_REQUIRED.diagnostic(
                &ptr(index, &format!("/recipe/ingredients/{i}/count")),
                &[("type", &label), ("what", "the ingredient count")],
            )),
            Some(_) => c.number(
                &label,
                index,
                &format!("/recipe/ingredients/{i}/count"),
                "ingredient count",
                ing.count.as_ref(),
                0.0,
                true,
            ),
        }
    }
    c.number(
        &label,
        index,
        "/recipe/workAmount",
        "work amount",
        r.work_amount.as_ref(),
        0.0,
        true,
    );
    if let Some(p) = r.products
        && p.value == 0
    {
        c.out.push(NUMBER_INVALID.diagnostic(
            &ptr(index, "/recipe/products"),
            &[
                ("type", &label),
                ("what", "products per craft"),
                ("value", "0"),
                ("reason", "it must be at least 1"),
            ],
        ));
    }
    for (i, u) in r.users.iter().enumerate() {
        c.reference(
            index,
            &format!("/recipe/users/{i}"),
            "the workbench",
            u.trim(),
        );
    }
    plausibility(c, index, t, &label);
}

/// The checks that need no def lookup: the spec on its own and against the installed ammunition. `prefix` is
/// the project prefix without the trailing underscore.
#[must_use]
pub fn validate_custom_ammo(
    spec: &CustomAmmoSpec,
    prefix: &str,
    model: &CeModel,
) -> Vec<Diagnostic> {
    let mut c = Check {
        spec,
        model,
        out: Vec::new(),
    };
    let base = "/ce/customAmmo";
    if spec.name.trim().is_empty() {
        c.out
            .push(NAME_MISSING.diagnostic(&format!("{base}/name"), &[]));
    } else if identifier(&spec.name) != spec.name.trim() {
        c.out.push(NAME_INVALID.diagnostic(
            &format!("{base}/name"),
            &[
                ("name", spec.name.trim()),
                ("reason", "use letters, digits and underscores only"),
            ],
        ));
    }
    if spec.caliber.trim().is_empty() {
        c.out
            .push(CALIBER_MISSING.diagnostic(&format!("{base}/caliber"), &[]));
    }
    for (what, text) in [
        ("the caliber", Some(spec.caliber.as_str())),
        ("the set label", spec.set_label.as_deref()),
    ] {
        if let Some(reason) = text.and_then(xml_safe) {
            c.out
                .push(TEXT_INVALID.diagnostic(base, &[("what", what), ("reason", reason)]));
        }
    }
    if spec.types.is_empty() {
        c.out
            .push(NO_TYPES.diagnostic(&format!("{base}/types"), &[]));
    }
    if model.ammo.is_empty() {
        c.out.push(NO_DATA.diagnostic(base, &[]));
    }
    let mut keys = BTreeSet::new();
    for (i, t) in spec.types.iter().enumerate() {
        let key = type_key(t);
        if !key.is_empty() && !keys.insert(key.clone()) {
            c.out
                .push(KEY_DUPLICATE.diagnostic(&ptr(i, "/key"), &[("key", &key)]));
        }
        type_checks(&mut c, i, t);
    }
    if let Some(d) = spec
        .default_type
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        && default_key(spec).as_deref() != Some(&identifier(d))
    {
        c.out
            .push(DEFAULT_UNKNOWN.diagnostic(&format!("{base}/defaultType"), &[("key", d)]));
    }
    if let Some(sim) = spec
        .similar_to
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && model.ammo_set(sim).is_none()
    {
        c.out.push(REF_UNRESOLVED.diagnostic(
            &format!("{base}/similarTo"),
            &[("what", "the ammo set it is similar to"), ("value", sim)],
        ));
    }
    if let Some(parent) = spec
        .category_parent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && !model.ammo.categories.is_empty()
        && !model.ammo.categories.contains_key(parent)
    {
        c.out.push(REF_DOUBTFUL.diagnostic(
            &format!("{base}/categoryParent"),
            &[("what", "the parent thing category"), ("value", parent)],
        ));
    }
    c.out
        .extend(duplicate_defs(spec, prefix, &installed_names(model)));
    let _ = c.spec;
    c.out
}

/// Every def name of the installed ammunition data: sets, items, projectiles, recipes and categories.
#[must_use]
pub fn installed_names(model: &CeModel) -> BTreeSet<String> {
    let lib = &model.ammo;
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(model.ammo_sets.iter().map(|s| s.def_name.clone()));
    names.extend(lib.ammo.keys().cloned());
    names.extend(lib.projectiles.keys().cloned());
    names.extend(lib.recipes.values().map(|r| r.def_name.clone()));
    names.extend(lib.categories.keys().cloned());
    names
}

/// The derived def names that are already in `existing` (the installed ammunition, the project's own defs).
#[must_use]
pub fn duplicate_defs(
    spec: &CustomAmmoSpec,
    prefix: &str,
    existing: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    if spec.name.trim().is_empty() {
        return Vec::new();
    }
    let names = DerivedNames::new(spec, prefix);
    let mut derived = vec![names.set()];
    if spec
        .category_parent
        .as_deref()
        .is_some_and(|p| !p.trim().is_empty())
    {
        derived.push(names.category());
    }
    for t in &spec.types {
        let key = type_key(t);
        derived.extend([names.ammo(&key), names.projectile(&key), names.recipe(&key)]);
    }
    derived
        .into_iter()
        .filter(|n| existing.contains(n))
        .map(|n| DUPLICATE_DEF.diagnostic("/ce/customAmmo/name", &[("name", &n)]))
        .collect()
}

/// The checks against the loaded defs of the game and the project: ingredients, workbenches, research,
/// damage defs and sounds must exist, and the derived ammo item and projectile names must not (a thing def
/// of that name is already loaded or in the project). Unknown answers produce nothing.
#[must_use]
pub fn validate_custom_ammo_refs(
    spec: &CustomAmmoSpec,
    prefix: &str,
    lookup: &dyn DefLookup,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !spec.name.trim().is_empty() {
        let names = DerivedNames::new(spec, prefix);
        for t in &spec.types {
            let key = type_key(t);
            for name in [names.ammo(&key), names.projectile(&key)] {
                if lookup.contains(RefKind::Thing, &name) == Some(true) {
                    out.push(DUPLICATE_DEF.diagnostic("/ce/customAmmo/name", &[("name", &name)]));
                }
            }
        }
    }
    let mut check = |kind: RefKind, pointer: String, what: &str, value: &str| {
        let value = value.trim();
        if !value.is_empty() && lookup.contains(kind, value) == Some(false) {
            out.push(REF_UNRESOLVED.diagnostic(&pointer, &[("what", what), ("value", value)]));
        }
    };
    for (i, t) in spec.types.iter().enumerate() {
        for (j, ing) in t.recipe.ingredients.iter().enumerate() {
            check(
                RefKind::Thing,
                ptr(i, &format!("/recipe/ingredients/{j}/thing")),
                "the ingredient",
                &ing.thing,
            );
        }
        for (j, u) in t.recipe.users.iter().enumerate() {
            check(
                RefKind::Thing,
                ptr(i, &format!("/recipe/users/{j}")),
                "the workbench",
                u,
            );
        }
        if let Some(r) = &t.recipe.research_prerequisite {
            check(
                RefKind::ResearchProject,
                ptr(i, "/recipe/researchPrerequisite"),
                "the research project",
                r,
            );
        }
        if let Some(d) = &t.projectile.damage_def {
            check(
                RefKind::DamageDef,
                ptr(i, "/projectile/damageDef"),
                "the damage def",
                d,
            );
        }
        for (j, s) in t.projectile.secondary_damage.iter().enumerate() {
            check(
                RefKind::DamageDef,
                ptr(i, &format!("/projectile/secondaryDamage/{j}/def")),
                "the secondary damage def",
                &s.def,
            );
        }
        for (field, sound) in [
            ("/projectile/soundExplode", &t.projectile.sound_explode),
            ("/projectile/soundAmbient", &t.projectile.sound_ambient),
            (
                "/projectile/soundHitThickRoof",
                &t.projectile.sound_hit_thick_roof,
            ),
            (
                "/projectile/soundImpactAnticipate",
                &t.projectile.sound_impact_anticipate,
            ),
        ] {
            if let Some(s) = sound {
                check(RefKind::SoundDef, ptr(i, field), "the sound", s);
            }
        }
    }
    out
}
