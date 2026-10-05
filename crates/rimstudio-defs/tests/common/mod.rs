//! Shared helpers of the integration tests: turn folders on disk into a [`LoadInput`].
//!
//! The def engine does no IO and no XML parsing, so the tests go through the support crate: mod
//! folders are read with `rimstudio_testing::loaders`, load folders are resolved with the core
//! load plan, and the resulting files are handed to `rimstudio_defs::load`.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::{FileId, ModIdx, SourceId};
use rimstudio_core::load_plan::{Listing, Subdir, collect_files, resolve_load_folders_os};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::os::Os;
use rimstudio_core::tree::Node;
use rimstudio_core::version::GameVersion;
use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, PatchFile, TypeInfo, TypeTable};
use rimstudio_testing::loaders::{
    DirParse, parse_defs_dir_report, parse_patch_dir_report, read_about, read_load_folders,
};

/// A real folder as a [`Listing`].
pub struct DiskListing;

fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
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

/// The mod entries and file tables of a prepared load.
pub struct Prepared {
    /// The input of `rimstudio_defs::load`.
    pub input: LoadInput,
    /// The relative path of each file, indexed by `FileId`.
    pub files: Vec<String>,
}

impl Prepared {
    /// The relative path of a file handle.
    pub fn file_path(&self, id: FileId) -> Option<&str> {
        self.files.get(id.index()).map(String::as_str)
    }

    /// The package id of a mod handle.
    pub fn package_id(&self, idx: ModIdx) -> Option<&str> {
        self.input
            .mods
            .iter()
            .find(|m| m.idx == idx)
            .map(|m| m.package_id.as_str())
    }
}

