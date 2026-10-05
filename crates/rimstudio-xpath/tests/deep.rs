//! Safety on deep documents and deep or huge expressions, and the micro benchmark.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::collections::HashMap;
use std::time::Instant;

use common::*;
use rimstudio_core::tree::{ArenaDoc, NodeBuilder, NodeId, OriginId};
use rimstudio_xpath::{IndexHint, Value, XPath};

/// A chain of `depth` nested `e` elements below `Defs`, built through the arena API (which has no
/// depth limit of its own, unlike import), with a text leaf at the bottom.
fn nested(depth: usize) -> (ArenaDoc, NodeId) {
    let mut d = ArenaDoc::new();
    let root = d.root();
    let mut cur = d.append_element(root, "Defs", OriginId::NONE).unwrap();
    for _ in 0..depth {
        cur = d.append_element(cur, "e", OriginId::NONE).unwrap();
    }
    d.append_text(cur, "bottom", OriginId::NONE).unwrap();
    (d, cur)
}

#[test]
fn ten_thousand_nested_elements_are_safe() {
    let (d, deepest) = nested(10_000);
    let sel = |e: &str| XPath::parse(e).unwrap().select(&d).unwrap();
    assert_eq!(sel("//e").len(), 10_000);
    assert_eq!(sel("//e[not(e)]").len(), 1);
    assert_eq!(sel("//e[last()]").len(), 10_000);
    assert_eq!(sel("/descendant::e[position() = 5000]").len(), 1);
    assert_eq!(sel("//text()").len(), 1);
    assert_eq!(sel("//e[not(e)]/ancestor::*").len(), 10_000);
    assert_eq!(sel("(//e)[5000]/following::*").len(), 0);
    assert_eq!(sel("(//e)[5000]/preceding::*").len(), 0);
    assert_eq!(sel("(//e)[5000]/descendant::e").len(), 5_000);
    assert_eq!(sel("//e/..").len(), 10_000);
    assert_eq!(sel("//e/e").len(), 9_999);
    assert_eq!(sel("/Defs//e").len(), 10_000);
    let up = XPath::parse("ancestor-or-self::e[1]/..").unwrap();
    assert_eq!(up.select_from(&d, deepest).unwrap().len(), 1);
    let xp = XPath::parse("string(/Defs)").unwrap();
    assert_eq!(
        xp.eval_document(&d).unwrap(),
        Value::String("bottom".into())
    );
    let xp = XPath::parse("count(//e[not(e)]/ancestor::e)").unwrap();
    assert_eq!(xp.eval_document(&d).unwrap(), Value::Number(9_999.0));
    let xp = XPath::parse("//e[contains(., 'bot')][1]").unwrap();
    assert_eq!(xp.select(&d).unwrap().len(), 10_000);
}

#[test]
fn a_ten_thousand_step_path_and_a_huge_chain_evaluate() {
    let (d, _) = nested(10_000);
    let path = format!("/Defs{}", "/e".repeat(10_000));
    assert_eq!(XPath::parse(&path).unwrap().select(&d).unwrap().len(), 1);
    let parents = format!("//e[not(e)]{}", "/..".repeat(9_999));
    assert_eq!(XPath::parse(&parents).unwrap().select(&d).unwrap().len(), 1);
    let chain = vec!["Defs/e"; 5_000].join(" | ");
    assert_eq!(XPath::parse(&chain).unwrap().select(&d).unwrap().len(), 1);
    let ors = format!("/Defs[{}]", vec!["x = 'y'"; 10_000].join(" or "));
    assert_eq!(XPath::parse(&ors).unwrap().select(&d).unwrap().len(), 0);
    let sum = vec!["1"; 10_000].join(" + ");
    assert_eq!(
        XPath::parse(&sum).unwrap().eval_document(&d).unwrap(),
        Value::Number(10_000.0)
    );
}

#[test]
fn deeply_nested_expressions_are_a_typed_error_at_parse_time() {
    for open in ["(", "-", "a["] {
        let close = match open {
            "(" => ")",
            "a[" => "]",
            _ => "",
        };
        let text = format!("{}1{}", open.repeat(10_000), close.repeat(10_000));
        let e = XPath::parse(&text).err().unwrap();
        assert_eq!(e.code(), "xpath.too-deep", "{open}");
    }
}

#[test]
fn nesting_at_the_documented_limit_still_parses_and_evaluates_on_a_small_stack() {
    // 60 levels is within the parser limit; a thread with a 1 MiB stack (unoptimised build) shows
    // that the recursion is bounded.
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let text = format!("{}1{}", "(".repeat(60), ")".repeat(60));
            let xp = XPath::parse(&text).unwrap();
            let d = defs_doc();
            let preds = format!("Defs{}", "[*".repeat(20) + &"]".repeat(20));
            let xp2 = XPath::parse(&preds).unwrap_or_else(|e| panic!("{preds}: {e}"));
            let nots = format!("{}1{}", "not(".repeat(60), ")".repeat(60));
            let xp3 = XPath::parse(&nots).unwrap();
            assert_eq!(xp3.eval_document(&d).unwrap(), Value::Boolean(true));
            (xp.eval_document(&d).unwrap(), xp2.select(&d).unwrap().len())
        })
        .unwrap();
    let (v, n) = handle.join().unwrap();
    assert_eq!(v, Value::Number(1.0));
    assert_eq!(n, 0);
}

