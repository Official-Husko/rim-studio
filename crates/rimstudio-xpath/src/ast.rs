//! The syntax tree of an XPath 1.0 expression and its printer.
//!
//! The tree follows the grammar of the XPath 1.0 recommendation with two normalisations:
//!
//! - Abbreviations are expanded: `//` becomes a `descendant-or-self::node()` step, `.` is
//!   `self::node()`, `..` is `parent::node()` and `@a` is `attribute::a`.
//! - Left associative operators of one precedence level are kept flat in
//!   [`Expr::Chain`], so a chain of ten thousand `or` operands is a vector, not a tree ten
//!   thousand levels deep. Parenthesised expressions leave no trace except through the
//!   structure they force.
//!
//! [`Expr`] implements [`std::fmt::Display`]. The printed text parses back to an equal tree for
//! every tree the parser can produce (string literals that contain both quote characters have
//! no textual form in XPath 1.0 and cannot come from the parser).

use std::fmt::{self, Write as _};

/// One of the thirteen XPath axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// `ancestor::`
    Ancestor,
    /// `ancestor-or-self::`
    AncestorOrSelf,
    /// `attribute::` or `@`
    Attribute,
    /// `child::` (the default axis)
    Child,
    /// `descendant::`
    Descendant,
    /// `descendant-or-self::`
    DescendantOrSelf,
    /// `following::`
    Following,
    /// `following-sibling::`
    FollowingSibling,
    /// `namespace::` (parsed, not evaluated: the unified document has no namespace nodes)
    Namespace,
    /// `parent::`
    Parent,
    /// `preceding::`
    Preceding,
    /// `preceding-sibling::`
    PrecedingSibling,
    /// `self::`
    SelfAxis,
}

impl Axis {
    /// The axis name as written in an expression.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ancestor => "ancestor",
            Self::AncestorOrSelf => "ancestor-or-self",
            Self::Attribute => "attribute",
            Self::Child => "child",
            Self::Descendant => "descendant",
            Self::DescendantOrSelf => "descendant-or-self",
            Self::Following => "following",
            Self::FollowingSibling => "following-sibling",
            Self::Namespace => "namespace",
            Self::Parent => "parent",
            Self::Preceding => "preceding",
            Self::PrecedingSibling => "preceding-sibling",
            Self::SelfAxis => "self",
        }
    }

    /// The axis for a written name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "ancestor" => Self::Ancestor,
            "ancestor-or-self" => Self::AncestorOrSelf,
            "attribute" => Self::Attribute,
            "child" => Self::Child,
            "descendant" => Self::Descendant,
            "descendant-or-self" => Self::DescendantOrSelf,
            "following" => Self::Following,
            "following-sibling" => Self::FollowingSibling,
            "namespace" => Self::Namespace,
            "parent" => Self::Parent,
            "preceding" => Self::Preceding,
            "preceding-sibling" => Self::PrecedingSibling,
            "self" => Self::SelfAxis,
            _ => return None,
        })
    }

    /// True for the axes whose proximity positions count backwards in document order.
    #[must_use]
    pub fn is_reverse(self) -> bool {
        matches!(
            self,
            Self::Ancestor | Self::AncestorOrSelf | Self::Preceding | Self::PrecedingSibling
        )
    }
}

/// The node test of a step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeTest {
    /// A name test, `local` or `prefix:local`.
    Name {
        /// The namespace prefix as written, if any.
        prefix: Option<String>,
        /// The local part.
        local: String,
    },
    /// `*`
    Wildcard,
    /// `prefix:*`
    PrefixWildcard(String),
    /// `text()`
    Text,
    /// `comment()`
    Comment,
    /// `processing-instruction()` with an optional target literal.
    ProcessingInstruction(Option<String>),
    /// `node()`
    AnyNode,
}

/// A location step: an axis, a node test and predicates.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The axis.
    pub axis: Axis,
    /// The node test.
    pub test: NodeTest,
    /// The predicates in order.
    pub predicates: Vec<Expr>,
}

impl Step {
    /// A step without predicates.
    #[must_use]
    pub fn new(axis: Axis, test: NodeTest) -> Self {
        Self {
            axis,
            test,
            predicates: Vec::new(),
        }
    }

    /// The step `child::name`.
    #[must_use]
    pub fn child(name: &str) -> Self {
        Self::new(
            Axis::Child,
            NodeTest::Name {
                prefix: None,
                local: name.to_owned(),
            },
        )
    }

    /// True for `descendant-or-self::node()` without predicates, the expansion of `//`.
    #[must_use]
    pub fn is_descendant_or_self_node(&self) -> bool {
        self.axis == Axis::DescendantOrSelf
            && self.test == NodeTest::AnyNode
            && self.predicates.is_empty()
    }
}

