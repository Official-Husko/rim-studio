//! Tests of the Combat Extended reader over the fictional set.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use rimstudio_core::tree::{Node, NodeBuilder};

use super::fixtures_tests::{Build, load_set, rows, table};
use super::*;
use crate::ce::formulas::{GunInput, gun_values, preset_for};
use crate::model::ValueSource;
use crate::reader::fixtures_tests::{self as vanilla, GunNumbers};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn gun_def() -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Target")
        .elem("statBases", |s| {
            s.text_elem("Mass", "3")
                .text_elem("AccuracyTouch", "0.7")
                .text_elem("AccuracyLong", "0.4")
                .text_elem("MarketValue", "100")
        })
        .elem("costList", |c| {
            c.text_elem("RS_Steel", "30").text_elem("RS_Wood", "5")
        })
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "Verb_Shoot")
                    .text_elem("range", "20")
            })
            .elem("li", |li| li.text_elem("verbClass", "RS_Other.Verb"))
        })
        .elem("weaponTags", |t| t.li("RS_Old"))
        .build()
}

fn spec() -> MakeGunSpec {
    let op = NodeBuilder::new("Operation")
        .text_elem("defName", " RS_Target ")
        .elem("statBases", |s| {
            s.text_elem("Mass", "2")
                .text_elem("Bulk", "5")
                .text_elem("AccuracyShort", "0.1")
        })
        .elem("costList", |c| c.text_elem("RS_Part", "2"))
        .elem("Properties", |p| {
            p.text_elem("range", "30").text_elem("warmupTime", "1")
        })
        .elem("AmmoUser", |a| a.text_elem("magazineSize", "10"))
        .elem("FireModes", |_| NodeBuilder::new("FireModes"))
        .elem("weaponTags", |t| t.li("RS_New"))
        .elem("researchPrerequisite", |_| {
            NodeBuilder::new("researchPrerequisite")
        })
        .text_elem("texPath", "Things/RS/Tex")
        .elem("attachmentLinks", |a| a.li("x"))
        .build();
    MakeGunSpec::parse(&op)
}

