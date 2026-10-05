//! Custom Combat Extended ammunition: `designer_ce_ammo_catalog` lists every ammo set of the user's install,
//! `designer_ce_ammo_suggest` fills a new ammo type from the nearest of the user's own ammunition (or copies
//! an existing type), and [`ref_diagnostics`] checks the custom ammo of a draft against the loaded defs.
//!
//! Both queries are read only and work for a draft whose Combat Extended switch is off: the catalogue and
//! the suggestions describe what the install offers and never turn the switch on (D-085). Without Combat
//! Extended data the answers say so plainly. The files of a custom caliber are planned with the weapon (see
//! the Combat Extended plan); nothing is written here.

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::Diagnostic;
use rimstudio_design::ce::ammo::{
    AmmoHints, CatalogQuery, DerivedNames, catalog, suggest_type, type_key,
    validate_custom_ammo_refs,
};
use rimstudio_design::ce::suggest::Candidate;
use rimstudio_design::model::{CustomAmmoSpec, DesignSpec};
use rimstudio_design::validation::DefLookup;
use rimstudio_ipc_types::designer::{
    CeAmmoCatalogDto, CeAmmoSuggestionDto, DesignerCeAmmoCatalogRequest,
    DesignerCeAmmoSuggestRequest,
};

use super::ce::suggestion_for;
use super::ctx::Ctx;
use super::dto::draft_from_dto;
use crate::error::{ToolkitError, ToolkitResult};

/// The field of the Combat Extended suggestion whose candidates rank the ammo sets for a design.
const AMMO_SET_FIELD: &str = "/ce/ammoSet";
/// The reason shown when no Combat Extended data is loaded.
const NO_CE: &str = "no Combat Extended data is loaded";

fn through_json<A, B>(a: &A) -> ToolkitResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(a).map_err(|e| ToolkitError::Internal {
        what: format!("an ammunition answer cannot be encoded: {e}"),
    })?;
    serde_json::from_value(value).map_err(|e| ToolkitError::Internal {
        what: format!("an ammunition answer has the wrong shape: {e}"),
    })
}

/// The sets the relevance ranking of the design suggests, with their scores.
fn suggested_sets(ctx: &Ctx, spec: &DesignSpec) -> BTreeMap<String, f64> {
    let suggestion = suggestion_for(ctx, spec);
    suggestion
        .choices
        .iter()
        .filter(|c| c.field == AMMO_SET_FIELD)
        .flat_map(|c| c.candidates.iter())
        .filter(|c: &&Candidate| c.used_by > 0)
        .map(|c| (c.name.clone(), c.score))
        .collect()
}

/// `designer_ce_ammo_catalog`: every ammo set of the install, searchable, filterable and paged.
///
/// With a draft in the request the sets that the relevance ranking of that design suggests are marked.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a draft that cannot be read.
pub fn ce_ammo_catalog(
    ctx: &Ctx,
    req: DesignerCeAmmoCatalogRequest,
) -> ToolkitResult<CeAmmoCatalogDto> {
    let suggested = match &req.draft {
        Some(d) => Some(draft_from_dto(d)?.spec),
        None => None,
    };
    let Some(engine) = ctx.engine().filter(|e| e.ce_available()) else {
        return Ok(CeAmmoCatalogDto {
            reason: Some(NO_CE.to_owned()),
            ..CeAmmoCatalogDto::default()
        });
    };
    let ranking = suggested
        .as_ref()
        .map(|spec| suggested_sets(ctx, spec))
        .unwrap_or_default();
    let query = CatalogQuery {
        query: req.query,
        caliber: req.caliber,
        class: req.class,
        page: req.page.map_or(0, |p| p as usize),
        page_size: req.page_size.map(|p| p as usize),
    };
    let page = catalog(engine.ce(), &query, &ranking);
    let mut dto: CeAmmoCatalogDto = through_json(&page)?;
    dto.available = true;
    Ok(dto)
}

/// `designer_ce_ammo_suggest`: defaults for a new ammo type of a class, from the nearest of the user's own
/// ammunition, or a complete copy of an existing ammo type.
///
/// # Errors
///
/// [`ToolkitError::Internal`] when the shapes disagree (the tests keep that from happening).
pub fn ce_ammo_suggest(
    ctx: &Ctx,
    req: DesignerCeAmmoSuggestRequest,
) -> ToolkitResult<CeAmmoSuggestionDto> {
    let engine = ctx.engine();
    let absent = rimstudio_design::ce::reader::CeModel::absent(NO_CE);
    let model = match engine.as_deref() {
        Some(e) if e.ce_available() => e.ce(),
        _ => &absent,
    };
    let hints = AmmoHints {
        caliber: req.hints.caliber,
        damage: req.hints.damage,
        speed: req.hints.speed,
        similar_set: req.hints.similar_set,
    };
    let suggestion = suggest_type(model, &req.class, &hints, req.copy_from.as_deref());
    through_json(&suggestion)
}

