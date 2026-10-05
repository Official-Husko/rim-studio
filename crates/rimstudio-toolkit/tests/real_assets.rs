//! A weapon with a custom shot sound and its own textures on a real install: the art and the clip are
//! copied from the owner's Lone Wolf mod (read only) into a temporary folder, a fictional weapon is
//! planned and applied into a temporary project, and the resulting mod is loaded through the def engine to
//! confirm that the `SoundDef` and the weapon resolve.
//!
//! The tests are `#[ignore]`: they read the install and the owner's mod folders and write only into
//! temporary folders.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CUSTOM_DIR="<folder with the owner's mods>" \
//! cargo test -p rimstudio-toolkit --release --test real_assets -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rimstudio_design::assets::{Detected, detect};
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_design::model::{
    CostEntry, CustomSound, DesignSpec, Draft, FloatRange, ParentRef, ProjectileChoice,
    ProjectileSpec, ScalarField, TechLevel, ValueSource,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerApplyPlanRequest, DesignerAssetInfoRequest, DesignerExportPlanRequest, FileKindDto,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::dto::draft_to_dto;
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{Ctx, apply_plan, asset_info, suggest_fill};
use rimstudio_workspace::project::read_mod_meta;
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(std::env::var(name).ok()?);
    path.is_dir().then_some(path)
}

fn session_for(game_dir: &Utf8PathBuf, extra_mod: Option<&Utf8PathBuf>) -> Arc<WorkspaceSession> {
    let text = std::fs::read_to_string(game_dir.join("Version.txt")).unwrap();
    let game = GameVersion::parse(text.trim()).unwrap();
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        game_dir.join("Data"),
    ));
    let outcome = Scanner::scan(
        &set,
        &ScanOptions::default().with_game_version(game.clone()),
        &NoopProgress,
        &CancelToken::new(),
    );
    let mut options = DesignerReferenceOptions::new(game);
    let mut builder = ModIndex::builder();
    for (_, meta) in outcome.index.iter() {
        builder.push(meta.clone());
    }
    if let Some(dir) = extra_mod {
        let meta = read_mod_meta(dir).unwrap().meta;
        options = options.with_mod(meta.package_id.as_str().to_owned());
        builder.push(meta);
    }
    let reference = ReferenceSet::reference_for_designer(&builder.build(), game_dir, &options);
    let input = OpenInput::new(reference).with_game_dir(game_dir.clone());
    Arc::new(WorkspaceSession::open_simple(input).unwrap())
}

fn copy_from(custom: &Utf8PathBuf, rel: &str, art: &Utf8PathBuf, name: &str) -> String {
    let source = custom.join("[OH] The Lone Wolf Weapon Package").join(rel);
    assert!(source.is_file(), "{source} is not a file");
    let target = art.join(name);
    std::fs::copy(source.as_std_path(), target.as_std_path()).unwrap();
    target.to_string()
}

