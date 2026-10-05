//! `designer_lint_files` over a fictional project: hand written patch files, generated ones, hostile files
//! and the rules that need Combat Extended data.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_project;

use common_project::{Proj, fixture, project, put};
use rimstudio_ipc_types::designer::{
    DesignerLintFilesRequest, DesignerLintFilesResult, LintFileStatusDto, LintedFileDto,
};
use rimstudio_ipc_types::diagnostic::SeverityDto;
use rimstudio_toolkit::designer::lint_files;

const MAKE_GUN: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Patch>
  <Operation Class="PatchOperationFindMod">
    <mods><li>ceteam.combatextended</li></mods>
    <match Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="RS_Old"]/foo</xpath></match>
  </Operation>
  <Operation Class="CombatExtended.PatchOperationMakeGunCECompatible">
    <defName>RS_LintGun</defName>
    <statBases><Bulk>6</Bulk></statBases>
    <Properties>
      <verbClass>Verb_Shoot</verbClass>
      <defaultProjectile>RS_NoSuchProjectile</defaultProjectile>
    </Properties>
    <AmmoUser><magazineSize>20</magazineSize><ammoSet>RS_NoSuchSet</ammoSet></AmmoUser>
    <FireModes />
  </Operation>
</Patch>
"#;

const LOAD_FOLDERS: &str = r#"<loadFolders>
  <v1.6>
    <li>/</li>
    <li IfModActive="ceteam.combatextended">Compat/CombatExtended</li>
  </v1.6>
</loadFolders>
"#;

const DTD: &str = "<?xml version=\"1.0\"?>\n<!DOCTYPE Patch [<!ENTITY a \"b\">]>\n<Patch/>\n";

fn setup(with_ce: bool) -> (common::Fixture, Proj) {
    let f = fixture(with_ce);
    let p = project(
        &f,
        &[
            ("LoadFolders.xml", LOAD_FOLDERS),
            ("Patches/hand.xml", MAKE_GUN),
            ("Compat/CombatExtended/Patches/gated.xml", MAKE_GUN),
            ("Compat/CombatExtended/Defs/RS_Gated.xml", "<Defs/>"),
            ("Patches/not_xml.xml", "this is not xml <<<"),
            ("Patches/dtd.xml", DTD),
            (
                "Defs/RS_Weapons.xml",
                "<Defs><ThingDef><defName>RS_Old</defName></ThingDef></Defs>",
            ),
        ],
    );
    (f, p)
}

fn lint(f: &common::Fixture, p: &Proj, paths: &[&str]) -> DesignerLintFilesResult {
    lint_files(
        &f.ctx,
        &DesignerLintFilesRequest {
            project_id: p.id.clone(),
            paths: paths.iter().map(|s| (*s).to_owned()).collect(),
        },
    )
    .unwrap()
}

fn file<'a>(r: &'a DesignerLintFilesResult, path: &str) -> &'a LintedFileDto {
    r.files.iter().find(|f| f.path == path).unwrap_or_else(|| {
        panic!(
            "{path} missing: {:?}",
            r.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        )
    })
}

fn rules(f: &LintedFileDto) -> Vec<&str> {
    f.findings
        .iter()
        .filter_map(|x| x.rule_id.as_deref())
        .collect()
}

#[test]
fn the_default_set_is_every_patch_file_and_nothing_else() {
    let (f, p) = setup(true);
    let r = lint(&f, &p, &[]);
    let paths: Vec<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "Compat/CombatExtended/Patches/gated.xml",
            "Patches/dtd.xml",
            "Patches/hand.xml",
            "Patches/not_xml.xml",
        ],
        "the Defs files are not patch files"
    );
    assert_eq!(r.counts.files, 4);
    assert_eq!(r.counts.checked, 2);
}

