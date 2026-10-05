//! `designer_lint_files`: the Combat Extended lint over the patch files of a project, hand written or
//! generated.
//!
//! The files are read through the guarded project reader (a safe relative path inside the project, a size
//! limit) and parsed with `rimstudio-xml` in game mode, so a file the game would skip is a finding and not a
//! failure: a file that is not XML, has a document type declaration or is too large each give one finding of
//! rule CEP017 and the run goes on. The structural rules run on the raw nodes. The rules that need Combat
//! Extended data run when the reference set has it; otherwise the lint lists them as not checked, with the
//! reason, and the call still succeeds.
//!
//! Without an explicit path list the call checks every XML file below a `Patches` folder (at any depth, so
//! version folders and the gated folder count) and every file of the gated Combat Extended folder whose root
//! element is `Patch`.

use std::collections::BTreeSet;

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_design::ce::lint::{LintContext, LintFile, rule_id, run_files};
use rimstudio_design::ce::reader::CeModel;
use rimstudio_ipc_types::designer::{
    DesignerLintFilesRequest, DesignerLintFilesResult, LintCountsDto, LintFileStatusDto,
    LintFindingDto, LintNotCheckedDto, LintedFileDto,
};
use rimstudio_ipc_types::diagnostic::{FIELD_ARG, SeverityDto, diagnostics_to_dtos};
use rimstudio_xml::error::XmlError;
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_document;

use super::convert::project_defs;
use super::ctx::Ctx;
use super::lint_explain::explain;
use crate::error::ToolkitResult;
use crate::shared::projectfs::ProjectView;
use crate::shared::writer::GuardedWriter;

/// The largest file the lint reads, in bytes. A larger file gives one finding and is not read.
pub const MAX_FILE_BYTES: u64 = 2_000_000;
/// The most files one call checks.
pub const MAX_FILES: usize = 1000;
/// The code of the diagnostic that says the list of files was cut.
const CAPPED: &str = "designer.lint-files-capped";
/// The diagnostic code of the rules that could not run.
const NOT_CHECKED_CODE: &str = "ce.not-checked";

/// One file as it was read from disk.
enum Raw {
    /// The bytes of the file.
    Bytes(Vec<u8>),
    /// The file does not exist.
    Missing,
    /// The file is larger than [`MAX_FILE_BYTES`].
    TooLarge(u64),
    /// The path was refused or the read failed.
    Unreadable(String),
}

/// A file ready for the lint.
struct Prepared {
    path: String,
    status: LintFileStatusDto,
    bytes: u64,
    operations: u32,
    /// The parsed root element, kept to locate findings.
    root: Option<Node>,
    /// What the lint gets; `None` for a file that is no patch file.
    lint: Option<LintFile>,
}

fn clip(text: &str) -> String {
    text.chars().take(300).collect()
}

/// Cleans a requested path: back slashes become slashes, a leading `./` goes.
fn clean_path(path: &str) -> String {
    let mut p = path.trim().replace('\\', "/");
    while let Some(rest) = p.strip_prefix("./") {
        p = rest.to_owned();
    }
    p
}

fn read_raw(writer: &GuardedWriter, rel: &str) -> (Raw, u64) {
    match writer.read_limited(rel, usize::try_from(MAX_FILE_BYTES).unwrap_or(usize::MAX)) {
        Ok(None) => (Raw::Missing, 0),
        Ok(Some(read)) if read.truncated => (Raw::TooLarge(read.total), read.total),
        Ok(Some(read)) => {
            let total = read.total;
            (Raw::Bytes(read.bytes), total)
        }
        Err(e) => (Raw::Unreadable(clip(&e.to_string())), 0),
    }
}

