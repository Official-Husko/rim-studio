//! Values of the XPath data model: node-sets, strings, numbers and booleans.

use rimstudio_core::tree::NodeId;

/// A node of the XPath data model.
///
/// The arena stores elements, text and the document node; attributes live inside their element
/// and have no handle of their own, so an attribute node is the pair of its owner and its
/// position in the owner's attribute list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XNode {
    /// The document node, an element or a text node.
    Node(NodeId),
    /// An attribute: the owning element and the position in its attribute list.
    Attr {
        /// The element carrying the attribute.
        owner: NodeId,
        /// Position of the attribute in the element's attribute order.
        index: u32,
    },
}

impl XNode {
    /// The arena handle when this is not an attribute.
    #[must_use]
    pub fn node_id(self) -> Option<NodeId> {
        match self {
            Self::Node(id) => Some(id),
            Self::Attr { .. } => None,
        }
    }

    /// The element that owns an attribute node, `None` for the other kinds.
    #[must_use]
    pub fn attr_owner(self) -> Option<NodeId> {
        match self {
            Self::Attr { owner, .. } => Some(owner),
            Self::Node(_) => None,
        }
    }

    /// True for attribute nodes.
    #[must_use]
    pub fn is_attr(self) -> bool {
        matches!(self, Self::Attr { .. })
    }
}

/// The result of evaluating an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A node-set in document order without duplicates.
    NodeSet(Vec<XNode>),
    /// A string.
    String(String),
    /// A double precision number (may be NaN or infinite).
    Number(f64),
    /// A boolean.
    Boolean(bool),
}

impl Value {
    /// The XPath type name: `node-set`, `string`, `number` or `boolean`.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::NodeSet(_) => "node-set",
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Boolean(_) => "boolean",
        }
    }
}