fn child_dirs(root: &Path) -> Vec<String> {
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

fn dir_parse(
    cache: &mut BTreeMap<(Utf8PathBuf, bool), Option<DirParse>>,
    folder: &Utf8Path,
    defs: bool,
) -> Option<DirParse> {
    cache
        .entry((folder.to_owned(), defs))
        .or_insert_with(|| {
            if defs {
                parse_defs_dir_report(folder.join("Defs").as_std_path()).ok()
            } else {
                parse_patch_dir_report(folder.join("Patches").as_std_path()).ok()
            }
        })
        .clone()
}

fn content_of(parse: &Option<DirParse>, rel: &str) -> FileContent {
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

/// Reads the mod folders (in load order) the way the game's loader does and builds a load input.
pub fn prepare(
    roots: &[PathBuf],
    game_version: &str,
    extra_active: &[String],
    types: Arc<TypeTable>,
) -> Prepared {
    let game = GameVersion::parse(game_version).unwrap();
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
    let active = ActiveSet::from_ids(
        metas
            .iter()
            .map(|m| m.package_id.as_str().to_owned())
            .chain(extra_active.iter().cloned()),
    );
    let mut mods = Vec::new();
    let mut def_files = Vec::new();
    let mut patch_files = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut cache = BTreeMap::new();
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
                let parse = dir_parse(&mut cache, folder, defs);
                let rel_in = f.relative.split_once('/').map_or("", |(_, r)| r);
                let content = content_of(&parse, rel_in);
                let file = FileId(u32::try_from(files.len()).unwrap());
                files.push(f.relative.clone());
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
    input.extra_active_ids = extra_active.to_vec();
    Prepared { input, files }
}

/// A small type table for the vectors: the root, one def type and the given subclasses of it.
pub fn vector_types(extra: &BTreeMap<String, String>) -> Arc<TypeTable> {
    let mut types = vec![
        (
            "Verse.Def".to_owned(),
            TypeInfo::with_base("Verse.Editable"),
        ),
        (
            "Verse.ThingDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
    ];
    for (name, base) in extra {
        types.push((name.clone(), TypeInfo::with_base(base.clone())));
    }
    Arc::new(TypeTable::new(types).unwrap())
}

/// Parses an XML fragment (one element) with the support crate's loaders.
pub fn parse_fragment(xml: &str) -> Node {
    let dir = tempfile::tempdir().unwrap();
    let defs = dir.path().join("Defs");
    fs::create_dir_all(&defs).unwrap();
    fs::write(defs.join("e.xml"), format!("<Defs>{xml}</Defs>")).unwrap();
    let parsed = parse_defs_dir_report(&defs).unwrap();
    assert!(parsed.failures.is_empty(), "fragment does not parse: {xml}");
    parsed
        .files
        .into_iter()
        .next()
        .and_then(|f| f.root.elements().next().cloned())
        .unwrap()
}

// ------------------------------------------------------------------ research vectors

#[derive(Debug, Deserialize)]
pub struct VMod {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// Distinguishes a missing key (`None`) from an explicit `null` (`Some(None)`).
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Debug, Default, Deserialize)]
pub struct Provenance {
    #[serde(default, rename = "mod", deserialize_with = "double_option")]
    pub mod_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub file: Option<Option<String>>,
    #[serde(default)]
    pub parents: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Expect {
    #[serde(default)]
    pub defs: BTreeMap<String, String>,
    #[serde(default)]
    pub absent: Vec<String>,
    #[serde(default)]
    pub provenance: BTreeMap<String, Provenance>,
    #[serde(default)]
    pub diag: BTreeMap<String, u64>,
    #[serde(default)]
    pub patch_results: Option<Vec<bool>>,
    #[serde(default)]
    pub db_order: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub stats: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
pub struct Vector {
    pub name: String,
    pub doc: String,
    pub game_version: String,
    pub mods: Vec<VMod>,
    #[serde(default)]
    pub extra_types: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_active_ids: Vec<String>,
    pub expect: Expect,
}

pub fn load_vectors() -> Vec<Vector> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let text = fs::read_to_string(&p).unwrap();
        let mut v: Vec<Vector> = serde_json::from_str(&text).unwrap();
        out.append(&mut v);
    }
    out
}

pub fn write_mods(vector: &Vector, base: &std::path::Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for (i, m) in vector.mods.iter().enumerate() {
        let root = base.join(format!("{i:02}_{}", m.id.replace('/', "_")));
        let mut files = m.files.clone();
        if !files
            .keys()
            .any(|k| k.eq_ignore_ascii_case("about/about.xml"))
        {
            files.insert(
                "About/About.xml".to_owned(),
                format!(
                    "<ModMetaData><name>{}</name><packageId>{}</packageId></ModMetaData>",
                    m.name.as_deref().unwrap_or(&m.id),
                    m.id
                ),
            );
        }
        for (rel, text) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        roots.push(root);
    }
    roots
}

/// Runs a vector end to end: writes the mods, prepares the input and loads it.
pub fn run_vector(vector: &Vector) -> (Prepared, rimstudio_defs::LoadOutput) {
    let dir = tempfile::tempdir().unwrap();
    let roots = write_mods(vector, dir.path());
    let types = vector_types(&vector.extra_types);
    let prepared = prepare(
        &roots,
        &vector.game_version,
        &vector.extra_active_ids,
        types,
    );
    let out = rimstudio_defs::load(prepared.input.clone());
    (prepared, out)
}

// ------------------------------------------------------------------ synthetic corpus

use rimstudio_core::tree::NodeBuilder;

/// The type table of the synthetic corpus.
pub fn synth_types() -> Arc<TypeTable> {
    Arc::new(
        TypeTable::new(vec![
            (
                "Verse.Def".to_owned(),
                TypeInfo::with_base("Verse.Editable"),
            ),
            (
                "Verse.RS_BuildableDef".to_owned(),
                TypeInfo::with_base("Verse.Def"),
            ),
            (
                "Verse.RS_ThingDef".to_owned(),
                TypeInfo::with_base("Verse.RS_BuildableDef"),
            ),
            (
                "RS_Mod.RS_AmmoDef".to_owned(),
                TypeInfo::with_base("Verse.RS_ThingDef"),
            ),
        ])
        .unwrap(),
    )
}

fn synth_def(i: usize, parent: Option<&str>, name: Option<&str>, tag: &str) -> NodeBuilder {
    let mut b = NodeBuilder::new(tag);
    if let Some(n) = name {
        b = b.attr("Name", n).attr("Abstract", "True");
    }
    if let Some(p) = parent {
        b = b.attr("ParentName", p);
    }
    if name.is_none() {
        b = b.text_elem("defName", format!("RS_Def{i}"));
    }
    b = b.text_elem("label", format!("rs item {i}")).text_elem(
        "description",
        "A fictional item used for load tests. It has no meaning.",
    );
    let mut stats = NodeBuilder::new("statBases");
    for (k, s) in [
        "RS_Hp",
        "RS_Mass",
        "RS_Beauty",
        "RS_Cost",
        "RS_Work",
        "RS_Flam",
        "RS_Cool",
        "RS_Acc1",
        "RS_Acc2",
        "RS_Acc3",
    ]
    .iter()
    .enumerate()
    {
        stats = stats.text_elem(*s, format!("{}", (i + k) % 97));
    }
    b = b.child(stats);
    let mut comps = NodeBuilder::new("comps");
    for k in 0..3 {
        comps = comps.child(
            NodeBuilder::new("li")
                .attr("Class", format!("RS_Comp{k}"))
                .text_elem("rsField", format!("{}", i % 13)),
        );
    }
    b = b.child(comps);
    let mut tools = NodeBuilder::new("tools");
    for k in 0..2 {
        tools = tools.child(
            NodeBuilder::new("li")
                .text_elem("label", format!("tool {k}"))
                .text_elem("power", "5")
                .text_elem("cooldown", "2")
                .child(NodeBuilder::new("capacities").text_elem("li", "RS_Cut")),
        );
    }
    b.child(tools)
}

/// A load input at the given scale: `defs` concrete defs under chains of three abstract bases, spread
/// over three mods and files of about nine nodes, plus `patch_ops` patch operations of the shapes the
/// real corpus uses (`Defs/Type[defName="x"]` with Add, Replace, AddModExtension and Conditional).
pub fn synth(defs: usize, patch_ops: usize) -> LoadInput {
    let mods = vec![
        ModEntry::new(ModIdx(0), "ludeon.rimworld", "RS Core"),
        ModEntry::new(ModIdx(1), "rs.two", "RS Two"),
        ModEntry::new(ModIdx(2), "rs.three", "RS Three"),
    ];
    let groups = (defs / 30).max(1);
    let mut nodes: Vec<(u32, NodeBuilder)> = Vec::new();
    let mut leaf = 0usize;
    for g in 0..groups {
        let m = u32::try_from(g % 3).unwrap();
        nodes.push((
            m,
            synth_def(g, None, Some(&format!("RS_Base{g}_0")), "RS_ThingDef"),
        ));
        nodes.push((
            m,
            synth_def(
                g,
                Some(&format!("RS_Base{g}_0")),
                Some(&format!("RS_Base{g}_1")),
                "RS_ThingDef",
            ),
        ));
        nodes.push((
            m,
            synth_def(
                g,
                Some(&format!("RS_Base{g}_1")),
                Some(&format!("RS_Base{g}_2")),
                "RS_ThingDef",
            ),
        ));
        let per = defs / groups + usize::from(g < defs % groups);
        for _ in 0..per {
            let tag = if leaf.is_multiple_of(50) {
                "RS_Mod.RS_AmmoDef"
            } else {
                "RS_ThingDef"
            };
            nodes.push((
                m,
                synth_def(leaf, Some(&format!("RS_Base{g}_2")), None, tag),
            ));
            leaf += 1;
        }
    }
    let mut def_files = Vec::new();
    let mut files = 0u32;
    for m in 0..3u32 {
        let mine: Vec<NodeBuilder> = nodes
            .iter()
            .filter(|(mm, _)| *mm == m)
            .map(|(_, n)| n.clone())
            .collect();
        for chunk in mine.chunks(9) {
            def_files.push(DefFile {
                mod_idx: ModIdx(m),
                file: FileId(files),
                rel_path: format!("Defs/RS_{files}.xml"),
                content: FileContent::parsed(
                    NodeBuilder::new("Defs").children(chunk.to_vec()).build(),
                ),
            });
            files += 1;
        }
    }
    let mut patch_files = Vec::new();
    let mut ops: Vec<NodeBuilder> = Vec::new();
    for k in 0..patch_ops {
        // every 50th def is an ammo def (another element name), so skip those targets
        let mut n = (k * 7) % defs.max(1);
        if n.is_multiple_of(50) {
            n += 1;
        }
        let target = format!("Defs/RS_ThingDef[defName=\"RS_Def{n}\"]");
        let op = match k % 4 {
            0 => NodeBuilder::new("Operation")
                .attr("Class", "PatchOperationAdd")
                .text_elem("xpath", format!("{target}/statBases"))
                .child(NodeBuilder::new("value").text_elem("RS_Bulk", "1")),
            1 => NodeBuilder::new("Operation")
                .attr("Class", "PatchOperationReplace")
                .text_elem("xpath", format!("{target}/label"))
                .child(NodeBuilder::new("value").text_elem("label", format!("patched {k}"))),
            2 => NodeBuilder::new("Operation")
                .attr("Class", "PatchOperationAddModExtension")
                .text_elem("xpath", target.clone())
                .child(
                    NodeBuilder::new("value").child(NodeBuilder::new("li").attr("Class", "RS_Ext")),
                ),
            _ => NodeBuilder::new("Operation")
                .attr("Class", "PatchOperationConditional")
                .text_elem("xpath", format!("{target}/comps"))
                .child(
                    NodeBuilder::new("match")
                        .attr("Class", "PatchOperationAdd")
                        .text_elem("xpath", format!("{target}/comps"))
                        .child(
                            NodeBuilder::new("value")
                                .child(NodeBuilder::new("li").attr("Class", "RS_Added")),
                        ),
                ),
        };
        ops.push(op);
    }
    for (i, chunk) in ops.chunks(40).enumerate() {
        patch_files.push(PatchFile {
            mod_idx: ModIdx(2),
            file: FileId(files + u32::try_from(i).unwrap()),
            rel_path: format!("Patches/RS_{i}.xml"),
            content: FileContent::parsed(
                NodeBuilder::new("Patch").children(chunk.to_vec()).build(),
            ),
        });
    }
    let mut input = LoadInput::new(mods, synth_types());
    input.def_files = def_files;
    input.patch_files = patch_files;
    input
}
