//! Arena mutation sequences compared with a naive owned-tree reference model, document order after
//! random mutations, stale handle behaviour and a large tree smoke test. All names are fictional.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod tree_support;

use std::cmp::Ordering;

use proptest::prelude::*;
use rimstudio_core::tree::{ArenaDoc, Child, Node, NodeId, NodeKind, OriginId, TreeError};

const O: OriginId = OriginId(7);
const ATTR_NAMES: [&str; 3] = ["Class", "Name", "MayRequire"];

#[derive(Debug, Clone)]
enum Op {
    AppendElement {
        parent: usize,
        tag: String,
    },
    AppendText {
        parent: usize,
        text: String,
    },
    InsertBefore {
        target: usize,
        tag: String,
    },
    InsertAfter {
        target: usize,
        tag: String,
    },
    Remove {
        target: usize,
    },
    ReplaceWithTwo {
        target: usize,
        a: String,
        b: String,
    },
    SetAttr {
        target: usize,
        name: usize,
        value: String,
    },
    RemoveAttr {
        target: usize,
        name: usize,
    },
    CloneAppend {
        source: usize,
        parent: usize,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let sel = 0usize..64;
    prop_oneof![
        3 => (sel.clone(), tree_support::tag()).prop_map(|(parent, tag)| Op::AppendElement { parent, tag }),
        2 => (sel.clone(), tree_support::text()).prop_map(|(parent, text)| Op::AppendText { parent, text }),
        2 => (sel.clone(), tree_support::tag()).prop_map(|(target, tag)| Op::InsertBefore { target, tag }),
        2 => (sel.clone(), tree_support::tag()).prop_map(|(target, tag)| Op::InsertAfter { target, tag }),
        2 => sel.clone().prop_map(|target| Op::Remove { target }),
        1 => (sel.clone(), tree_support::tag(), tree_support::tag()).prop_map(|(target, a, b)| Op::ReplaceWithTwo { target, a, b }),
        2 => (sel.clone(), 0..ATTR_NAMES.len(), "[a-z0-9]{0,4}").prop_map(|(target, name, value)| Op::SetAttr { target, name, value }),
        1 => (sel.clone(), 0..ATTR_NAMES.len()).prop_map(|(target, name)| Op::RemoveAttr { target, name }),
        1 => (sel.clone(), sel).prop_map(|(source, parent)| Op::CloneAppend { source, parent }),
    ]
}

/// Paths (child index chains) of every node below `root` in pre order, excluding the root.
fn model_paths(root: &Node) -> Vec<Vec<usize>> {
    fn walk(n: &Node, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        for (i, c) in n.children.iter().enumerate() {
            prefix.push(i);
            out.push(prefix.clone());
            if let Child::Element(e) = c {
                walk(e, prefix, out);
            }
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &mut out);
    out
}

fn model_node<'a>(root: &'a Node, path: &[usize]) -> Option<&'a Child> {
    let (last, parents) = path.split_last()?;
    let mut n = root;
    for i in parents {
        n = n.children.get(*i)?.as_element()?;
    }
    n.children.get(*last)
}

fn model_parent_mut<'a>(root: &'a mut Node, path: &[usize]) -> Option<(&'a mut Node, usize)> {
    let (last, parents) = path.split_last()?;
    let mut n = root;
    for i in parents {
        n = n.children.get_mut(*i)?.as_element_mut()?;
    }
    Some((n, *last))
}

fn model_element_mut<'a>(root: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let mut n = root;
    for i in path {
        n = n.children.get_mut(*i)?.as_element_mut()?;
    }
    Some(n)
}

fn is_element(root: &Node, path: &[usize]) -> bool {
    path.is_empty() || matches!(model_node(root, path), Some(Child::Element(_)))
}

fn el(tag: &str) -> Node {
    Node::new(tag)
}

