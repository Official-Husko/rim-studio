//! `LoadFolders.xml`: reading into core's [`LoadFoldersSpec`], span edits and creation.
//!
//! Reading follows the game: every child of the root is a version block (its name lower cased with
//! a leading `v` removed, repeated blocks merged), every child of a block is an entry whose text is
//! a folder relative to the mod root (`/` or `\` mean the root itself), and the attributes
//! `IfModActive`, `IfModActiveAll` and `IfModNotActive` hold comma separated package ids. Other
//! attributes are ignored by the game and are reported.
//!
//! Editing keeps the file as it is: entries are added or removed by splicing, new blocks are
//! appended to the root. Gated entries (`IfModActive`) are how the Combat Extended patch folder
//! is kept out of a vanilla load.

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_core::version::normalize_version_key;

use crate::about::normalise_encoding;
use crate::edit::{ElementInfo, SpanEditor};
use crate::error::{XmlResult, codes};
use crate::modes::ParseMode;
use crate::reader::parse_document;
use crate::render::{RenderOpts, render};

/// The result of [`read`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadFoldersRead {
    /// The blocks and entries in file order.
    pub spec: LoadFoldersSpec,
    /// Warnings found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

fn split_ids(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn make_entry(attrs: &[(String, String)], text: &str) -> LoadEntry {
    let path = match text {
        "/" | "\\" => String::new(),
        other if other.trim().is_empty() => String::new(),
        other => other.to_owned(),
    };
    let mut entry = LoadEntry {
        path,
        ..LoadEntry::default()
    };
    for (k, v) in attrs {
        match k.as_str() {
            "IfModActive" => entry.if_active = split_ids(v),
            "IfModActiveAll" => entry.if_active_all = split_ids(v),
            "IfModNotActive" => entry.if_not_active = split_ids(v),
            other => entry.ignored_attributes.push(other.to_owned()),
        }
    }
    entry
}

fn same_entry(a: &LoadEntry, b: &LoadEntry) -> bool {
    a.path == b.path
        && lower(&a.if_active) == lower(&b.if_active)
        && lower(&a.if_active_all) == lower(&b.if_active_all)
        && lower(&a.if_not_active) == lower(&b.if_not_active)
}

fn lower(ids: &[String]) -> Vec<String> {
    ids.iter().map(|s| s.trim().to_lowercase()).collect()
}

/// Reads a `LoadFolders.xml` (any UTF byte order mark is honoured, the root name is not checked).
///
/// # Errors
/// [`crate::XmlError`] when the file has no root element at all; the game then uses an empty
/// specification and the caller can do the same.
pub fn read(bytes: &[u8]) -> XmlResult<LoadFoldersRead> {
    let data = normalise_encoding(bytes);
    let doc = parse_document(&data, ParseMode::Tolerant)?;
    let mut diagnostics = doc.diagnostics;
    let mut spec = LoadFoldersSpec::new();
    for block in doc.root.elements() {
        let mut entries = Vec::new();
        for li in block.elements() {
            let entry = make_entry(&li.attrs, &li.text_content());
            for attr in &entry.ignored_attributes {
                diagnostics.push(Diagnostic::new(
                    codes::LOADFOLDERS_IGNORED_ATTR,
                    Severity::Warning,
                    format!(
                        "attribute {attr} on the entry {:?} of block {} is not read by the game",
                        entry.path, block.tag
                    ),
                ));
            }
            entries.push(entry);
        }
        spec.add_block(&block.tag, entries);
    }
    Ok(LoadFoldersRead { spec, diagnostics })
}

/// The element name written for a block key: `v1.6` for numeric keys, `default` as it is.
#[must_use]
pub fn block_tag(key: &str) -> String {
    if key.starts_with(|c: char| c.is_ascii_digit()) {
        format!("v{key}")
    } else {
        key.to_owned()
    }
}

/// The `li` node of one entry.
#[must_use]
pub fn entry_node(entry: &LoadEntry) -> Node {
    let path = if entry.path.is_empty() {
        "/"
    } else {
        &entry.path
    };
    let mut b = NodeBuilder::new("li");
    if !entry.if_active.is_empty() {
        b = b.attr("IfModActive", entry.if_active.join(","));
    }
    if !entry.if_active_all.is_empty() {
        b = b.attr("IfModActiveAll", entry.if_active_all.join(","));
    }
    if !entry.if_not_active.is_empty() {
        b = b.attr("IfModNotActive", entry.if_not_active.join(","));
    }
    b.text(path).build()
}

fn block_node(key: &str, entries: &[LoadEntry]) -> Node {
    NodeBuilder::new(block_tag(key))
        .children(entries.iter().map(entry_node))
        .build()
}

/// The node tree of a whole specification (root `loadFolders`).
#[must_use]
pub fn to_node(spec: &LoadFoldersSpec) -> Node {
    NodeBuilder::new("loadFolders")
        .children(spec.blocks.iter().map(|b| block_node(&b.key, &b.entries)))
        .build()
}

/// Renders a new `LoadFolders.xml` from a specification.
#[must_use]
pub fn create(spec: &LoadFoldersSpec, opts: &RenderOpts) -> String {
    render(&to_node(spec), opts)
}

/// The path of the block with this key (compared after normalisation), if the file has one.
fn find_block(ed: &SpanEditor, key: &str) -> XmlResult<Option<String>> {
    let want = normalize_version_key(key);
    let root = format!("/{}", ed.root_tag());
    for child in ed.children(&root)? {
        if normalize_version_key(&child.tag) == want {
            return Ok(Some(child.path));
        }
    }
    Ok(None)
}

fn entry_of(info: &ElementInfo) -> LoadEntry {
    make_entry(&info.attrs, &info.text)
}

/// Adds `entry` to the end of the block `block_key`, creating the block when needed. Does nothing
/// when an equal entry (same folder and conditions, ignoring case of the ids) is already there.
///
/// # Errors
/// [`crate::XmlError`] when `text` is not a well formed document.
pub fn add_entry(text: &str, block_key: &str, entry: &LoadEntry) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    match find_block(&ed, block_key)? {
        Some(path) => {
            for child in ed.children(&path)? {
                if same_entry(&entry_of(&ed.element(&child.path)?), entry) {
                    return Ok(text.to_owned());
                }
            }
            ed.insert_child(&path, &entry_node(entry))?;
        }
        None => {
            let root = format!("/{}", ed.root_tag());
            let key = normalize_version_key(block_key);
            ed.insert_child(&root, &block_node(&key, std::slice::from_ref(entry)))?;
        }
    }
    Ok(ed.into_text())
}

