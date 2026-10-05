//! Golden JSON of the mod basics DTOs (`project_about_*`, `project_load_folders_*`, `project_version_add`,
//! `library_mod_search`). The files under `tests/golden` are the contract the webview types are generated
//! against; run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite them, then review the diff.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rimstudio_ipc_types::diagnostic::{DiagnosticDto, SeverityDto};
use rimstudio_ipc_types::library::SourceKindDto;
use rimstudio_ipc_types::project_about::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn check<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(name: &str, value: &T) {
    let mut text = serde_json::to_string_pretty(value).unwrap();
    text.push('\n');
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.json"));
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &text).unwrap();
    } else {
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing golden {} ({e}); run with RIMSTUDIO_UPDATE_GOLDEN=1",
                path.display()
            )
        });
        assert_eq!(
            text.replace("\r\n", "\n"),
            expected.replace("\r\n", "\n"),
            "golden mismatch for {name}"
        );
    }
    let back: T = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, value, "{name} does not survive a round trip");
}

fn text(value: &str, present: bool) -> AboutTextDto {
    AboutTextDto {
        value: value.to_owned(),
        present,
    }
}

fn list(items: &[&str], present: bool) -> AboutListDto {
    AboutListDto {
        items: items.iter().map(|s| (*s).to_owned()).collect(),
        present,
    }
}

fn finding(code: &str, severity: SeverityDto, field: &str, message: &str) -> DiagnosticDto {
    DiagnosticDto {
        code: code.to_owned(),
        severity,
        message: message.to_owned(),
        field: Some(field.to_owned()),
        mod_idx: None,
        file_id: None,
        span: None,
        args: Default::default(),
    }
}

fn about() -> ProjectAboutDto {
    ProjectAboutDto {
        project_id: "p-0a1b2c3d".into(),
        path: "About/About.xml".into(),
        file_hash: "0".repeat(64),
        bytes: 612,
        parsed: true,
        editable: true,
        not_editable_reason: None,
        name: text("RS Arms", true),
        short_name: text("", false),
        author: text("RS Author", true),
        authors: list(&[], false),
        package_id: text("rs.arms", true),
        description: text("Adds fictional arms.", true),
        url: text("", false),
        mod_version: text("0.1", true),
        mod_icon_path: text("", false),
        supported_versions: list(&["1.5", "1.6"], true),
        mod_dependencies: vec![AboutDependencyDto {
            package_id: "rs.base".into(),
            display_name: "RS Base".into(),
            steam_workshop_url: Some(
                "https://steamcommunity.com/sharedfiles/filedetails/?id=1".into(),
            ),
            download_url: None,
            alternatives: vec!["rs.base2".into()],
        }],
        mod_dependencies_present: true,
        load_before: list(&[], false),
        load_after: list(&["ludeon.rimworld"], true),
        force_load_before: list(&[], false),
        force_load_after: list(&[], false),
        incompatible_with: list(&[], false),
        steam_app_id: None,
        by_version: AboutByVersionDto {
            advanced: true,
            descriptions: vec![AboutVersionTextDto {
                version: "1.6".into(),
                text: "For 1.6.".into(),
            }],
            relations: vec![AboutVersionRelationsDto {
                version: "1.5".into(),
                load_after: vec!["rs.old".into()],
                ..AboutVersionRelationsDto::default()
            }],
        },
        unknown_tags: vec!["customThing".into()],
        preview: AboutPreviewDto {
            exists: true,
            path: "About/Preview.png".into(),
            bytes: Some(48_213),
            width: Some(640),
            height: Some(360),
            sha256: Some("1".repeat(64)),
            data_url: None,
        },
        game_version: Some("1.6".into()),
        raw_text: "<ModMetaData/>".into(),
        raw_truncated: false,
        diagnostics: vec![finding(
            "about.package-id-collision",
            SeverityDto::Warning,
            "/packageId",
            "the mod \"Other\" in /mods/Other uses the same packageId",
        )],
    }
}