/// Where a path starts.
#[derive(Debug, Clone, PartialEq)]
pub enum PathBase {
    /// `/`: the root of the tree containing the context node.
    Root,
    /// A relative path, starting at the context node.
    Context,
    /// A filter expression: the nodes of a primary expression, as in `(a | b)/c` or `f()/c`.
    Expr(Box<Expr>),
}

/// A path: a start and a list of steps.
#[derive(Debug, Clone, PartialEq)]
pub struct PathExpr {
    /// Where the path starts.
    pub base: PathBase,
    /// The steps; empty only for the root path `/`.
    pub steps: Vec<Step>,
}

/// A binary operator. Operators of one precedence level share an [`Expr::Chain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    /// `or`
    Or,
    /// `and`
    And,
    /// `=`
    Eq,
    /// `!=`
    Neq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `div`
    Div,
    /// `mod`
    Mod,
    /// `|`
    Union,
}

impl BinOp {
    /// Binding strength: higher binds tighter. Unary minus sits at 7.
    #[must_use]
    pub fn precedence(self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Eq | Self::Neq => 3,
            Self::Lt | Self::Le | Self::Gt | Self::Ge => 4,
            Self::Add | Self::Sub => 5,
            Self::Mul | Self::Div | Self::Mod => 6,
            Self::Union => 8,
        }
    }

    /// The operator as written.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Or => "or",
            Self::And => "and",
            Self::Eq => "=",
            Self::Neq => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "div",
            Self::Mod => "mod",
            Self::Union => "|",
        }
    }
}

/// The functions of the XPath 1.0 core library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Func {
    /// `last()`
    Last,
    /// `position()`
    Position,
    /// `count(node-set)`
    Count,
    /// `id(object)`
    Id,
    /// `local-name(node-set?)`
    LocalName,
    /// `namespace-uri(node-set?)`
    NamespaceUri,
    /// `name(node-set?)`
    Name,
    /// `string(object?)`
    String,
    /// `concat(string, string, string*)`
    Concat,
    /// `starts-with(string, string)`
    StartsWith,
    /// `contains(string, string)`
    Contains,
    /// `substring-before(string, string)`
    SubstringBefore,
    /// `substring-after(string, string)`
    SubstringAfter,
    /// `substring(string, number, number?)`
    Substring,
    /// `string-length(string?)`
    StringLength,
    /// `normalize-space(string?)`
    NormalizeSpace,
    /// `translate(string, string, string)`
    Translate,
    /// `boolean(object)`
    Boolean,
    /// `not(boolean)`
    Not,
    /// `true()`
    True,
    /// `false()`
    False,
    /// `lang(string)`: parsed, evaluation reports [`XPathError::Unsupported`](crate::XPathError)
    Lang,
    /// `number(object?)`
    Number,
    /// `sum(node-set)`
    Sum,
    /// `floor(number)`
    Floor,
    /// `ceiling(number)`
    Ceiling,
    /// `round(number)`
    Round,
}

impl Func {
    /// The function for a written name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "last" => Self::Last,
            "position" => Self::Position,
            "count" => Self::Count,
            "id" => Self::Id,
            "local-name" => Self::LocalName,
            "namespace-uri" => Self::NamespaceUri,
            "name" => Self::Name,
            "string" => Self::String,
            "concat" => Self::Concat,
            "starts-with" => Self::StartsWith,
            "contains" => Self::Contains,
            "substring-before" => Self::SubstringBefore,
            "substring-after" => Self::SubstringAfter,
            "substring" => Self::Substring,
            "string-length" => Self::StringLength,
            "normalize-space" => Self::NormalizeSpace,
            "translate" => Self::Translate,
            "boolean" => Self::Boolean,
            "not" => Self::Not,
            "true" => Self::True,
            "false" => Self::False,
            "lang" => Self::Lang,
            "number" => Self::Number,
            "sum" => Self::Sum,
            "floor" => Self::Floor,
            "ceiling" => Self::Ceiling,
            "round" => Self::Round,
            _ => return None,
        })
    }

    /// The function name as written.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Last => "last",
            Self::Position => "position",
            Self::Count => "count",
            Self::Id => "id",
            Self::LocalName => "local-name",
            Self::NamespaceUri => "namespace-uri",
            Self::Name => "name",
            Self::String => "string",
            Self::Concat => "concat",
            Self::StartsWith => "starts-with",
            Self::Contains => "contains",
            Self::SubstringBefore => "substring-before",
            Self::SubstringAfter => "substring-after",
            Self::Substring => "substring",
            Self::StringLength => "string-length",
            Self::NormalizeSpace => "normalize-space",
            Self::Translate => "translate",
            Self::Boolean => "boolean",
            Self::Not => "not",
            Self::True => "true",
            Self::False => "false",
            Self::Lang => "lang",
            Self::Number => "number",
            Self::Sum => "sum",
            Self::Floor => "floor",
            Self::Ceiling => "ceiling",
            Self::Round => "round",
        }
    }

    /// The accepted argument counts as `(minimum, maximum)`; `None` means unbounded.
    #[must_use]
    pub fn arity(self) -> (usize, Option<usize>) {
        match self {
            Self::Last | Self::Position | Self::True | Self::False => (0, Some(0)),
            Self::Count
            | Self::Id
            | Self::Boolean
            | Self::Not
            | Self::Lang
            | Self::Sum
            | Self::Floor
            | Self::Ceiling
            | Self::Round => (1, Some(1)),
            Self::LocalName
            | Self::NamespaceUri
            | Self::Name
            | Self::String
            | Self::StringLength
            | Self::NormalizeSpace
            | Self::Number => (0, Some(1)),
            Self::Concat => (2, None),
            Self::StartsWith | Self::Contains | Self::SubstringBefore | Self::SubstringAfter => {
                (2, Some(2))
            }
            Self::Substring => (2, Some(3)),
            Self::Translate => (3, Some(3)),
        }
    }
}

