//! The workspace session: a reference set loaded through the def engine.
//!
//! [`WorkspaceSession::open`] takes a [`ReferenceSet`] (the game, expansions, chosen mods and
//! optionally the project folder as the last pack) and does, in this order:
//!
//! 1. builds the def type table from the game and mod assemblies at run time (or takes the one
//!    given in [`OpenInput::type_table`]), through the cache when one is configured;
//! 2. reads and parses every Defs and Patches file in parallel through `rimstudio-xml`, skipping
//!    files whose stat key still matches the [`ParseCache`];
//! 3. runs the def engine (`rimstudio_defs::load`: merge, patch, inherit, build databases);
//! 4. builds the [`DefIndex`].
//!
//! Content problems are collected as diagnostics and never abort the open; only cancellation is an
//! error. The result is an immutable [`Snapshot`] plus the index. [`WorkspaceSession::is_stale`]
//! compares stat keys with what the snapshot was built from, and
//! [`WorkspaceSession::rebuild_changed`] re-reads only the changed files (everything else comes from
//! the parse cache) and swaps in a new snapshot with a higher revision.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::diag::{DiagSink, Diagnostic, DiagnosticSummary, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::jobs::{CancelToken, NoopProgress, Progress, ProgressSink, ProgressUnit};
use rimstudio_core::load_plan::{Subdir, collect_files};
use rimstudio_defs::{
    CustomRegistry, DefDatabases, FileContent, LoadInput, LoadOutput, ModEntry, PatchFile,
    TypeTable, load,
};
use rimstudio_io::statkey::StatKey;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::defindex::{DefIndex, DefIndexBuilder, Page, PageResult, Query};
use crate::error::{WorkspaceError, WorkspaceResult, codes};
use crate::listing::DiskListing;
use crate::parse::{FileKind, Outcome, ParseCache, stat_key};
use crate::project::{find_about_file, find_load_folders_file};
use crate::refset::{PackKind, ReferenceSet};
use crate::snapshot::{DefRef, FileRef, PackRef, ResolvedDefView, Snapshot};
use crate::typetable::{CacheConfig, assembly_paths, load_type_table};

/// What [`WorkspaceSession::open`] needs.
#[derive(Clone)]
pub struct OpenInput {
    /// The packs to load, in load order. Use [`OpenInput::with_project`] to add the project folder.
    pub reference: ReferenceSet,
    /// The game install folder, where the game assemblies are looked for. Without it (and without
    /// [`OpenInput::type_table`]) the base game's def types are unknown.
    pub game_dir: Option<Utf8PathBuf>,
    /// A ready type table. When set no assembly is read.
    pub type_table: Option<Arc<TypeTable>>,
    /// Where the type table cache lives. Without it the table is always rebuilt.
    pub cache: Option<CacheConfig>,
    /// A parse cache to share between sessions (a fresh one is made when `None`).
    pub parse_cache: Option<Arc<ParseCache>>,
    /// Handlers for custom patch operation classes (for example the Combat Extended settings
    /// conditional, registered by the crate that owns that knowledge).
    pub custom_ops: CustomRegistry,
    /// Package ids that count as active although no pack exists for them.
    pub extra_active_ids: Vec<String>,
    /// The number of worker threads (the global rayon pool when `None`).
    pub threads: Option<usize>,
}

impl std::fmt::Debug for OpenInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenInput")
            .field("packs", &self.reference.packs.len())
            .field("game_dir", &self.game_dir)
            .finish_non_exhaustive()
    }
}

impl OpenInput {
    /// An input for a reference set with every option at its default.
    #[must_use]
    pub fn new(reference: ReferenceSet) -> Self {
        OpenInput {
            reference,
            game_dir: None,
            type_table: None,
            cache: None,
            parse_cache: None,
            custom_ops: CustomRegistry::new(),
            extra_active_ids: Vec::new(),
            threads: None,
        }
    }

    /// Sets the game install folder.
    #[must_use]
    pub fn with_game_dir(mut self, dir: impl Into<Utf8PathBuf>) -> Self {
        self.game_dir = Some(dir.into());
        self
    }

    /// Supplies the type table.
    #[must_use]
    pub fn with_type_table(mut self, table: Arc<TypeTable>) -> Self {
        self.type_table = Some(table);
        self
    }

