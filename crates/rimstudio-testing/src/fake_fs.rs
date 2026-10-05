//! [`FakeFs`]: an in memory tree that implements `FsProbe`, and [`RecordingFs`], which adds write
//! operations on top of it and records every attempt for fence tests.
//!
//! Paths are text with `/` or `\` separators and are normalised to `/`. Windows style drive paths
//! (`C:\Steam`) work: `C:` is the root of that drive. The tree can be case insensitive to mimic
//! Windows and macOS volumes. Symbolic links are followed with a hop limit, so dangling links and
//! loops behave like the real thing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{
    DirEntryInfo, EntryKind, FsMeta, FsProbe, PortError, PortErrorKind, PortResult, VolumeClass,
};
use serde::Serialize;

const MAX_HOPS: usize = 40;
const BASE_MTIME_NS: i128 = 1_700_000_000_000_000_000;
const TICK_NS: i128 = 1_000_000_000;

#[derive(Debug, Clone)]
enum Node {
    File(Vec<u8>),
    Dir,
    Symlink(String),
}

#[derive(Debug, Clone)]
struct Entry {
    /// Normalised path in the case it was created with.
    path: String,
    node: Node,
    id: u128,
    mtime_ns: Option<i128>,
}

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<String, Entry>,
    next_id: u128,
    clock_ns: i128,
    case_insensitive: bool,
    denied: BTreeSet<String>,
    hung: BTreeSet<String>,
    volumes: Vec<(String, VolumeClass)>,
    default_volume: Option<VolumeClass>,
}

/// An in memory file system implementing [`FsProbe`].
#[derive(Debug, Default)]
pub struct FakeFs {
    state: Mutex<State>,
}

/// Normalises a path to `/` separators, removing `.`, empty parts and resolving `..`.
fn normalize(path: &str) -> String {
    let text = path.replace('\\', "/");
    let absolute = text.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for p in text.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if absolute {
        format!("/{}", parts.join("/"))
    } else {
        parts.join("/")
    }
}

