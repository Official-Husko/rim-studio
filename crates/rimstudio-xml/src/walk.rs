//! The event walker: one place that turns bytes into start, end and text callbacks.
//!
//! Both the node tree reader and the About reader sit on top of it, so the game's loading rules
//! (byte order mark, forced UTF-8, whitespace and comment dropping, DTD refusal) and the tolerant
//! recovery rules exist exactly once.

use std::borrow::Cow;

use quick_xml::XmlVersion;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rimstudio_core::diag::{Diagnostic, Severity, Span};
use rimstudio_core::tree::MAX_DEPTH;

use crate::error::{XmlError, codes};
use crate::modes::ParseMode;
use crate::text::{is_valid_name, is_ws_only, line_col, predefined_entity};

/// Receives the structure of a document in order.
pub(crate) trait Sink {
    /// An element starts. `attrs` holds unescaped, normalised values in source order.
    fn start(&mut self, tag: &str, attrs: Vec<(String, String)>);
    /// The innermost open element ends.
    fn end(&mut self);
    /// A text run (adjacent text, CDATA and references merged) inside the innermost element.
    fn text(&mut self, text: String);
    /// A comment or processing instruction was dropped at the current position.
    fn comment(&mut self) {}
}

/// What the walk learned besides the callbacks.
#[derive(Debug, Default)]
pub(crate) struct WalkInfo {
    /// Warnings collected on the way.
    pub(crate) diagnostics: Vec<Diagnostic>,
}

struct Frame {
    name: String,
    preserve: bool,
    prefixes: Vec<String>,
}

struct Ctx<'a> {
    text: &'a str,
    mode: ParseMode,
    base: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn parse_error(&self, offset: usize, message: impl Into<String>) -> XmlError {
        let (line, column) = line_col(self.text, offset);
        XmlError::Parse {
            message: message.into(),
            line,
            column,
            offset: offset + self.base,
        }
    }

    fn warn(&mut self, code: rimstudio_core::diag::DiagCode, offset: usize, message: String) {
        let (line, column) = line_col(self.text, offset);
        self.diagnostics.push(
            Diagnostic::new(code, Severity::Warning, message)
                .with_span(Span::LineCol { line, column }),
        );
    }

    /// Fails in game mode, warns in tolerant mode.
    fn problem(
        &mut self,
        code: rimstudio_core::diag::DiagCode,
        offset: usize,
        message: String,
    ) -> Result<(), XmlError> {
        if self.mode.is_tolerant() {
            self.warn(code, offset, message);
            Ok(())
        } else {
            Err(self.parse_error(offset, message))
        }
    }
}

/// Decodes the bytes the way the game's loader does: a UTF-8 byte order mark is dropped, the rest
/// is read as UTF-8 and invalid sequences become U+FFFD. Returns the text, the number of bytes
/// dropped at the front and whether invalid sequences were replaced.
pub(crate) fn decode(bytes: &[u8]) -> (Cow<'_, str>, usize, bool) {
    let (body, base) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (rest, 3),
        None => (bytes, 0),
    };
    match std::str::from_utf8(body) {
        Ok(s) => (Cow::Borrowed(s), base, false),
        Err(_) => (
            Cow::Owned(String::from_utf8_lossy(body).into_owned()),
            base,
            true,
        ),
    }
}

fn depth_limit() -> XmlError {
    XmlError::TooDeep { max: MAX_DEPTH }
}

