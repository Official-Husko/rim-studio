//! Custom Combat Extended ammunition through the toolkit: `designer_ce_ammo_catalog`,
//! `designer_ce_ammo_suggest`, and the plan and apply of a draft that carries a custom caliber. The
//! fictional Combat Extended of this file has two calibers with three ammo types each. Owner rule: vanilla by
//! default, so a draft whose switch is off gets answers, never a Combat Extended file.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{TypeInfo, TypeTable};
use rimstudio_design::model::{
    CePatchSpec, CustomAmmoRecipe, CustomAmmoSpec, CustomAmmoType, CustomIngredient, DesignSpec,
    Sourced,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    CeAmmoSourceDto, DesignerApplyPlanRequest, DesignerCeAmmoCatalogRequest,
    DesignerCeAmmoSuggestRequest, FileActionDto, FileKindDto, WritePlanDto,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder};
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{Ctx, apply_plan, ce_ammo_catalog, ce_ammo_suggest};

use common::CE_ID;
use common_project::{Proj, ce_block, draft, ranged, request};

const AMMO_DEF: &str = "CombatExtended.AmmoDef";

fn types() -> Arc<TypeTable> {
    let base = common::types();
    let mut value = base.to_json_value();
    if let Some(map) = value.get_mut("types").and_then(|t| t.as_object_mut()) {
        for (name, parent) in [
            (AMMO_DEF, "Verse.ThingDef"),
            ("CombatExtended.AmmoCategoryDef", "Verse.Def"),
        ] {
            map.insert(
                name.to_owned(),
                serde_json::to_value(TypeInfo::with_base(parent)).unwrap(),
            );
        }
    }
    Arc::new(TypeTable::from_json_value(value).unwrap())
}

fn class(def: &str, label: &str, short: &str) -> Node {
    NodeBuilder::new("CombatExtended.AmmoCategoryDef")
        .text_elem("defName", def)
        .text_elem("label", label)
        .text_elem("labelShort", short)
        .build()
}

fn caliber(name: &str, damage: f64) -> Vec<Node> {
    let mut out = vec![
        NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", format!("RS_AmmoSet_{name}"))
            .text_elem("label", format!("{name} test"))
            .elem("ammoTypes", |t| {
                [("FMJ", 1.0), ("AP", 0.7), ("HP", 1.3)]
                    .iter()
                    .fold(t, |t, (c, _)| {
                        t.text_elem(
                            format!("RS_Ammo_{name}_{c}"),
                            format!("RS_Bullet_{name}_{c}"),
                        )
                    })
            })
            .build(),
    ];
    for (c, f) in [("FMJ", 1.0), ("AP", 0.7), ("HP", 1.3)] {
        out.push(
            NodeBuilder::new("ThingDef")
                .attr("Class", AMMO_DEF)
                .text_elem("defName", format!("RS_Ammo_{name}_{c}"))
                .text_elem("label", format!("{name} ({c})"))
                .text_elem("category", "Item")
                .elem("statBases", |s| {
                    s.text_elem("Mass", "0.02").text_elem("Bulk", "0.03")
                })
                .text_elem("ammoClass", format!("RS_{c}"))
                .text_elem("cookOffProjectile", format!("RS_Bullet_{name}_{c}"))
                .build(),
        );
        out.push(
            NodeBuilder::new("ThingDef")
                .text_elem("defName", format!("RS_Bullet_{name}_{c}"))
                .elem("projectile", |p| {
                    p.text_elem("damageAmountBase", (damage * f).round().to_string())
                        .text_elem(
                            "armorPenetrationSharp",
                            (damage * f / 2.0).round().to_string(),
                        )
                        .text_elem("armorPenetrationBlunt", "12")
                        .text_elem("speed", "120")
                })
                .build(),
        );
    }
    out
}

fn gun(i: u32) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", format!("RS_CeGun{i:02}"))
        .text_elem("label", format!("ce gun {i}"))
        .text_elem("category", "Item")
        .elem("comps", |c| {
            c.elem("li", |li| {
                li.attr("Class", "CombatExtended.CompProperties_AmmoUser")
                    .text_elem("magazineSize", "30")
                    .text_elem("reloadTime", "4")
                    .text_elem("ammoSet", "RS_AmmoSet_Nine")
            })
        })
        .elem("weaponTags", |w| w.li("RS_Rifle").li("CE_AI_RS_Rifle"))
        .build()
}

