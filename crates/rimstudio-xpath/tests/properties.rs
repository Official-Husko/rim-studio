//! Property tests: print and parse round trip, document order, parser robustness, and a
//! differential check of the evaluator's fast paths against its general path.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use proptest::prelude::*;
use rimstudio_core::tree::ArenaDoc;
use rimstudio_xpath::ast::{Axis, BinOp, Expr, Func, NodeTest, PathBase, PathExpr, Step};
use rimstudio_xpath::parser::parse;
use rimstudio_xpath::{XNode, XPath};

// ---------------------------------------------------------------- AST strategy

const NAMES: &[&str] = &[
    "a", "b", "li", "defName", "x-y", "v1.5", "div", "and", "or", "mod", "text", "node", "R_S",
];

const AXES: &[Axis] = &[
    Axis::Ancestor,
    Axis::AncestorOrSelf,
    Axis::Attribute,
    Axis::Child,
    Axis::Descendant,
    Axis::DescendantOrSelf,
    Axis::Following,
    Axis::FollowingSibling,
    Axis::Namespace,
    Axis::Parent,
    Axis::Preceding,
    Axis::PrecedingSibling,
    Axis::SelfAxis,
];

const FUNCS: &[Func] = &[
    Func::Last,
    Func::Position,
    Func::Count,
    Func::Id,
    Func::LocalName,
    Func::Name,
    Func::String,
    Func::Concat,
    Func::StartsWith,
    Func::Contains,
    Func::Substring,
    Func::StringLength,
    Func::NormalizeSpace,
    Func::Translate,
    Func::Boolean,
    Func::Not,
    Func::True,
    Func::Number,
    Func::Sum,
    Func::Round,
];

const OP_CLASSES: &[&[BinOp]] = &[
    &[BinOp::Or],
    &[BinOp::And],
    &[BinOp::Eq, BinOp::Neq],
    &[BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge],
    &[BinOp::Add, BinOp::Sub],
    &[BinOp::Mul, BinOp::Div, BinOp::Mod],
    &[BinOp::Union],
];

const LITERALS: &[&str] = &[
    "",
    "x",
    "RS_RifleA",
    "two words",
    "it's",
    "say \"hi\"",
    "1",
    "a/b[1]",
];

fn arb_test() -> impl Strategy<Value = NodeTest> {
    prop_oneof![
        4 => prop::sample::select(NAMES).prop_map(|n| NodeTest::Name { prefix: None, local: n.to_owned() }),
        1 => Just(NodeTest::Wildcard),
        1 => Just(NodeTest::Text),
        1 => Just(NodeTest::Comment),
        1 => Just(NodeTest::AnyNode),
        1 => Just(NodeTest::ProcessingInstruction(None)),
        1 => Just(NodeTest::ProcessingInstruction(Some("t".to_owned()))),
        1 => prop::sample::select(NAMES).prop_map(|n| NodeTest::Name { prefix: Some("p".to_owned()), local: n.to_owned() }),
        1 => Just(NodeTest::PrefixWildcard("p".to_owned())),
    ]
}