fn names(parent: &Node, tag: &str) -> Vec<String> {
    parent
        .child(tag)
        .map(|c| c.elements().map(|e| e.tag.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn the_parsed_operation_keeps_every_parameter() {
    let s = spec();
    assert_eq!(s.def_name, "RS_Target");
    assert_eq!(s.stat_bases.len(), 3);
    assert_eq!(s.cost_list.as_ref().map(Vec::len), Some(1));
    assert_eq!(s.properties.len(), 2);
    assert!(s.ammo_user.is_some());
    assert_eq!(
        s.fire_modes,
        Some(Vec::new()),
        "an empty element is present"
    );
    assert_eq!(s.tex_path.as_deref(), Some("Things/RS/Tex"));
    assert_eq!(s.unsupported, vec!["attachmentLinks"]);
    assert!(
        MakeGunSpec::parse(&Node::new("Operation"))
            .def_name
            .is_empty()
    );
}

#[test]
fn the_merge_follows_the_documented_behaviour() {
    let classes = CeClassNames::default();
    let mut def = gun_def();
    let report = merge_into_def(&mut def, &spec(), &classes);
    // statBases: vanilla accuracy deleted, same named replaced, new appended.
    let stats = def.child("statBases").unwrap();
    let tags: Vec<&str> = stats.elements().map(|e| e.tag.as_str()).collect();
    assert_eq!(tags, vec!["MarketValue", "Mass", "Bulk", "AccuracyShort"]);
    assert_eq!(stats.child_text("Mass"), Some("2"));
    assert_eq!(report.accuracy_removed, 2);
    // costList cleared, then written.
    assert_eq!(names(&def, "costList"), vec!["RS_Part"]);
    // The vanilla shoot verb is gone, the other verb stays, one converted verb is appended.
    let verbs = def.child("verbs").unwrap();
    let items: Vec<&Node> = verbs.children_named("li").collect();
    assert_eq!(items.len(), 2);
    assert_eq!(report.verbs_removed, 1);
    assert!(report.verb_added);
    assert_eq!(items[0].child_text("verbClass"), Some("RS_Other.Verb"));
    assert_eq!(
        items[1].attr("Class"),
        Some(classes.verb_properties.as_str())
    );
    assert_eq!(items[1].child_text("range"), Some("30"));
    // Comps: ammo user and fire modes (empty element still writes a component).
    let comps: Vec<&Node> = def.child("comps").unwrap().children_named("li").collect();
    assert_eq!(comps.len(), 2);
    assert_eq!(comps[0].attr("Class"), Some(classes.ammo_user.as_str()));
    assert_eq!(comps[1].attr("Class"), Some(classes.fire_modes.as_str()));
    assert!(report.created.contains(&"comps".to_owned()));
    // Tags appended, never replaced.
    let tags: Vec<String> = def
        .child("weaponTags")
        .unwrap()
        .children_named("li")
        .map(Node::text_content)
        .collect();
    assert_eq!(tags, vec!["RS_Old", "RS_New"]);
    // Research and graphic.
    assert!(def.find("recipeMaker/researchPrerequisite").is_some());
    assert_eq!(
        def.find("graphicData/texPath")
            .map(Node::text_content)
            .as_deref(),
        Some("Things/RS/Tex")
    );
    assert_eq!(
        def.find("graphicData/graphicClass")
            .map(Node::text_content)
            .as_deref(),
        Some("Graphic_Single")
    );
}

#[test]
fn the_merge_is_not_idempotent() {
    let classes = CeClassNames::default();
    let mut def = gun_def();
    let _ = merge_into_def(&mut def, &spec(), &classes);
    let _ = merge_into_def(&mut def, &spec(), &classes);
    let tags = def
        .child("weaponTags")
        .unwrap()
        .children_named("li")
        .count();
    assert_eq!(tags, 3, "a second run appends the tag again");
    let comps = def.child("comps").unwrap().children_named("li").count();
    assert_eq!(comps, 4);
    let verbs = def.child("verbs").unwrap().children_named("li").count();
    assert_eq!(
        verbs, 3,
        "the converted verb of the first run is not a vanilla shoot verb, so it stays"
    );
}

#[test]
fn an_empty_cost_list_and_missing_blocks_change_nothing() {
    let mut def = gun_def();
    let before = def.clone();
    let op = NodeBuilder::new("Operation")
        .text_elem("defName", "RS_Target")
        .elem("costList", |c| c)
        .build();
    let report = merge_into_def(&mut def, &MakeGunSpec::parse(&op), &CeClassNames::default());
    assert_eq!(def, before);
    assert_eq!(report, MergeReport::default());
}

#[test]
fn detection_needs_a_converted_verb_tool_or_ammo_component() {
    let mut def = gun_def();
    assert!(!is_conversion(&def));
    let _ = merge_into_def(&mut def, &spec(), &CeClassNames::default());
    assert!(is_conversion(&def));
    let m = detect_markers(&def, &CeClassNames::default());
    assert!(m.verb && m.ammo_comp && m.fire_modes && !m.tool);
    let tooled = NodeBuilder::new("ThingDef")
        .elem("tools", |t| {
            t.elem("li", |li| li.attr("Class", CeClassNames::default().tool))
        })
        .build();
    assert!(is_conversion(&tooled));
}

#[test]
fn without_combat_extended_the_model_is_absent_with_a_reason() {
    let out = load_set(
        &rows(),
        Build {
            ce: false,
            ..Build::default()
        },
    );
    let model = read_conversions(&out.databases);
    assert!(!model.is_present());
    let reason = model.absent.clone().unwrap();
    assert!(reason.contains("not part of the reference set"));
    assert!(model.guns.is_empty() && model.melee.is_empty() && model.ammo_sets.is_empty());
    assert!(model.probe_def.is_none());
    assert!(
        model
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.absent")
    );
    assert_eq!(model, CeModel::absent(reason));
}

#[test]
fn the_mod_order_alone_makes_combat_extended_present() {
    let out = load_set(
        &rows(),
        Build {
            register_op: false,
            ce_types: false,
            patches: false,
            ..Build::default()
        },
    );
    // No type for the ammo sets and no conversion: only the package id shows Combat Extended.
    let plain = read_conversions(&out.databases);
    assert!(!plain.is_present());
    let options = CeReadOptions {
        order: Some(&out.order),
        ..CeReadOptions::default()
    };
    let model = read_conversions_with(&out.databases, &options);
    assert!(model.is_present());
    let names = model.names.clone().unwrap();
    assert_eq!(names.package_id, "CETeam.CombatExtended");
    assert_eq!(names.name, "RS Combat Extended");
    assert!(model.guns.is_empty());
}

#[test]
fn converted_guns_are_read_as_the_game_sees_them() {
    let rows = rows();
    let out = load_set(&rows, Build::default());
    let model = read_conversions(&out.databases);
    assert!(model.is_present());
    assert_eq!(model.guns.len(), 12);
    let g = model.gun("RS_CeGun03").unwrap();
    let row = &rows[3];
    assert!(close(g.stats["mass"], row.ce.mass));
    assert!(close(g.stats["bulk"], row.ce.bulk));
    assert!(close(g.stats["range"], row.ce.range));
    assert!(close(g.stats["warmup"], row.ce.warmup));
    assert!(close(g.stats["cooldown"], row.ce.cooldown));
    assert!(close(g.stats["spread"], row.ce.spread));
    assert!(close(g.stats["magazine"], f64::from(row.ce.magazine)));
    assert!(close(g.stats["reload"], row.ce.reload));
    assert!(close(g.stats["recoil"], 1.4));
    // Projectile numbers come from the first member of the ammo set.
    assert!(close(g.stats["damage"], 9.0));
    assert!(close(g.stats["ap_sharp"], 2.5));
    assert!(close(g.stats["ap_blunt"], 14.0));
    assert!(close(g.stats["speed"], 120.0));
    assert_eq!(g.ammo_set.as_deref(), Some("RS_AmmoSetA"));
    assert_eq!(g.default_projectile.as_deref(), Some("RS_CeBullet1"));
    assert_eq!(g.ai_aim_mode.as_deref(), Some("AimedShot"));
    // The accuracy stats of the vanilla gun are gone from the converted def.
    let def = out.databases.get("ThingDef", "RS_CeGun03").unwrap();
    assert!(def.node.find("statBases/AccuracyTouch").is_none());
    // The def keeps its mod and records the operation that changed it.
    assert_eq!(def.mod_idx(), Some(rimstudio_core::ids::ModIdx(0)));
    assert_eq!(def.patched_by.len(), 1);
    // Vanilla accuracy and the vanilla verb are gone; one converted verb remains.
    assert_eq!(
        def.node
            .child("verbs")
            .unwrap()
            .children_named("li")
            .count(),
        1
    );
    // Tags and classes.
    assert!(model.knows_tag("CE_AI_RS_Rifle"));
    assert!(
        !model.knows_tag("RS_Handgun"),
        "only tags with the prefix are collected"
    );
    assert_eq!(
        model.ai_class_tags,
        vec!["CE_AI_RS_Pistol", "CE_AI_RS_Rifle"]
    );
    assert_eq!(model.classes.shoot_verb, "CombatExtended.Verb_ShootCE");
    assert_eq!(model.probe_def.as_deref(), Some("RS_AmmoSetA"));
    assert!(model.projectile_in_set("RS_AmmoSetA", "RS_CeBullet1"));
    assert!(!model.projectile_in_set("RS_AmmoSetA", "RS_CeBullet2"));
    assert!(!model.projectile_in_set("RS_NoSet", "RS_CeBullet1"));
    assert_eq!(model.ammo_sets.len(), 2);
    assert!(model.stat_rules.contains_key("Mass"));
}

#[test]
fn a_converted_melee_weapon_is_read_with_its_tools_and_ratios() {
    let out = load_set(&rows(), Build::default());
    let model = read_conversions(&out.databases);
    let m = model.melee_weapon("RS_CeEdge").unwrap();
    assert_eq!(m.tools.len(), 1);
    assert_eq!(m.tools[0].capacities, vec!["Cut"]);
    assert!(close(m.stats["bulk"], 6.5));
    assert!(close(m.stats["tool_power"], 20.0));
    assert!(close(m.stats["ap_sharp_ratio"], 0.03));
    assert!(close(m.stats["ap_blunt_ratio"], 0.06));
    assert!(close(m.stats["mass"], 1.4));
}

#[test]
fn when_the_conversion_operation_is_not_registered_no_gun_is_converted() {
    let out = load_set(
        &rows(),
        Build {
            register_op: false,
            ..Build::default()
        },
    );
    let model = read_conversions(&out.databases);
    assert!(model.is_present(), "the ammo sets show Combat Extended");
    assert!(model.guns.is_empty());
    assert!(
        out.patch_report
            .events
            .iter()
            .any(|e| e.class.contains("MakeGun") && !e.result)
    );
}

#[test]
fn without_the_type_table_entries_ammo_sets_are_unknown_types() {
    let out = load_set(
        &rows(),
        Build {
            ce_types: false,
            ..Build::default()
        },
    );
    assert!(out.databases.database("CombatExtended.AmmoSetDef").is_err());
    let model = read_conversions(&out.databases);
    assert!(model.ammo_sets.is_empty());
    assert_eq!(
        model.guns.len(),
        12,
        "guns are found by their markers alone"
    );
    assert!(
        model
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.ammo-set-unresolved")
    );
    // Without the set the verb's own projectile is measured.
    assert!(close(model.gun("RS_CeGun00").unwrap().stats["damage"], 9.0));
}

#[test]
fn type_table_entries_are_added_once_and_keep_the_rest() {
    let base = table(false);
    let classes = CeClassNames::default();
    let extended = with_ce_types(&base, &classes).unwrap();
    assert_eq!(extended.len(), base.len() + ce_type_entries(&classes).len());
    assert!(extended.info("CombatExtended.AmmoSetDef").is_some());
    assert_eq!(extended.root(), base.root());
    let again = with_ce_types(&extended, &classes).unwrap();
    assert_eq!(again.len(), extended.len());
}

#[test]
fn twins_pair_converted_guns_with_their_vanilla_values() {
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows,
        Build {
            ce: false,
            ..Build::default()
        },
    );
    let options = CeReadOptions {
        vanilla: Some(&plain.databases),
        ..CeReadOptions::default()
    };
    let model = read_conversions_with(&ce.databases, &options);
    assert_eq!(model.twin_count(), 12);
    let g = model.gun("RS_CeGun05").unwrap();
    let twin = g.twin.as_ref().unwrap();
    assert!(close(twin["mass"], rows[5].vanilla.mass));
    assert!(close(twin["range"], rows[5].vanilla.range));
    assert!(close(twin["warmup"], rows[5].vanilla.warmup));
    assert!(close(twin["damage"], 10.0));
    let edge = model.melee_weapon("RS_CeEdge").unwrap();
    let t = edge.twin.as_ref().unwrap();
    assert!(close(t["mass"], 1.4));
    assert!(
        close(t["tool_power"], 13.5),
        "mean over the two vanilla tools"
    );
}

#[test]
fn a_converted_def_without_a_vanilla_counterpart_gets_a_hint() {
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows[..6],
        Build {
            ce: false,
            ..Build::default()
        },
    );
    let options = CeReadOptions {
        vanilla: Some(&plain.databases),
        ..CeReadOptions::default()
    };
    let model = read_conversions_with(&ce.databases, &options);
    assert_eq!(model.twin_count(), 6);
    let hints = model
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "ce.twin-missing")
        .count();
    assert_eq!(hints, 6);
}

