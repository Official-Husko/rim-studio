//! Level 1 of the scan: content folders, markers and the definition index of one mod (crate private).
//!
//! The folders searched are a superset of what the game loads: every folder of the load plan, once
//! with all installed mods treated as active and once with none, so conditional `LoadFolders.xml`
//! entries on either side are covered. Without a game version every block of `LoadFolders.xml` (or
//! every version folder, `Common` and the root when there is none) is searched. Definition files are
//! indexed through `rimstudio-xml`; a file whose stat key matches the previous manifest is not read.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::jobs::CancelToken;
use rimstudio_core::load_plan::resolve_load_folders;
use rimstudio_core::mods::{ActiveSet, ModMeta};
use rimstudio_core::os::Os;
use rimstudio_core::paths;
use rimstudio_core::version::{GameVersion, parse_major_minor};
use rimstudio_io::statkey::{FatPolicy, FileIdFn, StatKey, system_time_ns};
use rimstudio_xml::defs_scan::index_file_detailed;
use rustc_hash::FxHashMap;

use crate::cache::{DefFileCache, FileStamp, ModCache, file_unchanged};
use crate::error::codes;
use crate::index::{DefFile, ModContent};

/// Definition files larger than this are skipped with a diagnostic.
pub(crate) const MAX_DEF_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Folders nested deeper than this below a content folder are not entered.
const MAX_WALK_DEPTH: usize = 32;

/// Everything level 1 needs besides the mod itself.
pub(crate) struct Level1Ctx<'a> {
    pub(crate) game_version: Option<&'a GameVersion>,
    pub(crate) active_all: &'a ActiveSet,
    pub(crate) file_id: Option<FileIdFn>,
    pub(crate) policy: FatPolicy,
    pub(crate) cancel: &'a CancelToken,
}

/// The result of indexing one mod.
#[derive(Debug, Default)]
pub(crate) struct Level1Out {
    pub(crate) content: ModContent,
    pub(crate) files: Vec<DefFileCache>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) parsed: u64,
    pub(crate) reused: u64,
    pub(crate) hashed: u64,
    pub(crate) bytes_read: u64,
    pub(crate) dir_errors: u64,
    pub(crate) non_utf8: u64,
}

fn normalise_folder(raw: &str) -> Option<String> {
    let text = raw.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            other => parts.push(other),
        }
    }
    if text.starts_with('/') && !parts.is_empty() {
        // An absolute path: the game would join it below the mod root, but a leading separator is
        // the root marker only when alone. Treat anything else as unusable.
        return None;
    }
    if parts.first().is_some_and(|p| p.contains(':')) {
        return None;
    }
    Some(parts.join("/"))
}

/// The folders (relative to the mod root, empty for the root) to search for content.
pub(crate) fn candidate_folders(
    meta: &ModMeta,
    game: Option<&GameVersion>,
    active_all: &ActiveSet,
) -> (Vec<String>, Vec<Diagnostic>) {
    let mut raw: Vec<String> = Vec::new();
    if let Some(game) = game {
        let a = resolve_load_folders(meta, game, active_all);
        let b = resolve_load_folders(meta, game, &ActiveSet::new());
        for plan in [a, b] {
            raw.extend(plan.folders.into_iter().map(|f| f.relative));
        }
    } else if let Some(spec) = meta.load_folders.as_ref().filter(|s| !s.is_empty()) {
        for block in &spec.blocks {
            for entry in &block.entries {
                raw.push(entry.folder_for(Os::current()));
            }
        }
    } else {
        for dir in &meta.root_dirs {
            if parse_major_minor(dir, false).is_some()
                || dir.eq_ignore_ascii_case(paths::COMMON_DIR)
            {
                raw.push(dir.clone());
            }
        }
        raw.push(String::new());
    }
    let mut diagnostics = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut rejected: Vec<String> = Vec::new();
    for text in raw {
        match normalise_folder(&text) {
            Some(folder) => {
                if !out.contains(&folder) {
                    out.push(folder);
                }
            }
            None => {
                if !rejected.contains(&text) {
                    diagnostics.push(
                        Diagnostic::new(
                            codes::LOAD_FOLDER_OUTSIDE,
                            Severity::Warning,
                            "A LoadFolders.xml entry points outside the mod folder and is ignored.",
                        )
                        .with_arg("entry", text.clone()),
                    );
                    rejected.push(text);
                }
            }
        }
    }
    (out, diagnostics)
}

