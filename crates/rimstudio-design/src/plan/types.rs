//! The write plan: pure data describing the files a design would produce.
//!
//! A plan carries JSON node trees, never XML text. `rimstudio-xml` renders the trees (the toolkit fills
//! [`PlannedFile::rendered`] afterwards) and the toolkit compares the rendering with what is on disk to
//! decide between [`FileAction::Create`], [`FileAction::UpdateRegion`] and [`FileAction::Unchanged`] and to
//! fill [`PlannedFile::diff`]. Paths are relative to the project root with `/` separators; a plan never
//! holds a path outside the root.
//!
//! # How a comment is represented
//!
//! The node tree has no comment node (the XML reader drops comments, so a tree could never hold one).
//! Section comment headers travel beside the tree: [`PlannedFile::sections`] lists
//! [`SectionHeader`]s, each naming the index of the first child of the root element that the comment
//! introduces. [`PlannedFile::section_groups`] turns that into consecutive groups of top level nodes, one per
//! header, which is exactly the shape of the renderer's `Section::with_comment(comment, nodes)` and
//! `render_list_file(root_tag, sections, opts)`. The comment text excludes the `<!--` and `-->` markers; the
//! renderer writes `<!-- {text} -->`.

use serde::{Deserialize, Serialize};

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::{Child, Node};

/// What happens to a file when the plan is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileAction {
    /// The file does not exist yet and is written whole.
    Create,
    /// The file exists and only the byte spans in `edits` change.
    UpdateRegion,
    /// The file exists and already holds exactly this content.
    Unchanged,
    /// The file exists with other content and is replaced whole (a copied asset; the old bytes are backed
    /// up first). Text files are never replaced whole, they use [`FileAction::UpdateRegion`].
    Replace,
}

/// What a planned file is for. Gating tests and the apply step use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FileKind {
    /// A vanilla definition file (`Defs/...`). Never holds a Combat Extended class.
    VanillaDefs,
    /// A Combat Extended patch file in the gated CE folder.
    CePatch,
    /// A Combat Extended definition file (`Defs` root) in the gated CE folder: the ammunition of a custom
    /// caliber. Never written when the Combat Extended switch is off.
    CeDefs,
    /// `LoadFolders.xml`.
    LoadFolders,
    /// `About/About.xml`.
    About,
    /// A binary asset (a texture or a sound clip) copied from a file on this machine. The plan holds the
    /// source and its hash, never the bytes.
    Copy,
}

/// A byte span replacement in an existing file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    /// First byte replaced.
    pub start: usize,
    /// One past the last byte replaced.
    pub end: usize,
    /// The replacement text.
    pub replacement: String,
}

/// A comment header placed before a child of the root element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionHeader {
    /// Index (among the children of the root element) of the first node the comment introduces.
    pub first_child: usize,
    /// The comment text without the comment markers, for example `====== RS_TestRifle ======`.
    pub comment: String,
}

impl SectionHeader {
    /// The conventional header `====== Name ======` placed before `first_child`.
    #[must_use]
    pub fn banner(first_child: usize, name: &str) -> Self {
        Self {
            first_child,
            comment: format!("====== {name} ======"),
        }
    }
}

/// A run of top level nodes with an optional comment header, in the order of the file.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionGroup {
    /// The comment above the group, if any.
    pub comment: Option<String>,
    /// The nodes of the group.
    pub nodes: Vec<Node>,
}

/// The source of a [`FileKind::Copy`] file: what the toolkit read from the file the user pointed at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyPlan {
    /// The source path as written in the spec (absolute, or relative to the project root).
    pub source: String,
    /// SHA-256 of the source, 64 lower case hexadecimal characters. Apply refuses a source that changed.
    pub sha256: String,
    /// Size of the source in bytes.
    pub bytes: u64,
    /// Width in pixels, for an image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height in pixels, for an image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// One file of a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    /// Path relative to the project root, with `/` separators.
    pub path: String,
    /// What the file is for.
    pub kind: FileKind,
    /// What applying the plan does to it.
    pub action: FileAction,
    /// The root element of the document (for example `Defs`), as a node tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<Node>,
    /// Comment headers for the children of the root element.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionHeader>,
    /// Byte span edits when the action is [`FileAction::UpdateRegion`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<TextEdit>,
    /// The rendered XML text, filled by the toolkit after `rimstudio-xml` renders the tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered: Option<String>,
    /// A unified diff against the file on disk, filled by the toolkit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// For a [`FileKind::Copy`] file: where the bytes come from. Such a file has no tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopyPlan>,
}

