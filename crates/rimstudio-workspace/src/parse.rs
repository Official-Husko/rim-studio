//! Reading and parsing Defs and Patches files, with a cache keyed by stat keys.
//!
//! Files are parsed in the game's mode through `rimstudio-xml` ([`ParseMode::Game`]): a file the
//! game would skip becomes a failed file (one diagnostic later), never an error. The
//! [`ParseCache`] keeps the parsed tree of each file with the stat key it was read under; a file
//! whose key still matches is not read again, so a second open of an unchanged reference set parses
//! zero files. The cache is shared through an `Arc` and can serve several sessions.
//!
//! Each cached Defs file also keeps the [`DefSummary`] rows of its defs, the input of the def index.

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_io::statkey::StatKey;
use rimstudio_xml::{ParseMode, parse_document};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::defindex::DefSummary;
use crate::error::codes;

/// Which kind of file a path is, decided by the folder it was listed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileKind {
    /// A file below a `Defs` folder.
    Defs,
    /// A file below a `Patches` folder.
    Patches,
}

/// The outcome of reading one file.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// The file parsed; `defs` is empty for patch files and for files without a `Defs` root.
    Parsed {
        root: Node,
        diagnostics: Vec<Diagnostic>,
        defs: Vec<DefSummary>,
    },
    /// The file could not be read or parsed; the game skips it.
    Failed { message: String },
}

/// A file as the cache keeps it.
#[derive(Debug)]
pub(crate) struct CachedFile {
    pub(crate) key: Option<StatKey>,
    pub(crate) outcome: Outcome,
}

/// Counters of a [`ParseCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseCacheStats {
    /// Files currently cached.
    pub entries: usize,
    /// Files parsed since the cache was created.
    pub parsed: u64,
    /// Lookups answered from the cache since it was created.
    pub hits: u64,
}

/// A cache of parsed Defs and Patches files, keyed by absolute path and stat key.
#[derive(Debug, Default)]
pub struct ParseCache {
    entries: RwLock<FxHashMap<Utf8PathBuf, Arc<CachedFile>>>,
    parsed: AtomicU64,
    hits: AtomicU64,
}

/// The stat key of a file (links followed), keyed by its absolute path. `None` when the file cannot
/// be examined.
pub(crate) fn stat_key(path: &Utf8Path) -> Option<StatKey> {
    let meta = fs_err::metadata(path).ok()?;
    meta.is_file()
        .then(|| StatKey::from_metadata(path.as_str(), &meta, None))
}

fn summarize_defs(root: &Node) -> Vec<DefSummary> {
    if root.tag != "Defs" {
        return Vec::new();
    }
    root.elements()
        .map(|el| {
            let text = |tag: &str| {
                el.child_text(tag)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
            };
            let attr = |name: &str| {
                el.attr(name)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
            };
            // the game reads the type from the `Class` attribute when there is one
            DefSummary {
                def_type: attr("Class").unwrap_or_else(|| el.tag.clone()),
                def_name: text("defName").unwrap_or_default(),
                label: text("label"),
                name: attr("Name"),
                parent: attr("ParentName"),
                is_abstract: el
                    .attr("Abstract")
                    .is_some_and(|a| a.trim().eq_ignore_ascii_case("true")),
            }
        })
        .collect()
}

fn parse_bytes(bytes: &[u8], kind: FileKind) -> Outcome {
    match parse_document(bytes, ParseMode::Game) {
        Ok(doc) => {
            let defs = if kind == FileKind::Defs {
                summarize_defs(&doc.root)
            } else {
                Vec::new()
            };
            Outcome::Parsed {
                root: doc.root,
                diagnostics: doc.diagnostics,
                defs,
            }
        }
        Err(e) => Outcome::Failed {
            message: e.to_string(),
        },
    }
}

/// The result of asking the cache for a file.
pub(crate) struct Fetched {
    pub(crate) file: Arc<CachedFile>,
    /// True when the file was read and parsed for this call.
    pub(crate) parsed: bool,
    /// A diagnostic when the file could not be read from disk.
    pub(crate) problem: Option<Diagnostic>,
}

