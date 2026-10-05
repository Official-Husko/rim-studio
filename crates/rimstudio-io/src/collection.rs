//! The in house JSON document database: one folder, one JSON file per document.
//!
//! A [`Collection<T>`] is a directory (under a data root) holding `<id>.json` files. Every file is
//! the versioned envelope `{"kind": KIND, "v": VERSION, "data": T}` written as stable pretty JSON, so
//! equal values give equal bytes. There is no SQL and no other engine; the folder is the database.
//!
//! # Layout
//!
//! ```text
//! <collection>/
//!   <id>.json          one document per id (ids: [a-z0-9._-], 1 to 128 characters)
//!   index.json         optional derived index: id -> {v, size, summary}; rebuilt when missing or stale
//!   .quarantine/       corrupt documents moved aside with a <id>.<ms>.reason.json beside each
//!   .backups/<id>.json/ verified backup copies per document (when a backup policy is set)
//!   .lock              informational file naming the process that opened the collection
//! ```
//!
//! # Guarantees
//!
//! - Writes are atomic (temp file in the same folder, fsync, rename); a crash leaves the old or the
//!   new file. The index is written after the document and is only a cache: a stale or damaged index
//!   is detected by comparing it with the folder listing (ids and sizes) and rebuilt.
//! - Reads run forward only migrations on the `data` value in memory. [`Collection::get`] writes the
//!   migrated form back lazily (best effort, only when the file still holds the older version);
//!   [`Collection::scan`] never writes.
//! - A document written by a newer app is returned read only ([`Loaded::read_only`]) and is never
//!   rewritten, deleted or moved; if its payload cannot be decoded the error is
//!   [`StoreError::NewerThanApp`].
//! - A document file larger than [`MAX_DOCUMENT_BYTES`] is never read into memory: it is moved to
//!   the quarantine like a corrupt one, and a value that would serialise larger is refused.
//! - A document that is not valid JSON, has the wrong kind or envelope, or fails its migration or
//!   decoding is moved to `.quarantine/` with a reason file and reported as
//!   [`StoreError::DocumentCorrupt`]; the rest of the collection keeps working.
//! - Access inside one process is serialised by a per collection lock (writes, index maintenance
//!   and quarantine). Across processes the design assumes a single writer: `.lock` documents who
//!   opened the folder but is not enforced, and readers in other processes see only complete files.
//!
//! # Limits
//!
//! There are no transactions across documents: a batch of puts can stop half way. The folder name
//! `.journal` is reserved for a future write ahead journal that would make multi document updates
//! all or nothing; nothing writes it today.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::atomic::{FsOps, RealOps, atomic_write, atomic_write_with, cleanup_stale_temps};
use crate::backup::{self, BackupPolicy};
use crate::error::StoreError;
use crate::jsonc::to_json_bytes;
use crate::migrate::MigrationRegistry;
use crate::roots::{DataRoots, RootKind};
use crate::schema::{LoadOrigin, Loaded, Versioned};

/// File name of the derived index.
pub const INDEX_FILE: &str = "index.json";
/// Folder name of quarantined documents.
pub const QUARANTINE_DIR: &str = ".quarantine";
/// Folder name of per document backups.
pub const BACKUP_DIR: &str = ".backups";
/// Name of the informational lock file.
pub const LOCK_FILE: &str = ".lock";
/// The longest allowed document id.
pub const MAX_ID_LEN: usize = 128;
/// The largest document file the store reads or writes (64 MiB). A larger file is treated as corrupt
/// (moved to the quarantine, never read into memory) and a larger value is refused on write.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

const INDEX_VERSION: u32 = 1;

/// Up to this many documents the index file is rewritten after every put and delete. Above it the
/// in memory index is updated at once and the file is written by the next listing or rebuild, so a
/// burst of writes into a large collection costs one index write instead of one per document. A
/// crash only leaves a stale file, which the next listing detects by comparing it with the folder.
const INDEX_EAGER_LIMIT: usize = 1000;