/// Parses a file read from disk and decides how the lint sees it.
fn prepare(path: &str, raw: Raw, total: u64, gated_only: bool) -> Prepared {
    let failed = |status, reason: String| Prepared {
        path: path.to_owned(),
        status,
        bytes: total,
        operations: 0,
        root: None,
        lint: Some(LintFile::failed(Some(path.to_owned()), reason)),
    };
    let bytes = match raw {
        Raw::Missing => {
            return failed(
                LintFileStatusDto::Missing,
                "the file does not exist in the project".to_owned(),
            );
        }
        Raw::TooLarge(size) => {
            return failed(
                LintFileStatusDto::TooLarge,
                format!("the file has {size} bytes, more than the {MAX_FILE_BYTES} the lint reads"),
            );
        }
        Raw::Unreadable(reason) => {
            return failed(
                LintFileStatusDto::Unreadable,
                format!("the file cannot be read: {reason}"),
            );
        }
        Raw::Bytes(bytes) => bytes,
    };
    match parse_document(&bytes, ParseMode::Game) {
        Ok(doc) => {
            if gated_only && doc.root.tag != "Patch" {
                return Prepared {
                    path: path.to_owned(),
                    status: LintFileStatusDto::NotAPatch,
                    bytes: total,
                    operations: 0,
                    root: None,
                    lint: None,
                };
            }
            let operations = doc.root.elements().filter(|n| n.tag == "Operation").count();
            Prepared {
                path: path.to_owned(),
                status: LintFileStatusDto::Checked,
                bytes: total,
                operations: u32::try_from(operations).unwrap_or(u32::MAX),
                lint: Some(LintFile::parsed(Some(path.to_owned()), doc.root.clone())),
                root: Some(doc.root),
            }
        }
        Err(XmlError::DtdRejected { line }) => failed(
            LintFileStatusDto::Doctype,
            format!(
                "the file has a document type declaration (line {line}), which the game refuses"
            ),
        ),
        Err(e) => failed(LintFileStatusDto::ParseFailed, clip(&e.to_string())),
    }
}

/// The one based number after the first `Operation` segment of a pointer, and the xpath of the innermost
/// element on the way that has an `xpath` child.
fn locate(root: &Node, pointer: &str) -> (Option<u32>, Option<String>) {
    let mut node = root;
    let mut operation = None;
    let mut xpath = node.child("xpath").map(Node::text_content);
    for (depth, segment) in pointer.split('/').filter(|s| !s.is_empty()).enumerate() {
        if depth == 0 {
            continue;
        }
        let Some((tag, rest)) = segment.split_once('[') else {
            break;
        };
        let Some(n) = rest
            .trim_end_matches(']')
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
        else {
            break;
        };
        let Some(next) = node.elements().filter(|e| e.tag == tag).nth(n - 1) else {
            break;
        };
        if depth == 1 && tag == "Operation" {
            operation = u32::try_from(n).ok();
        }
        node = next;
        if let Some(x) = node.child("xpath") {
            xpath = Some(x.text_content());
        }
    }
    let xpath = xpath.map(|x| x.trim().to_owned()).filter(|x| !x.is_empty());
    (operation, xpath)
}

fn finding_of(d: &Diagnostic, file: Option<&str>, root: Option<&Node>) -> LintFindingDto {
    let rule = d
        .args
        .get("ruleId")
        .cloned()
        .or_else(|| rule_id(d.code.as_str()));
    let field = d.args.get(FIELD_ARG).filter(|f| !f.is_empty()).cloned();
    let (operation, xpath) = match (root, field.as_deref()) {
        (Some(root), Some(pointer)) => locate(root, pointer),
        _ => (None, None),
    };
    LintFindingDto {
        explanation: rule.as_deref().and_then(explain).map(str::to_owned),
        rule_id: rule,
        code: d.code.as_str().to_owned(),
        severity: SeverityDto::from(d.severity),
        message: d.message.clone(),
        field,
        operation,
        xpath,
        file: file.map(str::to_owned),
    }
}

fn count_findings(counts: &mut LintCountsDto, findings: &[LintFindingDto]) {
    for f in findings {
        match f.severity {
            SeverityDto::Error => counts.errors = counts.errors.saturating_add(1),
            SeverityDto::Warning => counts.warnings = counts.warnings.saturating_add(1),
            SeverityDto::Info | SeverityDto::Hint => counts.notes = counts.notes.saturating_add(1),
        }
    }
}

/// Everything the lint needs besides the files.
pub struct LintSetup<'a> {
    /// The Combat Extended model; an absent model skips the data dependent rules.
    pub model: &'a CeModel,
    /// The lint context without the paths: `LoadFolders.xml`, game version, known defs.
    pub context: LintContext,
}

