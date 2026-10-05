//! Small helpers that read numbers, texts and lists out of resolved def nodes.
//!
//! Resolved defs are plain [`Node`] trees. The helpers never fail: a missing child, an empty element or text
//! that is not a finite number reads as `None`.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;

/// Parses a number the way the game's XML reader does for a float field: surrounding white space is ignored,
/// the text must be a finite decimal number.
#[must_use]
pub fn parse_number(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

/// The trimmed text of an element, `None` when it is empty.
#[must_use]
pub fn text_of(node: &Node) -> Option<String> {
    let text = node.text_content();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The trimmed text of the child element `tag`.
#[must_use]
pub fn child_text(node: &Node, tag: &str) -> Option<String> {
    text_of(node.child(tag)?)
}

/// The number in the child element `tag`.
#[must_use]
pub fn child_number(node: &Node, tag: &str) -> Option<f64> {
    parse_number(&node.child(tag)?.text_content())
}

/// The boolean in the child element `tag` (`true` or `false`, any case).
#[must_use]
pub fn child_bool(node: &Node, tag: &str) -> Option<bool> {
    match child_text(node, tag)?.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The texts of the `li` children of the child element `tag`, empty entries dropped.
#[must_use]
pub fn list_texts(node: &Node, tag: &str) -> Vec<String> {
    node.child(tag)
        .map(|list| {
            list.children_named("li")
                .filter_map(text_of)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// The numeric children of the child element `container` as a map from child name to value (a stat list such as
/// `statBases`). A repeated name keeps the first value, as the game's dictionary loader does for the first
/// occurrence. Children that are not numbers are skipped.
#[must_use]
pub fn number_map(node: &Node, container: &str) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if let Some(list) = node.child(container) {
        for entry in list.elements() {
            if let Some(v) = parse_number(&entry.text_content()) {
                out.entry(entry.tag.clone()).or_insert(v);
            }
        }
    }
    out
}

/// The `Class` attribute of an element.
#[must_use]
pub fn class_attr(node: &Node) -> Option<&str> {
    node.attr("Class")
}

/// The `li` elements of the child list `tag`.
#[must_use]
pub fn list_items<'a>(node: &'a Node, tag: &str) -> Vec<&'a Node> {
    node.child(tag)
        .map(|list| list.children_named("li").collect())
        .unwrap_or_default()
}

/// The names (child tags) of a list whose entries are named elements, such as `costList`, with their numbers.
#[must_use]
pub fn named_numbers(node: &Node, container: &str) -> Vec<(String, f64)> {
    node.child(container)
        .map(|list| {
            list.elements()
                .filter_map(|e| Some((e.tag.clone(), parse_number(&e.text_content())?)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    #[test]
    fn numbers_ignore_white_space_and_reject_non_finite_text() {
        assert_eq!(parse_number(" 2.5 \n"), Some(2.5));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("NaN"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number(""), None);
    }

    #[test]
    fn number_map_keeps_the_first_of_a_repeated_name() {
        let n = NodeBuilder::new("ThingDef")
            .elem("statBases", |s| {
                s.text_elem("Mass", "2")
                    .text_elem("Mass", "9")
                    .text_elem("Bad", "x")
            })
            .build();
        let m = number_map(&n, "statBases");
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("Mass"), Some(&2.0));
        assert!(number_map(&n, "missing").is_empty());
    }

    #[test]
    fn lists_and_booleans() {
        let n = NodeBuilder::new("ThingDef")
            .elem("weaponTags", |t| t.li("RS_A").li(" ").li("RS_B"))
            .text_elem("menuHidden", "True")
            .build();
        assert_eq!(list_texts(&n, "weaponTags"), vec!["RS_A", "RS_B"]);
        assert_eq!(child_bool(&n, "menuHidden"), Some(true));
        assert_eq!(child_bool(&n, "other"), None);
        assert!(list_texts(&n, "none").is_empty());
    }
}