/// Checks a document id: 1 to 128 characters of `[a-z0-9._-]`, not starting or ending with a dot,
/// without `..`, not the reserved name `index` and not a Windows device name.
pub fn validate_id(id: &str) -> Result<(), StoreError> {
    let bad = |reason: &'static str| StoreError::IdInvalid {
        id: id.chars().take(64).collect(),
        reason,
    };
    if id.is_empty() {
        return Err(bad("empty"));
    }
    if id.len() > MAX_ID_LEN {
        return Err(bad("longer than 128 characters"));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(bad("only a-z, 0-9, dot, underscore and hyphen are allowed"));
    }
    if id.starts_with('.') || id.ends_with('.') || id.contains("..") {
        return Err(bad("dots may not lead, trail or repeat"));
    }
    if id == "index" {
        return Err(bad("reserved name"));
    }
    if crate::guard::sanitize_name(id).is_err() {
        return Err(bad("reserved device name"));
    }
    Ok(())
}

/// Tuning of a collection.
#[derive(Clone)]
pub struct CollectionOptions {
    /// Keep backups of replaced and deleted documents under this policy (`None` keeps none). When
    /// the policy has no root, copies go under `<collection>/.backups/`.
    pub backup: Option<BackupPolicy>,
    /// Re-read every written document and compare the bytes.
    pub verify_writes: bool,
    /// Fsync files and folders. Turn off only for caches and bulk loads.
    pub durable: bool,
    /// Replaces the file system steps (crash injection in tests).
    pub ops: Option<Arc<dyn FsOps>>,
}

impl Default for CollectionOptions {
    fn default() -> Self {
        CollectionOptions {
            backup: None,
            verify_writes: false,
            durable: true,
            ops: None,
        }
    }
}

impl std::fmt::Debug for CollectionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectionOptions")
            .field("backup", &self.backup)
            .field("verify_writes", &self.verify_writes)
            .field("durable", &self.durable)
            .finish_non_exhaustive()
    }
}

impl CollectionOptions {
    /// Options for user data: light backups and verified writes.
    pub fn user_data() -> Self {
        CollectionOptions {
            backup: Some(BackupPolicy::light()),
            verify_writes: true,
            durable: true,
            ops: None,
        }
    }

    /// Options for derived caches: no backups, no fsync, no verification.
    pub fn cache() -> Self {
        CollectionOptions {
            backup: None,
            verify_writes: false,
            durable: false,
            ops: None,
        }
    }
}

/// One row of the compact index.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexEntry {
    /// The document id.
    pub id: String,
    /// The schema version stored in the document.
    pub v: u32,
    /// The size of the document file in bytes.
    pub size: u64,
    /// The small summary supplied by [`Collection::with_summarizer`] (`null` without one).
    pub summary: Value,
}

/// A document that was moved to the quarantine folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineEntry {
    /// The id the document had.
    pub id: String,
    /// Where the bytes are now.
    pub path: Utf8PathBuf,
    /// Why it was rejected.
    pub reason: String,
    /// When it was moved (milliseconds since the Unix epoch).
    pub at_ms: u64,
}

/// One successfully read document of a scan.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanItem<T> {
    /// The document id.
    pub id: String,
    /// The value and how it was read.
    pub loaded: Loaded<T>,
}

#[derive(Debug, Clone, PartialEq)]
struct IndexRow {
    v: u32,
    size: u64,
    summary: Value,
}

#[derive(Default)]
struct Shared {
    write_lock: Mutex<()>,
    index: Mutex<Option<BTreeMap<String, IndexRow>>>,
    /// The in memory index is newer than the index file.
    index_dirty: AtomicBool,
}

fn lock<M>(m: &Mutex<M>) -> MutexGuard<'_, M> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

enum Fault {
    Corrupt(String),
    Other(StoreError),
}

struct ReadDoc<T> {
    loaded: Loaded<T>,
    size: u64,
}

