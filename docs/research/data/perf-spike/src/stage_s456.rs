//! Stages S4 (persisted manifest, incremental re-scan, cache formats), S5 (peak RSS with and without string
//! interning) and S6 (thread scaling, internal vs external drive, warm vs approximate cold).

use crate::about::decode;
use crate::defs::*;
use crate::modscan::*;
use crate::roots;
use crate::stage_s3::drop_caches;
use crate::util::*;
use crate::walk::{Entry, make_pool};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, PartialEq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct FileRec {
    pub path: String,
    pub size: u64,
    pub mtime_ns: i64,
    pub defs: Vec<DefRec>,
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ModRec {
    pub dir: String,
    pub files: Vec<FileRec>,
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub mods: Vec<ModRec>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub reparsed: u64,
    pub reused: u64,
    pub bytes_read: u64,
}

fn is_def_entry(e: &Entry) -> bool {
    let n = e.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !n.starts_with('.') && n.to_ascii_lowercase().ends_with(".xml") && e.path.components().any(|c| c.as_os_str() == "Defs")
}

fn parse_file(path: &Path, size: u64, buf: &mut Vec<u8>) -> Option<Vec<DefRec>> {
    let mut f = std::fs::File::open(path).ok()?;
    buf.clear();
    buf.reserve(size as usize);
    f.read_to_end(buf).ok()?;
    let text = decode(buf);
    let mut defs = Vec::new();
    let mut emit = |d: DefRef| defs.push(d.into_owned_rec());
    let _ = Parser::Scan.run(&text, &mut emit);
    Some(defs)
}

/// Scan one mod; reuse records of `prev` whose (path, size, mtime) is unchanged.
fn scan_mod_rec(dir: &Path, prev: Option<ModRec>, buf: &mut Vec<u8>, st: &mut Stats) -> ModRec {
    let ms = scan_mod(dir, &Scope::Superset, &["Defs"]);
    let mut old: FxHashMap<String, FileRec> = prev.map(|p| p.files.into_iter().map(|f| (f.path.clone(), f)).collect()).unwrap_or_default();
    let mut files = vec![];
    for e in ms.files.iter().filter(|e| is_def_entry(e)) {
        let rel = e.path.strip_prefix(dir).unwrap_or(&e.path).to_string_lossy().into_owned();
        if let Some(o) = old.remove(&rel) {
            if o.size == e.size && o.mtime_ns == e.mtime_ns {
                st.reused += 1;
                files.push(o);
                continue;
            }
        }
        st.reparsed += 1;
        st.bytes_read += e.size;
        let defs = parse_file(&e.path, e.size, buf).unwrap_or_default();
        files.push(FileRec { path: rel, size: e.size, mtime_ns: e.mtime_ns, defs });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    ModRec { dir: dir.to_string_lossy().into_owned(), files }
}

pub fn build(dirs: &[PathBuf], prev: Option<Manifest>, pool: &rayon::ThreadPool) -> (Manifest, Stats) {
    let mut old: HashMap<String, ModRec> = prev.map(|m| m.mods.into_iter().map(|r| (r.dir.clone(), r)).collect()).unwrap_or_default();
    let jobs: Vec<(&PathBuf, Option<ModRec>)> = dirs.iter().map(|d| (d, old.remove(&d.to_string_lossy().to_string()))).collect();
    let res: Vec<(ModRec, Stats)> = pool.install(|| {
        jobs.into_par_iter()
            .map_init(
                || Vec::<u8>::with_capacity(1 << 16),
                |buf, (d, p)| {
                    let mut st = Stats::default();
                    let r = scan_mod_rec(d, p, buf, &mut st);
                    (r, st)
                },
            )
            .collect()
    });
    let mut st = Stats::default();
    let mut mods = vec![];
    for (m, s) in res {
        st.reparsed += s.reparsed;
        st.reused += s.reused;
        st.bytes_read += s.bytes_read;
        mods.push(m);
    }
    (Manifest { version: 1, mods }, st)
}

fn mod_dirs_of(names: &[String]) -> Vec<PathBuf> {
    let mut v = vec![];
    for r in roots::select(names) {
        v.extend(roots::mod_dirs(&r));
    }
    v
}

fn count_defs(m: &Manifest) -> usize {
    m.mods.iter().flat_map(|x| x.files.iter()).map(|f| f.defs.len()).sum()
}

// ---------------------------------------------------------------------------------------------
// Compact JSON layout: one string table plus integer tuples
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
pub struct Compact {
    pub v: u32,
    pub strings: Vec<String>,
    /// [dir string idx, first file, file count]
    pub mods: Vec<[u32; 3]>,
    /// [path string idx, first def, def count, size lo, size hi, mtime lo, mtime hi]
    pub files: Vec<[u64; 5]>,
    /// [tag, class, defName, name, parent, flags, offset]
    pub defs: Vec<[u32; 7]>,
}

pub fn to_compact(m: &Manifest) -> Compact {
    let mut c = Compact { v: 1, ..Default::default() };
    let mut idx: FxHashMap<&str, u32> = FxHashMap::default();
    fn intern<'a>(s: &'a str, idx: &mut FxHashMap<&'a str, u32>, strings: &mut Vec<String>) -> u32 {
        if let Some(i) = idx.get(s) {
            return *i;
        }
        let i = strings.len() as u32;
        strings.push(s.to_string());
        idx.insert(s, i);
        i
    }
    for md in &m.mods {
        let d = intern(&md.dir, &mut idx, &mut c.strings);
        c.mods.push([d, c.files.len() as u32, md.files.len() as u32]);
        for f in &md.files {
            let p = intern(&f.path, &mut idx, &mut c.strings);
            c.files.push([p as u64, c.defs.len() as u64, f.size, f.mtime_ns as u64, f.defs.len() as u64]);
            for d in &f.defs {
                let t = intern(&d.tag, &mut idx, &mut c.strings);
                let cl = intern(&d.class, &mut idx, &mut c.strings);
                let dn = intern(&d.def_name, &mut idx, &mut c.strings);
                let n = intern(&d.name, &mut idx, &mut c.strings);
                let pa = intern(&d.parent, &mut idx, &mut c.strings);
                c.defs.push([t, cl, dn, n, pa, d.is_abstract as u32, d.offset]);
            }
        }
    }
    c
}

pub fn from_compact(c: &Compact) -> Manifest {
    let s = |i: u32| c.strings[i as usize].clone();
    let mut m = Manifest { version: c.v, mods: vec![] };
    for md in &c.mods {
        let mut files = vec![];
        for f in &c.files[md[1] as usize..(md[1] + md[2]) as usize] {
            let defs = c.defs[f[1] as usize..(f[1] + f[4]) as usize]
                .iter()
                .map(|d| DefRec { tag: s(d[0]), class: s(d[1]), def_name: s(d[2]), name: s(d[3]), parent: s(d[4]), is_abstract: d[5] != 0, offset: d[6] })
                .collect();
            files.push(FileRec { path: s(f[0] as u32), size: f[2], mtime_ns: f[3] as i64, defs });
        }
        m.mods.push(ModRec { dir: s(md[0]), files });
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Formats
// ---------------------------------------------------------------------------------------------

pub const FORMATS: &[&str] = &["json-serde_json", "json-sonic-rs", "json-simd-json", "compact-serde_json", "compact-sonic-rs", "bincode", "rkyv-full", "rkyv-access"];

fn encode(fmt: &str, m: &Manifest) -> Vec<u8> {
    match fmt {
        "json-serde_json" => serde_json::to_vec(m).unwrap(),
        "json-sonic-rs" => sonic_rs::to_vec(m).unwrap(),
        "json-simd-json" => simd_json::to_vec(m).unwrap(),
        "compact-serde_json" => serde_json::to_vec(&to_compact(m)).unwrap(),
        "compact-sonic-rs" => sonic_rs::to_vec(&to_compact(m)).unwrap(),
        "bincode" => bincode::encode_to_vec(m, bincode::config::standard()).unwrap(),
        "rkyv-full" | "rkyv-access" => rkyv::to_bytes::<rkyv::rancor::Error>(m).unwrap().to_vec(),
        _ => panic!("fmt"),
    }
}

/// Decode from bytes; the returned box keeps the decoded structure alive.
fn decode_fmt(fmt: &str, b: &[u8]) -> Box<dyn std::any::Any> {
    match fmt {
        "json-serde_json" => Box::new(serde_json::from_slice::<Manifest>(b).unwrap()),
        "json-sonic-rs" => Box::new(sonic_rs::from_slice::<Manifest>(b).unwrap()),
        "json-simd-json" => {
            let mut c = b.to_vec();
            Box::new(simd_json::serde::from_slice::<Manifest>(&mut c).unwrap())
        }
        "compact-serde_json" => Box::new(serde_json::from_slice::<Compact>(b).unwrap()),
        "compact-sonic-rs" => Box::new(sonic_rs::from_slice::<Compact>(b).unwrap()),
        "bincode" => Box::new(bincode::decode_from_slice::<Manifest, _>(b, bincode::config::standard()).unwrap().0),
        "rkyv-full" => {
            let mut v = rkyv::util::AlignedVec::<16>::new();
            v.extend_from_slice(b);
            Box::new(rkyv::from_bytes::<Manifest, rkyv::rancor::Error>(&v).unwrap())
        }
        "rkyv-access" => {
            let mut v = rkyv::util::AlignedVec::<16>::new();
            v.extend_from_slice(b);
            rkyv::access::<ArchivedManifest, rkyv::rancor::Error>(&v).unwrap();
            Box::new(v)
        }
        _ => panic!("fmt"),
    }
}

fn pick_changed(m: &Manifest, every: usize) -> Vec<usize> {
    (0..m.mods.len()).filter(|i| i % every == every / 2 && !m.mods[*i].files.is_empty()).collect()
}

fn touch_mods(m: &mut Manifest, which: &[usize]) {
    for &i in which {
        for f in &mut m.mods[i].files {
            f.mtime_ns += 1;
        }
    }
}

pub fn run_s4(a: &Args) {
    let names = if a.list("roots").is_empty() { vec!["workshop".to_string(), "data".to_string()] } else { a.list("roots") };
    let runs = a.usize("runs", 5);
    let dir = PathBuf::from(a.get("dir").expect("--dir <cache dir>"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rec = Recorder::new(a.get("out"), "S4");
    rec.dataset = names.join("+");
    let pool = make_pool(16);
    let dirs = mod_dirs_of(&names);
    let params = json!({"mods": dirs.len()});

    // 1. full build (no manifest)
    let mut walls = vec![];
    let mut manifest = Manifest::default();
    let mut st0 = Stats::default();
    for r in 0..runs {
        let (res, s) = timed(|| build(&dirs, None, &pool));
        rec.record("full-build", r, &s, params.clone(), json!({"reparsed": res.1.reparsed}), 0.0, false);
        walls.push(ms(s.wall));
        manifest = res.0;
        st0 = res.1;
    }
    let (med, mn, mx) = summarize(walls);
    println!("S4 full build: {:.1} ms median (min {:.1}, max {:.1}); files parsed {}, defs {}, bytes read {:.1} MB", med, mn, mx, st0.reparsed, count_defs(&manifest), st0.bytes_read as f64 / 1e6);

    // 2. warm re-scan scenarios (manifest in memory)
    let one = {
        // one mid-sized mod: the mod whose file count is the median among non-empty mods
        let mut v: Vec<(usize, usize)> = manifest.mods.iter().enumerate().filter(|(_, m)| !m.files.is_empty()).map(|(i, m)| (m.files.len(), i)).collect();
        v.sort();
        vec![v[v.len() / 2].1]
    };
    let pct1 = pick_changed(&manifest, 100);
    let scenarios: Vec<(&str, Vec<usize>)> = vec![("unchanged", vec![]), ("one-mod-changed", one.clone()), ("1pct-mods-changed", pct1.clone())];
    println!("S4 warm re-scan from in-memory manifest (changed mods: one={}, 1pct={})", one.len(), pct1.len());
    for (name, which) in &scenarios {
        let mut walls = vec![];
        let mut last = Stats::default();
        for r in 0..runs {
            let mut m = manifest.clone();
            touch_mods(&mut m, which);
            let (res, s) = timed(|| build(&dirs, Some(m), &pool));
            assert_eq!(count_defs(&res.0), count_defs(&manifest));
            rec.record(&format!("rescan:{name}"), r, &s, params.clone(), json!({"reparsed": res.1.reparsed, "reused": res.1.reused}), 0.0, false);
            walls.push(ms(s.wall));
            last = res.1;
        }
        let (med, mn, mx) = summarize(walls);
        println!("  {:<20} {:>8.1} ms median (min {:.1}, max {:.1}); reparsed {} files ({:.2} MB), reused {}", name, med, mn, mx, last.reparsed, last.bytes_read as f64 / 1e6, last.reused);
    }

    // 3. formats
    println!("S4 formats (manifest: {} mods, {} defs)", manifest.mods.len(), count_defs(&manifest));
    println!("  {:<20} {:>10} {:>11} {:>11}", "format", "bytes", "encode ms", "decode ms");
    let exe = std::env::current_exe().unwrap();
    for fmt in FORMATS {
        let mut enc = vec![];
        let mut bytes = vec![];
        for _ in 0..runs {
            let t = std::time::Instant::now();
            bytes = encode(fmt, &manifest);
            enc.push(ms(t.elapsed()));
        }
        let mut dec = vec![];
        for _ in 0..runs {
            let t = std::time::Instant::now();
            let x = decode_fmt(fmt, &bytes);
            dec.push(ms(t.elapsed()));
            drop(x);
        }
        let (em, _, _) = summarize(enc);
        let (dm, dmin, dmax) = summarize(dec);
        let path = dir.join(format!("manifest.{fmt}"));
        std::fs::write(&path, &bytes).unwrap();
        println!("  {:<20} {:>10} {:>11.1} {:>11.1}  (min {:.1} max {:.1})", fmt, bytes.len(), em, dm, dmin, dmax);
        rec.record(&format!("fmt:{fmt}"), 0, &Sample::default(), params.clone(), json!({"bytes": bytes.len(), "encode_ms": em, "decode_ms": dm}), 0.0, false);
        // child process: load from file, report RSS
        let out = std::process::Command::new(&exe).args(["s4-load", "--fmt", fmt, "--file", path.to_str().unwrap(), "--runs", &runs.to_string()]).output().unwrap();
        print!("      {}", String::from_utf8_lossy(&out.stdout));
    }
    let t = std::time::Instant::now();
    let c = to_compact(&manifest);
    let t_to = ms(t.elapsed());
    let t = std::time::Instant::now();
    let back = from_compact(&c);
    println!("  compact: to_compact {:.1} ms, from_compact (expand to rows) {:.1} ms, roundtrip equal: {}, strings in table: {}", t_to, ms(t.elapsed()), back == manifest, c.strings.len());
    // warm start = load + rescan unchanged, for the JSON baseline
    let bytes = std::fs::read(dir.join("manifest.json-serde_json")).unwrap();
    let mut walls = vec![];
    for _ in 0..runs {
        let t = std::time::Instant::now();
        let m: Manifest = serde_json::from_slice(&bytes).unwrap();
        let (res, _) = build(&dirs, Some(m), &pool);
        walls.push(ms(t.elapsed()));
        drop(res);
    }
    let (med, mn, mx) = summarize(walls);
    println!("S4 warm start end to end (read JSON manifest with serde_json + re-scan nothing changed): {:.1} ms median (min {:.1}, max {:.1})", med, mn, mx);
}

pub fn run_s4_load(a: &Args) {
    let fmt = a.get("fmt").unwrap().to_string();
    let file = a.get("file").unwrap();
    let runs = a.usize("runs", 5);
    let (rss0, _, _, _) = proc_status_kib();
    let mut walls = vec![];
    let mut held: Option<Box<dyn std::any::Any>> = None;
    let mut rss_after = 0;
    for i in 0..runs {
        let t = std::time::Instant::now();
        let b = std::fs::read(file).unwrap();
        let x = decode_fmt(&fmt, &b);
        walls.push(ms(t.elapsed()));
        if i == 0 {
            drop(b);
            let (r, _, _, _) = proc_status_kib();
            rss_after = r;
            held = Some(x);
        }
    }
    let (_, hwm, _, _) = proc_status_kib();
    let (med, mn, mx) = summarize(walls);
    println!("load incl. file read: {:.1} ms median (min {:.1} max {:.1}); RSS after first load held {:.1} MB over baseline {:.1} MB; peak RSS {:.1} MB", med, mn, mx, (rss_after.saturating_sub(rss0)) as f64 / 1024.0, rss0 as f64 / 1024.0, hwm as f64 / 1024.0);
    drop(held);
}

// ---------------------------------------------------------------------------------------------
// S5: memory
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct IDef {
    _tag: lasso::Spur,
    _class: lasso::Spur,
    _def_name: lasso::Spur,
    _name: lasso::Spur,
    _parent: lasso::Spur,
    _abs: bool,
    _offset: u32,
    _file: u32,
}

pub fn run_s5(a: &Args) {
    let names = if a.list("roots").is_empty() { vec!["workshop".to_string(), "data".to_string()] } else { a.list("roots") };
    let runs = a.usize("runs", 3);
    let exe = std::env::current_exe().unwrap();
    println!("S5 peak RSS of the full def index (separate process per run; baseline = RSS before the build)");
    println!("  {:<22} {:>9} {:>11} {:>11} {:>10} {:>10}", "mode", "defs", "held MB", "peak MB", "build ms", "baseline MB");
    for mode in ["discard", "owned-strings", "interned", "interned-1thread"] {
        let mut rows = vec![];
        for _ in 0..runs {
            let out = std::process::Command::new(&exe).args(["s5-child", "--mode", mode, "--roots", &names.join(",")]).output().unwrap();
            let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let v: Vec<f64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() == 5 {
                rows.push(v);
            }
        }
        rows.sort_by(|x, y| x[2].partial_cmp(&y[2]).unwrap());
        if let Some(m) = rows.get(rows.len() / 2) {
            println!("  {:<22} {:>9} {:>11.1} {:>11.1} {:>10.0} {:>10.1}", mode, m[0], m[1], m[2], m[3], m[4]);
        }
    }
}

pub fn run_s5_child(a: &Args) {
    let mode = a.get("mode").unwrap().to_string();
    let names = a.list("roots");
    let dirs = mod_dirs_of(&names);
    let threads = if mode == "interned-1thread" { 1 } else { 16 };
    let pool = make_pool(threads);
    let files: Vec<Entry> = pool.install(|| dirs.par_iter().flat_map(|d| scan_mod(d, &Scope::Superset, &["Defs"]).files.into_iter().filter(is_def_entry).collect::<Vec<_>>()).collect());
    let (rss0, _, _, _) = proc_status_kib();
    let t = std::time::Instant::now();
    let rodeo = lasso::ThreadedRodeo::<lasso::Spur, rustc_hash::FxBuildHasher>::with_hasher(rustc_hash::FxBuildHasher);
    let mut total = 0usize;
    let mut keep_owned: Vec<Vec<DefRec>> = vec![];
    let mut keep_int: Vec<Vec<IDef>> = vec![];
    match mode.as_str() {
        "discard" => {
            total = pool.install(|| {
                files
                    .par_iter()
                    .map_init(Vec::new, |buf, e| {
                        let mut n = 0;
                        if let Some(d) = parse_file(&e.path, e.size, buf) {
                            n = d.len();
                        }
                        n
                    })
                    .sum()
            });
        }
        "owned-strings" => {
            keep_owned = pool.install(|| files.par_iter().map_init(Vec::new, |buf, e| parse_file(&e.path, e.size, buf).unwrap_or_default()).collect());
            total = keep_owned.iter().map(|v| v.len()).sum();
        }
        _ => {
            keep_int = pool.install(|| {
                files
                    .par_iter()
                    .enumerate()
                    .map_init(Vec::new, |buf: &mut Vec<u8>, (fi, e)| {
                        let mut out = vec![];
                        if let Ok(mut f) = std::fs::File::open(&e.path) {
                            buf.clear();
                            if f.read_to_end(buf).is_ok() {
                                let text = decode(buf);
                                let mut emit = |d: DefRef| {
                                    out.push(IDef {
                                        _tag: rodeo.get_or_intern(d.tag.as_ref()),
                                        _class: rodeo.get_or_intern(d.class.as_deref().unwrap_or("")),
                                        _def_name: rodeo.get_or_intern(d.def_name.as_deref().unwrap_or("")),
                                        _name: rodeo.get_or_intern(d.name.as_deref().unwrap_or("")),
                                        _parent: rodeo.get_or_intern(d.parent.as_deref().unwrap_or("")),
                                        _abs: d.is_abstract,
                                        _offset: d.offset,
                                        _file: fi as u32,
                                    })
                                };
                                let _ = Parser::Scan.run(&text, &mut emit);
                            }
                        }
                        out
                    })
                    .collect()
            });
            total = keep_int.iter().map(|v| v.len()).sum();
        }
    }
    let wall = ms(t.elapsed());
    let (rss1, hwm, _, _) = proc_status_kib();
    println!("{} {:.1} {:.1} {:.0} {:.1}", total, rss1.saturating_sub(rss0) as f64 / 1024.0, hwm.saturating_sub(rss0) as f64 / 1024.0, wall, rss0 as f64 / 1024.0);
    if mode.starts_with("interned") {
        eprintln!("unique strings: {}", rodeo.len());
    }
    std::hint::black_box((&keep_owned, &keep_int));
}

// ---------------------------------------------------------------------------------------------
// S6: thread scaling, drives, cold
// ---------------------------------------------------------------------------------------------

pub fn run_s6(a: &Args) {
    let sets: Vec<(String, Vec<String>)> = if a.list("sets").is_empty() {
        vec![("workshop+data".into(), vec!["workshop".into(), "data".into()]), ("owner".into(), vec!["owner".into()]), ("ce".into(), vec!["ce".into()])]
    } else {
        a.list("sets").into_iter().map(|s| (s.clone(), s.split('+').map(String::from).collect())).collect()
    };
    let threads: Vec<usize> = if a.list("threads").is_empty() { vec![1, 2, 4, 8, 16] } else { a.list("threads").iter().filter_map(|s| s.parse().ok()).collect() };
    let runs = a.usize("runs", 5);
    let mut rec = Recorder::new(a.get("out"), "S6");
    println!("S6 end to end full def index build (walk + read + parse + owned records), median ms of {runs}");
    for (label, names) in sets {
        let dirs = mod_dirs_of(&names);
        rec.dataset = label.clone();
        let pool16 = make_pool(16);
        let files: Vec<Entry> = pool16.install(|| dirs.par_iter().flat_map(|d| scan_mod(d, &Scope::Superset, &["Defs"]).files.into_iter().filter(is_def_entry).collect::<Vec<_>>()).collect());
        let bytes: u64 = files.iter().map(|e| e.size).sum();
        println!("== {label}: mods={} def files={} bytes={:.1} MB", dirs.len(), files.len(), bytes as f64 / 1e6);
        for cold in [false, true] {
            if cold && a.flag("no-cold") {
                continue;
            }
            rec.cache_state = if cold { "cold-approx".into() } else { "warm".into() };
            let mut line = format!("  {:<12}", if cold { "cold-approx" } else { "warm" });
            for &t in &threads {
                let pool = make_pool(t);
                let mut w = vec![];
                for r in 0..runs {
                    if cold {
                        drop_caches(&files);
                    }
                    let (res, s) = timed(|| build(&dirs, None, &pool));
                    rec.record(&format!("e2e@{t}"), r, &s, json!({"threads": t, "files": files.len(), "bytes": bytes}), json!({"parsed": res.1.reparsed}), 0.0, false);
                    w.push(ms(s.wall));
                }
                let (med, _, _) = summarize(w);
                line += &format!(" {t}t={med:.0}");
            }
            println!("{line}");
        }
        // walk only (pruned scan, no parse) per thread count, warm
        let mut line = format!("  {:<12}", "walk-only");
        for &t in &threads {
            let pool = make_pool(t);
            let mut w = vec![];
            for _ in 0..runs {
                let (_, s) = timed(|| pool.install(|| dirs.par_iter().map(|d| scan_mod(d, &Scope::Superset, &["Defs"]).files.len()).sum::<usize>()));
                w.push(ms(s.wall));
            }
            let (med, _, _) = summarize(w);
            line += &format!(" {t}t={med:.1}");
        }
        println!("{line}");
    }
}

/// Mod list start-up path: list mod dirs, find About.xml, read and parse it. Warm and approximate cold.
pub fn run_about_cold(a: &Args) {
    let names = if a.list("roots").is_empty() { vec!["workshop".to_string(), "data".to_string()] } else { a.list("roots") };
    let runs = a.usize("runs", 5);
    let pool = make_pool(a.usize("threads", 16));
    for cold in [false, true] {
        let mut w = vec![];
        for _ in 0..runs {
            if cold {
                let dirs = mod_dirs_of(&names);
                let files: Vec<Entry> = dirs.iter().filter_map(|d| crate::about::find_about(d)).map(|p| Entry { size: 0, mtime_ns: 0, kind: 0, path: p }).collect();
                drop_caches(&files);
            }
            let (n, s) = timed(|| {
                let dirs = mod_dirs_of(&names);
                pool.install(|| {
                    dirs.par_iter()
                        .filter_map(|d| {
                            let p = crate::about::find_about(d)?;
                            let b = std::fs::read(p).ok()?;
                            let (ab, _) = crate::about::parse_events(&crate::about::decode(&b), false);
                            Some(ab)
                        })
                        .count()
                })
            });
            w.push(ms(s.wall));
            std::hint::black_box(n);
        }
        let (med, mn, mx) = summarize(w);
        println!("mod list (list dirs + read + parse About.xml, {} threads) {}: {:.1} ms median (min {:.1}, max {:.1})", a.usize("threads", 16), if cold { "cold-approx" } else { "warm" }, med, mn, mx);
    }
}
