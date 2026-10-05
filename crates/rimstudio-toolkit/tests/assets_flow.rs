//! Imported assets over a fictional install: texture imports, custom shot sounds, the copy step of apply,
//! backups of replaced files, hostile sources and the facts query. Every name starts with `RS_`; the images
//! and sounds are tiny files built in code.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use camino::Utf8PathBuf;
use common_project::{Proj, assert_text_golden, fixture, melee, project, put, ranged, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_design::assets::ogg::build_ogg_vorbis;
use rimstudio_design::assets::png::build_png;
use rimstudio_design::assets::wav::build_wav;
use rimstudio_design::model::{CustomSound, DesignSpec, FloatRange};
use rimstudio_io::sha256::sha256_hex;
use rimstudio_ipc_types::designer::{
    AssetKindDto, AssetStatusDto, DesignerApplyPlanRequest, DesignerAssetInfoRequest,
    DesignerExportPlanRequest, FileActionDto, FileKindDto, WritePlanDto,
};
use rimstudio_ipc_types::project::{ProjectLayoutCheckRequest, ProjectTreeRequest};
use rimstudio_toolkit::designer::plan::{export_plan, prepare};
use rimstudio_toolkit::designer::{ApplyOptions, apply_built, apply_plan, asset_info};
use rimstudio_toolkit::error::ToolkitError;

const WEAPON_TEX: &str = "Textures/Things/Item/Equipment/WeaponRanged/RS_NewRifle.png";
const PROJECTILE_TEX: &str = "Textures/Things/Projectile/RS_NewBullet.png";
const CLIP_A: &str = "Sounds/Weapons/RS_NewRifle_Shot/bang.wav";
const CLIP_B: &str = "Sounds/Weapons/RS_NewRifle_Shot/crack.ogg";
const SOUND_DEFS: &str = "Defs/SoundDefs/World_Oneshots_Weapons.xml";
const WEAPON_DEF: &str = "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml";

/// A folder outside the project with source art and clips.
struct Art {
    _tmp: tempfile::TempDir,
    dir: Utf8PathBuf,
}

impl Art {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
        Self { _tmp: tmp, dir }
    }

    fn put(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.dir.join(name);
        std::fs::write(path.as_std_path(), bytes).unwrap();
        path.to_string()
    }
}

fn spec_with_assets(art: &Art) -> DesignSpec {
    let mut spec = ranged();
    spec.assets.texture = Some(art.put("rifle.png", &build_png(64, 64)));
    spec.assets.projectile_texture = Some(art.put("bullet.png", &build_png(16, 16)));
    spec.sounds.shot = Some(CustomSound {
        clips: vec![
            art.put("bang.wav", &build_wav(1, 22_050, 2000)),
            art.put("crack.ogg", &build_ogg_vorbis(1, 44_100)),
        ],
        volume: Some(FloatRange::new(30.0, 34.0)),
        pitch: Some(FloatRange::new(0.95, 1.05)),
        distance: Some(FloatRange::new(10.0, 60.0)),
        max_simultaneous: Some(1),
        ..CustomSound::default()
    });
    spec
}

fn read_bytes(p: &Proj, rel: &str) -> Vec<u8> {
    std::fs::read(p.root.join(rel).as_std_path()).unwrap()
}

fn apply_request(req: &DesignerExportPlanRequest, plan: &WritePlanDto) -> DesignerApplyPlanRequest {
    DesignerApplyPlanRequest {
        plan_id: plan.plan_id.clone(),
        request: req.clone(),
        backup: true,
        dry_apply: true,
    }
}

