//! Minimal KeyValues1 text parser used only to evaluate the "write our own parser" option.
//! Grammar (informal):
//!   file   := (ws|comment)* ( macro | pair )* EOF            // several top-level pairs allowed
//!   macro  := '#base' string | '#include' string             // recorded, never followed
//!   pair   := string cond? value cond?
//!   value  := string | '{' pair* '}'
//!   string := '"' chars '"' | bare                            // bare ends at ws, '"', '{', '}'
//!   cond   := '[' ... ']'                                     // bare token starting with '[' and ending with ']'
//!   comment:= '//' to end of line
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub enum Val<'a> {
    Str(Cow<'a, str>),
    Obj(Vec<(Cow<'a, str>, Val<'a>)>),
}

#[derive(Debug)]
pub struct Doc<'a> {
    pub root: Vec<(Cow<'a, str>, Val<'a>)>,
    pub macros: Vec<(&'a str, Cow<'a, str>)>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Esc { Process, Literal }

pub struct P<'a> { s: &'a str, b: &'a [u8], i: usize, esc: Esc, macros: Vec<(&'a str, Cow<'a, str>)> }

type R<T> = Result<T, String>;

impl<'a> P<'a> {
    fn ws(&mut self) {
        loop {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n' | 0) { self.i += 1; }
            if self.b[self.i..].starts_with(b"//") {
                while self.i < self.b.len() && self.b[self.i] != b'\n' { self.i += 1; }
            } else { break; }
        }
    }
    fn tok(&mut self) -> R<Option<(Cow<'a, str>, bool)>> { // (text, quoted)
        self.ws();
        if self.i >= self.b.len() { return Ok(None); }
        match self.b[self.i] {
            b'{' | b'}' => { let c = &self.s[self.i..self.i + 1]; self.i += 1; Ok(Some((Cow::Borrowed(c), false))) }
            b'"' => {
                self.i += 1;
                let start = self.i;
                let mut has_esc = false;
                while self.i < self.b.len() && self.b[self.i] != b'"' {
                    if self.b[self.i] == b'\\' && self.esc == Esc::Process { has_esc = true; self.i += 1; }
                    self.i += 1;
                }
                if self.i >= self.b.len() { return Err("unterminated string".into()); }
                let raw = &self.s[start..self.i];
                self.i += 1;
                if !has_esc { return Ok(Some((Cow::Borrowed(raw), true))); }
                let mut out = String::with_capacity(raw.len());
                let mut it = raw.chars();
                while let Some(c) = it.next() {
                    if c == '\\' {
                        match it.next() { Some('n') => out.push('\n'), Some('t') => out.push('\t'), Some('r') => out.push('\r'),
                            Some('\\') => out.push('\\'), Some('"') => out.push('"'),
                            Some(o) => { out.push('\\'); out.push(o); } // tolerate unknown escapes
                            None => out.push('\\') }
                    } else { out.push(c) }
                }
                Ok(Some((Cow::Owned(out), true)))
            }
            _ => {
                let start = self.i;
                while self.i < self.b.len() && !matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'{' | b'}' | 0) { self.i += 1; }
                Ok(Some((Cow::Borrowed(&self.s[start..self.i]), false)))
            }
        }
    }
    fn is_cond(t: &(Cow<'a, str>, bool)) -> bool { !t.1 && t.0.starts_with('[') && t.0.ends_with(']') }
    fn pairs(&mut self, top: bool) -> R<Vec<(Cow<'a, str>, Val<'a>)>> {
        let mut out = Vec::new();
        loop {
            let Some((k, kq)) = self.tok()? else { return if top { Ok(out) } else { Err("eof in object".into()) } };
            if !kq && k == "}" { return if top { Err("stray }".into()) } else { Ok(out) }; }
            if top && !kq && (k.eq_ignore_ascii_case("#base") || k.eq_ignore_ascii_case("#include")) {
                let name = if k.eq_ignore_ascii_case("#base") { "#base" } else { "#include" };
                let Some((p, _)) = self.tok()? else { return Err("macro without path".into()) };
                self.macros.push((name, p)); continue;
            }
            let Some(mut v) = self.tok()? else { return Err("key without value".into()) };
            if Self::is_cond(&v) { v = self.tok()?.ok_or("eof after cond")?; }
            let val = if !v.1 && v.0 == "{" { Val::Obj(self.pairs(false)?) }
                      else if !v.1 && v.0 == "}" { return Err("unexpected }".into()) }
                      else { Val::Str(v.0) };
            // optional trailing conditional on a string value
            let save = self.i;
            if let Some(t) = self.tok()? { if !Self::is_cond(&t) { self.i = save; } } else { self.i = save; }
            out.push((k, val));
        }
    }
}

pub fn parse(text: &str, esc: Esc) -> R<Doc<'_>> {
    let s = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut p = P { s, b: s.as_bytes(), i: 0, esc, macros: vec![] };
    let root = p.pairs(true)?;
    Ok(Doc { root, macros: p.macros })
}

pub fn canon(v: &Val<'_>, out: &mut String) {
    match v {
        Val::Str(s) => { out.push('"'); out.push_str(s); out.push('"'); }
        Val::Obj(items) => {
            let mut sorted: Vec<&(Cow<str>, Val)> = items.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0)); // stable: duplicate keys keep file order
            out.push('{');
            for (k, v) in sorted { out.push('"'); out.push_str(k); out.push_str("\":"); canon(v, out); out.push(','); }
            out.push('}');
        }
    }
}
