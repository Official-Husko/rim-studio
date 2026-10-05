//! A KeyValues1 text reader and printer (the format of Steam's `.vdf` and `.acf` files).
//!
//! The reader is tolerant where Steam's own files need it and strict only where the text cannot be
//! interpreted:
//!
//! - a leading UTF-8 byte order mark is stripped, `//` starts a comment that ends with the line;
//! - tokens are quoted (`"text"`, with the escapes `\n \t \r \\ \"`) or bare (they end at white
//!   space, a quote or a brace); a backslash before any other character is kept literally, so a
//!   lone backslash in a Windows path survives; a quoted string whose closing quote is hidden by a
//!   trailing backslash is re-read without escapes;
//! - platform conditionals such as `[$WIN32]` after a key or a value are skipped;
//! - `#base` and `#include` at the top level are recorded in [`VdfDoc::macros`] and never followed;
//! - duplicate keys are kept, in file order;
//! - several top level pairs and an empty file are accepted.
//!
//! Errors carry the line and column of the problem. The reader never panics and is not recursive:
//! nesting deeper than [`MAX_DEPTH`] is an error.

use thiserror::Error;

/// The deepest object nesting the reader accepts.
pub const MAX_DEPTH: usize = 256;

/// A value: a string or an object holding more pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VdfValue {
    /// A string value. Numbers stay strings until a typed accessor asks.
    Str(String),
    /// A nested object.
    Obj(VdfObject),
}

impl VdfValue {
    /// The string, `None` for an object.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::Str(s) => Some(s),
            VdfValue::Obj(_) => None,
        }
    }

    /// The object, `None` for a string.
    pub fn as_obj(&self) -> Option<&VdfObject> {
        match self {
            VdfValue::Obj(o) => Some(o),
            VdfValue::Str(_) => None,
        }
    }
}

/// An ordered list of key and value pairs. Keys may repeat; order is the file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VdfObject {
    entries: Vec<(String, VdfValue)>,
}

impl VdfObject {
    /// An empty object.
    pub fn new() -> Self {
        VdfObject::default()
    }

    /// Appends a pair.
    pub fn push(&mut self, key: impl Into<String>, value: VdfValue) {
        self.entries.push((key.into(), value));
    }

    /// The pairs in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &VdfValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// The number of pairs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no pairs.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The first value with this key, ignoring ASCII case (KeyValues keys are case insensitive).
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// Every value with this key, ignoring ASCII case, in order.
    pub fn get_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a VdfValue> {
        self.entries
            .iter()
            .filter(move |(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// The first value with this key when it is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(VdfValue::as_str)
    }

    /// The first value with this key when it is an object.
    pub fn get_obj(&self, key: &str) -> Option<&VdfObject> {
        self.get(key).and_then(VdfValue::as_obj)
    }

    /// Follows a path of keys through nested objects (first match at each step).
    pub fn lookup(&self, path: &[&str]) -> Option<&VdfValue> {
        let (first, rest) = path.split_first()?;
        let mut current = self.get(first)?;
        for key in rest {
            current = current.as_obj()?.get(key)?;
        }
        Some(current)
    }
}

/// The kind of a top level macro line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroKind {
    /// `#base "file"`.
    Base,
    /// `#include "file"`.
    Include,
}

/// A recorded `#base` or `#include`. It is never followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VdfMacro {
    /// Which macro it is.
    pub kind: MacroKind,
    /// The file name the macro names.
    pub path: String,
}

/// A parsed document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VdfDoc {
    /// The top level pairs (usually exactly one, such as `libraryfolders` or `AppState`).
    pub root: VdfObject,
    /// The `#base` and `#include` lines in file order.
    pub macros: Vec<VdfMacro>,
}

impl VdfDoc {
    /// The value of the first top level pair with this key (ignoring ASCII case).
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.root.get(key)
    }

    /// The object of the first top level pair with this key.
    pub fn get_obj(&self, key: &str) -> Option<&VdfObject> {
        self.root.get_obj(key)
    }
}

