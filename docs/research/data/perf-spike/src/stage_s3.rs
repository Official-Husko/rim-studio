//! Driver for S3 (def index build) and for the page cache experiments around it (S7).

use crate::about::decode;
use crate::defs::*;
use crate::modscan::*;
use crate::roots;
use crate::util::*;
use crate::walk::{Entry, make_pool};
use rayon::prelude::*;
use serde_json::json;
use std::io::Read;
use std::path::PathBuf;

pub struct FileSet {
    pub files: Vec<Entry>,
    pub mods: usize,
    pub scanned_files: u64,
}

/// Build the list of def XML files with the pruned scanner (parallel over mods).
pub fn collect_def_files(root_names: &[String], scope: &str, pool: &rayon::ThreadPool) -> FileSet {
    let active = ActiveSet::from_modsconfig(&modsconfig_path());
    let mut mod_dirs: Vec<PathBuf> = vec![];
    for r in roots::select(root_names) {
        mod_dirs.extend(roots::mod_dirs(&r));
    }
    let scopes = match scope {
        "resolved" => Scope::Resolved { active: &active, game: (1, 6) },
        _ => Scope::Superset,
    };
    let dedupe = scope == "resolved";
    let scans: Vec<ModScan> = pool.install(|| mod_dirs.par_iter().map(|m| scan_mod(m, &scopes, &["Defs"])).collect());
    let mut files = vec![];
    let mut scanned = 0u64;
    for ms in &scans {
        scanned += ms.files.len() as u64;
        if dedupe {
            files.extend(def_files(ms).into_iter().cloned());
        } else {
            files.extend(
                ms.files
                    .iter()
                    .filter(|e| {
                        let n = e.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        !n.starts_with('.') && n.to_ascii_lowercase().ends_with(".xml") && e.path.components().any(|c| c.as_os_str() == "Defs")
                    })
                    .cloned(),
            );
        }
    }
    FileSet { files, mods: mod_dirs.len(), scanned_files: scanned }
}

pub fn drop_caches(files: &[Entry]) -> usize {
    files.par_iter().filter(|e| fadvise_dontneed(&e.path)).count()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ReadMode {
    Fs,
    Reuse,
    Mmap,
}

impl ReadMode {
    pub fn name(self) -> &'static str {
        match self {
            ReadMode::Fs => "fs::read",
            ReadMode::Reuse => "reuse-buf",
            ReadMode::Mmap => "mmap",
        }
    }
}

/// Read one file and run `f` on its decoded text.
pub fn with_text<R>(e: &Entry, mode: ReadMode, buf: &mut Vec<u8>, f: impl FnOnce(&str) -> R) -> Option<R> {
    match mode {
        ReadMode::Fs => {
            let b = std::fs::read(&e.path).ok()?;
            Some(f(&decode(&b)))
        }
        ReadMode::Reuse => {
            let mut file = std::fs::File::open(&e.path).ok()?;
            buf.clear();
            buf.resize(e.size as usize, 0);
            file.read_exact(buf).ok()?;
            Some(f(&decode(buf)))
        }
        ReadMode::Mmap => {
            let file = std::fs::File::open(&e.path).ok()?;
            if e.size == 0 {
                return Some(f(""));
            }
            let m = unsafe { memmap2::Mmap::map(&file).ok()? };
            Some(f(&decode(&m)))
        }
    }
}

