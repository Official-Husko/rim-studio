//! Settings use cases: get and comment preserving update of settings.jsonc.
//!
//! The file lives in the config root. A first run creates it with explanatory comments and no
//! values (defaults are never written, only keys the user changed exist). [`update`] applies section
//! patches with merge patch semantics (a `null` resets a key to its default), validates the result
//! with the core helpers and saves through `rimstudio-io`'s store, which syncs the change into the
//! existing text so comments, key order and layout elsewhere stay byte identical.
//!
//! A value that fails validation in the file is kept and reported by [`get`]; the effective value
//! of that section is the default for the run. A file with a syntax error, or one written by a newer
//! version, is read only: [`get`] still answers and [`update`] refuses.

use camino::Utf8PathBuf;
use rimstudio_core::settings::{SettingProblem, Settings};
use rimstudio_io::atomic::atomic_write;
use rimstudio_io::error::StoreError;
use rimstudio_io::jsonc::{JsoncEditor, values_equal};
use rimstudio_io::schema::{LoadOrigin, Loaded};
use rimstudio_io::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ctx::{Ctx, SETTINGS_FILE, refuse_read_only};
use crate::error::{ManagerError, ManagerResult};

/// The sections a patch may address, in the order of the file.
pub const SECTIONS: &[&str] = &[
    "language",
    "appearance",
    "library",
    "sorting",
    "history",
    "shortcuts",
    "tools",
    "designer",
    "updates",
    "logging",
    "onboarding",
    "window",
];

/// The text written to a new `settings.jsonc`: comments only, no values.
pub const DEFAULT_FILE_TEXT: &str = r#"{
  // RimStudio settings.
  //
  // Only keys that differ from the defaults are written by the app. Every key below is
  // an example that you can uncomment. Comments, key order and layout are kept when
  // the app saves this file, and unknown keys are preserved.
  //
  // "language": "en",               // UI language code; absent follows the system
  // "appearance": {
  //   "theme": "dark",              // light | dark | system | the name of a file in themes/
  //   "density": "compact",         // standard | compact | touch
  //   "fontScale": 1.1              // 0.8 to 1.6
  // },
  // "library": {
  //   "scanThreads": 0,             // 0 = automatic (at most 8)
  //   "checkWorkshopUpdates": true  // read the Steam manifest on refresh, no network
  // },
  // "sorting": { "dependenciesAsLoadAfter": true },
  // "tools": { "textEditor": { "command": "code" } },
  // "designer": { "mode": "simple" },      // simple | calibrated
  // "logging": { "level": "info" },        // error | warn | info | debug | trace
  "$schema": "./schemas/settings.schema.json",
  "schemaVersion": 1
}
"#;

/// A request for the settings, optionally narrowed to some sections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SettingsGetRequest {
    /// The sections to put in [`SettingsView::effective`]; `None` means all of them.
    pub sections: Option<Vec<String>>,
}

/// One section patch: a section name and a merge patch for its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionPatch {
    /// A name from [`SECTIONS`].
    pub section: String,
    /// Objects merge key by key, `null` resets a key (or the whole section) to its default and any
    /// other value replaces the old one.
    pub value: Value,
}

/// A change of one or more sections, applied together or not at all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SettingsUpdateRequest {
    /// The patches, applied in order.
    pub patches: Vec<SectionPatch>,
}

/// A problem that stopped the file from loading faithfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProblem {
    /// The stable error code, for example `store.parse`.
    pub code: String,
    /// A developer facing message.
    pub message: String,
    /// One based line, when known.
    pub line: Option<usize>,
    /// One based column, when known.
    pub column: Option<usize>,
}

/// How the settings document was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsOrigin {
    /// There was no file; defaults are in effect.
    Missing,
    /// Read as stored.
    File,
    /// Read and migrated in memory from an older schema.
    Migrated,
    /// Written by a newer version; read only.
    NewerThanApp,
    /// The file was unreadable and the newest readable backup is shown; read only.
    Backup,
    /// The file was unreadable and defaults are shown; read only.
    Fallback,
}

