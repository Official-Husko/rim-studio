//! Custom Combat Extended ammunition through the public API: the library the reader makes of the defs, the
//! catalogue, the suggestions, the generated definition files (golden), validation, lint, the dry load, the
//! plan and its idempotence. Everything is fictional; the real install is compared by `real_ce_ammo`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ammo;
mod common_ce;

use std::collections::BTreeMap;

use rimstudio_design::ce::ammo::{self, AmmoHints, CatalogQuery};
use rimstudio_design::model::{CePatchSpec, CustomAmmoSpec, DesignSpec, ValueSource};

#[test]
fn the_library_is_read_from_the_defs() {
    let model = common_ammo::model();
    assert!(model.is_present());
    assert_eq!(model.ammo_sets.len(), 6);
    let lib = &model.ammo;
    assert_eq!(lib.classes.len(), 4);
    assert_eq!(lib.ammo.len(), 16);
    let fmj = lib.ammo.get("RS_Ammo_Pistol9_FMJ").unwrap();
    assert_eq!(fmj.ammo_class, "RS_FMJ");
    assert_eq!(fmj.parent.as_deref(), Some("RS_AmmoBase"));
    assert!(fmj.mass.is_some() && fmj.bulk.is_some());
    let bullet = lib.projectiles.get("RS_Bullet_Pistol9_FMJ").unwrap();
    assert_eq!(bullet.damage, Some(10.0));
    assert_eq!(bullet.damage_def.as_deref(), Some("RS_Bullet"));
    assert_eq!(bullet.parent.as_deref(), Some("RS_BulletBase"));
    let he = lib.projectiles.get("RS_Bullet_Grenade_HE").unwrap();
    assert_eq!(he.secondary, vec![("RS_BombSecondary".to_owned(), 4.0)]);
    assert_eq!(he.fragments.len(), 1);
    assert_eq!(
        lib.recipes.get("RS_Ammo_Pistol9_FMJ").unwrap().products,
        Some(500.0)
    );
    assert_eq!(
        lib.caliber_of("RS_Ammo_Pistol9_FMJ"),
        Some(("9mm test".to_owned(), Some("pistol".to_owned())))
    );
}

fn model() -> rimstudio_design::ce::reader::CeModel {
    common_ammo::model()
}

#[test]
fn the_catalogue_lists_every_set_with_its_types_and_users() {
    let model = model();
    let page = ammo::catalog(&model, &CatalogQuery::default(), &BTreeMap::new());
    assert_eq!(page.total, 6);
    assert_eq!(page.matching, 6);
    let pistol = page
        .entries
        .iter()
        .find(|e| e.def_name == "RS_AmmoSet_Pistol9")
        .unwrap();
    assert_eq!(pistol.label, "9mm test");
    assert_eq!(pistol.caliber, "9mm test");
    assert_eq!(pistol.family.as_deref(), Some("pistol"));
    assert_eq!(pistol.types.len(), 3);
    assert!(pistol.generic);
    assert_eq!(pistol.similar_sets, 1);
    assert_eq!(pistol.weapon_count, 4);
    assert_eq!(pistol.example_weapons.len(), 3);
    let t = &pistol.types[1];
    assert_eq!(t.ammo_def, "RS_Ammo_Pistol9_AP");
    assert_eq!(t.ammo_class, "RS_AP");
    assert_eq!(t.ammo_class_label, "AP");
    assert_eq!(t.projectile_def, "RS_Bullet_Pistol9_AP");
    assert_eq!(t.damage, Some(7.0));
    assert_eq!(t.armor_penetration_sharp, Some(10.0));
    let similar = page
        .entries
        .iter()
        .find(|e| e.def_name == "RS_AmmoSet_Rifle8")
        .unwrap();
    assert_eq!(similar.similar_to.as_deref(), Some("RS_AmmoSet_Pistol9"));
    assert!(!similar.generic);
    let grenade = page
        .entries
        .iter()
        .find(|e| e.def_name == "RS_AmmoSet_Grenade")
        .unwrap();
    assert_eq!(grenade.types[0].secondary_damage.len(), 1);
    assert_eq!(grenade.types[0].explosion_radius, Some(1.5));
}

