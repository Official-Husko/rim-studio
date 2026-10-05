//! JSONC reading, comment preserving edits and the stable writer for new files.
//!
//! # Findings of spike S-02 (what the `jsonc-parser` CST API supports)
//!
//! Verified by the tests in this module against `jsonc-parser` 0.34.0 with the `cst` feature:
//!
//! - Replace a value: `CstObjectProp::set_value` replaces scalars, arrays and objects and keeps the
//!   comments around the property.
//! - Insert a key at a chosen position: `CstObject::insert(index, name, value)` and `append`; the
//!   new property takes the indentation of its siblings.
//! - Remove a key: `CstObjectProp::remove` removes the property with its comma and the comments
//!   attached to it, and leaves every other comment untouched.
//! - Append, insert and remove array elements: `CstArray::append`, `insert`, `CstNode::remove`;
//!   replace an element that is an object with `CstObject::replace_with`.
//! - Trailing commas, line endings and indentation of the existing file are kept.
//! - Missing: there is no "set at path" helper, so [`JsoncEditor`] walks the path itself; numbers
//!   are inserted as raw text, so the editor formats them through `serde_json`; the CST types are
//!   `Rc` based and not `Send`, so an editor lives on one thread inside one call.
//! - Limits that matter: comments inside an array or object that is replaced as a whole are lost
//!   (so [`JsoncEditor::sync_with`] recurses into objects and only replaces what differs), and a
//!   new key inserted before the first property goes after the leading comment block.
//!
//! The fallback of the design (a serde rewrite with a warning) is therefore not needed for the
//! operations the app uses; it remains available through [`render_new`] for hostile files.

use camino::{Utf8Path, Utf8PathBuf};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstNode, CstObject, CstRootNode};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::StoreError;

/// Removes a leading UTF-8 byte order mark.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn parse_error(path: &Utf8Path, e: &jsonc_parser::errors::ParseError) -> StoreError {
    StoreError::Parse {
        path: path.to_owned(),
        message: e.kind().to_string(),
        line: e.line_display(),
        column: e.column_display(),
    }
}

/// Parses JSONC text (comments and trailing commas allowed) into a JSON value. Empty or comment
/// only text gives `null`.
pub fn parse_value(text: &str, path: &Utf8Path) -> Result<Value, StoreError> {
    let text = strip_bom(text);
    jsonc_parser::parse_to_serde_value::<Value>(text, &ParseOptions::default())
        .map_err(|e| parse_error(path, &e))
}

/// Parses JSONC text into `T`, reporting the field path of a mismatch.
pub fn from_str<T: DeserializeOwned>(
    text: &str,
    path: &Utf8Path,
    kind: &'static str,
) -> Result<T, StoreError> {
    let value = parse_value(text, path)?;
    crate::schema::decode_value(value, kind, path)
}

/// Pretty JSON bytes with two space indentation, `\n` line ends and a final newline. Key order is
/// the order of the value (the crate enables `preserve_order`), so equal values give equal bytes.
pub fn to_json_bytes(value: &Value) -> Result<Vec<u8>, StoreError> {
    let mut out = serde_json::to_vec_pretty(value).map_err(|e| StoreError::Serialize {
        message: e.to_string(),
    })?;
    out.push(b'\n');
    Ok(out)
}

/// Renders a new JSONC file: each line of `header` as a `//` comment, then the pretty value.
pub fn render_new(header: &str, value: &Value) -> Result<String, StoreError> {
    let mut out = String::new();
    for line in header.lines() {
        if line.is_empty() {
            out.push_str("//\n");
        } else {
            out.push_str("// ");
            out.push_str(line);
            out.push('\n');
        }
    }
    let body = to_json_bytes(value)?;
    out.push_str(&String::from_utf8_lossy(&body));
    Ok(out)
}

/// Equality that treats `1` and `1.0` as the same number.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x == y {
                return true;
            }
            match (x.as_f64(), y.as_f64()) {
                (Some(p), Some(q)) => p == q && (x.is_f64() || y.is_f64()),
                _ => false,
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| values_equal(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| values_equal(v, w)))
        }
        _ => a == b,
    }
}

/// Converts a JSON value to the CST input form.
pub fn to_input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(to_input).collect()),
        Value::Object(map) => {
            CstInputValue::Object(map.iter().map(|(k, v)| (k.clone(), to_input(v))).collect())
        }
    }
}

/// A comment preserving editor over one JSONC document. Edits touch only the nodes they name.
pub struct JsoncEditor {
    root: CstRootNode,
    path: Utf8PathBuf,
}

