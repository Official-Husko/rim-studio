//! Converting an existing mod's weapons to Combat Extended: scan, ask list, plans and update mode.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{Proj, fixture, project, ranged, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_design::model::{CePatchSpec, CeToolPenetration, Sourced};
use rimstudio_ipc_types::designer::{
    ConvertAnswerGroupDto, ConvertAnswersDto, ConvertRequestDto, ConvertStatusDto,
    DesignerApplyPlanRequest, DesignerConvertScanRequest, DesignerExportPlanRequest, FileKindDto,
    WritePlanDto,
};
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{apply_plan, convert_scan};

const DEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
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
  <ThingDef>
    <defName>RS_ProjBare</defName>
    <label>proj bare</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_Shot01</defaultProjectile>
        <warmupTime>1.2</warmupTime>
        <range>22</range>
      </li>
    </verbs>
    <weaponTags><li>RS_Rifle</li></weaponTags>
  </ThingDef>
  <ThingDef>
    <defName>RS_ProjBlade</defName>
    <label>proj blade</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases><Mass>1.2</Mass></statBases>
    <weaponTags><li>RS_Melee</li></weaponTags>
    <tools>
      <li>
        <label>edge</label>
        <capacities><li>Cut</li></capacities>
        <power>10</power>
        <cooldownTime>2</cooldownTime>
      </li>
    </tools>
  </ThingDef>
  <ThingDef>
    <defName>RS_ProjConverted</defName>
    <label>proj converted</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases>
      <Mass>2.5</Mass>
      <Bulk>6</Bulk>
    </statBases>
    <verbs>
      <li Class="CombatExtended.VerbPropertiesCE">
        <verbClass>CombatExtended.Verb_ShootCE</verbClass>
        <defaultProjectile>RS_CeBullet1</defaultProjectile>
        <warmupTime>0.8</warmupTime>
        <range>40</range>
      </li>
    </verbs>
    <comps>
      <li Class="CombatExtended.CompProperties_AmmoUser">
        <magazineSize>20</magazineSize>
        <reloadTime>3</reloadTime>
        <ammoSet>RS_AmmoSetA</ammoSet>
      </li>
    </comps>
    <weaponTags><li>RS_Rifle</li></weaponTags>
  </ThingDef>
</Defs>
"#;

fn dto_of<T: serde::Serialize, U: serde::de::DeserializeOwned>(v: &T) -> U {
    serde_json::from_value(serde_json::to_value(v).unwrap()).unwrap()
}

fn gun_answers() -> ConvertAnswersDto {
    ConvertAnswersDto {
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("RS_CE_Class".into()),
        one_handed: Some(false),
        belt_fed: Some(false),
        overrides: dto_of(&CePatchSpec {
            bulk: Some(Sourced::typed(6.0)),
            sway_factor: Some(Sourced::typed(1.1)),
            shot_spread: Some(Sourced::typed(0.09)),
            magazine_size: Some(Sourced::typed(25)),
            reload_time: Some(Sourced::typed(4.0)),
            ..CePatchSpec::default()
        }),
        ..ConvertAnswersDto::default()
    }
}

fn melee_answers() -> ConvertAnswersDto {
    ConvertAnswersDto {
        overrides: dto_of(&CePatchSpec {
            bulk: Some(Sourced::typed(5.0)),
            melee_crit_chance: Some(Sourced::typed(0.1)),
            melee_parry_chance: Some(Sourced::typed(0.2)),
            melee_dodge_chance: Some(Sourced::typed(-0.05)),
            tool_penetration: vec![CeToolPenetration {
                tool: "edge".into(),
                sharp: Some(Sourced::typed(0.5)),
                blunt: Some(Sourced::typed(2.0)),
            }],
            ..CePatchSpec::default()
        }),
        ..ConvertAnswersDto::default()
    }
}

fn setup() -> (common::Fixture, Proj) {
    let f = fixture(true);
    let p = project(&f, &[("Defs/RS_ProjWeapons.xml", DEFS)]);
    (f, p)
}

fn convert_request(p: &Proj, def: &str, answers: ConvertAnswersDto) -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        convert: Some(ConvertRequestDto {
            def_name: def.into(),
            answers,
            groups: Vec::new(),
        }),
        ..request(p, &ranged())
    }
}

