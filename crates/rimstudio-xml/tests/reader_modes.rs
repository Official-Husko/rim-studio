//! Game mode and tolerant mode behaviour of the document reader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::{Child, Node};
use rimstudio_xml::{ParseMode, XmlError, parse_document};
use rstest::rstest;

fn game(text: &str) -> Result<Node, XmlError> {
    parse_document(text.as_bytes(), ParseMode::Game).map(|d| d.root)
}

fn tolerant(text: &str) -> (Node, Vec<String>) {
    let doc = parse_document(text.as_bytes(), ParseMode::Tolerant).unwrap();
    let codes = doc
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_owned())
        .collect();
    (doc.root, codes)
}

#[test]
fn utf8_bom_is_stripped() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"<?xml version=\"1.0\"?><Defs><a>x</a></Defs>");
    let doc = parse_document(&bytes, ParseMode::Game).unwrap();
    assert_eq!(doc.root.tag, "Defs");
    assert!(doc.diagnostics.is_empty());
}

#[test]
fn utf16_input_fails_like_the_game_because_only_the_utf8_bom_is_removed() {
    let mut bytes = vec![0xFF, 0xFE];
    for u in "<Defs/>".encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let err = parse_document(&bytes, ParseMode::Game).unwrap_err();
    assert_eq!(err.code(), "xml.parse");
}

#[test]
fn declared_encoding_is_ignored_and_bytes_are_utf8() {
    let root =
        game("<?xml version=\"1.0\" encoding=\"utf-16\"?><Defs><a>h\u{e9}llo</a></Defs>").unwrap();
    assert_eq!(root.child_text("a"), Some("h\u{e9}llo"));
}

#[test]
fn invalid_utf8_is_replaced_and_reported() {
    let bytes = b"<Defs><a>caf\xe9</a></Defs>";
    let doc = parse_document(bytes, ParseMode::Game).unwrap();
    assert_eq!(doc.root.child_text("a"), Some("caf\u{FFFD}"));
    assert_eq!(doc.diagnostics[0].code.as_str(), "xml.invalid-utf8");
}

#[test]
fn comments_and_whitespace_only_text_are_dropped() {
    let root = game("<Defs>\n  <!-- note -->\n  <a> </a>\n  <b>  x  </b>\n</Defs>").unwrap();
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.child("a").unwrap().children.len(), 0);
    assert_eq!(root.child_text("b"), Some("  x  "));
}

#[test]
fn whitespace_is_kept_below_xml_space_preserve_and_reset_by_default() {
    let root =
        game("<r xml:space=\"preserve\"> <a> </a><b xml:space=\"default\"> <c/> </b></r>").unwrap();
    assert_eq!(root.children[0], Child::Text(" ".into()));
    let a = root.child("a").unwrap();
    assert_eq!(a.children, vec![Child::Text(" ".into())]);
    let b = root.child("b").unwrap();
    assert_eq!(b.children.len(), 1);
}

#[test]
fn cdata_and_entities_merge_into_one_text_node() {
    let root = game("<a>x &amp; <![CDATA[<y>]]> &#65;&#x42;&lt;</a>").unwrap();
    assert_eq!(root.children, vec![Child::Text("x & <y> AB<".into())]);
}

#[test]
fn whitespace_only_cdata_is_kept() {
    let root = game("<a><![CDATA[ ]]></a>").unwrap();
    assert_eq!(root.children, vec![Child::Text(" ".into())]);
}

#[test]
fn line_endings_in_text_are_normalised_and_in_attributes_become_spaces() {
    let root = game("<a k=\"x\r\ny\tz\">l1\r\nl2\rl3</a>").unwrap();
    assert_eq!(root.attr("k"), Some("x y z"));
    assert_eq!(root.leaf_text(), Some("l1\nl2\nl3"));
}

#[test]
fn character_references_to_control_characters_pass() {
    let root = game("<a>&#1;&#x7;</a>").unwrap();
    assert_eq!(root.leaf_text(), Some("\u{1}\u{7}"));
}

#[test]
fn raw_control_characters_pass_because_character_checking_is_off() {
    let root = game("<a>x\u{1}y</a>").unwrap();
    assert_eq!(root.leaf_text(), Some("x\u{1}y"));
}

#[test]
fn doctype_is_rejected_in_game_mode_and_ignored_when_tolerant() {
    let text = "<!DOCTYPE Defs [<!ENTITY e \"x\">]><Defs><a>&e;</a></Defs>";
    assert_eq!(game(text).unwrap_err().code(), "xml.dtd-rejected");
    let (root, codes) = tolerant(text);
    assert_eq!(root.tag, "Defs");
    assert!(codes.contains(&"xml.dtd-ignored".to_owned()));
}

