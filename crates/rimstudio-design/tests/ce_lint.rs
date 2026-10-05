//! The lint rules CEP001 to CEP022: one positive and one negative case per rule, the "not checked" reports
//! without Combat Extended data, the report fields and the order.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common_ce;

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::lint::{self, LintContext, LintFile, REGISTRY};
use rimstudio_design::ce::reader::{CeClassNames, CeModel};

use common_ce::{CLASS_TAG, ce_model, make_gun, op, patch};

fn count(findings: &[Diagnostic], rule: &str) -> usize {
    findings
        .iter()
        .filter(|d| d.args.get("ruleId").is_some_and(|r| r == rule))
        .count()
}

fn run_one(root: Node, model: &CeModel, ctx: &LintContext) -> Vec<Diagnostic> {
    lint::run(&[root], model, ctx)
}

fn find_mod(entries: &[&str]) -> Node {
    let mut mods = Node::new("mods");
    for e in entries {
        mods.push_child(Node::with_text("li", *e));
    }
    patch(vec![op("PatchOperationFindMod").child(mods).build()])
}

fn gated_ctx(path: &str) -> LintContext {
    let load_folders = NodeBuilder::new("loadFolders")
        .elem("v1.6", |b| {
            b.li("/").child({
                let mut e = Node::with_text("li", "CE");
                e.set_attr("IfModActive", "RS.CombatExt");
                e
            })
        })
        .build();
    LintContext {
        paths: vec![path.to_owned()],
        load_folders: Some(load_folders),
        ..LintContext::default()
    }
}

fn replace_tools(tools: Node) -> Node {
    let mut value = Node::new("value");
    value.push_child(tools);
    patch(vec![
        op("PatchOperationReplace")
            .text_elem("xpath", "Defs/ThingDef[defName=\"RS_X\"]/tools")
            .child(value)
            .build(),
    ])
}

fn tool_li(class: Option<&str>, with_penetration: bool) -> Node {
    let mut b = NodeBuilder::new("li");
    if let Some(c) = class {
        b = b.attr("Class", c);
    }
    b = b.text_elem("label", "blade");
    if with_penetration {
        b = b.text_elem("armorPenetrationBlunt", "0.5");
    }
    b.build()
}

fn tools_of(li: Node) -> Node {
    let mut t = Node::new("tools");
    t.push_child(li);
    t
}

// ---------------------------------------------------------------------------------------------------------

#[test]
fn cep001_a_find_mod_entry_that_looks_like_a_package_id() {
    let model = ce_model();
    let ctx = LintContext::default();
    assert_eq!(
        count(
            &run_one(find_mod(&["Some.Package_Id"]), &model, &ctx),
            "CEP001"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(find_mod(&["RS Combat Ext"]), &model, &ctx),
            "CEP001"
        ),
        0
    );
}

#[test]
fn cep002_a_find_mod_entry_that_misspells_the_mod_name() {
    let model = ce_model();
    let ctx = LintContext::default();
    assert_eq!(
        count(
            &run_one(find_mod(&["RS combat ext"]), &model, &ctx),
            "CEP002"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(find_mod(&["RS Combat Ext"]), &model, &ctx),
            "CEP002"
        ),
        0
    );
    assert_eq!(
        count(
            &run_one(find_mod(&["Unrelated Mod"]), &model, &ctx),
            "CEP002"
        ),
        0
    );
}

#[test]
fn cep003_a_find_mod_entry_with_white_space() {
    let model = ce_model();
    let ctx = LintContext::default();
    assert_eq!(
        count(
            &run_one(find_mod(&[" RS Combat Ext "]), &model, &ctx),
            "CEP003"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(find_mod(&["RS Combat Ext"]), &model, &ctx),
            "CEP003"
        ),
        0
    );
}

