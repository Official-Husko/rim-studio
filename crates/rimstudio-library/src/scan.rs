//! The two level library scanner.
//!
//! [`Scanner::scan`] turns a [`SourceSet`] into a [`LibraryIndex`]:
//!
//! - **Discovery** lists each enabled source and finds the mod roots (`About/About.xml`, with the
//!   `About` folder matched ignoring case), following the layout and depth of custom folders.
//!   The same folder reached through two sources is listed once, under the first source.
//! - **Level 0** reads `About.xml` leniently and `LoadFolders.xml`, lists the folders of each mod
//!   and notes the icon and preview. This is all the mod list needs.
//! - **Level 1** (when [`ScanOptions::level`] is [`ScanLevel::Definitions`]) searches the content
//!   folders the game could load, indexes every definition file through `rimstudio-xml`, and
//!   records patch files, language folders and assemblies.
//!
//! Work runs on at most eight rayon workers. Symbolic links below a source root are never followed.
//! Problems are diagnostics and counts, never errors: a broken mod does not stop the scan. With a
//! previous [`Manifest`] in the options, files whose [`StatKey`]
//! is unchanged are not read again, and the outcome carries the new manifest for the caller to
//! store. The index, the diagnostics and the manifest are identical for any worker count.
//!
//! Cancelling keeps what is done: the outcome has `cancelled` set, the index holds the mods
//! processed so far (a cancelled level 1 leaves the level 0 list intact) and the manifest keeps
//! every file already indexed.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::diag::{DiagSink, Diagnostic, DiagnosticSummary, Severity};
use rimstudio_core::ids::{PackageId, SourceId, WorkshopId};
use rimstudio_core::jobs::{CancelToken, Progress, ProgressSink, ProgressUnit};
use rimstudio_core::mods::{ActiveSet, ModMeta, ModSource, SourceKind, SourceSet};
use rimstudio_core::os::Os;
use rimstudio_core::ports::FsProbe;
use rimstudio_core::settings::CustomFolder;
use rimstudio_core::version::GameVersion;
use rimstudio_io::statkey::{FatPolicy, FileIdFn, StatKey};
use rimstudio_xml::about::{About, AboutRead, read_lenient};
use rimstudio_xml::load_folders;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::cache::{AboutCache, FileStamp, LoadFoldersCache, Manifest, ModCache, file_unchanged};
use crate::content::{Level1Ctx, index_mod};
use crate::discover::{Found, discover_source};
use crate::error::codes;
use crate::index::{
    AboutStatus, DefFile, LibraryIndex, LibraryMod, Loadability, ModContent, ModInfo,
};
use crate::sources::{FolderSpec, is_visible_to_game};

/// `About.xml` and `LoadFolders.xml` files larger than this are not read.
const MAX_SMALL_XML_BYTES: u64 = 8 * 1024 * 1024;
/// A progress record is sent every this many mods.
const PROGRESS_EVERY: usize = 32;
/// The most workers a scan uses.
pub const MAX_WORKERS: usize = 8;

/// How much a scan reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanLevel {
    /// Mod roots, `About.xml`, `LoadFolders.xml` and the folder listing: enough for the mod list.
    Metadata,
    /// Level 0 plus the content folders, markers and the definition index.
    Definitions,
}

/// Settings of one scan.
#[derive(Clone)]
pub struct ScanOptions {
    /// Worker threads, clamped to `1..=8`.
    pub workers: usize,
    /// How much to read.
    pub level: ScanLevel,
    /// The game version, which narrows level 1 to the folders the game would load. Without it every
    /// version folder is searched.
    pub game_version: Option<GameVersion>,
    /// Scan specs of custom sources by id; a custom source without an entry uses the default
    /// (`Auto`, depth 1).
    pub folders: BTreeMap<SourceId, FolderSpec>,
    /// The manifest of an earlier scan; unchanged files are taken from it.
    pub previous: Option<Arc<Manifest>>,
    /// The file id hook (inode and device) that `rimstudio-platform` supplies; optional.
    pub file_id: Option<FileIdFn>,
    /// Used to classify the volume of each source (FAT family volumes compare time stamps with
    /// rounding and a hash). Without it every volume is compared exactly.
    pub fs: Option<Arc<dyn FsProbe>>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
        ScanOptions {
            workers: cores.min(MAX_WORKERS),
            level: ScanLevel::Definitions,
            game_version: None,
            folders: BTreeMap::new(),
            previous: None,
            file_id: None,
            fs: None,
        }
    }
}

