//! Source use cases: list, add, update, remove and probe game, workshop and custom mod folders.
//!
//! The built in sources come from detection: the install `Data` folder, the install `Mods` folder
//! and one Workshop content folder per library that holds content (plus extra folders from the path
//! overrides). Custom folders (any number, R4) live in `workspace.jsonc`. Adding checks the folder
//! (it must exist, be a folder and not overlap any other source: nested, duplicate or inside the game
//! `Mods` folder are refused). Removing a custom folder only edits the document and never touches a
//! file. Every function here is read only on the file system.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::Diagnostic;
use rimstudio_core::ids::SourceId;
use rimstudio_core::mods::{ModSource, SourceKind, SourceSet};
use rimstudio_core::ports::VolumeClass;
use rimstudio_core::settings::{
    CustomFolder, FolderLayout, LinkMode, MAX_SCAN_DEPTH, MIN_SCAN_DEPTH, SourceEntry, VolumeHint,
    WorkspaceSettings, normalize_path, validate_custom_folders,
};
use rimstudio_library::sources::{
    FolderClass, FolderKind, FolderProbe as LibraryProbe, FolderWarning, SourceOverlap,
    classify_folder, probe_folder as library_probe,
};
use rimstudio_steam::report::DetectionReport;
use serde::{Deserialize, Serialize};

use crate::ctx::{Ctx, PROBE_DEADLINE};
use crate::detect::{current_report, select_install};
use crate::error::{ManagerError, ManagerResult};

/// What the file system says about a source folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    /// The path exists and is a folder.
    pub exists: bool,
    /// The folder can be listed.
    pub readable: bool,
    /// The folder looks like it is on a removable or external drive (a mount below `/run/media`,
    /// `/media`, `/mnt` or `/Volumes`, a non system drive letter, or a FAT family volume).
    pub removable_drive: bool,
    /// How many mods were found at the suggested depth; `None` when not estimated.
    pub mod_count_estimate: Option<usize>,
    /// What the folder looks like; `None` when not classified.
    pub kind: Option<FolderKind>,
    /// Soft problems found while classifying.
    pub warnings: Vec<FolderWarning>,
}

/// One row of the sources list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRow {
    /// The source id (`game-data`, `game-mods`, `workshop-N` or `cf_...`).
    pub id: SourceId,
    /// The kind.
    pub kind: SourceKind,
    /// The display label.
    pub label: String,
    /// The folder.
    pub path: Utf8PathBuf,
    /// Included in scans.
    pub enabled: bool,
    /// Only custom folders can be removed; built in sources can be disabled.
    pub can_remove: bool,
    /// Zero based position in the list (built in sources first, then custom folders in order).
    pub order: usize,
    /// The layout of a custom folder.
    pub layout: Option<FolderLayout>,
    /// The scan depth of a custom folder.
    pub scan_depth: Option<u8>,
    /// Never write inside the folder (custom folders).
    pub read_only: bool,
    /// How the game gets the mods of a custom folder.
    pub link: Option<LinkMode>,
    /// The state of the folder.
    pub status: FolderStatus,
}

/// A request for the sources list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourcesListRequest {
    /// Estimate the mod count of each folder (lists the folder, costs time on big folders).
    pub estimate_mod_counts: bool,
}

/// The sources list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesListResponse {
    /// Built in sources first, then custom folders.
    pub rows: Vec<SourceRow>,
    /// The install the built in rows come from, when one was detected.
    pub install_id: Option<String>,
    /// The text of the install's `Version.txt`.
    pub game_version: Option<String>,
}

/// A request to add a custom folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourcesAddFolderRequest {
    /// The absolute folder path as text.
    pub path: String,
    /// The display name; defaults to the last path component.
    pub label: Option<String>,
    /// How to read the folder; defaults to `auto`.
    pub layout: Option<FolderLayout>,
    /// Levels below the root to search (1 to 4); defaults to the depth that finds the mods.
    pub scan_depth: Option<u8>,
    /// Never write inside the folder.
    pub read_only: bool,
}

/// The result of adding a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesAddFolderResponse {
    /// The new row.
    pub row: SourceRow,
    /// Warnings found while probing (a folder with errors is refused, so these are soft).
    pub diagnostics: Vec<Diagnostic>,
}

