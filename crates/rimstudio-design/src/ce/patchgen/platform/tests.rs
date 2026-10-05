//! Tests of the platform and under barrel conversion over the fictional reference set, run through the real
//! def engine (the dry run) so the generated operations are applied, not only inspected.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::{Node, NodeBuilder};

use super::*;
use crate::ce::patchgen::{
    CeProjectState, ConversionSource, ConvertAnswers, ConvertEnv, ConvertStatus, GeneratedPatch,
    PatchMode, convert, dry_apply, gun_patch, scan,
};
use crate::ce::reader::fixtures_tests::{Build, load_set, rows};
use crate::ce::reader::platform::read_platform;
use crate::ce::reader::{CeModel, CeReadOptions, read_conversions_with};
use crate::model::{
    CeUnderBarrelFireModes, DesignSpec, RangedInputs, Sourced, TechLevel, ToolSpec, ValueSource,
};
use crate::plan::ProjectLayout;
use crate::reader::fixtures_tests::{self as vanilla, GunNumbers};

const ABILITY: &str = "CompProperties_EquippableAbilityReloadable";

fn model() -> CeModel {
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows,
        Build {
            ce: false,
            ..Build::default()
        },
    );
    read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            vanilla: Some(&plain.databases),
            ..CeReadOptions::default()
        },
    )
}

/// A fictional gun, with an ability component when asked.
fn gun_node(name: &str, ability: bool) -> Node {
    let mut node = vanilla::gun(
        name,
        "RS_Shot1",
        GunNumbers {
            mass: 2.4,
            range: 30.0,
            warmup: 1.1,
            cooldown: 1.4,
            burst: 3,
            tier: "Industrial",
        },
        &["RS_Gun"],
    );
    let mut comps = NodeBuilder::new("comps")
        .attr("Inherit", "False")
        .elem("li", |li| li.attr("Class", "RS_CompSlot"))
        .build();
    if ability {
        comps.push_child(
            NodeBuilder::new("li")
                .attr("Class", ABILITY)
                .text_elem("abilityDef", "RS_Burner")
                .build(),
        );
    }
    node.push_child(comps);
    node
}

fn gun_spec(name: &str) -> DesignSpec {
    let mut s = DesignSpec::new_ranged(name, "test gun");
    s.tech_level = Some(TechLevel::Industrial);
    s.mass = Some(Sourced::typed(2.4));
    s.ranged = Some(RangedInputs {
        range: Some(Sourced::typed(30.0)),
        warmup: Some(Sourced::typed(1.1)),
        cooldown: Some(Sourced::typed(1.4)),
        burst_count: Some(Sourced::typed(3)),
        ticks_between_burst_shots: Some(Sourced::typed(8.0)),
        sound_cast: Some("RS_Shot".into()),
        ..RangedInputs::default()
    });
    s.tools = vec![ToolSpec::new("grip", &["Blunt"]).with_numbers(8.0, 2.0, ValueSource::Typed)];
    s.ce = Some(CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        default_projectile: Some("RS_CeBullet1".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        magazine_size: Some(Sourced::typed(24)),
        reload_time: Some(Sourced::typed(3.5)),
        bulk: Some(Sourced::typed(5.5)),
        sway_factor: Some(Sourced::typed(1.25)),
        shot_spread: Some(Sourced::typed(0.09)),
        tool_penetration: vec![crate::model::CeToolPenetration {
            tool: "grip".into(),
            sharp: None,
            blunt: Some(Sourced::typed(3.0)),
        }],
        ..CePatchSpec::default()
    });
    s
}

fn unit() -> CeUnderBarrel {
    CeUnderBarrel {
        standard_label: Some("switch to rifle".into()),
        under_barrel_label: Some("switch to spray".into()),
        ammo_set: Some("RS_AmmoSetB".into()),
        default_projectile: Some("RS_CeBullet2".into()),
        magazine_size: Some(Sourced::typed(6)),
        reload_time: Some(Sourced::typed(5.0)),
        recoil_amount: Some(Sourced::typed(0.4)),
        warmup_time: Some(Sourced::typed(1.1)),
        range: Some(Sourced::typed(12.0)),
        min_range: Some(1.0),
        burst_shot_count: Some(5),
        ticks_between_burst_shots: Some(3),
        sound_cast: Some("RS_Spray".into()),
        muzzle_flash_scale: Some(0.0),
        verb_extra: vec![
            NodeBuilder::new("targetParams")
                .text_elem("canTargetLocations", "true")
                .build(),
        ],
        fire_modes: CeUnderBarrelFireModes {
            ai_use_burst_mode: Some(false),
            ai_aim_mode: Some("AimedShot".into()),
            aimed_burst_shot_count: Some(5),
            no_single_shot: true,
        },
        ..CeUnderBarrel::default()
    }
}

