//! Projects: the mod folders the user works on, and the store that remembers them.
//!
//! A project is a mod folder. The tool owned record (path, name, package id, times, the last used
//! designer settings) lives in the app's own JSON document store ([`Collection`]) under the data
//! root, never inside the mod folder: opening or registering a project writes nothing there. The
//! folder itself is only read, and only `About/About.xml` is required.
//!
//! The project id is derived from the normalised folder path (`p-` and eight hex characters), so
//! registering the same folder twice is the same project. A folder that moved is a new project; the
//! old record can be removed.
//!
//! [`read_mod_meta`] reads a mod folder into the core [`ModMeta`] the same way the library scanner
//! does for one folder (About, LoadFolders, folder names); the session uses it for the project
//! overlay.

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ids::{PackageId, ProjectId, SourceId};
use rimstudio_core::mods::ModMeta;
use rimstudio_core::paths;
use rimstudio_core::ports::Clock;
use rimstudio_io::collection::{Collection, CollectionOptions};
use rimstudio_io::guard::lexical_clean;
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_io::schema::Versioned;
use rimstudio_xml::{about, load_folders};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{WorkspaceError, WorkspaceResult, codes};
use crate::listing::DiskListing;

/// The schema version of [`ProjectRecord`].
pub const PROJECT_SCHEMA_VERSION: u32 = 1;

/// The name of the collection folder under the data root.
pub const PROJECTS_COLLECTION: &str = "projects";

/// The source id given to the project folder in the content pack list.
pub const PROJECT_SOURCE_ID: &str = "project";

/// The tool owned record of one project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    /// The project id, derived from the path.
    pub id: ProjectId,
    /// The absolute mod folder.
    pub path: Utf8PathBuf,
    /// The mod's display name (from About.xml, else the folder name).
    pub name: String,
    /// The `packageId` of About.xml when it has a usable one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// When the project was first registered, milliseconds since the Unix epoch.
    pub created_ms: u64,
    /// When the project was last opened, milliseconds since the Unix epoch.
    pub last_opened_ms: u64,
    /// The game version the project targets (`major.minor`), when the user chose one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,
    /// The last used designer settings, an opaque JSON value owned by the designer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer: Option<Value>,
    /// Keys written by a newer version of the app, kept as they are.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Versioned for ProjectRecord {
    const KIND: &'static str = "project";
    const VERSION: u32 = PROJECT_SCHEMA_VERSION;
}

fn summarize(record: &ProjectRecord) -> Value {
    json!({
        "path": record.path,
        "name": record.name,
        "packageId": record.package_id,
        "lastOpenedMs": record.last_opened_ms,
    })
}

/// The id of the project at `path` (the path must already be normalised).
#[must_use]
pub fn project_id_for(path: &Utf8Path) -> ProjectId {
    ProjectId::from_seed(path.as_str().as_bytes())
}

/// Normalises a project folder path: absolute, no `.` parts, no `..`, no NUL.
///
/// # Errors
/// [`WorkspaceError::InvalidPath`] for anything else.
pub fn normalize_project_path(path: &Utf8Path) -> WorkspaceResult<Utf8PathBuf> {
    lexical_clean(path).map_err(|e| WorkspaceError::InvalidPath {
        path: path.as_str().chars().take(120).collect(),
        reason: e.to_string(),
    })
}

/// Finds the About file of a mod folder: the folder `About` (any letter case) and in it the file
/// `About.xml` (any letter case). Exact spellings win over case variants. Read only.
#[must_use]
pub fn find_about_file(root: &Utf8Path) -> Option<Utf8PathBuf> {
    let dir = find_entry(root, paths::ABOUT_DIR, true)?;
    find_entry(&dir, paths::ABOUT_XML_NAME, false)
}

/// Finds `LoadFolders.xml` (any letter case) directly in the mod root. Read only.
#[must_use]
pub fn find_load_folders_file(root: &Utf8Path) -> Option<Utf8PathBuf> {
    find_entry(root, paths::LOAD_FOLDERS_XML, false)
}

