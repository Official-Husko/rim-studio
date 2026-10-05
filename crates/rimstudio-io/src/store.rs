//! Single file stores for settings like documents (JSONC, user editable).
//!
//! [`Store<T>`] owns one file in a root folder. It
//! - loads with comments, trailing commas and a byte order mark accepted, runs forward migrations in
//!   memory and reports what happened through [`Loaded`];
//! - never overwrites a file it could not read faithfully: a file with a syntax error loads the
//!   newest readable backup (or the defaults) and blocks saving until the file parses again, and a
//!   file written by a newer app is read only;
//! - saves by syncing the typed value into the existing text through the CST, so comments, key
//!   order and layout of everything that did not change stay byte identical, after a verified
//!   backup (and a `.bak-vN` copy before the first migrated save);
//! - supports targeted edits through [`Store::edit`] and the free function [`edit_jsonc`], which
//!   verify that the result still decodes before anything is written.
//!
//! Machine files (caches, state) are plain JSON written through [`crate::collection`] or
//! [`crate::atomic`]; this module is for files a person may edit by hand. See [`crate::jsonc`] for
//! the findings of spike S-02 about the CST.

use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::atomic::atomic_write;
use crate::backup::{self, BackupPolicy};
use crate::error::StoreError;
use crate::jsonc::{self, JsoncEditor};
use crate::migrate::MigrationRegistry;
use crate::roots::{DataRoots, RootKind};
use crate::schema::{
    LoadOrigin, LoadProblem, Loaded, VERSION_FIELD, Versioned, decode_value, read_version,
    stamp_version,
};

/// The largest settings file the store reads (16 MiB). A larger file is reported as unreadable and
/// saving is blocked, so it is never loaded into memory and never overwritten.
pub const MAX_SETTINGS_BYTES: u64 = 16 * 1024 * 1024;

/// The header written at the top of a newly created file.
pub const DEFAULT_HEADER: &str =
    "RimStudio configuration. Comments and key order are kept when the app saves.";

#[derive(Debug, Default)]
struct State {
    /// Why saving is refused, when it is.
    blocked: Option<String>,
    /// The stored version when the last load migrated in memory and nothing was saved since.
    pending_migration: Option<u32>,
}

/// How a piece of text decoded.
enum Decoded<T> {
    Current(T),
    Migrated(T, u32),
    Newer(T, u32),
}

/// A typed single file store.
pub struct Store<T> {
    path: Utf8PathBuf,
    policy: BackupPolicy,
    registry: MigrationRegistry,
    clock: Arc<dyn Clock>,
    header: String,
    state: Mutex<State>,
    write_lock: Mutex<()>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Store<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn lock<M>(m: &Mutex<M>) -> MutexGuard<'_, M> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read_text(path: &Utf8Path) -> Result<Option<String>, StoreError> {
    if let Ok(meta) = fs_err::metadata(path)
        && meta.is_file()
        && meta.len() > MAX_SETTINGS_BYTES
    {
        return Err(StoreError::io(
            "read",
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("the file is larger than the limit of {MAX_SETTINGS_BYTES} bytes"),
            ),
        ));
    }
    match fs_err::read(path) {
        // Strict: a file in another encoding must never be loaded with replacement characters and
        // then written back that way.
        Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|e| {
            StoreError::io(
                "read",
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("the file is not valid UTF-8 ({e})"),
                ),
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(StoreError::io("read", path, e)),
    }
}

