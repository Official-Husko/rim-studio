//! IPC lab: indicative measurements of Tauri 2.12.1 IPC paths for RimStudio research.
//! Runs a benchmark page inside the real webview and writes a JSON report.
use serde::{Deserialize, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::http::{header, Response as HttpResponse};
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request, Response};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Dep {
    package_id: String,
    display_name: String,
    steam_workshop_url: String,
    download_url: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModFull {
    id: u32,
    package_id: String,
    name: String,
    author: String,
    authors: Vec<String>,
    description: String,
    url: String,
    mod_version: String,
    supported_versions: Vec<String>,
    mod_dependencies: Vec<Dep>,
    load_before: Vec<String>,
    load_after: Vec<String>,
    force_load_before: Vec<String>,
    force_load_after: Vec<String>,
    incompatible_with: Vec<String>,
    workshop_id: u64,
    path: String,
    source: String,
    size_bytes: u64,
    updated_unix: i64,
    enabled: bool,
    load_index: Option<u32>,
    tags: Vec<String>,
    has_preview: bool,
    flags: u32,
    errors: u32,
    warnings: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModRow {
    id: u32,
    package_id: String,
    name: String,
    author: String,
    mod_version: String,
    workshop_id: u64,
    size_bytes: u64,
    updated_unix: i64,
    enabled: bool,
    load_index: Option<u32>,
    has_preview: bool,
    flags: u32,
    errors: u32,
    warnings: u32,
    versions_mask: u16,
}

fn version_mask(vs: &[String]) -> u16 {
    let mut m = 0u16;
    for v in vs {
        let idx = match v.as_str() {
            "1.0" => 0, "1.1" => 1, "1.2" => 2, "1.3" => 3, "1.4" => 4, "1.5" => 5, "1.6" => 6,
            _ => 15,
        };
        m |= 1 << idx;
    }
    m
}

fn to_row(m: &ModFull) -> ModRow {
    ModRow {
        id: m.id,
        package_id: m.package_id.clone(),
        name: m.name.clone(),
        author: m.author.clone(),
        mod_version: m.mod_version.clone(),
        workshop_id: m.workshop_id,
        size_bytes: m.size_bytes,
        updated_unix: m.updated_unix,
        enabled: m.enabled,
        load_index: m.load_index,
        has_preview: m.has_preview,
        flags: m.flags,
        errors: m.errors,
        warnings: m.warnings,
        versions_mask: version_mask(&m.supported_versions),
    }
}

fn pad8(buf: &mut Vec<u8>) {
    while buf.len() % 8 != 0 {
        buf.push(0);
    }
}

fn pack_columnar(rows: &[ModRow]) -> Vec<u8> {
    let n = rows.len();
    let mut b = Vec::with_capacity(n * 160);
    b.extend_from_slice(&0x314D5352u32.to_le_bytes());
    b.extend_from_slice(&(n as u32).to_le_bytes());
    b.extend_from_slice(&11u32.to_le_bytes());
    b.extend_from_slice(&4u32.to_le_bytes());
    for r in rows { b.extend_from_slice(&r.id.to_le_bytes()); }
    pad8(&mut b);
    for r in rows { b.extend_from_slice(&r.flags.to_le_bytes()); }
    pad8(&mut b);
    for r in rows { b.extend_from_slice(&(r.load_index.map(|x| x as i32).unwrap_or(-1)).to_le_bytes()); }
    pad8(&mut b);
    for r in rows { b.extend_from_slice(&(r.updated_unix as u32).to_le_bytes()); }
    pad8(&mut b);
    for r in rows { b.extend_from_slice(&(r.size_bytes as f64).to_le_bytes()); }
    for r in rows { b.extend_from_slice(&(r.workshop_id as f64).to_le_bytes()); }
    for r in rows { b.push(r.errors as u8); }
    pad8(&mut b);
    for r in rows { b.push(r.warnings as u8); }
    pad8(&mut b);
    for r in rows { b.push(r.enabled as u8); }
    pad8(&mut b);
    for r in rows { b.push(r.has_preview as u8); }
    pad8(&mut b);
    for r in rows { b.extend_from_slice(&r.versions_mask.to_le_bytes()); }
    pad8(&mut b);
    let cols: [&dyn Fn(&ModRow) -> &str; 4] = [&|r| &r.package_id, &|r| &r.name, &|r| &r.author, &|r| &r.mod_version];
    for col in cols {
        let mut off = 0u32;
        b.extend_from_slice(&off.to_le_bytes());
        for r in rows {
            off += col(r).len() as u32;
            b.extend_from_slice(&off.to_le_bytes());
        }
        pad8(&mut b);
        for r in rows { b.extend_from_slice(col(r).as_bytes()); }
        pad8(&mut b);
    }
    b
}

/// Serializes the first `n` items of a shared vector without cloning it.
struct ArcSlice<T> {
    data: Arc<Vec<T>>,
    n: usize,
}

impl<T: Serialize> Serialize for ArcSlice<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.data.iter().take(self.n))
    }
}