fn codes(plan: &WritePlanDto) -> Vec<String> {
    plan.diagnostics.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn a_design_with_assets_plans_copies_a_sound_def_and_the_paths() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let spec = spec_with_assets(&art);
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);

    let paths: Vec<(&str, FileKindDto, FileActionDto)> = plan
        .files
        .iter()
        .map(|x| (x.path.as_str(), x.kind, x.action))
        .collect();
    // copies come first, then the definitions
    assert_eq!(paths.len(), 6, "{paths:?}");
    assert!(paths[..4].iter().all(|(_, k, _)| *k == FileKindDto::Copy));
    assert!(
        paths[4..]
            .iter()
            .all(|(_, k, _)| *k == FileKindDto::VanillaDefs)
    );
    for want in [
        WEAPON_TEX,
        PROJECTILE_TEX,
        CLIP_A,
        CLIP_B,
        SOUND_DEFS,
        WEAPON_DEF,
    ] {
        assert!(paths.iter().any(|(path, ..)| *path == want), "{want}");
    }
    assert!(paths.iter().all(|(_, _, a)| *a == FileActionDto::Create));

    let texture = plan.files.iter().find(|x| x.path == WEAPON_TEX).unwrap();
    let copy = texture.copy.as_ref().unwrap();
    assert_eq!((copy.width, copy.height), (Some(64), Some(64)));
    assert_eq!(copy.sha256, sha256_hex(&build_png(64, 64)));
    assert_eq!(copy.existing_sha256, None);
    assert_eq!(texture.rendered, "");
    assert_eq!(texture.bytes, copy.bytes);

    let weapon = plan.files.iter().find(|x| x.path == WEAPON_DEF).unwrap();
    assert!(
        weapon
            .rendered
            .contains("<texPath>Things/Item/Equipment/WeaponRanged/RS_NewRifle</texPath>")
    );
    assert!(
        weapon
            .rendered
            .contains("<texPath>Things/Projectile/RS_NewBullet</texPath>")
    );
    assert!(
        weapon
            .rendered
            .contains("<soundCast>RS_NewRifle_Shot</soundCast>")
    );
    assert!(!weapon.rendered.contains("CombatExtended"));
    assert!(!codes(&plan).contains(&"design.texture-reserved".to_owned()));

    let sound = plan.files.iter().find(|x| x.path == SOUND_DEFS).unwrap();
    assert_text_golden("assets_sound_def.xml", &sound.rendered);
    assert_text_golden("assets_weapon.xml", &weapon.rendered);
}

#[test]
fn apply_copies_verified_files_and_a_second_plan_is_all_unchanged() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let spec = spec_with_assets(&art);
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let report = apply_plan(
        &f.ctx,
        apply_request(&req, &plan),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.written.len(), 6);
    assert!(report.written.iter().all(|w| w.verified));
    // assets are written before the definitions that name them
    let order: Vec<&str> = report.written.iter().map(|w| w.path.as_str()).collect();
    assert!(order.iter().position(|x| *x == CLIP_A) < order.iter().position(|x| *x == SOUND_DEFS));
    let tex = report
        .written
        .iter()
        .find(|w| w.path == WEAPON_TEX)
        .unwrap();
    assert_eq!(tex.kind, Some(FileKindDto::Copy));
    assert_eq!(
        tex.sha256.as_deref(),
        Some(sha256_hex(&build_png(64, 64)).as_str())
    );
    assert_eq!(read_bytes(&p, WEAPON_TEX), build_png(64, 64));
    assert_eq!(read_bytes(&p, CLIP_A), build_wav(1, 22_050, 2000));
    assert_eq!(read_bytes(&p, CLIP_B), build_ogg_vorbis(1, 44_100));

    let again = export_plan(&f.ctx, req).unwrap();
    assert!(
        again
            .files
            .iter()
            .all(|x| x.action == FileActionDto::Unchanged),
        "{:?}",
        again
            .files
            .iter()
            .map(|x| (&x.path, x.action))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_different_file_at_the_target_is_replaced_after_a_backup_outside_the_project() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.assets.texture = Some(art.put("rifle.png", &build_png(32, 32)));
    // an older texture of another size is already there
    let old = build_png(8, 8);
    std::fs::create_dir_all(
        p.root
            .join("Textures/Things/Item/Equipment/WeaponRanged")
            .as_std_path(),
    )
    .unwrap();
    std::fs::write(p.root.join(WEAPON_TEX).as_std_path(), &old).unwrap();
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let file = plan.files.iter().find(|x| x.path == WEAPON_TEX).unwrap();
    assert_eq!(file.action, FileActionDto::Replace);
    assert_eq!(
        file.copy.as_ref().unwrap().existing_sha256.as_deref(),
        Some(sha256_hex(&old).as_str())
    );
    let report = apply_plan(
        &f.ctx,
        apply_request(&req, &plan),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let written = report
        .written
        .iter()
        .find(|w| w.path == WEAPON_TEX)
        .unwrap();
    let backup = written.backup_path.as_deref().expect("a backup was made");
    assert_eq!(std::fs::read(backup).unwrap(), old);
    assert!(
        !backup.starts_with(p.root.as_str()),
        "the backup is outside the project: {backup}"
    );
    assert_eq!(read_bytes(&p, WEAPON_TEX), build_png(32, 32));
}

#[test]
fn a_source_that_changes_before_apply_is_refused_and_nothing_is_written() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let spec = spec_with_assets(&art);
    let req = request(&p, &spec);
    let prepared = prepare(&f.ctx, &req).unwrap();
    assert!(!prepared.plan.has_errors());
    // the source is edited after the plan was made
    art.put("rifle.png", &build_png(48, 48));
    let err = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::ApplyRefused { .. }), "{err:?}");
    assert!(!p.root.join(WEAPON_TEX).as_std_path().exists());
    assert!(!p.root.join(WEAPON_DEF).as_std_path().exists());
}