/// What went wrong while reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VdfErrorKind {
    /// A quoted string has no closing quote.
    UnterminatedString,
    /// The text ended inside an object.
    UnclosedObject,
    /// A closing brace without an open object.
    StrayClose,
    /// An opening brace where a key was expected.
    UnexpectedOpen,
    /// A key with nothing after it.
    MissingValue,
    /// A `#base` or `#include` without a file name.
    MissingMacroPath,
    /// Nesting deeper than [`MAX_DEPTH`].
    TooDeep,
}

impl VdfErrorKind {
    /// A stable kebab-case name.
    pub fn name(self) -> &'static str {
        match self {
            VdfErrorKind::UnterminatedString => "unterminated-string",
            VdfErrorKind::UnclosedObject => "unclosed-object",
            VdfErrorKind::StrayClose => "stray-close",
            VdfErrorKind::UnexpectedOpen => "unexpected-open",
            VdfErrorKind::MissingValue => "missing-value",
            VdfErrorKind::MissingMacroPath => "missing-macro-path",
            VdfErrorKind::TooDeep => "too-deep",
        }
    }

    fn describe(self) -> &'static str {
        match self {
            VdfErrorKind::UnterminatedString => "quoted string is not closed",
            VdfErrorKind::UnclosedObject => "object is not closed",
            VdfErrorKind::StrayClose => "closing brace without an open object",
            VdfErrorKind::UnexpectedOpen => "opening brace where a key was expected",
            VdfErrorKind::MissingValue => "key without a value",
            VdfErrorKind::MissingMacroPath => "macro without a file name",
            VdfErrorKind::TooDeep => "objects are nested too deeply",
        }
    }
}

/// A syntax error with its position (1 based line and column, counted in characters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("{}, line {line}, column {column}", kind.describe())]
pub struct VdfError {
    /// What went wrong.
    pub kind: VdfErrorKind,
    /// The line of the problem.
    pub line: u32,
    /// The column of the problem.
    pub column: u32,
}

#[derive(Debug)]
enum Tok {
    Open,
    Close,
    Word { text: String, quoted: bool },
}

impl Tok {
    fn is_conditional(&self) -> bool {
        match self {
            Tok::Word {
                text,
                quoted: false,
            } => text.len() >= 2 && text.starts_with('[') && text.ends_with(']'),
            _ => false,
        }
    }
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
}

fn is_delimiter(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0 | b'"' | b'{' | b'}')
}

impl<'a> Lexer<'a> {
    fn byte(&self, at: usize) -> Option<u8> {
        self.text.as_bytes().get(at).copied()
    }

    fn position(&self, at: usize) -> (u32, u32) {
        let before = self.text.get(..at).unwrap_or("");
        let line = before.bytes().filter(|b| *b == b'\n').count();
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let column = before.get(line_start..).map_or(0, |s| s.chars().count());
        (
            u32::try_from(line + 1).unwrap_or(u32::MAX),
            u32::try_from(column + 1).unwrap_or(u32::MAX),
        )
    }

    fn error(&self, kind: VdfErrorKind, at: usize) -> VdfError {
        let (line, column) = self.position(at);
        VdfError { kind, line, column }
    }