#[test]
fn the_reader_output_is_deterministic() {
    let rows = rows();
    let a = read_conversions(&load_set(&rows, Build::default()).databases);
    let b = read_conversions(&load_set(&rows, Build::default()).databases);
    assert_eq!(a, b);
    let json = serde_json::to_string(&a).unwrap();
    let back: CeModel = serde_json::from_str(&json).unwrap();
    // (serde_json without float_roundtrip may change the last bit of a float, so compare the shape.)
    assert_eq!(back.guns.len(), a.guns.len());
    assert_eq!(back.ammo_sets, a.ammo_sets);
    assert_eq!(back.gun_presets.len(), a.gun_presets.len());
}

#[test]
fn presets_are_read_from_the_install_defs_and_drive_the_formulas() {
    let out = load_set(&rows(), Build::default());
    let model = read_conversions(&out.databases);
    assert_eq!(model.gun_presets.len(), 1);
    let p = &model.gun_presets[0];
    assert_eq!(p.def_name, "RS_PresetRifle");
    assert_eq!(p.names, vec!["rsrifle", "rscarbine"]);
    assert!(p.discard_designations && p.determine_caliber);
    assert_eq!(p.range_range.max, 40.0);
    assert_eq!(p.range_curve.as_ref().map(|c| c.points().len()), Some(2));
    assert!(close(p.flat_range, 40.0) && close(p.flat_warmup, 0.9));
    assert_eq!(p.burst_shot_count, 3.0);
    assert_eq!(p.recoil_amount, Some(1.5));
    assert_eq!(p.sights_efficiency, Some(0.8));
    assert_eq!(p.caliber_ranges.len(), 1);
    assert_eq!(p.special_guns[0].mag_cap, Some(12.0));
    // The preset claims a gun by its label token, and the formulas produce its values.
    let gun = GunInput {
        label: "RSRifle".into(),
        mass: Some(3.0),
        range: Some(30.0),
        ..GunInput::default()
    };
    let claim = preset_for(&gun, &model.gun_presets).unwrap();
    assert_eq!(claim.index, 0);
    let values = gun_values(&gun, p).unwrap();
    assert!(close(values.range, 50.0));
    assert!(close(values.magazine, 25.0));
    assert!(model.apparel_presets.is_empty());
}