#[test]
fn a_reviewed_plan_goes_stale_when_a_source_changes() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let spec = spec_with_assets(&art);
    let req = request(&p, &spec);
    let reviewed = export_plan(&f.ctx, req.clone()).unwrap();
    art.put("bang.wav", &build_wav(1, 22_050, 3000));
    let err = apply_plan(
        &f.ctx,
        apply_request(&req, &reviewed),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::PlanStale { .. }), "{err:?}");
}

#[test]
fn a_target_that_changes_before_apply_is_refused() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.assets.texture = Some(art.put("rifle.png", &build_png(32, 32)));
    let req = request(&p, &spec);
    let prepared = prepare(&f.ctx, &req).unwrap();
    // another program drops a file at the target after planning
    put(&p.root, WEAPON_TEX, "someone else's art");
    let err = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::ApplyRefused { .. }), "{err:?}");
    assert_eq!(read_bytes(&p, WEAPON_TEX), b"someone else's art");
}

fn texture_error(art: &Art, name: &str, bytes: &[u8]) -> Vec<String> {
    let f = fixture(false);
    let p = project(&f, &[]);
    let mut spec = ranged();
    spec.assets.texture = Some(art.put(name, bytes));
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors, "{name} should be refused");
    assert!(plan.files.is_empty());
    assert!(!p.root.join("Textures").as_std_path().exists());
    codes(&plan)
}

#[test]
fn hostile_textures_are_refused_with_their_codes() {
    let art = Art::new();
    let png = build_png(32, 32);
    assert!(texture_error(&art, "cut.png", &png[..20]).contains(&"design.texture-not-png".into()));
    assert!(
        texture_error(&art, "half.png", &png[..png.len() - 7])
            .contains(&"design.texture-not-png".into())
    );
    assert!(
        texture_error(&art, "huge.png", &build_png(9000, 9000))
            .contains(&"design.texture-too-large".into())
    );
    assert!(
        texture_error(&art, "text.png", b"definitely not an image")
            .contains(&"design.texture-not-png".into())
    );
    assert!(
        texture_error(&art, "audio.png", &build_wav(1, 8000, 10))
            .contains(&"design.texture-not-png".into())
    );
    assert!(texture_error(&art, "empty.png", b"").contains(&"design.texture-not-png".into()));
}

