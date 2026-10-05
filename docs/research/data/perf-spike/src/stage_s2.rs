//! Driver for S2 (About.xml parsing): speed, agreement between parsers, failure counts, robustness
//! against synthetic damage.

use crate::about::*;
use crate::roots;
use crate::util::*;
use crate::walk::make_pool;
use rayon::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// All About.xml files of the selected roots, with the root name.
pub fn about_files(root_names: &[String]) -> Vec<(String, PathBuf)> {
    let mut v = vec![];
    for r in roots::select(root_names) {
        for m in roots::mod_dirs(&r) {
            if let Some(p) = find_about(&m) {
                v.push((r.name.to_string(), p));
            }
        }
    }
    v
}

pub fn run(a: &Args) {
    let root_names = if a.list("roots").is_empty() { vec!["workshop".to_string()] } else { a.list("roots") };
    let runs = a.usize("runs", 7);
    let iters = a.usize("iters", 30);
    let threads: Vec<usize> = if a.list("threads").is_empty() { vec![1, 2, 4, 8, 16] } else { a.list("threads").iter().filter_map(|s| s.parse().ok()).collect() };
    let files = about_files(&root_names);
    let label = root_names.join("+");
    println!("== S2 roots={label} about_files={}", files.len());
    let mut rec = Recorder::new(a.get("out"), "S2");
    rec.dataset = label.clone();

    // Preload bytes (outside of any timing).
    let blobs: Vec<Vec<u8>> = files.iter().map(|(_, p)| std::fs::read(p).unwrap_or_default()).collect();
    let total_bytes: usize = blobs.iter().map(|b| b.len()).sum();
    println!("  total bytes {} (median file {} B)", total_bytes, {
        let mut s: Vec<f64> = blobs.iter().map(|b| b.len() as f64).collect();
        median(&mut s)
    });

    // ---- C: agreement and failure counts -------------------------------------------------
    let texts: Vec<String> = blobs.iter().map(|b| decode(b).into_owned()).collect();
    let mut stats: BTreeMap<&str, (usize, usize, usize, usize, usize)> = BTreeMap::new(); // ok, recovered, failed, differs, unknown-entity files
    let reference: Vec<About> = texts.iter().map(|t| normalize(parse_events(t, false).0)).collect();
    for (i, t) in texts.iter().enumerate() {
        let (ev, rep) = parse_events(t, false);
        let e = stats.entry("events-lenient").or_default();
        if rep.error.is_some() {
            e.1 += 1;
        } else {
            e.0 += 1;
        }
        if rep.unknown_entities > 0 {
            e.4 += 1;
        }
        if normalize(ev) != reference[i] {
            e.3 += 1;
        }
        let (ev2, rep2) = parse_events(t, true);
        let e = stats.entry("events-strict").or_default();
        if rep2.error.is_some() {
            e.1 += 1;
        } else {
            e.0 += 1;
        }
        if normalize(ev2) != reference[i] {
            e.3 += 1;
        }
        let e = stats.entry("serde").or_default();
        match parse_serde(t) {
            Ok(x) => {
                e.0 += 1;
                if x != reference[i] {
                    e.3 += 1;
                    if a.flag("show-diffs") {
                        println!("  serde differs: {}", files[i].1.display());
                    }
                }
            }
            Err(err) => {
                e.2 += 1;
                if a.flag("show-diffs") {
                    println!("  serde failed {}: {}", files[i].1.display(), err);
                }
            }
        }
        let e = stats.entry("roxmltree").or_default();
        match parse_dom(t) {
            Ok(x) => {
                e.0 += 1;
                if x != reference[i] {
                    e.3 += 1;
                    if a.flag("show-diffs") {
                        println!("  dom differs: {}", files[i].1.display());
                    }
                }
            }
            Err(err) => {
                e.2 += 1;
                if a.flag("show-diffs") {
                    println!("  dom failed {}: {}", files[i].1.display(), err);
                }
            }
        }
    }
    println!("  agreement vs events-lenient (n={}):", files.len());
    for (k, (ok, rc, fl, df, ue)) in &stats {
        println!("    {k:<16} ok={ok} recovered={rc} failed={fl} differs={df} files_with_unknown_entities={ue}");
        rec.record(
            &format!("check:{k}"),
            0,
            &Sample::default(),
            json!({"n": files.len()}),
            json!({"ok": ok, "recovered": rc, "failed": fl, "differs": df, "unknown_entity_files": ue}),
            0.0,
            false,
        );
    }
    let with_pkg = reference.iter().filter(|a| !a.package_id.is_empty()).count();
    let with_deps = reference.iter().filter(|a| !a.mod_dependencies.is_empty()).count();
    println!("  parsed: with packageId={with_pkg} with modDependencies={with_deps}");

    // ---- A: pure parse speed from memory -----------------------------------------------------
    println!("  A) parse only, single thread, {} iterations over {} files", iters, files.len());
    let params = json!({"mode": "parse-only", "iters": iters, "files": files.len(), "bytes": total_bytes});
    let variants: Vec<(String, u8)> =
        vec![("events-lenient".into(), 0), ("events-strict".into(), 1), ("serde".into(), 2), ("roxmltree".into(), 3)];
    interleaved(
        &mut rec,
        &variants,
        runs,
        &params,
        |_| {},
        |_, v| {
            let mut sink = 0usize;
            for _ in 0..iters {
                for t in &texts {
                    sink += match v {
                        0 => parse_events(t, false).0.name.len(),
                        1 => parse_events(t, true).0.name.len(),
                        2 => parse_serde(t).map(|a| a.name.len()).unwrap_or(0),
                        _ => parse_dom(t).map(|a| a.name.len()).unwrap_or(0),
                    };
                }
            }
            sink
        },
        |_, _| json!({"parses": iters * files.len()}),
    );

    // ---- B: end to end (read + decode + parse) with thread counts ----------------------------
    println!("  B) end to end: read + decode + parse");
    let paths: Vec<PathBuf> = files.iter().map(|(_, p)| p.clone()).collect();
    let mut variants: Vec<(String, (u8, usize))> = vec![];
    for &t in &threads {
        variants.push((format!("events@{t}"), (0, t)));
    }
    variants.push(("serde@1".into(), (2, 1)));
    variants.push(("roxmltree@1".into(), (3, 1)));
    let pools: std::collections::HashMap<usize, _> = threads.iter().chain([1usize].iter()).map(|&t| (t, make_pool(t))).collect();
    let params = json!({"mode": "end-to-end", "files": files.len(), "bytes": total_bytes});
    interleaved(
        &mut rec,
        &variants,
        runs,
        &params,
        |_| {},
        |_, (k, t)| {
            pools[t].install(|| {
                paths
                    .par_iter()
                    .map(|p| {
                        let b = std::fs::read(p).unwrap_or_default();
                        let text = decode(&b);
                        match k {
                            0 => parse_events(&text, false).0.name.len(),
                            2 => parse_serde(&text).map(|a| a.name.len()).unwrap_or(0),
                            _ => parse_dom(&text).map(|a| a.name.len()).unwrap_or(0),
                        }
                    })
                    .sum::<usize>()
            })
        },
        |_, _| json!({}),
    );
}