/// Adds a gated folder: an entry for `folder` that loads only when one of `if_mod_active` is
/// active (`IfModActive`). See [`add_entry`].
///
/// # Errors
/// [`crate::XmlError`] when `text` is not a well formed document.
pub fn gate_folder(
    text: &str,
    block_key: &str,
    folder: &str,
    if_mod_active: &[&str],
) -> XmlResult<String> {
    let entry = LoadEntry::dir(folder).if_active(if_mod_active.iter().copied());
    add_entry(text, block_key, &entry)
}

/// Removes the entries of a block that equal `entry`. Does nothing when there are none.
///
/// # Errors
/// [`crate::XmlError`] when `text` is not a well formed document.
pub fn remove_entry(text: &str, block_key: &str, entry: &LoadEntry) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    let Some(path) = find_block(&ed, block_key)? else {
        return Ok(text.to_owned());
    };
    loop {
        let mut victim = None;
        for child in ed.children(&path)? {
            if same_entry(&entry_of(&ed.element(&child.path)?), entry) {
                victim = Some(child.path);
                break;
            }
        }
        match victim {
            Some(p) => {
                ed.remove(&p)?;
            }
            None => break,
        }
    }
    Ok(ed.into_text())
}

/// Parses a block key as `major.minor` numbers.
fn key_tuple(key: &str) -> Option<(u32, u32)> {
    let mut parts = key.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Makes sure the file has a block for `block_key`. A missing block is created as a copy of the
/// nearest lower version block, else of the `default` block, else from `fallback`.
///
/// # Errors
/// [`crate::XmlError`] when `text` is not a well formed document.
pub fn ensure_block(text: &str, block_key: &str, fallback: &[LoadEntry]) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    if find_block(&ed, block_key)?.is_some() {
        return Ok(text.to_owned());
    }
    let key = normalize_version_key(block_key);
    let wanted = key_tuple(&key);
    let root = format!("/{}", ed.root_tag());
    let mut best: Option<((u32, u32), String)> = None;
    let mut default_path = None;
    for child in ed.children(&root)? {
        let k = normalize_version_key(&child.tag);
        if k == "default" {
            default_path = Some(child.path.clone());
        }
        if let (Some(t), Some(w)) = (key_tuple(&k), wanted)
            && t <= w
            && best.as_ref().is_none_or(|(bt, _)| t > *bt)
        {
            best = Some((t, child.path.clone()));
        }
    }
    let source = best.map(|(_, p)| p).or(default_path);
    let entries: Vec<LoadEntry> = match source {
        Some(path) => {
            let mut out = Vec::new();
            for child in ed.children(&path)? {
                out.push(entry_of(&ed.element(&child.path)?));
            }
            out
        }
        None => fallback.to_vec(),
    };
    ed.insert_child(&root, &block_node(&key, &entries))?;
    Ok(ed.into_text())
}
