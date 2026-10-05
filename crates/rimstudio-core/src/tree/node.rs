//! The owned node tree: the JSON serialisable form of every XML document in RimStudio.
//!
//! JSON shape (identical to the def engine vectors):
//!
//! ```json
//! {"tag": "li", "attrs": [["Class", "RS_Comp"]], "children": [{"tag": "x", "attrs": [], "children": ["text"]}]}
//! ```
//!
//! `attrs` is an ordered list of `[name, value]` pairs (order is observable), `children` mixes
//! element objects and text strings. On read `attrs` and `children` may be absent; on write both
//! are always present. Unknown fields are rejected.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::error::{TreeError, coded_message};
use super::names::{check_name, first_invalid_char};
use super::{MAX_DEPTH, NodeBuilder};

/// An XML element: a tag, ordered attributes and ordered children.
///
/// Equality and hashing are structural and order sensitive (attributes and children). Names
/// and text are not validated on construction; call [`Node::validate`] on untrusted trees.
/// JSON decoding validates as it goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Node {
    /// Element name.
    pub tag: String,
    /// Attributes in source order, as `(name, value)` pairs.
    pub attrs: Vec<(String, String)>,
    /// Child elements and text, in document order.
    pub children: Vec<Child>,
}

/// A child of an element: another element or a text run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(untagged)]
pub enum Child {
    /// A nested element.
    Element(Node),
    /// A text node.
    Text(String),
}

impl From<Node> for Child {
    fn from(node: Node) -> Self {
        Self::Element(node)
    }
}

impl From<NodeBuilder> for Child {
    fn from(builder: NodeBuilder) -> Self {
        Self::Element(builder.build())
    }
}

impl From<String> for Child {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for Child {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl Child {
    /// The element, if this child is one.
    #[must_use]
    pub fn as_element(&self) -> Option<&Node> {
        match self {
            Self::Element(n) => Some(n),
            Self::Text(_) => None,
        }
    }

    /// The element, mutably, if this child is one.
    pub fn as_element_mut(&mut self) -> Option<&mut Node> {
        match self {
            Self::Element(n) => Some(n),
            Self::Text(_) => None,
        }
    }

    /// The text, if this child is a text node.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Element(_) => None,
            Self::Text(t) => Some(t),
        }
    }

    /// Consumes the child and returns the element, if it is one.
    #[must_use]
    pub fn into_element(self) -> Option<Node> {
        match self {
            Self::Element(n) => Some(n),
            Self::Text(_) => None,
        }
    }

    /// True for text nodes.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }
}

impl Node {
    /// An element with the given tag and no attributes or children.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Starts a fluent builder for an element with the given tag.
    #[must_use]
    pub fn builder(tag: impl Into<String>) -> NodeBuilder {
        NodeBuilder::new(tag)
    }

