//! The evaluator: expressions over an [`ArenaDoc`].
//!
//! # Axes
//!
//! Twelve of the thirteen axes are evaluated: `child`, `descendant`, `descendant-or-self`,
//! `self`, `parent`, `ancestor`, `ancestor-or-self`, `following`, `following-sibling`,
//! `preceding`, `preceding-sibling` and `attribute`. The `namespace` axis parses but evaluating
//! it returns [`XPathError::UnsupportedAxis`]: the unified document has no namespace nodes.
//!
//! # Data model notes
//!
//! - The node kinds are the document node, elements, text and attributes. The arena has no
//!   comment or processing instruction nodes (the game's loader drops them), so `comment()` and
//!   `processing-instruction()` tests select nothing.
//! - Attributes named `xmlns` or `xmlns:*` are namespace declarations and are not attribute
//!   nodes, as in the XPath data model.
//! - Element names are compared as written. The `local-name` of `a:b` is `b`, the
//!   `namespace-uri` of every node is the empty string.
//! - `string-length` and `substring` count Unicode scalar values.
//! - `id()` evaluates its argument and returns the empty node-set, because documents have no DTD.
//! - `lang()` is not supported and returns [`XPathError::Unsupported`].
//!
//! # Document order and speed
//!
//! Node-sets are always in document order without duplicates. Steps keep two facts about their
//! result (sorted, and no node an ancestor of another) so the common child and attribute chains
//! never sort; everything else is sorted and de-duplicated with the arena's order numbering.
//! `//name` is evaluated as `descendant::name` when no predicate of the name step can depend on
//! the position. A comparison of a one step child, attribute or self path with a string literal
//! (`defName = "x"`) is evaluated without allocating.
//!
//! # Depth
//!
//! The parser bounds nesting, and evaluation counts the depth of the expressions it enters and
//! fails with [`XPathError::TooDeep`] beyond [`MAX_EVAL_DEPTH`]. Walks over the document use the
//! arena's iterative iterators, so documents nested ten thousand levels deep are safe.

use std::cmp::Ordering;

use rimstudio_core::tree::{ArenaDoc, NodeId, NodeKind, Sym};

use crate::ast::{Axis, BinOp, Expr, Func, NodeTest, PathBase, PathExpr, Step};
use crate::coerce::{compare, node_string, to_boolean, to_number, xpath_round};
use crate::error::{XPathError, XPathResult};
use crate::value::{Value, XNode};

/// The deepest expression nesting evaluation will enter.
pub const MAX_EVAL_DEPTH: usize = 1024;

/// The evaluation context of one expression: node, proximity position and context size.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx {
    pub(crate) node: XNode,
    pub(crate) position: usize,
    pub(crate) size: usize,
}

impl Ctx {
    pub(crate) fn single(node: XNode) -> Self {
        Self {
            node,
            position: 1,
            size: 1,
        }
    }
}

/// What is known about the order of a node-set during path evaluation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Order {
    /// Strictly increasing in document order (so also free of duplicates).
    sorted: bool,
    /// No node of the set is an ancestor of another.
    independent: bool,
}

const SINGLE: Order = Order {
    sorted: true,
    independent: true,
};

pub(crate) struct Evaluator<'d> {
    pub(crate) doc: &'d ArenaDoc,
    depth: usize,
    scratch: Vec<XNode>,
}

// ------------------------------------------------------------------------ node tests

enum MKind<'t> {
    Never,
    Name { local: &'t str },
    Wildcard,
    Text,
    AnyNode,
}

struct Matcher<'t> {
    kind: MKind<'t>,
    sym: Option<Sym>,
    attr_axis: bool,
}

impl<'t> Matcher<'t> {
    fn new(doc: &ArenaDoc, test: &'t NodeTest, attr_axis: bool) -> Self {
        let mut sym = None;
        let kind = match test {
            NodeTest::Name {
                prefix: None,
                local,
            } => {
                sym = doc.sym(local);
                if sym.is_some() {
                    MKind::Name { local }
                } else {
                    MKind::Never
                }
            }
            NodeTest::Name {
                prefix: Some(_), ..
            }
            | NodeTest::PrefixWildcard(_)
            | NodeTest::Comment
            | NodeTest::ProcessingInstruction(_) => MKind::Never,
            NodeTest::Wildcard => MKind::Wildcard,
            NodeTest::Text => MKind::Text,
            NodeTest::AnyNode => MKind::AnyNode,
        };
        Self {
            kind,
            sym,
            attr_axis,
        }
    }

