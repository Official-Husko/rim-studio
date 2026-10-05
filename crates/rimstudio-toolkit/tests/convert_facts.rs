//! The weapon tags, classes and numbers of the convert scan candidates, over a fictional project.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_project;

use common_project::{fixture, project};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_ipc_types::designer::{ConvertScanDto, DesignerConvertScanRequest};
use rimstudio_toolkit::designer::convert_scan;

const DEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Defs>
  <ThingDef>
    <defName>RS_FactGun</defName>
    <label>fact gun</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases>
      <Mass>3.5</Mass>
      <MarketValue>450</MarketValue>
      <RangedWeapon_Cooldown>1.75</RangedWeapon_Cooldown>
    </statBases>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_Shot00</defaultProjectile>
        <warmupTime>1.25</warmupTime>
        <range>25.9</range>
        <burstShotCount>3</burstShotCount>
      </li>
    </verbs>
    <weaponTags><li>RS_Rifle</li><li>RS_Extra</li></weaponTags>
    <weaponClasses><li>RS_Ranged</li><li>RS_Short</li></weaponClasses>
  </ThingDef>
  <ThingDef>
    <defName>RS_FactBare</defName>
    <label>fact bare</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_NoSuchShot</defaultProjectile>
        <range>x</range>
      </li>
    </verbs>
  </ThingDef>
  <ThingDef>
    <defName>RS_FactBlade</defName>
    <label>fact blade</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases><Mass>1.2</Mass><MarketValue>80</MarketValue></statBases>
    <weaponTags><li>RS_Melee</li></weaponTags>
    <tools>
      <li><label>handle</label><capacities><li>Blunt</li></capacities><power>7</power><cooldownTime>1.5</cooldownTime></li>
      <li><label>edge</label><capacities><li>Cut</li></capacities><power>10</power><cooldownTime>2</cooldownTime></li>
      <li><label>tip</label><capacities><li>Stab</li></capacities><power>10</power><cooldownTime>2.5</cooldownTime></li>
    </tools>
  </ThingDef>
  <ThingDef Name="RS_FactBase" Abstract="True">
    <weaponTags><li>RS_Base</li></weaponTags>
    <verbs><li><verbClass>Verb_Shoot</verbClass><defaultProjectile>RS_Shot00</defaultProjectile></li></verbs>
  </ThingDef>
</Defs>
"#;

fn scan() -> ConvertScanDto {
    let f = fixture(true);
    let p = project(&f, &[("Defs/RS_Facts.xml", DEFS)]);
    convert_scan(
        &f.ctx,
        DesignerConvertScanRequest {
            project_id: p.id.clone(),
            include_converted: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap()
}

#[test]
fn a_gun_carries_its_tags_classes_file_and_numbers() {
    let scan = scan();
    let gun = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FactGun")
        .unwrap();
    assert_eq!(gun.tags, ["RS_Rifle", "RS_Extra"]);
    assert_eq!(gun.weapon_classes, ["RS_Ranged", "RS_Short"]);
    assert_eq!(gun.file.as_deref(), Some("Defs/RS_Facts.xml"));
    let v = gun.vanilla.as_ref().unwrap();
    assert_eq!(v.damage, Some(8.0));
    assert_eq!(v.range, Some(25.9));
    assert_eq!(v.cooldown, Some(1.75));
    assert_eq!(v.warmup, Some(1.25));
    assert_eq!(v.mass, Some(3.5));
    assert_eq!(v.burst, Some(3));
    assert_eq!(v.market_value, Some(450.0));
}

#[test]
fn a_number_the_definition_does_not_give_is_absent() {
    let scan = scan();
    let bare = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FactBare")
        .unwrap();
    assert!(bare.tags.is_empty());
    assert!(bare.weapon_classes.is_empty());
    let v = bare.vanilla.as_ref().unwrap();
    assert_eq!(v.damage, None, "the projectile is not defined");
    assert_eq!(v.range, None, "the range is not a number");
    assert_eq!(v.mass, None);
    assert_eq!(v.market_value, None);
    assert_eq!(v.burst, None);
}

#[test]
fn a_melee_weapon_reads_its_strongest_tool() {
    let scan = scan();
    let blade = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FactBlade")
        .unwrap();
    let v = blade.vanilla.as_ref().unwrap();
    assert_eq!(v.damage, Some(10.0));
    assert_eq!(v.cooldown, Some(2.0), "the first of equally strong tools");
    assert_eq!(v.range, None);
    assert_eq!(v.warmup, None);
    assert_eq!(v.burst, None);
    assert_eq!(v.mass, Some(1.2));
    assert_eq!(v.market_value, Some(80.0));
    assert_eq!(blade.tags, ["RS_Melee"]);
}

#[test]
fn an_abstract_base_has_no_numbers() {
    let scan = scan();
    let base = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FactBase")
        .unwrap();
    assert!(base.vanilla.is_none());
    assert!(base.tags.is_empty());
}

#[test]
fn the_json_uses_camel_case_and_omits_empty_lists() {
    let scan = scan();
    let text = serde_json::to_string(&scan).unwrap();
    assert!(text.contains("\"weaponClasses\""));
    assert!(text.contains("\"marketValue\":450.0"));
    let base = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FactBase")
        .unwrap();
    let json = serde_json::to_value(base).unwrap();
    assert!(json.get("tags").is_none());
    assert!(json.get("vanilla").is_none());
}
