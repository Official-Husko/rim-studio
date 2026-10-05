//! A tool module never uses another tool module (ADR 0004): the designer, the project tool and the def
//! explorer reach each other only through `shared`, and `shared` reaches none of them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// True when `line` holds `path` as a whole path segment (`crate::project` but not `crate::projectfs`).
fn names_path(line: &str, path: &str) -> bool {
    let mut from = 0;
    while let Some(at) = line.get(from..).and_then(|s| s.find(path)) {
        let end = from + at + path.len();
        let next = line.get(end..).and_then(|s| s.chars().next());
        if next.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
            return true;
        }
        from = end;
    }
    false
}

fn mentions(file: &Path, needles: &[&str]) -> Vec<String> {
    let text = std::fs::read_to_string(file).unwrap();
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .filter(|(_, l)| needles.iter().any(|n| names_path(l, n)))
        .map(|(i, l)| format!("{}:{}: {}", file.display(), i + 1, l.trim()))
        .collect()
}

fn tool_paths(own: &str) -> Vec<String> {
    ["designer", "project", "defs"]
        .iter()
        .filter(|t| **t != own)
        .flat_map(|t| {
            [
                format!("crate::{t}"),
                format!("super::{t}"),
                format!("rimstudio_toolkit::{t}"),
            ]
        })
        .collect()
}

fn check(files: &[PathBuf], own: &str) -> Vec<String> {
    let needles = tool_paths(own);
    let needles: Vec<&str> = needles.iter().map(String::as_str).collect();
    files.iter().flat_map(|f| mentions(f, &needles)).collect()
}

#[test]
fn the_designer_does_not_use_the_project_or_defs_modules() {
    let mut files = Vec::new();
    rust_files(&src().join("designer"), &mut files);
    assert!(!files.is_empty());
    let hits = check(&files, "designer");
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn the_project_tool_does_not_use_the_designer_or_defs_modules() {
    let mut files = Vec::new();
    rust_files(&src().join("project"), &mut files);
    assert!(files.len() >= 5, "{files:?}");
    let hits = check(&files, "project");
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn the_def_explorer_does_not_use_the_designer_or_project_modules() {
    let hits = check(&[src().join("defs.rs")], "defs");
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn shared_pieces_use_no_tool_module() {
    let mut files = Vec::new();
    rust_files(&src().join("shared"), &mut files);
    assert!(!files.is_empty());
    let hits = check(&files, "shared");
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn no_file_of_the_toolkit_holds_xml_markup_or_depends_on_an_xml_library() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for lib in ["quick-xml", "xml-rs", "roxmltree", "minidom", "xmltree"] {
        assert!(
            !manifest.contains(lib),
            "{lib} must not be a dependency (IT-050)"
        );
    }
}