/// Lints files that are already read. `files` are `(path, raw)` pairs; `gated_only` marks the paths whose
/// non patch files are skipped silently instead of reported.
fn run_prepared(
    prepared: Vec<Prepared>,
    setup: &LintSetup<'_>,
) -> (
    Vec<LintedFileDto>,
    Vec<LintFindingDto>,
    Vec<LintNotCheckedDto>,
    LintCountsDto,
) {
    let lint_files: Vec<LintFile> = prepared.iter().filter_map(|p| p.lint.clone()).collect();
    let mut ctx = setup.context.clone();
    ctx.paths = lint_files.iter().filter_map(|f| f.path.clone()).collect();
    let diagnostics = run_files(&lint_files, setup.model, &ctx);

    let mut counts = LintCountsDto::default();
    let mut not_checked = Vec::new();
    let mut project = Vec::new();
    let mut by_file: std::collections::BTreeMap<String, Vec<&Diagnostic>> =
        std::collections::BTreeMap::new();
    for d in &diagnostics {
        if d.code.as_str() == NOT_CHECKED_CODE {
            let rule = d.args.get("rule").cloned().unwrap_or_default();
            let reason = d.args.get("reason").cloned().unwrap_or_default();
            if !not_checked
                .iter()
                .any(|n: &LintNotCheckedDto| n.rule_id == rule)
            {
                not_checked.push(LintNotCheckedDto {
                    rule_id: rule,
                    reason,
                });
            }
            continue;
        }
        match d.args.get("path") {
            Some(path) => by_file.entry(path.clone()).or_default().push(d),
            None => project.push(finding_of(d, None, None)),
        }
    }
    count_findings(&mut counts, &project);

    let mut files = Vec::new();
    for p in prepared {
        let findings: Vec<LintFindingDto> = by_file
            .remove(&p.path)
            .unwrap_or_default()
            .into_iter()
            .map(|d| finding_of(d, Some(&p.path), p.root.as_ref()))
            .collect();
        count_findings(&mut counts, &findings);
        counts.files = counts.files.saturating_add(1);
        if p.status == LintFileStatusDto::Checked {
            counts.checked = counts.checked.saturating_add(1);
        }
        files.push(LintedFileDto {
            path: p.path,
            status: p.status,
            bytes: p.bytes,
            operations: p.operations,
            findings,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    (files, project, not_checked, counts)
}

/// The folders the walk for patch files does not enter: links are never followed either.
const NOT_ENTERED: [&str; 8] = [
    ".git",
    ".vs",
    "node_modules",
    "target",
    "Textures",
    "Sounds",
    "Assemblies",
    "Source",
];
/// The deepest folder level the walk descends to.
const MAX_DEPTH: usize = 16;

fn has_segment(rel: &str, name: &str) -> bool {
    rel.split('/').any(|s| s.eq_ignore_ascii_case(name))
}

fn walk(dir: &std::path::Path, rel: &str, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if kind.is_dir() {
            if !NOT_ENTERED.contains(&name.as_str()) {
                walk(&entry.path(), &child, depth + 1, out);
            }
        } else if kind.is_file() && name.to_ascii_lowercase().ends_with(".xml") {
            out.push(child);
        }
    }
}

/// The files a call without paths checks, each with the flag that marks a file of the gated folder only
/// (a file there that is no patch is skipped without a finding), and whether the list was cut.
fn default_paths(view: &ProjectView) -> (Vec<(String, bool)>, bool) {
    let mut all = Vec::new();
    walk(view.root.as_std_path(), "", 0, &mut all);
    let gated = view.layout.ce_dir().to_ascii_lowercase();
    let mut out: Vec<(String, bool)> = all
        .into_iter()
        .filter_map(|rel| {
            if has_segment(&rel, "Patches") {
                return Some((rel, false));
            }
            let lower = rel.to_ascii_lowercase();
            let in_gated = lower.starts_with(&format!("{gated}/"));
            // Definition files of the gated folder are not patch files.
            (in_gated && !has_segment(&rel, "Defs")).then_some((rel, true))
        })
        .collect();
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    let capped = out.len() > MAX_FILES;
    out.truncate(MAX_FILES);
    (out, capped)
}

/// The def names the rules may treat as existing: the thing defs of the reference set and every def of the
/// project.
fn known_defs(ctx: &Ctx, view: &ProjectView) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    if let Some(engine) = ctx.engine() {
        if let Ok(db) = engine.databases().database("ThingDef") {
            known.extend(db.names().into_iter().map(str::to_owned));
        }
        if let Ok(session) = ctx.project_session(&view.root) {
            let defs = project_defs(&session, view);
            for node in &defs.nodes {
                if let Some(name) = node.child_text("defName") {
                    known.insert(name.to_owned());
                }
            }
        }
    }
    known
}

/// `designer_lint_files`: checks the patch files of a project. Reads the project, writes nothing.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`] for an unknown project and [`ToolkitError::ProjectInvalid`] when the
/// folder is no longer a mod. A problem with a file is a finding, never an error.
pub fn lint_files(
    ctx: &Ctx,
    req: &DesignerLintFilesRequest,
) -> ToolkitResult<DesignerLintFilesResult> {
    let (record, view) = ctx.env().view(&req.project_id)?;
    let writer = ctx.writer_for(record.id.as_str(), &view.root)?;
    let mut diagnostics: Vec<Diagnostic> = view.diagnostics.clone();

    let (wanted, capped): (Vec<(String, bool)>, bool) = if req.paths.is_empty() {
        default_paths(&view)
    } else {
        let mut seen = BTreeSet::new();
        let list: Vec<(String, bool)> = req
            .paths
            .iter()
            .map(|p| clean_path(p))
            .filter(|p| !p.is_empty() && seen.insert(p.clone()))
            .map(|p| (p, false))
            .collect();
        let capped = list.len() > MAX_FILES;
        (list.into_iter().take(MAX_FILES).collect(), capped)
    };
    if capped {
        diagnostics.push(Diagnostic::new(
            rimstudio_core::diag::DiagCode::new(CAPPED),
            Severity::Warning,
            format!("only the first {MAX_FILES} files were checked"),
        ));
    }

    let prepared: Vec<Prepared> = wanted
        .iter()
        .map(|(path, gated_only)| {
            let (raw, total) = read_raw(&writer, path);
            prepare(path, raw, total, *gated_only)
        })
        .collect();

    let engine = ctx.engine().filter(|e| e.ce_available());
    let absent;
    let (model, ce_data): (&CeModel, bool) = match &engine {
        Some(e) => (e.ce(), true),
        None => {
            absent = CeModel::absent("no Combat Extended data is loaded");
            (&absent, false)
        }
    };
    let game_version = {
        let fallback = ctx
            .session()
            .map(|s| {
                let v = &s.reference().game_version;
                format!("{}.{}", v.major, v.minor)
            })
            .unwrap_or_else(|| "1.6".to_owned());
        view.game_version(&fallback)
    };
    let setup = LintSetup {
        model,
        context: LintContext {
            paths: Vec::new(),
            load_folders: view.load_folders.as_ref().map(|f| f.root.clone()),
            game_version: game_version.clone(),
            known_defs: known_defs(ctx, &view),
            ce_types: None,
            known_fields: None,
        },
    };
    let (files, project, not_checked, counts) = run_prepared(prepared, &setup);
    Ok(DesignerLintFilesResult {
        files,
        project,
        not_checked,
        ce_data,
        game_version,
        counts,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    })
}

/// Lints files given as bytes, without a project or a session. Used by tests and by callers that already
/// hold the bytes; `paths` are the names shown in the findings.
#[must_use]
pub fn lint_bytes(
    files: &[(String, Vec<u8>)],
    setup: &LintSetup<'_>,
) -> (
    Vec<LintedFileDto>,
    Vec<LintFindingDto>,
    Vec<LintNotCheckedDto>,
    LintCountsDto,
) {
    let prepared = files
        .iter()
        .map(|(path, bytes)| {
            let total = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            let raw = if total > MAX_FILE_BYTES {
                Raw::TooLarge(total)
            } else {
                Raw::Bytes(bytes.clone())
            };
            prepare(path, raw, total, false)
        })
        .collect();
    run_prepared(prepared, setup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(model: &CeModel) -> LintSetup<'_> {
        LintSetup {
            model,
            context: LintContext::default(),
        }
    }

    fn patch(body: &str) -> Vec<u8> {
        format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Patch>{body}</Patch>").into_bytes()
    }

    #[test]
    fn locate_finds_the_operation_and_its_xpath() {
        let doc = parse_document(
            &patch(
                "<Operation Class=\"PatchOperationSequence\"><operations><li Class=\"PatchOperationAdd\">\
                 <xpath>Defs/ThingDef[defName=\"RS_A\"]</xpath><value><x/></value></li></operations></Operation>\
                 <Operation Class=\"PatchOperationAdd\"><xpath>Defs/ThingDef[defName=\"RS_B\"]</xpath></Operation>",
            ),
            ParseMode::Game,
        )
        .unwrap();
        let (op, xpath) = locate(
            &doc.root,
            "/Patch/Operation[1]/operations[1]/li[1]/value[1]",
        );
        assert_eq!(op, Some(1));
        assert_eq!(xpath.as_deref(), Some("Defs/ThingDef[defName=\"RS_A\"]"));
        let (op, xpath) = locate(&doc.root, "/Patch/Operation[2]");
        assert_eq!(op, Some(2));
        assert_eq!(xpath.as_deref(), Some("Defs/ThingDef[defName=\"RS_B\"]"));
        assert_eq!(locate(&doc.root, "/Patch/Operation[9]/x[1]").0, None);
        assert_eq!(locate(&doc.root, "").0, None);
        assert_eq!(locate(&doc.root, "/Patch/Operation[0]").0, None);
    }

    #[test]
    fn clean_path_normalises_separators() {
        assert_eq!(clean_path(".\\Patches\\a.xml"), "Patches/a.xml");
        assert_eq!(clean_path("  ./././x.xml "), "x.xml");
    }

    #[test]
    fn a_file_that_is_not_xml_is_a_finding() {
        let model = CeModel::absent("test");
        let (files, _, not_checked, counts) = lint_bytes(
            &[(
                "Patches/a.xml".to_owned(),
                b"this is not xml at all".to_vec(),
            )],
            &setup(&model),
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, LintFileStatusDto::ParseFailed);
        assert!(
            files[0]
                .findings
                .iter()
                .any(|f| f.rule_id.as_deref() == Some("CEP017"))
        );
        assert!(counts.errors >= 1);
        assert!(!not_checked.is_empty());
    }

    #[test]
    fn a_doctype_is_refused_like_the_game_does() {
        let model = CeModel::absent("test");
        let bytes = b"<?xml version=\"1.0\"?><!DOCTYPE Patch [<!ENTITY a \"b\">]><Patch/>".to_vec();
        let (files, ..) = lint_bytes(&[("Patches/a.xml".to_owned(), bytes)], &setup(&model));
        assert_eq!(files[0].status, LintFileStatusDto::Doctype);
        assert!(
            files[0]
                .findings
                .iter()
                .any(|f| f.severity == SeverityDto::Error)
        );
    }

    #[test]
    fn a_huge_file_is_not_read() {
        let model = CeModel::absent("test");
        let bytes = vec![b' '; usize::try_from(MAX_FILE_BYTES).unwrap_or(0) + 1];
        let (files, ..) = lint_bytes(&[("Patches/big.xml".to_owned(), bytes)], &setup(&model));
        assert_eq!(files[0].status, LintFileStatusDto::TooLarge);
        assert_eq!(files[0].operations, 0);
    }

    #[test]
    fn a_wrong_root_is_a_finding_with_the_rule_explanation() {
        let model = CeModel::absent("test");
        let (files, ..) = lint_bytes(
            &[("Patches/a.xml".to_owned(), b"<Defs/>".to_vec())],
            &setup(&model),
        );
        let finding = files[0].findings.first().unwrap();
        assert_eq!(finding.rule_id.as_deref(), Some("CEP017"));
        assert!(finding.explanation.is_some());
        assert_eq!(finding.file.as_deref(), Some("Patches/a.xml"));
    }

    #[test]
    fn findings_carry_the_operation_index_and_the_xpath() {
        let model = CeModel::absent("test");
        let body = "<Operation Class=\"PatchOperationFindMod\"><mods><li>ceteam.combatextended</li></mods>\
                    <match Class=\"PatchOperationAdd\"><xpath>Defs/ThingDef[defName=\"RS_A\"]</xpath>\
                    <value><x/></value></match></Operation>";
        let (files, ..) = lint_bytes(&[("Patches/a.xml".to_owned(), patch(body))], &setup(&model));
        let f = files[0]
            .findings
            .iter()
            .find(|f| f.rule_id.as_deref() == Some("CEP001"))
            .unwrap();
        assert_eq!(f.operation, Some(1));
        assert!(
            f.field
                .as_deref()
                .unwrap_or("")
                .starts_with("/Patch/Operation[1]")
        );
    }

    #[test]
    fn rules_that_need_data_are_listed_as_not_checked_without_it() {
        let model = CeModel::absent("test");
        let (_, _, not_checked, _) =
            lint_bytes(&[("Patches/a.xml".to_owned(), patch(""))], &setup(&model));
        let ids: Vec<&str> = not_checked.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"CEP013"));
        assert!(not_checked.iter().all(|n| !n.reason.is_empty()));
    }
}