#[test]
fn a_missing_texture_and_a_texture_over_the_size_limit_are_refused() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.assets.texture = Some(art.dir.join("gone.png").to_string());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert_eq!(codes(&plan), ["design.texture-missing"]);
    let big = art.dir.join("big.png");
    std::fs::File::create(big.as_std_path())
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    spec.assets.texture = Some(big.to_string());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert_eq!(codes(&plan), ["design.texture-too-large"]);
}

#[test]
fn hostile_clips_are_refused_with_their_codes() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut bad_chunk = build_wav(1, 8000, 100);
    let at = bad_chunk.len() - 200 - 4;
    bad_chunk[at..at + 4].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("chunk.wav", bad_chunk, "design.sound-clip-not-audio"),
        (
            "text.wav",
            b"RIFFnope".to_vec(),
            "design.sound-clip-not-audio",
        ),
        ("image.wav", build_png(4, 4), "design.sound-clip-not-audio"),
        ("empty.ogg", Vec::new(), "design.sound-clip-not-audio"),
    ];
    for (name, bytes, want) in cases {
        let mut spec = ranged();
        spec.sounds.shot = Some(CustomSound {
            clips: vec![art.put(name, &bytes)],
            ..CustomSound::default()
        });
        let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
        assert!(plan.has_errors, "{name}");
        assert!(
            codes(&plan).contains(&want.to_owned()),
            "{name}: {:?}",
            codes(&plan)
        );
        assert!(plan.files.is_empty());
    }
    // a missing clip and a clip over the limit
    let mut spec = ranged();
    let huge = art.dir.join("huge.wav");
    std::fs::File::create(huge.as_std_path())
        .unwrap()
        .set_len(20 * 1024 * 1024 + 1)
        .unwrap();
    spec.sounds.shot = Some(CustomSound {
        clips: vec![art.dir.join("gone.wav").to_string(), huge.to_string()],
        ..CustomSound::default()
    });
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    let got = codes(&plan);
    assert!(
        got.contains(&"design.sound-clip-missing".to_owned()),
        "{got:?}"
    );
    assert!(
        got.contains(&"design.sound-clip-too-large".to_owned()),
        "{got:?}"
    );
}

#[test]
fn a_stereo_clip_is_only_a_warning_and_a_melee_weapon_cannot_have_a_shot_sound() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.sounds.shot = Some(CustomSound {
        clips: vec![art.put("stereo.wav", &build_wav(2, 44_100, 100))],
        ..CustomSound::default()
    });
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(!plan.has_errors);
    assert!(codes(&plan).contains(&"design.sound-stereo".to_owned()));
    let mut blade = melee();
    blade.sounds.shot = Some(CustomSound::default());
    let plan = export_plan(&f.ctx, request(&p, &blade)).unwrap();
    assert!(codes(&plan).contains(&"design.sound-needs-ranged".to_owned()));
}