fn link(attachment: &str) -> CeAttachmentLink {
    CeAttachmentLink {
        attachment: attachment.into(),
        draw_scale: Some("(1.2,1.2)".into()),
        draw_offset: Some("(0.1,0)".into()),
        stat_offsets: vec![CeStatEntry {
            stat: "RS_Spread".into(),
            value: -0.02,
        }],
        stat_multipliers: vec![CeStatEntry {
            stat: "RS_Sway".into(),
            value: 0.9,
        }],
        stat_replacers: Vec::new(),
    }
}

fn part() -> CeGraphicPart {
    CeGraphicPart {
        part_graphic: Some(
            NodeBuilder::new("partGraphicData")
                .text_elem("texPath", "Things/RS/Stock")
                .text_elem("graphicClass", "Graphic_Single")
                .build(),
        ),
        outline_graphic: None,
        slot_tags: vec!["RS_Stock".into()],
    }
}

fn scratch_defs(extra: Node) -> Vec<Node> {
    let mut defs = vanilla::base_defs();
    defs.push(extra);
    defs
}

fn comp_count(def: &Node, class: &str) -> usize {
    def.child("comps")
        .map(|l| {
            l.children_named("li")
                .filter(|li| li.attr("Class") == Some(class))
                .count()
        })
        .unwrap_or(0)
}

fn generate(spec: &DesignSpec, node: &Node) -> GeneratedPatch {
    let patch = gun_patch(spec, &model(), &Container::from_node(node)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    patch
}

#[test]
fn the_unit_replaces_the_ability_component_and_restores_the_equippable_component() {
    let model = model();
    let node = gun_node("RS_UnitGun", true);
    let mut spec = gun_spec("RS_UnitGun");
    spec.ce.as_mut().unwrap().under_barrel = Some(unit());
    let patch = generate(&spec, &node);
    assert_eq!(patch.mode, PatchMode::New);
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_UnitGun").unwrap();
    assert_eq!(comp_count(def, &model.classes.under_barrel_comp), 1);
    assert_eq!(comp_count(def, ABILITY), 0, "the vanilla ability is gone");
    assert_eq!(comp_count(def, &model.classes.ammo_user), 1);
    let plain = def
        .child("comps")
        .unwrap()
        .children_named("li")
        .filter(|li| li.child_text("compClass") == Some(EQUIPPABLE_COMP))
        .count();
    assert_eq!(plain, 1, "the plain equippable component is back");
    let read = read_platform(def, &model.classes, ValueSource::Typed);
    let got = read.under_barrel.unwrap();
    assert_eq!(got.ammo_set.as_deref(), Some("RS_AmmoSetB"));
    assert_eq!(got.range.map(|r| r.value), Some(12.0));
    assert_eq!(got.fire_modes, unit().fire_modes);
    assert_eq!(got.verb_extra, unit().verb_extra);
}

#[test]
fn the_unit_is_added_when_the_target_has_no_ability_component() {
    let model = model();
    let node = gun_node("RS_AddGun", false);
    let mut spec = gun_spec("RS_AddGun");
    spec.ce.as_mut().unwrap().under_barrel = Some(unit());
    let patch = generate(&spec, &node);
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_AddGun").unwrap();
    assert_eq!(comp_count(def, &model.classes.under_barrel_comp), 1);
    assert_eq!(comp_count(def, "RS_CompSlot"), 1, "other components stay");
    let plain = def
        .child("comps")
        .unwrap()
        .children_named("li")
        .filter(|li| li.child_text("compClass") == Some(EQUIPPABLE_COMP))
        .count();
    assert_eq!(plain, 0, "nothing stood in for the equippable component");
}

#[test]
fn a_bare_unit_is_the_slot_form_without_unit_data() {
    let model = model();
    let node = gun_node("RS_SlotGun", false);
    let mut spec = gun_spec("RS_SlotGun");
    spec.ce.as_mut().unwrap().under_barrel = Some(CeUnderBarrel::default());
    let patch = generate(&spec, &node);
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_SlotGun").unwrap();
    let comp = def
        .child("comps")
        .unwrap()
        .children_named("li")
        .find(|li| li.attr("Class") == Some(model.classes.under_barrel_comp.as_str()))
        .unwrap();
    assert!(comp.elements().next().is_none(), "no unit data");
}

#[test]
fn a_platform_conversion_changes_the_type_of_the_def() {
    let model = model();
    let node = gun_node("RS_PlatGun", false);
    let mut spec = gun_spec("RS_PlatGun");
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.is_weapon_platform = true;
        ce.attachment_links = vec![link("RS_Scope"), link("RS_Grip")];
        ce.default_graphic_parts = vec![part()];
    }
    let patch = generate(&spec, &node);
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    assert!(
        !run.diagnostics
            .iter()
            .any(|d| d.code.as_str() == crate::ce::patchgen::simulate::SIMULATION_UNSUPPORTED),
        "the merge applies the platform parameters now"
    );
    let def = run.def("ThingDef", "RS_PlatGun").unwrap();
    assert_eq!(
        def.attr("Class"),
        Some(model.classes.weapon_platform_def.as_str())
    );
    assert_eq!(
        def.child_text("thingClass"),
        Some(model.classes.weapon_platform_thing.as_str())
    );
    assert_eq!(def.child_text("drawerType"), Some("RealtimeOnly"));
    let read = read_platform(def, &model.classes, ValueSource::Typed);
    assert!(read.is_platform);
    assert_eq!(read.links, vec![link("RS_Scope"), link("RS_Grip")]);
    assert_eq!(read.parts, vec![part()]);
}

