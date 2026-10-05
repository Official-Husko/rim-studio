//! Byte level behaviour of the span editor.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_xml::edit::SpanEditor;

const ABOUT: &str = "\u{FEFF}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<!-- authored by hand -->\r\n<ModMetaData>\r\n\t<name>  RS Test   Mod </name>\r\n\t<!-- the id -->\r\n\t<packageId   >rs.old.id</packageId>\r\n\t<supportedVersions>\r\n\t\t<li>1.5</li>\r\n\t</supportedVersions>\r\n\t<description>Line one &amp; two\r\n\r\n  <![CDATA[raw <b>]]></description>\r\n\t<odd   attr = 'x'   other=\"y\" />\r\n</ModMetaData>\r\n";

/// Asserts that the two texts agree everywhere outside `changed` (a byte range of `before`) and
/// that the part after the change is identical.
fn assert_only_changed(before: &str, after: &str, changed: std::ops::Range<usize>) {
    assert_eq!(&before[..changed.start], &after[..changed.start]);
    let tail = &before[changed.end..];
    assert!(after.ends_with(tail), "tail differs");
}

#[test]
fn replacing_package_id_text_touches_only_the_text() {
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.replace_text("/ModMetaData/packageId", "rs.new.id")
        .unwrap();
    let start = ABOUT.find("rs.old.id").unwrap();
    let after = ed.text();
    assert_only_changed(ABOUT, after, start..start + "rs.old.id".len());
    assert!(after.contains("<packageId   >rs.new.id</packageId>"));
    assert!(after.starts_with('\u{FEFF}'));
    assert!(after.contains("<!-- the id -->"));
    assert_eq!(
        after.len() + "rs.old.id".len(),
        ABOUT.len() + "rs.new.id".len()
    );
}

#[test]
fn every_byte_outside_the_span_is_preserved_for_each_edit_kind() {
    // replace_text
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.replace_text("/ModMetaData/name", "Renamed").unwrap();
    let s = ABOUT.find("  RS Test   Mod ").unwrap();
    assert_only_changed(ABOUT, ed.text(), s..s + "  RS Test   Mod ".len());

    // set_attr on an existing single quoted attribute keeps the quote character
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.set_attr("/ModMetaData/odd", "attr", "it's").unwrap();
    assert!(ed.text().contains("attr = 'it&apos;s'"));
    let s = ABOUT.find("attr = 'x'").unwrap() + "attr = '".len();
    assert_only_changed(ABOUT, ed.text(), s..s + 1);

    // remove an element on its own lines removes the lines
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.remove("/ModMetaData/supportedVersions").unwrap();
    assert!(!ed.text().contains("supportedVersions"));
    assert!(ed.text().contains("</packageId>\r\n\t<description>"));
}

#[test]
fn replace_text_escapes_and_uses_the_document_line_ending() {
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.replace_text("/ModMetaData/description", "a < b & c\nsecond")
        .unwrap();
    assert!(
        ed.text()
            .contains("<description>a &lt; b &amp; c\r\nsecond</description>")
    );
    assert_eq!(
        ed.element_text("/ModMetaData/description").unwrap(),
        "a < b & c\nsecond"
    );
}

#[test]
fn reading_text_resolves_entities_and_cdata() {
    let ed = SpanEditor::open(ABOUT).unwrap();
    assert_eq!(
        ed.element_text("/ModMetaData/description").unwrap(),
        "Line one & two\n\n  raw <b>"
    );
    let info = ed.element("/ModMetaData/odd").unwrap();
    assert_eq!(
        info.attrs,
        vec![
            ("attr".to_owned(), "x".to_owned()),
            ("other".to_owned(), "y".to_owned())
        ]
    );
}

#[test]
fn empty_element_becomes_a_pair_when_text_is_set() {
    let mut ed = SpanEditor::open("<a>\n  <b x=\"1\" />\n</a>\n").unwrap();
    ed.replace_text("/a/b", "v").unwrap();
    assert_eq!(ed.text(), "<a>\n  <b x=\"1\">v</b>\n</a>\n");
}

#[test]
fn setting_text_on_an_element_with_children_is_refused() {
    let mut ed = SpanEditor::open("<a><b/></a>").unwrap();
    let err = ed.replace_text("/a", "x").unwrap_err();
    assert_eq!(err.code(), "xml.edit-unsupported");
}