/// A change of one source. Absent fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourcesUpdateRequest {
    /// The source id.
    pub id: String,
    /// A new label (custom folders).
    pub label: Option<String>,
    /// Include or exclude from scans (any source).
    pub enabled: Option<bool>,
    /// A new position among the custom folders, zero based; clamped to the end.
    pub order: Option<usize>,
    /// A new layout (custom folders).
    pub layout: Option<FolderLayout>,
    /// A new scan depth, 1 to 4 (custom folders).
    pub scan_depth: Option<u8>,
    /// A new read only flag (custom folders).
    pub read_only: Option<bool>,
}

/// A request to remove a custom folder from the list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourcesRemoveRequest {
    /// The custom folder id.
    pub id: String,
}

/// The result of removing a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesRemoveResponse {
    /// The id that was removed.
    pub id: SourceId,
    /// The path that was listed; the folder itself is untouched.
    pub path: Utf8PathBuf,
    /// How many custom folders remain.
    pub remaining: usize,
}

/// A request to check a folder before adding it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourcesProbeFolderRequest {
    /// The absolute folder path as text.
    pub path: String,
}

/// What probing a folder found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesProbeFolderResponse {
    /// The normalised path.
    pub path: Utf8PathBuf,
    /// A label to offer.
    pub suggested_label: String,
    /// What the folder looks like, with the suggested depth and layout.
    pub class: FolderClass,
    /// The sources it overlaps.
    pub overlaps: Vec<SourceOverlap>,
    /// Errors and warnings.
    pub diagnostics: Vec<Diagnostic>,
    /// False when the folder cannot be added.
    pub can_add: bool,
    /// The folder looks like it is on a removable or external drive.
    pub removable_drive: bool,
    /// The volume hint that would be stored.
    pub volume_hint: Option<VolumeHint>,
}

/// All sources of a report and the workspace, built in ones first, as a [`SourceSet`].
///
/// Built in sources follow the `modSources` entries for their enabled flag. The Workshop sources are
/// the content folders of the selected install, then the extra folders of the path overrides.
pub fn effective_sources(report: &DetectionReport, ws: &WorkspaceSettings) -> SourceSet {
    let mut set = SourceSet::new();
    let enabled = |id: &SourceId| {
        ws.mod_sources
            .iter()
            .find(|e| &e.id == id)
            .is_none_or(|e| e.enabled)
    };
    if let Some(install) = select_install(report, ws) {
        let mut data = ModSource::new(
            SourceId::game_data(),
            SourceKind::GameData,
            install.game_root.join("Data"),
        );
        data.label = "Game data".to_owned();
        data.enabled = enabled(&data.id);
        set.insert(data);
        let mut mods = ModSource::new(
            SourceId::game_mods(),
            SourceKind::GameMods,
            install.mods_dir.path.clone(),
        );
        mods.label = "Game mods".to_owned();
        mods.enabled = enabled(&mods.id);
        set.insert(mods);
        let mut dirs: Vec<Utf8PathBuf> = install
            .workshop
            .iter()
            .map(|w| w.content_dir.clone())
            .collect();
        for extra in &ws.paths.extra_workshop_dirs {
            if let Ok(p) = normalize_path(extra.as_str())
                && !dirs.contains(&p)
            {
                dirs.push(p);
            }
        }
        let many = dirs.len() > 1;
        for (n, dir) in dirs.into_iter().enumerate() {
            let mut w = ModSource::new(SourceId::workshop(n), SourceKind::Workshop, dir);
            w.label = if many {
                format!("Workshop {}", n + 1)
            } else {
                "Workshop".to_owned()
            };
            w.enabled = enabled(&w.id);
            set.insert(w);
        }
    }
    for source in ws.custom_sources() {
        set.insert(source);
    }
    set
}

/// Lists the sources with their status.
///
/// Uses the cached detection report, running detection when there is none.
///
/// # Errors
/// A store error when the workspace document cannot be opened.
pub fn list(ctx: &Ctx, req: SourcesListRequest) -> ManagerResult<SourcesListResponse> {
    let ws = ctx.workspace_store()?.load().value;
    let (report, _) = current_report(ctx)?;
    let set = effective_sources(&report, &ws);
    let rows = rows_of(ctx, &set, &ws, req.estimate_mod_counts);
    let install = select_install(&report, &ws);
    Ok(SourcesListResponse {
        rows,
        install_id: install.map(|i| i.id.clone()),
        game_version: install.and_then(|i| i.version.as_ref().map(|v| v.raw.clone())),
    })
}

