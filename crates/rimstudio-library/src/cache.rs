//! The scan manifest: what the scanner remembers between runs.
//!
//! The manifest holds, per mod folder, the parsed `About.xml`, the parsed `LoadFolders.xml` and the
//! indexed definitions of every definition file, each keyed by a [`StatKey`] (relative path, size,
//! modification time in nanoseconds, optional file id). A rescan compares keys for equality and
//! parses only files whose key changed ([`file_unchanged`]). On FAT family volumes the comparison
//! tolerates two second rounding and falls back to a stored blake3 hash.
//!
//! On disk the manifest is one compact JSON document (`v`, `parser`, one string table, and mods,
//! files and definitions as arrays of tuples), the layout measured in the scan performance spike.
//! A missing, truncated, malformed or newer file is treated as absent: [`Manifest::load`] never
//! fails. Writes go through [`ManifestWriter`], which writes atomically and at most once per
//! second.

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::Diagnostic;
use rimstudio_core::load_plan::LoadFoldersSpec;
use rimstudio_core::mods::ModMeta;
use rimstudio_core::ports::Clock;
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_io::statkey::StatKey;
pub use rimstudio_io::statkey::{FatPolicy, KeyVerdict};
use rimstudio_xml::defs_scan::DefIndexRecord;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::error::CacheError;

/// The layout version of the manifest file.
pub const MANIFEST_VERSION: u32 = 1;
/// The version of the parsers whose output the manifest holds; bump it when a parser changes.
pub const PARSER_VERSION: u32 = 1;
/// The file name of the manifest inside the cache root.
pub const MANIFEST_FILE: &str = "library-manifest.json";
/// The shortest time between two manifest writes.
pub const MIN_WRITE_INTERVAL_MS: u64 = 1000;
/// Manifests larger than this are ignored as damaged.
const MAX_MANIFEST_BYTES: u64 = 512 * 1024 * 1024;

/// The path of the manifest below the cache root.
pub fn manifest_path(roots: &DataRoots) -> Utf8PathBuf {
    roots.path(RootKind::Cache).join(MANIFEST_FILE)
}

/// A file's change key and, on FAT family volumes, its content hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    /// The stat key.
    pub key: StatKey,
    /// The blake3 hash as hex, stored only when the volume needs hash comparison.
    pub hash: Option<String>,
}

/// The cached result of reading one `About.xml`.
#[derive(Debug, Clone, PartialEq)]
pub struct AboutCache {
    /// The key of the file read.
    pub stamp: FileStamp,
    /// The metadata (its `path` and `source` are replaced by the scanner on reuse).
    pub meta: ModMeta,
    /// False when the file could not be parsed at all.
    pub parsed: bool,
    /// True when the package id is a stand-in.
    pub synthetic_id: bool,
    /// Warnings found while reading.
    pub warnings: Vec<Diagnostic>,
}

/// The cached result of reading one `LoadFolders.xml`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadFoldersCache {
    /// The key of the file read.
    pub stamp: FileStamp,
    /// The parsed content, `None` when the file could not be read.
    pub spec: Option<LoadFoldersSpec>,
    /// Warnings found while reading.
    pub warnings: Vec<Diagnostic>,
}

/// The cached definition index of one file.
#[derive(Debug, Clone, PartialEq)]
pub struct DefFileCache {
    /// The key; its `rel_path` is relative to the mod root.
    pub stamp: FileStamp,
    /// One record per definition, shared with the index built from the same scan.
    pub records: Arc<Vec<DefIndexRecord>>,
}

/// Everything cached for one mod folder.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModCache {
    /// The `About.xml` result.
    pub about: Option<AboutCache>,
    /// The `LoadFolders.xml` result.
    pub load_folders: Option<LoadFoldersCache>,
    /// The definition files, sorted by relative path.
    pub files: Vec<DefFileCache>,
}

/// The whole manifest: mod folder path to its cache, sorted by path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Manifest {
    /// The entries, keyed by the absolute path of the mod folder.
    pub mods: BTreeMap<String, ModCache>,
}

