//! Hostile and random input never panics and never produces inconsistent results.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::tree::Node;
use rimstudio_xml::about::read_lenient;
use rimstudio_xml::defs_scan::{index_file, index_file_detailed};
use rimstudio_xml::edit::SpanEditor;
use rimstudio_xml::load_folders;
use rimstudio_xml::mods_config;
use rimstudio_xml::patches::parse_patch_file_with;
use rimstudio_xml::render::{RenderOpts, render};
use rimstudio_xml::save_meta::read_meta;
use rimstudio_xml::{ParseMode, parse_document};

/// Text made of markup fragments, so that many inputs are almost well formed.
fn markup_soup() -> impl Strategy<Value = String> {
    let token = prop::sample::select(vec![
        "<",
        ">",
        "</",
        "/>",
        "<a>",
        "</a>",
        "<b x=\"1\">",
        "</b>",
        "<c/>",
        "&amp;",
        "&",
        ";",
        "&#1;",
        "&#x110000;",
        "&bogus;",
        "\"",
        "'",
        "=",
        " ",
        "\n",
        "\r\n",
        "<!--",
        "-->",
        "<![CDATA[",
        "]]>",
        "<?xml version=\"1.0\"?>",
        "<?pi?>",
        "<!DOCTYPE x>",
        "text",
        "\u{e9}",
        "\u{FEFF}",
        "xml:space=\"preserve\"",
        "<p:q>",
        "xmlns:p=\"u\"",
        "\u{0}",
        "\u{1}",
    ]);
    prop::collection::vec(token, 0..40).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn readers_never_panic_on_random_bytes(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
        for mode in [ParseMode::Game, ParseMode::Tolerant] {
            let _ = parse_document(&bytes, mode);
        }
        let _ = index_file(&bytes);
        let _ = read_lenient(&bytes);
        let _ = load_folders::read(&bytes);
        let _ = mods_config::read(&bytes);
        let _ = parse_patch_file_with(&bytes, ParseMode::Tolerant);
        let _ = read_meta(std::io::Cursor::new(bytes));
    }

    #[test]
    fn readers_never_panic_on_markup_soup(text in markup_soup()) {
        let game = parse_document(text.as_bytes(), ParseMode::Game);
        let tolerant = parse_document(text.as_bytes(), ParseMode::Tolerant);
        // Whatever the game accepts, the tolerant mode accepts with the same tree and no more
        // than warnings.
        if let Ok(g) = &game {
            let t = tolerant.as_ref().unwrap();
            prop_assert_eq!(&g.root, &t.root);
        }
        let idx = index_file_detailed(text.as_bytes());
        if game.is_ok() {
            prop_assert!(idx.error.is_none(), "index failed on a document the game accepts: {:?}", idx.error);
        }
        let _ = read_lenient(text.as_bytes());
        let _ = load_folders::read(text.as_bytes());
        if let Ok(mut ed) = SpanEditor::open(text.clone()) {
            let root = format!("/{}", ed.root_tag());
            let _ = ed.set_attr(&root, "k", "v\"'<&");
            let _ = ed.replace_text(&root, "t");
            let _ = ed.insert_child(&root, &Node::with_text("n", "x"));
            let _ = ed.insert_child_at(&root, 0, &Node::new("first"));
            let _ = ed.replace_children(&root, &[Node::new("only")]);
            let _ = ed.remove(&format!("{root}/*"));
            let _ = ed.remove_attr(&root, "k");
            // After any sequence of edits the document still indexes.
            prop_assert!(SpanEditor::open(ed.text().to_owned()).is_ok());
        }
    }

    #[test]
    fn game_mode_trees_survive_render_and_parse(text in markup_soup()) {
        if let Ok(doc) = parse_document(text.as_bytes(), ParseMode::Game) {
            // Whitespace only text under xml:space is the only thing that cannot be re-indented, so
            // compare after rendering without indentation.
            let opts = RenderOpts { indent: rimstudio_xml::render::Indent::None, declaration: false, ..RenderOpts::default() };
            let again = parse_document(render(&doc.root, &opts).as_bytes(), ParseMode::Game).unwrap();
            prop_assert_eq!(again.root, doc.root);
        }
    }
}

const BASES: &[&str] = &[
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Defs>\n  <ThingDef Name=\"RS_A\" Abstract=\"True\">\n    <label>a &amp; b</label>\n  </ThingDef>\n  <ThingDef ParentName=\"RS_A\"><defName>RS_B</defName><x><![CDATA[<y>]]></x></ThingDef>\n</Defs>\n",
    "<ModMetaData>\n  <name>N</name>\n  <packageId>rs.a</packageId>\n  <supportedVersions><li>1.6</li></supportedVersions>\n  <!-- c -->\n  <description>d</description>\n</ModMetaData>",
    "<loadFolders><v1.6><li>/</li><li IfModActive=\"a,b\">CE</li></v1.6></loadFolders>",
];

fn mutated() -> impl Strategy<Value = String> {
    let token = prop::sample::select(vec![
        "<", ">", "&", "\"", "</x>", "<!--", "]]>", "=", "/", "\u{e9}", "<z", "&bad;",
    ]);
    (
        prop::sample::select(BASES.to_vec()),
        0usize..400,
        0usize..12,
        prop::option::of(token),
    )
        .prop_map(|(base, at, cut, insert)| {
            let mut s = base.to_owned();
            let mut i = at.min(s.len());
            while !s.is_char_boundary(i) {
                i -= 1;
            }
            let mut j = (i + cut).min(s.len());
            while !s.is_char_boundary(j) {
                j += 1;
            }
            s.replace_range(i..j, insert.unwrap_or(""));
            s
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    #[test]
    fn slightly_damaged_documents_behave(text in mutated()) {
        let game = parse_document(text.as_bytes(), ParseMode::Game);
        let tolerant = parse_document(text.as_bytes(), ParseMode::Tolerant);
        if let Ok(g) = &game {
            prop_assert_eq!(&g.root, &tolerant.as_ref().unwrap().root);
            prop_assert!(index_file_detailed(text.as_bytes()).error.is_none());
        }
        let read = read_lenient(text.as_bytes());
        prop_assert_eq!(read.parsed, tolerant.is_ok());
        let _ = index_file(text.as_bytes());
        let _ = load_folders::read(text.as_bytes());
        if let Ok(mut ed) = SpanEditor::open(text.clone()) {
            let root = format!("/{}", ed.root_tag());
            if ed.replace_text(&format!("{root}/name"), "X").is_ok() {
                prop_assert!(SpanEditor::open(ed.text().to_owned()).is_ok());
            }
            if ed.insert_child(&root, &Node::with_text("added", "1")).is_ok() {
                let added = format!("{root}/added");
                prop_assert!(ed.exists(&added));
            }
        }
    }
}
