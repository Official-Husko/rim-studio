//! The game write fence (invariant I-05).
//!
//! Nothing under the RimWorld install folder or the game's config folder is written except through
//! [`GameWriteFence`]. The fence has two allow lists and refuses everything else:
//!
//! 1. entries inside `<install>/Mods` that RimStudio created and recorded as owned: directory links
//!    (created and removed through the [`LinkBackend`] port, unlink only, the target is never
//!    touched) and owned copies (a folder carrying the `.rimstudio.json` marker);
//! 2. the single file `ModsConfig.xml`, only after a verified timestamped backup and only when the
//!    caller attests that the game is not running ([`GameIdle`]).
//!
//! Operations are typed ([`FenceOp`]); every successful one is appended to a journal
//! ([`GameWriteFence::performed`]) so tests can assert the complete write set of a scenario with a
//! recording [`FenceFs`]. The designer and the project tools never use the fence for their own
//! output: they call [`GameWriteFence::check_project_write`], which accepts a path only when it is
//! inside the registered project roots and refuses anything inside a protected game folder.
//!
//! The caller owns the ownership manifest on disk (`rimstudio-library`); the fence is seeded with it
//! through [`GameWriteFence::with_owned`] and reports changes through
//! [`GameWriteFence::owned`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{Clock, LinkBackend};
use serde_json::json;

use crate::backup::{self, BackupPolicy};
use crate::error::GuardError;
use crate::guard::{
    FOLD_CASE_PATHS, RootGuard, ValidatedPath, lexical_clean, path_starts_with, resolve_path,
    sanitize_name,
};

/// The marker file inside an owned copy.
pub const COPY_MARKER: &str = ".rimstudio.json";

/// The writes the fence performs, abstracted so tests can record them.
pub trait FenceFs: Send + Sync {
    /// Replaces a file atomically (parents are created).
    fn write_file(&self, path: &Utf8Path, bytes: &[u8]) -> std::io::Result<()>;
    /// Copies a folder tree to a path that does not exist yet. Links inside the source are skipped.
    fn copy_tree(&self, source: &Utf8Path, dest: &Utf8Path) -> std::io::Result<()>;
    /// Removes a folder tree. Only called on a verified owned copy that is not a link.
    fn remove_tree(&self, path: &Utf8Path) -> std::io::Result<()>;
}

/// The real file system for the fence.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFenceFs;

fn copy_tree_inner(source: &Utf8Path, dest: &Utf8Path) -> std::io::Result<()> {
    fs_err::create_dir_all(dest)?;
    for entry in fs_err::read_dir(source)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let name = entry.file_name();
        let from = entry.path();
        let to = dest.as_std_path().join(&name);
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            let (Some(f), Some(t)) = (
                Utf8Path::from_path(&from).map(Utf8Path::to_path_buf),
                Utf8Path::from_path(&to).map(Utf8Path::to_path_buf),
            ) else {
                continue;
            };
            copy_tree_inner(&f, &t)?;
        } else if ft.is_file() {
            fs_err::copy(&from, &to)?;
        }
    }
    Ok(())
}

impl FenceFs for RealFenceFs {
    fn write_file(&self, path: &Utf8Path, bytes: &[u8]) -> std::io::Result<()> {
        crate::atomic::atomic_write(path, bytes).map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn copy_tree(&self, source: &Utf8Path, dest: &Utf8Path) -> std::io::Result<()> {
        copy_tree_inner(source, dest)
    }

    fn remove_tree(&self, path: &Utf8Path) -> std::io::Result<()> {
        fs_err::remove_dir_all(path)
    }
}

/// What kind of owned entry a name is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnedKind {
    /// A directory link (symlink or junction).
    Link,
    /// A copied folder with the marker file.
    Copy,
}

/// One entry of the ownership manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedEntry {
    /// The folder name inside `<install>/Mods`.
    pub name: String,
    /// Link or copy.
    pub kind: OwnedKind,
    /// The link target (links) or the copy source (copies).
    pub target: Utf8PathBuf,
}

