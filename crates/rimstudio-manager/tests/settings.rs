//! Settings use cases: first run, comment preserving updates, validation and read only files.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Harness;
use rimstudio_manager::error::ManagerError;
use rimstudio_manager::settings::{
    SectionPatch, SettingsGetRequest, SettingsOrigin, SettingsUpdateRequest, get, update,
};
use serde_json::json;

fn patch(section: &str, value: serde_json::Value) -> SettingsUpdateRequest {
    SettingsUpdateRequest {
        patches: vec![SectionPatch {
            section: section.to_owned(),
            value,
        }],
    }
}

/// The part of `before` and of `after` that differs once the common prefix and suffix are removed.
fn edited_span<'a>(before: &'a str, after: &'a str) -> (&'a str, &'a str) {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let (b, a) = (&before[prefix..], &after[prefix..]);
    let suffix = b
        .bytes()
        .rev()
        .zip(a.bytes().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (&b[..b.len() - suffix], &a[..a.len() - suffix])
}

#[test]
fn first_run_creates_the_commented_defaults_file() {
    let h = Harness::bare();
    let view = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    assert!(view.created);
    assert_eq!(view.origin, SettingsOrigin::File);
    assert!(view.problems.is_empty());
    assert!(!view.read_only);
    let text = h.read(&h.settings_path());
    assert!(text.contains("// RimStudio settings."));
    assert!(text.contains("\"schemaVersion\": 1"));
    assert_eq!(view.effective["appearance"]["density"], "standard");
    assert_eq!(view.effective["library"]["scanThreads"], 0);
}

#[test]
fn a_second_get_leaves_the_file_alone() {
    let h = Harness::bare();
    get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    let before = h.read(&h.settings_path());
    let view = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    assert!(!view.created);
    assert_eq!(h.read(&h.settings_path()), before);
}

