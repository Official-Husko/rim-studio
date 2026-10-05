//! Whole field edits of an existing `About.xml`, as byte splices.
//!
//! [`crate::about`] has the single step helpers (set one text field, replace one list). This module adds
//! what a form editor needs on top of them: a field is removed when it is cleared, a missing field is
//! inserted at the conventional place (see [`ROOT_ORDER`]), a list entry is added, removed or moved without
//! touching its neighbours, and a dependency is changed field by field. Comments, element order, unknown
//! elements and every field that is not edited stay byte for byte as they were; the line ending, the
//! indentation unit and a byte order mark of the file decide how new text is laid out.
//!
//! [`AboutEditor`] holds the text and applies one edit at a time. Every edit re-indexes the result, so an edit
//! that would break the document returns an error and leaves the text as it was.

use rimstudio_core::mods::ModDependency;
use rimstudio_core::tree::{Child, Node};
use rimstudio_core::version::normalize_version_key;

use crate::edit::SpanEditor;
use crate::error::{XmlError, XmlResult};

/// The conventional order of the children of the `ModMetaData` root. A field that the file lacks is
/// inserted after the nearest field that comes before it in this list and that the file has, else before
/// the nearest one that comes after it, else at the end.
pub const ROOT_ORDER: &[&str] = &[
    "name",
    "shortName",
    "author",
    "authors",
    "packageId",
    "url",
    "modVersion",
    "steamAppId",
    "supportedVersions",
    "modDependencies",
    "modDependenciesByVersion",
    "loadBefore",
    "loadBeforeByVersion",
    "loadAfter",
    "loadAfterByVersion",
    "forceLoadBefore",
    "forceLoadAfter",
    "incompatibleWith",
    "incompatibleWithByVersion",
    "modIconPath",
    "descriptionsByVersion",
    "description",
];

/// The conventional order of the children of one dependency.
pub const DEPENDENCY_ORDER: &[&str] = &[
    "packageId",
    "displayName",
    "steamWorkshopUrl",
    "downloadUrl",
    "alternativePackageIds",
];

/// What to change in one dependency. A member that is `None` is left as it is; `Some("")` removes an
/// optional element (the package id and the display name are kept as empty elements instead).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependencyPatch {
    /// New `packageId`.
    pub package_id: Option<String>,
    /// New `displayName`.
    pub display_name: Option<String>,
    /// New `steamWorkshopUrl`; empty removes it.
    pub steam_workshop_url: Option<String>,
    /// New `downloadUrl`; empty removes it.
    pub download_url: Option<String>,
    /// New `alternativePackageIds`; an empty list removes the element.
    pub alternatives: Option<Vec<String>>,
}

/// An `About.xml` text under edit.
#[derive(Debug, Clone)]
pub struct AboutEditor {
    text: String,
}

fn missing(path: impl Into<String>) -> XmlError {
    XmlError::EditPathMissing { path: path.into() }
}

fn rank_of(order: &[&str], tag: &str) -> Option<usize> {
    order.iter().position(|t| *t == tag)
}

fn li(text: &str) -> Node {
    Node::with_text("li", text)
}

fn list_node(tag: &str, items: &[String]) -> Node {
    Node {
        tag: tag.to_owned(),
        attrs: Vec::new(),
        children: items.iter().map(|i| Child::Element(li(i))).collect(),
    }
}

/// Inserts `node` below `parent` at the place `order` suggests.
fn insert_ordered(ed: &mut SpanEditor, parent: &str, node: &Node, order: &[&str]) -> XmlResult<()> {
    let Some(rank) = rank_of(order, &node.tag) else {
        ed.insert_child(parent, node)?;
        return Ok(());
    };
    let kids = ed.children(parent)?;
    // after the last known element (in file order) that conventionally comes before the new one, so a file
    // whose order differs from the convention is not reshuffled; else before the first one that follows
    let mut before: Option<String> = None;
    let mut after: Option<String> = None;
    for k in &kids {
        let Some(r) = rank_of(order, &k.tag) else {
            continue;
        };
        if r <= rank {
            before = Some(k.path.clone());
        } else if after.is_none() {
            after = Some(k.path.clone());
        }
    }
    match (before, after) {
        (Some(path), _) => {
            ed.insert_after(&path, node)?;
        }
        (None, Some(path)) => {
            ed.insert_before(&path, node)?;
        }
        (None, None) => {
            ed.insert_child(parent, node)?;
        }
    }
    Ok(())
}

