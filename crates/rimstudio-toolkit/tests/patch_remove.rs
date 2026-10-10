//! Taking the generated Combat Extended patch of a weapon out of a project: what the scan reports, the
//! plan, the apply step and the files that must never be touched.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{Proj, fixture, project, put, ranged, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_design::model::{CePatchSpec, Sourced};
use rimstudio_ipc_types::designer::{
    ConvertAnswersDto, ConvertCandidateDto, ConvertRequestDto, ConvertStatusDto,
    DesignerApplyPlanRequest, DesignerConvertScanRequest, DesignerExportPlanRequest, FileActionDto,
    FileKindDto, PatchOriginDto,
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

/// A patch file a person wrote: a section comment of the same shape, but no generator mark.
const HAND_WRITTEN: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Patch>
  <!-- ====== RS_ProjBlade ====== my own conversion -->
  <Operation Class="PatchOperationReplace">
    <xpath>/Defs/ThingDef[defName="RS_ProjBlade"]/statBases/Mass</xpath>
    <value><Mass>1.1</Mass></value>
  </Operation>
</Patch>
"#;

fn answers() -> ConvertAnswersDto {
    let ce = CePatchSpec {
        bulk: Some(Sourced::typed(6.0)),
        sway_factor: Some(Sourced::typed(1.1)),
        shot_spread: Some(Sourced::typed(0.09)),
        magazine_size: Some(Sourced::typed(25)),
        reload_time: Some(Sourced::typed(4.0)),
        ..CePatchSpec::default()
    };
    ConvertAnswersDto {
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("RS_CE_Class".into()),
        one_handed: Some(false),
        belt_fed: Some(false),
        overrides: serde_json::from_value(serde_json::to_value(ce).unwrap()).unwrap(),
        ..ConvertAnswersDto::default()
    }
}

fn convert_request(p: &Proj, def: &str, remove: bool) -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        convert: Some(ConvertRequestDto {
            def_name: def.into(),
            answers: if remove {
                ConvertAnswersDto::default()
            } else {
                answers()
            },
            groups: Vec::new(),
            remove,
        }),
        ..request(p, &ranged())
    }
}

