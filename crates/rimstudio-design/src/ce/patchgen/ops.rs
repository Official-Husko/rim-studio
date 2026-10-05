//! Builders for patch operation nodes: the enumerated vanilla classes, xpath helpers and the
//! ensure-container idiom.
//!
//! Operation classes come from [`OpClass`], never from free text, so a misspelt class cannot be generated.
//! Everything here builds [`Node`] trees; no text is formatted as markup.

use std::collections::BTreeSet;

use rimstudio_core::tree::{Node, NodeBuilder};

use super::container::Container;

/// The patch operation classes the generator uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OpClass {
    /// Adds the value nodes to the selected nodes.
    Add,
    /// Replaces the selected nodes with the value nodes.
    Replace,
    /// Runs the matching branch on the xpath result.
    Conditional,
    /// Runs the branch when a mod with the given name is active.
    FindMod,
}

impl OpClass {
    /// The class name as written in a `Class` attribute.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "PatchOperationAdd",
            Self::Replace => "PatchOperationReplace",
            Self::Conditional => "PatchOperationConditional",
            Self::FindMod => "PatchOperationFindMod",
        }
    }
}

/// The attribute that makes a list replace the inherited one instead of merging with it.
pub const INHERIT_ATTR: &str = "Inherit";

/// A string as an xpath literal: double quotes, or single quotes when the text holds a double quote. Text
/// with both kinds of quote cannot be written as a literal and is returned with the double quotes dropped
/// (such a def name is rejected by validation before it gets here).
#[must_use]
pub fn xpath_literal(text: &str) -> String {
    if !text.contains('"') {
        format!("\"{text}\"")
    } else if !text.contains('\'') {
        format!("'{text}'")
    } else {
        format!("\"{}\"", text.replace('"', ""))
    }
}

/// The xpath of a thing def by name: `Defs/ThingDef[defName="X"]`.
#[must_use]
pub fn thing_xpath(def_name: &str) -> String {
    format!("Defs/ThingDef[defName={}]", xpath_literal(def_name))
}

/// The xpath of a list entry that carries a `Class` attribute: `li[@Class="..."]`.
#[must_use]
pub fn class_li_xpath(class: &str) -> String {
    format!("li[@Class={}]", xpath_literal(class))
}

/// An `Operation` element with the given class, ready for children.
#[must_use]
pub fn operation(class: OpClass) -> NodeBuilder {
    NodeBuilder::new("Operation").attr("Class", class.as_str())
}

fn with_value(builder: NodeBuilder, value: Vec<Node>) -> Node {
    let mut v = Node::new("value");
    for n in value {
        v.push_child(n);
    }
    builder.child(v).build()
}

/// A `PatchOperationReplace` of the nodes at `xpath` by `value`.
#[must_use]
pub fn replace(xpath: &str, value: Vec<Node>) -> Node {
    with_value(operation(OpClass::Replace).text_elem("xpath", xpath), value)
}

/// A `PatchOperationAdd` of `value` into the nodes at `xpath`.
#[must_use]
pub fn add(xpath: &str, value: Vec<Node>) -> Node {
    with_value(operation(OpClass::Add).text_elem("xpath", xpath), value)
}

/// The ensure-container idiom: a conditional on `<def>/<container>` whose `nomatch` branch adds an empty
/// container to the def. When the container exists nothing happens and the operation still succeeds.
#[must_use]
pub fn ensure_container(def_xpath: &str, container: &str) -> Node {
    let mut nomatch = Node::new("nomatch");
    nomatch.set_attr("Class", OpClass::Add.as_str());
    nomatch.push_child(Node::with_text("xpath", def_xpath));
    let mut value = Node::new("value");
    value.push_child(Node::new(container));
    nomatch.push_child(value);
    operation(OpClass::Conditional)
        .text_elem("xpath", format!("{def_xpath}/{container}"))
        .child(nomatch)
        .build()
}

/// Runs `run` only when nothing matches `xpath`: a `PatchOperationConditional` whose `nomatch` branch is the
/// operation. It makes an operation that appends (a gun conversion) safe to apply twice: the second time the
/// xpath matches what the first application wrote and nothing happens.
#[must_use]
pub fn unless_present(xpath: &str, run: Node) -> Node {
    operation(OpClass::Conditional)
        .text_elem("xpath", xpath)
        .child(rename(run, "nomatch"))
        .build()
}

/// An operation with a `match` and a `nomatch` branch on one xpath.
#[must_use]
pub fn branch(xpath: &str, matched: Node, unmatched: Node) -> Node {
    operation(OpClass::Conditional)
        .text_elem("xpath", xpath)
        .child(rename(matched, "match"))
        .child(rename(unmatched, "nomatch"))
        .build()
}

