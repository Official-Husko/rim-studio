//! The XPath 1.0 lexer, including the disambiguation rules of section 3.7 of the
//! recommendation.
//!
//! Whether `*` is the multiplication operator or a name test, and whether `and`, `or`, `mod`
//! and `div` are operators or element names, depends on the previous token. A name followed by
//! `(` is a function name or a node type, a name followed by `::` is an axis name. The lexer
//! resolves all of that, so the parser works on unambiguous tokens.
//!
//! White space between tokens is space, tab, carriage return and line feed. Names follow the
//! `NCName` production closely enough for real documents: a letter or `_` first, then letters,
//! digits, `.`, `-`, `_`, the middle dot and combining marks (any Unicode alphabetic character
//! counts as a letter).

use crate::ast::{Axis, BinOp};
use crate::error::{XPathError, XPathResult};

/// The node type keywords that take parentheses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// `comment`
    Comment,
    /// `text`
    Text,
    /// `processing-instruction`
    ProcessingInstruction,
    /// `node`
    Node,
}

/// A lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A number literal.
    Number(f64),
    /// A string literal without its quotes.
    Literal(String),
    /// `$qname`
    Variable(String),
    /// A name test: `name`, `prefix:name` or `prefix:*` (the text as written).
    Name(String),
    /// A `*` used as a name test.
    Star,
    /// An axis name; the following `::` is consumed with it.
    Axis(Axis),
    /// A node type keyword followed by `(`; the `(` is not consumed.
    NodeType(NodeType),
    /// A function name followed by `(`; the `(` is not consumed.
    Function(String),
    /// A binary operator word or symbol.
    Op(BinOp),
    /// `/`
    Slash,
    /// `//`
    DoubleSlash,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `@`
    At,
    /// `,`
    Comma,
}

/// A token with its byte range in the source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    /// The token.
    pub token: Token,
    /// Byte offset of the first character.
    pub start: usize,
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn is_name_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_name_char(c: char) -> bool {
    is_name_start(c)
        || c.is_ascii_digit()
        || matches!(c, '.' | '-' | '\u{B7}')
        || (!c.is_ascii() && (c.is_alphanumeric() || ('\u{300}'..='\u{36F}').contains(&c)))
}

/// Splits an expression into tokens.
///
/// # Errors
/// [`XPathError::Parse`] for an unterminated literal, a character that starts no token, an
/// unknown axis name, a name where an operator is required, or a number outside the range of a
/// double.
pub fn tokenize(src: &str) -> XPathResult<Vec<Spanned>> {
    let mut lx = Lexer {
        src,
        pos: 0,
        out: Vec::new(),
    };
    lx.run()?;
    Ok(lx.out)
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    out: Vec<Spanned>,
}