fn changes() -> Vec<AboutChangeDto> {
    vec![
        AboutChangeDto::Set {
            field: AboutTextFieldDto::Name,
            value: "RS Arms".into(),
        },
        AboutChangeDto::Clear {
            field: AboutTextFieldDto::Url,
        },
        AboutChangeDto::ListSet {
            field: AboutListFieldDto::SupportedVersions,
            items: vec!["1.6".into()],
        },
        AboutChangeDto::ListAdd {
            field: AboutListFieldDto::LoadAfter,
            value: "rs.base".into(),
            at: Some(0),
        },
        AboutChangeDto::ListRemove {
            field: AboutListFieldDto::LoadBefore,
            value: "rs.late".into(),
        },
        AboutChangeDto::ListMove {
            field: AboutListFieldDto::LoadAfter,
            value: "rs.base".into(),
            to: 2,
        },
        AboutChangeDto::DependencyAdd {
            dependency: AboutDependencyDto {
                package_id: "rs.base".into(),
                display_name: "RS Base".into(),
                ..AboutDependencyDto::default()
            },
            at: None,
        },
        AboutChangeDto::DependencyUpdate {
            package_id: "rs.base".into(),
            change: AboutDependencyChangeDto {
                display_name: Some("RS Base Mod".into()),
                download_url: Some(String::new()),
                ..AboutDependencyChangeDto::default()
            },
        },
        AboutChangeDto::DependencyRemove {
            package_id: "rs.gone".into(),
        },
        AboutChangeDto::DependencyMove {
            package_id: "rs.base".into(),
            to: 0,
        },
        AboutChangeDto::ByVersionListSet {
            field: AboutByVersionFieldDto::LoadAfter,
            version: "1.6".into(),
            items: vec!["rs.six".into()],
        },
        AboutChangeDto::ByVersionDescriptionSet {
            version: "1.6".into(),
            value: None,
        },
    ]
}

fn load_folders() -> ProjectLoadFoldersDto {
    ProjectLoadFoldersDto {
        project_id: "p-0a1b2c3d".into(),
        exists: true,
        path: "LoadFolders.xml".into(),
        file_hash: Some("2".repeat(64)),
        editable: true,
        not_editable_reason: None,
        blocks: vec![LoadBlockDto {
            index: 0,
            tag: "v1.6".into(),
            key: "1.6".into(),
            entries: vec![
                LoadEntryDto {
                    index: 0,
                    path: String::new(),
                    folder_exists: true,
                    ..LoadEntryDto::default()
                },
                LoadEntryDto {
                    index: 1,
                    path: "1.6/Compat/CE".into(),
                    if_mod_active: vec!["ceteam.combatextended".into()],
                    ignored_attributes: vec!["IfModActiveAny".into()],
                    folder_exists: false,
                    ..LoadEntryDto::default()
                },
            ],
        }],
        supported_versions: vec!["1.6".into()],
        version_folders: vec!["1.6".into()],
        raw_text: "<loadFolders/>".into(),
        raw_truncated: false,
        diagnostics: vec![finding(
            "loadfolders.folder-missing",
            SeverityDto::Warning,
            "/blocks/0/entries/1/path",
            "the folder 1.6/Compat/CE does not exist in the mod",
        )],
    }
}

#[test]
fn golden_about_model() {
    check("project-about", &about());
}

#[test]
fn golden_about_changes_cover_every_operation() {
    let all = changes();
    assert_eq!(all.len(), 12);
    check(
        "project-about-changes",
        &ProjectAboutUpdateRequest {
            project_id: "p-0a1b2c3d".into(),
            changes: all,
            expected_hash: Some("0".repeat(64)),
        },
    );
}

#[test]
fn golden_about_requests_and_responses() {
    check(
        "project-about-preview-request",
        &ProjectAboutPreviewRequest {
            project_id: "p-0a1b2c3d".into(),
            changes: changes().into_iter().take(2).collect(),
            expected_hash: Some("0".repeat(64)),
        },
    );
    check(
        "project-about-preview",
        &ProjectAboutPreviewDto {
            changed: true,
            diff: "--- About/About.xml\n+++ About/About.xml\n".into(),
            current_hash: "0".repeat(64),
            result: about(),
        },
    );
    check(
        "project-about-update",
        &ProjectAboutUpdateDto {
            written: true,
            diff: "--- About/About.xml\n".into(),
            backup: Some(
                "/data/project-backups/p-0a1b2c3d/About/About.xml.20261005T101010Z.bak".into(),
            ),
            verified: true,
            about: about(),
        },
    );
    check(
        "project-about-set-preview",
        &ProjectAboutSetPreviewDto {
            preview: about().preview,
            replaced: true,
            backup: None,
            diagnostics: vec![finding(
                "about.preview-dimensions",
                SeverityDto::Warning,
                "",
                "the preview image is 100 by 50 pixels",
            )],
        },
    );
    check(
        "project-about-remove-preview",
        &ProjectAboutRemovePreviewDto {
            removed: true,
            backup: Some("/data/backups/Preview.png.bak".into()),
        },
    );
}

