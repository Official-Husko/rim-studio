//! `About/Preview.png`: what is on disk, copying a new image in and removing it.
//!
//! The image is the Workshop page's picture and the first thing a mod manager shows. A new image must be a
//! complete PNG with a readable header; it is copied through the guarded writer (the source is a regular
//! file read under a size limit, a different file already there is backed up first, the copy is read back
//! and compared by hash). The size the Workshop page shows without cropping is 640 by 360 and Steam refuses
//! a preview above 1 MB; a file that differs gets a warning, not a refusal. `About/Manifest.xml` and
//! `PublishedFileId.txt` are never created or changed here.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::Diagnostic;
use rimstudio_design::assets::{Detected, MAX_TEXTURE_DIMENSION, detect};
use rimstudio_io::copy::read_source;
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_ipc_types::project_about::{
    AboutPreviewDto, ProjectAboutRemovePreviewDto, ProjectAboutRemovePreviewRequest,
    ProjectAboutSetPreviewDto, ProjectAboutSetPreviewRequest,
};

use super::about_lint::{PreviewFacts, lint_preview};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::assets::{base64_encode, resolve_source};
use crate::shared::env::ProjectEnv;
use crate::shared::writer::GuardedWriter;

/// The file name the game and the uploader look for.
pub const PREVIEW_NAME: &str = "Preview.png";
/// The largest image `project_about_set_preview` copies (8 MiB).
pub const MAX_PREVIEW_COPY_BYTES: u64 = 8 * 1024 * 1024;
/// The largest image returned as a data URL (2 MiB).
pub const MAX_DATA_URL_BYTES: usize = 2 * 1024 * 1024;
/// How much of the file is read to find the dimensions.
const HEADER_BYTES: usize = 64 * 1024;

fn invalid(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::EditInvalid {
        reason: reason.into(),
    }
}

/// The name of the preview file in the `About` folder as it is spelled on disk (`Preview.png` wins over a
/// case variant), `None` when there is none.
fn spelled_name(root: &Utf8Path, about_dir: &str) -> Option<String> {
    let dir = root.join(about_dir);
    let mut found: Vec<String> = std::fs::read_dir(dir.as_std_path())
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n.eq_ignore_ascii_case(PREVIEW_NAME))
        .collect();
    found.sort();
    if found.iter().any(|n| n == PREVIEW_NAME) {
        return Some(PREVIEW_NAME.to_owned());
    }
    found.into_iter().next()
}

/// The preview as found on disk: the findings input and the DTO.
pub struct PreviewState {
    /// What the findings need.
    pub facts: PreviewFacts,
    /// The DTO of the about model.
    pub dto: AboutPreviewDto,
}

/// Reads the preview of the project at `root` (the About folder is `about_dir`, as spelled on disk).
/// `with_image` adds the image as a data URL when it is small enough.
///
/// # Errors
///
/// A path refusal from the writer.
pub fn read_preview(
    writer: &GuardedWriter,
    about_dir: &str,
    with_image: bool,
) -> ToolkitResult<PreviewState> {
    let name = spelled_name(writer.root(), about_dir);
    let shown = format!("{about_dir}/{}", name.as_deref().unwrap_or(PREVIEW_NAME));
    let Some(name) = name else {
        return Ok(PreviewState {
            facts: PreviewFacts::default(),
            dto: AboutPreviewDto {
                exists: false,
                path: shown,
                ..AboutPreviewDto::default()
            },
        });
    };
    let rel = format!("{about_dir}/{name}");
    let limit = if with_image {
        MAX_DATA_URL_BYTES
    } else {
        HEADER_BYTES
    };
    let Some(read) = writer.read_limited(&rel, limit)? else {
        return Ok(PreviewState {
            facts: PreviewFacts::default(),
            dto: AboutPreviewDto {
                exists: false,
                path: shown,
                ..AboutPreviewDto::default()
            },
        });
    };
    let dimensions = match detect(&read.bytes) {
        Detected::Png(p) => Some((p.width, p.height)),
        _ => None,
    };
    let sha256 = if read.truncated {
        writer
            .existing_file_hash(&rel)
            .ok()
            .flatten()
            .map(|(_, h)| h)
    } else {
        Some(rimstudio_io::sha256::sha256_hex(&read.bytes))
    };
    let data_url = (with_image && !read.truncated && dimensions.is_some())
        .then(|| format!("data:image/png;base64,{}", base64_encode(&read.bytes)));
    Ok(PreviewState {
        facts: PreviewFacts {
            exists: true,
            other_spelling: (name != PREVIEW_NAME).then_some(name),
            bytes: read.total,
            dimensions,
        },
        dto: AboutPreviewDto {
            exists: true,
            path: shown,
            bytes: Some(read.total),
            width: dimensions.map(|d| d.0),
            height: dimensions.map(|d| d.1),
            sha256,
            data_url,
        },
    })
}

