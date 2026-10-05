//! Stage S3: building a def index (defName, def type, Name, ParentName, Abstract, offset) from Defs XML.
//! Three implementations with the same output: roxmltree DOM, quick-xml streaming, and a hand written
//! structural scanner on top of memchr.

use memchr::{memchr, memchr3, memmem};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// A def element as seen by a parser. Borrowed where possible.
#[derive(Debug, Clone)]
pub struct DefRef<'a> {
    pub tag: Cow<'a, str>,
    pub class: Option<Cow<'a, str>>,
    pub def_name: Option<Cow<'a, str>>,
    pub name: Option<Cow<'a, str>>,
    pub parent: Option<Cow<'a, str>>,
    pub is_abstract: bool,
    pub offset: u32,
}

/// Owned form used by the index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct DefRec {
    pub tag: String,
    pub class: String,
    pub def_name: String,
    pub name: String,
    pub parent: String,
    pub is_abstract: bool,
    pub offset: u32,
}

impl DefRef<'_> {
    /// Moves owned strings, copies borrowed ones.
    pub fn into_owned_rec(self) -> DefRec {
        DefRec {
            tag: self.tag.into_owned(),
            class: self.class.map(|c| c.into_owned()).unwrap_or_default(),
            def_name: self.def_name.map(|c| c.into_owned()).unwrap_or_default(),
            name: self.name.map(|c| c.into_owned()).unwrap_or_default(),
            parent: self.parent.map(|c| c.into_owned()).unwrap_or_default(),
            is_abstract: self.is_abstract,
            offset: self.offset,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ScanInfo {
    pub defs: u32,
    /// root element is not named "Defs" (the game logs an error but still imports the children)
    pub root_not_defs: bool,
}

pub type Emit<'e> = &'e mut dyn for<'a> FnMut(DefRef<'a>);

/// Strip a UTF-8 BOM; returns the text without it. The caller decides what to do with invalid UTF-8.
pub fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

pub fn is_true(v: &str) -> bool {
    v.trim().eq_ignore_ascii_case("true")
}

fn unescape_cow(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().contains(&b'&') {
        return Cow::Borrowed(s);
    }
    match quick_xml::escape::unescape(s) {
        Ok(c) => c,
        Err(_) => Cow::Borrowed(s),
    }
}

// ---------------------------------------------------------------------------------------------
// roxmltree
// ---------------------------------------------------------------------------------------------

pub fn defs_roxmltree(text: &str, emit: Emit) -> Result<ScanInfo, String> {
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(text, opts).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let mut info = ScanInfo { root_not_defs: root.tag_name().name() != "Defs", ..Default::default() };
    for n in root.children().filter(|c| c.is_element()) {
        let def_name = n
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "defName")
            .and_then(|c| c.text())
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(Cow::Borrowed);
        emit(DefRef {
            tag: Cow::Borrowed(n.tag_name().name()),
            class: n.attribute("Class").map(Cow::Borrowed),
            def_name,
            name: n.attribute("Name").map(Cow::Borrowed),
            parent: n.attribute("ParentName").map(Cow::Borrowed),
            is_abstract: n.attribute("Abstract").map(is_true).unwrap_or(false),
            offset: n.range().start as u32,
        });
        info.defs += 1;
    }
    Ok(info)
}

// ---------------------------------------------------------------------------------------------
// quick-xml streaming
// ---------------------------------------------------------------------------------------------

struct Partial<'a> {
    tag: Cow<'a, str>,
    class: Option<Cow<'a, str>>,
    name: Option<Cow<'a, str>>,
    parent: Option<Cow<'a, str>>,
    is_abstract: bool,
    def_name: Option<String>,
    offset: u32,
}

fn partial_from<'a>(text: &'a str, e: &BytesStart<'_>, offset: u32) -> Partial<'a> {
    let name = e.name().into_inner();
    let start = offset as usize + 1;
    let tag = match text.get(start..start + name.len()) {
        Some(s) if s == name => Cow::Borrowed(s),
        _ => Cow::Owned(name.to_string()),
    };
    let mut p = Partial { tag, class: None, name: None, parent: None, is_abstract: false, def_name: None, offset };
    for a in e.attributes().flatten() {
        // quick-xml ties attribute values to the event, so they are copied once here
        match a.key.into_inner() {
            "Name" => p.name = a.unescape_value().ok().map(|c| Cow::Owned(c.into_owned())),
            "ParentName" => p.parent = a.unescape_value().ok().map(|c| Cow::Owned(c.into_owned())),
            "Class" => p.class = a.unescape_value().ok().map(|c| Cow::Owned(c.into_owned())),
            "Abstract" => p.is_abstract = a.unescape_value().map(|v| is_true(&v)).unwrap_or(false),
            _ => {}
        }
    }
    p
}

