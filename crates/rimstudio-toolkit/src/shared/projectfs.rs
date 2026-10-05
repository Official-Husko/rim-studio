//! A read only view of a mod folder: what the planners need to know before they write anything.
//!
//! [`read_project`] reads `About/About.xml` (leniently), `LoadFolders.xml` when there is one, and looks at the
//! folder to decide the file layout of the project (flat, or one folder per game version). Nothing is
//! written. The view is what the designer's plan and the project tool share, so neither has to use the
//! other.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_design::plan::{ProjectLayout, file_stem};
use rimstudio_workspace::listing::DiskListing;
use rimstudio_workspace::project::{find_about_file, find_load_folders_file};
use rimstudio_xml::about::read_lenient;
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_document;

use crate::error::{ToolkitError, ToolkitResult};

/// The attribute that gates a `LoadFolders.xml` entry on active mods.
const IF_MOD_ACTIVE: &str = "IfModActive";

/// The `LoadFolders.xml` of a project.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadFoldersFile {
    /// The path relative to the project root (the name as it is on disk, for example `loadfolders.xml`).
    pub rel: String,
    /// The file text (a byte order mark is kept).
    pub text: String,
    /// The parsed root element.
    pub root: Node,
}

/// What the planners know about a project folder.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectView {
    /// The absolute, clean project root.
    pub root: Utf8PathBuf,
    /// The mod name from About.xml, the folder name when it has none.
    pub name: String,
    /// The package id from About.xml, when it has a usable one.
    pub package_id: Option<String>,
    /// The `supportedVersions` entries in file order.
    pub supported_versions: Vec<String>,
    /// The About file relative to the project root.
    pub about_rel: String,
    /// The `LoadFolders.xml`, when the project has one that could be read.
    pub load_folders: Option<LoadFoldersFile>,
    /// The file layout of the project (version folder, mod slug, CE folder).
    pub layout: ProjectLayout,
    /// Problems found while reading (About quirks, an unreadable `LoadFolders.xml`).
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectView {
    /// True when `LoadFolders.xml` has an entry gated on the given package id (any case) in any block.
    #[must_use]
    pub fn gates_on(&self, package_id: &str) -> bool {
        let Some(file) = &self.load_folders else {
            return false;
        };
        file.root.elements().any(|block| {
            block.children_named("li").any(|li| {
                li.attr(IF_MOD_ACTIVE).is_some_and(|ids| {
                    ids.split(',')
                        .any(|id| id.trim().eq_ignore_ascii_case(package_id))
                })
            })
        })
    }

    /// The game version a plan serves: the newest `major.minor` of `supportedVersions`, else `fallback`.
    #[must_use]
    pub fn game_version(&self, fallback: &str) -> String {
        let mut best: Option<((u32, u32), &str)> = None;
        for v in &self.supported_versions {
            let key = v.trim().trim_start_matches(['v', 'V']);
            let mut parts = key.split('.');
            let (Some(major), Some(minor)) = (parts.next(), parts.next()) else {
                continue;
            };
            let (Ok(major), Ok(minor)) = (major.parse::<u32>(), minor.parse::<u32>()) else {
                continue;
            };
            if best.is_none_or(|(b, _)| (major, minor) > b) {
                best = Some(((major, minor), v.trim()));
            }
        }
        best.map_or_else(
            || fallback.to_owned(),
            |((major, minor), _)| format!("{major}.{minor}"),
        )
    }
}

fn last_segment_slug(package_id: &str) -> String {
    let last = package_id.rsplit('.').next().unwrap_or(package_id);
    if last.is_empty() {
        String::new()
    } else {
        file_stem(last)
    }
}

/// Decides the layout of a project folder: a flat project has a `Defs` folder at its root; a project that
/// keeps one folder per game version is recognised by a folder named like a supported version.
#[must_use]
pub fn detect_layout(
    root: &Utf8Path,
    supported: &[String],
    package_id: Option<&str>,
) -> ProjectLayout {
    let dirs = DiskListing::child_dirs(root);
    let mut layout = ProjectLayout::default();
    if !dirs.iter().any(|d| d == "Defs") {
        let mut candidates: Vec<&String> = supported.iter().collect();
        candidates.reverse();
        for v in candidates {
            if dirs.iter().any(|d| d == v.trim()) {
                layout.version_folder = Some(v.trim().to_owned());
                break;
            }
        }
    }
    if let Some(ce) = dirs.iter().find(|d| d.eq_ignore_ascii_case("CE")) {
        layout.ce_folder.clone_from(ce);
    }
    if let Some(id) = package_id {
        layout.mod_slug = last_segment_slug(id);
    }
    layout
}

fn relative_to(root: &Utf8Path, path: &Utf8Path) -> String {
    path.strip_prefix(root)
        .map_or_else(|_| path.to_string(), |p| p.as_str().replace('\\', "/"))
}

