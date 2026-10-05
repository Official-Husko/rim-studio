//! A streaming index of the definitions in a Defs file, built without a tree.
//!
//! [`index_file`] reads the events of one file and records, for every top level element below the
//! root, its element name (the def type), the `Class`, `Name`, `ParentName`, `Abstract` and
//! `MayRequire` attributes, the text of the first `defName` and the first `label` child and the
//! byte offset of the element. It is not a validator: nesting errors are not detected (use
//! [`crate::parse_document`] for that), and a file that breaks half way yields the records read so
//! far together with the error in [`FileIndex`].

use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};

use crate::error::XmlError;
use crate::text::{line_col, predefined_entity, resolve_reference};
use crate::walk::decode;

/// One definition seen in a Defs file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndexRecord {
    /// The element name, which is the def type (`ThingDef`).
    pub def_type: String,
    /// The `Class` attribute, which overrides the type the game instantiates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// The trimmed text of the first `defName` child.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def_name: Option<String>,
    /// The `Name` attribute (a parent name other defs inherit from).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The `ParentName` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<String>,
    /// True when `Abstract` is `true` (any case, trimmed).
    #[serde(default)]
    pub is_abstract: bool,
    /// The trimmed text of the first `label` child.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The `MayRequire` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub may_require: Option<String>,
    /// Byte offset of the element's `<` in the file as given (a byte order mark counts).
    pub offset: u32,
}

/// The outcome of indexing one file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileIndex {
    /// The definitions, in file order.
    pub records: Vec<DefIndexRecord>,
    /// The name of the root element when one was seen.
    pub root_tag: Option<String>,
    /// Set when reading stopped early; the game would skip such a file.
    pub error: Option<XmlError>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Capture {
    DefName,
    Label,
}

fn attr_value(a: &quick_xml::events::attributes::Attribute<'_>) -> String {
    match a.normalized_value(XmlVersion::Implicit1_0) {
        Ok(v) => v.into_owned(),
        Err(_) => a.value.clone().into_owned(),
    }
}

fn start_record(e: &BytesStart<'_>, offset: usize) -> DefIndexRecord {
    let mut rec = DefIndexRecord {
        def_type: e.name().0.to_owned(),
        offset: u32::try_from(offset).unwrap_or(u32::MAX),
        ..DefIndexRecord::default()
    };
    for attr in e.attributes().flatten() {
        match attr.key.0 {
            "Name" => rec.name = Some(attr_value(&attr)),
            "ParentName" => rec.parent_name = Some(attr_value(&attr)),
            "Class" => rec.class = Some(attr_value(&attr)),
            "MayRequire" => rec.may_require = Some(attr_value(&attr)),
            "Abstract" => {
                rec.is_abstract = attr_value(&attr).trim().eq_ignore_ascii_case("true");
            }
            _ => {}
        }
    }
    rec
}

fn commit(capture: Option<Capture>, buf: &str, rec: Option<&mut DefIndexRecord>) {
    if let (Some(which), Some(rec)) = (capture, rec) {
        let value = non_empty_trimmed(buf);
        match which {
            Capture::DefName => rec.def_name = value,
            Capture::Label => rec.label = value,
        }
    }
}

fn non_empty_trimmed(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_owned())
    }
}

/// Indexes one Defs file. See the module documentation for what is recorded; read errors end the
/// scan quietly (use [`index_file_detailed`] to see them).
#[must_use]
pub fn index_file(bytes: &[u8]) -> Vec<DefIndexRecord> {
    index_file_detailed(bytes).records
}

/// Indexes one Defs file and reports how reading ended.
#[must_use]
pub fn index_file_detailed(bytes: &[u8]) -> FileIndex {
    let (text, base, _lossy) = decode(bytes);
    let text: &str = &text;
    let mut reader = Reader::from_str(text);
    {
        let cfg = reader.config_mut();
        cfg.check_end_names = false;
        cfg.allow_unmatched_ends = true;
        cfg.allow_dangling_amp = true;
    }
    let mut out = FileIndex::default();
    let mut depth: u32 = 0;
    let mut cur: Option<DefIndexRecord> = None;
    let mut def_name_done = false;
    let mut label_done = false;
    let mut capture: Option<Capture> = None;
    let mut buf = String::new();

    loop {
        let pos = usize::try_from(reader.buffer_position()).unwrap_or(0);
        let event = match reader.read_event() {
            Ok(ev) => ev,
            Err(err) => {
                commit(capture.take(), &buf, cur.as_mut());
                if let Some(rec) = cur.take() {
                    out.records.push(rec);
                }
                let at = usize::try_from(reader.error_position()).unwrap_or(pos);
                let (line, column) = line_col(text, at);
                out.error = Some(XmlError::Parse {
                    message: err.to_string(),
                    line,
                    column,
                    offset: at + base,
                });
                return out;
            }
        };
        match event {
            Event::Start(ref e) => {
                match depth {
                    0 => out.root_tag = Some(e.name().0.to_owned()),
                    1 => {
                        cur = Some(start_record(e, pos + base));
                        def_name_done = false;
                        label_done = false;
                    }
                    2 if cur.is_some() && capture.is_none() => {
                        let name = e.name().0;
                        if name == "defName" && !def_name_done {
                            def_name_done = true;
                            capture = Some(Capture::DefName);
                            buf.clear();
                        } else if name == "label" && !label_done {
                            label_done = true;
                            capture = Some(Capture::Label);
                            buf.clear();
                        }
                    }
                    _ => {}
                }
                depth += 1;
            }
            Event::Empty(ref e) => match depth {
                0 => out.root_tag = Some(e.name().0.to_owned()),
                1 => out.records.push(start_record(e, pos + base)),
                2 if cur.is_some() && capture.is_none() => match e.name().0 {
                    "defName" => def_name_done = true,
                    "label" => label_done = true,
                    _ => {}
                },
                _ => {}
            },
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 2 {
                    commit(capture.take(), &buf, cur.as_mut());
                } else if depth == 1
                    && let Some(rec) = cur.take()
                {
                    out.records.push(rec);
                }
            }
            Event::Text(ref t) if capture.is_some() => buf.push_str(&t.xml10_content()),
            Event::CData(ref c) if capture.is_some() => buf.push_str(&c.xml10_content()),
            Event::GeneralRef(ref r) if capture.is_some() => {
                let resolved = if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten()
                } else {
                    predefined_entity(r).or_else(|| resolve_reference(r))
                };
                match resolved {
                    Some(c) => buf.push(c),
                    None => {
                        buf.push('&');
                        buf.push_str(r);
                        buf.push(';');
                    }
                }
            }
            Event::DocType(_) => {
                commit(capture.take(), &buf, cur.as_mut());
                if let Some(rec) = cur.take() {
                    out.records.push(rec);
                }
                let (line, _) = line_col(text, pos);
                out.error = Some(XmlError::DtdRejected { line });
                return out;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    commit(capture.take(), &buf, cur.as_mut());
    if let Some(rec) = cur.take() {
        out.records.push(rec);
    }
    if depth > 0 {
        let (line, column) = line_col(text, text.len());
        out.error = Some(XmlError::Parse {
            message: "unexpected end of input".to_owned(),
            line,
            column,
            offset: text.len() + base,
        });
    }
    out
}
