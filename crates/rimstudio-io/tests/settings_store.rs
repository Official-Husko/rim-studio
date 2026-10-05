//! The single file store against the real settings types of rimstudio-core.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use camino::Utf8PathBuf;
use rimstudio_core::ports::Clock;
use rimstudio_core::settings::{Settings, WorkspaceSettings};
use rimstudio_io::backup::list_backups;
use rimstudio_io::schema::LoadOrigin;
use rimstudio_io::store::Store;
use serde_json::json;

struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.fetch_add(1000, Ordering::SeqCst)
    }
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(TestClock(AtomicU64::new(1_894_708_800_000)))
}

fn path(name: &str) -> (tempfile::TempDir, Utf8PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(t.path().join(name)).unwrap();
    (t, p)
}

const HAND_WRITTEN: &str = r#"// My RimStudio settings, hand edited.
{
  "$schema": "./schemas/settings.schema.json",
  "schemaVersion": 1,

  // Colours I like.
  "appearance": {
    "theme": "dark", // always dark
    "accent": "amber"
  },

  /* explicit defaults I wrote on purpose */
  "history": {},

  "somethingFromANewerBuild": { "keep": [1, 2, 3] }, // unknown, must survive
}
"#;

#[test]
fn an_unchanged_save_leaves_a_hand_written_file_byte_identical() {
    let (t, p) = path("settings.jsonc");
    fs_err::write(&p, HAND_WRITTEN).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let l = s.load();
    assert_eq!(l.origin, LoadOrigin::File);
    assert_eq!(l.value.appearance.theme, "dark");
    s.save(&l.value).unwrap();
    assert_eq!(fs_err::read_to_string(&p).unwrap(), HAND_WRITTEN);
    assert!(
        !t.path().join("backups").exists(),
        "no backup for a no-op save"
    );
}

#[test]
fn changing_one_setting_through_the_value_keeps_every_comment() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(&p, HAND_WRITTEN).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let mut l = s.load();
    l.value.appearance.accent = "teal".to_owned();
    s.save(&l.value).unwrap();
    assert_eq!(
        fs_err::read_to_string(&p).unwrap(),
        HAND_WRITTEN.replace("\"accent\": \"amber\"", "\"accent\": \"teal\"")
    );
    assert_eq!(list_backups(&p, s.policy()).len(), 1);
    assert_eq!(s.load().value.appearance.accent, "teal");
}

#[test]
fn changing_one_setting_through_edit_is_byte_identical_elsewhere() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(&p, HAND_WRITTEN).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    s.load();
    s.edit(|ed| ed.set(&["appearance", "theme"], &json!("light")))
        .unwrap();
    assert_eq!(
        fs_err::read_to_string(&p).unwrap(),
        HAND_WRITTEN.replace("\"theme\": \"dark\"", "\"theme\": \"light\"")
    );
}

#[test]
fn edit_that_breaks_the_type_is_refused_and_the_file_is_untouched() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(&p, HAND_WRITTEN).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    s.load();
    let err = s
        .edit(|ed| ed.set(&["appearance", "density"], &json!("enormous")))
        .unwrap_err();
    assert_eq!(err.code(), "store.shape");
    assert_eq!(fs_err::read_to_string(&p).unwrap(), HAND_WRITTEN);
}

#[test]
fn unknown_keys_survive_a_change_made_elsewhere() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(&p, HAND_WRITTEN).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let mut l = s.load();
    assert!(l.value.extra.contains_key("somethingFromANewerBuild"));
    l.value.language = Some("de".to_owned());
    s.save(&l.value).unwrap();
    let text = fs_err::read_to_string(&p).unwrap();
    assert!(text.contains("somethingFromANewerBuild"));
    assert!(text.contains("// unknown, must survive"));
    assert!(text.contains("\"language\": \"de\""));
}

#[test]
fn a_section_returning_to_its_defaults_is_dropped_from_the_file() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(
        &p,
        "{\n  \"schemaVersion\": 1,\n  \"appearance\": { \"theme\": \"dark\" }\n}\n",
    )
    .unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let mut l = s.load();
    l.value.appearance = Default::default();
    s.save(&l.value).unwrap();
    let text = fs_err::read_to_string(&p).unwrap();
    assert!(!text.contains("appearance"), "{text}");
}