fn scan(
    f: &common::Fixture,
    p: &Proj,
    include_converted: bool,
) -> rimstudio_ipc_types::designer::ConvertScanDto {
    convert_scan(
        &f.ctx,
        DesignerConvertScanRequest {
            project_id: p.id.clone(),
            include_converted,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap()
}

fn read(p: &Proj, rel: &str) -> String {
    std::fs::read_to_string(p.root.join(rel).as_std_path()).unwrap()
}

#[test]
fn the_scan_lists_converted_and_unconverted_weapons_with_their_questions() {
    let (f, p) = setup();
    let scan = scan(&f, &p, true);
    let status = |name: &str| {
        scan.candidates
            .iter()
            .find(|c| c.def_name == name)
            .unwrap_or_else(|| panic!("{name} missing: {:?}", scan.candidates))
            .status
    };
    assert_eq!(status("RS_ProjGun"), ConvertStatusDto::NotConverted);
    assert_eq!(status("RS_ProjBare"), ConvertStatusDto::NotConverted);
    assert_eq!(status("RS_ProjBlade"), ConvertStatusDto::NotConverted);
    assert_eq!(status("RS_ProjConverted"), ConvertStatusDto::AlreadyCe);
    assert_eq!(scan.counts.not_converted, 3);
    assert_eq!(scan.counts.already_ce, 1);
    let gun = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjGun")
        .unwrap();
    assert_eq!(gun.file.as_deref(), Some("Defs/RS_ProjWeapons.xml"));
    assert!(
        gun.asks
            .iter()
            .any(|a| a.field == "/ce/ammoSet" && a.options.contains(&"RS_AmmoSetA".to_owned()))
    );
    let done = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjConverted")
        .unwrap();
    assert!(done.asks.is_empty());
    let names: Vec<&str> = scan
        .candidates
        .iter()
        .map(|c| c.def_name.as_str())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "candidates are ordered by definition name");
    // the converted ones can be left out
    let without = self::scan(&f, &p, false);
    assert!(
        without
            .candidates
            .iter()
            .all(|c| c.status != ConvertStatusDto::AlreadyCe)
    );
}

#[test]
fn a_conversion_with_open_questions_has_no_files_and_one_error_per_question() {
    let (f, p) = setup();
    let plan = export_plan(
        &f.ctx,
        convert_request(&p, "RS_ProjGun", ConvertAnswersDto::default()),
    )
    .unwrap();
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
    let fields: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code == "designer.convert-needs-answer")
        .filter_map(|d| d.field.as_deref())
        .collect();
    assert!(fields.contains(&"/ce/ammoSet"), "{fields:?}");
}

fn only_patch_files(plan: &WritePlanDto) {
    assert!(!plan.files.is_empty());
    assert!(
        plan.files
            .iter()
            .all(|x| matches!(x.kind, FileKindDto::CePatch | FileKindDto::LoadFolders)),
        "{:?}",
        plan.files
            .iter()
            .map(|x| (&x.path, x.kind))
            .collect::<Vec<_>>()
    );
    for file in &plan.files {
        if file.rendered.contains("CombatExtended.") {
            assert!(file.path.starts_with("CE/"), "{}", file.path);
        }
    }
}

#[test]
fn converting_a_gun_writes_patch_files_only_and_never_touches_the_definition() {
    let (f, p) = setup();
    let before = read(&p, "Defs/RS_ProjWeapons.xml");
    let req = convert_request(&p, "RS_ProjGun", gun_answers());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    only_patch_files(&plan);
    let patch = plan
        .files
        .iter()
        .find(|x| x.kind == FileKindDto::CePatch)
        .unwrap();
    assert!(patch.rendered.contains("PatchOperationMakeGunCECompatible"));
    assert!(patch.rendered.contains("RS_ProjGun"));
    let report = apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id,
            request: req,
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.dry_apply_ok, Some(true), "{:?}", report.diagnostics);
    assert_eq!(
        read(&p, "Defs/RS_ProjWeapons.xml"),
        before,
        "the definition is byte identical (IT-057)"
    );
    assert!(p.root.join("LoadFolders.xml").exists());
}