fn arb_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        prop::sample::select(LITERALS).prop_map(|s| Expr::Literal(s.to_owned())),
        (0u32..100_000).prop_map(|n| Expr::Number(f64::from(n) / 16.0)),
        prop::sample::select(&["v", "w1"][..]).prop_map(|n| Expr::Variable(n.to_owned())),
        prop::sample::select(NAMES).prop_map(|n| Expr::Path(PathExpr {
            base: PathBase::Context,
            steps: vec![Step::child(n)],
        })),
    ];
    leaf.prop_recursive(4, 40, 4, |inner| {
        let step = (
            prop::sample::select(AXES),
            arb_test(),
            prop::collection::vec(inner.clone(), 0..=2),
        )
            .prop_map(|(axis, test, predicates)| Step {
                axis,
                test,
                predicates,
            });
        let steps = prop::collection::vec(step, 1..=3).boxed();
        prop_oneof![
            // chains of one precedence level
            (
                inner.clone(),
                prop::collection::vec(inner.clone(), 1..=3),
                prop::sample::select(OP_CLASSES),
                prop::collection::vec(0usize..4, 3),
            )
                .prop_map(|(first, operands, ops, picks)| Expr::Chain {
                    first: Box::new(first),
                    rest: operands
                        .into_iter()
                        .enumerate()
                        .map(|(i, e)| (ops[picks[i] % ops.len()], e))
                        .collect(),
                }),
            inner.clone().prop_map(|e| Expr::Neg(Box::new(e))),
            (
                prop::sample::select(FUNCS),
                prop::collection::vec(inner.clone(), 0..=3)
            )
                .prop_filter_map("arity", |(func, args)| {
                    let (min, max) = func.arity();
                    let n = args.len();
                    (n >= min && max.is_none_or(|m| n <= m))
                        .then_some(Expr::Function { func, args })
                }),
            (inner.clone(), prop::collection::vec(inner.clone(), 1..=2)).prop_map(
                |(primary, predicates)| Expr::Filter {
                    primary: Box::new(primary),
                    predicates,
                }
            ),
            steps.clone().prop_map(|steps| Expr::Path(PathExpr {
                base: PathBase::Context,
                steps
            })),
            prop::collection::vec(
                (prop::sample::select(AXES), arb_test())
                    .prop_map(|(axis, test)| Step::new(axis, test)),
                0..=3
            )
            .prop_map(|steps| Expr::Path(PathExpr {
                base: PathBase::Root,
                steps
            })),
            (inner.clone(), steps).prop_map(|(base, steps)| Expr::Path(PathExpr {
                base: PathBase::Expr(Box::new(base)),
                steps
            })),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    #[test]
    fn parse_of_a_printed_ast_equals_the_ast(e in arb_expr()) {
        let text = e.to_string();
        let back = parse(&text);
        prop_assert!(back.is_ok(), "{} did not parse: {:?}", text, back.err());
        prop_assert_eq!(back.unwrap(), e, "text: {}", text);
    }

    #[test]
    fn printing_is_idempotent_through_a_parse(e in arb_expr()) {
        let once = e.to_string();
        let twice = parse(&once).unwrap().to_string();
        prop_assert_eq!(once, twice);
    }
}

// ------------------------------------------------------------ robustness on random text

fn arb_xpathish() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(
            &[
                "a",
                "b",
                "li",
                "defName",
                "Defs",
                "RS_ThingDef",
                "/",
                "//",
                "[",
                "]",
                "(",
                ")",
                "@",
                ".",
                "..",
                "*",
                "|",
                "=",
                "!=",
                "<",
                ">=",
                "'x'",
                "\"y\"",
                " ",
                "\n",
                "::",
                ":",
                ",",
                "-",
                "+",
                "div",
                "mod",
                "and",
                "or",
                "not",
                "count",
                "last",
                "position",
                "text",
                "node",
                "1",
                "2.5",
                "$",
                "child",
                "parent",
                "descendant",
                "following-sibling",
                "string",
                "contains",
                "name",
                "sum",
                "{",
                "}",
                "é",
                "\u{0}",
            ][..],
        ),
        0..24,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn parse_never_panics_on_arbitrary_text(s in any::<String>()) {
        let _ = XPath::parse(&s);
    }

    #[test]
    fn parse_never_panics_on_xpath_like_text(s in arb_xpathish()) {
        let _ = XPath::parse(&s);
    }

    #[test]
    fn evaluation_never_panics_on_xpath_like_text(s in arb_xpathish()) {
        let d = defs_doc();
        if let Ok(xp) = XPath::parse(&s) {
            let _ = xp.eval_document(&d);
            let _ = xp.select_nodes(&d, d.root());
        }
    }

    #[test]
    fn trimming_is_applied_before_parsing(s in arb_xpathish()) {
        let padded = format!(" \n\t{s}\r\n ");
        let a = XPath::parse(&s).map(|x| x.to_string());
        let b = XPath::parse(&padded).map(|x| x.to_string());
        // the trimmed texts are equal unless the original has its own edge whitespace, in
        // which case it is trimmed as well
        prop_assert_eq!(a.is_ok(), b.is_ok());
        if let (Ok(a), Ok(b)) = (a, b) {
            prop_assert_eq!(a, b);
        }
    }
}

// ------------------------------------------------ document order and fast path parity

const STEPS: &[&str] = &[
    "Defs",
    "RS_ThingDef",
    "li",
    "*",
    "..",
    ".",
    "label",
    "comps",
    "tools",
    "text()",
    "node()",
    "@Class",
    "@*",
    "preceding-sibling::*",
    "following-sibling::li",
    "ancestor::*",
    "descendant::li",
    "descendant-or-self::node()",
    "parent::*",
    "following::label",
    "preceding::power",
    "self::*",
    "weaponTags",
    "statBases/Mass",
];

const PREDS: &[&str] = &[
    "",
    "",
    "",
    "[1]",
    "[last()]",
    "[position() > 1]",
    "[label]",
    "[not(x)]",
    "[@Class]",
    "[li]",
    "[2]",
    "[defName = 'RS_RifleA']",
    "[power > 6]",
    "[count(li) = 2]",
];

fn arb_path() -> impl Strategy<Value = String> {
    let step = (prop::sample::select(STEPS), prop::sample::select(PREDS))
        .prop_map(|(s, p)| format!("{s}{p}"));
    let sep = prop::sample::select(&["/", "//"][..]);
    let start = prop::sample::select(&["", "/", "//", "Defs/", "/Defs/", "//li/"][..]);
    (start, prop::collection::vec((sep, step), 1..5)).prop_map(|(start, steps)| {
        let mut out = start.to_owned();
        for (i, (sep, step)) in steps.iter().enumerate() {
            if i > 0 {
                out.push_str(sep);
            }
            out.push_str(step);
        }
        out
    })
}

fn arb_union() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => arb_path(),
        1 => (arb_path(), arb_path()).prop_map(|(a, b)| format!("{a} | {b}")),
        1 => (arb_path(), arb_path(), arb_path()).prop_map(|(a, b, c)| format!("({a} | {b})/{c}")),
    ]
}

