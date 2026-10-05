//! Fixtures of the app tests: a fictional install with guns, melee weapons and a fictional Combat
//! Extended mod on a real temporary disk, fake ports for everything else, and an `AppContext` booted
//! through the same path as production. Every name starts with `RS_` and every number is invented;
//! nothing here comes from a game install.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_app::context::{AppOptions, Platform};
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{AppContext, BootInput, boot};
use rimstudio_core::os::Os;
use rimstudio_core::ports::InstallSource;
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{TypeInfo, TypeTable};
use rimstudio_design::model::{
    CostEntry, DesignSpec, Draft, ParentRef, ProjectileChoice, ProjectileSpec, ScalarField,
    TechLevel, ValueSource,
};
use rimstudio_io::real_fs::RealFs;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::DraftDto;
use rimstudio_testing::fakes::{
    FakeClock, FakeCredentialStore, FakeEnv, FakeInstallSource, FakeLauncher, FakeLinkBackend,
    FakeProcessProbe, FakeRegistry, FakeSandbox,
};
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder, TempInstall};
use rimstudio_toolkit::designer::dto::draft_to_dto;
use serde_json::{Value, json};

pub const CE_ID: &str = "ceteam.combatextended";

pub fn types() -> Arc<TypeTable> {
    let list = vec![
        (
            "Verse.Def".to_owned(),
            TypeInfo::with_base("Verse.Editable"),
        ),
        (
            "Verse.ThingDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
        ("Verse.StatDef".to_owned(), TypeInfo::with_base("Verse.Def")),
        (
            "Verse.DamageDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
        (
            "CombatExtended.AmmoSetDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
    ];
    Arc::new(TypeTable::new(list).unwrap())
}

fn stat_def(name: &str) -> Node {
    NodeBuilder::new("StatDef")
        .text_elem("defName", name)
        .text_elem("defaultBaseValue", "0")
        .build()
}

fn projectile(name: &str, damage: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("projectile", |p| {
            p.text_elem("damageDef", "RS_Damage")
                .text_elem("damageAmountBase", damage.to_string())
                .text_elem("speed", "55")
        })
        .build()
}

fn ingredient(name: &str, value: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("category", "Item")
        .elem("statBases", |s| {
            s.text_elem("MarketValue", value.to_string())
        })
        .build()
}

fn gun(name: &str, projectile: &str, i: u32, tier: &str, tag: &str) -> Node {
    let f = f64::from(i);
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", (1.5 + 0.3 * f).to_string())
                .text_elem("RangedWeapon_Cooldown", (1.0 + 0.1 * f).to_string())
                .text_elem("AccuracyTouch", "0.7")
                .text_elem("AccuracyShort", (0.8 - 0.01 * f).to_string())
                .text_elem("AccuracyMedium", (0.65 - 0.01 * f).to_string())
                .text_elem("AccuracyLong", (0.5 - 0.01 * f).to_string())
                .text_elem("WorkToMake", (6000.0 + 700.0 * f).to_string())
        })
        .elem("costList", |c| c.text_elem("RS_Steel", "30"))
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", projectile)
                    .text_elem("warmupTime", (0.8 + 0.07 * f).to_string())
                    .text_elem("range", (18.0 + 1.5 * f).to_string())
            })
        })
        .elem("weaponTags", |w| w.li(tag))
        .build()
}

fn melee(name: &str, i: u32, tier: &str, cap: &str) -> Node {
    let f = f64::from(i);
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", (0.8 + 0.2 * f).to_string())
                .text_elem("WorkToMake", (3000.0 + 400.0 * f).to_string())
        })
        .elem("weaponTags", |w| w.li("RS_Melee"))
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "head")
                    .elem("capacities", |c| c.li(cap))
                    .text_elem("power", (7.0 + 1.1 * f).to_string())
                    .text_elem("cooldownTime", (1.8 + 0.1 * f).to_string())
            })
        })
        .build()
}

fn abstract_base(name: &str) -> Node {
    NodeBuilder::new("ThingDef")
        .attr("Name", name)
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .build()
}

fn core_defs() -> Vec<Node> {
    let mut defs = vec![
        stat_def("Mass"),
        stat_def("RangedWeapon_Cooldown"),
        NodeBuilder::new("DamageDef")
            .text_elem("defName", "RS_Damage")
            .text_elem("defaultDamage", "10")
            .text_elem("defaultArmorPenetration", "0.2")
            .build(),
        ingredient("RS_Steel", 2.0),
        ingredient("RS_Part", 40.0),
    ];
    for i in 0..14_u32 {
        defs.push(projectile(&format!("RS_Shot{i:02}"), 8.0 + f64::from(i)));
        let tag = if i % 3 == 2 { "RS_Pistol" } else { "RS_Rifle" };
        let tier = if i < 8 { "Industrial" } else { "Spacer" };
        defs.push(gun(
            &format!("RS_Gun{i:02}"),
            &format!("RS_Shot{i:02}"),
            i,
            tier,
            tag,
        ));
    }
    for i in 0..8_u32 {
        let cap = if i % 2 == 0 { "Cut" } else { "Blunt" };
        let tier = if i < 4 { "Medieval" } else { "Industrial" };
        defs.push(melee(&format!("RS_Blade{i:02}"), i, tier, cap));
    }
    for base in ["RS_BaseGun", "RS_BaseMelee", "RS_BaseBullet"] {
        defs.push(abstract_base(base));
    }
    defs
}