impl std::fmt::Debug for ScanOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanOptions")
            .field("workers", &self.workers)
            .field("level", &self.level)
            .field("game_version", &self.game_version)
            .field("folders", &self.folders)
            .field("has_previous", &self.previous.is_some())
            .finish_non_exhaustive()
    }
}

impl ScanOptions {
    /// Adds the scan specs of custom folders (layout and depth by source id).
    #[must_use]
    pub fn with_custom_folders(mut self, folders: &[CustomFolder]) -> Self {
        for f in folders {
            self.folders
                .insert(f.id.clone(), FolderSpec::from_custom(f));
        }
        self
    }

    /// Sets the level.
    #[must_use]
    pub fn with_level(mut self, level: ScanLevel) -> Self {
        self.level = level;
        self
    }

    /// Sets the worker count (clamped to `1..=8` when the scan runs).
    #[must_use]
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    /// Sets the game version.
    #[must_use]
    pub fn with_game_version(mut self, version: GameVersion) -> Self {
        self.game_version = Some(version);
        self
    }

    /// Sets the manifest of the previous scan.
    #[must_use]
    pub fn with_previous(mut self, manifest: Arc<Manifest>) -> Self {
        self.previous = Some(manifest);
        self
    }

    fn spec_for(&self, source: &ModSource) -> FolderSpec {
        if source.kind == SourceKind::Custom {
            self.folders.get(&source.id).copied().unwrap_or_default()
        } else {
            FolderSpec::mods_root()
        }
    }

    fn policy_for(&self, source: &ModSource) -> FatPolicy {
        match &self.fs {
            Some(fs) => FatPolicy::for_volume(fs.volume_class(&source.path)),
            None => FatPolicy::Exact,
        }
    }
}

/// Counts of what a scan did. All values except timings are deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    /// Folders listed during discovery.
    pub folders_probed: u64,
    /// Mod roots found, before duplicates of the same folder were removed.
    pub mods_found: u64,
    /// Mods in the index (restored offline rows included).
    pub mods_indexed: u64,
    /// `About.xml` files read and parsed.
    pub about_parsed: u64,
    /// `About.xml` results taken from the manifest.
    pub about_reused: u64,
    /// `LoadFolders.xml` files read and parsed.
    pub load_folders_parsed: u64,
    /// `LoadFolders.xml` results taken from the manifest.
    pub load_folders_reused: u64,
    /// Definition files read and indexed.
    pub def_files_parsed: u64,
    /// Definition files taken from the manifest.
    pub def_files_reused: u64,
    /// Definitions in the index.
    pub defs: u64,
    /// Files hashed to settle a FAT time stamp difference.
    pub files_hashed: u64,
    /// Bytes read from definition files.
    pub bytes_read: u64,
    /// Folders or entries that could not be read.
    pub dir_errors: u64,
    /// Names skipped because they are not UTF-8.
    pub non_utf8: u64,
}

impl ScanStats {
    /// The number of files that were read and parsed: `About.xml`, `LoadFolders.xml` and
    /// definition files. Zero for a rescan of an unchanged library.
    pub fn files_parsed(&self) -> u64 {
        self.about_parsed + self.load_folders_parsed + self.def_files_parsed
    }
}

/// How long each phase took. Timings are the only non deterministic part of an outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanTimings {
    /// Listing the sources and finding mod roots.
    pub discover: Duration,
    /// Level 0.
    pub metadata: Duration,
    /// Level 1 (zero when it did not run).
    pub definitions: Duration,
    /// The whole scan.
    pub total: Duration,
}

/// What state a source was in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceStatus {
    /// The folder was listed.
    Ready,
    /// The source is disabled and was not scanned.
    Disabled,
    /// The folder is missing or cannot be reached; cached mods stay listed as unavailable.
    Offline,
    /// The path exists and is not a folder.
    NotDirectory,
}

/// What happened to one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    /// The source id.
    pub id: SourceId,
    /// The source kind.
    pub kind: SourceKind,
    /// The folder.
    pub path: Utf8PathBuf,
    /// The state of the folder.
    pub status: SourceStatus,
    /// Mods in the index that belong to this source.
    pub mods: usize,
}

/// The result of a scan.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    /// The mods. Dereferences to the core [`ModIndex`](rimstudio_core::mods::ModIndex).
    pub index: LibraryIndex,
    /// Counts per code and the first samples of each code, sorted.
    pub diagnostics: DiagnosticSummary,
    /// How long each phase took.
    pub timings: ScanTimings,
    /// What the scan did.
    pub stats: ScanStats,
    /// One entry per source of the set, in order.
    pub sources: Vec<SourceReport>,
    /// The manifest to store for the next scan.
    pub manifest: Manifest,
    /// True when the scan was cancelled before it finished.
    pub cancelled: bool,
}