fn keys(doc: &ArenaDoc, nodes: &[XNode]) -> Vec<(u32, u32)> {
    nodes
        .iter()
        .map(|n| match n {
            XNode::Node(id) => (doc.order_key(*id).unwrap(), 0),
            XNode::Attr { owner, index } => (doc.order_key(*owner).unwrap(), index + 1),
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn results_are_in_document_order_without_duplicates(src in arb_union()) {
        let d = defs_doc();
        let b = book_doc();
        for doc in [&d, &b] {
            let Ok(xp) = XPath::parse(&src) else { continue };
            let Ok(nodes) = xp.select_nodes(doc, doc.root()) else { continue };
            let k = keys(doc, &nodes);
            prop_assert!(k.windows(2).all(|w| w[0] < w[1]), "{} gave {:?}", src, k);
        }
    }

    #[test]
    fn fused_descendant_steps_match_the_general_path(src in arb_path()) {
        // `[true()]` on the descendant-or-self step switches the fast paths off.
        let slow = src.replace("//", "/descendant-or-self::node()[true()]/");
        let d = defs_doc();
        let (Ok(fast), Ok(slow)) = (XPath::parse(&src), XPath::parse(&slow)) else { return Ok(()) };
        let a = fast.select_nodes(&d, d.root());
        let b = slow.select_nodes(&d, d.root());
        prop_assert_eq!(a.is_ok(), b.is_ok());
        if let (Ok(a), Ok(b)) = (a, b) {
            prop_assert_eq!(a, b, "{}", src);
        }
    }

    #[test]
    fn evaluation_is_deterministic(src in arb_union()) {
        let d = defs_doc();
        let Ok(xp) = XPath::parse(&src) else { return Ok(()) };
        let one = xp.select_nodes(&d, d.root());
        let two = xp.select_nodes(&d, d.root());
        prop_assert_eq!(one, two);
    }
}
