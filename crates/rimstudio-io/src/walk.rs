//! Directory walking: mod roots and a bounded parallel recursive scan.
//!
//! [`list_mod_roots`] lists the immediate sub folders of a mods folder (links to folders count and
//! are flagged). [`scan_parallel`] walks one root with a purpose built `rayon` recursion over
//! `read_dir` and `DirEntry::file_type`: at most eight workers, symbolic links are reported but
//! never followed (so link loops cannot hang the walk), a directory that cannot be read is counted
//! and sampled instead of failing the scan, names that are not UTF-8 are counted and skipped, and
//! the result is sorted by relative path so equal trees give equal output regardless of timing.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::ports::EntryKind;

use crate::error::StoreError;
use crate::statkey::{FileIdFn, system_time_ns};

/// The most workers a scan may use.
pub const MAX_WORKERS: usize = 8;

/// One candidate mod folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRoot {
    /// The folder name.
    pub name: String,
    /// The folder path (the link itself when it is a link).
    pub path: Utf8PathBuf,
    /// True when the entry is a symbolic link to a folder.
    pub is_link: bool,
}

/// The result of [`list_mod_roots`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModRootListing {
    /// The folders sorted by name.
    pub roots: Vec<ModRoot>,
    /// Entries skipped because their name is not UTF-8.
    pub non_utf8: u64,
}

/// Lists the immediate sub folders of `dir` (links to folders included and flagged), sorted by
/// name. Files are ignored. An unreadable `dir` is an error.
pub fn list_mod_roots(dir: &Utf8Path) -> Result<ModRootListing, StoreError> {
    let rd = fs_err::read_dir(dir).map_err(|e| StoreError::io("read-dir", dir, e))?;
    let mut out = ModRootListing::default();
    for entry in rd.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            out.non_utf8 += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else { continue };
        let path = dir.join(&name);
        let is_link = ft.is_symlink();
        let is_dir = if is_link {
            fs_err::metadata(&path).map(|m| m.is_dir()).unwrap_or(false)
        } else {
            ft.is_dir()
        };
        if is_dir {
            out.roots.push(ModRoot {
                name,
                path,
                is_link,
            });
        }
    }
    out.roots.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// A predicate over `(relative path, kind)`; `true` skips the entry (and a skipped folder is not
/// entered).
pub type PruneFn = dyn Fn(&str, EntryKind) -> bool + Send + Sync;

/// Options of [`scan_parallel`].
#[derive(Clone)]
pub struct ScanOptions {
    /// Worker threads, clamped to `1..=8`.
    pub workers: usize,
    /// The deepest level to enter (the root's children are depth 1); `None` is unlimited.
    pub max_depth: Option<usize>,
    /// Also read size, mtime and file id of every entry (one `lstat` each).
    pub stat: bool,
    /// The file id hook used with `stat`.
    pub file_id: Option<FileIdFn>,
    /// Entries to skip.
    pub prune: Option<Arc<PruneFn>>,
    /// How many distinct error samples to keep.
    pub error_samples: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            workers: MAX_WORKERS,
            max_depth: None,
            stat: false,
            file_id: None,
            prune: None,
            error_samples: 16,
        }
    }
}

impl std::fmt::Debug for ScanOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanOptions")
            .field("workers", &self.workers)
            .field("max_depth", &self.max_depth)
            .field("stat", &self.stat)
            .finish_non_exhaustive()
    }
}

/// One entry found by a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// The path relative to the root with `/` separators.
    pub rel_path: String,
    /// The kind (links are [`EntryKind::Symlink`] and not followed).
    pub kind: EntryKind,
    /// Size in bytes (0 unless `stat` was requested).
    pub size: u64,
    /// Modification time in ns since the epoch (`stat` only).
    pub mtime_ns: Option<i128>,
    /// File identity (`stat` with a hook only).
    pub file_id: Option<u128>,
}

impl ScanEntry {
    /// The stat key of this entry (meaningful when the scan used `stat`).
    pub fn stat_key(&self) -> crate::statkey::StatKey {
        crate::statkey::StatKey {
            rel_path: self.rel_path.clone(),
            size: self.size,
            mtime_ns: self.mtime_ns,
            file_id: self.file_id,
        }
    }
}