/// The settings as the UI shows them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// The effective typed settings: a section whose stored value is invalid is the default.
    pub settings: Settings,
    /// Every section with its full effective value, defaults included (filtered by the request).
    pub effective: Map<String, Value>,
    /// Validation problems of the stored values; those values stay in the file.
    pub problems: Vec<SettingProblem>,
    /// Why the file could not be read faithfully, when it could not.
    pub file_problem: Option<FileProblem>,
    /// How the document was obtained.
    pub origin: SettingsOrigin,
    /// The schema version found in the file when it was migrated, newer or older than this build.
    pub stored_schema_version: Option<u32>,
    /// True when updates are refused.
    pub read_only: bool,
    /// True when this call created the file with its default comments.
    pub created: bool,
    /// The settings file path.
    pub path: Utf8PathBuf,
    /// A short fingerprint of the effective settings; changes whenever a value changes.
    pub rev: String,
}

/// Returns the settings, creating the commented defaults file on first run.
///
/// A failure to create the file (for example a read only config folder) is logged and does not
/// fail the call.
///
/// # Errors
/// [`ManagerError::UnknownSection`] when the request names a section that does not exist, or a
/// store error when the path of the file cannot be formed.
pub fn get(ctx: &Ctx, req: SettingsGetRequest) -> ManagerResult<SettingsView> {
    check_sections(req.sections.as_deref())?;
    let store = ctx.settings_store()?;
    let created = match ensure_file(store.path()) {
        Ok(created) => created,
        Err(e) => {
            tracing::warn!(path = %store.path(), error = %e, "could not create the settings file");
            false
        }
    };
    let loaded = store.load();
    Ok(build_view(
        &loaded,
        store.path(),
        created,
        req.sections.as_deref(),
    ))
}

