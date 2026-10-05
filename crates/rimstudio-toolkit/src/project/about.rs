//! The basics of a mod: `project_about_get`, `project_about_preview` and `project_about_update`.
//!
//! A person edits a form; the form sends small changes (set this field, add this entry, move that one)
//! with the hash of the file it read. The backend applies them as byte span edits of the existing text
//! ([`super::about_apply`]), so comments, element order, unknown elements and untouched fields stay as they
//! were, and writes through the guarded writer: a backup of the replaced file goes to the app data folder
//! (D-090), the bytes go down atomically, and the file is read back and compared. A file that changed on
//! disk since the form read it is refused (the hash, and again at write time). Findings
//! ([`super::about_lint`]) never block a save; only a file that is not well formed XML does.
//!
//! The library scan is an input, not something this module starts: with no scan the findings that compare
//! against other mods are left out.

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::version::{GameVersion, parse_major_minor};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_ipc_types::project_about::{
    AboutByVersionDto, AboutDependencyDto, AboutListDto, AboutTextDto, AboutVersionRelationsDto,
    AboutVersionTextDto, ProjectAboutDto, ProjectAboutGetRequest, ProjectAboutPreviewDto,
    ProjectAboutPreviewRequest, ProjectAboutUpdateDto, ProjectAboutUpdateRequest,
};
use rimstudio_library::index::LibraryIndex;
use rimstudio_workspace::project::find_about_file;
use rimstudio_xml::about::{About, has_whitespace_before_declaration, read_lenient};
use rimstudio_xml::edit::SpanEditor;
use rimstudio_xml::error::codes as xml_codes;

use super::about_apply::apply_changes;
use super::about_lint::{LEADING_WHITESPACE, LintContext, NOT_UTF8, TOO_LARGE, UNPARSEABLE, lint};
use super::about_preview::{PreviewState, read_preview};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::diff::unified_diff;
use crate::shared::env::ProjectEnv;
use crate::shared::writer::{Expect, GuardedWriter};

/// The largest About file the edits of this module accept (1 MiB). A larger file is shown as not editable.
pub const MAX_ABOUT_BYTES: usize = 1 << 20;
/// How much of the file text a response carries (256 KiB).
pub const RAW_TEXT_LIMIT: usize = 256 * 1024;

/// What the findings compare against, supplied by the caller (the library scan is never started here).
#[derive(Clone, Copy, Default)]
pub struct AboutContext<'a> {
    /// The last scan of the library.
    pub library: Option<&'a LibraryIndex>,
    /// The installed game version.
    pub game_version: Option<&'a GameVersion>,
}

/// The About file of a project as read from disk.
struct Loaded {
    rel: String,
    about_dir: String,
    bytes: Vec<u8>,
    total: u64,
    truncated: bool,
    hash: String,
}

fn load(writer: &GuardedWriter, root: &Utf8Path) -> ToolkitResult<Loaded> {
    let path = find_about_file(root).ok_or_else(|| ToolkitError::ProjectInvalid {
        path: root.to_string(),
        reason: "the folder has no About/About.xml".to_owned(),
    })?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| ToolkitError::ProjectInvalid {
            path: path.to_string(),
            reason: "the About file is outside the project".to_owned(),
        })?
        .as_str()
        .replace('\\', "/");
    let about_dir = rel.split('/').next().unwrap_or("About").to_owned();
    let read = writer.read_limited(&rel, MAX_ABOUT_BYTES)?.ok_or_else(|| {
        ToolkitError::ProjectInvalid {
            path: rel.clone(),
            reason: "the About file does not exist".to_owned(),
        }
    })?;
    let hash = if read.truncated {
        writer
            .existing_file_hash(&rel)
            .ok()
            .flatten()
            .map_or_else(|| rimstudio_io::sha256::sha256_hex(&read.bytes), |(_, h)| h)
    } else {
        rimstudio_io::sha256::sha256_hex(&read.bytes)
    };
    Ok(Loaded {
        rel,
        about_dir,
        total: read.total,
        truncated: read.truncated,
        bytes: read.bytes,
        hash,
    })
}

fn text_item(value: &str, present: bool) -> AboutTextDto {
    AboutTextDto {
        value: value.to_owned(),
        present,
    }
}

fn list_item(items: &[String], present: bool) -> AboutListDto {
    AboutListDto {
        items: items.to_vec(),
        present,
    }
}

