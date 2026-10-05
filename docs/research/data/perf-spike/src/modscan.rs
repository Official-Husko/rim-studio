//! RimWorld-aware scanning: LoadFolders.xml, folder resolution that mirrors the game's rules
//! (decompiled:Verse/ModContentPack.cs InitLoadFolders, Verse/LoadFolder.cs ShouldLoad), and pruned walks
//! that only descend into the folders the tools care about.

use crate::util::mtime_ns;
use crate::walk::Entry;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------------
// Active mod set (ModsConfig.xml)
// ---------------------------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct ActiveSet(pub FxHashSet<String>);

fn norm_id(id: &str) -> String {
    let l = id.trim().to_ascii_lowercase();
    l.strip_suffix("_steam").or_else(|| l.strip_suffix("_copy")).map(|s| s.to_string()).unwrap_or(l)
}

impl ActiveSet {
    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(&norm_id(id))
    }
    pub fn any(&self, ids: &[String]) -> bool {
        ids.iter().any(|i| self.contains(i))
    }
    pub fn all(&self, ids: &[String]) -> bool {
        ids.iter().all(|i| self.contains(i))
    }
    /// Reads <activeMods><li>packageId</li>...</activeMods>.
    pub fn from_modsconfig(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut set = FxHashSet::default();
        let mut rd = Reader::from_str(&text);
        let mut in_active = false;
        let mut in_li = false;
        let mut buf = String::new();
        while let Ok(ev) = rd.read_event() {
            match ev {
                Event::Start(e) => {
                    let n = e.name().into_inner();
                    if n == "activeMods" {
                        in_active = true;
                    } else if in_active && n == "li" {
                        in_li = true;
                        buf.clear();
                    }
                }
                Event::Text(t) if in_li => buf.push_str(&t.xml10_content()),
                Event::End(e) => {
                    let n = e.name().into_inner();
                    if n == "li" && in_li {
                        in_li = false;
                        set.insert(norm_id(&buf));
                    } else if n == "activeMods" {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        ActiveSet(set)
    }
}

pub fn modsconfig_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!("{home}/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml"))
}

// ---------------------------------------------------------------------------------------------
// LoadFolders.xml
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LoadFolderEntry {
    pub folder: String, // "" = mod root
    pub any_of: Vec<String>,
    pub all_of: Vec<String>,
    pub none_of: Vec<String>,
}

impl LoadFolderEntry {
    pub fn should_load(&self, active: &ActiveSet) -> bool {
        (self.any_of.is_empty() || active.any(&self.any_of))
            && (self.all_of.is_empty() || active.all(&self.all_of))
            && (self.none_of.is_empty() || !active.any(&self.none_of))
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadFolders {
    /// (version key lowercased with a leading "v" removed, entries in document order)
    pub by_version: Vec<(String, Vec<LoadFolderEntry>)>,
}

fn split_ids(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

pub fn parse_load_folders(text: &str) -> LoadFolders {
    let mut lf = LoadFolders::default();
    let mut rd = Reader::from_str(text);
    {
        let c = rd.config_mut();
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
        c.allow_dangling_amp = true;
        c.expand_empty_elements = true;
    }
    let mut depth = 0usize;
    let mut cur: Option<LoadFolderEntry> = None;
    let mut buf = String::new();
    while let Ok(ev) = rd.read_event() {
        match ev {
            Event::Start(e) => {
                depth += 1;
                if depth == 2 {
                    let mut k = e.name().into_inner().to_ascii_lowercase();
                    if let Some(r) = k.strip_prefix('v') {
                        k = r.to_string();
                    }
                    lf.by_version.push((k, vec![]));
                } else if depth == 3 {
                    let mut ent = LoadFolderEntry::default();
                    for a in e.attributes().flatten() {
                        let v = a.unescape_value().map(|c| c.into_owned()).unwrap_or_default();
                        match a.key.into_inner() {
                            "IfModActive" => ent.any_of = split_ids(&v),
                            "IfModActiveAll" => ent.all_of = split_ids(&v),
                            "IfModNotActive" => ent.none_of = split_ids(&v),
                            _ => {}
                        }
                    }
                    cur = Some(ent);
                    buf.clear();
                }
            }
            Event::Text(t) if depth == 3 => buf.push_str(&t.xml10_content()),
            Event::End(_) => {
                if depth == 3 {
                    if let (Some(mut ent), Some((_, v))) = (cur.take(), lf.by_version.last_mut()) {
                        let t = buf.trim();
                        ent.folder = if t == "/" || t == "\\" { String::new() } else { t.to_string() };
                        v.push(ent);
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    lf
}

fn parse_ver(s: &str) -> Option<(u32, u32)> {
    // the game requires at least two dot separated integers (VersionControl.TryParseVersionString)
    let mut it = s.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    Some((a, b))
}

/// Mirrors ModContentPack.InitLoadFolders. Returns folders in descending priority (relative to the
/// mod root, "" = the root itself). `dirs` are the names of the sub directories of the mod root.
pub fn resolve_folders(lf: Option<&LoadFolders>, dirs: &[String], active: &ActiveSet, game: (u32, u32)) -> Vec<String> {
    if let Some(lf) = lf.filter(|l| !l.by_version.is_empty()) {
        let add = |entries: &Vec<LoadFolderEntry>| -> Vec<String> {
            entries.iter().rev().filter(|e| e.should_load(active)).map(|e| e.folder.clone()).collect()
        };
        let exact = format!("{}.{}", game.0, game.1);
        if let Some((_, e)) = lf.by_version.iter().find(|(k, _)| *k == exact) {
            if !e.is_empty() {
                return add(e);
            }
        }
        let best = lf
            .by_version
            .iter()
            .filter(|(k, _)| k != "default" && k.contains('.'))
            .filter_map(|(k, e)| parse_ver(k).filter(|v| *v <= game).map(|v| (v, e)))
            .max_by_key(|(v, _)| *v);
        if let Some((_, e)) = best {
            return add(e);
        }
        if let Some((_, e)) = lf.by_version.iter().find(|(k, _)| k == "default") {
            return add(e);
        }
    }
    let mut out = vec![];
    let exact = format!("{}.{}", game.0, game.1);
    if dirs.iter().any(|d| *d == exact) {
        out.push(exact);
    } else {
        // highest version folder not above the game version; if all are above, the lowest one
        let mut vs: Vec<((u32, u32), &String)> = dirs.iter().filter_map(|d| parse_ver(d).map(|v| (v, d))).collect();
        vs.sort();
        let pick = vs.iter().filter(|(v, _)| *v <= game).last().or_else(|| vs.first());
        if let Some((v, _)) = pick {
            let name = format!("{}.{}", v.0, v.1);
            if dirs.iter().any(|d| *d == name) {
                out.push(name);
            }
        }
    }
    if dirs.iter().any(|d| d == "Common") {
        out.push("Common".into());
    }
    out.push(String::new());
    out
}

// ---------------------------------------------------------------------------------------------
// Per-mod pruned scan
// ---------------------------------------------------------------------------------------------

pub enum Scope<'a> {
    /// every candidate folder: root, Common, anything that starts like a version, anything named in LoadFolders.xml
    Superset,
    /// folders the game would load for `game` given the active set
    Resolved { active: &'a ActiveSet, game: (u32, u32) },
}

#[derive(Default)]
pub struct ModScan {
    pub mod_root: PathBuf,
    pub has_about: bool,
    pub has_load_folders: bool,
    pub folders: Vec<String>,
    pub files: Vec<Entry>,
    pub dirs: u64,
    pub errors: u64,
}

pub const TARGET_SUBDIRS: &[&str] = &["Defs", "Patches", "Languages", "Assemblies"];

fn version_like(name: &str) -> bool {
    let n = name.strip_prefix(['v', 'V']).unwrap_or(name);
    n.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
}

/// Recursively list all files below `dir` (serial, std), appending to `out`.
pub fn list_tree(dir: &Path, out: &mut Vec<Entry>, dirs: &mut u64, errors: &mut u64) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => {
                *errors += 1;
                continue;
            }
        };
        *dirs += 1;
        for e in rd {
            let Ok(e) = e else {
                *errors += 1;
                continue;
            };
            let Ok(ft) = e.file_type() else {
                *errors += 1;
                continue;
            };
            if ft.is_dir() {
                stack.push(e.path());
            } else {
                match e.metadata() {
                    Ok(md) => out.push(Entry { path: e.path(), size: md.len(), mtime_ns: mtime_ns(&md), kind: if ft.is_file() { 0 } else if ft.is_symlink() { 1 } else { 2 } }),
                    Err(_) => *errors += 1,
                }
            }
        }
    }
}

pub fn scan_mod(mod_root: &Path, scope: &Scope, subdirs: &[&str]) -> ModScan {
    let mut ms = ModScan { mod_root: mod_root.to_path_buf(), ..Default::default() };
    // 1. list the mod root once
    let mut dirs: Vec<String> = vec![];
    let mut load_folders_file: Option<PathBuf> = None;
    match std::fs::read_dir(mod_root) {
        Ok(rd) => {
            ms.dirs += 1;
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                match e.file_type() {
                    Ok(ft) if ft.is_dir() => dirs.push(name),
                    Ok(_) => {
                        if name.eq_ignore_ascii_case("loadfolders.xml") {
                            load_folders_file = Some(e.path());
                        }
                    }
                    Err(_) => ms.errors += 1,
                }
            }
        }
        Err(_) => {
            ms.errors += 1;
            return ms;
        }
    }
    // 2. About/
    if dirs.iter().any(|d| d == "About") {
        ms.has_about = true;
        list_tree(&mod_root.join("About"), &mut ms.files, &mut ms.dirs, &mut ms.errors);
    }
    // 3. LoadFolders.xml
    let mut lf: Option<LoadFolders> = None;
    if let Some(p) = &load_folders_file {
        ms.has_load_folders = true;
        if let Ok(md) = std::fs::metadata(p) {
            ms.files.push(Entry { path: p.clone(), size: md.len(), mtime_ns: mtime_ns(&md), kind: 0 });
        }
        if let Ok(b) = std::fs::read(p) {
            lf = Some(parse_load_folders(&crate::about::decode(&b)));
        }
    }
    // 4. candidate folders
    let folders: Vec<String> = match scope {
        Scope::Resolved { active, game } => resolve_folders(lf.as_ref(), &dirs, active, *game),
        Scope::Superset => {
            let mut f: Vec<String> = vec![String::new()];
            if dirs.iter().any(|d| d == "Common") {
                f.push("Common".into());
            }
            for d in &dirs {
                if version_like(d) {
                    f.push(d.clone());
                }
            }
            if let Some(lf) = &lf {
                for (_, es) in &lf.by_version {
                    for e in es {
                        if !f.contains(&e.folder) {
                            f.push(e.folder.clone());
                        }
                    }
                }
            }
            f
        }
    };
    // 5. descend into the target sub directories of every folder
    for folder in &folders {
        let base = if folder.is_empty() { mod_root.to_path_buf() } else { mod_root.join(folder) };
        let names: Vec<String> = if folder.is_empty() {
            dirs.clone()
        } else {
            match std::fs::read_dir(&base) {
                Ok(rd) => {
                    ms.dirs += 1;
                    rd.flatten().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).map(|e| e.file_name().to_string_lossy().into_owned()).collect()
                }
                Err(_) => continue, // folder named in LoadFolders.xml does not exist: not an error for us
            }
        };
        for sub in subdirs {
            if names.iter().any(|n| n == sub) {
                list_tree(&base.join(sub), &mut ms.files, &mut ms.dirs, &mut ms.errors);
            }
        }
    }
    // the same directory can be reached twice (LoadFolders entries that overlap, trailing slashes): keep one
    {
        let mut seen: FxHashSet<PathBuf> = FxHashSet::default();
        ms.files.retain(|e| seen.insert(e.path.clone()));
    }
    ms.folders = folders;
    ms
}

/// Game rule for def files: every *.xml below <folder>/Defs, names starting with "." skipped, the first
/// folder (highest priority) wins for the same relative path (decompiled:Verse/DirectXmlLoader.cs).
pub fn def_files(ms: &ModScan) -> Vec<&Entry> {
    let mut seen: FxHashSet<String> = FxHashSet::default();
    let mut out = vec![];
    for folder in &ms.folders {
        let base = if folder.is_empty() { ms.mod_root.clone() } else { ms.mod_root.join(folder) };
        let defs = base.join("Defs");
        for e in &ms.files {
            if let Ok(rel) = e.path.strip_prefix(&defs) {
                let name = e.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || !name.to_ascii_lowercase().ends_with(".xml") {
                    continue;
                }
                if seen.insert(rel.to_string_lossy().into_owned()) {
                    out.push(e);
                }
            }
        }
    }
    out
}

/// True for XML files that live below a directory named exactly `Defs` (a simple superset rule that
/// works on any file list, for example the output of the full walk).
pub fn is_defs_xml_path(p: &Path, mod_root_len: usize) -> bool {
    let s = p.as_os_str().to_string_lossy();
    let rel = &s[mod_root_len.min(s.len())..];
    let lower_ext_xml = rel.len() > 4 && rel.as_bytes()[rel.len() - 4..].eq_ignore_ascii_case(b".xml");
    if !lower_ext_xml {
        return false;
    }
    let mut comps = rel.split('/').filter(|c| !c.is_empty()).collect::<Vec<_>>();
    let leaf = comps.pop().unwrap_or("");
    !leaf.starts_with('.') && comps.iter().any(|c| *c == "Defs")
}