/// Walks the document in `bytes`, calling the sink. See [`ParseMode`] for the two behaviours.
///
/// # Errors
/// In game mode the first well-formedness problem; in tolerant mode only a document from which
/// nothing could be recovered (no root element).
pub(crate) fn walk(
    bytes: &[u8],
    mode: ParseMode,
    sink: &mut impl Sink,
) -> Result<WalkInfo, XmlError> {
    let (text, base, lossy) = decode(bytes);
    let text: &str = &text;
    let mut ctx = Ctx {
        text,
        mode,
        base,
        diagnostics: Vec::new(),
    };
    if lossy {
        ctx.diagnostics.push(Diagnostic::new(
            codes::INVALID_UTF8,
            Severity::Warning,
            "the input is not valid UTF-8; invalid sequences were replaced",
        ));
    }
    let mut reader = Reader::from_str(text);
    {
        let cfg = reader.config_mut();
        cfg.check_end_names = false;
        cfg.allow_unmatched_ends = true;
        cfg.allow_dangling_amp = mode.is_tolerant();
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut root_tag: Option<String> = None;
    let mut skipping: usize = 0;
    let mut pending = String::new();
    let mut pending_ws_only = true;

    loop {
        let pos = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let event = match reader.read_event() {
            Ok(ev) => ev,
            Err(err) => {
                let at = usize::try_from(reader.error_position()).unwrap_or(pos);
                if mode.is_tolerant() && root_tag.is_some() {
                    ctx.warn(
                        codes::PARSE_RECOVERED,
                        at,
                        format!("parsing stopped: {err}"),
                    );
                    break;
                }
                return Err(ctx.parse_error(at, err.to_string()));
            }
        };

        // Text-like events accumulate; everything else flushes first.
        match &event {
            Event::Text(t) => {
                if skipping == 0 {
                    let content = t.xml10_content();
                    if !is_ws_only(&content) {
                        pending_ws_only = false;
                    }
                    pending.push_str(&content);
                }
                continue;
            }
            Event::CData(c) => {
                if skipping == 0 {
                    let content = c.xml10_content();
                    if !content.is_empty() {
                        pending_ws_only = false;
                    }
                    pending.push_str(&content);
                }
                continue;
            }
            Event::GeneralRef(r) => {
                if skipping == 0 {
                    if r.is_char_ref() {
                        match r.resolve_char_ref() {
                            Ok(Some(ch)) => {
                                pending.push(ch);
                                pending_ws_only = false;
                            }
                            _ => {
                                ctx.problem(
                                    codes::UNDEFINED_ENTITY,
                                    pos,
                                    format!("invalid character reference &{};", &**r),
                                )?;
                                pending.push('&');
                                pending.push_str(r);
                                pending.push(';');
                                pending_ws_only = false;
                            }
                        }
                    } else if let Some(ch) = predefined_entity(r) {
                        pending.push(ch);
                        pending_ws_only = false;
                    } else {
                        ctx.problem(
                            codes::UNDEFINED_ENTITY,
                            pos,
                            format!("undefined entity &{};", &**r),
                        )?;
                        pending.push('&');
                        pending.push_str(r);
                        pending.push(';');
                        pending_ws_only = false;
                    }
                }
                continue;
            }
            Event::Comment(_) | Event::PI(_) => {
                if skipping == 0 {
                    sink.comment();
                }
                continue;
            }
            _ => {}
        }

        // Flush the pending text run.
        if !pending.is_empty() {
            let run = std::mem::take(&mut pending);
            let ws = std::mem::replace(&mut pending_ws_only, true);
            match stack.last() {
                Some(top) => {
                    if !ws || top.preserve {
                        sink.text(run);
                    }
                }
                None => {
                    if !ws {
                        let at = pos.saturating_sub(run.len());
                        ctx.problem(
                            codes::EXTRA_CONTENT,
                            at,
                            "text outside the root element".to_owned(),
                        )?;
                    }
                }
            }
        } else {
            pending_ws_only = true;
        }

        match event {
            Event::Decl(_) => {
                if pos != 0 {
                    ctx.problem(
                        codes::EXTRA_CONTENT,
                        pos,
                        "the XML declaration is not at the very start of the document".to_owned(),
                    )?;
                }
            }
            Event::DocType(_) => {
                if mode.is_tolerant() {
                    ctx.warn(
                        codes::DTD_IGNORED,
                        pos,
                        "a document type declaration was ignored".to_owned(),
                    );
                } else {
                    let (line, _) = line_col(text, pos);
                    return Err(XmlError::DtdRejected { line });
                }
            }
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(event, Event::Empty(_));
                if skipping > 0 {
                    if !is_empty {
                        skipping += 1;
                    }
                    continue;
                }
                if stack.is_empty() && root_tag.is_some() {
                    ctx.problem(codes::EXTRA_ROOT, pos, "a second root element".to_owned())?;
                    if !is_empty {
                        skipping = 1;
                    }
                    continue;
                }
                let name = e.name().0;
                if !is_valid_name(name) {
                    ctx.problem(
                        codes::PARSE_RECOVERED,
                        pos,
                        format!("invalid element name {name:?}"),
                    )?;
                }
                let (attrs, declared, preserve_attr) = collect_attrs(&mut ctx, e, pos)?;
                check_prefix(&mut ctx, "element", name, &declared, &stack, pos)?;
                for (k, _) in &attrs {
                    check_prefix(&mut ctx, "attribute", k, &declared, &stack, pos)?;
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(depth_limit());
                }
                let preserve = match preserve_attr {
                    Some(p) => p,
                    None => stack.last().is_some_and(|f| f.preserve),
                };
                if root_tag.is_none() {
                    root_tag = Some(name.to_owned());
                }
                sink.start(name, attrs);
                if is_empty {
                    sink.end();
                } else {
                    stack.push(Frame {
                        name: name.to_owned(),
                        preserve,
                        prefixes: declared,
                    });
                }
            }
            Event::End(ref e) => {
                if skipping > 0 {
                    skipping -= 1;
                    continue;
                }
                let name = e.name().0;
                match stack.last() {
                    None => {
                        ctx.problem(
                            codes::MISMATCHED_END,
                            pos,
                            format!("closing tag </{name}> without an open element"),
                        )?;
                    }
                    Some(top) if top.name == name => {
                        stack.pop();
                        sink.end();
                    }
                    Some(top) => {
                        if !mode.is_tolerant() {
                            return Err(ctx.parse_error(
                                pos,
                                format!(
                                    "mismatched closing tag: expected </{}>, found </{name}>",
                                    top.name
                                ),
                            ));
                        }
                        match stack.iter().rposition(|f| f.name == name) {
                            Some(idx) => {
                                ctx.warn(
                                    codes::MISMATCHED_END,
                                    pos,
                                    format!(
                                        "closing tag </{name}> closes {} unclosed element(s)",
                                        stack.len() - idx - 1
                                    ),
                                );
                                while stack.len() > idx {
                                    stack.pop();
                                    sink.end();
                                }
                            }
                            None => {
                                ctx.warn(
                                    codes::MISMATCHED_END,
                                    pos,
                                    format!("closing tag </{name}> matches no open element"),
                                );
                            }
                        }
                    }
                }
            }
            Event::Eof => break,
            // Text-like events were handled above.
            _ => {}
        }
    }

    // End of input.
    if !pending.is_empty() {
        let run = std::mem::take(&mut pending);
        if let Some(top) = stack.last()
            && (!pending_ws_only || top.preserve)
        {
            sink.text(run);
        }
    }
    if !stack.is_empty() {
        let end = text.len();
        if !mode.is_tolerant() {
            let open = stack.last().map(|f| f.name.clone()).unwrap_or_default();
            return Err(ctx.parse_error(
                end,
                format!("unexpected end of input: element <{open}> is not closed"),
            ));
        }
        ctx.warn(
            codes::UNCLOSED,
            end,
            format!("{} element(s) were not closed", stack.len()),
        );
        while stack.pop().is_some() {
            sink.end();
        }
    }
    if root_tag.is_none() {
        return Err(XmlError::NoRoot);
    }
    Ok(WalkInfo {
        diagnostics: ctx.diagnostics,
    })
}

