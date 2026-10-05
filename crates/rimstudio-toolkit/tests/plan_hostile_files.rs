//! A plan against project files that are unusual or hostile: nothing is guessed, the plan reports an error
//! and the file is left alone, or the edit keeps every byte it does not own.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{fixture, project, ranged, ranged_ce, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_ipc_types::designer::DesignerApplyPlanRequest;
use rimstudio_toolkit::designer::apply_plan;
use rimstudio_toolkit::designer::plan::{export_plan, prepare};

fn utf16le(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

#[test]
fn files_that_are_not_utf8_text_are_reported_and_never_overwritten() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let cases: [(&str, Vec<u8>); 3] = [
        (
            "utf16",
            utf16le("<Defs>\r\n<ThingDef><defName>RS_Other</defName></ThingDef>\r\n</Defs>"),
        ),
        ("latin1", b"<Defs><!-- caf\xe9 --></Defs>".to_vec()),
        ("binary", vec![0, 159, 146, 150, 0, 1, 2]),
    ];
    for (name, bytes) in cases {
        let path = p.root.join("Defs/Weapons/RS_NewRifle.xml");
        std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
        std::fs::write(path.as_std_path(), &bytes).unwrap();
        let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
        assert!(prepared.plan.has_errors(), "{name}");
        let diag = prepared
            .plan
            .diagnostics
            .iter()
            .find(|d| d.message.contains("RS_NewRifle.xml"))
            .unwrap_or_else(|| panic!("{name}: {:?}", prepared.plan.diagnostics));
        assert_eq!(
            diag.code.as_str(),
            "designer.merge-failed",
            "{name}: an unreadable file is not a path problem"
        );
        assert!(prepared.plan.file("Defs/Weapons/RS_NewRifle.xml").is_none());
        let plan = export_plan(&f.ctx, request(&p, &ranged())).unwrap();
        let err = apply_plan(
            &f.ctx,
            DesignerApplyPlanRequest {
                plan_id: plan.plan_id,
                request: request(&p, &ranged()),
                backup: true,
                dry_apply: true,
            },
            &NoopProgress,
            &CancelToken::new(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "designer.apply-failed", "{name}");
        assert_eq!(std::fs::read(path.as_std_path()).unwrap(), bytes, "{name}");
    }
}

#[test]
fn defs_files_with_another_root_or_two_roots_or_trailing_text_are_left_alone() {
    let f = fixture(true);
    let p = project(&f, &[]);
    for (name, text) in [
        ("other root", "<Patch></Patch>"),
        ("two roots", "<Defs><ThingDef/></Defs><Defs/>"),
        ("trailing text", "<Defs><ThingDef/></Defs>stray"),
        ("truncated", "<Defs><ThingDef><defName>RS_X</defName>"),
        ("empty file", ""),
    ] {
        common_project::put(&p.root, "Defs/Weapons/RS_NewRifle.xml", text);
        let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
        if name == "empty file" {
            // an empty file holds nothing to keep: the design is written fresh, which is the whole file
            continue;
        }
        assert!(
            prepared.plan.has_errors(),
            "{name}: {:?}",
            prepared.plan.diagnostics
        );
        assert!(
            prepared.plan.file("Defs/Weapons/RS_NewRifle.xml").is_none(),
            "{name}"
        );
    }
}

#[test]
fn a_load_folders_with_bom_crlf_and_comments_keeps_its_style_when_the_gate_is_added() {
    let f = fixture(true);
    let existing = "\u{FEFF}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<!-- hand written -->\r\n<loadFolders>\r\n\t<!-- old -->\r\n\t<v1.5>\r\n\t\t<li>/</li>\r\n\t</v1.5>\r\n</loadFolders>\r\n";
    let p = project(&f, &[("LoadFolders.xml", existing)]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged_ce())).unwrap();
    assert!(
        !prepared.plan.has_errors(),
        "{:?}",
        prepared.plan.diagnostics
    );
    let lf = prepared.plan.file("LoadFolders.xml").unwrap();
    assert!(lf.text.starts_with('\u{FEFF}'), "the BOM stays");
    assert!(lf.text.contains("<!-- hand written -->\r\n"));
    assert!(lf.text.contains("<!-- old -->"));
    assert!(
        !lf.text.replace("\r\n", "").contains('\n'),
        "line endings stay CRLF: {:?}",
        lf.text
    );
    assert!(lf.text.contains("ceteam.combatextended"));
    // the old block is untouched, byte for byte
    assert!(
        lf.text
            .contains("\t<v1.5>\r\n\t\t<li>/</li>\r\n\t</v1.5>\r\n")
    );
}

#[test]
fn hand_written_operations_next_to_a_generated_section_survive_a_second_apply() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id.clone(),
            request: req.clone(),
            backup: true,
            dry_apply: false,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let patch_path = plan
        .files
        .iter()
        .find(|x| x.path.contains("CE/"))
        .unwrap()
        .path
        .clone();
    let text = std::fs::read_to_string(p.root.join(&patch_path).as_std_path()).unwrap();
    // the person appends an operation of their own right after the generated section, without a comment
    let mine = "\t<Operation Class=\"PatchOperationRemove\"><xpath>Defs/ThingDef[defName=\"RS_Mine\"]/comps</xpath></Operation>\n";
    let at = text.rfind("</Patch>").unwrap();
    let edited = format!("{}{mine}{}", &text[..at], &text[at..]);
    std::fs::write(p.root.join(&patch_path).as_std_path(), &edited).unwrap();
    // the design changes a Combat Extended number, so the section is regenerated
    let mut spec = ranged_ce();
    if let Some(ce) = spec.ce.as_mut() {
        ce.bulk = Some(rimstudio_design::model::Sourced::typed(9.5));
    }
    let prepared = prepare(&f.ctx, &request(&p, &spec)).unwrap();
    let file = prepared.plan.file(&patch_path).unwrap();
    assert!(
        file.text.contains("RS_Mine"),
        "the hand written operation must survive a regenerated section:\n{}",
        file.text
    );
}

#[test]
fn a_regenerated_section_replaces_the_old_one_without_duplicating_operations() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id.clone(),
            request: req,
            backup: true,
            dry_apply: false,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let patch_path = plan
        .files
        .iter()
        .find(|x| x.path.contains("CE/"))
        .unwrap()
        .path
        .clone();
    let before = std::fs::read_to_string(p.root.join(&patch_path).as_std_path()).unwrap();
    let mut spec = ranged_ce();
    if let Some(ce) = spec.ce.as_mut() {
        ce.bulk = Some(rimstudio_design::model::Sourced::typed(9.5));
    }
    let prepared = prepare(&f.ctx, &request(&p, &spec)).unwrap();
    let after = &prepared.plan.file(&patch_path).unwrap().text;
    assert_ne!(*after, before, "the changed number is in the new text");
    assert_eq!(
        after.matches("<Operation").count(),
        before.matches("<Operation").count()
    );
    assert_eq!(
        after.matches("======").count(),
        before.matches("======").count()
    );
}
