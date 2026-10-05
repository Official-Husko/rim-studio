//! Loaders that turn folders of a real or fictional install into node trees.
//!
//! Used by the `#[ignore]` real install tests of other crates and by tests over fixtures built
//! with [`crate::install_tree`]. The loaders stop at nodes: they never run an engine (merge,
//! patch, inheritance), and they only read.
//!
//! Files are visited in ordinal order of their relative path (case sensitive byte order), so the
//! result is the same on every file system. Names starting with a dot are skipped, as the game
//! does. A file that the game's loader would reject is reported in
//! [`DirParse::failures`] and is not part of [`DirParse::files`].

use std::fs;
use std::io;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_xml::about::{AboutRead, read_lenient};
use rimstudio_xml::error::codes;
use rimstudio_xml::load_folders::{self, LoadFoldersRead};
use rimstudio_xml::mods_config::{self, ModsConfigRead};
use rimstudio_xml::{ParseMode, XmlError, parse_document};

/// One parsed XML file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    /// The path relative to the folder that was loaded, with `/` separators.
    pub rel_path: Utf8PathBuf,
    /// The root element (`Defs` or `Patch` in a healthy file).
    pub root: Node,
    /// Warnings found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

/// A file the game's loader would skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedFile {
    /// The path relative to the folder that was loaded.
    pub rel_path: Utf8PathBuf,
    /// Why it failed.
    pub error: XmlError,
}

/// The outcome of loading a folder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirParse {
    /// Files that parsed, in ordinal path order.
    pub files: Vec<ParsedFile>,
    /// Files that did not, in ordinal path order.
    pub failures: Vec<FailedFile>,
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_xml_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

fn collect(
    dir: &Path,
    rel: &Utf8Path,
    out: &mut Vec<(Utf8PathBuf, std::path::PathBuf)>,
) -> io::Result<()> {
    let mut entries: Vec<(String, std::path::PathBuf, bool)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_hidden(&name) {
            continue;
        }
        let is_dir = entry.file_type()?.is_dir();
        entries.push((name, entry.path(), is_dir));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, path, is_dir) in entries {
        let child_rel = rel.join(&name);
        if is_dir {
            collect(&path, &child_rel, out)?;
        } else if is_xml_name(&name) {
            out.push((child_rel, path));
        }
    }
    Ok(())
}

fn parse_dir(dir: &Path, expected_root: &str) -> io::Result<DirParse> {
    let mut found = Vec::new();
    collect(dir, Utf8Path::new(""), &mut found)?;
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = DirParse::default();
    for (rel_path, path) in found {
        let bytes = fs::read(&path)?;
        match parse_document(&bytes, ParseMode::Game) {
            Ok(doc) => {
                let mut diagnostics = doc.diagnostics;
                if doc.root.tag != expected_root {
                    diagnostics.push(Diagnostic::new(
                        codes::UNEXPECTED_ROOT.clone(),
                        Severity::Warning,
                        format!(
                            "the root element is named {}, expected {expected_root}",
                            doc.root.tag
                        ),
                    ));
                }
                out.files.push(ParsedFile {
                    rel_path,
                    root: doc.root,
                    diagnostics,
                });
            }
            Err(error) => out.failures.push(FailedFile { rel_path, error }),
        }
    }
    Ok(out)
}

/// Parses every `*.xml` below a `Defs` folder in game mode and reports failures separately.
///
/// # Errors
/// File system errors (a missing folder is [`io::ErrorKind::NotFound`]).
pub fn parse_defs_dir_report(path: impl AsRef<Path>) -> io::Result<DirParse> {
    parse_dir(path.as_ref(), "Defs")
}

/// Parses every `*.xml` below a `Defs` folder; files the game would skip are left out. Use
/// [`parse_defs_dir_report`] to see them. Panics are not used: an unreadable folder gives an empty
/// list.
#[must_use]
pub fn parse_defs_dir(path: impl AsRef<Path>) -> Vec<ParsedFile> {
    parse_defs_dir_report(path)
        .map(|d| d.files)
        .unwrap_or_default()
}

/// Parses every `*.xml` below a `Patches` folder in game mode and reports failures separately.
///
/// # Errors
/// File system errors (a missing folder is [`io::ErrorKind::NotFound`]).
pub fn parse_patch_dir_report(path: impl AsRef<Path>) -> io::Result<DirParse> {
    parse_dir(path.as_ref(), "Patch")
}

/// Parses every `*.xml` below a `Patches` folder; files the game would skip are left out. The
/// root of each [`ParsedFile`] is the `Patch` element, its children are the operations.
#[must_use]
pub fn parse_patch_dir(path: impl AsRef<Path>) -> Vec<ParsedFile> {
    parse_patch_dir_report(path)
        .map(|d| d.files)
        .unwrap_or_default()
}

/// Finds a file by name ignoring case inside a directory (the game resolves some names that way).
fn find_ignoring_case(dir: &Path, name: &str) -> io::Result<Option<std::path::PathBuf>> {
    let mut best: Option<(String, std::path::PathBuf)> = None;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Some(n) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if n.eq_ignore_ascii_case(name) && entry.file_type()?.is_file() {
            // Prefer the exact spelling, otherwise the first by name for determinism.
            let better = match &best {
                None => true,
                Some((b, _)) => n == name || (b != name && n < *b),
            };
            if better {
                best = Some((n, entry.path()));
            }
        }
    }
    Ok(best.map(|(_, p)| p))
}