/// The checks of the custom ammo of a spec against the loaded defs and the project: ingredients, workbenches,
/// research, damage defs and sounds must exist, and the derived def names must be free.
#[must_use]
pub fn ref_diagnostics(
    spec: &DesignSpec,
    custom: &CustomAmmoSpec,
    lookup: &dyn DefLookup,
) -> Vec<Diagnostic> {
    validate_custom_ammo_refs(custom, &spec.identity.mod_prefix, lookup)
}

/// The def names of the custom ammunition of a spec (the ammo set, the ammo items and the projectiles): the
/// names the Combat Extended lint may treat as defined besides the installed data, because the plan writes
/// them.
#[must_use]
pub fn custom_known_defs(spec: &DesignSpec) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    if let Some(custom) = spec.ce.as_ref().and_then(|c| c.custom_ammo.as_ref()) {
        let names = DerivedNames::new(custom, &spec.identity.mod_prefix);
        known.insert(names.set());
        for t in &custom.types {
            let key = type_key(t);
            known.insert(names.ammo(&key));
            known.insert(names.projectile(&key));
        }
    }
    known
}

#[cfg(test)]
mod tests {
    use rimstudio_core::tree::Node;
    use rimstudio_design::ce::ammo::{
        AmmoSourceKind, AmmoSuggestion, CatalogEntry, CatalogFacet, CatalogPage, CatalogSecondary,
        CatalogType, NearestAmmo, SuggestedAmmoField,
    };
    use rimstudio_design::ce::classes::Reliability;
    use rimstudio_design::model::{
        CookOffKind, CustomAmmoItem, CustomAmmoRecipe, CustomAmmoType, CustomFragment,
        CustomIngredient, CustomProjectile, CustomSecondaryDamage, ValueSource,
    };
    use rimstudio_ipc_types::designer::CustomAmmoDto;

    use super::*;

    fn s(v: f64) -> Option<rimstudio_design::model::Sourced<f64>> {
        Some(rimstudio_design::model::Sourced::new(
            v,
            ValueSource::Answered,
        ))
    }

    fn node() -> Node {
        let mut n = Node::new("extra");
        n.set_attr("Inherit", "False");
        n.push_child(Node::with_text("child", "text"));
        n
    }

    fn full_type() -> CustomAmmoType {
        CustomAmmoType {
            key: "FMJ".into(),
            ammo_class: "RS_FMJ".into(),
            label: Some("label".into()),
            description: Some("description".into()),
            copied_from: Some("RS_Ammo".into()),
            projectile: CustomProjectile {
                label: Some("p".into()),
                parent: Some("RS_Base".into()),
                thing_class: Some("RS_Class".into()),
                damage_def: Some("RS_Damage".into()),
                damage: s(1.0),
                armor_penetration_sharp: s(2.0),
                armor_penetration_blunt: s(3.0),
                speed: s(4.0),
                pellet_count: s(5.0),
                spread_mult: s(6.0),
                secondary_damage: vec![CustomSecondaryDamage {
                    def: "RS_Bomb".into(),
                    amount: s(7.0),
                }],
                explosion_radius: s(8.0),
                explosion_neighbors: Some(true),
                suppression_factor: s(9.0),
                danger_factor: s(10.0),
                drops_casings: Some(true),
                casing_mote: Some("m".into()),
                casing_filth: Some("f".into()),
                incendiary: Some(false),
                fly_overhead: Some(true),
                tex_path: Some("t".into()),
                graphic_class: Some("g".into()),
                draw_size: Some("(1,1)".into()),
                graphic_extra: vec![node()],
                sound_explode: Some("a".into()),
                sound_ambient: Some("b".into()),
                sound_hit_thick_roof: Some("c".into()),
                sound_impact_anticipate: Some("d".into()),
                fragments: vec![CustomFragment {
                    def: "RS_Frag".into(),
                    count: 3,
                }],
                extra: vec![node()],
                thing_extra: vec![node()],
            },
            item: CustomAmmoItem {
                parent: Some("RS_AmmoBase".into()),
                mass: s(0.1),
                bulk: s(0.2),
                market_value: s(0.3),
                stack_limit: Some(rimstudio_design::model::Sourced::typed(500)),
                thing_categories: vec!["RS_Cat".into()],
                trade_tags: vec!["RS_Tag".into()],
                tex_path: Some("t".into()),
                graphic_class: Some("g".into()),
                draw_size: Some("(2,2)".into()),
                graphic_extra: vec![node()],
                stat_bases: [(
                    "RS_Stat".to_owned(),
                    rimstudio_design::model::Sourced::typed(1.5),
                )]
                .into(),
                tech_level: Some("Industrial".into()),
                cook_off: Some(CookOffKind::Detonate),
                extra: vec![node()],
            },
            recipe: CustomAmmoRecipe {
                parent: Some("RS_Recipe".into()),
                label: Some("l".into()),
                description: Some("d".into()),
                job_string: Some("j".into()),
                ingredients: vec![CustomIngredient {
                    thing: "RS_Steel".into(),
                    count: s(12.0),
                    alternatives: vec!["RS_Iron".into()],
                    categories: vec!["RS_Metals".into()],
                }],
                products: Some(rimstudio_design::model::Sourced::suggested(500)),
                work_amount: s(1800.0),
                users: vec!["RS_Bench".into()],
                research_prerequisite: Some(String::new()),
                research_prerequisites: vec!["RS_Research".into()],
                extra: vec![node()],
                skill_level: Some(rimstudio_design::model::Sourced::typed(4)),
            },
        }
    }

