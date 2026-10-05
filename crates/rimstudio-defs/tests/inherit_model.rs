//! Cross checks the arena based inheritance against a small reference model written directly on
//! owned trees, over random forests with duplicate names, missing parents, cycles, `Inherit`
//! attributes, text and mixed content. Also checks that the result does not depend on the thread
//! count.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use proptest::prelude::*;
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::{Child, Node};
use rimstudio_defs::{
    DefFile, FileContent, InheritConfig, ModEntry, ModOrder, ResolvedDoc, merge,
    resolve_inheritance,
};

// ------------------------------------------------------------------ generators

fn arb_text() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["t", "u", "1", "x y"]).prop_map(str::to_owned)
}

fn arb_tag() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["a", "b", "c", "li", "list", "forcedTraits"])
}

fn arb_children(depth: u32) -> BoxedStrategy<Vec<Child>> {
    let leaf = prop_oneof![
        arb_text().prop_map(Child::Text),
        (arb_tag(), arb_text()).prop_map(|(t, x)| Child::Element(Node::with_text(t, x))),
        arb_tag().prop_map(|t| Child::Element(Node::new(t))),
    ];
    if depth == 0 {
        return prop::collection::vec(leaf, 0..4)
            .prop_map(normalise)
            .boxed();
    }
    let inner = arb_tag().prop_flat_map(move |t| {
        (
            prop::option::of(prop::sample::select(vec!["false", "False", "true"])),
            arb_children(depth - 1),
        )
            .prop_map(move |(inh, kids)| {
                let mut n = Node::new(t);
                if let Some(v) = inh {
                    n.attrs.push(("Inherit".to_owned(), v.to_owned()));
                }
                n.children = kids;
                Child::Element(n)
            })
    });
    prop::collection::vec(prop_oneof![leaf, inner], 0..4)
        .prop_map(normalise)
        .boxed()
}

