//! Node trees to XML text.
//!
//! Output is deterministic: attributes are written in stored order, indentation is fixed by the
//! options, text and attribute values are escaped so that [`crate::parse_document`] in game mode
//! reads back the same tree (for trees without whitespace only or adjacent text nodes).
//!
//! Layout rules: an element whose children are all elements is written one child per line; an
//! element with any text child (mixed content) and every element below `xml:space="preserve"` is
//! written inline so that no insignificant white space becomes significant.

use rimstudio_core::tree::{Child, Node};

use crate::error::{XmlError, XmlResult};
use crate::text::{escape_attr, escape_text, is_valid_name};

/// How one level of nesting is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Indent {
    /// This many spaces per level.
    Spaces(u8),
    /// One tab per level.
    Tab,
    /// No indentation and no line breaks between elements.
    None,
}

impl Indent {
    pub(crate) fn unit(self) -> String {
        match self {
            Indent::Spaces(n) => " ".repeat(usize::from(n)),
            Indent::Tab => "\t".to_owned(),
            Indent::None => String::new(),
        }
    }
}

/// Line ending written between lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineEnding {
    /// `\n`.
    #[default]
    Lf,
    /// `\r\n`.
    Crlf,
}

impl LineEnding {
    /// The line ending as text.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    }
}

/// Options of the renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOpts {
    /// Indentation per nesting level.
    pub indent: Indent,
    /// Line ending.
    pub eol: LineEnding,
    /// Write `<?xml version="1.0" encoding="utf-8"?>` first.
    pub declaration: bool,
    /// Write a UTF-8 byte order mark first (the game accepts both).
    pub bom: bool,
    /// End the text with a line ending.
    pub final_newline: bool,
    /// Write `<a/>` for empty elements instead of `<a></a>`.
    pub self_close_empty: bool,
    /// Put an empty line between the top level entries of a Defs or Patch file.
    pub blank_line_between_top_level: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            indent: Indent::Spaces(2),
            eol: LineEnding::Lf,
            declaration: true,
            bom: false,
            final_newline: true,
            self_close_empty: true,
            blank_line_between_top_level: false,
        }
    }
}

impl RenderOpts {
    /// Options for a fragment: no declaration and no final line ending.
    #[must_use]
    pub fn fragment() -> Self {
        Self {
            declaration: false,
            final_newline: false,
            ..Self::default()
        }
    }
}

/// A group of top level nodes with an optional comment header, for generated files.
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    /// Comment written above the group (no `--` sequences survive; they are softened).
    pub comment: Option<&'a str>,
    /// The nodes of the group.
    pub nodes: &'a [Node],
}

impl<'a> Section<'a> {
    /// A group without a header.
    #[must_use]
    pub fn plain(nodes: &'a [Node]) -> Self {
        Section {
            comment: None,
            nodes,
        }
    }

    /// A group with a comment header.
    #[must_use]
    pub fn with_comment(comment: &'a str, nodes: &'a [Node]) -> Self {
        Section {
            comment: Some(comment),
            nodes,
        }
    }
}

const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>";

struct Layout<'a> {
    base: &'a str,
    unit: &'a str,
    block: bool,
    self_close: bool,
}

struct Frame<'a> {
    node: &'a Node,
    next: usize,
    depth: usize,
    block: bool,
    preserve: bool,
}

fn has_space_attr(node: &Node, value: &str) -> bool {
    node.attrs
        .iter()
        .any(|(k, v)| k == "xml:space" && v == value)
}

fn is_block(node: &Node, preserve: bool, layout: &Layout<'_>, inline_ctx: bool) -> bool {
    layout.block
        && !inline_ctx
        && !preserve
        && !node.children.is_empty()
        && node.children.iter().all(|c| matches!(c, Child::Element(_)))
}

fn push_indent(out: &mut String, unit: &str, depth: usize) {
    for _ in 0..depth {
        out.push_str(unit);
    }
}

fn open_tag(out: &mut String, node: &Node) {
    out.push('<');
    out.push_str(&node.tag);
    for (k, v) in &node.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_attr(v));
        out.push('"');
    }
}

