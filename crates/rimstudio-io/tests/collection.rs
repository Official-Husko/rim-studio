//! Integration tests of the JSON document store.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;
use rimstudio_io::atomic::{FsOps, RealOps, TEMP_MARKER};
use rimstudio_io::backup::BackupPolicy;
use rimstudio_io::collection::{Collection, CollectionOptions, validate_id};
use rimstudio_io::error::{MigrateError, StoreError};
use rimstudio_io::migrate::MigrationFn;
use rimstudio_io::schema::{LoadOrigin, Versioned};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

impl RsDraft {
    fn new(name: &str, count: u32) -> Self {
        RsDraft {
            name: name.to_owned(),
            count,
        }
    }
}

impl Versioned for RsDraft {
    const KIND: &'static str = "rs-draft";
    const VERSION: u32 = 2;
    fn migrations() -> Vec<(u32, MigrationFn)> {
        fn v1(mut v: Value) -> Result<Value, MigrateError> {
            let label = v
                .get("label")
                .cloned()
                .ok_or_else(|| MigrateError::failed("rs-draft", 1, "label missing"))?;
            if let Some(o) = v.as_object_mut() {
                o.remove("label");
                o.insert("name".into(), label);
            }
            Ok(v)
        }
        vec![(1, v1)]
    }
}

fn open() -> (tempfile::TempDir, Collection<RsDraft>) {
    let t = tempfile::tempdir().unwrap();
    let dir = Utf8PathBuf::from_path_buf(t.path().join("drafts")).unwrap();
    let c = Collection::<RsDraft>::open_dir(dir, clock()).unwrap();
    (t, c)
}

fn write_raw(c: &Collection<RsDraft>, id: &str, text: &str) {
    fs_err::write(c.dir().join(format!("{id}.json")), text).unwrap();
}

#[test]
fn put_get_round_trip_and_replace() {
    let (_t, c) = open();
    assert!(c.get("a").unwrap().is_none());
    c.put("a", &RsDraft::new("RS_One", 1)).unwrap();
    let l = c.get("a").unwrap().unwrap();
    assert_eq!(l.value, RsDraft::new("RS_One", 1));
    assert_eq!(l.origin, LoadOrigin::File);
    assert!(!l.read_only);
    c.put("a", &RsDraft::new("RS_Two", 2)).unwrap();
    assert_eq!(c.get("a").unwrap().unwrap().value.count, 2);
    assert!(c.exists("a").unwrap());
    assert!(!c.exists("b").unwrap());
}

#[test]
fn file_is_the_documented_envelope_with_stable_bytes() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("RS_One", 1)).unwrap();
    let text = fs_err::read_to_string(c.dir().join("a.json")).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v,
        json!({"kind": "rs-draft", "v": 2, "data": {"name": "RS_One", "count": 1}})
    );
    assert!(text.ends_with('\n'));
    c.put("a", &RsDraft::new("RS_One", 1)).unwrap();
    assert_eq!(
        fs_err::read_to_string(c.dir().join("a.json")).unwrap(),
        text
    );
}

#[test]
fn ids_are_listed_sorted_and_counted() {
    let (_t, c) = open();
    assert!(c.is_empty().unwrap());
    for id in ["zeta", "alpha", "m-1", "m.2", "m_3"] {
        c.put(id, &RsDraft::new(id, 0)).unwrap();
    }
    assert_eq!(
        c.list_ids().unwrap(),
        ["alpha", "m-1", "m.2", "m_3", "zeta"]
    );
    assert_eq!(c.len().unwrap(), 5);
    // Foreign files are not documents.
    fs_err::write(c.dir().join("README.txt"), "x").unwrap();
    fs_err::write(c.dir().join("Bad Name.json"), "x").unwrap();
    assert_eq!(c.len().unwrap(), 5);
}