#[test]
fn cep004_a_ce_class_in_a_file_that_loads_without_ce() {
    let model = ce_model();
    let file = patch(vec![make_gun("RS_X").build()]);
    assert_eq!(
        count(
            &run_one(file.clone(), &model, &LintContext::default()),
            "CEP004"
        ),
        1
    );
    // In the folder that LoadFolders.xml gates on the package id: no finding.
    let gated = gated_ctx("CE/Patches/a.xml");
    assert_eq!(count(&run_one(file.clone(), &model, &gated), "CEP004"), 0);
    // In the root folder (not gated): a finding.
    let root = gated_ctx("Patches/a.xml");
    assert_eq!(count(&run_one(file, &model, &root), "CEP004"), 1);
}

#[test]
fn cep005_may_require_on_an_operation() {
    let model = ce_model();
    let ctx = LintContext::default();
    let bad = patch(vec![
        op("PatchOperationRemove")
            .attr("MayRequire", "rs.other")
            .text_elem("xpath", "Defs/ThingDef[defName=\"RS_X\"]")
            .build(),
    ]);
    assert_eq!(count(&run_one(bad, &model, &ctx), "CEP005"), 1);
    // On a list item of a sequence it is honoured by the game.
    let sequence = op("PatchOperationSequence")
        .elem("operations", |o| {
            o.elem("li", |li| {
                li.attr("Class", "PatchOperationRemove")
                    .attr("MayRequire", "rs.other")
                    .text_elem("xpath", "Defs/ThingDef[defName=\"RS_X\"]")
            })
        })
        .build();
    assert_eq!(
        count(&run_one(patch(vec![sequence]), &model, &ctx), "CEP005"),
        0
    );
}

#[test]
fn cep007_the_same_def_converted_twice() {
    let model = ce_model();
    let ctx = LintContext::default();
    let twice = patch(vec![make_gun("RS_X").build(), make_gun("RS_X").build()]);
    assert_eq!(count(&run_one(twice, &model, &ctx), "CEP007"), 1);
    let two_defs = patch(vec![make_gun("RS_X").build(), make_gun("RS_Y").build()]);
    assert_eq!(count(&run_one(two_defs, &model, &ctx), "CEP007"), 0);
}

#[test]
fn cep008_an_incomplete_gun_conversion() {
    let model = ce_model();
    let ctx = LintContext::default();
    let classes = CeClassNames::default();
    let incomplete = op(&classes.make_gun_op)
        .text_elem("defName", "RS_X")
        .elem("Properties", |p| {
            p.text_elem("verbClass", &classes.shoot_verb)
        })
        .build();
    let findings = run_one(patch(vec![incomplete]), &model, &ctx);
    // defaultProjectile, AmmoUser and FireModes are missing.
    assert_eq!(count(&findings, "CEP008"), 3, "{findings:?}");
    assert_eq!(
        count(
            &run_one(patch(vec![make_gun("RS_X").build()]), &model, &ctx),
            "CEP008"
        ),
        0
    );
    // A one use verb needs neither an ammo user nor fire modes.
    let one_use = op(&classes.make_gun_op)
        .text_elem("defName", "RS_X")
        .elem("Properties", |p| {
            p.text_elem("verbClass", format!("{}OneUse", classes.shoot_verb))
                .text_elem("defaultProjectile", "RS_Bullet_CE")
        })
        .build();
    assert_eq!(
        count(&run_one(patch(vec![one_use]), &model, &ctx), "CEP008"),
        0
    );
}

#[test]
fn cep009_a_verb_class_that_is_not_a_ce_class() {
    let model = ce_model();
    let ctx = LintContext::default();
    let classes = CeClassNames::default();
    let vanilla_verb = op(&classes.make_gun_op)
        .text_elem("defName", "RS_X")
        .elem("Properties", |p| {
            p.text_elem("verbClass", "Verb_Shoot")
                .text_elem("defaultProjectile", "RS_Bullet_CE")
        })
        .elem("AmmoUser", |a| {
            a.text_elem("magazineSize", "5")
                .text_elem("ammoSet", "RS_AmmoSet")
        })
        .elem("FireModes", |f| f)
        .build();
    assert_eq!(
        count(&run_one(patch(vec![vanilla_verb]), &model, &ctx), "CEP009"),
        1
    );
    assert_eq!(
        count(
            &run_one(patch(vec![make_gun("RS_X").build()]), &model, &ctx),
            "CEP009"
        ),
        0
    );
}

