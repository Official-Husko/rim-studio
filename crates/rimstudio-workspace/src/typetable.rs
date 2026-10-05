//! The def type table, built at run time from the user's assemblies.
//!
//! The repository ships no table of game types. The def engine needs one to know which element
//! names are defs, so it is read from the DLLs on disk: the game's managed assemblies (found
//! through [`managed_dirs`]) first, then every `.dll` in the `Assemblies` folders of the reference
//! set's packs in load order (each pack's load plan decides which version folders count). A mod
//! such as Combat Extended therefore brings its own def classes.
//!
//! The files are read and parsed in parallel with `rimstudio-defs::assembly`, a pure byte reader;
//! an unreadable or malformed DLL is skipped with a diagnostic, a native library is skipped with an
//! info diagnostic. The resulting table is cached as a JSON document in the cache folder, keyed by
//! the stat keys of the DLLs: the same set of files with the same sizes and times reads the table
//! back instead of parsing 16,000 type definitions again.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ports::Clock;
use rimstudio_defs::assembly::{
    AssemblyError, AssemblyTypes, build_type_table, is_runtime_library, managed_dirs,
    read_assembly, sort_game_assemblies,
};
use rimstudio_defs::type_table::{DEFAULT_ROOT, TypeTable};
use rimstudio_io::collection::{Collection, CollectionOptions};
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_io::schema::Versioned;
use rimstudio_io::statkey::StatKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{WorkspaceError, WorkspaceResult, codes};
use crate::parse::stat_key;
use crate::refset::ContentPack;

/// The schema version of the cached table document.
pub const TYPE_CACHE_VERSION: u32 = 1;

/// Where the table cache lives and which clock stamps it.
#[derive(Clone)]
pub struct CacheConfig {
    /// The folder of the cache collection (created when missing).
    pub dir: Utf8PathBuf,
    /// The clock for document stamps.
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for CacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheConfig")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl CacheConfig {
    /// A cache in an explicit folder.
    #[must_use]
    pub fn new(dir: impl Into<Utf8PathBuf>, clock: Arc<dyn Clock>) -> Self {
        CacheConfig {
            dir: dir.into(),
            clock,
        }
    }

    /// The workspace cache folder inside the cache root of the data roots.
    #[must_use]
    pub fn in_roots(roots: &DataRoots, clock: Arc<dyn Clock>) -> Self {
        CacheConfig::new(roots.path(RootKind::Cache).join("workspace-types"), clock)
    }
}

/// The cached document: the keys it was built under and the table JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TypeTableDoc {
    keys: Vec<StatKey>,
    assemblies: usize,
    table: Value,
}

impl Versioned for TypeTableDoc {
    const KIND: &'static str = "type-table-cache";
    const VERSION: u32 = TYPE_CACHE_VERSION;
}

/// The outcome of [`load_type_table`].
#[derive(Debug, Clone)]
pub struct TypeTableBuild {
    /// The table (empty when it could not be built).
    pub table: Arc<TypeTable>,
    /// True when the table came from the cache.
    pub from_cache: bool,
    /// DLLs parsed as managed assemblies (0 when the cache answered).
    pub assemblies_read: usize,
    /// DLLs skipped (unreadable, damaged or native).
    pub assemblies_skipped: usize,
    /// The stat keys of the DLLs the table stands for.
    pub keys: Vec<StatKey>,
    /// Problems found (skipped DLLs, cache trouble, a failed build).
    pub diagnostics: Vec<Diagnostic>,
}

/// The DLLs to read, in load order: the game assemblies, then the pack assemblies.
///
/// The game assemblies come from the first of [`managed_dirs`] that exists, without runtime
/// libraries, with `Assembly-CSharp` first. A missing game folder is a warning; a mod DLL that
/// appears under two packs is read twice (load order matters, the later one wins).
#[must_use]
pub fn assembly_paths(
    game_dir: Option<&Utf8Path>,
    packs: &[ContentPack],
) -> (Vec<Utf8PathBuf>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut paths: Vec<Utf8PathBuf> = Vec::new();
    let mut game: Vec<Utf8PathBuf> = Vec::new();
    if let Some(dir) = game_dir {
        for managed in managed_dirs(dir) {
            let Ok(read) = std::fs::read_dir(managed.as_std_path()) else {
                continue;
            };
            for entry in read.flatten() {
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                let is_dll = name
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("dll"));
                if is_dll && !is_runtime_library(&name) {
                    game.push(managed.join(name));
                }
            }
            if !game.is_empty() {
                break;
            }
        }
    }
    if game.is_empty() {
        diagnostics.push(Diagnostic::new(
            codes::NO_GAME_ASSEMBLIES,
            Severity::Warning,
            "no game assembly was found; the base game def types are unknown",
        ));
    }
    sort_game_assemblies(&mut game);
    paths.extend(game);
    for pack in packs {
        paths.extend(pack.assemblies.iter().map(|f| f.path.clone()));
    }
    (paths, diagnostics)
}

/// A DLL read result.
enum Read {
    Ok(Box<AssemblyTypes>),
    NotManaged(String),
    Failed(String),
}