fn find_entry(dir: &Utf8Path, name: &str, want_dir: bool) -> Option<Utf8PathBuf> {
    let exact = dir.join(name);
    let ok = |p: &Utf8Path| {
        std::fs::metadata(p.as_std_path())
            .is_ok_and(|m| if want_dir { m.is_dir() } else { m.is_file() })
    };
    if ok(&exact) {
        return Some(exact);
    }
    let read = std::fs::read_dir(dir.as_std_path()).ok()?;
    let mut candidates: Vec<String> = read
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n.eq_ignore_ascii_case(name))
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| ok(p.as_path()))
}

/// What [`read_mod_meta`] found.
#[derive(Debug, Clone)]
pub struct ModMetaRead {
    /// The mod metadata, filled as the library scanner fills it (About, LoadFolders, root folders).
    pub meta: ModMeta,
    /// The About path that was read.
    pub about_path: Utf8PathBuf,
    /// False when About.xml could not be parsed (defaults were used).
    pub about_parsed: bool,
    /// Warnings of the reading (About quirks, a made up package id).
    pub diagnostics: Vec<Diagnostic>,
}

fn fallback_package_id(root: &Utf8Path) -> String {
    let folder = root.file_name().unwrap_or("unnamed");
    let cleaned: String = folder
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = if cleaned.is_empty() {
        "unnamed".to_owned()
    } else {
        cleaned
    };
    format!("rs.project.{cleaned}")
}

/// Reads a mod folder into a [`ModMeta`]: About.xml (leniently), `LoadFolders.xml` when present, and
/// the names of the folders directly inside the root. A missing package id is replaced by a made up
/// `rs.project.<folder>` id with a warning, so the folder can still be worked on.
///
/// # Errors
/// [`WorkspaceError::NotAProject`] when there is no About file; [`WorkspaceError::Io`] when it
/// cannot be read.
pub fn read_mod_meta(root: &Utf8Path) -> WorkspaceResult<ModMetaRead> {
    let about_path = find_about_file(root).ok_or_else(|| WorkspaceError::NotAProject {
        path: root.to_string(),
    })?;
    let bytes =
        fs_err::read(&about_path).map_err(|e| WorkspaceError::io("read-about", &about_path, &e))?;
    let read = about::read_lenient(&bytes);
    let mut diagnostics = read.warnings;
    let about_parsed = read.parsed;
    if !about_parsed {
        diagnostics.push(Diagnostic::new(
            codes::PROJECT_ABOUT_UNREADABLE,
            Severity::Warning,
            format!("{about_path} could not be parsed, defaults are used"),
        ));
    }
    let mut about = read.about;
    if PackageId::parse(&about.package_id).is_err() {
        let made_up = fallback_package_id(root);
        diagnostics.push(Diagnostic::new(
            codes::PROJECT_ABOUT_UNREADABLE,
            Severity::Warning,
            format!("{about_path} has no usable packageId, using {made_up}"),
        ));
        about.package_id = made_up;
    }
    let source = SourceId::new(PROJECT_SOURCE_ID).map_err(|_| WorkspaceError::InvalidPath {
        path: root.to_string(),
        reason: "bad source id".to_owned(),
    })?;
    let mut meta =
        about
            .into_meta(source, root.to_owned())
            .map_err(|e| WorkspaceError::InvalidPath {
                path: root.to_string(),
                reason: e.to_string(),
            })?;
    if let Some(lf_path) = find_load_folders_file(root) {
        match fs_err::read(&lf_path) {
            Ok(bytes) => match load_folders::read(&bytes) {
                Ok(read) => {
                    diagnostics.extend(read.diagnostics);
                    meta.load_folders = Some(read.spec);
                }
                Err(e) => diagnostics.push(e.to_diagnostic()),
            },
            Err(e) => diagnostics.push(Diagnostic::new(
                codes::FILE_UNREADABLE,
                Severity::Warning,
                format!("{lf_path}: {e}"),
            )),
        }
    }
    meta.root_dirs = DiskListing::child_dirs(root);
    Ok(ModMetaRead {
        meta,
        about_path,
        about_parsed,
        diagnostics,
    })
}

/// The store of registered projects, one JSON document per project in the data root.
#[derive(Clone)]
pub struct ProjectStore {
    collection: Collection<ProjectRecord>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for ProjectStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectStore")
            .field("dir", &self.collection.dir())
            .finish_non_exhaustive()
    }
}