#[test]
fn cep010_a_class_that_is_not_a_type_of_the_installed_ce() {
    let model = ce_model();
    let file = patch(vec![make_gun("RS_X").build()]);
    let class = CeClassNames::default();
    let known: BTreeSet<String> = [class.make_gun_op.clone()].into_iter().collect();
    let mut ctx = LintContext {
        ce_types: Some(known),
        ..LintContext::default()
    };
    assert_eq!(count(&run_one(file.clone(), &model, &ctx), "CEP010"), 0);
    ctx.ce_types = Some(BTreeSet::new());
    assert_eq!(count(&run_one(file.clone(), &model, &ctx), "CEP010"), 1);
    // Without a table the rule is reported as not checked.
    let findings = run_one(file, &model, &LintContext::default());
    assert_eq!(count(&findings, "CEP010"), 1);
    assert!(findings.iter().any(|d| d.code.as_str() == "ce.not-checked"
        && d.args.get("ruleId").is_some_and(|r| r == "CEP010")));
}

#[test]
fn cep011_a_converted_tool_without_penetration() {
    let model = ce_model();
    let ctx = LintContext::default();
    let class = CeClassNames::default().tool;
    let bad = replace_tools(tools_of(tool_li(Some(&class), false)));
    assert_eq!(count(&run_one(bad, &model, &ctx), "CEP011"), 1);
    let good = replace_tools(tools_of(tool_li(Some(&class), true)));
    assert_eq!(count(&run_one(good, &model, &ctx), "CEP011"), 0);
}

#[test]
fn cep012_replaced_tools_without_the_ce_class() {
    let model = ce_model();
    let ctx = LintContext::default();
    let bad = replace_tools(tools_of(tool_li(None, true)));
    assert_eq!(count(&run_one(bad, &model, &ctx), "CEP012"), 1);
    let class = CeClassNames::default().tool;
    let good = replace_tools(tools_of(tool_li(Some(&class), true)));
    assert_eq!(count(&run_one(good, &model, &ctx), "CEP012"), 0);
}

fn gun_with(set: &str, projectile: &str) -> Node {
    let classes = CeClassNames::default();
    patch(vec![
        op(&classes.make_gun_op)
            .text_elem("defName", "RS_X")
            .elem("Properties", |p| {
                p.text_elem("verbClass", &classes.shoot_verb)
                    .text_elem("defaultProjectile", projectile)
            })
            .elem("AmmoUser", |a| {
                a.text_elem("magazineSize", "5").text_elem("ammoSet", set)
            })
            .elem("FireModes", |f| f)
            .build(),
    ])
}

#[test]
fn cep013_an_ammo_set_that_is_not_defined() {
    let model = ce_model();
    let ctx = LintContext::default();
    assert_eq!(
        count(
            &run_one(gun_with("RS_Missing", "RS_Bullet_CE"), &model, &ctx),
            "CEP013"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(gun_with("RS_AmmoSet", "RS_Bullet_CE"), &model, &ctx),
            "CEP013"
        ),
        0
    );
    let known = LintContext {
        known_defs: ["RS_Missing".to_owned()].into_iter().collect(),
        ..LintContext::default()
    };
    assert_eq!(
        count(
            &run_one(gun_with("RS_Missing", "RS_Bullet_CE"), &model, &known),
            "CEP013"
        ),
        0
    );
}

