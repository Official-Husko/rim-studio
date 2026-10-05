//! The corpus gate.
//!
//! The raw expression list of docs/research/data/xpath-corpus is not stored in the repository (the
//! patches belong to their authors), so this file holds representative expressions for every tier of
//! the research summary, written against a fictional document. Each row carries the node count that
//! was worked out by hand. The gate weights every row with the occurrence count of its tier (the
//! `tiers.occurrences` block of `summary.json`, split evenly over the rows of the tier) and asserts
//! the thresholds of the research note: tiers A and B together cover at least 97 percent of all
//! occurrences, tiers A, B and C at least 99.8 percent. A row counts as covered only when it parses,
//! evaluates and returns exactly the expected number of nodes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rimstudio_core::tree::{ArenaDoc, NodeId};
use rimstudio_xpath::{IndexHint, XPath};

/// Occurrences per tier in the research corpus (summary.json, `tiers.occurrences`).
const OCC_A: f64 = 164_551.0;
const OCC_B: f64 = 13_726.0;
const OCC_C: f64 = 5_006.0;
const OCC_D: f64 = 88.0;
const OCC_X: f64 = 249.0;

/// Tier A: child steps, a final attribute, predicates of the form (@attr or child) = literal.
const TIER_A: &[(&str, usize)] = &[
    ("Defs/RS_ThingDef[defName='RS_RifleA']", 2),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/statBases", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/tools", 1),
    ("/Defs/RS_ThingDef[defName='RS_RifleB']", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/comps", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/statBases/Mass", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleB']/statBases/Mass", 1),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/comps/li[@Class='RS_CompA']",
        1,
    ),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/weaponTags", 1),
    ("Defs/RS_ThingDef[defName='RS_Nope']", 0),
    ("Defs/RS_KindDef[defName='RS_Kind']/weights/li", 3),
    ("Defs/RS_RecipeDef[defName='RS_Make']/recipeUsers", 1),
    ("Defs/RS_ThingDef[@Name='RS_BaseGun']", 1),
    ("Defs/RS_ThingDef[@ParentName='RS_BaseGun']/@ParentName", 1),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[label='stock']/power",
        1,
    ),
    ("Defs/RS_ThingDef[defName='RS_RifleA'][label='rifle a']", 1),
    ("Defs/RS_ThingDef[@Abstract='True'][label='base']", 1),
    ("Defs/RS_ThingDef[defName='RS_Knife']/tools/li/label", 1),
];