/// The scanner. It has no state: everything comes in through [`Scanner::scan`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Scanner;

/// One mod root that survived the removal of repeated folders.
struct Item<'a> {
    found: Found,
    source: &'a ModSource,
    policy: FatPolicy,
    also_in: Vec<SourceId>,
    also_kinds: Vec<SourceKind>,
}

#[derive(Default)]
struct L0Counters {
    about_parsed: u64,
    about_reused: u64,
    lf_parsed: u64,
    lf_reused: u64,
    hashed: u64,
}

struct L0 {
    meta: ModMeta,
    info: ModInfo,
    about: Option<AboutCache>,
    load_folders: Option<LoadFoldersCache>,
    diagnostics: Vec<Diagnostic>,
    counters: L0Counters,
}

fn run_in_pool<R: Send>(pool: Option<&rayon::ThreadPool>, f: impl FnOnce() -> R + Send) -> R {
    match pool {
        Some(p) => p.install(f),
        None => f(),
    }
}

fn report(sink: &dyn ProgressSink, phase: &str, done: usize, total: usize, unit: ProgressUnit) {
    sink.report(
        Progress::new(phase, done as u64)
            .with_total(total as u64)
            .with_unit(unit),
    );
}

fn canonical_of(path: &Utf8Path) -> Utf8PathBuf {
    fs_err::canonicalize(path)
        .ok()
        .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
        .unwrap_or_else(|| path.to_owned())
}

fn fold_key(path: &Utf8Path) -> String {
    if Os::current().case_insensitive_default() {
        path.as_str().to_lowercase()
    } else {
        path.as_str().to_owned()
    }
}

fn stat_key(abs: &Utf8Path, rel: &str, hook: Option<FileIdFn>) -> std::io::Result<StatKey> {
    let meta = fs_err::metadata(abs)?;
    Ok(StatKey::from_metadata(rel, &meta, hook))
}

/// A package id for a mod that declares none: the shape `<author>.<name>` with every character that
/// is not an ASCII letter or digit replaced, and three digits taken from a hash of the description
/// so that two unnamed mods by one author differ. It is stable but deliberately not the id the game
/// would make up, so the mod is marked [`ModInfo::synthetic_package_id`].
pub(crate) fn synthesize_package_id(about: &About, folder: &str) -> String {
    fn ascii(text: &str) -> String {
        text.chars()
            .take(40)
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c
                } else {
                    char::from(b'A' + (u32::from(c) % 25) as u8)
                }
            })
            .collect()
    }
    let author = about
        .author
        .clone()
        .filter(|a| !a.trim().is_empty())
        .or_else(|| about.authors.first().cloned())
        .unwrap_or_else(|| "none".to_owned());
    let digits: String = if about.description.is_empty() {
        String::new()
    } else {
        let hash = blake3::hash(about.description.as_bytes());
        let n = u32::from_le_bytes([
            hash.as_bytes()[0],
            hash.as_bytes()[1],
            hash.as_bytes()[2],
            0,
        ]);
        format!("{:03}", n % 1000)
    };
    let name = if about.name.trim().is_empty() {
        folder
    } else {
        about.name.trim()
    };
    format!("{}.{}", ascii(&format!("{author}{digits}")), ascii(name))
}

/// Builds the metadata of a mod from what `About.xml` said. Returns the metadata and whether the
/// package id had to be made up; `None` only if no usable id can be formed at all (the parser
/// accepts every trimmed text without control characters, so this does not happen in practice).
fn meta_from_about(
    about: About,
    source: &SourceId,
    path: &Utf8Path,
    folder: &str,
) -> Option<(ModMeta, bool)> {
    let mut about = about;
    let mut synthetic = false;
    if PackageId::parse(&about.package_id).is_err() {
        about.package_id = synthesize_package_id(&about, folder);
        synthetic = true;
    }
    match about.clone().into_meta(source.clone(), path.to_owned()) {
        Ok(meta) => Some((meta, synthetic)),
        Err(_) => {
            let id = PackageId::parse(&synthesize_package_id(&about, folder))
                .or_else(|_| PackageId::parse("unknown.mod"))
                .ok()?;
            Some((
                ModMeta::new(id, folder, source.clone(), path.to_owned()),
                true,
            ))
        }
    }
}