#[test]
fn golden_load_folders_and_versions() {
    check("project-load-folders", &load_folders());
    check(
        "project-load-folders-changes",
        &ProjectLoadFoldersUpdateRequest {
            project_id: "p-0a1b2c3d".into(),
            expected_hash: Some("2".repeat(64)),
            create: false,
            dry_run: true,
            changes: vec![
                LoadFoldersChangeDto::AddBlock {
                    version: "1.6".into(),
                    entries: vec![LoadEntryInputDto {
                        path: "/".into(),
                        ..LoadEntryInputDto::default()
                    }],
                    at: Some(0),
                },
                LoadFoldersChangeDto::RemoveBlock { block: 1 },
                LoadFoldersChangeDto::AddEntry {
                    block: 0,
                    entry: LoadEntryInputDto {
                        path: "1.6/Compat/CE".into(),
                        if_mod_active: vec!["ceteam.combatextended".into()],
                        ..LoadEntryInputDto::default()
                    },
                    at: None,
                },
                LoadFoldersChangeDto::RemoveEntry { block: 0, entry: 2 },
                LoadFoldersChangeDto::MoveEntry {
                    block: 0,
                    entry: 2,
                    to: 0,
                },
                LoadFoldersChangeDto::SetEntry {
                    block: 0,
                    entry: 1,
                    change: LoadEntryChangeDto {
                        if_mod_active: Some(vec!["a.b".into()]),
                        drop_ignored_attributes: true,
                        ..LoadEntryChangeDto::default()
                    },
                },
            ],
        },
    );
    check(
        "project-load-folders-update",
        &ProjectLoadFoldersUpdateDto {
            written: true,
            created: false,
            diff: "--- LoadFolders.xml\n".into(),
            backup: Some("/data/backups/LoadFolders.xml.bak".into()),
            verified: true,
            load_folders: load_folders(),
        },
    );
    check(
        "project-version-add",
        &ProjectVersionAddDto {
            version: "1.7".into(),
            folder: "1.7".into(),
            created_folders: vec!["1.7".into(), "1.7/Patches".into()],
            block_added: true,
            load_folders_created: false,
            not_in_supported_versions: true,
            diagnostics: Vec::new(),
        },
    );
}

#[test]
fn golden_library_mod_search() {
    check(
        "library-mod-search",
        &LibraryModSearchDto {
            scanned: true,
            hits: vec![LibraryModHitDto {
                package_id: "ceteam.combatextended".into(),
                name: "Combat Extended".into(),
                authors: vec!["CE Team".into()],
                source: SourceKindDto::Workshop,
                workshop_id: Some("2890901044".into()),
                workshop_url: Some(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id=2890901044".into(),
                ),
                path: "/steam/workshop/content/294100/2890901044".into(),
                loadable: true,
                url: None,
            }],
            hint: None,
        },
    );
    check(
        "library-mod-search-empty",
        &LibraryModSearchDto {
            scanned: false,
            hits: Vec::new(),
            hint: Some("The library has not been scanned yet. Scan it to search your mods.".into()),
        },
    );
}

#[test]
fn a_change_without_an_operation_is_refused() {
    assert!(serde_json::from_str::<AboutChangeDto>(r#"{"field":"name","value":"x"}"#).is_err());
    assert!(serde_json::from_str::<AboutChangeDto>(r#"{"op":"explode"}"#).is_err());
    assert!(
        serde_json::from_str::<AboutChangeDto>(r#"{"op":"set","field":"nope","value":"x"}"#)
            .is_err()
    );
    let ok: AboutChangeDto =
        serde_json::from_str(r#"{"op":"list-add","field":"loadAfter","value":"a.b"}"#).unwrap();
    assert_eq!(
        ok,
        AboutChangeDto::ListAdd {
            field: AboutListFieldDto::LoadAfter,
            value: "a.b".into(),
            at: None
        }
    );
}
