//! The Combat Extended suggestions of the designer: `designer_ce_suggest` and the `acceptSuggestions` option
//! of plan and apply. The fictional Combat Extended of this file has eight converted guns to learn from.
//! Owner rule: vanilla by default, so a draft whose toggle is off gets suggestions, never a Combat Extended
//! file, and the toggle is never turned on.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::model::{CePatchSpec, DesignSpec, Sourced};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    AcceptSuggestionsDto, CeFieldStatusDto, CeSourceDto, CeSuggestionDto, DesignerApplyPlanRequest,
    DesignerCeSuggestRequest, DesignerExportPlanRequest, FileKindDto, WritePlanDto,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder};
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{Ctx, apply_plan, ce_suggest};

use common::CE_ID;
use common_project::{Proj, draft, ranged, request};

fn converted_gun(i: u32) -> Node {
    let f = f64::from(i);
    NodeBuilder::new("ThingDef")
        .text_elem("defName", format!("RS_CeGun{i:02}"))
        .text_elem("label", format!("ce gun {i}"))
        .text_elem("category", "Item")
        .text_elem("techLevel", "Industrial")
        .elem("statBases", |s| {
            s.text_elem("Mass", (2.0 + 0.2 * f).to_string())
                .text_elem("Bulk", (6.0 + 0.5 * f).to_string())
                .text_elem("ShotSpread", "0.07")
                .text_elem("SwayFactor", "1.3")
                .text_elem("SightsEfficiency", "1")
                .text_elem("RangedWeapon_Cooldown", "0.4")
        })
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.attr("Class", "CombatExtended.VerbPropertiesCE")
                    .text_elem("verbClass", "CombatExtended.Verb_ShootCE")
                    .text_elem("defaultProjectile", "RS_CeBullet1")
                    .text_elem("warmupTime", (0.7 + 0.02 * f).to_string())
                    .text_elem("range", (30.0 + f).to_string())
                    .text_elem("recoilAmount", "1.5")
            })
        })
        .elem("comps", |c| {
            c.elem("li", |li| {
                li.attr("Class", "CombatExtended.CompProperties_AmmoUser")
                    .text_elem("magazineSize", "30")
                    .text_elem("reloadTime", "4")
                    .text_elem("ammoSet", "RS_AmmoSetA")
            })
        })
        .elem("weaponTags", |w| w.li("RS_Rifle").li("CE_AI_RS_Rifle"))
        .build()
}

/// The fictional CE mod of the common fixture plus eight converted guns.
fn ce_with_conversions() -> ModFolder {
    let ammo = |name: &str, ammo: &str, bullet: &str| {
        NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", name)
            .elem("ammoTypes", |t| t.text_elem(ammo, bullet))
            .build()
    };
    let bullet = |name: &str, damage: &str| {
        NodeBuilder::new("ThingDef")
            .text_elem("defName", name)
            .elem("projectile", |p| {
                p.text_elem("damageAmountBase", damage)
                    .text_elem("speed", "120")
            })
            .build()
    };
    let mut defs = vec![
        ammo("RS_AmmoSetA", "RS_AmmoA1", "RS_CeBullet1"),
        ammo("RS_AmmoSetB", "RS_AmmoB1", "RS_CeBullet2"),
        bullet("RS_CeBullet1", "9"),
        bullet("RS_CeBullet2", "30"),
    ];
    defs.extend((0..8).map(converted_gun));
    ModFolder::new("RS_CE", CE_ID)
        .name("Combat Extended")
        .defs_file("RS_Ce.xml", defs)
}

struct World {
    _install: rimstudio_testing::install_tree::TempInstall,
    _tmp: tempfile::TempDir,
    ctx: Ctx,
}

fn world() -> World {
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
    let install = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .mod_folder(common::extra_mod())
        .mod_folder(ce_with_conversions())
        .build_temp()
        .unwrap();
    let session = common::open_session(&install, true);
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

fn suggest(w: &World, spec: &DesignSpec) -> CeSuggestionDto {
    ce_suggest(&w.ctx, DesignerCeSuggestRequest { draft: draft(spec) }).unwrap()
}

fn field<'a>(
    s: &'a CeSuggestionDto,
    pointer: &str,
) -> &'a rimstudio_ipc_types::designer::CeSuggestedFieldDto {
    s.fields.iter().find(|f| f.field == pointer).unwrap()
}

/// The typed Combat Extended block of a user who answered the choices and typed the bulk only.
fn answered_block() -> CePatchSpec {
    CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        bulk: Some(Sourced::typed(6.5)),
        ..CePatchSpec::default()
    }
}

fn with_accept(mut req: DesignerExportPlanRequest, fields: &[&str]) -> DesignerExportPlanRequest {
    req.accept_suggestions = Some(AcceptSuggestionsDto {
        fields: fields.iter().map(|f| (*f).to_owned()).collect(),
    });
    req
}

