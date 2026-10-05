//! `project_version_add`: a folder for one more game version.
//!
//! The call creates the version folder (for example `1.6`) and, when asked, the standard sub folders of a
//! version folder; they come from the scaffold plan of a versioned mod, so there is one list of them. It
//! never copies or moves a file. When the mod has a `LoadFolders.xml` (or the caller asks for one) it adds a
//! block for the version: the `Common` folder when the mod has one, and the new folder. The block is the
//! last thing written, so a stop leaves folders only. `supportedVersions` of About.xml is not changed; the
//! answer says when the version is missing from it.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::paths;
use rimstudio_core::version::{normalize_version_key, parse_major_minor};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_ipc_types::project_about::{
    LoadEntryInputDto, LoadFoldersChangeDto, ProjectVersionAddDto, ProjectVersionAddRequest,
};
use rimstudio_workspace::scaffold::{ScaffoldLayout, ScaffoldSpec, plan as scaffold_plan};

use super::load_folders::{self, load, supported_versions};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;
use crate::shared::writer::Expect;

/// `version.folder-exists`
pub const FOLDER_EXISTS: DiagCode = DiagCode::new("version.folder-exists");
/// `version.block-exists`
pub const BLOCK_EXISTS: DiagCode = DiagCode::new("version.block-exists");
/// `version.folder-refused`
pub const FOLDER_REFUSED: DiagCode = DiagCode::new("version.folder-refused");

fn invalid(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::EditInvalid {
        reason: reason.into(),
    }
}

/// The standard sub folders of a version folder, relative to the mod root, from the scaffold plan of a
/// versioned mod for that version.
fn standard_subfolders(key: &str) -> Vec<String> {
    let mut spec = ScaffoldSpec::new("/rimstudio-plan", "Plan", "plan.mod");
    spec.layout = ScaffoldLayout::Versioned;
    spec.supported_versions = vec![key.to_owned()];
    let prefix = format!("{key}/");
    let mut out: Vec<String> = scaffold_plan(&spec)
        .dirs()
        .filter(|d| d.starts_with(&prefix))
        .map(str::to_owned)
        .collect();
    out.sort();
    out
}

fn block_exists(text: &str, key: &str) -> bool {
    rimstudio_xml::load_folders::read(text.as_bytes())
        .map(|r| {
            r.spec
                .blocks
                .iter()
                .any(|b| normalize_version_key(&b.key) == key)
        })
        .unwrap_or(false)
}

/// The entries of the new block: the root and `Common` entries the mod already uses, else `Common` when the
/// folder exists, then the new version folder.
fn block_entries(root: &Utf8Path, text: Option<&str>, key: &str) -> Vec<LoadEntryInputDto> {
    let mut entries: Vec<LoadEntryInputDto> = Vec::new();
    fn push(entries: &mut Vec<LoadEntryInputDto>, path: &str) {
        if !entries.iter().any(|e| e.path.eq_ignore_ascii_case(path)) {
            entries.push(LoadEntryInputDto {
                path: path.to_owned(),
                ..LoadEntryInputDto::default()
            });
        }
    }
    if let Some(Ok(read)) = text.map(|t| rimstudio_xml::load_folders::read(t.as_bytes())) {
        let wanted = parse_major_minor(key, false);
        let nearest = read
            .spec
            .blocks
            .iter()
            .filter(|b| {
                parse_major_minor(&b.key, false)
                    .zip(wanted)
                    .is_some_and(|(have, want)| have <= want)
            })
            .max_by_key(|b| parse_major_minor(&b.key, false));
        if let Some(b) = nearest {
            for e in &b.entries {
                let plain = e.if_active.is_empty()
                    && e.if_active_all.is_empty()
                    && e.if_not_active.is_empty();
                if plain && (e.path.is_empty() || e.path.eq_ignore_ascii_case(paths::COMMON_DIR)) {
                    push(&mut entries, &e.path);
                }
            }
        }
    }
    if entries.is_empty() && root.join(paths::COMMON_DIR).as_std_path().is_dir() {
        push(&mut entries, paths::COMMON_DIR);
    }
    push(&mut entries, key);
    entries
}

