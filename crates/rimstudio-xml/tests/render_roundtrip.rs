//! Rendering a node tree and parsing it again gives the same tree.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::tree::{Child, Node, NodeBuilder};
use rimstudio_xml::render::{Indent, LineEnding, RenderOpts, render, render_defs_file};
use rimstudio_xml::{ParseMode, parse_document};

fn name() -> impl Strategy<Value = String> {
    "[A-Za-z_][A-Za-z0-9_.-]{0,8}"
}

fn attr_value() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 \t\n\r&<>\"'\u{e9}\u{1F600}\u{1}-]{0,12}"
}

fn text() -> impl Strategy<Value = String> {
    "[ \t\n\r]{0,3}[a-zA-Z0-9][a-zA-Z0-9 \t\n\r&<>\"'\u{e9}\u{1F600}\u{1}\\]-]{0,14}"
}

fn attrs() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((name(), attr_value()), 0..4).prop_map(|pairs| {
        let mut out: Vec<(String, String)> = Vec::new();
        for (k, v) in pairs {
            if !out.iter().any(|(ek, _)| *ek == k) {
                out.push((k, v));
            }
        }
        out
    })
}

fn node() -> impl Strategy<Value = Node> {
    let leaf = (name(), attrs(), prop::option::of(text())).prop_map(|(tag, attrs, t)| Node {
        tag,
        attrs,
        children: t.into_iter().map(Child::Text).collect(),
    });
    leaf.prop_recursive(4, 40, 5, |inner| {
        (
            name(),
            attrs(),
            prop::collection::vec((inner, prop::option::of(text())), 0..5),
            prop::option::of(text()),
        )
            .prop_map(|(tag, attrs, kids, lead)| {
                let mut children = Vec::new();
                if let Some(t) = lead {
                    children.push(Child::Text(t));
                }
                for (el, t) in kids {
                    children.push(Child::Element(el));
                    if let Some(t) = t {
                        children.push(Child::Text(t));
                    }
                }
                Node {
                    tag,
                    attrs,
                    children,
                }
            })
    })
}

fn opts() -> impl Strategy<Value = RenderOpts> {
    (
        prop::sample::select(vec![
            Indent::Spaces(2),
            Indent::Spaces(4),
            Indent::Tab,
            Indent::None,
        ]),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(indent, crlf, decl, bom, fin, close)| RenderOpts {
            indent,
            eol: if crlf {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            },
            declaration: decl,
            bom,
            final_newline: fin,
            self_close_empty: close,
            blank_line_between_top_level: false,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn render_then_parse_is_identity(n in node(), o in opts()) {
        let text = render(&n, &o);
        let back = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        prop_assert_eq!(back.root, n);
        prop_assert!(back.diagnostics.is_empty());
    }

    #[test]
    fn rendering_is_deterministic_and_tolerant_mode_agrees(n in node(), o in opts()) {
        let a = render(&n, &o);
        let b = render(&n.clone(), &o);
        prop_assert_eq!(&a, &b);
        let game = parse_document(a.as_bytes(), ParseMode::Game).unwrap();
        let tol = parse_document(a.as_bytes(), ParseMode::Tolerant).unwrap();
        prop_assert_eq!(game.root, tol.root);
    }

    #[test]
    fn defs_files_round_trip(nodes in prop::collection::vec(node(), 0..5), o in opts()) {
        let text = render_defs_file(&nodes, &o);
        let back = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        let tops: Vec<Node> = back.root.elements().cloned().collect();
        prop_assert_eq!(tops, nodes);
        prop_assert_eq!(back.root.tag, "Defs");
    }
}

#[test]
fn preserved_whitespace_survives_a_round_trip() {
    let n = NodeBuilder::new("r")
        .attr("xml:space", "preserve")
        .text(" ")
        .elem("a", |b| b.text("\n"))
        .text("  ")
        .build();
    for indent in [Indent::Spaces(2), Indent::None] {
        let text = render(
            &n,
            &RenderOpts {
                indent,
                ..RenderOpts::default()
            },
        );
        let back = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        assert_eq!(back.root, n);
    }
}
