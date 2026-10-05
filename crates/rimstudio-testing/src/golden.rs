//! JSON golden files over the `goldenfile` crate.
//!
//! A golden is canonical JSON: object keys sorted, two space indentation, a trailing newline. A
//! normal run compares and fails with a diff; running with `UPDATE_GOLDENFILES=1` rewrites the file
//! (review the diff before committing). Goldens must contain synthetic data only.

use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

/// The environment variable that turns comparison into regeneration.
pub const UPDATE_VAR: &str = "UPDATE_GOLDENFILES";

/// True when `UPDATE_GOLDENFILES` is set to something other than empty or `0`.
pub fn update_requested() -> bool {
    std::env::var(UPDATE_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

fn sort_value(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = Map::new();
            for (k, val) in entries {
                out.insert(k, sort_value(val));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_value).collect()),
        other => other,
    }
}

/// Serialises `value` as canonical JSON text: sorted keys, two space indent, trailing newline.
///
/// # Panics
/// When the value cannot be serialised (a non string map key, for example); goldens are test code.
#[allow(clippy::expect_used)]
pub fn to_canonical_json<T: Serialize + ?Sized>(value: &T) -> String {
    let v = serde_json::to_value(value).expect("golden value must serialise to JSON");
    let mut text = serde_json::to_string_pretty(&sort_value(v)).expect("JSON value prints");
    text.push('\n');
    text
}

/// Compares `value` with the golden file `<dir>/<name>.json` (a name that already ends in `.json`
/// is used as is). With `UPDATE_GOLDENFILES=1` the file is written instead.
///
/// # Panics
/// When the golden is missing or differs (the message names the file and the update hint), or when
/// the folder cannot be written in update mode.
#[allow(clippy::expect_used)]
pub fn assert_json_golden<T: Serialize + ?Sized>(dir: impl AsRef<Path>, name: &str, value: &T) {
    let dir = dir.as_ref();
    let file = if name.ends_with(".json") {
        name.to_owned()
    } else {
        format!("{name}.json")
    };
    let text = to_canonical_json(value);
    if !update_requested() && !dir.join(&file).exists() {
        panic!(
            "golden file {} is missing; create it with {UPDATE_VAR}=1 and review it",
            dir.join(&file).display()
        );
    }
    let mut mint = goldenfile::Mint::new(dir);
    let mut out = mint
        .new_goldenfile(&file)
        .expect("golden folder must be writable");
    out.write_all(text.as_bytes())
        .expect("golden file must accept the text");
    // Dropping the mint compares (or regenerates when UPDATE_GOLDENFILES is set) and prints a diff.
}

/// Compares a value with `tests/data/<name>.json` of the calling crate.
///
/// ```text
/// golden_json!("my-report", &report);
/// ```
#[macro_export]
macro_rules! golden_json {
    ($name:expr, $value:expr) => {
        $crate::golden::assert_json_golden(
            ::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data"),
            $name,
            $value,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_and_ends_with_a_newline() {
        let text =
            to_canonical_json(&json!({"b": 1, "a": {"z": [2, {"y": 1, "x": 2}], "c": null}}));
        assert_eq!(
            text,
            "{\n  \"a\": {\n    \"c\": null,\n    \"z\": [\n      2,\n      {\n        \"x\": 2,\n        \"y\": 1\n      }\n    ]\n  },\n  \"b\": 1\n}\n"
        );
    }

    #[test]
    fn canonical_json_is_identical_for_different_key_orders() {
        let a = to_canonical_json(&json!({"a": 1, "b": 2}));
        let b = to_canonical_json(&json!({"b": 2, "a": 1}));
        assert_eq!(a, b);
    }

    #[test]
    fn matching_golden_passes_and_name_suffix_is_optional() {
        let dir = tempfile::tempdir().unwrap();
        let value = json!({"k": [1, 2]});
        std::fs::write(dir.path().join("one.json"), to_canonical_json(&value)).unwrap();
        assert_json_golden(dir.path(), "one", &value);
        assert_json_golden(dir.path(), "one.json", &value);
    }

    #[test]
    #[should_panic(expected = "UPDATE_GOLDENFILES")]
    fn missing_golden_panics_with_the_update_hint() {
        if update_requested() {
            // In update mode a missing golden is created, so emulate the failure for the test.
            panic!("UPDATE_GOLDENFILES is set");
        }
        let dir = tempfile::tempdir().unwrap();
        assert_json_golden(dir.path(), "absent", &json!({}));
    }

    #[test]
    #[should_panic]
    fn differing_golden_panics() {
        if update_requested() {
            panic!("update mode rewrites instead of failing");
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("g.json"), "{}\n").unwrap();
        assert_json_golden(dir.path(), "g", &json!({"changed": true}));
    }

    #[test]
    fn update_mode_flag_reads_the_variable() {
        // Only checks that the helper does not panic; the variable is process global.
        let _ = update_requested();
    }
}