fn ce_mod() -> ModFolder {
    let mut defs = vec![
        class("RS_FMJ", "full metal jacket", "FMJ"),
        class("RS_AP", "armor piercing", "AP"),
        class("RS_HP", "hollow point", "HP"),
    ];
    defs.extend(caliber("Nine", 10.0));
    defs.extend(caliber("Ten", 12.0));
    defs.extend((0..3).map(gun));
    ModFolder::new("RS_CE", CE_ID)
        .name("Combat Extended")
        .defs_file("RS_Ce.xml", defs)
}

struct World {
    _install: rimstudio_testing::install_tree::TempInstall,
    _tmp: tempfile::TempDir,
    ctx: Ctx,
}

fn world(with_ce: bool) -> World {
    let mut core = common::core_defs_n(14, 8);
    for base in ["RS_BaseGun", "RS_BaseMelee", "RS_BaseBullet"] {
        core.push(
            NodeBuilder::new("ThingDef")
                .attr("Name", base)
                .attr("Abstract", "True")
                .text_elem("category", "Item")
                .build(),
        );
    }
    let mut b = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", core)
        .mod_folder(common::extra_mod());
    if with_ce {
        b = b.mod_folder(ce_mod());
    }
    let install = b.build_temp().unwrap();
    let mut ids = vec!["ludeon.rimworld", "rs.extra"];
    if with_ce {
        ids.push(CE_ID);
    }
    let session = common::open_session_with_types(&install, &ids, types());
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let ctx = Ctx::new(session, &roots, Arc::new(clock)).unwrap();
    World {
        _install: install,
        _tmp: tmp,
        ctx,
    }
}

fn project_of(w: &World) -> Proj {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().join("RS_TestMod")).unwrap();
    std::fs::create_dir_all(root.as_std_path()).unwrap();
    common_project::put(
        &root,
        "About/About.xml",
        &common_project::about("rs.testmod", &["1.5", "1.6"]),
    );
    let record = w.ctx.env().projects().open(&root).unwrap();
    Proj {
        tmp,
        id: record.id.as_str().to_owned(),
        root,
    }
}

fn custom(w: &World) -> CustomAmmoSpec {
    let s = ce_ammo_suggest(
        &w.ctx,
        DesignerCeAmmoSuggestRequest {
            class: "RS_FMJ".into(),
            hints: rimstudio_ipc_types::designer::CeAmmoHintsDto {
                damage: Some(11.0),
                ..Default::default()
            },
            copy_from: None,
        },
    )
    .unwrap();
    let mut t: CustomAmmoType =
        serde_json::from_value(serde_json::to_value(&s.ammo_type).unwrap()).unwrap();
    t.key = "FMJ".into();
    t.recipe = CustomAmmoRecipe {
        ingredients: vec![CustomIngredient {
            thing: "RS_Steel".into(),
            count: Some(Sourced::typed(10.0)),
            ..CustomIngredient::default()
        }],
        ..CustomAmmoRecipe::default()
    };
    CustomAmmoSpec {
        name: "Mine".into(),
        caliber: "my test caliber".into(),
        similar_to: Some("RS_AmmoSet_Nine".into()),
        types: vec![t],
        ..CustomAmmoSpec::default()
    }
}

fn weapon(w: &World, name: &str) -> DesignSpec {
    let mut spec = ranged();
    spec.identity.mod_prefix = "RS".into();
    let mut ce: CePatchSpec = ce_block();
    ce.ammo_set = None;
    ce.default_projectile = None;
    let mut c = custom(w);
    c.name = name.into();
    ce.custom_ammo = Some(c);
    spec.ce = Some(ce);
    spec
}

fn ammo_files(plan: &WritePlanDto) -> Vec<&str> {
    plan.files
        .iter()
        .filter(|f| f.kind == FileKindDto::CeDefs)
        .map(|f| f.path.as_str())
        .collect()
}