/// An XPath 1.0 expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// `first op rest[0].0 rest[0].1 op ...`, left associative, all operators of one precedence
    /// level. `rest` is never empty for trees produced by the parser.
    Chain {
        /// The leftmost operand.
        first: Box<Expr>,
        /// The following operators and operands in order.
        rest: Vec<(BinOp, Expr)>,
    },
    /// Unary minus.
    Neg(Box<Expr>),
    /// A string literal.
    Literal(String),
    /// A number literal (finite, never negative).
    Number(f64),
    /// A variable reference `$name`. Parsed for completeness; [`crate::XPath::parse`] rejects it.
    Variable(String),
    /// A call of a core function.
    Function {
        /// The function.
        func: Func,
        /// The arguments.
        args: Vec<Expr>,
    },
    /// A primary expression with predicates, `(a | b)[1]`.
    Filter {
        /// The filtered expression.
        primary: Box<Expr>,
        /// The predicates, at least one.
        predicates: Vec<Expr>,
    },
    /// A path.
    Path(PathExpr),
}

impl Expr {
    /// Binding strength of the outermost construct, used by the printer.
    fn precedence(&self) -> u8 {
        match self {
            Self::Chain { rest, first: _ } => rest.first().map_or(10, |(op, _)| op.precedence()),
            Self::Neg(_) => 7,
            _ => 10,
        }
    }

    /// True for expressions that need no parentheses when used as the start of a path or as the
    /// subject of a predicate list.
    fn is_primary(&self) -> bool {
        matches!(
            self,
            Self::Literal(_) | Self::Number(_) | Self::Variable(_) | Self::Function { .. }
        )
    }

    /// The longest nesting of expressions below this one, counting this node as 1.
    ///
    /// Computed iteratively, so it is safe on trees of any depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut best = 0;
        let mut stack: Vec<(&Expr, usize)> = vec![(self, 1)];
        while let Some((e, d)) = stack.pop() {
            best = best.max(d);
            let next = d.saturating_add(1);
            e.for_each_child(&mut |c| stack.push((c, next)));
        }
        best
    }

    /// Calls `f` for every direct sub-expression, including predicates and path bases.
    pub fn for_each_child<'a>(&'a self, f: &mut dyn FnMut(&'a Expr)) {
        match self {
            Self::Chain { first, rest } => {
                f(first);
                rest.iter().for_each(|(_, e)| f(e));
            }
            Self::Neg(e) => f(e),
            Self::Literal(_) | Self::Number(_) | Self::Variable(_) => {}
            Self::Function { args, .. } => args.iter().for_each(f),
            Self::Filter {
                primary,
                predicates,
            } => {
                f(primary);
                predicates.iter().for_each(f);
            }
            Self::Path(p) => {
                if let PathBase::Expr(e) = &p.base {
                    f(e);
                }
                for s in &p.steps {
                    s.predicates.iter().for_each(&mut *f);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------ printing

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_expr(f, self, 0)
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_step(f, self)
    }
}

impl fmt::Display for NodeTest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { prefix, local } => {
                if let Some(p) = prefix {
                    write!(f, "{p}:")?;
                }
                f.write_str(local)
            }
            Self::Wildcard => f.write_char('*'),
            Self::PrefixWildcard(p) => write!(f, "{p}:*"),
            Self::Text => f.write_str("text()"),
            Self::Comment => f.write_str("comment()"),
            Self::ProcessingInstruction(None) => f.write_str("processing-instruction()"),
            Self::ProcessingInstruction(Some(t)) => {
                f.write_str("processing-instruction(")?;
                write_literal(f, t)?;
                f.write_char(')')
            }
            Self::AnyNode => f.write_str("node()"),
        }
    }
}

