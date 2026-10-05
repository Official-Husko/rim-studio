//! `lint ce` over hand written patch files, single files and hostile files of a fictional mod.
//!
//! Nothing here reads a real game install; every name starts with `RS_` and every number is invented.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::path::Path;

use common::Env;
use serde_json::Value;

const HAND: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Patch>
  <Operation Class="PatchOperationFindMod">
    <mods><li>ceteam.combatextended</li></mods>
    <match Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="RS_Old"]/foo</xpath></match>
  </Operation>
</Patch>
"#;

const CLEAN: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Patch>
  <Operation Class="PatchOperationAdd">
    <xpath>Defs/ThingDef[defName="RS_Old"]</xpath>
    <value><label>old</label></value>
  </Operation>
</Patch>
"#;

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn file<'a>(result: &'a Value, path: &str) -> &'a Value {
    result["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == path)
        .unwrap_or_else(|| panic!("{path} missing in {result}"))
}

#[test]
fn a_mod_folder_is_checked_file_by_file_and_errors_exit_with_one() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder(
        "RS_Lint",
        &[
            ("Patches/hand.xml", HAND),
            ("Patches/clean.xml", CLEAN),
            ("Defs/RS_Defs.xml", "<Defs/>"),
        ],
    );
    let p = path_str(&proj);
    let json = env.run(&["--json", "lint", "ce", p]).expect(1).json();
    assert!(json["errors"].as_u64().unwrap() >= 1);
    let result = &json["results"][0];
    assert_eq!(result["files"].as_array().unwrap().len(), 2);
    let hand = file(result, "Patches/hand.xml");
    let finding = hand["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "CEP001")
        .unwrap();
    assert_eq!(finding["operation"], 1);
    assert!(
        finding["explanation"]
            .as_str()
            .unwrap()
            .contains("package id")
    );
    assert!(
        file(result, "Patches/clean.xml")["findings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(result["ceData"], true);
    let text = env.run(&["lint", "ce", "--explain", p]).expect(1).stdout();
    assert!(text.contains("CEP001 operation 1"), "{text}");
    assert!(text.contains("display name"), "{text}");
    assert!(text.contains("not checked CEP010"), "{text}");
}

#[test]
fn a_single_file_is_checked_inside_its_mod() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder(
        "RS_LintOne",
        &[("Patches/hand.xml", HAND), ("Patches/clean.xml", CLEAN)],
    );
    let clean = proj.join("Patches/clean.xml");
    let json = env
        .run(&["--json", "lint", "ce", path_str(&clean)])
        .expect(0)
        .json();
    assert_eq!(json["errors"], 0);
    assert_eq!(json["results"][0]["files"].as_array().unwrap().len(), 1);
    let hand = proj.join("Patches/hand.xml");
    env.run(&["lint", "ce", path_str(&hand)]).expect(1);
}

#[test]
fn a_file_outside_any_mod_is_a_usage_error() {
    let env = Env::new(true);
    env.select_install();
    let loose = env.path("loose").join("a.xml");
    std::fs::create_dir_all(loose.parent().unwrap()).unwrap();
    std::fs::write(&loose, CLEAN).unwrap();
    env.run(&["lint", "ce", path_str(&loose)]).expect(2);
}

#[test]
fn hostile_files_are_findings_and_never_crash_the_command() {
    let env = Env::new(true);
    env.select_install();
    let big = format!("<Patch>{}</Patch>", " ".repeat(2_100_000));
    let proj = env.mod_folder(
        "RS_LintHostile",
        &[
            ("Patches/not_xml.xml", "<<< not xml"),
            (
                "Patches/dtd.xml",
                "<?xml version=\"1.0\"?><!DOCTYPE Patch [<!ENTITY a \"b\">]><Patch/>",
            ),
            ("Patches/huge.xml", &big),
            ("Patches/empty.xml", ""),
        ],
    );
    let json = env
        .run(&["--json", "lint", "ce", path_str(&proj)])
        .expect(1)
        .json();
    let result = &json["results"][0];
    assert_eq!(
        file(result, "Patches/not_xml.xml")["status"],
        "parse-failed"
    );
    assert_eq!(file(result, "Patches/dtd.xml")["status"], "doctype");
    assert_eq!(file(result, "Patches/huge.xml")["status"], "too-large");
    assert_eq!(file(result, "Patches/empty.xml")["status"], "parse-failed");
    assert!(json["errors"].as_u64().unwrap() >= 4);
}

#[test]
fn without_combat_extended_the_command_still_runs_and_names_the_unchecked_rules() {
    let env = Env::new(false);
    env.select_install();
    let proj = env.mod_folder("RS_LintPlain", &[("Patches/clean.xml", CLEAN)]);
    let json = env
        .run(&["--json", "lint", "ce", path_str(&proj)])
        .expect(0)
        .json();
    let result = &json["results"][0];
    assert_eq!(result["ceData"], false);
    let ids: Vec<&str> = result["notChecked"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["ruleId"].as_str())
        .collect();
    assert!(ids.contains(&"CEP013"), "{ids:?}");
    let text = env.run(&["lint", "ce", path_str(&proj)]).expect(0).stdout();
    assert!(text.contains("0 errors"), "{text}");
    assert!(text.contains("no Combat Extended data is loaded"), "{text}");
    env.run(&["lint", "ce"]).expect(2);
}
