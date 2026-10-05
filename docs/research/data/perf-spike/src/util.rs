//! Timing, resource accounting, result recording and small OS helpers (Linux only).

use serde_json::{Map, Value, json};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------------
// Resource usage
// ---------------------------------------------------------------------------------------------

pub fn rusage_self() -> (Duration, Duration, u64) {
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        let user = Duration::new(ru.ru_utime.tv_sec as u64, (ru.ru_utime.tv_usec as u32) * 1000);
        let sys = Duration::new(ru.ru_stime.tv_sec as u64, (ru.ru_stime.tv_usec as u32) * 1000);
        (user, sys, ru.ru_maxrss as u64) // ru_maxrss is in KiB on Linux
    }
}

/// (busy jiffies, total jiffies) of the whole machine from the first line of /proc/stat.
pub fn proc_stat_cpu() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let line = s.lines().next().unwrap_or("");
    let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if v.len() < 8 {
        return (0, 1);
    }
    let idle = v[3] + v[4]; // idle + iowait
    let total: u64 = v[..8].iter().sum();
    (total - idle, total)
}

pub fn loadavg() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
        .unwrap_or(f64::NAN)
}

pub fn ncpu() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

/// Fields of /proc/self/status in KiB: (VmRSS, VmHWM, RssAnon, RssFile).
pub fn proc_status_kib() -> (u64, u64, u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |key: &str| -> u64 {
        s.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|x| x.parse().ok())
            .unwrap_or(0)
    };
    (get("VmRSS:"), get("VmHWM:"), get("RssAnon:"), get("RssFile:"))
}

/// Estimate how many cores other processes keep busy: sample the machine for `window`.
pub fn background_cores(window: Duration) -> f64 {
    let (b0, t0) = proc_stat_cpu();
    std::thread::sleep(window);
    let (b1, t1) = proc_stat_cpu();
    let dt = (t1 - t0).max(1) as f64;
    (b1 - b0) as f64 / dt * ncpu() as f64
}