fn ce_mod() -> ModFolder {
    let ammo_set = NodeBuilder::new("CombatExtended.AmmoSetDef")
        .text_elem("defName", "RS_AmmoSetA")
        .elem("ammoTypes", |t| t.text_elem("RS_AmmoA1", "RS_CeBullet1"))
        .build();
    let bullet = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_CeBullet1")
        .elem("projectile", |p| {
            p.text_elem("damageAmountBase", "9")
                .text_elem("armorPenetrationSharp", "2.5")
                .text_elem("armorPenetrationBlunt", "14")
                .text_elem("speed", "120")
        })
        .build();
    ModFolder::new("RS_CE", CE_ID)
        .name("Combat Extended")
        .defs_file("RS_Ce.xml", vec![ammo_set, bullet])
}

/// The text of an `About.xml`.
pub fn about(id: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ModMetaData>\n  <name>RS Test Mod</name>\n  <packageId>{id}</packageId>\n  <author>RS Author</author>\n  <supportedVersions>\n    <li>1.6</li>\n  </supportedVersions>\n  <description>fictional</description>\n</ModMetaData>\n"
    )
}

/// A gun definition of a mod that has not been converted.
pub const PROJECT_DEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Defs>
  <ThingDef>
    <defName>RS_ProjGun</defName>
    <label>proj gun</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases>
      <Mass>3</Mass>
      <RangedWeapon_Cooldown>1.5</RangedWeapon_Cooldown>
      <AccuracyTouch>0.7</AccuracyTouch>
      <AccuracyShort>0.7</AccuracyShort>
      <AccuracyMedium>0.6</AccuracyMedium>
      <AccuracyLong>0.5</AccuracyLong>
    </statBases>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_Shot00</defaultProjectile>
        <warmupTime>1</warmupTime>
        <range>25</range>
      </li>
    </verbs>
    <weaponTags><li>RS_Rifle</li></weaponTags>
  </ThingDef>
</Defs>
"#;

/// A complete ranged design (every number typed), Combat Extended off.
pub fn ranged_spec() -> DesignSpec {
    let mut spec = DesignSpec::new_ranged("RS_NewRifle", "new rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some("RS_Rifle".into());
    spec.cost_list = vec![CostEntry::new("RS_Steel", 30.0)];
    for (f, v) in [
        (ScalarField::Damage, 12.0),
        (ScalarField::Warmup, 1.0),
        (ScalarField::Cooldown, 1.5),
        (ScalarField::Range, 28.0),
        (ScalarField::Mass, 3.5),
        (ScalarField::WorkToMake, 9000.0),
        (ScalarField::AccuracyTouch, 0.7),
        (ScalarField::AccuracyShort, 0.75),
        (ScalarField::AccuracyMedium, 0.6),
        (ScalarField::AccuracyLong, 0.45),
    ] {
        spec.offer(f, v, ValueSource::Typed);
    }
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

pub fn draft_dto(spec: &DesignSpec) -> DraftDto {
    draft_to_dto(&Draft::new(spec.clone())).unwrap()
}

pub fn draft_json(spec: &DesignSpec) -> Value {
    serde_json::to_value(draft_dto(spec)).unwrap()
}

/// Everything a test needs.
pub struct Fixture {
    pub tmp: tempfile::TempDir,
    pub install: TempInstall,
    pub base: Utf8PathBuf,
    pub clock: FakeClock,
    pub app: AppContext,
}

pub fn platform(base: &Utf8Path, clock: &FakeClock) -> Platform {
    let home = base.join("home");
    std::fs::create_dir_all(home.as_std_path()).unwrap();
    Platform {
        os: Os::Linux,
        clock: Arc::new(clock.clone()),
        env: Arc::new(
            FakeEnv::new()
                .with_home(home)
                .with_exe_dir(base.join("exe")),
        ),
        registry: Arc::new(FakeRegistry::new()),
        fs: Arc::new(RealFs::new()),
        process: Arc::new(FakeProcessProbe::new()),
        links: Arc::new(FakeLinkBackend::new()),
        launcher: Arc::new(FakeLauncher::new()),
        credentials: Arc::new(FakeCredentialStore::new()),
        sandbox: Arc::new(FakeSandbox::none()),
        install_source: Arc::new(FakeInstallSource(InstallSource::Portable)),
        file_id: None,
    }
}

pub fn boot_at(base: &Utf8Path, clock: &FakeClock) -> AppContext {
    let roots = DataRoots::under_base(&base.join("app"));
    let input = BootInput::new(platform(base, clock))
        .with_roots(roots)
        .with_log(LogConfig::off())
        .with_options(AppOptions {
            type_table: Some(types()),
        });
    boot(input).unwrap()
}

/// A fictional install (with the fictional Combat Extended when asked) and a booted application.
pub fn fixture(with_ce: bool) -> Fixture {
    let mut b = InstallBuilder::new().core_defs_file("RS_Core.xml", core_defs());
    if with_ce {
        b = b.mod_folder(ce_mod());
    }
    let install = b.build_temp().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let app = boot_at(&base, &clock);
    Fixture {
        tmp,
        install,
        base,
        clock,
        app,
    }
}

/// Points the application at the fictional install through the override command.
pub fn select_install(f: &Fixture) {
    let out = rimstudio_app::dispatch_blocking(
        &f.app,
        "detect_set_override",
        json!({"field": "game-install", "path": f.install.game_dir.as_str()}),
    );
    assert!(out.is_ok(), "{out:?}");
}

/// A mod folder outside the install with an About file and the given extra files.
pub fn project_folder(f: &Fixture, name: &str, files: &[(&str, &str)]) -> Utf8PathBuf {
    let root = f.base.join("projects").join(name);
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        about("rs.testmod"),
    )
    .unwrap();
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
        std::fs::write(path.as_std_path(), text).unwrap();
    }
    root
}