#[test]
fn the_update_mode_block_is_read_from_a_converted_def() {
    let out = load_set(&rows(), Build::default());
    let def = out.databases.get("ThingDef", "RS_CeGun02").unwrap();
    let block = ce_block_from_def(def, &CeClassNames::default(), ValueSource::Anchor).unwrap();
    assert_eq!(block.ammo_set.as_deref(), Some("RS_AmmoSetA"));
    assert_eq!(block.default_projectile.as_deref(), Some("RS_CeBullet1"));
    assert_eq!(block.weapon_tag_class.as_deref(), Some("CE_AI_RS_Pistol"));
    assert_eq!(block.magazine_size.map(|m| m.value), Some(8));
    assert_eq!(
        block.magazine_size.map(|m| m.source),
        Some(ValueSource::Anchor)
    );
    assert!(close(block.bulk.unwrap().value, 2.0 * 2.3));
    assert!(close(block.reload_time.unwrap().value, 4.0));
    assert!(close(block.recoil_amount.unwrap().value, 1.4));
    assert!(block.tool_penetration.is_empty() || block.tool_penetration.len() == 1);
    let edge = out.databases.get("ThingDef", "RS_CeEdge").unwrap();
    let melee = ce_block_from_def(edge, &CeClassNames::default(), ValueSource::Typed).unwrap();
    assert_eq!(melee.tool_penetration.len(), 1);
    assert!(close(melee.tool_penetration[0].sharp.unwrap().value, 0.6));
    assert!(close(melee.tool_penetration[0].blunt.unwrap().value, 1.2));
    let plain = out.databases.get("ThingDef", "RS_Shot1").unwrap();
    assert!(ce_block_from_def(plain, &CeClassNames::default(), ValueSource::Typed).is_none());
}