/// A typed folder of JSON documents. Cloning is cheap and clones share the lock and the index.
pub struct Collection<T> {
    dir: Utf8PathBuf,
    options: CollectionOptions,
    registry: MigrationRegistry,
    summarize: Option<fn(&T) -> Value>,
    clock: Arc<dyn Clock>,
    shared: Arc<Shared>,
    ops: Arc<dyn FsOps>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Collection<T> {
    fn clone(&self) -> Self {
        Collection {
            dir: self.dir.clone(),
            options: self.options.clone(),
            registry: self.registry.clone(),
            summarize: self.summarize,
            clock: Arc::clone(&self.clock),
            shared: Arc::clone(&self.shared),
            ops: Arc::clone(&self.ops),
            _marker: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Collection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collection")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl<T> Collection<T>
where
    T: Versioned + Serialize + DeserializeOwned,
{
    /// Opens (creating when missing) the collection `name` under the root `kind`.
    pub fn open(
        roots: &DataRoots,
        kind: RootKind,
        name: &str,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, StoreError> {
        let name = crate::guard::sanitize_name(name).map_err(|_| StoreError::IdInvalid {
            id: name.chars().take(64).collect(),
            reason: "not a single safe folder name",
        })?;
        Self::open_dir(roots.path(kind).join(name), clock)
    }

    /// Opens (creating when missing) the collection in an explicit folder. Temporary files left by
    /// an interrupted write more than a minute ago are removed.
    pub fn open_dir(
        dir: impl Into<Utf8PathBuf>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, StoreError> {
        let dir = dir.into();
        fs_err::create_dir_all(&dir).map_err(|e| StoreError::io("create-dir", &dir, e))?;
        cleanup_stale_temps(&dir, std::time::Duration::from_secs(60));
        let options = CollectionOptions::default();
        let col = Collection {
            ops: Arc::new(RealOps {
                durable: options.durable,
            }),
            dir,
            options,
            registry: MigrationRegistry::for_type::<T>(),
            summarize: None,
            clock,
            shared: Arc::new(Shared::default()),
            _marker: PhantomData,
        };
        let info = json!({"pid": std::process::id(), "openedMs": col.clock.now_unix_ms(), "kind": T::KIND});
        // Informational only; a read only folder is still usable.
        if let Ok(bytes) = to_json_bytes(&info) {
            let _ = atomic_write(&col.dir.join(LOCK_FILE), &bytes);
        }
        Ok(col)
    }

    /// Replaces the options.
    pub fn with_options(mut self, options: CollectionOptions) -> Self {
        self.ops = match &options.ops {
            Some(o) => Arc::clone(o),
            None => Arc::new(RealOps {
                durable: options.durable,
            }),
        };
        self.options = options;
        self
    }

    /// Sets the function that builds the small per document summary stored in the index.
    pub fn with_summarizer(mut self, f: fn(&T) -> Value) -> Self {
        self.summarize = Some(f);
        self
    }

    /// Replaces the migration registry.
    pub fn with_registry(mut self, registry: MigrationRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// The folder of the collection.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    fn path_of(&self, id: &str) -> Utf8PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn quarantine_dir(&self) -> Utf8PathBuf {
        self.dir.join(QUARANTINE_DIR)
    }

    fn backup_policy(&self) -> Option<BackupPolicy> {
        self.options.backup.as_ref().map(|p| {
            if p.root.is_some() {
                p.clone()
            } else {
                p.clone().with_root(self.dir.join(BACKUP_DIR))
            }
        })
    }

    /// True when a document with this id exists.
    pub fn exists(&self, id: &str) -> Result<bool, StoreError> {
        validate_id(id)?;
        Ok(self.path_of(id).is_file())
    }

    /// The ids of all documents, sorted.
    pub fn list_ids(&self) -> Result<Vec<String>, StoreError> {
        let rd =
            fs_err::read_dir(&self.dir).map_err(|e| StoreError::io("read-dir", &self.dir, e))?;
        let mut ids: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json")?;
                validate_id(id).ok()?;
                e.file_type().ok()?.is_file().then(|| id.to_owned())
            })
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// The number of documents.
    pub fn len(&self) -> Result<usize, StoreError> {
        self.list_ids().map(|v| v.len())
    }

    /// True when the collection has no documents.
    pub fn is_empty(&self) -> Result<bool, StoreError> {
        self.len().map(|n| n == 0)
    }

    fn dir_state(&self) -> Result<BTreeMap<String, u64>, StoreError> {
        let rd =
            fs_err::read_dir(&self.dir).map_err(|e| StoreError::io("read-dir", &self.dir, e))?;
        let mut out = BTreeMap::new();
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if validate_id(id).is_err() {
                continue;
            }
            if let Ok(meta) = e.metadata()
                && meta.is_file()
            {
                out.insert(id.to_owned(), meta.len());
            }
        }
        Ok(out)
    }

    // -- reading ---------------------------------------------------------------------------

    fn read_doc(&self, id: &str, path: &Utf8Path) -> Result<Option<ReadDoc<T>>, Fault> {
        if let Ok(meta) = fs_err::metadata(path)
            && meta.is_file()
            && meta.len() > MAX_DOCUMENT_BYTES
        {
            return Err(Fault::Corrupt(format!(
                "the file is {} bytes, more than the limit of {MAX_DOCUMENT_BYTES}",
                meta.len()
            )));
        }
        let bytes = match fs_err::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Fault::Other(StoreError::io("read", path, e))),
        };
        let size = bytes.len() as u64;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| {
            Fault::Corrupt(format!(
                "invalid JSON at line {}, column {}: {e}",
                e.line(),
                e.column()
            ))
        })?;
        let (v, data) = split_envelope(&value, T::KIND).map_err(Fault::Corrupt)?;
        if v > T::VERSION {
            return match crate::schema::decode_value::<T>(data, T::KIND, path) {
                Ok(value) => {
                    let mut loaded = Loaded::new(value, LoadOrigin::NewerThanApp { found: v });
                    loaded.read_only = true;
                    Ok(Some(ReadDoc { loaded, size }))
                }
                Err(_) => Err(Fault::Other(StoreError::NewerThanApp {
                    path: path.to_owned(),
                    kind: T::KIND,
                    found: v,
                    supported: T::VERSION,
                })),
            };
        }
        let (data, origin) = if v < T::VERSION {
            let migrated = self
                .registry
                .migrate(T::KIND, v, T::VERSION, data)
                .map_err(|e| Fault::Corrupt(e.to_string()))?;
            (migrated, LoadOrigin::Migrated { from: v })
        } else {
            (data, LoadOrigin::File)
        };
        let value = crate::schema::decode_value::<T>(data, T::KIND, path)
            .map_err(|e| Fault::Corrupt(format!("document {id}: {e}")))?;
        Ok(Some(ReadDoc {
            loaded: Loaded::new(value, origin),
            size,
        }))
    }

    /// Reads one document. `Ok(None)` when it does not exist. Runs migrations (and writes the
    /// migrated form back, best effort). A corrupt document is quarantined and reported as
    /// [`StoreError::DocumentCorrupt`]; asking again then gives `Ok(None)`.
    pub fn get(&self, id: &str) -> Result<Option<Loaded<T>>, StoreError> {
        validate_id(id)?;
        let path = self.path_of(id);
        let first = self.read_doc(id, &path);
        let result = match first {
            Err(Fault::Corrupt(_)) => {
                let _g = lock(&self.shared.write_lock);
                match self.read_doc(id, &path) {
                    Err(Fault::Corrupt(reason)) => {
                        return Err(self.quarantine_move(id, &path, &reason));
                    }
                    other => other,
                }
            }
            other => other,
        };
        match result {
            Ok(None) => Ok(None),
            Ok(Some(doc)) => {
                if matches!(doc.loaded.origin, LoadOrigin::Migrated { .. }) {
                    let _g = lock(&self.shared.write_lock);
                    if let Err(e) = self.put_locked(id, &path, &doc.loaded.value, true) {
                        tracing::debug!(id, error = %e, "lazy write back of a migrated document failed");
                    }
                }
                Ok(Some(doc.loaded))
            }
            Err(Fault::Corrupt(_)) => Ok(None),
            Err(Fault::Other(e)) => Err(e),
        }
    }

    /// Moves a corrupt file aside and returns the error describing it. The caller holds the write
    /// lock or knows no other writer is active.
    fn quarantine_move(&self, id: &str, path: &Utf8Path, reason: &str) -> StoreError {
        lock(&self.shared.index).as_mut().map(|m| m.remove(id));
        let qdir = self.quarantine_dir();
        let now = self.clock.now_unix_ms();
        let moved = (|| -> std::io::Result<Utf8PathBuf> {
            fs_err::create_dir_all(&qdir)?;
            let mut n = 0u32;
            let (target, reason_path) = loop {
                let suffix = if n == 0 {
                    String::new()
                } else {
                    format!("-{n}")
                };
                let t = qdir.join(format!("{id}.{now}{suffix}.json"));
                if !t.exists() {
                    break (t, qdir.join(format!("{id}.{now}{suffix}.reason.json")));
                }
                n += 1;
            };
            fs_err::rename(path, &target)?;
            let info = json!({"id": id, "reason": reason, "atMs": now});
            if let Ok(bytes) = to_json_bytes(&info) {
                let _ = atomic_write(&reason_path, &bytes);
            }
            Ok(target)
        })();
        let quarantined = match moved {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(id, error = %e, "could not quarantine a corrupt document");
                None
            }
        };
        tracing::warn!(id, reason, "corrupt document moved to quarantine");
        StoreError::DocumentCorrupt {
            id: id.to_owned(),
            reason: reason.to_owned(),
            quarantined,
        }
    }