    fn never(&self) -> bool {
        matches!(self.kind, MKind::Never)
    }

    fn matches(&self, doc: &ArenaDoc, n: XNode) -> bool {
        match &self.kind {
            MKind::Never => false,
            MKind::AnyNode => true,
            MKind::Name { local } => match n {
                XNode::Node(id) => !self.attr_axis && doc.name_sym(id) == self.sym,
                XNode::Attr { owner, index } => {
                    self.attr_axis
                        && doc
                            .attr_at(owner, index as usize)
                            .is_some_and(|(k, _)| k == *local)
                }
            },
            MKind::Wildcard => match n {
                XNode::Node(id) => !self.attr_axis && doc.kind(id) == Some(NodeKind::Element),
                XNode::Attr { .. } => self.attr_axis,
            },
            MKind::Text => match n {
                XNode::Node(id) => doc.kind(id) == Some(NodeKind::Text),
                XNode::Attr { .. } => false,
            },
        }
    }
}

fn is_namespace_decl(name: &str) -> bool {
    name == "xmlns" || name.starts_with("xmlns:")
}

// ------------------------------------------------------------------ static analysis

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaticType {
    Bool,
    Num,
    Str,
    Nodes,
    Unknown,
}

fn static_type(e: &Expr) -> StaticType {
    match e {
        Expr::Chain { rest, .. } => match rest.first().map(|(op, _)| *op) {
            Some(BinOp::Or | BinOp::And | BinOp::Eq | BinOp::Neq) => StaticType::Bool,
            Some(BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) => StaticType::Bool,
            Some(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod) => StaticType::Num,
            Some(BinOp::Union) => StaticType::Nodes,
            None => StaticType::Unknown,
        },
        Expr::Neg(_) | Expr::Number(_) => StaticType::Num,
        Expr::Literal(_) => StaticType::Str,
        Expr::Variable(_) => StaticType::Unknown,
        Expr::Filter { .. } | Expr::Path(_) => StaticType::Nodes,
        Expr::Function { func, .. } => match func {
            Func::Last
            | Func::Position
            | Func::Count
            | Func::StringLength
            | Func::Number
            | Func::Sum
            | Func::Floor
            | Func::Ceiling
            | Func::Round => StaticType::Num,
            Func::Id => StaticType::Nodes,
            Func::LocalName
            | Func::NamespaceUri
            | Func::Name
            | Func::String
            | Func::Concat
            | Func::SubstringBefore
            | Func::SubstringAfter
            | Func::Substring
            | Func::NormalizeSpace
            | Func::Translate => StaticType::Str,
            Func::StartsWith
            | Func::Contains
            | Func::Boolean
            | Func::Not
            | Func::True
            | Func::False
            | Func::Lang => StaticType::Bool,
        },
    }
}

fn mentions_position(e: &Expr) -> bool {
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        if matches!(
            x,
            Expr::Function {
                func: Func::Position | Func::Last,
                ..
            }
        ) {
            return true;
        }
        x.for_each_child(&mut |c| stack.push(c));
    }
    false
}

/// True when no predicate can select by position, so a filter over a set of candidates gives the
/// same answer no matter how the candidates are grouped.
fn non_positional(preds: &[Expr]) -> bool {
    preds.iter().all(|p| {
        matches!(
            static_type(p),
            StaticType::Bool | StaticType::Nodes | StaticType::Str
        ) && !mentions_position(p)
    })
}

/// A path with one child, attribute or self step without predicates, relative to the context.
fn simple_step(e: &Expr) -> Option<&Step> {
    if let Expr::Path(PathExpr {
        base: PathBase::Context,
        steps,
    }) = e
        && let [s] = steps.as_slice()
        && s.predicates.is_empty()
        && matches!(s.axis, Axis::Child | Axis::Attribute | Axis::SelfAxis)
    {
        return Some(s);
    }
    None
}

