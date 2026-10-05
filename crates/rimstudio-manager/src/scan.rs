//! Library scan use case over the detected and custom sources.
//!
//! [`library_scan`] builds the source set from detection plus the custom folders, runs
//! `rimstudio_library::Scanner` with the manifest cache in the cache root and returns counts,
//! diagnostics and timings together with the [`LibraryIndex`] that `rimstudio-app` keeps. A mod that
//! exists only in a custom folder is flagged not loadable (it needs a link or a copy) until it is
//! linked into the game `Mods` folder. The scan reads the file system and writes only the manifest.

use std::sync::Arc;

use rimstudio_core::diag::DiagnosticSummary;
use rimstudio_core::jobs::{CancelToken, ProgressSink};
use rimstudio_core::version::GameVersion;
use rimstudio_library::cache::{LoadStatus, Manifest, ManifestWriter, manifest_path};
use rimstudio_library::duplicates::{DuplicatePolicy, resolve};
use rimstudio_library::index::{AboutStatus, LibraryIndex, Loadability};
use rimstudio_library::scan::{
    ScanLevel, ScanOptions, ScanStats, ScanTimings, Scanner, SourceReport,
};
use serde::{Deserialize, Serialize};

use crate::ctx::Ctx;
use crate::detect::{DetectRunRequest, current_report, run, select_install};
use crate::error::{ManagerError, ManagerResult};
use crate::sources::effective_sources;

/// A request to scan the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryScanRequest {
    /// Run detection again and ignore the manifest, so every file is read afresh.
    pub full: bool,
    /// How much to read; the default also indexes definitions.
    pub level: ScanLevel,
    /// Worker threads; `None` uses `library.scanThreads` from the settings (0 is automatic).
    pub workers: Option<usize>,
}

impl Default for LibraryScanRequest {
    fn default() -> Self {
        LibraryScanRequest {
            full: false,
            level: ScanLevel::Definitions,
            workers: None,
        }
    }
}

/// Counts over the scanned library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    /// Mods in the index.
    pub mods: usize,
    /// Mods the game can load as they are.
    pub loadable: usize,
    /// Mods that exist only in a custom folder and need a link or a copy.
    pub needs_link: usize,
    /// Rows restored from the cache because their source is offline.
    pub unavailable: usize,
    /// Mods whose `About.xml` could not be parsed.
    pub unparsed_about: usize,
    /// Mods with a made up package id.
    pub synthetic_ids: usize,
    /// Definitions in the index.
    pub defs: usize,
    /// Package ids that appear more than once.
    pub duplicate_groups: usize,
}

/// What happened to the manifest cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestState {
    /// There was no manifest before the scan.
    Missing,
    /// The previous manifest was used.
    Used,
    /// The previous manifest was damaged or from another version and was ignored.
    Ignored,
    /// A full scan was asked for; the previous manifest was not read.
    Skipped,
}

/// The result of a scan.
#[derive(Debug, Clone)]
pub struct LibraryScanResult {
    /// The mods, to be kept by the caller.
    pub index: LibraryIndex,
    /// Counts over the index.
    pub counts: LibraryCounts,
    /// Diagnostic counts and the first samples of each code.
    pub diagnostics: DiagnosticSummary,
    /// How long each phase took.
    pub timings: ScanTimings,
    /// What the scan did (files parsed and reused).
    pub stats: ScanStats,
    /// One entry per source, in order.
    pub sources: Vec<SourceReport>,
    /// True when the scan was cancelled; the manifest is then not saved.
    pub cancelled: bool,
    /// The game version the scan narrowed definitions to, when the install has a readable one.
    pub game_version: Option<GameVersion>,
    /// What happened to the manifest before the scan.
    pub manifest_before: ManifestState,
    /// True when the manifest was written after the scan.
    pub manifest_saved: bool,
}