    /// The documents that were moved to quarantine, sorted by id then time.
    pub fn quarantined(&self) -> Vec<QuarantineEntry> {
        let qdir = self.quarantine_dir();
        let Ok(rd) = fs_err::read_dir(&qdir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            let Some(stem) = name.strip_suffix(".reason.json") else {
                continue;
            };
            let Ok(text) = fs_err::read_to_string(e.path()) else {
                continue;
            };
            let Ok(info) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            out.push(QuarantineEntry {
                id: info
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or(stem)
                    .to_owned(),
                path: qdir.join(format!("{stem}.json")),
                reason: info
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                at_ms: info.get("atMs").and_then(Value::as_u64).unwrap_or(0),
            });
        }
        out.sort_by(|a, b| (&a.id, a.at_ms, &a.path).cmp(&(&b.id, b.at_ms, &b.path)));
        out
    }

    // -- writing ---------------------------------------------------------------------------

    /// Writes (creating or replacing) one document atomically. Refused with
    /// [`StoreError::NewerThanApp`] when the stored document is newer than this app. A stored file
    /// that is unreadable is quarantined first.
    pub fn put(&self, id: &str, value: &T) -> Result<(), StoreError> {
        validate_id(id)?;
        let _g = lock(&self.shared.write_lock);
        self.put_locked(id, &self.path_of(id), value, false)
    }

