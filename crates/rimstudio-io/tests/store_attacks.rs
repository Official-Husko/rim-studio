//! Hostile and damaged files against the document store and the settings store: nothing is lost, nothing
//! is silently rewritten, nothing panics. Names are fictional.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use camino::Utf8PathBuf;
use rimstudio_core::ports::Clock;
use rimstudio_core::settings::Settings;
use rimstudio_io::collection::{Collection, CollectionOptions, validate_id};
use rimstudio_io::error::StoreError;
use rimstudio_io::schema::{LoadOrigin, Versioned};
use rimstudio_io::store::Store;
use serde::{Deserialize, Serialize};

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(TestClock(AtomicU64::new(1_894_708_800_000)))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RsDraft {
    name: String,
    count: u32,
}

impl Versioned for RsDraft {
    const KIND: &'static str = "rs-draft";
    const VERSION: u32 = 1;
}

fn draft(name: &str, count: u32) -> RsDraft {
    RsDraft {
        name: name.to_owned(),
        count,
    }
}

fn open() -> (tempfile::TempDir, Collection<RsDraft>) {
    let t = tempfile::tempdir().unwrap();
    let dir = Utf8PathBuf::from_path_buf(t.path().join("drafts")).unwrap();
    let c = Collection::open_dir(dir, clock()).unwrap();
    (t, c)
}

fn doc_path(c: &Collection<RsDraft>, id: &str) -> Utf8PathBuf {
    c.dir().join(format!("{id}.json"))
}

fn all_files(dir: &camino::Utf8Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            out.push(e.file_name().to_string_lossy().into_owned());
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), out);
            }
        }
    }
    walk(dir.as_std_path(), &mut out);
    out
}

// ---------------------------------------------------------------- document store

#[test]
fn a_document_truncated_at_any_byte_is_quarantined_with_its_bytes_and_never_panics() {
    let (_t, c) = open();
    c.put("a", &draft("RS_Alpha", 7)).unwrap();
    let full = std::fs::read(doc_path(&c, "a")).unwrap();
    // only the final newline may go: the JSON is whole without it
    for cut in 0..full.len() - 1 {
        let bytes = &full[..cut];
        std::fs::write(doc_path(&c, "a"), bytes).unwrap();
        match c.get("a") {
            Err(StoreError::DocumentCorrupt { quarantined, .. }) => {
                let moved = quarantined.expect("the bytes were moved aside");
                assert_eq!(
                    std::fs::read(moved.as_std_path()).unwrap(),
                    bytes,
                    "cut {cut}"
                );
            }
            other => panic!("cut {cut}: {other:?}"),
        }
        assert!(
            !doc_path(&c, "a").exists(),
            "the bad file is gone from the collection"
        );
    }
}

#[test]
fn an_empty_file_a_nul_file_and_binary_noise_are_quarantined() {
    let (_t, c) = open();
    for (i, bytes) in [
        Vec::new(),
        vec![0u8; 64],
        vec![0xFF, 0xFE, 0x7B, 0x00],
        (0..=255u8).collect(),
        b"\xEF\xBB\xBF{\"kind\":\"rs-draft\"}".to_vec(),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("n{i}");
        std::fs::write(doc_path(&c, &id), &bytes).unwrap();
        assert!(
            matches!(c.get(&id), Err(StoreError::DocumentCorrupt { .. })),
            "{id}"
        );
    }
    assert_eq!(c.quarantined().len(), 5);
}

#[test]
fn deeply_nested_json_is_a_corrupt_document_and_not_a_stack_overflow() {
    let (_t, c) = open();
    let bomb = "[".repeat(200_000) + &"]".repeat(200_000);
    std::fs::write(doc_path(&c, "deep"), bomb).unwrap();
    assert!(matches!(
        c.get("deep"),
        Err(StoreError::DocumentCorrupt { .. })
    ));
}

#[test]
fn a_document_over_the_size_limit_is_quarantined_without_being_read_into_memory() {
    let (_t, c) = open();
    let path = doc_path(&c, "huge");
    let file = std::fs::File::create(&path).unwrap();
    // a sparse file: the limit is checked on the length, not by reading
    file.set_len(rimstudio_io::collection::MAX_DOCUMENT_BYTES + 1)
        .unwrap();
    drop(file);
    let started = Instant::now();
    assert!(matches!(
        c.get("huge"),
        Err(StoreError::DocumentCorrupt { .. })
    ));
    assert!(started.elapsed().as_secs() < 5);
    assert_eq!(c.quarantined().len(), 1);
    assert!(
        std::fs::metadata(c.quarantined()[0].path.as_std_path())
            .unwrap()
            .len()
            > rimstudio_io::collection::MAX_DOCUMENT_BYTES,
        "the bytes were moved, not deleted"
    );
}

