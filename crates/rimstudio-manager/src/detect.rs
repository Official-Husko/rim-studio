//! Detection use cases: run the game locator, cache the report and apply user overrides.
//!
//! [`run`] asks `rimstudio-steam` for a [`DetectionReport`] over the context's ports, with the
//! user's overrides from `workspace.jsonc`, and caches the report as JSON in the cache root.
//! [`get_report`] answers from that cache without touching the disk layout of the game.
//! [`set_override`] validates a user chosen path (a game install is accepted only when
//! `Data/Core/About/About.xml` exists below it), stores it in the `paths` section of the workspace
//! document and runs detection again.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::settings::{PathOverride, WorkspaceSettings, normalize_path};
use rimstudio_io::atomic::atomic_write;
use rimstudio_io::roots::RootKind;
use rimstudio_steam::install::CONTENT_MARKER;
use rimstudio_steam::locator::{DetectOptions, detect};
use rimstudio_steam::report::{DetectionReport, Install, REPORT_SCHEMA};
use serde::{Deserialize, Serialize};

use crate::ctx::{Ctx, PROBE_DEADLINE};
use crate::error::{ManagerError, ManagerResult};

/// The file name of the cached report in the cache root.
pub const REPORT_FILE: &str = "detection-report.json";

/// The path overrides a user can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverrideField {
    /// The RimWorld install folder (the one that holds `Data`).
    GameInstall,
    /// The user data folder (holds `Config/ModsConfig.xml`).
    UserDir,
    /// The Steam root folder (holds `steamapps`).
    SteamRoot,
    /// An extra Workshop content folder (setting adds one; clearing removes all of them).
    WorkshopDir,
}

impl OverrideField {
    /// The key name used in messages and in `workspace.jsonc`.
    pub fn key(self) -> &'static str {
        match self {
            OverrideField::GameInstall => "gameInstall",
            OverrideField::UserDir => "userDir",
            OverrideField::SteamRoot => "steamRoot",
            OverrideField::WorkshopDir => "extraWorkshopDirs",
        }
    }
}

/// A request to run detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DetectRunRequest {
    /// A `-savedatafolder` launch override; it wins over the stored user folder override. When
    /// absent the `launch.saveDataFolder` workspace setting is used.
    pub savedata_folder: Option<Utf8PathBuf>,
}

/// The state of one stored override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideStatus {
    /// Which override.
    pub field: OverrideField,
    /// The stored path.
    pub path: Utf8PathBuf,
    /// Detection does not auto switch away from a pinned override.
    pub pinned: bool,
    /// Whether the path passes validation now.
    pub valid: bool,
    /// Why it does not, when it does not.
    pub reason: Option<String>,
}

/// The result of a detection run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectRunResponse {
    /// The report.
    pub report: DetectionReport,
    /// The stored overrides and whether each is valid.
    pub overrides: Vec<OverrideStatus>,
    /// The id of the install the manager uses: the stored active install when it was found,
    /// else the one detection selected.
    pub selected_install: Option<String>,
    /// False when the report could not be written to the cache (it is still returned).
    pub cached: bool,
}

/// A request for the cached report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DetectGetReportRequest {}

/// The cached report, when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectGetReportResponse {
    /// The last report; `None` before the first run or when the cache is unusable.
    pub report: Option<DetectionReport>,
    /// The time of the run in milliseconds since the epoch.
    pub generated_at_ms: Option<u64>,
    /// The cache existed but was damaged or written by another schema, and was ignored.
    pub ignored_cache: bool,
    /// The stored overrides and whether each is valid.
    pub overrides: Vec<OverrideStatus>,
    /// The install the manager uses.
    pub selected_install: Option<String>,
}

/// A request to set or clear an override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectSetOverrideRequest {
    /// Which override.
    pub field: OverrideField,
    /// The new path text; `None` clears the override.
    pub path: Option<String>,
    /// Pin the override (ignored when clearing).
    #[serde(default)]
    pub pinned: bool,
}