#[rstest]
#[case("a", true)]
#[case("rs_draft-1.v2", true)]
#[case(&"a".repeat(128), true)]
#[case(&"a".repeat(129), false)]
#[case("", false)]
#[case("A", false)]
#[case("a b", false)]
#[case("../x", false)]
#[case("a/b", false)]
#[case("a\\b", false)]
#[case(".hidden", false)]
#[case("x.", false)]
#[case("a..b", false)]
#[case("index", false)]
#[case("con", false)]
#[case("é", false)]
#[case("a\0b", false)]
fn id_validation_table(#[case] id: &str, #[case] ok: bool) {
    assert_eq!(validate_id(id).is_ok(), ok, "{id:?}");
}

#[test]
fn invalid_ids_never_touch_the_file_system() {
    let (_t, c) = open();
    for id in ["../escape", "a/b", "", "INDEX", "index"] {
        assert_eq!(
            c.put(id, &RsDraft::new("x", 0)).unwrap_err().code(),
            "store.id-invalid"
        );
        assert_eq!(c.get(id).unwrap_err().code(), "store.id-invalid");
        assert_eq!(c.delete(id).unwrap_err().code(), "store.id-invalid");
        assert_eq!(c.exists(id).unwrap_err().code(), "store.id-invalid");
    }
    let parent = c.dir().parent().unwrap();
    assert!(!parent.join("escape.json").exists());
}

#[test]
fn delete_removes_and_reports_absence() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("x", 1)).unwrap();
    assert!(c.delete("a").unwrap());
    assert!(!c.delete("a").unwrap());
    assert!(c.get("a").unwrap().is_none());
}

#[rstest]
#[case::not_json("{ nope", "invalid JSON")]
#[case::not_object("[1,2]", "not a JSON object")]
#[case::wrong_kind(r#"{"kind":"other","v":2,"data":{}}"#, "kind")]
#[case::no_kind(r#"{"v":2,"data":{}}"#, "no kind")]
#[case::no_version(r#"{"kind":"rs-draft","data":{}}"#, "version")]
#[case::zero_version(r#"{"kind":"rs-draft","v":0,"data":{}}"#, "version")]
#[case::no_data(r#"{"kind":"rs-draft","v":2}"#, "no data")]
#[case::bad_shape(r#"{"kind":"rs-draft","v":2,"data":{"name":5,"count":1}}"#, "name")]
fn corrupt_documents_are_quarantined_with_a_reason(#[case] text: &str, #[case] needle: &str) {
    let (_t, c) = open();
    c.put("good", &RsDraft::new("RS_Good", 1)).unwrap();
    write_raw(&c, "bad", text);
    let err = c.get("bad").unwrap_err();
    assert_eq!(err.code(), "store.document-corrupt");
    let StoreError::DocumentCorrupt {
        quarantined,
        reason,
        ..
    } = err
    else {
        panic!()
    };
    assert!(reason.contains(needle), "{reason}");
    let moved = quarantined.unwrap();
    assert_eq!(fs_err::read_to_string(&moved).unwrap(), text);
    assert!(!c.dir().join("bad.json").exists());
    assert!(c.get("bad").unwrap().is_none(), "second read finds nothing");
    let q = c.quarantined();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].id, "bad");
    assert!(q[0].reason.contains(needle));
    assert_eq!(c.get("good").unwrap().unwrap().value.name, "RS_Good");
    assert_eq!(c.list_ids().unwrap(), ["good"]);
}

#[test]
fn scan_reports_bad_documents_and_continues() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    write_raw(&c, "b", "garbage");
    c.put("c", &RsDraft::new("RS_C", 3)).unwrap();
    let items: Vec<_> = c.scan().unwrap().collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].as_ref().unwrap().id, "a");
    assert_eq!(
        items[1].as_ref().unwrap_err().code(),
        "store.document-corrupt"
    );
    assert_eq!(items[2].as_ref().unwrap().loaded.value.count, 3);
    assert_eq!(c.quarantined().len(), 1);
}

