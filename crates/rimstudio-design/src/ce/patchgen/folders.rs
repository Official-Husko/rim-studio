//! `LoadFolders.xml` as a node tree: the gated Combat Extended folder.
//!
//! Combat Extended classes may only live in a folder that the game loads when Combat Extended is active.
//! The generator creates `LoadFolders.xml` when the project has none (the root folder and the CE folder under
//! the block of the current game version) and edits an existing one: the CE folder entry is added to the
//! block of the game version, the block is created from the previous block when it is missing (the game uses
//! the exact version block, else the highest older one, else `default`), and a `default` block is kept in
//! step when the target is the newest block. Entries and attributes the file already has are kept as they
//! are.
//!
//! The gate is the lower case package id of the installed Combat Extended; the mod name (`ceName`) has no
//! part here. Existing entries whose id carries a platform suffix are reported (CEP019) and never copied.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::{Child, Node};

use crate::ce::lint::codes::CEP019;
use crate::plan::{FileAction, FileKind, PlannedFile, ProjectLayout};
use crate::validation::codes::VALUE_INVALID;

/// The attribute that gates an entry on active mods.
pub const IF_MOD_ACTIVE: &str = "IfModActive";
/// The element name of the fallback block.
pub const DEFAULT_BLOCK: &str = "default";
/// The suffixes that make an `IfModActive` id unable to match.
pub const ID_SUFFIXES: [&str; 2] = ["_copy", "_steam"];

/// The planned `LoadFolders.xml` file (when anything changes) and the diagnostics found on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadFoldersOutcome {
    /// The file: [`FileAction::Create`] when the project had none, [`FileAction::UpdateRegion`] (with the
    /// complete new tree and no edits, the toolkit computes the spans) when an existing file changes,
    /// [`FileAction::Unchanged`] when it already holds the gate. `None` when the existing file cannot be
    /// edited (see the diagnostics).
    pub file: Option<PlannedFile>,
    /// Problems and hints.
    pub diagnostics: Vec<Diagnostic>,
}

/// Normalises a folder entry for comparison: slashes trimmed, `/` stays the root.
pub(crate) fn normal(entry: &str) -> String {
    let t = entry.trim().trim_matches('/');
    if t.is_empty() {
        "/".to_owned()
    } else {
        t.to_owned()
    }
}

/// The ids of an `IfModActive` style attribute value, trimmed.
pub(crate) fn ids(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').map(str::trim).filter(|s| !s.is_empty())
}

fn li(text: &str, gate: Option<&str>) -> Node {
    let mut node = Node::with_text("li", text);
    if let Some(id) = gate {
        node.set_attr(IF_MOD_ACTIVE, id);
    }
    node
}

fn has_gate(block: &Node, folder: &str, package_id: &str) -> bool {
    block.children_named("li").any(|e| {
        normal(&e.text_content()) == normal(folder)
            && e.attr(IF_MOD_ACTIVE)
                .is_some_and(|v| ids(v).any(|i| i.eq_ignore_ascii_case(package_id)))
    })
}

/// The version numbers of a `v<major>.<minor>` block name.
pub(crate) fn block_version(tag: &str) -> Option<(u32, u32)> {
    let rest = tag.strip_prefix('v').or_else(|| tag.strip_prefix('V'))?;
    let (major, minor) = rest.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

pub(crate) fn version_numbers(version: &str) -> Option<(u32, u32)> {
    block_version(&format!("v{version}"))
}

/// The folder entries the project's layout needs: the base folder and the gated CE folder.
fn layout_entries(layout: &ProjectLayout) -> (String, String) {
    match &layout.version_folder {
        Some(v) => (v.clone(), format!("{v}/{}", layout.ce_folder)),
        None => ("/".to_owned(), layout.ce_folder.clone()),
    }
}

pub(crate) fn suffix_diagnostics(root: &Node) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for block in root.elements() {
        for (i, e) in block.children_named("li").enumerate() {
            for attr in [IF_MOD_ACTIVE, "IfModActiveAll"] {
                let Some(value) = e.attr(attr) else { continue };
                for id in ids(value) {
                    let lower = id.to_ascii_lowercase();
                    if ID_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
                        out.push(
                            CEP019
                                .diagnostic(
                                    &format!("/{}/{}/li[{}]/@{attr}", root.tag, block.tag, i + 1),
                                    &[("value", id)],
                                )
                                .with_arg("path", "LoadFolders.xml"),
                        );
                    }
                }
            }
        }
    }
    out
}

