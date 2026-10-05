//! `project_read_file`: the text of one file of a project for the file viewer.
//!
//! The path goes through the guarded writer's checks (a safe relative path, inside the project root, no link
//! out of it, outside the protected game folders), the read is size limited, and a file that is not UTF-8
//! text is reported as binary without its content.

use camino::Utf8PathBuf;
use rimstudio_ipc_types::project::{ProjectFileDto, ProjectReadFileRequest};

use super::scan::role_of;
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;

/// The default number of bytes `project_read_file` returns.
pub const DEFAULT_MAX_BYTES: u32 = 262_144;
/// The most bytes `project_read_file` returns, whatever is asked.
pub const HARD_MAX_BYTES: u32 = 1_048_576;

/// The text of the bytes read, without a byte order mark. `None` when the bytes are not UTF-8 text (a NUL
/// byte, or an invalid sequence that is not just a character cut by the limit).
fn text_of(bytes: &[u8], truncated: bool) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text.to_owned()),
        Err(e) if truncated && e.error_len().is_none() => {
            let valid = bytes.get(..e.valid_up_to()).unwrap_or_default();
            Some(String::from_utf8_lossy(valid).into_owned())
        }
        Err(_) => None,
    }
}

/// `project_read_file`: reads a text file of a project.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`] for an unknown project, [`ToolkitError::PathRefused`] for an unsafe path,
/// a folder, or a path outside the project, and [`ToolkitError::ProjectInvalid`] when the file does not exist.
pub fn read_file(
    env: &ProjectEnv,
    req: &ProjectReadFileRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectFileDto> {
    let (record, view) = env.view(&req.project_id)?;
    let writer = env.writer(&view.root, record.id.as_str(), protected)?;
    let limit = req
        .max_bytes
        .unwrap_or(DEFAULT_MAX_BYTES)
        .clamp(1, HARD_MAX_BYTES);
    let read = writer
        .read_limited(&req.path, usize::try_from(limit).unwrap_or(usize::MAX))?
        .ok_or_else(|| ToolkitError::ProjectInvalid {
            path: req.path.chars().take(200).collect(),
            reason: "the file does not exist in the project".to_owned(),
        })?;
    let role = role_of(
        &req.path,
        &view.layout,
        view.root.join(view.layout.ce_dir()).is_dir(),
    );
    let text = text_of(&read.bytes, read.truncated);
    Ok(ProjectFileDto {
        path: req.path.clone(),
        role,
        bytes: read.total,
        binary: text.is_none(),
        text: text.unwrap_or_default(),
        truncated: read.truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::text_of;

    #[test]
    fn text_is_utf8_without_a_nul_byte() {
        assert_eq!(text_of(b"hello", false).as_deref(), Some("hello"));
        assert_eq!(text_of(b"\xEF\xBB\xBFhi", false).as_deref(), Some("hi"));
        assert_eq!(text_of(b"a\0b", false), None);
        assert_eq!(text_of(&[0xFF, 0xFE, 0x41], false), None);
    }

    #[test]
    fn a_character_cut_by_the_limit_is_dropped_not_reported_as_binary() {
        let full = "aé".as_bytes();
        let cut = &full[..2];
        assert_eq!(text_of(cut, true).as_deref(), Some("a"));
        assert_eq!(text_of(cut, false), None);
    }
}