#[test]
fn get_can_be_narrowed_to_sections_and_rejects_unknown_ones() {
    let h = Harness::bare();
    let view = get(
        &h.ctx(),
        SettingsGetRequest {
            sections: Some(vec!["logging".to_owned()]),
        },
    )
    .unwrap();
    assert_eq!(view.effective.len(), 1);
    assert_eq!(view.effective["logging"]["level"], "info");
    let err = get(
        &h.ctx(),
        SettingsGetRequest {
            sections: Some(vec!["nope".to_owned()]),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.unknown-section");
}

#[test]
fn an_update_changes_only_the_edited_span_of_the_file() {
    let h = Harness::bare();
    get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    let original = h.read(&h.settings_path());

    let view = update(&h.ctx(), patch("appearance", json!({"theme": "dark"}))).unwrap();
    assert_eq!(view.settings.appearance.theme, "dark");
    let after_first = h.read(&h.settings_path());
    let (removed, added) = edited_span(&original, &after_first);
    assert!(
        removed.trim().is_empty(),
        "nothing of the original text may be removed, got {removed:?}"
    );
    assert!(
        added.contains("appearance") && added.contains("\"dark\"") && !added.contains("density"),
        "added {added:?}"
    );
    assert!(after_first.contains("// RimStudio settings."));
    assert!(after_first.contains("// \"language\": \"en\","));

    update(&h.ctx(), patch("appearance", json!({"theme": "light"}))).unwrap();
    let after_second = h.read(&h.settings_path());
    let (removed, added) = edited_span(&after_first, &after_second);
    assert_eq!(removed, "dark");
    assert_eq!(added, "light");
}

#[test]
fn comments_and_unknown_keys_written_by_hand_survive_an_update() {
    let h = Harness::bare();
    std::fs::create_dir_all(&h.roots.config).unwrap();
    let text = "{\n  // my note\n  \"schemaVersion\": 1,\n  \"future\": {\"x\": [1, 2]}, // keep me\n  \"logging\": { \"level\": \"warn\" }\n}\n";
    std::fs::write(h.settings_path(), text).unwrap();
    update(&h.ctx(), patch("designer", json!({"mode": "calibrated"}))).unwrap();
    let after = h.read(&h.settings_path());
    assert!(after.contains("// my note"));
    assert!(after.contains("\"future\": {\"x\": [1, 2]}, // keep me"));
    assert!(after.contains("\"level\": \"warn\""));
    assert!(after.contains("calibrated"));
}

#[test]
fn a_null_patch_resets_a_key_and_a_section() {
    let h = Harness::bare();
    update(
        &h.ctx(),
        patch("appearance", json!({"theme": "dark", "density": "compact"})),
    )
    .unwrap();
    let view = update(&h.ctx(), patch("appearance", json!({"theme": null}))).unwrap();
    assert_eq!(view.settings.appearance.theme, "system");
    assert_eq!(view.effective["appearance"]["density"], "compact");
    let view = update(&h.ctx(), patch("appearance", json!(null))).unwrap();
    assert_eq!(view.effective["appearance"]["density"], "standard");
    let text = h.read(&h.settings_path());
    assert!(!text.contains("\"density\": \"compact\"\n"), "{text}");
}

#[test]
fn several_patches_apply_together_and_change_the_rev() {
    let h = Harness::bare();
    let before = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    let view = update(
        &h.ctx(),
        SettingsUpdateRequest {
            patches: vec![
                SectionPatch {
                    section: "language".into(),
                    value: json!("de"),
                },
                SectionPatch {
                    section: "history".into(),
                    value: json!({"keep": 10}),
                },
                SectionPatch {
                    section: "shortcuts".into(),
                    value: json!({"list.save": "Mod+S"}),
                },
            ],
        },
    )
    .unwrap();
    assert_eq!(view.settings.language.as_deref(), Some("de"));
    assert_eq!(view.settings.history.keep, Some(10));
    assert_eq!(
        view.settings.shortcuts.get("list.save").map(String::as_str),
        Some("Mod+S")
    );
    assert_ne!(view.rev, before.rev);
}

#[test]
fn invalid_updates_are_refused_and_write_nothing() {
    let h = Harness::bare();
    get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    let before = h.read(&h.settings_path());

    let err = update(&h.ctx(), patch("appearance", json!({"fontScale": 5.0}))).unwrap_err();
    match &err {
        ManagerError::SettingsInvalid { problems } => {
            assert_eq!(problems[0].field, "appearance.fontScale");
        }
        other => panic!("unexpected {other:?}"),
    }
    let err = update(&h.ctx(), patch("appearance", json!({"theem": "x"}))).unwrap_err();
    assert_eq!(err.code(), "manager.unknown-keys");
    let err = update(&h.ctx(), patch("appearance", json!({"density": "huge"}))).unwrap_err();
    assert_eq!(err.code(), "manager.settings-invalid");
    let err = update(&h.ctx(), patch("bogus", json!({}))).unwrap_err();
    assert_eq!(err.code(), "manager.unknown-section");
    assert_eq!(h.read(&h.settings_path()), before);
}

#[test]
fn an_invalid_stored_value_is_reported_kept_and_replaced_by_the_default_for_the_run() {
    let h = Harness::bare();
    std::fs::create_dir_all(&h.roots.config).unwrap();
    let text = "{\n  \"schemaVersion\": 1,\n  \"appearance\": { \"fontScale\": 9, \"theme\": \"dark\" }\n}\n";
    std::fs::write(h.settings_path(), text).unwrap();
    let view = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    assert_eq!(view.problems.len(), 1);
    assert_eq!(view.problems[0].field, "appearance.fontScale");
    assert!((view.settings.appearance.font_scale - 1.0).abs() < f64::EPSILON);
    assert_eq!(view.settings.appearance.theme, "system");
    assert_eq!(h.read(&h.settings_path()), text);
}

#[test]
fn a_file_with_a_syntax_error_is_read_only_and_never_overwritten() {
    let h = Harness::bare();
    std::fs::create_dir_all(&h.roots.config).unwrap();
    let text = "{ \"schemaVersion\": 1, \"logging\": { \"level\": }\n";
    std::fs::write(h.settings_path(), text).unwrap();
    let view = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    assert!(view.read_only);
    assert_eq!(view.file_problem.as_ref().unwrap().code, "store.parse");
    assert!(view.file_problem.as_ref().unwrap().line.is_some());
    let err = update(&h.ctx(), patch("logging", json!({"level": "debug"}))).unwrap_err();
    assert_eq!(err.code(), "manager.read-only");
    assert_eq!(h.read(&h.settings_path()), text);
}

#[test]
fn a_file_from_a_newer_version_is_read_only() {
    let h = Harness::bare();
    std::fs::create_dir_all(&h.roots.config).unwrap();
    let text = "{ \"schemaVersion\": 7, \"logging\": { \"level\": \"debug\" } }\n";
    std::fs::write(h.settings_path(), text).unwrap();
    let view = get(&h.ctx(), SettingsGetRequest::default()).unwrap();
    assert_eq!(view.origin, SettingsOrigin::NewerThanApp);
    assert_eq!(view.stored_schema_version, Some(7));
    assert!(view.read_only);
    let err = update(&h.ctx(), patch("logging", json!({"level": "info"}))).unwrap_err();
    assert_eq!(err.code(), "manager.read-only");
    assert_eq!(h.read(&h.settings_path()), text);
}

#[test]
fn an_update_of_a_missing_file_creates_it_with_the_comments() {
    let h = Harness::bare();
    update(&h.ctx(), patch("logging", json!({"level": "debug"}))).unwrap();
    let text = h.read(&h.settings_path());
    assert!(text.contains("// RimStudio settings."));
    assert!(text.contains("\"debug\""));
}