impl PlannedFile {
    /// A file to be created from a node tree with comment headers.
    #[must_use]
    pub fn new_file(
        path: impl Into<String>,
        kind: FileKind,
        tree: Node,
        sections: Vec<SectionHeader>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            action: FileAction::Create,
            tree: Some(tree),
            sections,
            edits: Vec::new(),
            rendered: None,
            diff: None,
            copy: None,
        }
    }

    /// A binary file to be copied from `copy.source` to `path` (a texture or a sound clip).
    #[must_use]
    pub fn copy_file(path: impl Into<String>, copy: CopyPlan) -> Self {
        Self {
            path: path.into(),
            kind: FileKind::Copy,
            action: FileAction::Create,
            tree: None,
            sections: Vec::new(),
            edits: Vec::new(),
            rendered: None,
            diff: None,
            copy: Some(copy),
        }
    }

    /// The top level nodes grouped by comment header, in file order. Children before the first header form
    /// a leading group without a comment. A header past the last child introduces nothing and is ignored.
    #[must_use]
    pub fn section_groups(&self) -> Vec<SectionGroup> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };
        let children: Vec<&Node> = tree.children.iter().filter_map(Child::as_element).collect();
        let mut headers: Vec<&SectionHeader> = self
            .sections
            .iter()
            .filter(|h| h.first_child < children.len())
            .collect();
        headers.sort_by_key(|h| h.first_child);
        let mut groups = Vec::new();
        let mut start = 0usize;
        let mut comment: Option<String> = None;
        for header in headers {
            if header.first_child > start || (header.first_child == start && comment.is_some()) {
                groups.push(SectionGroup {
                    comment: comment.take(),
                    nodes: children[start..header.first_child]
                        .iter()
                        .map(|n| (*n).clone())
                        .collect(),
                });
                start = header.first_child;
            }
            comment = Some(header.comment.clone());
        }
        if start < children.len() || comment.is_some() {
            groups.push(SectionGroup {
                comment,
                nodes: children[start..].iter().map(|n| (*n).clone()).collect(),
            });
        }
        groups
    }

    /// True when the tree, a section comment or an edit contains `needle` anywhere (tags, attributes, text).
    #[must_use]
    pub fn contains_text(&self, needle: &str) -> bool {
        self.sections.iter().any(|s| s.comment.contains(needle))
            || self.edits.iter().any(|e| e.replacement.contains(needle))
            || self.tree.as_ref().is_some_and(|t| node_contains(t, needle))
    }
}

fn node_contains(root: &Node, needle: &str) -> bool {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.tag.contains(needle)
            || n.attrs
                .iter()
                .any(|(k, v)| k.contains(needle) || v.contains(needle))
        {
            return true;
        }
        for child in &n.children {
            match child {
                Child::Text(t) if t.contains(needle) => return true,
                Child::Text(_) => {}
                Child::Element(e) => stack.push(e),
            }
        }
    }
    false
}

/// The files a design would write and the diagnostics found while planning them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlan {
    /// The files, sorted by path.
    pub files: Vec<PlannedFile>,
    /// Problems found while planning. A plan with an error diagnostic has no files (nothing may be written).
    pub diagnostics: Vec<Diagnostic>,
}

impl WritePlan {
    /// True when any diagnostic is an error; such a plan must not be applied.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// The file with the given path.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&PlannedFile> {
        self.files.iter().find(|f| f.path == path)
    }

    /// The paths of all files, in plan order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// True when any file contains `needle` (see [`PlannedFile::contains_text`]).
    #[must_use]
    pub fn contains_text(&self, needle: &str) -> bool {
        self.files.iter().any(|f| f.contains_text(needle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    fn defs(n: usize) -> Node {
        let mut b = NodeBuilder::new("Defs");
        for i in 0..n {
            b = b.elem("ThingDef", |t| t.text_elem("defName", format!("RS_{i}")));
        }
        b.build()
    }

    #[test]
    fn groups_follow_headers_and_keep_leading_nodes() {
        let mut f = PlannedFile::new_file("Defs/X.xml", FileKind::VanillaDefs, defs(4), vec![]);
        f.sections = vec![SectionHeader::banner(2, "B"), SectionHeader::banner(1, "A")];
        let groups = f.section_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].comment, None);
        assert_eq!(groups[0].nodes.len(), 1);
        assert_eq!(groups[1].comment.as_deref(), Some("====== A ======"));
        assert_eq!(groups[1].nodes.len(), 1);
        assert_eq!(groups[2].comment.as_deref(), Some("====== B ======"));
        assert_eq!(groups[2].nodes.len(), 2);
    }

    #[test]
    fn header_at_zero_gives_one_commented_group_and_late_headers_are_ignored() {
        let mut f = PlannedFile::new_file("Defs/X.xml", FileKind::VanillaDefs, defs(2), vec![]);
        f.sections = vec![
            SectionHeader::banner(0, "A"),
            SectionHeader::banner(9, "late"),
        ];
        let groups = f.section_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].comment.as_deref(), Some("====== A ======"));
        assert_eq!(groups[0].nodes.len(), 2);
        assert!(
            PlannedFile::new_file("p", FileKind::About, Node::new("Defs"), vec![])
                .section_groups()
                .is_empty()
        );
    }

    #[test]
    fn text_search_covers_tags_attributes_text_and_comments() {
        let tree = NodeBuilder::new("Defs")
            .elem("ThingDef", |t| {
                t.attr("Class", "Needle.Thing").text_elem("defName", "x")
            })
            .build();
        let mut f = PlannedFile::new_file("a.xml", FileKind::VanillaDefs, tree, vec![]);
        assert!(f.contains_text("Needle"));
        assert!(!f.contains_text("Haystack"));
        f.sections.push(SectionHeader::banner(0, "Haystack"));
        assert!(f.contains_text("Haystack"));
    }

    #[test]
    fn plan_serializes_with_kebab_enums_and_camel_fields() {
        let f = PlannedFile::new_file("Defs/X.xml", FileKind::VanillaDefs, defs(1), vec![]);
        let plan = WritePlan {
            files: vec![f],
            diagnostics: vec![],
        };
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json["files"][0]["action"], "create");
        assert_eq!(json["files"][0]["kind"], "vanilla-defs");
        assert!(json["files"][0].get("rendered").is_none());
        let back: WritePlan = serde_json::from_value(json).unwrap();
        assert_eq!(back, plan);
    }
}