fn ce_files(plan: &WritePlanDto) -> Vec<&str> {
    plan.files
        .iter()
        .filter(|f| f.kind == FileKindDto::CePatch)
        .map(|f| f.path.as_str())
        .collect()
}

fn codes(plan: &WritePlanDto) -> Vec<&str> {
    plan.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

// ---------------------------------------------------------------------------------------------------------
// designer_ce_suggest

#[test]
fn without_combat_extended_the_answer_is_the_plain_reason() {
    let f = common::fixture(false);
    let s = ce_suggest(
        &f.ctx,
        DesignerCeSuggestRequest {
            draft: draft(&ranged()),
        },
    )
    .unwrap();
    assert!(!s.available);
    assert!(
        s.reason
            .as_deref()
            .is_some_and(|r| r.contains("Combat Extended"))
    );
    assert!(s.fields.is_empty() && s.choices.is_empty());
}

#[test]
fn without_a_game_install_the_answer_is_the_plain_reason_too() {
    let (_tmp, ctx, _clock) = common::offline();
    let s = ce_suggest(
        &ctx,
        DesignerCeSuggestRequest {
            draft: draft(&ranged()),
        },
    )
    .unwrap();
    assert!(!s.available);
    assert!(
        s.reason
            .as_deref()
            .is_some_and(|r| r.contains("no game install"))
    );
}

#[test]
fn a_draft_with_the_toggle_off_gets_suggestions_and_the_toggle_stays_off() {
    let w = world();
    let spec = ranged();
    assert!(spec.ce.is_none());
    let s = suggest(&w, &spec);
    assert!(s.available && !s.toggle_on);
    assert_eq!(s.pool, 8);
    assert_eq!(
        field(&s, "/ce/swayFactor").status,
        CeFieldStatusDto::Derived
    );
    let sway = field(&s, "/ce/swayFactor");
    assert_eq!(sway.value, Some(1.3));
    assert!(matches!(sway.source, Some(CeSourceDto::Predicted { n, .. }) if n >= 3));
    assert!(sway.band.is_some() && sway.rating.is_some());
    // the caliber is asked, with the set the conversions use first
    let ammo = s.choices.iter().find(|c| c.field == "/ce/ammoSet").unwrap();
    assert_eq!(ammo.status, CeFieldStatusDto::Ask);
    assert_eq!(
        ammo.candidates.first().map(|c| c.name.as_str()),
        Some("RS_AmmoSetA")
    );
    assert_eq!(ammo.candidates.first().map(|c| c.used_by), Some(8));
    assert!(s.asks.items.iter().any(|a| a.field == "/ce/ammoSet"));
    // the draft is untouched, and so is its JSON
    assert!(spec.ce.is_none());
}

#[test]
fn the_suggestion_is_deterministic() {
    let w = world();
    let a = serde_json::to_string(&suggest(&w, &ranged())).unwrap();
    let b = serde_json::to_string(&suggest(&w, &ranged())).unwrap();
    assert_eq!(a, b);
}

#[test]
fn held_values_are_reported_as_held_beside_the_estimate() {
    let w = world();
    let mut spec = ranged();
    spec.ce = Some(answered_block());
    let s = suggest(&w, &spec);
    assert!(s.toggle_on);
    let bulk = field(&s, "/ce/bulk");
    assert_eq!(bulk.status, CeFieldStatusDto::Held);
    assert_eq!(bulk.held, Some(6.5));
    let ammo = s.choices.iter().find(|c| c.field == "/ce/ammoSet").unwrap();
    assert_eq!(ammo.held.as_deref(), Some("RS_AmmoSetA"));
    let projectile = s
        .choices
        .iter()
        .find(|c| c.field == "/ce/defaultProjectile")
        .unwrap();
    assert_eq!(projectile.status, CeFieldStatusDto::Derived);
    assert_eq!(projectile.value.as_deref(), Some("RS_CeBullet1"));
}

// ---------------------------------------------------------------------------------------------------------
// plan and apply with acceptSuggestions

#[test]
fn accepting_fills_the_empty_fields_marks_them_and_keeps_typed_values() {
    let w = world();
    let p = project_of(&w);
    let mut spec = ranged();
    spec.ce = Some(answered_block());
    let plain = export_plan(&w.ctx, request(&p, &spec)).unwrap();
    assert!(
        plain.has_errors,
        "without the option the open fields are errors"
    );
    let plan = export_plan(&w.ctx, with_accept(request(&p, &spec), &[])).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(ce_files(&plan).len(), 1);
    let patch = plan
        .files
        .iter()
        .find(|f| f.kind == FileKindDto::CePatch)
        .unwrap();
    // the typed bulk is written as typed, not replaced by the estimate
    assert!(
        patch.rendered.contains("<Bulk>6.5</Bulk>"),
        "{}",
        patch.rendered
    );
    // each filled field is marked by an info diagnostic
    let marked: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| {
            d.code == "ce.derived-value" && d.message.contains("Combat Extended suggestion")
        })
        .map(|d| d.field.as_deref().unwrap_or(""))
        .collect();
    assert!(marked.contains(&"/ce/swayFactor"), "{marked:?}");
    assert!(marked.contains(&"/ce/magazineSize"), "{marked:?}");
    assert!(!marked.contains(&"/ce/bulk"), "{marked:?}");
    // the same request builds the same plan
    let again = export_plan(&w.ctx, with_accept(request(&p, &spec), &[])).unwrap();
    assert_eq!(again.plan_id, plan.plan_id);
}

