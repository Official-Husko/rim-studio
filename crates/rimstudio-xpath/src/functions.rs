//! The XPath 1.0 core function library.

use rimstudio_core::tree::NodeKind;

use crate::ast::{Expr, Func};
use crate::coerce::{is_xpath_space, node_string, to_boolean};
use crate::error::{XPathError, XPathResult};
use crate::eval::{Ctx, Evaluator};
use crate::value::{Value, XNode};

impl Evaluator<'_> {
    /// Calls a core function. Arities were validated by the parser.
    pub(crate) fn call(&mut self, func: Func, args: &[Expr], ctx: Ctx) -> XPathResult<Value> {
        let mut vals: Vec<Value> = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval(a, ctx)?);
        }
        let doc = self.doc;
        let string_arg = |this: &Self, i: usize| -> String {
            vals.get(i).map_or_else(
                || node_string(doc, ctx.node).into_owned(),
                |v| this.string_of(v),
            )
        };
        let plain = |this: &Self, i: usize| -> String {
            vals.get(i).map_or_else(String::new, |v| this.string_of(v))
        };
        Ok(match func {
            Func::Last => Value::Number(ctx.size as f64),
            Func::Position => Value::Number(ctx.position as f64),
            Func::Count => match vals.first() {
                Some(Value::NodeSet(s)) => Value::Number(s.len() as f64),
                _ => return Err(not_a_set(func, vals.first())),
            },
            Func::Id => Value::NodeSet(Vec::new()),
            Func::LocalName | Func::NamespaceUri | Func::Name => {
                let node = match vals.first() {
                    None => Some(ctx.node),
                    Some(Value::NodeSet(s)) => s.first().copied(),
                    other => return Err(not_a_set(func, other)),
                };
                let name = node.map_or_else(String::new, |n| self.node_name(n));
                match func {
                    Func::Name => Value::String(name),
                    Func::LocalName => Value::String(
                        name.split_once(':')
                            .map_or(name.as_str(), |(_, l)| l)
                            .to_owned(),
                    ),
                    _ => Value::String(String::new()),
                }
            }
            Func::String => Value::String(string_arg(self, 0)),
            Func::Concat => {
                let mut out = String::new();
                for v in &vals {
                    out.push_str(&self.string_of(v));
                }
                Value::String(out)
            }
            Func::StartsWith => Value::Boolean(plain(self, 0).starts_with(plain(self, 1).as_str())),
            Func::Contains => Value::Boolean(plain(self, 0).contains(plain(self, 1).as_str())),
            Func::SubstringBefore => {
                let (s, t) = (plain(self, 0), plain(self, 1));
                Value::String(
                    s.find(t.as_str())
                        .map_or_else(String::new, |i| s.get(..i).unwrap_or_default().to_owned()),
                )
            }
            Func::SubstringAfter => {
                let (s, t) = (plain(self, 0), plain(self, 1));
                Value::String(s.find(t.as_str()).map_or_else(String::new, |i| {
                    s.get(i + t.len()..).unwrap_or_default().to_owned()
                }))
            }
            Func::Substring => {
                let s = plain(self, 0);
                let start = vals.get(1).map_or(f64::NAN, |v| self.number_of(v));
                let len = vals.get(2).map(|v| self.number_of(v));
                Value::String(self.substring(&s, start, len))
            }
            Func::StringLength => Value::Number(string_arg(self, 0).chars().count() as f64),
            Func::NormalizeSpace => {
                let s = string_arg(self, 0);
                let words: Vec<&str> = s.split(is_xpath_space).filter(|w| !w.is_empty()).collect();
                Value::String(words.join(" "))
            }
            Func::Translate => {
                let (s, from, to) = (plain(self, 0), plain(self, 1), plain(self, 2));
                Value::String(translate(&s, &from, &to))
            }
            Func::Boolean => Value::Boolean(vals.first().is_some_and(to_boolean)),
            Func::Not => Value::Boolean(!vals.first().is_some_and(to_boolean)),
            Func::True => Value::Boolean(true),
            Func::False => Value::Boolean(false),
            Func::Lang => {
                return Err(XPathError::Unsupported {
                    what: "the lang() function".to_owned(),
                });
            }
            Func::Number => Value::Number(vals.first().map_or_else(
                || crate::coerce::string_to_number(&node_string(doc, ctx.node)),
                |v| self.number_of(v),
            )),
            Func::Sum => match vals.first() {
                Some(Value::NodeSet(s)) => Value::Number(
                    s.iter()
                        .map(|n| crate::coerce::string_to_number(&node_string(doc, *n)))
                        .sum(),
                ),
                other => return Err(not_a_set(func, other)),
            },
            Func::Floor => Value::Number(self.arg_number(&vals, 0).floor()),
            Func::Ceiling => Value::Number(self.arg_number(&vals, 0).ceil()),
            Func::Round => Value::Number(self.round(self.arg_number(&vals, 0))),
        })
    }

    fn arg_number(&self, vals: &[Value], i: usize) -> f64 {
        vals.get(i).map_or(f64::NAN, |v| self.number_of(v))
    }

    /// The `name()` of a node: the element or attribute name, empty for the others.
    fn node_name(&self, n: XNode) -> String {
        match n {
            XNode::Node(id) => {
                if self.doc.kind(id) == Some(NodeKind::Element) {
                    self.doc.name(id).unwrap_or_default().to_owned()
                } else {
                    String::new()
                }
            }
            XNode::Attr { owner, index } => self
                .doc
                .attr_at(owner, index as usize)
                .map_or_else(String::new, |(k, _)| k.to_owned()),
        }
    }

    fn substring(&self, s: &str, start: f64, len: Option<f64>) -> String {
        let start = self.round(start);
        let end = match len {
            Some(l) => start + self.round(l),
            None => f64::INFINITY,
        };
        s.chars()
            .enumerate()
            .filter(|(i, _)| {
                let p = (*i + 1) as f64;
                p >= start && p < end
            })
            .map(|(_, c)| c)
            .collect()
    }
}

fn not_a_set(func: Func, got: Option<&Value>) -> XPathError {
    XPathError::type_error(format!(
        "{}() needs a node-set argument, found {}",
        func.name(),
        got.map_or("nothing", Value::type_name)
    ))
}

fn translate(s: &str, from: &str, to: &str) -> String {
    let from: Vec<char> = from.chars().collect();
    let to: Vec<char> = to.chars().collect();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match from.iter().position(|f| *f == c) {
            Some(i) => {
                if let Some(r) = to.get(i) {
                    out.push(*r);
                }
            }
            None => out.push(c),
        }
    }
    out
}
