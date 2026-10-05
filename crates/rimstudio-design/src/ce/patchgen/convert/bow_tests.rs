//! Tests that bows and crossbows are listed as an unsupported kind by the convert scan.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::Node;

use super::*;
use crate::reader::fixtures_tests::{self as vanilla, GunNumbers};

const NUMBERS: GunNumbers = GunNumbers {
    mass: 1.0,
    range: 25.0,
    warmup: 1.0,
    cooldown: 1.5,
    burst: 1,
    tier: "Neolithic",
};

fn weapon(name: &str, projectile: &str, tags: &[&str]) -> Node {
    vanilla::gun(name, projectile, NUMBERS, tags)
}

fn scan_one(node: Node) -> ConvertCandidate {
    let mut defs = vanilla::base_defs();
    defs.push(node.clone());
    let out = vanilla::load_defs(defs, &vanilla::types(&[]));
    let model = CeModel::absent("test");
    let mut found = scan(&[node], &out.databases, &model);
    assert_eq!(found.len(), 1);
    found.remove(0)
}

#[rstest::rstest]
#[case::bow_tag("RS_Longbow", "RS_Shot1", &["RS_Bow"])]
#[case::crossbow_tag("RS_Cross", "RS_Shot1", &["RS_CrossbowHeavy"])]
#[case::arrow_projectile("RS_Stick", "RS_Arrow_Short", &["RS_Gun"])]
#[case::bolt_projectile("RS_Thrower", "Bolt_RS", &["RS_Gun"])]
fn bows_and_crossbows_are_unsupported(
    #[case] name: &str,
    #[case] projectile: &str,
    #[case] tags: &[&str],
) {
    let c = scan_one(weapon(name, projectile, tags));
    assert_eq!(c.status, ConvertStatus::UnsupportedKind);
    assert_eq!(c.reason, "CE bow conversion is not available in 0.1.0");
    assert_eq!(c.kind, Some(ItemKind::Ranged));
}

#[test]
fn a_plain_gun_with_a_bolt_action_style_name_is_still_supported() {
    let c = scan_one(weapon("RS_Rifle", "Bullet_BoltAction", &["RS_Gun"]));
    assert_eq!(c.status, ConvertStatus::NotConverted);
}

#[test]
fn a_weapon_class_naming_a_bow_is_unsupported() {
    let mut node = weapon("RS_Classed", "RS_Shot1", &["RS_Gun"]);
    let mut classes = Node::new("weaponClasses");
    classes.push_child(Node::with_text("li", "RS_ShortBow"));
    node.push_child(classes);
    let c = scan_one(node);
    assert_eq!(c.status, ConvertStatus::UnsupportedKind);
}