#[test]
fn quarantine_names_do_not_collide() {
    let (_t, c) = open();
    for _ in 0..3 {
        write_raw(&c, "x", "bad");
        assert!(c.get("x").is_err());
    }
    assert_eq!(c.quarantined().len(), 3);
}

#[test]
fn older_documents_are_migrated_on_read_and_written_back_by_get() {
    let (_t, c) = open();
    let old = r#"{"kind":"rs-draft","v":1,"data":{"label":"RS_Old","count":4}}"#;
    write_raw(&c, "a", old);
    write_raw(&c, "b", old);
    // scan migrates in memory only.
    let first = c.scan().unwrap().next().unwrap().unwrap();
    assert_eq!(first.loaded.origin, LoadOrigin::Migrated { from: 1 });
    assert_eq!(first.loaded.value, RsDraft::new("RS_Old", 4));
    assert_eq!(fs_err::read_to_string(c.dir().join("a.json")).unwrap(), old);
    // get writes the migrated form back.
    let l = c.get("a").unwrap().unwrap();
    assert_eq!(l.origin, LoadOrigin::Migrated { from: 1 });
    let on_disk: Value =
        serde_json::from_str(&fs_err::read_to_string(c.dir().join("a.json")).unwrap()).unwrap();
    assert_eq!(on_disk["v"], 2);
    assert_eq!(on_disk["data"], json!({"name": "RS_Old", "count": 4}));
    assert_eq!(c.get("a").unwrap().unwrap().origin, LoadOrigin::File);
    assert_eq!(fs_err::read_to_string(c.dir().join("b.json")).unwrap(), old);
}