impl<T> Store<T>
where
    T: Versioned + Serialize + DeserializeOwned + Default,
{
    /// A store for the file `name` inside the root `kind`. `name` must be a single file name.
    pub fn open(
        roots: &DataRoots,
        kind: RootKind,
        name: &str,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, StoreError> {
        let name = crate::guard::sanitize_name(name).map_err(|_| StoreError::IdInvalid {
            id: name.chars().take(64).collect(),
            reason: "not a single safe file name",
        })?;
        Ok(Self::at(roots.path(kind).join(name), clock))
    }

    /// A store for an explicit path.
    pub fn at(path: impl Into<Utf8PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Store {
            path: path.into(),
            policy: BackupPolicy::user_data(),
            registry: MigrationRegistry::for_type::<T>(),
            clock,
            header: DEFAULT_HEADER.to_owned(),
            state: Mutex::new(State::default()),
            write_lock: Mutex::new(()),
            _marker: PhantomData,
        }
    }

    /// Replaces the backup policy.
    pub fn with_policy(mut self, policy: BackupPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Replaces the migration registry (tests and types with steps registered elsewhere).
    pub fn with_registry(mut self, registry: MigrationRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Replaces the header comment of newly created files.
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = header.into();
        self
    }

    /// The file path.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// The backup policy in use.
    pub fn policy(&self) -> &BackupPolicy {
        &self.policy
    }

    /// The reason saving is refused right now, if it is.
    pub fn blocked_reason(&self) -> Option<String> {
        lock(&self.state).blocked.clone()
    }

    fn decode_text(&self, text: &str) -> Result<Decoded<T>, StoreError> {
        let value = jsonc::parse_value(text, &self.path)?;
        if value.is_null() {
            return Ok(Decoded::Current(T::default()));
        }
        if !value.is_object() {
            return Err(StoreError::Shape {
                path: self.path.clone(),
                kind: T::KIND,
                message: "the document root must be an object".to_owned(),
            });
        }
        let found = read_version(&value).unwrap_or(1);
        if found > T::VERSION {
            return match decode_value::<T>(value, T::KIND, &self.path) {
                Ok(v) => Ok(Decoded::Newer(v, found)),
                Err(_) => Err(StoreError::NewerThanApp {
                    path: self.path.clone(),
                    kind: T::KIND,
                    found,
                    supported: T::VERSION,
                }),
            };
        }
        if found < T::VERSION {
            let migrated =
                self.registry
                    .migrate_with(T::KIND, found, T::VERSION, value, |v, doc| {
                        stamp_version(doc, v)
                    })?;
            let v = decode_value::<T>(migrated, T::KIND, &self.path)?;
            return Ok(Decoded::Migrated(v, found));
        }
        Ok(Decoded::Current(decode_value::<T>(
            value,
            T::KIND,
            &self.path,
        )?))
    }

    /// Loads the file. Never fails: problems are reported in the result and saving is blocked
    /// when the file on disk must not be overwritten.
    pub fn load(&self) -> Loaded<T> {
        let text = match read_text(&self.path) {
            Ok(Some(t)) => t,
            Ok(None) => {
                let mut st = lock(&self.state);
                st.blocked = None;
                st.pending_migration = None;
                return Loaded::new(T::default(), LoadOrigin::Missing);
            }
            Err(e) => return self.fallback(e),
        };
        match self.decode_text(&text) {
            Ok(Decoded::Current(v)) => {
                let mut st = lock(&self.state);
                st.blocked = None;
                st.pending_migration = None;
                Loaded::new(v, LoadOrigin::File)
            }
            Ok(Decoded::Migrated(v, from)) => {
                let mut st = lock(&self.state);
                st.blocked = None;
                st.pending_migration = Some(from);
                Loaded::new(v, LoadOrigin::Migrated { from })
            }
            Ok(Decoded::Newer(v, found)) => {
                let mut st = lock(&self.state);
                st.blocked = Some(format!("written by a newer version (v{found})"));
                st.pending_migration = None;
                let mut l = Loaded::new(v, LoadOrigin::NewerThanApp { found });
                l.read_only = true;
                l
            }
            Err(e @ StoreError::NewerThanApp { found, .. }) => {
                let mut st = lock(&self.state);
                st.blocked = Some(format!("written by a newer version (v{found})"));
                let mut l = Loaded::new(T::default(), LoadOrigin::NewerThanApp { found });
                l.read_only = true;
                l.problem = Some(LoadProblem::from_error(&e));
                l
            }
            Err(e) => self.fallback(e),
        }
    }

    /// The file could not be read faithfully: try the backups, else defaults; block saving.
    fn fallback(&self, error: StoreError) -> Loaded<T> {
        let problem = LoadProblem::from_error(&error);
        {
            let mut st = lock(&self.state);
            st.blocked = Some(error.to_string());
            st.pending_migration = None;
        }
        tracing::warn!(path = %self.path, code = error.code(), "settings file unreadable, using a fallback");
        for entry in backup::list_backups(&self.path, &self.policy).iter().rev() {
            let Ok(Some(text)) = read_text(&entry.path) else {
                continue;
            };
            let value = match self.decode_text(&text) {
                Ok(Decoded::Current(v) | Decoded::Migrated(v, _)) => v,
                _ => continue,
            };
            let mut l = Loaded::new(
                value,
                LoadOrigin::Backup {
                    path: entry.path.clone(),
                },
            );
            l.read_only = true;
            l.problem = Some(problem);
            return l;
        }
        let mut l = Loaded::new(T::default(), LoadOrigin::Fallback);
        l.read_only = true;
        l.problem = Some(problem);
        l
    }

    fn check_writable(&self) -> Result<(), StoreError> {
        match &lock(&self.state).blocked {
            Some(reason) => Err(StoreError::WriteBlocked {
                path: self.path.clone(),
                reason: reason.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Saves `value`. When the file exists the change is applied through the CST so that comments
    /// and layout survive; a new file gets a header comment. Nothing is written when the text would
    /// not change. Refused while the file on disk could not be read faithfully.
    pub fn save(&self, value: &T) -> Result<(), StoreError> {
        let _guard = lock(&self.write_lock);
        self.check_writable()?;
        let mut new = serde_json::to_value(value).map_err(|e| StoreError::Serialize {
            message: e.to_string(),
        })?;
        if !new.is_object() {
            return Err(StoreError::Serialize {
                message: "the document must serialise to an object".to_owned(),
            });
        }
        stamp_version(&mut new, T::VERSION);
        let existing = read_text(&self.path)?;
        let text = match &existing {
            None => jsonc::render_new(&self.header, &new)?,
            Some(old) => {
                // What the file meant before this save, when it was a current version document.
                let baseline = match self.decode_text(old) {
                    Ok(Decoded::Current(v)) => serde_json::to_value(&v).ok().map(|mut b| {
                        stamp_version(&mut b, T::VERSION);
                        b
                    }),
                    _ => None,
                };
                if baseline
                    .as_ref()
                    .is_some_and(|b| jsonc::values_equal(b, &new))
                {
                    // Nothing changed semantically: the file stays exactly as the person wrote it.
                    return Ok(());
                }
                let ed = JsoncEditor::parse(old, &self.path)?;
                ed.sync_with_baseline(&new, baseline.as_ref())?;
                ed.finish()
            }
        };
        // The edited text must mean exactly the value being saved (default valued sections that were
        // kept in the text decode to the same value, so they pass).
        let round_trips = match self.decode_text(&text) {
            Ok(Decoded::Current(v)) => serde_json::to_value(&v).ok().is_some_and(|mut meant| {
                stamp_version(&mut meant, T::VERSION);
                jsonc::values_equal(&meant, &new)
            }),
            _ => false,
        };
        if !round_trips {
            return Err(StoreError::Edit {
                path: self.path.clone(),
                message: "the edited text does not round trip; nothing was written".to_owned(),
            });
        }
        if existing.as_deref() == Some(text.as_str()) {
            return Ok(());
        }
        if existing.is_some() {
            let pending = lock(&self.state).pending_migration;
            if let Some(from) = pending {
                backup::keep_pre_migration(&self.path, &self.policy, from)?;
            }
            backup::rotate(&self.path, &self.policy, &*self.clock)?;
        }
        write_verified(&self.path, text.as_bytes())?;
        lock(&self.state).pending_migration = None;
        Ok(())
    }

    /// Applies a targeted, comment preserving edit and writes it after a verified backup. The
    /// result must still decode as `T`, otherwise nothing is written and the error is returned.
    /// A pending in memory migration is saved first so the edit starts from the current schema.
    pub fn edit(
        &self,
        f: impl FnOnce(&JsoncEditor) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        if lock(&self.state).pending_migration.is_some() {
            let loaded = self.load();
            if !loaded.read_only {
                self.save(&loaded.value)?;
            }
        }
        let _guard = lock(&self.write_lock);
        self.check_writable()?;
        let initial = match read_text(&self.path)? {
            Some(t) => t,
            None => {
                let mut v =
                    serde_json::to_value(T::default()).map_err(|e| StoreError::Serialize {
                        message: e.to_string(),
                    })?;
                stamp_version(&mut v, T::VERSION);
                jsonc::render_new(&self.header, &v)?
            }
        };
        let ed = JsoncEditor::parse(&initial, &self.path)?;
        f(&ed)?;
        let text = ed.finish();
        match self.decode_text(&text)? {
            Decoded::Current(_) => {}
            Decoded::Migrated(..) | Decoded::Newer(..) => {
                return Err(StoreError::Edit {
                    path: self.path.clone(),
                    message: format!("the edit changed `{VERSION_FIELD}`"),
                });
            }
        }
        if text == initial && self.path.exists() {
            return Ok(());
        }
        backup::rotate(&self.path, &self.policy, &*self.clock)?;
        write_verified(&self.path, text.as_bytes())
    }
}

/// Writes atomically and re-reads the file to verify the bytes.
fn write_verified(path: &Utf8Path, bytes: &[u8]) -> Result<(), StoreError> {
    atomic_write(path, bytes)?;
    let back = fs_err::read(path).map_err(|e| StoreError::io("verify-read", path, e))?;
    if back != bytes {
        return Err(StoreError::WriteVerifyFailed {
            path: path.to_owned(),
        });
    }
    Ok(())
}

/// A comment preserving edit of any JSONC file without a typed model. `verify` receives the edited
/// text and may refuse it (return an error) before anything is written. A backup is taken first.
/// Returns whether the file changed.
pub fn edit_jsonc(
    path: &Utf8Path,
    policy: &BackupPolicy,
    clock: &dyn Clock,
    f: impl FnOnce(&JsoncEditor) -> Result<(), StoreError>,
    verify: impl FnOnce(&Value) -> Result<(), StoreError>,
) -> Result<bool, StoreError> {
    let initial = read_text(path)?.unwrap_or_else(|| "{}\n".to_owned());
    let ed = JsoncEditor::parse(&initial, path)?;
    f(&ed)?;
    let text = ed.finish();
    verify(&jsonc::parse_value(&text, path)?)?;
    if text == initial && path.exists() {
        return Ok(false);
    }
    backup::rotate(path, policy, clock)?;
    write_verified(path, text.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::error::MigrateError;
    use crate::migrate::MigrationFn;

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now_unix_ms(&self) -> u64 {
            self.0.fetch_add(1000, Ordering::SeqCst)
        }
    }

    fn clock() -> Arc<dyn Clock> {
        Arc::new(TestClock(AtomicU64::new(1_894_708_800_000)))
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase", default)]
    struct RsDoc {
        schema_version: u32,
        title: String,
        size: f64,
        #[serde(flatten)]
        extra: serde_json::Map<String, Value>,
    }

    impl Versioned for RsDoc {
        const KIND: &'static str = "rs-doc";
        const VERSION: u32 = 2;
        fn migrations() -> Vec<(u32, MigrationFn)> {
            fn step(mut v: Value) -> Result<Value, MigrateError> {
                let name = v
                    .get("name")
                    .cloned()
                    .ok_or_else(|| MigrateError::failed("rs-doc", 1, "no name"))?;
                if let Some(o) = v.as_object_mut() {
                    o.remove("name");
                    o.insert("title".into(), name);
                }
                Ok(v)
            }
            vec![(1, step)]
        }
    }

    fn temp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("doc.jsonc")).unwrap();
        (t, p)
    }

    fn store(p: &Utf8Path) -> Store<RsDoc> {
        Store::at(p, clock())
    }

    #[test]
    fn missing_file_loads_defaults_and_first_save_writes_a_header() {
        let (_t, p) = temp();
        let s = store(&p);
        let l = s.load();
        assert_eq!(l.origin, LoadOrigin::Missing);
        assert!(!l.read_only);
        let mut v = l.value;
        v.title = "RS_Title".into();
        s.save(&v).unwrap();
        let text = fs_err::read_to_string(&p).unwrap();
        assert!(text.starts_with("// RimStudio configuration"));
        assert!(text.contains("\"schemaVersion\": 2"));
        let back = s.load();
        assert_eq!(back.origin, LoadOrigin::File);
        assert_eq!(back.value.title, "RS_Title");
    }

    #[test]
    fn changing_one_setting_keeps_comments_byte_identical() {
        let (_t, p) = temp();
        let text = "// my notes\n{\n  // keep this\n  \"schemaVersion\": 2,\n  \"title\": \"RS_A\", // note\n  \"size\": 1.5,\n  \"custom\": [1, 2], // unknown key\n}\n";
        fs_err::write(&p, text).unwrap();
        let s = store(&p);
        let mut l = s.load();
        assert_eq!(l.value.extra.get("custom"), Some(&json!([1, 2])));
        l.value.title = "RS_B".into();
        s.save(&l.value).unwrap();
        let out = fs_err::read_to_string(&p).unwrap();
        assert_eq!(out, text.replace("RS_A", "RS_B"));
    }

    #[test]
    fn saving_an_unchanged_value_writes_nothing_and_makes_no_backup() {
        let (t, p) = temp();
        let s = store(&p);
        s.save(&RsDoc::default()).unwrap();
        let l = s.load();
        s.save(&l.value).unwrap();
        s.save(&l.value).unwrap();
        assert!(!t.path().join("backups").exists());
    }

    #[test]
    fn a_rewrite_takes_a_verified_backup() {
        let (_t, p) = temp();
        let s = store(&p);
        s.save(&RsDoc::default()).unwrap();
        let mut v = s.load().value;
        v.title = "RS_Changed".into();
        s.save(&v).unwrap();
        let backups = backup::list_backups(&p, s.policy());
        assert_eq!(backups.len(), 1);
        assert!(
            fs_err::read_to_string(&backups[0].path)
                .unwrap()
                .contains("\"title\": \"\"")
        );
    }

    #[test]
    fn older_version_is_migrated_and_a_pre_migration_copy_is_kept_on_save() {
        let (_t, p) = temp();
        let old = "{\n  // old file\n  \"schemaVersion\": 1,\n  \"name\": \"RS_Old\"\n}\n";
        fs_err::write(&p, old).unwrap();
        let s = store(&p);
        let l = s.load();
        assert_eq!(l.origin, LoadOrigin::Migrated { from: 1 });
        assert_eq!(l.value.title, "RS_Old");
        assert_eq!(
            fs_err::read_to_string(&p).unwrap(),
            old,
            "load must not write"
        );
        s.save(&l.value).unwrap();
        let out = fs_err::read_to_string(&p).unwrap();
        assert!(out.contains("// old file"), "{out}");
        assert!(out.contains("\"schemaVersion\": 2"));
        assert!(out.contains("RS_Old"));
        let pre = s.policy().dir_for(&p).join("doc.jsonc.bak-v1");
        assert_eq!(fs_err::read_to_string(pre).unwrap(), old);
        assert_eq!(s.load().origin, LoadOrigin::File);
    }

    #[test]
    fn newer_file_is_read_only_and_never_rewritten() {
        let (_t, p) = temp();
        let text = "{\"schemaVersion\": 9, \"title\": \"RS_Future\", \"fancy\": true}";
        fs_err::write(&p, text).unwrap();
        let s = store(&p);
        let l = s.load();
        assert_eq!(l.origin, LoadOrigin::NewerThanApp { found: 9 });
        assert!(l.read_only);
        assert_eq!(l.value.title, "RS_Future");
        let err = s.save(&l.value).unwrap_err();
        assert_eq!(err.code(), "store.write-blocked");
        assert_eq!(fs_err::read_to_string(&p).unwrap(), text);
    }

    #[test]
    fn syntax_error_blocks_saving_and_uses_the_newest_good_backup() {
        let (_t, p) = temp();
        let s = store(&p);
        let mut v = RsDoc {
            title: "RS_Good".into(),
            ..RsDoc::default()
        };
        s.save(&v).unwrap();
        v.title = "RS_Good2".into();
        s.save(&v).unwrap();
        let broken = "{ \"title\": ,, }";
        fs_err::write(&p, broken).unwrap();
        let l = s.load();
        assert!(l.read_only);
        assert!(
            matches!(l.origin, LoadOrigin::Backup { .. }),
            "{:?}",
            l.origin
        );
        assert_eq!(l.value.title, "RS_Good");
        let problem = l.problem.unwrap();
        assert_eq!(problem.code, "store.parse");
        assert_eq!(problem.line, Some(1));
        assert!(s.save(&l.value).is_err());
        assert_eq!(fs_err::read_to_string(&p).unwrap(), broken);
        // The person fixes the file; the next load unblocks saving.
        fs_err::write(&p, "{\"schemaVersion\": 2, \"title\": \"RS_Fixed\"}").unwrap();
        let l = s.load();
        assert_eq!(l.value.title, "RS_Fixed");
        assert!(s.save(&l.value).is_ok());
    }

    #[test]
    fn syntax_error_without_backups_gives_defaults() {
        let (_t, p) = temp();
        fs_err::write(&p, "not json at all").unwrap();
        let l = store(&p).load();
        assert_eq!(l.origin, LoadOrigin::Fallback);
        assert!(l.read_only);
    }

    #[test]
    fn shape_error_names_the_field() {
        let (_t, p) = temp();
        fs_err::write(&p, "{\"schemaVersion\": 2, \"size\": \"big\"}").unwrap();
        let l = store(&p).load();
        let problem = l.problem.unwrap();
        assert_eq!(problem.code, "store.shape");
        assert!(problem.message.contains("size"), "{}", problem.message);
    }

    #[test]
    fn failed_migration_blocks_and_keeps_the_file() {
        let (_t, p) = temp();
        let text = "{\"schemaVersion\": 1}";
        fs_err::write(&p, text).unwrap();
        let s = store(&p);
        let l = s.load();
        assert_eq!(l.problem.unwrap().code, "migrate.failed");
        assert!(s.save(&RsDoc::default()).is_err());
        assert_eq!(fs_err::read_to_string(&p).unwrap(), text);
    }

    #[test]
    fn edit_changes_one_value_and_verifies_the_result() {
        let (_t, p) = temp();
        let text = "{\n  \"schemaVersion\": 2,\n  // size note\n  \"size\": 1\n}\n";
        fs_err::write(&p, text).unwrap();
        let s = store(&p);
        s.load();
        s.edit(|ed| ed.set(&["size"], &json!(2))).unwrap();
        let out = fs_err::read_to_string(&p).unwrap();
        assert_eq!(out, text.replace("\"size\": 1", "\"size\": 2"));
        let err = s.edit(|ed| ed.set(&["size"], &json!("huge"))).unwrap_err();
        assert_eq!(err.code(), "store.shape");
        assert_eq!(
            fs_err::read_to_string(&p).unwrap(),
            out,
            "a refused edit writes nothing"
        );
    }

    #[test]
    fn edit_cannot_bump_the_schema_version() {
        let (_t, p) = temp();
        let s = store(&p);
        s.save(&RsDoc::default()).unwrap();
        let err = s
            .edit(|ed| ed.set(&["schemaVersion"], &json!(5)))
            .unwrap_err();
        assert!(
            matches!(
                err,
                StoreError::NewerThanApp { .. } | StoreError::Edit { .. }
            ),
            "{err}"
        );
    }

    #[test]
    fn edit_on_a_missing_file_creates_it() {
        let (_t, p) = temp();
        let s = store(&p);
        s.edit(|ed| ed.set(&["title"], &json!("RS_New"))).unwrap();
        assert_eq!(s.load().value.title, "RS_New");
    }

    #[test]
    fn free_edit_runs_the_verifier_before_writing() {
        let (_t, p) = temp();
        fs_err::write(&p, "{\"a\": 1}\n").unwrap();
        let pol = BackupPolicy::user_data();
        let c = TestClock(AtomicU64::new(1_894_708_800_000));
        let refused = edit_jsonc(
            &p,
            &pol,
            &c,
            |ed| ed.set(&["a"], &json!(2)),
            |_| {
                Err(StoreError::Edit {
                    path: Utf8PathBuf::new(),
                    message: "no".into(),
                })
            },
        );
        assert!(refused.is_err());
        assert_eq!(fs_err::read_to_string(&p).unwrap(), "{\"a\": 1}\n");
        let changed = edit_jsonc(&p, &pol, &c, |ed| ed.set(&["a"], &json!(2)), |_| Ok(())).unwrap();
        assert!(changed);
        let same = edit_jsonc(&p, &pol, &c, |ed| ed.set(&["a"], &json!(2)), |_| Ok(())).unwrap();
        assert!(!same);
    }

    #[test]
    fn open_rejects_path_tricks_in_the_name() {
        let t = tempfile::tempdir().unwrap();
        let roots = DataRoots::under_base(Utf8Path::from_path(t.path()).unwrap());
        for bad in ["../x.jsonc", "a/b.jsonc", "", ".."] {
            assert!(
                Store::<RsDoc>::open(&roots, RootKind::Config, bad, clock()).is_err(),
                "{bad}"
            );
        }
        assert!(Store::<RsDoc>::open(&roots, RootKind::Config, "settings.jsonc", clock()).is_ok());
    }

    #[test]
    fn real_settings_type_round_trips_through_the_store() {
        use rimstudio_core::settings::Settings;
        let (_t, p) = temp();
        let s: Store<Settings> = Store::at(&p, clock());
        let l = s.load();
        s.save(&l.value).unwrap();
        let again = s.load();
        assert_eq!(again.origin, LoadOrigin::File);
        assert_eq!(again.value, l.value);
    }
}
