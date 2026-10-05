//! Bytes to node trees.
//!
//! [`parse_document`] reads one XML document into a [`Node`] tree. The same function serves the
//! game faithful mode ([`ParseMode::Game`]) and the recovering mode ([`ParseMode::Tolerant`]); the
//! tree shape is identical in both: comments and whitespace only text are gone, CDATA and entity
//! references are plain text, attributes keep their source order.

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::{Child, Node};

use crate::error::{XmlError, XmlResult, codes};
use crate::modes::ParseMode;
use crate::walk::{Sink, walk};

/// A parsed document: the root element and the warnings found while reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDoc {
    /// The root element.
    pub root: Node,
    /// Warnings (always empty for a clean game mode parse).
    pub diagnostics: Vec<Diagnostic>,
}

/// A Defs or Patches file: the top level nodes below the root element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTopLevel {
    /// The name of the root element (`Defs` or `Patch` in a healthy file).
    pub root_tag: String,
    /// The root's child elements in file order (text directly below the root is dropped).
    pub nodes: Vec<Node>,
    /// Warnings found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

struct TreeSink {
    stack: Vec<Node>,
    root: Option<Node>,
}

impl Sink for TreeSink {
    fn start(&mut self, tag: &str, attrs: Vec<(String, String)>) {
        self.stack.push(Node {
            tag: tag.to_owned(),
            attrs,
            children: Vec::new(),
        });
    }

    fn end(&mut self) {
        if let Some(done) = self.stack.pop() {
            match self.stack.last_mut() {
                Some(parent) => parent.children.push(Child::Element(done)),
                None => self.root = Some(done),
            }
        }
    }

    fn text(&mut self, text: String) {
        if let Some(top) = self.stack.last_mut() {
            top.children.push(Child::Text(text));
        }
    }
}

/// Parses `bytes` into a node tree.
///
/// In [`ParseMode::Game`] the function fails exactly where the game's loader fails; see
/// [`ParseMode`] for the rules. In [`ParseMode::Tolerant`] it returns what could be read together
/// with warnings, and fails only when there is no root element at all.
///
/// # Errors
/// [`XmlError::Parse`], [`XmlError::DtdRejected`], [`XmlError::NoRoot`] or [`XmlError::TooDeep`].
pub fn parse_document(bytes: &[u8], mode: ParseMode) -> XmlResult<ParsedDoc> {
    let mut sink = TreeSink {
        stack: Vec::new(),
        root: None,
    };
    let info = walk(bytes, mode, &mut sink)?;
    match sink.root {
        Some(root) => Ok(ParsedDoc {
            root,
            diagnostics: info.diagnostics,
        }),
        None => Err(XmlError::NoRoot),
    }
}

/// [`parse_document`] for text that is already a string.
///
/// # Errors
/// As [`parse_document`].
pub fn parse_str(text: &str, mode: ParseMode) -> XmlResult<ParsedDoc> {
    parse_document(text.as_bytes(), mode)
}

/// Parses a file whose root holds a list of top level elements (a Defs or Patches file).
///
/// Adds a [`codes::UNEXPECTED_ROOT`] warning when the root is not named `expected_root`; the
/// game logs that too but still imports the children.
///
/// # Errors
/// As [`parse_document`].
pub fn parse_top_level(
    bytes: &[u8],
    mode: ParseMode,
    expected_root: &str,
) -> XmlResult<ParsedTopLevel> {
    let ParsedDoc {
        root,
        mut diagnostics,
    } = parse_document(bytes, mode)?;
    if root.tag != expected_root {
        diagnostics.push(Diagnostic::new(
            codes::UNEXPECTED_ROOT.clone(),
            Severity::Warning,
            format!(
                "the root element is <{}>, expected <{expected_root}>",
                root.tag
            ),
        ));
    }
    let Node { tag, children, .. } = root;
    let nodes = children
        .into_iter()
        .filter_map(Child::into_element)
        .collect();
    Ok(ParsedTopLevel {
        root_tag: tag,
        nodes,
        diagnostics,
    })
}
