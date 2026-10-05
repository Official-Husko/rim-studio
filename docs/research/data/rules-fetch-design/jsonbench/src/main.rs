use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
#[derive(Deserialize)]
struct Slim { #[serde(rename="packageId", default)] pid: Option<String>, #[serde(rename="packageid", default)] pid2: Option<String>, #[serde(default)] unpublished: Option<bool>, #[serde(rename="steamName", default)] sn: Option<String>, #[serde(default)] name: Option<String>, #[serde(default)] authors: Option<String>, #[serde(rename="gameVersions", default)] gv: Option<Vec<Option<String>>> }
#[derive(Deserialize)]
struct Db { database: HashMap<String, Slim> }
#[derive(Deserialize)]
struct DbFull { database: HashMap<String, serde_json::Value> }
fn rss() -> u64 { let s = std::fs::read_to_string("/proc/self/status").unwrap(); for l in s.lines() { if l.starts_with("VmHWM") { return l.split_whitespace().nth(1).unwrap().parse().unwrap(); } } 0 }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = a[1].as_str(); let path = &a[2];
    let t = Instant::now();
    let mut bytes = std::fs::read(path).unwrap();
    let read = t.elapsed();
    let t = Instant::now();
    let n = match mode {
        "serde_value" => { let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap(); v["database"].as_object().unwrap().len() }
        "serde_typed_slim" => { let v: Db = serde_json::from_slice(&bytes).unwrap(); v.database.len() }
        "serde_typed_full_value_map" => { let v: DbFull = serde_json::from_slice(&bytes).unwrap(); v.database.len() }
        "simd_value" => { let v = simd_json::to_owned_value(&mut bytes).unwrap(); use simd_json::prelude::*; v.get("database").unwrap().as_object().unwrap().len() }
        "simd_typed_slim" => { let v: Db = simd_json::serde::from_slice(&mut bytes).unwrap(); v.database.len() }
        "slim_index" => { let v: HashMap<String, Slim> = serde_json::from_slice(&bytes).unwrap(); v.len() }
        _ => panic!(),
    };
    println!("{mode} entries={n} read_ms={:.1} parse_ms={:.1} peak_rss_mb={:.1}", read.as_secs_f64()*1e3, t.elapsed().as_secs_f64()*1e3, rss() as f64/1024.0);
}