/// Applies section patches and saves them, keeping the comments and layout of the file.
///
/// Nothing is written when any patch is unknown or the result would be invalid, or when the file
/// must not be overwritten.
///
/// # Errors
/// [`ManagerError::UnknownSection`], [`ManagerError::UnknownKeys`],
/// [`ManagerError::SettingsInvalid`], [`ManagerError::ReadOnly`] or a store error.
pub fn update(ctx: &Ctx, req: SettingsUpdateRequest) -> ManagerResult<SettingsView> {
    for p in &req.patches {
        if !SECTIONS.contains(&p.section.as_str()) {
            return Err(ManagerError::UnknownSection {
                name: p.section.clone(),
            });
        }
    }
    let store = ctx.settings_store()?;
    let created = ensure_file(store.path()).map_err(|e| {
        ManagerError::io(
            "create-settings",
            store.path(),
            &std::io::Error::other(e.to_string()),
        )
    })?;
    let loaded = store.load();
    refuse_read_only(&store, &loaded)?;
    let current =
        serde_json::to_value(&loaded.value).map_err(|e| ManagerError::InvalidRequest {
            field: "settings",
            reason: e.to_string(),
        })?;
    let mut root = match current {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    for p in &req.patches {
        let slot = root.remove(&p.section).unwrap_or(Value::Null);
        let mut target = slot;
        if p.value.is_null() {
            // Resetting a whole section removes it; the default applies again.
            continue;
        }
        merge_patch(&mut target, &p.value);
        root.insert(p.section.clone(), target);
    }
    let next: Settings =
        serde_json::from_value(Value::Object(root)).map_err(|e| ManagerError::SettingsInvalid {
            problems: vec![SettingProblem {
                field: "settings".to_owned(),
                reason: e.to_string(),
            }],
        })?;
    let mut unknown = Vec::new();
    for p in &req.patches {
        if matches!(p.section.as_str(), "shortcuts" | "window" | "language") || p.value.is_null() {
            continue;
        }
        let typed = section_value(&next, &p.section).unwrap_or(Value::Null);
        collect_unknown(&p.value, &typed, &p.section, &mut unknown);
    }
    if !unknown.is_empty() {
        return Err(ManagerError::UnknownKeys { keys: unknown });
    }
    let problems = next.validate();
    if !problems.is_empty() {
        return Err(ManagerError::SettingsInvalid { problems });
    }
    apply_to_file(&store, &req.patches)?;
    let loaded = store.load();
    Ok(build_view(&loaded, store.path(), created, None))
}

/// Writes the patches into the file as minimal edits: each changed leaf is set (or removed when it
/// equals the default or the patch says `null`), so defaults are never written and everything else
/// in the text stays byte identical.
fn apply_to_file(store: &Store<Settings>, patches: &[SectionPatch]) -> ManagerResult<()> {
    let defaults = Settings::default();
    store.edit(|ed| {
        for p in patches {
            if p.value.is_null() {
                ed.remove(&[p.section.as_str()])?;
                continue;
            }
            let default = section_value(&defaults, &p.section).unwrap_or(Value::Null);
            let mut path = vec![p.section.clone()];
            if matches!(p.section.as_str(), "window") {
                ed.set(&[p.section.as_str()], &p.value)?;
            } else {
                write_leaves(ed, &mut path, &p.value, Some(&default))?;
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn as_refs(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

fn write_leaves(
    ed: &JsoncEditor,
    path: &mut Vec<String>,
    value: &Value,
    default: Option<&Value>,
) -> Result<(), StoreError> {
    match value {
        Value::Object(items) if !items.is_empty() => {
            for (key, inner) in items {
                path.push(key.clone());
                write_leaves(ed, path, inner, default.and_then(|d| d.get(key)))?;
                path.pop();
            }
        }
        Value::Null => {
            ed.remove(&as_refs(path))?;
        }
        other => {
            if default.is_some_and(|d| values_equal(d, other)) {
                ed.remove(&as_refs(path))?;
            } else {
                ed.set(&as_refs(path), other)?;
            }
        }
    }
    Ok(())
}

/// Creates the defaults file when it does not exist. Returns whether it was created.
fn ensure_file(path: &camino::Utf8Path) -> Result<bool, rimstudio_io::error::StoreError> {
    if path.exists() {
        return Ok(false);
    }
    atomic_write(path, DEFAULT_FILE_TEXT.as_bytes())?;
    Ok(true)
}

fn check_sections(sections: Option<&[String]>) -> ManagerResult<()> {
    for name in sections.unwrap_or_default() {
        if !SECTIONS.contains(&name.as_str()) {
            return Err(ManagerError::UnknownSection { name: name.clone() });
        }
    }
    Ok(())
}

/// RFC 7386 style merge: objects merge, `null` removes, anything else replaces.
fn merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(items) => {
            if !target.is_object() {
                *target = Value::Object(Map::new());
            }
            if let Value::Object(map) = target {
                for (key, value) in items {
                    if value.is_null() {
                        map.remove(key);
                    } else {
                        merge_patch(map.entry(key.clone()).or_insert(Value::Null), value);
                    }
                }
            }
        }
        other => *target = other.clone(),
    }
}

/// Collects the dotted paths of patch keys that the typed section did not keep.
fn collect_unknown(patch: &Value, typed: &Value, path: &str, out: &mut Vec<String>) {
    let Value::Object(items) = patch else {
        return;
    };
    for (key, value) in items {
        if value.is_null() {
            continue;
        }
        let here = format!("{path}.{key}");
        match typed.get(key) {
            None => out.push(here),
            Some(inner) => collect_unknown(value, inner, &here, out),
        }
    }
}

/// The full value of one section of the settings, defaults included.
fn section_value(s: &Settings, name: &str) -> Option<Value> {
    let v = match name {
        "language" => serde_json::to_value(&s.language),
        "appearance" => serde_json::to_value(&s.appearance),
        "library" => serde_json::to_value(&s.library),
        "sorting" => serde_json::to_value(&s.sorting),
        "history" => serde_json::to_value(&s.history),
        "shortcuts" => serde_json::to_value(&s.shortcuts),
        "tools" => serde_json::to_value(&s.tools),
        "designer" => serde_json::to_value(&s.designer),
        "updates" => serde_json::to_value(&s.updates),
        "logging" => serde_json::to_value(&s.logging),
        "onboarding" => serde_json::to_value(&s.onboarding),
        "window" => serde_json::to_value(&s.window),
        _ => return None,
    };
    v.ok()
}

fn reset_section(s: &mut Settings, name: &str) {
    match name {
        "language" => s.language = None,
        "appearance" => s.appearance = Default::default(),
        "library" => s.library = Default::default(),
        "sorting" => s.sorting = Default::default(),
        "history" => s.history = Default::default(),
        "shortcuts" => s.shortcuts.clear(),
        "tools" => s.tools = Default::default(),
        "designer" => s.designer = Default::default(),
        "updates" => s.updates = Default::default(),
        "logging" => s.logging = Default::default(),
        "onboarding" => s.onboarding = Default::default(),
        "window" => s.window = None,
        _ => {}
    }
}

fn build_view(
    loaded: &Loaded<Settings>,
    path: &camino::Utf8Path,
    created: bool,
    filter: Option<&[String]>,
) -> SettingsView {
    let problems = loaded.value.validate();
    let mut settings = loaded.value.clone();
    for p in &problems {
        let name = p
            .field
            .split(['.', '['])
            .next()
            .unwrap_or_default()
            .to_owned();
        reset_section(&mut settings, &name);
    }
    let mut effective = Map::new();
    for name in SECTIONS {
        if filter.is_some_and(|f| !f.iter().any(|x| x == name)) {
            continue;
        }
        if let Some(v) = section_value(&settings, name) {
            effective.insert((*name).to_owned(), v);
        }
    }
    let (origin, stored) = match &loaded.origin {
        LoadOrigin::Missing => (SettingsOrigin::Missing, None),
        LoadOrigin::File => (SettingsOrigin::File, None),
        LoadOrigin::Migrated { from } => (SettingsOrigin::Migrated, Some(*from)),
        LoadOrigin::NewerThanApp { found } => (SettingsOrigin::NewerThanApp, Some(*found)),
        LoadOrigin::Backup { .. } => (SettingsOrigin::Backup, None),
        LoadOrigin::Fallback => (SettingsOrigin::Fallback, None),
    };
    let rev = fingerprint(&settings);
    SettingsView {
        settings,
        effective,
        problems,
        file_problem: loaded.problem.as_ref().map(|p| FileProblem {
            code: p.code.to_owned(),
            message: p.message.clone(),
            line: p.line,
            column: p.column,
        }),
        origin,
        stored_schema_version: stored,
        read_only: loaded.read_only,
        created,
        path: path.to_owned(),
        rev,
    }
}

/// FNV-1a over the canonical JSON of the settings, as 16 hex digits.
fn fingerprint(s: &Settings) -> String {
    let text = serde_json::to_string(s).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The name of the settings file, for callers that show it.
pub fn file_name() -> &'static str {
    SETTINGS_FILE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_patch_merges_objects_and_removes_nulls() {
        let mut t = json!({"a": {"b": 1, "c": 2}, "d": 3});
        merge_patch(&mut t, &json!({"a": {"b": null, "e": 5}, "d": 4}));
        assert_eq!(t, json!({"a": {"c": 2, "e": 5}, "d": 4}));
    }

    #[test]
    fn merge_patch_replaces_scalars_with_objects() {
        let mut t = json!({"a": 1});
        merge_patch(&mut t, &json!({"a": {"x": 1}}));
        assert_eq!(t, json!({"a": {"x": 1}}));
    }

    #[test]
    fn default_file_text_is_valid_jsonc_with_the_current_version() {
        let v = rimstudio_io::jsonc::parse_value(DEFAULT_FILE_TEXT, camino::Utf8Path::new("x"))
            .unwrap();
        assert_eq!(v["schemaVersion"], 1);
        let s: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(s.validate(), Vec::new());
    }

    #[test]
    fn fingerprint_changes_with_a_value() {
        let a = Settings::default();
        let mut b = Settings::default();
        b.appearance.font_scale = 1.2;
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert_eq!(fingerprint(&a), fingerprint(&Settings::default()));
    }

    #[test]
    fn every_section_has_a_value_and_a_reset() {
        let mut s = Settings::default();
        for name in SECTIONS {
            assert!(section_value(&s, name).is_some(), "{name}");
            reset_section(&mut s, name);
        }
        assert!(section_value(&s, "nope").is_none());
    }

    #[test]
    fn collect_unknown_finds_misspelled_keys() {
        let mut out = Vec::new();
        collect_unknown(
            &json!({"theme": "dark", "thme": 1, "n": {"x": 1}}),
            &json!({"theme": "dark", "n": {}}),
            "appearance",
            &mut out,
        );
        assert_eq!(
            out,
            vec!["appearance.thme".to_owned(), "appearance.n.x".to_owned()]
        );
    }
}
