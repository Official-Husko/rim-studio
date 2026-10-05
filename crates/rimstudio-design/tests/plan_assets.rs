//! Tests of the plan of imported assets: copy files, the `SoundDef`, paths from the layout, determinism,
//! version folders, the vanilla gate and the plan of a design without imports.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{melee_spec, ranged_ce, ranged_spec};
use rimstudio_design::assets::png::build_png;
use rimstudio_design::assets::wav::build_wav;
use rimstudio_design::assets::{AssetFacts, AssetInfo, SourceState, detect};
use rimstudio_design::model::{CustomSound, FloatRange, ProjectileChoice, ProjectileSpec};
use rimstudio_design::plan::{
    FileAction, FileKind, LayoutProfile, ProjectLayout, WritePlan, export_vanilla_plan,
    export_vanilla_plan_with,
};

fn state(bytes: &[u8]) -> SourceState {
    SourceState::Read(AssetInfo {
        bytes: bytes.len() as u64,
        sha256: stand_in_hash(bytes),
        detected: detect(bytes),
    })
}

/// A stand in hash: the tests only need a stable value per content.
fn stand_in_hash(bytes: &[u8]) -> String {
    format!(
        "{:064x}",
        bytes.iter().map(|b| u128::from(*b)).sum::<u128>()
    )
}

fn facts() -> AssetFacts {
    let mut f = AssetFacts::new();
    f.insert("/art/rifle.png", state(&build_png(64, 64)));
    f.insert("/art/bullet.png", state(&build_png(16, 16)));
    f.insert("/art/shot.wav", state(&build_wav(1, 22_050, 500)));
    f
}

fn with_assets() -> rimstudio_design::model::DesignSpec {
    let mut spec = ranged_spec();
    spec.assets.texture = Some("/art/rifle.png".into());
    spec.assets.projectile_texture = Some("/art/bullet.png".into());
    spec.sounds.shot = Some(CustomSound {
        clips: vec!["/art/shot.wav".into()],
        volume: Some(FloatRange::new(30.0, 35.0)),
        ..CustomSound::default()
    });
    spec
}

fn paths(plan: &WritePlan) -> Vec<&str> {
    plan.paths()
}

#[test]
fn a_plan_with_imports_lists_copies_and_the_sound_def() {
    let spec = with_assets();
    // the fixture spec has an own projectile; check the names before asserting paths
    let Some(ProjectileChoice::Inline(p)) = spec.ranged.as_ref().unwrap().projectile.clone() else {
        panic!("the fixture has an own projectile");
    };
    let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let want = [
        "Defs/SoundDefs/World_Oneshots_Weapons.xml".to_owned(),
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_TestRifle.xml".to_owned(),
        "Sounds/Weapons/RS_TestRifle_Shot/shot.wav".to_owned(),
        "Textures/Things/Item/Equipment/WeaponRanged/RS_TestRifle.png".to_owned(),
        format!("Textures/Things/Projectile/{}.png", p.def_name),
    ];
    assert_eq!(
        paths(&plan),
        want.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let copies: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Copy)
        .collect();
    assert_eq!(copies.len(), 3);
    for c in copies {
        assert_eq!(c.action, FileAction::Create);
        assert!(c.tree.is_none() && c.copy.is_some());
    }
}

#[test]
fn the_plan_is_deterministic() {
    let a = export_vanilla_plan_with(&with_assets(), &ProjectLayout::default(), &facts());
    let b = export_vanilla_plan_with(&with_assets(), &ProjectLayout::default(), &facts());
    assert_eq!(a, b);
}

#[test]
fn a_version_folder_moves_every_asset_path() {
    let layout = ProjectLayout::with_version_folder("1.6");
    let plan = export_vanilla_plan_with(&with_assets(), &layout, &facts());
    assert!(!plan.has_errors());
    assert!(
        paths(&plan).iter().all(|p| p.starts_with("1.6/")),
        "{:?}",
        paths(&plan)
    );
    // the texture path in the def does not carry the folder: the game reads it below Textures
    assert!(plan.contains_text("Things/Item/Equipment/WeaponRanged/RS_TestRifle"));
    assert!(!plan.contains_text("1.6/Things"));
}

