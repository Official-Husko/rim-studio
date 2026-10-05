//! The plan builder: the extension point other generators (the CE patch task) use to add files.

use rimstudio_core::diag::Diagnostic;

use crate::validation::codes;

use super::types::{PlannedFile, WritePlan};

/// True when `path` is a relative path made of plain segments: not empty, no leading `/`, no backslash, no
/// `..` or `.` segment, no empty segment, no drive prefix and no control character. This is the guard that
/// keeps every planned file inside the project root (IT-004).
#[must_use]
pub fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    path.split('/').all(|seg| {
        !seg.is_empty()
            && seg != "."
            && seg != ".."
            && !seg.contains(':')
            && !seg.chars().any(char::is_control)
    })
}

/// Collects files and diagnostics into a [`WritePlan`].
///
/// Adding a file whose path is not safe, or already used, records an error diagnostic and drops the file.
/// [`PlanBuilder::build`] sorts the files by path, so the plan does not depend on the order of insertion.
#[derive(Debug, Default)]
pub struct PlanBuilder {
    files: Vec<PlannedFile>,
    diagnostics: Vec<Diagnostic>,
}

impl PlanBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file. Returns `false` (and records an error diagnostic) when the path is unsafe or already
    /// taken.
    pub fn add_file(&mut self, file: PlannedFile) -> bool {
        if !is_safe_relative_path(&file.path) {
            self.diagnostics
                .push(codes::PLAN_PATH_INVALID.diagnostic("", &[("path", &file.path)]));
            return false;
        }
        if self.files.iter().any(|f| f.path == file.path) {
            self.diagnostics
                .push(codes::PLAN_PATH_CONFLICT.diagnostic("", &[("path", &file.path)]));
            return false;
        }
        self.files.push(file);
        true
    }

    /// Records one diagnostic.
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Records many diagnostics.
    pub fn extend_diagnostics(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    /// True when an error diagnostic was recorded.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        crate::validation::has_errors(&self.diagnostics)
    }

    /// The paths added so far.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// Finishes the plan: files sorted by path, diagnostics in the order they were recorded.
    #[must_use]
    pub fn build(mut self) -> WritePlan {
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        WritePlan {
            files: self.files,
            diagnostics: self.diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::types::FileKind;
    use rimstudio_core::tree::Node;
    use rstest::rstest;

    #[rstest]
    #[case("Defs/Weapons/A.xml", true)]
    #[case("1.6/Defs/A.xml", true)]
    #[case("A.xml", true)]
    #[case("", false)]
    #[case("/etc/passwd", false)]
    #[case("../x.xml", false)]
    #[case("a/../x.xml", false)]
    #[case("a//x.xml", false)]
    #[case("a/./x.xml", false)]
    #[case("a\\x.xml", false)]
    #[case("C:/x.xml", false)]
    #[case("a/", false)]
    fn path_safety(#[case] path: &str, #[case] safe: bool) {
        assert_eq!(is_safe_relative_path(path), safe, "{path}");
    }

    fn file(path: &str) -> PlannedFile {
        PlannedFile::new_file(path, FileKind::VanillaDefs, Node::new("Defs"), vec![])
    }

    #[test]
    fn builder_sorts_files_and_rejects_bad_paths() {
        let mut b = PlanBuilder::new();
        assert!(b.add_file(file("b.xml")));
        assert!(b.add_file(file("a.xml")));
        assert!(!b.add_file(file("a.xml")));
        assert!(!b.add_file(file("../escape.xml")));
        assert!(b.has_errors());
        let plan = b.build();
        assert_eq!(plan.paths(), ["a.xml", "b.xml"]);
        assert_eq!(plan.diagnostics.len(), 2);
        assert!(plan.has_errors());
    }
}