/// Checks a folder before it is added: what it looks like, and whether it overlaps a source.
///
/// # Errors
/// [`ManagerError::Path`] when the text is not an absolute path.
pub fn probe_folder(
    ctx: &Ctx,
    req: SourcesProbeFolderRequest,
) -> ManagerResult<SourcesProbeFolderResponse> {
    let path = normalize_path(&req.path)?;
    let ws = ctx.workspace_store()?.load().value;
    let (report, _) = current_report(ctx)?;
    let probe = run_probe(ctx, &path, &report, &ws);
    let removable = is_removable(ctx, &path);
    Ok(SourcesProbeFolderResponse {
        suggested_label: default_label(&path),
        can_add: probe.can_save,
        class: probe.class,
        overlaps: probe.overlaps,
        diagnostics: probe.diagnostics,
        removable_drive: removable,
        volume_hint: removable.then(|| volume_hint(&path)),
        path,
    })
}

/// Adds a custom folder.
///
/// # Errors
/// [`ManagerError::Path`] for a bad path, [`ManagerError::InvalidRequest`] for an out of range
/// depth, [`ManagerError::FolderRejected`] when the folder is missing, not a folder or overlaps
/// another source (nested, duplicate, inside the game `Mods` folder), [`ManagerError::ReadOnly`]
/// when `workspace.jsonc` must not be overwritten, or a store error.
pub fn add_folder(
    ctx: &Ctx,
    req: SourcesAddFolderRequest,
) -> ManagerResult<SourcesAddFolderResponse> {
    let path = normalize_path(&req.path)?;
    if let Some(depth) = req.scan_depth {
        check_depth(depth)?;
    }
    let (store, mut ws) = ctx.workspace_for_edit()?;
    let (report, _) = current_report(ctx)?;
    let probe = run_probe(ctx, &path, &report, &ws);
    if !probe.can_save {
        return Err(ManagerError::FolderRejected {
            path,
            diagnostics: probe.diagnostics,
        });
    }
    let id = fresh_id(ctx, &path, &report, &ws);
    let mut folder = CustomFolder::new(id.clone(), path.clone());
    if let Some(label) = req
        .label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        label.clone_into(&mut folder.label);
    } else {
        folder.label = default_label(&path);
    }
    folder.layout = req.layout.unwrap_or(FolderLayout::Auto);
    folder.scan_depth = req
        .scan_depth
        .unwrap_or(probe.class.suggested_depth)
        .clamp(MIN_SCAN_DEPTH, MAX_SCAN_DEPTH);
    folder.read_only = req.read_only;
    if is_removable(ctx, &path) {
        folder.volume_hint = Some(volume_hint(&path));
    }
    ws.custom_mod_folders.push(folder);
    let problems = validate_custom_folders(&ws.custom_mod_folders);
    if let Some(p) = problems.first() {
        return Err(ManagerError::InvalidRequest {
            field: "path",
            reason: format!("{}: {}", p.field, p.reason),
        });
    }
    store.save(&ws)?;
    let set = effective_sources(&report, &ws);
    let rows = rows_of(ctx, &set, &ws, true);
    let row =
        rows.into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| ManagerError::SourceNotFound {
                id: id.as_str().to_owned(),
            })?;
    Ok(SourcesAddFolderResponse {
        row,
        diagnostics: probe.diagnostics,
    })
}

/// Changes a source: label, enabled flag, position, layout, scan depth and read only flag.
///
/// Built in sources accept only `enabled`.
///
/// # Errors
/// [`ManagerError::SourceNotFound`], [`ManagerError::InvalidRequest`] (empty label, depth out of
/// range, a custom only field on a built in source), [`ManagerError::ReadOnly`] or a store error.
pub fn update(ctx: &Ctx, req: SourcesUpdateRequest) -> ManagerResult<SourceRow> {
    let (store, mut ws) = ctx.workspace_for_edit()?;
    let (report, _) = current_report(ctx)?;
    if let Some(depth) = req.scan_depth {
        check_depth(depth)?;
    }
    if let Some(label) = &req.label
        && label.trim().is_empty()
    {
        return Err(ManagerError::InvalidRequest {
            field: "label",
            reason: "must not be empty".to_owned(),
        });
    }
    let custom_pos = ws
        .custom_mod_folders
        .iter()
        .position(|f| f.id.as_str() == req.id);
    let id = if let Some(pos) = custom_pos {
        let target = ws.custom_mod_folders.get_mut(pos);
        if let Some(f) = target {
            if let Some(label) = &req.label {
                label.trim().clone_into(&mut f.label);
            }
            if let Some(v) = req.enabled {
                f.enabled = v;
            }
            if let Some(v) = req.layout {
                f.layout = v;
            }
            if let Some(v) = req.scan_depth {
                f.scan_depth = v;
            }
            if let Some(v) = req.read_only {
                f.read_only = v;
            }
        }
        if let Some(to) = req.order {
            let to = to.min(ws.custom_mod_folders.len().saturating_sub(1));
            let item = ws.custom_mod_folders.remove(pos);
            ws.custom_mod_folders.insert(to, item);
        }
        SourceId::new(&req.id)?
    } else {
        update_builtin(&report, &mut ws, &req)?
    };
    store.save(&ws)?;
    let set = effective_sources(&report, &ws);
    rows_of(ctx, &set, &ws, true)
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(ManagerError::SourceNotFound { id: req.id })
}