#[test]
fn a_conversion_of_a_missing_def_fails_with_a_warning() {
    let mut rows = rows();
    rows.truncate(2);
    let mut out_rows = rows.clone();
    out_rows[1].name = "RS_NotThere".into();
    // The vanilla defs use the changed name, so build the load by hand with a wrong target.
    let ce_rows = rows;
    let mut input_defs = super::fixtures_tests::vanilla_defs(&out_rows);
    input_defs.push(vanilla::projectile("RS_Extra", 1.0, 1.0, None));
    let table = super::fixtures_tests::table(true);
    let mut input = rimstudio_defs::LoadInput::new(
        vec![rimstudio_defs::ModEntry::new(
            rimstudio_core::ids::ModIdx(0),
            "rs.vanilla",
            "RS",
        )],
        table,
    );
    let mut root = Node::new("Defs");
    for d in input_defs {
        root.push_child(d);
    }
    input.def_files.push(rimstudio_defs::DefFile {
        mod_idx: rimstudio_core::ids::ModIdx(0),
        file: rimstudio_core::ids::FileId(0),
        rel_path: "Defs/a.xml".into(),
        content: rimstudio_defs::FileContent::parsed(root),
    });
    let mut patch = Node::new("Patch");
    patch.push_child(super::fixtures_tests::makegun_op(&ce_rows[0]));
    patch.push_child(super::fixtures_tests::makegun_op(&ce_rows[1]));
    patch.push_child(
        NodeBuilder::new("Operation")
            .attr("Class", CeClassNames::default().make_gun_op)
            .build(),
    );
    input.patch_files.push(rimstudio_defs::PatchFile {
        mod_idx: rimstudio_core::ids::ModIdx(0),
        file: rimstudio_core::ids::FileId(1),
        rel_path: "Patches/a.xml".into(),
        content: rimstudio_defs::FileContent::parsed(patch),
    });
    input.custom_ops = custom_registry(&CeClassNames::default(), BTreeMap::new());
    let out = rimstudio_defs::load(input);
    let results = out.patch_report.results();
    assert_eq!(results, vec![true, false, false]);
    let codes: Vec<String> = out
        .diagnostics
        .samples
        .iter()
        .map(|d| d.code.as_str().to_owned())
        .collect();
    assert!(codes.iter().any(|c| c == "ce.makegun-missing-def"));
}