    fn skip_blank(&mut self) {
        loop {
            match self.byte(self.pos) {
                Some(b' ' | b'\t' | b'\r' | b'\n' | 0) => self.pos += 1,
                Some(b'/') if self.byte(self.pos + 1) == Some(b'/') => {
                    while let Some(b) = self.byte(self.pos) {
                        if b == b'\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    /// The next token and the byte offset where it starts.
    fn next(&mut self) -> Result<Option<(Tok, usize)>, VdfError> {
        self.skip_blank();
        let start = self.pos;
        let Some(b) = self.byte(start) else {
            return Ok(None);
        };
        match b {
            b'{' => {
                self.pos += 1;
                Ok(Some((Tok::Open, start)))
            }
            b'}' => {
                self.pos += 1;
                Ok(Some((Tok::Close, start)))
            }
            b'"' => {
                let word = self.quoted(start)?;
                Ok(Some((word, start)))
            }
            _ => {
                let mut end = start;
                while let Some(c) = self.byte(end) {
                    if is_delimiter(c) {
                        break;
                    }
                    end += 1;
                }
                self.pos = end;
                let text = self.text.get(start..end).unwrap_or("").to_owned();
                Ok(Some((
                    Tok::Word {
                        text,
                        quoted: false,
                    },
                    start,
                )))
            }
        }
    }

    fn quoted(&mut self, open: usize) -> Result<Tok, VdfError> {
        let start = open + 1;
        // First pass: backslash escapes the next byte.
        let mut i = start;
        let mut closing = None;
        while let Some(b) = self.byte(i) {
            match b {
                b'"' => {
                    closing = Some(i);
                    break;
                }
                b'\\' => i += 2,
                _ => i += 1,
            }
        }
        let (end, escapes) = match closing {
            Some(end) => (end, true),
            None => {
                // The escape reading never found a closing quote; read the string literally.
                let rest = self.text.get(start..).unwrap_or("");
                match rest.find('"') {
                    Some(offset) => (start + offset, false),
                    None => return Err(self.error(VdfErrorKind::UnterminatedString, open)),
                }
            }
        };
        self.pos = end + 1;
        let raw = self.text.get(start..end).unwrap_or("");
        let text = if escapes && raw.contains('\\') {
            unescape(raw)
        } else {
            raw.to_owned()
        };
        Ok(Tok::Word { text, quoted: true })
    }

    /// Skips one conditional token when it comes next.
    fn skip_conditional(&mut self) {
        let saved = self.pos;
        match self.next() {
            Ok(Some((tok, _))) if tok.is_conditional() => {}
            _ => self.pos = saved,
        }
    }
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Parses KeyValues1 text.
///
/// # Errors
/// A [`VdfError`] with the line and column of the first problem.
pub fn parse(input: &str) -> Result<VdfDoc, VdfError> {
    let text = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut lexer = Lexer { text, pos: 0 };
    let mut doc = VdfDoc::default();
    let mut current = VdfObject::new();
    // Each frame is the pending key, the enclosing object and the offset of the opening brace.
    let mut stack: Vec<(String, VdfObject, usize)> = Vec::new();

    loop {
        let Some((tok, at)) = lexer.next()? else {
            return match stack.last() {
                None => {
                    doc.root = current;
                    Ok(doc)
                }
                Some((_, _, open)) => Err(lexer.error(VdfErrorKind::UnclosedObject, *open)),
            };
        };
        match tok {
            Tok::Close => {
                let Some((key, mut parent, _)) = stack.pop() else {
                    return Err(lexer.error(VdfErrorKind::StrayClose, at));
                };
                parent.push(key, VdfValue::Obj(std::mem::take(&mut current)));
                current = parent;
                lexer.skip_conditional();
            }
            Tok::Open => return Err(lexer.error(VdfErrorKind::UnexpectedOpen, at)),
            Tok::Word { text, quoted } => {
                if stack.is_empty() && !quoted {
                    let kind = if text.eq_ignore_ascii_case("#base") {
                        Some(MacroKind::Base)
                    } else if text.eq_ignore_ascii_case("#include") {
                        Some(MacroKind::Include)
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        match lexer.next()? {
                            Some((Tok::Word { text: path, .. }, _)) => {
                                doc.macros.push(VdfMacro { kind, path });
                                continue;
                            }
                            _ => return Err(lexer.error(VdfErrorKind::MissingMacroPath, at)),
                        }
                    }
                }
                let mut value = lexer.next()?;
                if matches!(&value, Some((t, _)) if t.is_conditional()) {
                    value = lexer.next()?;
                }
                match value {
                    None => return Err(lexer.error(VdfErrorKind::MissingValue, at)),
                    Some((Tok::Close, p)) => return Err(lexer.error(VdfErrorKind::MissingValue, p)),
                    Some((Tok::Open, p)) => {
                        if stack.len() >= MAX_DEPTH {
                            return Err(lexer.error(VdfErrorKind::TooDeep, p));
                        }
                        stack.push((text, std::mem::take(&mut current), p));
                    }
                    Some((Tok::Word { text: v, .. }, _)) => {
                        current.push(text, VdfValue::Str(v));
                        lexer.skip_conditional();
                    }
                }
            }
        }
    }
}

fn escape_into(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
}

fn render_object(out: &mut String, object: &VdfObject, depth: usize) {
    for (key, value) in object.iter() {
        for _ in 0..depth {
            out.push('\t');
        }
        escape_into(out, key);
        match value {
            VdfValue::Str(s) => {
                out.push_str("\t\t");
                escape_into(out, s);
                out.push('\n');
            }
            VdfValue::Obj(inner) => {
                out.push('\n');
                for _ in 0..depth {
                    out.push('\t');
                }
                out.push_str("{\n");
                render_object(out, inner, depth + 1);
                for _ in 0..depth {
                    out.push('\t');
                }
                out.push_str("}\n");
            }
        }
    }
}

/// Prints a document as KeyValues1 text: macros first, then the pairs, tab indented, every token
/// quoted. `parse(&render(&doc))` gives back an equal document.
pub fn render(doc: &VdfDoc) -> String {
    let mut out = String::new();
    for m in &doc.macros {
        out.push_str(match m.kind {
            MacroKind::Base => "#base ",
            MacroKind::Include => "#include ",
        });
        escape_into(&mut out, &m.path);
        out.push('\n');
    }
    render_object(&mut out, &doc.root, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn s(doc: &VdfDoc, path: &[&str]) -> Option<String> {
        doc.root.lookup(path)?.as_str().map(str::to_owned)
    }

    #[test]
    fn reads_a_nested_document() {
        let doc = parse("\"A\"\n{\n\t\"k\"\t\t\"v\"\n\t\"o\" { \"x\" \"1\" }\n}\n").unwrap();
        assert_eq!(s(&doc, &["A", "k"]).as_deref(), Some("v"));
        assert_eq!(s(&doc, &["a", "O", "x"]).as_deref(), Some("1"));
    }

    #[rstest]
    #[case::bom("\u{feff}\"a\" \"b\"")]
    #[case::crlf("\"a\"\r\n\"b\"\r\n")]
    #[case::bare("a b")]
    #[case::comment("// head\n\"a\" \"b\" // tail\n")]
    #[case::trailing_nul("\"a\" \"b\"\0")]
    #[case::conditional_after_value("\"a\" \"b\" [$WIN32]")]
    #[case::conditional_after_key("\"a\" [$WIN32] \"b\"")]
    fn accepts_lenient_input_with_one_pair(#[case] text: &str) {
        let doc = parse(text).unwrap();
        assert_eq!(doc.root.len(), 1, "{text:?}");
        assert_eq!(doc.root.get_str("a"), Some("b"));
    }

    #[test]
    fn conditional_before_a_brace_is_skipped() {
        let doc = parse("\"a\" [$WIN32] { \"b\" \"c\" } [$OSX]").unwrap();
        assert_eq!(s(&doc, &["a", "b"]).as_deref(), Some("c"));
    }

    #[test]
    fn duplicate_keys_are_kept_in_order() {
        let doc = parse("\"k\" \"1\" \"k\" \"2\" \"k\" \"3\"").unwrap();
        let all: Vec<_> = doc.root.get_all("k").filter_map(VdfValue::as_str).collect();
        assert_eq!(all, ["1", "2", "3"]);
    }

    #[test]
    fn escapes_and_lone_backslashes() {
        let doc = parse(r#""p" "D:\\Lib" "q" "C:\Steam\apps" "r" "a\tb\"c""#).unwrap();
        assert_eq!(doc.root.get_str("p"), Some("D:\\Lib"));
        assert_eq!(doc.root.get_str("q"), Some("C:\\Steam\\apps"));
        assert_eq!(doc.root.get_str("r"), Some("a\tb\"c"));
    }

    #[test]
    fn a_trailing_backslash_before_the_closing_quote_is_tolerated() {
        let doc = parse("\"p\" \"C:\\Steam\\\"").unwrap();
        assert_eq!(doc.root.get_str("p"), Some("C:\\Steam\\"));
    }

    #[test]
    fn macros_are_recorded_and_not_followed() {
        let doc = parse("#base \"a.vdf\"\n#include other.vdf\n\"k\" \"v\"").unwrap();
        assert_eq!(
            doc.macros,
            vec![
                VdfMacro {
                    kind: MacroKind::Base,
                    path: "a.vdf".into()
                },
                VdfMacro {
                    kind: MacroKind::Include,
                    path: "other.vdf".into()
                }
            ]
        );
        assert_eq!(doc.root.len(), 1);
    }

    #[rstest]
    #[case::empty("")]
    #[case::blank("  \n\t ")]
    #[case::only_comment("// nothing")]
    fn empty_documents_are_accepted(#[case] text: &str) {
        let doc = parse(text).unwrap();
        assert!(doc.root.is_empty());
    }

    #[test]
    fn two_top_level_pairs_are_accepted() {
        let doc = parse("\"a\" \"1\" \"b\" { }").unwrap();
        assert_eq!(doc.root.len(), 2);
    }

    #[rstest]
    #[case::unterminated("\"a\" \"b", VdfErrorKind::UnterminatedString, 1)]
    #[case::unclosed("\"a\"\n{\n\"b\" \"c\"\n", VdfErrorKind::UnclosedObject, 2)]
    #[case::stray("\"a\" \"b\"\n}", VdfErrorKind::StrayClose, 2)]
    #[case::open_as_key("{ \"a\" \"b\" }", VdfErrorKind::UnexpectedOpen, 1)]
    #[case::missing_value("\"a\"", VdfErrorKind::MissingValue, 1)]
    #[case::close_as_value("\"a\" {\n\"b\" }\n}", VdfErrorKind::MissingValue, 2)]
    #[case::macro_without_path("#base", VdfErrorKind::MissingMacroPath, 1)]
    fn errors_carry_kind_and_line(
        #[case] text: &str,
        #[case] kind: VdfErrorKind,
        #[case] line: u32,
    ) {
        let err = parse(text).unwrap_err();
        assert_eq!(err.kind, kind, "{text:?}");
        assert_eq!(err.line, line, "{text:?}");
    }

    #[test]
    fn block_comments_are_not_comments() {
        // `/* x */` reads as three bare tokens: a key, a value and a key without a value.
        assert!(parse("/* x */").is_err());
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        let mut text = String::new();
        for _ in 0..(MAX_DEPTH + 10) {
            text.push_str("\"a\" {");
        }
        assert_eq!(parse(&text).unwrap_err().kind, VdfErrorKind::TooDeep);
        let ok_depth = "\"a\" {".repeat(MAX_DEPTH) + &"}".repeat(MAX_DEPTH);
        assert!(parse(&ok_depth).is_ok());
    }

    #[test]
    fn render_round_trips_awkward_text() {
        let mut inner = VdfObject::new();
        inner.push("k \"q\"", VdfValue::Str("a\\b\n\tc\r".into()));
        inner.push("", VdfValue::Str(String::new()));
        let mut root = VdfObject::new();
        root.push("o", VdfValue::Obj(inner));
        root.push("o", VdfValue::Obj(VdfObject::new()));
        let doc = VdfDoc {
            root,
            macros: vec![VdfMacro {
                kind: MacroKind::Include,
                path: "x y".into(),
            }],
        };
        assert_eq!(parse(&render(&doc)).unwrap(), doc);
    }

    #[test]
    fn error_text_names_line_and_column() {
        let err = parse("\"a\"\n  \"b").unwrap_err();
        assert_eq!(
            err.to_string(),
            "quoted string is not closed, line 2, column 3"
        );
    }

    #[test]
    fn non_ascii_text_is_handled_on_char_boundaries() {
        let doc = parse("\"k\u{e9}y\" \"v\u{4e2d}l\" bar\u{e9} baz").unwrap();
        assert_eq!(doc.root.get_str("k\u{e9}y"), Some("v\u{4e2d}l"));
        assert_eq!(doc.root.get_str("bar\u{e9}"), Some("baz"));
    }
}