// --------------------------------------------------------------------------- benchmark

/// About 13,000 nodes: 800 fictional definitions of 16 or 17 nodes each.
fn big_doc() -> (ArenaDoc, Vec<String>) {
    let mut names = Vec::new();
    let mut b = NodeBuilder::new("Defs");
    for i in 0..800 {
        let name = format!("RS_Def{i:04}");
        names.push(name.clone());
        let mut def = NodeBuilder::new("RS_ThingDef")
            .text_elem("defName", name)
            .text_elem("label", format!("def {i}"))
            .elem("statBases", |b| {
                b.text_elem("Mass", format!("{}", i % 7))
                    .text_elem("MarketValue", "10")
            })
            .elem("comps", |b| {
                b.elem("li", |b| b.attr("Class", "RS_CompA").text_elem("x", "1"))
                    .elem("li", |b| b.attr("Class", "RS_CompB"))
            });
        if i % 3 == 0 {
            def = def.elem("tools", |b| b.elem("li", |b| b.text_elem("power", "4")));
        }
        b = b.child(def);
    }
    (doc_of(b), names)
}

/// 2,849 operations shaped like a large patch set: mostly the dominant `defName` lookup with a
/// trailing path, some `or` chains, some predicates on children.
fn operations(names: &[String]) -> Vec<String> {
    let tails = [
        "",
        "/statBases",
        "/statBases/Mass",
        "/comps",
        "/comps/li[@Class='RS_CompA']",
        "/tools",
    ];
    let mut out = Vec::new();
    let mut i = 0usize;
    while out.len() < 2_849 {
        let n = &names[(i * 37) % names.len()];
        let t = tails[i % tails.len()];
        let e = match i % 10 {
            0 => format!("/Defs/RS_ThingDef[defName=\"{n}\"]{t}"),
            1 => format!(
                "Defs/RS_ThingDef[defName='{n}' or defName='{}']{t}",
                names[(i * 11) % names.len()]
            ),
            2 => format!("Defs/RS_ThingDef[defName=\"{n}\"][comps]"),
            3 => format!("Defs/RS_ThingDef[defName=\"{n}\"]/comps/li[not(x)]"),
            _ => format!("Defs/RS_ThingDef[defName=\"{n}\"]{t}"),
        };
        out.push(e);
        i += 1;
    }
    out
}

#[test]
#[ignore = "micro benchmark: run with --ignored --nocapture"]
fn bench_2849_operations_over_a_13000_node_document() {
    let (d, names) = big_doc();
    let nodes = d.len();
    let ops: Vec<XPath> = operations(&names)
        .iter()
        .map(|s| XPath::parse(s).unwrap())
        .collect();
    assert_eq!(ops.len(), 2_849);
    println!("document: {nodes} nodes, operations: {}", ops.len());

    // 1. the general evaluator for every operation
    let start = Instant::now();
    let mut total = 0usize;
    for xp in &ops {
        total += xp.select(&d).unwrap().len();
    }
    let general = start.elapsed();
    println!("general evaluator: {general:?} ({total} nodes selected)");

    // 2. through a hash index where the expression has a plan
    let mut index: HashMap<(String, String), Vec<NodeId>> = HashMap::new();
    let defs = d.document_element().unwrap();
    for c in d.children(defs) {
        let Some(ty) = d.name(c) else { continue };
        for dn in d.children_named(c, "defName") {
            let v = d.string_value(dn).unwrap().into_owned();
            index.entry((ty.to_owned(), v)).or_default().push(c);
        }
    }
    let start = Instant::now();
    let mut total_indexed = 0usize;
    let mut planned = 0usize;
    for xp in &ops {
        if let Some(plan) = xp.index_plan() {
            planned += 1;
            let mut found: Vec<NodeId> = Vec::new();
            let keys = match &plan.hint {
                IndexHint::DefByName { def_type, name } => vec![(def_type.clone(), name.clone())],
                IndexHint::DefsByNames(k) => k
                    .iter()
                    .map(|k| (k.def_type.clone(), k.name.clone()))
                    .collect(),
            };
            for k in keys {
                if let Some(v) = index.get(&k) {
                    found.extend(v.iter().copied());
                }
            }
            total_indexed += xp.select_with_defs(&d, &found).unwrap().len();
        } else {
            total_indexed += xp.select(&d).unwrap().len();
        }
    }
    let indexed = start.elapsed();
    println!(
        "index plans: {indexed:?} ({planned} of {} planned)",
        ops.len()
    );
    assert_eq!(total, total_indexed, "indexed evaluation must agree");
}