fn dependency_dto(d: &rimstudio_core::mods::ModDependency) -> AboutDependencyDto {
    AboutDependencyDto {
        package_id: d.package_id.clone(),
        display_name: d.display_name.clone(),
        steam_workshop_url: d.steam_workshop_url.clone(),
        download_url: d.download_url.clone(),
        alternatives: d.alternatives.clone(),
    }
}

fn by_version_dto(about: &About) -> AboutByVersionDto {
    AboutByVersionDto {
        advanced: true,
        descriptions: about
            .descriptions_by_version
            .iter()
            .map(|(version, text)| AboutVersionTextDto {
                version: version.clone(),
                text: text.clone(),
            })
            .collect(),
        relations: about
            .by_version
            .iter()
            .map(|(version, r)| AboutVersionRelationsDto {
                version: version.clone(),
                mod_dependencies: r.mod_dependencies.iter().map(dependency_dto).collect(),
                load_before: r.load_before.clone(),
                load_after: r.load_after.clone(),
                force_load_before: r.force_load_before.clone(),
                force_load_after: r.force_load_after.clone(),
                incompatible_with: r.incompatible_with.clone(),
            })
            .collect(),
    }
}

/// The names of the children of the root, `None` when the text is not a document the editor can index.
fn root_children(text: &str) -> Option<BTreeSet<String>> {
    let ed = SpanEditor::open(text).ok()?;
    let root = format!("/{}", ed.root_tag());
    Some(
        ed.children(&root)
            .ok()?
            .into_iter()
            .map(|c| c.tag)
            .collect(),
    )
}

fn sev_rank(s: Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Hint => 3,
    }
}

/// True when the texture named by `modIconPath` exists below `Textures` of the mod root, of `Common` or of
/// a version folder.
fn icon_exists(root: &Utf8Path, icon: &str) -> bool {
    let rel = icon.trim().replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|s| s == "..") {
        return false;
    }
    let mut roots: Vec<Utf8PathBuf> = vec![root.to_owned()];
    if let Ok(read) = std::fs::read_dir(root.as_std_path()) {
        for e in read.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_dir())
                && (name.eq_ignore_ascii_case("common")
                    || parse_major_minor(&name, false).is_some())
                && let Ok(p) = Utf8PathBuf::from_path_buf(e.path())
            {
                roots.push(p);
            }
        }
    }
    roots.iter().any(|r| {
        ["", ".png", ".jpg", ".jpeg", ".psd"]
            .iter()
            .any(|ext| r.join("Textures").join(format!("{rel}{ext}")).is_file())
    })
}

