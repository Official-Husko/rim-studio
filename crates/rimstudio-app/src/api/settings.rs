//! `settings_get` and `settings_update`.
//!
//! The manager keeps the settings file (`settings.jsonc`) and the workspace document
//! (`workspace.jsonc`, path overrides and custom folders). The DTO is one flat view of both, so an
//! update is split: section patches go to the settings file in one atomic edit, path overrides go to
//! the detection override store. The revision of the DTO is the settings fingerprint of the manager
//! cut to 52 bits; `expectedRev` 0 means "do not check".

use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_ipc_types::settings::{
    PathOverrideDto, SettingsDto, SettingsGetRequest, SettingsUpdate,
};
use rimstudio_manager::detect::{self, DetectSetOverrideRequest, OverrideField};
use rimstudio_manager::settings::{
    self, SECTIONS, SectionPatch, SettingsUpdateRequest, SettingsView,
};
use serde_json::{Map, Value, json};

use crate::context::{AppContext, AppEvent};
use crate::convert;
use crate::error::AppError;

/// One change to a path override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathChange {
    /// Set an override of one of the single path fields.
    Set(OverrideField, PathOverrideDto),
    /// Clear an override.
    Clear(OverrideField),
    /// Replace the list of extra Workshop folders.
    ReplaceWorkshop(Vec<String>),
}

/// `settings_get`: the settings and the path overrides as one DTO. Creates the commented defaults file
/// on first run. The `sections` filter of the request is not applied: the DTO is always whole.
///
/// # Errors
/// The mapped manager error.
pub fn settings_get(ctx: &AppContext, _req: SettingsGetRequest) -> Result<SettingsDto, ApiError> {
    let view = ctx
        .with_manager(|m| settings::get(m, settings::SettingsGetRequest::default()))
        .map_err(|e| ctx.manager_error(&e))?;
    ctx.settings.set(view.settings.clone());
    Ok(convert::settings_dto(&view, &ctx.workspace_settings()))
}

/// Wraps `leaf` in the objects named by `path`.
fn nest(path: &[&str], leaf: Value) -> Value {
    path.iter().rev().fold(leaf, |inner, key| {
        let mut map = Map::new();
        map.insert((*key).to_owned(), inner);
        Value::Object(map)
    })
}

fn object_or_none(map: Map<String, Value>) -> Option<Value> {
    (!map.is_empty()).then_some(Value::Object(map))
}

fn encode<T: serde::Serialize>(value: &T, what: &str) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|e| {
        AppError::internal(format!("the {what} patch cannot be encoded: {e}")).to_api()
    })
}

/// Splits an update into the section patches of the settings file and the path changes.
///
/// # Errors
/// `ipc.invalid-request` for a reset key that names nothing.
pub fn split_update(
    req: &SettingsUpdate,
) -> Result<(Vec<SectionPatch>, Vec<PathChange>), ApiError> {
    let mut patches = Vec::new();
    let mut paths = Vec::new();
    let mut push = |section: &str, value: Value| {
        patches.push(SectionPatch {
            section: section.to_owned(),
            value,
        });
    };
    if let Some(language) = &req.language {
        push(
            "language",
            if language.trim().is_empty() {
                Value::Null
            } else {
                json!(language)
            },
        );
    }
    if let Some(a) = &req.appearance
        && let Some(v) = encode(a, "appearance")?
            .as_object()
            .cloned()
            .and_then(object_or_none)
    {
        push("appearance", v);
    }
    if let Some(l) = &req.library {
        let mut library = Map::new();
        if let Some(n) = l.scan_threads {
            library.insert("scanThreads".to_owned(), json!(n));
        }
        if let Some(b) = l.check_workshop_updates {
            library.insert("checkWorkshopUpdates".to_owned(), json!(b));
        }
        let mut watch = Map::new();
        if let Some(mode) = l.watch_mode {
            watch.insert("mode".to_owned(), encode(&mode, "watch mode")?);
        }
        if let Some(n) = l.poll_interval_seconds {
            watch.insert("pollIntervalSeconds".to_owned(), json!(n));
        }
        if let Some(n) = l.debounce_ms {
            watch.insert("debounceMs".to_owned(), json!(n));
        }
        if let Some(w) = object_or_none(watch) {
            library.insert("watch".to_owned(), w);
        }
        if let Some(v) = object_or_none(library) {
            push("library", v);
        }
    }
    if let Some(s) = &req.sorting
        && let Some(v) = encode(s, "sorting")?
            .as_object()
            .cloned()
            .and_then(object_or_none)
    {
        push("sorting", v);
    }
    if let Some(mode) = req.designer_mode {
        push("designer", nest(&["mode"], encode(&mode, "designer mode")?));
    }
    if let Some(level) = req.log_level {
        push("logging", nest(&["level"], encode(&level, "log level")?));
    }
    if let Some(done) = req.onboarding_completed {
        push("onboarding", nest(&["completed"], json!(done)));
    }
    if let Some(p) = &req.paths {
        for (field, value) in [
            (OverrideField::GameInstall, &p.game_install),
            (OverrideField::UserDir, &p.user_dir),
            (OverrideField::SteamRoot, &p.steam_root),
        ] {
            if let Some(v) = value {
                paths.push(PathChange::Set(field, v.clone()));
            }
        }
        if let Some(list) = &p.extra_workshop_dirs {
            paths.push(PathChange::ReplaceWorkshop(list.clone()));
        }
    }
    for key in &req.reset {
        match key.as_str() {
            "paths.gameInstall" => paths.push(PathChange::Clear(OverrideField::GameInstall)),
            "paths.userDir" => paths.push(PathChange::Clear(OverrideField::UserDir)),
            "paths.steamRoot" => paths.push(PathChange::Clear(OverrideField::SteamRoot)),
            "paths.extraWorkshopDirs" => paths.push(PathChange::ReplaceWorkshop(Vec::new())),
            "designerMode" => patches.push(null_patch("designer", &["mode"])),
            "logLevel" => patches.push(null_patch("logging", &["level"])),
            "onboardingCompleted" => patches.push(null_patch("onboarding", &["completed"])),
            other => {
                let segments: Vec<&str> = other.split('.').collect();
                let section = segments.first().copied().unwrap_or("");
                if !SECTIONS.contains(&section) {
                    return Err(AppError::invalid(
                        "settings_update",
                        "reset",
                        format!("{other:?} is not a settings key"),
                    )
                    .to_api());
                }
                patches.push(null_patch(section, segments.get(1..).unwrap_or(&[])));
            }
        }
    }
    Ok((patches, paths))
}