    /// An element whose only child is the given text.
    #[must_use]
    pub fn with_text(tag: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attrs: Vec::new(),
            children: vec![Child::Text(text.into())],
        }
    }

    // ----------------------------------------------------------------- attributes

    /// The value of the attribute `name`, if present (case sensitive).
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// True when the attribute `name` exists.
    #[must_use]
    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|(k, _)| k == name)
    }

    /// Sets an attribute: replaces the value in place (keeping its position) or appends a new
    /// pair. Returns the previous value.
    pub fn set_attr(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<String> {
        let name = name.into();
        let value = value.into();
        if let Some((_, v)) = self.attrs.iter_mut().find(|(k, _)| *k == name) {
            Some(std::mem::replace(v, value))
        } else {
            self.attrs.push((name, value));
            None
        }
    }

    /// Removes the attribute `name` and returns its value.
    pub fn remove_attr(&mut self, name: &str) -> Option<String> {
        let pos = self.attrs.iter().position(|(k, _)| k == name)?;
        Some(self.attrs.remove(pos).1)
    }

    // ----------------------------------------------------------------- children

    /// Appends a child element.
    pub fn push_child(&mut self, child: impl Into<Node>) {
        self.children.push(Child::Element(child.into()));
    }

    /// Appends a text node.
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.children.push(Child::Text(text.into()));
    }

    /// The child elements in order, skipping text.
    pub fn elements(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(Child::as_element)
    }

    /// The child elements in order, mutably.
    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.children.iter_mut().filter_map(Child::as_element_mut)
    }

    /// The first child element with the given tag.
    #[must_use]
    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.elements().find(|n| n.tag == tag)
    }

    /// The first child element with the given tag, mutably.
    pub fn child_mut(&mut self, tag: &str) -> Option<&mut Node> {
        self.elements_mut().find(|n| n.tag == tag)
    }

    /// All child elements with the given tag, in order.
    pub fn children_named<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> {
        self.elements().filter(move |n| n.tag == tag)
    }

    /// Removes and returns the first child element with the given tag.
    pub fn remove_child(&mut self, tag: &str) -> Option<Node> {
        let pos = self
            .children
            .iter()
            .position(|c| c.as_element().is_some_and(|n| n.tag == tag))?;
        self.children.remove(pos).into_element()
    }

    /// The text of an element that has exactly one child and that child is text.
    #[must_use]
    pub fn leaf_text(&self) -> Option<&str> {
        match self.children.as_slice() {
            [Child::Text(t)] => Some(t),
            _ => None,
        }
    }

    /// [`Node::leaf_text`] of the first child element named `tag`, for the common
    /// `<defName>RS_X</defName>` lookups.
    #[must_use]
    pub fn child_text(&self, tag: &str) -> Option<&str> {
        self.child(tag).and_then(Node::leaf_text)
    }

    /// Sets the text of the first child element `tag`, replacing its children, or appends a new
    /// child element when none exists.
    pub fn set_child_text(&mut self, tag: &str, text: impl Into<String>) {
        let text = Child::Text(text.into());
        match self.child_mut(tag) {
            Some(n) => n.children = vec![text],
            None => self.children.push(Child::Element(Node {
                tag: tag.to_owned(),
                attrs: Vec::new(),
                children: vec![text],
            })),
        }
    }

    /// The XPath string value: the concatenation of every descendant text node.
    #[must_use]
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<std::slice::Iter<'_, Child>> = vec![self.children.iter()];
        while let Some(top) = stack.last_mut() {
            match top.next() {
                Some(Child::Text(t)) => out.push_str(t),
                Some(Child::Element(n)) => stack.push(n.children.iter()),
                None => {
                    stack.pop();
                }
            }
        }
        out
    }

    // ----------------------------------------------------------------- paths

    /// Finds the first descendant addressed by a simple slash separated path.
    ///
    /// Each segment is an element name or `*`, optionally followed by a 1-based index among the
    /// matching siblings: `comps/li[2]/compClass`. Empty segments are ignored, the empty path
    /// addresses `self`. A malformed segment matches nothing. When an earlier segment matches
    /// several siblings, the search backtracks, so the result is the first match in document order.
    #[must_use]
    pub fn find(&self, path: &str) -> Option<&Node> {
        let p = self.find_path(path)?;
        self.at_path(&p)
    }

    /// Mutable form of [`Node::find`].
    pub fn find_mut(&mut self, path: &str) -> Option<&mut Node> {
        let p = self.find_path(path)?;
        let mut cur = self;
        for i in p {
            cur = match cur.children.get_mut(i) {
                Some(Child::Element(n)) => n,
                _ => return None,
            };
        }
        Some(cur)
    }

    /// Every node addressed by the path (see [`Node::find`]), in document order.
    #[must_use]
    pub fn find_all(&self, path: &str) -> Vec<&Node> {
        let Some(segs) = parse_path(path) else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        collect_paths(self, &segs, &mut Vec::new(), &mut paths, false);
        paths.iter().filter_map(|p| self.at_path(p)).collect()
    }

    fn find_path(&self, path: &str) -> Option<Vec<usize>> {
        let segs = parse_path(path)?;
        let mut paths = Vec::new();
        collect_paths(self, &segs, &mut Vec::new(), &mut paths, true);
        paths.into_iter().next()
    }

    fn at_path(&self, path: &[usize]) -> Option<&Node> {
        let mut cur = self;
        for &i in path {
            cur = cur.children.get(i)?.as_element()?;
        }
        Some(cur)
    }

    // ----------------------------------------------------------------- metrics

    /// Number of element nodes in this subtree, including `self`.
    #[must_use]
    pub fn element_count(&self) -> usize {
        let mut count = 0;
        let mut stack = vec![self];
        while let Some(n) = stack.pop() {
            count += 1;
            stack.extend(n.elements());
        }
        count
    }

    /// Nesting depth: 1 for a node without element children.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut best = 0;
        let mut stack = vec![(self, 1usize)];
        while let Some((n, d)) = stack.pop() {
            best = best.max(d);
            stack.extend(n.elements().map(|c| (c, d.saturating_add(1))));
        }
        best
    }

    // ----------------------------------------------------------------- validation

    /// Checks names, attribute uniqueness, text characters and depth for the whole subtree.
    ///
    /// # Errors
    /// The first violation found in document order, with the location of the node.
    pub fn validate(&self) -> Result<(), TreeError> {
        let mut stack: Vec<(&Node, usize)> = Vec::new();
        check_local(self, &stack)?;
        stack.push((self, 0));
        while let Some(top) = stack.last_mut() {
            let (node, cursor) = (top.0, top.1);
            top.1 += 1;
            match node.children.get(cursor) {
                None => {
                    stack.pop();
                }
                Some(Child::Text(t)) => {
                    if let Some(ch) = first_invalid_char(t) {
                        return Err(TreeError::InvalidChar {
                            path: path_of(&stack),
                            ch: u32::from(ch),
                        });
                    }
                }
                Some(Child::Element(n)) => {
                    check_local(n, &stack)?;
                    stack.push((n, 0));
                }
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------------- JSON

    /// Parses and validates a node from JSON text.
    ///
    /// Note that `serde_json` limits nesting to 128 JSON levels, which is about 60 element levels
    /// (an element costs an object and an array).
    ///
    /// # Errors
    /// [`TreeError::Json`] with code `tree.json` for syntax or shape errors, or the specific
    /// validation code (`tree.invalid_name`, `tree.duplicate_attr`, `tree.invalid_char`).
    pub fn from_json_str(json: &str) -> Result<Self, TreeError> {
        serde_json::from_str(json).map_err(|e| TreeError::from_json_error(&e))
    }

    /// Parses and validates a node from a JSON value.
    ///
    /// # Errors
    /// See [`Node::from_json_str`].
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, TreeError> {
        serde_json::from_value(value).map_err(|e| TreeError::from_json_error(&e))
    }

    /// Compact JSON text.
    #[must_use]
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Indented JSON text, for files and diffs.
    #[must_use]
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The JSON value form.
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// A deterministic 32 byte BLAKE3 digest of the structure (tags, attributes in order,
    /// children in order, text). Equal trees give equal digests in every process and release.
    #[must_use]
    pub fn content_hash(&self) -> [u8; 32] {
        fn put_str(h: &mut blake3::Hasher, s: &str) {
            h.update(&(s.len() as u64).to_le_bytes());
            h.update(s.as_bytes());
        }
        fn open(h: &mut blake3::Hasher, n: &Node) {
            h.update(&[1]);
            put_str(h, &n.tag);
            h.update(&(n.attrs.len() as u64).to_le_bytes());
            for (k, v) in &n.attrs {
                put_str(h, k);
                put_str(h, v);
            }
            h.update(&(n.children.len() as u64).to_le_bytes());
        }
        let mut h = blake3::Hasher::new();
        open(&mut h, self);
        let mut stack: Vec<std::slice::Iter<'_, Child>> = vec![self.children.iter()];
        while let Some(top) = stack.last_mut() {
            match top.next() {
                Some(Child::Text(t)) => {
                    h.update(&[2]);
                    put_str(&mut h, t);
                }
                Some(Child::Element(n)) => {
                    open(&mut h, n);
                    stack.push(n.children.iter());
                }
                None => {
                    stack.pop();
                }
            }
        }
        *h.finalize().as_bytes()
    }
}

impl fmt::Display for Node {
    /// Compact JSON.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json_string())
    }
}

// --------------------------------------------------------------------- validation helpers

fn check_local(node: &Node, stack: &[(&Node, usize)]) -> Result<(), TreeError> {
    let here = || {
        let mut p = path_of(stack);
        if !p.is_empty() {
            p.push('/');
        }
        p.push_str(&node.tag);
        p
    };
    if stack.len() >= MAX_DEPTH {
        return Err(TreeError::TooDeep {
            path: here(),
            max: MAX_DEPTH,
        });
    }
    if let Err(reason) = check_name(&node.tag) {
        return Err(TreeError::InvalidName {
            path: here(),
            name: node.tag.clone(),
            reason,
        });
    }
    check_attrs(&node.attrs).map_err(|e| match e {
        AttrProblem::Name(name, reason) => TreeError::InvalidName {
            path: here(),
            name,
            reason,
        },
        AttrProblem::Duplicate(name) => TreeError::DuplicateAttr { path: here(), name },
        AttrProblem::Char(ch) => TreeError::InvalidChar {
            path: here(),
            ch: u32::from(ch),
        },
    })
}

enum AttrProblem {
    Name(String, &'static str),
    Duplicate(String),
    Char(char),
}

fn check_attrs(attrs: &[(String, String)]) -> Result<(), AttrProblem> {
    for (i, (k, v)) in attrs.iter().enumerate() {
        if let Err(reason) = check_name(k) {
            return Err(AttrProblem::Name(k.clone(), reason));
        }
        if let Some(c) = first_invalid_char(v) {
            return Err(AttrProblem::Char(c));
        }
        let earlier = attrs.get(..i).unwrap_or_default();
        if earlier.iter().any(|(k2, _)| k2 == k) {
            return Err(AttrProblem::Duplicate(k.clone()));
        }
    }
    Ok(())
}

/// Location string of the node on top of the walk stack, `Defs/ThingDef[2]/label` style.
fn path_of(stack: &[(&Node, usize)]) -> String {
    let mut out = String::new();
    for (i, (node, _)) in stack.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(&node.tag);
        if let Some((parent, cursor)) = i.checked_sub(1).and_then(|p| stack.get(p)) {
            let upto = cursor.saturating_sub(1);
            let nth = parent
                .children
                .iter()
                .take(upto.saturating_add(1))
                .filter(|c| c.as_element().is_some_and(|n| n.tag == node.tag))
                .count();
            if nth > 1 {
                out.push_str(&format!("[{nth}]"));
            }
        }
    }
    out
}

// --------------------------------------------------------------------- simple path support

struct Segment<'a> {
    name: &'a str,
    index: Option<usize>,
}