#[rstest]
#[case("<a>&nbsp;</a>")]
#[case("<a>1 & 2</a>")]
#[case("<a>1 < 2</a>")]
#[case("<a><b></a>")]
#[case("<a></b>")]
#[case("<a/><b/>")]
#[case("<a k=\"1\" k=\"2\"/>")]
#[case("<a>")]
#[case("")]
#[case("   ")]
#[case("<a>x</a>trailing")]
#[case("\n<?xml version=\"1.0\"?><a/>")]
#[case("<p:a/>")]
#[case("<1a/>")]
#[case("<a k=v/>")]
#[case("<a><!-- open")]
fn malformed_input_fails_in_game_mode(#[case] text: &str) {
    assert!(game(text).is_err(), "{text:?} should fail");
}

#[rstest]
#[case("<?xml version=\"1.0\"?><a/>")]
#[case("  <a/>")]
#[case("<a/><!-- trailing comment -->\n")]
#[case("<r xmlns:p=\"urn:x\"><p:a p:k=\"1\"/></r>")]
#[case("<a xml:space=\"preserve\"/>")]
fn well_formed_oddities_pass_in_game_mode(#[case] text: &str) {
    assert!(game(text).is_ok(), "{text:?} should parse");
}

#[test]
fn parse_errors_carry_line_and_column() {
    let err = game("<a>\n  <b>\n  </c>\n</a>").unwrap_err();
    match err {
        XmlError::Parse { line, column, .. } => {
            assert_eq!(line, 3);
            assert!(column >= 3);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn tolerant_mode_recovers_a_mismatched_end_tag() {
    let (root, codes) = tolerant("<Defs><a><b>x</a><c>y</c></Defs>");
    assert_eq!(root.child("a").unwrap().child_text("b"), Some("x"));
    assert_eq!(root.child_text("c"), Some("y"));
    assert!(codes.contains(&"xml.mismatched-end".to_owned()));
}

#[test]
fn tolerant_mode_keeps_undefined_entities_and_dangling_ampersands() {
    let (root, codes) = tolerant("<a>x&nbsp;y & z</a>");
    assert_eq!(root.leaf_text(), Some("x&nbsp;y & z"));
    assert!(codes.contains(&"xml.undefined-entity".to_owned()));
}

#[test]
fn tolerant_mode_closes_truncated_documents() {
    let (root, codes) = tolerant("<Defs><ThingDef><defName>RS_A</defName><label>cut");
    let def = root.child("ThingDef").unwrap();
    assert_eq!(def.child_text("defName"), Some("RS_A"));
    assert_eq!(def.child_text("label"), Some("cut"));
    assert!(codes.contains(&"xml.unclosed".to_owned()));
}

#[test]
fn tolerant_mode_keeps_the_first_root_and_skips_the_rest() {
    let (root, codes) = tolerant("<a><x/></a><b><y/></b>");
    assert_eq!(root.tag, "a");
    assert!(codes.contains(&"xml.extra-root".to_owned()));
}

#[test]
fn tolerant_mode_stops_at_a_syntax_error_but_keeps_the_tree() {
    let (root, codes) = tolerant("<Defs><a>1</a><b k=\"v\"<c/></Defs>");
    assert_eq!(root.child_text("a"), Some("1"));
    assert!(!codes.is_empty());
}

#[test]
fn tolerant_mode_skips_duplicate_attributes() {
    let (root, codes) = tolerant("<a k=\"1\" k=\"2\" j=\"3\"/>");
    assert_eq!(root.attr("k"), Some("1"));
    assert_eq!(root.attr("j"), Some("3"));
    assert!(codes.contains(&"xml.bad-attribute".to_owned()));
}

#[test]
fn tolerant_mode_without_any_element_still_fails() {
    let err = parse_document(b"just text", ParseMode::Tolerant).unwrap_err();
    assert!(matches!(err.code(), "xml.parse" | "xml.no-root"));
    assert_eq!(
        parse_document(b"", ParseMode::Tolerant).unwrap_err().code(),
        "xml.no-root"
    );
}

#[test]
fn deep_nesting_beyond_the_limit_is_an_error_not_a_crash() {
    let depth = 2000;
    let text = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    assert_eq!(game(&text).unwrap_err().code(), "xml.too-deep");
}

#[test]
fn attribute_order_is_kept() {
    let root = game("<a z=\"1\" b=\"2\" m=\"3\"/>").unwrap();
    let keys: Vec<&str> = root.attrs.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["z", "b", "m"]);
}

#[test]
fn processing_instructions_are_dropped() {
    let root = game("<a><?pi data?>x</a>").unwrap();
    assert_eq!(root.leaf_text(), Some("x"));
}
