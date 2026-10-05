//! Shared fixtures of the designer tests: a fictional install with fourteen guns, eight melee weapons and
//! a fictional Combat Extended mod. Every name starts with `RS_` and every number is invented; nothing
//! here comes from a game install.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::{PackageId, SourceId};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ActiveList, ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_core::version::GameVersion;
use rimstudio_defs::{TypeInfo, TypeTable};
use rimstudio_io::roots::DataRoots;
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::{BuiltInstall, InstallBuilder, ModFolder, TempInstall};
use rimstudio_toolkit::designer::Ctx;
use rimstudio_workspace::refset::ReferenceSet;
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};

pub const CE_ID: &str = "ceteam.combatextended";

pub fn game() -> GameVersion {
    GameVersion::parse("1.6.1000").unwrap()
}

pub fn types() -> Arc<TypeTable> {
    let mut list = vec![
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
    ];
    list.push((
        "CombatExtended.AmmoSetDef".to_owned(),
        TypeInfo::with_base("Verse.Def"),
    ));
    Arc::new(TypeTable::new(list).unwrap())
}

fn stat_def(name: &str, default: f64) -> Node {
    NodeBuilder::new("StatDef")
        .text_elem("defName", name)
        .text_elem("defaultBaseValue", default.to_string())
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

fn armor(name: &str, sharp: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("category", "Item")
        .elem("apparel", |a| a.elem("layers", |l| l.li("Shell")))
        .elem("statBases", |s| {
            s.text_elem("ArmorRating_Sharp", sharp.to_string())
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

/// A fictional gun. `i` spreads the numbers; `tags` decide the role.
pub fn gun(name: &str, projectile: &str, i: u32, tier: &str, tag: &str) -> Node {
    let f = f64::from(i);
    let burst = if i.is_multiple_of(4) { 3 } else { 1 };
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
                    .when(burst > 1, |li| {
                        li.text_elem("burstShotCount", burst.to_string())
                            .text_elem("ticksBetweenBurstShots", "8")
                    })
            })
        })
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "grip")
                    .elem("capacities", |c| c.li("Blunt"))
                    .text_elem("power", "8")
                    .text_elem("cooldownTime", "2")
            })
        })
        .elem("weaponTags", |w| w.li(tag))
        .build()
}