/// A file found below `Defs` or `Patches`.
struct RawFile {
    rel: String,
    abs: Utf8PathBuf,
    size: u64,
    mtime_ns: Option<i128>,
    file_id: Option<u128>,
}

#[derive(Default)]
struct WalkCounters {
    dir_errors: u64,
    non_utf8: u64,
}

fn is_xml_file(name: &str) -> bool {
    !name.starts_with('.')
        && name
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case(paths::XML_EXT))
}

fn walk_xml(
    abs: &Utf8Path,
    rel: &str,
    depth: usize,
    file_id: Option<FileIdFn>,
    out: &mut Vec<RawFile>,
    counters: &mut WalkCounters,
) {
    let rd = match fs_err::read_dir(abs) {
        Ok(rd) => rd,
        Err(_) => {
            counters.dir_errors += 1;
            return;
        }
    };
    for entry in rd {
        let Ok(entry) = entry else {
            counters.dir_errors += 1;
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            counters.non_utf8 += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else {
            counters.dir_errors += 1;
            continue;
        };
        let child_rel = format!("{rel}/{name}");
        if ft.is_dir() {
            if depth < MAX_WALK_DEPTH && !name.starts_with('.') {
                walk_xml(
                    &abs.join(&name),
                    &child_rel,
                    depth + 1,
                    file_id,
                    out,
                    counters,
                );
            }
        } else if ft.is_file() && is_xml_file(&name) {
            let (size, mtime_ns, id) = match entry.metadata() {
                Ok(m) => (
                    m.len(),
                    m.modified().ok().map(system_time_ns),
                    file_id.and_then(|f| f(&m)),
                ),
                Err(_) => {
                    counters.dir_errors += 1;
                    continue;
                }
            };
            out.push(RawFile {
                rel: child_rel,
                abs: abs.join(&name),
                size,
                mtime_ns,
                file_id: id,
            });
        }
    }
}

/// Finds a child folder by name, preferring the exact spelling, else the smallest name that matches
/// ignoring case.
fn find_dir<'a>(names: &'a [String], wanted: &str) -> Option<&'a str> {
    if let Some(exact) = names.iter().find(|n| n.as_str() == wanted) {
        return Some(exact);
    }
    names
        .iter()
        .filter(|n| n.eq_ignore_ascii_case(wanted))
        .min()
        .map(String::as_str)
}

