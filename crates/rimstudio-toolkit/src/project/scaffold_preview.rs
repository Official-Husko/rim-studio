//! `project_scaffold_preview`: what creating a new mod would write, and what is wrong with the values.
//!
//! A pure function over the scaffold plan of `rimstudio-workspace` and the findings about the package id of
//! [`super::about_lint`] (the game's format rule, the hints, and the collision with another mod of the last
//! library scan). The caller says which files exist; nothing is read or written here.

use camino::Utf8Path;
use rimstudio_core::diag::Diagnostic;
use rimstudio_ipc_types::diagnostic::{FIELD_ARG, diagnostics_to_dtos};
use rimstudio_ipc_types::project_new::{
    ProjectScaffoldPreviewDto, ScaffoldEntryDto, ScaffoldEntryKindDto,
};
use rimstudio_workspace::scaffold::{PlanItem, ScaffoldSpec, plan as scaffold_plan};

use super::about_lint::{LintContext, package_id_findings};

/// The preview of a spec. `exists` answers whether an absolute path exists on disk. The findings of the
/// scaffold (empty name, bad package id, bad versions, relative path) come first, then the package id
/// hints and the collision with the library.
#[must_use]
pub fn preview(
    spec: &ScaffoldSpec,
    ctx: &LintContext<'_>,
    exists: &dyn Fn(&Utf8Path) -> bool,
) -> ProjectScaffoldPreviewDto {
    let plan = scaffold_plan(spec);
    let id = spec.package_id.trim();
    // the scaffold names a malformed package id with the parser's term; the finding of the About lint says
    // the same in the game's words, so that one is shown instead, with the hints and the collision
    let id_findings = if id.is_empty() {
        Vec::new()
    } else {
        package_id_findings(id, ctx)
    };
    let id_reported = id_findings
        .iter()
        .any(|d| d.code.as_str() == "about.package-id-format");
    let mut diagnostics: Vec<Diagnostic> = plan
        .problems
        .iter()
        .filter(|d| {
            !(id_reported && d.args.get(FIELD_ARG).map(String::as_str) == Some("packageId"))
        })
        .cloned()
        .collect();
    diagnostics.extend(id_findings);
    let valid = plan.is_valid();
    let entries = plan
        .items
        .iter()
        .map(|item| ScaffoldEntryDto {
            path: item.path().to_owned(),
            kind: match item {
                PlanItem::Dir { .. } => ScaffoldEntryKindDto::Folder,
                PlanItem::File { .. } => ScaffoldEntryKindDto::File,
            },
        })
        .collect();
    let conflicts = if valid {
        plan.conflicts(exists)
            .into_iter()
            .filter_map(|p| {
                p.strip_prefix(&plan.root)
                    .ok()
                    .map(|rel| rel.as_str().to_owned())
            })
            .collect()
    } else {
        Vec::new()
    };
    ProjectScaffoldPreviewDto {
        root: plan.root.as_str().to_owned(),
        valid,
        diagnostics: diagnostics_to_dtos(&diagnostics),
        entries,
        target_exists: exists(&plan.root),
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::about_lint::PreviewFacts;
    use camino::Utf8PathBuf;
    use rimstudio_ipc_types::project_new::ScaffoldEntryKindDto;

    fn lint<'a>(facts: &'a PreviewFacts) -> LintContext<'a> {
        LintContext {
            library: None,
            game_version: None,
            preview: facts,
            icon_exists: &|_| false,
        }
    }

    fn spec(id: &str) -> ScaffoldSpec {
        ScaffoldSpec::new(Utf8PathBuf::from("/work/Test Mod"), "Test Mod", id)
    }

    #[test]
    fn a_default_spec_lists_its_entries_and_has_no_findings() {
        let facts = PreviewFacts::default();
        let out = preview(&spec("test.mod"), &lint(&facts), &|_| false);
        assert!(out.valid);
        assert!(
            out.diagnostics
                .iter()
                .all(|d| d.code != "about.package-id-format")
        );
        assert!(
            out.entries
                .iter()
                .any(|e| e.path == "About/About.xml" && e.kind == ScaffoldEntryKindDto::File)
        );
        assert!(
            out.entries
                .iter()
                .any(|e| e.path == "Defs" && e.kind == ScaffoldEntryKindDto::Folder)
        );
        assert!(!out.target_exists);
        assert!(out.conflicts.is_empty());
    }

    #[test]
    fn a_bad_package_id_is_an_error_and_lists_no_entries() {
        let facts = PreviewFacts::default();
        let out = preview(&spec("nodots"), &lint(&facts), &|_| false);
        assert!(!out.valid);
        assert!(out.entries.is_empty());
        assert!(out.diagnostics.iter().any(|d| d.field.is_some()));
    }

    #[test]
    fn capital_letters_are_a_hint_and_existing_files_are_conflicts() {
        let facts = PreviewFacts::default();
        let out = preview(&spec("Test.Mod"), &lint(&facts), &|p| {
            p.as_str() == "/work/Test Mod" || p.as_str().ends_with("About/About.xml")
        });
        assert!(out.valid);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == "about.package-id-case")
        );
        assert!(out.target_exists);
        assert_eq!(out.conflicts, vec!["About/About.xml".to_owned()]);
    }
}