impl ProjectStore {
    /// Opens (creating when missing) the store in the data root.
    ///
    /// # Errors
    /// [`WorkspaceError::Store`] when the folder cannot be created.
    pub fn open_in(roots: &DataRoots, clock: Arc<dyn Clock>) -> WorkspaceResult<ProjectStore> {
        let collection = Collection::open(
            roots,
            RootKind::Data,
            PROJECTS_COLLECTION,
            Arc::clone(&clock),
        )?;
        Ok(Self::from_collection(collection, clock))
    }

    /// Opens (creating when missing) the store in an explicit folder.
    ///
    /// # Errors
    /// [`WorkspaceError::Store`] when the folder cannot be created.
    pub fn open_dir(
        dir: impl Into<Utf8PathBuf>,
        clock: Arc<dyn Clock>,
    ) -> WorkspaceResult<ProjectStore> {
        let collection = Collection::open_dir(dir, Arc::clone(&clock))?;
        Ok(Self::from_collection(collection, clock))
    }

    fn from_collection(collection: Collection<ProjectRecord>, clock: Arc<dyn Clock>) -> Self {
        ProjectStore {
            collection: collection
                .with_options(CollectionOptions::user_data())
                .with_summarizer(summarize),
            clock,
        }
    }

    /// The folder of the store.
    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        self.collection.dir()
    }

    /// Opens a project: checks that the folder has an About.xml (read only), registers the folder
    /// when it is new, refreshes the name and package id from About.xml and stamps the last opened
    /// time. Nothing is written into the mod folder.
    ///
    /// # Errors
    /// [`WorkspaceError::InvalidPath`], [`WorkspaceError::NotAProject`], [`WorkspaceError::Io`] or a
    /// store error.
    pub fn open(&self, path: &Utf8Path) -> WorkspaceResult<ProjectRecord> {
        let path = normalize_project_path(path)?;
        let about_path = find_about_file(&path).ok_or_else(|| WorkspaceError::NotAProject {
            path: path.to_string(),
        })?;
        let bytes = fs_err::read(&about_path)
            .map_err(|e| WorkspaceError::io("read-about", &about_path, &e))?;
        let read = about::read_lenient(&bytes);
        let name = if read.about.name.trim().is_empty() {
            path.file_name().unwrap_or("").to_owned()
        } else {
            read.about.name.trim().to_owned()
        };
        let package_id = PackageId::parse(&read.about.package_id)
            .ok()
            .map(|p| p.as_str().to_owned());
        let now = self.clock.now_unix_ms();
        let id = project_id_for(&path);
        let existing = self.collection.get(id.as_str())?.map(|l| l.value);
        let record = match existing {
            Some(mut record) => {
                record.name = name;
                record.package_id = package_id;
                record.path = path;
                record.last_opened_ms = now;
                record
            }
            None => ProjectRecord {
                id: id.clone(),
                path,
                name,
                package_id,
                created_ms: now,
                last_opened_ms: now,
                target_version: None,
                designer: None,
                extra: BTreeMap::new(),
            },
        };
        self.collection.put(id.as_str(), &record)?;
        Ok(record)
    }

    /// The record with this id.
    ///
    /// # Errors
    /// A store error when the document is damaged (it is then moved to quarantine).
    pub fn get(&self, id: &ProjectId) -> WorkspaceResult<Option<ProjectRecord>> {
        Ok(self.collection.get(id.as_str())?.map(|l| l.value))
    }

    /// The record of the project at `path`, when registered.
    ///
    /// # Errors
    /// [`WorkspaceError::InvalidPath`] or a store error.
    pub fn find_by_path(&self, path: &Utf8Path) -> WorkspaceResult<Option<ProjectRecord>> {
        let path = normalize_project_path(path)?;
        self.get(&project_id_for(&path))
    }

    /// Every registered project, the most recently opened first (ties by name, then id). Damaged
    /// documents are skipped (the store quarantines them).
    ///
    /// # Errors
    /// A store error when the folder cannot be read.
    pub fn list(&self) -> WorkspaceResult<Vec<ProjectRecord>> {
        let mut out: Vec<ProjectRecord> = self
            .collection
            .scan()?
            .filter_map(Result::ok)
            .map(|item| item.loaded.value)
            .collect();
        out.sort_by(|a, b| {
            b.last_opened_ms
                .cmp(&a.last_opened_ms)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(out)
    }

    /// Removes a project from the store. The mod folder is never touched. Returns whether a record
    /// existed.
    ///
    /// # Errors
    /// A store error.
    pub fn remove(&self, id: &ProjectId) -> WorkspaceResult<bool> {
        Ok(self.collection.delete(id.as_str())?)
    }

    /// Changes a record with a closure and saves it.
    ///
    /// # Errors
    /// [`WorkspaceError::ProjectNotFound`] or a store error.
    pub fn update(
        &self,
        id: &ProjectId,
        change: impl FnOnce(&mut ProjectRecord),
    ) -> WorkspaceResult<ProjectRecord> {
        let mut record = self
            .get(id)?
            .ok_or_else(|| WorkspaceError::ProjectNotFound { id: id.to_string() })?;
        change(&mut record);
        self.collection.put(id.as_str(), &record)?;
        Ok(record)
    }

    /// Stores the last used designer settings of a project.
    ///
    /// # Errors
    /// [`WorkspaceError::ProjectNotFound`] or a store error.
    pub fn set_designer_settings(
        &self,
        id: &ProjectId,
        settings: Value,
    ) -> WorkspaceResult<ProjectRecord> {
        self.update(id, |r| r.designer = Some(settings))
    }

    /// Stores the target game version of a project.
    ///
    /// # Errors
    /// [`WorkspaceError::ProjectNotFound`] or a store error.
    pub fn set_target_version(
        &self,
        id: &ProjectId,
        version: Option<String>,
    ) -> WorkspaceResult<ProjectRecord> {
        self.update(id, |r| r.target_version = version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_testing::fakes::FakeClock;
    use rimstudio_testing::install_tree::ModFolder;

    fn utf8(p: &std::path::Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()
    }

    fn write_mod(parent: &Utf8Path, folder: &str, package: &str, name: &str) -> Utf8PathBuf {
        ModFolder::new(folder, package)
            .name(name)
            .write_to(parent)
            .unwrap()
    }

    #[test]
    fn project_id_is_stable_for_one_path() {
        let a = project_id_for(Utf8Path::new("/mods/A"));
        assert_eq!(a, project_id_for(Utf8Path::new("/mods/A")));
        assert_ne!(a, project_id_for(Utf8Path::new("/mods/B")));
        assert!(a.as_str().starts_with("p-"));
    }

    #[test]
    fn about_is_found_ignoring_case() {
        let tmp = tempfile::tempdir().unwrap();
        let root = utf8(tmp.path());
        let path = ModFolder::new("M", "rs.m")
            .lowercase_about_dir()
            .about_file_name("about.xml")
            .write_to(&root)
            .unwrap();
        let found = find_about_file(&path).unwrap();
        assert!(found.as_str().ends_with("about/about.xml"));
        assert!(find_about_file(&root).is_none());
    }

    #[test]
    fn open_registers_lists_and_removes_without_touching_the_mod() {
        let mods = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let mods_root = utf8(mods.path());
        let a = write_mod(&mods_root, "RS_ModA", "rs.mod.a", "RS Mod A");
        let b = write_mod(&mods_root, "RS_ModB", "rs.mod.b", "RS Mod B");
        let before = snapshot_files(&mods_root);
        let clock = FakeClock::new(1_000);
        let store =
            ProjectStore::open_dir(utf8(data.path()).join("projects"), Arc::new(clock.clone()))
                .unwrap();

        let ra = store.open(&a).unwrap();
        assert_eq!(ra.name, "RS Mod A");
        assert_eq!(ra.package_id.as_deref(), Some("rs.mod.a"));
        assert_eq!(ra.created_ms, 1_000);
        clock.advance(500);
        let rb = store.open(&b).unwrap();
        assert_eq!(rb.created_ms, 1_500);
        clock.advance(500);
        let ra2 = store.open(&a).unwrap();
        assert_eq!(ra2.id, ra.id);
        assert_eq!(ra2.created_ms, 1_000);
        assert_eq!(ra2.last_opened_ms, 2_000);

        let listed = store.list().unwrap();
        assert_eq!(
            listed.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            vec!["RS Mod A", "RS Mod B"]
        );
        assert_eq!(store.find_by_path(&a).unwrap().unwrap().id, ra.id);
        assert!(store.remove(&ra.id).unwrap());
        assert!(!store.remove(&ra.id).unwrap());
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(
            snapshot_files(&mods_root),
            before,
            "the mod folder must not change"
        );
    }

    fn snapshot_files(root: &Utf8Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_owned()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = utf8(&e.path());
                let meta = e.metadata().unwrap();
                out.push(format!("{p}:{}", meta.len()));
                if meta.is_dir() {
                    stack.push(p);
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn open_rejects_a_folder_without_about() {
        let tmp = tempfile::tempdir().unwrap();
        let root = utf8(tmp.path());
        std::fs::create_dir_all(root.join("Plain/Defs")).unwrap();
        let store =
            ProjectStore::open_dir(root.join("store"), Arc::new(FakeClock::new(1))).unwrap();
        let err = store.open(&root.join("Plain")).unwrap_err();
        assert_eq!(err.code(), "workspace.not-a-project");
        let err = store.open(Utf8Path::new("relative/path")).unwrap_err();
        assert_eq!(err.code(), "workspace.invalid-path");
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn designer_settings_round_trip_and_unknown_keys_survive() {
        let tmp = tempfile::tempdir().unwrap();
        let root = utf8(tmp.path());
        let m = write_mod(&root, "RS_Mod", "rs.mod", "RS Mod");
        let store =
            ProjectStore::open_dir(root.join("store"), Arc::new(FakeClock::new(5))).unwrap();
        let rec = store.open(&m).unwrap();
        store
            .set_designer_settings(&rec.id, json!({"lastTemplate": "RS_rifle", "ce": false}))
            .unwrap();
        store
            .set_target_version(&rec.id, Some("1.6".into()))
            .unwrap();
        let again = store.get(&rec.id).unwrap().unwrap();
        assert_eq!(
            again.designer,
            Some(json!({"lastTemplate": "RS_rifle", "ce": false}))
        );
        assert_eq!(again.target_version.as_deref(), Some("1.6"));
        // a key from a newer app is kept through an update
        let file = store.dir().join(format!("{}.json", rec.id));
        let text = std::fs::read_to_string(&file).unwrap();
        let mut value: Value = serde_json::from_str(&text).unwrap();
        value["data"]["futureKey"] = json!(7);
        std::fs::write(&file, serde_json::to_string(&value).unwrap()).unwrap();
        store
            .update(&rec.id, |r| r.name = "Renamed".into())
            .unwrap();
        let after = store.get(&rec.id).unwrap().unwrap();
        assert_eq!(after.extra.get("futureKey"), Some(&json!(7)));
        assert_eq!(after.name, "Renamed");
        assert!(matches!(
            store
                .update(&ProjectId::from_seed(b"nothing"), |_| {})
                .unwrap_err(),
            WorkspaceError::ProjectNotFound { .. }
        ));
    }

    #[test]
    fn read_mod_meta_fills_load_folders_and_root_dirs() {
        use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
        let tmp = tempfile::tempdir().unwrap();
        let root = utf8(tmp.path());
        let spec = LoadFoldersSpec::from_blocks([(
            "1.6",
            vec![
                LoadEntry::root(),
                LoadEntry::dir("CE").if_active(["rs.other"]),
            ],
        )]);
        let m = ModFolder::new("RS_Gated", "rs.gated")
            .load_folders(spec.clone())
            .empty_dir("CE/Patches")
            .write_to(&root)
            .unwrap();
        let read = read_mod_meta(&m).unwrap();
        assert!(read.about_parsed);
        assert_eq!(read.meta.package_id.as_str(), "rs.gated");
        assert_eq!(read.meta.load_folders, Some(spec));
        assert!(read.meta.root_dirs.contains(&"CE".to_owned()));
        assert!(read.meta.root_dirs.contains(&"About".to_owned()));
    }

    #[test]
    fn read_mod_meta_makes_up_a_package_id_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = utf8(tmp.path());
        let m = ModFolder::new("My Cool-Mod", "x")
            .raw_about(b"<ModMetaData><name>Plain</name></ModMetaData>".to_vec())
            .write_to(&root)
            .unwrap();
        let read = read_mod_meta(&m).unwrap();
        assert_eq!(read.meta.package_id.as_str(), "rs.project.my_cool_mod");
        assert!(
            read.diagnostics
                .iter()
                .any(|d| d.code == codes::PROJECT_ABOUT_UNREADABLE)
        );
    }
}
