//! Driver for S1 (directory walk comparison).

use crate::roots;
use crate::util::*;
use crate::walk::*;
use serde_json::json;
use std::path::PathBuf;

pub fn run(a: &Args) {
    let roots = roots::select(&a.list("roots"));
    let variants_wanted = if a.list("variants").is_empty() {
        vec!["std", "walkdir", "jwalk", "jwalk-tuned", "ignore", "rayon", "rayon-names"].into_iter().map(String::from).collect()
    } else {
        a.list("variants")
    };
    let threads: Vec<usize> = if a.list("threads").is_empty() { vec![16] } else { a.list("threads").iter().filter_map(|s| s.parse().ok()).collect() };
    let runs = a.usize("runs", 7);
    let mut rec = Recorder::new(a.get("out"), "S1");
    rec.cache_state = a.get("cache").unwrap_or("warm").to_string();

    for root in roots {
        rec.dataset = root.name.to_string();
        println!("== S1 root={} path={}", root.name, root.path.display());
        // Build the variant list: serial variants once, parallel variants per thread count.
        let mut variants: Vec<(String, (String, usize))> = vec![];
        for v in &variants_wanted {
            match v.as_str() {
                "std" | "walkdir" => variants.push((v.clone(), (v.clone(), 1))),
                other => {
                    for &t in &threads {
                        variants.push((format!("{other}@{t}"), (other.to_string(), t)));
                    }
                }
            }
        }
        let pools: std::collections::HashMap<usize, _> = threads.iter().chain(std::iter::once(&1)).map(|&t| (t, make_pool(t))).collect();
        let path: PathBuf = root.path.clone();
        let run_one = |kind: &str, t: usize| -> WalkOut {
            match kind {
                "std" => walk_std(&path),
                "walkdir" => walk_walkdir(&path),
                "jwalk" => walk_jwalk(&path, pools[&t].clone(), false),
                "jwalk-tuned" => walk_jwalk(&path, pools[&t].clone(), true),
                "ignore" => walk_ignore(&path, t),
                "rayon" => walk_rayon(&path, &pools[&t]),
                "rayon-names" => {
                    let (f, d) = walk_rayon_names_only(&path, &pools[&t]);
                    WalkOut { chunks: vec![], dirs: d, errors: f } // errors field abused for file count; see extra()
                }
                _ => panic!("unknown variant {kind}"),
            }
        };
        // Verification pass: every stat-collecting walker must report an identical digest.
        let reference = digest(&run_one("std", 1));
        println!("  reference digest (std): files={} bytes={} hash={:016x}", reference.0, reference.1, reference.2);
        for (name, (kind, t)) in &variants {
            if kind == "rayon-names" {
                continue;
            }
            let d = digest(&run_one(kind, *t));
            if d != reference {
                println!("  !! digest mismatch for {name}: {:?}", d);
            }
        }
        let params = json!({"root": root.name});
        interleaved(
            &mut rec,
            &variants,
            runs,
            &params,
            |_| {},
            |_, (kind, t)| run_one(kind, *t),
            |name, r| {
                let (kind, t) = &variants.iter().find(|(n, _)| n == name).unwrap().1;
                if kind == "rayon-names" {
                    json!({"threads": t, "walker": kind, "files": r.errors, "dirs": r.dirs})
                } else {
                    json!({"threads": t, "walker": kind, "files": r.files(), "dirs": r.dirs, "errors": r.errors})
                }
            },
        );
    }
}
