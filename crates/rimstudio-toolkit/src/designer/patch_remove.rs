//! Taking the generated Combat Extended patch of a weapon out of a project (flow D, remove mode).
//!
//! A conversion request with `remove` set plans the removal ([`remove_plan`]); the ordinary
//! `designer_apply_plan` job applies it, so a removal is reviewed as a diff, checked against the files on
//! disk and backed up like any other write. Only files that RimStudio generated are touched: a patch file
//! counts as generated when it carries the generator's mark, and a section is the comment
//! `====== <defName> ======` with the run of operations that follows it. A file that loses its last section
//! is deleted; every other file keeps every byte outside the removed sections. A hand written patch, a
//! patch of another mod and the definitions of the project are never changed.

use std::collections::BTreeMap;

use camino::Utf8Path;
use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_design::plan::{FileAction, FileKind};

use super::convert::OWNED_MARK;
use super::plan::{BuiltFile, MERGE_FAILED, PATH_REFUSED};
use crate::error::ToolkitError;
use crate::shared::diff::unified_diff;
use crate::shared::merge::{changed_span, remove_sections};
use crate::shared::writer::GuardedWriter;

/// The diagnostic code (error) of a removal that found no generated section of the weapon.
pub const REMOVE_NOTHING: DiagCode = DiagCode::new("designer.remove-nothing");

/// The root element of a patch file.
const PATCH_ROOT: &str = "Patch";
/// The marker that opens the comment of a section.
const BANNER_OPEN: &str = "<!-- ====== ";
/// The marker that closes the name in the comment of a section.
const BANNER_CLOSE: &str = " ======";
/// The deepest folder nesting the walk follows.
const MAX_DEPTH: usize = 12;
/// The largest patch file the walk reads, in bytes.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// Folders of a mod that never hold patches and can be large.
const SKIPPED_FOLDERS: [&str; 6] = [
    "Textures",
    "Sounds",
    "Assemblies",
    "Source",
    ".git",
    "node_modules",
];

fn walk(
    dir: &std::path::Path,
    rel: &str,
    under_patches: bool,
    depth: usize,
    out: &mut Vec<String>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        // a link is never followed: the removal stays inside the folder it was asked for
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if kind.is_dir() {
            if SKIPPED_FOLDERS
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let inside = under_patches || name.eq_ignore_ascii_case("Patches");
            walk(&entry.path(), &child_rel, inside, depth + 1, out);
        } else if kind.is_file()
            && under_patches
            && Utf8Path::new(&name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
        {
            out.push(child_rel);
        }
    }
}

/// The patch files of a project: every XML file below a folder called `Patches`, relative to the project
/// root with `/` separators, sorted.
#[must_use]
pub fn patch_files(root: &Utf8Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root.as_std_path(), "", false, 0, &mut out);
    out.sort();
    out
}

/// The names in the section comments of a text, in order, without repeats.
fn section_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (at, _) in text.match_indices(BANNER_OPEN) {
        let rest = text.get(at + BANNER_OPEN.len()..).unwrap_or("");
        let Some(end) = rest.find(BANNER_CLOSE) else {
            continue;
        };
        let name = rest.get(..end).unwrap_or("").trim();
        if !name.is_empty() && !name.contains(['<', '>', '\n']) && !names.iter().any(|n| n == name)
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// The generated patch files of a project by definition name: the files that carry the generator's mark
/// and hold a `====== <defName> ======` section of that definition. The files of one definition are in path
/// order.
#[must_use]
pub fn generated_sections(root: &Utf8Path) -> BTreeMap<String, Vec<String>> {
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in patch_files(root) {
        let path = root.join(&rel);
        let Ok(meta) = std::fs::metadata(path.as_std_path()) else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path.as_std_path()) else {
            continue;
        };
        if !text.contains(OWNED_MARK) {
            continue;
        }
        for name in section_names(&text) {
            index.entry(name).or_default().push(rel.clone());
        }
    }
    index
}

/// What a removal plans: the files that change or go, and the diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovalPlan {
    /// The files in path order.
    pub files: Vec<BuiltFile>,
    /// Problems: no section found, a file that cannot be read or edited.
    pub diagnostics: Vec<Diagnostic>,
}

fn file_problem(code: &DiagCode, path: &str, message: String) -> Diagnostic {
    Diagnostic::new(code.clone(), Severity::Error, message).with_arg("path", path)
}

/// Plans the removal of the generated patch of `def_name`: every generated patch file of the project that
/// holds a section of the definition loses it, and a file with nothing left is deleted. Reads the project,
/// writes nothing.
#[must_use]
pub fn remove_plan(writer: &GuardedWriter, def_name: &str) -> RemovalPlan {
    let index = generated_sections(writer.root());
    let holders = index.get(def_name).cloned().unwrap_or_default();
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    for rel in holders {
        let old = match writer.read_text(&rel) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(e) => {
                let code = match e {
                    ToolkitError::PathRefused { .. } => &PATH_REFUSED,
                    _ => &MERGE_FAILED,
                };
                diagnostics.push(file_problem(
                    code,
                    &rel,
                    format!("{rel} cannot be read: {e}"),
                ));
                continue;
            }
        };
        let removal = match remove_sections(&old, PATCH_ROOT, def_name) {
            Ok(r) if r.removed > 0 => r,
            Ok(_) => continue,
            Err(reason) => {
                diagnostics.push(
                    file_problem(
                        &MERGE_FAILED,
                        &rel,
                        format!("{rel} cannot be updated: {reason}"),
                    )
                    .with_arg("reason", reason),
                );
                continue;
            }
        };
        let built = if removal.empty {
            BuiltFile {
                path: rel.clone(),
                kind: FileKind::CePatch,
                action: FileAction::Delete,
                text: String::new(),
                diff: Some(unified_diff(&rel, &old, "")).filter(|d| !d.is_empty()),
                edits: Vec::new(),
                previous: Some(old),
                copy: None,
            }
        } else {
            BuiltFile {
                path: rel.clone(),
                kind: FileKind::CePatch,
                action: FileAction::UpdateRegion,
                diff: Some(unified_diff(&rel, &old, &removal.text)).filter(|d| !d.is_empty()),
                edits: changed_span(&old, &removal.text),
                text: removal.text,
                previous: Some(old),
                copy: None,
            }
        };
        files.push(built);
    }
    if files.is_empty() && diagnostics.is_empty() {
        diagnostics.push(
            Diagnostic::new(
                REMOVE_NOTHING,
                Severity::Error,
                format!(
                    "no patch file generated by RimStudio holds a section of {def_name}; there is \
                     nothing to remove"
                ),
            )
            .with_arg("field", "/convert/defName"),
        );
    }
    RemovalPlan { files, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_names_are_read_in_order_without_repeats() {
        let text = "<Patch>\n  <!-- ====== RS_A ====== generated by RimStudio, draft RS_A -->\n  \
                    <Operation/>\n  <!-- ====== RS_B ====== generated by RimStudio, draft RS_B -->\n  \
                    <Operation/>\n  <!-- ====== RS_A ====== generated by RimStudio, draft RS_A -->\n\
                    </Patch>";
        assert_eq!(section_names(text), vec!["RS_A", "RS_B"]);
        assert!(section_names("<Patch><!-- plain --></Patch>").is_empty());
    }
}