#[test]
fn a_def_without_stat_bases_gets_a_patch_that_applies_cleanly() {
    let (f, p) = setup();
    let req = convert_request(&p, "RS_ProjBare", gun_answers());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    only_patch_files(&plan);
    let patch = plan
        .files
        .iter()
        .find(|x| x.kind == FileKindDto::CePatch)
        .unwrap();
    // the gun conversion creates the missing statBases container itself
    assert!(patch.rendered.contains("PatchOperationMakeGunCECompatible"));
    assert!(patch.rendered.contains("<statBases>"));
    let report = apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id,
            request: req,
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.dry_apply_ok, Some(true), "{:?}", report.diagnostics);
}

#[test]
fn a_melee_weapon_converts_with_the_tool_penetration_given() {
    let (f, p) = setup();
    let plan = export_plan(&f.ctx, convert_request(&p, "RS_ProjBlade", melee_answers())).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    only_patch_files(&plan);
    let patch = plan
        .files
        .iter()
        .find(|x| x.kind == FileKindDto::CePatch)
        .unwrap();
    assert!(patch.path.contains("Weapons_Melee"), "{}", patch.path);
    assert!(!patch.rendered.contains("MakeGun"));
}

#[test]
fn an_already_converted_def_uses_update_mode_and_never_a_second_make_gun() {
    let (f, p) = setup();
    let answers = ConvertAnswersDto {
        overrides: dto_of(&CePatchSpec {
            bulk: Some(Sourced::typed(7.5)),
            ..CePatchSpec::default()
        }),
        ..ConvertAnswersDto::default()
    };
    let plan = export_plan(&f.ctx, convert_request(&p, "RS_ProjConverted", answers)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    only_patch_files(&plan);
    for file in &plan.files {
        assert!(
            !file.rendered.contains("MakeGunCECompatible"),
            "{}",
            file.path
        );
    }
    let patch = plan
        .files
        .iter()
        .find(|x| x.kind == FileKindDto::CePatch)
        .unwrap();
    assert!(patch.path.ends_with("_Update.xml"), "{}", patch.path);
    assert!(patch.rendered.contains("Bulk"));
}

#[test]
fn an_unknown_definition_is_an_error_plan() {
    let (f, p) = setup();
    let plan = export_plan(
        &f.ctx,
        convert_request(&p, "RS_NotThere", ConvertAnswersDto::default()),
    )
    .unwrap();
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.convert-unknown-def")
    );
}