type Attrs = (Vec<(String, String)>, Vec<String>, Option<bool>);

/// Collects the attributes of a start tag: values, namespace prefixes it declares and an explicit
/// `xml:space` setting.
fn collect_attrs(
    ctx: &mut Ctx<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    pos: usize,
) -> Result<Attrs, XmlError> {
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut declared: Vec<String> = Vec::new();
    let mut preserve: Option<bool> = None;
    let mut guard = 0usize;
    for item in e.attributes() {
        guard += 1;
        if guard > 100_000 {
            break;
        }
        let attr = match item {
            Ok(a) => a,
            Err(err) => {
                ctx.problem(codes::BAD_ATTRIBUTE, pos, format!("bad attribute: {err}"))?;
                continue;
            }
        };
        let key = attr.key.0;
        if !is_valid_name(key) {
            ctx.problem(
                codes::BAD_ATTRIBUTE,
                pos,
                format!("invalid attribute name {key:?}"),
            )?;
        }
        let value = match attr.normalized_value(XmlVersion::Implicit1_0) {
            Ok(v) => v.into_owned(),
            Err(err) => {
                ctx.problem(
                    codes::UNDEFINED_ENTITY,
                    pos,
                    format!("bad value for attribute {key:?}: {err}"),
                )?;
                attr.value.clone().into_owned()
            }
        };
        if let Some(prefix) = key.strip_prefix("xmlns:") {
            declared.push(prefix.to_owned());
        } else if key == "xml:space" {
            match value.as_str() {
                "preserve" => preserve = Some(true),
                "default" => preserve = Some(false),
                _ => {}
            }
        }
        attrs.push((key.to_owned(), value));
    }
    Ok((attrs, declared, preserve))
}

fn check_prefix(
    ctx: &mut Ctx<'_>,
    what: &str,
    name: &str,
    declared: &[String],
    stack: &[Frame],
    pos: usize,
) -> Result<(), XmlError> {
    let Some((prefix, _)) = name.split_once(':') else {
        return Ok(());
    };
    if prefix == "xml" || prefix == "xmlns" {
        return Ok(());
    }
    let known = declared.iter().any(|d| d == prefix)
        || stack.iter().any(|f| f.prefixes.iter().any(|d| d == prefix));
    if known {
        return Ok(());
    }
    ctx.problem(
        codes::UNDECLARED_PREFIX,
        pos,
        format!("{what} prefix {prefix:?} is not declared"),
    )
}