#[test]
fn accepting_named_fields_fills_only_those() {
    let w = world();
    let p = project_of(&w);
    let mut spec = ranged();
    spec.ce = Some(answered_block());
    let plan = export_plan(
        &w.ctx,
        with_accept(request(&p, &spec), &["swayFactor", "nonsense"]),
    )
    .unwrap();
    // the other required numbers stay open, so the plan still refuses
    assert!(plan.has_errors);
    let marked: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| {
            d.code == "ce.derived-value" && d.message.contains("Combat Extended suggestion")
        })
        .map(|d| d.field.as_deref().unwrap_or(""))
        .collect();
    assert!(marked.contains(&"/ce/swayFactor"));
    assert!(!marked.contains(&"/ce/shotSpread"));
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.ce-suggestion-skipped" && d.message.contains("nonsense"))
    );
}

#[test]
fn the_ammo_set_and_the_tag_class_are_never_chosen_for_the_user() {
    let w = world();
    let p = project_of(&w);
    let mut spec = ranged();
    spec.ce = Some(CePatchSpec {
        bulk: Some(Sourced::typed(6.5)),
        ..CePatchSpec::default()
    });
    let plan = export_plan(&w.ctx, with_accept(request(&p, &spec), &[])).unwrap();
    assert!(plan.has_errors);
    let open: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code == "designer.ce-needs-answer")
        .map(|d| d.field.as_deref().unwrap_or(""))
        .collect();
    assert!(open.contains(&"/ce/ammoSet"), "{open:?}");
    assert!(open.contains(&"/ce/weaponTagClass"), "{open:?}");
    let ammo = plan
        .diagnostics
        .iter()
        .find(|d| d.field.as_deref() == Some("/ce/ammoSet") && d.code == "designer.ce-needs-answer")
        .unwrap();
    assert!(
        ammo.args
            .get("candidates")
            .is_some_and(|c| c.contains("RS_AmmoSetA"))
    );
    assert!(
        ce_files(&plan).is_empty(),
        "no patch while an answer is open: {:?}",
        ce_files(&plan)
    );
}

#[test]
fn with_the_toggle_off_the_option_is_ignored_and_no_ce_file_ever_appears() {
    let w = world();
    let p = project_of(&w);
    let spec = ranged();
    let off = export_plan(&w.ctx, request(&p, &spec)).unwrap();
    for fields in [&[][..], &["bulk"][..]] {
        let plan = export_plan(&w.ctx, with_accept(request(&p, &spec), fields)).unwrap();
        assert!(ce_files(&plan).is_empty());
        assert!(
            plan.files
                .iter()
                .all(|f| f.kind != FileKindDto::LoadFolders)
        );
        assert!(
            plan.files
                .iter()
                .all(|f| !f.rendered.contains("CombatExtended.")),
            "no Combat Extended class anywhere"
        );
        assert!(codes(&plan).contains(&"designer.ce-suggestion-ignored"));
        assert_eq!(plan.has_errors, off.has_errors);
        // the files are exactly those of the plain vanilla plan
        let a: Vec<(&str, &str)> = plan
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.rendered.as_str()))
            .collect();
        let b: Vec<(&str, &str)> = off
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.rendered.as_str()))
            .collect();
        assert_eq!(a, b);
    }
}

#[test]
fn apply_with_the_option_writes_the_same_files_the_plan_listed() {
    let w = world();
    let p = project_of(&w);
    let mut spec = ranged();
    spec.ce = Some(answered_block());
    let req = with_accept(request(&p, &spec), &[]);
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
    assert_eq!(report.plan_id, plan.plan_id);
    assert!(report.written.iter().all(|f| f.verified));
    assert_eq!(report.dry_apply_ok, Some(true), "{:?}", report.diagnostics);
    for file in &plan.files {
        let text = std::fs::read_to_string(p.root.join(&file.path).as_std_path()).unwrap();
        assert_eq!(text, file.rendered, "{}", file.path);
    }
}

#[test]
fn a_stale_option_is_refused_by_the_plan_id() {
    let w = world();
    let p = project_of(&w);
    let mut spec = ranged();
    spec.ce = Some(answered_block());
    let plan = export_plan(&w.ctx, with_accept(request(&p, &spec), &[])).unwrap();
    // applying with another option than the one that was reviewed is refused
    let err = apply_plan(
        &w.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id,
            request: with_accept(request(&p, &spec), &["bulk"]),
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    );
    assert!(err.is_err());
}