#[test]
fn applying_the_patch_twice_changes_nothing() {
    let model = model();
    for ability in [true, false] {
        let node = gun_node("RS_TwiceGun", ability);
        let mut spec = gun_spec("RS_TwiceGun");
        {
            let ce = spec.ce.as_mut().unwrap();
            ce.under_barrel = Some(unit());
            ce.is_weapon_platform = true;
            ce.attachment_links = vec![link("RS_Scope")];
            ce.default_graphic_parts = vec![part()];
        }
        let patch = generate(&spec, &node);
        let once = dry_apply(&scratch_defs(node.clone()), &[patch.patch_root()], &model);
        let twice = dry_apply(
            &scratch_defs(node),
            &[patch.patch_root(), patch.patch_root()],
            &model,
        );
        assert!(twice.is_clean(), "{:?}", twice.diagnostics);
        assert_eq!(
            once.def("ThingDef", "RS_TwiceGun"),
            twice.def("ThingDef", "RS_TwiceGun"),
            "ability {ability}"
        );
    }
}

#[test]
fn the_block_alone_decides_a_spec_without_platform_choices_is_unchanged() {
    let model = model();
    let node = gun_node("RS_PlainGun", true);
    let spec = gun_spec("RS_PlainGun");
    let patch = generate(&spec, &node);
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    let def = run.def("ThingDef", "RS_PlainGun").unwrap();
    assert_eq!(def.attr("Class"), None);
    assert_eq!(comp_count(def, &model.classes.under_barrel_comp), 0);
    assert_eq!(
        comp_count(def, ABILITY),
        1,
        "without a unit the vanilla comp stays"
    );
}

#[test]
fn validation_asks_for_the_unit_numbers_and_checks_the_references() {
    let model = model();
    let mut ce = gun_spec("RS_X").ce.unwrap();
    ce.under_barrel = Some(CeUnderBarrel {
        ammo_set: Some("RS_AmmoSetMissing".into()),
        ..CeUnderBarrel::default()
    });
    ce.attachment_links = vec![CeAttachmentLink::default()];
    let found = validate(&ce, &model);
    let pointers: Vec<&str> = found
        .iter()
        .filter_map(crate::validation::codes::diagnostic_field)
        .collect();
    for want in [
        "/ce/underBarrel/ammoSet",
        "/ce/underBarrel/defaultProjectile",
        "/ce/underBarrel/magazineSize",
        "/ce/underBarrel/reloadTime",
        "/ce/underBarrel/range",
        "/ce/attachmentLinks/0/attachment",
    ] {
        assert!(pointers.contains(&want), "{want} in {pointers:?}");
    }
    // A projectile of another set is refused; a bare unit asks for nothing.
    ce.under_barrel = Some(CeUnderBarrel {
        default_projectile: Some("RS_CeBullet1".into()),
        ..unit()
    });
    ce.attachment_links.clear();
    let found = validate(&ce, &model);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        crate::validation::codes::diagnostic_field(&found[0]),
        Some("/ce/underBarrel/defaultProjectile")
    );
    ce.under_barrel = Some(CeUnderBarrel::default());
    assert!(validate(&ce, &model).is_empty());
}

