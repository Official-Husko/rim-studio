//! Shared proptest strategies for the tree tests (fictional names only).

use proptest::prelude::*;
use rimstudio_core::tree::{Child, Node};

const TAGS: [&str; 6] = ["a", "b", "li", "RS_Item", "x.y", "ns:t"];
const ATTRS: [&str; 5] = ["Class", "Name", "ParentName", "MayRequire", "xml:space"];

/// A valid attribute list without duplicate names.
pub(crate) fn attrs() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((0..ATTRS.len(), "[ -~\u{e9}\u{4e2d}\t\n]{0,6}"), 0..4).prop_map(|raw| {
        let mut out: Vec<(String, String)> = Vec::new();
        for (i, v) in raw {
            let name = ATTRS[i];
            if !out.iter().any(|(k, _)| k == name) {
                out.push((name.to_owned(), v));
            }
        }
        out
    })
}

/// A valid non empty text run.
pub(crate) fn text() -> impl Strategy<Value = String> {
    "[ -~\u{e9}\u{4e2d}\t\n<>&\"']{1,8}"
}

/// A valid tag name.
pub(crate) fn tag() -> impl Strategy<Value = String> {
    (0..TAGS.len()).prop_map(|i| TAGS[i].to_owned())
}

/// A valid element tree of bounded size.
pub(crate) fn node() -> impl Strategy<Value = Node> {
    let leaf = (tag(), attrs(), prop::option::of(text())).prop_map(|(tag, attrs, t)| Node {
        tag,
        attrs,
        children: t.into_iter().map(Child::Text).collect(),
    });
    leaf.prop_recursive(4, 40, 4, |inner| {
        (tag(), attrs(), prop::collection::vec(child_of(inner), 0..4)).prop_map(
            |(tag, attrs, children)| Node {
                tag,
                attrs,
                children,
            },
        )
    })
}

fn child_of(inner: impl Strategy<Value = Node> + 'static) -> impl Strategy<Value = Child> {
    prop_oneof![3 => inner.prop_map(Child::Element), 1 => text().prop_map(Child::Text)]
}