fn join_rel(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

/// Indexes the content of one mod.
///
/// Returns early (with `content.complete == false`) when `cancel` is set; what was indexed so far is
/// returned and can be cached.
pub(crate) fn index_mod(meta: &ModMeta, prev: Option<&ModCache>, ctx: &Level1Ctx<'_>) -> Level1Out {
    let mut out = Level1Out::default();
    let (folders, mut diags) = candidate_folders(meta, ctx.game_version, ctx.active_all);
    out.diagnostics.append(&mut diags);
    let mut counters = WalkCounters::default();

    let mut def_raw: Vec<RawFile> = Vec::new();
    let mut patch_files: Vec<String> = Vec::new();
    let mut languages: Vec<String> = Vec::new();
    let mut assemblies: Vec<String> = Vec::new();

    for folder in &folders {
        let dir_abs = if folder.is_empty() {
            meta.path.clone()
        } else {
            meta.path.join(folder)
        };
        let mut names: Vec<String> = Vec::new();
        match fs_err::read_dir(&dir_abs) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    let Ok(name) = entry.file_name().into_string() else {
                        counters.non_utf8 += 1;
                        continue;
                    };
                    if entry.file_type().is_ok_and(|t| t.is_dir()) {
                        names.push(name);
                    }
                }
            }
            Err(e) => {
                // A missing folder named by LoadFolders.xml is normal for conditional entries.
                if e.kind() != std::io::ErrorKind::NotFound {
                    counters.dir_errors += 1;
                }
                continue;
            }
        }
        if let Some(defs) = find_dir(&names, paths::DEFS_DIR) {
            walk_xml(
                &dir_abs.join(defs),
                &join_rel(folder, defs),
                1,
                ctx.file_id,
                &mut def_raw,
                &mut counters,
            );
        }
        if let Some(patches) = find_dir(&names, paths::PATCHES_DIR) {
            let mut raw = Vec::new();
            walk_xml(
                &dir_abs.join(patches),
                &join_rel(folder, patches),
                1,
                None,
                &mut raw,
                &mut counters,
            );
            patch_files.extend(raw.into_iter().map(|r| r.rel));
        }
        if let Some(lang) = find_dir(&names, paths::LANGUAGES_DIR)
            && let Ok(rd) = fs_err::read_dir(dir_abs.join(lang))
        {
            for entry in rd.flatten() {
                if let Ok(name) = entry.file_name().into_string()
                    && entry.file_type().is_ok_and(|t| t.is_dir())
                    && !name.starts_with('.')
                {
                    languages.push(format!("{}/{name}", join_rel(folder, lang)));
                }
            }
        }
        if let Some(asm) = find_dir(&names, paths::ASSEMBLIES_DIR)
            && let Ok(rd) = fs_err::read_dir(dir_abs.join(asm))
        {
            for entry in rd.flatten() {
                if let Ok(name) = entry.file_name().into_string()
                    && !name.starts_with('.')
                    && entry.file_type().is_ok_and(|t| t.is_file())
                    && name
                        .rsplit_once('.')
                        .is_some_and(|(_, e)| e.eq_ignore_ascii_case(paths::DLL_EXT))
                {
                    assemblies.push(format!("{}/{name}", join_rel(folder, asm)));
                }
            }
        }
    }

    def_raw.sort_by(|a, b| a.rel.cmp(&b.rel));
    def_raw.dedup_by(|a, b| a.rel == b.rel);
    patch_files.sort();
    patch_files.dedup();
    languages.sort();
    languages.dedup();
    assemblies.sort();
    assemblies.dedup();
    out.dir_errors += counters.dir_errors;
    out.non_utf8 += counters.non_utf8;

    let prev_files: FxHashMap<&str, &DefFileCache> = prev
        .map(|p| {
            p.files
                .iter()
                .map(|f| (f.stamp.key.rel_path.as_str(), f))
                .collect()
        })
        .unwrap_or_default();

    let mut def_files: Vec<DefFile> = Vec::with_capacity(def_raw.len());
    let mut cached: Vec<DefFileCache> = Vec::with_capacity(def_raw.len());
    let mut complete = true;
    for raw in &def_raw {
        if ctx.cancel.is_cancelled() {
            complete = false;
            break;
        }
        let key = StatKey {
            rel_path: raw.rel.clone(),
            size: raw.size,
            mtime_ns: raw.mtime_ns,
            file_id: raw.file_id,
        };
        if let Some(old) = prev_files.get(raw.rel.as_str()) {
            let (same, fresh_hash) = file_unchanged(ctx.policy, &old.stamp, &key, &raw.abs);
            if fresh_hash.is_some() {
                out.hashed += 1;
            }
            if same {
                out.reused += 1;
                def_files.push(DefFile {
                    rel_path: raw.rel.clone(),
                    size: raw.size,
                    records: Arc::clone(&old.records),
                });
                cached.push(DefFileCache {
                    stamp: FileStamp {
                        key,
                        hash: fresh_hash.or_else(|| old.stamp.hash.clone()),
                    },
                    records: Arc::clone(&old.records),
                });
                continue;
            }
        }
        if raw.size > MAX_DEF_FILE_BYTES {
            out.diagnostics.push(
                Diagnostic::new(
                    codes::DEF_FILE_BROKEN,
                    Severity::Warning,
                    "A definition file is too large to index and was skipped.",
                )
                .with_arg("file", raw.rel.clone()),
            );
            continue;
        }
        let bytes = match fs_err::read(&raw.abs) {
            Ok(b) => b,
            Err(e) => {
                out.dir_errors += 1;
                out.diagnostics.push(
                    Diagnostic::new(
                        codes::DEF_FILE_BROKEN,
                        Severity::Warning,
                        "A definition file could not be read.",
                    )
                    .with_arg("file", raw.rel.clone())
                    .with_arg("error", e.to_string()),
                );
                continue;
            }
        };
        out.bytes_read += bytes.len() as u64;
        let hash =
            (ctx.policy == FatPolicy::Fat).then(|| blake3::hash(&bytes).to_hex().to_string());
        let indexed = index_file_detailed(&bytes);
        out.parsed += 1;
        let broken = indexed.error.is_some();
        if let Some(err) = &indexed.error {
            out.diagnostics.push(
                Diagnostic::new(
                    codes::DEF_FILE_BROKEN,
                    Severity::Warning,
                    "A definition file ends early or is malformed; the definitions before the error are listed.",
                )
                .with_arg("file", raw.rel.clone())
                .with_arg("error", err.to_string()),
            );
        }
        let records = Arc::new(indexed.records);
        def_files.push(DefFile {
            rel_path: raw.rel.clone(),
            size: raw.size,
            records: Arc::clone(&records),
        });
        if !broken {
            cached.push(DefFileCache {
                stamp: FileStamp { key, hash },
                records,
            });
        }
    }

    patch_files.shrink_to_fit();
    out.content = ModContent {
        folders,
        def_files,
        patch_files,
        languages,
        assemblies,
        complete,
    };
    out.files = cached;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ids::{PackageId, SourceId};
    use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};

    fn meta(root_dirs: &[&str], spec: Option<LoadFoldersSpec>) -> ModMeta {
        let mut m = ModMeta::new(
            PackageId::parse("rs.test").unwrap(),
            "RS Test",
            SourceId::game_mods(),
            Utf8PathBuf::from("/m/RS_Test"),
        );
        m.root_dirs = root_dirs.iter().map(|s| (*s).to_owned()).collect();
        m.load_folders = spec;
        m
    }

    #[test]
    fn folders_are_normalised_and_unsafe_entries_rejected() {
        assert_eq!(normalise_folder("1.6/Common"), Some("1.6/Common".into()));
        assert_eq!(normalise_folder("/"), Some(String::new()));
        assert_eq!(normalise_folder("\\"), Some(String::new()));
        assert_eq!(normalise_folder("./1.6/"), Some("1.6".into()));
        assert_eq!(normalise_folder("1.6\\Defs"), Some("1.6/Defs".into()));
        assert_eq!(normalise_folder("../x"), None);
        assert_eq!(normalise_folder("a/../../x"), None);
        assert_eq!(normalise_folder("/abs/path"), None);
        assert_eq!(normalise_folder("C:/x"), None);
    }

    #[test]
    fn without_a_version_every_version_folder_and_common_are_searched() {
        let m = meta(&["1.4", "1.6", "Common", "Textures", "About"], None);
        let (f, d) = candidate_folders(&m, None, &ActiveSet::new());
        assert!(d.is_empty());
        assert_eq!(f, vec!["1.4", "1.6", "Common", ""]);
    }

    #[test]
    fn with_a_version_only_the_load_plan_folders_are_searched() {
        let m = meta(&["1.4", "1.6", "Common"], None);
        let v = GameVersion::parse("1.6.4000").unwrap();
        let (f, _) = candidate_folders(&m, Some(&v), &ActiveSet::new());
        assert_eq!(f, vec!["1.6", "Common", ""]);
    }

    #[test]
    fn conditional_entries_are_covered_on_both_sides() {
        let spec = LoadFoldersSpec::from_blocks([(
            "1.6",
            vec![
                LoadEntry::dir("Base"),
                LoadEntry::dir("WithOther").if_active(["rs.other"]),
                LoadEntry::dir("WithoutOther").if_not_active(["rs.other"]),
                LoadEntry::dir("../escape"),
            ],
        )]);
        let m = meta(&["Base", "WithOther", "WithoutOther"], Some(spec));
        let v = GameVersion::parse("1.6.4000").unwrap();
        let active = ActiveSet::from_ids(["rs.other"]);
        let (mut f, d) = candidate_folders(&m, Some(&v), &active);
        f.sort();
        assert_eq!(f, vec!["Base", "WithOther", "WithoutOther"]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, codes::LOAD_FOLDER_OUTSIDE);
    }

    #[test]
    fn find_dir_prefers_exact_case_then_the_smallest_match() {
        let names: Vec<String> = ["defs", "Defs", "DEFS"].map(String::from).to_vec();
        assert_eq!(find_dir(&names, "Defs"), Some("Defs"));
        let names: Vec<String> = ["defs", "DEFS"].map(String::from).to_vec();
        assert_eq!(find_dir(&names, "Defs"), Some("DEFS"));
        assert_eq!(find_dir(&names, "Patches"), None);
    }

    #[test]
    fn xml_file_names() {
        assert!(is_xml_file("a.xml"));
        assert!(is_xml_file("A.XML"));
        assert!(!is_xml_file(".hidden.xml"));
        assert!(!is_xml_file("a.xmlx"));
        assert!(!is_xml_file("xml"));
        assert!(!is_xml_file(".xml"));
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn normalised_folders_never_escape_the_mod_root(raw in "[a-zA-Z0-9_.:/\\\\ -]{0,30}") {
                if let Some(folder) = normalise_folder(&raw) {
                    prop_assert!(!folder.split('/').any(|p| p == ".." || p == "." ));
                    prop_assert!(!folder.starts_with('/'));
                    prop_assert!(!folder.contains('\\'));
                }
            }
        }
    }
}
