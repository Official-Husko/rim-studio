//! Schema versioning: the [`Versioned`] trait, load results and decoding helpers.
//!
//! A document type declares its kind and version. Single files (settings) carry the version in a
//! top level `schemaVersion` key; collection documents carry it in the envelope. Both stores share
//! [`Loaded`] to report what happened while reading: defaults, migration, a fallback to a backup
//! or a read only file written by a newer app.

use camino::Utf8Path;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::StoreError;
use crate::migrate::MigrationFn;
use rimstudio_core::settings::{
    SETTINGS_SCHEMA_VERSION, Settings, WORKSPACE_SCHEMA_VERSION, WorkspaceSettings,
};

/// The key that holds the version in a single file document.
pub const VERSION_FIELD: &str = "schemaVersion";

/// A type that is stored with a kind name and a schema version.
pub trait Versioned {
    /// The stable kind name, for example `settings`.
    const KIND: &'static str;
    /// The schema version this build writes (at least 1).
    const VERSION: u32;

    /// The migration steps of this type as `(from_version, step)` pairs. Defaults to none.
    fn migrations() -> Vec<(u32, MigrationFn)> {
        Vec::new()
    }
}

impl Versioned for Settings {
    const KIND: &'static str = "settings";
    const VERSION: u32 = SETTINGS_SCHEMA_VERSION;
}

impl Versioned for WorkspaceSettings {
    const KIND: &'static str = "workspace-settings";
    const VERSION: u32 = WORKSPACE_SCHEMA_VERSION;
}

/// How a value came to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOrigin {
    /// The file or document does not exist; the value is the default.
    Missing,
    /// Read as stored, at the current version.
    File,
    /// Read and migrated in memory from an older version.
    Migrated {
        /// The stored version.
        from: u32,
    },
    /// Written by a newer app; the value is a best effort view and must not be written back.
    NewerThanApp {
        /// The stored version.
        found: u32,
    },
    /// The file was unreadable; the value comes from the newest readable backup.
    Backup {
        /// The backup that was used.
        path: camino::Utf8PathBuf,
    },
    /// The file was unreadable and no backup helped; the value is the default.
    Fallback,
}

/// A problem found while loading, kept as plain data so [`Loaded`] stays cloneable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    /// The stable error code, for example `store.parse`.
    pub code: &'static str,
    /// A developer facing message.
    pub message: String,
    /// One based line, when known.
    pub line: Option<usize>,
    /// One based column, when known.
    pub column: Option<usize>,
}

impl LoadProblem {
    /// Describes a [`StoreError`].
    pub fn from_error(e: &StoreError) -> Self {
        let (line, column) = match e {
            StoreError::Parse { line, column, .. } => (Some(*line), Some(*column)),
            _ => (None, None),
        };
        LoadProblem {
            code: e.code(),
            message: e.to_string(),
            line,
            column,
        }
    }
}

/// The result of loading a document.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded<T> {
    /// The value.
    pub value: T,
    /// How it was obtained.
    pub origin: LoadOrigin,
    /// True when the value must not be written back (newer than the app, or unreadable file).
    pub read_only: bool,
    /// The problem that led to a fallback, if any.
    pub problem: Option<LoadProblem>,
}

impl<T> Loaded<T> {
    /// A freshly built value with no problems.
    pub fn new(value: T, origin: LoadOrigin) -> Self {
        Loaded {
            value,
            origin,
            read_only: false,
            problem: None,
        }
    }
}

/// The version stored in a top level `schemaVersion` key, `None` when absent or not a positive
/// integer.
pub fn read_version(value: &Value) -> Option<u32> {
    value
        .get(VERSION_FIELD)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
}

/// Sets `schemaVersion` as the first key of an object value; other values are left alone.
pub fn stamp_version(value: &mut Value, version: u32) {
    let Value::Object(map) = value else { return };
    if map.get(VERSION_FIELD).and_then(Value::as_u64) == Some(u64::from(version))
        && map.keys().next().map(String::as_str) == Some(VERSION_FIELD)
    {
        return;
    }
    let mut out = Map::new();
    out.insert(VERSION_FIELD.to_owned(), Value::from(version));
    for (k, v) in std::mem::take(map) {
        if k != VERSION_FIELD {
            out.insert(k, v);
        }
    }
    *map = out;
}

/// Decodes a JSON value into `T` and reports the path of the failing field.
pub fn decode_value<T: DeserializeOwned>(
    value: Value,
    kind: &'static str,
    path: &Utf8Path,
) -> Result<T, StoreError> {
    serde_path_to_error::deserialize(value).map_err(|e| StoreError::Shape {
        path: path.to_owned(),
        kind,
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn read_version_accepts_positive_integers_only() {
        assert_eq!(read_version(&json!({"schemaVersion": 3})), Some(3));
        assert_eq!(read_version(&json!({"schemaVersion": 0})), None);
        assert_eq!(read_version(&json!({"schemaVersion": "2"})), None);
        assert_eq!(
            read_version(&json!({"schemaVersion": 99999999999u64})),
            None
        );
        assert_eq!(read_version(&json!([1])), None);
    }

    #[test]
    fn stamp_moves_version_first() {
        let mut v = json!({"b": 1, "schemaVersion": 1, "a": 2});
        stamp_version(&mut v, 4);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["schemaVersion", "b", "a"]);
        assert_eq!(v["schemaVersion"], 4);
    }

    #[test]
    fn decode_reports_field_path() {
        #[derive(serde::Deserialize, Debug)]
        struct RsDoc {
            #[allow(dead_code)]
            inner: RsInner,
        }
        #[derive(serde::Deserialize, Debug)]
        struct RsInner {
            #[allow(dead_code)]
            count: u32,
        }
        let err = decode_value::<RsDoc>(
            json!({"inner": {"count": "x"}}),
            "rs-doc",
            Utf8Path::new("/x.json"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("inner.count"), "{err}");
        assert_eq!(err.code(), "store.shape");
    }

    #[test]
    fn settings_are_versioned() {
        assert_eq!(<Settings as Versioned>::KIND, "settings");
        assert_eq!(<Settings as Versioned>::VERSION, SETTINGS_SCHEMA_VERSION);
    }
}
