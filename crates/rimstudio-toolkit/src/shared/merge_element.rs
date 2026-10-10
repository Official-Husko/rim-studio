//! Merging a regenerated definition into the element of the same name that a file already has.
//!
//! A definition that a person wrote has comments, an order of its members and a number format of its own.
//! Replacing the whole element with a freshly rendered one would move members around and rewrite `0.80` as
//! `0.8` even when nothing changed. [`replace_in_place`] edits only what differs in meaning: a changed leaf
//! text is replaced in place, a member the new element has and the old one lacks is appended, a member the
//! new element dropped is removed, and everything else keeps its bytes. The result is checked: when the
//! merged element does not mean what the new one means, the whole element is replaced instead, so the file
//! never ends up with a definition that differs from the design.
//!
//! Meaning ([`same_meaning`]) ignores comments, white space around text, the order of the members of one
//! element, and number formatting. The order of the entries of a list (`li`) and the values count.

use rimstudio_core::tree::Node;
use rimstudio_xml::edit::SpanEditor;

/// The relative tolerance of two numbers that are the same value written differently.
const NUMBER_EPS: f64 = 1e-9;

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

/// True when two texts are equal after trimming, or are the same number.
fn same_text(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a == b {
        return true;
    }
    match (number(a), number(b)) {
        (Some(x), Some(y)) => (x - y).abs() <= NUMBER_EPS * x.abs().max(y.abs()).max(1.0),
        _ => false,
    }
}

/// True when two elements mean the same: the same tag and attributes, the same members (by tag, in order
/// among members of one tag, in any order between tags) and the same texts.
#[must_use]
pub fn same_meaning(a: &Node, b: &Node) -> bool {
    if a.tag != b.tag
        || a.attrs.len() != b.attrs.len()
        || !a.attrs.iter().all(|(k, v)| b.attr(k) == Some(v.as_str()))
    {
        return false;
    }
    let ea: Vec<&Node> = a.elements().collect();
    let eb: Vec<&Node> = b.elements().collect();
    if ea.is_empty() && eb.is_empty() {
        return same_text(&a.text_content(), &b.text_content());
    }
    if ea.len() != eb.len() {
        return false;
    }
    let mut used = vec![false; eb.len()];
    for x in ea {
        let Some(at) = eb
            .iter()
            .enumerate()
            .position(|(i, y)| !used[i] && y.tag == x.tag)
        else {
            return false;
        };
        used[at] = true;
        if !same_meaning(x, eb[at]) {
            return false;
        }
    }
    true
}

/// The element at `path` as a node tree: its tag and attributes, the child elements in order, and the text
/// of an element without child elements.
///
/// # Errors
///
/// The reason when the path does not address an element.
pub fn node_at(ed: &SpanEditor, path: &str) -> Result<Node, String> {
    let info = ed.element(path).map_err(|e| e.to_string())?;
    let mut node = Node::new(info.tag);
    node.attrs = info.attrs;
    if info.child_count == 0 {
        if !info.text.is_empty() {
            node.push_text(info.text);
        }
    } else {
        for kid in ed.children(path).map_err(|e| e.to_string())? {
            node.push_child(node_at(ed, &kid.path)?);
        }
    }
    Ok(node)
}