fn null_patch(section: &str, rest: &[&str]) -> SectionPatch {
    SectionPatch {
        section: section.to_owned(),
        value: if rest.is_empty() {
            Value::Null
        } else {
            nest(rest, Value::Null)
        },
    }
}

fn changed_sections(patches: &[SectionPatch], paths: &[PathChange]) -> Vec<String> {
    let mut out: Vec<String> = patches.iter().map(|p| p.section.clone()).collect();
    if !paths.is_empty() {
        out.push("paths".to_owned());
    }
    out.sort();
    out.dedup();
    out
}

fn apply_path_change(ctx: &AppContext, change: &PathChange) -> Result<(), ApiError> {
    let call = |field: OverrideField, path: Option<String>, pinned: bool| {
        ctx.with_manager(|m| {
            detect::set_override(
                m,
                DetectSetOverrideRequest {
                    field,
                    path,
                    pinned,
                },
            )
        })
        .map(|_| ())
        .map_err(|e| ctx.manager_error(&e))
    };
    match change {
        PathChange::Set(field, value) => call(*field, Some(value.path.clone()), value.pinned),
        PathChange::Clear(field) => call(*field, None, false),
        PathChange::ReplaceWorkshop(list) => {
            call(OverrideField::WorkshopDir, None, false)?;
            for dir in list {
                call(OverrideField::WorkshopDir, Some(dir.clone()), false)?;
            }
            Ok(())
        }
    }
}

fn check_rev(expected: u64, view: &SettingsView) -> Result<(), ApiError> {
    let current = convert::rev_of_fingerprint(&view.rev);
    if expected != 0 && expected != current {
        return Err(ApiError::new(
            codes::SETTINGS_REVISION_CONFLICT,
            "the settings changed since this revision",
        )
        .detail("expectedRev", expected)
        .detail("currentRev", current));
    }
    Ok(())
}

/// `settings_update`: applies section patches to the settings file and path overrides to the
/// detection overrides, then returns the new settings.
///
/// The section patches are validated and written together or not at all. A path override that fails
/// after them leaves the already written settings in place and reports its own error.
///
/// # Errors
/// `settings.revision-conflict` for a stale `expectedRev`, the mapped manager errors otherwise.
pub fn settings_update(ctx: &AppContext, req: SettingsUpdate) -> Result<SettingsDto, ApiError> {
    let (patches, paths) = split_update(&req)?;
    let before = ctx
        .with_manager(|m| settings::get(m, settings::SettingsGetRequest::default()))
        .map_err(|e| ctx.manager_error(&e))?;
    check_rev(req.expected_rev, &before)?;
    let sections = changed_sections(&patches, &paths);
    let mut view = before;
    if !patches.is_empty() {
        view = ctx
            .with_manager(|m| settings::update(m, SettingsUpdateRequest { patches }))
            .map_err(|e| ctx.manager_error(&e))?;
        ctx.settings.set(view.settings.clone());
    }
    for change in &paths {
        apply_path_change(ctx, change)?;
    }
    if !paths.is_empty() {
        ctx.workspace.bump();
    }
    if !sections.is_empty() {
        ctx.events.emit(&AppEvent::SettingsChanged {
            rev: convert::rev_of_fingerprint(&view.rev),
            sections,
        });
    }
    Ok(convert::settings_dto(&view, &ctx.workspace_settings()))
}