#[test]
fn an_invalid_unit_gives_no_operations() {
    let node = gun_node("RS_BadGun", true);
    let mut spec = gun_spec("RS_BadGun");
    spec.ce.as_mut().unwrap().under_barrel = Some(CeUnderBarrel {
        ammo_set: Some("RS_AmmoSetB".into()),
        ..CeUnderBarrel::default()
    });
    let patch = gun_patch(&spec, &model(), &Container::from_node(&node)).unwrap();
    assert!(patch.has_errors());
    assert!(patch.operations.is_empty());
}

#[test]
fn update_mode_adds_the_missing_unit_and_changes_only_what_differs() {
    let model = model();
    let node = gun_node("RS_UpdGun", true);
    // First convert without a unit, then ask for one on the converted def.
    let plain = gun_spec("RS_UpdGun");
    let first = generate(&plain, &node);
    let run = dry_apply(&scratch_defs(node.clone()), &[first.patch_root()], &model);
    let converted = run.def("ThingDef", "RS_UpdGun").unwrap().clone();
    let record = rimstudio_defs::DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: "RS_UpdGun".into(),
        origin: None,
        node: converted.clone(),
        parents: Vec::new(),
        patched_by: Vec::new(),
    };
    let container = Container::from_def(
        &record,
        Some(&converted),
        &model.classes,
        ConversionSource::Unknown,
    );
    assert!(container.is_converted());
    let mut spec = gun_spec("RS_UpdGun");
    spec.ce = container.existing.as_ref().map(|e| e.block.clone());
    spec.ce.as_mut().unwrap().under_barrel = Some(unit());
    let patch = gun_patch(&spec, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    assert!(!patch.operations.is_empty());
    let run = dry_apply(&scratch_defs(converted), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_UpdGun").unwrap();
    assert_eq!(comp_count(def, &model.classes.under_barrel_comp), 1);
    // The converted def now holds the unit: asking again for the same unit writes nothing, a new range
    // writes one field.
    let record = rimstudio_defs::DefRecord {
        node: def.clone(),
        ..record
    };
    let container = Container::from_def(
        &record,
        Some(def),
        &model.classes,
        ConversionSource::Unknown,
    );
    let mut same = spec.clone();
    same.ce = container.existing.as_ref().map(|e| e.block.clone());
    assert!(
        gun_patch(&same, &model, &container)
            .unwrap()
            .operations
            .is_empty()
    );
    let mut longer = same.clone();
    longer
        .ce
        .as_mut()
        .unwrap()
        .under_barrel
        .as_mut()
        .unwrap()
        .range = Some(Sourced::typed(14.0));
    let patch = gun_patch(&longer, &model, &container).unwrap();
    assert_eq!(patch.operations.len(), 1, "{:?}", patch.operations);
    let run = dry_apply(&scratch_defs(def.clone()), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let read = read_platform(
        run.def("ThingDef", "RS_UpdGun").unwrap(),
        &model.classes,
        ValueSource::Typed,
    );
    assert_eq!(
        read.under_barrel.unwrap().range.map(|r| r.value),
        Some(14.0)
    );
}

#[test]
fn update_mode_turns_a_converted_def_into_a_platform_and_adds_missing_links() {
    let model = model();
    let node = gun_node("RS_UpdPlat", false);
    let first = generate(&gun_spec("RS_UpdPlat"), &node);
    let run = dry_apply(&scratch_defs(node), &[first.patch_root()], &model);
    let converted = run.def("ThingDef", "RS_UpdPlat").unwrap().clone();
    let record = rimstudio_defs::DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: "RS_UpdPlat".into(),
        origin: None,
        node: converted.clone(),
        parents: Vec::new(),
        patched_by: Vec::new(),
    };
    let container = Container::from_def(
        &record,
        Some(&converted),
        &model.classes,
        ConversionSource::Unknown,
    );
    let mut spec = gun_spec("RS_UpdPlat");
    spec.ce = container.existing.as_ref().map(|e| e.block.clone());
    spec.ce.as_mut().unwrap().attachment_links = vec![link("RS_Scope")];
    let patch = gun_patch(&spec, &model, &container).unwrap();
    let run = dry_apply(
        &scratch_defs(converted),
        &[patch.patch_root(), patch.patch_root()],
        &model,
    );
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_UpdPlat").unwrap();
    assert_eq!(
        def.attr("Class"),
        Some(model.classes.weapon_platform_def.as_str())
    );
    let read = read_platform(def, &model.classes, ValueSource::Typed);
    assert_eq!(read.links.len(), 1, "applied twice, one link");
    // Now a platform: a second link is added and the first is left alone.
    let record = rimstudio_defs::DefRecord {
        node: def.clone(),
        ..record
    };
    let container = Container::from_def(
        &record,
        Some(def),
        &model.classes,
        ConversionSource::Unknown,
    );
    let mut spec = gun_spec("RS_UpdPlat");
    spec.ce = container.existing.as_ref().map(|e| e.block.clone());
    spec.ce
        .as_mut()
        .unwrap()
        .attachment_links
        .push(link("RS_Grip"));
    let patch = gun_patch(&spec, &model, &container).unwrap();
    let run = dry_apply(&scratch_defs(def.clone()), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let links = read_platform(
        run.def("ThingDef", "RS_UpdPlat").unwrap(),
        &model.classes,
        ValueSource::Typed,
    )
    .links;
    assert_eq!(
        links
            .iter()
            .map(|l| l.attachment.as_str())
            .collect::<Vec<_>>(),
        ["RS_Scope", "RS_Grip"]
    );
}

#[test]
fn the_overlay_merges_links_by_attachment_and_keeps_parts_unique() {
    let mut base = CePatchSpec {
        attachment_links: vec![link("RS_Scope")],
        default_graphic_parts: vec![part()],
        ..CePatchSpec::default()
    };
    let mut changed = link("RS_Scope");
    changed.draw_scale = Some("(2,2)".into());
    let with = CePatchSpec {
        is_weapon_platform: true,
        attachment_links: vec![changed.clone(), link("RS_Grip")],
        default_graphic_parts: vec![part()],
        under_barrel: Some(unit()),
        ..CePatchSpec::default()
    };
    overlay(&mut base, &with);
    assert!(base.is_weapon_platform);
    assert_eq!(base.attachment_links.len(), 2);
    assert_eq!(base.attachment_links[0], changed);
    assert_eq!(base.default_graphic_parts.len(), 1);
    assert_eq!(base.under_barrel, Some(unit()));
}

#[test]
fn a_library_habit_needs_enough_agreeing_units() {
    use super::derive::unit_fire_habit;
    let mut m = model();
    assert_eq!(unit_fire_habit(&m), None, "no units, no habit");
    let example = |name: &str, aim: &str| crate::ce::reader::UnderBarrelExample {
        def_name: name.into(),
        main_ammo_set: None,
        unit: CeUnderBarrel {
            fire_modes: CeUnderBarrelFireModes {
                ai_aim_mode: Some(aim.into()),
                no_single_shot: true,
                ..CeUnderBarrelFireModes::default()
            },
            ..unit()
        },
    };
    m.platform.under_barrels = vec![example("RS_A", "AimedShot")];
    assert_eq!(unit_fire_habit(&m), None, "one unit is not a habit");
    m.platform.under_barrels = vec![example("RS_A", "AimedShot"), example("RS_B", "AimedShot")];
    let habit = unit_fire_habit(&m).unwrap();
    assert_eq!(habit.ai_aim_mode.as_deref(), Some("AimedShot"));
    assert!(habit.no_single_shot);
    m.platform.under_barrels = vec![example("RS_A", "AimedShot"), example("RS_B", "Snapshot")];
    let habit = unit_fire_habit(&m).unwrap();
    assert_eq!(habit.ai_aim_mode, None, "a split vote is not used");
}

fn env<'a>(
    dbs: &'a rimstudio_defs::DefDatabases,
    model: &'a CeModel,
    project: &'a [Node],
    state: &'a CeProjectState,
    layout: &'a ProjectLayout,
    reader: &'a crate::reader::options::ReaderOptions,
    source: &'a ConversionSource,
) -> ConvertEnv<'a> {
    ConvertEnv {
        dbs,
        model,
        layout,
        reader,
        project,
        state,
        source,
    }
}

