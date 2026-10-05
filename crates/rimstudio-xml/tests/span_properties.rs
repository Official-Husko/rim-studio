//! Property tests: edits never disturb bytes outside the changed span.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::tree::Node;
use rimstudio_xml::edit::SpanEditor;

fn ws() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "", " ", "\n", "\r\n", "\n\t", "\n    ", "  \n  ", "\t",
    ])
    .prop_map(str::to_owned)
}

fn comment() -> impl Strategy<Value = String> {
    prop::option::of("[a-z <>/]{0,10}")
        .prop_map(|c| c.map(|c| format!("<!--{c}-->")).unwrap_or_default())
}

fn body() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,]{0,10}"
}

/// A document: root with `n` leaf children separated by random white space and comments.
fn document() -> impl Strategy<Value = (String, usize)> {
    (
        prop::collection::vec((ws(), comment(), body(), prop::bool::ANY, ws()), 1..7),
        ws(),
        prop::bool::ANY,
    )
        .prop_map(|(kids, tail, bom)| {
            let mut s = String::new();
            if bom {
                s.push('\u{FEFF}');
            }
            s.push_str("<?xml version=\"1.0\"?>\n<root a='1'>");
            for (i, (lead, c, b, empty, trail)) in kids.iter().enumerate() {
                s.push_str(lead);
                s.push_str(c);
                if *empty {
                    s.push_str(&format!("<k{i} x = \"v\" />"));
                } else {
                    s.push_str(&format!("<k{i}  y='1'>{b}</k{i}>"));
                }
                s.push_str(trail);
            }
            s.push_str(&tail);
            s.push_str("</root>\n");
            (s, kids.len())
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn replace_text_changes_only_the_element_content((doc, n) in document(), pick in 0usize..7, new in "[a-z&<]{0,8}") {
        let i = pick % n;
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        let path = format!("/root/k{i}");
        let before_info = ed.element(&path).unwrap();
        ed.replace_text(&path, &new).unwrap();
        let after = ed.text().to_owned();
        // Everything before the element and after it is untouched.
        prop_assert_eq!(&after[..before_info.span.start], &doc[..before_info.span.start]);
        let tail = &doc[before_info.span.end..];
        prop_assert!(after.ends_with(tail));
        // The text reads back.
        prop_assert_eq!(ed.element_text(&path).unwrap(), new.clone());
        // Every other child is byte identical.
        for j in (0..n).filter(|j| *j != i) {
            let p = format!("/root/k{j}");
            let (a, b) = (SpanEditor::open(doc.clone()).unwrap().element(&p).unwrap(), ed.element(&p).unwrap());
            prop_assert_eq!(a.attrs, b.attrs);
            prop_assert_eq!(a.text, b.text);
        }
    }

    #[test]
    fn insert_after_only_adds_bytes_next_to_the_target((doc, n) in document(), pick in 0usize..7) {
        let i = pick % n;
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        let path = format!("/root/k{i}");
        let span = ed.element(&path).unwrap().span;
        ed.insert_after(&path, &Node::with_text("new", "v")).unwrap();
        let after = ed.text();
        prop_assert_eq!(&after[..span.end], &doc[..span.end]);
        prop_assert!(after.ends_with(&doc[span.end..]));
        prop_assert_eq!(ed.children("/root").unwrap().len(), n + 1);
        prop_assert_eq!(ed.element_text("/root/new").unwrap(), "v");
    }

    #[test]
    fn remove_deletes_one_contiguous_span((doc, n) in document(), pick in 0usize..7) {
        let i = pick % n;
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        let path = format!("/root/k{i}");
        let span = ed.element(&path).unwrap().span;
        ed.remove(&path).unwrap();
        let after = ed.text().to_owned();
        let removed = doc.len() - after.len();
        prop_assert!(removed >= span.end - span.start);
        // The removed bytes form one contiguous block of the original.
        let k = doc.bytes().zip(after.bytes()).take_while(|(a, b)| a == b).count().min(doc.len() - removed);
        prop_assert_eq!(format!("{}{}", &doc[..k], &doc[k + removed..]), after);
    }

    #[test]
    fn set_attr_changes_only_attribute_bytes((doc, _n) in document(), value in "[a-z&\"<]{0,6}") {
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        ed.set_attr("/root", "a", &value).unwrap();
        prop_assert_eq!(ed.attr("/root", "a").unwrap(), Some(value));
        let start = doc.find("<root").unwrap();
        prop_assert_eq!(&ed.text()[..start], &doc[..start]);
        let close = doc.rfind("</root>").unwrap();
        prop_assert!(ed.text().ends_with(&doc[close..]));
    }
}