#[test]
fn failing_migration_quarantines_the_document() {
    let (_t, c) = open();
    write_raw(&c, "a", r#"{"kind":"rs-draft","v":1,"data":{"count":4}}"#);
    let err = c.get("a").unwrap_err();
    assert_eq!(err.code(), "store.document-corrupt");
    assert!(c.quarantined()[0].reason.contains("label missing"));
}

#[test]
fn newer_documents_are_read_only_and_protected() {
    let (_t, c) = open();
    let newer = r#"{"kind":"rs-draft","v":7,"data":{"name":"RS_Future","count":9,"extra":true}}"#;
    write_raw(&c, "a", newer);
    let l = c.get("a").unwrap().unwrap();
    assert!(l.read_only);
    assert_eq!(l.origin, LoadOrigin::NewerThanApp { found: 7 });
    assert_eq!(l.value.name, "RS_Future");
    assert_eq!(
        c.put("a", &RsDraft::new("x", 1)).unwrap_err().code(),
        "store.newer-than-app"
    );
    assert_eq!(c.delete("a").unwrap_err().code(), "store.newer-than-app");
    assert_eq!(
        fs_err::read_to_string(c.dir().join("a.json")).unwrap(),
        newer
    );
    assert!(c.quarantined().is_empty());
}

#[test]
fn undecodable_newer_document_is_an_error_but_never_moved() {
    let (_t, c) = open();
    let newer = r#"{"kind":"rs-draft","v":7,"data":{"shape":"changed"}}"#;
    write_raw(&c, "a", newer);
    assert_eq!(c.get("a").unwrap_err().code(), "store.newer-than-app");
    assert_eq!(
        fs_err::read_to_string(c.dir().join("a.json")).unwrap(),
        newer
    );
    assert!(c.quarantined().is_empty());
}

#[test]
fn put_over_an_unreadable_file_quarantines_the_old_bytes() {
    let (_t, c) = open();
    write_raw(&c, "a", "{{{");
    c.put("a", &RsDraft::new("RS_New", 1)).unwrap();
    assert_eq!(c.get("a").unwrap().unwrap().value.name, "RS_New");
    assert_eq!(c.quarantined().len(), 1);
}

fn summary(d: &RsDraft) -> Value {
    json!({"title": d.name})
}

#[test]
fn index_tracks_puts_and_deletes_and_carries_summaries() {
    let (_t, c) = open();
    let c = c.with_summarizer(summary);
    c.put("b", &RsDraft::new("RS_B", 2)).unwrap();
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    let s = c.list_summaries().unwrap();
    assert_eq!(
        s.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(s[0].summary, json!({"title": "RS_A"}));
    assert_eq!(s[0].v, 2);
    assert!(c.dir().join("index.json").is_file());
    c.put("c", &RsDraft::new("RS_C", 3)).unwrap();
    c.delete("a").unwrap();
    let s = c.list_summaries().unwrap();
    assert_eq!(
        s.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["b", "c"]
    );
    // The index file on disk matches the listing.
    let on_disk: Value =
        serde_json::from_str(&fs_err::read_to_string(c.dir().join("index.json")).unwrap()).unwrap();
    assert_eq!(on_disk["entries"].as_object().unwrap().len(), 2);
}

#[test]
fn missing_damaged_and_stale_indexes_are_rebuilt() {
    let (_t, c) = open();
    let c = c.with_summarizer(summary);
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    c.put("b", &RsDraft::new("RS_B", 2)).unwrap();
    let idx = c.dir().join("index.json");
    let baseline = c.list_summaries().unwrap();

    fs_err::remove_file(&idx).unwrap();
    let fresh = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    assert_eq!(fresh.list_summaries().unwrap(), baseline);
    assert!(idx.is_file());

    fs_err::write(&idx, "not json").unwrap();
    let fresh = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    assert_eq!(fresh.list_summaries().unwrap(), baseline);
    let rebuilt: Value = serde_json::from_str(&fs_err::read_to_string(&idx).unwrap()).unwrap();
    assert_eq!(rebuilt["kind"], "collection-index");

    // A document added behind the back of the index.
    write_raw(
        &c,
        "c",
        r#"{"kind":"rs-draft","v":2,"data":{"name":"RS_C","count":3}}"#,
    );
    let fresh = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    let s = fresh.list_summaries().unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!(s[2].summary, json!({"title": "RS_C"}));

    // A document removed behind its back, and one whose size changed.
    fs_err::remove_file(c.dir().join("a.json")).unwrap();
    write_raw(
        &c,
        "b",
        r#"{"kind":"rs-draft","v":2,"data":{"name":"RS_B_longer_name","count":3}}"#,
    );
    let fresh = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    let s = fresh.list_summaries().unwrap();
    assert_eq!(
        s.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["b", "c"]
    );
    assert_eq!(s[0].summary, json!({"title": "RS_B_longer_name"}));
}

#[test]
fn rebuild_never_changes_documents_and_is_deterministic() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    c.put("b", &RsDraft::new("RS_B", 2)).unwrap();
    let before = fs_err::read(c.dir().join("a.json")).unwrap();
    c.rebuild_index().unwrap();
    let one = fs_err::read(c.dir().join("index.json")).unwrap();
    c.rebuild_index().unwrap();
    let two = fs_err::read(c.dir().join("index.json")).unwrap();
    assert_eq!(one, two);
    assert_eq!(fs_err::read(c.dir().join("a.json")).unwrap(), before);
}

#[test]
fn rebuild_quarantines_corrupt_documents_and_lists_the_rest() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    write_raw(&c, "b", "oops");
    let s = c.list_summaries().unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(c.quarantined().len(), 1);
    // A second listing is stable and does not deadlock.
    assert_eq!(c.list_summaries().unwrap(), s);
}