#[test]
fn the_catalogue_searches_filters_and_pages() {
    let model = model();
    let none = BTreeMap::new();
    let q = |query: Option<&str>,
             caliber: Option<&str>,
             class: Option<&str>,
             page: usize,
             size: usize| CatalogQuery {
        query: query.map(str::to_owned),
        caliber: caliber.map(str::to_owned),
        class: class.map(str::to_owned),
        page,
        page_size: Some(size),
    };
    assert_eq!(
        ammo::catalog(&model, &q(Some("rifle"), None, None, 0, 25), &none).matching,
        3
    );
    assert_eq!(
        ammo::catalog(&model, &q(Some("9mm hollow"), None, None, 0, 25), &none).matching,
        1
    );
    assert_eq!(
        ammo::catalog(&model, &q(None, Some("Pistol"), None, 0, 25), &none).matching,
        2
    );
    assert_eq!(
        ammo::catalog(&model, &q(None, None, Some("rs_he"), 0, 25), &none).matching,
        1
    );
    assert_eq!(
        ammo::catalog(
            &model,
            &q(Some("nothing like it"), None, None, 0, 25),
            &none
        )
        .matching,
        0
    );
    let p0 = ammo::catalog(&model, &q(None, None, None, 0, 4), &none);
    let p1 = ammo::catalog(&model, &q(None, None, None, 1, 4), &none);
    let p2 = ammo::catalog(&model, &q(None, None, None, 2, 4), &none);
    assert_eq!(
        (p0.entries.len(), p1.entries.len(), p2.entries.len()),
        (4, 2, 0)
    );
    assert!(p0.entries.iter().all(|e| !p1.entries.contains(e)));
    assert!(
        p0.calibers
            .iter()
            .any(|f| f.name == "rifle" && f.count == 3)
    );
    assert!(
        p0.classes
            .iter()
            .any(|f| f.name == "RS_HE" && f.label == "HE" && f.count == 1)
    );
    let again = ammo::catalog(&model, &q(None, None, None, 0, 4), &none);
    assert_eq!(p0, again);
}

#[test]
fn suggested_sets_come_first_and_carry_their_score() {
    let model = model();
    let mut suggested = BTreeMap::new();
    suggested.insert("RS_AmmoSet_Rifle8".to_owned(), 2.5);
    suggested.insert("RS_AmmoSet_Pistol10".to_owned(), 1.0);
    let page = ammo::catalog(&model, &CatalogQuery::default(), &suggested);
    assert_eq!(page.entries[0].def_name, "RS_AmmoSet_Rifle8");
    assert!(page.entries[0].suggested);
    assert_eq!(page.entries[0].score, Some(2.5));
    assert_eq!(page.entries[1].def_name, "RS_AmmoSet_Pistol10");
    assert!(!page.entries[2].suggested);
}

fn custom(model: &rimstudio_design::ce::reader::CeModel) -> CustomAmmoSpec {
    let hints = AmmoHints {
        damage: Some(16.0),
        speed: Some(155.0),
        ..AmmoHints::default()
    };
    let mut fmj = ammo::suggest_type(model, "RS_FMJ", &hints, None).ammo_type;
    fmj.key = "FMJ".into();
    fmj.label = Some("6.8mm test (FMJ)".into());
    fmj.recipe.ingredients = vec![rimstudio_design::model::CustomIngredient {
        thing: "RS_Steel".into(),
        count: Some(rimstudio_design::model::Sourced::typed(12.0)),
        ..Default::default()
    }];
    let mut ap = ammo::suggest_type(
        model,
        "RS_AP",
        &AmmoHints::default(),
        Some("RS_Ammo_Rifle7_AP"),
    )
    .ammo_type;
    ap.key = "AP".into();
    ap.label = Some("6.8mm test (AP)".into());
    ap.projectile.armor_penetration_sharp = Some(rimstudio_design::model::Sourced::typed(15.0));
    CustomAmmoSpec {
        name: "Test68".into(),
        caliber: "6.8mm test".into(),
        similar_to: Some("RS_AmmoSet_Rifle7".into()),
        category_parent: Some("RS_AmmoRifles".into()),
        category_icon: Some("RS/Icons/Category".into()),
        types: vec![fmj, ap],
        ..CustomAmmoSpec::default()
    }
}