fn write_literal(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    let quote = if s.contains('"') { '\'' } else { '"' };
    f.write_char(quote)?;
    f.write_str(s)?;
    f.write_char(quote)
}

fn write_expr(f: &mut fmt::Formatter<'_>, e: &Expr, min_prec: u8) -> fmt::Result {
    // A lone `/` followed by an operator word would lex the word as a name test, so inside
    // operator chains the root path is parenthesised.
    let lone_root = matches!(
        e,
        Expr::Path(PathExpr {
            base: PathBase::Root,
            steps
        }) if steps.is_empty()
    );
    let paren = e.precedence() < min_prec || (lone_root && min_prec > 0);
    if paren {
        f.write_char('(')?;
    }
    match e {
        Expr::Chain { first, rest } => {
            let p = e.precedence();
            // The chain is flat and left associative, so a nested chain of the same level on
            // the left needs parentheses to survive a parse.
            write_expr(f, first, p.saturating_add(1))?;
            for (op, operand) in rest {
                write!(f, " {} ", op.symbol())?;
                write_expr(f, operand, p.saturating_add(1))?;
            }
        }
        Expr::Neg(inner) => {
            f.write_char('-')?;
            write_expr(f, inner, 7)?;
        }
        Expr::Literal(s) => write_literal(f, s)?,
        Expr::Number(n) => write!(f, "{n}")?,
        Expr::Variable(name) => write!(f, "${name}")?,
        Expr::Function { func, args } => {
            f.write_str(func.name())?;
            f.write_char('(')?;
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write_expr(f, a, 0)?;
            }
            f.write_char(')')?;
        }
        Expr::Filter {
            primary,
            predicates,
        } => {
            write_primary(f, primary)?;
            for p in predicates {
                f.write_char('[')?;
                write_expr(f, p, 0)?;
                f.write_char(']')?;
            }
        }
        Expr::Path(p) => write_path(f, p)?,
    }
    if paren {
        f.write_char(')')?;
    }
    Ok(())
}

fn write_primary(f: &mut fmt::Formatter<'_>, e: &Expr) -> fmt::Result {
    if e.is_primary() {
        write_expr(f, e, 0)
    } else {
        f.write_char('(')?;
        write_expr(f, e, 0)?;
        f.write_char(')')
    }
}

fn write_path(f: &mut fmt::Formatter<'_>, p: &PathExpr) -> fmt::Result {
    let leading = match &p.base {
        PathBase::Root => {
            if p.steps.is_empty() {
                return f.write_char('/');
            }
            true
        }
        PathBase::Context => false,
        PathBase::Expr(e) => {
            if matches!(**e, Expr::Filter { .. }) {
                write_expr(f, e, 0)?;
            } else {
                write_primary(f, e)?;
            }
            true
        }
    };
    let mut i = 0;
    while let Some(step) = p.steps.get(i) {
        let next_plain = p
            .steps
            .get(i + 1)
            .is_some_and(|n| !n.is_descendant_or_self_node());
        if step.is_descendant_or_self_node() && next_plain && (i > 0 || leading) {
            // `a//b` or `//b`: this step is the first separator, the next step writes the second.
            f.write_char('/')?;
            i += 1;
            continue;
        }
        if i > 0 || leading {
            f.write_char('/')?;
        }
        write_step(f, step)?;
        i += 1;
    }
    Ok(())
}

fn write_step(f: &mut fmt::Formatter<'_>, s: &Step) -> fmt::Result {
    if s.predicates.is_empty() && s.test == NodeTest::AnyNode {
        match s.axis {
            Axis::SelfAxis => return f.write_char('.'),
            Axis::Parent => return f.write_str(".."),
            _ => {}
        }
    }
    match s.axis {
        Axis::Child => {}
        Axis::Attribute => f.write_char('@')?,
        other => write!(f, "{}::", other.name())?,
    }
    write!(f, "{}", s.test)?;
    for p in &s.predicates {
        f.write_char('[')?;
        write_expr(f, p, 0)?;
        f.write_char(']')?;
    }
    Ok(())
}
