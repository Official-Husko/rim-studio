//! Stage S1: directory walkers. Every walker returns the same thing: one record per non-directory
//! entry with (path, size, mtime, type), plus the number of directories and errors.

use crate::util::mtime_ns;
use rayon::ThreadPool;
use rayon::prelude::*;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use xxhash_rust::xxh3::xxh3_64;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ns: i64,
    /// 0 = regular file, 1 = symlink (not followed), 2 = other
    pub kind: u8,
}

#[derive(Default)]
pub struct WalkOut {
    /// Chunks (one per directory for the parallel walkers, a single chunk for the serial ones).
    pub chunks: Vec<Vec<Entry>>,
    pub dirs: u64,
    pub errors: u64,
}

impl WalkOut {
    pub fn files(&self) -> u64 {
        self.chunks.iter().map(|c| c.len() as u64).sum()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.chunks.iter().flatten()
    }
    pub fn flatten(self) -> Vec<Entry> {
        let mut v = Vec::with_capacity(self.chunks.iter().map(|c| c.len()).sum());
        for c in self.chunks {
            v.extend(c);
        }
        v
    }
}

/// Order independent digest of (path, size, mtime): lets us prove that all walkers saw the same tree.
pub fn digest(out: &WalkOut) -> (u64, u64, u64) {
    let (mut n, mut bytes, mut h) = (0u64, 0u64, 0u64);
    for e in out.iter() {
        n += 1;
        bytes += e.size;
        h = h.wrapping_add(xxh3_64(e.path.as_os_str().as_bytes()) ^ e.size.rotate_left(7) ^ (e.mtime_ns as u64).rotate_left(23));
    }
    (n, bytes, h)
}

fn kind_of(ft: &std::fs::FileType) -> u8 {
    if ft.is_file() {
        0
    } else if ft.is_symlink() {
        1
    } else {
        2
    }
}

// ---------------------------------------------------------------------------------------------
// 1. std::fs, serial, explicit stack
// ---------------------------------------------------------------------------------------------

pub fn walk_std(root: &Path) -> WalkOut {
    let mut out = WalkOut::default();
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                out.errors += 1;
                continue;
            }
        };
        out.dirs += 1;
        for e in rd {
            let Ok(e) = e else {
                out.errors += 1;
                continue;
            };
            let Ok(ft) = e.file_type() else {
                out.errors += 1;
                continue;
            };
            if ft.is_dir() {
                stack.push(e.path());
                continue;
            }
            match e.metadata() {
                Ok(md) => files.push(Entry { path: e.path(), size: md.len(), mtime_ns: mtime_ns(&md), kind: kind_of(&ft) }),
                Err(_) => out.errors += 1,
            }
        }
    }
    out.chunks.push(files);
    out
}

// ---------------------------------------------------------------------------------------------
// 2. walkdir (serial)
// ---------------------------------------------------------------------------------------------

pub fn walk_walkdir(root: &Path) -> WalkOut {
    let mut out = WalkOut::default();
    let mut files = Vec::new();
    for e in walkdir::WalkDir::new(root).follow_links(false) {
        match e {
            Ok(e) => {
                let ft = e.file_type();
                if ft.is_dir() {
                    out.dirs += 1;
                } else {
                    match e.metadata() {
                        Ok(md) => files.push(Entry { path: e.into_path(), size: md.len(), mtime_ns: mtime_ns(&md), kind: kind_of(&ft) }),
                        Err(_) => out.errors += 1,
                    }
                }
            }
            Err(_) => out.errors += 1,
        }
    }
    out.chunks.push(files);
    out
}

// ---------------------------------------------------------------------------------------------
// 3. jwalk (parallel). `tuned` does the stat inside process_read_dir (on the worker threads).
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct JState;
impl jwalk::ClientState for JState {
    type ReadDirState = ();
    type DirEntryState = Option<(u64, i64)>;
}

pub fn walk_jwalk(root: &Path, pool: Arc<ThreadPool>, tuned: bool) -> WalkOut {
    let mut w = jwalk::WalkDirGeneric::<JState>::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .sort(false)
        .parallelism(jwalk::Parallelism::RayonExistingPool { pool, busy_timeout: None });
    if tuned {
        w = w.process_read_dir(|_depth, _path, _state, children| {
            for c in children.iter_mut().flatten() {
                if !c.file_type.is_dir() {
                    if let Ok(md) = c.metadata() {
                        c.client_state = Some((md.len(), mtime_ns(&md)));
                    }
                }
            }
        });
    }
    let mut out = WalkOut::default();
    let mut files = Vec::new();
    for e in w {
        match e {
            Ok(e) => {
                if e.file_type.is_dir() {
                    out.dirs += 1;
                } else {
                    let kind = kind_of(&e.file_type);
                    let st = if tuned {
                        e.client_state
                    } else {
                        e.metadata().ok().map(|md| (md.len(), mtime_ns(&md)))
                    };
                    match st {
                        Some((size, mt)) => files.push(Entry { path: e.path(), size, mtime_ns: mt, kind }),
                        None => out.errors += 1,
                    }
                }
            }
            Err(_) => out.errors += 1,
        }
    }
    out.chunks.push(files);
    out
}