#[test]
fn without_combat_extended_the_answers_give_the_plain_reason() {
    let w = world(false);
    let c = ce_ammo_catalog(&w.ctx, DesignerCeAmmoCatalogRequest::default()).unwrap();
    assert!(!c.available);
    assert!(c.reason.is_some());
    let s = ce_ammo_suggest(
        &w.ctx,
        DesignerCeAmmoSuggestRequest {
            class: "RS_FMJ".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!s.available);
}

#[test]
fn the_catalogue_lists_the_sets_of_the_install_and_marks_the_suggested_ones() {
    let w = world(true);
    let c = ce_ammo_catalog(&w.ctx, DesignerCeAmmoCatalogRequest::default()).unwrap();
    assert!(c.available);
    assert_eq!(c.total, 2);
    let nine = c
        .entries
        .iter()
        .find(|e| e.def_name == "RS_AmmoSet_Nine")
        .unwrap();
    assert_eq!(nine.types.len(), 3);
    assert_eq!(nine.types[0].ammo_class_label, "FMJ");
    assert_eq!(nine.types[0].damage, Some(10.0));
    assert_eq!(nine.weapon_count, 3);
    assert!(!nine.suggested, "no draft, no ranking");
    let with_draft = ce_ammo_catalog(
        &w.ctx,
        DesignerCeAmmoCatalogRequest {
            draft: Some(draft(&ranged())),
            ..Default::default()
        },
    )
    .unwrap();
    let nine = with_draft
        .entries
        .iter()
        .find(|e| e.def_name == "RS_AmmoSet_Nine")
        .unwrap();
    assert!(nine.suggested, "the guns of the set rank it for a rifle");
    assert!(nine.score.is_some());
    let filtered = ce_ammo_catalog(
        &w.ctx,
        DesignerCeAmmoCatalogRequest {
            query: Some("ten".into()),
            class: Some("RS_AP".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.matching, 1);
    let page = ce_ammo_catalog(
        &w.ctx,
        DesignerCeAmmoCatalogRequest {
            page: Some(1),
            page_size: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((page.entries.len(), page.total), (1, 2));
}

#[test]
fn a_suggestion_comes_from_the_nearest_ammunition_and_a_copy_from_one_type() {
    let w = world(true);
    let s = ce_ammo_suggest(
        &w.ctx,
        DesignerCeAmmoSuggestRequest {
            class: "RS_AP".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(s.available);
    assert_eq!(s.nearest.len(), 2);
    assert!(
        s.fields
            .iter()
            .any(|f| f.field == "/projectile/damage" && f.source == CeAmmoSourceDto::Nearest)
    );
    assert_eq!(s.ammo_type.ammo_class, "RS_AP");
    let copy = ce_ammo_suggest(
        &w.ctx,
        DesignerCeAmmoSuggestRequest {
            copy_from: Some("RS_Ammo_Ten_HP".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(copy.copied_from.as_deref(), Some("RS_Ammo_Ten_HP"));
    assert_eq!(copy.ammo_type.ammo_class, "RS_HP");
    assert_eq!(
        copy.ammo_type.projectile.damage.map(|d| d.value),
        Some(16.0)
    );
}

#[test]
fn the_plan_has_the_ammunition_file_only_with_the_switch_on() {
    let w = world(true);
    let p = project_of(&w);
    let on = export_plan(&w.ctx, request(&p, &weapon(&w, "Mine"))).unwrap();
    assert!(!on.has_errors, "{:?}", on.diagnostics);
    assert_eq!(
        ammo_files(&on),
        ["Compat/CombatExtended/Defs/Ammo/RS_Mine.xml"]
    );
    let file = on
        .files
        .iter()
        .find(|f| f.kind == FileKindDto::CeDefs)
        .unwrap();
    assert!(
        file.rendered.contains("<defName>RS_AmmoSet_Mine</defName>"),
        "{}",
        file.rendered
    );
    assert!(file.rendered.contains("====== RS_Ammo_Mine_FMJ ======"));
    assert!(
        on.files
            .iter()
            .any(|f| f.kind == FileKindDto::CePatch && f.rendered.contains("RS_AmmoSet_Mine"))
    );
    let mut spec = weapon(&w, "Mine");
    spec.ce = None;
    let off = export_plan(&w.ctx, request(&p, &spec)).unwrap();
    assert!(ammo_files(&off).is_empty());
    assert!(
        !off.files
            .iter()
            .any(|f| f.rendered.contains("RS_AmmoSet_Mine"))
    );
}

#[test]
fn custom_ammunition_is_never_asked_for_an_ammo_set() {
    let w = world(true);
    let p = project_of(&w);
    let asks_for_set = |spec: &DesignSpec| {
        let mut req = request(&p, spec);
        req.accept_suggestions =
            Some(rimstudio_ipc_types::designer::AcceptSuggestionsDto { fields: Vec::new() });
        export_plan(&w.ctx, req)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| {
                d.code == "designer.ce-needs-answer"
                    && matches!(
                        d.field.as_deref(),
                        Some("/ce/ammoSet" | "/ce/defaultProjectile")
                    )
            })
    };
    // with the custom caliber the set and the projectile come from it
    assert!(!asks_for_set(&weapon(&w, "Mine")));
    // without it the question is still open
    let mut plain = weapon(&w, "Mine");
    if let Some(ce) = plain.ce.as_mut() {
        ce.custom_ammo = None;
    }
    assert!(asks_for_set(&plain));
}

#[test]
fn a_derived_name_that_exists_is_a_plan_error() {
    let w = world(true);
    let p = project_of(&w);
    let plan = export_plan(&w.ctx, request(&p, &weapon(&w, "Nine"))).unwrap();
    assert!(plan.has_errors);
    assert!(ammo_files(&plan).is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "ce.ammo-duplicate-def")
    );
}

#[test]
fn an_ingredient_that_does_not_exist_is_a_plan_error() {
    let w = world(true);
    let p = project_of(&w);
    let mut spec = weapon(&w, "Mine");
    spec.ce
        .as_mut()
        .unwrap()
        .custom_ammo
        .as_mut()
        .unwrap()
        .types[0]
        .recipe
        .ingredients[0]
        .thing = "RS_Unobtainium".into();
    let plan = export_plan(&w.ctx, request(&p, &spec)).unwrap();
    assert!(plan.has_errors);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "ce.ammo-ref-unresolved")
    );
}

#[test]
fn apply_writes_the_file_once_and_a_changed_spec_replaces_only_its_section() {
    let w = world(true);
    let p = project_of(&w);
    let apply = |spec: &DesignSpec| {
        let req = request(&p, spec);
        let plan = export_plan(&w.ctx, req.clone()).unwrap();
        assert!(!plan.has_errors, "{:?}", plan.diagnostics);
        let report = apply_plan(
            &w.ctx,
            DesignerApplyPlanRequest {
                plan_id: plan.plan_id.clone(),
                request: req,
                backup: true,
                dry_apply: true,
            },
            &NoopProgress,
            &CancelToken::new(),
        )
        .unwrap();
        assert!(report.written.iter().all(|f| f.verified));
        plan
    };
    let spec = weapon(&w, "Mine");
    let first = apply(&spec);
    let path = p.root.join("Compat/CombatExtended/Defs/Ammo/RS_Mine.xml");
    let text = std::fs::read_to_string(path.as_std_path()).unwrap();
    let file = first
        .files
        .iter()
        .find(|f| f.kind == FileKindDto::CeDefs)
        .unwrap();
    assert_eq!(text, file.rendered);
    // the same design again changes nothing
    let again = export_plan(&w.ctx, request(&p, &spec)).unwrap();
    let ammo = again
        .files
        .iter()
        .find(|f| f.kind == FileKindDto::CeDefs)
        .unwrap();
    assert_eq!(ammo.action, FileActionDto::Unchanged);
    // a note the user keeps in the file survives a changed design
    let noted = text.replacen("<Defs>", "<Defs>\n  <!-- my note -->", 1);
    std::fs::write(path.as_std_path(), &noted).unwrap();
    let mut changed = spec.clone();
    changed
        .ce
        .as_mut()
        .unwrap()
        .custom_ammo
        .as_mut()
        .unwrap()
        .types[0]
        .projectile
        .damage = Some(Sourced::typed(21.0));
    apply(&changed);
    let after = std::fs::read_to_string(path.as_std_path()).unwrap();
    assert!(after.contains("my note"));
    assert!(after.contains("<damageAmountBase>21</damageAmountBase>"));
    assert_eq!(
        after
            .matches("<defName>RS_Bullet_Mine_FMJ</defName>")
            .count(),
        1
    );
}