fn read_published_id(path: &Utf8Path) -> Option<WorkshopId> {
    let meta = fs_err::metadata(path).ok()?;
    if meta.len() > 1024 {
        return None;
    }
    let text = fs_err::read_to_string(path).ok()?;
    WorkshopId::parse(text.lines().next().unwrap_or("")).ok()
}

fn level0(item: &Item<'_>, prev: Option<&ModCache>, opts: &ScanOptions) -> Option<L0> {
    let found = &item.found;
    let probe = &found.probe;
    let path = &found.path;
    let source_id = &item.source.id;
    let kind = item.source.kind;
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut counters = L0Counters::default();

    let about_dir = probe.about_dir.clone().unwrap_or_default();
    if about_dir != rimstudio_core::paths::ABOUT_DIR {
        diagnostics.push(
            Diagnostic::new(
                codes::ABOUT_DIR_CASE,
                Severity::Warning,
                "The About folder is not spelled About; the game may not find it on a case sensitive file system.",
            )
            .with_arg("folder", about_dir.clone()),
        );
    }
    let xml_name = probe.about.xml.clone().unwrap_or_default();
    let about_rel = format!("{about_dir}/{xml_name}");
    let about_abs = path.join(&about_rel);

    // About.xml: from the manifest when the key matches, else read and parse.
    let mut about_mtime = None;
    let parsed: bool;
    let synthetic: bool;
    let mut about_cache: Option<AboutCache> = None;
    let mut meta: ModMeta;
    match stat_key(&about_abs, &about_rel, opts.file_id) {
        Err(e) => {
            diagnostics.push(
                Diagnostic::new(
                    codes::ABOUT_UNREADABLE,
                    Severity::Error,
                    "About.xml could not be read.",
                )
                .with_arg("error", e.to_string()),
            );
            parsed = false;
            let (m, s) = meta_from_about(About::default(), source_id, path, &found.folder_name)?;
            meta = m;
            synthetic = s;
        }
        Ok(key) => {
            about_mtime = key.mtime_ns;
            let reused = prev.and_then(|p| p.about.as_ref()).and_then(|old| {
                let (same, fresh) = file_unchanged(item.policy, &old.stamp, &key, &about_abs);
                if fresh.is_some() {
                    counters.hashed += 1;
                }
                same.then_some((old, fresh))
            });
            if let Some((old, fresh)) = reused {
                counters.about_reused += 1;
                meta = old.meta.clone();
                meta.path = path.clone();
                meta.source = source_id.clone();
                parsed = old.parsed;
                synthetic = old.synthetic_id;
                diagnostics.extend(old.warnings.iter().cloned());
                about_cache = Some(AboutCache {
                    stamp: FileStamp {
                        key,
                        hash: fresh.or_else(|| old.stamp.hash.clone()),
                    },
                    meta: old.meta.clone(),
                    parsed,
                    synthetic_id: synthetic,
                    warnings: old.warnings.clone(),
                });
            } else {
                let mut warnings: Vec<Diagnostic> = Vec::new();
                let mut hash = None;
                let read: AboutRead = if key.size > MAX_SMALL_XML_BYTES {
                    diagnostics.push(Diagnostic::new(
                        codes::ABOUT_UNREADABLE,
                        Severity::Error,
                        "About.xml is too large to read.",
                    ));
                    AboutRead {
                        about: About::default(),
                        parsed: false,
                        warnings: Vec::new(),
                    }
                } else {
                    match fs_err::read(&about_abs) {
                        Ok(bytes) => {
                            if item.policy == FatPolicy::Fat {
                                hash = Some(blake3::hash(&bytes).to_hex().to_string());
                            }
                            counters.about_parsed += 1;
                            read_lenient(&bytes)
                        }
                        Err(e) => {
                            diagnostics.push(
                                Diagnostic::new(
                                    codes::ABOUT_UNREADABLE,
                                    Severity::Error,
                                    "About.xml could not be read.",
                                )
                                .with_arg("error", e.to_string()),
                            );
                            AboutRead {
                                about: About::default(),
                                parsed: false,
                                warnings: Vec::new(),
                            }
                        }
                    }
                };
                parsed = read.parsed;
                warnings.extend(read.warnings);
                let (m, s) = meta_from_about(read.about, source_id, path, &found.folder_name)?;
                meta = m;
                synthetic = s;
                diagnostics.extend(warnings.iter().cloned());
                about_cache = Some(AboutCache {
                    stamp: FileStamp { key, hash },
                    meta: meta.clone(),
                    parsed,
                    synthetic_id: synthetic,
                    warnings,
                });
            }
        }
    }
    if !parsed {
        diagnostics.push(Diagnostic::new(
            codes::ABOUT_UNPARSED,
            Severity::Error,
            "About.xml could not be parsed; the mod is listed with defaults.",
        ));
    }
    if synthetic {
        diagnostics.push(
            Diagnostic::new(
                codes::PACKAGE_ID_MISSING,
                Severity::Warning,
                "About.xml has no usable packageId; a stand-in id is used.",
            )
            .with_arg("packageId", meta.package_id.as_str()),
        );
    }

    // LoadFolders.xml.
    let mut lf_cache: Option<LoadFoldersCache> = None;
    meta.load_folders = None;
    if let Some(name) = &probe.load_folders {
        let abs = path.join(name);
        if let Ok(key) = stat_key(&abs, name, opts.file_id) {
            let reused = prev.and_then(|p| p.load_folders.as_ref()).and_then(|old| {
                let (same, fresh) = file_unchanged(item.policy, &old.stamp, &key, &abs);
                if fresh.is_some() {
                    counters.hashed += 1;
                }
                same.then_some((old, fresh))
            });
            if let Some((old, fresh)) = reused {
                counters.lf_reused += 1;
                diagnostics.extend(old.warnings.iter().cloned());
                meta.load_folders = old.spec.clone();
                lf_cache = Some(LoadFoldersCache {
                    stamp: FileStamp {
                        key,
                        hash: fresh.or_else(|| old.stamp.hash.clone()),
                    },
                    spec: old.spec.clone(),
                    warnings: old.warnings.clone(),
                });
            } else {
                let mut spec = None;
                let mut warnings = Vec::new();
                let mut hash = None;
                match if key.size <= MAX_SMALL_XML_BYTES {
                    fs_err::read(&abs)
                } else {
                    Err(std::io::Error::other("file too large"))
                } {
                    Ok(bytes) => {
                        if item.policy == FatPolicy::Fat {
                            hash = Some(blake3::hash(&bytes).to_hex().to_string());
                        }
                        counters.lf_parsed += 1;
                        match load_folders::read(&bytes) {
                            Ok(r) => {
                                spec = Some(r.spec);
                                warnings = r.diagnostics;
                            }
                            Err(e) => warnings.push(
                                Diagnostic::new(
                                    codes::LOAD_FOLDERS_UNREADABLE,
                                    Severity::Warning,
                                    "LoadFolders.xml could not be parsed and is ignored.",
                                )
                                .with_arg("error", e.to_string()),
                            ),
                        }
                    }
                    Err(e) => warnings.push(
                        Diagnostic::new(
                            codes::LOAD_FOLDERS_UNREADABLE,
                            Severity::Warning,
                            "LoadFolders.xml could not be read and is ignored.",
                        )
                        .with_arg("error", e.to_string()),
                    ),
                }
                diagnostics.extend(warnings.iter().cloned());
                meta.load_folders = spec.clone();
                lf_cache = Some(LoadFoldersCache {
                    stamp: FileStamp { key, hash },
                    spec,
                    warnings,
                });
            }
        }
    }

    // Folder facts that are listed on every scan.
    meta.root_dirs = probe.dirs.iter().map(|d| d.name.clone()).collect();
    meta.icon_path = probe
        .about
        .icon
        .as_ref()
        .map(|n| path.join(&about_dir).join(n));
    meta.preview_path = probe
        .about
        .preview
        .as_ref()
        .map(|n| path.join(&about_dir).join(n));

    // Workshop identity.
    let numeric_folder = (!found.folder_name.is_empty()
        && found.folder_name.bytes().all(|b| b.is_ascii_digit()))
    .then(|| WorkshopId::parse(&found.folder_name).ok())
    .flatten();
    let published = if kind == SourceKind::Workshop && numeric_folder.is_some() {
        numeric_folder
    } else {
        probe
            .about
            .published_id
            .as_ref()
            .and_then(|n| read_published_id(&path.join(&about_dir).join(n)))
    };
    meta.workshop_id = if kind == SourceKind::Workshop {
        numeric_folder.or(published)
    } else {
        None
    };

    // The cached copy carries the final metadata so an offline source can be restored from it.
    if let Some(cache) = about_cache.as_mut() {
        cache.meta = meta.clone();
    }

    let info = ModInfo {
        folder_name: found.folder_name.clone(),
        is_link: found.is_link,
        available: true,
        loadable: if is_visible_to_game(kind, &item.also_kinds) {
            Loadability::Loadable
        } else {
            Loadability::NeedsLink
        },
        about: if parsed {
            AboutStatus::Parsed
        } else {
            AboutStatus::Unparsed
        },
        synthetic_package_id: synthetic,
        published_file_id: published,
        about_mtime_ns: about_mtime,
        also_in: item.also_in.clone(),
    };
    Some(L0 {
        meta,
        info,
        about: about_cache,
        load_folders: lf_cache,
        diagnostics,
        counters,
    })
}

