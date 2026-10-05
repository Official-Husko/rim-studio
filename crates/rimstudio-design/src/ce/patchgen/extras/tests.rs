//! Unit tests of the optional additions of a conversion.

use super::*;
use crate::ce::patchgen::conventions::{
    MIN_EXAMPLES, dropped_tool_fields, recoil_pattern_habit, reload_one_at_a_time_habit,
};
use crate::ce::reader::extras::WeaponExtras;
use crate::ce::reader::{CeGun, CeMelee};
use std::collections::BTreeMap;

fn gun(def: &str, class: &str, reload: bool) -> CeGun {
    CeGun {
        def_name: def.into(),
        label: def.into(),
        mod_idx: None,
        tech_level: None,
        weapon_tags: vec![class.into()],
        ce_tags: vec![class.into()],
        ai_class: Some(class.into()),
        ammo_set: None,
        default_projectile: None,
        reload_one_at_a_time: reload,
        ai_aim_mode: None,
        use_burst_mode: None,
        aimed_burst: None,
        tools: Vec::new(),
        bow: false,
        ammo_gen_per_mag: None,
        stats: BTreeMap::new(),
        twin: None,
        twin_tags: Vec::new(),
        excluded: None,
    }
}

fn melee(def: &str) -> CeMelee {
    CeMelee {
        def_name: def.into(),
        label: def.into(),
        mod_idx: None,
        tech_level: None,
        weapon_tags: Vec::new(),
        tools: Vec::new(),
        stats: BTreeMap::new(),
        twin: None,
        twin_tags: Vec::new(),
        excluded: None,
    }
}

fn model() -> CeModel {
    let mut m = CeModel::absent(String::new());
    m.absent = None;
    m
}

fn extras(def: &str, twin: &[&str], own: &[&str], recoil: Option<&str>) -> WeaponExtras {
    WeaponExtras {
        def_name: def.into(),
        tool_fields: own.iter().map(|s| (*s).to_owned()).collect(),
        twin_tool_fields: twin.iter().map(|s| (*s).to_owned()).collect(),
        recoil_pattern: recoil.map(str::to_owned),
    }
}

#[test]
fn a_list_entry_is_guarded_by_class_or_by_text_only() {
    let class = Node::builder("li").attr("Class", "A.B").build();
    assert_eq!(
        entry_guard("modExtensions", &class).as_deref(),
        Some("modExtensions/li[@Class=\"A.B\"]")
    );
    let text = Node::with_text("li", "x");
    assert_eq!(
        entry_guard("tags", &text).as_deref(),
        Some("tags/li[.=\"x\"]")
    );
    let neither = Node::builder("li").text_elem("a", "1").build();
    assert!(entry_guard("tags", &neither).is_none());
}

#[test]
fn only_a_list_is_a_list() {
    assert!(!is_list(&Node::new("a")));
    assert!(!is_list(&Node::with_text("a", "1")));
    assert!(is_list(&Node::builder("a").text_elem("li", "1").build()));
    assert!(!is_list(
        &Node::builder("a")
            .text_elem("li", "1")
            .text_elem("b", "2")
            .build()
    ));
}

#[test]
fn extra_tags_are_added_once_and_in_order() {
    let ce = CePatchSpec {
        extra_tags: vec!["B".into(), "A".into(), "B".into(), String::new()],
        ..CePatchSpec::default()
    };
    let mut tags = vec!["A".to_owned()];
    push_extra_tags(&mut tags, &ce);
    assert_eq!(tags, ["A", "B"]);
}

#[test]
fn an_unknown_extra_tag_is_a_warning_and_a_raw_node_problem_is_an_error() {
    let ce = CePatchSpec {
        extra_tags: vec!["Nope".into()],
        raw_extras: vec![Node::new("defName")],
        ..CePatchSpec::default()
    };
    let out = validate(&ce, &model());
    let codes: Vec<&str> = out.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"ce.raw-extra-invalid"));
    assert!(codes.iter().any(|c| c.starts_with("ce.cep016")));
}

#[test]
fn a_tool_child_is_dropped_only_when_enough_twins_have_it_and_most_dropped_it() {
    let mut m = model();
    m.melee = vec![melee("A"), melee("B"), melee("C")];
    m.extras.melee = vec![
        extras("A", &["x"], &[], None),
        extras("B", &["x"], &[], None),
        extras("C", &["x"], &["x"], None),
    ];
    // 2 of 3 dropped is below 75 percent
    assert!(dropped_tool_fields(&m, ItemKind::Melee).is_empty());
    m.extras.melee[2] = extras("C", &["x"], &[], None);
    let dropped = dropped_tool_fields(&m, ItemKind::Melee);
    assert_eq!(dropped.len(), 1);
    assert_eq!((dropped[0].dropped, dropped[0].total), (3, 3));
    // a single example is no habit
    m.melee.truncate(MIN_EXAMPLES - 1);
    assert!(dropped_tool_fields(&m, ItemKind::Melee).is_empty());
    // and a gun is never read from the melee list
    assert!(dropped_tool_fields(&m, ItemKind::Ranged).is_empty());
}

#[test]
fn class_habits_need_agreeing_guns_of_the_class() {
    let mut m = model();
    m.guns = vec![
        gun("A", "C1", true),
        gun("B", "C1", true),
        gun("C", "C1", false),
        gun("D", "C2", true),
    ];
    m.extras.guns = vec![
        extras("A", &[], &[], Some("Mounted")),
        extras("B", &[], &[], Some("Mounted")),
        extras("C", &[], &[], None),
        extras("D", &[], &[], Some("Mounted")),
    ];
    assert_eq!(reload_one_at_a_time_habit(&m, "C1"), Some((2, 3)));
    assert_eq!(
        recoil_pattern_habit(&m, "C1"),
        Some(("Mounted".to_owned(), 2, 3))
    );
    // one gun of a class is no habit
    assert_eq!(reload_one_at_a_time_habit(&m, "C2"), None);
    assert_eq!(recoil_pattern_habit(&m, "C2"), None);
}
