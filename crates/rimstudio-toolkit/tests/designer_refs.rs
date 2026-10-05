//! The reference list over the fictional install.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_ipc_types::designer::{DesignerReferenceListRequest, ItemKindDto, TechLevelDto};
use rimstudio_toolkit::designer::{reference_list, reference_list_ce};

fn request() -> DesignerReferenceListRequest {
    DesignerReferenceListRequest {
        limit: 500,
        ..DesignerReferenceListRequest::default()
    }
}

#[test]
fn the_ranged_list_has_every_weapon_in_strength_order() {
    let f = common::fixture(false);
    let list = reference_list(&f.ctx, request()).unwrap();
    assert_eq!(list.kind, ItemKindDto::Ranged);
    assert_eq!(list.total, 15);
    assert_eq!(list.items.len(), 15);
    assert_eq!(list.class_label, "all ranged weapons");
    for (i, item) in list.items.iter().enumerate() {
        assert_eq!(item.index as usize, i);
    }
    let strengths: Vec<f64> = list.items.iter().filter_map(|i| i.strength).collect();
    assert_eq!(strengths.len(), 15);
    assert!(strengths.windows(2).all(|w| w[0] <= w[1]));
    assert!(list.items.iter().all(|i| i.stats.contains_key("mass")));
}

#[test]
fn filters_by_role_and_tier_and_pages() {
    let f = common::fixture(false);
    let mut req = request();
    req.role = Some("rs_rifle".into());
    req.tier = Some(TechLevelDto::Industrial);
    let list = reference_list(&f.ctx, req).unwrap();
    assert_eq!(list.total, 7);
    assert_eq!(list.class_label, "Industrial rs_rifle ranged weapons");
    assert!(
        list.items
            .iter()
            .all(|i| i.role.as_deref() == Some("RS_Rifle"))
    );
    assert!(
        list.items
            .iter()
            .all(|i| i.tier == Some(TechLevelDto::Industrial))
    );

    let mut page = request();
    page.limit = 4;
    page.offset = 13;
    let tail = reference_list(&f.ctx, page).unwrap();
    assert_eq!(tail.total, 15);
    assert_eq!(tail.offset, 13);
    assert_eq!(tail.items.len(), 2);
    assert_eq!(tail.items[0].index, 13);
}

#[test]
fn pool_distributions_cover_the_filtered_class() {
    let f = common::fixture(false);
    let list = reference_list(&f.ctx, request()).unwrap();
    let mass = list.pools.iter().find(|p| p.stat == "mass").unwrap();
    assert_eq!(mass.n, 15);
    assert!(mass.min <= mass.p10 && mass.p10 <= mass.median);
    assert!(mass.median <= mass.p90 && mass.p90 <= mass.max);
    let names: Vec<&str> = list.pools.iter().map(|p| p.stat.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn the_mod_id_names_mods_and_is_absent_for_official_content() {
    let f = common::fixture(false);
    let list = reference_list(&f.ctx, request()).unwrap();
    let modded = list
        .items
        .iter()
        .find(|i| i.def_name == "RS_ModGun")
        .unwrap();
    assert_eq!(modded.mod_id.as_deref(), Some("rs.extra"));
    let core = list
        .items
        .iter()
        .find(|i| i.def_name == "RS_Gun00")
        .unwrap();
    assert_eq!(core.mod_id, None);
}

#[test]
fn the_melee_list_uses_the_melee_pool() {
    let f = common::fixture(false);
    let mut req = request();
    req.kind = ItemKindDto::Melee;
    let list = reference_list(&f.ctx, req).unwrap();
    assert_eq!(list.kind, ItemKindDto::Melee);
    assert_eq!(list.total, 8);
    assert!(
        list.items
            .iter()
            .all(|i| i.def_name.starts_with("RS_Blade"))
    );
}

#[test]
fn without_a_game_install_the_list_reports_why() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = reference_list(&ctx, request()).unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn the_vanilla_list_never_shows_combat_extended_even_when_it_is_loaded() {
    let f = common::fixture(true);
    let list = reference_list(&f.ctx, request()).unwrap();
    assert_eq!(list.total, 15);
    assert!(list.items.iter().all(|i| {
        i.stats
            .keys()
            .all(|k| !k.starts_with("vanilla.") && !k.starts_with("ratio."))
    }));
}

#[test]
fn the_combat_extended_list_needs_combat_extended() {
    let without = common::fixture(false);
    let err = reference_list_ce(&without.ctx, request()).unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
    let with = common::fixture(true);
    let list = reference_list_ce(&with.ctx, request()).unwrap();
    assert_eq!(
        list.total, 0,
        "the fictional Combat Extended has no converted guns"
    );
}