fn read_one(path: &Utf8Path) -> Read {
    let bytes = match fs_err::read(path) {
        Ok(b) => b,
        Err(e) => return Read::Failed(format!("{path}: {e}")),
    };
    match read_assembly(&bytes) {
        Ok(asm) => Read::Ok(Box::new(asm)),
        Err(e @ (AssemblyError::NotPortableExecutable | AssemblyError::NoCliMetadata)) => {
            Read::NotManaged(format!("{path}: {e}"))
        }
        Err(e) => Read::Failed(format!("{path}: {} ({})", e, e.code())),
    }
}

fn cache_id(paths: &[Utf8PathBuf]) -> String {
    let mut hasher = blake3::Hasher::new();
    for p in paths {
        hasher.update(p.as_str().as_bytes());
        hasher.update(b"\n");
    }
    format!("tt-{}", &hasher.finalize().to_hex().as_str()[..32])
}

/// The stat keys of the files that can be examined, in path order as given.
#[must_use]
pub fn stat_keys(paths: &[Utf8PathBuf]) -> Vec<StatKey> {
    paths.par_iter().filter_map(|p| stat_key(p)).collect()
}

/// Builds (or reads from the cache) the def type table for these DLLs, in the given order. When
/// `reuse` holds the keys and table of an earlier build and the keys still match, that table is
/// returned without any reading.
///
/// Problems become diagnostics and an empty table, so defs then report an unknown type instead of
/// the session refusing to open.
///
/// # Errors
/// [`WorkspaceError::TypeTable`] only when not even an empty table can be made (cannot happen).
pub fn load_type_table(
    paths: &[Utf8PathBuf],
    cache: Option<&CacheConfig>,
    reuse: Option<(&[StatKey], &Arc<TypeTable>)>,
) -> WorkspaceResult<TypeTableBuild> {
    let keys = stat_keys(paths);
    let mut diagnostics = Vec::new();
    if let Some((old_keys, table)) = reuse
        && old_keys == keys.as_slice()
    {
        return Ok(TypeTableBuild {
            table: Arc::clone(table),
            from_cache: true,
            assemblies_read: 0,
            assemblies_skipped: 0,
            keys,
            diagnostics,
        });
    }
    let collection: Option<Collection<TypeTableDoc>> = cache.and_then(
        |c| match Collection::open_dir(c.dir.clone(), Arc::clone(&c.clock)) {
            Ok(col) => Some(col.with_options(CollectionOptions::cache())),
            Err(e) => {
                diagnostics.push(Diagnostic::new(
                    codes::TYPE_TABLE_CACHE,
                    Severity::Warning,
                    format!("the type table cache cannot be opened: {e}"),
                ));
                None
            }
        },
    );
    let id = cache_id(paths);
    if let Some(col) = &collection
        && let Ok(Some(loaded)) = col.get(&id)
        && loaded.value.keys == keys
        && let Ok(table) = TypeTable::from_json_value(loaded.value.table)
    {
        return Ok(TypeTableBuild {
            table: Arc::new(table),
            from_cache: true,
            assemblies_read: 0,
            assemblies_skipped: 0,
            keys,
            diagnostics,
        });
    }

    let results: Vec<Read> = paths.par_iter().map(|p| read_one(p)).collect();
    let mut assemblies = Vec::new();
    let mut skipped = 0usize;
    for r in results {
        match r {
            Read::Ok(a) => assemblies.push(*a),
            Read::NotManaged(msg) => {
                skipped += 1;
                diagnostics.push(Diagnostic::new(
                    codes::ASSEMBLY_NOT_MANAGED,
                    Severity::Info,
                    format!("skipped, not a managed assembly: {msg}"),
                ));
            }
            Read::Failed(msg) => {
                skipped += 1;
                diagnostics.push(Diagnostic::new(
                    codes::ASSEMBLY_SKIPPED,
                    Severity::Warning,
                    format!("skipped, unreadable assembly: {msg}"),
                ));
            }
        }
    }
    let read = assemblies.len();
    let built = build_type_table(&assemblies, DEFAULT_ROOT);
    let table = match built {
        Ok(t) => t,
        Err(e) => {
            diagnostics.push(Diagnostic::new(
                codes::TYPE_TABLE_FAILED,
                Severity::Error,
                format!("the def type table could not be built: {e}"),
            ));
            return Ok(TypeTableBuild {
                table: Arc::new(empty_table()?),
                from_cache: false,
                assemblies_read: read,
                assemblies_skipped: skipped,
                keys,
                diagnostics,
            });
        }
    };
    if let Some(col) = &collection {
        let doc = TypeTableDoc {
            keys: keys.clone(),
            assemblies: read,
            table: table.to_json_value(),
        };
        if let Err(e) = col.put(&id, &doc) {
            diagnostics.push(Diagnostic::new(
                codes::TYPE_TABLE_CACHE,
                Severity::Warning,
                format!("the type table cache cannot be written: {e}"),
            ));
        }
    }
    Ok(TypeTableBuild {
        table: Arc::new(table),
        from_cache: false,
        assemblies_read: read,
        assemblies_skipped: skipped,
        keys,
        diagnostics,
    })
}

/// A table with no types.
fn empty_table() -> WorkspaceResult<TypeTable> {
    TypeTable::new(Vec::new()).map_err(|e| WorkspaceError::TypeTable {
        message: e.to_string(),
    })
}