/// Wait (bounded) until the machine is reasonably quiet. Returns (observed background cores, timed_out).
pub fn quiet_gate(max_cores: f64, max_wait: Duration) -> (f64, bool) {
    let start = Instant::now();
    loop {
        let bg = background_cores(Duration::from_millis(150));
        if bg <= max_cores {
            return (bg, false);
        }
        if start.elapsed() > max_wait {
            return (bg, true);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub wall: Duration,
    pub user: Duration,
    pub sys: Duration,
}

pub fn timed<R>(f: impl FnOnce() -> R) -> (R, Sample) {
    let (u0, s0, _) = rusage_self();
    let t0 = Instant::now();
    let r = f();
    let wall = t0.elapsed();
    let (u1, s1, _) = rusage_self();
    (r, Sample { wall, user: u1 - u0, sys: s1 - s0 })
}

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

pub fn median(v: &mut Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

pub fn summarize(mut v: Vec<f64>) -> (f64, f64, f64) {
    let med = median(&mut v);
    (med, *v.first().unwrap_or(&f64::NAN), *v.last().unwrap_or(&f64::NAN))
}

// ---------------------------------------------------------------------------------------------
// Result recording (JSON lines)
// ---------------------------------------------------------------------------------------------

pub struct Recorder {
    file: Option<File>,
    pub stage: String,
    pub dataset: String,
    pub cache_state: String,
    pub gate_max_cores: f64,
}

impl Recorder {
    pub fn new(out: Option<&str>, stage: &str) -> Self {
        let file = out.map(|p| OpenOptions::new().create(true).append(true).open(p).expect("open results file"));
        Recorder {
            file,
            stage: stage.to_string(),
            dataset: String::new(),
            cache_state: "warm".into(),
            gate_max_cores: 12.0,
        }
    }

    /// Record one measurement. `params` and `extra` are merged into the record.
    pub fn record(&mut self, variant: &str, run: usize, s: &Sample, params: Value, extra: Value, bg: f64, gate_timeout: bool) {
        let mut m = Map::new();
        m.insert("stage".into(), json!(self.stage));
        m.insert("dataset".into(), json!(self.dataset));
        m.insert("variant".into(), json!(variant));
        m.insert("cache".into(), json!(self.cache_state));
        m.insert("run".into(), json!(run));
        m.insert("wall_ms".into(), json!(ms(s.wall)));
        m.insert("user_ms".into(), json!(ms(s.user)));
        m.insert("sys_ms".into(), json!(ms(s.sys)));
        m.insert("bg_cores".into(), json!(bg));
        m.insert("gate_timeout".into(), json!(gate_timeout));
        m.insert("load1".into(), json!(loadavg()));
        for (k, v) in [params, extra].into_iter().filter_map(|v| v.as_object().cloned()).flatten() {
            m.insert(k, v);
        }
        let line = Value::Object(m).to_string();
        if let Some(f) = self.file.as_mut() {
            writeln!(f, "{line}").ok();
        }
    }
}

/// Run `f` `runs` times for every variant, interleaved (round robin) so that slow drifts of the
/// background load hit every variant equally. `prep` runs before each timed call (outside timing).
pub fn interleaved<V: Clone, R>(
    rec: &mut Recorder,
    variants: &[(String, V)],
    runs: usize,
    params: &Value,
    mut prep: impl FnMut(&str),
    mut body: impl FnMut(&str, &V) -> R,
    mut extra: impl FnMut(&str, &R) -> Value,
) {
    let mut table: Vec<(String, Vec<f64>, Value)> = variants.iter().map(|(n, _)| (n.clone(), vec![], Value::Null)).collect();
    for run in 0..runs {
        for (i, (name, v)) in variants.iter().enumerate() {
            prep(name);
            let (bg, to) = quiet_gate(rec.gate_max_cores, Duration::from_millis(std::env::var("SPIKE_GATE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(500)));
            let (r, s) = timed(|| body(name, v));
            let ex = extra(name, &r);
            rec.record(name, run, &s, params.clone(), ex.clone(), bg, to);
            table[i].1.push(ms(s.wall));
            table[i].2 = ex;
            drop(r);
        }
    }
    println!("  {:<34} {:>9} {:>9} {:>9}   (n={})", "variant", "median ms", "min ms", "max ms", runs);
    for (name, v, ex) in table {
        let (med, mn, mx) = summarize(v);
        let exs = if ex.is_null() { String::new() } else { ex.to_string() };
        println!("  {:<34} {:>9.2} {:>9.2} {:>9.2}   {}", name, med, mn, mx, exs);
    }
}

// ---------------------------------------------------------------------------------------------
// Page cache helpers
// ---------------------------------------------------------------------------------------------

/// Ask the kernel to drop the cached pages of one file. Works without root for clean pages.
pub fn fadvise_dontneed(path: &Path) -> bool {
    use std::os::unix::io::AsRawFd;
    match File::open(path) {
        Ok(f) => unsafe { libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) == 0 },
        Err(_) => false,
    }
}

/// Fraction of the file's pages that are resident in the page cache (mmap + mincore).
pub fn resident_fraction(path: &Path) -> Option<f64> {
    use std::os::unix::io::AsRawFd;
    let f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len() as usize;
    if len == 0 {
        return Some(1.0);
    }
    unsafe {
        let p = libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_SHARED, f.as_raw_fd(), 0);
        if p == libc::MAP_FAILED {
            return None;
        }
        let pages = len.div_ceil(4096);
        let mut vec = vec![0u8; pages];
        let rc = libc::mincore(p, len, vec.as_mut_ptr());
        libc::munmap(p, len);
        if rc != 0 {
            return None;
        }
        Some(vec.iter().filter(|b| **b & 1 == 1).count() as f64 / pages as f64)
    }
}

pub fn mtime_ns(md: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    md.mtime() * 1_000_000_000 + md.mtime_nsec()
}

// ---------------------------------------------------------------------------------------------
// Tiny argument parser: --key value / --flag
// ---------------------------------------------------------------------------------------------

pub struct Args {
    pub pos: Vec<String>,
    kv: Vec<(String, String)>,
}

impl Args {
    pub fn parse() -> Self {
        let mut pos = vec![];
        let mut kv = vec![];
        let mut it = std::env::args().skip(1).peekable();
        while let Some(a) = it.next() {
            if let Some(k) = a.strip_prefix("--") {
                if let Some((k, v)) = k.split_once('=') {
                    kv.push((k.to_string(), v.to_string()));
                } else if it.peek().map(|n| !n.starts_with("--")).unwrap_or(false) {
                    kv.push((k.to_string(), it.next().unwrap()));
                } else {
                    kv.push((k.to_string(), "true".into()));
                }
            } else {
                pos.push(a);
            }
        }
        Args { pos, kv }
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.kv.iter().rev().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str())
    }
    pub fn flag(&self, k: &str) -> bool {
        self.get(k).map(|v| v != "false").unwrap_or(false)
    }
    pub fn usize(&self, k: &str, d: usize) -> usize {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    pub fn list(&self, k: &str) -> Vec<String> {
        self.get(k).map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    }
}