/// The caller's attestation about the running game before `ModsConfig.xml` is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameIdle {
    /// The process probe and the Steam running flag say the game is not running.
    NotRunning,
    /// The state is unknown (a sandbox) and the person confirmed the game is closed.
    ConfirmedByUser,
}

/// A typed fence operation.
#[derive(Debug, Clone, Copy)]
pub enum FenceOp<'a> {
    /// Create a directory link `<install>/Mods/<name>` pointing at `target`.
    CreateOwnedLink {
        /// The link's folder name.
        name: &'a str,
        /// The absolute target.
        target: &'a Utf8Path,
    },
    /// Remove an owned link (unlink only).
    RemoveOwnedLink {
        /// The link's folder name.
        name: &'a str,
    },
    /// Copy `source` to `<install>/Mods/<name>` and write the marker.
    CreateOwnedCopy {
        /// The copy's folder name.
        name: &'a str,
        /// The absolute source folder (read only).
        source: &'a Utf8Path,
    },
    /// Remove an owned copy after verifying ownership.
    RemoveOwnedCopy {
        /// The copy's folder name.
        name: &'a str,
    },
    /// Replace `ModsConfig.xml` after a verified backup.
    WriteModsConfig {
        /// The new file content.
        bytes: &'a [u8],
        /// The attestation that the game is closed.
        game: GameIdle,
    },
}

impl FenceOp<'_> {
    /// The operation name used in errors and the journal.
    pub fn name(&self) -> &'static str {
        match self {
            FenceOp::CreateOwnedLink { .. } => "CreateOwnedLink",
            FenceOp::RemoveOwnedLink { .. } => "RemoveOwnedLink",
            FenceOp::CreateOwnedCopy { .. } => "CreateOwnedCopy",
            FenceOp::RemoveOwnedCopy { .. } => "RemoveOwnedCopy",
            FenceOp::WriteModsConfig { .. } => "WriteModsConfig",
        }
    }
}

/// One performed operation in the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceRecord {
    /// The operation name.
    pub operation: &'static str,
    /// The path that was written, linked or removed.
    pub path: Utf8PathBuf,
}

/// The folders the fence knows about.
#[derive(Debug, Clone)]
pub struct FenceConfig {
    /// `<install>/Mods`.
    pub install_mods: Utf8PathBuf,
    /// The path of `ModsConfig.xml` in the game's config folder.
    pub mods_config: Utf8PathBuf,
    /// Every folder that is protected: at least the install folder and the game config folder.
    pub protected: Vec<Utf8PathBuf>,
    /// The backup policy of `ModsConfig.xml` (the design: last 20 and 30 daily).
    pub backup: BackupPolicy,
}

impl FenceConfig {
    /// The default policy for `ModsConfig.xml` backups: the newest 20 plus the last of each day for
    /// 30 days, kept in `backup_root`.
    pub fn mods_config_backup_policy(backup_root: impl Into<Utf8PathBuf>) -> BackupPolicy {
        BackupPolicy {
            keep_last: 20,
            keep_all_today: false,
            daily_window_days: 30,
            skip_identical: false,
            root: Some(backup_root.into()),
        }
    }
}

/// The fence. Shareable between threads.
pub struct GameWriteFence {
    config: FenceConfig,
    protected: Vec<Utf8PathBuf>,
    mods_canonical: Utf8PathBuf,
    fs: Arc<dyn FenceFs>,
    links: Arc<dyn LinkBackend>,
    clock: Arc<dyn Clock>,
    owned: Mutex<BTreeMap<String, OwnedEntry>>,
    journal: Mutex<Vec<FenceRecord>>,
}

impl std::fmt::Debug for GameWriteFence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameWriteFence")
            .field("install_mods", &self.config.install_mods)
            .finish_non_exhaustive()
    }
}

fn lock<M>(m: &Mutex<M>) -> MutexGuard<'_, M> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn refused(op: &'static str, reason: impl Into<String>) -> GuardError {
    GuardError::FenceRefused {
        operation: op,
        reason: reason.into(),
    }
}