    fn put_locked(
        &self,
        id: &str,
        path: &Utf8Path,
        value: &T,
        only_if_older: bool,
    ) -> Result<(), StoreError> {
        let existing_bytes = if is_oversized(path) {
            Ok(None)
        } else {
            fs_err::read(path).map(Some)
        };
        match existing_bytes {
            Ok(Some(bytes)) => {
                let existing = serde_json::from_slice::<Value>(&bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|v| split_envelope(&v, T::KIND).map(|(v, _)| v));
                match existing {
                    Ok(v) if v > T::VERSION => {
                        return Err(StoreError::NewerThanApp {
                            path: path.to_owned(),
                            kind: T::KIND,
                            found: v,
                            supported: T::VERSION,
                        });
                    }
                    Ok(v) if only_if_older && v >= T::VERSION => return Ok(()),
                    Ok(_) => {}
                    Err(reason) => {
                        if only_if_older {
                            return Ok(());
                        }
                        self.quarantine_before_replace(id, path, &reason)?;
                    }
                }
            }
            Ok(None) => {
                if only_if_older {
                    return Ok(());
                }
                self.quarantine_before_replace(id, path, "the file is larger than the limit")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(StoreError::io("read", path, e)),
        }
        let data = serde_json::to_value(value).map_err(|e| StoreError::Serialize {
            message: e.to_string(),
        })?;
        let summary = self.summarize.map_or(Value::Null, |f| f(value));
        let mut env = Map::new();
        env.insert("kind".into(), Value::from(T::KIND));
        env.insert("v".into(), Value::from(T::VERSION));
        env.insert("data".into(), data);
        let bytes = to_json_bytes(&Value::Object(env))?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(StoreError::Serialize {
                message: format!(
                    "the document is {} bytes, more than the limit of {MAX_DOCUMENT_BYTES}",
                    bytes.len()
                ),
            });
        }
        if let Some(policy) = self.backup_policy() {
            backup::rotate(path, &policy, &*self.clock)?;
        }
        atomic_write_with(&*self.ops, path, &bytes)?;
        if self.options.verify_writes {
            let back = fs_err::read(path).map_err(|e| StoreError::io("verify-read", path, e))?;
            if back != bytes {
                return Err(StoreError::WriteVerifyFailed {
                    path: path.to_owned(),
                });
            }
        }
        self.index_put(
            id,
            IndexRow {
                v: T::VERSION,
                size: bytes.len() as u64,
                summary,
            },
        );
        Ok(())
    }

