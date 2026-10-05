//! Fixtures of the project, plan, apply and convert tests: a fictional mod folder in a temporary
//! directory, ready-made specs for a vanilla and a Combat Extended design, and a text golden helper.
//! Every name starts with `RS_`; every number is invented.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::jobs::NoopProgress;
use rimstudio_core::tree::NodeBuilder;
use rimstudio_design::model::{
    CePatchSpec, DesignSpec, ParentRef, ProjectileChoice, ProjectileSpec, ScalarField, Sourced,
    TechLevel, ToolSpec, ValueSource,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{DesignerExportPlanRequest, DraftDto};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::InstallBuilder;
use rimstudio_toolkit::designer::Ctx;

use super::common::{self, Fixture};

pub use super::common::dto;

fn abstract_base(name: &str) -> rimstudio_core::tree::Node {
    NodeBuilder::new("ThingDef")
        .attr("Name", name)
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .build()
}

/// The standard fixture install plus the abstract bases `RS_BaseGun`, `RS_BaseMelee` and `RS_BaseBullet`
/// that designs name as their parents.
pub fn fixture(with_ce: bool) -> Fixture {
    let mut defs = common::core_defs_n(14, 8);
    for base in ["RS_BaseGun", "RS_BaseMelee", "RS_BaseBullet"] {
        defs.push(abstract_base(base));
    }
    let mut b = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .mod_folder(common::extra_mod());
    if with_ce {
        b = b.mod_folder(common::ce_mod());
    }
    let install = b.build_temp().unwrap();
    let session = common::open_session(&install, with_ce);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let ctx = Ctx::new(session.clone(), &roots, Arc::new(clock.clone())).unwrap();
    Fixture {
        install,
        session,
        tmp,
        roots,
        clock,
        ctx,
    }
}

/// A fictional mod folder in a temporary directory, registered as a project.
pub struct Proj {
    pub tmp: tempfile::TempDir,
    pub root: Utf8PathBuf,
    pub id: String,
}

/// The text of an `About.xml` with the given package id and supported versions.
pub fn about(id: &str, versions: &[&str]) -> String {
    let items: String = versions
        .iter()
        .map(|v| format!("    <li>{v}</li>\n"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ModMetaData>\n  <name>RS Test Mod</name>\n  <packageId>{id}</packageId>\n  <author>RS Author</author>\n  <supportedVersions>\n{items}  </supportedVersions>\n  <description>fictional</description>\n</ModMetaData>\n"
    )
}

/// Writes a file below `root`, creating folders.
pub fn put(root: &Utf8PathBuf, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(path.as_std_path(), text).unwrap();
}

/// A project with an About file (`rs.testmod`, versions 1.5 and 1.6) and the given extra files.
pub fn project(f: &Fixture, files: &[(&str, &str)]) -> Proj {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().join("RS_TestMod")).unwrap();
    std::fs::create_dir_all(root.as_std_path()).unwrap();
    if !files
        .iter()
        .any(|(p, _)| p.eq_ignore_ascii_case("About/About.xml"))
    {
        put(
            &root,
            "About/About.xml",
            &about("rs.testmod", &["1.5", "1.6"]),
        );
    }
    for (rel, text) in files {
        put(&root, rel, text);
    }
    let record = f.ctx.env().projects().open(&root).unwrap();
    Proj {
        tmp,
        id: record.id.as_str().to_owned(),
        root,
    }
}

/// A Combat Extended block for the fictional CE mod of the fixture install (one ammo set).
pub fn ce_block() -> CePatchSpec {
    CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        default_projectile: Some("RS_CeBullet1".into()),
        weapon_tag_class: Some("RS_CE_Class".into()),
        magazine_size: Some(Sourced::typed(30)),
        reload_time: Some(Sourced::typed(4.0)),
        bulk: Some(Sourced::typed(6.5)),
        sway_factor: Some(Sourced::typed(1.2)),
        shot_spread: Some(Sourced::typed(0.08)),
        ..CePatchSpec::default()
    }
}

/// A complete ranged design (every number typed), Combat Extended off.
pub fn ranged() -> DesignSpec {
    let mut spec = common::typed_ranged();
    spec.parent = Some(ParentRef::named("RS_BaseGun"));
    if let Some(r) = spec.ranged.as_mut() {
        r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
            def_name: "RS_NewBullet".into(),
            label: "new bullet".into(),
            parent: Some("RS_BaseBullet".into()),
            damage_def: Some("RS_Damage".into()),
            ..ProjectileSpec::default()
        }));
    }
    spec
}

/// A complete ranged design with the optional Combat Extended block on.
pub fn ranged_ce() -> DesignSpec {
    let mut spec = ranged();
    spec.ce = Some(ce_block());
    spec
}

/// A complete melee design (one tool), Combat Extended off.
pub fn melee() -> DesignSpec {
    let mut spec = common::melee_spec();
    spec.parent = Some(ParentRef::named("RS_BaseMelee"));
    spec.tech_level = Some(TechLevel::Industrial);
    spec.offer(ScalarField::Mass, 1.4, ValueSource::Typed);
    spec.offer(ScalarField::WorkToMake, 4200.0, ValueSource::Typed);
    spec.tools = vec![ToolSpec::new("head", &["Cut"]).with_numbers(11.0, 2.1, ValueSource::Typed)];
    spec
}

/// A complete melee design with the optional Combat Extended block on.
pub fn melee_ce() -> DesignSpec {
    let mut spec = melee();
    spec.ce = Some(CePatchSpec {
        bulk: Some(Sourced::typed(5.5)),
        melee_crit_chance: Some(Sourced::typed(0.1)),
        melee_parry_chance: Some(Sourced::typed(0.2)),
        melee_dodge_chance: Some(Sourced::typed(-0.05)),
        tool_penetration: vec![rimstudio_design::model::CeToolPenetration {
            tool: "head".into(),
            sharp: Some(Sourced::typed(0.6)),
            blunt: Some(Sourced::typed(3.0)),
        }],
        ..CePatchSpec::default()
    });
    spec
}

/// The export request of a spec for a project.
pub fn request(project: &Proj, spec: &DesignSpec) -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        project_id: project.id.clone(),
        draft: dto(spec.clone()),
        convert: None,
        accept_suggestions: None,
    }
}

/// The draft DTO of a spec.
pub fn draft(spec: &DesignSpec) -> DraftDto {
    dto(spec.clone())
}

/// The context of a fixture, shared.
pub fn ctx(f: &Fixture) -> &Ctx {
    &f.ctx
}

pub fn noop() -> Arc<NoopProgress> {
    Arc::new(NoopProgress)
}

/// Compares `text` with `tests/golden/<name>`; with `UPDATE_GOLDENFILES=1` the file is rewritten.
pub fn assert_text_golden(name: &str, text: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    let update = std::env::var("UPDATE_GOLDENFILES").is_ok_and(|v| !v.is_empty() && v != "0");
    if update {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {name}; run with UPDATE_GOLDENFILES=1"));
    assert_eq!(
        text, expected,
        "golden {name} differs (UPDATE_GOLDENFILES=1 rewrites it)"
    );
}