/// Writes one element (and its subtree) with `\n` line breaks. The first line starts at the
/// current end of `out`; deeper lines are indented relative to `depth`.
fn write_node(out: &mut String, root: &Node, layout: &Layout<'_>, depth: usize, inline_ctx: bool) {
    let mut stack: Vec<Frame<'_>> = Vec::new();
    // Enter the root.
    enter(out, root, layout, depth, inline_ctx, false, &mut stack);
    while let Some(top) = stack.last_mut() {
        let node = top.node;
        match node.children.get(top.next) {
            None => {
                let (depth, block) = (top.depth, top.block);
                stack.pop();
                if block {
                    out.push('\n');
                    out.push_str(layout.base);
                    push_indent(out, layout.unit, depth);
                }
                out.push_str("</");
                out.push_str(&node.tag);
                out.push('>');
            }
            Some(child) => {
                top.next += 1;
                let (depth, block, preserve) = (top.depth, top.block, top.preserve);
                match child {
                    Child::Text(t) => out.push_str(&escape_text(t)),
                    Child::Element(el) => {
                        if block {
                            out.push('\n');
                            out.push_str(layout.base);
                            push_indent(out, layout.unit, depth + 1);
                        }
                        enter(out, el, layout, depth + 1, !block, preserve, &mut stack);
                    }
                }
            }
        }
    }
}

fn enter<'a>(
    out: &mut String,
    node: &'a Node,
    layout: &Layout<'_>,
    depth: usize,
    inline_ctx: bool,
    parent_preserve: bool,
    stack: &mut Vec<Frame<'a>>,
) {
    open_tag(out, node);
    let live = node
        .children
        .iter()
        .any(|c| !matches!(c, Child::Text(t) if t.is_empty()));
    if !live {
        if layout.self_close {
            out.push_str("/>");
        } else {
            out.push_str("></");
            out.push_str(&node.tag);
            out.push('>');
        }
        return;
    }
    out.push('>');
    let preserve = if has_space_attr(node, "preserve") {
        true
    } else if has_space_attr(node, "default") {
        false
    } else {
        parent_preserve
    };
    stack.push(Frame {
        node,
        next: 0,
        depth,
        block: is_block(node, preserve, layout, inline_ctx),
        preserve,
    });
}

fn finish(mut body: String, opts: &RenderOpts) -> String {
    if opts.final_newline {
        body.push('\n');
    }
    let mut out = String::with_capacity(body.len() + 8);
    if opts.bom {
        out.push('\u{FEFF}');
    }
    match opts.eol {
        LineEnding::Lf => out.push_str(&body),
        LineEnding::Crlf => out.push_str(&body.replace('\n', "\r\n")),
    }
    out
}

/// Renders one element as a complete document.
#[must_use]
pub fn render(node: &Node, opts: &RenderOpts) -> String {
    let unit = opts.indent.unit();
    let layout = Layout {
        base: "",
        unit: &unit,
        block: opts.indent != Indent::None,
        self_close: opts.self_close_empty,
    };
    let mut body = String::new();
    if opts.declaration {
        body.push_str(DECLARATION);
        body.push('\n');
    }
    write_node(&mut body, node, &layout, 0, false);
    finish(body, opts)
}

/// Renders one element after checking that every name and character can be written.
///
/// # Errors
/// [`XmlError::InvalidValue`] for an element or attribute name that is not an XML name.
pub fn try_render(node: &Node, opts: &RenderOpts) -> XmlResult<String> {
    check_names(node)?;
    Ok(render(node, opts))
}

pub(crate) fn check_names(node: &Node) -> XmlResult<()> {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if !is_valid_name(&n.tag) {
            return Err(XmlError::InvalidValue {
                what: "element name".to_owned(),
                reason: format!("{:?} is not an XML name", n.tag),
            });
        }
        for (k, _) in &n.attrs {
            if !is_valid_name(k) {
                return Err(XmlError::InvalidValue {
                    what: "attribute name".to_owned(),
                    reason: format!("{k:?} is not an XML name"),
                });
            }
        }
        stack.extend(n.elements());
    }
    Ok(())
}