#[test]
fn a_custom_sound_replaces_a_typed_shot_sound_and_a_second_sound_joins_the_same_file() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut first = ranged();
    first.ranged.as_mut().unwrap().sound_cast = Some("RS_Shot".into());
    first.sounds.shot = Some(CustomSound {
        clips: vec![art.put("one.wav", &build_wav(1, 8000, 100))],
        ..CustomSound::default()
    });
    let req = request(&p, &first);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(codes(&plan).contains(&"design.sound-cast-replaced".to_owned()));
    apply_plan(
        &f.ctx,
        apply_request(&req, &plan),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();

    // a second weapon adds a marked section to the sound file the first one made
    let mut second = ranged();
    second.identity.def_name = "RS_OtherRifle".into();
    second.identity.label = "other rifle".into();
    second.ranged.as_mut().unwrap().projectile =
        Some(rimstudio_design::model::ProjectileChoice::Inline(
            rimstudio_design::model::ProjectileSpec {
                def_name: "RS_OtherBullet".into(),
                parent: Some("RS_BaseBullet".into()),
                damage_def: Some("RS_Damage".into()),
                ..Default::default()
            },
        ));
    second.sounds.shot = Some(CustomSound {
        clips: vec![art.put("two.wav", &build_wav(1, 8000, 100))],
        ..CustomSound::default()
    });
    let req2 = request(&p, &second);
    let plan2 = export_plan(&f.ctx, req2.clone()).unwrap();
    assert!(!plan2.has_errors, "{:?}", plan2.diagnostics);
    let sound_file = plan2.files.iter().find(|x| x.path == SOUND_DEFS).unwrap();
    assert_eq!(sound_file.action, FileActionDto::UpdateRegion);
    apply_plan(
        &f.ctx,
        apply_request(&req2, &plan2),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let text = String::from_utf8(read_bytes(&p, SOUND_DEFS)).unwrap();
    assert!(text.contains("<defName>RS_NewRifle_Shot</defName>"));
    assert!(text.contains("<defName>RS_OtherRifle_Shot</defName>"));
    assert_text_golden("assets_sound_def_two.xml", &text);
}

#[cfg(unix)]
#[test]
fn a_link_is_never_a_source_and_a_link_in_the_project_is_never_a_target() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let real = art.put("real.png", &build_png(16, 16));
    let link = art.dir.join("link.png");
    std::os::unix::fs::symlink(&real, link.as_std_path()).unwrap();
    let mut spec = ranged();
    spec.assets.texture = Some(link.to_string());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert_eq!(codes(&plan), ["design.asset-refused"]);

    // the Textures folder of the project is a link out of the project
    let outside = art.dir.join("elsewhere");
    std::fs::create_dir_all(outside.as_std_path()).unwrap();
    std::os::unix::fs::symlink(outside.as_std_path(), p.root.join("Textures").as_std_path())
        .unwrap();
    spec.assets.texture = Some(real);
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors || plan.files.iter().all(|x| x.kind != FileKindDto::Copy));
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.path-refused")
    );
    assert!(
        std::fs::read_dir(outside.as_std_path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn a_relative_source_cannot_climb_out_of_the_project() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let mut spec = ranged();
    spec.assets.texture = Some("../outside.png".into());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert_eq!(codes(&plan), ["design.asset-refused"]);
    // art kept inside the project (Source/Art) is fine
    put(&p.root, "Source/Art/rifle.png", "placeholder");
    std::fs::write(
        p.root.join("Source/Art/rifle.png").as_std_path(),
        build_png(24, 24),
    )
    .unwrap();
    spec.assets.texture = Some("Source/Art/rifle.png".into());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
}

#[test]
fn a_hostile_name_cannot_steer_a_target_out_of_the_project() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.identity.def_name = "../../escape".into();
    spec.assets.texture = Some(art.put("a.png", &build_png(8, 8)));
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
    let mut spec = ranged();
    spec.sounds.shot = Some(CustomSound {
        def_name: Some("../../evil".into()),
        clips: vec![art.put("a.wav", &build_wav(1, 8000, 10))],
        ..CustomSound::default()
    });
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors);
    assert!(codes(&plan).contains(&"design.name-invalid".to_owned()));
}

#[test]
fn imported_assets_show_in_the_project_tree_and_the_layout_check_is_quiet_about_them() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let spec = spec_with_assets(&art);
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        apply_request(&req, &plan),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let tree = rimstudio_toolkit::project::tree(
        f.ctx.env(),
        &ProjectTreeRequest {
            project_id: p.id.clone(),
            max_nodes: None,
        },
    )
    .unwrap();
    assert_eq!(tree.counts.textures, 2);
    assert_eq!(tree.counts.sounds, 2);
    assert_eq!(tree.counts.weapon_defs, 1);
    let check = rimstudio_toolkit::project::layout_check(
        f.ctx.env(),
        &ProjectLayoutCheckRequest {
            project_id: p.id.clone(),
        },
    )
    .unwrap();
    assert!(
        !check
            .issues
            .iter()
            .any(|i| i.code == "layout.texture-missing"),
        "{:?}",
        check.issues.iter().map(|i| &i.code).collect::<Vec<_>>()
    );
}

