//! XPath 1.0 parser and evaluator over the node tree.
//!
//! Layer `l2-engine`. The crate implements the language the game's runtime evaluates for patch
//! operations (XPath 1.0 over a document, with the document node as the context node, so both
//! `Defs/ThingDef[...]` and `/Defs/ThingDef[...]` work), over [`rimstudio_core::tree::ArenaDoc`].
//! It depends on no XML or XPath crate.
//!
//! - [`XPath::parse`] trims the text, parses the full XPath 1.0 grammar and rejects what the game
//!   rejects (unknown functions, wrong argument counts, variables, namespace prefixes).
//! - [`XPath::select`] returns node handles in document order without duplicates;
//!   [`XPath::select_nodes`] also reports attribute nodes; [`XPath::eval`] returns any
//!   [`Value`] with the exact XPath 1.0 conversions of the recommendation.
//! - [`XPath::index_hint`] recognises the dominant shape `Defs/Type[defName="x"]` for the def
//!   engine's hash index.
//!
//! See the [`eval`] module documentation for the supported axes and the data model notes.

pub mod ast;
pub mod coerce;
pub mod error;
pub mod eval;
mod functions;
pub mod index_hint;
pub mod lexer;
pub mod parser;
pub mod value;

use rimstudio_core::tree::{ArenaDoc, NodeId};

pub use ast::{Axis, BinOp, Expr, Func, NodeTest, PathBase, PathExpr, Step};
pub use error::{XPathError, XPathResult};
pub use index_hint::{DefKey, IndexHint, IndexPlan};
pub use value::{Value, XNode};

use eval::{Ctx, Evaluator};

/// A parsed, validated XPath 1.0 expression.
#[derive(Debug, Clone, PartialEq)]
pub struct XPath {
    source: String,
    expr: Expr,
}

impl XPath {
    /// Parses an expression. Leading and trailing white space (including newlines) is trimmed first;
    /// white space inside the expression is ordinary token separation.
    ///
    /// # Errors
    /// [`XPathError::Parse`] with a byte offset into the trimmed text, [`XPathError::UnknownFunction`],
    /// [`XPathError::Arity`], [`XPathError::UnboundPrefix`], [`XPathError::UnboundVariable`] or
    /// [`XPathError::TooDeep`].
    pub fn parse(text: &str) -> XPathResult<Self> {
        let source = text.trim();
        let expr = parser::parse(source)?;
        parser::validate(source, &expr)?;
        Ok(Self {
            source: source.to_owned(),
            expr,
        })
    }

    /// The trimmed expression text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// The syntax tree.
    #[must_use]
    pub fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Evaluates the expression with `ctx` as the context node (position 1 of 1).
    ///
    /// # Errors
    /// [`XPathError::StaleContext`], type errors and unsupported constructs.
    pub fn eval(&self, doc: &ArenaDoc, ctx: NodeId) -> XPathResult<Value> {
        if !doc.contains(ctx) {
            return Err(XPathError::StaleContext);
        }
        Evaluator::new(doc).eval(&self.expr, Ctx::single(XNode::Node(ctx)))
    }

    /// Evaluates with the document node as the context node, the way the game applies patches.
    ///
    /// # Errors
    /// See [`XPath::eval`].
    pub fn eval_document(&self, doc: &ArenaDoc) -> XPathResult<Value> {
        self.eval(doc, doc.root())
    }

    /// Selects nodes with `ctx` as the context node, including attribute nodes. The result is
    /// in document order without duplicates (attributes follow their element in attribute order).
    ///
    /// # Errors
    /// [`XPathError::NotNodeSet`] when the expression yields a number, string or boolean, plus
    /// the errors of [`XPath::eval`].
    pub fn select_nodes(&self, doc: &ArenaDoc, ctx: NodeId) -> XPathResult<Vec<XNode>> {
        node_set(self.eval(doc, ctx)?)
    }

    /// Selects node handles with `ctx` as the context node, in document order without duplicates.
    ///
    /// # Errors
    /// [`XPathError::AttributeResult`] when the result contains attribute nodes (use
    /// [`XPath::select_nodes`]), plus the errors of [`XPath::select_nodes`].
    pub fn select_from(&self, doc: &ArenaDoc, ctx: NodeId) -> XPathResult<Vec<NodeId>> {
        ids(self.select_nodes(doc, ctx)?)
    }

    /// Selects node handles with the document node as the context node, in document order
    /// without duplicates. This is `XmlDocument.SelectNodes`.
    ///
    /// # Errors
    /// See [`XPath::select_from`].
    pub fn select(&self, doc: &ArenaDoc) -> XPathResult<Vec<NodeId>> {
        self.select_from(doc, doc.root())
    }

    /// The index lookup that answers the whole expression, when it has the shape
    /// `Defs/Type[defName="x"]` (see [`index_hint`] for the exact family). Valid for evaluation
    /// with the document node as the context node.
    #[must_use]
    pub fn index_hint(&self) -> Option<IndexHint> {
        index_hint::plan(&self.expr)
            .filter(|p| p.rest.is_empty())
            .map(|p| p.hint)
    }

    /// The index lookup for the leading `Defs/Type[defName="x"]` part plus the steps after it.
    #[must_use]
    pub fn index_plan(&self) -> Option<IndexPlan<'_>> {
        index_hint::plan(&self.expr)
    }

    /// Finishes an index lookup: applies the steps after the indexed definitions (see
    /// [`XPath::index_plan`]) to `defs`, the definition elements the index found, in any order.
    ///
    /// # Errors
    /// [`XPathError::Unsupported`] when the expression has no index plan, and
    /// [`XPathError::AttributeResult`] for attribute results, plus evaluation errors.
    pub fn select_with_defs(&self, doc: &ArenaDoc, defs: &[NodeId]) -> XPathResult<Vec<NodeId>> {
        let Some(plan) = index_hint::plan(&self.expr) else {
            return Err(XPathError::Unsupported {
                what: "select_with_defs needs an expression with an index plan".to_owned(),
            });
        };
        let live: Vec<NodeId> = defs.iter().copied().filter(|d| doc.contains(*d)).collect();
        let nodes = Evaluator::new(doc).eval_steps_from_nodes(live, plan.rest)?;
        ids(nodes)
    }
}

fn node_set(v: Value) -> XPathResult<Vec<XNode>> {
    match v {
        Value::NodeSet(n) => Ok(n),
        Value::Number(_) => Err(XPathError::NotNodeSet { found: "number" }),
        Value::String(_) => Err(XPathError::NotNodeSet { found: "string" }),
        Value::Boolean(_) => Err(XPathError::NotNodeSet { found: "boolean" }),
    }
}

fn ids(nodes: Vec<XNode>) -> XPathResult<Vec<NodeId>> {
    nodes
        .into_iter()
        .map(|n| n.node_id().ok_or(XPathError::AttributeResult))
        .collect()
}

impl std::fmt::Display for XPath {
    /// The normalised form of the expression (abbreviations kept where the printer can).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.expr.fmt(f)
    }
}