impl Lexer<'_> {
    fn rest(&self) -> &str {
        self.src.get(self.pos..).unwrap_or("")
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn peek2(&self) -> Option<char> {
        self.rest().chars().nth(1)
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_ws) {
            self.bump();
        }
    }

    fn push(&mut self, start: usize, token: Token) {
        self.out.push(Spanned { token, start });
    }

    /// True when the previous token ends an operand, so the next `*` or name is an operator.
    fn operator_expected(&self) -> bool {
        match self.out.last().map(|s| &s.token) {
            None => false,
            Some(
                Token::At
                | Token::Axis(_)
                | Token::LParen
                | Token::LBracket
                | Token::Comma
                | Token::Op(_)
                | Token::Slash
                | Token::DoubleSlash,
            ) => false,
            Some(_) => true,
        }
    }

    fn run(&mut self) -> XPathResult<()> {
        loop {
            self.skip_ws();
            let start = self.pos;
            let Some(c) = self.peek() else {
                return Ok(());
            };
            match c {
                '(' => self.single(start, Token::LParen),
                ')' => self.single(start, Token::RParen),
                '[' => self.single(start, Token::LBracket),
                ']' => self.single(start, Token::RBracket),
                '@' => self.single(start, Token::At),
                ',' => self.single(start, Token::Comma),
                '|' => self.single(start, Token::Op(BinOp::Union)),
                '+' => self.single(start, Token::Op(BinOp::Add)),
                '-' => self.single(start, Token::Op(BinOp::Sub)),
                '=' => self.single(start, Token::Op(BinOp::Eq)),
                '!' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        self.push(start, Token::Op(BinOp::Neq));
                    } else {
                        return Err(XPathError::parse(start, "expected '=' after '!'"));
                    }
                }
                '<' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        self.push(start, Token::Op(BinOp::Le));
                    } else {
                        self.push(start, Token::Op(BinOp::Lt));
                    }
                }
                '>' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        self.push(start, Token::Op(BinOp::Ge));
                    } else {
                        self.push(start, Token::Op(BinOp::Gt));
                    }
                }
                '/' => {
                    self.bump();
                    if self.peek() == Some('/') {
                        self.bump();
                        self.push(start, Token::DoubleSlash);
                    } else {
                        self.push(start, Token::Slash);
                    }
                }
                '*' => {
                    self.bump();
                    let t = if self.operator_expected() {
                        Token::Op(BinOp::Mul)
                    } else {
                        Token::Star
                    };
                    self.push(start, t);
                }
                '"' | '\'' => self.literal(start, c)?,
                '$' => self.variable(start)?,
                '.' => {
                    if self.peek2().is_some_and(|d| d.is_ascii_digit()) {
                        self.number(start)?;
                    } else {
                        self.bump();
                        if self.peek() == Some('.') {
                            self.bump();
                            self.push(start, Token::DotDot);
                        } else {
                            self.push(start, Token::Dot);
                        }
                    }
                }
                d if d.is_ascii_digit() => self.number(start)?,
                n if is_name_start(n) => self.name(start)?,
                other => {
                    return Err(XPathError::parse(
                        start,
                        format!("unexpected character {other:?}"),
                    ));
                }
            }
        }
    }

    fn single(&mut self, start: usize, token: Token) {
        self.bump();
        self.push(start, token);
    }

    fn literal(&mut self, start: usize, quote: char) -> XPathResult<()> {
        self.bump();
        let body_start = self.pos;
        loop {
            match self.peek() {
                None => return Err(XPathError::parse(start, "unterminated string literal")),
                Some(c) if c == quote => {
                    let body = self.src.get(body_start..self.pos).unwrap_or("").to_owned();
                    self.bump();
                    self.push(start, Token::Literal(body));
                    return Ok(());
                }
                Some(_) => self.bump(),
            }
        }
    }

    fn number(&mut self, start: usize) -> XPathResult<()> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some('.') {
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        let text = self.src.get(start..self.pos).unwrap_or("");
        let normalized = if text.starts_with('.') {
            format!("0{text}")
        } else {
            text.to_owned()
        };
        match normalized.parse::<f64>() {
            Ok(v) if v.is_finite() => {
                self.push(start, Token::Number(v));
                Ok(())
            }
            _ => Err(XPathError::parse(start, "number is out of range")),
        }
    }

    fn scan_ncname(&mut self) {
        if self.peek().is_some_and(is_name_start) {
            self.bump();
            while self.peek().is_some_and(is_name_char) {
                self.bump();
            }
        }
    }

    /// Scans `NCName`, `NCName:NCName` or `NCName:*` and returns its text.
    fn scan_qname(&mut self) -> String {
        let begin = self.pos;
        self.scan_ncname();
        if self.peek() == Some(':') && self.peek2() != Some(':') {
            match self.peek2() {
                Some('*') => {
                    self.bump();
                    self.bump();
                }
                Some(c) if is_name_start(c) => {
                    self.bump();
                    self.scan_ncname();
                }
                _ => {}
            }
        }
        self.src.get(begin..self.pos).unwrap_or("").to_owned()
    }

    fn variable(&mut self, start: usize) -> XPathResult<()> {
        self.bump();
        if !self.peek().is_some_and(is_name_start) {
            return Err(XPathError::parse(
                start,
                "expected a variable name after '$'",
            ));
        }
        let name = self.scan_qname();
        self.push(start, Token::Variable(name));
        Ok(())
    }

    /// The next significant character after optional white space, without consuming anything.
    fn next_significant(&self) -> (Option<char>, Option<char>) {
        let mut it = self.rest().chars().skip_while(|c| is_ws(*c));
        (it.next(), it.next())
    }

    fn name(&mut self, start: usize) -> XPathResult<()> {
        let text = self.scan_qname();
        if self.operator_expected() {
            let op = match text.as_str() {
                "and" => BinOp::And,
                "or" => BinOp::Or,
                "mod" => BinOp::Mod,
                "div" => BinOp::Div,
                _ => {
                    return Err(XPathError::parse(
                        start,
                        format!("unexpected name {text:?}: an operator is expected here"),
                    ));
                }
            };
            self.push(start, Token::Op(op));
            return Ok(());
        }
        let (c1, c2) = self.next_significant();
        if c1 == Some('(') {
            let token = match text.as_str() {
                "comment" => Token::NodeType(NodeType::Comment),
                "text" => Token::NodeType(NodeType::Text),
                "processing-instruction" => Token::NodeType(NodeType::ProcessingInstruction),
                "node" => Token::NodeType(NodeType::Node),
                _ => Token::Function(text),
            };
            self.push(start, token);
            return Ok(());
        }
        if c1 == Some(':') && c2 == Some(':') && !text.contains(':') {
            let Some(axis) = Axis::from_name(&text) else {
                return Err(XPathError::parse(start, format!("unknown axis {text:?}")));
            };
            self.skip_ws();
            self.bump();
            self.bump();
            self.push(start, Token::Axis(axis));
            return Ok(());
        }
        self.push(start, Token::Name(text));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Token> {
        tokenize(src)
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.token)
            .collect()
    }

    #[test]
    fn star_after_operand_is_multiplication() {
        assert_eq!(
            kinds("2 * 3"),
            vec![
                Token::Number(2.0),
                Token::Op(BinOp::Mul),
                Token::Number(3.0)
            ]
        );
        assert_eq!(
            kinds("a/*"),
            vec![Token::Name("a".into()), Token::Slash, Token::Star]
        );
        assert_eq!(
            kinds("* * *"),
            vec![Token::Star, Token::Op(BinOp::Mul), Token::Star]
        );
    }

    #[test]
    fn operator_words_are_names_where_an_operand_is_expected() {
        assert_eq!(kinds("and"), vec![Token::Name("and".into())]);
        assert_eq!(
            kinds("a and b"),
            vec![
                Token::Name("a".into()),
                Token::Op(BinOp::And),
                Token::Name("b".into())
            ]
        );
        assert_eq!(kinds("/div"), vec![Token::Slash, Token::Name("div".into())]);
    }

    #[test]
    fn functions_axes_and_node_types_are_resolved() {
        assert_eq!(
            kinds("child :: a"),
            vec![Token::Axis(Axis::Child), Token::Name("a".into())]
        );
        assert_eq!(
            kinds("text()"),
            vec![
                Token::NodeType(NodeType::Text),
                Token::LParen,
                Token::RParen
            ]
        );
        assert_eq!(kinds("count (a)")[0], Token::Function("count".into()));
    }

    #[test]
    fn names_may_contain_hyphens_dots_and_prefixes() {
        assert_eq!(kinds("a-b.c"), vec![Token::Name("a-b.c".into())]);
        assert_eq!(kinds("p:q"), vec![Token::Name("p:q".into())]);
        assert_eq!(kinds("p:*"), vec![Token::Name("p:*".into())]);
    }

    #[test]
    fn numbers_and_dots() {
        assert_eq!(kinds(".5"), vec![Token::Number(0.5)]);
        assert_eq!(kinds("3."), vec![Token::Number(3.0)]);
        assert_eq!(kinds(". .."), vec![Token::Dot, Token::DotDot]);
    }

    #[test]
    fn bad_input_is_an_error_with_a_position() {
        let e = tokenize("a = 'x").err();
        assert_eq!(e.map(|e| e.code()), Some("xpath.parse"));
        let e = tokenize("a ! b").err().and_then(|e| e.position());
        assert_eq!(e, Some(2));
        assert!(tokenize("{x}").is_err());
        assert!(tokenize("bogus::a").is_err());
        assert!(tokenize("1 a").is_err());
    }
}