#[test]
fn the_info_query_describes_images_and_sounds_without_decoding_them() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let png = art.put("a.png", &build_png(40, 20));
    let ask = |path: &str, project_id: Option<&str>| {
        asset_info(
            &f.ctx,
            DesignerAssetInfoRequest {
                path: path.to_owned(),
                project_id: project_id.map(str::to_owned),
            },
        )
        .unwrap()
    };
    let info = ask(&png, None);
    assert_eq!(info.status, AssetStatusDto::Found);
    assert_eq!(info.kind, Some(AssetKindDto::Png));
    assert_eq!((info.width, info.height), (Some(40), Some(20)));
    assert_eq!(
        info.sha256.as_deref(),
        Some(sha256_hex(&build_png(40, 20)).as_str())
    );
    assert!(
        info.preview
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        info.diagnostics
            .iter()
            .any(|d| d.code == "design.texture-not-square")
    );

    let wav = ask(&art.put("s.wav", &build_wav(2, 44_100, 44_100)), None);
    assert_eq!(wav.kind, Some(AssetKindDto::Wav));
    assert_eq!(
        (wav.channels, wav.sample_rate, wav.duration_ms),
        (Some(2), Some(44_100), Some(1000))
    );
    assert!(wav.preview.is_none());
    assert!(
        wav.diagnostics
            .iter()
            .any(|d| d.code == "design.sound-stereo")
    );

    let ogg = ask(&art.put("s.ogg", &build_ogg_vorbis(1, 48_000)), None);
    assert_eq!(
        (ogg.kind, ogg.channels, ogg.sample_rate),
        (Some(AssetKindDto::Ogg), Some(1), Some(48_000))
    );

    let text = ask(&art.put("t.png", b"hello"), None);
    assert_eq!(text.kind, Some(AssetKindDto::Unknown));
    assert!(
        text.diagnostics
            .iter()
            .any(|d| d.code == "design.texture-not-png")
    );

    assert_eq!(
        ask(art.dir.join("none.png").as_str(), None).status,
        AssetStatusDto::Missing
    );
    assert_eq!(ask(art.dir.as_str(), None).status, AssetStatusDto::Refused);
    assert_eq!(ask("relative.png", None).status, AssetStatusDto::Refused);

    // a relative path is read against the project
    put(&p.root, "Source/Art/x.png", "x");
    std::fs::write(
        p.root.join("Source/Art/x.png").as_std_path(),
        build_png(8, 8),
    )
    .unwrap();
    let rel = ask("Source/Art/x.png", Some(&p.id));
    assert_eq!(rel.status, AssetStatusDto::Found);
    assert!(matches!(
        asset_info(
            &f.ctx,
            DesignerAssetInfoRequest {
                path: "x.png".into(),
                project_id: Some("nope".into())
            }
        ),
        Err(ToolkitError::ProjectNotOpen { .. })
    ));

    let big = art.dir.join("big.png");
    std::fs::File::create(big.as_std_path())
        .unwrap()
        .set_len(9 * 1024 * 1024)
        .unwrap();
    let info = ask(big.as_str(), None);
    assert_eq!(info.status, AssetStatusDto::TooLarge);
    assert_eq!(info.bytes, Some(9 * 1024 * 1024));
}

#[cfg(unix)]
#[test]
fn the_info_query_refuses_a_link() {
    let f = fixture(false);
    let art = Art::new();
    let real = art.put("real.png", &build_png(8, 8));
    let link = art.dir.join("link.png");
    std::os::unix::fs::symlink(&real, link.as_std_path()).unwrap();
    let info = asset_info(
        &f.ctx,
        DesignerAssetInfoRequest {
            path: link.to_string(),
            project_id: None,
        },
    )
    .unwrap();
    assert_eq!(info.status, AssetStatusDto::Refused);
    assert!(info.preview.is_none());
}