impl Scanner {
    /// Scans every enabled source of `sources`. See the module documentation for the phases.
    ///
    /// `progress` receives `library.scan.metadata` and `library.scan.definitions` records;
    /// `cancel` stops the scan at the next mod boundary.
    pub fn scan(
        sources: &SourceSet,
        opts: &ScanOptions,
        progress: &dyn ProgressSink,
        cancel: &CancelToken,
    ) -> ScanOutcome {
        let total_start = Instant::now();
        let workers = opts.workers.clamp(1, MAX_WORKERS);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .ok();
        let pool = pool.as_ref();
        let mut stats = ScanStats::default();
        let mut timings = ScanTimings::default();
        let mut loose: Vec<Diagnostic> = Vec::new();
        let mut reports: Vec<SourceReport> = Vec::new();

        // Discovery.
        let discover_start = Instant::now();
        let mut items: Vec<Item<'_>> = Vec::new();
        let mut owner_of: FxHashMap<String, usize> = FxHashMap::default();
        let mut offline: Vec<&ModSource> = Vec::new();
        let mut modless_diags: Vec<(Utf8PathBuf, Diagnostic)> = Vec::new();
        for source in sources.iter() {
            let mut status = SourceStatus::Ready;
            if !source.enabled {
                status = SourceStatus::Disabled;
            } else {
                match fs_err::metadata(&source.path) {
                    Ok(m) if m.is_dir() => {}
                    Ok(_) => status = SourceStatus::NotDirectory,
                    Err(_) => status = SourceStatus::Offline,
                }
            }
            reports.push(SourceReport {
                id: source.id.clone(),
                kind: source.kind,
                path: source.path.clone(),
                status,
                mods: 0,
            });
            match status {
                SourceStatus::Offline | SourceStatus::NotDirectory => {
                    loose.push(
                        Diagnostic::new(
                            codes::SOURCE_OFFLINE,
                            Severity::Warning,
                            "A source folder is missing or cannot be reached.",
                        )
                        .with_arg("source", source.id.as_str())
                        .with_arg("path", source.path.as_str()),
                    );
                    offline.push(source);
                    continue;
                }
                SourceStatus::Disabled => continue,
                SourceStatus::Ready => {}
            }
            if cancel.is_cancelled() {
                break;
            }
            let spec = opts.spec_for(source);
            let discovery = run_in_pool(pool, || {
                discover_source(&source.path, source.kind, spec, cancel)
            });
            stats.folders_probed += discovery.probed as u64;
            stats.dir_errors += discovery.dir_errors;
            stats.non_utf8 += discovery.non_utf8;
            loose.extend(discovery.diagnostics);
            let policy = opts.policy_for(source);
            let source_canon = canonical_of(&source.path);
            for found in discovery.found {
                stats.mods_found += 1;
                let canon = if found.is_link {
                    canonical_of(&found.path)
                } else if found.rel.as_str().is_empty() {
                    source_canon.clone()
                } else {
                    source_canon.join(&found.rel)
                };
                let key = fold_key(&canon);
                if let Some(&owner) = owner_of.get(&key) {
                    let first = &mut items[owner];
                    if !first.also_in.contains(&source.id) {
                        first.also_in.push(source.id.clone());
                        first.also_kinds.push(source.kind);
                    }
                    modless_diags.push((
                        first.found.path.clone(),
                        Diagnostic::new(
                            codes::SAME_FOLDER,
                            Severity::Info,
                            "The same folder is reachable through two sources and is listed once.",
                        )
                        .with_arg("path", found.path.as_str())
                        .with_arg("also", source.id.as_str()),
                    ));
                    continue;
                }
                owner_of.insert(key, items.len());
                items.push(Item {
                    found,
                    source,
                    policy,
                    also_in: Vec::new(),
                    also_kinds: Vec::new(),
                });
            }
        }
        timings.discover = discover_start.elapsed();

        // Level 0.
        let metadata_start = Instant::now();
        let previous = opts.previous.as_deref();
        let total_items = items.len();
        let counter = AtomicUsize::new(0);
        report(
            progress,
            "library.scan.metadata",
            0,
            total_items,
            ProgressUnit::Items,
        );
        let l0s: Vec<Option<L0>> = run_in_pool(pool, || {
            items
                .par_iter()
                .map(|item| {
                    if cancel.is_cancelled() {
                        return None;
                    }
                    let prev = previous.and_then(|m| m.mods.get(item.found.path.as_str()));
                    let result = level0(item, prev, opts)?;
                    let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if done.is_multiple_of(PROGRESS_EVERY) {
                        report(
                            progress,
                            "library.scan.metadata",
                            done,
                            total_items,
                            ProgressUnit::Items,
                        );
                    }
                    Some(result)
                })
                .collect()
        });
        report(
            progress,
            "library.scan.metadata",
            counter.load(Ordering::Relaxed),
            total_items,
            ProgressUnit::Items,
        );
        timings.metadata = metadata_start.elapsed();

        // Pair every finished item with its level 0 result, in discovery order.
        let mut done: Vec<(Item<'_>, L0)> = Vec::with_capacity(items.len());
        for (item, l0) in items.into_iter().zip(l0s) {
            if let Some(l0) = l0 {
                done.push((item, l0));
            }
        }
        for (_, l0) in &done {
            stats.about_parsed += l0.counters.about_parsed;
            stats.about_reused += l0.counters.about_reused;
            stats.load_folders_parsed += l0.counters.lf_parsed;
            stats.load_folders_reused += l0.counters.lf_reused;
            stats.files_hashed += l0.counters.hashed;
        }

        // Level 1.
        let mut l1s: Vec<Option<crate::content::Level1Out>> = Vec::new();
        if opts.level >= ScanLevel::Definitions && !cancel.is_cancelled() {
            let definitions_start = Instant::now();
            let active_all =
                ActiveSet::from_ids(done.iter().map(|(_, l)| l.meta.package_id.as_str()));
            let counter = AtomicUsize::new(0);
            let total = done.len();
            report(
                progress,
                "library.scan.definitions",
                0,
                total,
                ProgressUnit::Items,
            );
            l1s = run_in_pool(pool, || {
                done.par_iter()
                    .map(|(item, l0)| {
                        if cancel.is_cancelled() {
                            return None;
                        }
                        let ctx = Level1Ctx {
                            game_version: opts.game_version.as_ref(),
                            active_all: &active_all,
                            file_id: opts.file_id,
                            policy: item.policy,
                            cancel,
                        };
                        let prev = previous.and_then(|m| m.mods.get(item.found.path.as_str()));
                        let out = index_mod(&l0.meta, prev, &ctx);
                        let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        if n.is_multiple_of(PROGRESS_EVERY) {
                            report(
                                progress,
                                "library.scan.definitions",
                                n,
                                total,
                                ProgressUnit::Items,
                            );
                        }
                        Some(out)
                    })
                    .collect()
            });
            report(
                progress,
                "library.scan.definitions",
                counter.load(Ordering::Relaxed),
                total,
                ProgressUnit::Items,
            );
            timings.definitions = definitions_start.elapsed();
        }
        l1s.resize_with(done.len(), || None);

        // Assemble the index, the manifest and the diagnostics.
        let mut builder = LibraryIndex::builder();
        let mut manifest = Manifest::new();
        let mut per_mod_diags: Vec<(Utf8PathBuf, Diagnostic)> = Vec::new();
        let mut present: rustc_hash::FxHashSet<Utf8PathBuf> = rustc_hash::FxHashSet::default();
        for ((item, l0), l1) in done.into_iter().zip(l1s) {
            let path = item.found.path.clone();
            let mut cache = ModCache {
                about: l0.about,
                load_folders: l0.load_folders,
                files: Vec::new(),
            };
            let content = match l1 {
                Some(out) => {
                    stats.def_files_parsed += out.parsed;
                    stats.def_files_reused += out.reused;
                    stats.files_hashed += out.hashed;
                    stats.bytes_read += out.bytes_read;
                    stats.dir_errors += out.dir_errors;
                    stats.non_utf8 += out.non_utf8;
                    per_mod_diags.extend(out.diagnostics.into_iter().map(|d| (path.clone(), d)));
                    cache.files = out.files;
                    out.content
                }
                None => {
                    // Level 1 did not reach this mod: keep what the previous manifest knew.
                    if let Some(prev) = previous.and_then(|m| m.mods.get(path.as_str())) {
                        cache.files = prev.files.clone();
                    }
                    ModContent::default()
                }
            };
            per_mod_diags.extend(l0.diagnostics.into_iter().map(|d| (path.clone(), d)));
            present.insert(path.clone());
            manifest.mods.insert(path.as_str().to_owned(), cache);
            builder.push(LibraryMod {
                meta: l0.meta,
                info: l0.info,
                content,
            });
        }

        // Rows of offline sources come back from the previous manifest, marked unavailable.
        if let Some(prev) = previous {
            for source in &offline {
                for (path_text, cached) in &prev.mods {
                    let path = Utf8Path::new(path_text);
                    if !path.starts_with(&source.path) || present.contains(path) {
                        continue;
                    }
                    let Some(about) = &cached.about else { continue };
                    let mut meta = about.meta.clone();
                    meta.path = path.to_owned();
                    meta.source = source.id.clone();
                    meta.load_folders = cached.load_folders.as_ref().and_then(|l| l.spec.clone());
                    let def_files: Vec<DefFile> = cached
                        .files
                        .iter()
                        .map(|f| DefFile {
                            rel_path: f.stamp.key.rel_path.clone(),
                            size: f.stamp.key.size,
                            records: Arc::clone(&f.records),
                        })
                        .collect();
                    let complete = !def_files.is_empty();
                    present.insert(path.to_owned());
                    manifest.mods.insert(path_text.clone(), cached.clone());
                    builder.push(LibraryMod {
                        info: ModInfo {
                            folder_name: path.file_name().unwrap_or("").to_owned(),
                            is_link: false,
                            available: false,
                            loadable: if is_visible_to_game(source.kind, &[]) {
                                Loadability::Loadable
                            } else {
                                Loadability::NeedsLink
                            },
                            about: if about.parsed {
                                AboutStatus::Parsed
                            } else {
                                AboutStatus::Unparsed
                            },
                            synthetic_package_id: about.synthetic_id,
                            published_file_id: None,
                            about_mtime_ns: about.stamp.key.mtime_ns,
                            also_in: Vec::new(),
                        },
                        content: ModContent {
                            def_files,
                            complete,
                            ..ModContent::default()
                        },
                        meta,
                    });
                }
            }
        }

        let index = builder.build();
        stats.mods_indexed = index.len() as u64;
        stats.defs = index.def_count() as u64;
        for report in &mut reports {
            report.mods = index.iter().filter(|(_, m)| m.source == report.id).count();
        }

        let mut sink = DiagSink::new();
        for d in loose {
            sink.push(d);
        }
        for (path, d) in modless_diags.into_iter().chain(per_mod_diags) {
            let d = match index.by_path(&path) {
                Some(idx) => d.with_mod(idx),
                None => d,
            };
            sink.push(d);
        }
        let cancelled = cancel.is_cancelled();
        if cancelled {
            sink.push(Diagnostic::new(
                codes::CANCELLED,
                Severity::Info,
                "The scan was cancelled before it finished.",
            ));
        }
        timings.total = total_start.elapsed();
        ScanOutcome {
            index,
            diagnostics: sink.finish(),
            timings,
            stats,
            sources: reports,
            manifest,
            cancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn synthesized_ids_always_parse_and_are_stable(
            author in "[ -~\\u{e9}\\u{4e2d}]{0,60}",
            name in "[ -~\\u{e9}\\u{4e2d}]{0,60}",
            description in "[ -~]{0,40}",
            folder in "[A-Za-z0-9_ -]{1,20}",
        ) {
            let about = About {
                author: Some(author),
                name,
                description,
                ..About::default()
            };
            let a = synthesize_package_id(&about, &folder);
            prop_assert_eq!(&a, &synthesize_package_id(&about, &folder));
            prop_assert!(PackageId::parse(&a).is_ok());
            prop_assert!(a.is_ascii());
        }
    }

    #[test]
    fn a_standin_id_depends_on_the_description_and_name() {
        let base = About {
            author: Some("Tester".to_owned()),
            name: "RS Name".to_owned(),
            description: "one".to_owned(),
            ..About::default()
        };
        let other = About {
            description: "two".to_owned(),
            ..base.clone()
        };
        assert_ne!(
            synthesize_package_id(&base, "f"),
            synthesize_package_id(&other, "f")
        );
        let empty = About::default();
        assert_eq!(synthesize_package_id(&empty, "RS_Folder"), "none.RSUFolder");
    }
}
