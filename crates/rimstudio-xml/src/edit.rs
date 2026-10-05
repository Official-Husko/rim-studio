//! Byte span editing of existing XML files.
//!
//! [`SpanEditor`] indexes the elements of a document by byte offset and applies edits as splices
//! of the original text, so every byte outside the changed span (comments, odd white space,
//! attribute quoting, line endings, a byte order mark) stays exactly as it was. It is the tool for
//! files that people formatted by hand: `About.xml`, `LoadFolders.xml`, a mod's Defs.
//!
//! Elements are addressed with paths such as `/loadFolders/v1.6/li[2]`: the first segment is the
//! root element, each further segment is a child element name with an optional one based position
//! among the children of the same name (`li[2]`, default 1). `*` matches any name.

use std::ops::Range;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rimstudio_core::tree::Node;

use crate::error::{XmlError, XmlResult};
use crate::render::{check_names, render_at};
use crate::text::{
    escape_attr, escape_text, is_valid_name, is_ws_only, line_col, normalize_eols, unescape_lenient,
};

/// Where an attribute sits inside its start tag.
#[derive(Debug, Clone)]
struct AttrSpan {
    name: String,
    /// Offset of the first character of the name.
    start: usize,
    /// Offset just after the closing quote.
    end: usize,
    /// Offset of the first character of the value (just after the opening quote).
    value_start: usize,
    /// Offset of the closing quote.
    value_end: usize,
    quote: u8,
}

#[derive(Debug, Clone)]
struct Elem {
    tag: String,
    parent: Option<usize>,
    children: Vec<usize>,
    /// Offset of `<`.
    start: usize,
    /// Offset just after the `>` of the start tag.
    open_end: usize,
    /// Offset of `</` (equal to `end` for an empty element tag).
    close_start: usize,
    /// Offset just after the element.
    end: usize,
    /// Written as `<tag/>`.
    empty: bool,
    /// Holds text other than white space, CDATA or references.
    has_text: bool,
    attrs: Vec<AttrSpan>,
}

/// One child element of a path, as listed by [`SpanEditor::children`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRef {
    /// Element name.
    pub tag: String,
    /// Zero based position among all element children.
    pub index: usize,
    /// One based position among the children with the same name.
    pub nth: usize,
    /// A path that addresses exactly this child.
    pub path: String,
}

/// What [`SpanEditor::element`] reports about an element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementInfo {
    /// Element name.
    pub tag: String,
    /// Attributes in source order, unescaped.
    pub attrs: Vec<(String, String)>,
    /// All text below the element (character data and CDATA), unescaped.
    pub text: String,
    /// Number of child elements.
    pub child_count: usize,
    /// Byte range of the whole element in the text.
    pub span: Range<usize>,
}

/// An XML document held as text, with edit operations that splice bytes.
#[derive(Debug, Clone)]
pub struct SpanEditor {
    text: String,
    elems: Vec<Elem>,
    ignore_case: bool,
}

impl SpanEditor {
    /// Indexes the document. A leading byte order mark is kept and ignored by the index.
    ///
    /// The document must be well formed as far as element nesting goes; entity references are not
    /// resolved (so `&nbsp;` and a lone `&` are tolerated) and a document type declaration is
    /// skipped.
    ///
    /// # Errors
    /// [`XmlError::Parse`] when tags are mismatched or the document has no or several roots.
    pub fn open(text: impl Into<String>) -> XmlResult<SpanEditor> {
        let text = text.into();
        let elems = index(&text)?;
        Ok(SpanEditor {
            text,
            elems,
            ignore_case: false,
        })
    }

    /// Makes element name matching in paths ASCII case insensitive (off by default).
    #[must_use]
    pub fn case_insensitive(mut self, on: bool) -> Self {
        self.ignore_case = on;
        self
    }

    /// The current text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Consumes the editor and returns the text.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }

    /// The name of the root element.
    #[must_use]
    pub fn root_tag(&self) -> &str {
        self.elems.first().map_or("", |e| e.tag.as_str())
    }

    /// The line ending of the document: `\r\n` when the first line break is one, else `\n`.
    #[must_use]
    pub fn eol(&self) -> &'static str {
        match self.text.find('\n') {
            Some(i) if i > 0 && self.text.as_bytes().get(i - 1) == Some(&b'\r') => "\r\n",
            _ => "\n",
        }
    }

    /// The indentation step of the document, measured on the first nested element that sits on its
    /// own line (two spaces when there is none).
    #[must_use]
    pub fn indent_unit(&self) -> String {
        for e in self.elems.iter().skip(1) {
            let Some(parent) = e.parent else { continue };
            let (Some(child_ind), Some(parent_ind)) = (
                self.line_indent(e.start),
                self.line_indent(self.elems[parent].start),
            ) else {
                continue;
            };
            if child_ind.len() > parent_ind.len() && child_ind.starts_with(&parent_ind) {
                return child_ind[parent_ind.len()..].to_owned();
            }
        }
        "  ".to_owned()
    }

    // ------------------------------------------------------------------ reading

    /// True when the path addresses an element.
    #[must_use]
    pub fn exists(&self, path: &str) -> bool {
        self.resolve(path).is_ok()
    }

    /// Lists the child elements of `path`, in document order.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] or [`XmlError::EditPathInvalid`].
    pub fn children(&self, path: &str) -> XmlResult<Vec<ChildRef>> {
        let idx = self.resolve(path)?;
        let parent_path = self.canonical_path(idx);
        let mut counts: Vec<(&str, usize)> = Vec::new();
        let mut out = Vec::new();
        for (index, &c) in self.elems[idx].children.iter().enumerate() {
            let tag = self.elems[c].tag.as_str();
            let nth = match counts.iter_mut().find(|(t, _)| *t == tag) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    counts.push((tag, 1));
                    1
                }
            };
            out.push(ChildRef {
                tag: tag.to_owned(),
                index,
                nth,
                path: format!("{parent_path}/{tag}[{nth}]"),
            });
        }
        Ok(out)
    }

    /// Describes the element at `path`.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] or [`XmlError::EditPathInvalid`].
    pub fn element(&self, path: &str) -> XmlResult<ElementInfo> {
        let idx = self.resolve(path)?;
        let e = &self.elems[idx];
        Ok(ElementInfo {
            tag: e.tag.clone(),
            attrs: e
                .attrs
                .iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        unescape_lenient(&self.text[a.value_start..a.value_end]).into_owned(),
                    )
                })
                .collect(),
            text: self.inner_text(idx),
            child_count: e.children.len(),
            span: e.start..e.end,
        })
    }

    /// The unescaped text below the element at `path`.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] or [`XmlError::EditPathInvalid`].
    pub fn element_text(&self, path: &str) -> XmlResult<String> {
        Ok(self.inner_text(self.resolve(path)?))
    }

    /// The unescaped value of an attribute, if the element has it.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] or [`XmlError::EditPathInvalid`].
    pub fn attr(&self, path: &str, name: &str) -> XmlResult<Option<String>> {
        let idx = self.resolve(path)?;
        Ok(self.elems[idx]
            .attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| unescape_lenient(&self.text[a.value_start..a.value_end]).into_owned()))
    }

    // ------------------------------------------------------------------ editing

    /// Replaces everything inside the element with `new` (written escaped). The element must not
    /// have child elements. An element written `<a/>` becomes `<a>new</a>`.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::EditUnsupported`] when the element has child elements.
    pub fn replace_text(&mut self, path: &str, new: &str) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        let e = &self.elems[idx];
        if !e.children.is_empty() {
            return Err(unsupported(path, "the element has child elements"));
        }
        let body = self.to_doc_eol(&escape_text(&normalize_eols(new)));
        if e.empty {
            if body.is_empty() {
                return Ok(&self.text);
            }
            let name = e.tag.clone();
            let range = self.empty_tag_tail(idx);
            return self.splice(range, &format!(">{body}</{name}>"));
        }
        let range = e.open_end..e.close_start;
        self.splice(range, &body)
    }

    /// Replaces the children of the element with `nodes`, laid out in the document's own
    /// indentation style. Comments and text that were inside the element are removed.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::InvalidValue`] for names that are not XML names.
    pub fn replace_children(&mut self, path: &str, nodes: &[Node]) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        for n in nodes {
            check_names(n)?;
        }
        let e = &self.elems[idx];
        let (start_tag_tail, inner) = if e.empty {
            (Some(self.empty_tag_tail(idx)), None)
        } else {
            (None, Some(e.open_end..e.close_start))
        };
        let name = e.tag.clone();
        let own_indent = self.line_indent(e.start);
        let content = self.children_block(nodes, own_indent.as_deref());
        match (start_tag_tail, inner) {
            (Some(range), _) => {
                if nodes.is_empty() {
                    return Ok(&self.text);
                }
                self.splice(range, &format!(">{content}</{name}>"))
            }
            (None, Some(range)) => self.splice(range, &content),
            (None, None) => Ok(&self.text),
        }
    }

    /// Appends `node` as the last child of the element at `parent_path`.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::InvalidValue`] for names that are not XML names.
    pub fn insert_child(&mut self, parent_path: &str, node: &Node) -> XmlResult<&str> {
        let idx = self.resolve(parent_path)?;
        check_names(node)?;
        let e = &self.elems[idx];
        if let Some(&last) = e.children.last() {
            return self.insert_relative(last, node, true);
        }
        let name = e.tag.clone();
        let own_indent = self.line_indent(e.start);
        if e.has_text {
            let at = e.close_start;
            let frag = self.to_doc_eol(&render_at(node, "", "", true));
            return self.splice(at..at, &frag);
        }
        let content = self.children_block(std::slice::from_ref(node), own_indent.as_deref());
        if e.empty {
            let range = self.empty_tag_tail(idx);
            self.splice(range, &format!(">{content}</{name}>"))
        } else {
            let range = e.open_end..e.close_start;
            self.splice(range, &content)
        }
    }

    /// Inserts `node` so that it becomes the child number `index` (zero based) of the element at
    /// `parent_path`; an index past the end appends.
    ///
    /// # Errors
    /// As [`SpanEditor::insert_child`].
    pub fn insert_child_at(
        &mut self,
        parent_path: &str,
        index: usize,
        node: &Node,
    ) -> XmlResult<&str> {
        let idx = self.resolve(parent_path)?;
        match self.elems[idx].children.get(index) {
            Some(&target) => {
                check_names(node)?;
                self.insert_relative(target, node, false)
            }
            None => self.insert_child(parent_path, node),
        }
    }

    /// Inserts `node` as the sibling just before the element at `path`.
    ///
    /// # Errors
    /// Path errors, [`XmlError::EditUnsupported`] for the root, or [`XmlError::InvalidValue`].
    pub fn insert_before(&mut self, path: &str, node: &Node) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        self.require_non_root(idx, path)?;
        check_names(node)?;
        self.insert_relative(idx, node, false)
    }

    /// Inserts `node` as the sibling just after the element at `path`.
    ///
    /// # Errors
    /// As [`SpanEditor::insert_before`].
    pub fn insert_after(&mut self, path: &str, node: &Node) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        self.require_non_root(idx, path)?;
        check_names(node)?;
        self.insert_relative(idx, node, true)
    }

    /// Replaces the whole element at `path` with `node`.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::InvalidValue`] for names that are not XML names.
    pub fn replace_element(&mut self, path: &str, node: &Node) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        check_names(node)?;
        let e = &self.elems[idx];
        let range = e.start..e.end;
        let (base, unit) = match self.line_indent(e.start) {
            Some(ind) => (ind, self.indent_unit()),
            None => (String::new(), String::new()),
        };
        let frag = self.to_doc_eol(&render_at(node, &base, &unit, true));
        self.splice(range, &frag)
    }

    /// Removes the element at `path`. When it sits alone on its lines the whole lines go.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::EditUnsupported`] for the root.
    pub fn remove(&mut self, path: &str) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        self.require_non_root(idx, path)?;
        let (s, e) = (self.elems[idx].start, self.elems[idx].end);
        let base = self.bom_len();
        let line_start = self.text[..s].rfind('\n').map_or(base, |i| i + 1).max(base);
        let before_ok = is_blank(&self.text[line_start..s]);
        let rest = &self.text[e..];
        let trailing_ws = rest
            .bytes()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        let after = &rest[trailing_ws..];
        let range = if before_ok && (after.starts_with('\n') || after.starts_with("\r\n")) {
            let nl = if after.starts_with("\r\n") { 2 } else { 1 };
            line_start..e + trailing_ws + nl
        } else if before_ok && after.is_empty() {
            line_start..e + trailing_ws
        } else {
            s..e
        };
        self.splice(range, "")
    }

    /// Sets an attribute: the value of an existing one is replaced inside its own quotes, a new one
    /// is appended after the last attribute.
    ///
    /// # Errors
    /// Path errors, or [`XmlError::InvalidValue`] when `name` is not an XML name.
    pub fn set_attr(&mut self, path: &str, name: &str, value: &str) -> XmlResult<&str> {
        if !is_valid_name(name) {
            return Err(XmlError::InvalidValue {
                what: "attribute name".to_owned(),
                reason: format!("{name:?} is not an XML name"),
            });
        }
        let idx = self.resolve(path)?;
        let e = &self.elems[idx];
        if let Some(a) = e.attrs.iter().find(|a| a.name == name) {
            let escaped = escape_attr_quoted(value, a.quote);
            let range = a.value_start..a.value_end;
            return self.splice(range, &escaped);
        }
        let at = match e.attrs.last() {
            Some(a) => a.end,
            None => e.start + 1 + e.tag.len(),
        };
        let piece = format!(" {name}=\"{}\"", escape_attr(value));
        self.splice(at..at, &piece)
    }

    /// Removes an attribute (and the white space before it). Does nothing when it is absent.
    ///
    /// # Errors
    /// Path errors.
    pub fn remove_attr(&mut self, path: &str, name: &str) -> XmlResult<&str> {
        let idx = self.resolve(path)?;
        let e = &self.elems[idx];
        let Some(a) = e.attrs.iter().find(|a| a.name == name) else {
            return Ok(&self.text);
        };
        let ws = self.text[..a.start]
            .bytes()
            .rev()
            .take_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .count();
        let range = a.start - ws..a.end;
        self.splice(range, "")
    }

    // ------------------------------------------------------------------ internals

    fn bom_len(&self) -> usize {
        if self.text.starts_with('\u{FEFF}') {
            3
        } else {
            0
        }
    }

    fn require_non_root(&self, idx: usize, path: &str) -> XmlResult<()> {
        if self.elems[idx].parent.is_none() {
            Err(unsupported(
                path,
                "the root element cannot be addressed this way",
            ))
        } else {
            Ok(())
        }
    }

    fn to_doc_eol(&self, s: &str) -> String {
        if self.eol() == "\r\n" {
            s.replace('\n', "\r\n")
        } else {
            s.to_owned()
        }
    }

    /// The white space at the start of the line holding `pos`, when nothing else precedes `pos`.
    fn line_indent(&self, pos: usize) -> Option<String> {
        let base = self.bom_len();
        let line_start = self.text[..pos]
            .rfind('\n')
            .map_or(base, |i| i + 1)
            .max(base);
        let lead = &self.text[line_start..pos];
        if lead.bytes().all(|b| matches!(b, b' ' | b'\t')) {
            Some(lead.to_owned())
        } else {
            None
        }
    }

    /// The range from the whitespace before `/>` through the end of an empty element tag.
    fn empty_tag_tail(&self, idx: usize) -> Range<usize> {
        let e = &self.elems[idx];
        let slash = e.end.saturating_sub(2);
        let ws = self.text[..slash]
            .bytes()
            .rev()
            .take_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .count();
        // Never eat into the tag name or an attribute value.
        let floor = e.attrs.last().map_or(e.start + 1 + e.tag.len(), |a| a.end);
        (slash - ws).max(floor)..e.end
    }

    /// Text for a list of children placed inside a parent whose own line indent is `own`.
    fn children_block(&self, nodes: &[Node], own: Option<&str>) -> String {
        if nodes.is_empty() {
            return String::new();
        }
        let Some(own) = own else {
            let mut out = String::new();
            for n in nodes {
                out.push_str(&self.to_doc_eol(&render_at(n, "", "", true)));
            }
            return out;
        };
        let unit = self.indent_unit();
        let child_ind = format!("{own}{unit}");
        let eol = self.eol();
        let mut out = String::new();
        for n in nodes {
            out.push_str(eol);
            out.push_str(&child_ind);
            out.push_str(&self.to_doc_eol(&render_at(n, &child_ind, &unit, true)));
        }
        out.push_str(eol);
        out.push_str(own);
        out
    }

    fn insert_relative(&mut self, target: usize, node: &Node, after: bool) -> XmlResult<&str> {
        let e = &self.elems[target];
        let (at, indent) = (
            if after { e.end } else { e.start },
            self.line_indent(e.start),
        );
        let (frag, lead, trail) = match &indent {
            Some(ind) => {
                let unit = self.indent_unit();
                let rendered = self.to_doc_eol(&render_at(node, ind, &unit, true));
                let eol = self.eol();
                if after {
                    (rendered, format!("{eol}{ind}"), String::new())
                } else {
                    (rendered, String::new(), format!("{eol}{ind}"))
                }
            }
            None => (
                self.to_doc_eol(&render_at(node, "", "", true)),
                String::new(),
                String::new(),
            ),
        };
        self.splice(at..at, &format!("{lead}{frag}{trail}"))
    }

    fn splice(&mut self, range: Range<usize>, replacement: &str) -> XmlResult<&str> {
        let mut new = String::with_capacity(self.text.len() + replacement.len());
        new.push_str(&self.text[..range.start]);
        new.push_str(replacement);
        new.push_str(&self.text[range.end..]);
        let elems = index(&new)?;
        self.text = new;
        self.elems = elems;
        Ok(&self.text)
    }

    fn canonical_path(&self, idx: usize) -> String {
        let mut parts = Vec::new();
        let mut cur = idx;
        loop {
            let e = &self.elems[cur];
            match e.parent {
                Some(p) => {
                    let nth = self.elems[p]
                        .children
                        .iter()
                        .take_while(|&&c| c != cur)
                        .filter(|&&c| self.elems[c].tag == e.tag)
                        .count()
                        + 1;
                    parts.push(format!("{}[{nth}]", e.tag));
                    cur = p;
                }
                None => {
                    parts.push(e.tag.clone());
                    break;
                }
            }
        }
        parts.reverse();
        format!("/{}", parts.join("/"))
    }

    fn tag_matches(&self, pattern: &str, tag: &str) -> bool {
        pattern == "*"
            || if self.ignore_case {
                pattern.eq_ignore_ascii_case(tag)
            } else {
                pattern == tag
            }
    }

    fn resolve(&self, path: &str) -> XmlResult<usize> {
        let invalid = |reason: &str| XmlError::EditPathInvalid {
            path: path.to_owned(),
            reason: reason.to_owned(),
        };
        let trimmed = path.trim();
        let body = trimmed.strip_prefix('/').unwrap_or(trimmed);
        if body.is_empty() {
            return Err(invalid("the path is empty"));
        }
        let mut segments = body.split('/');
        let (root_name, root_n) = parse_segment(segments.next().unwrap_or("")).map_err(&invalid)?;
        let Some(root) = self.elems.first() else {
            return Err(XmlError::EditPathMissing {
                path: path.to_owned(),
            });
        };
        if root_n != 1 || !self.tag_matches(root_name, &root.tag) {
            return Err(XmlError::EditPathMissing {
                path: path.to_owned(),
            });
        }
        let mut cur = 0usize;
        for seg in segments {
            let (name, n) = parse_segment(seg).map_err(&invalid)?;
            let mut seen = 0usize;
            let mut found = None;
            for &c in &self.elems[cur].children {
                if self.tag_matches(name, &self.elems[c].tag) {
                    seen += 1;
                    if seen == n {
                        found = Some(c);
                        break;
                    }
                }
            }
            match found {
                Some(c) => cur = c,
                None => {
                    return Err(XmlError::EditPathMissing {
                        path: path.to_owned(),
                    });
                }
            }
        }
        Ok(cur)
    }

    /// All text below an element, unescaped, with CDATA content taken literally.
    fn inner_text(&self, idx: usize) -> String {
        let e = &self.elems[idx];
        let mut raw = &self.text[e.open_end..e.close_start.max(e.open_end)];
        let mut out = String::new();
        while let Some(lt) = raw.find('<') {
            push_text(&mut out, &raw[..lt]);
            let tail = &raw[lt..];
            if let Some(rest) = tail.strip_prefix("<![CDATA[") {
                match rest.find("]]>") {
                    Some(end) => {
                        out.push_str(&rest[..end]);
                        raw = &rest[end + 3..];
                    }
                    None => {
                        out.push_str(rest);
                        raw = "";
                    }
                }
            } else if let Some(rest) = tail.strip_prefix("<!--") {
                raw = rest.find("-->").map_or("", |end| &rest[end + 3..]);
            } else if let Some(rest) = tail.strip_prefix("<?") {
                raw = rest.find("?>").map_or("", |end| &rest[end + 2..]);
            } else {
                raw = find_tag_end(tail).map_or("", |end| &tail[end + 1..]);
            }
        }
        push_text(&mut out, raw);
        out
    }
}