// ---------------------------------------------------------------------------------------------
// Robustness matrix with synthetic damage (the real corpus has no malformed About.xml)
// ---------------------------------------------------------------------------------------------

fn mutate(kind: &str, original: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(original).into_owned();
    let s = s.trim_start_matches('\u{feff}').to_string();
    match kind {
        "bom-added" => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(s.as_bytes());
            v
        }
        "utf16le-bom" => {
            let mut v = vec![0xFF, 0xFE];
            for u in s.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        "tags-lowercase" | "tags-uppercase" | "tags-pascalcase" => {
            // rewrite the names of the tags that are children of ModMetaData and of dependency lists
            let mut out = String::with_capacity(s.len());
            let mut rest = s.as_str();
            while let Some(i) = rest.find('<') {
                out.push_str(&rest[..i]);
                let tail = &rest[i..];
                let end = tail.find('>').map(|e| e + 1).unwrap_or(tail.len());
                let tag = &tail[..end];
                let (slash, body) = if let Some(b) = tag.strip_prefix("</") { ("/", b) } else { ("", &tag[1..]) };
                if tag.starts_with("<?") || tag.starts_with("<!") || body.is_empty() {
                    out.push_str(tag);
                } else {
                    let name_end = body.find(|c: char| c == '>' || c == ' ' || c == '/' || c == '\t' || c == '\r' || c == '\n').unwrap_or(body.len());
                    let (name, remainder) = body.split_at(name_end);
                    let new = match kind {
                        "tags-lowercase" => name.to_ascii_lowercase(),
                        "tags-uppercase" => name.to_ascii_uppercase(),
                        _ => {
                            let mut c = name.chars();
                            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
                        }
                    };
                    // keep <li> untouched: the game requires lowercase li
                    let new = if name.eq_ignore_ascii_case("li") { name.to_string() } else { new };
                    out.push('<');
                    out.push_str(slash);
                    out.push_str(&new);
                    out.push_str(remainder);
                }
                rest = &tail[end..];
            }
            out.push_str(rest);
            out.into_bytes()
        }
        "truncated-60pct" => original[..original.len() * 6 / 10].to_vec(),
        "missing-root-close" => s.replace("</ModMetaData>", "").into_bytes(),
        "mismatched-end-tag" => s.replacen("</name>", "</Name>", 1).into_bytes(),
        "stray-ampersand" => s.replacen("<name>", "<name>Tom & Jerry ", 1).into_bytes(),
        "undefined-entity-nbsp" => s.replacen("<description>", "<description>a&nbsp;b ", 1).into_bytes(),
        "latin1-byte" => {
            let mut v = s.replacen("<name>", "<name>caf", 1).into_bytes();
            let pos = v.windows(3).position(|w| w == b"caf").unwrap_or(0) + 3;
            v.insert(pos, 0xE9);
            v
        }
        "blank-line-before-decl" => format!("\n{s}").into_bytes(),
        "duplicate-element" => s.replacen("</name>", "</name><name>Second</name>", 1).into_bytes(),
        "unknown-extra-elements" => s.replacen("</ModMetaData>", "<foo><bar>1</bar></foo></ModMetaData>", 1).into_bytes(),
        "html-in-description" => s.replacen("<description>", "<description>Hello <b>bold</b> world ", 1).into_bytes(),
        _ => original.to_vec(),
    }
}