#[test]
fn cep014_a_projectile_that_is_not_defined() {
    let model = ce_model();
    let ctx = LintContext::default();
    assert_eq!(
        count(
            &run_one(gun_with("RS_AmmoSet", "RS_NoBullet"), &model, &ctx),
            "CEP014"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(gun_with("RS_AmmoSet", "RS_Bullet_Other"), &model, &ctx),
            "CEP014"
        ),
        0
    );
    let known = LintContext {
        known_defs: ["RS_NoBullet".to_owned()].into_iter().collect(),
        ..LintContext::default()
    };
    assert_eq!(
        count(
            &run_one(gun_with("RS_AmmoSet", "RS_NoBullet"), &model, &known),
            "CEP014"
        ),
        0
    );
}

#[test]
fn cep015_a_field_that_is_not_known_to_its_section() {
    let model = ce_model();
    let file = patch(vec![make_gun("RS_X").build()]);
    let section =
        |names: &[&str]| -> BTreeSet<String> { names.iter().map(|n| (*n).to_owned()).collect() };
    let mut known: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    known.insert(
        "Properties".into(),
        section(&[
            "verbClass",
            "hasStandardCommand",
            "defaultProjectile",
            "range",
        ]),
    );
    known.insert("AmmoUser".into(), section(&["magazineSize", "reloadTime"]));
    known.insert("FireModes".into(), section(&["aiAimMode"]));
    let ctx = LintContext {
        known_fields: Some(known.clone()),
        ..LintContext::default()
    };
    // ammoSet is not in the (fictional) AmmoUser table.
    assert_eq!(count(&run_one(file.clone(), &model, &ctx), "CEP015"), 1);
    known.insert(
        "AmmoUser".into(),
        section(&["magazineSize", "reloadTime", "ammoSet"]),
    );
    let ctx = LintContext {
        known_fields: Some(known),
        ..LintContext::default()
    };
    let findings = run_one(file, &model, &ctx);
    assert_eq!(count(&findings, "CEP015"), 0, "{findings:?}");
}

#[test]
fn cep016_a_ce_tag_that_is_unknown_to_the_installed_ce() {
    let mut model = ce_model();
    model.weapon_tags.push("CE_Known".into());
    let ctx = LintContext::default();
    let with_tag = |tag: &str| {
        patch(vec![
            op("PatchOperationAdd")
                .text_elem("xpath", "Defs/ThingDef[defName=\"RS_X\"]/weaponTags")
                .elem("value", |v| v.li(tag))
                .build(),
        ])
    };
    assert_eq!(
        count(&run_one(with_tag("CE_Unknown"), &model, &ctx), "CEP016"),
        1
    );
    assert_eq!(
        count(&run_one(with_tag("CE_Known"), &model, &ctx), "CEP016"),
        0
    );
    // Inside a gun conversion too.
    let classes = CeClassNames::default();
    let gun = op(&classes.make_gun_op)
        .text_elem("defName", "RS_X")
        .elem("weaponTags", |t| t.li("CE_Unknown"))
        .build();
    assert_eq!(count(&run_one(patch(vec![gun]), &model, &ctx), "CEP016"), 1);
}

#[test]
fn cep017_a_file_that_is_not_a_patch_file() {
    let model = ce_model();
    let ctx = LintContext::default();
    let failed = lint::run_files(
        &[LintFile::failed(None, "unexpected end of file")],
        &model,
        &ctx,
    );
    assert_eq!(count(&failed, "CEP017"), 1);
    assert_eq!(
        count(&run_one(Node::new("Defs"), &model, &ctx), "CEP017"),
        1
    );
    let wrong_child = patch(vec![Node::new("NotAnOperation")]);
    assert_eq!(count(&run_one(wrong_child, &model, &ctx), "CEP017"), 1);
    assert_eq!(
        count(
            &run_one(patch(vec![make_gun("RS_X").build()]), &model, &ctx),
            "CEP017"
        ),
        0
    );
}