fn parse_path(path: &str) -> Option<Vec<Segment<'_>>> {
    let mut out = Vec::new();
    for raw in path.split('/').filter(|s| !s.is_empty()) {
        let (name, index) = match raw.split_once('[') {
            Some((name, rest)) => {
                let digits = rest.strip_suffix(']')?;
                let n: usize = digits.parse().ok()?;
                if n == 0 {
                    return None;
                }
                (name, Some(n))
            }
            None => (raw, None),
        };
        if name.is_empty() {
            return None;
        }
        out.push(Segment { name, index });
    }
    Some(out)
}

/// Depth first search for every child index path matching `segs`; stops at the first when
/// `first_only` is set.
fn collect_paths(
    node: &Node,
    segs: &[Segment<'_>],
    cur: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
    first_only: bool,
) {
    let Some((seg, rest)) = segs.split_first() else {
        out.push(cur.clone());
        return;
    };
    let mut nth = 0usize;
    for (i, child) in node.children.iter().enumerate() {
        let Child::Element(n) = child else { continue };
        if seg.name != "*" && n.tag != seg.name {
            continue;
        }
        nth += 1;
        if seg.index.is_some_and(|want| want != nth) {
            continue;
        }
        cur.push(i);
        collect_paths(n, rest, cur, out, first_only);
        cur.pop();
        if first_only && !out.is_empty() {
            return;
        }
    }
}

// --------------------------------------------------------------------- deserialisation

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(NodeVisitor)
    }
}