/// Removes a custom folder from the list. The folder and its files are never touched.
///
/// # Errors
/// [`ManagerError::SourceNotFound`] for an unknown id, [`ManagerError::InvalidRequest`] for a built
/// in source (disable it instead), [`ManagerError::ReadOnly`] or a store error.
pub fn remove(ctx: &Ctx, req: SourcesRemoveRequest) -> ManagerResult<SourcesRemoveResponse> {
    let (store, mut ws) = ctx.workspace_for_edit()?;
    let Some(pos) = ws
        .custom_mod_folders
        .iter()
        .position(|f| f.id.as_str() == req.id)
    else {
        let builtin =
            req.id == "game-data" || req.id == "game-mods" || req.id.starts_with("workshop-");
        return Err(if builtin {
            ManagerError::InvalidRequest {
                field: "id",
                reason: "built in sources cannot be removed; disable them instead".to_owned(),
            }
        } else {
            ManagerError::SourceNotFound { id: req.id }
        });
    };
    let removed = ws.custom_mod_folders.remove(pos);
    store.save(&ws)?;
    Ok(SourcesRemoveResponse {
        id: removed.id,
        path: removed.path,
        remaining: ws.custom_mod_folders.len(),
    })
}

fn update_builtin(
    report: &DetectionReport,
    ws: &mut WorkspaceSettings,
    req: &SourcesUpdateRequest,
) -> ManagerResult<SourceId> {
    let set = effective_sources(report, ws);
    let Some(source) = set.iter().find(|s| s.id.as_str() == req.id) else {
        return Err(ManagerError::SourceNotFound { id: req.id.clone() });
    };
    if req.label.is_some()
        || req.order.is_some()
        || req.layout.is_some()
        || req.scan_depth.is_some()
        || req.read_only.is_some()
    {
        return Err(ManagerError::InvalidRequest {
            field: "id",
            reason: "built in sources can only be enabled or disabled".to_owned(),
        });
    }
    let id = source.id.clone();
    let kind = source.kind;
    if let Some(v) = req.enabled {
        match ws.mod_sources.iter_mut().find(|e| e.id == id) {
            Some(e) => e.enabled = v,
            None => ws.mod_sources.push(SourceEntry {
                id: id.clone(),
                kind,
                enabled: v,
            }),
        }
    }
    Ok(id)
}

fn check_depth(depth: u8) -> ManagerResult<()> {
    if (MIN_SCAN_DEPTH..=MAX_SCAN_DEPTH).contains(&depth) {
        Ok(())
    } else {
        Err(ManagerError::InvalidRequest {
            field: "scanDepth",
            reason: format!("must be between {MIN_SCAN_DEPTH} and {MAX_SCAN_DEPTH}"),
        })
    }
}

fn run_probe(
    ctx: &Ctx,
    path: &Utf8Path,
    report: &DetectionReport,
    ws: &WorkspaceSettings,
) -> LibraryProbe {
    let existing: Vec<ModSource> = effective_sources(report, ws).iter().cloned().collect();
    library_probe(ctx.fs, path, &existing, ctx.os.case_insensitive_default())
}

fn fresh_id(
    ctx: &Ctx,
    path: &Utf8Path,
    report: &DetectionReport,
    ws: &WorkspaceSettings,
) -> SourceId {
    let taken = effective_sources(report, ws);
    let now = ctx.clock.now_unix_ms();
    for n in 0u32.. {
        let seed = format!("{path}|{now}|{n}");
        let id = SourceId::custom_from_seed(seed.as_bytes());
        if taken.get(&id).is_none() {
            return id;
        }
    }
    SourceId::custom_from_seed(path.as_str().as_bytes())
}