/// Runs detection with the stored overrides and caches the report.
///
/// # Errors
/// A store error when the workspace document cannot be opened; a damaged document is not an error
/// (detection runs without overrides).
pub fn run(ctx: &Ctx, req: DetectRunRequest) -> ManagerResult<DetectRunResponse> {
    let loaded = ctx.workspace_store()?.load();
    let ws = loaded.value;
    let mut options = DetectOptions::new(ctx.os).with_paths(ws.paths.clone());
    options.mods_config_version = ctx.mods_config_version;
    options.savedata_folder = req
        .savedata_folder
        .or_else(|| ws.launch.save_data_folder.clone());
    let report = detect(ctx.detect_env, &options);
    let cached = match write_cache(ctx, &report) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(error = %e, "could not cache the detection report");
            false
        }
    };
    Ok(DetectRunResponse {
        overrides: override_statuses(ctx, &ws),
        selected_install: select_install(&report, &ws).map(|i| i.id.clone()),
        report,
        cached,
    })
}

/// Returns the last cached report without running detection.
///
/// # Errors
/// A store error when the workspace document cannot be opened.
pub fn get_report(
    ctx: &Ctx,
    _req: DetectGetReportRequest,
) -> ManagerResult<DetectGetReportResponse> {
    let ws = ctx.workspace_store()?.load().value;
    let (report, ignored) = read_cache(ctx);
    Ok(DetectGetReportResponse {
        generated_at_ms: report.as_ref().map(|r| r.generated_at_ms),
        selected_install: report
            .as_ref()
            .and_then(|r| select_install(r, &ws))
            .map(|i| i.id.clone()),
        overrides: override_statuses(ctx, &ws),
        report,
        ignored_cache: ignored,
    })
}

/// Validates and stores a path override (or clears it), then runs detection again.
///
/// # Errors
/// [`ManagerError::Path`] for text that is not an absolute path, [`ManagerError::OverrideInvalid`]
/// when the folder does not look right, [`ManagerError::ReadOnly`] when `workspace.jsonc` must not
/// be overwritten, or a store error.
pub fn set_override(ctx: &Ctx, req: DetectSetOverrideRequest) -> ManagerResult<DetectRunResponse> {
    let (store, mut ws) = ctx.workspace_for_edit()?;
    match req.path.as_deref() {
        None => match req.field {
            OverrideField::GameInstall => ws.paths.game_install = None,
            OverrideField::UserDir => ws.paths.user_dir = None,
            OverrideField::SteamRoot => ws.paths.steam_root = None,
            OverrideField::WorkshopDir => ws.paths.extra_workshop_dirs.clear(),
        },
        Some(text) => {
            let path = normalize_path(text)?;
            if let Err(reason) = validate_override(ctx, req.field, &path) {
                return Err(ManagerError::OverrideInvalid {
                    field: req.field.key(),
                    reason,
                });
            }
            let value = PathOverride {
                path: path.clone(),
                pinned: req.pinned,
            };
            match req.field {
                OverrideField::GameInstall => ws.paths.game_install = Some(value),
                OverrideField::UserDir => ws.paths.user_dir = Some(value),
                OverrideField::SteamRoot => ws.paths.steam_root = Some(value),
                OverrideField::WorkshopDir => {
                    if !ws.paths.extra_workshop_dirs.contains(&path) {
                        ws.paths.extra_workshop_dirs.push(path);
                    }
                }
            }
        }
    }
    store.save(&ws)?;
    run(ctx, DetectRunRequest::default())
}

/// The cached report, or a fresh run when there is none. The flag says whether it was cached.
pub(crate) fn current_report(ctx: &Ctx) -> ManagerResult<(DetectionReport, bool)> {
    if let (Some(report), _) = read_cache(ctx) {
        return Ok((report, true));
    }
    Ok((run(ctx, DetectRunRequest::default())?.report, false))
}