#[test]
fn other_profiles_keep_the_same_asset_paths() {
    for profile in [LayoutProfile::CoreStyle, LayoutProfile::Flat] {
        let layout = ProjectLayout::default().with_profile(profile);
        let plan = export_vanilla_plan_with(&with_assets(), &layout, &facts());
        assert!(!plan.has_errors(), "{profile:?}");
        assert!(
            plan.file("Sounds/Weapons/RS_TestRifle_Shot/shot.wav")
                .is_some()
        );
    }
}

#[test]
fn imports_never_bring_combat_extended_into_the_vanilla_plan() {
    let mut spec = with_assets();
    spec.ce = Some(ranged_ce());
    let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert!(!plan.has_errors());
    assert!(!plan.contains_text("CombatExtended"));
}

#[test]
fn an_error_in_a_source_leaves_the_plan_without_files() {
    let mut f = facts();
    f.insert("/art/rifle.png", SourceState::Missing);
    let plan = export_vanilla_plan_with(&with_assets(), &ProjectLayout::default(), &f);
    assert!(plan.has_errors());
    assert!(plan.files.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.texture-missing")
    );
}

#[test]
fn a_spec_with_imports_but_no_facts_is_refused_not_guessed() {
    let plan = export_vanilla_plan(&with_assets(), &ProjectLayout::default());
    assert!(plan.has_errors());
    assert!(plan.files.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.asset-unchecked")
    );
}

#[test]
fn a_spec_without_imports_plans_the_same_with_or_without_facts() {
    let spec = ranged_spec();
    let a = export_vanilla_plan(&spec, &ProjectLayout::default());
    let b = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert_eq!(a, b);
}

#[test]
fn a_melee_weapon_takes_a_texture_import_into_the_melee_folder() {
    let mut spec = melee_spec();
    spec.assets.texture = Some("/art/rifle.png".into());
    let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert!(
        plan.paths()
            .iter()
            .any(|p| p.starts_with("Textures/Things/Item/Equipment/WeaponMelee/"))
    );
}

#[test]
fn the_texture_replaces_a_typed_path_and_clears_the_reserved_hint() {
    let mut spec = with_assets();
    spec.texture_path = Some("Things/Item/Equipment/WeaponRanged/SomethingElse".into());
    let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert!(plan.contains_text("Things/Item/Equipment/WeaponRanged/RS_TestRifle"));
    assert!(!plan.contains_text("SomethingElse"));
    assert!(
        !plan
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.texture-reserved")
    );
}

#[test]
fn an_inline_projectile_without_an_own_texture_import_keeps_what_it_had() {
    let mut spec = ranged_spec();
    if let Some(r) = spec.ranged.as_mut() {
        r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
            def_name: "RS_B".into(),
            texture_path: Some("Things/Projectile/Bullet_Small".into()),
            ..ProjectileSpec::default()
        }));
    }
    let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &facts());
    assert!(plan.contains_text("Things/Projectile/Bullet_Small"));
}

mod properties {
    use super::*;
    use proptest::prelude::*;
    use rimstudio_design::plan::is_safe_relative_path;

    proptest! {
        /// Whatever the file names of the sources are, every planned path is a safe relative path inside the
        /// project and the planner never panics: a hostile name cannot steer a copy out of the project.
        #[test]
        fn hostile_source_names_never_leave_the_project(
            clips in proptest::collection::vec("\\PC{0,40}", 1..4),
            texture in "\\PC{0,40}",
        ) {
            let mut spec = ranged_spec();
            let mut f = AssetFacts::new();
            for c in clips.iter().chain(std::iter::once(&texture)) {
                f.insert(c.trim(), state(&build_wav(1, 8000, 4)));
            }
            f.insert(texture.trim(), state(&build_png(8, 8)));
            spec.assets.texture = Some(texture.clone());
            spec.sounds.shot = Some(CustomSound { clips: clips.clone(), ..CustomSound::default() });
            let plan = export_vanilla_plan_with(&spec, &ProjectLayout::default(), &f);
            for file in &plan.files {
                prop_assert!(is_safe_relative_path(&file.path), "{}", file.path);
                prop_assert!(!file.path.contains(".."), "{}", file.path);
            }
        }
    }
}