fn io_err(op: &'static str, path: &Utf8Path, source: std::io::Error) -> GuardError {
    GuardError::Io {
        op,
        path: path.as_str().chars().take(200).collect(),
        source,
    }
}

impl GameWriteFence {
    /// Builds a fence. All folders must be absolute. `install_mods` and `mods_config` are added to
    /// the protected set automatically.
    pub fn new(
        config: FenceConfig,
        fs: Arc<dyn FenceFs>,
        links: Arc<dyn LinkBackend>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, GuardError> {
        let mut protected = Vec::new();
        let extra = [
            Some(config.install_mods.as_path()),
            config.mods_config.parent(),
        ];
        for p in config
            .protected
            .iter()
            .map(Utf8PathBuf::as_path)
            .chain(extra.into_iter().flatten())
        {
            let canon = resolve_path(p)?;
            if !protected.contains(&canon) {
                protected.push(canon);
            }
        }
        let mods_canonical = resolve_path(&config.install_mods)?;
        Ok(GameWriteFence {
            config,
            protected,
            mods_canonical,
            fs,
            links,
            clock,
            owned: Mutex::new(BTreeMap::new()),
            journal: Mutex::new(Vec::new()),
        })
    }

    /// Seeds the ownership set from the manifest on disk.
    pub fn with_owned(self, entries: impl IntoIterator<Item = OwnedEntry>) -> Self {
        {
            let mut owned = lock(&self.owned);
            for e in entries {
                owned.insert(e.name.clone(), e);
            }
        }
        self
    }

    /// Replaces the ownership set with the entries of the manifest on disk. A caller that shares one
    /// fence between calls re-reads its manifest before every operation and seeds the fence with this.
    pub fn set_owned(&self, entries: impl IntoIterator<Item = OwnedEntry>) {
        let mut owned = lock(&self.owned);
        owned.clear();
        for e in entries {
            owned.insert(e.name.clone(), e);
        }
    }

    /// The `Mods` folder of the install this fence protects, as configured.
    pub fn install_mods(&self) -> &Utf8Path {
        &self.config.install_mods
    }

    /// The current ownership set (sorted by name), for persisting the manifest.
    pub fn owned(&self) -> Vec<OwnedEntry> {
        lock(&self.owned).values().cloned().collect()
    }

    /// The operations performed so far, in order.
    pub fn performed(&self) -> Vec<FenceRecord> {
        lock(&self.journal).clone()
    }

    fn record(&self, operation: &'static str, path: &Utf8Path) {
        lock(&self.journal).push(FenceRecord {
            operation,
            path: path.to_path_buf(),
        });
    }

    /// True when `path` (normalised, links resolved) is inside a protected game folder.
    pub fn is_protected(&self, path: &Utf8Path) -> Result<bool, GuardError> {
        let resolved = resolve_path(path)?;
        Ok(self
            .protected
            .iter()
            .any(|p| path_starts_with(&resolved, p, FOLD_CASE_PATHS)))
    }

    /// Checks a write of the designer or a project tool: the path must be inside the registered
    /// project roots (`guard`) and must not be inside any protected game folder.
    pub fn check_project_write(
        &self,
        guard: &RootGuard,
        path: &Utf8Path,
    ) -> Result<ValidatedPath, GuardError> {
        let v = guard.validate(path)?;
        if self
            .protected
            .iter()
            .any(|p| path_starts_with(&v.path, p, FOLD_CASE_PATHS))
        {
            return Err(refused(
                "ProjectWrite",
                "the path is inside a protected game folder",
            ));
        }
        Ok(v)
    }

    fn entry_path(&self, op: &'static str, name: &str) -> Result<Utf8PathBuf, GuardError> {
        sanitize_name(name)
            .map_err(|_| refused(op, "the name is not a single safe folder name"))?;
        if name.starts_with('.') {
            return Err(refused(op, "names starting with a dot are not allowed"));
        }
        Ok(self.config.install_mods.join(name))
    }

    fn require_absolute_source(op: &'static str, p: &Utf8Path) -> Result<Utf8PathBuf, GuardError> {
        let clean = lexical_clean(p)?;
        resolve_path(&clean).map_err(|e| match e {
            GuardError::LinkEscape { .. } => refused(op, "the path has a dangling link"),
            other => other,
        })
    }

    /// A link or copy source must lie outside the game folder (the folder that holds `Mods`) and must
    /// not contain it: a target inside the install would let the game read its own files as a mod, and
    /// a target that is an ancestor of `Mods` would make the link loop back into itself.
    fn require_outside_mods(&self, op: &'static str, p: &Utf8Path) -> Result<(), GuardError> {
        if p.starts_with(&self.mods_canonical) {
            return Err(refused(op, "the path is inside the Mods folder"));
        }
        if self.mods_canonical.starts_with(p) {
            return Err(refused(op, "the path contains the Mods folder"));
        }
        if let Some(game_root) = self.mods_canonical.parent()
            && p.starts_with(game_root)
        {
            return Err(refused(op, "the path is inside the game folder"));
        }
        Ok(())
    }

    fn name_is_free(&self, op: &'static str, path: &Utf8Path) -> Result<(), GuardError> {
        match fs_err::symlink_metadata(path) {
            Ok(_) => Err(refused(
                op,
                "an entry with that name already exists and is not ours",
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err("stat", path, e)),
        }
    }

    fn owned_entry(
        &self,
        op: &'static str,
        name: &str,
        kind: OwnedKind,
    ) -> Result<OwnedEntry, GuardError> {
        match lock(&self.owned).get(name) {
            Some(e) if e.kind == kind => Ok(e.clone()),
            Some(_) => Err(refused(op, "the entry is owned but is another kind")),
            None => Err(refused(op, "the entry is not in the ownership manifest")),
        }
    }

    /// Performs one typed operation. Every refusal is a [`GuardError::FenceRefused`]; the file
    /// system is untouched in that case.
    pub fn apply(&self, op: &FenceOp<'_>) -> Result<(), GuardError> {
        let name = op.name();
        match *op {
            FenceOp::CreateOwnedLink {
                name: entry,
                target,
            } => {
                let link = self.entry_path(name, entry)?;
                let target = Self::require_absolute_source(name, target)?;
                self.require_outside_mods(name, &target)?;
                self.name_is_free(name, &link)?;
                self.links
                    .create_dir_link(&target, &link)
                    .map_err(|e| refused(name, e.to_string()))?;
                lock(&self.owned).insert(
                    entry.to_owned(),
                    OwnedEntry {
                        name: entry.to_owned(),
                        kind: OwnedKind::Link,
                        target,
                    },
                );
                self.record(name, &link);
                Ok(())
            }
            FenceOp::RemoveOwnedLink { name: entry } => {
                let link = self.entry_path(name, entry)?;
                let owned = self.owned_entry(name, entry, OwnedKind::Link)?;
                if !self.links.is_link(&link) {
                    return Err(refused(name, "the entry is not a link any more"));
                }
                match self.links.read_link(&link) {
                    Some(t) if t == owned.target => {}
                    _ => {
                        return Err(refused(
                            name,
                            "the link no longer points at the recorded target",
                        ));
                    }
                }
                self.links
                    .remove_link(&link)
                    .map_err(|e| refused(name, e.to_string()))?;
                lock(&self.owned).remove(entry);
                self.record(name, &link);
                Ok(())
            }
            FenceOp::CreateOwnedCopy {
                name: entry,
                source,
            } => {
                let dest = self.entry_path(name, entry)?;
                let source = Self::require_absolute_source(name, source)?;
                self.require_outside_mods(name, &source)?;
                if !source.is_dir() {
                    return Err(refused(name, "the source is not a folder"));
                }
                self.name_is_free(name, &dest)?;
                let result = self.fs.copy_tree(&source, &dest).and_then(|()| {
                    let marker = json!({"owner": "rimstudio", "kind": "copy", "name": entry});
                    let bytes = crate::jsonc::to_json_bytes(&marker)
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    self.fs.write_file(&dest.join(COPY_MARKER), &bytes)
                });
                if let Err(e) = result {
                    // The destination did not exist before, so a partial copy is ours to remove.
                    let _ = self.fs.remove_tree(&dest);
                    return Err(io_err("copy", &dest, e));
                }
                lock(&self.owned).insert(
                    entry.to_owned(),
                    OwnedEntry {
                        name: entry.to_owned(),
                        kind: OwnedKind::Copy,
                        target: source,
                    },
                );
                self.record(name, &dest);
                Ok(())
            }
            FenceOp::RemoveOwnedCopy { name: entry } => {
                let path = self.entry_path(name, entry)?;
                self.owned_entry(name, entry, OwnedKind::Copy)?;
                let meta = fs_err::symlink_metadata(&path)
                    .map_err(|e| refused(name, format!("cannot inspect the entry: {e}")))?;
                if meta.file_type().is_symlink() || self.links.is_link(&path) {
                    return Err(refused(name, "the entry is a link, not a copy"));
                }
                if !meta.is_dir() {
                    return Err(refused(name, "the entry is not a folder"));
                }
                let parent = resolve_path(&path)?;
                if parent.parent() != Some(self.mods_canonical.as_path()) {
                    return Err(refused(
                        name,
                        "the entry is not directly inside the Mods folder",
                    ));
                }
                let marker = fs_err::read(path.join(COPY_MARKER))
                    .map_err(|_| refused(name, "the ownership marker is missing"))?;
                let valid = serde_json::from_slice::<serde_json::Value>(&marker)
                    .ok()
                    .and_then(|v| v.get("owner").and_then(|o| o.as_str().map(str::to_owned)))
                    .is_some_and(|o| o == "rimstudio");
                if !valid {
                    return Err(refused(name, "the ownership marker is not ours"));
                }
                self.fs
                    .remove_tree(&path)
                    .map_err(|e| io_err("remove", &path, e))?;
                lock(&self.owned).remove(entry);
                self.record(name, &path);
                Ok(())
            }
            FenceOp::WriteModsConfig { bytes, game: _ } => {
                let path = &self.config.mods_config;
                backup::rotate(path, &self.config.backup, &*self.clock)
                    .map_err(|e| refused(name, format!("the backup failed: {e}")))?;
                self.fs
                    .write_file(path, bytes)
                    .map_err(|e| io_err("write", path, e))?;
                self.record(name, path);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use rimstudio_core::ports::{LinkKind, LinkSupport, PortError, PortErrorKind, PortResult};

    use super::*;

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now_unix_ms(&self) -> u64 {
            self.0.fetch_add(1, Ordering::SeqCst)
        }
    }

    /// Records every call and writes nothing.
    #[derive(Default)]
    struct RecordingFs {
        calls: Mutex<Vec<String>>,
    }

    impl FenceFs for RecordingFs {
        fn write_file(&self, path: &Utf8Path, _bytes: &[u8]) -> std::io::Result<()> {
            lock(&self.calls).push(format!("write {path}"));
            Ok(())
        }
        fn copy_tree(&self, source: &Utf8Path, dest: &Utf8Path) -> std::io::Result<()> {
            lock(&self.calls).push(format!("copy {source} -> {dest}"));
            fs_err::create_dir_all(dest)
        }
        fn remove_tree(&self, path: &Utf8Path) -> std::io::Result<()> {
            lock(&self.calls).push(format!("remove {path}"));
            fs_err::remove_dir_all(path)
        }
    }

    /// An in memory link table that also creates real symlink free marker folders so name checks see
    /// the entry.
    #[derive(Default)]
    struct FakeLinks {
        links: Mutex<BTreeMap<Utf8PathBuf, Utf8PathBuf>>,
    }

    impl LinkBackend for FakeLinks {
        fn support(&self) -> LinkSupport {
            LinkSupport {
                symlink: true,
                junction: false,
                needs_privilege: false,
            }
        }
        fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
            fs_err::create_dir_all(link)
                .map_err(|e| PortError::new(PortErrorKind::Io, e.to_string()))?;
            lock(&self.links).insert(link.to_path_buf(), target.to_path_buf());
            Ok(LinkKind::Symlink)
        }
        fn remove_link(&self, link: &Utf8Path) -> PortResult<()> {
            lock(&self.links).remove(link);
            fs_err::remove_dir(link).map_err(|e| PortError::new(PortErrorKind::Io, e.to_string()))
        }
        fn is_link(&self, path: &Utf8Path) -> bool {
            lock(&self.links).contains_key(path)
        }
        fn read_link(&self, path: &Utf8Path) -> Option<Utf8PathBuf> {
            lock(&self.links).get(path).cloned()
        }
    }

    struct Fixture {
        _t: tempfile::TempDir,
        install: Utf8PathBuf,
        config: Utf8PathBuf,
        mods: Utf8PathBuf,
        custom: Utf8PathBuf,
        fs: Arc<RecordingFs>,
        links: Arc<FakeLinks>,
        fence: GameWriteFence,
    }

    fn fixture() -> Fixture {
        let t = tempfile::tempdir().unwrap();
        let base = Utf8PathBuf::from_path_buf(dunce::canonicalize(t.path()).unwrap()).unwrap();
        let install = base.join("RimWorld");
        let config = base.join("GameConfig");
        let mods = install.join("Mods");
        let custom = base.join("MyMods");
        for d in [&mods, &config, &custom.join("RS_ModA")] {
            fs_err::create_dir_all(d).unwrap();
        }
        fs_err::write(custom.join("RS_ModA/About.txt"), "a").unwrap();
        fs_err::write(config.join("ModsConfig.xml"), "old").unwrap();
        fs_err::write(config.join("Prefs.xml"), "prefs").unwrap();
        let fs = Arc::new(RecordingFs::default());
        let links = Arc::new(FakeLinks::default());
        let fence = GameWriteFence::new(
            FenceConfig {
                install_mods: mods.clone(),
                mods_config: config.join("ModsConfig.xml"),
                protected: vec![install.clone(), config.clone()],
                backup: FenceConfig::mods_config_backup_policy(base.join("backups")),
            },
            fs.clone(),
            links.clone(),
            Arc::new(TestClock(AtomicU64::new(1_894_708_800_000))),
        )
        .unwrap();
        Fixture {
            _t: t,
            install,
            config,
            mods,
            custom,
            fs,
            links,
            fence,
        }
    }

    fn refusal(r: Result<(), GuardError>) -> String {
        match r {
            Err(GuardError::FenceRefused { reason, .. }) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn owned_link_lifecycle_is_unlink_only() {
        let f = fixture();
        let target = f.custom.join("RS_ModA");
        f.fence
            .apply(&FenceOp::CreateOwnedLink {
                name: "RS_ModA__rs1",
                target: &target,
            })
            .unwrap();
        assert_eq!(f.fence.owned().len(), 1);
        f.fence
            .apply(&FenceOp::RemoveOwnedLink {
                name: "RS_ModA__rs1",
            })
            .unwrap();
        assert!(f.fence.owned().is_empty());
        assert!(
            target.join("About.txt").is_file(),
            "the target must stay intact"
        );
        let ops: Vec<_> = f
            .fence
            .performed()
            .into_iter()
            .map(|r| r.operation)
            .collect();
        assert_eq!(ops, ["CreateOwnedLink", "RemoveOwnedLink"]);
        assert!(
            f.fs.calls.lock().unwrap().is_empty(),
            "link operations never touch the fence fs"
        );
    }

    #[test]
    fn foreign_entries_are_never_replaced_or_removed() {
        let f = fixture();
        fs_err::create_dir_all(f.mods.join("Foreign")).unwrap();
        let target = f.custom.join("RS_ModA");
        assert!(
            refusal(f.fence.apply(&FenceOp::CreateOwnedLink {
                name: "Foreign",
                target: &target
            }))
            .contains("already exists")
        );
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedLink { name: "Foreign" }))
                .contains("manifest")
        );
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedCopy { name: "Foreign" }))
                .contains("manifest")
        );
        assert!(f.mods.join("Foreign").is_dir());
        assert!(f.fence.performed().is_empty());
    }

    #[test]
    fn retargeted_link_is_not_removed() {
        let f = fixture();
        let target = f.custom.join("RS_ModA");
        f.fence
            .apply(&FenceOp::CreateOwnedLink {
                name: "x",
                target: &target,
            })
            .unwrap();
        lock(&f.links.links).insert(f.mods.join("x"), f.custom.clone());
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedLink { name: "x" }))
                .contains("recorded target")
        );
        lock(&f.links.links).remove(&f.mods.join("x"));
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedLink { name: "x" })).contains("not a link")
        );
    }

    #[test]
    fn manifest_seeded_entries_can_be_removed() {
        let f = fixture();
        let target = f.custom.join("RS_ModA");
        fs_err::create_dir_all(f.mods.join("seeded")).unwrap();
        lock(&f.links.links).insert(f.mods.join("seeded"), target.clone());
        let fence = GameWriteFence::new(
            f.fence.config.clone(),
            f.fs.clone(),
            f.links.clone(),
            Arc::new(TestClock(AtomicU64::new(1))),
        )
        .unwrap()
        .with_owned([OwnedEntry {
            name: "seeded".into(),
            kind: OwnedKind::Link,
            target,
        }]);
        fence
            .apply(&FenceOp::RemoveOwnedLink { name: "seeded" })
            .unwrap();
    }

    #[test]
    fn bad_names_and_targets_are_refused() {
        let f = fixture();
        let target = f.custom.join("RS_ModA");
        for bad in ["", "..", "a/b", "../x", ".hidden", "con", "x."] {
            assert!(
                f.fence
                    .apply(&FenceOp::CreateOwnedLink {
                        name: bad,
                        target: &target
                    })
                    .is_err(),
                "{bad:?}"
            );
        }
        assert!(
            f.fence
                .apply(&FenceOp::CreateOwnedLink {
                    name: "ok",
                    target: Utf8Path::new("rel/x")
                })
                .is_err()
        );
        assert!(
            f.fence
                .apply(&FenceOp::CreateOwnedLink {
                    name: "ok",
                    target: &f.mods
                })
                .is_err()
        );
        assert!(
            f.fence
                .apply(&FenceOp::CreateOwnedLink {
                    name: "ok",
                    target: &f.mods.join("sub")
                })
                .is_err()
        );
        assert!(f.fence.performed().is_empty());
        assert!(fs_err::read_dir(&f.mods).unwrap().next().is_none());
    }

    #[test]
    fn owned_copy_lifecycle_uses_the_marker() {
        let f = fixture();
        let source = f.custom.join("RS_ModA");
        f.fence
            .apply(&FenceOp::CreateOwnedCopy {
                name: "RS_Copy",
                source: &source,
            })
            .unwrap();
        let calls = f.fs.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].starts_with("copy "));
        assert!(calls[1].starts_with("write ") && calls[1].ends_with(".rimstudio.json"));
        // The recording fs wrote no marker; add the real one to test removal.
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedCopy { name: "RS_Copy" }))
                .contains("marker is missing")
        );
        fs_err::write(
            f.mods.join("RS_Copy").join(COPY_MARKER),
            r#"{"owner":"someone"}"#,
        )
        .unwrap();
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedCopy { name: "RS_Copy" }))
                .contains("not ours")
        );
        fs_err::write(
            f.mods.join("RS_Copy").join(COPY_MARKER),
            r#"{"owner":"rimstudio"}"#,
        )
        .unwrap();
        f.fence
            .apply(&FenceOp::RemoveOwnedCopy { name: "RS_Copy" })
            .unwrap();
        assert!(!f.mods.join("RS_Copy").exists());
        assert!(source.join("About.txt").is_file());
    }

    #[test]
    fn owned_copy_that_became_a_link_is_not_removed() {
        let f = fixture();
        let source = f.custom.join("RS_ModA");
        f.fence
            .apply(&FenceOp::CreateOwnedCopy {
                name: "RS_Copy",
                source: &source,
            })
            .unwrap();
        lock(&f.links.links).insert(f.mods.join("RS_Copy"), source);
        assert!(
            refusal(f.fence.apply(&FenceOp::RemoveOwnedCopy { name: "RS_Copy" })).contains("link")
        );
    }

    #[test]
    fn real_copy_and_removal_with_the_real_fence_fs() {
        let f = fixture();
        let fence = GameWriteFence::new(
            f.fence.config.clone(),
            Arc::new(RealFenceFs),
            f.links.clone(),
            Arc::new(TestClock(AtomicU64::new(1))),
        )
        .unwrap();
        let source = f.custom.join("RS_ModA");
        fs_err::create_dir_all(source.join("Defs")).unwrap();
        fs_err::write(source.join("Defs/x.xml"), "d").unwrap();
        fence
            .apply(&FenceOp::CreateOwnedCopy {
                name: "RS_Real",
                source: &source,
            })
            .unwrap();
        assert_eq!(
            fs_err::read_to_string(f.mods.join("RS_Real/Defs/x.xml")).unwrap(),
            "d"
        );
        assert!(f.mods.join("RS_Real").join(COPY_MARKER).is_file());
        fence
            .apply(&FenceOp::RemoveOwnedCopy { name: "RS_Real" })
            .unwrap();
        assert!(!f.mods.join("RS_Real").exists());
        assert!(source.join("Defs/x.xml").is_file());
    }

    #[test]
    fn mods_config_write_makes_a_verified_backup_first() {
        let f = fixture();
        f.fence
            .apply(&FenceOp::WriteModsConfig {
                bytes: b"new",
                game: GameIdle::NotRunning,
            })
            .unwrap();
        let calls = f.fs.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            [format!("write {}", f.config.join("ModsConfig.xml"))]
        );
        let backups =
            backup::list_backups(&f.config.join("ModsConfig.xml"), &f.fence.config.backup);
        assert_eq!(backups.len(), 1);
        assert_eq!(fs_err::read_to_string(&backups[0].path).unwrap(), "old");
    }

    #[test]
    fn only_the_declared_mods_config_path_can_be_written() {
        // There is no operation that takes a path for a general write: the write set of every
        // scenario is the journal.
        let f = fixture();
        f.fence
            .apply(&FenceOp::WriteModsConfig {
                bytes: b"x",
                game: GameIdle::ConfirmedByUser,
            })
            .unwrap();
        let paths: Vec<_> = f.fence.performed().into_iter().map(|r| r.path).collect();
        assert_eq!(paths, [f.config.join("ModsConfig.xml")]);
        assert_eq!(
            fs_err::read_to_string(f.config.join("Prefs.xml")).unwrap(),
            "prefs"
        );
    }

    #[test]
    fn project_writes_are_confined_to_project_roots_and_never_the_game_folders() {
        let f = fixture();
        let project = f.custom.join("RS_ModA");
        let guard = RootGuard::new([&project, &f.install]).unwrap();
        assert!(
            f.fence
                .check_project_write(&guard, &project.join("Defs/Weapons.xml"))
                .is_ok()
        );
        // Allowed by the guard (the install is a root here) but protected by the fence.
        let err = f
            .fence
            .check_project_write(&guard, &f.install.join("Data/x.xml"))
            .unwrap_err();
        assert_eq!(err.code(), "fence.refused");
        let err = f
            .fence
            .check_project_write(&guard, &f.config.join("Prefs.xml"))
            .unwrap_err();
        assert_eq!(err.code(), "io.path-outside-roots");
        let err = f
            .fence
            .check_project_write(&guard, &f.custom.join("Other/x.xml"))
            .unwrap_err();
        assert_eq!(err.code(), "io.path-outside-roots");
        assert!(f.fence.is_protected(&f.mods.join("anything")).unwrap());
        assert!(f.fence.is_protected(&f.config.join("Prefs.xml")).unwrap());
        assert!(!f.fence.is_protected(&f.custom).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_alias_into_the_install_is_still_protected() {
        let f = fixture();
        let alias = f.custom.join("alias");
        if std::os::unix::fs::symlink(&f.install, &alias).is_err() {
            return;
        }
        assert!(f.fence.is_protected(&alias.join("Data/x.xml")).unwrap());
    }
}
