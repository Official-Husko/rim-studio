//! Property tests for the invariants of the storage layer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use rimstudio_core::ports::Clock;
use rimstudio_io::backup::{BackupPolicy, select_keep};
use rimstudio_io::collection::{Collection, CollectionOptions, validate_id};
use rimstudio_io::guard::{RootGuard, sanitize_name, to_safe_component};
use rimstudio_io::jsonc::{JsoncEditor, parse_value, values_equal};
use rimstudio_io::schema::Versioned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
struct RsDoc {
    n: u32,
}

impl Versioned for RsDoc {
    const KIND: &'static str = "rs-prop-doc";
    const VERSION: u32 = 1;
}

proptest! {
    #[test]
    fn accepted_ids_are_safe_file_names(id in ".{0,140}") {
        if validate_id(&id).is_ok() {
            prop_assert!(id.len() <= 128 && !id.is_empty());
            prop_assert!(id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')));
            prop_assert!(!id.starts_with('.') && !id.contains(".."));
            prop_assert_ne!(id.as_str(), "index");
            let file_name = format!("{}.json", id);
            prop_assert!(sanitize_name(&file_name).is_ok());
        }
    }

    #[test]
    fn sanitized_names_are_single_components(name in ".{0,300}") {
        if let Ok(ok) = sanitize_name(&name) {
            prop_assert!(!ok.contains('/') && !ok.contains('\\') && !ok.contains('\0'));
            prop_assert!(ok != "." && ok != "..");
            prop_assert!(Utf8Path::new(&ok).components().count() == 1);
        }
        let safe = to_safe_component(&name);
        prop_assert!(sanitize_name(&safe).is_ok(), "{:?} -> {:?}", name, safe);
    }

    #[test]
    fn guard_never_validates_a_path_outside_its_root(
        segs in proptest::collection::vec(prop_oneof![Just(".."), Just("."), Just("a"), Just("b"), Just(""), Just("..."), Just("c d")], 0..8),
        sep in prop_oneof![Just("/"), Just("//")],
    ) {
        let t = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(t.path().join("root")).unwrap();
        fs_err::create_dir_all(root.join("a")).unwrap();
        let guard = RootGuard::new([&root]).unwrap();
        let candidate = Utf8PathBuf::from(format!("{root}{sep}{}", segs.join(sep)));
        if let Ok(v) = guard.validate(&candidate) {
            let canon_root = Utf8PathBuf::from_path_buf(dunce::canonicalize(&root).unwrap()).unwrap();
            prop_assert!(v.path.starts_with(&canon_root), "{} escaped to {}", candidate, v.path);
            prop_assert!(!segs.contains(&".."));
        }
    }

    #[test]
    fn retention_always_keeps_the_newest_and_never_more_than_exists(
        mut stamps in proptest::collection::vec(0u64..4_000_000_000_000, 0..40),
        keep_last in 0usize..6,
        today in any::<bool>(),
        window in 0u32..20,
        now in 1_000_000_000_000u64..4_000_000_000_000,
    ) {
        stamps.sort_unstable();
        let policy = BackupPolicy { keep_last, keep_all_today: today, daily_window_days: window, skip_identical: false, root: None };
        let keep = select_keep(&stamps, now, &policy);
        prop_assert_eq!(keep.len(), stamps.len());
        if !stamps.is_empty() {
            prop_assert!(*keep.last().unwrap());
            prop_assert!(keep.iter().filter(|k| **k).count() >= keep_last.min(stamps.len()).max(1));
            // The newest keep_last entries are all kept.
            for k in keep.iter().rev().take(keep_last) {
                prop_assert!(*k);
            }
        }
    }
}

fn json_leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-1000i64..1000).prop_map(Value::from),
        "[a-z ]{0,8}".prop_map(Value::from),
    ]
}

fn json_value() -> impl Strategy<Value = Value> {
    json_leaf().prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            proptest::collection::btree_map("[a-z]{1,4}", inner, 0..4)
                .prop_map(|m| Value::Object(m.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

fn json_object() -> impl Strategy<Value = Value> {
    proptest::collection::btree_map("[a-z]{1,4}", json_value(), 0..5)
        .prop_map(|m| Value::Object(m.into_iter().collect::<Map<_, _>>()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn sync_with_makes_the_text_mean_the_new_value(old in json_object(), new in json_object()) {
        let pretty = serde_json::to_string_pretty(&old).unwrap();
        let text = format!("// leading comment\n{pretty}\n");
        let path = Utf8Path::new("/p.jsonc");
        let ed = JsoncEditor::parse(&text, path).unwrap();
        ed.sync_with(&new).unwrap();
        let out = ed.finish();
        let back = parse_value(&out, path).unwrap();
        prop_assert!(values_equal(&back, &new), "{} != {}\n{}", back, new, out);
        prop_assert!(out.contains("// leading comment"));
    }

    #[test]
    fn sync_with_an_equal_value_changes_no_byte(v in json_object()) {
        let text = format!("// c\n{}\n", serde_json::to_string_pretty(&v).unwrap());
        let ed = JsoncEditor::parse(&text, Utf8Path::new("/p.jsonc")).unwrap();
        ed.sync_with(&v).unwrap();
        prop_assert_eq!(ed.finish(), text);
    }
}

#[derive(Debug, Clone)]
enum Op {
    Put(u8, u32),
    Delete(u8),
    Get(u8),
    List,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..6, any::<u32>()).prop_map(|(i, n)| Op::Put(i, n)),
        (0u8..6).prop_map(Op::Delete),
        (0u8..6).prop_map(Op::Get),
        Just(Op::List),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn collection_behaves_like_a_map(ops in proptest::collection::vec(op(), 1..40)) {
        let t = tempfile::tempdir().unwrap();
        let dir = Utf8PathBuf::from_path_buf(t.path().join("c")).unwrap();
        let c = Collection::<RsDoc>::open_dir(&dir, clock()).unwrap().with_options(CollectionOptions::cache());
        let mut model: BTreeMap<String, u32> = BTreeMap::new();
        for op in ops {
            match op {
                Op::Put(i, n) => {
                    let id = format!("doc-{i}");
                    c.put(&id, &RsDoc { n }).unwrap();
                    model.insert(id, n);
                }
                Op::Delete(i) => {
                    let id = format!("doc-{i}");
                    prop_assert_eq!(c.delete(&id).unwrap(), model.remove(&id).is_some());
                }
                Op::Get(i) => {
                    let id = format!("doc-{i}");
                    let got = c.get(&id).unwrap().map(|l| l.value.n);
                    prop_assert_eq!(got, model.get(&id).copied());
                }
                Op::List => {
                    let ids: Vec<String> = model.keys().cloned().collect();
                    prop_assert_eq!(c.list_ids().unwrap(), ids.clone());
                    let listed: Vec<String> = c.list_summaries().unwrap().into_iter().map(|e| e.id).collect();
                    prop_assert_eq!(listed, ids);
                }
            }
        }
        prop_assert_eq!(c.len().unwrap(), model.len());
        prop_assert!(c.quarantined().is_empty());
    }
}