/// Renders a fragment for insertion into an existing document: the first line has no indent, deeper
/// lines start with `base` and add `unit` per nesting level. An empty `unit` renders compactly.
/// Uses `\n` line breaks.
pub(crate) fn render_at(node: &Node, base: &str, unit: &str, self_close: bool) -> String {
    let layout = Layout {
        base,
        unit,
        block: !unit.is_empty(),
        self_close,
    };
    let mut out = String::new();
    write_node(&mut out, node, &layout, 0, false);
    out
}

/// Renders a Defs file: a `Defs` root holding `nodes`.
#[must_use]
pub fn render_defs_file(nodes: &[Node], opts: &RenderOpts) -> String {
    render_list_file("Defs", &[Section::plain(nodes)], opts)
}

/// Renders a Patch file: a `Patch` root holding the operations.
#[must_use]
pub fn render_patch_file(operations: &[Node], opts: &RenderOpts) -> String {
    render_list_file("Patch", &[Section::plain(operations)], opts)
}

/// Renders a file whose root `root_tag` holds groups of top level nodes, each group optionally
/// introduced by a comment.
#[must_use]
pub fn render_list_file(root_tag: &str, sections: &[Section<'_>], opts: &RenderOpts) -> String {
    let unit = opts.indent.unit();
    let block = opts.indent != Indent::None;
    let layout = Layout {
        base: "",
        unit: &unit,
        block,
        self_close: opts.self_close_empty,
    };
    let mut body = String::new();
    if opts.declaration {
        body.push_str(DECLARATION);
        body.push('\n');
    }
    let empty = sections
        .iter()
        .all(|s| s.nodes.is_empty() && s.comment.is_none());
    if empty {
        body.push('<');
        body.push_str(root_tag);
        body.push_str(if opts.self_close_empty { "/>" } else { "></" });
        if !opts.self_close_empty {
            body.push_str(root_tag);
            body.push('>');
        }
        return finish(body, opts);
    }
    body.push('<');
    body.push_str(root_tag);
    body.push('>');
    let mut first = true;
    for section in sections {
        if section.nodes.is_empty() && section.comment.is_none() {
            continue;
        }
        let mut after_comment = false;
        if let Some(comment) = section.comment {
            separator(&mut body, &mut first, opts, block, false);
            body.push_str(&unit);
            write_comment(&mut body, comment, &unit);
            after_comment = true;
        }
        for node in section.nodes {
            separator(&mut body, &mut first, opts, block, after_comment);
            after_comment = false;
            body.push_str(&unit);
            write_node(&mut body, node, &layout, 1, false);
        }
    }
    body.push('\n');
    body.push_str("</");
    body.push_str(root_tag);
    body.push('>');
    finish(body, opts)
}

fn separator(
    body: &mut String,
    first: &mut bool,
    opts: &RenderOpts,
    block: bool,
    glued_to_comment: bool,
) {
    if block {
        body.push('\n');
        if !*first && opts.blank_line_between_top_level && !glued_to_comment {
            body.push('\n');
        }
    }
    *first = false;
}

/// Writes a comment, softening `--` and a trailing `-` so the comment stays well formed.
pub(crate) fn write_comment(out: &mut String, text: &str, unit: &str) {
    let mut clean = String::with_capacity(text.len());
    let mut prev_dash = false;
    for c in text.chars() {
        if c == '-' && prev_dash {
            clean.push(' ');
        }
        clean.push(c);
        prev_dash = c == '-';
    }
    if clean.ends_with('-') {
        clean.push(' ');
    }
    let clean = clean.replace('\r', "");
    if clean.contains('\n') {
        out.push_str("<!--\n");
        for line in clean.lines() {
            out.push_str(unit);
            out.push_str(unit);
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(unit);
        out.push_str("-->");
    } else {
        out.push_str("<!-- ");
        out.push_str(clean.trim());
        out.push_str(" -->");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParseMode, parse_document};
    use rimstudio_core::tree::NodeBuilder;

    fn sample() -> Node {
        NodeBuilder::new("ThingDef")
            .attr("ParentName", "RS_BaseGun")
            .text_elem("defName", "RS_TestRifle")
            .elem("statBases", |b| b.text_elem("Mass", "3.5"))
            .empty_elem("comps")
            .build()
    }

    #[test]
    fn renders_declaration_indent_and_self_closing_elements() {
        let text = render(&sample(), &RenderOpts::default());
        let expected = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ThingDef ParentName=\"RS_BaseGun\">\n  <defName>RS_TestRifle</defName>\n  <statBases>\n    <Mass>3.5</Mass>\n  </statBases>\n  <comps/>\n</ThingDef>\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn crlf_and_bom_and_tabs_are_applied() {
        let opts = RenderOpts {
            eol: LineEnding::Crlf,
            bom: true,
            indent: Indent::Tab,
            declaration: false,
            ..RenderOpts::default()
        };
        let text = render(&sample(), &opts);
        assert!(text.starts_with("\u{FEFF}<ThingDef"));
        assert!(text.contains("\r\n\t<defName>"));
        assert!(!text.contains("\n\n"));
    }

    #[test]
    fn mixed_content_is_not_indented() {
        let node = NodeBuilder::new("p")
            .text("a ")
            .elem("b", |b| b.text("bold"))
            .text(" c")
            .build();
        assert_eq!(
            render(&node, &RenderOpts::fragment()),
            "<p>a <b>bold</b> c</p>"
        );
    }

    #[test]
    fn preserve_scope_is_written_inline() {
        let node = NodeBuilder::new("root")
            .attr("xml:space", "preserve")
            .elem("a", |b| b.text_elem("b", "x"))
            .build();
        assert_eq!(
            render(&node, &RenderOpts::fragment()),
            "<root xml:space=\"preserve\"><a><b>x</b></a></root>"
        );
    }

    #[test]
    fn escaping_is_applied_to_text_and_attributes() {
        let node = NodeBuilder::new("a")
            .attr("k", "q\"<&\n")
            .text("1 < 2 & 3 > 2\r")
            .build();
        let text = render(&node, &RenderOpts::fragment());
        assert_eq!(
            text,
            "<a k=\"q&quot;&lt;&amp;&#10;\">1 &lt; 2 &amp; 3 &gt; 2&#13;</a>"
        );
        let back = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        assert_eq!(back.root, node);
    }

    #[test]
    fn defs_file_with_sections_and_comments() {
        let a = [Node::with_text("ThingDef", "a")];
        let b = [Node::with_text("ThingDef", "b")];
        let text = render_list_file(
            "Defs",
            &[
                Section::with_comment("first -- group-", &a),
                Section::plain(&b),
            ],
            &RenderOpts {
                blank_line_between_top_level: true,
                ..RenderOpts::default()
            },
        );
        assert!(text.contains("<!-- first - - group- -->"));
        assert!(text.contains("<ThingDef>a</ThingDef>\n\n  <ThingDef>b</ThingDef>"));
        assert!(text.contains("-->\n  <ThingDef>a</ThingDef>"));
        let doc = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        assert_eq!(doc.root.elements().count(), 2);
    }

    #[test]
    fn empty_defs_file_is_a_self_closing_root() {
        let text = render_defs_file(&[], &RenderOpts::default());
        assert_eq!(
            text,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Defs/>\n"
        );
    }

    #[test]
    fn try_render_rejects_bad_names() {
        let node = Node::new("not a name");
        assert_eq!(
            try_render(&node, &RenderOpts::default())
                .unwrap_err()
                .code(),
            "xml.invalid-value"
        );
    }

    #[test]
    fn control_characters_are_written_as_references() {
        let node = Node::with_text("a", "x\u{1}y\u{0}z");
        assert_eq!(render(&node, &RenderOpts::fragment()), "<a>x&#x1;yz</a>");
    }
}