pub fn run(a: &Args) {
    let root_names = if a.list("roots").is_empty() { vec!["workshop".to_string(), "data".to_string()] } else { a.list("roots") };
    let scope = a.get("scope").unwrap_or("superset").to_string();
    let runs = a.usize("runs", 7);
    let threads: Vec<usize> = if a.list("threads").is_empty() { vec![1, 16] } else { a.list("threads").iter().filter_map(|s| s.parse().ok()).collect() };
    let parsers: Vec<Parser> = if a.list("parsers").is_empty() {
        vec![Parser::Roxmltree, Parser::QuickXml, Parser::Scan]
    } else {
        a.list("parsers").iter().filter_map(|s| Parser::from_name(s)).collect()
    };
    let cold = a.flag("cold");
    let mut rec = Recorder::new(a.get("out"), "S3");
    rec.dataset = format!("{}|{}", root_names.join("+"), scope);
    rec.cache_state = if cold { "cold-approx".into() } else { "warm".into() };
    let pool16 = make_pool(16);

    let fs = collect_def_files(&root_names, &scope, &pool16);
    let total_bytes: u64 = fs.files.iter().map(|e| e.size).sum();
    println!(
        "== S3 roots={} scope={} mods={} def_files={} bytes={:.1}MB (scanned {} files)",
        root_names.join("+"),
        scope,
        fs.mods,
        fs.files.len(),
        total_bytes as f64 / 1e6,
        fs.scanned_files
    );
    if a.flag("list-only") {
        return;
    }

    // Load all text once for the parse-only measurements and the agreement check.
    let texts: Vec<String> = fs.files.par_iter().map(|e| std::fs::read(&e.path).map(|b| decode(&b).into_owned()).unwrap_or_default()).collect();
    let text_bytes: usize = texts.iter().map(|t| t.len()).sum();

    // ---- agreement check ----
    let mut counts: Vec<(Parser, usize, usize, usize, usize)> = vec![];
    let mut reference: Option<Vec<Vec<DefRec>>> = None;
    for p in &parsers {
        let res: Vec<(Vec<DefRec>, Result<ScanInfo, String>)> = pool16.install(|| texts.par_iter().map(|t| collect(t, *p)).collect());
        let defs: usize = res.iter().map(|(v, _)| v.len()).sum();
        let errors = res.iter().filter(|(_, r)| r.is_err()).count();
        let with_name = res.iter().flat_map(|(v, _)| v.iter()).filter(|d| !d.def_name.is_empty()).count();
        let mut differ = 0usize;
        let recs: Vec<Vec<DefRec>> = res.into_iter().map(|(v, _)| v).collect();
        if let Some(r) = &reference {
            for (i, (x, y)) in recs.iter().zip(r.iter()).enumerate() {
                if x != y {
                    differ += 1;
                    if a.flag("show-diffs") && differ <= 5 {
                        println!("  {} differs from {} on {}: {} vs {} defs", p.name(), parsers[0].name(), fs.files[i].path.display(), x.len(), y.len());
                    }
                }
            }
        } else {
            reference = Some(recs);
        }
        counts.push((*p, defs, with_name, errors, differ));
    }
    println!("  parsers: defs / with defName / files with parse error / files differing from {}", parsers[0].name());
    for (p, defs, wn, er, df) in &counts {
        println!("    {:<10} defs={defs} with_defName={wn} errors={er} differs={df}", p.name());
        rec.record(&format!("check:{}", p.name()), 0, &Sample::default(), json!({"files": fs.files.len()}), json!({"defs": defs, "with_defname": wn, "error_files": er, "differs": df}), 0.0, false);
    }
    let total_defs = counts.first().map(|c| c.1).unwrap_or(0);

    // ---- parse only (in memory) ----
    println!("  A) parse only (text in memory), owned DefRec output");
    let pools: std::collections::HashMap<usize, _> = threads.iter().map(|&t| (t, make_pool(t))).collect();
    let mut variants: Vec<(String, (Parser, usize))> = vec![];
    for p in &parsers {
        for &t in &threads {
            variants.push((format!("{}@{}", p.name(), t), (*p, t)));
        }
    }
    let params = json!({"mode": "parse-only", "files": fs.files.len(), "bytes": text_bytes, "defs": total_defs});
    interleaved(
        &mut rec,
        &variants,
        runs,
        &params,
        |_| {},
        |_, (p, t)| {
            pools[t].install(|| {
                texts
                    .par_iter()
                    .map(|txt| {
                        let mut n = 0usize;
                        let mut v: Vec<DefRec> = Vec::new();
                        let mut f = |d: DefRef| {
                            v.push(d.into_owned_rec());
                            n += 1;
                        };
                        let _ = p.run(txt, &mut f);
                        v
                    })
                    .map(|v| v.len())
                    .sum::<usize>()
            })
        },
        |_, _| json!({}),
    );

    println!("  A2) parse only, borrowed (no allocation of records)");
    let params = json!({"mode": "parse-only-borrowed", "files": fs.files.len(), "bytes": text_bytes, "defs": total_defs});
    interleaved(
        &mut rec,
        &variants,
        runs,
        &params,
        |_| {},
        |_, (p, t)| {
            pools[t].install(|| {
                texts
                    .par_iter()
                    .map(|txt| {
                        let mut n = 0usize;
                        let mut f = |d: DefRef| {
                            n += d.tag.len() & 1;
                        };
                        let _ = p.run(txt, &mut f);
                        n
                    })
                    .sum::<usize>()
            })
        },
        |_, _| json!({}),
    );

    // ---- end to end ----
    println!("  B) end to end: read + decode + parse + owned records ({})", rec.cache_state);
    let files = fs.files.clone();
    let params = json!({"mode": "end-to-end", "files": files.len(), "bytes": total_bytes, "defs": total_defs});
    interleaved(
        &mut rec,
        &variants,
        runs,
        &params,
        |_| {
            if cold {
                drop_caches(&files);
            }
        },
        |_, (p, t)| {
            pools[t].install(|| {
                files
                    .par_iter()
                    .map_init(
                        || Vec::<u8>::with_capacity(1 << 16),
                        |buf, e| {
                            with_text(e, ReadMode::Reuse, buf, |txt| {
                                let mut v: Vec<DefRec> = Vec::new();
                                let mut f = |d: DefRef| v.push(d.into_owned_rec());
                                let _ = p.run(txt, &mut f);
                                v
                            })
                            .unwrap_or_default()
                        },
                    )
                    .map(|v| v.len())
                    .sum::<usize>()
            })
        },
        |_, _| json!({}),
    );

    // ---- read strategy for the fastest parser ----
    let best = Parser::Scan;
    println!("  C) read strategy with parser={} (end to end, {} threads)", best.name(), threads.last().unwrap());
    let t = *threads.last().unwrap();
    let rvariants: Vec<(String, ReadMode)> = vec![ReadMode::Fs, ReadMode::Reuse, ReadMode::Mmap].into_iter().map(|m| (m.name().to_string(), m)).collect();
    let params = json!({"mode": "read-strategy", "parser": best.name(), "threads": t, "files": files.len(), "bytes": total_bytes});
    interleaved(
        &mut rec,
        &rvariants,
        runs,
        &params,
        |_| {
            if cold {
                drop_caches(&files);
            }
        },
        |_, m| {
            pools[&t].install(|| {
                files
                    .par_iter()
                    .map_init(
                        || Vec::<u8>::with_capacity(1 << 16),
                        |buf, e| {
                            with_text(e, *m, buf, |txt| {
                                let mut n = 0usize;
                                let mut f = |d: DefRef| n += d.tag.len() & 1;
                                let _ = best.run(txt, &mut f);
                                n
                            })
                            .unwrap_or(0)
                        },
                    )
                    .sum::<usize>()
            })
        },
        |_, _| json!({}),
    );

    // ---- slowest files for each parser ----
    println!("  D) slowest files (single thread, best of 3, parse only)");
    for p in &parsers {
        let mut times: Vec<(f64, usize, usize)> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut best = f64::MAX;
                for _ in 0..3 {
                    let t0 = std::time::Instant::now();
                    let mut n = 0usize;
                    let mut f = |d: DefRef| n += d.tag.len() & 1;
                    let _ = p.run(t, &mut f);
                    best = best.min(t0.elapsed().as_secs_f64() * 1000.0);
                }
                (best, i, t.len())
            })
            .collect();
        times.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let sum_ms: f64 = times.iter().map(|x| x.0).sum();
        println!("    {} total {:.1} ms; top 3:", p.name(), sum_ms);
        for (ms_, i, len) in times.iter().take(3) {
            let rel = fs.files[*i].path.to_string_lossy().replace(&format!("{}/", std::env::var("HOME").unwrap_or_default()), "~/");
            println!("      {:8.2} ms {:>9} B  {}", ms_, len, rel);
            rec.record(&format!("slowest:{}", p.name()), 0, &Sample::default(), json!({"path": rel, "bytes": len}), json!({"ms": ms_}), 0.0, false);
        }
        let mut top1pct = times.iter().take((times.len() / 100).max(1)).map(|x| x.0).sum::<f64>();
        top1pct = top1pct / sum_ms * 100.0;
        println!("      slowest 1% of files = {:.1}% of total parse time", top1pct);
    }
}