fn default_label(path: &Utf8Path) -> String {
    path.file_name()
        .map_or_else(|| path.as_str().to_owned(), str::to_owned)
}

fn rows_of(ctx: &Ctx, set: &SourceSet, ws: &WorkspaceSettings, estimate: bool) -> Vec<SourceRow> {
    set.iter()
        .enumerate()
        .map(|(order, s)| {
            let custom = ws.custom_mod_folders.iter().find(|f| f.id == s.id);
            SourceRow {
                id: s.id.clone(),
                kind: s.kind,
                label: s.label.clone(),
                path: s.path.clone(),
                enabled: s.enabled,
                can_remove: custom.is_some(),
                order,
                layout: custom.map(|f| f.layout),
                scan_depth: custom.map(|f| f.scan_depth),
                read_only: s.read_only,
                link: custom.map(|f| f.link),
                status: status_of(ctx, &s.path, estimate),
            }
        })
        .collect()
}

fn status_of(ctx: &Ctx, path: &Utf8Path, estimate: bool) -> FolderStatus {
    let exists = ctx.fs.is_dir(path, PROBE_DEADLINE).unwrap_or(false);
    let readable = exists && ctx.fs.read_dir(path, PROBE_DEADLINE).is_ok();
    let (count, kind, warnings) = if estimate && readable {
        let class = classify_folder(ctx.fs, path);
        (Some(class.mod_count), Some(class.kind), class.warnings)
    } else {
        (None, None, Vec::new())
    };
    FolderStatus {
        exists,
        readable,
        removable_drive: is_removable(ctx, path),
        mod_count_estimate: count,
        kind,
        warnings,
    }
}

/// True when the folder looks like it is on a removable or external drive.
pub fn is_removable(ctx: &Ctx, path: &Utf8Path) -> bool {
    if matches!(
        ctx.fs.volume_class(path),
        VolumeClass::Fat | VolumeClass::Exfat
    ) {
        return true;
    }
    looks_removable(path.as_str())
}

/// The path only rule behind [`is_removable`].
pub fn looks_removable(path: &str) -> bool {
    const PREFIXES: [&str; 4] = ["/run/media/", "/media/", "/mnt/", "/Volumes/"];
    if PREFIXES.iter().any(|p| path.starts_with(p)) {
        return true;
    }
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !b[0].eq_ignore_ascii_case(&b'c')
}

/// The mount point and label that identify the volume of a removable path.
pub fn volume_hint(path: &Utf8Path) -> VolumeHint {
    let text = path.as_str();
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();
    let (take, mount_root) = if text.starts_with("/run/media/") {
        (4usize, "/")
    } else if text.starts_with("/media/") {
        (3, "/")
    } else if text.starts_with("/mnt/") || text.starts_with("/Volumes/") {
        (2, "/")
    } else {
        (1, "")
    };
    let used: Vec<&str> = parts.iter().take(take).copied().collect();
    let mount = if mount_root.is_empty() {
        format!("{}/", used.first().copied().unwrap_or_default())
    } else {
        format!("/{}", used.join("/"))
    };
    VolumeHint {
        label: used.last().copied().unwrap_or_default().to_owned(),
        mount,
        uuid: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("/run/media/u/disk/mods", true)]
    #[case("/media/u/disk", true)]
    #[case("/mnt/data", true)]
    #[case("/Volumes/Stick/x", true)]
    #[case("D:/mods", true)]
    #[case("c:/mods", false)]
    #[case("/home/u/mods", false)]
    fn removable_drive_hint_follows_the_path(#[case] path: &str, #[case] expected: bool) {
        assert_eq!(looks_removable(path), expected);
    }

    #[rstest]
    #[case("/run/media/u/disk/mods", "/run/media/u/disk", "disk")]
    #[case("/media/u/disk/mods", "/media/u/disk", "disk")]
    #[case("/mnt/data/mods", "/mnt/data", "data")]
    #[case("D:/mods/x", "D:/", "D:")]
    fn volume_hint_names_the_mount(#[case] path: &str, #[case] mount: &str, #[case] label: &str) {
        let h = volume_hint(Utf8Path::new(path));
        assert_eq!(h.mount, mount);
        assert_eq!(h.label, label);
    }
}