fn about_dir_of(root: &Utf8Path) -> ToolkitResult<String> {
    let about = rimstudio_workspace::project::find_about_file(root).ok_or_else(|| {
        ToolkitError::ProjectInvalid {
            path: root.to_string(),
            reason: "the folder has no About/About.xml".to_owned(),
        }
    })?;
    about
        .parent()
        .and_then(Utf8Path::file_name)
        .map(str::to_owned)
        .ok_or_else(|| ToolkitError::ProjectInvalid {
            path: root.to_string(),
            reason: "the About folder has no name".to_owned(),
        })
}

/// `project_about_set_preview`: copies a PNG to `About/Preview.png`.
///
/// # Errors
///
/// [`ToolkitError::EditInvalid`] when the source is not a complete PNG, is too large or cannot be read,
/// [`ToolkitError::ProjectNotOpen`], a path refusal, and store errors from the copy.
pub fn set_preview(
    env: &ProjectEnv,
    req: &ProjectAboutSetPreviewRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectAboutSetPreviewDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let about_dir = about_dir_of(&record.path)?;
    let source = resolve_source(&req.source_path, None).map_err(invalid)?;
    let read = read_source(&source, MAX_PREVIEW_COPY_BYTES).map_err(|e| invalid(e.to_string()))?;
    let info = match detect(&read.bytes) {
        Detected::Png(p) if p.complete => p,
        Detected::Png(_) | Detected::Broken { .. } => {
            return Err(invalid("the file is not a complete PNG image"));
        }
        _ => return Err(invalid("the file is not a PNG image")),
    };
    if info.width > MAX_TEXTURE_DIMENSION || info.height > MAX_TEXTURE_DIMENSION {
        return Err(invalid(format!(
            "the image is {} by {} pixels; the limit is {MAX_TEXTURE_DIMENSION}",
            info.width, info.height
        )));
    }
    let name = spelled_name(&record.path, &about_dir).unwrap_or_else(|| PREVIEW_NAME.to_owned());
    let rel = format!("{about_dir}/{name}");
    let existing = writer.existing_file_hash(&rel)?.map(|(_, h)| h);
    let report = writer.copy_in(
        &rel,
        &source,
        &read.sha256,
        MAX_PREVIEW_COPY_BYTES,
        true,
        existing.as_deref(),
    )?;
    let state = read_preview(&writer, &about_dir, false)?;
    let mut findings: Vec<Diagnostic> = Vec::new();
    lint_preview(&state.facts, &mut findings);
    Ok(ProjectAboutSetPreviewDto {
        preview: state.dto,
        replaced: report.replaced,
        backup: report.backup.map(|p| p.to_string()),
        diagnostics: diagnostics_to_dtos(&findings),
    })
}

/// `project_about_remove_preview`: removes `About/Preview.png` after copying it to the backup folder.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`], a path refusal, and store errors from the backup or the removal.
pub fn remove_preview(
    env: &ProjectEnv,
    req: &ProjectAboutRemovePreviewRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectAboutRemovePreviewDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let about_dir = about_dir_of(&record.path)?;
    let Some(name) = spelled_name(&record.path, &about_dir) else {
        return Ok(ProjectAboutRemovePreviewDto {
            removed: false,
            backup: None,
        });
    };
    let report = writer.remove_file(&format!("{about_dir}/{name}"))?;
    Ok(ProjectAboutRemovePreviewDto {
        removed: report.removed,
        backup: report.backup.map(|p| p.to_string()),
    })
}