#[test]
fn backups_keep_replaced_and_deleted_documents() {
    let (_t, c) = open();
    let c = c.with_options(CollectionOptions {
        backup: Some(BackupPolicy::light()),
        verify_writes: true,
        durable: false,
        ops: None,
    });
    c.put("a", &RsDraft::new("RS_V1", 1)).unwrap();
    c.put("a", &RsDraft::new("RS_V2", 2)).unwrap();
    let dir = c.dir().join(".backups").join("a.json");
    let count = || fs_err::read_dir(&dir).unwrap().count();
    assert_eq!(count(), 1);
    c.delete("a").unwrap();
    assert_eq!(count(), 2);
    let any = fs_err::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| fs_err::read_to_string(e.path()).unwrap())
        .collect::<Vec<_>>();
    assert!(any.iter().any(|t| t.contains("RS_V1")));
    assert!(any.iter().any(|t| t.contains("RS_V2")));
    assert!(c.list_ids().unwrap().is_empty());
}

/// Fails the n-th and every later call, as a dead process would.
struct CrashAt {
    inner: RealOps,
    at: usize,
    calls: AtomicUsize,
}

impl CrashAt {
    fn gate(&self) -> std::io::Result<()> {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= self.at {
            Err(std::io::Error::other("injected crash"))
        } else {
            Ok(())
        }
    }
}

impl FsOps for CrashAt {
    fn create_dir_all(&self, d: &Utf8Path) -> std::io::Result<()> {
        self.gate()?;
        self.inner.create_dir_all(d)
    }
    fn write_temp(&self, t: &Utf8Path, b: &[u8]) -> std::io::Result<()> {
        self.gate()?;
        self.inner.write_temp(t, b)
    }
    fn sync_file(&self, p: &Utf8Path) -> std::io::Result<()> {
        self.gate()?;
        self.inner.sync_file(p)
    }
    fn rename(&self, a: &Utf8Path, b: &Utf8Path) -> std::io::Result<()> {
        self.gate()?;
        self.inner.rename(a, b)
    }
    fn sync_dir(&self, d: &Utf8Path) -> std::io::Result<()> {
        self.gate()?;
        self.inner.sync_dir(d)
    }
    fn remove_file(&self, p: &Utf8Path) -> std::io::Result<()> {
        self.gate()?;
        self.inner.remove_file(p)
    }
}

#[test]
fn crash_during_put_leaves_the_old_document_readable() {
    // A put runs: create_dir, write_temp, sync_file, rename, sync_dir (document), then the same
    // five steps for the index when one exists.
    for at in 0..10 {
        let (_t, c) = open();
        c.put("a", &RsDraft::new("RS_Old", 1)).unwrap();
        c.list_summaries().unwrap();
        let crashing = c.clone().with_options(CollectionOptions {
            ops: Some(Arc::new(CrashAt {
                inner: RealOps { durable: false },
                at,
                calls: AtomicUsize::new(0),
            })),
            ..CollectionOptions::default()
        });
        let result = crashing.put("a", &RsDraft::new("RS_New", 2));
        // Whatever happened, a fresh open reads a complete document.
        let fresh = Collection::<RsDraft>::open_dir(c.dir(), clock()).unwrap();
        let l = fresh.get("a").unwrap().unwrap();
        assert!(
            l.value.name == "RS_Old" || l.value.name == "RS_New",
            "step {at}: {:?}",
            l.value
        );
        if at <= 3 {
            assert!(result.is_err(), "step {at}");
            assert_eq!(l.value.name, "RS_Old", "step {at}");
        }
        // The index is repaired from the folder whatever state the crash left.
        let s = fresh.list_summaries().unwrap();
        assert_eq!(s.len(), 1, "step {at}");
        assert!(fresh.quarantined().is_empty(), "step {at}");
    }
}

#[test]
fn stale_temp_files_are_not_documents_and_are_cleaned_on_open() {
    let (t, c) = open();
    let tmp = c.dir().join(format!(".a.json{TEMP_MARKER}1-1"));
    fs_err::write(&tmp, "partial").unwrap();
    assert!(c.list_ids().unwrap().is_empty());
    // Younger than a minute: kept on open. Make it old by reopening after setting the mtime.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    fs_err::File::options()
        .write(true)
        .open(&tmp)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let _again = Collection::<RsDraft>::open_dir(
        Utf8Path::from_path(t.path()).unwrap().join("drafts"),
        clock(),
    )
    .unwrap();
    assert!(!tmp.exists());
}