// ---------------------------------------------------------------------------------------------
// 4. ignore (parallel), with every filter disabled
// ---------------------------------------------------------------------------------------------

type IgnoreSink = Arc<Mutex<Vec<(Vec<Entry>, u64, u64)>>>;

struct IgnoreCollector {
    local: Vec<Entry>,
    dirs: u64,
    errors: u64,
    sink: IgnoreSink,
}

impl ignore::ParallelVisitor for IgnoreCollector {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        match entry {
            Ok(e) => match e.file_type() {
                Some(ft) if ft.is_dir() => self.dirs += 1,
                ft => {
                    let kind = ft.as_ref().map(kind_of).unwrap_or(2);
                    match e.metadata() {
                        Ok(md) => self.local.push(Entry { path: e.into_path(), size: md.len(), mtime_ns: mtime_ns(&md), kind }),
                        Err(_) => self.errors += 1,
                    }
                }
            },
            Err(_) => self.errors += 1,
        }
        ignore::WalkState::Continue
    }
}

impl Drop for IgnoreCollector {
    fn drop(&mut self) {
        self.sink.lock().unwrap().push((std::mem::take(&mut self.local), self.dirs, self.errors));
    }
}

struct IgnoreBuilder {
    sink: IgnoreSink,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for IgnoreBuilder {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(IgnoreCollector { local: Vec::new(), dirs: 0, errors: 0, sink: self.sink.clone() })
    }
}

pub fn walk_ignore(root: &Path, threads: usize) -> WalkOut {
    let sink: IgnoreSink = Arc::new(Mutex::new(Vec::new()));
    let walker = ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_global(false)
        .git_ignore(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .threads(threads)
        .build_parallel();
    let mut b = IgnoreBuilder { sink: sink.clone() };
    walker.visit(&mut b);
    drop(b);
    let mut out = WalkOut::default();
    for (v, d, e) in std::mem::take(&mut *sink.lock().unwrap()) {
        out.dirs += d;
        out.errors += e;
        out.chunks.push(v);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// 5. hand-rolled rayon recursion: read_dir + DirEntry::file_type (d_type, no stat) + one statx per file
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct RayonSink {
    chunks: Mutex<Vec<Vec<Entry>>>,
    dirs: AtomicU64,
    errors: AtomicU64,
}

fn rayon_dir(dir: PathBuf, sink: &RayonSink) {
    let rd = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => {
            sink.errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    sink.dirs.fetch_add(1, Ordering::Relaxed);
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<Entry> = Vec::new();
    let mut errors = 0u64;
    for e in rd {
        let Ok(e) = e else {
            errors += 1;
            continue;
        };
        let Ok(ft) = e.file_type() else {
            errors += 1;
            continue;
        };
        if ft.is_dir() {
            subdirs.push(e.path());
        } else {
            match e.metadata() {
                Ok(md) => files.push(Entry { path: e.path(), size: md.len(), mtime_ns: mtime_ns(&md), kind: kind_of(&ft) }),
                Err(_) => errors += 1,
            }
        }
    }
    if errors > 0 {
        sink.errors.fetch_add(errors, Ordering::Relaxed);
    }
    if !files.is_empty() {
        sink.chunks.lock().unwrap().push(files);
    }
    subdirs.into_par_iter().for_each(|d| rayon_dir(d, sink));
}

pub fn walk_rayon(root: &Path, pool: &ThreadPool) -> WalkOut {
    let sink = RayonSink::default();
    pool.install(|| rayon_dir(root.to_path_buf(), &sink));
    WalkOut { chunks: sink.chunks.into_inner().unwrap(), dirs: sink.dirs.into_inner(), errors: sink.errors.into_inner() }
}

// ---------------------------------------------------------------------------------------------
// 6. hand-rolled, names only (no stat at all): the lower bound of any directory walk
// ---------------------------------------------------------------------------------------------

pub fn walk_rayon_names_only(root: &Path, pool: &ThreadPool) -> (u64, u64) {
    fn go(dir: PathBuf, files: &AtomicU64, dirs: &AtomicU64) {
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        dirs.fetch_add(1, Ordering::Relaxed);
        let mut sub = Vec::new();
        let mut n = 0u64;
        for e in rd.flatten() {
            match e.file_type() {
                Ok(ft) if ft.is_dir() => sub.push(e.path()),
                _ => n += 1,
            }
        }
        files.fetch_add(n, Ordering::Relaxed);
        sub.into_par_iter().for_each(|d| go(d, files, dirs));
    }
    let (f, d) = (AtomicU64::new(0), AtomicU64::new(0));
    pool.install(|| go(root.to_path_buf(), &f, &d));
    (f.into_inner(), d.into_inner())
}

pub fn make_pool(n: usize) -> Arc<ThreadPool> {
    Arc::new(rayon::ThreadPoolBuilder::new().num_threads(n).thread_name(move |i| format!("spike-{i}")).build().unwrap())
}
