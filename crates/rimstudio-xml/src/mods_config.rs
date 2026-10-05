//! `Config/ModsConfig.xml`: the game's active mod list.
//!
//! Reading yields the stored game version, the ordered active package ids and the known expansion
//! ids. Writing replaces only those parts of an existing file by byte splice, so unknown elements
//! the game or other tools added, comments and the file's line endings survive.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::{Node, NodeBuilder};

use crate::about::normalise_encoding;
use crate::edit::SpanEditor;
use crate::error::XmlResult;
use crate::modes::ParseMode;
use crate::reader::parse_document;
use crate::render::{RenderOpts, render};

/// The parts of `ModsConfig.xml` RimStudio reads and writes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModsConfigData {
    /// The `version` element (alias `buildNumber`), for example `1.6.4871 rev598`.
    pub version: Option<String>,
    /// The `activeMods` ids in load order, as written (lower case in files the game wrote).
    pub active_mods: Vec<String>,
    /// The `knownExpansions` ids. `None` means the element is absent (read) or left alone (write).
    pub known_expansions: Option<Vec<String>>,
}

/// The result of [`read`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModsConfigRead {
    /// What the file holds.
    pub data: ModsConfigData,
    /// Warnings found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

fn id_list(node: &Node) -> Vec<String> {
    node.elements()
        .map(|li| li.text_content().trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Reads a `ModsConfig.xml`. Ids are trimmed and empty entries skipped.
///
/// # Errors
/// [`crate::XmlError`] when the file has no root element at all.
pub fn read(bytes: &[u8]) -> XmlResult<ModsConfigRead> {
    let data = normalise_encoding(bytes);
    let doc = parse_document(&data, ParseMode::Tolerant)?;
    let version = doc
        .root
        .child("version")
        .or_else(|| doc.root.child("buildNumber"))
        .map(|n| n.text_content().trim().to_owned());
    let active_mods = doc
        .root
        .child("activeMods")
        .map(id_list)
        .unwrap_or_default();
    let known_expansions = doc.root.child("knownExpansions").map(id_list);
    Ok(ModsConfigRead {
        data: ModsConfigData {
            version,
            active_mods,
            known_expansions,
        },
        diagnostics: doc.diagnostics,
    })
}

fn li_nodes(ids: &[String]) -> Vec<Node> {
    ids.iter()
        .map(|i| Node::with_text("li", i.clone()))
        .collect()
}

fn list_node(tag: &str, ids: &[String]) -> Node {
    NodeBuilder::new(tag).children(li_nodes(ids)).build()
}

/// The node tree of a new file (root `ModsConfigData`).
#[must_use]
pub fn to_node(data: &ModsConfigData) -> Node {
    NodeBuilder::new("ModsConfigData")
        .text_elem_opt("version", data.version.clone())
        .child(list_node("activeMods", &data.active_mods))
        .when(data.known_expansions.is_some(), |b| {
            b.child(list_node(
                "knownExpansions",
                data.known_expansions.as_deref().unwrap_or(&[]),
            ))
        })
        .build()
}

/// Renders a new `ModsConfig.xml`.
#[must_use]
pub fn create(data: &ModsConfigData, opts: &RenderOpts) -> String {
    render(&to_node(data), opts)
}

/// Writes `data` into an existing `ModsConfig.xml` text: the `activeMods` list is replaced, the
/// version is set when `data.version` is given, `knownExpansions` is replaced when it is `Some`.
/// Elements that are missing are added; everything else in the file stays byte for byte.
///
/// # Errors
/// [`crate::XmlError`] when `original` is not a well formed document.
pub fn write_active(original: &str, data: &ModsConfigData) -> XmlResult<String> {
    let mut ed = SpanEditor::open(original)?;
    let root = format!("/{}", ed.root_tag());

    if let Some(version) = &data.version {
        let alias = if ed.exists(&format!("{root}/version")) {
            Some("version")
        } else if ed.exists(&format!("{root}/buildNumber")) {
            Some("buildNumber")
        } else {
            None
        };
        match alias {
            Some(name) => {
                ed.replace_text(&format!("{root}/{name}"), version)?;
            }
            None => {
                ed.insert_child_at(&root, 0, &Node::with_text("version", version.clone()))?;
            }
        }
    }

    set_list(&mut ed, &root, "activeMods", &data.active_mods)?;
    if let Some(known) = &data.known_expansions {
        set_list(&mut ed, &root, "knownExpansions", known)?;
    }
    Ok(ed.into_text())
}

fn set_list(ed: &mut SpanEditor, root: &str, tag: &str, ids: &[String]) -> XmlResult<()> {
    let path = format!("{root}/{tag}");
    if ed.exists(&path) {
        ed.replace_children(&path, &li_nodes(ids))?;
    } else {
        ed.insert_child(root, &list_node(tag, ids))?;
    }
    Ok(())
}