#[test]
fn cep018_a_patch_file_in_a_folder_that_is_never_loaded() {
    let model = ce_model();
    let file = patch(vec![make_gun("RS_X").build()]);
    let loaded = gated_ctx("CE/Patches/a.xml");
    assert_eq!(count(&run_one(file.clone(), &model, &loaded), "CEP018"), 0);
    let not_loaded = gated_ctx("Other/Patches/a.xml");
    assert_eq!(
        count(&run_one(file.clone(), &model, &not_loaded), "CEP018"),
        1
    );
    // Without a LoadFolders.xml the rule has nothing to compare with.
    let ctx = LintContext {
        paths: vec!["Other/Patches/a.xml".into()],
        ..LintContext::default()
    };
    assert_eq!(count(&run_one(file, &model, &ctx), "CEP018"), 0);
}

#[test]
fn cep019_an_if_mod_active_id_that_cannot_match() {
    let model = ce_model();
    let build = |id: &str| {
        NodeBuilder::new("loadFolders")
            .elem("v1.6", |b| {
                b.li("/").child({
                    let mut e = Node::with_text("li", "CE");
                    e.set_attr("IfModActive", id);
                    e
                })
            })
            .build()
    };
    let bad = LintContext {
        load_folders: Some(build("RS.CombatExt_copy")),
        ..LintContext::default()
    };
    assert_eq!(count(&lint::run(&[], &model, &bad), "CEP019"), 1);
    let steam = LintContext {
        load_folders: Some(build("RS.CombatExt_steam")),
        ..LintContext::default()
    };
    assert_eq!(count(&lint::run(&[], &model, &steam), "CEP019"), 1);
    let good = LintContext {
        load_folders: Some(build("rs.combatext")),
        ..LintContext::default()
    };
    assert_eq!(count(&lint::run(&[], &model, &good), "CEP019"), 0);
}

#[test]
fn cep020_a_folder_or_extension_in_a_different_case() {
    let model = ce_model();
    let file = patch(vec![make_gun("RS_X").build()]);
    let at = |path: &str| LintContext {
        paths: vec![path.to_owned()],
        ..LintContext::default()
    };
    assert_eq!(
        count(
            &run_one(file.clone(), &model, &at("CE/patches/a.xml")),
            "CEP020"
        ),
        1
    );
    assert_eq!(
        count(
            &run_one(file.clone(), &model, &at("CE/Patches/a.XML")),
            "CEP020"
        ),
        1
    );
    assert_eq!(
        count(&run_one(file, &model, &at("CE/Patches/a.xml")), "CEP020"),
        0
    );
}

#[test]
fn cep021_an_exact_duplicate_operation() {
    let model = ce_model();
    let ctx = LintContext::default();
    let remove = |def: &str| {
        op("PatchOperationRemove")
            .text_elem(
                "xpath",
                format!("Defs/ThingDef[defName=\"{def}\"]/costList"),
            )
            .build()
    };
    let dup = patch(vec![remove("RS_X"), remove("RS_X")]);
    assert_eq!(count(&run_one(dup, &model, &ctx), "CEP021"), 1);
    let different = patch(vec![remove("RS_X"), remove("RS_Y")]);
    assert_eq!(count(&run_one(different, &model, &ctx), "CEP021"), 0);
    // Across files of the same mod too.
    let one = patch(vec![remove("RS_X")]);
    assert_eq!(
        count(&lint::run(&[one.clone(), one], &model, &ctx), "CEP021"),
        1
    );
}