/// A `FindMod` gate on a mod name (the exact `About` name), for files that hold vanilla classes only.
#[must_use]
pub fn find_mod(mod_name: &str, matched: Node) -> Node {
    let mut mods = Node::new("mods");
    mods.push_child(Node::with_text("li", mod_name));
    operation(OpClass::FindMod)
        .child(mods)
        .child(rename(matched, "match"))
        .build()
}

fn rename(mut node: Node, tag: &str) -> Node {
    tag.clone_into(&mut node.tag);
    node
}

/// Collects the operations for one def, tracking which containers exist or were ensured so that an ensure
/// operation is written once and never for a container that is there.
#[derive(Debug)]
pub struct DefOps<'a> {
    def_xpath: String,
    target: &'a Container,
    ensured: BTreeSet<String>,
    ops: Vec<Node>,
}

impl<'a> DefOps<'a> {
    /// A collector for the def `def_name` with the given target state.
    #[must_use]
    pub fn new(def_name: &str, target: &'a Container) -> Self {
        Self {
            def_xpath: thing_xpath(def_name),
            target,
            ensured: BTreeSet::new(),
            ops: Vec::new(),
        }
    }

    /// The xpath of the def.
    #[must_use]
    pub fn def_xpath(&self) -> &str {
        &self.def_xpath
    }

    /// The xpath of a path below the def.
    #[must_use]
    pub fn path(&self, below: &str) -> String {
        format!("{}/{below}", self.def_xpath)
    }

    /// True when the container is known to exist (in the target or by an earlier ensure).
    #[must_use]
    pub fn container_known(&self, container: &str) -> bool {
        self.target.has(container) || self.ensured.contains(container)
    }

    /// Writes the ensure operation for a container that may be missing.
    pub fn ensure(&mut self, container: &str) {
        if !self.container_known(container) {
            self.ops.push(ensure_container(&self.def_xpath, container));
            self.ensured.insert(container.to_owned());
        }
    }

    /// Adds nodes into a container (ensured first).
    pub fn add_into(&mut self, container: &str, value: Vec<Node>) {
        if value.is_empty() {
            return;
        }
        self.ensure(container);
        self.ops.push(add(&self.path(container), value));
    }

    /// Sets named entries of a list of named numbers (`statBases`, `equippedStatOffsets`): an entry that the
    /// target has is replaced; any other is set with a conditional (replace when it is there by then, add
    /// when it is not), so that applying the patch twice leaves one entry. Upserts come before replaces.
    pub fn set_entries(&mut self, container: &str, entries: &[(&str, String)]) {
        let mut upserts = Vec::new();
        let mut replaced = Vec::new();
        for (name, value) in entries {
            let node = Node::with_text(*name, value.clone());
            let entry = self.path(&format!("{container}/{name}"));
            if self.target.has_entry(container, name) {
                replaced.push(replace(&entry, vec![node]));
            } else {
                // The entry is not in the def as the generator read it, but another patch may have added it
                // since: replace it when it is there, add it when it is not, so a second application changes
                // nothing.
                let add_into = add(&self.path(container), vec![node.clone()]);
                upserts.push(branch(&entry, replace(&entry, vec![node]), add_into));
            }
        }
        if !upserts.is_empty() {
            self.ensure(container);
        }
        self.ops.extend(upserts);
        self.ops.extend(replaced);
    }

    /// Adds list entries (`<li>text</li>`) to a list below the def, each only when the list does not hold
    /// the text yet (ensured first).
    pub fn add_unique_li(&mut self, container: &str, texts: &[String]) {
        if texts.is_empty() {
            return;
        }
        self.ensure(container);
        for text in texts {
            let xpath = self.path(&format!("{container}/li[.={}]", xpath_literal(text)));
            let add_op = add(
                &self.path(container),
                vec![Node::with_text("li", text.clone())],
            );
            self.ops.push(unless_present(&xpath, add_op));
        }
    }

    /// Replaces a whole list below the def. When the def has no such list of its own, the list is added to
    /// the def with `Inherit="False"`, so that it replaces a list the def may inherit instead of merging
    /// with it.
    pub fn replace_list(&mut self, list: Node) {
        let tag = list.tag.clone();
        if self.container_known(&tag) {
            self.ops.push(replace(&self.path(&tag), vec![list]));
        } else {
            // The list is written with `Inherit="False"` in both branches, so that it replaces a list the def
            // may inherit, and a second application leaves the same element.
            let mut fresh = list;
            fresh.set_attr(INHERIT_ATTR, "False");
            let here = self.path(&tag);
            let def = self.def_xpath.clone();
            self.ops.push(branch(
                &here,
                replace(&here, vec![fresh.clone()]),
                add(&def, vec![fresh]),
            ));
            self.ensured.insert(tag);
        }
    }

