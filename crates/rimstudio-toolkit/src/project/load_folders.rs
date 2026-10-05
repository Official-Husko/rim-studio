//! `LoadFolders.xml` as a form: `project_load_folders_get` and `project_load_folders_update`.
//!
//! The file says which folders of a mod the game loads for which game version and with which other mods
//! active. The model lists the blocks and entries by position (the game merges repeated blocks, so a block's
//! name is not an address) with the conditions and whether each folder exists. The update applies small
//! changes (add or remove a block, add, remove, move or change an entry) as byte span edits through
//! [`rimstudio_xml::load_folders_edit::LoadFoldersEditor`], so comments and layout stay, and writes through
//! the guarded writer with a backup, an atomic write and a read back. A file that changed since it was read
//! is refused. Findings never block a save.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::load_plan::{DEFAULT_BLOCK_KEY, LoadEntry};
use rimstudio_core::paths;
use rimstudio_core::version::{normalize_version_key, parse_major_minor};
use rimstudio_ipc_types::diagnostic::{FIELD_ARG, diagnostics_to_dtos};
use rimstudio_ipc_types::project_about::{
    LoadBlockDto, LoadEntryDto, LoadEntryInputDto, LoadFoldersChangeDto, ProjectLoadFoldersDto,
    ProjectLoadFoldersGetRequest, ProjectLoadFoldersUpdateDto, ProjectLoadFoldersUpdateRequest,
};
use rimstudio_workspace::project::{find_about_file, find_load_folders_file};
use rimstudio_xml::XmlError;
use rimstudio_xml::about::read_lenient;
use rimstudio_xml::edit::SpanEditor;
use rimstudio_xml::load_folders;
use rimstudio_xml::load_folders_edit::{EntryPatch, LoadFoldersEditor};
use rimstudio_xml::render::RenderOpts;

use super::about::{MAX_ABOUT_BYTES, RAW_TEXT_LIMIT};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::diff::unified_diff;
use crate::shared::env::ProjectEnv;
use crate::shared::writer::{Expect, GuardedWriter};

/// `loadfolders.unparseable`
pub const UNPARSEABLE: DiagCode = DiagCode::new("loadfolders.unparseable");
/// `loadfolders.folder-missing`
pub const FOLDER_MISSING: DiagCode = DiagCode::new("loadfolders.folder-missing");
/// `loadfolders.unsupported-version`
pub const UNSUPPORTED_VERSION: DiagCode = DiagCode::new("loadfolders.unsupported-version");
/// `loadfolders.ungated-compat-folder`
pub const UNGATED_COMPAT: DiagCode = DiagCode::new("loadfolders.ungated-compat-folder");
/// `loadfolders.ignored-attribute`
pub const IGNORED_ATTRIBUTE: DiagCode = DiagCode::new("loadfolders.ignored-attribute");
/// `loadfolders.empty-block`
pub const EMPTY_BLOCK: DiagCode = DiagCode::new("loadfolders.empty-block");
/// `loadfolders.duplicate-block`
pub const DUPLICATE_BLOCK: DiagCode = DiagCode::new("loadfolders.duplicate-block");
/// `loadfolders.too-large`
pub const TOO_LARGE: DiagCode = DiagCode::new("loadfolders.too-large");
/// `loadfolders.not-utf8`
pub const NOT_UTF8: DiagCode = DiagCode::new("loadfolders.not-utf8");

/// The most changes one request may carry.
pub const MAX_CHANGES: usize = 500;

/// The file as read from the project.
pub(crate) struct LoadedFile {
    pub(crate) rel: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) total: u64,
    pub(crate) truncated: bool,
    pub(crate) hash: String,
}

fn invalid(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::EditInvalid {
        reason: reason.into(),
    }
}

fn xml_error(e: XmlError) -> ToolkitError {
    match e {
        XmlError::EditPathMissing { path } => invalid(format!(
            "the file has no such block or entry ({path}); read the file again"
        )),
        XmlError::EditUnsupported { path, reason } => {
            invalid(format!("the entry at {path} cannot be edited: {reason}"))
        }
        XmlError::InvalidValue { what, reason } => {
            invalid(format!("{what} cannot be written: {reason}"))
        }
        other => ToolkitError::Xml(other),
    }
}