fn edit_error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Brings the element at `path` (read as `old`) to the meaning of `new` by small edits. Returns an error
/// for a change it does not edit in place (a leaf that became a container or the other way round); the
/// caller replaces the element then.
fn merge_children(ed: &mut SpanEditor, path: &str, old: &Node, new: &Node) -> Result<(), String> {
    for (name, value) in &new.attrs {
        if old.attr(name) != Some(value.as_str()) {
            ed.set_attr(path, name, value).map_err(edit_error)?;
        }
    }
    for (name, _) in &old.attrs {
        if new.attr(name).is_none() {
            ed.remove_attr(path, name).map_err(edit_error)?;
        }
    }
    let old_kids: Vec<&Node> = old.elements().collect();
    let new_kids: Vec<&Node> = new.elements().collect();
    if old_kids.is_empty() && new_kids.is_empty() {
        let text = new.text_content();
        if !same_text(&old.text_content(), &text) {
            ed.replace_text(path, &text).map_err(edit_error)?;
        }
        return Ok(());
    }
    if old_kids.is_empty() || new_kids.is_empty() {
        return Err("the shape of the element changed".to_owned());
    }
    // pair the members by tag, in order within one tag
    let mut taken = vec![false; old_kids.len()];
    let mut pairs: Vec<(&Node, Option<usize>)> = Vec::with_capacity(new_kids.len());
    for nk in &new_kids {
        let at = old_kids
            .iter()
            .enumerate()
            .position(|(i, ok)| !taken[i] && ok.tag == nk.tag);
        if let Some(i) = at {
            taken[i] = true;
        }
        pairs.push((nk, at));
    }
    let paths: Vec<String> = ed
        .children(path)
        .map_err(edit_error)?
        .into_iter()
        .map(|c| c.path)
        .collect();
    if paths.len() != old_kids.len() {
        return Err("the element changed under the merge".to_owned());
    }
    // 1. members both have: the count of members of this element does not change, so the paths hold
    for (nk, at) in &pairs {
        let Some(i) = at else { continue };
        let (ok, p) = (old_kids[*i], &paths[*i]);
        if same_meaning(ok, nk) {
            continue;
        }
        if merge_children(ed, p, ok, nk).is_err() {
            ed.replace_element(p, nk).map_err(edit_error)?;
        }
    }
    // 2. members the new element dropped, last first so the paths of the others hold
    for (i, taken) in taken.iter().enumerate().rev() {
        if !taken {
            ed.remove(&paths[i]).map_err(edit_error)?;
        }
    }
    // 3. members that are new go at the end, in the order the new element has them
    for (nk, at) in &pairs {
        if at.is_none() {
            ed.insert_child(path, nk).map_err(edit_error)?;
        }
    }
    Ok(())
}