#[test]
fn a_hand_written_patch_gets_findings_with_operation_index_xpath_and_explanation() {
    let (f, p) = setup(true);
    let r = lint(&f, &p, &["Patches/hand.xml"]);
    let hand = file(&r, "Patches/hand.xml");
    assert_eq!(hand.status, LintFileStatusDto::Checked);
    assert_eq!(hand.operations, 2);
    let ids = rules(hand);
    for rule in ["CEP001", "CEP004", "CEP009", "CEP013", "CEP014", "CEP023"] {
        assert!(ids.contains(&rule), "{rule} missing in {ids:?}");
    }
    let find_mod = hand
        .findings
        .iter()
        .find(|x| x.rule_id.as_deref() == Some("CEP001"))
        .unwrap();
    assert_eq!(find_mod.operation, Some(1));
    assert_eq!(find_mod.file.as_deref(), Some("Patches/hand.xml"));
    assert_eq!(find_mod.severity, SeverityDto::Error);
    assert!(
        find_mod
            .explanation
            .as_deref()
            .unwrap()
            .contains("display name")
    );
    let ammo = hand
        .findings
        .iter()
        .find(|x| x.rule_id.as_deref() == Some("CEP013"))
        .unwrap();
    assert_eq!(ammo.operation, Some(2));
    assert!(
        ammo.field
            .as_deref()
            .unwrap()
            .ends_with("/AmmoUser/ammoSet")
    );
    assert!(ammo.message.contains("RS_NoSuchSet"));
    assert!(r.ce_data);
}

#[test]
fn a_file_in_the_gated_folder_is_not_flagged_for_a_missing_gate() {
    let (f, p) = setup(true);
    let r = lint(&f, &p, &["Compat/CombatExtended/Patches/gated.xml"]);
    let gated = file(&r, "Compat/CombatExtended/Patches/gated.xml");
    assert!(!rules(gated).contains(&"CEP004"), "{:?}", rules(gated));
    assert!(rules(gated).contains(&"CEP013"));
}

#[test]
fn without_combat_extended_data_the_data_rules_are_listed_as_not_checked() {
    let (f, p) = setup(false);
    let r = lint(&f, &p, &["Patches/hand.xml"]);
    assert!(!r.ce_data);
    let hand = file(&r, "Patches/hand.xml");
    let ids = rules(hand);
    assert!(!ids.contains(&"CEP013"));
    assert!(!ids.contains(&"CEP014"));
    assert!(ids.contains(&"CEP001"), "structural rules still run");
    for rule in ["CEP013", "CEP014", "CEP016"] {
        let entry = r
            .not_checked
            .iter()
            .find(|n| n.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} not listed: {:?}", r.not_checked));
        assert!(!entry.reason.is_empty());
    }
}

#[test]
fn with_data_only_the_tables_that_are_not_supplied_stay_unchecked() {
    let (f, p) = setup(true);
    let r = lint(&f, &p, &["Patches/hand.xml"]);
    let ids: Vec<&str> = r.not_checked.iter().map(|n| n.rule_id.as_str()).collect();
    assert!(!ids.contains(&"CEP013"));
    assert!(ids.contains(&"CEP010"));
    assert!(ids.contains(&"CEP015"));
}

#[test]
fn hostile_files_give_findings_and_never_fail_the_call() {
    let (f, p) = setup(true);
    put(&p.root, "Patches/empty.xml", "");
    put(&p.root, "Patches/binary.xml", "\u{0}\u{1}\u{2}<<<>>>");
    put(&p.root, "Patches/wrongroot.xml", "<Defs/>");
    let big = format!("<Patch>{}</Patch>", " ".repeat(2_100_000));
    put(&p.root, "Patches/huge.xml", &big);
    let r = lint(&f, &p, &[]);
    let status = |name: &str| file(&r, name).status;
    assert_eq!(
        status("Patches/not_xml.xml"),
        LintFileStatusDto::ParseFailed
    );
    assert_eq!(status("Patches/dtd.xml"), LintFileStatusDto::Doctype);
    assert_eq!(status("Patches/huge.xml"), LintFileStatusDto::TooLarge);
    assert_eq!(status("Patches/empty.xml"), LintFileStatusDto::ParseFailed);
    assert_eq!(status("Patches/binary.xml"), LintFileStatusDto::ParseFailed);
    assert_eq!(status("Patches/wrongroot.xml"), LintFileStatusDto::Checked);
    for name in [
        "Patches/not_xml.xml",
        "Patches/dtd.xml",
        "Patches/huge.xml",
        "Patches/empty.xml",
        "Patches/binary.xml",
        "Patches/wrongroot.xml",
    ] {
        let one = file(&r, name);
        assert!(
            one.findings
                .iter()
                .any(|x| x.rule_id.as_deref() == Some("CEP017") && x.severity == SeverityDto::Error),
            "{name}: {:?}",
            one.findings
        );
    }
    assert!(file(&r, "Patches/huge.xml").bytes > 2_000_000);
}