#[test]
fn the_convert_flow_asks_for_the_unit_of_a_weapon_with_an_ability_and_converts_once_answered() {
    let model = model();
    let ability = NodeBuilder::new("AbilityDef")
        .text_elem("defName", "RS_Burner")
        .elem("verbProperties", |v| {
            v.text_elem("range", "9.9").text_elem("warmupTime", "0.5")
        })
        .build();
    let weapon = gun_node("RS_FlameGun", true);
    let mut defs = vanilla::base_defs();
    defs.push(ability);
    defs.push(weapon.clone());
    defs.push(gun_node("RS_PlainGun", false));
    let table = vanilla::types(&[("Verse.AbilityDef", "Verse.Def")]);
    let loaded = vanilla::load_defs(defs, &table);
    let project = vec![weapon, gun_node("RS_PlainGun", false)];
    let candidates = scan(&project, &loaded.databases, &model);
    let flame = candidates.iter().find(|c| c.def == "RS_FlameGun").unwrap();
    assert_eq!(flame.status, ConvertStatus::NotConverted);
    assert!(flame.reason.contains("under barrel"), "{}", flame.reason);
    let plain = candidates.iter().find(|c| c.def == "RS_PlainGun").unwrap();
    assert_eq!(plain.reason, "can be converted");

    let layout = ProjectLayout::default();
    let reader = crate::reader::options::ReaderOptions::default();
    let state = CeProjectState::default();
    let source = ConversionSource::Unknown;
    let e = env(
        &loaded.databases,
        &model,
        &project,
        &state,
        &layout,
        &reader,
        &source,
    );
    let base = ConvertAnswers {
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        one_handed: Some(false),
        belt_fed: Some(false),
        ..ConvertAnswers::default()
    };
    let bare = convert(flame, &base, &e);
    let unit_asks: Vec<&str> = bare
        .asks
        .items
        .iter()
        .map(|i| i.field.as_str())
        .filter(|f| f.starts_with("/ce/underBarrel"))
        .collect();
    assert_eq!(
        unit_asks,
        [
            "/ce/underBarrel/ammoSet",
            "/ce/underBarrel/magazineSize",
            "/ce/underBarrel/reloadTime",
            "/ce/underBarrel/range"
        ]
    );
    let range = bare
        .asks
        .items
        .iter()
        .find(|i| i.field == "/ce/underBarrel/range")
        .unwrap();
    assert_eq!(
        range.suggestion,
        Some(9.9),
        "the vanilla ability is shown as a reference"
    );
    // The plain weapon asks nothing about a unit.
    let plain_out = convert(plain, &base, &e);
    assert!(
        !plain_out
            .asks
            .items
            .iter()
            .any(|i| i.field.starts_with("/ce/underBarrel"))
    );
    // Fully answered, the plan is written and the spec carries the unit.
    let full = ConvertAnswers {
        overrides: CePatchSpec {
            bulk: Some(Sourced::typed(8.0)),
            sway_factor: Some(Sourced::typed(1.2)),
            shot_spread: Some(Sourced::typed(0.1)),
            magazine_size: Some(Sourced::typed(30)),
            reload_time: Some(Sourced::typed(4.0)),
            under_barrel: Some(unit()),
            ..CePatchSpec::default()
        },
        tool_penetration: vec![crate::model::CeToolPenetration {
            tool: "grip".into(),
            sharp: None,
            blunt: Some(Sourced::typed(2.0)),
        }],
        ..base.clone()
    };
    let done = convert(flame, &full, &e);
    assert!(done.asks.is_empty(), "{:?}", done.asks);
    assert!(!done.plan.files.is_empty());
    assert_eq!(done.spec.unwrap().ce.unwrap().under_barrel, Some(unit()));
    // Skipping the unit removes the questions.
    let skipped = convert(
        flame,
        &ConvertAnswers {
            skip_under_barrel: true,
            ..base.clone()
        },
        &e,
    );
    assert!(
        !skipped
            .asks
            .items
            .iter()
            .any(|i| i.field.starts_with("/ce/underBarrel"))
    );
}