#[test]
fn a_value_over_the_size_limit_is_refused_on_put() {
    let (_t, c) = open();
    let big =
        "x".repeat(usize::try_from(rimstudio_io::collection::MAX_DOCUMENT_BYTES).unwrap() + 1);
    let err = c.put("big", &draft(&big, 1)).unwrap_err();
    assert_eq!(err.code(), "store.serialize", "{err:?}");
    assert!(!doc_path(&c, "big").exists());
}

#[test]
fn a_document_from_a_newer_app_is_not_changed_by_any_operation() {
    let (_t, c) = open();
    let text = "{\"kind\":\"rs-draft\",\"v\":99,\"data\":{\"name\":\"RS_Future\",\"count\":1,\"extra\":[1,2]}}";
    std::fs::write(doc_path(&c, "f"), text).unwrap();
    let loaded = c.get("f").unwrap().unwrap();
    assert!(loaded.read_only);
    assert!(matches!(
        loaded.origin,
        LoadOrigin::NewerThanApp { found: 99 }
    ));
    assert!(matches!(
        c.put("f", &draft("RS_Mine", 2)),
        Err(StoreError::NewerThanApp { .. })
    ));
    assert!(matches!(
        c.delete("f"),
        Err(StoreError::NewerThanApp { .. })
    ));
    c.rebuild_index().unwrap();
    let _ = c.list_summaries().unwrap();
    assert_eq!(std::fs::read_to_string(doc_path(&c, "f")).unwrap(), text);
    assert!(c.quarantined().is_empty());
}

#[test]
fn a_put_over_a_corrupt_document_that_cannot_be_quarantined_keeps_the_corrupt_bytes() {
    let (_t, c) = open();
    std::fs::write(doc_path(&c, "a"), "{{{ precious but broken").unwrap();
    // a file where the quarantine folder must be makes every quarantine fail
    std::fs::write(c.dir().join(".quarantine"), "not a folder").unwrap();
    let r = c.put("a", &draft("RS_New", 1));
    assert!(r.is_err(), "{r:?}");
    assert_eq!(
        std::fs::read_to_string(doc_path(&c, "a")).unwrap(),
        "{{{ precious but broken",
        "the unreadable bytes survive a refused put"
    );
}

#[test]
fn a_delete_of_a_corrupt_document_keeps_its_bytes_somewhere() {
    let (_t, c) = open();
    let c = c.with_options(CollectionOptions::user_data());
    std::fs::write(doc_path(&c, "a"), "{{{ broken").unwrap();
    assert!(c.delete("a").unwrap());
    let found = all_files(c.dir())
        .iter()
        .any(|n| n.starts_with("a.json.") && n.ends_with(".bak"));
    assert!(
        found,
        "a backup of the deleted bytes exists: {:?}",
        all_files(c.dir())
    );
}