    #[test]
    fn the_custom_ammo_spec_has_the_same_json_in_the_dto() {
        let spec = CustomAmmoSpec {
            name: "Mine".into(),
            caliber: "caliber".into(),
            set_label: Some("set".into()),
            similar_to: Some("RS_Set".into()),
            category_parent: Some("RS_Parent".into()),
            category_icon: Some("icon".into()),
            default_type: Some("FMJ".into()),
            set_extra: vec![node()],
            types: vec![full_type()],
        };
        let dto: CustomAmmoDto = through_json(&spec).unwrap();
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            serde_json::to_value(&spec).unwrap()
        );
        let back: CustomAmmoSpec = through_json(&dto).unwrap();
        assert_eq!(back, spec);
        let empty: CustomAmmoDto = through_json(&CustomAmmoSpec::default()).unwrap();
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::to_value(CustomAmmoSpec::default()).unwrap()
        );
    }

    #[test]
    fn the_catalogue_and_the_suggestion_have_the_same_json_in_the_dto() {
        let page = CatalogPage {
            total: 2,
            matching: 1,
            page: 0,
            page_size: 25,
            entries: vec![CatalogEntry {
                def_name: "RS_Set".into(),
                label: "set".into(),
                caliber: "cal".into(),
                family: Some("fam".into()),
                similar_to: Some("RS_Other".into()),
                generic: true,
                similar_sets: 2,
                types: vec![CatalogType {
                    ammo_def: "RS_Ammo".into(),
                    ammo_label: "ammo".into(),
                    ammo_class: "RS_FMJ".into(),
                    ammo_class_label: "FMJ".into(),
                    projectile_def: "RS_Bullet".into(),
                    damage_def: Some("RS_Damage".into()),
                    damage: Some(1.0),
                    armor_penetration_sharp: Some(2.0),
                    armor_penetration_blunt: Some(3.0),
                    speed: Some(4.0),
                    pellets: Some(5.0),
                    explosion_radius: Some(6.0),
                    secondary_damage: vec![CatalogSecondary {
                        def: "RS_Bomb".into(),
                        amount: 7.0,
                    }],
                }],
                weapon_count: 3,
                example_weapons: vec!["gun".into()],
                suggested: true,
                score: Some(1.5),
            }],
            calibers: vec![CatalogFacet {
                name: "fam".into(),
                label: "fam".into(),
                count: 1,
            }],
            classes: vec![CatalogFacet {
                name: "RS_FMJ".into(),
                label: "FMJ".into(),
                count: 1,
            }],
        };
        let dto: CeAmmoCatalogDto = through_json(&page).unwrap();
        let mut want = serde_json::to_value(&page).unwrap();
        want["available"] = serde_json::json!(false);
        assert_eq!(
            serde_json::to_value(&dto).unwrap().get("entries"),
            want.get("entries")
        );
        let suggestion = AmmoSuggestion {
            available: true,
            reason: Some("because".into()),
            class: "RS_FMJ".into(),
            class_label: "FMJ".into(),
            copied_from: Some("RS_Ammo".into()),
            nearest: vec![NearestAmmo {
                ammo_def: "RS_Ammo".into(),
                ammo_label: "ammo".into(),
                set: Some("RS_Set".into()),
                projectile_def: "RS_Bullet".into(),
                damage: Some(1.0),
                speed: Some(2.0),
                energy: Some(2.0),
                distance: 0.5,
            }],
            fields: vec![SuggestedAmmoField {
                field: "/projectile/damage".into(),
                label: "damage".into(),
                value: Some(1.0),
                text: Some("t".into()),
                source: AmmoSourceKind::Nearest,
                rating: Reliability::Rough,
                n: 5,
                from: vec!["RS_Ammo".into()],
                range: Some((1.0, 2.0)),
            }],
            ammo_type: full_type(),
            notes: vec!["note".into()],
        };
        let dto: CeAmmoSuggestionDto = through_json(&suggestion).unwrap();
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            serde_json::to_value(&suggestion).unwrap()
        );
    }
}
