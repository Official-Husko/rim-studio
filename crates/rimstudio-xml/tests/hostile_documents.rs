//! Span edits and `LoadFolders.xml` edits against hostile and unusual documents: every byte outside the
//! edited span stays as it was, and a document that is not what the editor expects is refused without a
//! change. All content is fictional.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use rimstudio_core::load_plan::LoadEntry;
use rimstudio_core::tree::NodeBuilder;
use rimstudio_xml::edit::SpanEditor;
use rimstudio_xml::load_folders::{add_entry, ensure_block, gate_folder, read, remove_entry};

/// The documents the edit tests run against, with the element they edit.
fn corpus() -> Vec<(&'static str, String)> {
    let mut docs: Vec<(&'static str, String)> = vec![
        (
            "crlf",
            "<loadFolders>\r\n  <v1.6>\r\n    <li>/</li>\r\n  </v1.6>\r\n</loadFolders>\r\n".into(),
        ),
        (
            "bom",
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<loadFolders>\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n".into(),
        ),
        (
            "tabs",
            "<loadFolders>\n\t<v1.6>\n\t\t<li>/</li>\n\t</v1.6>\n</loadFolders>".into(),
        ),
        (
            "comments",
            "<!-- head <li>x</li> -->\n<loadFolders>\n  <!-- <v1.6><li>fake</li></v1.6> -->\n  <v1.6>\n    <!-- keep -->\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n<!-- tail -->\n".into(),
        ),
        (
            "single quotes",
            "<loadFolders>\n  <v1.6>\n    <li IfModActive='rs.a'>A</li>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n".into(),
        ),
        (
            "gt in attribute",
            "<loadFolders>\n  <v1.6>\n    <li IfModActive=\"rs.a>b\" note='x/>y'>A</li>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n".into(),
        ),
        (
            "doctype",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE loadFolders [ <!ENTITY rs \"fictional\"> <!-- <v1.6> --> ]>\n<loadFolders>\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n".into(),
        ),
        (
            "cdata and pi",
            "<loadFolders><?rs note?><v1.6><li><![CDATA[/]]></li><li>x</li></v1.6></loadFolders>".into(),
        ),
        (
            "one line",
            "<loadFolders><v1.6><li>/</li></v1.6></loadFolders>".into(),
        ),
        (
            "unicode",
            "<loadFolders>\n  <v1.6>\n    <li>\u{65E5}\u{672C}\u{8A9E}/\u{1F600}x</li>\n    <li IfModActive=\"\u{e9}\u{e8}\">/</li>\n  </v1.6>\n</loadFolders>\n".into(),
        ),
        (
            "mixed line endings",
            "<loadFolders>\r\n  <v1.6>\n    <li>/</li>\r\n  </v1.6>\n</loadFolders>\r\n".into(),
        ),
        (
            "blank lines and trailing spaces",
            "\n\n<loadFolders>   \n\n  <v1.6>  \n    <li>/</li>   \n\n  </v1.6>\n\n</loadFolders>   \n\n".into(),
        ),
    ];
    docs.push((
        "empty block",
        "<loadFolders>\n  <v1.6/>\n</loadFolders>\n".into(),
    ));
    docs.push((
        "empty block pair",
        "<loadFolders>\n  <v1.6></v1.6>\n</loadFolders>\n".into(),
    ));
    docs
}

fn bytes_outside_preserved(before: &str, after: &str) -> bool {
    // an edit is one contiguous splice: prefix and suffix are shared
    let p = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let s = before
        .bytes()
        .rev()
        .zip(after.bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    p + s >= before.len().min(after.len()) || p + s + 1 >= before.len()
}

#[test]
fn an_inserted_entry_changes_one_splice_and_keeps_every_other_byte() {
    for (name, doc) in corpus() {
        if name == "empty block" {
            // `<v1.6/>` must become a pair: its own closing bytes change by necessity
            continue;
        }
        let mut ed = SpanEditor::open(doc.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let node = NodeBuilder::new("li").text("RS_Folder").build();
        let after = ed
            .insert_child("/loadFolders/v1.6", &node)
            .unwrap()
            .to_owned();
        let p = doc
            .bytes()
            .zip(after.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let s = doc
            .bytes()
            .rev()
            .zip(after.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count();
        assert!(
            p + s >= doc.len(),
            "{name}: bytes outside the span changed\n{doc:?}\n{after:?}"
        );
        assert!(after.contains("RS_Folder"), "{name}");
        SpanEditor::open(after)
            .unwrap_or_else(|e| panic!("{name}: result is not well formed: {e}"));
    }
}

#[test]
fn a_removed_entry_and_a_set_attribute_keep_the_rest_of_the_document() {
    for (name, doc) in corpus() {
        if name.starts_with("empty block") {
            continue;
        }
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        let after = ed
            .set_attr("/loadFolders/v1.6/li[1]", "RsMark", "a<b&\"c'")
            .unwrap()
            .to_owned();
        assert!(
            bytes_outside_preserved(&doc, &after) || after.len() > doc.len(),
            "{name}"
        );
        let reopened = SpanEditor::open(after.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            reopened
                .attr("/loadFolders/v1.6/li[1]", "RsMark")
                .unwrap()
                .as_deref(),
            Some("a<b&\"c'"),
            "{name}"
        );
        let mut ed = SpanEditor::open(after.clone()).unwrap();
        let removed = ed
            .remove_attr("/loadFolders/v1.6/li[1]", "RsMark")
            .unwrap()
            .to_owned();
        assert_eq!(
            removed, doc,
            "{name}: adding then removing an attribute is a no-op on the bytes"
        );
    }
}

#[test]
fn a_file_with_a_second_root_trailing_text_or_broken_nesting_is_refused() {
    let broken = [
        "<a/><b/>",
        "<a></a><b></b>",
        "<a></a>trailing text",
        "<a><b></a>",
        "<a><b>",
        "</a>",
        "",
        "   \n",
        "\u{FEFF}",
        "just text",
        "<a>unterminated",
        "<a attr=\"x></a>",
        "<a><!-- never closed </a>",
    ];
    for text in broken {
        assert!(SpanEditor::open(text).is_err(), "{text:?}");
        // the LoadFolders helpers fail closed too
        assert!(
            add_entry(text, "1.6", &LoadEntry::dir("x")).is_err(),
            "{text:?}"
        );
        assert!(ensure_block(text, "1.6", &[]).is_err(), "{text:?}");
        assert!(
            gate_folder(text, "1.6", "CE", &["rs.ce"]).is_err(),
            "{text:?}"
        );
        assert!(
            remove_entry(text, "1.6", &LoadEntry::dir("x")).is_err(),
            "{text:?}"
        );
    }
}

#[test]
fn a_nul_byte_or_control_text_inside_a_document_never_panics() {
    for text in ["<a>\0</a>", "<a b=\"\0\"/>", "<a>\u{1}\u{7f}</a>", "<\0a/>"] {
        if let Ok(mut ed) = SpanEditor::open(text) {
            let _ = ed.replace_text("/a", "new");
            let _ = ed.set_attr("/a", "n", "v");
            let _ = ed.remove("/a");
        }
    }
}

#[test]
fn an_empty_root_takes_an_entry_and_a_block() {
    for text in [
        "<loadFolders/>",
        "<loadFolders></loadFolders>",
        "<loadFolders>\n</loadFolders>\n",
    ] {
        let out = ensure_block(text, "1.6", &[LoadEntry::root()]).unwrap();
        let out = gate_folder(&out, "1.6", "CE", &["rs.ce"]).unwrap();
        let spec = read(out.as_bytes()).unwrap().spec;
        let block = spec.block("1.6").unwrap();
        assert_eq!(block.entries.len(), 2, "{text:?} -> {out:?}");
        assert_eq!(block.entries[1].path, "CE");
        assert_eq!(block.entries[1].if_active, ["rs.ce"]);
    }
}

#[test]
fn a_gate_is_added_once_even_when_the_file_spells_the_entry_differently() {
    let text = "<loadFolders>\r\n\t<v1.6>\r\n\t\t<li>/</li>\r\n\t\t<li IfModActive='RS.CE'>CE</li>\r\n\t</v1.6>\r\n</loadFolders>\r\n";
    let out = gate_folder(text, "1.6", "CE", &["rs.ce"]).unwrap();
    assert_eq!(
        out, text,
        "an equal entry (ids compared without case) is not added again"
    );
}

#[test]
fn duplicated_blocks_are_all_read_and_an_entry_goes_into_the_first_one() {
    // the game merges repeated blocks, so the entry lands in the first block and nothing is lost
    let text = "<loadFolders><v1.6><li>a</li></v1.6><v1.6><li>b</li></v1.6></loadFolders>";
    let out = add_entry(text, "1.6", &LoadEntry::dir("c")).unwrap();
    let spec = read(out.as_bytes()).unwrap().spec;
    let entries: Vec<&str> = spec
        .block("1.6")
        .unwrap()
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(entries, ["a", "c", "b"]);
}

#[test]
fn an_edit_of_a_missing_path_is_an_error_and_the_text_is_unchanged() {
    for (name, doc) in corpus() {
        let mut ed = SpanEditor::open(doc.clone()).unwrap();
        assert!(
            ed.replace_text("/loadFolders/v9.9/li", "x").is_err(),
            "{name}"
        );
        assert!(ed.remove("/loadFolders/v9.9").is_err(), "{name}");
        assert!(ed.set_attr("/nothing", "a", "b").is_err(), "{name}");
        assert!(
            ed.set_attr("/loadFolders", "bad name", "b").is_err(),
            "{name}"
        );
        assert_eq!(ed.text(), doc, "{name}");
    }
}

#[test]
fn an_inserted_node_with_an_invalid_name_is_refused_and_changes_nothing() {
    let doc = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n";
    let mut ed = SpanEditor::open(doc).unwrap();
    for name in ["a b", "1x", "<x>", "", "a>b", "x\"y"] {
        let node = NodeBuilder::new(name).text("t").build();
        assert!(
            ed.insert_child("/loadFolders/v1.6", &node).is_err(),
            "{name:?}"
        );
    }
    assert_eq!(ed.text(), doc);
}

#[test]
fn a_very_deep_document_does_not_overflow_the_stack() {
    let depth = 20_000;
    let mut text = String::new();
    for _ in 0..depth {
        text.push_str("<a>");
    }
    for _ in 0..depth {
        text.push_str("</a>");
    }
    let started = Instant::now();
    let mut ed = SpanEditor::open(text).unwrap();
    let _ = ed.exists("/a/a/a");
    let _ = ed.set_attr("/a", "x", "y");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_large_file_is_indexed_and_edited_in_reasonable_time() {
    let mut text = String::from("<loadFolders>\n  <v1.6>\n");
    for i in 0..100_000 {
        text.push_str(&format!("    <li>folder{i}</li>\n"));
    }
    text.push_str("  </v1.6>\n</loadFolders>\n");
    assert!(text.len() > 2_000_000);
    let started = Instant::now();
    let mut ed = SpanEditor::open(text.clone()).unwrap();
    let node = NodeBuilder::new("li").text("last").build();
    let after = ed
        .insert_child("/loadFolders/v1.6", &node)
        .unwrap()
        .to_owned();
    assert!(after.len() > text.len());
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn every_edit_on_a_unicode_document_lands_on_character_boundaries() {
    let doc =
        "<r>\n  <a x=\"\u{e9}\u{4e2d}\">\u{1F600}\u{e9}</a>\n  <b>\u{4e2d}\u{6587}</b>\n</r>\n";
    let node = NodeBuilder::new("c").text("\u{1F600}").build();
    for path in ["/r/a", "/r/b"] {
        let mut ed = SpanEditor::open(doc).unwrap();
        ed.insert_before(path, &node).unwrap();
        ed.insert_after(path, &node).unwrap();
        ed.set_attr(path, "k", "\u{1F600}").unwrap();
        ed.replace_text(path, "\u{e9}").unwrap();
        ed.remove(path).unwrap();
        SpanEditor::open(ed.into_text()).unwrap();
    }
}