#[test]
fn named_paths_that_do_not_exist_or_leave_the_project_are_findings() {
    let (f, p) = setup(true);
    let r = lint(
        &f,
        &p,
        &[
            "Patches/nope.xml",
            "../outside.xml",
            "/etc/passwd",
            "Patches",
            "Patches\\hand.xml",
            "Patches/hand.xml",
        ],
    );
    assert_eq!(
        file(&r, "Patches/nope.xml").status,
        LintFileStatusDto::Missing
    );
    assert_eq!(
        file(&r, "../outside.xml").status,
        LintFileStatusDto::Unreadable
    );
    assert_eq!(
        file(&r, "/etc/passwd").status,
        LintFileStatusDto::Unreadable
    );
    assert_eq!(file(&r, "Patches").status, LintFileStatusDto::Unreadable);
    assert_eq!(
        file(&r, "Patches/hand.xml").status,
        LintFileStatusDto::Checked
    );
    assert_eq!(r.files.len(), 5, "the repeated path is listed once");
    for name in [
        "Patches/nope.xml",
        "../outside.xml",
        "/etc/passwd",
        "Patches",
    ] {
        assert!(!file(&r, name).findings.is_empty(), "{name}");
    }
}

#[test]
fn a_named_file_that_is_no_patch_is_reported_by_its_root() {
    let (f, p) = setup(true);
    let r = lint(&f, &p, &["Defs/RS_Weapons.xml"]);
    let defs = file(&r, "Defs/RS_Weapons.xml");
    assert!(rules(defs).contains(&"CEP017"));
}

#[test]
fn the_gated_folder_lists_only_patch_files() {
    let (f, p) = setup(true);
    put(&p.root, "Compat/CombatExtended/Misc/loose.xml", "<Patch/>");
    put(&p.root, "Compat/CombatExtended/Misc/notes.xml", "<Notes/>");
    let r = lint(&f, &p, &[]);
    assert_eq!(
        file(&r, "Compat/CombatExtended/Misc/loose.xml").status,
        LintFileStatusDto::Checked
    );
    assert_eq!(
        file(&r, "Compat/CombatExtended/Misc/notes.xml").status,
        LintFileStatusDto::NotAPatch
    );
    assert!(
        file(&r, "Compat/CombatExtended/Misc/notes.xml")
            .findings
            .is_empty()
    );
    assert!(
        r.files
            .iter()
            .all(|x| x.path != "Compat/CombatExtended/Defs/RS_Gated.xml")
            || file(&r, "Compat/CombatExtended/Defs/RS_Gated.xml").status
                == LintFileStatusDto::NotAPatch
    );
}

#[test]
fn an_unknown_project_is_an_error() {
    let (f, _p) = setup(true);
    let err = lint_files(
        &f.ctx,
        &DesignerLintFilesRequest {
            project_id: "p-does-not-exist".into(),
            paths: Vec::new(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("project"), "{err}");
}

#[test]
fn the_result_is_deterministic() {
    let (f, p) = setup(true);
    let a = serde_json::to_string(&lint(&f, &p, &[])).unwrap();
    let b = serde_json::to_string(&lint(&f, &p, &[])).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_loadfolders_that_never_loads_the_folder_is_a_project_or_file_finding() {
    let (f, p) = setup(true);
    put(
        &p.root,
        "LoadFolders.xml",
        "<loadFolders><v1.6><li>/</li></v1.6></loadFolders>",
    );
    let r = lint(&f, &p, &["Compat/CombatExtended/Patches/gated.xml"]);
    let gated = file(&r, "Compat/CombatExtended/Patches/gated.xml");
    assert!(rules(gated).contains(&"CEP018"), "{:?}", rules(gated));
}