#[test]
fn cep022_a_malformed_xpath() {
    let model = ce_model();
    let ctx = LintContext::default();
    let remove = |xpath: &str| {
        patch(vec![
            op("PatchOperationRemove").text_elem("xpath", xpath).build(),
        ])
    };
    for bad in [
        "ThingDef[defName=\"RS_X\"]",
        "Defs/ThingDef[defName=\"RS_X\"",
        "Defs/ThingDef[\"RS_X\"]",
    ] {
        assert_eq!(
            count(&run_one(remove(bad), &model, &ctx), "CEP022"),
            1,
            "{bad}"
        );
    }
    assert_eq!(
        count(
            &run_one(
                remove("Defs/ThingDef[defName=\"RS_X\"]/costList"),
                &model,
                &ctx
            ),
            "CEP022"
        ),
        0
    );
}

// ---------------------------------------------------------------------------------------------------------

#[test]
fn without_ce_the_data_rules_report_not_checked_and_the_others_run() {
    let absent = CeModel::absent("not installed");
    let file = patch(vec![make_gun("RS_X").build(), make_gun("RS_X").build()]);
    let findings = run_one(file, &absent, &LintContext::default());
    let not_checked: BTreeSet<String> = findings
        .iter()
        .filter(|d| d.code.as_str() == "ce.not-checked")
        .filter_map(|d| d.args.get("ruleId").cloned())
        .collect();
    let want: BTreeSet<String> = ["CEP010", "CEP013", "CEP014", "CEP015", "CEP016"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(not_checked, want);
    // The structural rules still ran: the repeated conversion and the ungated class.
    assert_eq!(count(&findings, "CEP007"), 1);
    assert_eq!(count(&findings, "CEP004"), 2);
    // No data rule fired.
    for rule in ["CEP013", "CEP014", "CEP016"] {
        assert!(
            findings
                .iter()
                .filter(|d| d.args.get("ruleId").is_some_and(|r| r == rule))
                .all(|d| d.code.as_str() == "ce.not-checked")
        );
    }
}

#[test]
fn findings_carry_the_report_fields_and_come_in_a_stable_order() {
    let model = ce_model();
    let ctx = LintContext {
        paths: vec!["Patches/a.xml".into(), "Patches/b.xml".into()],
        ..LintContext::default()
    };
    let files = [
        patch(vec![
            find_mod(&["Some.Id"]).elements().next().unwrap().clone(),
            make_gun("RS_A").build(),
        ]),
        patch(vec![make_gun("RS_B").build()]),
    ];
    let all = lint::run(&files, &model, &ctx);
    assert_eq!(all, lint::run(&files, &model, &ctx));
    // The reports about missing data come first and belong to no file.
    let first: Vec<Diagnostic> = all
        .into_iter()
        .filter(|d| d.code.as_str() != "ce.not-checked")
        .collect();
    assert!(!first.is_empty());
    for d in &first {
        assert!(d.args.contains_key("ruleId"));
        assert!(d.args.contains_key("field"));
        assert!(d.args.contains_key("path"));
        assert!(d.code.as_str().starts_with("ce.cep"));
    }
    // File a before file b, operation 1 before operation 2.
    let paths: Vec<&str> = first.iter().map(|d| d.args["path"].as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted);
    assert!(first[0].args["field"].starts_with("/Patch/Operation[1]"));
}

#[test]
fn every_rule_is_registered_once_with_the_documented_severity() {
    assert_eq!(REGISTRY.len(), 21);
    let severity = |rule: &str| {
        REGISTRY
            .iter()
            .find(|c| c.code.starts_with(&format!("ce.{}", rule.to_lowercase())))
            .unwrap()
            .severity
    };
    for rule in [
        "CEP005", "CEP009", "CEP011", "CEP012", "CEP015", "CEP016", "CEP020", "CEP021",
    ] {
        assert_eq!(severity(rule), Severity::Warning, "{rule}");
    }
    for rule in [
        "CEP001", "CEP004", "CEP007", "CEP013", "CEP017", "CEP018", "CEP022",
    ] {
        assert_eq!(severity(rule), Severity::Error, "{rule}");
    }
    assert!(!REGISTRY.iter().any(|c| c.code.starts_with("ce.cep006")));
    let _ = CLASS_TAG;
}