#[cfg(test)]
mod tests {
    use rimstudio_ipc_types::settings::{
        AppearancePatch, DensityDto, LibraryPatch, LogLevelDto, PathsPatch, WatchModeDto,
    };

    use super::*;

    #[test]
    fn an_empty_update_has_no_patches() {
        let (patches, paths) = split_update(&SettingsUpdate::default()).expect("splits");
        assert!(patches.is_empty() && paths.is_empty());
    }

    #[test]
    fn typed_patches_become_section_patches_with_wire_names() {
        let req = SettingsUpdate {
            appearance: Some(AppearancePatch {
                density: Some(DensityDto::Compact),
                font_scale: Some(1.25),
                ..AppearancePatch::default()
            }),
            library: Some(LibraryPatch {
                watch_mode: Some(WatchModeDto::Poll),
                scan_threads: Some(4),
                ..LibraryPatch::default()
            }),
            log_level: Some(LogLevelDto::Debug),
            ..SettingsUpdate::default()
        };
        let (patches, _) = split_update(&req).expect("splits");
        let by_section = |name: &str| {
            patches
                .iter()
                .find(|p| p.section == name)
                .map(|p| p.value.clone())
        };
        assert_eq!(
            by_section("appearance"),
            Some(json!({"density": "compact", "fontScale": 1.25}))
        );
        assert_eq!(
            by_section("library"),
            Some(json!({"scanThreads": 4, "watch": {"mode": "poll"}}))
        );
        assert_eq!(by_section("logging"), Some(json!({"level": "debug"})));
    }

    #[test]
    fn reset_keys_become_null_leaves_or_path_clears() {
        let req = SettingsUpdate {
            reset: vec![
                "appearance.theme".to_owned(),
                "paths.gameInstall".to_owned(),
                "designer".to_owned(),
            ],
            ..SettingsUpdate::default()
        };
        let (patches, paths) = split_update(&req).expect("splits");
        assert_eq!(patches[0].value, json!({"theme": null}));
        assert_eq!(patches[1].section, "designer");
        assert!(patches[1].value.is_null());
        assert_eq!(paths, vec![PathChange::Clear(OverrideField::GameInstall)]);
    }

    #[test]
    fn a_reset_key_that_names_nothing_is_a_request_error() {
        let req = SettingsUpdate {
            reset: vec!["nonsense.key".to_owned()],
            ..SettingsUpdate::default()
        };
        let err = split_update(&req).err().map(|e| e.code);
        assert_eq!(err, Some("ipc.invalid-request".to_owned()));
    }

    #[test]
    fn path_patches_split_into_overrides() {
        let req = SettingsUpdate {
            paths: Some(PathsPatch {
                game_install: Some(PathOverrideDto {
                    path: "/fiction/RimWorld".into(),
                    pinned: true,
                }),
                extra_workshop_dirs: Some(vec!["/fiction/w".into()]),
                ..PathsPatch::default()
            }),
            ..SettingsUpdate::default()
        };
        let (patches, paths) = split_update(&req).expect("splits");
        assert!(patches.is_empty());
        assert_eq!(paths.len(), 2);
        assert!(matches!(&paths[1], PathChange::ReplaceWorkshop(l) if l.len() == 1));
    }

    #[test]
    fn language_is_set_or_reset() {
        let set = SettingsUpdate {
            language: Some("de".into()),
            ..SettingsUpdate::default()
        };
        assert_eq!(split_update(&set).expect("splits").0[0].value, json!("de"));
        let clear = SettingsUpdate {
            language: Some(String::new()),
            ..SettingsUpdate::default()
        };
        assert!(split_update(&clear).expect("splits").0[0].value.is_null());
    }

    #[test]
    fn the_changed_sections_are_sorted_and_include_paths() {
        let patches = vec![
            SectionPatch {
                section: "logging".into(),
                value: Value::Null,
            },
            SectionPatch {
                section: "appearance".into(),
                value: Value::Null,
            },
        ];
        let paths = vec![PathChange::Clear(OverrideField::UserDir)];
        assert_eq!(
            changed_sections(&patches, &paths),
            vec!["appearance", "logging", "paths"]
        );
    }
}