#[test]
fn a_new_settings_file_gets_a_header_and_only_non_default_sections() {
    let (_t, p) = path("settings.jsonc");
    let s: Store<Settings> = Store::at(&p, clock());
    let mut v = s.load().value;
    v.language = Some("fr".into());
    s.save(&v).unwrap();
    let text = fs_err::read_to_string(&p).unwrap();
    assert!(text.starts_with("// "));
    assert!(text.contains("\"schemaVersion\": 1"));
    assert!(!text.contains("appearance"));
    assert_eq!(s.load().value.language.as_deref(), Some("fr"));
}

#[test]
fn a_settings_file_from_a_newer_app_is_read_only() {
    let (_t, p) = path("settings.jsonc");
    let text = "{ \"schemaVersion\": 99, \"appearance\": { \"theme\": \"dark\" } }";
    fs_err::write(&p, text).unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let l = s.load();
    assert!(l.read_only);
    assert_eq!(l.origin, LoadOrigin::NewerThanApp { found: 99 });
    assert!(s.save(&l.value).is_err());
    assert_eq!(fs_err::read_to_string(&p).unwrap(), text);
}

#[test]
fn a_syntax_error_in_settings_is_reported_with_line_and_column() {
    let (_t, p) = path("settings.jsonc");
    fs_err::write(
        &p,
        "{\n  \"schemaVersion\": 1,\n  \"appearance\": { \"theme\": \"dark\" \n}\n",
    )
    .unwrap();
    let s: Store<Settings> = Store::at(&p, clock());
    let l = s.load();
    let problem = l.problem.unwrap();
    assert_eq!(problem.code, "store.parse");
    assert!(problem.line.is_some() && problem.column.is_some());
    assert!(l.read_only);
}

#[test]
fn workspace_settings_with_custom_folders_round_trip_with_comments() {
    let (_t, p) = path("workspace.jsonc");
    let text = r#"// workspace
{
  "schemaVersion": 1,
  "customModFolders": [
    // my own mods
    { "id": "cf_0a1b2c3d", "path": "/mods/RS_Folder", "label": "RS_Folder" }
  ]
}
"#;
    fs_err::write(&p, text).unwrap();
    let s: Store<WorkspaceSettings> = Store::at(&p, clock());
    let mut l = s.load();
    assert_eq!(l.origin, LoadOrigin::File, "{:?}", l.problem);
    assert_eq!(l.value.custom_mod_folders.len(), 1);
    l.value.custom_mod_folders[0].label = "RS_Renamed".to_owned();
    s.save(&l.value).unwrap();
    let out = fs_err::read_to_string(&p).unwrap();
    assert!(out.contains("// my own mods"));
    assert!(out.contains("RS_Renamed"));
    assert_eq!(s.load().value.custom_mod_folders[0].label, "RS_Renamed");
}

#[test]
fn edit_by_id_updates_one_folder_and_leaves_the_others() {
    let (_t, p) = path("workspace.jsonc");
    let text = r#"{
  "schemaVersion": 1,
  "customModFolders": [
    { "id": "cf_0a1b2c3d", "path": "/mods/A", "label": "A" }, // first
    { "id": "cf_0a1b2c3e", "path": "/mods/B", "label": "B" }
  ]
}
"#;
    fs_err::write(&p, text).unwrap();
    let s: Store<WorkspaceSettings> = Store::at(&p, clock());
    s.load();
    s.edit(|ed| {
        ed.upsert_by_id(
            &["customModFolders"],
            "id",
            "cf_0a1b2c3e",
            &json!({"id": "cf_0a1b2c3e", "path": "/mods/B", "label": "B2"}),
        )
    })
    .unwrap();
    let out = fs_err::read_to_string(&p).unwrap();
    assert!(out.contains("// first"));
    assert!(out.contains("\"label\": \"B2\""));
    assert!(out.contains("\"label\": \"A\""));
}