// -------------------------------------------------------------------------- evaluator

/// Node sets up to this size are ordered by pairwise comparison instead of by the numbering of the
/// whole document.
const SMALL_SET: usize = 16;

impl<'d> Evaluator<'d> {
    pub(crate) fn new(doc: &'d ArenaDoc) -> Self {
        Self {
            doc,
            depth: 0,
            scratch: Vec::new(),
        }
    }

    pub(crate) fn eval(&mut self, e: &Expr, ctx: Ctx) -> XPathResult<Value> {
        self.depth += 1;
        if self.depth > MAX_EVAL_DEPTH {
            self.depth -= 1;
            return Err(XPathError::TooDeep {
                limit: MAX_EVAL_DEPTH,
            });
        }
        let r = self.eval_inner(e, ctx);
        self.depth -= 1;
        r
    }

    fn eval_inner(&mut self, e: &Expr, ctx: Ctx) -> XPathResult<Value> {
        match e {
            Expr::Literal(s) => Ok(Value::String(s.clone())),
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Variable(name) => Err(XPathError::UnboundVariable {
                pos: 0,
                name: name.clone(),
            }),
            Expr::Neg(inner) => {
                let v = self.eval(inner, ctx)?;
                Ok(Value::Number(-to_number(self.doc, &v)))
            }
            Expr::Chain { first, rest } => self.eval_chain(first, rest, ctx),
            Expr::Function { func, args } => self.call(*func, args, ctx),
            Expr::Filter {
                primary,
                predicates,
            } => {
                let Value::NodeSet(mut set) = self.eval(primary, ctx)? else {
                    return Err(XPathError::type_error(
                        "a predicate can only filter a node-set",
                    ));
                };
                self.apply_predicates(predicates, &mut set)?;
                Ok(Value::NodeSet(set))
            }
            Expr::Path(p) => self.eval_path(p, ctx).map(Value::NodeSet),
        }
    }

    // ------------------------------------------------------------------ chains

    fn eval_chain(&mut self, first: &Expr, rest: &[(BinOp, Expr)], ctx: Ctx) -> XPathResult<Value> {
        let Some((op0, _)) = rest.first() else {
            return self.eval(first, ctx);
        };
        match op0 {
            BinOp::Or => {
                if to_boolean(&self.eval(first, ctx)?) {
                    return Ok(Value::Boolean(true));
                }
                for (_, x) in rest {
                    if to_boolean(&self.eval(x, ctx)?) {
                        return Ok(Value::Boolean(true));
                    }
                }
                Ok(Value::Boolean(false))
            }
            BinOp::And => {
                if !to_boolean(&self.eval(first, ctx)?) {
                    return Ok(Value::Boolean(false));
                }
                for (_, x) in rest {
                    if !to_boolean(&self.eval(x, ctx)?) {
                        return Ok(Value::Boolean(false));
                    }
                }
                Ok(Value::Boolean(true))
            }
            BinOp::Union => {
                let mut all: Vec<XNode> = Vec::new();
                let mut parts = 0usize;
                for x in std::iter::once(first).chain(rest.iter().map(|(_, x)| x)) {
                    match self.eval(x, ctx)? {
                        Value::NodeSet(mut s) => {
                            all.append(&mut s);
                            parts += 1;
                        }
                        other => {
                            return Err(XPathError::type_error(format!(
                                "the union operator needs node-sets, found a {}",
                                other.type_name()
                            )));
                        }
                    }
                }
                if parts > 1 {
                    self.normalize(&mut all);
                }
                Ok(Value::NodeSet(all))
            }
            _ => {
                if let [(op, right)] = rest
                    && matches!(op, BinOp::Eq | BinOp::Neq)
                    && let Some(b) = self.try_literal_compare(*op, first, right, ctx)?
                {
                    return Ok(Value::Boolean(b));
                }
                let mut acc = self.eval(first, ctx)?;
                for (op, x) in rest {
                    let r = self.eval(x, ctx)?;
                    acc = self.apply_binary(*op, &acc, &r);
                }
                Ok(acc)
            }
        }
    }

