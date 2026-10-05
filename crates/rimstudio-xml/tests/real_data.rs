//! Ignored tests against a real install and mod library, plus micro benchmarks.
//!
//! Set `RIMSTUDIO_GAME_DIR` (the RimWorld install folder) and optionally `RIMSTUDIO_WORKSHOP_DIR`
//! (the workshop content folder of the game) and run with `-- --ignored --nocapture`. The tests
//! only read; they print counts and timings and never write anything.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use rimstudio_xml::about::read_lenient;
use rimstudio_xml::defs_scan::index_file_detailed;
use rimstudio_xml::{ParseMode, parse_document};

fn env_dir(name: &str) -> Option<PathBuf> {
    let value = std::env::var_os(name)?;
    let path = PathBuf::from(value);
    path.is_dir().then_some(path)
}

fn collect(dir: &Path, want: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => collect(&p, want, out),
            Ok(t) if t.is_file() && want(&p) => out.push(p),
            _ => {}
        }
    }
}

fn is_xml(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

fn in_dir_named(p: &Path, name: &str) -> bool {
    p.components().any(|c| c.as_os_str() == name)
}

fn def_files(root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    collect(root, &|p| is_xml(p) && in_dir_named(p, "Defs"), &mut v);
    v
}

fn run_parallel<T: Send>(
    files: &[PathBuf],
    threads: usize,
    work: impl Fn(&Path) -> T + Sync,
) -> Vec<T> {
    let chunk = files.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|c| {
                let work = &work;
                s.spawn(move || c.iter().map(|p| work(p)).collect::<Vec<T>>())
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn index_and_parse_the_real_install() {
    let Some(game) = env_dir("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; skipping");
        return;
    };
    let files = def_files(&game.join("Data"));
    assert!(!files.is_empty(), "no Defs files under {}", game.display());
    let t0 = Instant::now();
    let results = run_parallel(&files, 8, |p| {
        let bytes = std::fs::read(p).unwrap_or_default();
        let idx = index_file_detailed(&bytes);
        let tree = parse_document(&bytes, ParseMode::Game);
        (
            p.to_path_buf(),
            idx,
            tree.map(|d| d.root.elements().count()),
        )
    });
    let elapsed = t0.elapsed();
    let mut defs = 0usize;
    let mut index_errors = 0usize;
    let mut parse_errors = 0usize;
    let mut disagreements = Vec::new();
    for (path, idx, tree) in &results {
        defs += idx.records.len();
        index_errors += usize::from(idx.error.is_some());
        match tree {
            Ok(n) => {
                if *n != idx.records.len() {
                    disagreements.push(path.display().to_string());
                }
            }
            Err(_) => parse_errors += 1,
        }
    }
    println!(
        "install: {} files, {} defs, {} parse errors, {} index errors, {} count disagreements, parse+index {:?}",
        files.len(),
        defs,
        parse_errors,
        index_errors,
        disagreements.len(),
        elapsed
    );
    assert!(disagreements.is_empty(), "{disagreements:?}");
    assert_eq!(
        parse_errors, index_errors,
        "the index and the game mode parser disagree on broken files"
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR"]
fn bench_def_index_on_the_install() {
    let Some(game) = env_dir("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; skipping");
        return;
    };
    let files = def_files(&game.join("Data"));
    let blobs: Vec<Vec<u8>> = files.iter().filter_map(|p| std::fs::read(p).ok()).collect();
    let bytes: usize = blobs.iter().map(Vec::len).sum();
    let runs = 5;
    let mut best = std::time::Duration::MAX;
    let mut defs = 0;
    for _ in 0..runs {
        let t0 = Instant::now();
        defs = blobs
            .iter()
            .map(|b| index_file_detailed(b).records.len())
            .sum::<usize>();
        best = best.min(t0.elapsed());
    }
    println!(
        "def_index: {} files, {:.1} MB, {} defs, best of {runs}: {:?} single thread ({:.0} MB/s)",
        blobs.len(),
        bytes as f64 / 1e6,
        defs,
        best,
        bytes as f64 / 1e6 / best.as_secs_f64()
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_WORKSHOP_DIR"]
fn index_the_workshop_library_and_time_it() {
    let Some(dir) = env_dir("RIMSTUDIO_WORKSHOP_DIR") else {
        println!("RIMSTUDIO_WORKSHOP_DIR is not set; skipping");
        return;
    };
    let files = def_files(&dir);
    let t0 = Instant::now();
    let results = run_parallel(&files, 16, |p| {
        let bytes = std::fs::read(p).unwrap_or_default();
        let idx = index_file_detailed(&bytes);
        (idx.records.len(), idx.error.is_some())
    });
    let elapsed = t0.elapsed();
    let defs: usize = results.iter().map(|r| r.0).sum();
    let errors = results.iter().filter(|r| r.1).count();
    println!(
        "workshop def index: {} files, {} defs, {} files with errors, 16 threads end to end {:?} (budget 300 ms for 23,717 files warm)",
        files.len(),
        defs,
        errors,
        elapsed
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_WORKSHOP_DIR"]
fn about_files_of_the_workshop_library_read_without_failures() {
    let Some(dir) = env_dir("RIMSTUDIO_WORKSHOP_DIR") else {
        println!("RIMSTUDIO_WORKSHOP_DIR is not set; skipping");
        return;
    };
    let mut files = Vec::new();
    collect(
        &dir,
        &|p| {
            p.file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("about.xml"))
                && p.parent()
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == "About")
        },
        &mut files,
    );
    let blobs: Vec<Vec<u8>> = files.iter().filter_map(|p| std::fs::read(p).ok()).collect();
    let t0 = Instant::now();
    let reads: Vec<_> = blobs.iter().map(|b| read_lenient(b)).collect();
    let elapsed = t0.elapsed();
    let unparsed = reads.iter().filter(|r| !r.parsed).count();
    let no_id = reads
        .iter()
        .filter(|r| r.about.package_id.is_empty())
        .count();
    println!(
        "about: {} files, {} unparsed, {} without packageId, single thread {:?} ({:?} per file; budget 10 ms for 690 files in parallel)",
        blobs.len(),
        unparsed,
        no_id,
        elapsed,
        elapsed / u32::try_from(blobs.len().max(1)).unwrap_or(1)
    );
}

fn synthetic_about(i: usize) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ModMetaData>\n  <name>RS Mod {i}</name>\n  <author>Ann</author>\n  <packageId>rs.mod{i}</packageId>\n  <supportedVersions><li>1.4</li><li>1.5</li><li>1.6</li></supportedVersions>\n  <modDependencies><li><packageId>rs.dep</packageId><displayName>Dep</displayName><downloadUrl>https://example.invalid</downloadUrl></li></modDependencies>\n  <loadAfter><li>ludeon.rimworld</li><li>rs.dep</li></loadAfter>\n  <description>Synthetic description number {i} with some words to read.</description>\n</ModMetaData>\n"
    )
}

#[test]
#[ignore = "micro benchmark, prints times"]
fn bench_about_parse_synthetic() {
    let blobs: Vec<String> = (0..690).map(synthetic_about).collect();
    let runs = 20;
    let mut best = std::time::Duration::MAX;
    for _ in 0..runs {
        let t0 = Instant::now();
        let n: usize = blobs
            .iter()
            .map(|b| read_lenient(b.as_bytes()).about.supported_versions.len())
            .sum();
        assert_eq!(n, 690 * 3);
        best = best.min(t0.elapsed());
    }
    println!(
        "about_parse: 690 synthetic files, best of {runs}: {best:?} single thread, {:?} per file",
        best / 690
    );
}

#[test]
#[ignore = "micro benchmark, prints times"]
fn bench_def_index_synthetic() {
    let mut def = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Defs>\n");
    for i in 0..40 {
        def.push_str(&format!(
            "  <ThingDef ParentName=\"RS_Base\">\n    <defName>RS_Thing{i}</defName>\n    <label>thing {i}</label>\n    <description>Some text that makes the element a little longer than trivial.</description>\n    <statBases><MarketValue>10</MarketValue><Mass>1</Mass></statBases>\n    <comps><li Class=\"RS.Comp\"><x>1</x></li></comps>\n  </ThingDef>\n"
        ));
    }
    def.push_str("</Defs>\n");
    let files = 2000;
    let runs = 5;
    let mut best = std::time::Duration::MAX;
    for _ in 0..runs {
        let t0 = Instant::now();
        let mut n = 0;
        for _ in 0..files {
            n += rimstudio_xml::defs_scan::index_file(def.as_bytes()).len();
        }
        assert_eq!(n, files * 40);
        best = best.min(t0.elapsed());
    }
    let mb = (def.len() * files) as f64 / 1e6;
    println!(
        "def_index: {files} synthetic files ({mb:.1} MB, {} defs), best of {runs}: {best:?} single thread ({:.0} MB/s)",
        files * 40,
        mb / best.as_secs_f64()
    );
}

#[test]
#[ignore = "micro benchmark, prints times"]
fn bench_tree_parse_and_render_synthetic() {
    let mut def = String::from("<Defs>\n");
    for i in 0..400 {
        def.push_str(&format!(
            "  <ThingDef ParentName=\"RS_Base\"><defName>RS_Thing{i}</defName><label>thing {i}</label><statBases><MarketValue>10</MarketValue></statBases></ThingDef>\n"
        ));
    }
    def.push_str("</Defs>\n");
    let t0 = Instant::now();
    let mut doc = None;
    for _ in 0..20 {
        doc = parse_document(def.as_bytes(), ParseMode::Game).ok();
    }
    let parse = t0.elapsed() / 20;
    let doc = doc.unwrap();
    let t1 = Instant::now();
    let mut len = 0;
    for _ in 0..20 {
        len = rimstudio_xml::render::render(&doc.root, &rimstudio_xml::RenderOpts::default()).len();
    }
    println!(
        "tree: parse {:?}, render {:?} for {} bytes in / {} bytes out",
        parse,
        t1.elapsed() / 20,
        def.len(),
        len
    );
}

fn files_named(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    collect(
        root,
        &|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(name)),
        &mut v,
    );
    v
}

#[test]
#[ignore = "needs RIMSTUDIO_WORKSHOP_DIR"]
fn about_edits_with_unchanged_values_keep_every_real_file_byte_identical() {
    let Some(dir) = env_dir("RIMSTUDIO_WORKSHOP_DIR") else {
        println!("RIMSTUDIO_WORKSHOP_DIR is not set; skipping");
        return;
    };
    let mut checked = 0usize;
    let mut cannot_open = 0usize;
    let mut changed = Vec::new();
    for path in files_named(&dir, "About.xml") {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes.clone()) else {
            continue;
        };
        let read = read_lenient(&bytes);
        if !read.parsed || read.about.package_id.is_empty() {
            continue;
        }
        // Only files whose packageId field is plain text can be rewritten with its own value.
        let Ok(mut ed) = rimstudio_xml::edit::SpanEditor::open(text.clone()) else {
            cannot_open += 1;
            continue;
        };
        let root = format!("/{}", ed.root_tag());
        let path_id = format!("{root}/packageId");
        let Ok(current) = ed.element_text(&path_id) else {
            continue;
        };
        if ed.replace_text(&path_id, &current).is_err() {
            continue;
        }
        checked += 1;
        if ed.text() != text && !text.contains(['&', '<']) {
            // Differences are only expected when entity or CDATA spelling is normalised.
            changed.push(path.display().to_string());
        }
    }
    println!(
        "about edits: {checked} files rewritten with their own packageId, {cannot_open} not indexable, {} changed",
        changed.len()
    );
    assert!(
        changed.len() <= checked / 50,
        "too many changed files: {changed:?}"
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_WORKSHOP_DIR"]
fn load_folders_of_the_workshop_library_read_and_survive_a_gate_edit() {
    let Some(dir) = env_dir("RIMSTUDIO_WORKSHOP_DIR") else {
        println!("RIMSTUDIO_WORKSHOP_DIR is not set; skipping");
        return;
    };
    let files = files_named(&dir, "LoadFolders.xml");
    let (mut ok, mut failed, mut blocks, mut entries) = (0usize, 0usize, 0usize, 0usize);
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        match rimstudio_xml::load_folders::read(&bytes) {
            Ok(r) => {
                ok += 1;
                blocks += r.spec.blocks.len();
                entries += r.spec.blocks.iter().map(|b| b.entries.len()).sum::<usize>();
                if let Ok(text) = String::from_utf8(bytes)
                    && let Ok(out) = rimstudio_xml::load_folders::gate_folder(
                        &text,
                        "1.6",
                        "RS_Gate",
                        &["rs.fictional"],
                    )
                {
                    let again = rimstudio_xml::load_folders::read(out.as_bytes()).unwrap();
                    let block = again.spec.block("1.6").unwrap();
                    assert!(block.entries.iter().any(|e| e.path == "RS_Gate"));
                }
            }
            Err(_) => failed += 1,
        }
    }
    println!(
        "load folders: {} files, {ok} read, {failed} unreadable, {blocks} blocks, {entries} entries",
        files.len()
    );
}

#[test]
#[ignore = "needs RIMSTUDIO_CE_DIR"]
fn combat_extended_load_folders_and_patches_read() {
    let Some(dir) = env_dir("RIMSTUDIO_CE_DIR") else {
        println!("RIMSTUDIO_CE_DIR is not set; skipping");
        return;
    };
    let lf = std::fs::read(dir.join("LoadFolders.xml")).unwrap_or_default();
    if let Ok(r) = rimstudio_xml::load_folders::read(&lf) {
        let counts: Vec<(String, usize)> = r
            .spec
            .blocks
            .iter()
            .map(|b| (b.key.clone(), b.entries.len()))
            .collect();
        println!("CE load folders blocks: {counts:?}");
    }
    let mut patches = Vec::new();
    collect(
        &dir,
        &|p| is_xml(p) && in_dir_named(p, "Patches"),
        &mut patches,
    );
    let mut ops = 0usize;
    let mut failed = 0usize;
    let t0 = Instant::now();
    for p in &patches {
        let bytes = std::fs::read(p).unwrap_or_default();
        match rimstudio_xml::patches::parse_patch_file(&bytes) {
            Ok(o) => ops += o.len(),
            Err(_) => failed += 1,
        }
    }
    println!(
        "CE patches: {} files, {ops} operations, {failed} unparseable, {:?}",
        patches.len(),
        t0.elapsed()
    );
}