/// Builds the model of an About file. `preview` is what is on disk next to it.
fn build(
    project_id: &str,
    root: &Utf8Path,
    file: &Loaded,
    preview: PreviewState,
    ctx: &AboutContext<'_>,
) -> ProjectAboutDto {
    let text = std::str::from_utf8(&file.bytes).ok();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut about = About::default();
    let mut parsed = false;
    let mut reason: Option<String> = None;
    if file.truncated {
        reason = Some(format!(
            "the file is larger than {} KiB",
            MAX_ABOUT_BYTES / 1024
        ));
        diagnostics.push(Diagnostic::new(
            TOO_LARGE,
            Severity::Error,
            "the About file is larger than 1 MiB, so it is shown and edited as text only",
        ));
    } else {
        let read = read_lenient(&file.bytes);
        parsed = read.parsed;
        about = read.about;
        let wrong_encoding = text.is_none();
        for w in read.warnings {
            if w.code == xml_codes::ABOUT_NO_VERSIONS {
                continue;
            }
            diagnostics.push(w);
        }
        if !parsed {
            let why = diagnostics
                .iter()
                .find(|d| d.severity == Severity::Error)
                .map_or_else(
                    || "it is not well formed XML".to_owned(),
                    |d| d.message.clone(),
                );
            reason = Some(format!("the file cannot be read: {why}"));
            diagnostics.push(Diagnostic::new(
                UNPARSEABLE,
                Severity::Error,
                format!("About.xml cannot be read ({why}); the game falls back to defaults. Fix the text first."),
            ));
        } else if wrong_encoding {
            reason = Some("the file is not UTF-8 text".to_owned());
            diagnostics.push(Diagnostic::new(
                NOT_UTF8,
                Severity::Warning,
                "About.xml is not UTF-8 (it is UTF-16 or another encoding); it is shown but not edited, so no byte is rewritten",
            ));
        } else if let Some(t) = text {
            if let Err(e) = SpanEditor::open(t) {
                // the game reads a document that is not well formed as defaults
                parsed = false;
                about = About::default();
                reason = Some(format!("the file is not well formed XML: {e}"));
                diagnostics.push(Diagnostic::new(
                    UNPARSEABLE,
                    Severity::Error,
                    format!("About.xml is not well formed XML ({e}); the game falls back to defaults. Fix the text first."),
                ));
            }
            if has_whitespace_before_declaration(t) {
                diagnostics.push(Diagnostic::new(
                    LEADING_WHITESPACE,
                    Severity::Error,
                    "there is white space before the XML declaration; the game rejects the file",
                ));
            }
        }
    }
    let present = if reason.is_none() {
        text.and_then(root_children).unwrap_or_default()
    } else {
        text.filter(|_| !file.truncated)
            .and_then(root_children)
            .unwrap_or_default()
    };
    if parsed {
        let icon = |p: &str| icon_exists(root, p);
        diagnostics.extend(lint(
            &about,
            &present,
            &LintContext {
                library: ctx.library.map(|l| (l, root)),
                game_version: ctx.game_version,
                preview: &preview.facts,
                icon_exists: &icon,
            },
        ));
    }
    diagnostics.sort_by_key(|d| sev_rank(d.severity));
    let has = |tag: &str| present.contains(tag);
    let raw_end = file.bytes.len().min(RAW_TEXT_LIMIT);
    let raw = String::from_utf8_lossy(file.bytes.get(..raw_end).unwrap_or_default()).into_owned();
    let raw = raw.trim_start_matches('\u{feff}').to_owned();
    ProjectAboutDto {
        project_id: project_id.to_owned(),
        path: file.rel.clone(),
        file_hash: file.hash.clone(),
        bytes: file.total,
        parsed,
        editable: reason.is_none() && parsed,
        not_editable_reason: reason,
        name: text_item(&about.name, has("name")),
        short_name: text_item(about.short_name.as_deref().unwrap_or(""), has("shortName")),
        author: text_item(about.author.as_deref().unwrap_or(""), has("author")),
        authors: list_item(&about.authors, has("authors")),
        package_id: text_item(&about.package_id, has("packageId")),
        description: text_item(&about.description, has("description")),
        url: text_item(about.url.as_deref().unwrap_or(""), has("url")),
        mod_version: text_item(
            about.mod_version.as_deref().unwrap_or(""),
            has("modVersion"),
        ),
        mod_icon_path: text_item(
            about.mod_icon_path.as_deref().unwrap_or(""),
            has("modIconPath"),
        ),
        supported_versions: list_item(&about.supported_versions, has("supportedVersions")),
        mod_dependencies: about.mod_dependencies.iter().map(dependency_dto).collect(),
        mod_dependencies_present: has("modDependencies"),
        load_before: list_item(&about.load_before, has("loadBefore")),
        load_after: list_item(&about.load_after, has("loadAfter")),
        force_load_before: list_item(&about.force_load_before, has("forceLoadBefore")),
        force_load_after: list_item(&about.force_load_after, has("forceLoadAfter")),
        incompatible_with: list_item(&about.incompatible_with, has("incompatibleWith")),
        steam_app_id: about.steam_app_id,
        by_version: by_version_dto(&about),
        unknown_tags: about.unknown_tags.clone(),
        preview: preview.dto,
        game_version: ctx.game_version.map(|g| {
            let (major, minor) = g.major_minor();
            format!("{major}.{minor}")
        }),
        raw_truncated: file.bytes.len() > RAW_TEXT_LIMIT || file.total > file.bytes.len() as u64,
        raw_text: raw,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    }
}

/// `project_about_get`: every basic of the mod, with the findings.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`] for an unknown project, [`ToolkitError::ProjectInvalid`] when the folder
/// has no About file, and path refusals from the guarded reader.
pub fn get(
    env: &ProjectEnv,
    req: &ProjectAboutGetRequest,
    ctx: &AboutContext<'_>,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectAboutDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let file = load(&writer, &record.path)?;
    let preview = read_preview(&writer, &file.about_dir, req.include_preview_image)?;
    Ok(build(&req.project_id, &record.path, &file, preview, ctx))
}

/// What a request of changes would do to the file.
struct Prepared {
    writer: GuardedWriter,
    root: Utf8PathBuf,
    file: Loaded,
    old_text: String,
    new_text: String,
}