#[test]
fn a_design_without_imports_plans_exactly_what_it_did_before() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].kind, FileKindDto::VanillaDefs);
    assert!(plan.files[0].copy.is_none());
    assert!(codes(&plan).contains(&"design.texture-reserved".to_owned()));
}

/// The standard fixture install with extra definitions in its core file.
fn fixture_with(extra: Vec<rimstudio_core::tree::Node>) -> common::Fixture {
    use rimstudio_core::tree::NodeBuilder;
    let mut defs = common::core_defs_n(14, 8);
    for base in ["RS_BaseGun", "RS_BaseMelee", "RS_BaseBullet"] {
        defs.push(
            NodeBuilder::new("ThingDef")
                .attr("Name", base)
                .attr("Abstract", "True")
                .text_elem("category", "Item")
                .build(),
        );
    }
    defs.extend(extra);
    let install = rimstudio_testing::install_tree::InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .mod_folder(common::extra_mod())
        .build_temp()
        .unwrap();
    let session = common::open_session(&install, false);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = rimstudio_io::roots::DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = rimstudio_testing::fakes::FakeClock::new(
        rimstudio_testing::fakes::FakeClock::DEFAULT_START_MS,
    );
    let ctx = rimstudio_toolkit::designer::Ctx::new(
        session.clone(),
        &roots,
        std::sync::Arc::new(clock.clone()),
    )
    .unwrap();
    common::Fixture {
        install,
        session,
        tmp,
        roots,
        clock,
        ctx,
    }
}

#[test]
fn a_sound_def_name_that_a_loaded_def_already_uses_is_refused_and_a_typed_one_that_is_gone_is_not_checked()
 {
    use rimstudio_core::tree::NodeBuilder;
    let taken = NodeBuilder::new("SoundDef")
        .text_elem("defName", "RS_NewRifle_Shot")
        .build();
    let f = fixture_with(vec![taken]);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.sounds.shot = Some(CustomSound {
        clips: vec![art.put("a.wav", &build_wav(1, 8000, 10))],
        ..CustomSound::default()
    });
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors);
    assert!(
        codes(&plan).contains(&"design.sound-duplicate".to_owned()),
        "{:?}",
        codes(&plan)
    );
    // another name is fine
    if let Some(s) = spec.sounds.shot.as_mut() {
        s.def_name = Some("RS_Different_Shot".into());
    }
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    // a typed soundCast that does not exist is replaced by the custom sound, so it is not reported
    spec.ranged.as_mut().unwrap().sound_cast = Some("RS_NoSuchSound".into());
    let plan = export_plan(&f.ctx, request(&p, &spec)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert!(!codes(&plan).contains(&"design.ref-unresolved".to_owned()));
}

#[test]
fn a_read_only_target_is_never_replaced() {
    let f = fixture(false);
    let p = project(&f, &[]);
    let art = Art::new();
    let mut spec = ranged();
    spec.assets.texture = Some(art.put("rifle.png", &build_png(32, 32)));
    let old = build_png(8, 8);
    std::fs::create_dir_all(
        p.root
            .join("Textures/Things/Item/Equipment/WeaponRanged")
            .as_std_path(),
    )
    .unwrap();
    let target = p.root.join(WEAPON_TEX);
    std::fs::write(target.as_std_path(), &old).unwrap();
    let mut perms = std::fs::metadata(target.as_std_path())
        .unwrap()
        .permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(target.as_std_path(), perms).unwrap();
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let err = apply_plan(
        &f.ctx,
        apply_request(&req, &plan),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            ToolkitError::PathRefused { .. } | ToolkitError::ApplyRefused { .. }
        ),
        "{err:?}"
    );
    assert_eq!(read_bytes(&p, WEAPON_TEX), old);
    // nothing else was written either: the refusal comes before the first byte
    assert!(!p.root.join(WEAPON_DEF).as_std_path().exists());
}