#[test]
fn the_conversion_needs_combat_extended_data() {
    let f = fixture(false);
    let p = project(&f, &[("Defs/RS_ProjWeapons.xml", DEFS)]);
    let err = convert_scan(
        &f.ctx,
        DesignerConvertScanRequest {
            project_id: p.id.clone(),
            include_converted: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn a_scan_can_be_cancelled() {
    let (f, p) = setup();
    let token = CancelToken::new();
    token.cancel();
    let err = convert_scan(
        &f.ctx,
        DesignerConvertScanRequest {
            project_id: p.id.clone(),
            include_converted: true,
        },
        &NoopProgress,
        &token,
    )
    .unwrap_err();
    assert_eq!(err.code(), "job.cancelled");
}

#[test]
fn a_number_ask_of_the_scan_and_of_the_plan_carries_the_reason_and_the_rejected_estimate() {
    let (f, p) = setup();
    let scan = scan(&f, &p, false);
    let gun = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjGun")
        .unwrap();
    let with_reason: Vec<_> = gun.asks.iter().filter(|a| a.reason.is_some()).collect();
    assert!(
        !with_reason.is_empty(),
        "the fictional library cannot rate every number"
    );
    // a choice without an estimate carries neither
    let ammo = gun.asks.iter().find(|a| a.field == "/ce/ammoSet").unwrap();
    assert!(ammo.reason.is_none() && ammo.suggestion.is_none());
    // the plan repeats the reason in the message of the question
    let plan = export_plan(
        &f.ctx,
        convert_request(&p, "RS_ProjGun", ConvertAnswersDto::default()),
    )
    .unwrap();
    let asked = with_reason[0];
    let message = plan
        .diagnostics
        .iter()
        .find(|d| {
            d.code == "designer.convert-needs-answer" && d.field.as_deref() == Some(&asked.field)
        })
        .map(|d| d.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains(asked.reason.as_deref().unwrap_or("?")),
        "{message}"
    );
}

fn gun_def(name: &str, projectile: &str) -> String {
    format!(
        r#"  <ThingDef>
    <defName>{name}</defName>
    <label>{name}</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases><Mass>3</Mass><RangedWeapon_Cooldown>1.5</RangedWeapon_Cooldown></statBases>
    <verbs><li>
      <verbClass>Verb_Shoot</verbClass>
      <defaultProjectile>{projectile}</defaultProjectile>
      <warmupTime>1</warmupTime>
      <range>25</range>
    </li></verbs>
    <weaponTags><li>RS_Rifle</li></weaponTags>
  </ThingDef>
"#
    )
}

fn family_project() -> (common::Fixture, Proj) {
    let f = fixture(true);
    let defs = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Defs>\n{}{}{}</Defs>\n",
        gun_def("RS_FamA", "RS_Shot00"),
        gun_def("RS_FamB", "RS_Shot00"),
        gun_def("RS_FamC", "RS_Shot01"),
    );
    let p = project(&f, &[("Defs/RS_Family.xml", &defs)]);
    (f, p)
}

fn family_group() -> ConvertAnswerGroupDto {
    ConvertAnswerGroupDto {
        family: None,
        def_names: Vec::new(),
        answers: serde_json::json!({
            "ammoSet": "RS_AmmoSetA", "weaponTagClass": "RS_CE_Class",
            "oneHanded": false, "beltFed": false,
            "overrides": {
                "bulk": {"value": 6.0}, "swayFactor": {"value": 1.1}, "shotSpread": {"value": 0.09},
                "magazineSize": {"value": 25}, "reloadTime": {"value": 4.0}
            }
        }),
    }
}

fn grouped_request(p: &Proj, def: &str, group: ConvertAnswerGroupDto) -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        convert: Some(ConvertRequestDto {
            def_name: def.into(),
            answers: ConvertAnswersDto::default(),
            groups: vec![group],
        }),
        ..request(p, &ranged())
    }
}

#[test]
fn guns_of_one_caliber_and_class_share_a_family_key_in_the_scan() {
    let (f, p) = family_project();
    let scan = scan(&f, &p, false);
    let family = |name: &str| {
        scan.candidates
            .iter()
            .find(|c| c.def_name == name)
            .unwrap()
            .family
            .clone()
    };
    assert_eq!(family("RS_FamA"), "ranged/RS_Rifle/RS_Shot00");
    assert_eq!(family("RS_FamA"), family("RS_FamB"));
    assert_ne!(family("RS_FamA"), family("RS_FamC"));
}

#[test]
fn one_family_group_answers_every_gun_of_the_family_and_only_those() {
    let (f, p) = family_project();
    let scan = scan(&f, &p, false);
    let key = scan
        .candidates
        .iter()
        .find(|c| c.def_name == "RS_FamA")
        .unwrap()
        .family
        .clone();
    let mut group = family_group();
    group.family = Some(key);
    for def in ["RS_FamA", "RS_FamB"] {
        let plan = export_plan(&f.ctx, grouped_request(&p, def, group.clone())).unwrap();
        assert!(!plan.has_errors, "{def}: {:?}", plan.diagnostics);
        only_patch_files(&plan);
        let patch = plan
            .files
            .iter()
            .find(|x| x.kind == FileKindDto::CePatch)
            .unwrap();
        assert!(patch.rendered.contains(def));
    }
    // the gun of the other caliber is not covered by the group and keeps its open questions
    let other = export_plan(&f.ctx, grouped_request(&p, "RS_FamC", group)).unwrap();
    assert!(other.has_errors);
    assert!(
        other
            .diagnostics
            .iter()
            .any(|d| d.code == "designer.convert-needs-answer")
    );
}

#[test]
fn a_group_of_definition_names_answers_exactly_those_guns() {
    let (f, p) = family_project();
    let mut group = family_group();
    group.def_names = vec!["RS_FamA".into(), "RS_FamC".into()];
    for (def, ok) in [("RS_FamA", true), ("RS_FamB", false), ("RS_FamC", true)] {
        let plan = export_plan(&f.ctx, grouped_request(&p, def, group.clone())).unwrap();
        assert_eq!(!plan.has_errors, ok, "{def}: {:?}", plan.diagnostics);
    }
}