/// A sample of a directory that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanError {
    /// The folder, relative to the root (empty for the root itself).
    pub rel_path: String,
    /// The error text.
    pub message: String,
}

/// The result of [`scan_parallel`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanOutput {
    /// Every entry below the root, sorted by relative path.
    pub entries: Vec<ScanEntry>,
    /// Directories read successfully (the root included).
    pub dirs_read: u64,
    /// Directories that could not be read.
    pub dir_errors: u64,
    /// Entries skipped because their name is not UTF-8.
    pub non_utf8: u64,
    /// The first few directory errors.
    pub error_samples: Vec<ScanError>,
}

impl ScanOutput {
    fn merge(&mut self, mut other: ScanOutput, cap: usize) {
        self.entries.append(&mut other.entries);
        self.dirs_read += other.dirs_read;
        self.dir_errors += other.dir_errors;
        self.non_utf8 += other.non_utf8;
        for s in other.error_samples {
            if self.error_samples.len() < cap {
                self.error_samples.push(s);
            }
        }
    }
}

fn kind_of(ft: std::fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn walk_dir(abs: &Utf8Path, rel: &str, depth: usize, opts: &ScanOptions) -> ScanOutput {
    let mut out = ScanOutput::default();
    let rd = match fs_err::read_dir(abs) {
        Ok(rd) => rd,
        Err(e) => {
            out.dir_errors = 1;
            if opts.error_samples > 0 {
                out.error_samples.push(ScanError {
                    rel_path: rel.to_owned(),
                    message: e.to_string(),
                });
            }
            return out;
        }
    };
    out.dirs_read = 1;
    let mut subdirs: Vec<(Utf8PathBuf, String)> = Vec::new();
    for entry in rd {
        let Ok(entry) = entry else {
            out.dir_errors += 1;
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            out.non_utf8 += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else {
            out.dir_errors += 1;
            continue;
        };
        let kind = kind_of(ft);
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if opts.prune.as_ref().is_some_and(|p| p(&child_rel, kind)) {
            continue;
        }
        let mut item = ScanEntry {
            rel_path: child_rel.clone(),
            kind,
            size: 0,
            mtime_ns: None,
            file_id: None,
        };
        if opts.stat
            && let Ok(meta) = entry.metadata()
        {
            item.size = if kind == EntryKind::Dir {
                0
            } else {
                meta.len()
            };
            item.mtime_ns = meta.modified().ok().map(system_time_ns);
            item.file_id = opts.file_id.and_then(|f| f(&meta));
        }
        out.entries.push(item);
        let may_enter = opts.max_depth.is_none_or(|m| depth < m);
        if kind == EntryKind::Dir && may_enter {
            subdirs.push((abs.join(&name), child_rel));
        }
    }
    let parts: Vec<ScanOutput> = subdirs
        .par_iter()
        .map(|(p, r)| walk_dir(p, r, depth + 1, opts))
        .collect();
    for part in parts {
        out.merge(part, opts.error_samples);
    }
    out
}

/// Scans `root` recursively with at most eight workers. See the module docs for the rules. An
/// unreadable root is an error; unreadable sub folders are counted in the output.
pub fn scan_parallel(root: &Utf8Path, opts: &ScanOptions) -> Result<ScanOutput, StoreError> {
    let meta = fs_err::metadata(root).map_err(|e| StoreError::io("scan-root", root, e))?;
    if !meta.is_dir() {
        return Err(StoreError::io(
            "scan-root",
            root,
            std::io::Error::other("not a directory"),
        ));
    }
    let workers = opts.workers.clamp(1, MAX_WORKERS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .map_err(|e| StoreError::io("scan-pool", root, std::io::Error::other(e.to_string())))?;
    let mut out = pool.install(|| walk_dir(root, "", 1, opts));
    if out.dirs_read == 0
        && let Some(sample) = out.error_samples.first()
    {
        return Err(StoreError::io(
            "scan-root",
            root,
            std::io::Error::other(sample.message.clone()),
        ));
    }
    out.entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        for d in [
            "RS_ModA/About",
            "RS_ModA/Defs/Things",
            "RS_ModB/About",
            "RS_ModB/.git/objects",
            "empty",
        ] {
            fs_err::create_dir_all(root.join(d)).unwrap();
        }
        for (f, body) in [
            ("RS_ModA/About/About.xml", "a"),
            ("RS_ModA/Defs/Things/x.xml", "bb"),
            ("RS_ModB/About/About.xml", "ccc"),
            ("RS_ModB/.git/objects/o", "dddd"),
            ("notes.txt", "e"),
        ] {
            fs_err::write(root.join(f), body).unwrap();
        }
        (t, root)
    }

    fn rels(o: &ScanOutput) -> Vec<&str> {
        o.entries.iter().map(|e| e.rel_path.as_str()).collect()
    }

    #[test]
    fn scan_lists_everything_sorted() {
        let (_t, root) = tree();
        let out = scan_parallel(&root, &ScanOptions::default()).unwrap();
        assert_eq!(out.dir_errors, 0);
        assert_eq!(out.dirs_read, 10);
        assert_eq!(
            rels(&out),
            [
                "RS_ModA",
                "RS_ModA/About",
                "RS_ModA/About/About.xml",
                "RS_ModA/Defs",
                "RS_ModA/Defs/Things",
                "RS_ModA/Defs/Things/x.xml",
                "RS_ModB",
                "RS_ModB/.git",
                "RS_ModB/.git/objects",
                "RS_ModB/.git/objects/o",
                "RS_ModB/About",
                "RS_ModB/About/About.xml",
                "empty",
                "notes.txt"
            ]
        );
    }

    #[test]
    fn output_is_identical_for_any_worker_count() {
        let (_t, root) = tree();
        let one = scan_parallel(
            &root,
            &ScanOptions {
                workers: 1,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        let many = scan_parallel(
            &root,
            &ScanOptions {
                workers: 64,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert_eq!(one, many);
    }

    #[test]
    fn prune_skips_entries_and_does_not_enter_folders() {
        let (_t, root) = tree();
        let opts = ScanOptions {
            prune: Some(Arc::new(|rel: &str, _| {
                rel.ends_with(".git") || rel == "notes.txt"
            })),
            ..ScanOptions::default()
        };
        let out = scan_parallel(&root, &opts).unwrap();
        assert!(
            !rels(&out)
                .iter()
                .any(|r| r.contains(".git") || *r == "notes.txt")
        );
        assert!(rels(&out).contains(&"RS_ModB/About/About.xml"));
    }

    #[test]
    fn depth_limit_stops_entering_folders() {
        let (_t, root) = tree();
        let out = scan_parallel(
            &root,
            &ScanOptions {
                max_depth: Some(1),
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert_eq!(rels(&out), ["RS_ModA", "RS_ModB", "empty", "notes.txt"]);
        let out = scan_parallel(
            &root,
            &ScanOptions {
                max_depth: Some(2),
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert!(rels(&out).contains(&"RS_ModA/About"));
        assert!(!rels(&out).contains(&"RS_ModA/About/About.xml"));
    }

    #[test]
    fn stat_fills_size_and_mtime_for_files() {
        let (_t, root) = tree();
        let out = scan_parallel(
            &root,
            &ScanOptions {
                stat: true,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        let x = out
            .entries
            .iter()
            .find(|e| e.rel_path == "RS_ModA/Defs/Things/x.xml")
            .unwrap();
        assert_eq!(x.size, 2);
        assert!(x.mtime_ns.is_some());
        assert_eq!(x.stat_key().rel_path, "RS_ModA/Defs/Things/x.xml");
        let d = out
            .entries
            .iter()
            .find(|e| e.rel_path == "RS_ModA")
            .unwrap();
        assert_eq!(d.size, 0);
        let plain = scan_parallel(&root, &ScanOptions::default()).unwrap();
        assert!(
            plain
                .entries
                .iter()
                .all(|e| e.mtime_ns.is_none() && e.size == 0)
        );
    }

    #[test]
    fn missing_root_or_file_root_is_an_error() {
        let (_t, root) = tree();
        assert!(scan_parallel(&root.join("nope"), &ScanOptions::default()).is_err());
        assert!(scan_parallel(&root.join("notes.txt"), &ScanOptions::default()).is_err());
    }

    #[test]
    fn mod_roots_are_the_sorted_sub_folders() {
        let (_t, root) = tree();
        let l = list_mod_roots(&root).unwrap();
        let names: Vec<&str> = l.roots.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["RS_ModA", "RS_ModB", "empty"]);
        assert!(l.roots.iter().all(|r| !r.is_link));
        assert!(list_mod_roots(&root.join("nope")).is_err());
    }

    #[cfg(unix)]
    mod links {
        use super::*;

        #[test]
        fn links_are_reported_but_never_followed() {
            let (_t, root) = tree();
            // A loop back to the root and a link to a sibling folder.
            std::os::unix::fs::symlink(&root, root.join("RS_ModA/loop")).unwrap();
            std::os::unix::fs::symlink(root.join("RS_ModB"), root.join("linked")).unwrap();
            let out = scan_parallel(&root, &ScanOptions::default()).unwrap();
            let loop_entry = out
                .entries
                .iter()
                .find(|e| e.rel_path == "RS_ModA/loop")
                .unwrap();
            assert_eq!(loop_entry.kind, EntryKind::Symlink);
            let linked = out.entries.iter().find(|e| e.rel_path == "linked").unwrap();
            assert_eq!(linked.kind, EntryKind::Symlink);
            assert!(
                !rels(&out)
                    .iter()
                    .any(|r| r.starts_with("linked/") || r.starts_with("RS_ModA/loop/"))
            );
        }

        #[test]
        fn mod_roots_flag_links_to_folders_and_skip_dangling_ones() {
            let (_t, root) = tree();
            std::os::unix::fs::symlink(root.join("RS_ModB"), root.join("linked")).unwrap();
            std::os::unix::fs::symlink(root.join("gone"), root.join("dangling")).unwrap();
            let l = list_mod_roots(&root).unwrap();
            let linked = l.roots.iter().find(|r| r.name == "linked").unwrap();
            assert!(linked.is_link);
            assert!(!l.roots.iter().any(|r| r.name == "dangling"));
        }

        #[test]
        fn unreadable_directory_is_counted_not_fatal() {
            use std::os::unix::fs::PermissionsExt;
            let (_t, root) = tree();
            let locked = root.join("RS_ModA/Defs");
            fs_err::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
            let readable = fs_err::read_dir(&locked).is_ok(); // running as root ignores the mode
            let out = scan_parallel(&root, &ScanOptions::default()).unwrap();
            fs_err::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
            if !readable {
                assert_eq!(out.dir_errors, 1);
                assert_eq!(out.error_samples[0].rel_path, "RS_ModA/Defs");
                assert!(rels(&out).contains(&"RS_ModA/About/About.xml"));
                assert!(!rels(&out).contains(&"RS_ModA/Defs/Things"));
            }
        }
    }

    #[test]
    #[ignore = "timing run: set RIMSTUDIO_GAME_DIR, RIMSTUDIO_WORKSHOP_DIR and RIMSTUDIO_CUSTOM_DIR, then cargo test -p rimstudio-io walk -- --ignored --nocapture"]
    fn timing_over_a_real_install_and_workshop() {
        use std::time::Instant;
        for var in [
            "RIMSTUDIO_GAME_DIR",
            "RIMSTUDIO_WORKSHOP_DIR",
            "RIMSTUDIO_CUSTOM_DIR",
        ] {
            let Ok(dir) = std::env::var(var) else {
                println!("{var} is not set, skipped");
                continue;
            };
            let root = Utf8PathBuf::from(dir);
            let t0 = Instant::now();
            let mods = list_mod_roots(&root).map(|l| l.roots.len()).unwrap_or(0);
            println!("{var}: list_mod_roots ({mods} folders): {:?}", t0.elapsed());
            for (label, stat) in [("names only", false), ("with stat", true)] {
                for workers in [1, 4, 8] {
                    let opts = ScanOptions {
                        workers,
                        stat,
                        ..ScanOptions::default()
                    };
                    let t0 = Instant::now();
                    match scan_parallel(&root, &opts) {
                        Ok(o) => println!(
                            "{var}: scan {label}, {workers} workers: {} entries, {} dirs, {} errors in {:?}",
                            o.entries.len(),
                            o.dirs_read,
                            o.dir_errors,
                            t0.elapsed()
                        ),
                        Err(e) => println!("{var}: scan failed: {e}"),
                    }
                }
            }
        }
    }
}