impl<'de> Deserialize<'de> for Child {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ChildVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a node object {\"tag\": string, \"attrs\": [[name, value]], \"children\": [node or string]}")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut tag: Option<String> = None;
        let mut attrs: Option<Vec<(String, String)>> = None;
        let mut children: Option<Vec<Child>> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "tag" => {
                    if tag.is_some() {
                        return Err(de::Error::duplicate_field("tag"));
                    }
                    tag = Some(map.next_value()?);
                }
                "attrs" => {
                    if attrs.is_some() {
                        return Err(de::Error::duplicate_field("attrs"));
                    }
                    attrs = Some(map.next_value()?);
                }
                "children" => {
                    if children.is_some() {
                        return Err(de::Error::duplicate_field("children"));
                    }
                    children = Some(map.next_value()?);
                }
                _ => {
                    return Err(de::Error::unknown_field(
                        &key,
                        &["tag", "attrs", "children"],
                    ));
                }
            }
        }
        let tag = tag.ok_or_else(|| de::Error::missing_field("tag"))?;
        let attrs = attrs.unwrap_or_default();
        if let Err(reason) = check_name(&tag) {
            return Err(de::Error::custom(coded_message(
                "tree.invalid_name",
                &format!("element name {tag:?}: {reason}"),
            )));
        }
        check_attrs(&attrs).map_err(|e| {
            de::Error::custom(match e {
                AttrProblem::Name(n, r) => coded_message(
                    "tree.invalid_name",
                    &format!("attribute name {n:?} on <{tag}>: {r}"),
                ),
                AttrProblem::Duplicate(n) => coded_message(
                    "tree.duplicate_attr",
                    &format!("attribute {n:?} repeated on <{tag}>"),
                ),
                AttrProblem::Char(c) => coded_message(
                    "tree.invalid_char",
                    &format!("U+{:04X} in an attribute value on <{tag}>", u32::from(c)),
                ),
            })
        })?;
        Ok(Node {
            tag,
            attrs,
            children: children.unwrap_or_default(),
        })
    }
}

struct ChildVisitor;

impl<'de> Visitor<'de> for ChildVisitor {
    type Value = Child;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a node object or a text string")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Child, E> {
        if let Some(c) = first_invalid_char(v) {
            return Err(E::custom(coded_message(
                "tree.invalid_char",
                &format!("U+{:04X} in a text node", u32::from(c)),
            )));
        }
        Ok(Child::Text(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Child, E> {
        if let Some(c) = first_invalid_char(&v) {
            return Err(E::custom(coded_message(
                "tree.invalid_char",
                &format!("U+{:04X} in a text node", u32::from(c)),
            )));
        }
        Ok(Child::Text(v))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Child, A::Error> {
        NodeVisitor.visit_map(map).map(Child::Element)
    }
}