/// Removes every child of `parent` named `tag` (the game reads the first one, so a duplicate left behind
/// would take over).
fn remove_all(ed: &mut SpanEditor, parent: &str, tag: &str) -> XmlResult<bool> {
    let mut removed = false;
    loop {
        let path = format!("{parent}/{tag}");
        if !ed.exists(&path) {
            return Ok(removed);
        }
        ed.remove(&path)?;
        removed = true;
    }
}

fn set_child_text(
    ed: &mut SpanEditor,
    parent: &str,
    tag: &str,
    value: &str,
    order: &[&str],
) -> XmlResult<()> {
    let path = format!("{parent}/{tag}");
    if ed.exists(&path) {
        if ed.element(&path)?.child_count > 0 {
            ed.replace_element(&path, &Node::with_text(tag, value))?;
        } else if ed.element_text(&path)?.replace("\r\n", "\n") != value.replace("\r\n", "\n") {
            ed.replace_text(&path, value)?;
        }
    } else {
        insert_ordered(ed, parent, &Node::with_text(tag, value), order)?;
    }
    Ok(())
}

fn set_child_list(
    ed: &mut SpanEditor,
    parent: &str,
    tag: &str,
    items: &[String],
    order: &[&str],
) -> XmlResult<()> {
    let path = format!("{parent}/{tag}");
    if items.is_empty() {
        // an emptied list goes away; a list that was already empty is left exactly as the file had it
        if ed.exists(&path) && !list_values(ed, &path)?.is_empty() {
            remove_all(ed, parent, tag)?;
        }
        return Ok(());
    }
    if ed.exists(&path) {
        if list_values(ed, &path)?
            .iter()
            .map(|(_, v)| v)
            .eq(items.iter())
        {
            return Ok(());
        }
        let nodes: Vec<Node> = items.iter().map(|i| li(i)).collect();
        ed.replace_children(&path, &nodes)?;
    } else {
        insert_ordered(ed, parent, &list_node(tag, items), order)?;
    }
    Ok(())
}

/// The entries of a list element as the reader sees them: `li` children with some text, trimmed.
/// Returns the path of each entry and its text.
fn list_values(ed: &SpanEditor, list_path: &str) -> XmlResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for k in ed.children(list_path)? {
        if k.tag != "li" {
            continue;
        }
        let info = ed.element(&k.path)?;
        let text = info.text.trim().to_owned();
        if info.child_count == 0 && !text.is_empty() {
            out.push((k.path, text));
        }
    }
    Ok(out)
}

/// The dependencies as the reader sees them: `li` children with a non-empty `packageId`. Returns the path of
/// each entry and its package id.
fn dependency_entries(ed: &SpanEditor, list_path: &str) -> XmlResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for k in ed.children(list_path)? {
        if k.tag != "li" {
            continue;
        }
        let id_path = format!("{}/packageId", k.path);
        if !ed.exists(&id_path) {
            continue;
        }
        let id = ed.element_text(&id_path)?.trim().to_owned();
        if !id.is_empty() {
            out.push((k.path, id));
        }
    }
    Ok(out)
}

fn dependency_node(dep: &ModDependency) -> Node {
    let mut node = Node {
        tag: "li".to_owned(),
        attrs: Vec::new(),
        children: vec![Child::Element(Node::with_text(
            "packageId",
            &dep.package_id,
        ))],
    };
    node.children.push(Child::Element(Node::with_text(
        "displayName",
        &dep.display_name,
    )));
    if let Some(u) = dep.steam_workshop_url.as_deref().filter(|u| !u.is_empty()) {
        node.children
            .push(Child::Element(Node::with_text("steamWorkshopUrl", u)));
    }
    if let Some(u) = dep.download_url.as_deref().filter(|u| !u.is_empty()) {
        node.children
            .push(Child::Element(Node::with_text("downloadUrl", u)));
    }
    if !dep.alternatives.is_empty() {
        node.children.push(Child::Element(list_node(
            "alternativePackageIds",
            &dep.alternatives,
        )));
    }
    node
}

/// Swaps two sibling elements in the text: each moves to the other's place, everything between them (white
/// space, comments, other elements) stays.
pub(crate) fn swap_elements(text: &str, a_path: &str, b_path: &str) -> XmlResult<String> {
    let ed = SpanEditor::open(text)?;
    let a = ed.element(a_path)?.span;
    let b = ed.element(b_path)?.span;
    let (first, second) = if a.start <= b.start { (a, b) } else { (b, a) };
    let (Some(head), Some(first_t), Some(mid), Some(second_t), Some(tail)) = (
        text.get(..first.start),
        text.get(first.clone()),
        text.get(first.end..second.start),
        text.get(second.clone()),
        text.get(second.end..),
    ) else {
        return Err(missing(a_path));
    };
    Ok(format!("{head}{second_t}{mid}{first_t}{tail}"))
}