    fn apply_binary(&self, op: BinOp, a: &Value, b: &Value) -> Value {
        match op {
            BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                Value::Boolean(compare(self.doc, op, a, b))
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let x = to_number(self.doc, a);
                let y = to_number(self.doc, b);
                Value::Number(match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    BinOp::Div => x / y,
                    _ => x % y,
                })
            }
            BinOp::Or | BinOp::And => {
                let (x, y) = (to_boolean(a), to_boolean(b));
                Value::Boolean(if op == BinOp::Or { x || y } else { x && y })
            }
            BinOp::Union => Value::NodeSet(Vec::new()),
        }
    }

    /// `step = "literal"` and `"literal" = step` without building a node-set.
    fn try_literal_compare(
        &mut self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        ctx: Ctx,
    ) -> XPathResult<Option<bool>> {
        let (step, lit) = match (simple_step(left), right) {
            (Some(s), Expr::Literal(l)) => (s, l),
            _ => match (left, simple_step(right)) {
                (Expr::Literal(l), Some(s)) => (s, l),
                _ => return Ok(None),
            },
        };
        let doc = self.doc;
        let m = Matcher::new(doc, &step.test, step.axis == Axis::Attribute);
        if m.never() {
            return Ok(Some(false));
        }
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        let r = self.axis_nodes(ctx.node, step.axis, &m, &mut buf);
        let found = r.map(|()| {
            buf.iter().any(|n| {
                let s = node_string(doc, *n);
                if op == BinOp::Eq {
                    s == lit.as_str()
                } else {
                    s != lit.as_str()
                }
            })
        });
        self.scratch = buf;
        found.map(Some)
    }

    // -------------------------------------------------------------------- paths

    fn root_of(&self, from: XNode) -> NodeId {
        let id = match from {
            XNode::Node(id) => id,
            XNode::Attr { owner, .. } => owner,
        };
        let mut top = id;
        while let Some(p) = self.doc.parent(top) {
            top = p;
        }
        top
    }

    pub(crate) fn eval_path(&mut self, p: &PathExpr, ctx: Ctx) -> XPathResult<Vec<XNode>> {
        let (set, order) = match &p.base {
            PathBase::Root => (vec![XNode::Node(self.root_of(ctx.node))], SINGLE),
            PathBase::Context => (vec![ctx.node], SINGLE),
            PathBase::Expr(e) => match self.eval(e, ctx)? {
                Value::NodeSet(v) => {
                    let order = Order {
                        sorted: true,
                        independent: v.len() <= 1,
                    };
                    (v, order)
                }
                other => {
                    return Err(XPathError::type_error(format!(
                        "a path can only continue a node-set, found a {}",
                        other.type_name()
                    )));
                }
            },
        };
        self.eval_steps(set, order, &p.steps)
    }

    /// Applies `steps` to the nodes of `start`, which must be in document order.
    pub(crate) fn eval_steps(
        &mut self,
        mut set: Vec<XNode>,
        mut order: Order,
        steps: &[Step],
    ) -> XPathResult<Vec<XNode>> {
        let mut i = 0;
        while let Some(step) = steps.get(i) {
            if set.is_empty() {
                break;
            }
            if step.is_descendant_or_self_node()
                && let Some(next) = steps.get(i + 1)
                && next.axis == Axis::Child
                && non_positional(&next.predicates)
            {
                let (s, o) = self.eval_step(&set, order, Axis::Descendant, next)?;
                set = s;
                order = o;
                i += 2;
                continue;
            }
            let (s, o) = self.eval_step(&set, order, step.axis, step)?;
            set = s;
            order = o;
            i += 1;
        }
        Ok(set)
    }

    pub(crate) fn eval_steps_from_nodes(
        &mut self,
        nodes: Vec<NodeId>,
        steps: &[Step],
    ) -> XPathResult<Vec<XNode>> {
        let mut set: Vec<XNode> = nodes.into_iter().map(XNode::Node).collect();
        self.normalize(&mut set);
        let order = Order {
            sorted: true,
            independent: set.len() <= 1,
        };
        self.eval_steps(set, order, steps)
    }

    fn eval_step(
        &mut self,
        input: &[XNode],
        in_order: Order,
        axis: Axis,
        step: &Step,
    ) -> XPathResult<(Vec<XNode>, Order)> {
        if axis == Axis::Namespace {
            return Err(XPathError::UnsupportedAxis { axis: "namespace" });
        }
        let doc = self.doc;
        let m = Matcher::new(doc, &step.test, axis == Axis::Attribute);
        if m.never() {
            return Ok((Vec::new(), SINGLE));
        }
        let mut out: Vec<XNode> = Vec::new();
        let mut cands: Vec<XNode> = Vec::new();
        for node in input {
            cands.clear();
            self.axis_nodes(*node, axis, &m, &mut cands)?;
            if !step.predicates.is_empty() {
                self.apply_predicates(&step.predicates, &mut cands)?;
            }
            if axis.is_reverse() {
                cands.reverse();
            }
            out.extend_from_slice(&cands);
        }
        let flat = matches!(axis, Axis::Child | Axis::Attribute | Axis::SelfAxis);
        let order = if input.len() <= 1 {
            Order {
                sorted: true,
                independent: flat || out.len() <= 1,
            }
        } else if in_order.sorted && in_order.independent && flat {
            Order {
                sorted: true,
                independent: true,
            }
        } else if in_order.sorted
            && in_order.independent
            && matches!(axis, Axis::Descendant | Axis::DescendantOrSelf)
        {
            Order {
                sorted: true,
                independent: out.len() <= 1,
            }
        } else {
            self.normalize(&mut out);
            Order {
                sorted: true,
                independent: out.len() <= 1,
            }
        };
        Ok((out, order))
    }

    /// Appends the nodes on `axis` from `from` that pass the matcher, in axis order (nearest
    /// first on reverse axes).
    fn axis_nodes(
        &self,
        from: XNode,
        axis: Axis,
        m: &Matcher<'_>,
        out: &mut Vec<XNode>,
    ) -> XPathResult<()> {
        let doc = self.doc;
        let mut push = |n: XNode| {
            if m.matches(doc, n) {
                out.push(n);
            }
        };
        let id = match from {
            XNode::Node(id) => Some(id),
            XNode::Attr { .. } => None,
        };
        match axis {
            Axis::Child => {
                if let Some(id) = id {
                    for c in doc.children(id) {
                        push(XNode::Node(c));
                    }
                }
            }
            Axis::Descendant => {
                if let Some(id) = id {
                    for c in doc.descendants(id) {
                        push(XNode::Node(c));
                    }
                }
            }
            Axis::DescendantOrSelf => match from {
                XNode::Node(id) => {
                    for c in doc.preorder(id) {
                        push(XNode::Node(c));
                    }
                }
                attr @ XNode::Attr { .. } => push(attr),
            },
            Axis::SelfAxis => push(from),
            Axis::Parent => match from {
                XNode::Node(id) => {
                    if let Some(p) = doc.parent(id) {
                        push(XNode::Node(p));
                    }
                }
                XNode::Attr { owner, .. } => push(XNode::Node(owner)),
            },
            Axis::Ancestor | Axis::AncestorOrSelf => {
                if axis == Axis::AncestorOrSelf {
                    push(from);
                }
                let start = match from {
                    XNode::Node(id) => id,
                    XNode::Attr { owner, .. } => {
                        push(XNode::Node(owner));
                        owner
                    }
                };
                for a in doc.ancestors(start) {
                    push(XNode::Node(a));
                }
            }
            Axis::FollowingSibling => {
                if let Some(id) = id {
                    for s in doc.following_siblings(id) {
                        push(XNode::Node(s));
                    }
                }
            }
            Axis::PrecedingSibling => {
                if let Some(id) = id {
                    for s in doc.preceding_siblings(id) {
                        push(XNode::Node(s));
                    }
                }
            }
            Axis::Following => match from {
                XNode::Node(id) => {
                    for n in doc.following(id) {
                        push(XNode::Node(n));
                    }
                }
                XNode::Attr { owner, .. } => {
                    for n in doc.descendants(owner) {
                        push(XNode::Node(n));
                    }
                    for n in doc.following(owner) {
                        push(XNode::Node(n));
                    }
                }
            },
            Axis::Preceding => {
                let start = match from {
                    XNode::Node(id) => id,
                    XNode::Attr { owner, .. } => owner,
                };
                for n in doc.preceding(start) {
                    push(XNode::Node(n));
                }
            }
            Axis::Attribute => {
                if let Some(id) = id {
                    let count = doc.attr_count(id);
                    for i in 0..count {
                        let Some((name, _)) = doc.attr_at(id, i) else {
                            break;
                        };
                        if is_namespace_decl(name) {
                            continue;
                        }
                        if let Ok(index) = u32::try_from(i) {
                            push(XNode::Attr { owner: id, index });
                        }
                    }
                }
            }
            Axis::Namespace => return Err(XPathError::UnsupportedAxis { axis: "namespace" }),
        }
        Ok(())
    }

    // --------------------------------------------------------------- predicates

    pub(crate) fn apply_predicates(
        &mut self,
        preds: &[Expr],
        list: &mut Vec<XNode>,
    ) -> XPathResult<()> {
        for p in preds {
            if list.is_empty() {
                return Ok(());
            }
            if let Expr::Number(n) = p {
                // `[3]`: pick one candidate without evaluating anything.
                let picked = if n.fract() == 0.0 && *n >= 1.0 && *n <= list.len() as f64 {
                    list.get((*n as usize).saturating_sub(1)).copied()
                } else {
                    None
                };
                list.clear();
                list.extend(picked);
                continue;
            }
            let size = list.len();
            let old = std::mem::take(list);
            for (i, node) in old.iter().enumerate() {
                let ctx = Ctx {
                    node: *node,
                    position: i + 1,
                    size,
                };
                let keep = match self.eval(p, ctx)? {
                    Value::Number(n) => n == (i + 1) as f64,
                    other => to_boolean(&other),
                };
                if keep {
                    list.push(*node);
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ document order

    fn key(&self, n: XNode) -> Option<(u32, u32)> {
        match n {
            XNode::Node(id) => self.doc.order_key(id).map(|k| (k, 0)),
            XNode::Attr { owner, index } => self
                .doc
                .order_key(owner)
                .map(|k| (k, index.saturating_add(1))),
        }
    }

    /// Sorts into document order and removes duplicates.
    pub(crate) fn normalize(&self, v: &mut Vec<XNode>) {
        if v.len() < 2 {
            return;
        }
        let doc = self.doc;
        // Building the numbering is O(n) in the size of the whole arena after every structural
        // change; a handful of nodes is cheaper to compare directly (O(1) when the numbering is
        // already current).
        let keyed: Option<Vec<((u32, u32), XNode)>> = if v.len() > SMALL_SET {
            doc.prepare_order();
            v.iter().map(|n| self.key(*n).map(|k| (k, *n))).collect()
        } else {
            None
        };
        if let Some(mut keyed) = keyed {
            keyed.sort_unstable_by_key(|(k, _)| *k);
            keyed.dedup_by_key(|(k, _)| *k);
            v.clear();
            v.extend(keyed.into_iter().map(|(_, n)| n));
        } else {
            // Some nodes are detached: fall back to the arena's general comparison.
            let owner = |n: &XNode| match n {
                XNode::Node(id) => (*id, 0u32),
                XNode::Attr { owner, index } => (*owner, index.saturating_add(1)),
            };
            v.sort_by(|a, b| {
                let (oa, ia) = owner(a);
                let (ob, ib) = owner(b);
                doc.compare_order(oa, ob)
                    .unwrap_or(Ordering::Equal)
                    .then(ia.cmp(&ib))
            });
            v.dedup();
        }
    }

    // ---------------------------------------------------------------- helpers

    pub(crate) fn string_of(&self, v: &Value) -> String {
        crate::coerce::to_string(self.doc, v)
    }

    pub(crate) fn number_of(&self, v: &Value) -> f64 {
        to_number(self.doc, v)
    }

    pub(crate) fn round(&self, x: f64) -> f64 {
        xpath_round(x)
    }
}