#[test]
#[ignore = "reads the real install and the owner's Lone Wolf mod (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CUSTOM_DIR)"]
fn a_weapon_with_a_real_clip_and_real_art_applies_and_the_sound_def_resolves() {
    let (Some(game_dir), Some(custom)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CUSTOM_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CUSTOM_DIR is not set, skipping");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session_for(&game_dir, None),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    let engine = ctx.require_engine().unwrap();

    // the real files, copied out of the owner's mod
    let art = base.join("art");
    std::fs::create_dir_all(art.as_std_path()).unwrap();
    let texture = copy_from(
        &custom,
        "Textures/Things/Weapon/Ranged/Pistol/TLWWP_Glock_17.png",
        &art,
        "glock.png",
    );
    let bullet = copy_from(
        &custom,
        "Textures/Things/Projectile/TLWWP_RGD_5_Projectile.png",
        &art,
        "bullet.png",
    );
    let clip = copy_from(
        &custom,
        "Sounds/Weapons/TLWWP_Glock_17_Shot.wav",
        &art,
        "glock_shot.wav",
    );
    let facts = |path: &str| {
        asset_info(
            &ctx,
            DesignerAssetInfoRequest {
                path: path.to_owned(),
                project_id: None,
            },
        )
        .unwrap()
    };
    for path in [&texture, &bullet, &clip] {
        let info = facts(path);
        println!(
            "{path}: {:?} {:?} bytes, {:?}x{:?}, {:?} channel(s), {:?} Hz, {:?} ms, {} diagnostics",
            info.kind,
            info.bytes,
            info.width,
            info.height,
            info.channels,
            info.sample_rate,
            info.duration_ms,
            info.diagnostics.len()
        );
        for d in &info.diagnostics {
            println!("    {} {}", d.code, d.message);
        }
    }

    // the project: a fictional mod folder in a temporary directory
    let root = base.join("RS_Assets");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>RS Assets</name><packageId>rs.assets</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    )
    .unwrap();
    let project_id = ctx
        .env()
        .projects()
        .open(&root)
        .unwrap()
        .id
        .as_str()
        .to_owned();

    // a class of the real install, as in the end to end test
    let pool = engine.pool(PoolKind::Ranged).unwrap();
    let mut roles = pool.roles();
    roles.sort_by_key(|r| std::cmp::Reverse(r.1));
    let role = roles.first().map(|r| r.0.clone()).unwrap();
    let reference = pool
        .items
        .iter()
        .find(|i| i.role == role)
        .or_else(|| pool.items.first())
        .unwrap();
    let record = engine.databases().get("ThingDef", &reference.id).unwrap();
    let parent = record.parents.first().map(|p| p.name.clone());
    let cost: Vec<CostEntry> = record
        .node
        .child("costList")
        .map(|c| {
            c.elements()
                .take(2)
                .map(|e| CostEntry::new(&e.tag, e.text_content().trim().parse().unwrap_or(10.0)))
                .collect()
        })
        .unwrap_or_default();
    let mut spec = DesignSpec::new_ranged("RS_GlockLike", "glock like pistol");
    spec.identity.mod_prefix = "RS".into();
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some(role);
    spec.parent = parent.map(ParentRef::named);
    spec.cost_list = cost;
    let filled = suggest_fill(&ctx, draft_to_dto(&Draft::new(spec)).unwrap()).unwrap();
    let mut spec = rimstudio_toolkit::designer::dto::draft_from_dto(&filled)
        .unwrap()
        .spec;
    spec.offer(
        ScalarField::WorkToMake,
        reference.stat("work").unwrap_or(9000.0),
        ValueSource::Typed,
    );
    if let Some(r) = spec.ranged.as_mut() {
        r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
            def_name: "RS_GlockLike_Bullet".into(),
            label: "glock like bullet".into(),
            parent: Some("BaseBullet".into()),
            damage_def: Some("Bullet".into()),
            ..ProjectileSpec::default()
        }));
        r.damage = Some(rimstudio_design::model::Sourced::typed(12.0));
    }
    if spec.cost_list.is_empty() {
        spec.cost_list = vec![
            CostEntry::new("Steel", 40.0),
            CostEntry::new("ComponentIndustrial", 2.0),
        ];
    }
    spec.assets.texture = Some(texture.clone());
    spec.assets.projectile_texture = Some(bullet.clone());
    spec.sounds.shot = Some(CustomSound {
        clips: vec![clip.clone()],
        volume: Some(FloatRange::new(34.0, 40.0)),
        pitch: Some(FloatRange::new(1.19, 1.23)),
        distance: None,
        max_simultaneous: Some(1),
        def_name: None,
    });
    let request = DesignerExportPlanRequest {
        project_id: project_id.clone(),
        draft: draft_to_dto(&Draft::new(spec)).unwrap(),
        convert: None,
        accept_suggestions: None,
    };
    let plan = export_plan(&ctx, request.clone()).unwrap();
    println!(
        "plan: {} files, errors: {}",
        plan.files.len(),
        plan.has_errors
    );
    for d in &plan.diagnostics {
        println!("diagnostic {} {:?} {}", d.code, d.severity, d.message);
    }
    for f in &plan.files {
        println!(
            "  {} ({:?}, {:?}, {} bytes)",
            f.path, f.kind, f.action, f.bytes
        );
    }
    assert!(!plan.has_errors);
    assert_eq!(
        plan.files
            .iter()
            .filter(|f| f.kind == FileKindDto::Copy)
            .count(),
        3
    );

    let report = apply_plan(
        &ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id.clone(),
            request,
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(report.written.iter().all(|w| w.verified), "{report:?}");
    println!("applied {} files", report.written.len());

    // the copies are byte for byte the originals
    for (rel, source) in [
        (
            "Textures/Things/Item/Equipment/WeaponRanged/RS_GlockLike.png",
            &texture,
        ),
        (
            "Textures/Things/Projectile/RS_GlockLike_Bullet.png",
            &bullet,
        ),
        ("Sounds/Weapons/RS_GlockLike_Shot/glock_shot.wav", &clip),
    ] {
        assert_eq!(
            std::fs::read(root.join(rel).as_std_path()).unwrap(),
            std::fs::read(source).unwrap(),
            "{rel}"
        );
    }
    let png = std::fs::read(
        root.join("Textures/Things/Item/Equipment/WeaponRanged/RS_GlockLike.png")
            .as_std_path(),
    )
    .unwrap();
    assert!(matches!(detect(&png), Detected::Png(_)));

    // load the resulting mod through the def engine: the SoundDef, the weapon and its verb resolve
    let loaded = Ctx::new(
        session_for(&game_dir, Some(&root)),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    let engine = loaded.require_engine().unwrap();
    let sound = engine
        .databases()
        .get("SoundDef", "RS_GlockLike_Shot")
        .expect("the SoundDef of the new weapon resolves");
    let grains = sound
        .node
        .child("subSounds")
        .and_then(|s| s.children_named("li").next())
        .and_then(|li| li.child("grains"))
        .unwrap();
    assert_eq!(
        grains
            .children_named("li")
            .filter_map(|g| g.child_text("clipPath"))
            .collect::<Vec<_>>(),
        ["Weapons/RS_GlockLike_Shot/glock_shot"]
    );
    let weapon = engine
        .databases()
        .get("ThingDef", "RS_GlockLike")
        .expect("the weapon resolves");
    let cast = weapon
        .node
        .child("verbs")
        .and_then(|v| v.children_named("li").next())
        .and_then(|li| li.child_text("soundCast"));
    assert_eq!(cast, Some("RS_GlockLike_Shot"));
    println!("the SoundDef and the weapon resolve through the def engine");
}