/// Replaces the element at `path` with `new`, editing only what differs in meaning when it can and
/// replacing the whole element when it cannot. Nothing is touched when the element already means `new`.
///
/// # Errors
///
/// The reason when the path does not address an element or `new` has a name that is not an XML name.
pub fn replace_in_place(ed: &mut SpanEditor, path: &str, new: &Node) -> Result<(), String> {
    let old = node_at(ed, path)?;
    if same_meaning(&old, new) {
        return Ok(());
    }
    let before = ed.clone();
    if merge_children(ed, path, &old, new).is_ok()
        && node_at(ed, path).is_ok_and(|merged| same_meaning(&merged, new))
    {
        return Ok(());
    }
    *ed = before;
    ed.replace_element(path, new).map_err(edit_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    fn def(label: &str, range: &str) -> Node {
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .text_elem("label", label)
            .elem("verbs", |v| v.elem("li", |l| l.text_elem("range", range)))
            .build()
    }

    fn editor(text: &str) -> SpanEditor {
        SpanEditor::open(text).unwrap()
    }

    #[test]
    fn meaning_ignores_order_between_members_and_number_formatting() {
        let a = def("rifle", "44.0");
        let b = NodeBuilder::new("ThingDef")
            .elem("verbs", |v| v.elem("li", |l| l.text_elem("range", "44")))
            .text_elem("label", " rifle ")
            .text_elem("defName", "RS_A")
            .build();
        assert!(same_meaning(&a, &b));
        assert!(!same_meaning(&a, &def("rifle", "45")));
        assert!(!same_meaning(&a, &def("pistol", "44")));
    }

    #[test]
    fn list_entries_keep_their_order() {
        let ab = NodeBuilder::new("t")
            .elem("tags", |t| t.li("A").li("B"))
            .build();
        let ba = NodeBuilder::new("t")
            .elem("tags", |t| t.li("B").li("A"))
            .build();
        assert!(!same_meaning(&ab, &ba));
    }

    #[test]
    fn a_def_that_already_means_the_new_one_is_not_touched() {
        let text = "<Defs>\n  <!-- mine -->\n  <ThingDef>\n    <verbs><li><range>44.0</range></li></verbs>\n    <label>rifle</label>\n    <defName>RS_A</defName>\n  </ThingDef>\n</Defs>\n";
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &def("rifle", "44")).unwrap();
        assert_eq!(ed.text(), text);
    }

    #[test]
    fn only_the_changed_leaf_is_edited_and_the_rest_keeps_its_bytes() {
        let text = "<Defs>\n  <ThingDef>\n    <!-- the name -->\n    <defName>RS_A</defName>\n    <verbs><li><range>44.0</range></li></verbs>\n    <label>rifle</label>\n  </ThingDef>\n</Defs>\n";
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &def("rifle", "52")).unwrap();
        assert_eq!(ed.text(), text.replace("44.0", "52"));
    }

    #[test]
    fn a_new_member_is_appended_and_a_dropped_member_is_removed() {
        let text = "<Defs>\n  <ThingDef>\n    <defName>RS_A</defName>\n    <label>rifle</label>\n    <description>old text</description>\n  </ThingDef>\n</Defs>\n";
        let new = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .text_elem("label", "rifle")
            .text_elem("techLevel", "Industrial")
            .build();
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &new).unwrap();
        let merged = ed.text();
        assert!(!merged.contains("description"), "{merged}");
        assert!(
            merged.contains("<techLevel>Industrial</techLevel>"),
            "{merged}"
        );
        assert!(merged.contains("<defName>RS_A</defName>\n    <label>rifle</label>"));
        let after = node_at(&ed, "/Defs/ThingDef").unwrap();
        assert!(same_meaning(&after, &new));
    }

    #[test]
    fn a_list_grows_and_shrinks_at_its_end() {
        let text = "<Defs>\n  <ThingDef>\n    <defName>RS_A</defName>\n    <tags>\n      <li>A</li>\n      <li>B</li>\n      <li>C</li>\n    </tags>\n  </ThingDef>\n</Defs>\n";
        let two = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .elem("tags", |t| t.li("A").li("B"))
            .build();
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &two).unwrap();
        assert!(!ed.text().contains("<li>C</li>"));
        assert!(ed.text().contains("<li>B</li>"));
        let four = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .elem("tags", |t| t.li("A").li("B").li("C").li("D"))
            .build();
        replace_in_place(&mut ed, "/Defs/ThingDef", &four).unwrap();
        assert!(ed.text().contains("<li>D</li>"));
        assert!(same_meaning(
            &node_at(&ed, "/Defs/ThingDef").unwrap(),
            &four
        ));
    }

    #[test]
    fn a_leaf_that_became_a_container_is_replaced_as_a_whole() {
        let text = "<Defs>\n  <ThingDef>\n    <defName>RS_A</defName>\n    <graphic>Things/A</graphic>\n  </ThingDef>\n</Defs>\n";
        let new = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .elem("graphic", |g| g.text_elem("texPath", "Things/A"))
            .build();
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &new).unwrap();
        assert!(same_meaning(&node_at(&ed, "/Defs/ThingDef").unwrap(), &new));
        assert!(ed.text().contains("<texPath>Things/A</texPath>"));
    }

    #[test]
    fn attributes_are_set_and_removed() {
        let text = "<Defs>\n  <ThingDef ParentName=\"Old\" Class=\"X\">\n    <defName>RS_A</defName>\n  </ThingDef>\n</Defs>\n";
        let mut new = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_A")
            .build();
        new.set_attr("ParentName", "New");
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef", &new).unwrap();
        assert!(ed.text().contains("ParentName=\"New\""));
        assert!(!ed.text().contains("Class="));
    }

    #[test]
    fn the_second_element_of_two_is_addressed_by_its_path() {
        let text = "<Defs>\n  <ThingDef>\n    <defName>RS_A</defName>\n    <label>a</label>\n  </ThingDef>\n  <ThingDef>\n    <defName>RS_B</defName>\n    <label>b</label>\n  </ThingDef>\n</Defs>\n";
        let new = NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_B")
            .text_elem("label", "changed")
            .build();
        let mut ed = editor(text);
        replace_in_place(&mut ed, "/Defs/ThingDef[2]", &new).unwrap();
        assert!(ed.text().contains("<label>a</label>"));
        assert!(ed.text().contains("<label>changed</label>"));
    }
}