#[test]
fn documents_survive_reopening() {
    let (_t, c) = open();
    c.put("a", &RsDraft::new("RS_A", 1)).unwrap();
    let again = Collection::<RsDraft>::open_dir(c.dir(), clock()).unwrap();
    assert_eq!(again.get("a").unwrap().unwrap().value.name, "RS_A");
}

#[test]
fn concurrent_puts_and_listings_are_safe() {
    let (_t, c) = open();
    let c = c.with_summarizer(summary);
    std::thread::scope(|s| {
        for worker in 0..8u32 {
            let c = c.clone();
            s.spawn(move || {
                for i in 0..25u32 {
                    c.put(&format!("w{worker}-{i}"), &RsDraft::new("RS_X", i))
                        .unwrap();
                    c.put("shared", &RsDraft::new("RS_S", worker * 100 + i))
                        .unwrap();
                    if i % 5 == 0 {
                        c.list_summaries().unwrap();
                    }
                }
            });
        }
    });
    assert_eq!(c.len().unwrap(), 8 * 25 + 1);
    assert_eq!(c.list_summaries().unwrap().len(), 8 * 25 + 1);
    assert!(c.get("shared").unwrap().is_some());
    assert!(c.quarantined().is_empty());
}

#[test]
fn bulk_listing_and_scan_of_a_thousand_documents() {
    let (_t, c) = open();
    let c = c.with_options(CollectionOptions::cache());
    for i in 0..1000u32 {
        c.put(&format!("doc-{i:05}"), &RsDraft::new("RS_Bulk", i))
            .unwrap();
    }
    assert_eq!(c.list_summaries().unwrap().len(), 1000);
    let n = c.scan().unwrap().filter(Result::is_ok).count();
    assert_eq!(n, 1000);
}

#[test]
#[ignore = "timing run: cargo test -p rimstudio-io --test collection -- --ignored --nocapture"]
fn ten_thousand_documents_listing_and_scan_timing() {
    let (_t, c) = open();
    let c = c
        .with_options(CollectionOptions::cache())
        .with_summarizer(summary);
    let t0 = Instant::now();
    for i in 0..10_000u32 {
        c.put(&format!("doc-{i:05}"), &RsDraft::new("RS_Bulk", i))
            .unwrap();
    }
    println!(
        "put x10000 (no fsync, no index until the first listing): {:?}",
        t0.elapsed()
    );

    let t0 = Instant::now();
    let ids = c.list_ids().unwrap();
    println!("list_ids ({}): {:?}", ids.len(), t0.elapsed());

    let t0 = Instant::now();
    let rows = c.list_summaries().unwrap();
    println!(
        "list_summaries from a warm index ({}): {:?}",
        rows.len(),
        t0.elapsed()
    );

    let cold = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    let t0 = Instant::now();
    let rows = cold.list_summaries().unwrap();
    println!(
        "list_summaries from the index file after reopen ({}): {:?}",
        rows.len(),
        t0.elapsed()
    );

    std::fs::remove_file(c.dir().join("index.json")).unwrap();
    let cold = Collection::<RsDraft>::open_dir(c.dir(), clock())
        .unwrap()
        .with_summarizer(summary);
    let t0 = Instant::now();
    let rows = cold.list_summaries().unwrap();
    println!(
        "list_summaries with the index rebuilt ({}): {:?}",
        rows.len(),
        t0.elapsed()
    );

    let t0 = Instant::now();
    let n = c.scan().unwrap().filter(Result::is_ok).count();
    println!("scan of all documents ({n}): {:?}", t0.elapsed());
    assert_eq!(n, 10_000);
}