#[test]
fn insert_child_matches_sibling_indentation() {
    let src =
        "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li>Common</li>\n  </v1.6>\n</loadFolders>\n";
    let mut ed = SpanEditor::open(src).unwrap();
    let li = NodeBuilder::new("li")
        .attr("IfModActive", "ceteam.combatextended")
        .text("CE")
        .build();
    ed.insert_child("/loadFolders/v1.6", &li).unwrap();
    assert_eq!(
        ed.text(),
        "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li>Common</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n  </v1.6>\n</loadFolders>\n"
    );
}

#[test]
fn insert_child_into_empty_and_self_closed_parents() {
    let mut ed = SpanEditor::open("<r>\n  <a/>\n  <b></b>\n  <c>\n  </c>\n</r>\n").unwrap();
    let x = Node::with_text("x", "1");
    ed.insert_child("/r/a", &x).unwrap();
    ed.insert_child("/r/b", &x).unwrap();
    ed.insert_child("/r/c", &x).unwrap();
    assert_eq!(
        ed.text(),
        "<r>\n  <a>\n    <x>1</x>\n  </a>\n  <b>\n    <x>1</x>\n  </b>\n  <c>\n    <x>1</x>\n  </c>\n</r>\n"
    );
}

#[test]
fn insert_before_and_after_and_at_index() {
    let mut ed = SpanEditor::open("<r>\n\t<a/>\n\t<c/>\n</r>").unwrap();
    ed.insert_after("/r/a", &Node::new("b")).unwrap();
    ed.insert_before("/r/a", &Node::new("z")).unwrap();
    ed.insert_child_at("/r", 99, &Node::new("end")).unwrap();
    ed.insert_child_at("/r", 0, &Node::new("first")).unwrap();
    assert_eq!(
        ed.text(),
        "<r>\n\t<first/>\n\t<z/>\n\t<a/>\n\t<b/>\n\t<c/>\n\t<end/>\n</r>"
    );
}

#[test]
fn inline_siblings_get_inline_insertions() {
    let mut ed = SpanEditor::open("<r><a/><b/></r>").unwrap();
    ed.insert_after("/r/a", &Node::new("x")).unwrap();
    assert_eq!(ed.text(), "<r><a/><x/><b/></r>");
}

#[test]
fn remove_inline_element_removes_only_its_span() {
    let mut ed = SpanEditor::open("<r><a/> <b/></r>").unwrap();
    ed.remove("/r/a").unwrap();
    assert_eq!(ed.text(), "<r> <b/></r>");
}

#[test]
fn nested_node_insertion_uses_document_unit() {
    let mut ed = SpanEditor::open("<r>\n    <a/>\n</r>\n").unwrap();
    let n = NodeBuilder::new("b")
        .elem("c", |c| c.text_elem("d", "v"))
        .build();
    ed.insert_after("/r/a", &n).unwrap();
    assert_eq!(
        ed.text(),
        "<r>\n    <a/>\n    <b>\n        <c>\n            <d>v</d>\n        </c>\n    </b>\n</r>\n"
    );
}

#[test]
fn replace_children_relays_the_list() {
    let src = "<d>\n  <activeMods>\n    <li>old</li>\n    <!-- gone -->\n  </activeMods>\n  <keep/>\n</d>\n";
    let mut ed = SpanEditor::open(src).unwrap();
    let items = [Node::with_text("li", "a"), Node::with_text("li", "b")];
    ed.replace_children("/d/activeMods", &items).unwrap();
    assert_eq!(
        ed.text(),
        "<d>\n  <activeMods>\n    <li>a</li>\n    <li>b</li>\n  </activeMods>\n  <keep/>\n</d>\n"
    );
    ed.replace_children("/d/activeMods", &[]).unwrap();
    assert!(ed.text().contains("<activeMods></activeMods>"));
}

#[test]
fn replace_element_swaps_one_element() {
    let mut ed = SpanEditor::open("<r>\n  <a>1</a>\n  <b/>\n</r>\n").unwrap();
    ed.replace_element("/r/a", &Node::with_text("a", "2"))
        .unwrap();
    assert_eq!(ed.text(), "<r>\n  <a>2</a>\n  <b/>\n</r>\n");
}

#[test]
fn attributes_are_added_and_removed_in_place() {
    let mut ed = SpanEditor::open("<a x=\"1\"  y=\"2\"/>").unwrap();
    ed.set_attr("/a", "z", "3&").unwrap();
    assert_eq!(ed.text(), "<a x=\"1\"  y=\"2\" z=\"3&amp;\"/>");
    ed.remove_attr("/a", "y").unwrap();
    assert_eq!(ed.text(), "<a x=\"1\" z=\"3&amp;\"/>");
    ed.set_attr("/a", "x", "9").unwrap();
    assert_eq!(ed.attr("/a", "x").unwrap().as_deref(), Some("9"));
    let mut bare = SpanEditor::open("<a/>").unwrap();
    bare.set_attr("/a", "k", "v").unwrap();
    assert_eq!(bare.text(), "<a k=\"v\"/>");
    assert!(bare.set_attr("/a", "bad name", "v").is_err());
}