/// Reads a project folder.
///
/// # Errors
///
/// [`ToolkitError::ProjectInvalid`] when the folder has no About file or the file cannot be read.
pub fn read_project(root: &Utf8Path) -> ToolkitResult<ProjectView> {
    let root = rimstudio_workspace::project::normalize_project_path(root)?;
    let about_path = find_about_file(&root).ok_or_else(|| ToolkitError::ProjectInvalid {
        path: root.to_string(),
        reason: "it has no About/About.xml".to_owned(),
    })?;
    let bytes =
        std::fs::read(about_path.as_std_path()).map_err(|e| ToolkitError::ProjectInvalid {
            path: root.to_string(),
            reason: format!("About.xml cannot be read: {e}"),
        })?;
    let read = read_lenient(&bytes);
    let mut diagnostics = read.warnings;
    let about = read.about;
    let name = if about.name.trim().is_empty() {
        root.file_name().unwrap_or("").to_owned()
    } else {
        about.name.trim().to_owned()
    };
    let package_id = rimstudio_core::ids::PackageId::parse(&about.package_id)
        .ok()
        .map(|p| p.as_str().to_owned());
    let supported_versions = about.supported_versions.clone();
    let load_folders = match find_load_folders_file(&root) {
        None => None,
        Some(path) => match std::fs::read(path.as_std_path()) {
            Err(e) => {
                diagnostics.push(Diagnostic::new(
                    rimstudio_core::diag::DiagCode::new("project.file-unreadable"),
                    Severity::Warning,
                    format!("{path} cannot be read: {e}"),
                ));
                None
            }
            Ok(bytes) => match parse_document(&bytes, ParseMode::Tolerant) {
                Err(e) => {
                    diagnostics.push(e.to_diagnostic());
                    None
                }
                Ok(doc) => {
                    diagnostics.extend(doc.diagnostics);
                    Some(LoadFoldersFile {
                        rel: relative_to(&root, &path),
                        text: String::from_utf8_lossy(&bytes).into_owned(),
                        root: doc.root,
                    })
                }
            },
        },
    };
    let layout = detect_layout(&root, &supported_versions, package_id.as_deref());
    Ok(ProjectView {
        root,
        name,
        package_id,
        supported_versions,
        about_rel: relative_to_about(&about_path),
        load_folders,
        layout,
        diagnostics,
    })
}

fn relative_to_about(path: &Utf8Path) -> String {
    let n = path.components().count();
    let tail: Vec<&str> = path
        .components()
        .skip(n.saturating_sub(2))
        .map(|c| c.as_str())
        .collect();
    tail.join("/")
}

/// The number of definition files (`*.xml` below a `Defs` folder) of a project, at any depth, not following
/// links. Used for summaries only.
#[must_use]
pub fn count_def_files(root: &Utf8Path) -> u32 {
    fn walk(dir: &std::path::Path, in_defs: bool, depth: usize, count: &mut u32) {
        if depth > 12 {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                walk(&path, in_defs || name == "Defs", depth + 1, count);
            } else if in_defs && kind.is_file() && name.to_ascii_lowercase().ends_with(".xml") {
                *count = count.saturating_add(1);
            }
        }
    }
    let mut count = 0u32;
    walk(root.as_std_path(), false, 0, &mut count);
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Utf8Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
        std::fs::write(path.as_std_path(), text).unwrap();
    }

    fn about(id: &str, versions: &[&str]) -> String {
        let items: String = versions.iter().map(|v| format!("<li>{v}</li>")).collect();
        format!(
            "<ModMetaData><name>RS Mod</name><packageId>{id}</packageId><supportedVersions>{items}</supportedVersions></ModMetaData>"
        )
    }

    fn temp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        (t, p)
    }

    #[test]
    fn a_flat_project_is_read() {
        let (_t, root) = temp();
        write(
            &root.join("About/About.xml"),
            &about("rs.testmod", &["1.5", "1.6"]),
        );
        std::fs::create_dir_all(root.join("Defs").as_std_path()).unwrap();
        let view = read_project(&root).unwrap();
        assert_eq!(view.name, "RS Mod");
        assert_eq!(view.package_id.as_deref(), Some("rs.testmod"));
        assert_eq!(view.layout.version_folder, None);
        assert_eq!(view.layout.mod_slug, "testmod");
        assert_eq!(view.game_version("1.0"), "1.6");
        assert!(view.load_folders.is_none());
        assert!(!view.gates_on("ceteam.combatextended"));
    }

    #[test]
    fn a_versioned_project_and_its_gate_are_recognised() {
        let (_t, root) = temp();
        write(
            &root.join("About/About.xml"),
            &about("rs.testmod", &["1.5", "1.6"]),
        );
        std::fs::create_dir_all(root.join("1.6/Defs").as_std_path()).unwrap();
        write(
            &root.join("LoadFolders.xml"),
            "<loadFolders><v1.6><li>1.6</li><li IfModActive=\"CETeam.CombatExtended\">1.6/CE</li></v1.6></loadFolders>",
        );
        let view = read_project(&root).unwrap();
        assert_eq!(view.layout.version_folder.as_deref(), Some("1.6"));
        assert!(view.gates_on("ceteam.combatextended"));
        assert_eq!(view.load_folders.as_ref().unwrap().rel, "LoadFolders.xml");
        assert_eq!(count_def_files(&root), 0);
    }

    #[test]
    fn a_folder_without_about_is_not_a_project() {
        let (_t, root) = temp();
        assert!(matches!(
            read_project(&root),
            Err(ToolkitError::ProjectInvalid { .. })
        ));
    }

    #[test]
    fn game_version_falls_back_without_supported_versions() {
        let (_t, root) = temp();
        write(&root.join("About/About.xml"), &about("rs.testmod", &[]));
        let view = read_project(&root).unwrap();
        assert_eq!(view.game_version("1.6"), "1.6");
    }
}