fn design(model: &rimstudio_design::ce::reader::CeModel) -> DesignSpec {
    let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
    spec.identity.mod_prefix = "RS".into();
    spec.ce = Some(CePatchSpec {
        custom_ammo: Some(custom(model)),
        ..CePatchSpec::default()
    });
    spec
}

#[test]
fn a_suggestion_follows_the_nearest_ammunition_of_the_class() {
    let model = model();
    let hints = AmmoHints {
        damage: Some(16.0),
        speed: Some(155.0),
        ..AmmoHints::default()
    };
    let s = ammo::suggest_type(&model, "RS_FMJ", &hints, None);
    assert!(s.available);
    assert_eq!(s.class_label, "FMJ");
    assert_eq!(s.nearest.len(), 5);
    assert!(s.nearest.windows(2).all(|w| w[0].distance <= w[1].distance));
    assert!(s.nearest.iter().all(|n| n.ammo_def.ends_with("_FMJ")));
    let t = &s.ammo_type;
    assert_eq!(t.ammo_class, "RS_FMJ");
    assert_eq!(t.key, "FMJ");
    assert_eq!(
        t.projectile.damage.map(|d| (d.value, d.source)),
        Some((16.0, ValueSource::Answered))
    );
    assert_eq!(
        t.projectile.speed.map(|d| (d.value, d.source)),
        Some((155.0, ValueSource::Answered))
    );
    assert_eq!(t.projectile.parent.as_deref(), Some("RS_BulletBase"));
    assert_eq!(t.item.parent.as_deref(), Some("RS_AmmoBase"));
    assert_eq!(t.recipe.parent.as_deref(), Some("RS_AmmoRecipeBase"));
    assert!(
        t.projectile
            .armor_penetration_sharp
            .is_some_and(|v| v.source == ValueSource::Suggested)
    );
    assert!(t.copied_from.is_none() && t.projectile.extra.is_empty());
    let ap = s
        .fields
        .iter()
        .find(|f| f.field == "/projectile/armorPenetrationSharp")
        .unwrap();
    assert_eq!(ap.source, ammo::AmmoSourceKind::Nearest);
    assert_eq!(ap.n, 5);
    assert!(ap.range.is_some());
    let damage = s
        .fields
        .iter()
        .find(|f| f.field == "/projectile/damage")
        .unwrap();
    assert_eq!(damage.source, ammo::AmmoSourceKind::Hint);
}

#[test]
fn a_copy_takes_every_value_of_the_real_type() {
    let model = model();
    let s = ammo::suggest_type(&model, "", &AmmoHints::default(), Some("RS_Ammo_Rifle7_AP"));
    assert!(s.available);
    assert_eq!(s.copied_from.as_deref(), Some("RS_Ammo_Rifle7_AP"));
    let t = &s.ammo_type;
    assert_eq!(t.ammo_class, "RS_AP");
    assert_eq!(t.copied_from.as_deref(), Some("RS_Ammo_Rifle7_AP"));
    assert_eq!(t.projectile.damage.map(|v| v.value), Some(13.0));
    assert_eq!(t.recipe.ingredients.len(), 1);
    assert_eq!(t.recipe.ingredients[0].thing, "RS_Steel");
    assert!(
        s.fields
            .iter()
            .all(|f| f.source == ammo::AmmoSourceKind::Copied)
    );
    let missing = ammo::suggest_type(&model, "RS_AP", &AmmoHints::default(), Some("RS_Nope"));
    assert!(!missing.available);
}

#[test]
fn a_class_without_ammunition_falls_back_and_says_so() {
    let model = model();
    let s = ammo::suggest_type(&model, "RS_HE", &AmmoHints::default(), None);
    assert!(s.available);
    assert!(s.nearest.iter().any(|n| n.ammo_def == "RS_Ammo_Grenade_HE"));
    let s = ammo::suggest_type(&model, "RS_Unknown", &AmmoHints::default(), None);
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("no ammunition of the class"))
    );
    assert!(
        s.fields
            .iter()
            .filter(|f| f.value.is_some())
            .all(|f| f.rating == rimstudio_design::ce::classes::Reliability::Unmeasured)
    );
    let none = ammo::suggest_type(
        &rimstudio_design::ce::reader::CeModel::absent("no CE"),
        "RS_FMJ",
        &AmmoHints::default(),
        None,
    );
    assert!(!none.available);
}