/// A fictional melee weapon.
pub fn melee(name: &str, i: u32, tier: &str, cap: &str) -> Node {
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

/// The core defs: stats, the damage def, ingredients, armor, projectiles, guns and melee weapons.
pub fn core_defs() -> Vec<Node> {
    core_defs_n(14, 8)
}

/// The core defs with a chosen number of guns and melee weapons.
pub fn core_defs_n(guns: u32, blades: u32) -> Vec<Node> {
    let mut defs = vec![
        stat_def("Mass", 0.0),
        stat_def("RangedWeapon_Cooldown", 0.0),
        NodeBuilder::new("DamageDef")
            .text_elem("defName", "RS_Damage")
            .text_elem("defaultDamage", "10")
            .text_elem("defaultArmorPenetration", "0.2")
            .build(),
        ingredient("RS_Steel", 2.0),
        ingredient("RS_Part", 40.0),
        armor("RS_ArmorA", 0.3),
        armor("RS_ArmorB", 0.6),
        armor("RS_ArmorC", 0.9),
    ];
    for i in 0..guns {
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
    for i in 0..blades {
        let cap = if i % 2 == 0 { "Cut" } else { "Blunt" };
        let tier = if i < 4 { "Medieval" } else { "Industrial" };
        defs.push(melee(&format!("RS_Blade{i:02}"), i, tier, cap));
    }
    defs
}

/// The fictional Combat Extended mod: one ammo set with a projectile.
pub fn ce_mod() -> ModFolder {
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

/// A mod with one more gun, for the mod id of a reference item.
pub fn extra_mod() -> ModFolder {
    ModFolder::new("RS_Extra", "rs.extra")
        .name("RS Extra")
        .defs_file(
            "RS_Extra.xml",
            vec![
                projectile("RS_ShotExtra", 20.0),
                gun("RS_ModGun", "RS_ShotExtra", 6, "Industrial", "RS_Rifle"),
            ],
        )
}

pub fn install(with_ce: bool) -> TempInstall {
    install_custom(14, 8, true, with_ce)
}

/// An install with a chosen number of core guns and melee weapons, optionally the extra mod and the
/// fictional Combat Extended.
pub fn install_custom(guns: u32, blades: u32, extra: bool, with_ce: bool) -> TempInstall {
    let mut b = InstallBuilder::new().core_defs_file("RS_Core.xml", core_defs_n(guns, blades));
    if extra {
        b = b.mod_folder(extra_mod());
    }
    if with_ce {
        b = b.mod_folder(ce_mod());
    }
    b.build_temp().unwrap()
}

fn sources(b: &BuiltInstall) -> SourceSet {
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        b.data_dir.clone(),
    ));
    set.insert(ModSource::new(
        SourceId::game_mods(),
        SourceKind::GameMods,
        b.mods_dir.clone(),
    ));
    set.insert(ModSource::new(
        SourceId::workshop(0),
        SourceKind::Workshop,
        b.workshop_dir.clone(),
    ));
    set
}

fn scan(b: &BuiltInstall) -> ModIndex {
    let outcome = Scanner::scan(
        &sources(b),
        &ScanOptions::default().with_game_version(game()),
        &NoopProgress,
        &CancelToken::new(),
    );
    let index: &ModIndex = &outcome.index;
    index.clone()
}

fn active(ids: &[&str]) -> ActiveList {
    ActiveList::from_ids(ids.iter().map(|i| PackageId::parse(i).unwrap()))
}

pub fn open_session(b: &BuiltInstall, with_ce: bool) -> Arc<WorkspaceSession> {
    let mut ids = vec!["ludeon.rimworld", "rs.extra"];
    if with_ce {
        ids.push(CE_ID);
    }
    open_session_with(b, &ids)
}

/// A session over the given active package ids.
pub fn open_session_with(b: &BuiltInstall, ids: &[&str]) -> Arc<WorkspaceSession> {
    let reference = ReferenceSet::resolve(&scan(b), &active(ids), &game());
    Arc::new(
        WorkspaceSession::open_simple(OpenInput::new(reference).with_type_table(types())).unwrap(),
    )
}

/// A fictional install, its session, temporary data roots and a context.
pub struct Fixture {
    pub install: TempInstall,
    pub session: Arc<WorkspaceSession>,
    pub tmp: tempfile::TempDir,
    pub roots: DataRoots,
    pub clock: FakeClock,
    pub ctx: Ctx,
}

pub fn fixture(with_ce: bool) -> Fixture {
    let install = install(with_ce);
    let session = open_session(&install, with_ce);
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

/// A context without a game install over fresh temporary roots.
pub fn offline() -> (tempfile::TempDir, Ctx, FakeClock) {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let ctx = Ctx::offline(&roots, Arc::new(clock.clone())).unwrap();
    (tmp, ctx, clock)
}

use rimstudio_design::model::{
    CostEntry, DesignSpec, Draft, ScalarField, TechLevel, ToolSpec, ValueSource,
};
use rimstudio_ipc_types::designer::DraftDto;
use rimstudio_toolkit::designer::dto::draft_to_dto;

/// A new ranged design with tier, role and a cost list, nothing else.
pub fn ranged_spec() -> DesignSpec {
    let mut spec = DesignSpec::new_ranged("RS_NewRifle", "new rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some("RS_Rifle".into());
    spec.cost_list = vec![CostEntry::new("RS_Steel", 30.0)];
    spec
}

/// A new melee design with one tool.
pub fn melee_spec() -> DesignSpec {
    let mut spec = DesignSpec::new_melee("RS_NewBlade", "new blade");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some("blade".into());
    spec.cost_list = vec![CostEntry::new("RS_Steel", 40.0)];
    spec.tools = vec![ToolSpec::new("head", &["Cut"])];
    spec
}

pub fn dto(spec: DesignSpec) -> DraftDto {
    draft_to_dto(&Draft::new(spec)).unwrap()
}

/// A ranged spec with every number typed, as a user who knows all of them would enter.
pub fn typed_ranged() -> DesignSpec {
    let mut spec = ranged_spec();
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
    spec
}