fn push_text(out: &mut String, raw: &str) {
    out.push_str(&normalize_eols(&unescape_lenient(raw)));
}

fn unsupported(path: &str, reason: &str) -> XmlError {
    XmlError::EditUnsupported {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}

fn is_blank(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t'))
}

fn escape_attr_quoted(value: &str, quote: u8) -> String {
    let escaped = escape_attr(value).into_owned();
    if quote == b'\'' {
        // The double quote escape is harmless but a single quote must be escaped.
        escaped.replace('\'', "&apos;")
    } else {
        escaped
    }
}

/// Splits `name` or `name[n]` into the name and the one based position.
fn parse_segment(seg: &str) -> Result<(&str, usize), &'static str> {
    if seg.is_empty() {
        return Err("a path segment is empty");
    }
    match seg.split_once('[') {
        None => Ok((seg, 1)),
        Some((name, rest)) => {
            let Some(num) = rest.strip_suffix(']') else {
                return Err("a position is missing its closing bracket");
            };
            let n: usize = num
                .trim()
                .parse()
                .map_err(|_| "a position is not a number")?;
            if n == 0 {
                return Err("positions start at 1");
            }
            if name.is_empty() {
                return Err("a segment has no name");
            }
            Ok((name, n))
        }
    }
}

/// The offset of the `>` that ends the tag at the start of `s`, skipping quoted values.
fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, b) in s.bytes().enumerate() {
        match (quote, b) {
            (Some(q), b) if b == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(b),
            (None, b'>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// Scans the attributes of the start tag `text[start..open_end]`.
fn scan_attrs(text: &str, start: usize, open_end: usize, empty: bool) -> Vec<AttrSpan> {
    let bytes = text.as_bytes();
    let stop = if empty {
        open_end.saturating_sub(2)
    } else {
        open_end.saturating_sub(1)
    };
    let mut i = start + 1;
    while i < stop && !matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>') {
        i += 1;
    }
    let mut out = Vec::new();
    while i < stop {
        while i < stop && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i >= stop {
            break;
        }
        let name_start = i;
        while i < stop && !matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n' | b'=') {
            i += 1;
        }
        let name_end = i;
        while i < stop && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i >= stop || bytes[i] != b'=' {
            break;
        }
        i += 1;
        while i < stop && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i >= stop || !matches!(bytes[i], b'"' | b'\'') {
            break;
        }
        let quote = bytes[i];
        let value_start = i + 1;
        let Some(rel) = bytes[value_start..stop.max(value_start)]
            .iter()
            .position(|&b| b == quote)
        else {
            break;
        };
        let value_end = value_start + rel;
        out.push(AttrSpan {
            name: text[name_start..name_end].to_owned(),
            start: name_start,
            end: value_end + 1,
            value_start,
            value_end,
            quote,
        });
        i = value_end + 1;
    }
    out
}

/// Builds the element index of a document.
fn index(text: &str) -> XmlResult<Vec<Elem>> {
    let base = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let body = &text[base..];
    let mut reader = Reader::from_str(body);
    {
        let cfg = reader.config_mut();
        cfg.check_end_names = false;
        cfg.allow_unmatched_ends = true;
        cfg.allow_dangling_amp = true;
    }
    let fail = |offset: usize, message: String| {
        let (line, column) = line_col(text, offset);
        XmlError::Parse {
            message,
            line,
            column,
            offset,
        }
    };
    let mut elems: Vec<Elem> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut root_done = false;
    loop {
        let pos = base + usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let event = reader.read_event().map_err(|err| {
            let at = base + usize::try_from(reader.error_position()).unwrap_or(0);
            fail(at, err.to_string())
        })?;
        let after = base + usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                if stack.is_empty() && root_done {
                    return Err(fail(pos, "more than one root element".to_owned()));
                }
                let idx = elems.len();
                let parent = stack.last().copied();
                elems.push(Elem {
                    tag: e.name().0.to_owned(),
                    parent,
                    children: Vec::new(),
                    start: pos,
                    open_end: after,
                    close_start: if empty { after } else { 0 },
                    end: if empty { after } else { 0 },
                    empty,
                    has_text: false,
                    attrs: scan_attrs(text, pos, after, empty),
                });
                if let Some(p) = parent {
                    elems[p].children.push(idx);
                }
                if empty {
                    if parent.is_none() {
                        root_done = true;
                    }
                } else {
                    stack.push(idx);
                }
            }
            Event::End(ref e) => {
                let Some(top) = stack.pop() else {
                    return Err(fail(pos, "closing tag without an open element".to_owned()));
                };
                if elems[top].tag != e.name().0 {
                    return Err(fail(
                        pos,
                        format!(
                            "mismatched closing tag: expected </{}>, found </{}>",
                            elems[top].tag,
                            e.name().0
                        ),
                    ));
                }
                elems[top].close_start = pos;
                elems[top].end = after;
                if stack.is_empty() {
                    root_done = true;
                }
            }
            Event::Text(ref t) => {
                if let Some(&top) = stack.last() {
                    if !is_ws_only(t) {
                        elems[top].has_text = true;
                    }
                } else if !is_ws_only(t) {
                    return Err(fail(pos, "text outside the root element".to_owned()));
                }
            }
            Event::CData(_) | Event::GeneralRef(_) => {
                if let Some(&top) = stack.last() {
                    elems[top].has_text = true;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(&open) = stack.last() {
        return Err(fail(
            text.len(),
            format!("element <{}> is not closed", elems[open].tag),
        ));
    }
    if elems.is_empty() {
        return Err(XmlError::NoRoot);
    }
    Ok(elems)
}
