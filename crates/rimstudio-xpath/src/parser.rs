//! A recursive descent parser for the full XPath 1.0 grammar.
//!
//! [`parse`] accepts every expression of the recommendation and produces an [`Expr`].
//! [`validate`] then applies the restrictions of the game's runtime, which compiles expressions
//! without a namespace resolver or variable bindings: variables and prefixed names are
//! rejected. Unknown function names and wrong argument counts are rejected by [`parse`] itself.
//!
//! Nesting (parentheses, predicates, function arguments, unary minus chains) is limited to
//! [`MAX_NESTING`] levels; a deeper expression is an [`XPathError::TooDeep`] error. Operator
//! chains and long paths are flat vectors, so they have no depth limit of their own.

use crate::ast::{Axis, BinOp, Expr, Func, NodeTest, PathBase, PathExpr, Step};
use crate::error::{XPathError, XPathResult};
use crate::lexer::{NodeType, Spanned, Token, tokenize};

/// The deepest nesting of parentheses, predicates, function arguments and unary minus signs.
pub const MAX_NESTING: usize = 64;

/// Parses an expression. The text is used as is; trimming is the caller's business (see
/// [`crate::XPath::parse`]).
///
/// # Errors
/// [`XPathError::Parse`] for syntax errors, [`XPathError::UnknownFunction`],
/// [`XPathError::Arity`] and [`XPathError::TooDeep`].
pub fn parse(src: &str) -> XPathResult<Expr> {
    let tokens = tokenize(src)?;
    let mut p = Parser {
        tokens: &tokens,
        idx: 0,
        depth: 0,
        end: src.len(),
    };
    let expr = p.parse_or()?;
    match p.tokens.get(p.idx) {
        None => Ok(expr),
        Some(extra) => Err(XPathError::parse(
            extra.start,
            format!("unexpected {}", describe(&extra.token)),
        )),
    }
}

