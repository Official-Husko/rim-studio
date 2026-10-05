//! Golden trees of the scaffolder: for each option combination the planned paths and the rendered
//! text of every file. Regenerate with `UPDATE_GOLDENFILES=1`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::mods::ModDependency;
use rimstudio_testing::golden_json;
use rimstudio_workspace::scaffold::{ScaffoldLayout, ScaffoldSpec, plan};
use rimstudio_xml::render::RenderOpts;
use rimstudio_xml::{ParseMode, parse_document};
use serde_json::{Value, json};

fn base() -> ScaffoldSpec {
    let mut s = ScaffoldSpec::new("/work/RS_Example", "RS Example", "rs.example");
    s.author = "RS Tester".into();
    s.description = "A fictional example mod.".into();
    s
}

fn golden_of(spec: &ScaffoldSpec) -> Value {
    let p = plan(spec);
    assert!(p.is_valid(), "{:?}", p.problems);
    let rendered: serde_json::Map<String, Value> = p
        .render(&RenderOpts::default())
        .into_iter()
        .map(|f| (f.path, Value::String(f.text)))
        .collect();
    json!({
        "root": p.root,
        "items": p.items,
        "rendered": rendered,
    })
}

#[test]
fn golden_flat_vanilla_skeleton() {
    golden_json!("scaffold-flat", &golden_of(&base()));
}

#[test]
fn golden_flat_with_combat_extended_folder() {
    let mut s = base();
    s.ce_patch_folder = true;
    golden_json!("scaffold-flat-ce", &golden_of(&s));
}

#[test]
fn golden_versioned_two_versions() {
    let mut s = base();
    s.layout = ScaffoldLayout::Versioned;
    s.supported_versions = vec!["1.5".into(), "1.6".into()];
    s.languages_folder = true;
    golden_json!("scaffold-versioned", &golden_of(&s));
}

#[test]
fn golden_versioned_with_combat_extended_and_dependency() {
    let mut s = base();
    s.layout = ScaffoldLayout::Versioned;
    s.ce_patch_folder = true;
    s.dependencies.push(ModDependency {
        package_id: "rs.framework".into(),
        display_name: "RS Framework".into(),
        ..ModDependency::default()
    });
    golden_json!("scaffold-versioned-ce-dependency", &golden_of(&s));
}

#[test]
fn golden_flat_with_assemblies_and_placeholders() {
    let mut s = base();
    s.assemblies_folder = true;
    s.defs_folder = false;
    s.placeholder_files = true;
    golden_json!("scaffold-flat-placeholders", &golden_of(&s));
}

fn arb_spec() -> impl Strategy<Value = ScaffoldSpec> {
    (
        "[A-Za-z][A-Za-z0-9 ]{0,12}",
        "[a-z]{1,6}\\.[a-z]{1,6}",
        proptest::collection::vec(1u32..3, 1..4),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(name, id, minors, versioned, ce, defs, patches, keep)| {
            let mut s = ScaffoldSpec::new("/work/RS_Gen", name, id);
            s.supported_versions = minors.into_iter().map(|m| format!("1.{m}")).collect();
            s.layout = if versioned {
                ScaffoldLayout::Versioned
            } else {
                ScaffoldLayout::Flat
            };
            s.ce_patch_folder = ce;
            s.defs_folder = defs;
            s.patches_folder = patches;
            s.placeholder_files = keep;
            s
        })
}

proptest! {
    #[test]
    fn plans_are_valid_deterministic_and_never_leak_combat_extended_outside_its_folder(spec in arb_spec()) {
        let a = plan(&spec);
        let b = plan(&spec);
        prop_assert!(a.is_valid(), "{:?}", a.problems);
        prop_assert_eq!(&a, &b);
        // sorted, unique, relative paths without climbing
        let paths: Vec<&str> = a.items.iter().map(|i| i.path()).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        prop_assert_eq!(&paths, &sorted);
        for p in &paths {
            prop_assert!(!p.starts_with('/') && !p.contains(".."), "{p}");
        }
        let has_ce_dir = paths.iter().any(|p| p.contains("CombatExtended"));
        prop_assert_eq!(has_ce_dir, spec.ce_patch_folder);
        for f in a.render(&RenderOpts::default()) {
            if f.path.ends_with(".xml") {
                prop_assert!(parse_document(f.text.as_bytes(), ParseMode::Game).is_ok(), "{}", f.path);
            }
            prop_assert!(!f.text.contains("CombatExtended."));
        }
        // the gated entry exists exactly when the folder was asked for
        let lf = a.render(&RenderOpts::default()).into_iter().find(|f| f.path == "LoadFolders.xml");
        match (spec.ce_patch_folder, lf) {
            (true, Some(f)) => prop_assert!(f.text.contains("IfModActive=\"ceteam.combatextended\"")),
            (false, Some(f)) => prop_assert!(!f.text.contains("IfModActive")),
            (true, None) => prop_assert!(false, "the gate needs a LoadFolders.xml"),
            (false, None) => {}
        }
    }
}
