//! Fluent construction of [`Node`] trees for templates and the designer.

use super::error::TreeError;
use super::node::{Child, Node};

/// Builds a [`Node`] step by step.
///
/// ```
/// use rimstudio_core::tree::NodeBuilder;
/// let node = NodeBuilder::new("ThingDef")
///     .attr("ParentName", "RS_BaseGun")
///     .text_elem("defName", "RS_TestRifle")
///     .elem("statBases", |b| b.text_elem("Mass", "3.5"))
///     .build();
/// assert_eq!(node.child_text("defName"), Some("RS_TestRifle"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeBuilder {
    node: Node,
}

impl NodeBuilder {
    /// Starts an element with the given tag.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            node: Node::new(tag),
        }
    }

    /// Sets an attribute. A repeated name replaces the earlier value in place.
    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.node.set_attr(name, value);
        self
    }

    /// Sets an attribute only when the value is present.
    #[must_use]
    pub fn attr_opt(self, name: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(v) => self.attr(name, v),
            None => self,
        }
    }

    /// Sets several attributes in order.
    #[must_use]
    pub fn attrs<K: Into<String>, V: Into<String>>(
        mut self,
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        for (k, v) in pairs {
            self.node.set_attr(k, v);
        }
        self
    }

    /// Appends a text node.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.node.children.push(Child::Text(text.into()));
        self
    }

    /// Appends any child (element, builder or text).
    #[must_use]
    pub fn child(mut self, child: impl Into<Child>) -> Self {
        self.node.children.push(child.into());
        self
    }

    /// Appends several children.
    #[must_use]
    pub fn children<C: Into<Child>>(mut self, children: impl IntoIterator<Item = C>) -> Self {
        self.node
            .children
            .extend(children.into_iter().map(Into::into));
        self
    }

    /// Appends `<tag>text</tag>`.
    #[must_use]
    pub fn text_elem(self, tag: impl Into<String>, text: impl Into<String>) -> Self {
        self.child(Node::with_text(tag, text))
    }

    /// Appends `<tag>text</tag>` only when the value is present.
    #[must_use]
    pub fn text_elem_opt(self, tag: impl Into<String>, text: Option<impl Into<String>>) -> Self {
        match text {
            Some(t) => self.text_elem(tag, t),
            None => self,
        }
    }

    /// Appends an empty element `<tag/>`.
    #[must_use]
    pub fn empty_elem(self, tag: impl Into<String>) -> Self {
        self.child(Node::new(tag))
    }

    /// Appends `<tag>` whose content is produced by `build`.
    #[must_use]
    pub fn elem(
        self,
        tag: impl Into<String>,
        build: impl FnOnce(NodeBuilder) -> NodeBuilder,
    ) -> Self {
        self.child(build(NodeBuilder::new(tag)))
    }

    /// Appends `<li>text</li>`.
    #[must_use]
    pub fn li(self, text: impl Into<String>) -> Self {
        self.text_elem("li", text)
    }

    /// Appends one `<li>` per item.
    #[must_use]
    pub fn li_each<S: Into<String>>(mut self, items: impl IntoIterator<Item = S>) -> Self {
        for item in items {
            self.node
                .children
                .push(Child::Element(Node::with_text("li", item)));
        }
        self
    }

    /// Applies `build` only when `condition` holds.
    #[must_use]
    pub fn when(self, condition: bool, build: impl FnOnce(NodeBuilder) -> NodeBuilder) -> Self {
        if condition { build(self) } else { self }
    }

    /// The finished node (not validated).
    #[must_use]
    pub fn build(self) -> Node {
        self.node
    }

    /// The finished node after [`Node::validate`].
    ///
    /// # Errors
    /// The first validation failure.
    pub fn try_build(self) -> Result<Node, TreeError> {
        self.node.validate()?;
        Ok(self.node)
    }
}

impl From<NodeBuilder> for Node {
    fn from(builder: NodeBuilder) -> Self {
        builder.build()
    }
}