/// Reads the About of a mod: `path` is either the mod folder (the `About` folder must be spelled
/// exactly, the file name is matched ignoring case) or the About file itself.
///
/// # Errors
/// [`io::ErrorKind::NotFound`] when there is no About file, other file system errors.
pub fn read_about(path: impl AsRef<Path>) -> io::Result<AboutRead> {
    let path = path.as_ref();
    let file = if path.is_file() {
        path.to_path_buf()
    } else {
        find_ignoring_case(&path.join("About"), "About.xml")?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no About.xml in {}", path.display()),
            )
        })?
    };
    Ok(read_lenient(&fs::read(file)?))
}

/// Reads the `LoadFolders.xml` of a mod folder (file name matched ignoring case). `Ok(None)` when
/// the mod has none.
///
/// # Errors
/// File system errors, or [`io::ErrorKind::InvalidData`] when the file has no root element.
pub fn read_load_folders(mod_root: impl AsRef<Path>) -> io::Result<Option<LoadFoldersRead>> {
    let Some(file) = find_ignoring_case(mod_root.as_ref(), "LoadFolders.xml")? else {
        return Ok(None);
    };
    load_folders::read(&fs::read(file)?)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// Reads a `ModsConfig.xml` file.
///
/// # Errors
/// File system errors, or [`io::ErrorKind::InvalidData`] when the file has no root element.
pub fn read_mods_config(file: impl AsRef<Path>) -> io::Result<ModsConfigRead> {
    mods_config::read(&fs::read(file)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_tree::{InstallBuilder, ModFolder};
    use rimstudio_core::tree::NodeBuilder;

    fn gun(name: &str) -> Node {
        NodeBuilder::new("ThingDef")
            .text_elem("defName", name)
            .build()
    }

    #[test]
    fn defs_are_loaded_in_ordinal_order_with_relative_paths() {
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_A", "rs.a")
                    .defs_file("b/Second.xml", vec![gun("RS_2")])
                    .defs_file("a/First.xml", vec![gun("RS_1")])
                    .defs_file("Top.xml", vec![gun("RS_0")])
                    .raw_file("Defs/.hidden.xml", b"<Defs/>".to_vec())
                    .raw_file("Defs/notes.txt", b"x".to_vec()),
            )
            .build_temp()
            .unwrap();
        let files = parse_defs_dir(install.mod_path("RS_A").unwrap().join("Defs"));
        let names: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(names, ["Top.xml", "a/First.xml", "b/Second.xml"]);
        assert_eq!(
            files[1]
                .root
                .child("ThingDef")
                .unwrap()
                .child_text("defName"),
            Some("RS_1")
        );
    }

    #[test]
    fn broken_files_are_reported_not_loaded() {
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_A", "rs.a")
                    .def(gun("RS_1"))
                    .raw_file("Defs/Bad.xml", b"<Defs><a></Defs>".to_vec())
                    .raw_file("Defs/Odd.xml", b"<Other/>".to_vec()),
            )
            .build_temp()
            .unwrap();
        let report = parse_defs_dir_report(install.mod_path("RS_A").unwrap().join("Defs")).unwrap();
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].rel_path, "Bad.xml");
        assert_eq!(report.files.len(), 2);
        let odd = report
            .files
            .iter()
            .find(|f| f.rel_path == "Odd.xml")
            .unwrap();
        assert_eq!(odd.diagnostics[0].code.as_str(), "xml.unexpected-root");
    }

    #[test]
    fn missing_folders_are_empty_lists_or_not_found_errors() {
        assert!(parse_defs_dir("/definitely/not/here").is_empty());
        let err = parse_patch_dir_report("/definitely/not/here").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn patches_load_with_a_patch_root() {
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_A", "rs.a")
                    .patch_file("P.xml", vec![NodeBuilder::new("Operation").build()]),
            )
            .build_temp()
            .unwrap();
        let files = parse_patch_dir(install.mod_path("RS_A").unwrap().join("Patches"));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].root.tag, "Patch");
        assert_eq!(files[0].root.elements().count(), 1);
        assert!(files[0].diagnostics.is_empty());
    }

    #[test]
    fn about_is_found_by_folder_or_file_and_the_file_name_ignores_case() {
        let install = InstallBuilder::new()
            .mod_folder(ModFolder::new("RS_A", "rs.a").about_file_name("about.xml"))
            .mod_folder(ModFolder::new("RS_Lower", "rs.lower").lowercase_about_dir())
            .build_temp()
            .unwrap();
        let root = install.mod_path("RS_A").unwrap();
        assert_eq!(read_about(root).unwrap().about.package_id, "rs.a");
        assert_eq!(
            read_about(root.join("About/about.xml"))
                .unwrap()
                .about
                .package_id,
            "rs.a"
        );
        let lower = read_about(install.mod_path("RS_Lower").unwrap()).unwrap_err();
        assert_eq!(lower.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn load_folders_and_mods_config_are_read() {
        use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_A", "rs.a")
                    .load_folders_file_name("loadfolders.xml")
                    .load_folders(LoadFoldersSpec::from_blocks([(
                        "1.6",
                        vec![LoadEntry::root(), LoadEntry::dir("CE").if_active(["rs.ce"])],
                    )])),
            )
            .mod_folder(ModFolder::new("RS_B", "rs.b"))
            .active_mods(&["ludeon.rimworld", "rs.a"])
            .build_temp()
            .unwrap();
        let lf = read_load_folders(install.mod_path("RS_A").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(lf.spec.block("1.6").unwrap().entries.len(), 2);
        assert!(
            read_load_folders(install.mod_path("RS_B").unwrap())
                .unwrap()
                .is_none()
        );
        let cfg = read_mods_config(install.mods_config_path()).unwrap();
        assert_eq!(cfg.data.active_mods, ["ludeon.rimworld", "rs.a"]);
    }
}