#[test]
fn a_weapon_whose_verb_names_no_projectile_is_a_listed_gun_that_is_not_converted() {
    let model = model();
    let beam = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_BeamGun")
        .text_elem("label", "beam gun")
        .text_elem("category", "Item")
        .text_elem("techLevel", "Spacer")
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "RS_Verb_Beam")
                    .text_elem("range", "15")
            })
        })
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "grip")
                    .elem("capacities", |c| c.li("Blunt"))
                    .text_elem("power", "8")
                    .text_elem("cooldownTime", "2")
            })
        })
        .elem("weaponTags", |w| w.li("RS_Gun"))
        .build();
    let mut defs = vanilla::base_defs();
    defs.push(beam.clone());
    let loaded = vanilla::load_defs(defs, &vanilla::types(&[]));
    let candidates = scan(&[beam], &loaded.databases, &model);
    let c = candidates.iter().find(|c| c.def == "RS_BeamGun").unwrap();
    assert_eq!(c.status, ConvertStatus::UnsupportedKind);
    assert_eq!(c.kind, Some(crate::model::ItemKind::Ranged));
    assert!(c.reason.contains("no projectile"), "{}", c.reason);
}

#[test]
fn the_reader_round_trips_the_generated_nodes() {
    let model = model();
    let li = under_barrel_li(&model.classes, &unit());
    let read = crate::ce::reader::platform::read_under_barrel(&li, ValueSource::Typed);
    assert_eq!(read, unit());
    let links = [link("RS_Scope")];
    let nodes: Vec<Node> = links.iter().map(link_node).collect();
    let refs: Vec<&Node> = nodes.iter().collect();
    assert_eq!(crate::ce::reader::platform::links_from_nodes(&refs), links);
    assert!(make_gun_params(&CePatchSpec::default()).is_empty());
}