pub fn robust(a: &Args) {
    let root_names = if a.list("roots").is_empty() { vec!["workshop".to_string()] } else { a.list("roots") };
    let files = about_files(&root_names);
    let sample: Vec<Vec<u8>> = files.iter().step_by((files.len() / 200).max(1)).map(|(_, p)| std::fs::read(p).unwrap_or_default()).collect();
    let originals: Vec<About> = sample.iter().map(|b| normalize(parse_events(&decode(b), false).0)).collect();
    let kinds = [
        "bom-added",
        "utf16le-bom",
        "tags-lowercase",
        "tags-uppercase",
        "tags-pascalcase",
        "html-in-description",
        "unknown-extra-elements",
        "duplicate-element",
        "stray-ampersand",
        "undefined-entity-nbsp",
        "latin1-byte",
        "mismatched-end-tag",
        "missing-root-close",
        "truncated-60pct",
        "blank-line-before-decl",
    ];
    println!("== S2 robustness matrix on {} sampled real About.xml files (per cell: same / changed / partial / failed)", sample.len());
    println!("{:<24} {:>22} {:>22} {:>22} {:>22}", "mutation", "events-lenient", "events-strict", "serde", "roxmltree");
    let mut rec = Recorder::new(a.get("out"), "S2-robust");
    for kind in kinds {
        let mut cells: Vec<[usize; 4]> = vec![[0; 4]; 4]; // same, changed, partial, failed
        for (b, orig) in sample.iter().zip(&originals) {
            let m = mutate(kind, b);
            let text = decode(&m);
            // events-lenient
            let (x, rep) = parse_events(&text, false);
            let x = normalize(x);
            classify_cell(&mut cells[0], Some(x), rep.error.is_some(), orig);
            let (x, rep) = parse_events(&text, true);
            let x = normalize(x);
            classify_cell(&mut cells[1], Some(x), rep.error.is_some(), orig);
            match parse_serde(&text) {
                Ok(x) => classify_cell(&mut cells[2], Some(x), false, orig),
                Err(_) => classify_cell(&mut cells[2], None, true, orig),
            }
            match parse_dom(&text) {
                Ok(x) => classify_cell(&mut cells[3], Some(x), false, orig),
                Err(_) => classify_cell(&mut cells[3], None, true, orig),
            }
        }
        let f = |c: &[usize; 4]| format!("{}/{}/{}/{}", c[0], c[1], c[2], c[3]);
        println!("{:<24} {:>22} {:>22} {:>22} {:>22}", kind, f(&cells[0]), f(&cells[1]), f(&cells[2]), f(&cells[3]));
        for (i, name) in ["events-lenient", "events-strict", "serde", "roxmltree"].iter().enumerate() {
            rec.record(
                &format!("robust:{kind}:{name}"),
                0,
                &Sample::default(),
                json!({"mutation": kind, "parser": name, "n": sample.len()}),
                json!({"same": cells[i][0], "changed": cells[i][1], "partial": cells[i][2], "failed": cells[i][3]}),
                0.0,
                false,
            );
        }
    }
}

/// same: no error and identical to the original; changed: no error but different; partial: error but a
/// non-empty result (packageId or name present); failed: error and nothing usable.
fn classify_cell(cell: &mut [usize; 4], result: Option<About>, errored: bool, orig: &About) {
    match result {
        Some(x) if !errored => {
            if &x == orig {
                cell[0] += 1
            } else {
                cell[1] += 1
            }
        }
        Some(x) if !x.name.is_empty() || !x.package_id.is_empty() => cell[2] += 1,
        _ => cell[3] += 1,
    }
}