#[test]
fn ids_with_unicode_case_and_separator_tricks_never_reach_the_file_system() {
    let (_t, c) = open();
    let mut bad: Vec<String> = [
        "A",
        "Rs_Draft",
        "\u{ff21}\u{ff22}",
        "caf\u{e9}",
        "cafe\u{301}",
        "a/b",
        "a\\b",
        "../x",
        "..",
        ".",
        ".hidden",
        "trail.",
        "a..b",
        "a b",
        "a\0b",
        "a\u{202e}b",
        "\u{1F600}",
        "con",
        "nul.x",
        "COM1",
        "lpt\u{b9}",
        "index",
        "a%2fb",
        "a:b",
        "a*",
        "i\u{307}d",
        "\u{130}",
        "",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    bad.push("a".repeat(129));
    for id in &bad {
        assert!(validate_id(id).is_err(), "{id:?}");
        assert!(c.put(id, &draft("RS_X", 1)).is_err(), "{id:?}");
        assert!(c.get(id).is_err(), "{id:?}");
        assert!(c.delete(id).is_err(), "{id:?}");
        assert!(c.exists(id).is_err(), "{id:?}");
    }
    assert!(
        all_files(c.dir()).iter().all(|n| n == ".lock"),
        "{:?}",
        all_files(c.dir())
    );
    assert!(validate_id(&"a".repeat(128)).is_ok());
}

#[test]
fn two_instances_over_one_folder_never_corrupt_a_document_or_the_index() {
    // two processes behave like two Collection values with their own locks and indexes
    let (_t, first) = open();
    let second = Collection::<RsDraft>::open_dir(first.dir(), clock()).unwrap();
    std::thread::scope(|s| {
        for (n, c) in [first.clone(), second.clone()].into_iter().enumerate() {
            s.spawn(move || {
                for i in 0..60u32 {
                    c.put(&format!("w{n}-{i}"), &draft("RS_X", i)).unwrap();
                    c.put(
                        "shared",
                        &draft("RS_S", i + u32::try_from(n).unwrap() * 1000),
                    )
                    .unwrap();
                    if i % 7 == 0 {
                        let _ = c.list_summaries();
                    }
                }
            });
        }
    });
    for c in [&first, &second] {
        assert_eq!(c.list_summaries().unwrap().len(), 121);
        assert!(c.get("shared").unwrap().is_some());
        assert!(c.quarantined().is_empty());
    }
    let stray: Vec<String> = all_files(first.dir())
        .into_iter()
        .filter(|n| n.contains(".rstmp-"))
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
}

#[test]
fn a_large_collection_with_a_loaded_index_stays_correct_through_writes_a_crash_and_a_reopen() {
    let (_t, c) = open();
    let c = c.with_options(CollectionOptions::cache());
    for i in 0..1100u32 {
        c.put(&format!("d{i:05}"), &draft("RS_Bulk", i)).unwrap();
    }
    assert_eq!(c.list_summaries().unwrap().len(), 1100);
    // the index is loaded now: writes only update memory above the eager limit
    for i in 0..10u32 {
        c.put(&format!("n{i}"), &draft("RS_New", i)).unwrap();
    }
    for i in 0..5u32 {
        assert!(c.delete(&format!("d{i:05}")).unwrap());
    }
    assert_eq!(c.list_summaries().unwrap().len(), 1105);
    // a process that dies before the next listing leaves a stale index file: the next one notices
    c.put("late", &draft("RS_Late", 1)).unwrap();
    let after_crash = Collection::<RsDraft>::open_dir(c.dir(), clock()).unwrap();
    let ids: Vec<String> = after_crash
        .list_summaries()
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), 1106);
    assert!(ids.contains(&"late".to_owned()));
    assert!(!ids.contains(&"d00000".to_owned()));
}

#[test]
#[ignore = "slow: writes 100000 documents; run with --ignored --release for timings"]
fn a_hundred_thousand_documents_list_scan_and_survive_a_reopen() {
    let t = tempfile::tempdir().unwrap();
    let dir = Utf8PathBuf::from_path_buf(t.path().join("big")).unwrap();
    let c = Collection::<RsDraft>::open_dir(dir.clone(), clock())
        .unwrap()
        .with_options(CollectionOptions::cache());
    let started = Instant::now();
    for i in 0..100_000u32 {
        c.put(&format!("d{i:06}"), &draft("RS_Bulk", i)).unwrap();
    }
    println!("put: {:?}", started.elapsed());
    let started = Instant::now();
    assert_eq!(c.list_ids().unwrap().len(), 100_000);
    println!("list ids: {:?}", started.elapsed());
    let started = Instant::now();
    assert_eq!(c.list_summaries().unwrap().len(), 100_000);
    println!("summaries after writes: {:?}", started.elapsed());
    let started = Instant::now();
    for i in 0..100u32 {
        c.put(&format!("late{i:03}"), &draft("RS_Late", i)).unwrap();
    }
    println!("100 puts with a loaded index: {:?}", started.elapsed());
    for i in 0..100u32 {
        c.delete(&format!("late{i:03}")).unwrap();
    }
    let again = Collection::<RsDraft>::open_dir(dir, clock()).unwrap();
    let started = Instant::now();
    assert_eq!(again.list_summaries().unwrap().len(), 100_000);
    println!("summaries after reopen: {:?}", started.elapsed());
    let started = Instant::now();
    assert_eq!(again.scan().unwrap().filter(Result::is_ok).count(), 100_000);
    println!("scan: {:?}", started.elapsed());
}

// ---------------------------------------------------------------- settings store

fn settings_path() -> (tempfile::TempDir, Utf8PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(t.path().join("settings.jsonc")).unwrap();
    (t, p)
}