fn emit_partial(p: Partial<'_>, emit: &mut dyn for<'a> FnMut(DefRef<'a>)) {
    emit(DefRef {
        tag: p.tag,
        class: p.class,
        def_name: p.def_name.map(Cow::Owned),
        name: p.name,
        parent: p.parent,
        is_abstract: p.is_abstract,
        offset: p.offset,
    });
}

pub fn defs_quickxml(text: &str, emit: Emit) -> Result<ScanInfo, String> {
    let mut rd = Reader::from_str(text);
    {
        let c = rd.config_mut();
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
        c.allow_dangling_amp = true;
    }
    let mut info = ScanInfo::default();
    let mut depth = 0u32;
    let mut cur: Option<Partial> = None;
    let mut want_text = false;
    let mut tbuf = String::new();
    loop {
        let pos = rd.buffer_position() as u32;
        let ev = match rd.read_event() {
            Ok(e) => e,
            Err(e) => {
                // flush the def being read, keep everything emitted so far
                if let Some(p) = cur.take() {
                    emit_partial(p, emit);
                    info.defs += 1;
                }
                return Err(e.to_string());
            }
        };
        match ev {
            Event::Start(e) => {
                match depth {
                    0 => info.root_not_defs = e.name().into_inner() != "Defs",
                    1 => cur = Some(partial_from(text, &e, pos)),
                    2 => {
                        if let Some(p) = &cur {
                            if p.def_name.is_none() && e.name().into_inner() == "defName" {
                                want_text = true;
                                tbuf.clear();
                            }
                        }
                    }
                    _ => {}
                }
                depth += 1;
            }
            Event::Empty(e) => {
                if depth == 1 {
                    emit_partial(partial_from(text, &e, pos), emit);
                    info.defs += 1;
                }
            }
            Event::End(_) => {
                if depth == 3 && want_text {
                    want_text = false;
                    let t = tbuf.trim();
                    if let Some(p) = cur.as_mut() {
                        if !t.is_empty() {
                            p.def_name = Some(t.to_string());
                        }
                    }
                }
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some(p) = cur.take() {
                        emit_partial(p, emit);
                        info.defs += 1;
                    }
                }
            }
            Event::Text(t) if want_text => tbuf.push_str(&t.xml10_content()),
            Event::CData(c) if want_text => tbuf.push_str(&c.xml10_content()),
            Event::GeneralRef(r) if want_text => {
                if r.is_char_ref() {
                    if let Ok(Some(ch)) = r.resolve_char_ref() {
                        tbuf.push(ch);
                    }
                } else {
                    match &*r {
                        "amp" => tbuf.push('&'),
                        "lt" => tbuf.push('<'),
                        "gt" => tbuf.push('>'),
                        "quot" => tbuf.push('"'),
                        "apos" => tbuf.push('\''),
                        o => {
                            tbuf.push('&');
                            tbuf.push_str(o);
                            tbuf.push(';');
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(p) = cur.take() {
        emit_partial(p, emit);
        info.defs += 1;
    }
    Ok(info)
}

// ---------------------------------------------------------------------------------------------
// Structural scanner: no events, no allocation, memchr for the hot loops
// ---------------------------------------------------------------------------------------------

/// Finds the `>` that ends a start tag whose content begins at `from`, skipping quoted attribute values.
/// Returns the index of `>`.
#[inline]
fn find_tag_end(b: &[u8], from: usize) -> Option<usize> {
    let mut pos = from;
    loop {
        let p = memchr3(b'>', b'"', b'\'', &b[pos..])? + pos;
        match b[p] {
            b'>' => return Some(p),
            q => {
                let close = memchr(q, &b[p + 1..])? + p + 1;
                pos = close + 1;
            }
        }
    }
}

#[inline]
fn name_end(b: &[u8], from: usize, limit: usize) -> usize {
    let mut i = from;
    while i < limit {
        match b[i] {
            b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>' => break,
            _ => i += 1,
        }
    }
    i
}

/// Parse `key="value"` pairs of a def start tag. Only the four attributes the game reads for defs matter.
fn scan_attrs<'a>(text: &'a str, from: usize, to: usize, tag: &'a str, offset: u32) -> Partial<'a> {
    let b = text.as_bytes();
    let mut p = Partial { tag: Cow::Borrowed(tag), class: None, name: None, parent: None, is_abstract: false, def_name: None, offset };
    let mut i = from;
    while i < to {
        while i < to && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ks = i;
        while i < to && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = &text[ks..i];
        while i < to && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= to || b[i] != b'=' {
            if ks == i {
                break;
            }
            continue;
        }
        i += 1;
        while i < to && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= to || (b[i] != b'"' && b[i] != b'\'') {
            break;
        }
        let q = b[i];
        let vs = i + 1;
        let ve = match memchr(q, &b[vs..to.min(b.len())]) {
            Some(x) => vs + x,
            None => break,
        };
        let val = &text[vs..ve];
        match key {
            "Name" => p.name = Some(unescape_cow(val)),
            "ParentName" => p.parent = Some(unescape_cow(val)),
            "Class" => p.class = Some(unescape_cow(val)),
            "Abstract" => p.is_abstract = is_true(val),
            _ => {}
        }
        i = ve + 1;
    }
    p
}

pub fn defs_scan(text: &str, emit: Emit) -> Result<ScanInfo, String> {
    let b = text.as_bytes();
    let n = b.len();
    let mut info = ScanInfo::default();
    let mut i = 0usize;
    let mut depth = 0u32;
    let mut cur: Option<Partial> = None;
    macro_rules! bail {
        ($msg:expr) => {{
            if let Some(p) = cur.take() {
                emit_partial(p, emit);
                info.defs += 1;
            }
            return Err($msg.to_string());
        }};
    }
    while i < n {
        let lt = match memchr(b'<', &b[i..]) {
            Some(p) => i + p,
            None => break,
        };
        if lt + 1 >= n {
            break;
        }
        match b[lt + 1] {
            b'!' => {
                if b[lt + 1..].starts_with(b"!--") {
                    match memmem::find(&b[lt + 4..], b"-->") {
                        Some(p) => i = lt + 4 + p + 3,
                        None => bail!("unterminated comment"),
                    }
                } else if b[lt + 1..].starts_with(b"![CDATA[") {
                    match memmem::find(&b[lt + 9..], b"]]>") {
                        Some(p) => i = lt + 9 + p + 3,
                        None => bail!("unterminated CDATA"),
                    }
                } else {
                    // <!DOCTYPE ...> possibly with an internal subset [ ... ]
                    let mut j = lt + 2;
                    let mut bracket = 0i32;
                    loop {
                        if j >= n {
                            bail!("unterminated doctype");
                        }
                        match b[j] {
                            b'[' => bracket += 1,
                            b']' => bracket -= 1,
                            b'>' if bracket <= 0 => break,
                            _ => {}
                        }
                        j += 1;
                    }
                    i = j + 1;
                }
            }
            b'?' => match memmem::find(&b[lt + 2..], b"?>") {
                Some(p) => i = lt + 2 + p + 2,
                None => bail!("unterminated processing instruction"),
            },
            b'/' => {
                let gt = match memchr(b'>', &b[lt + 2..]) {
                    Some(p) => lt + 2 + p,
                    None => bail!("unterminated end tag"),
                };
                i = gt + 1;
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some(p) = cur.take() {
                        emit_partial(p, emit);
                        info.defs += 1;
                    }
                }
            }
            _ => {
                let gt = match find_tag_end(b, lt + 1) {
                    Some(g) => g,
                    None => bail!("unterminated start tag"),
                };
                let self_closing = b[gt - 1] == b'/';
                let name_s = lt + 1;
                let name_e = name_end(b, name_s, gt);
                let tag = &text[name_s..name_e];
                match depth {
                    0 => info.root_not_defs = tag != "Defs",
                    1 => {
                        let attr_to = if self_closing { gt - 1 } else { gt };
                        let p = scan_attrs(text, name_e, attr_to, tag, lt as u32);
                        if self_closing {
                            emit_partial(p, emit);
                            info.defs += 1;
                        } else {
                            cur = Some(p);
                        }
                    }
                    2 => {
                        if let Some(p) = cur.as_mut() {
                            if p.def_name.is_none() && tag == "defName" && !self_closing {
                                // text up to the next '<'
                                let ts = gt + 1;
                                let te = memchr(b'<', &b[ts..]).map(|x| ts + x).unwrap_or(n);
                                let raw = text[ts..te].trim();
                                if !raw.is_empty() {
                                    p.def_name = Some(unescape_cow(raw).into_owned());
                                }
                            }
                        }
                    }
                    _ => {}
                }
                i = gt + 1;
                if !self_closing {
                    depth += 1;
                } else if depth == 0 {
                    // an empty root element
                }
            }
        }
    }
    if let Some(p) = cur.take() {
        emit_partial(p, emit);
        info.defs += 1;
    }
    Ok(info)
}

/// Convenience: parse into owned records.
pub fn collect(text: &str, parser: Parser) -> (Vec<DefRec>, Result<ScanInfo, String>) {
    let mut v = Vec::new();
    let r = {
        let mut f = |d: DefRef| v.push(d.into_owned_rec());
        parser.run(text, &mut f)
    };
    (v, r)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parser {
    Roxmltree,
    QuickXml,
    Scan,
}

impl Parser {
    pub fn run(self, text: &str, emit: Emit) -> Result<ScanInfo, String> {
        match self {
            Parser::Roxmltree => defs_roxmltree(text, emit),
            Parser::QuickXml => defs_quickxml(text, emit),
            Parser::Scan => defs_scan(text, emit),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Parser::Roxmltree => "roxmltree",
            Parser::QuickXml => "quick-xml",
            Parser::Scan => "scan",
        }
    }
    pub fn from_name(s: &str) -> Option<Parser> {
        match s {
            "roxmltree" | "dom" => Some(Parser::Roxmltree),
            "quick-xml" | "quickxml" => Some(Parser::QuickXml),
            "scan" => Some(Parser::Scan),
            _ => None,
        }
    }
}
