//! Block and entry edits of an existing `LoadFolders.xml`, addressed by position.
//!
//! [`crate::load_folders`] reads the file and has the value based helpers (add, remove and gate an entry).
//! A form editor needs the positional ones: blocks and entries are addressed by their place in the file (the
//! reader merges repeated blocks, so a block's name is not an address), an entry can be moved or have its
//! conditions changed in place, and a block can be added or removed as a whole. Everything is a byte splice:
//! comments, order, odd white space, the line ending and a byte order mark of the file stay as they were.
//!
//! A block with no entries loads nothing in the game, so removing the last entry of a block leaves an empty
//! block and it is the caller's call whether to remove the block as well.

use rimstudio_core::load_plan::LoadEntry;
use rimstudio_core::tree::Node;

use crate::about_edit::swap_elements;
use crate::edit::SpanEditor;
use crate::error::{XmlError, XmlResult};
use crate::load_folders::{block_tag, entry_node};

/// What to change in one entry. A member that is `None` is left as it is; an empty list removes the
/// attribute.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryPatch {
    /// New folder text (`""` is written as `/`).
    pub path: Option<String>,
    /// New `IfModActive` ids.
    pub if_active: Option<Vec<String>>,
    /// New `IfModActiveAll` ids.
    pub if_active_all: Option<Vec<String>>,
    /// New `IfModNotActive` ids.
    pub if_not_active: Option<Vec<String>>,
    /// Remove the attributes the game does not read (for example `IfModActiveAny`).
    pub drop_ignored_attributes: bool,
}

/// A `LoadFolders.xml` text under edit.
#[derive(Debug, Clone)]
pub struct LoadFoldersEditor {
    text: String,
}

fn missing(path: impl Into<String>) -> XmlError {
    XmlError::EditPathMissing { path: path.into() }
}

const KNOWN_ATTRS: [&str; 3] = ["IfModActive", "IfModActiveAll", "IfModNotActive"];

impl LoadFoldersEditor {
    /// Starts editing a document.
    ///
    /// # Errors
    /// [`XmlError`] when the text is not a well formed document.
    pub fn new(text: impl Into<String>) -> XmlResult<Self> {
        let text = text.into();
        SpanEditor::open(text.as_str())?;
        Ok(Self { text })
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

    fn block_path(ed: &SpanEditor, block: usize) -> XmlResult<String> {
        let root = format!("/{}", ed.root_tag());
        ed.children(&root)?
            .into_iter()
            .nth(block)
            .map(|c| c.path)
            .ok_or_else(|| missing(format!("{root}/*[{}]", block + 1)))
    }

    fn entry_path(ed: &SpanEditor, block: usize, entry: usize) -> XmlResult<String> {
        let block_path = Self::block_path(ed, block)?;
        ed.children(&block_path)?
            .into_iter()
            .nth(entry)
            .map(|c| c.path)
            .ok_or_else(|| missing(format!("{block_path}/*[{}]", entry + 1)))
    }

    /// Adds a block for the version `key` (`1.6`, `default`) at position `at` (the end when `None` or past
    /// the end) holding `entries`.
    ///
    /// # Errors
    /// [`XmlError`] for a key that is not an XML name or an unusable document.
    pub fn add_block(
        &mut self,
        key: &str,
        entries: &[LoadEntry],
        at: Option<usize>,
    ) -> XmlResult<()> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let root = format!("/{}", ed.root_tag());
        let node = Node {
            tag: block_tag(key.trim()),
            attrs: Vec::new(),
            children: entries
                .iter()
                .map(|e| rimstudio_core::tree::Child::Element(entry_node(e)))
                .collect(),
        };
        match at {
            Some(i) => ed.insert_child_at(&root, i, &node)?,
            None => ed.insert_child(&root, &node)?,
        };
        self.text = ed.into_text();
        Ok(())
    }

    /// Removes the block at position `block` with everything in it.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such block.
    pub fn remove_block(&mut self, block: usize) -> XmlResult<()> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let path = Self::block_path(&ed, block)?;
        ed.remove(&path)?;
        self.text = ed.into_text();
        Ok(())
    }

    /// Adds an entry to the block at position `block`, at position `at` (the end when `None` or past the
    /// end).
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such block.
    pub fn add_entry(
        &mut self,
        block: usize,
        entry: &LoadEntry,
        at: Option<usize>,
    ) -> XmlResult<()> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let path = Self::block_path(&ed, block)?;
        let node = entry_node(entry);
        match at {
            Some(i) => ed.insert_child_at(&path, i, &node)?,
            None => ed.insert_child(&path, &node)?,
        };
        self.text = ed.into_text();
        Ok(())
    }

    /// Removes one entry.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such block or entry.
    pub fn remove_entry(&mut self, block: usize, entry: usize) -> XmlResult<()> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let path = Self::entry_path(&ed, block, entry)?;
        ed.remove(&path)?;
        self.text = ed.into_text();
        Ok(())
    }

    /// Moves an entry inside its block to position `to` (clamped).
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such block or entry.
    pub fn move_entry(&mut self, block: usize, from: usize, to: usize) -> XmlResult<()> {
        let mut at = from;
        loop {
            let ed = SpanEditor::open(self.text.as_str())?;
            let block_path = Self::block_path(&ed, block)?;
            let kids = ed.children(&block_path)?;
            let target = to.min(kids.len().saturating_sub(1));
            if at == target {
                return Ok(());
            }
            let next = if at < target { at + 1 } else { at - 1 };
            let (Some(a), Some(b)) = (kids.get(at), kids.get(next)) else {
                return Err(missing(format!("{block_path}/*[{}]", at + 1)));
            };
            self.text = swap_elements(&self.text, &a.path, &b.path)?;
            at = next;
        }
    }

    /// Changes the folder text and the conditions of one entry in place.
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such block or entry.
    pub fn set_entry(&mut self, block: usize, entry: usize, patch: &EntryPatch) -> XmlResult<()> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let path = Self::entry_path(&ed, block, entry)?;
        if let Some(p) = &patch.path {
            let text = if p.trim().is_empty() { "/" } else { p.as_str() };
            if ed.element(&path)?.child_count > 0 {
                return Err(XmlError::EditUnsupported {
                    path,
                    reason: "the entry holds child elements".to_owned(),
                });
            }
            if ed.element_text(&path)? != text {
                ed.replace_text(&path, text)?;
            }
        }
        for (name, ids) in [
            ("IfModActive", &patch.if_active),
            ("IfModActiveAll", &patch.if_active_all),
            ("IfModNotActive", &patch.if_not_active),
        ] {
            let Some(ids) = ids else { continue };
            let ids: Vec<&str> = ids
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if ids.is_empty() {
                ed.remove_attr(&path, name)?;
            } else {
                let value = ids.join(",");
                if ed.attr(&path, name)?.as_deref() != Some(value.as_str()) {
                    ed.set_attr(&path, name, &value)?;
                }
            }
        }
        if patch.drop_ignored_attributes {
            let attrs = ed.element(&path)?.attrs;
            for (name, _) in attrs {
                if !KNOWN_ATTRS.contains(&name.as_str()) {
                    ed.remove_attr(&path, &name)?;
                }
            }
        }
        self.text = ed.into_text();
        Ok(())
    }
}