/// Rejects what the game's runtime rejects at compile time: variable references and names with
/// a namespace prefix (including `prefix:*`).
///
/// # Errors
/// [`XPathError::UnboundVariable`] or [`XPathError::UnboundPrefix`].
pub fn validate(src: &str, expr: &Expr) -> XPathResult<()> {
    // The AST carries no positions, so the first offender is located by re-lexing.
    let mut stack: Vec<&Expr> = vec![expr];
    let mut bad: Option<XPathError> = None;
    while let Some(e) = stack.pop() {
        match e {
            Expr::Variable(name) => {
                bad.get_or_insert_with(|| XPathError::UnboundVariable {
                    pos: find_token(src, |t| matches!(t, Token::Variable(n) if n == name)),
                    name: name.clone(),
                });
            }
            Expr::Path(p) => {
                for s in &p.steps {
                    let prefix = match &s.test {
                        NodeTest::Name {
                            prefix: Some(p), ..
                        }
                        | NodeTest::PrefixWildcard(p) => Some(p),
                        _ => None,
                    };
                    if let Some(prefix) = prefix {
                        bad.get_or_insert_with(|| XPathError::UnboundPrefix {
                            pos: find_token(src, |t| {
                                matches!(t, Token::Name(n) if n.starts_with(&format!("{prefix}:")))
                            }),
                            prefix: prefix.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
        e.for_each_child(&mut |c| stack.push(c));
    }
    bad.map_or(Ok(()), Err)
}

fn find_token(src: &str, pred: impl Fn(&Token) -> bool) -> usize {
    tokenize(src)
        .ok()
        .and_then(|t| t.into_iter().find(|s| pred(&s.token)).map(|s| s.start))
        .unwrap_or(0)
}

fn describe(t: &Token) -> String {
    match t {
        Token::Number(n) => format!("number {n}"),
        Token::Literal(s) => format!("literal {s:?}"),
        Token::Variable(v) => format!("variable ${v}"),
        Token::Name(n) => format!("name {n:?}"),
        Token::Star => "'*'".into(),
        Token::Axis(a) => format!("axis {}::", a.name()),
        Token::NodeType(_) => "node type test".into(),
        Token::Function(n) => format!("function name {n:?}"),
        Token::Op(op) => format!("operator {:?}", op.symbol()),
        Token::Slash => "'/'".into(),
        Token::DoubleSlash => "'//'".into(),
        Token::LParen => "'('".into(),
        Token::RParen => "')'".into(),
        Token::LBracket => "'['".into(),
        Token::RBracket => "']'".into(),
        Token::Dot => "'.'".into(),
        Token::DotDot => "'..'".into(),
        Token::At => "'@'".into(),
        Token::Comma => "','".into(),
    }
}

struct Parser<'a> {
    tokens: &'a [Spanned],
    idx: usize,
    depth: usize,
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.idx).map(|s| &s.token)
    }

    fn pos(&self) -> usize {
        self.tokens.get(self.idx).map_or(self.end, |s| s.start)
    }

    fn bump(&mut self) {
        self.idx += 1;
    }

    fn eat(&mut self, t: &Token) -> bool {
        if self.peek() == Some(t) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Token, what: &str) -> XPathResult<()> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(self.unexpected(what))
        }
    }

    fn unexpected(&self, expected: &str) -> XPathError {
        let found = self
            .peek()
            .map_or_else(|| "end of expression".to_owned(), describe);
        XPathError::parse(self.pos(), format!("expected {expected} but found {found}"))
    }

    fn enter(&mut self) -> XPathResult<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            Err(XPathError::TooDeep { limit: MAX_NESTING })
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // ------------------------------------------------------------ binary levels

    fn parse_or(&mut self) -> XPathResult<Expr> {
        self.enter()?;
        let r = self.chain(&[BinOp::Or], Self::parse_and);
        self.leave();
        r
    }

    fn parse_and(&mut self) -> XPathResult<Expr> {
        self.chain(&[BinOp::And], Self::parse_equality)
    }

    fn parse_equality(&mut self) -> XPathResult<Expr> {
        self.chain(&[BinOp::Eq, BinOp::Neq], Self::parse_relational)
    }

    fn parse_relational(&mut self) -> XPathResult<Expr> {
        self.chain(
            &[BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge],
            Self::parse_additive,
        )
    }

    fn parse_additive(&mut self) -> XPathResult<Expr> {
        self.chain(&[BinOp::Add, BinOp::Sub], Self::parse_multiplicative)
    }

    fn parse_multiplicative(&mut self) -> XPathResult<Expr> {
        self.chain(&[BinOp::Mul, BinOp::Div, BinOp::Mod], Self::parse_unary)
    }

    fn chain(
        &mut self,
        ops: &[BinOp],
        next: fn(&mut Self) -> XPathResult<Expr>,
    ) -> XPathResult<Expr> {
        let first = next(self)?;
        let mut rest = Vec::new();
        while let Some(Token::Op(op)) = self.peek() {
            let op = *op;
            if !ops.contains(&op) {
                break;
            }
            self.bump();
            rest.push((op, next(self)?));
        }
        if rest.is_empty() {
            Ok(first)
        } else {
            Ok(Expr::Chain {
                first: Box::new(first),
                rest,
            })
        }
    }

    fn parse_unary(&mut self) -> XPathResult<Expr> {
        if self.peek() == Some(&Token::Op(BinOp::Sub)) {
            self.bump();
            self.enter()?;
            let inner = self.parse_unary();
            self.leave();
            return Ok(Expr::Neg(Box::new(inner?)));
        }
        self.parse_union()
    }

    fn parse_union(&mut self) -> XPathResult<Expr> {
        self.chain(&[BinOp::Union], Self::parse_path_expr)
    }

    // ------------------------------------------------------------ paths

    fn starts_step(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Token::Name(_)
                    | Token::Star
                    | Token::Axis(_)
                    | Token::NodeType(_)
                    | Token::At
                    | Token::Dot
                    | Token::DotDot
            )
        )
    }

    fn starts_primary(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Token::Variable(_)
                    | Token::LParen
                    | Token::Literal(_)
                    | Token::Number(_)
                    | Token::Function(_)
            )
        )
    }

    fn parse_path_expr(&mut self) -> XPathResult<Expr> {
        match self.peek() {
            Some(Token::Slash) => {
                self.bump();
                let mut steps = Vec::new();
                if self.starts_step() {
                    self.relative_steps(&mut steps)?;
                }
                Ok(Expr::Path(PathExpr {
                    base: PathBase::Root,
                    steps,
                }))
            }
            Some(Token::DoubleSlash) => {
                self.bump();
                let mut steps = vec![descendant_or_self()];
                if !self.starts_step() {
                    return Err(self.unexpected("a location step after '//'"));
                }
                self.relative_steps(&mut steps)?;
                Ok(Expr::Path(PathExpr {
                    base: PathBase::Root,
                    steps,
                }))
            }
            _ if self.starts_primary() => {
                let filter = self.parse_filter()?;
                let mut steps = Vec::new();
                match self.peek() {
                    Some(Token::Slash) => {
                        self.bump();
                        if !self.starts_step() {
                            return Err(self.unexpected("a location step after '/'"));
                        }
                        self.relative_steps(&mut steps)?;
                    }
                    Some(Token::DoubleSlash) => {
                        self.bump();
                        steps.push(descendant_or_self());
                        if !self.starts_step() {
                            return Err(self.unexpected("a location step after '//'"));
                        }
                        self.relative_steps(&mut steps)?;
                    }
                    _ => return Ok(filter),
                }
                Ok(Expr::Path(PathExpr {
                    base: PathBase::Expr(Box::new(filter)),
                    steps,
                }))
            }
            _ if self.starts_step() => {
                let mut steps = Vec::new();
                self.relative_steps(&mut steps)?;
                Ok(Expr::Path(PathExpr {
                    base: PathBase::Context,
                    steps,
                }))
            }
            _ => Err(self.unexpected("an expression")),
        }
    }

    /// Parses `step ('/' step | '//' step)*`.
    fn relative_steps(&mut self, steps: &mut Vec<Step>) -> XPathResult<()> {
        steps.push(self.parse_step()?);
        loop {
            match self.peek() {
                Some(Token::Slash) => {
                    self.bump();
                }
                Some(Token::DoubleSlash) => {
                    self.bump();
                    steps.push(descendant_or_self());
                }
                _ => return Ok(()),
            }
            if !self.starts_step() {
                return Err(self.unexpected("a location step"));
            }
            steps.push(self.parse_step()?);
        }
    }

    fn parse_step(&mut self) -> XPathResult<Step> {
        match self.peek() {
            Some(Token::Dot) => {
                self.bump();
                return Ok(Step::new(Axis::SelfAxis, NodeTest::AnyNode));
            }
            Some(Token::DotDot) => {
                self.bump();
                return Ok(Step::new(Axis::Parent, NodeTest::AnyNode));
            }
            _ => {}
        }
        let axis = match self.peek() {
            Some(Token::Axis(a)) => {
                let a = *a;
                self.bump();
                a
            }
            Some(Token::At) => {
                self.bump();
                Axis::Attribute
            }
            _ => Axis::Child,
        };
        let test = self.parse_node_test()?;
        let predicates = self.parse_predicates()?;
        Ok(Step {
            axis,
            test,
            predicates,
        })
    }

    fn parse_node_test(&mut self) -> XPathResult<NodeTest> {
        match self.peek().cloned() {
            Some(Token::Star) => {
                self.bump();
                Ok(NodeTest::Wildcard)
            }
            Some(Token::Name(text)) => {
                self.bump();
                if let Some(prefix) = text.strip_suffix(":*") {
                    return Ok(NodeTest::PrefixWildcard(prefix.to_owned()));
                }
                Ok(match text.split_once(':') {
                    Some((prefix, local)) => NodeTest::Name {
                        prefix: Some(prefix.to_owned()),
                        local: local.to_owned(),
                    },
                    None => NodeTest::Name {
                        prefix: None,
                        local: text,
                    },
                })
            }
            Some(Token::NodeType(kind)) => {
                self.bump();
                self.expect(&Token::LParen, "'('")?;
                let test = match kind {
                    NodeType::Comment => NodeTest::Comment,
                    NodeType::Text => NodeTest::Text,
                    NodeType::Node => NodeTest::AnyNode,
                    NodeType::ProcessingInstruction => {
                        if let Some(Token::Literal(target)) = self.peek().cloned() {
                            self.bump();
                            NodeTest::ProcessingInstruction(Some(target))
                        } else {
                            NodeTest::ProcessingInstruction(None)
                        }
                    }
                };
                self.expect(&Token::RParen, "')'")?;
                Ok(test)
            }
            _ => Err(self.unexpected("a node test")),
        }
    }

    fn parse_predicates(&mut self) -> XPathResult<Vec<Expr>> {
        let mut out = Vec::new();
        while self.peek() == Some(&Token::LBracket) {
            self.bump();
            out.push(self.parse_or()?);
            self.expect(&Token::RBracket, "']'")?;
        }
        Ok(out)
    }

    // ------------------------------------------------------------ primary and filter

    fn parse_filter(&mut self) -> XPathResult<Expr> {
        let primary = self.parse_primary()?;
        let predicates = self.parse_predicates()?;
        if predicates.is_empty() {
            Ok(primary)
        } else {
            Ok(Expr::Filter {
                primary: Box::new(primary),
                predicates,
            })
        }
    }

    fn parse_primary(&mut self) -> XPathResult<Expr> {
        let start = self.pos();
        match self.peek().cloned() {
            Some(Token::Variable(name)) => {
                self.bump();
                Ok(Expr::Variable(name))
            }
            Some(Token::LParen) => {
                self.bump();
                let inner = self.parse_or()?;
                self.expect(&Token::RParen, "')'")?;
                Ok(inner)
            }
            Some(Token::Literal(s)) => {
                self.bump();
                Ok(Expr::Literal(s))
            }
            Some(Token::Number(n)) => {
                self.bump();
                Ok(Expr::Number(n))
            }
            Some(Token::Function(name)) => {
                self.bump();
                self.expect(&Token::LParen, "'('")?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        args.push(self.parse_or()?);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Token::RParen, "')'")?;
                let Some(func) = Func::from_name(&name) else {
                    return Err(XPathError::UnknownFunction { pos: start, name });
                };
                let (min, max) = func.arity();
                let n = args.len();
                if n < min || max.is_some_and(|m| n > m) {
                    let expected = match max {
                        Some(m) if m == min => format!("{min}"),
                        Some(m) => format!("{min} to {m}"),
                        None => format!("at least {min}"),
                    };
                    return Err(XPathError::Arity {
                        pos: start,
                        name,
                        expected,
                        found: n,
                    });
                }
                Ok(Expr::Function { func, args })
            }
            _ => Err(self.unexpected("an expression")),
        }
    }
}