fn apply(
    f: &common::Fixture,
    req: DesignerExportPlanRequest,
) -> rimstudio_ipc_types::designer::ApplyReportDto {
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    apply_plan(
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
    .unwrap()
}

fn scan(f: &common::Fixture, p: &Proj) -> Vec<ConvertCandidateDto> {
    convert_scan(
        &f.ctx,
        DesignerConvertScanRequest {
            project_id: p.id.clone(),
            include_converted: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap()
    .candidates
}

fn read(p: &Proj, rel: &str) -> String {
    std::fs::read_to_string(p.root.join(rel).as_std_path()).unwrap()
}

fn exists(p: &Proj, rel: &str) -> bool {
    p.root.join(rel).as_std_path().exists()
}

/// Every file below `root`, relative to it with `/` separators, sorted.
fn files_under(root: &camino::Utf8Path) -> Vec<String> {
    fn go(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                go(&path, base, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    go(root.as_std_path(), root.as_std_path(), &mut out);
    out.sort();
    out
}

/// A project with two guns converted by the generator into one patch file.
fn converted() -> (common::Fixture, Proj, String) {
    let f = fixture(true);
    let p = project(&f, &[("Defs/RS_ProjWeapons.xml", DEFS)]);
    apply(&f, convert_request(&p, "RS_ProjGun", false));
    apply(&f, convert_request(&p, "RS_ProjBare", false));
    let patch = files_under(&p.root)
        .into_iter()
        .find(|rel| rel.ends_with("Weapons_Ranged.xml"))
        .expect("the generated patch file");
    (f, p, patch)
}

#[test]
fn a_generated_conversion_is_reported_as_removable_with_its_files() {
    let (f, p, patch) = converted();
    let candidates = scan(&f, &p);
    let gun = candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjGun")
        .unwrap();
    assert_eq!(gun.status, ConvertStatusDto::AlreadyCe);
    let existing = gun.existing.as_ref().expect("the existing patch");
    assert_eq!(existing.origin, PatchOriginDto::Rimstudio);
    assert!(existing.removable);
    assert_eq!(existing.files, vec![patch.clone()]);
    assert_eq!(existing.file.as_deref(), Some(patch.as_str()));
    // a conversion inside the definition is the project's, and there is nothing generated to remove
    let inline = candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjConverted")
        .unwrap();
    let existing = inline.existing.as_ref().expect("the existing patch");
    assert!(!existing.removable && existing.files.is_empty());
    assert_ne!(existing.origin, PatchOriginDto::Rimstudio);
    // a weapon that is not converted has no existing patch
    assert!(
        candidates
            .iter()
            .all(|c| { c.status == ConvertStatusDto::AlreadyCe || c.existing.is_none() })
    );
}

#[test]
fn removing_one_weapon_takes_its_section_and_keeps_the_other() {
    let (f, p, patch) = converted();
    let before = read(&p, &patch);
    assert!(before.contains("RS_ProjGun") && before.contains("RS_ProjBare"));
    let load_folders = read(&p, "LoadFolders.xml");
    let definitions = read(&p, "Defs/RS_ProjWeapons.xml");

    let req = convert_request(&p, "RS_ProjGun", true);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(plan.files.len(), 1);
    let file = &plan.files[0];
    assert_eq!(file.path, patch);
    assert_eq!(file.kind, FileKindDto::CePatch);
    assert_eq!(file.action, FileActionDto::UpdateRegion);
    let diff = file.diff.as_deref().unwrap();
    assert!(diff.contains("-") && diff.contains("RS_ProjGun"), "{diff}");
    assert!(!file.rendered.contains("RS_ProjGun"), "{}", file.rendered);
    assert!(file.rendered.contains("RS_ProjBare"));

    let report = apply(&f, req);
    assert_eq!(report.written.len(), 1);
    assert_eq!(report.written[0].action, FileActionDto::UpdateRegion);
    assert!(report.written[0].backup_path.is_some(), "a backup is made");
    assert_eq!(
        report.dry_apply_ok, None,
        "a removal has nothing to dry apply"
    );
    let after = read(&p, &patch);
    assert!(!after.contains("RS_ProjGun"));
    assert!(after.contains("RS_ProjBare"));
    assert_eq!(read(&p, "LoadFolders.xml"), load_folders);
    assert_eq!(read(&p, "Defs/RS_ProjWeapons.xml"), definitions);

    // the weapon is a candidate for conversion again
    let candidates = scan(&f, &p);
    let gun = candidates
        .iter()
        .find(|c| c.def_name == "RS_ProjGun")
        .unwrap();
    assert_eq!(gun.status, ConvertStatusDto::NotConverted, "{gun:?}");
    assert!(gun.existing.is_none());
}

#[test]
fn removing_the_last_section_deletes_the_file_after_a_backup() {
    let (f, p, patch) = converted();
    apply(&f, convert_request(&p, "RS_ProjGun", true));
    let req = convert_request(&p, "RS_ProjBare", true);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(plan.files[0].action, FileActionDto::Delete);
    assert!(plan.files[0].rendered.is_empty());
    assert!(
        plan.files[0]
            .diff
            .as_deref()
            .unwrap()
            .contains("RS_ProjBare")
    );

    let report = apply(&f, req);
    assert_eq!(report.written[0].action, FileActionDto::Delete);
    assert_eq!(report.written[0].bytes, 0);
    let backup = report.written[0].backup_path.clone().expect("backup");
    assert!(!exists(&p, &patch), "the file is gone");
    assert!(
        !backup.starts_with(p.root.as_str()),
        "backups stay outside the mod folder: {backup}"
    );
}

#[test]
fn a_hand_written_patch_is_never_touched() {
    let f = fixture(true);
    let p = project(
        &f,
        &[
            ("Defs/RS_ProjWeapons.xml", DEFS),
            ("Patches/Hand.xml", HAND_WRITTEN),
        ],
    );
    let plan = export_plan(&f.ctx, convert_request(&p, "RS_ProjBlade", true)).unwrap();
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.remove-nothing"),
        "{:?}",
        plan.diagnostics
    );
    assert_eq!(read(&p, "Patches/Hand.xml"), HAND_WRITTEN);
}

#[test]
fn removing_needs_no_combat_extended_data() {
    // a project made on a machine with Combat Extended can be cleaned on one without it
    let (_with, p, patch) = converted();
    let without = fixture(false);
    let record = without.ctx.env().projects().open(&p.root).unwrap();
    let req = DesignerExportPlanRequest {
        project_id: record.id.as_str().to_owned(),
        ..convert_request(&p, "RS_ProjGun", true)
    };
    let plan = export_plan(&without.ctx, req).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(plan.files[0].path, patch);
}

#[test]
fn a_file_changed_after_the_plan_is_not_overwritten() {
    let (f, p, patch) = converted();
    let req = convert_request(&p, "RS_ProjGun", true);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    // another program edits the file between the plan and the apply
    let edited = format!("{}\n<!-- edited elsewhere -->", read(&p, &patch));
    put(&p.root, &patch, &edited);
    let err = apply_plan(
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
    .unwrap_err();
    assert!(
        matches!(
            err,
            rimstudio_toolkit::error::ToolkitError::PlanStale { .. }
                | rimstudio_toolkit::error::ToolkitError::ApplyRefused { .. }
        ),
        "{err:?}"
    );
    assert_eq!(read(&p, &patch), edited);
}