#[test]
fn the_generated_definitions_match_their_golden_tree() {
    let model = model();
    let spec = design(&model);
    let layout = rimstudio_design::plan::ProjectLayout::default();
    let made = ammo::generate(
        spec.ce.as_ref().unwrap().custom_ammo.as_ref().unwrap(),
        "RS",
        &model,
        &layout,
    );
    assert_eq!(made.stem, "RS_Test68");
    assert_eq!(made.set_name, "RS_AmmoSet_Test68");
    assert_eq!(
        made.default_projectile.as_deref(),
        Some("RS_Bullet_Test68_FMJ")
    );
    let names: Vec<&str> = made
        .defs
        .iter()
        .map(|d| d.child_text("defName").unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "RS_AmmoTest68",
            "RS_AmmoSet_Test68",
            "RS_Ammo_Test68_FMJ",
            "RS_Bullet_Test68_FMJ",
            "RS_MakeAmmo_Test68_FMJ",
            "RS_Ammo_Test68_AP",
            "RS_Bullet_Test68_AP",
            "RS_MakeAmmo_Test68_AP",
        ]
    );
    assert_eq!(made.sections.len(), 8);
    assert!(made.sections[0].comment.contains("RS_AmmoTest68"));
    let mut root = rimstudio_core::tree::Node::new("Defs");
    for d in &made.defs {
        root.push_child(d.clone());
    }
    common::assert_golden("ce_ammo_defs.json", &root.to_json_pretty());
}