impl std::fmt::Debug for JsoncEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsoncEditor")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl JsoncEditor {
    /// Parses `text` (a leading byte order mark is removed). Empty text starts an empty object.
    pub fn parse(text: &str, path: &Utf8Path) -> Result<Self, StoreError> {
        let text = strip_bom(text);
        let root = CstRootNode::parse(text, &ParseOptions::default())
            .map_err(|e| parse_error(path, &e))?;
        Ok(JsoncEditor {
            root,
            path: path.to_owned(),
        })
    }

    fn err(&self, message: impl Into<String>) -> StoreError {
        StoreError::Edit {
            path: self.path.clone(),
            message: message.into(),
        }
    }

    /// The root object, created for an empty document.
    fn root_object(&self) -> Result<CstObject, StoreError> {
        if self.root.value().is_none() {
            return Ok(self.root.object_value_or_set());
        }
        self.root
            .object_value()
            .ok_or_else(|| self.err("the document root is not an object"))
    }

    /// Walks object keys from the root. With `create`, missing objects are created.
    fn object_at(&self, path: &[&str], create: bool) -> Result<Option<CstObject>, StoreError> {
        let mut cur = self.root_object()?;
        for seg in path {
            cur = match cur.get(seg) {
                Some(prop) => prop
                    .value()
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| self.err(format!("`{seg}` is not an object")))?,
                None if create => cur
                    .object_value_or_create(seg)
                    .ok_or_else(|| self.err(format!("cannot create `{seg}`")))?,
                None => return Ok(None),
            };
        }
        Ok(Some(cur))
    }

    /// The value at an object key path as JSON, `None` when absent.
    pub fn get(&self, path: &[&str]) -> Result<Option<Value>, StoreError> {
        let Some((last, parents)) = path.split_last() else {
            return Ok(self.root.to_serde_value());
        };
        let Some(obj) = self.object_at(parents, false)? else {
            return Ok(None);
        };
        Ok(obj
            .get(last)
            .and_then(|p| p.value())
            .and_then(|v| v.to_serde_value()))
    }

    /// Sets the value at an object key path, creating missing parent objects. An equal value is a
    /// no-op; a new key is appended after its siblings.
    pub fn set(&self, path: &[&str], value: &Value) -> Result<(), StoreError> {
        let (last, parents) = path.split_last().ok_or_else(|| self.err("empty path"))?;
        let obj = self
            .object_at(parents, true)?
            .ok_or_else(|| self.err("missing parent"))?;
        match obj.get(last) {
            Some(prop) => sync_prop(&prop, value, None),
            None => {
                obj.append(last, to_input(value));
            }
        }
        Ok(())
    }

    /// Inserts a new key at `index` among the properties of the object at `parents`. An existing
    /// key is replaced in place instead.
    pub fn insert_at(
        &self,
        parents: &[&str],
        index: usize,
        key: &str,
        value: &Value,
    ) -> Result<(), StoreError> {
        let obj = self
            .object_at(parents, true)?
            .ok_or_else(|| self.err("missing parent"))?;
        match obj.get(key) {
            Some(prop) => sync_prop(&prop, value, None),
            None => {
                let at = index.min(obj.properties().len());
                obj.insert(at, key, to_input(value));
            }
        }
        Ok(())
    }

    /// Removes the key at `path`; returns whether it existed.
    pub fn remove(&self, path: &[&str]) -> Result<bool, StoreError> {
        let (last, parents) = path.split_last().ok_or_else(|| self.err("empty path"))?;
        let Some(obj) = self.object_at(parents, false)? else {
            return Ok(false);
        };
        match obj.get(last) {
            Some(prop) => {
                prop.remove();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn array_at(&self, path: &[&str]) -> Result<jsonc_parser::cst::CstArray, StoreError> {
        let (last, parents) = path.split_last().ok_or_else(|| self.err("empty path"))?;
        let obj = self
            .object_at(parents, true)?
            .ok_or_else(|| self.err("missing parent"))?;
        match obj.get(last) {
            Some(prop) => prop
                .value()
                .and_then(|v| v.as_array())
                .ok_or_else(|| self.err(format!("`{last}` is not an array"))),
            None => obj
                .array_value_or_create(last)
                .ok_or_else(|| self.err(format!("cannot create `{last}`"))),
        }
    }

    /// Appends an element to the array at `path`, creating the array when missing.
    pub fn push(&self, path: &[&str], value: &Value) -> Result<(), StoreError> {
        self.array_at(path)?.append(to_input(value));
        Ok(())
    }

    fn find_by_id(
        array: &jsonc_parser::cst::CstArray,
        id_key: &str,
        id: &str,
    ) -> Option<(CstNode, CstObject)> {
        array.elements().into_iter().find_map(|el| {
            let obj = el.as_object()?;
            let v = obj.get(id_key)?.value()?.to_serde_value()?;
            (v.as_str() == Some(id)).then_some((el, obj))
        })
    }

    /// Removes the object element whose `id_key` equals `id` from the array at `path`; returns
    /// whether one was removed. Arrays of objects with an id are edited by id, not by index.
    pub fn remove_by_id(&self, path: &[&str], id_key: &str, id: &str) -> Result<bool, StoreError> {
        let array = self.array_at(path)?;
        match Self::find_by_id(&array, id_key, id) {
            Some((el, _)) => {
                el.remove();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Replaces the object element whose `id_key` equals `id`, or appends `value` when none does.
    /// Properties of the existing element that `value` still has keep their comments.
    pub fn upsert_by_id(
        &self,
        path: &[&str],
        id_key: &str,
        id: &str,
        value: &Value,
    ) -> Result<(), StoreError> {
        let array = self.array_at(path)?;
        match (Self::find_by_id(&array, id_key, id), value) {
            (Some((_, obj)), Value::Object(map)) => sync_object(&obj, map, None),
            (Some((_, obj)), other) => {
                obj.replace_with(to_input(other));
            }
            (None, v) => {
                array.append(to_input(v));
            }
        }
        Ok(())
    }

    /// Makes the document equal to `new` while touching only what differs: removed keys are removed,
    /// new keys are inserted after their predecessor, changed scalars are replaced, nested objects
    /// are merged recursively. Comments, key order and layout of unchanged parts stay as written.
    pub fn sync_with(&self, new: &Value) -> Result<(), StoreError> {
        self.sync_with_baseline(new, None)
    }

    /// Like [`sync_with`](Self::sync_with), with the canonical form of what the document meant
    /// before the change (`baseline`). A key that is in the text but absent from both `new` and the
    /// baseline is kept: the writer skips default valued sections, and a section a person wrote out
    /// explicitly must not vanish (with its comments) just because the app saved. Likewise a key that
    /// is missing from the text and has the same value in the baseline is not written: the text
    /// meant the default, and filling in every default would bury the person's file.
    pub fn sync_with_baseline(
        &self,
        new: &Value,
        baseline: Option<&Value>,
    ) -> Result<(), StoreError> {
        match new {
            Value::Object(map) => {
                let obj = self.root_object()?;
                sync_object(&obj, map, baseline.and_then(Value::as_object));
                Ok(())
            }
            other => {
                self.root.set_value(to_input(other));
                Ok(())
            }
        }
    }

    /// The current document as JSON.
    pub fn to_value(&self) -> Option<Value> {
        self.root.to_serde_value()
    }

    /// The edited text.
    pub fn finish(self) -> String {
        self.root.to_string()
    }
}

fn sync_object(obj: &CstObject, new: &Map<String, Value>, baseline: Option<&Map<String, Value>>) {
    for prop in obj.properties() {
        if let Some(name) = prop.decoded_name()
            && !new.contains_key(&name)
        {
            // Keep keys the baseline did not know either (default valued sections left in the text).
            let known_before = baseline.is_none_or(|b| b.contains_key(&name));
            if known_before {
                prop.remove();
            }
        }
    }
    let mut prev: Option<&str> = None;
    for (key, value) in new {
        match obj.get(key) {
            Some(prop) => sync_prop(&prop, value, baseline.and_then(|b| b.get(key))),
            None => {
                // Absent from the text but unchanged since the baseline: the text meant the
                // default, so nothing is written.
                if baseline
                    .and_then(|b| b.get(key))
                    .is_some_and(|b| values_equal(b, value))
                {
                    continue;
                }
                let index = match prev.and_then(|p| obj.get(p)) {
                    Some(pp) => pp.property_index() + 1,
                    None if prev.is_some() => obj.properties().len(),
                    None => 0,
                };
                obj.insert(index, key, to_input(value));
            }
        }
        prev = Some(key);
    }
}

fn sync_prop(prop: &jsonc_parser::cst::CstObjectProp, new: &Value, baseline: Option<&Value>) {
    let Some(cur) = prop.value() else {
        prop.set_value(to_input(new));
        return;
    };
    if cur
        .to_serde_value()
        .as_ref()
        .is_some_and(|c| values_equal(c, new))
    {
        return;
    }
    match (cur.as_object(), cur.as_array(), new) {
        (Some(obj), _, Value::Object(map)) => {
            sync_object(&obj, map, baseline.and_then(Value::as_object));
        }
        (_, Some(arr), Value::Array(items)) => {
            let elements = arr.elements();
            let objects_only = elements.len() == items.len()
                && elements
                    .iter()
                    .zip(items)
                    .all(|(el, it)| match (el.as_object(), it) {
                        (Some(_), Value::Object(_)) => true,
                        _ => el.to_serde_value().is_some_and(|v| values_equal(&v, it)),
                    });
            if objects_only {
                for (el, it) in elements.iter().zip(items) {
                    if let (Some(obj), Value::Object(map)) = (el.as_object(), it) {
                        sync_object(&obj, map, None);
                    }
                }
            } else {
                prop.set_value(to_input(new));
            }
        }
        _ => prop.set_value(to_input(new)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const P: &str = "/x/settings.jsonc";

    fn editor(text: &str) -> JsoncEditor {
        JsoncEditor::parse(text, Utf8Path::new(P)).unwrap()
    }

    const SAMPLE: &str = r#"// RS_Header comment
{
  // the theme block
  "theme": {
    "accent": "blue", // trailing note
    "dense": false
  },
  /* sources */
  "sources": [
    { "id": "a", "path": "/one" }, // first
    { "id": "b", "path": "/two" }
  ],
  "count": 3,
}
"#;

    #[test]
    fn reads_comments_and_trailing_commas() {
        let v = parse_value(SAMPLE, Utf8Path::new(P)).unwrap();
        assert_eq!(v["theme"]["accent"], "blue");
        assert_eq!(v["count"], 3);
    }

    #[test]
    fn syntax_error_has_line_and_column() {
        let err = parse_value("{\n  \"a\": ,\n}", Utf8Path::new(P)).unwrap_err();
        match err {
            StoreError::Parse { line, column, .. } => {
                assert_eq!(line, 2);
                assert!(column >= 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bom_is_accepted() {
        let v = parse_value("\u{feff}{\"a\":1}", Utf8Path::new(P)).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn replacing_one_value_keeps_everything_else_byte_identical() {
        let ed = editor(SAMPLE);
        ed.set(&["theme", "accent"], &json!("green")).unwrap();
        let out = ed.finish();
        assert_eq!(out, SAMPLE.replace("\"blue\"", "\"green\""));
    }

    #[test]
    fn setting_an_equal_value_changes_nothing() {
        let ed = editor(SAMPLE);
        ed.set(&["count"], &json!(3.0)).unwrap();
        ed.set(&["theme", "dense"], &json!(false)).unwrap();
        assert_eq!(ed.finish(), SAMPLE);
    }

    #[test]
    fn new_key_is_appended_and_old_comments_survive() {
        let ed = editor(SAMPLE);
        ed.set(&["theme", "radius"], &json!(4)).unwrap();
        let out = ed.finish();
        for needle in [
            "// RS_Header comment",
            "// the theme block",
            "// trailing note",
            "/* sources */",
            "// first",
        ] {
            assert!(out.contains(needle), "lost {needle}: {out}");
        }
        let v = parse_value(&out, Utf8Path::new(P)).unwrap();
        assert_eq!(v["theme"]["radius"], 4);
        assert_eq!(v["theme"]["accent"], "blue");
    }

    #[test]
    fn insert_at_puts_the_key_at_the_position() {
        let ed = editor("{\n  \"a\": 1,\n  \"c\": 3\n}\n");
        ed.insert_at(&[], 1, "b", &json!(2)).unwrap();
        let out = ed.finish();
        let v = parse_value(&out, Utf8Path::new(P)).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["a", "b", "c"]);
    }

    #[test]
    fn remove_drops_the_key_and_keeps_other_comments() {
        let ed = editor(SAMPLE);
        assert!(ed.remove(&["theme", "dense"]).unwrap());
        assert!(!ed.remove(&["theme", "missing"]).unwrap());
        assert!(!ed.remove(&["nothing", "here"]).unwrap());
        let out = ed.finish();
        assert!(!out.contains("dense"));
        assert!(out.contains("// trailing note"));
        assert!(out.contains("// the theme block"));
        parse_value(&out, Utf8Path::new(P)).unwrap();
    }

    #[test]
    fn missing_parents_are_created_on_set() {
        let ed = editor("{}\n");
        ed.set(&["a", "b", "c"], &json!(true)).unwrap();
        let v = parse_value(&ed.finish(), Utf8Path::new(P)).unwrap();
        assert_eq!(v["a"]["b"]["c"], true);
    }

    #[test]
    fn empty_document_becomes_an_object() {
        let ed = editor("");
        ed.set(&["k"], &json!("v")).unwrap();
        let v = parse_value(&ed.finish(), Utf8Path::new(P)).unwrap();
        assert_eq!(v["k"], "v");
    }

    #[test]
    fn array_elements_are_edited_by_id() {
        let ed = editor(SAMPLE);
        ed.upsert_by_id(
            &["sources"],
            "id",
            "b",
            &json!({"id": "b", "path": "/moved"}),
        )
        .unwrap();
        ed.upsert_by_id(
            &["sources"],
            "id",
            "c",
            &json!({"id": "c", "path": "/three"}),
        )
        .unwrap();
        assert!(ed.remove_by_id(&["sources"], "id", "a").unwrap());
        assert!(!ed.remove_by_id(&["sources"], "id", "zzz").unwrap());
        let out = ed.finish();
        let v = parse_value(&out, Utf8Path::new(P)).unwrap();
        assert_eq!(
            v["sources"],
            json!([{"id": "b", "path": "/moved"}, {"id": "c", "path": "/three"}])
        );
        assert!(out.contains("/* sources */"));
    }

    #[test]
    fn push_appends_to_an_existing_or_new_array() {
        let ed = editor(SAMPLE);
        ed.push(&["sources"], &json!({"id": "z", "path": "/z"}))
            .unwrap();
        ed.push(&["tags"], &json!("one")).unwrap();
        let v = parse_value(&ed.finish(), Utf8Path::new(P)).unwrap();
        assert_eq!(v["sources"].as_array().unwrap().len(), 3);
        assert_eq!(v["tags"], json!(["one"]));
    }

    #[test]
    fn non_object_parent_is_an_error() {
        let ed = editor("{\"a\": 1}");
        let err = ed.set(&["a", "b"], &json!(1)).unwrap_err();
        assert_eq!(err.code(), "store.edit");
    }

    #[test]
    fn sync_with_applies_a_minimal_diff() {
        let ed = editor(SAMPLE);
        let mut v = parse_value(SAMPLE, Utf8Path::new(P)).unwrap();
        v["theme"]["accent"] = json!("red");
        v["theme"].as_object_mut().unwrap().remove("dense");
        v.as_object_mut().unwrap().insert("extra".into(), json!(1));
        v["count"] = json!(3.0);
        ed.sync_with(&v).unwrap();
        let out = ed.finish();
        assert!(out.contains("// RS_Header comment"));
        assert!(out.contains("// the theme block"));
        assert!(out.contains("// first"));
        assert!(out.contains("/* sources */"));
        assert!(out.contains("\"count\": 3,"), "count rewritten: {out}");
        let back = parse_value(&out, Utf8Path::new(P)).unwrap();
        assert!(values_equal(&back, &v), "{back} != {v}");
    }

    #[test]
    fn sync_with_keeps_key_order_of_untouched_keys() {
        let ed = editor("{\n  \"b\": 1,\n  \"a\": 2\n}\n");
        ed.sync_with(&json!({"b": 1, "a": 5, "c": 9})).unwrap();
        let out = ed.finish();
        let v = parse_value(&out, Utf8Path::new(P)).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["b", "a", "c"]);
    }

    #[test]
    fn render_new_has_header_and_stable_bytes() {
        let v = json!({"b": 1, "a": [1, 2]});
        let one = render_new("RimStudio settings\n\nEdit freely.", &v).unwrap();
        let two = render_new("RimStudio settings\n\nEdit freely.", &v).unwrap();
        assert_eq!(one, two);
        assert!(one.starts_with("// RimStudio settings\n//\n// Edit freely.\n{"));
        assert!(one.ends_with("}\n"));
        assert_eq!(parse_value(&one, Utf8Path::new(P)).unwrap(), v);
    }

    #[test]
    fn integer_and_float_equality() {
        assert!(values_equal(&json!(1), &json!(1.0)));
        assert!(!values_equal(&json!(1), &json!(2.0)));
        assert!(!values_equal(&json!(1), &json!("1")));
    }
}