#[test]
fn a_unit_without_fire_modes_takes_the_habit_and_says_so() {
    let mut m = model();
    let learned = |name: &str| crate::ce::reader::UnderBarrelExample {
        def_name: name.into(),
        main_ammo_set: None,
        unit: CeUnderBarrel {
            fire_modes: CeUnderBarrelFireModes {
                ai_aim_mode: Some("AimedShot".into()),
                no_single_shot: true,
                ..CeUnderBarrelFireModes::default()
            },
            ..unit()
        },
    };
    m.platform.under_barrels = vec![learned("RS_A"), learned("RS_B")];
    let mut bare_modes = unit();
    bare_modes.fire_modes = CeUnderBarrelFireModes::default();
    let mut diagnostics = Vec::new();
    let out = with_habit(&bare_modes, &m, &mut diagnostics);
    assert_eq!(out.fire_modes.ai_aim_mode.as_deref(), Some("AimedShot"));
    assert!(out.fire_modes.no_single_shot);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_str(), "ce.derived-value");
    // A unit with fire modes of its own, and the bare slot form, are left alone.
    let mut diagnostics = Vec::new();
    assert_eq!(with_habit(&unit(), &m, &mut diagnostics), unit());
    let slot = CeUnderBarrel::default();
    assert_eq!(with_habit(&slot, &m, &mut diagnostics), slot);
    assert!(diagnostics.is_empty());
}

#[test]
fn a_unit_that_shares_the_main_ammo_holder_needs_no_ammo_of_its_own() {
    let model = model();
    let mut ce = gun_spec("RS_X").ce.unwrap();
    ce.under_barrel = Some(CeUnderBarrel {
        one_ammo_holder: true,
        default_projectile: Some("RS_CeBullet1".into()),
        range: Some(Sourced::typed(10.0)),
        ..CeUnderBarrel::default()
    });
    assert!(validate(&ce, &model).is_empty());
    let li = under_barrel_li(&model.classes, ce.under_barrel.as_ref().unwrap());
    assert_eq!(li.child_text("oneAmmoHolder"), Some("true"));
    assert!(
        li.child("propsUnderBarrel")
            .unwrap()
            .child("ammoSet")
            .is_none()
    );
}

#[test]
fn children_the_model_does_not_type_are_carried_as_written() {
    let model = model();
    let mut u = unit();
    u.props_extra = vec![Node::with_text("reloadOneAtATime", "true")];
    u.extra = vec![Node::with_text("underBarrelIconTexPath", "UI/RS/Spray")];
    u.verb_extra
        .push(Node::with_text("soundCastTail", "RS_Tail"));
    let li = under_barrel_li(&model.classes, &u);
    let read = crate::ce::reader::platform::read_under_barrel(&li, ValueSource::Typed);
    assert_eq!(read, u);
    assert_eq!(
        li.child("propsUnderBarrel")
            .unwrap()
            .child_text("reloadOneAtATime"),
        Some("true")
    );
}
