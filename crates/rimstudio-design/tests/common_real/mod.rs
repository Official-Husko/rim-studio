//! Loading helpers of the real data tests: parse an install and its mods the way the game would see them
//! and run the shared def engine over them. Used by the ignored tests that need `RIMSTUDIO_GAME_DIR`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::{FileId, ModIdx, SourceId};
use rimstudio_core::load_plan::{Listing, Subdir, collect_files, resolve_load_folders_os};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::os::Os;
use rimstudio_core::version::GameVersion;
use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, PatchFile, TypeTable};
use rimstudio_testing::loaders::{
    DirParse, parse_defs_dir_report, parse_patch_dir_report, read_about, read_load_folders,
};

pub(crate) const OFFICIAL_PACKS: [&str; 6] = [
    "Core", "Royalty", "Ideology", "Biotech", "Anomaly", "Odyssey",
];

pub(crate) struct DiskListing;

pub(crate) fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let r = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&entry.path(), &r, out),
            Ok(_) => out.push(r),
            Err(_) => {}
        }
    }
}

impl Listing for DiskListing {
    fn list_files(&self, dir: &Utf8Path) -> Vec<String> {
        let mut out = Vec::new();
        walk(dir.as_std_path(), "", &mut out);
        out
    }

    fn dir_exists(&self, dir: &Utf8Path) -> bool {
        dir.as_std_path().is_dir()
    }
}

pub(crate) fn child_dirs(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub(crate) fn content_of(parse: &Option<DirParse>, rel: &str) -> FileContent {
    let Some(p) = parse else {
        return FileContent::failed("folder missing");
    };
    if let Some(f) = p.files.iter().find(|f| f.rel_path.as_str() == rel) {
        return FileContent::parsed(f.root.clone());
    }
    if let Some(f) = p.failures.iter().find(|f| f.rel_path.as_str() == rel) {
        return FileContent::failed(f.error.to_string());
    }
    FileContent::failed("file not parsed")
}

pub(crate) fn prepare(roots: &[PathBuf], version: &str, types: Arc<TypeTable>) -> LoadInput {
    let game = GameVersion::parse(version).unwrap();
    let mut metas = Vec::new();
    for root in roots {
        let about = read_about(root).unwrap();
        let utf = Utf8PathBuf::from_path_buf(root.clone()).unwrap();
        let mut meta = about
            .about
            .into_meta(SourceId::new("rs").unwrap(), utf)
            .unwrap();
        if let Some(lf) = read_load_folders(root).unwrap() {
            meta.load_folders = Some(lf.spec);
        }
        meta.root_dirs = child_dirs(root);
        metas.push(meta);
    }
    let active = ActiveSet::from_ids(metas.iter().map(|m| m.package_id.as_str().to_owned()));
    let mut mods = Vec::new();
    let mut def_files = Vec::new();
    let mut patch_files = Vec::new();
    let mut next_file = 0u32;
    let mut cache: BTreeMap<(Utf8PathBuf, bool), Option<DirParse>> = BTreeMap::new();
    for (i, meta) in metas.iter().enumerate() {
        let idx = ModIdx(u32::try_from(i).unwrap());
        mods.push(ModEntry::new(
            idx,
            meta.package_id.as_str(),
            meta.name.clone(),
        ));
        let plan = resolve_load_folders_os(meta, &game, &active, Os::Linux);
        for (defs, subdir) in [(true, Subdir::Defs), (false, Subdir::Patches)] {
            for f in collect_files(&plan, subdir, &DiskListing) {
                let folder = &plan.folders[f.folder].path;
                let parse = cache
                    .entry((folder.clone(), defs))
                    .or_insert_with(|| {
                        if defs {
                            parse_defs_dir_report(folder.join("Defs").as_std_path()).ok()
                        } else {
                            parse_patch_dir_report(folder.join("Patches").as_std_path()).ok()
                        }
                    })
                    .clone();
                let rel_in = f.relative.split_once('/').map_or("", |(_, r)| r);
                let content = content_of(&parse, rel_in);
                let file = FileId(next_file);
                next_file += 1;
                if defs {
                    def_files.push(DefFile {
                        mod_idx: idx,
                        file,
                        rel_path: f.relative.clone(),
                        content,
                    });
                } else {
                    patch_files.push(PatchFile {
                        mod_idx: idx,
                        file,
                        rel_path: f.relative.clone(),
                        content,
                    });
                }
            }
        }
    }
    let mut input = LoadInput::new(mods, types);
    input.def_files = def_files;
    input.patch_files = patch_files;
    input
}

pub(crate) fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("RIMSTUDIO_GAME_DIR")?);
    dir.join("Version.txt").is_file().then_some(dir)
}

pub(crate) fn type_table() -> Option<TypeTable> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data/def-engine/data/def_types_vanilla.json");
    TypeTable::from_json_str(&fs::read_to_string(path).ok()?).ok()
}

pub(crate) fn version(dir: &Path) -> String {
    fs::read_to_string(dir.join("Version.txt"))
        .unwrap()
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

pub(crate) fn official(dir: &Path) -> Vec<PathBuf> {
    OFFICIAL_PACKS
        .iter()
        .map(|p| dir.join("Data").join(p))
        .filter(|p| p.is_dir())
        .collect()
}