    /// Moves an unreadable file aside before a new document replaces it. When the bytes cannot be
    /// kept (the quarantine is not writable) the replacement is refused: unreadable is not the same
    /// as worthless, and the person may still repair the file by hand.
    fn quarantine_before_replace(
        &self,
        id: &str,
        path: &Utf8Path,
        reason: &str,
    ) -> Result<(), StoreError> {
        match self.quarantine_move(id, path, reason) {
            StoreError::DocumentCorrupt {
                quarantined: None, ..
            } => Err(StoreError::io(
                "quarantine",
                path,
                std::io::Error::other(
                    "the unreadable document could not be moved to the quarantine, so it was not replaced",
                ),
            )),
            _ => Ok(()),
        }
    }

    /// Deletes one document (keeping a backup when a policy is set). Returns whether it existed.
    /// A document newer than the app is not deleted ([`StoreError::NewerThanApp`]).
    pub fn delete(&self, id: &str) -> Result<bool, StoreError> {
        validate_id(id)?;
        let path = self.path_of(id);
        let _g = lock(&self.shared.write_lock);
        if is_oversized(&path) {
            // never read into memory: keep the bytes in the quarantine and drop it from the collection
            let reason = "the file is larger than the limit";
            return match self.quarantine_move(id, &path, reason) {
                StoreError::DocumentCorrupt {
                    quarantined: Some(_),
                    ..
                } => {
                    self.index_remove(id);
                    Ok(true)
                }
                other => Err(other),
            };
        }
        let bytes = match fs_err::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(StoreError::io("read", &path, e)),
        };
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes)
            && let Ok((found, _)) = split_envelope(&v, T::KIND)
            && found > T::VERSION
        {
            return Err(StoreError::NewerThanApp {
                path,
                kind: T::KIND,
                found,
                supported: T::VERSION,
            });
        }
        if let Some(policy) = self.backup_policy() {
            backup::rotate(&path, &policy, &*self.clock)?;
        }
        fs_err::remove_file(&path).map_err(|e| StoreError::io("remove", &path, e))?;
        self.index_remove(id);
        Ok(true)
    }

    // -- scanning --------------------------------------------------------------------------

    /// An iterator over every document in id order. Documents are read one by one; a bad document
    /// yields an `Err` item (after being quarantined when corrupt) and the iteration goes on.
    pub fn scan(&self) -> Result<ScanIter<T>, StoreError> {
        Ok(ScanIter {
            col: self.clone(),
            ids: self.list_ids()?.into_iter(),
        })
    }

    // -- index -----------------------------------------------------------------------------

    fn index_path(&self) -> Utf8PathBuf {
        self.dir.join(INDEX_FILE)
    }

    fn load_index_file(&self) -> Option<BTreeMap<String, IndexRow>> {
        let bytes = fs_err::read(self.index_path()).ok()?;
        let v: Value = serde_json::from_slice(&bytes).ok()?;
        if v.get("kind")?.as_str()? != "collection-index"
            || v.get("v")?.as_u64()? != u64::from(INDEX_VERSION)
            || v.get("collection")?.as_str()? != T::KIND
        {
            return None;
        }
        let mut map = BTreeMap::new();
        for (id, row) in v.get("entries")?.as_object()? {
            map.insert(
                id.clone(),
                IndexRow {
                    v: u32::try_from(row.get("v")?.as_u64()?).ok()?,
                    size: row.get("size")?.as_u64()?,
                    summary: row.get("summary").cloned().unwrap_or(Value::Null),
                },
            );
        }
        Some(map)
    }

    fn write_index_file(&self, map: &BTreeMap<String, IndexRow>) -> Result<(), StoreError> {
        let mut entries = Map::new();
        for (id, row) in map {
            entries.insert(
                id.clone(),
                json!({"v": row.v, "size": row.size, "summary": row.summary}),
            );
        }
        let doc = json!({
            "kind": "collection-index",
            "v": INDEX_VERSION,
            "collection": T::KIND,
            "entries": Value::Object(entries),
        });
        atomic_write_with(&*self.ops, &self.index_path(), &to_json_bytes(&doc)?)
    }

    /// Keeps the index current after a put. Only touches the index when one is loaded or on disk;
    /// otherwise the next listing builds it.
    fn index_put(&self, id: &str, row: IndexRow) {
        self.index_change(|map| {
            map.insert(id.to_owned(), row);
        });
    }

    fn index_remove(&self, id: &str) {
        self.index_change(|map| {
            map.remove(id);
        });
    }

    fn index_change(&self, change: impl FnOnce(&mut BTreeMap<String, IndexRow>)) {
        let mut guard = lock(&self.shared.index);
        if guard.is_none() {
            *guard = self.load_index_file();
        }
        let Some(map) = guard.as_mut() else { return };
        change(map);
        if map.len() > INDEX_EAGER_LIMIT {
            self.shared.index_dirty.store(true, Ordering::SeqCst);
            return;
        }
        if let Err(e) = self.write_index_file(map) {
            tracing::debug!(error = %e, "index write failed, it will be rebuilt");
            *guard = None;
        } else {
            self.shared.index_dirty.store(false, Ordering::SeqCst);
        }
    }

    fn rebuild_locked(&self, state: &BTreeMap<String, u64>) -> BTreeMap<String, IndexRow> {
        let mut map = BTreeMap::new();
        for id in state.keys() {
            let path = self.path_of(id);
            match self.read_doc(id, &path) {
                Ok(Some(doc)) => {
                    let v = match doc.loaded.origin {
                        LoadOrigin::NewerThanApp { found } => found,
                        _ => T::VERSION,
                    };
                    let summary = self.summarize.map_or(Value::Null, |f| f(&doc.loaded.value));
                    map.insert(
                        id.clone(),
                        IndexRow {
                            v,
                            size: doc.size,
                            summary,
                        },
                    );
                }
                Ok(None) => {}
                Err(Fault::Corrupt(reason)) => {
                    let _ = self.quarantine_move(id, &path, &reason);
                }
                Err(Fault::Other(e)) => {
                    tracing::debug!(id, error = %e, "document skipped while rebuilding the index");
                }
            }
        }
        map
    }

    /// Rebuilds the index from the documents and writes it. Documents are never changed (corrupt
    /// ones are quarantined as in any read).
    pub fn rebuild_index(&self) -> Result<Vec<IndexEntry>, StoreError> {
        let _g = lock(&self.shared.write_lock);
        let state = self.dir_state()?;
        let map = self.rebuild_locked(&state);
        self.write_index_file(&map)?;
        self.shared.index_dirty.store(false, Ordering::SeqCst);
        let entries = entries_of(&map);
        *lock(&self.shared.index) = Some(map);
        Ok(entries)
    }

    /// The compact listing (id, version, size, summary) in id order, read from the index. The
    /// index is rebuilt when it is missing, unreadable, or does not match the folder (different
    /// ids or sizes).
    pub fn list_summaries(&self) -> Result<Vec<IndexEntry>, StoreError> {
        let _g = lock(&self.shared.write_lock);
        let state = self.dir_state()?;
        // The index lock is never held while documents are read: a read may quarantine, which
        // takes the same lock.
        let current = {
            let mut guard = lock(&self.shared.index);
            if guard.is_none() {
                *guard = self.load_index_file();
            }
            guard.clone()
        };
        let consistent = current.as_ref().is_some_and(|m| {
            m.len() == state.len() && m.iter().all(|(k, row)| state.get(k) == Some(&row.size))
        });
        let map = match current {
            Some(m) if consistent => {
                if self.shared.index_dirty.swap(false, Ordering::SeqCst)
                    && let Err(e) = self.write_index_file(&m)
                {
                    tracing::debug!(error = %e, "index write failed, it will be rebuilt");
                    self.shared.index_dirty.store(true, Ordering::SeqCst);
                }
                m
            }
            _ => {
                let map = self.rebuild_locked(&state);
                if let Err(e) = self.write_index_file(&map) {
                    tracing::debug!(error = %e, "index write failed after rebuild");
                }
                *lock(&self.shared.index) = Some(map.clone());
                map
            }
        };
        Ok(entries_of(&map))
    }
}