/// How loading the manifest went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStatus {
    /// There was no file.
    Missing,
    /// The file was read.
    Loaded,
    /// The file was damaged, truncated or unreadable and is ignored (the reason is for logs).
    Invalid(String),
    /// The file was written by another layout or parser version and is ignored.
    Outdated {
        /// The layout version found.
        found_layout: u32,
        /// The parser version found.
        found_parser: u32,
    },
}

/// The result of [`Manifest::load`].
#[derive(Debug, Clone)]
pub struct ManifestLoad {
    /// The manifest, empty unless `status` is [`LoadStatus::Loaded`].
    pub manifest: Manifest,
    /// What happened.
    pub status: LoadStatus,
}

// ---------------------------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Wire {
    v: u32,
    parser: u32,
    strings: Vec<String>,
    mods: Vec<WireMod>,
}

/// `[path, about, loadFolders, files]`
#[derive(Serialize, Deserialize)]
struct WireMod(
    u32,
    Option<WireAbout>,
    Option<WireLoadFolders>,
    Vec<WireFile>,
);

/// `[relPath, size, mtimeNs, fileId, hash]`
#[derive(Serialize, Deserialize)]
struct WireStamp(u32, u64, Option<i128>, Option<u128>, Option<String>);

/// `[stamp, parsed, synthetic, meta, warnings]`
#[derive(Serialize, Deserialize)]
struct WireAbout(WireStamp, bool, bool, ModMeta, Vec<Diagnostic>);

/// `[stamp, spec, warnings]`
#[derive(Serialize, Deserialize)]
struct WireLoadFolders(WireStamp, Option<LoadFoldersSpec>, Vec<Diagnostic>);

/// `[stamp, defs]`
#[derive(Serialize, Deserialize)]
struct WireFile(WireStamp, Vec<WireDef>);

/// `[type, class, defName, name, parentName, abstract, label, mayRequire, offset]`
#[derive(Serialize, Deserialize)]
struct WireDef(
    u32,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    bool,
    Option<u32>,
    Option<u32>,
    u32,
);

#[derive(Default)]
struct Interner {
    map: FxHashMap<String, u32>,
    list: Vec<String>,
}

impl Interner {
    fn id(&mut self, text: &str) -> u32 {
        if let Some(&i) = self.map.get(text) {
            return i;
        }
        let i = u32::try_from(self.list.len()).unwrap_or(u32::MAX);
        self.map.insert(text.to_owned(), i);
        self.list.push(text.to_owned());
        i
    }

    fn opt(&mut self, text: &Option<String>) -> Option<u32> {
        text.as_deref().map(|t| self.id(t))
    }
}

struct Strings<'a>(&'a [String]);

impl Strings<'_> {
    fn get(&self, i: u32) -> Result<&str, String> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.0.get(i))
            .map(String::as_str)
            .ok_or_else(|| format!("string index {i} out of range"))
    }

    fn opt(&self, i: Option<u32>) -> Result<Option<String>, String> {
        match i {
            Some(i) => Ok(Some(self.get(i)?.to_owned())),
            None => Ok(None),
        }
    }
}

fn stamp_to_wire(stamp: &FileStamp, strings: &mut Interner) -> WireStamp {
    WireStamp(
        strings.id(&stamp.key.rel_path),
        stamp.key.size,
        stamp.key.mtime_ns,
        stamp.key.file_id,
        stamp.hash.clone(),
    )
}

fn stamp_from_wire(w: WireStamp, strings: &Strings<'_>) -> Result<FileStamp, String> {
    Ok(FileStamp {
        key: StatKey {
            rel_path: strings.get(w.0)?.to_owned(),
            size: w.1,
            mtime_ns: w.2,
            file_id: w.3,
        },
        hash: w.4,
    })
}

impl Manifest {
    /// An empty manifest.
    pub fn new() -> Manifest {
        Manifest::default()
    }

    /// The number of mod folders.
    pub fn mod_count(&self) -> usize {
        self.mods.len()
    }

    /// The number of cached definition files.
    pub fn file_count(&self) -> usize {
        self.mods.values().map(|m| m.files.len()).sum()
    }