fn descendant_or_self() -> Step {
    Step::new(Axis::DescendantOrSelf, NodeTest::AnyNode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn roundtrip(src: &str) -> String {
        parse(src)
            .map(|e| e.to_string())
            .unwrap_or_else(|e| e.to_string())
    }

    #[rstest]
    #[case("Defs/ThingDef[defName=\"X\"]", "Defs/ThingDef[defName = \"X\"]")]
    #[case("/Defs//li", "/Defs//li")]
    #[case("//li", "//li")]
    #[case(".//a", ".//a")]
    #[case("a/../b", "a/../b")]
    #[case("@x", "@x")]
    #[case("child::a/following-sibling::*[1]", "a/following-sibling::*[1]")]
    #[case("1 + 2 * 3", "1 + 2 * 3")]
    #[case("(1 + 2) * 3", "(1 + 2) * 3")]
    #[case("a | b | c", "a | b | c")]
    #[case("-1", "-1")]
    #[case("not(a) and b or c", "not(a) and b or c")]
    #[case("(a | b)[1]/c", "(a | b)[1]/c")]
    #[case("/", "/")]
    fn printing_is_stable(#[case] src: &str, #[case] printed: &str) {
        assert_eq!(roundtrip(src), printed);
        assert_eq!(roundtrip(printed), printed);
    }

    #[rstest]
    #[case("")]
    #[case("a[")]
    #[case("a/")]
    #[case("a b")]
    #[case("a ==  b")]
    #[case("f()")]
    #[case("count()")]
    #[case("substring('a')")]
    #[case("a[1")]
    #[case("(a")]
    #[case("a//")]
    #[case("1 +")]
    #[case(".[1]")]
    fn invalid_syntax_is_rejected(#[case] src: &str) {
        assert!(parse(src).is_err(), "{src}");
    }

    #[test]
    fn validate_rejects_variables_and_prefixes() {
        let e = parse("a[$v]").map(|e| validate("a[$v]", &e));
        assert_eq!(
            e.ok().and_then(Result::err).map(|e| e.code()),
            Some("xpath.unbound-variable")
        );
        let e = parse("x:a").map(|e| validate("x:a", &e));
        assert_eq!(
            e.ok().and_then(Result::err).map(|e| e.position()),
            Some(Some(0))
        );
        let e = parse("a/x:*").map(|e| validate("a/x:*", &e));
        assert_eq!(
            e.ok().and_then(Result::err).map(|e| e.code()),
            Some("xpath.unbound-prefix")
        );
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        let src = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
        assert_eq!(parse(&src).err().map(|e| e.code()), Some("xpath.too-deep"));
        let neg = format!("{}1", "-".repeat(5000));
        assert_eq!(parse(&neg).err().map(|e| e.code()), Some("xpath.too-deep"));
        let preds = format!("a{}{}", "[b".repeat(5000), "]".repeat(5000));
        assert_eq!(
            parse(&preds).err().map(|e| e.code()),
            Some("xpath.too-deep")
        );
    }

    #[test]
    fn long_flat_chains_are_fine() {
        let src = vec!["a"; 20_000].join(" or ");
        let e = parse(&src);
        assert!(e.is_ok());
        assert_eq!(e.map(|e| e.depth()).unwrap_or(0), 2);
        let path = vec!["a"; 20_000].join("/");
        assert!(parse(&path).is_ok());
    }

    #[test]
    fn arity_errors_name_the_function() {
        let e = parse("substring('a')").err();
        assert!(matches!(e, Some(XPathError::Arity { found: 1, .. })));
        let e = parse("nope(1)").err();
        assert!(matches!(e, Some(XPathError::UnknownFunction { .. })));
    }
}