#[test]
fn comments_that_look_like_markup_are_not_elements() {
    let src = "<r><!-- <a>fake</a> --><a>real</a></r>";
    let mut ed = SpanEditor::open(src).unwrap();
    ed.replace_text("/r/a", "ok").unwrap();
    assert_eq!(ed.text(), "<r><!-- <a>fake</a> --><a>ok</a></r>");
}

#[test]
fn paths_support_positions_wildcards_and_errors() {
    let ed = SpanEditor::open("<r><li>1</li><x/><li>2</li><li>3</li></r>").unwrap();
    assert_eq!(ed.element_text("/r/li[2]").unwrap(), "2");
    assert_eq!(ed.element_text("/r/li[3]").unwrap(), "3");
    assert_eq!(ed.element_text("/r/*[2]").unwrap(), "");
    assert_eq!(ed.element("/r/*[2]").unwrap().tag, "x");
    assert_eq!(ed.element_text("r/li").unwrap(), "1");
    assert_eq!(
        ed.element_text("/r/li[4]").unwrap_err().code(),
        "xml.edit-path-missing"
    );
    assert_eq!(
        ed.element_text("/other").unwrap_err().code(),
        "xml.edit-path-missing"
    );
    assert_eq!(
        ed.element_text("/r/li[0]").unwrap_err().code(),
        "xml.edit-path-invalid"
    );
    assert_eq!(
        ed.element_text("/r//li").unwrap_err().code(),
        "xml.edit-path-invalid"
    );
    assert_eq!(
        ed.element_text("/r/li[x]").unwrap_err().code(),
        "xml.edit-path-invalid"
    );
    let kids = ed.children("/r").unwrap();
    assert_eq!(kids.len(), 4);
    assert_eq!(kids[3].path, "/r/li[3]");
    assert_eq!(kids[2].nth, 2);
}

#[test]
fn case_insensitive_matching_is_opt_in() {
    let ed = SpanEditor::open("<LoadFolders><V1.6/></LoadFolders>").unwrap();
    assert!(!ed.exists("/loadfolders/v1.6"));
    let ed = ed.case_insensitive(true);
    assert!(ed.exists("/loadfolders/v1.6"));
}

#[test]
fn malformed_documents_are_rejected_on_open() {
    for bad in ["<a><b></a>", "<a>", "", "<a/><b/>", "text", "<a></a>x"] {
        assert!(SpanEditor::open(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn documents_with_odd_but_tolerated_content_open() {
    let src = "<!DOCTYPE a><a>x &nbsp; & y<![CDATA[<>]]><?pi?></a>";
    let mut ed = SpanEditor::open(src).unwrap();
    ed.set_attr("/a", "k", "v").unwrap();
    assert!(ed.text().starts_with("<!DOCTYPE a><a k=\"v\">"));
}

#[test]
fn eol_and_indent_detection() {
    let ed = SpanEditor::open(ABOUT).unwrap();
    assert_eq!(ed.eol(), "\r\n");
    assert_eq!(ed.indent_unit(), "\t");
    let ed = SpanEditor::open("<a>\n    <b/>\n</a>").unwrap();
    assert_eq!(ed.eol(), "\n");
    assert_eq!(ed.indent_unit(), "    ");
    assert_eq!(SpanEditor::open("<a><b/></a>").unwrap().indent_unit(), "  ");
}

#[test]
fn edits_keep_the_document_well_formed_and_idempotent_replacements_do_not_change_text() {
    let mut ed = SpanEditor::open(ABOUT).unwrap();
    ed.replace_text("/ModMetaData/packageId", "rs.old.id")
        .unwrap();
    assert_eq!(ed.text(), ABOUT);
}

#[test]
fn a_failed_edit_leaves_the_document_untouched() {
    let mut ed = SpanEditor::open("<a><b/></a>").unwrap();
    let before = ed.text().to_owned();
    let bad = Node::new("not valid");
    assert!(ed.insert_child("/a", &bad).is_err());
    assert!(ed.remove("/a").is_err());
    assert_eq!(ed.text(), before);
}