/// Applies one operation to both sides. Selectors pick the n-th node modulo the node count.
fn apply(doc: &mut ArenaDoc, top: NodeId, model: &mut Node, op: &Op, removed: &mut Vec<NodeId>) {
    let paths = model_paths(model);
    let ids: Vec<NodeId> = doc.descendants(top).collect();
    assert_eq!(
        paths.len(),
        ids.len(),
        "model and arena node counts diverged"
    );
    // Index 0 stands for the top element itself, index k for the k-th descendant.
    let pick = |s: usize| s % (paths.len() + 1);
    let path_of = |k: usize| -> Vec<usize> {
        if k == 0 {
            Vec::new()
        } else {
            paths[k - 1].clone()
        }
    };
    let id_of = |k: usize| -> NodeId { if k == 0 { top } else { ids[k - 1] } };
    let elements: Vec<usize> = (0..=paths.len())
        .filter(|k| is_element(model, &path_of(*k)))
        .collect();
    let pick_el = |s: usize| elements[s % elements.len()];

    match op {
        Op::AppendElement { parent, tag } => {
            let k = pick_el(*parent);
            doc.append_element(id_of(k), tag, O).unwrap();
            model_element_mut(model, &path_of(k))
                .unwrap()
                .children
                .push(Child::Element(el(tag)));
        }
        Op::AppendText { parent, text } => {
            let k = pick_el(*parent);
            doc.append_text(id_of(k), text, O).unwrap();
            model_element_mut(model, &path_of(k))
                .unwrap()
                .children
                .push(Child::Text(text.clone()));
        }
        Op::InsertBefore { target, tag } | Op::InsertAfter { target, tag } => {
            let k = pick(*target);
            if k == 0 {
                return;
            }
            let new = doc.create_element(tag, O).unwrap();
            let after = matches!(op, Op::InsertAfter { .. });
            if after {
                doc.insert_after(id_of(k), new).unwrap();
            } else {
                doc.insert_before(id_of(k), new).unwrap();
            }
            let (parent, i) = model_parent_mut(model, &path_of(k)).unwrap();
            parent
                .children
                .insert(if after { i + 1 } else { i }, Child::Element(el(tag)));
        }
        Op::Remove { target } => {
            let k = pick(*target);
            if k == 0 {
                return;
            }
            let id = id_of(k);
            let subtree: Vec<NodeId> = doc.preorder(id).collect();
            doc.remove(id).unwrap();
            removed.extend(subtree);
            let (parent, i) = model_parent_mut(model, &path_of(k)).unwrap();
            parent.children.remove(i);
        }
        Op::ReplaceWithTwo { target, a, b } => {
            let k = pick(*target);
            if k == 0 {
                return;
            }
            let id = id_of(k);
            let (na, nb) = (
                doc.create_element(a, O).unwrap(),
                doc.create_element(b, O).unwrap(),
            );
            doc.replace_with(id, &[na, nb]).unwrap();
            // replace_with detaches the old node, it stays alive, so it is not tracked as stale.
            let (parent, i) = model_parent_mut(model, &path_of(k)).unwrap();
            parent
                .children
                .splice(i..=i, [Child::Element(el(a)), Child::Element(el(b))]);
        }
        Op::SetAttr {
            target,
            name,
            value,
        } => {
            let k = pick_el(*target);
            doc.set_attr(id_of(k), ATTR_NAMES[*name], value).unwrap();
            model_element_mut(model, &path_of(k))
                .unwrap()
                .set_attr(ATTR_NAMES[*name], value.clone());
        }
        Op::RemoveAttr { target, name } => {
            let k = pick_el(*target);
            let got = doc.remove_attr(id_of(k), ATTR_NAMES[*name]).unwrap();
            let want = model_element_mut(model, &path_of(k))
                .unwrap()
                .remove_attr(ATTR_NAMES[*name]);
            assert_eq!(got, want);
        }
        Op::CloneAppend { source, parent } => {
            let s = pick(*source);
            let p = pick_el(*parent);
            let sid = id_of(s);
            // Cloning a text node is allowed by the arena; the model copies the child.
            let copy = doc.deep_clone(sid).unwrap();
            let child = if s == 0 {
                Child::Element(model.clone())
            } else {
                model_node(model, &path_of(s)).unwrap().clone()
            };
            // Appending below the cloned subtree cannot happen: the copy is detached.
            doc.append_child(id_of(p), copy).unwrap();
            model_element_mut(model, &path_of(p))
                .unwrap()
                .children
                .push(child);
        }
    }
}

