//! The annotated tree of a project: every folder and file with its role in the layout, sizes, counts and
//! the issues of the layout check attached to the entries they are about.

use std::collections::{BTreeMap, HashMap};

use rimstudio_design::plan::LayoutProfile;
use rimstudio_ipc_types::project::{
    LayoutIssueDto, LayoutProfileDto, NodeRoleDto, ProjectTreeDto, TreeNodeDto, TreeNodeKindDto,
};

use super::check::check;
use super::scan::{Entry, Scan, scan};
use crate::shared::projectfs::ProjectView;

/// The default limit of entries in a tree.
pub const DEFAULT_MAX_NODES: usize = 4000;

/// The wire form of a layout profile.
#[must_use]
pub fn profile_dto(profile: LayoutProfile) -> LayoutProfileDto {
    match profile {
        LayoutProfile::Rimstudio => LayoutProfileDto::Rimstudio,
        LayoutProfile::CoreStyle => LayoutProfileDto::CoreStyle,
        LayoutProfile::Flat => LayoutProfileDto::Flat,
    }
}

struct Builder<'a> {
    by_parent: BTreeMap<&'a str, Vec<&'a Entry>>,
    bytes: HashMap<&'a str, u64>,
    files: HashMap<&'a str, u32>,
    issues: HashMap<String, u32>,
    budget: usize,
    truncated: bool,
}

impl<'a> Builder<'a> {
    fn new(scan: &'a Scan, issues: &[LayoutIssueDto], max_nodes: usize) -> Self {
        let mut by_parent: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
        let mut bytes: HashMap<&str, u64> = HashMap::new();
        let mut files: HashMap<&str, u32> = HashMap::new();
        for e in &scan.entries {
            by_parent.entry(e.parent()).or_default().push(e);
            if !e.is_dir {
                let mut dir = e.parent();
                loop {
                    *bytes.entry(dir).or_default() += e.bytes;
                    *files.entry(dir).or_default() += 1;
                    match dir.rsplit_once('/') {
                        Some((up, _)) => dir = up,
                        None if dir.is_empty() => break,
                        None => dir = "",
                    }
                }
            }
        }
        // folders before files, each group by name
        for list in by_parent.values_mut() {
            list.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        }
        let mut counts: HashMap<String, u32> = HashMap::new();
        for issue in issues {
            let mut path = issue.path.as_str();
            loop {
                *counts.entry(path.to_owned()).or_default() += 1;
                match path.rsplit_once('/') {
                    Some((up, _)) => path = up,
                    None if path.is_empty() => break,
                    None => path = "",
                }
            }
        }
        Self {
            by_parent,
            bytes,
            files,
            issues: counts,
            budget: max_nodes,
            truncated: false,
        }
    }

    fn node(&mut self, e: &Entry) -> TreeNodeDto {
        let children = if e.is_dir {
            self.children(&e.rel)
        } else {
            Vec::new()
        };
        TreeNodeDto {
            name: e.name.clone(),
            path: e.rel.clone(),
            kind: if e.is_dir {
                TreeNodeKindDto::Folder
            } else {
                TreeNodeKindDto::File
            },
            role: e.role,
            bytes: if e.is_dir {
                self.bytes.get(e.rel.as_str()).copied().unwrap_or(0)
            } else {
                e.bytes
            },
            files: if e.is_dir {
                self.files.get(e.rel.as_str()).copied().unwrap_or(0)
            } else {
                1
            },
            issues: self.issues.get(&e.rel).copied().unwrap_or(0),
            children,
        }
    }

    fn children(&mut self, parent: &str) -> Vec<TreeNodeDto> {
        let entries: Vec<&Entry> = self.by_parent.get(parent).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for e in entries {
            if self.budget == 0 {
                self.truncated = true;
                break;
            }
            self.budget -= 1;
            out.push(self.node(e));
        }
        out
    }
}

/// The tree of a scanned project with the given issues attached.
#[must_use]
pub fn tree_from_scan(
    view: &ProjectView,
    project_id: &str,
    scan: &Scan,
    issues: Vec<LayoutIssueDto>,
    max_nodes: usize,
) -> ProjectTreeDto {
    let mut b = Builder::new(scan, &issues, max_nodes);
    let children = b.children("");
    let root = TreeNodeDto {
        name: view.root.file_name().unwrap_or("").to_owned(),
        path: String::new(),
        kind: TreeNodeKindDto::Folder,
        role: NodeRoleDto::ContentRoot,
        bytes: b.bytes.get("").copied().unwrap_or(0),
        files: b.files.get("").copied().unwrap_or(0),
        issues: b.issues.get("").copied().unwrap_or(0),
        children,
    };
    let l = &view.layout;
    ProjectTreeDto {
        project_id: project_id.to_owned(),
        profile: profile_dto(l.profile),
        content_folder: l.version_folder.clone(),
        weapons_folder: l.weapons_folder(),
        ce_folder: l.ce_folder.clone(),
        ce_folder_exists: scan.ce_folder_exists,
        ce_folder_legacy: scan.ce_folder_exists && l.has_legacy_ce_folder(),
        ce_gated: view.gates_on(rimstudio_core::paths::CE_PACKAGE_ID),
        root,
        counts: scan.counts.clone(),
        issues,
        truncated: b.truncated || scan.truncated,
    }
}

/// Scans a project and builds its annotated tree (a read only call).
#[must_use]
pub fn project_tree_of(view: &ProjectView, project_id: &str, max_nodes: usize) -> ProjectTreeDto {
    let scan = scan(view);
    let issues = check(view, &scan);
    tree_from_scan(view, project_id, &scan, issues, max_nodes)
}