#[test]
fn the_settings_conditional_runs_on_the_users_settings() {
    let classes = CeClassNames::default();
    let build = |value: Option<bool>| {
        let mut settings = BTreeMap::new();
        if let Some(v) = value {
            settings.insert("RS_Setting".to_owned(), v);
        }
        let mut input = rimstudio_defs::LoadInput::new(
            vec![rimstudio_defs::ModEntry::new(
                rimstudio_core::ids::ModIdx(0),
                "rs.a",
                "RS",
            )],
            table(false),
        );
        let mut root = Node::new("Defs");
        for d in vanilla::base_defs().into_iter().take(1) {
            root.push_child(d);
        }
        root.push_child(
            NodeBuilder::new("ThingDef")
                .text_elem("defName", "RS_Item")
                .build(),
        );
        input.def_files.push(rimstudio_defs::DefFile {
            mod_idx: rimstudio_core::ids::ModIdx(0),
            file: rimstudio_core::ids::FileId(0),
            rel_path: "Defs/a.xml".into(),
            content: rimstudio_defs::FileContent::parsed(root),
        });
        let add = |tag: &str| {
            NodeBuilder::new("li")
                .attr("Class", "PatchOperationAdd")
                .text_elem("xpath", "Defs/ThingDef[defName=\"RS_Item\"]")
                .elem("value", |v| v.text_elem(tag, "1"))
                .build()
        };
        let mut op = NodeBuilder::new("Operation")
            .attr("Class", classes.settings_conditional_op.clone())
            .text_elem("settingName", "RS_Setting")
            .build();
        let mut yes = Node::new("match");
        yes.set_attr("Class", "PatchOperationAdd");
        yes.push_child(Node::with_text(
            "xpath",
            "Defs/ThingDef[defName=\"RS_Item\"]",
        ));
        yes.push_child(NodeBuilder::new("value").text_elem("RS_Yes", "1").build());
        let mut no = Node::new("nomatch");
        no.set_attr("Class", "PatchOperationAdd");
        no.push_child(Node::with_text(
            "xpath",
            "Defs/ThingDef[defName=\"RS_Item\"]",
        ));
        no.push_child(NodeBuilder::new("value").text_elem("RS_No", "1").build());
        op.push_child(yes);
        op.push_child(no);
        let _ = add("x");
        let mut patch = Node::new("Patch");
        patch.push_child(op);
        input.patch_files.push(rimstudio_defs::PatchFile {
            mod_idx: rimstudio_core::ids::ModIdx(0),
            file: rimstudio_core::ids::FileId(1),
            rel_path: "Patches/a.xml".into(),
            content: rimstudio_defs::FileContent::parsed(patch),
        });
        input.custom_ops = custom_registry(&classes, settings);
        rimstudio_defs::load(input)
    };
    let on = build(Some(true));
    let item = on.databases.get("ThingDef", "RS_Item").unwrap();
    assert!(item.node.child("RS_Yes").is_some() && item.node.child("RS_No").is_none());
    let off = build(Some(false));
    assert!(
        off.databases
            .get("ThingDef", "RS_Item")
            .unwrap()
            .node
            .child("RS_No")
            .is_some()
    );
    let missing = build(None);
    assert_eq!(missing.patch_report.results(), vec![false]);
    let _ = GunNumbers::default();
}