fn parent_of(norm: &str) -> Option<String> {
    if norm == "/" || norm.is_empty() {
        return None;
    }
    match norm.rfind('/') {
        Some(0) => Some("/".to_owned()),
        Some(i) => norm.get(..i).map(str::to_owned),
        None => None,
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn is_absolute(norm: &str) -> bool {
    norm.starts_with('/') || norm.as_bytes().get(1) == Some(&b':')
}

fn not_found(path: &str) -> PortError {
    PortError::new(PortErrorKind::NotFound, format!("no such path: {path}"))
}

impl State {
    fn key(&self, norm: &str) -> String {
        if self.case_insensitive {
            norm.to_lowercase()
        } else {
            norm.to_owned()
        }
    }

    fn is_under(&self, norm: &str, prefix_key: &str) -> bool {
        let k = self.key(norm);
        k == prefix_key
            || (k.starts_with(prefix_key)
                && (prefix_key.ends_with('/') || k.as_bytes().get(prefix_key.len()) == Some(&b'/')))
    }

    fn check_hung(&self, norm: &str) -> PortResult<()> {
        if self.hung.iter().any(|p| self.is_under(norm, p)) {
            return Err(PortError::new(
                PortErrorKind::TimedOut,
                format!("simulated hang: {norm}"),
            ));
        }
        Ok(())
    }

    fn check_denied(&self, norm: &str) -> PortResult<()> {
        if self.denied.iter().any(|p| self.is_under(norm, p)) {
            return Err(PortError::new(
                PortErrorKind::PermissionDenied,
                format!("simulated permission error: {norm}"),
            ));
        }
        Ok(())
    }

    fn next_mtime(&mut self) -> i128 {
        self.clock_ns += TICK_NS;
        BASE_MTIME_NS + self.clock_ns
    }

    fn insert(&mut self, norm: String, node: Node) {
        let key = self.key(&norm);
        let mtime_ns = Some(self.next_mtime());
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(
            key,
            Entry {
                path: norm,
                node,
                id,
                mtime_ns,
            },
        );
    }

    fn ensure_parents(&mut self, norm: &str) {
        let mut chain = Vec::new();
        let mut cur = parent_of(norm);
        while let Some(p) = cur {
            if self.entries.contains_key(&self.key(&p)) {
                break;
            }
            cur = parent_of(&p);
            chain.push(p);
        }
        for p in chain.into_iter().rev() {
            self.insert(p, Node::Dir);
        }
    }

    /// Resolves symbolic links in every component (the last one too when `follow_last`) and returns
    /// the key of the final entry, which must exist.
    fn resolve(&self, path: &str, follow_last: bool) -> PortResult<String> {
        let mut pending: Vec<String> = normalize(path)
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let absolute_unix = normalize(path).starts_with('/');
        let mut hops = 0;
        let mut cur = if absolute_unix {
            "/".to_owned()
        } else {
            String::new()
        };
        let mut idx = 0;
        while idx < pending.len() {
            let Some(name) = pending.get(idx) else { break };
            let next = join(&cur, name);
            let last = idx + 1 == pending.len();
            let Some(entry) = self.entries.get(&self.key(&next)) else {
                return Err(not_found(&next));
            };
            match &entry.node {
                Node::Symlink(target) if !last || follow_last => {
                    hops += 1;
                    if hops > MAX_HOPS {
                        return Err(PortError::new(
                            PortErrorKind::Io,
                            format!("too many levels of symbolic links: {path}"),
                        ));
                    }
                    let base = if is_absolute(&normalize(target)) {
                        normalize(target)
                    } else {
                        let joined = if cur.is_empty() {
                            target.clone()
                        } else {
                            format!("{cur}/{target}")
                        };
                        normalize(&joined)
                    };
                    let mut rest: Vec<String> = base
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    rest.extend(pending.iter().skip(idx + 1).cloned());
                    cur = if base.starts_with('/') {
                        "/".to_owned()
                    } else {
                        String::new()
                    };
                    pending = rest;
                    idx = 0;
                }
                _ => {
                    cur = entry.path.clone();
                    idx += 1;
                }
            }
        }
        let key = self.key(&cur);
        if cur.is_empty() || !self.entries.contains_key(&key) {
            return Err(not_found(path));
        }
        Ok(key)
    }
}

impl FakeFs {
    /// An empty, case sensitive tree with a root folder `/`.
    pub fn new() -> Self {
        let fs = FakeFs::default();
        fs.add_dir("/");
        fs
    }

    /// An empty case insensitive tree (Windows and macOS style).
    pub fn case_insensitive() -> Self {
        let fs = FakeFs::default();
        fs.lock().case_insensitive = true;
        fs.add_dir("/");
        fs
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds a file and any missing parent folders (set up, never fails).
    pub fn add_file(&self, path: impl AsRef<str>, content: impl Into<Vec<u8>>) {
        let norm = normalize(path.as_ref());
        let mut st = self.lock();
        st.ensure_parents(&norm);
        st.insert(norm, Node::File(content.into()));
    }

    /// Adds a folder and any missing parents.
    pub fn add_dir(&self, path: impl AsRef<str>) {
        let norm = normalize(path.as_ref());
        let mut st = self.lock();
        st.ensure_parents(&norm);
        if !st.entries.contains_key(&st.key(&norm)) {
            st.insert(norm, Node::Dir);
        }
    }

    /// Adds a symbolic link whose text is `target` (it may dangle).
    pub fn add_symlink(&self, link: impl AsRef<str>, target: impl Into<String>) {
        let norm = normalize(link.as_ref());
        let mut st = self.lock();
        st.ensure_parents(&norm);
        st.insert(norm, Node::Symlink(target.into()));
    }

    /// Sets the modification time of an existing entry (nanoseconds since the epoch).
    pub fn set_mtime_ns(&self, path: impl AsRef<str>, mtime_ns: Option<i128>) {
        let mut st = self.lock();
        if let Ok(key) = st.resolve(path.as_ref(), false)
            && let Some(e) = st.entries.get_mut(&key)
        {
            e.mtime_ns = mtime_ns;
        }
    }

    /// Makes reads and listings under `path` fail with a permission error.
    pub fn deny_read(&self, path: impl AsRef<str>) {
        let mut st = self.lock();
        let key = st.key(&normalize(path.as_ref()));
        st.denied.insert(key);
    }

    /// Makes every operation under `path` fail with a timeout (a dead network share).
    pub fn hang(&self, path: impl AsRef<str>) {
        let mut st = self.lock();
        let key = st.key(&normalize(path.as_ref()));
        st.hung.insert(key);
    }

    /// Declares the volume class of everything under `prefix` (the longest prefix wins).
    pub fn set_volume(&self, prefix: impl AsRef<str>, class: VolumeClass) {
        let mut st = self.lock();
        let key = st.key(&normalize(prefix.as_ref()));
        st.volumes.push((key, class));
    }

    /// Declares the class of volumes that match no prefix (default: unknown).
    pub fn set_default_volume(&self, class: VolumeClass) {
        self.lock().default_volume = Some(class);
    }

    /// True when the path exists without following a final link.
    pub fn contains(&self, path: impl AsRef<str>) -> bool {
        self.lock().resolve(path.as_ref(), false).is_ok()
    }

    /// The text of a file, `None` when missing, not a file or not UTF-8. No deadline, no faults.
    pub fn read_text(&self, path: impl AsRef<str>) -> Option<String> {
        let st = self.lock();
        let key = st.resolve(path.as_ref(), true).ok()?;
        match &st.entries.get(&key)?.node {
            Node::File(b) => String::from_utf8(b.clone()).ok(),
            _ => None,
        }
    }

    /// Every path in the tree, sorted, in the case it was created with.
    pub fn paths(&self) -> Vec<Utf8PathBuf> {
        let st = self.lock();
        let mut v: Vec<Utf8PathBuf> = st
            .entries
            .values()
            .map(|e| Utf8PathBuf::from(e.path.clone()))
            .collect();
        v.sort();
        v
    }

    // ---- write operations used by RecordingFs --------------------------------------------------

    pub(crate) fn write_file(&self, path: &str, content: &[u8]) -> PortResult<()> {
        let norm = normalize(path);
        let mut st = self.lock();
        let parent = parent_of(&norm).unwrap_or_default();
        if !parent.is_empty() {
            let pk = st.resolve(&parent, true)?;
            if !matches!(st.entries.get(&pk).map(|e| &e.node), Some(Node::Dir)) {
                return Err(PortError::new(PortErrorKind::Io, "parent is not a folder"));
            }
        }
        let target = match st.resolve(&norm, true) {
            Ok(k) => st.entries.get(&k).map(|e| e.path.clone()),
            Err(_) => None,
        };
        let existing_is_dir = target
            .as_ref()
            .and_then(|p| st.entries.get(&st.key(p)))
            .is_some_and(|e| matches!(e.node, Node::Dir));
        if existing_is_dir {
            return Err(PortError::new(PortErrorKind::Io, "is a folder"));
        }
        st.insert(target.unwrap_or(norm), Node::File(content.to_vec()));
        Ok(())
    }

    pub(crate) fn create_dir_all(&self, path: &str) -> PortResult<()> {
        let norm = normalize(path);
        let mut st = self.lock();
        st.ensure_parents(&norm);
        match st.entries.get(&st.key(&norm)).map(|e| &e.node) {
            Some(Node::Dir) => {}
            Some(_) => {
                return Err(PortError::new(
                    PortErrorKind::Io,
                    "exists and is not a folder",
                ));
            }
            None => st.insert(norm, Node::Dir),
        }
        Ok(())
    }

    pub(crate) fn remove_file(&self, path: &str) -> PortResult<()> {
        let mut st = self.lock();
        let key = st.resolve(path, false)?;
        if matches!(st.entries.get(&key).map(|e| &e.node), Some(Node::Dir)) {
            return Err(PortError::new(PortErrorKind::Io, "is a folder"));
        }
        st.entries.remove(&key);
        Ok(())
    }

    pub(crate) fn remove_dir_all(&self, path: &str) -> PortResult<()> {
        let mut st = self.lock();
        let key = st.resolve(path, false)?;
        let doomed: Vec<String> = st
            .entries
            .iter()
            .filter(|(k, _)| {
                **k == key || k.starts_with(&format!("{}/", key.trim_end_matches('/')))
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in doomed {
            st.entries.remove(&k);
        }
        Ok(())
    }

    pub(crate) fn rename(&self, from: &str, to: &str) -> PortResult<()> {
        let mut st = self.lock();
        let from_key = st.resolve(from, false)?;
        let to_norm = normalize(to);
        if let Some(parent) = parent_of(&to_norm)
            && !parent.is_empty()
        {
            st.resolve(&parent, true)?;
        }
        let prefix = format!("{}/", from_key.trim_end_matches('/'));
        let moving: Vec<(String, Entry)> = st
            .entries
            .iter()
            .filter(|(k, _)| **k == from_key || k.starts_with(&prefix))
            .map(|(k, e)| (k.clone(), e.clone()))
            .collect();
        let from_norm = moving
            .iter()
            .find(|(k, _)| *k == from_key)
            .map(|(_, e)| e.path.clone())
            .unwrap_or_default();
        for (k, _) in &moving {
            st.entries.remove(k);
        }
        let to_key = st.key(&to_norm);
        let to_prefix = format!("{}/", to_key.trim_end_matches('/'));
        let overwritten: Vec<String> = st
            .entries
            .keys()
            .filter(|k| **k == to_key || k.starts_with(&to_prefix))
            .cloned()
            .collect();
        for k in overwritten {
            st.entries.remove(&k);
        }
        for (_, mut e) in moving {
            let suffix = e.path.get(from_norm.len()..).unwrap_or_default().to_owned();
            e.path = format!("{to_norm}{suffix}");
            let k = st.key(&e.path);
            st.entries.insert(k, e);
        }
        Ok(())
    }

    pub(crate) fn create_symlink(&self, target: &str, link: &str) -> PortResult<()> {
        let norm = normalize(link);
        let mut st = self.lock();
        if st.resolve(&norm, false).is_ok() {
            return Err(PortError::new(PortErrorKind::Io, "link path exists"));
        }
        if let Some(parent) = parent_of(&norm)
            && !parent.is_empty()
        {
            st.resolve(&parent, true)?;
        }
        st.insert(norm, Node::Symlink(target.to_owned()));
        Ok(())
    }
}

fn kind_of(node: &Node) -> EntryKind {
    match node {
        Node::File(_) => EntryKind::File,
        Node::Dir => EntryKind::Dir,
        Node::Symlink(_) => EntryKind::Symlink,
    }
}

impl FsProbe for FakeFs {
    fn exists(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<bool> {
        let st = self.lock();
        st.check_hung(&normalize(path.as_str()))?;
        Ok(st.resolve(path.as_str(), true).is_ok())
    }

    fn is_dir(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<bool> {
        let st = self.lock();
        st.check_hung(&normalize(path.as_str()))?;
        Ok(st
            .resolve(path.as_str(), true)
            .ok()
            .and_then(|k| st.entries.get(&k))
            .is_some_and(|e| matches!(e.node, Node::Dir)))
    }

    fn metadata(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<FsMeta> {
        let st = self.lock();
        st.check_hung(&normalize(path.as_str()))?;
        let key = st.resolve(path.as_str(), false)?;
        let e = st
            .entries
            .get(&key)
            .ok_or_else(|| not_found(path.as_str()))?;
        let size = match &e.node {
            Node::File(b) => u64::try_from(b.len()).unwrap_or(u64::MAX),
            _ => 0,
        };
        Ok(FsMeta {
            kind: kind_of(&e.node),
            size,
            mtime_ns: e.mtime_ns,
            file_id: Some(e.id),
        })
    }

    fn read_dir(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<Vec<DirEntryInfo>> {
        let st = self.lock();
        let norm = normalize(path.as_str());
        st.check_hung(&norm)?;
        st.check_denied(&norm)?;
        let key = st.resolve(path.as_str(), true)?;
        let dir = st.entries.get(&key).ok_or_else(|| not_found(&norm))?;
        if !matches!(dir.node, Node::Dir) {
            return Err(PortError::new(PortErrorKind::Io, "not a folder"));
        }
        let dir_path = dir.path.clone();
        st.check_denied(&dir_path)?;
        let mut out: Vec<DirEntryInfo> = st
            .entries
            .values()
            .filter(|e| parent_of(&e.path).is_some_and(|p| st.key(&p) == st.key(&dir_path)))
            .filter_map(|e| {
                let name = e.path.rsplit('/').next()?.to_owned();
                (!name.is_empty()).then(|| DirEntryInfo {
                    name,
                    kind: kind_of(&e.node),
                })
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn read_to_string(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<String> {
        let st = self.lock();
        let norm = normalize(path.as_str());
        st.check_hung(&norm)?;
        st.check_denied(&norm)?;
        let key = st.resolve(path.as_str(), true)?;
        let e = st.entries.get(&key).ok_or_else(|| not_found(&norm))?;
        st.check_denied(&e.path)?;
        match &e.node {
            Node::File(b) => String::from_utf8(b.clone())
                .map_err(|_| PortError::new(PortErrorKind::Io, "file is not valid UTF-8")),
            _ => Err(PortError::new(PortErrorKind::Io, "not a file")),
        }
    }

    fn canonicalize(&self, path: &Utf8Path, _deadline: Duration) -> PortResult<Utf8PathBuf> {
        let st = self.lock();
        st.check_hung(&normalize(path.as_str()))?;
        let key = st.resolve(path.as_str(), true)?;
        let e = st
            .entries
            .get(&key)
            .ok_or_else(|| not_found(path.as_str()))?;
        Ok(Utf8PathBuf::from(e.path.clone()))
    }

    fn volume_class(&self, path: &Utf8Path) -> VolumeClass {
        let st = self.lock();
        let norm = normalize(path.as_str());
        st.volumes
            .iter()
            .filter(|(p, _)| st.is_under(&norm, p))
            .max_by_key(|(p, _)| p.len())
            .map(|(_, c)| *c)
            .or(st.default_volume)
            .unwrap_or(VolumeClass::Unknown)
    }
}

/// One write attempt seen by [`RecordingFs`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum WriteOp {
    /// A file was written with `len` bytes.
    WriteFile {
        /// The path written.
        path: Utf8PathBuf,
        /// The number of bytes.
        len: usize,
    },
    /// A folder (and its parents) was created.
    CreateDirAll {
        /// The folder path.
        path: Utf8PathBuf,
    },
    /// A file or link was removed.
    RemoveFile {
        /// The path removed.
        path: Utf8PathBuf,
    },
    /// A folder tree was removed.
    RemoveDirAll {
        /// The folder removed.
        path: Utf8PathBuf,
    },
    /// An entry was renamed.
    Rename {
        /// The old path.
        from: Utf8PathBuf,
        /// The new path.
        to: Utf8PathBuf,
    },
    /// A symbolic link was created.
    CreateSymlink {
        /// The link target text.
        target: Utf8PathBuf,
        /// The link path.
        link: Utf8PathBuf,
    },
}

impl WriteOp {
    /// The paths this operation changes (both ends of a rename, the link of a symlink).
    pub fn paths(&self) -> Vec<&Utf8Path> {
        match self {
            WriteOp::WriteFile { path, .. }
            | WriteOp::CreateDirAll { path }
            | WriteOp::RemoveFile { path }
            | WriteOp::RemoveDirAll { path } => vec![path.as_path()],
            WriteOp::Rename { from, to } => vec![from.as_path(), to.as_path()],
            WriteOp::CreateSymlink { link, .. } => vec![link.as_path()],
        }
    }
}

#[derive(Debug, Default)]
struct Faults {
    /// Number of operations still allowed before every later one fails.
    budget: Option<usize>,
    /// Folder prefixes under which writes fail, with the error kind.
    prefixes: Vec<(String, PortErrorKind)>,
}

/// A [`FakeFs`] with write operations that record every attempt, optionally failing on demand.
///
/// Use it to prove a use case writes only where it may (fence tests) and to inject crashes: after
/// [`RecordingFs::fail_after`] the next operations fail, so a test can crash at every step of a
/// sequence. Failed attempts are recorded too.
#[derive(Debug, Default)]
pub struct RecordingFs {
    fs: FakeFs,
    ops: Mutex<Vec<WriteOp>>,
    faults: Mutex<Faults>,
}

impl RecordingFs {
    /// An empty recording tree.
    pub fn new() -> Self {
        RecordingFs {
            fs: FakeFs::new(),
            ..RecordingFs::default()
        }
    }

    /// Wraps an existing tree.
    pub fn wrap(fs: FakeFs) -> Self {
        RecordingFs {
            fs,
            ..RecordingFs::default()
        }
    }

    /// The underlying tree, for set up and for reading results.
    pub fn fs(&self) -> &FakeFs {
        &self.fs
    }

    fn record(&self, op: WriteOp) -> PortResult<()> {
        let paths: Vec<String> = op.paths().iter().map(|p| p.as_str().to_owned()).collect();
        self.ops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(op);
        let mut f = self.faults.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(b) = f.budget.as_mut() {
            if *b == 0 {
                return Err(PortError::new(PortErrorKind::Io, "simulated crash"));
            }
            *b -= 1;
        }
        for path in &paths {
            let norm = normalize(path);
            for (prefix, kind) in &f.prefixes {
                if norm == *prefix
                    || norm.starts_with(&format!("{}/", prefix.trim_end_matches('/')))
                {
                    return Err(PortError::new(*kind, format!("simulated failure: {norm}")));
                }
            }
        }
        Ok(())
    }

    /// Lets `n` more operations succeed and fails every later one.
    pub fn fail_after(&self, n: usize) {
        self.faults
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .budget = Some(n);
    }

    /// Fails every write under `prefix` with `kind` (a read only folder is `PermissionDenied`).
    pub fn fail_writes_under(&self, prefix: impl AsRef<str>, kind: PortErrorKind) {
        self.faults
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .prefixes
            .push((normalize(prefix.as_ref()), kind));
    }

    /// Removes all injected failures.
    pub fn clear_faults(&self) {
        *self.faults.lock().unwrap_or_else(PoisonError::into_inner) = Faults::default();
    }

    /// Writes a file.
    pub fn write_file(&self, path: &Utf8Path, content: &[u8]) -> PortResult<()> {
        self.record(WriteOp::WriteFile {
            path: path.to_owned(),
            len: content.len(),
        })?;
        self.fs.write_file(path.as_str(), content)
    }

    /// Creates a folder and its parents.
    pub fn create_dir_all(&self, path: &Utf8Path) -> PortResult<()> {
        self.record(WriteOp::CreateDirAll {
            path: path.to_owned(),
        })?;
        self.fs.create_dir_all(path.as_str())
    }

    /// Removes a file or link.
    pub fn remove_file(&self, path: &Utf8Path) -> PortResult<()> {
        self.record(WriteOp::RemoveFile {
            path: path.to_owned(),
        })?;
        self.fs.remove_file(path.as_str())
    }

    /// Removes a folder tree.
    pub fn remove_dir_all(&self, path: &Utf8Path) -> PortResult<()> {
        self.record(WriteOp::RemoveDirAll {
            path: path.to_owned(),
        })?;
        self.fs.remove_dir_all(path.as_str())
    }

    /// Renames an entry, replacing the destination.
    pub fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> PortResult<()> {
        self.record(WriteOp::Rename {
            from: from.to_owned(),
            to: to.to_owned(),
        })?;
        self.fs.rename(from.as_str(), to.as_str())
    }

    /// Creates a symbolic link.
    pub fn create_symlink(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<()> {
        self.record(WriteOp::CreateSymlink {
            target: target.to_owned(),
            link: link.to_owned(),
        })?;
        self.fs.create_symlink(target.as_str(), link.as_str())
    }

    /// Every attempt so far, in order.
    pub fn ops(&self) -> Vec<WriteOp> {
        self.ops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Forgets the recorded operations.
    pub fn clear_ops(&self) {
        self.ops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// Every path touched by a recorded operation, sorted and without duplicates.
    pub fn written_paths(&self) -> Vec<Utf8PathBuf> {
        let mut v: Vec<Utf8PathBuf> = self
            .ops()
            .iter()
            .flat_map(|o| {
                o.paths()
                    .into_iter()
                    .map(Utf8Path::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// The touched paths that are not inside any of the `allowed` prefixes: the fence violations.
    pub fn writes_outside(&self, allowed: &[&Utf8Path]) -> Vec<Utf8PathBuf> {
        let allowed: Vec<String> = allowed.iter().map(|p| normalize(p.as_str())).collect();
        self.written_paths()
            .into_iter()
            .filter(|p| {
                let n = normalize(p.as_str());
                !allowed
                    .iter()
                    .any(|a| n == *a || n.starts_with(&format!("{}/", a.trim_end_matches('/'))))
            })
            .collect()
    }
}

impl FsProbe for RecordingFs {
    fn exists(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool> {
        self.fs.exists(path, deadline)
    }
    fn is_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool> {
        self.fs.is_dir(path, deadline)
    }
    fn metadata(&self, path: &Utf8Path, deadline: Duration) -> PortResult<FsMeta> {
        self.fs.metadata(path, deadline)
    }
    fn read_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Vec<DirEntryInfo>> {
        self.fs.read_dir(path, deadline)
    }
    fn read_to_string(&self, path: &Utf8Path, deadline: Duration) -> PortResult<String> {
        self.fs.read_to_string(path, deadline)
    }
    fn canonicalize(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Utf8PathBuf> {
        self.fs.canonicalize(path, deadline)
    }
    fn volume_class(&self, path: &Utf8Path) -> VolumeClass {
        self.fs.volume_class(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: Duration = Duration::from_millis(10);

    type Step = dyn Fn(&RecordingFs) -> PortResult<()>;

    fn p(s: &str) -> &Utf8Path {
        Utf8Path::new(s)
    }

    #[test]
    fn files_folders_and_listings_work() {
        let fs = FakeFs::new();
        fs.add_file("/a/b/one.txt", "1");
        fs.add_file("/a/b/two.txt", "22");
        fs.add_dir("/a/c");
        assert_eq!(fs.exists(p("/a/b/one.txt"), D), Ok(true));
        assert_eq!(fs.is_dir(p("/a/b"), D), Ok(true));
        assert_eq!(fs.is_dir(p("/a/b/one.txt"), D), Ok(false));
        assert_eq!(fs.exists(p("/a/zzz"), D), Ok(false));
        let names: Vec<String> = fs
            .read_dir(p("/a"), D)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["b", "c"]);
        assert_eq!(fs.read_to_string(p("/a/b/two.txt"), D).unwrap(), "22");
        let meta = fs.metadata(p("/a/b/two.txt"), D).unwrap();
        assert_eq!(meta.kind, EntryKind::File);
        assert_eq!(meta.size, 2);
        assert!(meta.mtime_ns.is_some() && meta.file_id.is_some());
    }

    #[test]
    fn missing_paths_report_not_found() {
        let fs = FakeFs::new();
        assert_eq!(
            fs.metadata(p("/nope"), D).unwrap_err().kind,
            PortErrorKind::NotFound
        );
        assert_eq!(
            fs.read_dir(p("/nope"), D).unwrap_err().kind,
            PortErrorKind::NotFound
        );
        assert_eq!(
            fs.read_to_string(p("/nope"), D).unwrap_err().kind,
            PortErrorKind::NotFound
        );
    }

    #[test]
    fn symlinks_are_followed_but_metadata_does_not_follow_them() {
        let fs = FakeFs::new();
        fs.add_file("/real/f.txt", "x");
        fs.add_symlink("/link", "/real");
        assert_eq!(fs.read_to_string(p("/link/f.txt"), D).unwrap(), "x");
        assert_eq!(fs.metadata(p("/link"), D).unwrap().kind, EntryKind::Symlink);
        assert_eq!(fs.canonicalize(p("/link/f.txt"), D).unwrap(), "/real/f.txt");
        assert_eq!(fs.is_dir(p("/link"), D), Ok(true));
    }

    #[test]
    fn relative_links_resolve_against_their_folder() {
        let fs = FakeFs::new();
        fs.add_file("/x/real/f.txt", "r");
        fs.add_symlink("/x/alias", "real");
        assert_eq!(fs.read_to_string(p("/x/alias/f.txt"), D).unwrap(), "r");
    }

    #[test]
    fn dangling_and_looping_links_fail_cleanly() {
        let fs = FakeFs::new();
        fs.add_symlink("/dangling", "/missing");
        fs.add_symlink("/loop", "/loop");
        fs.add_symlink("/a", "/b");
        fs.add_symlink("/b", "/a");
        assert_eq!(fs.exists(p("/dangling"), D), Ok(false));
        assert_eq!(
            fs.metadata(p("/dangling"), D).unwrap().kind,
            EntryKind::Symlink
        );
        assert_eq!(fs.exists(p("/loop"), D), Ok(false));
        assert_eq!(
            fs.canonicalize(p("/a"), D).unwrap_err().kind,
            PortErrorKind::Io
        );
        assert_eq!(fs.metadata(p("/loop"), D).unwrap().kind, EntryKind::Symlink);
    }

    #[test]
    fn case_insensitive_trees_keep_the_original_spelling() {
        let fs = FakeFs::case_insensitive();
        fs.add_file("C:\\Steam\\Config\\File.TXT", "t");
        assert_eq!(fs.exists(p("c:/steam/config/file.txt"), D), Ok(true));
        assert_eq!(
            fs.canonicalize(p("C:/STEAM/CONFIG/FILE.txt"), D).unwrap(),
            "C:/Steam/Config/File.TXT"
        );
        let names: Vec<String> = fs
            .read_dir(p("c:\\steam"), D)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["Config"]);
    }

    #[test]
    fn denied_and_hung_paths_fail_as_configured() {
        let fs = FakeFs::new();
        fs.add_file("/locked/f.txt", "x");
        fs.add_file("/slow/f.txt", "x");
        fs.deny_read("/locked");
        fs.hang("/slow");
        assert_eq!(
            fs.read_to_string(p("/locked/f.txt"), D).unwrap_err().kind,
            PortErrorKind::PermissionDenied
        );
        assert_eq!(
            fs.read_dir(p("/locked"), D).unwrap_err().kind,
            PortErrorKind::PermissionDenied
        );
        assert_eq!(fs.exists(p("/locked/f.txt"), D), Ok(true));
        assert_eq!(
            fs.exists(p("/slow/f.txt"), D).unwrap_err().kind,
            PortErrorKind::TimedOut
        );
    }

    #[test]
    fn volume_classes_use_the_longest_prefix() {
        let fs = FakeFs::new();
        fs.set_default_volume(VolumeClass::Posix);
        fs.set_volume("/mnt", VolumeClass::Fat);
        fs.set_volume("/mnt/ntfs", VolumeClass::Ntfs);
        assert_eq!(fs.volume_class(p("/home/x")), VolumeClass::Posix);
        assert_eq!(fs.volume_class(p("/mnt/a")), VolumeClass::Fat);
        assert_eq!(fs.volume_class(p("/mnt/ntfs/a")), VolumeClass::Ntfs);
        assert_eq!(FakeFs::new().volume_class(p("/x")), VolumeClass::Unknown);
    }

    #[test]
    fn later_writes_have_later_modification_times() {
        let fs = FakeFs::new();
        fs.add_file("/a", "1");
        fs.add_file("/b", "2");
        let a = fs.metadata(p("/a"), D).unwrap().mtime_ns.unwrap();
        let b = fs.metadata(p("/b"), D).unwrap().mtime_ns.unwrap();
        assert!(b > a);
        fs.set_mtime_ns("/a", Some(5));
        assert_eq!(fs.metadata(p("/a"), D).unwrap().mtime_ns, Some(5));
    }

    #[test]
    fn recording_fs_records_writes_and_applies_them() {
        let r = RecordingFs::new();
        r.create_dir_all(p("/g/Mods")).unwrap();
        r.write_file(p("/g/Mods/a.txt"), b"abc").unwrap();
        r.rename(p("/g/Mods/a.txt"), p("/g/Mods/b.txt")).unwrap();
        r.create_symlink(p("/src"), p("/g/Mods/link")).unwrap();
        r.remove_file(p("/g/Mods/link")).unwrap();
        r.remove_dir_all(p("/g/Mods")).unwrap();
        assert_eq!(r.ops().len(), 6);
        assert_eq!(
            r.ops()[1],
            WriteOp::WriteFile {
                path: "/g/Mods/a.txt".into(),
                len: 3
            }
        );
        assert!(!r.fs().contains("/g/Mods"));
        assert!(r.fs().contains("/g"));
    }

    #[test]
    fn rename_moves_whole_folders() {
        let r = RecordingFs::new();
        r.fs().add_file("/d/old/x/f.txt", "k");
        r.rename(p("/d/old"), p("/d/new")).unwrap();
        assert_eq!(
            r.read_to_string(p("/d/new/x/f.txt"), Duration::ZERO)
                .unwrap(),
            "k"
        );
        assert!(!r.fs().contains("/d/old"));
    }

    #[test]
    fn writes_outside_the_allowed_prefixes_are_reported() {
        let r = RecordingFs::new();
        r.create_dir_all(p("/game/Mods/own")).unwrap();
        r.write_file(p("/game/Mods/own/x"), b"1").unwrap();
        r.create_dir_all(p("/game/Data")).unwrap();
        let bad = r.writes_outside(&[p("/game/Mods")]);
        assert_eq!(bad, vec![Utf8PathBuf::from("/game/Data")]);
        assert!(r.writes_outside(&[p("/game")]).is_empty());
        assert_eq!(r.written_paths().len(), 3);
    }

    #[test]
    fn failed_attempts_are_recorded_too() {
        let r = RecordingFs::new();
        r.create_dir_all(p("/ro")).unwrap();
        r.fail_writes_under("/ro", PortErrorKind::PermissionDenied);
        let e = r.write_file(p("/ro/x"), b"1").unwrap_err();
        assert_eq!(e.kind, PortErrorKind::PermissionDenied);
        assert_eq!(r.ops().len(), 2);
        assert!(!r.fs().contains("/ro/x"));
        r.clear_faults();
        r.write_file(p("/ro/x"), b"1").unwrap();
        r.clear_ops();
        assert!(r.ops().is_empty());
    }

    #[test]
    fn fail_after_simulates_a_crash_at_every_step() {
        for allowed in 0..3 {
            let r = RecordingFs::new();
            r.fail_after(allowed);
            let steps: [&Step; 3] = [
                &|r| r.create_dir_all(p("/t")),
                &|r| r.write_file(p("/t/a"), b"1"),
                &|r| r.rename(p("/t/a"), p("/t/b")),
            ];
            let done = steps.iter().take_while(|s| s(&r).is_ok()).count();
            assert_eq!(done, allowed);
        }
    }

    #[test]
    fn writing_below_a_missing_folder_fails_like_the_real_thing() {
        let r = RecordingFs::new();
        let e = r.write_file(p("/missing/x"), b"1").unwrap_err();
        assert_eq!(e.kind, PortErrorKind::NotFound);
        assert_eq!(
            r.create_symlink(p("/t"), p("/missing/l")).unwrap_err().kind,
            PortErrorKind::NotFound
        );
    }

    #[test]
    fn write_ops_serialise_with_a_tag() {
        let op = WriteOp::Rename {
            from: "/a".into(),
            to: "/b".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, r#"{"op":"rename","from":"/a","to":"/b"}"#);
    }
}