/// True when the file exists and is larger than [`MAX_DOCUMENT_BYTES`].
fn is_oversized(path: &Utf8Path) -> bool {
    fs_err::metadata(path).is_ok_and(|m| m.is_file() && m.len() > MAX_DOCUMENT_BYTES)
}

fn entries_of(map: &BTreeMap<String, IndexRow>) -> Vec<IndexEntry> {
    map.iter()
        .map(|(id, row)| IndexEntry {
            id: id.clone(),
            v: row.v,
            size: row.size,
            summary: row.summary.clone(),
        })
        .collect()
}

/// Checks the envelope and returns the version and the payload.
fn split_envelope(value: &Value, kind: &str) -> Result<(u32, Value), String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "the document is not a JSON object".to_owned())?;
    match obj.get("kind").and_then(Value::as_str) {
        Some(k) if k == kind => {}
        Some(other) => return Err(format!("kind is {other:?}, expected {kind:?}")),
        None => return Err("the envelope has no kind".to_owned()),
    }
    let v = obj
        .get("v")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
        .ok_or_else(|| "the envelope version is missing or invalid".to_owned())?;
    let data = obj
        .get("data")
        .cloned()
        .ok_or_else(|| "the envelope has no data".to_owned())?;
    Ok((v, data))
}

/// The iterator returned by [`Collection::scan`].
pub struct ScanIter<T> {
    col: Collection<T>,
    ids: std::vec::IntoIter<String>,
}

impl<T> std::fmt::Debug for ScanIter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanIter").finish_non_exhaustive()
    }
}

impl<T> Iterator for ScanIter<T>
where
    T: Versioned + Serialize + DeserializeOwned,
{
    type Item = Result<ScanItem<T>, StoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.ids.next()?;
            let path = self.col.path_of(&id);
            match self.col.read_doc(&id, &path) {
                Ok(None) => continue,
                Ok(Some(doc)) => {
                    return Some(Ok(ScanItem {
                        id,
                        loaded: doc.loaded,
                    }));
                }
                Err(Fault::Corrupt(reason)) => {
                    let _g = lock(&self.col.shared.write_lock);
                    return Some(Err(self.col.quarantine_move(&id, &path, &reason)));
                }
                Err(Fault::Other(e)) => return Some(Err(e)),
            }
        }
    }
}