/// `project_version_add`: creates the folder of a game version, its standard sub folders when asked, and
/// its block in `LoadFolders.xml`. With `dry_run` nothing is written.
///
/// # Errors
///
/// [`ToolkitError::EditInvalid`] for a version that is not `major.minor` or a name taken by a file,
/// [`ToolkitError::ProjectNotOpen`], path refusals, and store errors.
pub fn add(
    env: &ProjectEnv,
    req: &ProjectVersionAddRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectVersionAddDto> {
    let record = env.record(&req.project_id)?;
    let writer = env.writer(&record.path, record.id.as_str(), protected)?;
    let raw = req.version.trim();
    let Some((major, minor)) = parse_major_minor(raw, true)
        .filter(|_| raw.trim_start_matches(['v', 'V']).split('.').count() == 2)
    else {
        return Err(invalid(format!(
            "\"{raw}\" is not a game version; write it as major.minor, for example 1.6"
        )));
    };
    let key = format!("{major}.{minor}");
    let root = &record.path;
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let target = root.join(&key);
    if target.as_std_path().is_file() {
        return Err(invalid(format!("{key} exists in the mod and is a file")));
    }
    if target.as_std_path().is_dir() {
        diagnostics.push(Diagnostic::new(
            FOLDER_EXISTS,
            Severity::Info,
            format!("the folder {key} exists already and is left as it is"),
        ));
    }
    let mut wanted = vec![key.clone()];
    if req.standard_folders {
        wanted.extend(standard_subfolders(&key));
    }
    let created_folders: Vec<String> = wanted
        .into_iter()
        .filter(|f| !root.join(f).as_std_path().is_dir())
        .collect();
    let existing = load(&writer, root)?;
    let want_block = existing.is_some() || req.add_block;
    let old_text =
        match &existing {
            Some(f) if f.truncated => {
                return Err(ToolkitError::FileNotEditable {
                    rel: f.rel.clone(),
                    reason: "the file is larger than 1 MiB".to_owned(),
                });
            }
            Some(f) => Some(String::from_utf8(f.bytes.clone()).map_err(|_| {
                ToolkitError::FileNotEditable {
                    rel: f.rel.clone(),
                    reason: "the file is not UTF-8 text".to_owned(),
                }
            })?),
            None => None,
        };
    let mut new_text: Option<String> = None;
    let mut block_added = false;
    if want_block {
        let base = old_text.clone().unwrap_or_else(load_folders::new_file_text);
        if block_exists(&base, &key) {
            diagnostics.push(Diagnostic::new(
                BLOCK_EXISTS,
                Severity::Info,
                format!("LoadFolders.xml has a block for {key} already and is left as it is"),
            ));
        } else {
            let entries = block_entries(root, old_text.as_deref(), &key);
            let text = load_folders::apply_changes(
                &base,
                &[LoadFoldersChangeDto::AddBlock {
                    version: key.clone(),
                    entries,
                    at: None,
                }],
            )?;
            new_text = Some(text);
            block_added = true;
        }
    }
    let load_folders_created = block_added && old_text.is_none();
    if !req.dry_run {
        for folder in &created_folders {
            if let Err(e) = writer.make_dir(folder) {
                diagnostics.push(Diagnostic::new(
                    FOLDER_REFUSED,
                    Severity::Error,
                    format!("the folder {folder} was not created: {e}"),
                ));
                return Ok(ProjectVersionAddDto {
                    version: key.clone(),
                    folder: key,
                    created_folders: Vec::new(),
                    block_added: false,
                    load_folders_created: false,
                    not_in_supported_versions: false,
                    diagnostics: diagnostics_to_dtos(&diagnostics),
                });
            }
        }
        if let Some(text) = &new_text {
            let rel = existing
                .as_ref()
                .map_or_else(|| paths::LOAD_FOLDERS_XML.to_owned(), |f| f.rel.clone());
            writer.write_expecting(&rel, text, true, Expect::Previous(old_text.as_deref()))?;
        }
    }
    let supported = supported_versions(root);
    let not_in_supported_versions = !supported
        .iter()
        .any(|s| parse_major_minor(s, true) == Some((major, minor)));
    Ok(ProjectVersionAddDto {
        version: key.clone(),
        folder: key,
        created_folders,
        block_added,
        load_folders_created,
        not_in_supported_versions,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_sub_folders_come_from_the_scaffold() {
        let dirs = standard_subfolders("1.6");
        assert!(dirs.contains(&"1.6/Defs/ThingDefs_Misc/Weapons".to_owned()));
        assert!(dirs.contains(&"1.6/Patches".to_owned()));
        assert!(dirs.iter().all(|d| d.starts_with("1.6/")));
        assert!(!dirs.iter().any(|d| d.contains("Source")));
    }

    #[test]
    fn a_new_block_keeps_the_plain_root_and_common_entries_of_the_nearest_lower_block() {
        let text = "<loadFolders><v1.5><li>/</li><li>1.5</li><li IfModActive=\"a.b\">Compat</li></v1.5></loadFolders>";
        let root = Utf8Path::new("/nonexistent-rimstudio");
        let entries = block_entries(root, Some(text), "1.6");
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["", "1.6"]);
        let entries = block_entries(root, None, "1.6");
        assert_eq!(entries.len(), 1);
    }
}
