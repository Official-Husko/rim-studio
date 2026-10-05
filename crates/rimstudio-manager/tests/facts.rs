//! The facts of a scan: counts per source, duplicate groups and the Combat Extended entry, over fictional
//! installs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Harness, def};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::SourceKind;
use rimstudio_library::duplicates::{ChoiceReason, GameTreatment};
use rimstudio_manager::facts::DUPLICATE_CAP;
use rimstudio_manager::scan::{LibraryScanRequest, LibraryScanResult, library_scan};
use rimstudio_manager::sources::{SourcesAddFolderRequest, add_folder};
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder};

const CE_ABOUT: &str = "<ModMetaData><name>RS Combat</name><packageId>CETeam.CombatExtended</packageId>\
<modVersion>9.9.9.0</modVersion><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";

fn scan(h: &Harness) -> LibraryScanResult {
    add_folder(
        &h.ctx(),
        SourcesAddFolderRequest {
            path: h.install.custom_dir.to_string(),
            ..SourcesAddFolderRequest::default()
        },
    )
    .unwrap();
    library_scan(
        &h.ctx(),
        LibraryScanRequest::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap()
}

fn harness(builder: InstallBuilder) -> Harness {
    let h = Harness::with_install(builder.build_temp().unwrap());
    h.pin_install();
    h
}

fn base() -> InstallBuilder {
    InstallBuilder::new()
        .core_def(def("RS_CoreThing"))
        .mod_folder(
            ModFolder::new("RS_ModsA", "rs.mods.a")
                .name("RS Mods A")
                .def(def("RS_A1")),
        )
        .workshop_mod(
            111,
            ModFolder::new("ignored", "rs.workshop.one").name("RS Workshop One"),
        )
        .custom_mod(ModFolder::new("RS_Custom", "rs.custom.one").name("RS Custom One"))
}

#[test]
fn counts_per_source_split_loadable_from_custom_only() {
    let h = harness(base());
    let out = scan(&h);
    let f = &out.facts;
    assert_eq!(out.counts.loadable, 3);
    assert_eq!(out.counts.needs_link, 1);
    let total: usize = f.per_source.iter().map(|s| s.mods).sum();
    let loadable: usize = f.per_source.iter().map(|s| s.loadable).sum();
    let custom_only: usize = f.per_source.iter().map(|s| s.custom_only).sum();
    assert_eq!(total, out.counts.mods);
    assert_eq!(loadable, out.counts.loadable);
    assert_eq!(custom_only, out.counts.needs_link);
    let custom_id = out
        .sources
        .iter()
        .find(|s| s.kind == SourceKind::Custom)
        .unwrap()
        .id
        .to_string();
    let custom = f.source(&custom_id).unwrap();
    assert_eq!(
        (custom.mods, custom.loadable, custom.custom_only),
        (1, 0, 1)
    );
    let mods = f.source("game-mods").unwrap();
    assert_eq!((mods.mods, mods.loadable, mods.custom_only), (1, 1, 0));
    assert_eq!(f.per_source.len(), out.sources.len());
}

#[test]
fn a_library_without_duplicates_or_combat_extended_reports_none() {
    let h = harness(base());
    let out = scan(&h);
    assert_eq!(out.facts.duplicates.total, 0);
    assert!(out.facts.duplicates.groups.is_empty());
    assert!(!out.facts.ce.present);
    assert_eq!(out.facts.ce.package_id, None);
    assert_eq!(out.facts.ce.version, None);
}

#[test]
fn duplicates_by_package_id_name_the_kept_and_the_skipped_copies() {
    let h = harness(
        base()
            .custom_mod(ModFolder::new("RS_CopyOfA", "RS.Mods.A").name("RS Mods A copy"))
            .custom_mod(ModFolder::new("RS_CopyOfW", "rs.workshop.one").name("RS Workshop copy")),
    );
    let out = scan(&h);
    let d = &out.facts.duplicates;
    assert_eq!(d.total, 2);
    assert_eq!(d.total, out.counts.duplicate_groups);
    assert_eq!(d.skipped_total, 2);
    let group = d
        .groups
        .iter()
        .find(|g| g.package_id.eq_ignore_ascii_case("rs.mods.a"))
        .unwrap();
    // The library keeps the user's own copy (a custom folder ranks first); the game cannot see it yet.
    assert_eq!(group.package_id, "RS.Mods.A");
    assert_eq!(group.kept.kind, SourceKind::Custom);
    assert_eq!(group.kept.name, "RS Mods A copy");
    assert_eq!(group.kept.game, GameTreatment::NotVisible);
    assert!(group.kept.why.is_none());
    assert!(!group.same_source);
    assert_eq!(group.reason, ChoiceReason::SourcePriority);
    assert_eq!(group.skipped.len(), 1);
    let skipped = &group.skipped[0];
    assert_eq!(skipped.kind, SourceKind::GameMods);
    assert_eq!(skipped.name, "RS Mods A");
    assert!(matches!(skipped.game, GameTreatment::Loaded { .. }));
    let why = skipped.why.as_deref().unwrap();
    assert!(why.contains("higher priority"), "{why}");
    let sorted: Vec<&str> = d.groups.iter().map(|g| g.package_id.as_str()).collect();
    let mut expected = sorted.clone();
    expected.sort_by_key(|s| s.to_lowercase());
    assert_eq!(sorted, expected);
}

#[test]
fn the_duplicate_list_is_capped_but_the_total_is_not() {
    let mut b = InstallBuilder::new().core_def(def("RS_CoreThing"));
    for n in 0..(DUPLICATE_CAP + 5) {
        let id = format!("rs.many.m{n:03}");
        b = b
            .mod_folder(ModFolder::new(format!("RS_M{n:03}"), id.clone()).name("RS Many"))
            .custom_mod(ModFolder::new(format!("RS_C{n:03}"), id).name("RS Many copy"));
    }
    let h = harness(b);
    let out = scan(&h);
    let d = &out.facts.duplicates;
    assert_eq!(d.total, DUPLICATE_CAP + 5);
    assert_eq!(d.groups.len(), DUPLICATE_CAP);
    assert_eq!(d.skipped_total, DUPLICATE_CAP + 5);
    assert_eq!(out.counts.duplicate_groups, DUPLICATE_CAP + 5);
}

#[test]
fn combat_extended_is_found_by_its_package_id_with_its_version() {
    let h = harness(
        base().workshop_mod(
            222,
            ModFolder::new("ignored", "CETeam.CombatExtended")
                .name("RS Combat")
                .raw_about(CE_ABOUT),
        ),
    );
    let out = scan(&h);
    let ce = &out.facts.ce;
    assert!(ce.present);
    assert_eq!(ce.package_id.as_deref(), Some("CETeam.CombatExtended"));
    assert_eq!(ce.name.as_deref(), Some("RS Combat"));
    assert_eq!(ce.version.as_deref(), Some("9.9.9.0"));
    assert_eq!(ce.source_kind, Some(SourceKind::Workshop));
    assert_eq!(ce.loadable, Some(true));
    assert!(ce.path.as_deref().unwrap().contains("222"));
}

#[test]
fn combat_extended_only_in_a_custom_folder_is_present_but_not_loadable() {
    let h = harness(
        base().custom_mod(
            ModFolder::new("RS_CE", "ceteam.combatextended")
                .name("RS Combat")
                .raw_about(CE_ABOUT),
        ),
    );
    let out = scan(&h);
    let ce = &out.facts.ce;
    assert!(ce.present);
    assert_eq!(ce.source_kind, Some(SourceKind::Custom));
    assert_eq!(ce.loadable, Some(false));
    assert_eq!(ce.version.as_deref(), Some("9.9.9.0"));
}

#[test]
fn a_mod_with_a_similar_name_is_not_combat_extended() {
    let h =
        harness(base().mod_folder(ModFolder::new("RS_NotCE", "rs.notce").name("Combat Extended")));
    let out = scan(&h);
    assert!(!out.facts.ce.present);
}

#[test]
fn the_facts_serialise_with_camel_case_names() {
    let h = harness(base());
    let out = scan(&h);
    let text = serde_json::to_string(&out.facts).unwrap();
    assert!(text.contains("\"perSource\""));
    assert!(text.contains("\"customOnly\""));
    assert!(text.contains("\"skippedTotal\""));
}