fn prepare(
    env: &ProjectEnv,
    project_id: &str,
    changes: &[rimstudio_ipc_types::project_about::AboutChangeDto],
    expected_hash: Option<&str>,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<Prepared> {
    let record = env.record(project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let file = load(&writer, &record.path)?;
    if let Some(want) = expected_hash
        && !want.eq_ignore_ascii_case(&file.hash)
    {
        return Err(ToolkitError::FileStale {
            rel: file.rel.clone(),
        });
    }
    let not_editable = |reason: &str| ToolkitError::FileNotEditable {
        rel: file.rel.clone(),
        reason: reason.to_owned(),
    };
    if file.truncated {
        return Err(not_editable("the file is larger than 1 MiB"));
    }
    let old_text = String::from_utf8(file.bytes.clone())
        .map_err(|_| not_editable("the file is not UTF-8 text"))?;
    if !read_lenient(&file.bytes).parsed {
        return Err(not_editable(
            "the file is not well formed XML; fix the text first",
        ));
    }
    if let Err(e) = SpanEditor::open(old_text.as_str()) {
        return Err(not_editable(&format!(
            "the file is not well formed XML ({e}); fix the text first"
        )));
    }
    let new_text = apply_changes(&old_text, changes)?;
    Ok(Prepared {
        writer,
        root: record.path,
        file,
        old_text,
        new_text,
    })
}

fn model_of_text(
    project_id: &str,
    p: &Prepared,
    text: &str,
    ctx: &AboutContext<'_>,
) -> ToolkitResult<ProjectAboutDto> {
    let bytes = text.as_bytes().to_vec();
    let file = Loaded {
        rel: p.file.rel.clone(),
        about_dir: p.file.about_dir.clone(),
        total: bytes.len() as u64,
        truncated: false,
        hash: rimstudio_io::sha256::sha256_hex(&bytes),
        bytes,
    };
    let preview = read_preview(&p.writer, &p.file.about_dir, false)?;
    Ok(build(project_id, &p.root, &file, preview, ctx))
}

/// `project_about_preview`: what the changes would do, nothing written.
///
/// # Errors
///
/// [`ToolkitError::FileStale`] when the file differs from `expected_hash`, [`ToolkitError::FileNotEditable`]
/// for a file that is not well formed, too large or not UTF-8, [`ToolkitError::EditInvalid`] for a change
/// that does not apply, and the errors of [`get`].
pub fn preview(
    env: &ProjectEnv,
    req: &ProjectAboutPreviewRequest,
    ctx: &AboutContext<'_>,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectAboutPreviewDto> {
    let p = prepare(
        env,
        &req.project_id,
        &req.changes,
        req.expected_hash.as_deref(),
        protected,
    )?;
    let result = model_of_text(&req.project_id, &p, &p.new_text, ctx)?;
    Ok(ProjectAboutPreviewDto {
        changed: p.new_text != p.old_text,
        diff: unified_diff(&p.file.rel, &p.old_text, &p.new_text),
        current_hash: p.file.hash.clone(),
        result,
    })
}

/// `project_about_update`: applies the changes and writes the file.
///
/// The write goes through [`GuardedWriter::write_expecting`]: a backup of the replaced file in the app data
/// folder, an atomic write, a read back, and a refusal when the file no longer holds the text the edits were
/// made against. Nothing is written when the changes change no byte.
///
/// # Errors
///
/// As [`preview`], plus store errors from the write.
pub fn update(
    env: &ProjectEnv,
    req: &ProjectAboutUpdateRequest,
    ctx: &AboutContext<'_>,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectAboutUpdateDto> {
    let p = prepare(
        env,
        &req.project_id,
        &req.changes,
        req.expected_hash.as_deref(),
        protected,
    )?;
    let changed = p.new_text != p.old_text;
    let diff = unified_diff(&p.file.rel, &p.old_text, &p.new_text);
    let (mut backup, mut verified) = (None, false);
    if changed {
        let report = p.writer.write_expecting(
            &p.file.rel,
            &p.new_text,
            true,
            Expect::Previous(Some(&p.old_text)),
        )?;
        backup = report.backup.map(|b| b.to_string());
        verified = report.verified;
        // the project record keeps the name and the package id: refresh it from the file just written
        if let Err(e) = env.projects().open(&p.root) {
            tracing::warn!(error = %e, "the project record could not be refreshed after an About edit");
        }
    }
    let file = load(&p.writer, &p.root)?;
    let preview = read_preview(&p.writer, &file.about_dir, false)?;
    Ok(ProjectAboutUpdateDto {
        written: changed,
        diff,
        backup,
        verified,
        about: build(&req.project_id, &p.root, &file, preview, ctx),
    })
}