struct Lab {
    start: Instant,
    full: Arc<Vec<ModFull>>,
    rows: Arc<Vec<ModRow>>,
    payloads: HashMap<String, Arc<Vec<u8>>>,
    images: Arc<Vec<Arc<Vec<u8>>>>,
    data_dir: String,
}

#[tauri::command]
fn ping_sync() {}

#[tauri::command(async)]
fn ping_async_attr() {}

#[tauri::command]
async fn ping_async() {}

#[tauri::command]
fn echo_small(s: String) -> String {
    s
}

#[tauri::command]
async fn get_full(lab: State<'_, Lab>, n: usize) -> Result<ArcSlice<ModFull>, String> {
    Ok(ArcSlice { data: lab.full.clone(), n: n.min(lab.full.len()) })
}

#[tauri::command]
async fn get_slim(lab: State<'_, Lab>, n: usize) -> Result<ArcSlice<ModRow>, String> {
    Ok(ArcSlice { data: lab.rows.clone(), n: n.min(lab.rows.len()) })
}

#[tauri::command]
async fn get_slim_msgpack(lab: State<'_, Lab>, n: usize) -> Result<Response, String> {
    let n = n.min(lab.rows.len());
    rmp_serde::to_vec_named(&lab.rows[..n]).map(Response::new).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_slim_cols(lab: State<'_, Lab>, n: usize) -> Result<Response, String> {
    let n = n.min(lab.rows.len());
    Ok(Response::new(pack_columnar(&lab.rows[..n])))
}

/// Pre-serialized payloads (3000 records) so the JS side can time pure decoding inside the webview engine.
#[tauri::command]
async fn get_payload(lab: State<'_, Lab>, name: String) -> Result<Response, String> {
    lab.payloads.get(&name).map(|v| Response::new(v.as_ref().clone())).ok_or_else(|| "unknown payload".to_string())
}

#[tauri::command]
async fn get_bytes(len: usize) -> Response {
    Response::new(vec![0xABu8; len])
}

#[tauri::command]
async fn put_bytes(request: Request<'_>) -> Result<usize, String> {
    match request.body() {
        InvokeBody::Raw(b) => Ok(b.len()),
        InvokeBody::Json(_) => Err("expected raw body".into()),
    }
}

#[tauri::command]
async fn put_json(rows: Vec<ModRow>) -> usize {
    rows.len()
}

#[tauri::command]
async fn stream_json(lab: State<'_, Lab>, n: usize, chunk: usize, ch: Channel<Vec<ModRow>>) -> Result<(), String> {
    let n = n.min(lab.rows.len());
    for part in lab.rows[..n].chunks(chunk) {
        ch.send(part.to_vec()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn stream_small(count: usize, size: usize, ch: Channel<serde_json::Value>) -> Result<(), String> {
    let pad = "x".repeat(size);
    for i in 0..count {
        ch.send(serde_json::json!({ "i": i, "pad": pad })).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn stream_raw(count: usize, size: usize, ch: Channel<InvokeResponseBody>) -> Result<(), String> {
    for i in 0..count {
        ch.send(InvokeResponseBody::Raw(vec![i as u8; size])).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn emit_events(app: AppHandle, count: usize, size: usize) -> Result<(), String> {
    let pad = "x".repeat(size);
    for i in 0..count {
        app.emit("lab-event", serde_json::json!({ "i": i, "pad": pad })).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[tauri::command]
async fn get_image_b64(lab: State<'_, Lab>, i: usize) -> Result<String, String> {
    lab.images.get(i).map(|d| b64(d)).ok_or_else(|| "no image".to_string())
}

#[tauri::command]
fn webkit_version() -> String {
    unsafe {
        format!(
            "{}.{}.{}",
            webkit2gtk_sys::webkit_get_major_version(),
            webkit2gtk_sys::webkit_get_minor_version(),
            webkit2gtk_sys::webkit_get_micro_version()
        )
    }
}

#[tauri::command]
fn lab_variant() -> String {
    std::env::var("LAB_VARIANT").unwrap_or_else(|_| "default".to_string())
}

#[tauri::command]
fn data_dir(lab: State<'_, Lab>) -> String {
    lab.data_dir.clone()
}

#[tauri::command]
fn mark(lab: State<'_, Lab>, phase: String) -> f64 {
    let ms = lab.start.elapsed().as_secs_f64() * 1000.0;
    println!("[mark] {phase} at {ms:.1} ms");
    ms
}

#[tauri::command]
fn proc_mem() -> serde_json::Value {
    let me = std::process::id();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for e in rd.flatten() {
            if let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() {
                if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                    if let Some(rp) = stat.rfind(')') {
                        let rest: Vec<&str> = stat[rp + 2..].split_whitespace().collect();
                        if let Some(ppid) = rest.get(1).and_then(|s| s.parse::<u32>().ok()) {
                            children.entry(ppid).or_default().push(pid);
                        }
                    }
                }
            }
        }
    }
    let mut stack = vec![me];
    let mut out = vec![];
    while let Some(pid) = stack.pop() {
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string();
        let mut pss = 0u64;
        let mut rss = 0u64;
        if let Ok(s) = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")) {
            for l in s.lines() {
                if let Some(v) = l.strip_prefix("Pss:") { pss = v.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0); }
                if let Some(v) = l.strip_prefix("Rss:") { rss = v.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0); }
            }
        }
        out.push(serde_json::json!({ "pid": pid, "comm": comm, "rssKb": rss, "pssKb": pss }));
        if let Some(c) = children.get(&pid) { stack.extend(c.iter().copied()); }
    }
    serde_json::Value::Array(out)
}

#[tauri::command]
fn report(app: AppHandle, json: String) {
    if let Ok(path) = std::env::var("LAB_OUT") {
        let _ = std::fs::write(path, json);
    }
    app.exit(0);
}

fn main() {
    let start = Instant::now();
    let dir = std::env::var("LAB_DATA_DIR").expect("set LAB_DATA_DIR");
    let full: Vec<ModFull> = serde_json::from_slice(&std::fs::read(format!("{dir}/mods5000.json")).unwrap()).unwrap();
    let rows: Vec<ModRow> = full.iter().map(to_row).collect();
    let mut payloads: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    let first3000 = &full[..3000];
    let rows3000 = &rows[..3000];
    payloads.insert("full.json".into(), Arc::new(serde_json::to_vec(first3000).unwrap()));
    payloads.insert("slim.json".into(), Arc::new(serde_json::to_vec(rows3000).unwrap()));
    payloads.insert("full.msgpack".into(), Arc::new(rmp_serde::to_vec_named(first3000).unwrap()));
    payloads.insert("slim.msgpack".into(), Arc::new(rmp_serde::to_vec_named(rows3000).unwrap()));
    payloads.insert("slim.cols".into(), Arc::new(pack_columnar(rows3000)));
    let mut images = vec![];
    for i in 0..200 {
        images.push(Arc::new(std::fs::read(format!("{dir}/img/{i}.jpg")).unwrap()));
    }
    let images = Arc::new(images);
    let proto_images = images.clone();
    let lab = Lab { start, full: Arc::new(full), rows: Arc::new(rows), payloads, images, data_dir: dir.clone() };
    println!("[lab] data ready at {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);

    tauri::Builder::default()
        .manage(lab)
        .register_asynchronous_uri_scheme_protocol("modimg", move |_ctx, request, responder| {
            let imgs = proto_images.clone();
            std::thread::spawn(move || {
                let path = request.uri().path().trim_start_matches('/');
                let idx: Option<usize> = path.split('.').next().and_then(|s| s.parse().ok());
                let resp = match idx.and_then(|i| imgs.get(i)) {
                    Some(bytes) => HttpResponse::builder()
                        .status(200)
                        .header(header::CONTENT_TYPE, "image/jpeg")
                        .header(header::CACHE_CONTROL, "max-age=3600")
                        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .body(Cow::Owned(bytes.as_ref().clone()))
                        .unwrap(),
                    None => HttpResponse::builder().status(404).body(Cow::Owned(Vec::new())).unwrap(),
                };
                responder.respond(resp);
            });
        })
        .invoke_handler(tauri::generate_handler![
            ping_sync, ping_async_attr, ping_async, echo_small, get_full, get_slim, get_slim_msgpack, get_slim_cols, get_payload,
            get_bytes, put_bytes, put_json, stream_json, stream_small, stream_raw, emit_events, get_image_b64, data_dir, mark,
            proc_mem, report, webkit_version, lab_variant
        ])
        .setup(move |app| {
            let _ = app;
            println!("[lab] setup done at {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ipc-lab");
}
