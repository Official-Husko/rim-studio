//! A plan against files that already exist: the hand made parts stay byte for byte (IT-053).
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{fixture, project, ranged, request};
use rimstudio_design::plan::FileAction;
use rimstudio_toolkit::designer::plan::prepare;

const HAND_MADE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<!--   written by hand   -->\r\n<Defs>\r\n\t<!-- ====== RS_NewRifle ====== -->\r\n\t<ThingDef ParentName=\"RS_BaseGun\">\r\n\t\t<defName>RS_NewRifle</defName>\r\n\t\t<label>old label</label>\r\n\t</ThingDef>\r\n\r\n\r\n\t<!--   my own def, odd   spacing   -->\r\n\t<ThingDef>\r\n\t\t<defName   >RS_Mine</defName>\r\n\t</ThingDef>\r\n</Defs>\r\n";

#[test]
fn only_the_section_of_the_design_changes_and_everything_else_is_identical() {
    let f = fixture(true);
    let p = project(&f, &[("Defs/Weapons/RS_NewRifle.xml", HAND_MADE)]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    assert!(
        !prepared.plan.has_errors(),
        "{:?}",
        prepared.plan.diagnostics
    );
    let file = prepared.plan.file("Defs/Weapons/RS_NewRifle.xml").unwrap();
    assert_eq!(file.action, FileAction::UpdateRegion);
    let old = file.previous.as_deref().unwrap();
    assert_eq!(old, HAND_MADE);
    assert_eq!(file.edits.len(), 1);
    let edit = &file.edits[0];
    assert_eq!(
        &old[..edit.start],
        &file.text[..edit.start],
        "bytes before the span"
    );
    let tail = old.len() - edit.end;
    assert_eq!(
        &old[edit.end..],
        &file.text[file.text.len() - tail..],
        "bytes after the span"
    );
    // the hand made parts are all still there, unchanged
    for keep in [
        "<!--   written by hand   -->\r\n",
        "<!--   my own def, odd   spacing   -->",
        "<defName   >RS_Mine</defName>",
    ] {
        assert!(file.text.contains(keep), "{keep}");
    }
    assert!(file.text.contains("<label>new rifle</label>"));
    assert!(!file.text.contains("old label"));
    assert!(
        !file.text.replace("\r\n", "").contains('\n'),
        "line endings stay CRLF"
    );
}

#[test]
fn a_file_that_cannot_be_parsed_is_reported_and_left_out_of_the_plan() {
    let f = fixture(true);
    let p = project(
        &f,
        &[("Defs/Weapons/RS_NewRifle.xml", "<Defs><ThingDef></Defs>")],
    );
    let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    assert!(prepared.plan.has_errors());
    assert!(
        prepared
            .plan
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "designer.merge-failed")
    );
    assert!(prepared.plan.file("Defs/Weapons/RS_NewRifle.xml").is_none());
}