impl ParseCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        ParseCache::default()
    }

    /// The counters of the cache.
    #[must_use]
    pub fn stats(&self) -> ParseCacheStats {
        let entries = self
            .entries
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        ParseCacheStats {
            entries,
            parsed: self.parsed.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
        }
    }

    /// Forgets one file, so the next lookup reads it again.
    pub fn invalidate(&self, path: &Utf8Path) {
        self.entries
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(path);
    }

    /// Forgets every file below `dir`.
    pub fn invalidate_under(&self, dir: &Utf8Path) {
        self.entries
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|p, _| !p.starts_with(dir));
    }

    /// Forgets everything.
    pub fn clear(&self) {
        self.entries
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// Returns the cached file when its stat key still matches, else reads and parses it.
    pub(crate) fn fetch(&self, path: &Utf8Path, kind: FileKind) -> Fetched {
        let key = stat_key(path);
        if let Some(key) = &key {
            let hit = self
                .entries
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .get(path)
                .filter(|c| c.key.as_ref() == Some(key))
                .cloned();
            if let Some(file) = hit {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Fetched {
                    file,
                    parsed: false,
                    problem: None,
                };
            }
        }
        match fs_err::read(path) {
            Ok(bytes) => {
                self.parsed.fetch_add(1, Ordering::Relaxed);
                let file = Arc::new(CachedFile {
                    key: key.clone(),
                    outcome: parse_bytes(&bytes, kind),
                });
                if key.is_some() {
                    self.entries
                        .write()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(path.to_owned(), Arc::clone(&file));
                }
                Fetched {
                    file,
                    parsed: true,
                    problem: None,
                }
            }
            Err(e) => Fetched {
                file: Arc::new(CachedFile {
                    key: None,
                    outcome: Outcome::Failed {
                        message: e.to_string(),
                    },
                }),
                parsed: false,
                problem: Some(Diagnostic::new(
                    codes::FILE_UNREADABLE,
                    Severity::Warning,
                    format!("cannot read {path}: {e}"),
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8(p: &std::path::Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()
    }

    fn write(dir: &Utf8Path, name: &str, text: &str) -> Utf8PathBuf {
        let p = dir.join(name);
        fs_err::write(&p, text).unwrap();
        p
    }

    #[test]
    fn a_second_fetch_of_an_unchanged_file_is_a_hit() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = utf8(tmp.path());
        let p = write(
            &dir,
            "a.xml",
            "<Defs><ThingDef><defName>RS_A</defName><label>a thing</label></ThingDef></Defs>",
        );
        let cache = ParseCache::new();
        let first = cache.fetch(&p, FileKind::Defs);
        assert!(first.parsed);
        let second = cache.fetch(&p, FileKind::Defs);
        assert!(!second.parsed);
        assert!(Arc::ptr_eq(&first.file, &second.file));
        let s = cache.stats();
        assert_eq!((s.entries, s.parsed, s.hits), (1, 1, 1));
    }

    #[test]
    fn a_changed_file_is_parsed_again() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = utf8(tmp.path());
        let p = write(
            &dir,
            "a.xml",
            "<Defs><ThingDef><defName>RS_A</defName></ThingDef></Defs>",
        );
        let cache = ParseCache::new();
        let _ = cache.fetch(&p, FileKind::Defs);
        write(
            &dir,
            "a.xml",
            "<Defs><ThingDef><defName>RS_A</defName></ThingDef><ThingDef><defName>RS_B</defName></ThingDef></Defs>",
        );
        let again = cache.fetch(&p, FileKind::Defs);
        assert!(again.parsed);
        let Outcome::Parsed { defs, .. } = &again.file.outcome else {
            panic!("should parse");
        };
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn def_summaries_read_name_label_parent_and_abstract() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = utf8(tmp.path());
        let p = write(
            &dir,
            "a.xml",
            r#"<Defs>
  <ThingDef Name="RS_Base" Abstract="True"><label>base</label></ThingDef>
  <ThingDef ParentName="RS_Base"><defName>RS_Child</defName></ThingDef>
  <ThingDef Class="RS_Mod.RS_AmmoDef"><defName>RS_Ammo</defName></ThingDef>
</Defs>"#,
        );
        let cache = ParseCache::new();
        let got = cache.fetch(&p, FileKind::Defs);
        let Outcome::Parsed { defs, .. } = &got.file.outcome else {
            panic!("should parse");
        };
        assert_eq!(defs[0].name.as_deref(), Some("RS_Base"));
        assert!(defs[0].is_abstract);
        assert_eq!(defs[0].label.as_deref(), Some("base"));
        assert_eq!(defs[1].parent.as_deref(), Some("RS_Base"));
        assert_eq!(defs[1].def_name, "RS_Child");
        assert_eq!(defs[1].def_type, "ThingDef");
        assert_eq!(
            defs[2].def_type, "RS_Mod.RS_AmmoDef",
            "the Class attribute names the type"
        );
    }

    #[test]
    fn broken_and_missing_files_are_failures_not_panics() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = utf8(tmp.path());
        let bad = write(&dir, "bad.xml", "<Defs><ThingDef></Defs>");
        let cache = ParseCache::new();
        let got = cache.fetch(&bad, FileKind::Defs);
        assert!(matches!(got.file.outcome, Outcome::Failed { .. }));
        assert!(got.problem.is_none());
        let missing = cache.fetch(&dir.join("nope.xml"), FileKind::Patches);
        assert!(matches!(missing.file.outcome, Outcome::Failed { .. }));
        assert_eq!(missing.problem.unwrap().code, codes::FILE_UNREADABLE);
    }

    #[test]
    fn patch_files_carry_no_def_rows_and_invalidate_forgets() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = utf8(tmp.path());
        let p = write(
            &dir,
            "p.xml",
            "<Patch><Operation Class=\"PatchOperationTest\"/></Patch>",
        );
        let cache = ParseCache::new();
        let got = cache.fetch(&p, FileKind::Patches);
        let Outcome::Parsed { defs, root, .. } = &got.file.outcome else {
            panic!("should parse");
        };
        assert!(defs.is_empty());
        assert_eq!(root.tag, "Patch");
        cache.invalidate(&p);
        assert!(cache.fetch(&p, FileKind::Patches).parsed);
        cache.invalidate_under(&dir);
        assert_eq!(cache.stats().entries, 0);
        cache.clear();
    }
}