fn check(doc: &ArenaDoc, top: NodeId, model: &Node) {
    assert_eq!(doc.check_integrity(), Ok(()));
    assert_eq!(doc.to_node(top).as_ref(), Some(model));
    let ids: Vec<NodeId> = doc.descendants(top).collect();
    // Document order agrees with pre order position, with and without the cache.
    let mut sample: Vec<NodeId> = std::iter::once(top).chain(ids.iter().copied()).collect();
    sample.truncate(24);
    for cached in [false, true] {
        if cached {
            doc.prepare_order();
        }
        for (i, a) in sample.iter().enumerate() {
            for (j, b) in sample.iter().enumerate() {
                assert_eq!(
                    doc.compare_order(*a, *b),
                    Some(i.cmp(&j)),
                    "order of {i} and {j} (cached {cached})"
                );
            }
        }
    }
    let mut shuffled = sample.clone();
    shuffled.reverse();
    doc.sort_document_order(&mut shuffled);
    assert_eq!(shuffled, sample);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn arena_follows_the_reference_model(start in tree_support::node(), ops in prop::collection::vec(op(), 0..40)) {
        let mut doc = ArenaDoc::from_node(&start, O).unwrap();
        let top = doc.document_element().unwrap();
        let mut model = start;
        let mut removed = Vec::new();
        check(&doc, top, &model);
        for op in &ops {
            apply(&mut doc, top, &mut model, op, &mut removed);
            check(&doc, top, &model);
        }
        // Every handle of a removed subtree is stale and stays harmless.
        for id in removed {
            prop_assert!(!doc.contains(id));
            prop_assert_eq!(doc.kind(id), None);
            prop_assert_eq!(doc.set_attr(id, "Class", "x"), Err(TreeError::StaleId));
            prop_assert_eq!(doc.compare_order(id, top), None);
            prop_assert_eq!(doc.children(id).count(), 0);
        }
    }

    #[test]
    fn import_export_is_identity(n in tree_support::node()) {
        let doc = ArenaDoc::from_node(&n, OriginId(1)).unwrap();
        prop_assert_eq!(doc.to_document_node(), Some(n));
        prop_assert_eq!(doc.check_integrity(), Ok(()));
    }

    #[test]
    fn string_value_is_the_concatenated_text(n in tree_support::node()) {
        let doc = ArenaDoc::from_node(&n, O).unwrap();
        let top = doc.document_element().unwrap();
        prop_assert_eq!(doc.string_value(top).map(|s| s.into_owned()), Some(n.text_content()));
    }
}

#[test]
fn removed_handles_stay_stale_after_slot_reuse() {
    let mut doc = ArenaDoc::from_node(&el("Root"), O).unwrap();
    let top = doc.document_element().unwrap();
    let a = doc.append_element(top, "A", O).unwrap();
    doc.remove(a).unwrap();
    for _ in 0..8 {
        doc.append_element(top, "B", O).unwrap();
    }
    assert!(!doc.contains(a));
    assert_eq!(doc.name(a), None);
    assert_eq!(doc.append_text(a, "x", O), Err(TreeError::StaleId));
    assert_eq!(doc.remove(a), Err(TreeError::StaleId));
    assert_eq!(doc.kind(top), Some(NodeKind::Element));
}

#[test]
fn a_large_tree_survives_import_order_and_export() {
    let width = 400usize;
    let depth_items = 250usize;
    let mut root = el("Defs");
    for i in 0..width {
        let mut def = el("ThingDef");
        def.set_attr("Name", format!("RS_Def{i}"));
        for j in 0..depth_items {
            let mut field = el("field");
            field.push_text(format!("{i}-{j}"));
            def.push_child(field);
        }
        root.push_child(def);
    }
    let mut doc = ArenaDoc::from_node(&root, O).unwrap();
    let top = doc.document_element().unwrap();
    assert_eq!(doc.len(), 1 + 1 + width + 2 * width * depth_items);
    assert_eq!(doc.check_integrity(), Ok(()));
    doc.prepare_order();
    let first = doc.first_child(top).unwrap();
    let last = doc.last_child(top).unwrap();
    assert_eq!(doc.compare_order(first, last), Some(Ordering::Less));
    doc.remove(first).unwrap();
    assert_eq!(doc.child_count(top), width - 1);
    assert_eq!(
        doc.to_document_node().map(|n| n.children.len()),
        Some(width - 1)
    );
    assert_eq!(doc.check_integrity(), Ok(()));
}