/// Reads `LoadFolders.xml` of the project when it exists.
pub(crate) fn load(writer: &GuardedWriter, root: &Utf8Path) -> ToolkitResult<Option<LoadedFile>> {
    let Some(path) = find_load_folders_file(root) else {
        return Ok(None);
    };
    let rel = path
        .strip_prefix(root)
        .map_err(|_| ToolkitError::ProjectInvalid {
            path: path.to_string(),
            reason: "LoadFolders.xml is outside the project".to_owned(),
        })?
        .as_str()
        .replace('\\', "/");
    let Some(read) = writer.read_limited(&rel, MAX_ABOUT_BYTES)? else {
        return Ok(None);
    };
    let hash = if read.truncated {
        writer
            .existing_file_hash(&rel)
            .ok()
            .flatten()
            .map_or_else(|| rimstudio_io::sha256::sha256_hex(&read.bytes), |(_, h)| h)
    } else {
        rimstudio_io::sha256::sha256_hex(&read.bytes)
    };
    Ok(Some(LoadedFile {
        rel,
        total: read.total,
        truncated: read.truncated,
        bytes: read.bytes,
        hash,
    }))
}

fn ids(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One block as the file has it, before the game merges repeated blocks.
struct RawBlock {
    index: usize,
    tag: String,
    entries: Vec<RawEntry>,
}

struct RawEntry {
    index: usize,
    path: String,
    if_active: Vec<String>,
    if_active_all: Vec<String>,
    if_not_active: Vec<String>,
    ignored: Vec<String>,
}

fn raw_blocks(text: &str) -> Option<Vec<RawBlock>> {
    let ed = SpanEditor::open(text).ok()?;
    let root = format!("/{}", ed.root_tag());
    let mut out = Vec::new();
    for b in ed.children(&root).ok()? {
        let mut entries = Vec::new();
        for e in ed.children(&b.path).ok()? {
            let info = ed.element(&e.path).ok()?;
            let mut entry = RawEntry {
                index: e.index,
                path: match info.text.as_str() {
                    "/" | "\\" => String::new(),
                    t if t.trim().is_empty() => String::new(),
                    t => t.to_owned(),
                },
                if_active: Vec::new(),
                if_active_all: Vec::new(),
                if_not_active: Vec::new(),
                ignored: Vec::new(),
            };
            for (k, v) in &info.attrs {
                match k.as_str() {
                    "IfModActive" => entry.if_active = ids(v),
                    "IfModActiveAll" => entry.if_active_all = ids(v),
                    "IfModNotActive" => entry.if_not_active = ids(v),
                    other => entry.ignored.push(other.to_owned()),
                }
            }
            entries.push(entry);
        }
        out.push(RawBlock {
            index: b.index,
            tag: b.tag,
            entries,
        });
    }
    Some(out)
}

/// True when `folder` (relative to the mod root, as the game reads it) exists.
fn folder_exists(root: &Utf8Path, folder: &str) -> bool {
    if folder.is_empty() {
        return true;
    }
    let rel = folder.replace('\\', "/");
    let rel = rel.trim_end_matches('/');
    if rel.starts_with('/') || rel.split('/').any(|s| s == "..") {
        return false;
    }
    root.join(rel).as_std_path().is_dir()
}

/// The folders of the mod root that are named like a game version.
pub(crate) fn version_folders(root: &Utf8Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root.as_std_path())
        .map(|r| {
            r.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|n| parse_major_minor(n, false).is_some())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// The `supportedVersions` of the About file of the project, empty when it cannot be read.
pub(crate) fn supported_versions(root: &Utf8Path) -> Vec<String> {
    find_about_file(root)
        .and_then(|p| std::fs::read(p.as_std_path()).ok())
        .map(|b| read_lenient(&b).about.supported_versions)
        .unwrap_or_default()
}

fn looks_like_compat(path: &str) -> bool {
    let norm = path.replace('\\', "/");
    let lower = norm.trim_matches('/').to_lowercase();
    lower
        .split('/')
        .any(|s| s == "compat" || s == "ce" || s == "combatextended" || s == "modpatches")
}

fn pointer(block: usize, entry: usize) -> String {
    format!("/blocks/{block}/entries/{entry}/path")
}

fn finding(code: DiagCode, sev: Severity, field: &str, message: impl Into<String>) -> Diagnostic {
    let d = Diagnostic::new(code, sev, message);
    if field.is_empty() {
        d
    } else {
        d.with_arg(FIELD_ARG, field)
    }
}

fn build_dto(
    project_id: &str,
    root: &Utf8Path,
    rel: &str,
    text_bytes: Option<&LoadedFile>,
    text: Option<&str>,
) -> ProjectLoadFoldersDto {
    let supported = supported_versions(root);
    let mut diagnostics = Vec::new();
    let mut reason: Option<String> = None;
    let mut blocks_raw = Vec::new();
    if let Some(file) = text_bytes {
        if file.truncated {
            reason = Some("the file is larger than 1 MiB".to_owned());
            diagnostics.push(finding(
                TOO_LARGE,
                Severity::Error,
                "",
                "LoadFolders.xml is larger than 1 MiB, so it is shown as text only",
            ));
        } else if text.is_none() {
            reason = Some("the file is not UTF-8 text".to_owned());
            diagnostics.push(finding(
                NOT_UTF8,
                Severity::Warning,
                "",
                "LoadFolders.xml is not UTF-8; it is shown but not edited, so no byte is rewritten",
            ));
        } else if let Some(t) = text {
            match raw_blocks(t) {
                Some(b) => blocks_raw = b,
                None => {
                    reason = Some("the file is not well formed XML".to_owned());
                    diagnostics.push(finding(
                        UNPARSEABLE,
                        Severity::Error,
                        "",
                        "LoadFolders.xml is not well formed XML; the game ignores it. Fix the text first.",
                    ));
                }
            }
        }
    }
    let mut seen_keys: Vec<String> = Vec::new();
    let mut blocks = Vec::new();
    for (bi, b) in blocks_raw.iter().enumerate() {
        let key = normalize_version_key(&b.tag);
        if seen_keys.contains(&key) {
            diagnostics.push(finding(
                DUPLICATE_BLOCK,
                Severity::Warning,
                &format!("/blocks/{bi}"),
                format!(
                    "there is more than one block for {key}; the game merges them into one list"
                ),
            ));
        }
        seen_keys.push(key.clone());
        if key != DEFAULT_BLOCK_KEY
            && !supported.is_empty()
            && let Some(mm) = parse_major_minor(&key, false)
            && !supported
                .iter()
                .any(|s| parse_major_minor(s, true) == Some(mm))
        {
            diagnostics.push(finding(
                UNSUPPORTED_VERSION,
                Severity::Warning,
                &format!("/blocks/{bi}"),
                format!("the block {} is for a version that supportedVersions of About.xml does not list", b.tag),
            ));
        }
        if b.entries.is_empty() {
            diagnostics.push(finding(
                EMPTY_BLOCK,
                Severity::Info,
                &format!("/blocks/{bi}"),
                format!(
                    "the block {} has no entries; the game then loads nothing for that version",
                    b.tag
                ),
            ));
        }
        let mut entries = Vec::new();
        for (ei, e) in b.entries.iter().enumerate() {
            let exists = folder_exists(root, &e.path);
            if !exists {
                diagnostics.push(
                    finding(
                        FOLDER_MISSING,
                        Severity::Warning,
                        &pointer(bi, ei),
                        format!("the folder {} does not exist in the mod", e.path),
                    )
                    .with_arg("folder", e.path.clone()),
                );
            }
            for attr in &e.ignored {
                diagnostics.push(finding(
                    IGNORED_ATTRIBUTE,
                    Severity::Warning,
                    &pointer(bi, ei),
                    format!("the attribute {attr} is not read by the game, so the folder {} loads always", e.path),
                ));
            }
            let gated = !e.if_active.is_empty() || !e.if_active_all.is_empty();
            if !gated && looks_like_compat(&e.path) {
                diagnostics.push(finding(
                    UNGATED_COMPAT,
                    Severity::Warning,
                    &pointer(bi, ei),
                    format!("the folder {} looks like a compatibility folder but loads without an IfModActive condition", e.path),
                ));
            }
            entries.push(LoadEntryDto {
                index: u32::try_from(e.index).unwrap_or(u32::MAX),
                path: e.path.clone(),
                if_mod_active: e.if_active.clone(),
                if_mod_active_all: e.if_active_all.clone(),
                if_mod_not_active: e.if_not_active.clone(),
                ignored_attributes: e.ignored.clone(),
                folder_exists: exists,
            });
        }
        blocks.push(LoadBlockDto {
            index: u32::try_from(b.index).unwrap_or(u32::MAX),
            tag: b.tag.clone(),
            key,
            entries,
        });
    }
    let (raw_text, raw_truncated) = match text_bytes {
        Some(f) => {
            let end = f.bytes.len().min(RAW_TEXT_LIMIT);
            let raw = String::from_utf8_lossy(f.bytes.get(..end).unwrap_or_default());
            (
                raw.trim_start_matches('\u{feff}').to_owned(),
                f.bytes.len() > RAW_TEXT_LIMIT || f.total > f.bytes.len() as u64,
            )
        }
        None => (String::new(), false),
    };
    diagnostics.sort_by_key(|d| match d.severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Hint => 3,
    });
    ProjectLoadFoldersDto {
        project_id: project_id.to_owned(),
        exists: text_bytes.is_some(),
        path: rel.to_owned(),
        file_hash: text_bytes.map(|f| f.hash.clone()),
        editable: reason.is_none(),
        not_editable_reason: reason,
        blocks,
        supported_versions: supported,
        version_folders: version_folders(root),
        raw_text,
        raw_truncated,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    }
}

/// `project_load_folders_get`: the blocks and entries of `LoadFolders.xml`, with the findings.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`] for an unknown project and path refusals from the guarded reader. A mod
/// without the file is a normal answer (`exists` is false).
pub fn get(
    env: &ProjectEnv,
    req: &ProjectLoadFoldersGetRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectLoadFoldersDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    match load(&writer, &record.path)? {
        None => Ok(build_dto(
            &req.project_id,
            &record.path,
            paths::LOAD_FOLDERS_XML,
            None,
            None,
        )),
        Some(file) => {
            let text = std::str::from_utf8(&file.bytes).ok().map(str::to_owned);
            Ok(build_dto(
                &req.project_id,
                &record.path,
                &file.rel,
                Some(&file),
                text.as_deref(),
            ))
        }
    }
}

fn position(n: u32) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

fn clean_ids(what: &str, list: &[String]) -> ToolkitResult<Vec<String>> {
    let mut out = Vec::new();
    for id in list {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        if id.contains(',') || id.chars().any(char::is_control) {
            return Err(invalid(format!(
                "{what} holds a comma or a control character"
            )));
        }
        out.push(id.to_owned());
    }
    Ok(out)
}

fn entry_of(input: &LoadEntryInputDto) -> ToolkitResult<LoadEntry> {
    Ok(LoadEntry {
        path: folder_text(&input.path)?,
        if_active: clean_ids("IfModActive", &input.if_mod_active)?,
        if_active_all: clean_ids("IfModActiveAll", &input.if_mod_active_all)?,
        if_not_active: clean_ids("IfModNotActive", &input.if_mod_not_active)?,
        ignored_attributes: Vec::new(),
    })
}

fn folder_text(path: &str) -> ToolkitResult<String> {
    if path.chars().any(char::is_control) {
        return Err(invalid("a folder holds a control character"));
    }
    let p = path.trim().replace('\\', "/");
    if p.split('/').any(|s| s == "..") || p.starts_with('/') && p != "/" {
        return Err(invalid(format!(
            "\"{path}\" is not a folder inside the mod"
        )));
    }
    Ok(if p == "/" { String::new() } else { p })
}

fn id_list(what: &str, list: &Option<Vec<String>>) -> ToolkitResult<Option<Vec<String>>> {
    list.as_deref().map(|l| clean_ids(what, l)).transpose()
}

/// Applies `changes` in order to the text of a `LoadFolders.xml`.
///
/// # Errors
///
/// [`ToolkitError::EditInvalid`] for a value that cannot be written or a block or entry the file does not
/// have, [`ToolkitError::Xml`] when the text is not a well formed document.
pub fn apply_changes(text: &str, changes: &[LoadFoldersChangeDto]) -> ToolkitResult<String> {
    if changes.len() > MAX_CHANGES {
        return Err(invalid(format!(
            "a request may carry at most {MAX_CHANGES} changes"
        )));
    }
    let mut ed = LoadFoldersEditor::new(text).map_err(xml_error)?;
    for change in changes {
        match change {
            LoadFoldersChangeDto::AddBlock {
                version,
                entries,
                at,
            } => {
                let key = version.trim();
                if key != DEFAULT_BLOCK_KEY
                    && (parse_major_minor(key, true).is_none()
                        || key.chars().any(char::is_whitespace))
                {
                    return Err(invalid(format!(
                        "\"{key}\" is not a game version; write it as major.minor, for example 1.6"
                    )));
                }
                let mut list = Vec::new();
                for e in entries {
                    list.push(entry_of(e)?);
                }
                ed.add_block(&normalize_version_key(key), &list, at.map(position))
                    .map_err(xml_error)?;
            }
            LoadFoldersChangeDto::RemoveBlock { block } => {
                ed.remove_block(position(*block)).map_err(xml_error)?;
            }
            LoadFoldersChangeDto::AddEntry { block, entry, at } => {
                ed.add_entry(position(*block), &entry_of(entry)?, at.map(position))
                    .map_err(xml_error)?;
            }
            LoadFoldersChangeDto::RemoveEntry { block, entry } => {
                ed.remove_entry(position(*block), position(*entry))
                    .map_err(xml_error)?;
            }
            LoadFoldersChangeDto::MoveEntry { block, entry, to } => {
                ed.move_entry(position(*block), position(*entry), position(*to))
                    .map_err(xml_error)?;
            }
            LoadFoldersChangeDto::SetEntry {
                block,
                entry,
                change,
            } => {
                let patch = EntryPatch {
                    path: change.path.as_deref().map(folder_text).transpose()?,
                    if_active: id_list("IfModActive", &change.if_mod_active)?,
                    if_active_all: id_list("IfModActiveAll", &change.if_mod_active_all)?,
                    if_not_active: id_list("IfModNotActive", &change.if_mod_not_active)?,
                    drop_ignored_attributes: change.drop_ignored_attributes,
                };
                ed.set_entry(position(*block), position(*entry), &patch)
                    .map_err(xml_error)?;
            }
        }
    }
    Ok(ed.into_text())
}

/// The text of a new, empty `LoadFolders.xml`.
#[must_use]
pub fn new_file_text() -> String {
    load_folders::create(
        &rimstudio_core::load_plan::LoadFoldersSpec::new(),
        &RenderOpts::default(),
    )
}

/// `project_load_folders_update`: applies the changes and writes the file (or only reports, with `dry_run`).
///
/// # Errors
///
/// [`ToolkitError::FileStale`] when the file differs from `expected_hash`, [`ToolkitError::FileNotEditable`]
/// for a file that is not well formed, too large or not UTF-8, [`ToolkitError::EditInvalid`] for a change
/// that does not apply or when the file does not exist and `create` is false, and store errors from the
/// write.
pub fn update(
    env: &ProjectEnv,
    req: &ProjectLoadFoldersUpdateRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectLoadFoldersUpdateDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let existing = load(&writer, &record.path)?;
    let (rel, old_text) = match &existing {
        Some(file) => {
            let not_editable = |reason: &str| ToolkitError::FileNotEditable {
                rel: file.rel.clone(),
                reason: reason.to_owned(),
            };
            if let Some(want) = &req.expected_hash
                && !want.eq_ignore_ascii_case(&file.hash)
            {
                return Err(ToolkitError::FileStale {
                    rel: file.rel.clone(),
                });
            }
            if file.truncated {
                return Err(not_editable("the file is larger than 1 MiB"));
            }
            let text = String::from_utf8(file.bytes.clone())
                .map_err(|_| not_editable("the file is not UTF-8 text"))?;
            (file.rel.clone(), Some(text))
        }
        None => {
            if req.expected_hash.is_some() {
                return Err(ToolkitError::FileStale {
                    rel: paths::LOAD_FOLDERS_XML.to_owned(),
                });
            }
            if !req.create {
                return Err(invalid(
                    "the mod has no LoadFolders.xml; ask for it to be created",
                ));
            }
            (paths::LOAD_FOLDERS_XML.to_owned(), None)
        }
    };
    let base = old_text.clone().unwrap_or_else(new_file_text);
    if raw_blocks(&base).is_none() {
        return Err(ToolkitError::FileNotEditable {
            rel,
            reason: "the file is not well formed XML; fix the text first".to_owned(),
        });
    }
    let new_text = apply_changes(&base, &req.changes)?;
    let created = old_text.is_none();
    let changed = created || new_text != base;
    let diff = unified_diff(&rel, old_text.as_deref().unwrap_or(""), &new_text);
    let (mut backup, mut verified, mut written) = (None, !changed, false);
    if changed && !req.dry_run {
        let report =
            writer.write_expecting(&rel, &new_text, true, Expect::Previous(old_text.as_deref()))?;
        backup = report.backup.map(|b| b.to_string());
        verified = report.verified;
        written = true;
    }
    let bytes = new_text.clone().into_bytes();
    let shown = LoadedFile {
        rel: rel.clone(),
        total: bytes.len() as u64,
        truncated: false,
        hash: rimstudio_io::sha256::sha256_hex(&bytes),
        bytes,
    };
    Ok(ProjectLoadFoldersUpdateDto {
        written,
        created: created && written,
        diff,
        backup,
        verified,
        load_folders: build_dto(
            &req.project_id,
            &record.path,
            &rel,
            Some(&shown),
            Some(&new_text),
        ),
    })
}
