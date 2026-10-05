//! Properties of the byte span merge and the line diff.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use proptest::prelude::*;
use rimstudio_core::tree::NodeBuilder;
use rimstudio_design::plan::{FileKind, PlannedFile, SectionHeader};
use rimstudio_toolkit::shared::diff::unified_diff;
use rimstudio_toolkit::shared::merge::{changed_span, merge_into, render_new};

fn file(names: &[String], label: &str) -> PlannedFile {
    let mut b = NodeBuilder::new("Defs");
    for n in names {
        b = b.elem("ThingDef", |t| {
            t.text_elem("defName", n).text_elem("label", label)
        });
    }
    PlannedFile::new_file(
        "Defs/A.xml",
        FileKind::VanillaDefs,
        b.build(),
        names
            .iter()
            .enumerate()
            .map(|(i, n)| SectionHeader::banner(i, n))
            .collect(),
    )
}

fn names() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::btree_set("RS_[A-Z][a-z]{1,6}", 1..4)
        .prop_map(|s| s.into_iter().collect())
}

proptest! {
    #[test]
    fn merging_a_file_into_its_own_rendering_changes_nothing(names in names(), label in "[a-z ]{1,10}") {
        let f = file(&names, &label);
        let text = render_new(&f);
        prop_assert_eq!(merge_into(&text, &f).unwrap(), text);
    }

    #[test]
    fn merging_is_idempotent_and_keeps_every_earlier_section(
        first in names(),
        second in names(),
        label in "[a-z ]{1,10}",
    ) {
        let a = file(&first, "one");
        let b = file(&second, &label);
        let text = render_new(&a);
        let once = merge_into(&text, &b).unwrap();
        let twice = merge_into(&once, &b).unwrap();
        prop_assert_eq!(&once, &twice);
        for n in first.iter().chain(second.iter()) {
            prop_assert!(once.contains(&format!("<defName>{n}</defName>")), "{n} is missing");
        }
        prop_assert!(rimstudio_xml::edit::SpanEditor::open(once.as_str()).is_ok());
    }

    #[test]
    fn the_changed_span_turns_the_old_text_into_the_new_one(old in "\\PC{0,40}", new in "\\PC{0,40}") {
        let edits = changed_span(&old, &new);
        if old == new {
            prop_assert!(edits.is_empty());
        } else {
            prop_assert_eq!(edits.len(), 1);
            let e = &edits[0];
            let mut text = old.clone();
            text.replace_range(e.start..e.end, &e.replacement);
            prop_assert_eq!(text, new);
        }
    }

    #[test]
    fn the_diff_is_empty_exactly_when_the_lines_are_equal(
        a in proptest::collection::vec("[a-c]{0,3}", 0..8),
        b in proptest::collection::vec("[a-c]{0,3}", 0..8),
    ) {
        let (ta, tb) = (a.join("\n") + "\n", b.join("\n") + "\n");
        let d = unified_diff("f", &ta, &tb);
        prop_assert_eq!(d.is_empty(), ta == tb);
        prop_assert_eq!(d, unified_diff("f", &ta, &tb));
    }
}