    /// Sets the type table cache.
    #[must_use]
    pub fn with_cache(mut self, cache: CacheConfig) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Shares a parse cache.
    #[must_use]
    pub fn with_parse_cache(mut self, cache: Arc<ParseCache>) -> Self {
        self.parse_cache = Some(cache);
        self
    }

    /// Sets the custom patch operation handlers.
    #[must_use]
    pub fn with_custom_ops(mut self, ops: CustomRegistry) -> Self {
        self.custom_ops = ops;
        self
    }

    /// Fixes the number of worker threads.
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads.max(1));
        self
    }

    /// Opens the session with the project folder as an extra, last pack, so the designer and the
    /// convert scans see the project's own defs.
    ///
    /// # Errors
    /// [`WorkspaceError::NotAProject`] when the folder has no About.xml, or an I/O error.
    pub fn with_project(mut self, project_root: &Utf8Path) -> WorkspaceResult<Self> {
        self.reference = self.reference.with_project(project_root, &DiskListing)?;
        Ok(self)
    }
}

/// A Defs or Patches file of the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// The pack the file belongs to (index into the reference set).
    pub pack: usize,
    /// Defs or Patches.
    pub kind: FileKind,
    /// The path relative to its load folder.
    pub relative: String,
    /// The absolute path.
    pub path: Utf8PathBuf,
    /// The stat key the file was read under (`None` when it could not be examined).
    pub key: Option<StatKey>,
}

/// Microseconds spent per phase of the last build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTimings {
    /// Building or reading the type table.
    pub types: u64,
    /// Reading and parsing the files.
    pub parse: u64,
    /// The def engine load.
    pub load: u64,
    /// Building the index.
    pub index: u64,
    /// The whole build.
    pub total: u64,
}

/// Counters of the last build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    /// Defs and Patches files in the session.
    pub files_total: usize,
    /// Files read and parsed by the last build.
    pub files_parsed: usize,
    /// Files answered by the parse cache in the last build.
    pub files_cached: usize,
    /// Nodes in the def index.
    pub index_entries: usize,
    /// True when the type table came from the cache (or was reused).
    pub type_table_cached: bool,
    /// DLLs parsed for the type table in the last build.
    pub assemblies_read: usize,
    /// DLLs skipped for the type table.
    pub assemblies_skipped: usize,
    /// Time per phase.
    pub timings: SessionTimings,
}

/// What [`WorkspaceSession::rebuild_changed`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    /// The revision of the new snapshot.
    pub revision: u64,
    /// Files parsed again.
    pub files_parsed: usize,
    /// Files in the session after the rebuild.
    pub files_total: usize,
    /// Packs whose file lists were listed again.
    pub packs_refreshed: usize,
}

#[derive(Clone)]
struct Config {
    game_dir: Option<Utf8PathBuf>,
    type_override: Option<Arc<TypeTable>>,
    cache: Option<CacheConfig>,
    custom_ops: CustomRegistry,
    extra_active_ids: Vec<String>,
    threads: Option<usize>,
}

/// The type table of a build with the keys it stands for.
#[derive(Clone)]
struct TableState {
    table: Arc<TypeTable>,
    keys: Vec<StatKey>,
}

struct Built {
    files: Vec<FileEntry>,
    table: TableState,
    watch: Vec<(Utf8PathBuf, Option<StatKey>)>,
    index: Arc<DefIndex>,
    snapshot: Arc<Snapshot>,
    diagnostics: DiagnosticSummary,
    stats: SessionStats,
}

/// A reference set loaded through the def engine, with its snapshot and index.
pub struct WorkspaceSession {
    config: Config,
    reference: ReferenceSet,
    parse_cache: Arc<ParseCache>,
    built: Built,
}

impl std::fmt::Debug for WorkspaceSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceSession")
            .field("packs", &self.reference.packs.len())
            .field("revision", &self.built.snapshot.revision)
            .field("files", &self.built.files.len())
            .finish_non_exhaustive()
    }
}