impl AboutEditor {
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

    fn with<R>(&mut self, f: impl FnOnce(&mut SpanEditor, &str) -> XmlResult<R>) -> XmlResult<R> {
        let mut ed = SpanEditor::open(self.text.as_str())?;
        let root = format!("/{}", ed.root_tag());
        let out = f(&mut ed, &root)?;
        self.text = ed.into_text();
        Ok(out)
    }

    /// Sets a text field of the root. The element is updated in place or inserted at its conventional place.
    /// An empty value (after the caller trimmed it) removes the element instead, see [`AboutEditor::clear`].
    ///
    /// # Errors
    /// [`XmlError`] for a name that is not an XML name or an unusable document.
    pub fn set_text(&mut self, field: &str, value: &str) -> XmlResult<()> {
        if value.trim().is_empty() {
            return self.clear(field);
        }
        self.with(|ed, root| set_child_text(ed, root, field, value, ROOT_ORDER))
    }

    /// Removes a field of the root, all copies of it. Does nothing when the file has none.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn clear(&mut self, field: &str) -> XmlResult<()> {
        self.with(|ed, root| remove_all(ed, root, field).map(|_| ()))
    }

    /// Replaces a list field with `items`. An empty list removes the element, unless the file's list was
    /// already empty (it is then left alone). A list that already holds exactly `items` is not touched.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn set_list(&mut self, field: &str, items: &[String]) -> XmlResult<()> {
        self.with(|ed, root| set_child_list(ed, root, field, items, ROOT_ORDER))
    }

    /// Adds `value` to a list field at position `at` (end when `None` or past the end). The list is created
    /// when missing. An entry that is already there (same text, ignoring case) is not added twice.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn list_add(&mut self, field: &str, value: &str, at: Option<usize>) -> XmlResult<()> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(());
        }
        self.with(|ed, root| {
            let path = format!("{root}/{field}");
            if !ed.exists(&path) {
                return insert_ordered(
                    ed,
                    root,
                    &list_node(field, &[value.to_owned()]),
                    ROOT_ORDER,
                );
            }
            let entries = list_values(ed, &path)?;
            if entries.iter().any(|(_, v)| v.eq_ignore_ascii_case(value)) {
                return Ok(());
            }
            match at.and_then(|i| entries.get(i)) {
                Some((target, _)) => ed.insert_before(target, &li(value)).map(|_| ()),
                None => ed.insert_child(&path, &li(value)).map(|_| ()),
            }
        })
    }

    /// Removes every entry of a list field that equals `value` (ignoring case and white space). The element
    /// goes away when that was its last entry. Does nothing when the entry is not there.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn list_remove(&mut self, field: &str, value: &str) -> XmlResult<()> {
        let value = value.trim().to_owned();
        self.with(|ed, root| {
            let path = format!("{root}/{field}");
            if !ed.exists(&path) {
                return Ok(());
            }
            loop {
                let entries = list_values(ed, &path)?;
                let Some((victim, _)) =
                    entries.iter().find(|(_, v)| v.eq_ignore_ascii_case(&value))
                else {
                    break;
                };
                let last = entries.len() == 1;
                if last {
                    remove_all(ed, root, field)?;
                    break;
                }
                ed.remove(victim)?;
            }
            Ok(())
        })
    }

    /// Moves the entry `value` of a list field to position `to` (zero based, clamped).
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when the entry is not in the list, or an unusable document.
    pub fn list_move(&mut self, field: &str, value: &str, to: usize) -> XmlResult<()> {
        let value = value.trim().to_owned();
        let (root, entries) = {
            let ed = SpanEditor::open(self.text.as_str())?;
            let root = format!("/{}", ed.root_tag());
            let path = format!("{root}/{field}");
            if !ed.exists(&path) {
                return Err(missing(path));
            }
            (root, list_values(&ed, &path)?)
        };
        let from = entries
            .iter()
            .position(|(_, v)| v.eq_ignore_ascii_case(&value))
            .ok_or_else(|| missing(format!("{root}/{field}/li[{value}]")))?;
        self.move_entry(&root, field, from, to, false)
    }

    fn move_entry(
        &mut self,
        root: &str,
        field: &str,
        from: usize,
        to: usize,
        dependencies: bool,
    ) -> XmlResult<()> {
        let mut at = from;
        let list_path = format!("{root}/{field}");
        let target = to;
        loop {
            let ed = SpanEditor::open(self.text.as_str())?;
            let entries = if dependencies {
                dependency_entries(&ed, &list_path)?
            } else {
                list_values(&ed, &list_path)?
            };
            let target = target.min(entries.len().saturating_sub(1));
            if at == target {
                return Ok(());
            }
            let next = if at < target { at + 1 } else { at - 1 };
            let (Some((a, _)), Some((b, _))) = (entries.get(at), entries.get(next)) else {
                return Err(missing(list_path));
            };
            self.text = swap_elements(&self.text, a, b)?;
            at = next;
        }
    }

    /// Adds a dependency at position `at` (end when `None`). A dependency whose package id is already listed
    /// (ignoring case) is not added twice.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn dependency_add(&mut self, dep: &ModDependency, at: Option<usize>) -> XmlResult<()> {
        let node = dependency_node(dep);
        let wanted = dep.package_id.trim().to_owned();
        self.with(|ed, root| {
            let path = format!("{root}/modDependencies");
            if !ed.exists(&path) {
                let list = Node {
                    tag: "modDependencies".to_owned(),
                    attrs: Vec::new(),
                    children: vec![Child::Element(node.clone())],
                };
                return insert_ordered(ed, root, &list, ROOT_ORDER);
            }
            let entries = dependency_entries(ed, &path)?;
            if entries
                .iter()
                .any(|(_, id)| id.eq_ignore_ascii_case(&wanted))
            {
                return Ok(());
            }
            match at.and_then(|i| entries.get(i)) {
                Some((target, _)) => ed.insert_before(target, &node).map(|_| ()),
                None => ed.insert_child(&path, &node).map(|_| ()),
            }
        })
    }

    /// Removes the dependencies with this package id (ignoring case). The list element goes away when it
    /// held nothing else. Does nothing when there is none.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn dependency_remove(&mut self, package_id: &str) -> XmlResult<()> {
        let wanted = package_id.trim().to_owned();
        self.with(|ed, root| {
            let path = format!("{root}/modDependencies");
            if !ed.exists(&path) {
                return Ok(());
            }
            loop {
                let entries = dependency_entries(ed, &path)?;
                let Some((victim, _)) = entries
                    .iter()
                    .find(|(_, id)| id.eq_ignore_ascii_case(&wanted))
                else {
                    break;
                };
                if entries.len() == 1 && ed.children(&path)?.len() == 1 {
                    remove_all(ed, root, "modDependencies")?;
                    break;
                }
                ed.remove(victim)?;
            }
            Ok(())
        })
    }

    /// Moves the dependency with this package id to position `to` (zero based, clamped).
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such dependency.
    pub fn dependency_move(&mut self, package_id: &str, to: usize) -> XmlResult<()> {
        let wanted = package_id.trim().to_owned();
        let (root, entries) = {
            let ed = SpanEditor::open(self.text.as_str())?;
            let root = format!("/{}", ed.root_tag());
            let path = format!("{root}/modDependencies");
            if !ed.exists(&path) {
                return Err(missing(path));
            }
            (root.clone(), dependency_entries(&ed, &path)?)
        };
        let from = entries
            .iter()
            .position(|(_, id)| id.eq_ignore_ascii_case(&wanted))
            .ok_or_else(|| missing(format!("{root}/modDependencies/li[{wanted}]")))?;
        self.move_entry(&root, "modDependencies", from, to, true)
    }

    /// Changes fields of the dependency with this package id (the first one when it is listed twice).
    ///
    /// # Errors
    /// [`XmlError::EditPathMissing`] when there is no such dependency.
    pub fn dependency_update(
        &mut self,
        package_id: &str,
        patch: &DependencyPatch,
    ) -> XmlResult<()> {
        let wanted = package_id.trim().to_owned();
        self.with(|ed, root| {
            let path = format!("{root}/modDependencies");
            if !ed.exists(&path) {
                return Err(missing(path));
            }
            let entries = dependency_entries(ed, &path)?;
            let Some((li_path, _)) = entries
                .iter()
                .find(|(_, id)| id.eq_ignore_ascii_case(&wanted))
                .cloned()
            else {
                return Err(missing(format!("{path}/li[{wanted}]")));
            };
            if let Some(v) = &patch.package_id {
                set_child_text(ed, &li_path, "packageId", v.trim(), DEPENDENCY_ORDER)?;
            }
            if let Some(v) = &patch.display_name {
                set_child_text(ed, &li_path, "displayName", v.trim(), DEPENDENCY_ORDER)?;
            }
            for (tag, value) in [
                ("steamWorkshopUrl", &patch.steam_workshop_url),
                ("downloadUrl", &patch.download_url),
            ] {
                let Some(v) = value else { continue };
                if v.trim().is_empty() {
                    remove_all(ed, &li_path, tag)?;
                } else {
                    set_child_text(ed, &li_path, tag, v.trim(), DEPENDENCY_ORDER)?;
                }
            }
            if let Some(alts) = &patch.alternatives {
                set_child_list(
                    ed,
                    &li_path,
                    "alternativePackageIds",
                    alts,
                    DEPENDENCY_ORDER,
                )?;
            }
            Ok(())
        })
    }

    /// Sets one version block of a `ByVersion` list field, for example `loadAfterByVersion` and `1.6`. The
    /// block keeps the tag the file already uses for that version (`v1.6` or `1.6`); a new one is written as
    /// `v1.6`. An empty list removes the block, and the field element when it was the last block.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn by_version_set_list(
        &mut self,
        field: &str,
        version: &str,
        items: &[String],
    ) -> XmlResult<()> {
        let key = normalize_version_key(version);
        self.with(|ed, root| {
            let path = format!("{root}/{field}");
            let existing = block_path(ed, &path, &key)?;
            match (existing, items.is_empty()) {
                (Some((block, _)), false) => {
                    let nodes: Vec<Node> = items.iter().map(|i| li(i)).collect();
                    ed.replace_children(&block, &nodes)?;
                }
                (Some((block, _)), true) => {
                    if ed.children(&path)?.len() == 1 {
                        remove_all(ed, root, field)?;
                    } else {
                        ed.remove(&block)?;
                    }
                }
                (None, true) => {}
                (None, false) => {
                    let block = list_node(&format!("v{key}"), items);
                    if ed.exists(&path) {
                        ed.insert_child(&path, &block)?;
                    } else {
                        let outer = Node {
                            tag: field.to_owned(),
                            attrs: Vec::new(),
                            children: vec![Child::Element(block)],
                        };
                        insert_ordered(ed, root, &outer, ROOT_ORDER)?;
                    }
                }
            }
            Ok(())
        })
    }

    /// Sets (or with `None` removes) the description of one game version in `descriptionsByVersion`.
    ///
    /// # Errors
    /// [`XmlError`] for an unusable document.
    pub fn by_version_set_description(
        &mut self,
        version: &str,
        text: Option<&str>,
    ) -> XmlResult<()> {
        let key = normalize_version_key(version);
        self.with(|ed, root| {
            let path = format!("{root}/descriptionsByVersion");
            let existing = block_path(ed, &path, &key)?;
            match (existing, text.filter(|t| !t.trim().is_empty())) {
                (Some((block, tag)), Some(t)) => {
                    if ed.element(&block)?.child_count > 0 {
                        ed.replace_element(&block, &Node::with_text(&tag, t))?;
                    } else {
                        ed.replace_text(&block, t)?;
                    }
                }
                (Some((block, _)), None) => {
                    if ed.children(&path)?.len() == 1 {
                        remove_all(ed, root, "descriptionsByVersion")?;
                    } else {
                        ed.remove(&block)?;
                    }
                }
                (None, None) => {}
                (None, Some(t)) => {
                    let node = Node::with_text(format!("v{key}"), t);
                    if ed.exists(&path) {
                        ed.insert_child(&path, &node)?;
                    } else {
                        let outer = Node {
                            tag: "descriptionsByVersion".to_owned(),
                            attrs: Vec::new(),
                            children: vec![Child::Element(node)],
                        };
                        insert_ordered(ed, root, &outer, ROOT_ORDER)?;
                    }
                }
            }
            Ok(())
        })
    }
}

/// The path and tag of the block of a `ByVersion` element whose key equals `key` (first one wins).
fn block_path(ed: &SpanEditor, path: &str, key: &str) -> XmlResult<Option<(String, String)>> {
    if !ed.exists(path) {
        return Ok(None);
    }
    for k in ed.children(path)? {
        if normalize_version_key(&k.tag) == key {
            return Ok(Some((k.path, k.tag)));
        }
    }
    Ok(None)
}