fn planned(layout: &ProjectLayout, tree: Node, action: FileAction) -> PlannedFile {
    let mut file = PlannedFile::new_file(
        layout.load_folders_path(),
        FileKind::LoadFolders,
        tree,
        vec![],
    );
    file.action = action;
    file
}

/// Plans `LoadFolders.xml` for the gated CE folder.
///
/// - `existing` is the parsed `LoadFolders.xml` of the project, `None` when it has none.
/// - `game_version` is the version block to serve, for example `1.6`.
/// - `package_id` is the package id of the installed Combat Extended (any case; written lower case).
#[must_use]
pub fn load_folders_plan(
    existing: Option<&Node>,
    layout: &ProjectLayout,
    game_version: &str,
    package_id: &str,
) -> LoadFoldersOutcome {
    let gate = package_id.to_ascii_lowercase();
    let (base, ce) = layout_entries(layout);
    let block_tag = format!("v{game_version}");
    let Some(root) = existing else {
        let mut block = Node::new(&block_tag);
        block.push_child(li(&base, None));
        block.push_child(li(&ce, Some(&gate)));
        let mut tree = Node::new("loadFolders");
        tree.push_child(block);
        return LoadFoldersOutcome {
            file: Some(planned(layout, tree, FileAction::Create)),
            diagnostics: Vec::new(),
        };
    };
    if root.tag != "loadFolders" {
        return LoadFoldersOutcome {
            file: None,
            diagnostics: vec![VALUE_INVALID.diagnostic(
                "",
                &[
                    ("label", "LoadFolders.xml"),
                    ("value", &root.tag),
                    ("reason", "the root element is not loadFolders"),
                ],
            )],
        };
    }
    let mut diagnostics = suffix_diagnostics(root);
    let mut tree = root.clone();
    let target = version_numbers(game_version);
    let newest = target.is_some_and(|t| {
        tree.elements()
            .filter_map(|b| block_version(&b.tag))
            .all(|v| v <= t)
    });
    // The block of the game version.
    if let Some(block) = tree.elements_mut().find(|b| b.tag == block_tag) {
        if !has_gate(block, &ce, &gate) {
            block.push_child(li(&ce, Some(&gate)));
        }
    } else {
        let previous = target.and_then(|t| {
            tree.elements()
                .filter_map(|b| block_version(&b.tag).map(|v| (v, b)))
                .filter(|(v, _)| *v < t)
                .max_by_key(|(v, _)| *v)
                .map(|(_, b)| b.clone())
        });
        let source = previous.or_else(|| tree.child(DEFAULT_BLOCK).cloned());
        let mut block = Node::new(&block_tag);
        match source {
            Some(src) => {
                for child in &src.children {
                    block.children.push(child.clone());
                }
            }
            None => block.push_child(li(&base, None)),
        }
        if !has_gate(&block, &ce, &gate) {
            block.push_child(li(&ce, Some(&gate)));
        }
        let at = tree
            .children
            .iter()
            .position(|c| matches!(c, Child::Element(e) if e.tag == DEFAULT_BLOCK))
            .unwrap_or(tree.children.len());
        tree.children.insert(at, Child::Element(block));
    }
    // The default block follows the newest block.
    if newest
        && let Some(block) = tree.elements_mut().find(|b| b.tag == DEFAULT_BLOCK)
        && !has_gate(block, &ce, &gate)
    {
        block.push_child(li(&ce, Some(&gate)));
    }
    let action = if tree == *root {
        FileAction::Unchanged
    } else {
        FileAction::UpdateRegion
    };
    diagnostics.sort_by(|a, b| a.args.cmp(&b.args));
    LoadFoldersOutcome {
        file: Some(planned(layout, tree, action)),
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    const ID: &str = "CETeam.CombatExtended";
    const CE_DIR: &str = "Compat/CombatExtended";

    fn gated(tree: &Node, block: &str) -> bool {
        tree.child(block)
            .is_some_and(|b| has_gate(b, CE_DIR, "ceteam.combatextended"))
    }

    #[test]
    fn no_file_creates_the_root_and_the_gated_folder() {
        let out = load_folders_plan(None, &ProjectLayout::default(), "1.6", ID);
        let file = out.file.unwrap();
        assert_eq!(file.action, FileAction::Create);
        assert_eq!(file.path, "LoadFolders.xml");
        let tree = file.tree.unwrap();
        let block = tree.child("v1.6").unwrap();
        let entries: Vec<(String, Option<&str>)> = block
            .children_named("li")
            .map(|e| (e.text_content(), e.attr("IfModActive")))
            .collect();
        assert_eq!(
            entries,
            vec![
                ("/".to_owned(), None),
                (CE_DIR.to_owned(), Some("ceteam.combatextended"))
            ]
        );
    }

    #[test]
    fn a_version_folder_layout_gates_the_folder_inside_it() {
        let layout = ProjectLayout::with_version_folder("1.6");
        let out = load_folders_plan(None, &layout, "1.6", ID);
        let tree = out.file.unwrap().tree.unwrap();
        let texts: Vec<String> = tree
            .child("v1.6")
            .unwrap()
            .children_named("li")
            .map(Node::text_content)
            .collect();
        assert_eq!(texts, ["1.6", "1.6/Compat/CombatExtended"]);
    }

    fn existing_without_ce() -> Node {
        NodeBuilder::new("loadFolders")
            .elem("v1.5", |b| b.li("/").li("1.5"))
            .elem(DEFAULT_BLOCK, |b| b.li("/"))
            .build()
    }

    #[test]
    fn an_existing_file_without_ce_gets_a_copied_block_and_a_default_entry() {
        let existing = existing_without_ce();
        let out = load_folders_plan(Some(&existing), &ProjectLayout::default(), "1.6", ID);
        let file = out.file.unwrap();
        assert_eq!(file.action, FileAction::UpdateRegion);
        let tree = file.tree.unwrap();
        // The v1.6 block copies the v1.5 block and adds the gate; the old block is untouched.
        let new_block = tree.child("v1.6").unwrap();
        assert_eq!(new_block.children_named("li").count(), 3);
        assert!(gated(&tree, "v1.6"));
        assert!(!gated(&tree, "v1.5"));
        assert!(gated(&tree, DEFAULT_BLOCK));
        // Blocks keep their order, the new one sits before default.
        let order: Vec<&str> = tree.elements().map(|e| e.tag.as_str()).collect();
        assert_eq!(order, ["v1.5", "v1.6", "default"]);
    }

    #[test]
    fn an_existing_file_with_the_gate_is_unchanged() {
        let first = load_folders_plan(None, &ProjectLayout::default(), "1.6", ID)
            .file
            .unwrap()
            .tree
            .unwrap();
        let out = load_folders_plan(
            Some(&first),
            &ProjectLayout::default(),
            "1.6",
            "ceteam.combatextended",
        );
        let file = out.file.unwrap();
        assert_eq!(file.action, FileAction::Unchanged);
        assert_eq!(file.tree.unwrap(), first);
        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn suffixed_ids_are_reported_and_not_copied() {
        let existing = NodeBuilder::new("loadFolders")
            .elem("v1.6", |b| {
                b.li("/").child({
                    let mut e = Node::with_text("li", CE_DIR);
                    e.set_attr("IfModActive", "CETeam.CombatExtended_copy");
                    e
                })
            })
            .build();
        let out = load_folders_plan(Some(&existing), &ProjectLayout::default(), "1.6", ID);
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(
            out.diagnostics[0].code.as_str(),
            "ce.cep019-ifmodactive-id-variant"
        );
        let tree = out.file.unwrap().tree.unwrap();
        let ids: Vec<&str> = tree
            .child("v1.6")
            .unwrap()
            .children_named("li")
            .filter_map(|e| e.attr("IfModActive"))
            .collect();
        // The old entry stays as it was, the canonical gate is added beside it.
        assert_eq!(ids, ["CETeam.CombatExtended_copy", "ceteam.combatextended"]);
    }

    #[test]
    fn a_wrong_root_cannot_be_edited() {
        let out = load_folders_plan(Some(&Node::new("x")), &ProjectLayout::default(), "1.6", ID);
        assert!(out.file.is_none());
        assert_eq!(out.diagnostics.len(), 1);
    }

    #[test]
    fn block_names_parse_as_versions() {
        assert_eq!(block_version("v1.6"), Some((1, 6)));
        assert_eq!(block_version("default"), None);
        assert_eq!(block_version("v1.x"), None);
    }
}