#[test]
fn a_settings_file_that_is_not_utf8_is_never_rewritten_with_replacement_characters() {
    let (_t, path) = settings_path();
    // a hand written file saved by an editor in a legacy encoding: the accent is one byte
    let bytes =
        b"{\n  \"schemaVersion\": 1,\n  \"appearance\": { \"theme\": \"dark\" }, // caf\xe9\n}\n"
            .to_vec();
    std::fs::write(&path, &bytes).unwrap();
    let store: Store<Settings> = Store::at(path.clone(), clock());
    let loaded = store.load();
    assert!(loaded.read_only, "{:?}", loaded.origin);
    assert!(store.blocked_reason().is_some());
    let err = store.save(&Settings::default()).unwrap_err();
    assert!(matches!(err, StoreError::WriteBlocked { .. }), "{err:?}");
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_settings_file_in_utf16_is_blocked_and_untouched() {
    let (_t, path) = settings_path();
    let mut bytes = vec![0xFF, 0xFE];
    for u in "{ \"schemaVersion\": 1 }".encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, &bytes).unwrap();
    let store: Store<Settings> = Store::at(path.clone(), clock());
    assert!(store.load().read_only);
    assert!(store.save(&Settings::default()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_settings_file_truncated_at_any_byte_never_panics_and_is_never_overwritten() {
    let (_t, path) = settings_path();
    let store: Store<Settings> = Store::at(path.clone(), clock());
    let mut settings = Settings::default();
    settings.appearance.theme = "dark".to_owned();
    store.save(&settings).unwrap();
    let full = std::fs::read(&path).unwrap();
    for cut in 0..full.len() {
        let bytes = &full[..cut];
        std::fs::write(&path, bytes).unwrap();
        let fresh: Store<Settings> = Store::at(path.clone(), clock());
        let loaded = fresh.load();
        if fresh.blocked_reason().is_some() {
            assert!(loaded.read_only, "cut {cut}");
            assert!(fresh.save(&Settings::default()).is_err(), "cut {cut}");
            assert_eq!(
                std::fs::read(&path).unwrap(),
                bytes,
                "cut {cut}: a blocked store writes nothing"
            );
        }
    }
}

#[test]
fn an_empty_or_comment_only_settings_file_loads_defaults_and_can_be_saved() {
    for text in ["", "   \n", "// only a comment\n", "\u{FEFF}"] {
        let (_t, path) = settings_path();
        std::fs::write(&path, text).unwrap();
        let store: Store<Settings> = Store::at(path.clone(), clock());
        let loaded = store.load();
        assert!(!loaded.read_only, "{text:?}");
        let mut s = loaded.value;
        s.appearance.theme = "light".to_owned();
        store.save(&s).unwrap();
        let back: Store<Settings> = Store::at(path.clone(), clock());
        assert_eq!(back.load().value.appearance.theme, "light".to_owned());
    }
}

#[test]
fn a_huge_settings_file_is_refused_instead_of_read_into_memory() {
    let (_t, path) = settings_path();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(rimstudio_io::store::MAX_SETTINGS_BYTES + 1)
        .unwrap();
    drop(file);
    let store: Store<Settings> = Store::at(path.clone(), clock());
    let loaded = store.load();
    assert!(loaded.read_only);
    assert!(store.save(&Settings::default()).is_err());
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        rimstudio_io::store::MAX_SETTINGS_BYTES + 1
    );
}

#[test]
fn deeply_nested_settings_text_is_a_syntax_problem_and_not_a_stack_overflow() {
    let (_t, path) = settings_path();
    let bomb = "{\"a\":".repeat(100_000) + "1" + &"}".repeat(100_000);
    std::fs::write(&path, bomb).unwrap();
    let store: Store<Settings> = Store::at(path, clock());
    assert!(store.load().read_only);
}

#[test]
fn two_stores_over_one_settings_file_end_with_a_valid_file_and_the_last_save_wins() {
    let (_t, path) = settings_path();
    let a: Store<Settings> = Store::at(path.clone(), clock());
    let b: Store<Settings> = Store::at(path.clone(), clock());
    let mut sa = a.load().value;
    let mut sb = b.load().value;
    sa.appearance.theme = "dark".to_owned();
    sb.appearance.theme = "light".to_owned();
    a.save(&sa).unwrap();
    b.save(&sb).unwrap();
    let fresh: Store<Settings> = Store::at(path, clock());
    let loaded = fresh.load();
    assert!(!loaded.read_only);
    assert_eq!(loaded.value.appearance.theme, "light".to_owned());
}