/// Merges adjacent text children and drops empty ones (an XML reader never produces them).
fn normalise(children: Vec<Child>) -> Vec<Child> {
    let mut out: Vec<Child> = Vec::new();
    for c in children {
        match c {
            Child::Text(t) => {
                if let Some(Child::Text(prev)) = out.last_mut() {
                    prev.push_str(&t);
                } else if !t.is_empty() {
                    out.push(Child::Text(t));
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn arb_top() -> impl Strategy<Value = Node> {
    (
        prop::option::of(0u8..4),
        prop::option::of(prop_oneof![(0u8..4).prop_map(Some), Just(None)]),
        prop::option::of(prop::sample::select(vec!["false", "False"])),
        arb_children(2),
    )
        .prop_map(|(name, parent, inherit, kids)| {
            let mut n = Node::new("RS_T");
            if let Some(i) = name {
                n.attrs.push(("Name".to_owned(), format!("N{i}")));
            }
            match parent {
                Some(Some(p)) => n.attrs.push(("ParentName".to_owned(), format!("N{p}"))),
                Some(None) => n
                    .attrs
                    .push(("ParentName".to_owned(), "Missing".to_owned())),
                None => {}
            }
            if let Some(v) = inherit {
                n.attrs.push(("Inherit".to_owned(), v.to_owned()));
            }
            n.children = kids;
            n
        })
}

// ------------------------------------------------------------------ the reference model

fn merge_ref(child: &Node, cur: &mut Node, allow: &BTreeSet<String>) {
    let inherit_false = child
        .attr("Inherit")
        .is_some_and(|v| v.eq_ignore_ascii_case("false"));
    if inherit_false {
        cur.children = child.children.clone();
        for (k, v) in &child.attrs {
            if k != "Inherit" {
                cur.remove_attr(k);
                cur.attrs.push((k.clone(), v.clone()));
            }
        }
        return;
    }
    cur.attrs = child.attrs.clone();
    let last_text = child
        .children
        .iter()
        .rev()
        .find_map(|c| c.as_text().filter(|t| !t.is_empty()));
    if let Some(t) = last_text {
        if !cur.children.is_empty() {
            cur.children.remove(0);
        }
        cur.children.push(Child::Text(t.to_owned()));
        return;
    }
    if child.elements().next().is_none() {
        if cur.elements().next().is_none() && !cur.children.is_empty() {
            cur.children.remove(0);
        }
        return;
    }
    for item in child.elements() {
        if item.tag == "li" || allow.contains(&child.tag) {
            cur.children.push(Child::Element(item.clone()));
        } else if let Some(t) = cur.child_mut(&item.tag) {
            merge_ref(item, t, allow);
        } else {
            cur.children.push(Child::Element(item.clone()));
        }
    }
}

fn normalise_node(n: &mut Node) {
    n.children = normalise(std::mem::take(&mut n.children));
    for c in n.elements_mut() {
        normalise_node(c);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Todo,
    Active,
    Done,
    Cyclic,
}

/// The expected `(node, resolved)` of every top-level node, for one mod.
fn reference(tops: &[Node], allow: &BTreeSet<String>) -> Vec<(Node, bool)> {
    // registration: Name or ParentName present, a repeated Name is not registered
    let mut registered: Vec<usize> = Vec::new();
    let mut names: Vec<(String, usize)> = Vec::new();
    for (i, n) in tops.iter().enumerate() {
        let name = n.attr("Name");
        if name.is_none() && n.attr("ParentName").is_none() {
            continue;
        }
        if let Some(nm) = name {
            if names.iter().any(|(x, _)| x == nm) {
                continue;
            }
            names.push((nm.to_owned(), i));
        }
        registered.push(i);
    }
    let parent_of = |i: usize| -> Option<usize> {
        let pn = tops[i].attr("ParentName")?;
        names.iter().find(|(x, _)| x == pn).map(|(_, e)| *e)
    };
    let mut state = vec![State::Todo; tops.len()];
    let mut resolved: Vec<Option<Node>> = vec![None; tops.len()];
    fn go(
        i: usize,
        tops: &[Node],
        parent_of: &dyn Fn(usize) -> Option<usize>,
        state: &mut Vec<State>,
        resolved: &mut Vec<Option<Node>>,
        allow: &BTreeSet<String>,
    ) {
        match state[i] {
            State::Done | State::Cyclic => return,
            State::Active => return,
            State::Todo => {}
        }
        state[i] = State::Active;
        match parent_of(i) {
            None => {
                resolved[i] = Some(tops[i].clone());
                state[i] = State::Done;
            }
            Some(p) => {
                if state[p] == State::Active {
                    state[i] = State::Cyclic;
                    return;
                }
                go(p, tops, parent_of, state, resolved, allow);
                match resolved[p].clone() {
                    Some(mut cur) if state[p] == State::Done => {
                        merge_ref(&tops[i], &mut cur, allow);
                        resolved[i] = Some(cur);
                        state[i] = State::Done;
                    }
                    _ => state[i] = State::Cyclic,
                }
            }
        }
    }
    for &i in &registered {
        go(i, tops, &parent_of, &mut state, &mut resolved, allow);
    }
    // a node that is part of a cycle makes everything on the cycle unresolved; members that were
    // marked Active by the walk but never completed are cyclic too
    (0..tops.len())
        .map(|i| {
            let reg = registered.contains(&i);
            let has_parent = tops[i].attr("ParentName").is_some();
            if reg && state[i] == State::Done {
                let mut n = resolved[i].clone().unwrap();
                normalise_node(&mut n);
                (n, true)
            } else if has_parent {
                (tops[i].clone(), false)
            } else {
                (tops[i].clone(), true)
            }
        })
        .collect()
}

fn doc_of(tops: &[Node]) -> rimstudio_defs::UnifiedDoc {
    let order = ModOrder::new(vec![ModEntry::new(ModIdx(0), "rs.m", "M")]);
    let file = DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/a.xml".into(),
        content: FileContent::parsed(Node {
            tag: "Defs".into(),
            attrs: vec![],
            children: tops.iter().cloned().map(Child::Element).collect(),
        }),
    };
    merge(&order, &[file])
}

fn pool(threads: usize) -> &'static rayon::ThreadPool {
    static ONE: OnceLock<rayon::ThreadPool> = OnceLock::new();
    static EIGHT: OnceLock<rayon::ThreadPool> = OnceLock::new();
    let cell = if threads == 1 { &ONE } else { &EIGHT };
    cell.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
    })
}

fn summary(r: &ResolvedDoc) -> Vec<(Node, bool, usize)> {
    r.nodes
        .iter()
        .map(|n| (n.node.clone(), n.resolved, n.parents.len()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn the_arena_inheritance_matches_the_reference_model(tops in prop::collection::vec(arb_top(), 1..9)) {
        let cfg = InheritConfig::default();
        let doc = doc_of(&tops);
        let got = resolve_inheritance(&doc, &ActiveSet::new(), &cfg);
        let want = reference(&tops, &cfg.allow_duplicate_nodes);
        prop_assert_eq!(got.nodes.len(), want.len());
        for (i, ((node, resolved), g)) in want.iter().zip(&got.nodes).enumerate() {
            prop_assert_eq!(&g.node, node, "node {}", i);
            prop_assert_eq!(g.resolved, *resolved, "resolved flag of node {}", i);
        }
    }

    #[test]
    fn the_result_does_not_depend_on_the_thread_count(tops in prop::collection::vec(arb_top(), 1..9)) {
        let cfg = InheritConfig::default();
        let doc = doc_of(&tops);
        let one = pool(1).install(|| resolve_inheritance(&doc, &ActiveSet::new(), &cfg));
        let eight = pool(8).install(|| resolve_inheritance(&doc, &ActiveSet::new(), &cfg));
        prop_assert_eq!(summary(&one), summary(&eight));
        prop_assert_eq!(one.diagnostics.total(), eight.diagnostics.total());
        prop_assert_eq!(one.cyclic, eight.cyclic);
    }
}
