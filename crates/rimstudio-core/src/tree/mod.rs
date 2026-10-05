//! The node tree: the single in memory form of every XML document in RimStudio.
//!
//! Two representations share one JSON shape and convert losslessly into each other:
//!
//! - [`Node`] and [`Child`]: the owned, `serde` serialisable tree. `rimstudio-xml` converts files to
//!   and from it, templates and the item designer build it with [`NodeBuilder`], write plans
//!   carry it as JSON.
//! - [`ArenaDoc`]: the mutable arena document with stable [`NodeId`] handles, parent and sibling
//!   links and per node [`OriginId`], used by the XPath, patch and inheritance engines.
//!
//! Errors are [`TreeError`] values with stable codes (`tree.invalid_name`, `tree.stale_id`, ...).

mod arena;
mod builder;
mod error;
mod names;
mod node;

pub use arena::{
    Ancestors, ArenaDoc, Children, NodeId, NodeKind, OriginId, Preceding, Preorder, SiblingIter,
    Sym,
};
pub use builder::NodeBuilder;
pub use error::TreeError;
pub use node::{Child, Node};

/// Deepest element nesting accepted by validation, JSON decoding and arena import.
pub const MAX_DEPTH: usize = 512;