/// Scans every enabled source.
///
/// Uses the cached detection report (or runs detection when there is none, or when `full` is set).
/// A failure to write the manifest is logged and reported through `manifest_saved`, not an error:
/// the scan result is still good.
///
/// # Errors
/// [`ManagerError::NoSources`] when there is nothing to scan, or a store error when the workspace
/// document cannot be opened.
pub fn library_scan(
    ctx: &Ctx,
    req: LibraryScanRequest,
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> ManagerResult<LibraryScanResult> {
    let ws = ctx.workspace_store()?.load().value;
    let report = if req.full {
        run(ctx, DetectRunRequest::default())?.report
    } else {
        current_report(ctx)?.0
    };
    let set = effective_sources(&report, &ws);
    if set.enabled().next().is_none() {
        return Err(ManagerError::NoSources);
    }
    let game_version = select_install(&report, &ws)
        .and_then(|i| i.version.as_ref())
        .and_then(|v| GameVersion::parse(&v.raw).ok());
    let threads = ctx.settings_store()?.load().value.library.scan_threads;

    let mut opts = ScanOptions::default()
        .with_custom_folders(&ws.custom_mod_folders)
        .with_level(req.level);
    let configured = req.workers.unwrap_or(usize::from(threads));
    if configured > 0 {
        opts = opts.with_workers(configured);
    }
    if let Some(v) = &game_version {
        opts = opts.with_game_version(v.clone());
    }
    opts.file_id = ctx.file_id;
    opts.fs.clone_from(&ctx.scan_fs);
    let path = manifest_path(ctx.roots);
    let manifest_before = if req.full {
        ManifestState::Skipped
    } else {
        let loaded = rimstudio_library::cache::Manifest::load(&path);
        let state = match loaded.status {
            LoadStatus::Missing => ManifestState::Missing,
            LoadStatus::Loaded => ManifestState::Used,
            LoadStatus::Invalid(_) | LoadStatus::Outdated { .. } => ManifestState::Ignored,
        };
        if state == ManifestState::Used {
            opts = opts.with_previous(Arc::new(loaded.manifest));
        }
        state
    };

    let outcome = Scanner::scan(&set, &opts, progress, cancel);

    let manifest_saved = if outcome.cancelled {
        false
    } else {
        save_manifest(ctx, outcome.manifest.clone())
    };
    let counts = count(&outcome.index, &set, game_version.as_ref());
    Ok(LibraryScanResult {
        counts,
        diagnostics: outcome.diagnostics,
        timings: outcome.timings,
        stats: outcome.stats,
        sources: outcome.sources,
        cancelled: outcome.cancelled,
        index: outcome.index,
        game_version,
        manifest_before,
        manifest_saved,
    })
}

fn save_manifest(ctx: &Ctx, manifest: Manifest) -> bool {
    let mut writer = ManifestWriter::in_cache_root(ctx.roots, ctx.frozen_clock());
    let result = writer.offer(manifest).and_then(|_| writer.flush());
    match result {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(error = %e, "could not write the scan manifest");
            false
        }
    }
}

fn count(
    index: &LibraryIndex,
    set: &rimstudio_core::mods::SourceSet,
    version: Option<&GameVersion>,
) -> LibraryCounts {
    let mut c = LibraryCounts {
        mods: index.len(),
        loadable: index.loadable_count(),
        defs: index.def_count(),
        ..LibraryCounts::default()
    };
    for (_, _, info, _) in index.iter_full() {
        if info.loadable == Loadability::NeedsLink {
            c.needs_link += 1;
        }
        if !info.available {
            c.unavailable += 1;
        }
        if info.about == AboutStatus::Unparsed {
            c.unparsed_about += 1;
        }
        if info.synthetic_package_id {
            c.synthetic_ids += 1;
        }
    }
    let mut policy = DuplicatePolicy::from_sources(set);
    if let Some(v) = version {
        policy = policy.with_game_version(v.clone());
    }
    c.duplicate_groups = resolve(index, &policy).group_count();
    c
}