fn micros(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn in_pool<R: Send>(threads: Option<usize>, work: impl FnOnce() -> R + Send) -> R {
    match threads.and_then(|n| rayon::ThreadPoolBuilder::new().num_threads(n).build().ok()) {
        Some(pool) => pool.install(work),
        None => work(),
    }
}

fn pack_refs(reference: &ReferenceSet) -> Vec<PackRef> {
    reference
        .packs
        .iter()
        .enumerate()
        .map(|(index, p)| PackRef {
            index,
            package_id: p.package_id.clone(),
            name: p.name.clone(),
            kind: p.kind,
        })
        .collect()
}

fn file_table(reference: &ReferenceSet) -> Vec<FileEntry> {
    let mut files = Vec::with_capacity(reference.file_count());
    for (pack, p) in reference.packs.iter().enumerate() {
        for (kind, list) in [(FileKind::Defs, &p.defs), (FileKind::Patches, &p.patches)] {
            for f in list {
                files.push(FileEntry {
                    pack,
                    kind,
                    relative: f.relative.clone(),
                    path: f.path.clone(),
                    key: None,
                });
            }
        }
    }
    files
}

fn handle(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

fn project_watch(reference: &ReferenceSet) -> Vec<(Utf8PathBuf, Option<StatKey>)> {
    let Some(project) = reference.project() else {
        return Vec::new();
    };
    let about = find_about_file(&project.root)
        .unwrap_or_else(|| project.root.join("About").join("About.xml"));
    let lf = find_load_folders_file(&project.root)
        .unwrap_or_else(|| project.root.join(rimstudio_core::paths::LOAD_FOLDERS_XML));
    [about, lf]
        .into_iter()
        .map(|p| {
            let key = stat_key(&p);
            (p, key)
        })
        .collect()
}

/// What a build reads from and reports to.
struct BuildCtx<'a> {
    config: &'a Config,
    cache: &'a ParseCache,
    progress: &'a dyn ProgressSink,
    cancel: &'a CancelToken,
}

fn build(
    ctx: &BuildCtx<'_>,
    reference: &ReferenceSet,
    previous: Option<&TableState>,
    extra: Vec<Diagnostic>,
    revision: u64,
) -> WorkspaceResult<Built> {
    let BuildCtx {
        config,
        cache,
        progress,
        cancel,
    } = *ctx;
    let started = Instant::now();
    let mut timings = SessionTimings::default();
    let mut sink = DiagSink::new();
    for d in reference.diagnostics.iter().chain(extra.iter()) {
        sink.push(d.clone());
    }
    let mut files = file_table(reference);
    let packs = pack_refs(reference);
    let mut stats = SessionStats::default();

    // 1. the type table
    let t = Instant::now();
    progress.report(Progress::new("workspace.types", 0).with_total(1));
    let mut watch = project_watch(reference);
    let (types, table_state) = match &config.type_override {
        Some(table) => (
            Arc::clone(table),
            TableState {
                table: Arc::clone(table),
                keys: Vec::new(),
            },
        ),
        None => {
            let (paths, diags) = assembly_paths(config.game_dir.as_deref(), &reference.packs);
            for d in diags {
                sink.push(d);
            }
            let reuse = previous.map(|p| (p.keys.as_slice(), &p.table));
            let built = load_type_table(&paths, config.cache.as_ref(), reuse)?;
            for d in built.diagnostics {
                sink.push(d);
            }
            stats.type_table_cached = built.from_cache;
            stats.assemblies_read = built.assemblies_read;
            stats.assemblies_skipped = built.assemblies_skipped;
            for p in &paths {
                watch.push((p.clone(), stat_key(p)));
            }
            let state = TableState {
                table: Arc::clone(&built.table),
                keys: built.keys,
            };
            (built.table, state)
        }
    };
    timings.types = micros(t);
    progress.report(Progress::new("workspace.types", 1).with_total(1));
    cancel.check().map_err(|_| WorkspaceError::Cancelled)?;

    // 2. read and parse the files
    let t = Instant::now();
    let total = files.len();
    let counter = AtomicUsize::new(0);
    let fetched: Vec<Option<crate::parse::Fetched>> = files
        .par_iter()
        .map(|f| {
            if cancel.is_cancelled() {
                return None;
            }
            let got = cache.fetch(&f.path, f.kind);
            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_multiple_of(64) || n == total {
                progress.report(
                    Progress::new("workspace.parse", n as u64)
                        .with_total(total as u64)
                        .with_unit(ProgressUnit::Files),
                );
            }
            Some(got)
        })
        .collect();
    cancel.check().map_err(|_| WorkspaceError::Cancelled)?;
    let fetched: Vec<crate::parse::Fetched> = fetched.into_iter().flatten().collect();
    timings.parse = micros(t);

    // diagnostics, keys and the engine input
    let mut parsed_now = 0usize;
    for (i, (entry, got)) in files.iter_mut().zip(&fetched).enumerate() {
        entry.key = got.file.key.clone();
        if got.parsed {
            parsed_now += 1;
        }
        let mod_idx = ModIdx(handle(entry.pack));
        let file_id = FileId(handle(i));
        if let Some(problem) = &got.problem {
            sink.push(problem.clone().with_mod(mod_idx).with_file(file_id));
        }
        if let Outcome::Parsed { diagnostics, .. } = &got.file.outcome {
            for d in diagnostics {
                sink.push(d.clone().with_mod(mod_idx).with_file(file_id));
            }
        }
    }
    stats.files_total = total;
    stats.files_parsed = parsed_now;
    stats.files_cached = total.saturating_sub(parsed_now);

    let contents: Vec<FileContent> = fetched
        .par_iter()
        .map(|got| match &got.file.outcome {
            Outcome::Parsed { root, .. } => FileContent::parsed(root.clone()),
            Outcome::Failed { message } => FileContent::failed(message.clone()),
        })
        .collect();
    let mods: Vec<ModEntry> = reference
        .packs
        .iter()
        .enumerate()
        .map(|(i, p)| ModEntry::new(ModIdx(handle(i)), p.package_id.clone(), p.name.clone()))
        .collect();
    let mut input = LoadInput::new(mods, types);
    input.extra_active_ids = config.extra_active_ids.clone();
    input.custom_ops = config.custom_ops.clone();
    for (i, (entry, content)) in files.iter().zip(contents).enumerate() {
        let mod_idx = ModIdx(handle(entry.pack));
        let file = FileId(handle(i));
        match entry.kind {
            FileKind::Defs => input.def_files.push(rimstudio_defs::DefFile {
                mod_idx,
                file,
                rel_path: entry.relative.clone(),
                content,
            }),
            FileKind::Patches => input.patch_files.push(PatchFile {
                mod_idx,
                file,
                rel_path: entry.relative.clone(),
                content,
            }),
        }
    }

    // 3. the def engine
    let t = Instant::now();
    progress.report(Progress::new("workspace.load", 0).with_total(1));
    let LoadOutput {
        defs,
        databases,
        patch_report,
        diagnostics: engine_diagnostics,
        stats: load_stats,
        timings: load_timings,
        order,
    } = load(input);
    progress.report(Progress::new("workspace.load", 1).with_total(1));
    timings.load = micros(t);
    cancel.check().map_err(|_| WorkspaceError::Cancelled)?;

    // 4. the index
    let t = Instant::now();
    let mut builder = DefIndexBuilder::new(packs.clone());
    for (entry, got) in files.iter().zip(&fetched) {
        let id = builder.add_file(entry.pack, &entry.relative, &entry.path);
        if let Outcome::Parsed { defs, .. } = &got.file.outcome {
            for d in defs {
                builder.add_def(id, d);
            }
        }
    }
    let index = builder.build();
    stats.index_entries = index.len();
    timings.index = micros(t);

    let file_refs: Vec<FileRef> = files
        .iter()
        .enumerate()
        .map(|(i, f)| FileRef {
            id: handle(i),
            pack: f.pack,
            relative: f.relative.clone(),
            path: f.path.clone(),
        })
        .collect();
    let snapshot = Snapshot {
        revision,
        databases: Arc::new(databases),
        defs,
        stats: load_stats,
        timings: load_timings,
        diagnostics: engine_diagnostics,
        patch_report,
        order,
        packs,
        files: file_refs,
    };
    timings.total = micros(started);
    stats.timings = timings;
    Ok(Built {
        files,
        table: table_state,
        watch,
        index: Arc::new(index),
        snapshot: Arc::new(snapshot),
        diagnostics: sink.finish(),
        stats,
    })
}

impl WorkspaceSession {
    /// Opens a session: types, parse, load and index (see the module documentation).
    ///
    /// Progress phases reported: `workspace.types`, `workspace.parse` (files), `workspace.load`.
    ///
    /// # Errors
    /// [`WorkspaceError::Cancelled`] when the token is cancelled before the build finishes; content
    /// problems are diagnostics, not errors.
    pub fn open(
        input: OpenInput,
        progress: &dyn ProgressSink,
        cancel: &CancelToken,
    ) -> WorkspaceResult<WorkspaceSession> {
        let OpenInput {
            reference,
            game_dir,
            type_table,
            cache,
            parse_cache,
            custom_ops,
            extra_active_ids,
            threads,
        } = input;
        let config = Config {
            game_dir,
            type_override: type_table,
            cache,
            custom_ops,
            extra_active_ids,
            threads,
        };
        let parse_cache = parse_cache.unwrap_or_default();
        let ctx = BuildCtx {
            config: &config,
            cache: &parse_cache,
            progress,
            cancel,
        };
        let built = in_pool(config.threads, || {
            build(&ctx, &reference, None, Vec::new(), 1)
        })?;
        Ok(WorkspaceSession {
            config,
            reference,
            parse_cache,
            built,
        })
    }

    /// Opens a session without progress reporting or cancellation.
    ///
    /// # Errors
    /// Never cancelled; the result type is shared with [`WorkspaceSession::open`].
    pub fn open_simple(input: OpenInput) -> WorkspaceResult<WorkspaceSession> {
        Self::open(input, &NoopProgress, &CancelToken::new())
    }

    /// The def index.
    #[must_use]
    pub fn index(&self) -> Arc<DefIndex> {
        Arc::clone(&self.built.index)
    }

    /// The def databases of the current snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Arc<DefDatabases> {
        Arc::clone(&self.built.snapshot.databases)
    }

    /// The current snapshot with its provenance tables.
    #[must_use]
    pub fn current(&self) -> Arc<Snapshot> {
        Arc::clone(&self.built.snapshot)
    }

    /// The revision of the current snapshot (1 after open, plus one per rebuild).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.built.snapshot.revision
    }

    /// A def after patches and inheritance, with its provenance.
    #[must_use]
    pub fn resolve_def(&self, def: &DefRef) -> Option<ResolvedDefView> {
        self.built.snapshot.resolve_def(def)
    }

    /// Searches the def index.
    #[must_use]
    pub fn search(&self, query: &Query, page: Page) -> PageResult {
        self.built.index.search(query, page)
    }

    /// The reference set the session was built from (with the project pack when there is one).
    #[must_use]
    pub fn reference(&self) -> &ReferenceSet {
        &self.reference
    }

    /// The Defs and Patches files of the session, by file handle.
    #[must_use]
    pub fn files(&self) -> &[FileEntry] {
        &self.built.files
    }

    /// The file with this handle.
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&FileEntry> {
        self.built.files.get(id.index())
    }

    /// The workspace level diagnostics: reference set problems, type table problems, unreadable
    /// files and the XML warnings of parsed files. The def engine's own diagnostics are in
    /// [`Snapshot::diagnostics`].
    #[must_use]
    pub fn diagnostics(&self) -> &DiagnosticSummary {
        &self.built.diagnostics
    }

    /// The counters and timings of the last build.
    #[must_use]
    pub fn stats(&self) -> &SessionStats {
        &self.built.stats
    }

    /// The def type table in use.
    #[must_use]
    pub fn type_table(&self) -> Arc<TypeTable> {
        Arc::clone(&self.built.table.table)
    }

    /// The parse cache (share it with the next session to skip unchanged files).
    #[must_use]
    pub fn parse_cache(&self) -> Arc<ParseCache> {
        Arc::clone(&self.parse_cache)
    }

    /// True when a file the snapshot was built from changed, vanished or appeared (compared by stat
    /// key), when the project's file list changed, or when its About, LoadFolders or an assembly
    /// changed. Reads no file content.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        let file_changed = self
            .built
            .files
            .par_iter()
            .any(|f| match (&f.key, stat_key(&f.path)) {
                (Some(old), Some(new)) => *old != new,
                (None, None) => false,
                _ => true,
            });
        if file_changed {
            return true;
        }
        if self
            .built
            .watch
            .iter()
            .any(|(path, old)| stat_key(path) != *old)
        {
            return true;
        }
        let Some(project) = self.reference.project() else {
            return false;
        };
        let pack = self.reference.packs.len().saturating_sub(1);
        let known: Vec<(&str, &Utf8Path)> = self
            .built
            .files
            .iter()
            .filter(|f| f.pack == pack)
            .map(|f| (f.relative.as_str(), f.path.as_path()))
            .collect();
        let mut now: Vec<(String, Utf8PathBuf)> = Vec::new();
        for subdir in [Subdir::Defs, Subdir::Patches] {
            for f in collect_files(&project.plan, subdir, &DiskListing) {
                now.push((f.relative, f.path));
            }
        }
        let mut a: Vec<(&str, &Utf8Path)> = known;
        let mut b: Vec<(&str, &Utf8Path)> =
            now.iter().map(|(r, p)| (r.as_str(), p.as_path())).collect();
        a.sort();
        b.sort();
        a != b
    }

    /// Re-reads the given changed files and rebuilds the snapshot and index.
    ///
    /// The packs that contain a changed path list their files again (so new and deleted files are
    /// seen; the project pack also re-reads About and LoadFolders), the changed files are parsed
    /// again, every other file comes from the parse cache, and the def engine runs on the whole
    /// set. The def type table is rebuilt only when an assembly changed.
    ///
    /// # Errors
    /// Never cancelled; the result type is shared with [`WorkspaceSession::rebuild_changed_with`].
    pub fn rebuild_changed(&mut self, changed: &[Utf8PathBuf]) -> WorkspaceResult<RebuildReport> {
        self.rebuild_changed_with(changed, &NoopProgress, &CancelToken::new())
    }

    /// [`WorkspaceSession::rebuild_changed`] with progress and cancellation. A cancelled rebuild
    /// leaves the session as it was.
    ///
    /// # Errors
    /// [`WorkspaceError::Cancelled`].
    pub fn rebuild_changed_with(
        &mut self,
        changed: &[Utf8PathBuf],
        progress: &dyn ProgressSink,
        cancel: &CancelToken,
    ) -> WorkspaceResult<RebuildReport> {
        let mut reference = self.reference.clone();
        let mut extra = Vec::new();
        let mut affected: Vec<usize> = Vec::new();
        for path in changed {
            self.parse_cache.invalidate(path);
            for (i, pack) in reference.packs.iter().enumerate() {
                if path.starts_with(&pack.root) && !affected.contains(&i) {
                    affected.push(i);
                }
            }
        }
        affected.sort_unstable();
        for &pos in &affected {
            if let Err(e) = reference.refresh_pack(pos, &DiskListing) {
                extra.push(Diagnostic::new(
                    codes::PROJECT_ABOUT_UNREADABLE,
                    Severity::Warning,
                    format!("the pack could not be resolved again, the earlier plan is kept: {e}"),
                ));
            }
        }
        let revision = self.built.snapshot.revision + 1;
        let previous = Some(self.built.table.clone());
        let config = self.config.clone();
        let cache = Arc::clone(&self.parse_cache);
        let ctx = BuildCtx {
            config: &config,
            cache: &cache,
            progress,
            cancel,
        };
        let built = in_pool(config.threads, || {
            build(&ctx, &reference, previous.as_ref(), extra, revision)
        })?;
        let report = RebuildReport {
            revision,
            files_parsed: built.stats.files_parsed,
            files_total: built.stats.files_total,
            packs_refreshed: affected.len(),
        };
        self.reference = reference;
        self.built = built;
        Ok(report)
    }

    /// The pack kind counts of the session, for summaries: (core, dlc, mods, project).
    #[must_use]
    pub fn pack_counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for p in &self.reference.packs {
            match p.kind {
                PackKind::Core => c.0 += 1,
                PackKind::Dlc => c.1 += 1,
                PackKind::Mod => c.2 += 1,
                PackKind::Project => c.3 += 1,
            }
        }
        c
    }

    /// The files of one pack.
    #[must_use]
    pub fn files_of(&self, pack: usize) -> Vec<&FileEntry> {
        self.built.files.iter().filter(|f| f.pack == pack).collect()
    }

    /// A map from absolute file path to file handle.
    #[must_use]
    pub fn file_handles(&self) -> FxHashMap<&Utf8Path, FileId> {
        self.built
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.path.as_path(), FileId(handle(i))))
            .collect()
    }
}