#[test]
fn the_dry_load_resolves_the_generated_definitions() {
    let model = model();
    let spec = design(&model);
    let layout = rimstudio_design::plan::ProjectLayout::default();
    let made = ammo::generate(
        spec.ce.as_ref().unwrap().custom_ammo.as_ref().unwrap(),
        "RS",
        &model,
        &layout,
    );
    let run = ammo::dry_load(&made.defs, &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    assert_eq!(run.loaded.len(), 8);
    assert!(run.loaded.contains(&(
        "CombatExtended.AmmoDef".to_owned(),
        "RS_Ammo_Test68_AP".to_owned()
    )));
    assert!(run.loaded.contains(&(
        "CombatExtended.AmmoSetDef".to_owned(),
        "RS_AmmoSet_Test68".to_owned()
    )));
    // an element of an unknown type does not load
    let mut broken = made.defs.clone();
    broken[1].tag = "RS_UnknownDef".into();
    let run = ammo::dry_load(&broken, &model);
    assert!(!run.is_clean());
    // a repeated def name is reported
    let mut twice = made.defs.clone();
    twice.push(made.defs[2].clone());
    assert!(!ammo::dry_load(&twice, &model).is_clean());
}

fn errors(d: &[rimstudio_core::diag::Diagnostic]) -> Vec<&str> {
    d.iter()
        .filter(|d| d.severity == rimstudio_core::diag::Severity::Error)
        .map(|d| d.code.as_str())
        .collect()
}

fn codes(d: &[rimstudio_core::diag::Diagnostic]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn an_empty_spec_is_refused_with_the_missing_pieces() {
    let model = model();
    let d = ammo::validate_custom_ammo(&CustomAmmoSpec::default(), "RS", &model);
    let e = errors(&d);
    assert!(e.contains(&"ce.ammo-name-missing"));
    assert!(e.contains(&"ce.ammo-caliber-missing"));
    assert!(e.contains(&"ce.ammo-no-types"));
}

#[test]
fn a_complete_spec_has_no_error() {
    let model = model();
    let d = ammo::validate_custom_ammo(&custom(&model), "RS", &model);
    assert!(errors(&d).is_empty(), "{d:?}");
    assert!(!codes(&d).contains(&"ce.ammo-implausible"), "{d:?}");
}

#[test]
fn types_are_checked_for_class_keys_numbers_and_references() {
    let model = model();
    let mut spec = custom(&model);
    spec.types[0].ammo_class = "RS_NoSuchClass".into();
    spec.types[1].key = "FMJ".into();
    spec.types[1].projectile.speed = Some(rimstudio_design::model::Sourced::typed(0.0));
    spec.types[1].projectile.pellet_count = Some(rimstudio_design::model::Sourced::typed(1.5));
    spec.types[1].recipe.ingredients[0].count = None;
    spec.default_type = Some("Nope".into());
    spec.similar_to = Some("RS_NoSuchSet".into());
    spec.types[0].label = Some("bad \u{1} label".into());
    let d = ammo::validate_custom_ammo(&spec, "RS", &model);
    let e = errors(&d);
    for code in [
        "ce.ammo-class-unknown",
        "ce.ammo-key-duplicate",
        "ce.ammo-number-invalid",
        "ce.ammo-field-required",
        "ce.ammo-default-unknown",
        "ce.ammo-ref-unresolved",
        "ce.ammo-text-invalid",
    ] {
        assert!(e.contains(&code), "{code} in {e:?}");
    }
}

#[test]
fn a_number_far_from_the_installed_ammunition_is_reported_and_never_clamped() {
    let model = model();
    let mut spec = custom(&model);
    spec.types[0].projectile.damage = Some(rimstudio_design::model::Sourced::typed(500.0));
    let d = ammo::validate_custom_ammo(&spec, "RS", &model);
    let hit = d
        .iter()
        .find(|d| d.code.as_str() == "ce.ammo-implausible")
        .expect("a plausibility warning");
    assert_eq!(hit.severity, rimstudio_core::diag::Severity::Warning);
    assert_eq!(hit.args.get("what").map(String::as_str), Some("damage"));
    assert_eq!(hit.args.get("n").map(String::as_str), Some("5"));
    assert!(errors(&d).is_empty());
    let layout = rimstudio_design::plan::ProjectLayout::default();
    let made = ammo::generate(&spec, "RS", &model, &layout);
    assert_eq!(
        made.defs[3]
            .child("projectile")
            .unwrap()
            .child_text("damageAmountBase"),
        Some("500")
    );
}

#[test]
fn derived_names_that_exist_are_duplicates() {
    let model = model();
    let mut spec = custom(&model);
    spec.name = "Pistol9".into();
    let d = ammo::validate_custom_ammo(&spec, "RS", &model);
    assert!(errors(&d).contains(&"ce.ammo-duplicate-def"));
    let mut project = std::collections::BTreeSet::new();
    project.insert("RS_AmmoSet_Test68".to_owned());
    let d = ammo::duplicate_defs(&custom(&model), "RS", &project);
    assert_eq!(d.len(), 1);
}

struct Lookup;
impl rimstudio_design::validation::DefLookup for Lookup {
    fn contains(&self, kind: rimstudio_design::validation::RefKind, name: &str) -> Option<bool> {
        match kind {
            rimstudio_design::validation::RefKind::Thing => {
                Some(name == "RS_Steel" || name == "RS_Bench")
            }
            rimstudio_design::validation::RefKind::DamageDef => None,
            _ => Some(name == "RS_Research"),
        }
    }
}

#[test]
fn recipe_ingredients_must_exist_when_the_lookup_knows() {
    let model = model();
    let mut spec = custom(&model);
    assert!(ammo::validate_custom_ammo_refs(&spec, "RS", &Lookup).is_empty());
    spec.types[0].recipe.ingredients[0].thing = "RS_Unobtainium".into();
    spec.types[1].recipe.research_prerequisite = Some("RS_NoResearch".into());
    let d = ammo::validate_custom_ammo_refs(&spec, "RS", &Lookup);
    assert_eq!(
        codes(&d),
        ["ce.ammo-ref-unresolved", "ce.ammo-ref-unresolved"]
    );
}

#[test]
fn lint_rules_flag_broken_ammunition_definitions() {
    use rimstudio_core::tree::NodeBuilder;
    let model = model();
    let mut root = rimstudio_core::tree::Node::new("Defs");
    root.push_child(
        NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", "RS_AmmoSet_Bad")
            .elem("ammoTypes", |p| {
                p.text_elem("RS_Ammo_Missing", "RS_Bullet_Missing")
            })
            .build(),
    );
    root.push_child(
        NodeBuilder::new("ThingDef")
            .attr("ParentName", "RS_NoSuchParent")
            .text_elem("defName", "RS_Ammo_Bad")
            .text_elem("ammoClass", "RS_FMJ")
            .text_elem("cookOffProjectile", "RS_Bullet_Missing")
            .build(),
    );
    root.push_child(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Bullet_Bad")
            .elem("projectile", |p| p.text_elem("speed", "10"))
            .build(),
    );
    root.push_child(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Bullet_Bad2")
            .elem("projectile", |p| {
                p.attr("Class", "CombatExtended.ProjectilePropertiesCE")
                    .text_elem("speed", "10")
            })
            .build(),
    );
    root.push_child(
        NodeBuilder::new("RecipeDef")
            .text_elem("defName", "RS_Make_Bad")
            .elem("products", |p| p.text_elem("RS_Ammo_Missing", "5"))
            .build(),
    );
    root.push_child(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Bullet_Bad2")
            .build(),
    );
    let d = ammo::lint_defs(
        &[(Some("Compat/CombatExtended/Defs/Ammo/X.xml".into()), root)],
        &model,
        &Default::default(),
    );
    let rules: Vec<&str> = d
        .iter()
        .filter_map(|d| d.args.get("ruleId").map(String::as_str))
        .collect();
    for rule in [
        "CEP050", "CEP051", "CEP052", "CEP053", "CEP054", "CEP055", "CEP056", "CEP057",
    ] {
        assert!(rules.contains(&rule), "{rule} in {rules:?}");
    }
    assert!(d.iter().all(|d| d.args.contains_key("path")));
}

#[test]
fn the_generated_files_lint_clean() {
    let model = model();
    let spec = design(&model);
    let plan = ammo::ammo_plan(
        &spec,
        &model,
        &rimstudio_design::plan::ProjectLayout::default(),
    );
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let file = &plan.files[0];
    let d = ammo::lint_defs(
        &[(Some(file.path.clone()), file.tree.clone().unwrap())],
        &model,
        &Default::default(),
    );
    assert!(d.is_empty(), "{d:?}");
}

fn weapon(model: &rimstudio_design::ce::reader::CeModel) -> DesignSpec {
    let mut spec = common::ranged_spec();
    let mut ce = common::ranged_ce();
    ce.ammo_set = None;
    ce.default_projectile = None;
    ce.custom_ammo = Some(custom(model));
    spec.ce = Some(ce);
    spec
}

#[test]
fn the_plan_adds_the_ammunition_file_and_the_weapon_uses_the_custom_set() {
    use rimstudio_design::plan::{FileKind, ProjectLayout, export_ce_plan};
    let model = model();
    let layout = ProjectLayout::default();
    let plan = export_ce_plan(&weapon(&model), &model, &layout);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let paths = plan.paths();
    assert!(
        paths.contains(&"Compat/CombatExtended/Defs/Ammo/RS_Test68.xml"),
        "{paths:?}"
    );
    assert!(paths.contains(&"LoadFolders.xml"));
    let file = plan
        .file("Compat/CombatExtended/Defs/Ammo/RS_Test68.xml")
        .unwrap();
    assert_eq!(file.kind, FileKind::CeDefs);
    assert_eq!(file.sections.len(), 8);
    let patch = plan
        .files
        .iter()
        .find(|f| f.kind == FileKind::CePatch)
        .unwrap();
    let text = patch.tree.as_ref().unwrap().to_json_string();
    assert!(text.contains("RS_AmmoSet_Test68"), "{text}");
    assert!(text.contains("RS_Bullet_Test68_FMJ"));
    assert!(rimstudio_design::plan::gate_violations(&plan, &layout).is_empty());
    let versioned = export_ce_plan(
        &weapon(&model),
        &model,
        &ProjectLayout::with_version_folder("1.6"),
    );
    assert!(
        versioned
            .paths()
            .contains(&"1.6/Compat/CombatExtended/Defs/Ammo/RS_Test68.xml")
    );
}

#[test]
fn nothing_is_planned_when_the_switch_is_off() {
    use rimstudio_design::plan::{ProjectLayout, export_ce_plan, export_vanilla_plan};
    let model = model();
    let mut off = weapon(&model);
    off.ce = None;
    assert!(
        export_ce_plan(&off, &model, &ProjectLayout::default())
            .files
            .is_empty()
    );
    let vanilla = export_vanilla_plan(&weapon(&model), &ProjectLayout::default());
    assert!(
        vanilla.paths().iter().all(|p| !p.contains("Defs/Ammo")),
        "{:?}",
        vanilla.paths()
    );
    assert!(!vanilla.contains_text("RS_AmmoSet_Test68"));
}

#[test]
fn planning_twice_is_identical_and_a_changed_spec_changes_its_section() {
    use rimstudio_design::plan::{ProjectLayout, export_ce_plan};
    let model = model();
    let layout = ProjectLayout::default();
    let a = export_ce_plan(&weapon(&model), &model, &layout);
    let b = export_ce_plan(&weapon(&model), &model, &layout);
    assert_eq!(a, b);
    let mut changed = weapon(&model);
    changed
        .ce
        .as_mut()
        .unwrap()
        .custom_ammo
        .as_mut()
        .unwrap()
        .types[0]
        .projectile
        .damage = Some(rimstudio_design::model::Sourced::typed(17.0));
    let c = export_ce_plan(&changed, &model, &layout);
    let path = "Compat/CombatExtended/Defs/Ammo/RS_Test68.xml";
    assert_ne!(a.file(path).unwrap().tree, c.file(path).unwrap().tree);
    assert_eq!(
        a.file(path).unwrap().sections,
        c.file(path).unwrap().sections
    );
}

#[test]
fn an_invalid_spec_plans_no_file_at_all() {
    use rimstudio_design::plan::{ProjectLayout, export_ce_plan};
    let model = model();
    let mut bad = weapon(&model);
    bad.ce
        .as_mut()
        .unwrap()
        .custom_ammo
        .as_mut()
        .unwrap()
        .caliber
        .clear();
    let plan = export_ce_plan(&bad, &model, &ProjectLayout::default());
    assert!(plan.has_errors());
    assert!(plan.files.is_empty());
    assert!(codes(&plan.diagnostics).contains(&"ce.ammo-caliber-missing"));
}

#[test]
fn a_typed_ammo_set_is_overridden_with_a_warning() {
    use rimstudio_design::plan::{ProjectLayout, export_ce_plan};
    let model = model();
    let mut spec = weapon(&model);
    spec.ce.as_mut().unwrap().ammo_set = Some("RS_AmmoSet_Pistol9".into());
    let plan = export_ce_plan(&spec, &model, &ProjectLayout::default());
    assert!(codes(&plan.diagnostics).contains(&"ce.ammo-set-overridden"));
    assert!(!plan.has_errors());
}

#[test]
fn the_art_that_is_not_imported_is_reserved_with_an_info() {
    use rimstudio_design::plan::ProjectLayout;
    let model = model();
    let mut spec = custom(&model);
    spec.types[0].item.tex_path = None;
    let made = ammo::generate(&spec, "RS", &model, &ProjectLayout::default());
    let d = made.diagnostics;
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].code.as_str(), "ce.ammo-art-reserved");
    assert_eq!(
        d[0].args.get("path").map(String::as_str),
        Some("Things/Item/Ammo/RS_Ammo_Test68_FMJ")
    );
}

#[test]
fn every_rebuilt_set_reproduces_its_definitions() {
    let model = model();
    let rebuilt = common_ammo::rebuild_all(&model, "RX");
    assert_eq!(rebuilt.len(), 6);
    let out = common_ammo::load_with(common_ammo::defs(), &rebuilt, &common_ammo::types());
    let report = common_ammo::compare_all(&model, "RX", &rebuilt, &out.databases);
    assert_eq!(report.unresolved, 0, "{:?}", report.diffs);
    assert_eq!(report.types, 16);
    assert_eq!(report.count("missing"), 0, "{:#?}", report.diffs);
    assert_eq!(report.count("different"), 0, "{:#?}", report.diffs);
    assert_eq!(report.count("extra"), 0, "{:#?}", report.diffs);
}