/// The install the manager works with: the stored active install when the report has it, else the
/// install detection selected.
pub fn select_install<'a>(
    report: &'a DetectionReport,
    ws: &WorkspaceSettings,
) -> Option<&'a Install> {
    ws.active_install
        .as_deref()
        .and_then(|id| report.install(id))
        .or_else(|| report.selected_install())
}

fn cache_path(ctx: &Ctx) -> Utf8PathBuf {
    ctx.roots.path(RootKind::Cache).join(REPORT_FILE)
}

fn write_cache(ctx: &Ctx, report: &DetectionReport) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    atomic_write(&cache_path(ctx), &bytes).map_err(|e| e.to_string())
}

/// Reads the cache; the flag is true when a file existed and was ignored.
fn read_cache(ctx: &Ctx) -> (Option<DetectionReport>, bool) {
    let path = cache_path(ctx);
    let Ok(bytes) = std::fs::read(&path) else {
        return (None, false);
    };
    match serde_json::from_slice::<DetectionReport>(&bytes) {
        Ok(r) if r.schema == REPORT_SCHEMA => (Some(r), false),
        _ => (None, true),
    }
}

fn override_statuses(ctx: &Ctx, ws: &WorkspaceSettings) -> Vec<OverrideStatus> {
    let mut out = Vec::new();
    let mut push = |field: OverrideField, path: &Utf8Path, pinned: bool| {
        let verdict = normalize_path(path.as_str())
            .map_err(|e| e.to_string())
            .and_then(|p| validate_override(ctx, field, &p));
        out.push(OverrideStatus {
            field,
            path: path.to_owned(),
            pinned,
            valid: verdict.is_ok(),
            reason: verdict.err(),
        });
    };
    let p = &ws.paths;
    if let Some(o) = &p.game_install {
        push(OverrideField::GameInstall, &o.path, o.pinned);
    }
    if let Some(o) = &p.user_dir {
        push(OverrideField::UserDir, &o.path, o.pinned);
    }
    if let Some(o) = &p.steam_root {
        push(OverrideField::SteamRoot, &o.path, o.pinned);
    }
    for d in &p.extra_workshop_dirs {
        push(OverrideField::WorkshopDir, d, false);
    }
    out
}

fn is_dir(ctx: &Ctx, path: &Utf8Path) -> bool {
    ctx.fs.is_dir(path, PROBE_DEADLINE).unwrap_or(false)
}

fn exists(ctx: &Ctx, path: &Utf8Path) -> bool {
    ctx.fs.exists(path, PROBE_DEADLINE).unwrap_or(false)
}

/// True when `path` is an install folder: it has `Data/Core/About/About.xml`, directly or inside an
/// application bundle (a `*.app` folder, the macOS layout).
pub fn is_game_install(ctx: &Ctx, path: &Utf8Path) -> bool {
    if exists(ctx, &path.join(CONTENT_MARKER)) {
        return true;
    }
    let Ok(entries) = ctx.fs.read_dir(path, PROBE_DEADLINE) else {
        return false;
    };
    entries.iter().any(|e| {
        e.name.to_lowercase().ends_with(".app")
            && exists(ctx, &path.join(&e.name).join(CONTENT_MARKER))
    })
}

fn validate_override(ctx: &Ctx, field: OverrideField, path: &Utf8Path) -> Result<(), String> {
    if !is_dir(ctx, path) {
        return Err("the folder does not exist or is not a folder".to_owned());
    }
    match field {
        OverrideField::GameInstall => {
            if is_game_install(ctx, path) {
                Ok(())
            } else {
                Err(format!("the folder does not contain {CONTENT_MARKER}"))
            }
        }
        OverrideField::SteamRoot => {
            if is_dir(ctx, &path.join("steamapps")) {
                Ok(())
            } else {
                Err("the folder does not contain a steamapps folder".to_owned())
            }
        }
        OverrideField::UserDir | OverrideField::WorkshopDir => Ok(()),
    }
}