    /// The number of cached definitions.
    pub fn def_count(&self) -> usize {
        self.mods
            .values()
            .flat_map(|m| m.files.iter())
            .map(|f| f.records.len())
            .sum()
    }

    /// Serialises to the compact JSON layout. The output is deterministic: mods are in path order
    /// and strings are numbered by first use.
    ///
    /// # Errors
    /// [`CacheError::Serialize`] when JSON encoding fails.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, CacheError> {
        let mut strings = Interner::default();
        let mut mods = Vec::with_capacity(self.mods.len());
        for (path, m) in &self.mods {
            let path_id = strings.id(path);
            let about = m.about.as_ref().map(|a| {
                WireAbout(
                    stamp_to_wire(&a.stamp, &mut strings),
                    a.parsed,
                    a.synthetic_id,
                    a.meta.clone(),
                    a.warnings.clone(),
                )
            });
            let lf = m.load_folders.as_ref().map(|l| {
                WireLoadFolders(
                    stamp_to_wire(&l.stamp, &mut strings),
                    l.spec.clone(),
                    l.warnings.clone(),
                )
            });
            let mut files = Vec::with_capacity(m.files.len());
            for f in &m.files {
                let stamp = stamp_to_wire(&f.stamp, &mut strings);
                let defs = f
                    .records
                    .iter()
                    .map(|r| {
                        WireDef(
                            strings.id(&r.def_type),
                            strings.opt(&r.class),
                            strings.opt(&r.def_name),
                            strings.opt(&r.name),
                            strings.opt(&r.parent_name),
                            r.is_abstract,
                            strings.opt(&r.label),
                            strings.opt(&r.may_require),
                            r.offset,
                        )
                    })
                    .collect();
                files.push(WireFile(stamp, defs));
            }
            mods.push(WireMod(path_id, about, lf, files));
        }
        let wire = Wire {
            v: MANIFEST_VERSION,
            parser: PARSER_VERSION,
            strings: strings.list,
            mods,
        };
        serde_json::to_vec(&wire).map_err(|e| CacheError::Serialize(e.to_string()))
    }

    /// Reads the compact JSON layout.
    ///
    /// # Errors
    /// A [`LoadStatus::Invalid`] or [`LoadStatus::Outdated`] value describing why the bytes are not
    /// a usable manifest.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Manifest, LoadStatus> {
        // The header is read first so a manifest of another version is reported as outdated even
        // when the rest of the layout has changed.
        #[derive(Deserialize)]
        struct Header {
            v: u32,
            parser: u32,
        }
        let header: Header =
            serde_json::from_slice(bytes).map_err(|e| LoadStatus::Invalid(e.to_string()))?;
        if header.v != MANIFEST_VERSION || header.parser != PARSER_VERSION {
            return Err(LoadStatus::Outdated {
                found_layout: header.v,
                found_parser: header.parser,
            });
        }
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|e| LoadStatus::Invalid(e.to_string()))?;
        Manifest::from_wire(wire).map_err(LoadStatus::Invalid)
    }

    fn from_wire(wire: Wire) -> Result<Manifest, String> {
        let strings = Strings(&wire.strings);
        let mut out = Manifest::new();
        for m in wire.mods {
            let path = strings.get(m.0)?.to_owned();
            let about = match m.1 {
                Some(a) => Some(AboutCache {
                    stamp: stamp_from_wire(a.0, &strings)?,
                    parsed: a.1,
                    synthetic_id: a.2,
                    meta: a.3,
                    warnings: a.4,
                }),
                None => None,
            };
            let load_folders = match m.2 {
                Some(l) => Some(LoadFoldersCache {
                    stamp: stamp_from_wire(l.0, &strings)?,
                    spec: l.1,
                    warnings: l.2,
                }),
                None => None,
            };
            let mut files = Vec::with_capacity(m.3.len());
            for f in m.3 {
                let stamp = stamp_from_wire(f.0, &strings)?;
                let mut records = Vec::with_capacity(f.1.len());
                for d in f.1 {
                    records.push(DefIndexRecord {
                        def_type: strings.get(d.0)?.to_owned(),
                        class: strings.opt(d.1)?,
                        def_name: strings.opt(d.2)?,
                        name: strings.opt(d.3)?,
                        parent_name: strings.opt(d.4)?,
                        is_abstract: d.5,
                        label: strings.opt(d.6)?,
                        may_require: strings.opt(d.7)?,
                        offset: d.8,
                    });
                }
                files.push(DefFileCache {
                    stamp,
                    records: Arc::new(records),
                });
            }
            out.mods.insert(
                path,
                ModCache {
                    about,
                    load_folders,
                    files,
                },
            );
        }
        Ok(out)
    }

    /// Reads the manifest at `path`. A missing, unreadable, truncated, malformed or outdated file
    /// gives an empty manifest and a status saying why; this never fails.
    pub fn load(path: &Utf8Path) -> ManifestLoad {
        let empty = |status| ManifestLoad {
            manifest: Manifest::new(),
            status,
        };
        match fs_err::metadata(path) {
            Ok(meta) if meta.len() > MAX_MANIFEST_BYTES => {
                return empty(LoadStatus::Invalid("file too large".to_owned()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return empty(LoadStatus::Missing);
            }
            Err(e) => return empty(LoadStatus::Invalid(e.to_string())),
        }
        let bytes = match fs_err::read(path) {
            Ok(b) => b,
            Err(e) => return empty(LoadStatus::Invalid(e.to_string())),
        };
        match Manifest::from_json_bytes(&bytes) {
            Ok(manifest) => ManifestLoad {
                manifest,
                status: LoadStatus::Loaded,
            },
            Err(status) => empty(status),
        }
    }

    /// Writes the manifest atomically (temporary file, flush, rename), creating parent folders.
    ///
    /// # Errors
    /// [`CacheError`] when encoding or writing fails.
    pub fn save(&self, path: &Utf8Path) -> Result<(), CacheError> {
        let bytes = self.to_json_bytes()?;
        write_bytes(path, &bytes)
    }
}

fn write_bytes(path: &Utf8Path, bytes: &[u8]) -> Result<(), CacheError> {
    rimstudio_io::atomic::atomic_write(path, bytes).map_err(|e| CacheError::Write {
        path: path.to_owned(),
        message: e.to_string(),
    })
}

/// Decides whether a file is unchanged since it was cached.
///
/// Returns `(unchanged, fresh_hash)`. `fresh_hash` is set when a content hash had to be computed
/// (FAT family volumes whose time stamps disagree), so the caller can store it. Any I/O error
/// while hashing counts as changed.
pub fn file_unchanged(
    policy: FatPolicy,
    old: &FileStamp,
    new: &StatKey,
    abs_path: &Utf8Path,
) -> (bool, Option<String>) {
    policy
        .is_unchanged(&old.key, old.hash.as_deref(), new, abs_path)
        .unwrap_or_default()
}

/// Writes a manifest at most once per [`MIN_WRITE_INTERVAL_MS`].
///
/// [`offer`](ManifestWriter::offer) writes immediately when the last write is old enough and
/// otherwise keeps the manifest as pending; [`poll`](ManifestWriter::poll) (call it from a timer or
/// at the next scan) and [`flush`](ManifestWriter::flush) (call it on exit) write a pending
/// manifest. A manifest that serialises to the bytes last written is not written again.
pub struct ManifestWriter {
    path: Utf8PathBuf,
    clock: Arc<dyn Clock>,
    interval_ms: u64,
    last_write_ms: Option<u64>,
    last_hash: Option<[u8; 32]>,
    pending: Option<Manifest>,
    writes: u64,
}

impl ManifestWriter {
    /// A writer for the manifest file at `path`.
    pub fn new(path: Utf8PathBuf, clock: Arc<dyn Clock>) -> ManifestWriter {
        ManifestWriter {
            path,
            clock,
            interval_ms: MIN_WRITE_INTERVAL_MS,
            last_write_ms: None,
            last_hash: None,
            pending: None,
            writes: 0,
        }
    }

    /// A writer for the manifest in the cache root.
    pub fn in_cache_root(roots: &DataRoots, clock: Arc<dyn Clock>) -> ManifestWriter {
        ManifestWriter::new(manifest_path(roots), clock)
    }

    /// The manifest file path.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// How many times the file was actually written.
    pub fn write_count(&self) -> u64 {
        self.writes
    }

    /// True when a manifest waits for the next allowed write.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn due(&self) -> bool {
        match self.last_write_ms {
            None => true,
            Some(last) => self.clock.now_unix_ms().saturating_sub(last) >= self.interval_ms,
        }
    }

    fn write(&mut self, manifest: &Manifest) -> Result<bool, CacheError> {
        let bytes = manifest.to_json_bytes()?;
        let hash = *blake3::hash(&bytes).as_bytes();
        self.last_write_ms = Some(self.clock.now_unix_ms());
        if self.last_hash == Some(hash) {
            return Ok(false);
        }
        write_bytes(&self.path, &bytes)?;
        self.last_hash = Some(hash);
        self.writes += 1;
        Ok(true)
    }

    /// Offers a manifest. Returns true when the file was written now.
    ///
    /// # Errors
    /// [`CacheError`] when a due write fails.
    pub fn offer(&mut self, manifest: Manifest) -> Result<bool, CacheError> {
        if self.due() {
            self.pending = None;
            self.write(&manifest)
        } else {
            self.pending = Some(manifest);
            Ok(false)
        }
    }

    /// Writes the pending manifest when the interval has passed. Returns true when it wrote.
    ///
    /// # Errors
    /// [`CacheError`] when the write fails.
    pub fn poll(&mut self) -> Result<bool, CacheError> {
        if self.pending.is_some()
            && self.due()
            && let Some(m) = self.pending.take()
        {
            return self.write(&m);
        }
        Ok(false)
    }

    /// Writes the pending manifest now, ignoring the interval. Returns true when it wrote.
    ///
    /// # Errors
    /// [`CacheError`] when the write fails.
    pub fn flush(&mut self) -> Result<bool, CacheError> {
        match self.pending.take() {
            Some(m) => self.write(&m),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ids::{PackageId, SourceId};
    use rimstudio_testing::fakes::FakeClock;

    fn stamp(rel: &str, size: u64, mtime: i128) -> FileStamp {
        FileStamp {
            key: StatKey {
                rel_path: rel.to_owned(),
                size,
                mtime_ns: Some(mtime),
                file_id: Some(u128::MAX - 5),
            },
            hash: None,
        }
    }

    fn sample() -> Manifest {
        let mut meta = ModMeta::new(
            PackageId::parse("rs.test.mod").unwrap(),
            "RS Test",
            SourceId::game_mods(),
            Utf8PathBuf::from("/m/RS_Test"),
        );
        meta.supported_versions = vec!["1.6".to_owned()];
        let mut m = Manifest::new();
        m.mods.insert(
            "/m/RS_Test".to_owned(),
            ModCache {
                about: Some(AboutCache {
                    stamp: stamp("About/About.xml", 120, 1_700_000_000_123_456_789),
                    meta,
                    parsed: true,
                    synthetic_id: false,
                    warnings: Vec::new(),
                }),
                load_folders: None,
                files: vec![DefFileCache {
                    stamp: stamp("Defs/a.xml", 40, -5),
                    records: Arc::new(vec![
                        DefIndexRecord {
                            def_type: "ThingDef".to_owned(),
                            def_name: Some("RS_Gun".to_owned()),
                            label: Some("gun".to_owned()),
                            offset: 22,
                            ..DefIndexRecord::default()
                        },
                        DefIndexRecord {
                            def_type: "ThingDef".to_owned(),
                            name: Some("RS_Base".to_owned()),
                            is_abstract: true,
                            offset: 90,
                            ..DefIndexRecord::default()
                        },
                    ]),
                }],
            },
        );
        m
    }

    #[test]
    fn manifest_round_trips_including_wide_integers() {
        let m = sample();
        let bytes = m.to_json_bytes().unwrap();
        let back = Manifest::from_json_bytes(&bytes).unwrap();
        assert_eq!(back, m);
        assert_eq!(m.def_count(), 2);
        assert_eq!(m.file_count(), 1);
    }

    #[test]
    fn serialisation_is_deterministic_and_interns_repeated_strings() {
        let m = sample();
        let a = m.to_json_bytes().unwrap();
        let b = m.to_json_bytes().unwrap();
        assert_eq!(a, b);
        let text = String::from_utf8(a).unwrap();
        assert_eq!(text.matches("\"ThingDef\"").count(), 1);
        assert!(text.starts_with("{\"v\":1,"));
    }

    #[test]
    fn truncated_or_garbage_files_are_treated_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("m.json")).unwrap();
        assert_eq!(Manifest::load(&path).status, LoadStatus::Missing);
        let bytes = sample().to_json_bytes().unwrap();
        for cut in [0, 1, bytes.len() / 3, bytes.len() - 1] {
            fs_err::write(&path, &bytes[..cut]).unwrap();
            let loaded = Manifest::load(&path);
            assert!(matches!(loaded.status, LoadStatus::Invalid(_)), "{cut}");
            assert!(loaded.manifest.mods.is_empty());
        }
        fs_err::write(&path, b"not json at all").unwrap();
        assert!(matches!(
            Manifest::load(&path).status,
            LoadStatus::Invalid(_)
        ));
    }

    #[test]
    fn out_of_range_string_index_is_invalid_not_a_panic() {
        let text = r#"{"v":1,"parser":1,"strings":["a"],"mods":[[7,null,null,[]]]}"#;
        assert!(matches!(
            Manifest::from_json_bytes(text.as_bytes()),
            Err(LoadStatus::Invalid(_))
        ));
    }

    #[test]
    fn other_versions_are_outdated() {
        let text = r#"{"v":99,"parser":1,"strings":[],"mods":[]}"#;
        assert!(matches!(
            Manifest::from_json_bytes(text.as_bytes()),
            Err(LoadStatus::Outdated {
                found_layout: 99,
                ..
            })
        ));
        let text = format!(
            r#"{{"v":{MANIFEST_VERSION},"parser":{},"strings":[],"mods":[]}}"#,
            PARSER_VERSION + 1
        );
        assert!(matches!(
            Manifest::from_json_bytes(text.as_bytes()),
            Err(LoadStatus::Outdated { .. })
        ));
    }

    #[test]
    fn save_and_load_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("sub/m.json")).unwrap();
        let m = sample();
        m.save(&path).unwrap();
        let loaded = Manifest::load(&path);
        assert_eq!(loaded.status, LoadStatus::Loaded);
        assert_eq!(loaded.manifest, m);
    }

    fn key(rel: &str, size: u64, mtime: i128) -> StatKey {
        StatKey {
            rel_path: rel.to_owned(),
            size,
            mtime_ns: Some(mtime),
            file_id: None,
        }
    }

    const SEC: i128 = 1_000_000_000;

    #[test]
    fn exact_policy_needs_identical_time_stamps() {
        let old = FileStamp {
            key: key("a.xml", 10, 100 * SEC),
            hash: None,
        };
        let p = Utf8Path::new("/nonexistent/a.xml");
        assert!(file_unchanged(FatPolicy::Exact, &old, &key("a.xml", 10, 100 * SEC), p).0);
        assert!(!file_unchanged(FatPolicy::Exact, &old, &key("a.xml", 10, 102 * SEC), p).0);
        assert!(!file_unchanged(FatPolicy::Exact, &old, &key("a.xml", 11, 100 * SEC), p).0);
    }

    #[test]
    fn fat_policy_tolerates_two_second_rounding_but_not_a_shift_without_hash() {
        let old = FileStamp {
            key: key("a.xml", 10, 100 * SEC),
            hash: None,
        };
        let p = Utf8Path::new("/nonexistent/a.xml");
        // Same two second slot: unchanged without reading the file.
        assert_eq!(
            file_unchanged(FatPolicy::Fat, &old, &key("a.xml", 10, 101 * SEC), p),
            (true, None)
        );
        // A one hour shift (daylight saving) needs the hash; the file is unreadable here, so it
        // counts as changed.
        assert!(
            !file_unchanged(
                FatPolicy::Fat,
                &old,
                &key("a.xml", 10, 100 * SEC + 3600 * SEC),
                p
            )
            .0
        );
    }

    #[test]
    fn fat_policy_accepts_a_one_hour_shift_when_the_hash_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("a.xml")).unwrap();
        fs_err::write(&path, b"<Defs/>").unwrap();
        let hash = rimstudio_io::statkey::hash_file(&path).unwrap();
        let old = FileStamp {
            key: key("a.xml", 7, 100 * SEC),
            hash: Some(hash.clone()),
        };
        let shifted = key("a.xml", 7, 100 * SEC + 3600 * SEC);
        let (same, fresh) = file_unchanged(FatPolicy::Fat, &old, &shifted, &path);
        assert!(same);
        assert_eq!(fresh.as_deref(), Some(hash.as_str()));
        // Different content with the same size and a shifted time is changed.
        fs_err::write(&path, b"<Defz/>").unwrap();
        assert!(!file_unchanged(FatPolicy::Fat, &old, &shifted, &path).0);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        fn opt_text() -> impl Strategy<Value = Option<String>> {
            proptest::option::of("[ -~\\u{e9}\\u{4e2d}]{0,12}")
        }

        fn record() -> impl Strategy<Value = DefIndexRecord> {
            (
                "[A-Za-z]{1,10}",
                opt_text(),
                opt_text(),
                opt_text(),
                opt_text(),
                any::<bool>(),
                opt_text(),
                opt_text(),
                any::<u32>(),
            )
                .prop_map(
                    |(
                        def_type,
                        class,
                        def_name,
                        name,
                        parent_name,
                        is_abstract,
                        label,
                        may_require,
                        offset,
                    )| {
                        DefIndexRecord {
                            def_type,
                            class,
                            def_name,
                            name,
                            parent_name,
                            is_abstract,
                            label,
                            may_require,
                            offset,
                        }
                    },
                )
        }

        proptest! {
            #[test]
            fn any_records_round_trip_through_the_manifest(
                records in proptest::collection::vec(record(), 0..20),
                size in any::<u64>(),
                mtime in proptest::option::of(any::<i64>()),
                id in proptest::option::of(any::<u128>()),
            ) {
                let mut m = Manifest::new();
                m.mods.insert("/m/x".to_owned(), ModCache {
                    about: None,
                    load_folders: None,
                    files: vec![DefFileCache {
                        stamp: FileStamp {
                            key: StatKey {
                                rel_path: "Defs/a.xml".to_owned(),
                                size,
                                mtime_ns: mtime.map(i128::from),
                                file_id: id,
                            },
                            hash: None,
                        },
                        records: Arc::new(records),
                    }],
                });
                let bytes = m.to_json_bytes().unwrap();
                prop_assert_eq!(Manifest::from_json_bytes(&bytes).unwrap(), m);
            }

            #[test]
            fn random_bytes_never_panic_the_manifest_reader(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
                let _ = Manifest::from_json_bytes(&bytes);
            }
        }
    }

    #[test]
    fn writer_writes_at_most_once_per_second_and_flushes_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("m.json")).unwrap();
        let clock = FakeClock::new(10_000);
        let mut w = ManifestWriter::new(path.clone(), Arc::new(clock.clone()));
        let a = sample();
        assert!(w.offer(a.clone()).unwrap());
        let mut b = sample();
        b.mods.insert("/m/other".to_owned(), ModCache::default());
        clock.advance(400);
        assert!(!w.offer(b.clone()).unwrap());
        assert!(w.has_pending());
        assert_eq!(Manifest::load(&path).manifest, a);
        clock.advance(400);
        assert!(!w.poll().unwrap());
        clock.advance(300);
        assert!(w.poll().unwrap());
        assert_eq!(Manifest::load(&path).manifest, b);
        assert_eq!(w.write_count(), 2);
        // Identical content is not written again.
        clock.advance(2000);
        assert!(!w.offer(b).unwrap());
        assert_eq!(w.write_count(), 2);
        // Flush ignores the interval.
        let mut c = sample();
        c.mods.clear();
        assert!(!w.offer(c.clone()).unwrap());
        assert!(w.flush().unwrap());
        assert_eq!(Manifest::load(&path).manifest, c);
    }
}
