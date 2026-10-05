//! Property and differential tests of the VDF reader.
//!
//! The reference parser is the dev-dependency `keyvalues-parser`. It rejects several things the
//! reader accepts on purpose (a byte order mark, `#include`, conditionals, an empty file), so the
//! differential tests only generate documents inside the common subset. Real Steam files are not
//! kept in the repository; documents are generated.

use std::collections::BTreeMap;

use keyvalues_parser::{Value as RefValue, Vdf as RefVdf};
use proptest::prelude::*;
use rimstudio_steam::vdf::{MacroKind, VdfDoc, VdfMacro, VdfObject, VdfValue, parse, render};

// ---------------------------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------------------------

/// Text for keys and values in the common subset: printable ASCII plus a few awkward characters,
/// escapes the printer writes and the reference understands (`\\`, `\"`, `\n`, `\t`).
fn common_text() -> BoxedStrategy<String> {
    proptest::collection::vec(
        prop_oneof![
            8 => proptest::char::range('a', 'z'),
            3 => proptest::char::range('0', '9'),
            2 => Just(' '),
            1 => Just('_'),
            1 => Just('/'),
            1 => Just('\\'),
            1 => Just('"'),
            1 => Just('\n'),
            1 => Just('\t'),
            1 => Just('{'),
            1 => Just('['),
            1 => Just('\u{e9}'),
        ],
        0..10,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .boxed()
}

/// Text for the round trip test: anything, including control characters.
fn any_text() -> BoxedStrategy<String> {
    proptest::collection::vec(any::<char>(), 0..8)
        .prop_map(|c| c.into_iter().collect())
        .boxed()
}

fn tree(text: BoxedStrategy<String>) -> impl Strategy<Value = VdfObject> {
    let leaf = text.clone().prop_map(VdfValue::Str);
    let value = leaf.prop_recursive(4, 40, 5, move |inner| {
        proptest::collection::vec((text.clone(), inner), 0..5).prop_map(|pairs| {
            let mut o = VdfObject::new();
            for (k, v) in pairs {
                o.push(k, v);
            }
            VdfValue::Obj(o)
        })
    });
    // The root holds one object pair, like every real Steam file.
    let key = Just("Root".to_owned());
    (key, value).prop_map(|(k, v)| {
        let mut o = VdfObject::new();
        let v = match v {
            VdfValue::Obj(_) => v,
            VdfValue::Str(_) => VdfValue::Obj(VdfObject::new()),
        };
        o.push(k, v);
        o
    })
}

// ---------------------------------------------------------------------------------------------
// Canonical form for the comparison
// ---------------------------------------------------------------------------------------------

fn canon_mine(object: &VdfObject, out: &mut String) {
    let mut grouped: BTreeMap<&str, Vec<&VdfValue>> = BTreeMap::new();
    for (k, v) in object.iter() {
        grouped.entry(k).or_default().push(v);
    }
    out.push('{');
    for (k, values) in grouped {
        for v in values {
            out.push_str(&format!("{k:?}:"));
            match v {
                VdfValue::Str(s) => out.push_str(&format!("{s:?}")),
                VdfValue::Obj(o) => canon_mine(o, out),
            }
            out.push(',');
        }
    }
    out.push('}');
}

fn canon_ref(value: &RefValue<'_>, out: &mut String) {
    match value {
        RefValue::Str(s) => out.push_str(&format!("{:?}", s.as_ref())),
        RefValue::Obj(obj) => {
            out.push('{');
            for (k, values) in obj.iter() {
                for v in values {
                    out.push_str(&format!("{:?}:", k.as_ref()));
                    canon_ref(v, out);
                    out.push(',');
                }
            }
            out.push('}');
        }
    }
}

/// Canonical text of the reference parse: the single top level pair as an object.
fn reference_canon(text: &str) -> Option<String> {
    let vdf = keyvalues_parser::parse(text).map(RefVdf::from).ok()?;
    let mut out = String::from("{");
    out.push_str(&format!("{:?}:", vdf.key.as_ref()));
    canon_ref(&vdf.value, &mut out);
    out.push_str(",}");
    Some(out)
}

fn mine_canon(text: &str) -> Option<String> {
    let doc = parse(text).ok()?;
    let mut out = String::new();
    canon_mine(&doc.root, &mut out);
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn parse_never_panics_on_arbitrary_text(text in any::<String>()) {
        let _ = parse(&text);
    }

    #[test]
    fn parse_never_panics_on_vdf_shaped_noise(
        parts in proptest::collection::vec(
            prop_oneof![
                Just("\""), Just("{"), Just("}"), Just("\\"), Just("//"), Just("\n"), Just(" "),
                Just("[$WIN32]"), Just("#base"), Just("#include"), Just("a"), Just("\u{feff}"),
                Just("\0"), Just("\u{e9}"),
            ],
            0..60,
        )
    ) {
        let text: String = parts.concat();
        let _ = parse(&text);
    }

    #[test]
    fn a_rendered_tree_parses_back_to_the_same_tree(
        root in tree(any_text()),
        macros in proptest::collection::vec((any::<bool>(), any_text()), 0..3),
    ) {
        let doc = VdfDoc {
            root,
            macros: macros
                .into_iter()
                .map(|(base, path)| VdfMacro {
                    kind: if base { MacroKind::Base } else { MacroKind::Include },
                    path,
                })
                .collect(),
        };
        let text = render(&doc);
        prop_assert_eq!(parse(&text).unwrap(), doc);
    }

    #[test]
    fn rendering_is_stable_under_a_second_round(root in tree(any_text())) {
        let doc = VdfDoc { root, macros: Vec::new() };
        let once = render(&doc);
        let twice = render(&parse(&once).unwrap());
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn reader_and_reference_parser_agree_on_generated_documents(root in tree(common_text())) {
        let doc = VdfDoc { root, macros: Vec::new() };
        let text = render(&doc);
        let reference = reference_canon(&text);
        prop_assert!(reference.is_some(), "reference rejected {text:?}");
        prop_assert_eq!(mine_canon(&text), reference, "{:?}", text);
    }
}

#[test]
fn reader_and_reference_agree_on_hand_written_documents() {
    let documents = [
        "\"A\" { \"k\" \"v\" }",
        "\"A\"\n{\n\t\"k\"\t\t\"v\"\n\t\"k\"\t\t\"w\"\n}\n",
        "A { k v }",
        "// comment\n\"A\" { \"o\" { \"x\" \"1\" } \"o\" { \"x\" \"2\" } }",
        "\"A\" { \"p\" \"D:\\\\Lib\" \"q\" \"a\\tb\\\"c\" }",
        "\"A\" {\r\n\"k\" \"v\"\r\n}\r\n",
        "\"A\" { \"\" \"\" }",
    ];
    for text in documents {
        let reference =
            reference_canon(text).unwrap_or_else(|| panic!("reference rejected {text:?}"));
        assert_eq!(
            mine_canon(text).as_deref(),
            Some(reference.as_str()),
            "{text:?}"
        );
    }
}

#[test]
fn the_reader_accepts_what_the_reference_rejects() {
    let lenient = [
        "\u{feff}\"A\" { \"k\" \"v\" }",
        "\"A\" { \"k\" \"v\" [$WIN32] }",
        "#base \"x.vdf\"\n\"A\" { }",
        "",
        "\"A\" { } \"B\" { }",
        "\"A\" { \"p\" \"C:\\Steam\\apps\" }",
    ];
    for text in lenient {
        assert!(parse(text).is_ok(), "{text:?}");
    }
}
