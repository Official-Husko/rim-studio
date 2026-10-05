//! The ownership manifest of the entries RimStudio created in a game `Mods` folder.
//!
//! One JSON document per entry in the data root (`game-links`). An entry is ours only when it is
//! recorded here and still looks like what was recorded; the fence of `rimstudio-io` refuses to remove
//! anything else.

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;
use rimstudio_io::collection::{Collection, CollectionOptions};
use rimstudio_io::error::StoreError;
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_io::schema::Versioned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The collection folder (below the data root).
pub const LINKS_COLLECTION: &str = "game-links";

/// How an entry was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryMode {
    /// A symbolic link.
    Symlink,
    /// A Windows junction.
    Junction,
    /// A marked copy of the folder.
    Copy,
}

impl EntryMode {
    /// True for the two link kinds.
    #[must_use]
    pub fn is_link(self) -> bool {
        !matches!(self, EntryMode::Copy)
    }

    /// The name on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EntryMode::Symlink => "symlink",
            EntryMode::Junction => "junction",
            EntryMode::Copy => "copy",
        }
    }
}

/// One recorded entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRecord {
    /// The document id.
    pub id: String,
    /// The `Mods` folder the entry lives in.
    pub mods_folder: Utf8PathBuf,
    /// The entry's folder name.
    pub name: String,
    /// How it was made.
    pub mode: EntryMode,
    /// The link target (links) or the copy source (copies), canonical.
    pub target: Utf8PathBuf,
    /// The project it was made for, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// When it was made, milliseconds since the Unix epoch.
    pub created_ms: u64,
    /// Keys written by a newer version, kept as they are.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Versioned for LinkRecord {
    const KIND: &'static str = "game-link";
    const VERSION: u32 = 1;
}

/// The document id of the entry `name` in `mods_folder`: a hash, so any folder name is a valid id.
#[must_use]
pub fn record_id(mods_folder: &Utf8Path, name: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(mods_folder.as_str().as_bytes());
    hasher.update(&[0]);
    hasher.update(name.as_bytes());
    let hex = hasher.finalize().to_hex();
    format!("l-{}", &hex.as_str()[..24])
}

/// The manifest store.
pub struct LinkManifest {
    collection: Collection<LinkRecord>,
}

impl std::fmt::Debug for LinkManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkManifest")
            .field("dir", &self.collection.dir())
            .finish()
    }
}

impl LinkManifest {
    /// Opens (creating when missing) the manifest in the data root.
    ///
    /// # Errors
    /// [`StoreError`] when the folder cannot be created.
    pub fn open(roots: &DataRoots, clock: Arc<dyn Clock>) -> Result<Self, StoreError> {
        let collection = Collection::open(roots, RootKind::Data, LINKS_COLLECTION, clock)?;
        Ok(Self::wrap(collection))
    }

    /// Opens (creating when missing) the manifest in an explicit folder.
    ///
    /// # Errors
    /// [`StoreError`] when the folder cannot be created.
    pub fn open_dir(
        dir: impl Into<Utf8PathBuf>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, StoreError> {
        Ok(Self::wrap(Collection::open_dir(dir, clock)?))
    }

    fn wrap(collection: Collection<LinkRecord>) -> Self {
        LinkManifest {
            collection: collection.with_options(CollectionOptions::user_data()),
        }
    }

    /// The record of one entry.
    ///
    /// # Errors
    /// [`StoreError`] when the document cannot be read.
    pub fn get(
        &self,
        mods_folder: &Utf8Path,
        name: &str,
    ) -> Result<Option<LinkRecord>, StoreError> {
        Ok(self
            .collection
            .get(&record_id(mods_folder, name))?
            .map(|loaded| loaded.value))
    }

    /// Every record of one `Mods` folder, sorted by name. A record that cannot be read is skipped.
    ///
    /// # Errors
    /// [`StoreError`] when the folder cannot be listed.
    pub fn list_for(&self, mods_folder: &Utf8Path) -> Result<Vec<LinkRecord>, StoreError> {
        let mut out = Vec::new();
        for id in self.collection.list_ids()? {
            if let Ok(Some(loaded)) = self.collection.get(&id)
                && loaded.value.mods_folder == mods_folder
            {
                out.push(loaded.value);
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Writes (replaces) a record atomically.
    ///
    /// # Errors
    /// [`StoreError`] when the write fails.
    pub fn put(&self, record: &LinkRecord) -> Result<(), StoreError> {
        self.collection.put(&record.id, record)
    }

    /// Deletes a record; false when there was none.
    ///
    /// # Errors
    /// [`StoreError`] when the delete fails.
    pub fn delete(&self, mods_folder: &Utf8Path, name: &str) -> Result<bool, StoreError> {
        self.collection.delete(&record_id(mods_folder, name))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now_unix_ms(&self) -> u64 {
            self.0.fetch_add(1, Ordering::SeqCst)
        }
    }

    fn record(mods: &str, name: &str) -> LinkRecord {
        let mods = Utf8PathBuf::from(mods);
        LinkRecord {
            id: record_id(&mods, name),
            mods_folder: mods,
            name: name.to_owned(),
            mode: EntryMode::Symlink,
            target: "/fiction/RS_Mod".into(),
            project_id: Some("p-1".into()),
            created_ms: 5,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn records_round_trip_and_are_listed_per_mods_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("game-links")).unwrap();
        let m = LinkManifest::open_dir(path, Arc::new(TestClock(AtomicU64::new(1)))).unwrap();
        let a = record("/g/Mods", "RS_A");
        let b = record("/g/Mods", "RS_B");
        let other = record("/h/Mods", "RS_A");
        for r in [&b, &a, &other] {
            m.put(r).unwrap();
        }
        assert_eq!(
            m.get(Utf8Path::new("/g/Mods"), "RS_A").unwrap(),
            Some(a.clone())
        );
        let names: Vec<_> = m
            .list_for(Utf8Path::new("/g/Mods"))
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["RS_A", "RS_B"]);
        assert!(m.delete(Utf8Path::new("/g/Mods"), "RS_A").unwrap());
        assert!(!m.delete(Utf8Path::new("/g/Mods"), "RS_A").unwrap());
        assert_eq!(
            m.get(Utf8Path::new("/h/Mods"), "RS_A").unwrap(),
            Some(other)
        );
    }

    #[test]
    fn the_id_is_a_valid_document_id_for_any_name() {
        for name in ["RS_Mod", "Ünï cödé", "with space", "con", "a.b"] {
            let id = record_id(Utf8Path::new("/g/Mods"), name);
            rimstudio_io::collection::validate_id(&id).unwrap();
        }
        assert_ne!(
            record_id(Utf8Path::new("/g/Mods"), "a"),
            record_id(Utf8Path::new("/g/Mods"), "b")
        );
    }
}