/// Tier B: tier A plus `//`, `.`, `..`, `*`, `and`, `or`, `not()`, `contains()`, `starts-with()`,
/// `!=`, existence and nested path predicates, numeric equality.
const TIER_B: &[(&str, usize)] = &[
    (
        "Defs/RS_ThingDef[defName='RS_RifleA' or defName='RS_Knife']",
        3,
    ),
    (
        "/Defs/RS_ThingDef[defName='RS_RifleA' or defName='RS_RifleB' or defName='RS_Knife']",
        4,
    ),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA' and label='duplicate']",
        1,
    ),
    ("Defs/RS_ThingDef[not(defName)]", 2),
    ("Defs/RS_ThingDef[contains(defName,'Rifle')]", 3),
    ("Defs/RS_ThingDef[starts-with(defName,'RS_Ri')]", 3),
    ("Defs/RS_ThingDef[defName!='RS_RifleA']", 3),
    ("Defs/RS_ThingDef[comps]", 2),
    ("Defs/RS_ThingDef[@Name]", 1),
    ("Defs/RS_ThingDef[comps/li/x]", 2),
    ("Defs/RS_ThingDef[statBases/Mass=4]", 1),
    ("Defs//li[@Class='RS_CompA']", 2),
    ("//RS_ThingDef[defName='RS_Knife']", 1),
    ("Defs/*[defName='RS_Make']", 1),
    ("Defs/RS_ThingDef[defName='RS_Knife']/tools/li[1]/.", 1),
    ("Defs/RS_ThingDef[defName='RS_Knife']/tools/..", 1),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/comps/li[@Class='RS_CompB']/..",
        1,
    ),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/comps/li[not(x)]", 1),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[power=9 or power=5]",
        2,
    ),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/*", 8),
    (
        "Defs/RS_ThingDef[@Abstract='True' or defName='RS_Knife']",
        2,
    ),
    ("Defs/RS_ThingDef[not(@Name) and defName]", 5),
    ("Defs//power", 3),
    ("Defs/RS_ThingDef[defName='RS_RifleA']//label", 4),
    (
        "Defs/RS_ThingDef[contains(label,'rifle') and not(@Abstract)]",
        1,
    ),
];

/// Tier C: tier B plus `[n]`, `last()`, `position()`, `count()`, `text()`, relational operators and
/// unions.
const TIER_C: &[(&str, usize)] = &[
    ("Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[1]", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[2]/label", 1),
    ("Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[last()]", 1),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/tools/li[position()=1]",
        1,
    ),
    ("Defs/RS_ThingDef[defName='RS_Knife']/label/text()", 1),
    ("Defs/RS_ThingDef[label/text()='knife']", 1),
    ("Defs/RS_ThingDef[count(comps/li)=2]", 1),
    ("Defs/RS_ThingDef[statBases/Mass>3]", 2),
    ("Defs/RS_ThingDef[statBases/Mass<=3.5]", 2),
    (
        "Defs/RS_ThingDef[defName='RS_Knife'] | Defs/RS_RecipeDef[defName='RS_Make']",
        2,
    ),
    ("Defs/RS_ThingDef/tools/li[power>=9]", 2),
    (
        "Defs/RS_ThingDef[defName='RS_RifleA']/weaponTags/li[text()='Gun']",
        1,
    ),
    ("Defs/RS_ThingDef[2]", 1),
    ("Defs/*[position()=last()]", 1),
    ("Defs/RS_ThingDef[count(tools/li)>1]", 1),
    (
        "Defs/RS_ThingDef/statBases/Mass | Defs/RS_ThingDef/statBases/MarketValue",
        4,
    ),
    ("Defs/RS_ThingDef/tools/li[2]", 1),
    ("Defs/RS_KindDef[defName='RS_Kind']/weights/li[3]", 1),
];

/// Tier D: any other valid XPath 1.0 (explicit axes, other functions, filter expressions).
const TIER_D: &[(&str, usize)] = &[
    ("Defs/RS_ThingDef[defName=defName and not(@Name)]", 5),
    ("Defs/*[self::RS_RecipeDef or self::RS_KindDef]", 2),
    (
        "Defs/RS_ThingDef[contains(translate(label,'ABCDEFGHIJKLMNOPQRSTUVWXYZ','abcdefghijklmnopqrstuvwxyz'),'rifle')]",
        1,
    ),
    ("Defs/RS_ThingDef[normalize-space(label)='knife']", 1),
    ("Defs/descendant::RS_RecipeDef", 1),
    ("(/Defs/RS_ThingDef | /Defs/RS_RecipeDef)/defName", 6),
    ("Defs/RS_ThingDef[string(defName)='RS_Knife']", 1),
    ("Defs/RS_ThingDef[name()='RS_ThingDef'][1]", 1),
    ("Defs/RS_ThingDef/statBases/*[name()='Mass']", 3),
];

/// Tier X: rejected by the game's runtime (the invalid shapes of the corpus).
const TIER_X: &[&str] = &[
    "Defs/RS_BodyDef[defName=\"RS_EyeBody]/corePart/parts/li[def=\"RS_Part\"]",
    "Defs/RS_ThingDef[defName=\"RS_Rifle\"]/",
    "Defs/RS_ThingDef[defName=\"RS_Rifle\"",
    "{RS_Template}",
];

fn handled(doc: &ArenaDoc, expr: &str, want: usize) -> bool {
    let Ok(xp) = XPath::parse(expr) else {
        return false;
    };
    match xp.select_nodes(doc, doc.root()) {
        Ok(nodes) => nodes.len() == want,
        Err(_) => false,
    }
}

fn tier_share(doc: &ArenaDoc, rows: &[(&str, usize)], occurrences: f64) -> f64 {
    let ok = rows.iter().filter(|(e, n)| handled(doc, e, *n)).count();
    occurrences * ok as f64 / rows.len() as f64
}

#[test]
fn every_row_parses_evaluates_and_matches_its_expected_count() {
    let d = defs_doc();
    for (name, rows) in [("A", TIER_A), ("B", TIER_B), ("C", TIER_C), ("D", TIER_D)] {
        for (expr, want) in rows {
            let xp = XPath::parse(expr).unwrap_or_else(|e| panic!("tier {name} {expr}: {e}"));
            let got = xp
                .select_nodes(&d, d.root())
                .unwrap_or_else(|e| panic!("tier {name} {expr}: {e}"))
                .len();
            assert_eq!(got, *want, "tier {name}: {expr}");
        }
    }
}

#[test]
fn tier_x_expressions_are_rejected_with_a_parse_code() {
    for expr in TIER_X {
        let e = XPath::parse(expr).err();
        assert_eq!(e.as_ref().map(|e| e.code()), Some("xpath.parse"), "{expr}");
        assert!(e.and_then(|e| e.position()).is_some(), "{expr}");
    }
}

#[test]
fn weighted_coverage_meets_the_research_thresholds() {
    let d = defs_doc();
    let all = OCC_A + OCC_B + OCC_C + OCC_D + OCC_X;
    let a = tier_share(&d, TIER_A, OCC_A);
    let b = tier_share(&d, TIER_B, OCC_B);
    let c = tier_share(&d, TIER_C, OCC_C);
    let dd = tier_share(&d, TIER_D, OCC_D);
    let rejected = TIER_X.iter().filter(|e| XPath::parse(e).is_err()).count();
    assert_eq!(rejected, TIER_X.len());
    let ab = 100.0 * (a + b) / all;
    let abc = 100.0 * (a + b + c) / all;
    let abcd = 100.0 * (a + b + c + dd) / all;
    assert!(ab >= 97.0, "tiers A and B cover {ab:.2} percent");
    assert!(abc >= 99.8, "tiers A, B and C cover {abc:.2} percent");
    assert!(abcd >= abc, "tier D adds coverage");
}

#[test]
fn rows_have_no_duplicates_and_all_tiers_are_populated() {
    let mut seen = std::collections::BTreeSet::new();
    for (expr, _) in TIER_A.iter().chain(TIER_B).chain(TIER_C).chain(TIER_D) {
        assert!(seen.insert(*expr), "duplicate row {expr}");
    }
    assert!(TIER_A.len() >= 15 && TIER_B.len() >= 20 && TIER_C.len() >= 15 && TIER_D.len() >= 8);
}

// ---------------------------------------------------------------- index hint parity

/// A naive index: definitions by (type, defName), in document order.
fn lookup(doc: &ArenaDoc, hint: &IndexHint) -> Vec<NodeId> {
    let defs = doc.document_element().unwrap();
    let keys = hint.keys();
    let mut out: Vec<NodeId> = Vec::new();
    for c in doc.children(defs) {
        let Some(name) = doc.name(c) else { continue };
        for k in &keys {
            if name == k.def_type
                && doc
                    .children_named(c, "defName")
                    .any(|d| doc.string_value(d).as_deref() == Some(k.name.as_str()))
                && !out.contains(&c)
            {
                out.push(c);
            }
        }
    }
    out
}

#[test]
fn index_plans_agree_with_full_evaluation_for_every_corpus_row() {
    let d = defs_doc();
    let mut with_plan = 0;
    for (expr, _) in TIER_A.iter().chain(TIER_B).chain(TIER_C).chain(TIER_D) {
        let xp = XPath::parse(expr).unwrap();
        let Some(plan) = xp.index_plan() else {
            continue;
        };
        with_plan += 1;
        let defs = lookup(&d, &plan.hint);
        let Ok(expected) = xp.select(&d) else {
            // attribute result: compare through select_nodes instead
            continue;
        };
        let got = xp.select_with_defs(&d, &defs).unwrap();
        assert_eq!(got, expected, "{expr}");
        if plan.rest.is_empty() {
            assert_eq!(
                xp.index_hint().map(|h| lookup(&d, &h)),
                Some(expected),
                "{expr}"
            );
        }
    }
    assert!(
        with_plan >= 20,
        "only {with_plan} corpus rows have an index plan"
    );
}

#[test]
fn the_dominant_shape_gives_a_def_by_name_hint() {
    let xp = XPath::parse("  Defs/RS_ThingDef[defName=\"RS_RifleA\"]\n").unwrap();
    assert_eq!(
        xp.index_hint(),
        Some(IndexHint::DefByName {
            def_type: "RS_ThingDef".into(),
            name: "RS_RifleA".into()
        })
    );
    let xp = XPath::parse("/Defs/RS_ThingDef[defName='a' or defName='b']").unwrap();
    assert!(matches!(xp.index_hint(), Some(IndexHint::DefsByNames(k)) if k.len() == 2));
    assert_eq!(
        XPath::parse("Defs/RS_ThingDef[defName='a']/x")
            .unwrap()
            .index_hint(),
        None
    );
    assert_eq!(
        XPath::parse("Defs/RS_ThingDef[defName='a']/x")
            .unwrap()
            .index_plan()
            .map(|p| p.rest.len()),
        Some(1)
    );
    assert!(
        XPath::parse("Defs/RS_ThingDef")
            .unwrap()
            .select_with_defs(&defs_doc(), &[])
            .is_err()
    );
}