    /// Pushes a finished operation.
    pub fn push(&mut self, op: Node) {
        self.ops.push(op);
    }

    /// The operations in order.
    #[must_use]
    pub fn finish(self) -> Vec<Node> {
        self.ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("RS_X", "\"RS_X\"")]
    #[case("a\"b", "'a\"b'")]
    fn literals(#[case] text: &str, #[case] want: &str) {
        assert_eq!(xpath_literal(text), want);
    }

    #[test]
    fn xpath_helpers_follow_the_conventions() {
        assert_eq!(thing_xpath("RS_X"), "Defs/ThingDef[defName=\"RS_X\"]");
        assert_eq!(class_li_xpath("A.B"), "li[@Class=\"A.B\"]");
    }

    #[test]
    fn ensure_is_written_once_and_only_when_needed() {
        let target = Container::unknown();
        let mut ops = DefOps::new("RS_X", &target);
        ops.add_into("statBases", vec![Node::with_text("Bulk", "2")]);
        ops.add_into("statBases", vec![Node::with_text("Mass", "1")]);
        let out = ops.finish();
        // one ensure, two adds
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].attr("Class"), Some("PatchOperationConditional"));
        assert_eq!(out[1].attr("Class"), Some("PatchOperationAdd"));

        let mut present = Container::unknown();
        present
            .containers
            .insert("statBases".into(), vec!["Mass".into()]);
        let mut ops = DefOps::new("RS_X", &present);
        ops.set_entries(
            "statBases",
            &[("Mass", "1".to_owned()), ("Bulk", "2".to_owned())],
        );
        let out = ops.finish();
        // the entry the def lacks is set with a conditional, the one it has is replaced
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].attr("Class"), Some("PatchOperationConditional"));
        assert_eq!(
            out[0].child_text("xpath"),
            Some("Defs/ThingDef[defName=\"RS_X\"]/statBases/Bulk")
        );
        assert_eq!(
            out[0].child("match").and_then(|m| m.attr("Class")),
            Some("PatchOperationReplace")
        );
        assert_eq!(
            out[0].child("nomatch").and_then(|m| m.attr("Class")),
            Some("PatchOperationAdd")
        );
        assert_eq!(out[1].attr("Class"), Some("PatchOperationReplace"));
        assert_eq!(
            out[1].child_text("xpath"),
            Some("Defs/ThingDef[defName=\"RS_X\"]/statBases/Mass")
        );
    }

    #[test]
    fn a_missing_list_is_replaced_or_added_with_inherit_false() {
        let target = Container::unknown();
        let mut ops = DefOps::new("RS_X", &target);
        ops.replace_list(Node::new("tools"));
        let out = ops.finish();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].attr("Class"), Some("PatchOperationConditional"));
        assert_eq!(
            out[0].child("match").and_then(|m| m.attr("Class")),
            Some("PatchOperationReplace")
        );
        let add = out[0].child("nomatch").unwrap();
        assert_eq!(add.attr("Class"), Some("PatchOperationAdd"));
        let value = add.child("value").unwrap();
        assert_eq!(value.child("tools").unwrap().attr("Inherit"), Some("False"));
    }

    #[test]
    fn a_list_the_def_has_is_replaced_plainly() {
        let mut target = Container::unknown();
        target.containers.insert("tools".into(), vec!["li".into()]);
        let mut ops = DefOps::new("RS_X", &target);
        ops.replace_list(Node::new("tools"));
        let out = ops.finish();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].attr("Class"), Some("PatchOperationReplace"));
    }

    #[test]
    fn list_entries_are_added_once_each() {
        let target = Container::unknown();
        let mut ops = DefOps::new("RS_X", &target);
        ops.add_unique_li("weaponTags", &["A".to_owned(), "B".to_owned()]);
        let out = ops.finish();
        // the ensure, then one guarded add per entry
        assert_eq!(out.len(), 3);
        assert_eq!(
            out[1].child_text("xpath"),
            Some("Defs/ThingDef[defName=\"RS_X\"]/weaponTags/li[.=\"A\"]")
        );
        assert_eq!(
            out[1].child("nomatch").and_then(|m| m.attr("Class")),
            Some("PatchOperationAdd")
        );
        assert!(out[1].child("match").is_none());
    }
}