fn record(node: Node) -> rimstudio_defs::DefRecord {
    rimstudio_defs::DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: node.child_text("defName").unwrap_or("RS_X").to_owned(),
        origin: None,
        node,
        parents: Vec::new(),
        patched_by: Vec::new(),
    }
}

fn converted_tool_node(extra: impl FnOnce(NodeBuilder) -> NodeBuilder) -> Node {
    extra(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Thing")
            .elem("weaponTags", |w| w.li("RS_Melee"))
            .elem("tools", |t| {
                t.elem("li", |li| {
                    li.attr("Class", CeClassNames::default().tool)
                        .text_elem("label", "claw")
                        .text_elem("power", "5")
                        .text_elem("cooldownTime", "2")
                })
            }),
    )
    .build()
}

#[test]
fn creatures_and_buildings_with_converted_tools_are_not_melee_weapons() {
    let ex = crate::reader::Exclusions::default();
    let animal = record(converted_tool_node(|b| b.elem("race", |r| r)));
    assert!(conversions::read_melee(&animal, &ex).is_none());
    let building = record(converted_tool_node(|b| b.elem("building", |r| r)));
    assert!(conversions::read_melee(&building, &ex).is_none());
    let sword = record(converted_tool_node(|b| b));
    let m = conversions::read_melee(&sword, &ex).unwrap();
    assert!(m.excluded.is_none());
    assert_eq!(m.tools.len(), 1);
}

#[test]
fn turret_guns_launchers_and_grenades_stay_out_of_the_class_pools() {
    use crate::reader::SkipReason;
    let ex = crate::reader::Exclusions::default();
    let hidden = record(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Hidden")
            .text_elem("menuHidden", "true")
            .build(),
    );
    let plain = record(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_Plain")
            .build(),
    );
    let tags = vec!["CE_AI_RS_Rifle".to_owned()];
    assert_eq!(
        conversions::exclusion_of(&hidden, &ex, None, false, &tags),
        Some(SkipReason::MenuHidden)
    );
    assert_eq!(
        conversions::exclusion_of(&plain, &ex, None, false, &tags),
        None
    );
    assert_eq!(
        conversions::exclusion_of(&plain, &ex, Some("RS_Mod.Verb_ShootOneUse"), false, &tags),
        Some(SkipReason::VerbClass("RS_Mod.Verb_ShootOneUse".into()))
    );
    assert_eq!(
        conversions::exclusion_of(&plain, &ex, None, true, &tags),
        Some(SkipReason::Explosive)
    );
    let turret_tags = vec!["RS_TurretGun".to_owned()];
    assert_eq!(
        conversions::exclusion_of(&plain, &ex, None, false, &turret_tags),
        Some(SkipReason::Tag("RS_TurretGun".into()))
    );
    // The pools skip a gun with a reason, while the model keeps it.
    let out = load_set(&rows(), Build::default());
    let mut model = read_conversions(&out.databases);
    model.guns[0].excluded = Some(SkipReason::DestroyOnDrop);
    let pool = crate::ce::classes::build_pool(
        &model,
        crate::classes::ItemKind::Ranged,
        &crate::ce::classes::CeClassOptions::default(),
    )
    .unwrap();
    assert_eq!(model.guns.len(), 12);
    assert_eq!(pool.len(), 11);
}

#[test]
fn the_conversion_operations_of_an_unregistered_load_are_listed_with_their_parameters() {
    let rows = rows();
    let out = load_set(
        &rows,
        Build {
            register_op: false,
            ..Build::default()
        },
    );
    let records = makegun_records(&out.patch_report, &CeClassNames::default());
    assert_eq!(records.len(), 12);
    assert!(
        records.iter().all(|r| !r.applied),
        "an unknown class fails like the base operation"
    );
    let first = &records[0];
    assert_eq!(first.spec.def_name, "RS_CeGun00");
    assert_eq!(first.spec.properties.len(), 8);
    assert!(first.spec.ammo_user.is_some() && first.spec.fire_modes.is_some());
    assert_eq!(first.mod_idx, rimstudio_core::ids::ModIdx(1));
    // A registered load handles the operation itself, so no raw element is kept.
    let handled = load_set(&rows, Build::default());
    assert!(makegun_records(&handled.patch_report, &CeClassNames::default()).is_empty());
}
