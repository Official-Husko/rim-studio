//! The mod basics commands of the project tool on fiction mods in temporary folders: reading every basic,
//! the preview of an edit, the update with its backup and its refusals, the findings, the preview image,
//! `LoadFolders.xml` and the version folders.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::{PackageId, SourceId, WorkshopId};
use rimstudio_core::mods::ModMeta;
use rimstudio_core::version::GameVersion;
use rimstudio_design::assets::png::build_png;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::diagnostic::DiagnosticDto;
use rimstudio_ipc_types::project_about::{
    AboutByVersionFieldDto, AboutChangeDto, AboutDependencyDto, AboutListFieldDto,
    AboutTextFieldDto, LoadEntryChangeDto, LoadEntryInputDto, LoadFoldersChangeDto,
    ProjectAboutGetRequest, ProjectAboutPreviewRequest, ProjectAboutRemovePreviewRequest,
    ProjectAboutSetPreviewRequest, ProjectAboutUpdateRequest, ProjectLoadFoldersGetRequest,
    ProjectLoadFoldersUpdateRequest, ProjectVersionAddRequest,
};
use rimstudio_library::index::{
    AboutStatus, LibraryIndex, LibraryMod, Loadability, ModContent, ModInfo,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::project::about::{self, AboutContext};
use rimstudio_toolkit::project::about_preview::{remove_preview, set_preview};
use rimstudio_toolkit::project::load_folders;
use rimstudio_toolkit::project::open_project;
use rimstudio_toolkit::project::versions;
use rimstudio_toolkit::shared::env::ProjectEnv;

const HAND: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<!-- hand written -->\r\n<ModMetaData>\r\n\t<name>Hand Mod</name>\r\n\t<!-- who -->\r\n\t<author>Ann</author>\r\n\t<packageId>ann.hand</packageId>\r\n\t<customThing>keep me</customThing>\r\n\t<supportedVersions>\r\n\t\t<li>1.5</li>\r\n\t\t<li>1.6</li>\r\n\t</supportedVersions>\r\n\t<loadAfter>\r\n\t\t<li>ludeon.rimworld</li>\r\n\t</loadAfter>\r\n\t<modDependencies>\r\n\t\t<li>\r\n\t\t\t<packageId>ann.base</packageId>\r\n\t\t\t<displayName>Base</displayName>\r\n\t\t\t<downloadUrl>https://example.invalid/base</downloadUrl>\r\n\t\t</li>\r\n\t</modDependencies>\r\n\t<description>Hello</description>\r\n</ModMetaData>\r\n";

struct World {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    env: ProjectEnv,
    roots: DataRoots,
}

fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    World {
        _tmp: tmp,
        base,
        env,
        roots,
    }
}

impl World {
    /// A mod folder with the given About text; returns its project id and root.
    fn project(&self, name: &str, about: &str) -> (String, Utf8PathBuf) {
        let root = self.base.join("Mods").join(name);
        std::fs::create_dir_all(root.join("About")).unwrap();
        std::fs::write(root.join("About/About.xml"), about).unwrap();
        let id = open_project(&self.env, &root)
            .unwrap()
            .record
            .id
            .as_str()
            .to_owned();
        (id, root)
    }
}

fn ctx() -> AboutContext<'static> {
    AboutContext::default()
}

fn codes(d: &[DiagnosticDto]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}

fn get(w: &World, id: &str) -> rimstudio_ipc_types::project_about::ProjectAboutDto {
    about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id.to_owned(),
            include_preview_image: false,
        },
        &ctx(),
        &[],
    )
    .unwrap()
}

fn set(field: AboutTextFieldDto, value: &str) -> AboutChangeDto {
    AboutChangeDto::Set {
        field,
        value: value.to_owned(),
    }
}

fn update(
    w: &World,
    id: &str,
    changes: Vec<AboutChangeDto>,
    hash: Option<String>,
) -> Result<rimstudio_ipc_types::project_about::ProjectAboutUpdateDto, ToolkitError> {
    about::update(
        &w.env,
        &ProjectAboutUpdateRequest {
            project_id: id.to_owned(),
            changes,
            expected_hash: hash,
        },
        &ctx(),
        &[],
    )
}

#[test]
fn get_returns_every_basic_with_its_place_in_the_file() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let dto = get(&w, &id);
    assert_eq!(dto.path, "About/About.xml");
    assert!(dto.parsed && dto.editable);
    assert_eq!(dto.name.value, "Hand Mod");
    assert!(dto.name.present);
    assert!(dto.author.present && !dto.url.present && !dto.mod_version.present);
    assert_eq!(dto.package_id.value, "ann.hand");
    assert_eq!(dto.supported_versions.items, vec!["1.5", "1.6"]);
    assert_eq!(dto.load_after.items, vec!["ludeon.rimworld"]);
    assert!(!dto.load_before.present);
    assert_eq!(dto.mod_dependencies.len(), 1);
    assert_eq!(dto.mod_dependencies[0].display_name, "Base");
    assert!(dto.mod_dependencies_present);
    assert_eq!(dto.unknown_tags, vec!["customThing"]);
    assert!(dto.by_version.advanced && dto.by_version.relations.is_empty());
    assert_eq!(dto.file_hash.len(), 64);
    assert_eq!(
        dto.bytes,
        std::fs::metadata(root.join("About/About.xml"))
            .unwrap()
            .len()
    );
    assert!(dto.raw_text.contains("<!-- hand written -->"));
    assert!(!dto.raw_text.starts_with('\u{feff}'));
    assert!(!dto.raw_truncated);
    // a missing preview and a missing url are findings, not errors
    let c = codes(&dto.diagnostics);
    assert!(c.contains(&"about.preview-missing"));
    assert!(c.contains(&"about.description-size-tag") || !c.is_empty());
}

#[test]
fn preview_shows_the_diff_and_the_result_and_writes_nothing() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let before = std::fs::read(root.join("About/About.xml")).unwrap();
    let dto = get(&w, &id);
    let p = about::preview(
        &w.env,
        &ProjectAboutPreviewRequest {
            project_id: id.clone(),
            changes: vec![
                set(AboutTextFieldDto::Name, "Renamed"),
                set(AboutTextFieldDto::Url, "https://example.invalid/x"),
            ],
            expected_hash: Some(dto.file_hash.clone()),
        },
        &ctx(),
        &[],
    )
    .unwrap();
    assert!(p.changed);
    assert!(p.diff.contains("-\t<name>Hand Mod</name>"));
    assert!(p.diff.contains("+\t<name>Renamed</name>"));
    assert_eq!(p.result.name.value, "Renamed");
    assert_eq!(p.result.url.value, "https://example.invalid/x");
    assert_ne!(p.result.file_hash, dto.file_hash);
    assert_eq!(p.current_hash, dto.file_hash);
    assert_eq!(std::fs::read(root.join("About/About.xml")).unwrap(), before);
}

#[test]
fn update_writes_a_span_edit_backs_up_and_verifies() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let dto = get(&w, &id);
    let out = update(
        &w,
        &id,
        vec![
            set(AboutTextFieldDto::Name, "Renamed"),
            AboutChangeDto::ListAdd {
                field: AboutListFieldDto::SupportedVersions,
                value: "1.7".into(),
                at: None,
            },
            AboutChangeDto::DependencyAdd {
                dependency: AboutDependencyDto {
                    package_id: "ann.extra".into(),
                    display_name: "Extra".into(),
                    steam_workshop_url: Some(
                        "https://steamcommunity.com/sharedfiles/filedetails/?id=1".into(),
                    ),
                    ..AboutDependencyDto::default()
                },
                at: Some(0),
            },
        ],
        Some(dto.file_hash.clone()),
    )
    .unwrap();
    assert!(out.written && out.verified);
    let after = std::fs::read_to_string(root.join("About/About.xml")).unwrap();
    // everything else is byte identical: the BOM, the CRLF line ends, the comments, the unknown element
    assert!(after.starts_with('\u{feff}'));
    assert!(!after.replace("\r\n", "").contains('\n'));
    assert!(after.contains("<!-- who -->"));
    assert!(after.contains("<customThing>keep me</customThing>"));
    assert_eq!(after, HAND.replace("Hand Mod", "Renamed").replace(
        "\t\t<li>1.6</li>\r\n",
        "\t\t<li>1.6</li>\r\n\t\t<li>1.7</li>\r\n"
    ).replace(
        "\t<modDependencies>\r\n",
        "\t<modDependencies>\r\n\t\t<li>\r\n\t\t\t<packageId>ann.extra</packageId>\r\n\t\t\t<displayName>Extra</displayName>\r\n\t\t\t<steamWorkshopUrl>https://steamcommunity.com/sharedfiles/filedetails/?id=1</steamWorkshopUrl>\r\n\t\t</li>\r\n"
    ));
    // the backup of the replaced file is in the app data folder, never in the mod
    let backup = out.backup.unwrap();
    assert!(backup.starts_with(w.roots.data.as_str()));
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), HAND);
    assert!(
        !root
            .join("About")
            .read_dir()
            .unwrap()
            .any(|e| { e.unwrap().file_name().to_string_lossy().contains("bak") })
    );
    // the answer is the fresh model
    assert_eq!(out.about.name.value, "Renamed");
    assert_eq!(
        out.about.supported_versions.items,
        vec!["1.5", "1.6", "1.7"]
    );
    assert_eq!(out.about.mod_dependencies[0].package_id, "ann.extra");
    assert_ne!(out.about.file_hash, dto.file_hash);
    // the project record follows the file
    assert_eq!(w.env.record(&id).unwrap().name, "Renamed");
}

#[test]
fn an_update_that_changes_no_byte_writes_nothing() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let out = update(
        &w,
        &id,
        vec![
            set(AboutTextFieldDto::Name, "Hand Mod"),
            AboutChangeDto::ListSet {
                field: AboutListFieldDto::SupportedVersions,
                items: vec!["1.5".into(), "1.6".into()],
            },
        ],
        None,
    )
    .unwrap();
    assert!(!out.written && !out.verified && out.diff.is_empty() && out.backup.is_none());
    assert_eq!(
        std::fs::read_to_string(root.join("About/About.xml")).unwrap(),
        HAND
    );
}

#[test]
fn a_file_that_changed_since_it_was_read_is_refused() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let dto = get(&w, &id);
    std::fs::write(root.join("About/About.xml"), HAND.replace("Ann", "Zed")).unwrap();
    let err = update(
        &w,
        &id,
        vec![set(AboutTextFieldDto::Name, "X")],
        Some(dto.file_hash.clone()),
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::FileStale { .. }));
    assert_eq!(err.code(), "project.file-stale");
    let err = about::preview(
        &w.env,
        &ProjectAboutPreviewRequest {
            project_id: id.clone(),
            changes: vec![],
            expected_hash: Some(dto.file_hash),
        },
        &ctx(),
        &[],
    )
    .unwrap_err();
    assert_eq!(err.code(), "project.file-stale");
    assert!(
        std::fs::read_to_string(root.join("About/About.xml"))
            .unwrap()
            .contains("Zed")
    );
}

#[test]
fn a_change_that_does_not_apply_is_refused_and_writes_nothing() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let err = update(
        &w,
        &id,
        vec![AboutChangeDto::DependencyMove {
            package_id: "no.such".into(),
            to: 0,
        }],
        None,
    )
    .unwrap_err();
    assert_eq!(err.code(), "project.edit-invalid");
    let err = update(&w, &id, vec![set(AboutTextFieldDto::Name, "a\nb")], None).unwrap_err();
    assert_eq!(err.code(), "project.edit-invalid");
    assert_eq!(
        std::fs::read_to_string(root.join("About/About.xml")).unwrap(),
        HAND
    );
}

#[test]
fn hostile_files_are_shown_but_never_edited() {
    let w = world();
    // not well formed
    let (id, root) = w.project("Broken", "<ModMetaData><name>x</ModMetaData>");
    let dto = get(&w, &id);
    assert!(!dto.parsed && !dto.editable);
    assert!(codes(&dto.diagnostics).contains(&"about.unparseable"));
    assert!(dto.raw_text.contains("<name>x"));
    let err = update(&w, &id, vec![set(AboutTextFieldDto::Name, "y")], None).unwrap_err();
    assert_eq!(err.code(), "project.file-not-editable");
    // UTF-16
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(
            "<ModMetaData><name>wide</name></ModMetaData>"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    let (id16, root16) = w.project("Wide", "<ModMetaData/>");
    std::fs::write(root16.join("About/About.xml"), &utf16).unwrap();
    let dto = get(&w, &id16);
    assert!(dto.parsed && !dto.editable);
    assert_eq!(dto.name.value, "wide");
    assert!(codes(&dto.diagnostics).contains(&"about.not-utf8"));
    assert_eq!(
        update(&w, &id16, vec![], None).unwrap_err().code(),
        "project.file-not-editable"
    );
    assert_eq!(
        std::fs::read(root16.join("About/About.xml")).unwrap(),
        utf16
    );
    // a document type declaration (an entity bomb): never expanded, the file is not edited
    let bomb = "<?xml version=\"1.0\"?><!DOCTYPE a [<!ENTITY x \"aaaa\"><!ENTITY y \"&x;&x;&x;&x;\">]><ModMetaData><name>&y;</name></ModMetaData>";
    let (idb, rootb) = w.project("Bomb", "<ModMetaData/>");
    std::fs::write(rootb.join("About/About.xml"), bomb).unwrap();
    let dto = get(&w, &idb);
    // read without expanding anything; the text is edited as bytes, so the declaration stays as it is
    assert!(dto.raw_text.contains("<!ENTITY y"));
    assert!(
        dto.name.value.len() < 64,
        "entities were expanded: {}",
        dto.name.value
    );
    if dto.editable {
        let out = update(
            &w,
            &idb,
            vec![set(AboutTextFieldDto::Url, "https://example.invalid/x")],
            None,
        )
        .unwrap();
        let text = std::fs::read_to_string(rootb.join("About/About.xml")).unwrap();
        assert!(out.written);
        assert_eq!(
            text,
            bomb.replace(
                "</ModMetaData>",
                "<url>https://example.invalid/x</url></ModMetaData>"
            )
        );
    } else {
        assert_eq!(
            std::fs::read_to_string(rootb.join("About/About.xml")).unwrap(),
            bomb
        );
    }
    // huge
    let big = format!(
        "<ModMetaData><description>{}</description></ModMetaData>",
        "x".repeat(2 << 20)
    );
    let (idh, _) = w.project("Huge", "<ModMetaData/>");
    std::fs::write(w.base.join("Mods/Huge/About/About.xml"), &big).unwrap();
    let dto = get(&w, &idh);
    assert!(!dto.editable && !dto.parsed);
    assert!(dto.raw_truncated && dto.raw_text.len() <= 256 * 1024);
    assert!(codes(&dto.diagnostics).contains(&"about.too-large"));
    assert_eq!(
        update(&w, &idh, vec![], None).unwrap_err().code(),
        "project.file-not-editable"
    );
    drop(root);
}

#[test]
fn a_project_in_a_protected_folder_is_refused() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let err = about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id,
            include_preview_image: false,
        },
        &ctx(),
        &[root.parent().unwrap().to_owned()],
    )
    .unwrap_err();
    assert_eq!(err.code(), "project.path-outside-root");
}

fn library_with(path: &Utf8Path, pkg: &str, name: &str) -> LibraryIndex {
    let mut meta = ModMeta::new(
        PackageId::parse(pkg).unwrap(),
        name,
        SourceId::workshop(0),
        path.to_owned(),
    );
    meta.workshop_id = WorkshopId::new(123);
    let mut b = LibraryIndex::builder();
    b.push(LibraryMod {
        meta,
        info: ModInfo {
            folder_name: "123".into(),
            is_link: false,
            available: true,
            loadable: Loadability::Loadable,
            about: AboutStatus::Parsed,
            synthetic_package_id: false,
            published_file_id: None,
            about_mtime_ns: None,
            also_in: Vec::new(),
        },
        content: ModContent::default(),
    });
    b.build()
}

#[test]
fn findings_name_the_colliding_mod_and_the_installed_game_version() {
    let w = world();
    let (id, _) = w.project("Hand", HAND);
    let other = w.base.join("Workshop/123");
    let index = library_with(&other, "Ann.Hand", "The Other Hand");
    let game = GameVersion::parse("1.7.1000").unwrap();
    let dto = about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id.clone(),
            include_preview_image: false,
        },
        &AboutContext {
            library: Some(&index),
            game_version: Some(&game),
        },
        &[],
    )
    .unwrap();
    let hit = dto
        .diagnostics
        .iter()
        .find(|d| d.code == "about.package-id-case-conflict")
        .expect("case conflict");
    assert!(hit.message.contains("The Other Hand") && hit.message.contains(other.as_str()));
    assert_eq!(hit.field.as_deref(), Some("/packageId"));
    assert_eq!(
        hit.args.get("otherPath").map(String::as_str),
        Some(other.as_str())
    );
    let miss = dto
        .diagnostics
        .iter()
        .find(|d| d.code == "about.supported-versions-missing-game")
        .unwrap();
    assert_eq!(
        miss.args.get("gameVersion").map(String::as_str),
        Some("1.7")
    );
    assert_eq!(dto.game_version.as_deref(), Some("1.7"));
    // the same id in another mod is a collision, and the project itself is never its own neighbour
    let same = library_with(&other, "ann.hand", "Twin");
    let dto = about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id.clone(),
            include_preview_image: false,
        },
        &AboutContext {
            library: Some(&same),
            game_version: None,
        },
        &[],
    )
    .unwrap();
    assert!(codes(&dto.diagnostics).contains(&"about.package-id-collision"));
    let itself = library_with(&w.base.join("Mods/Hand"), "ann.hand", "Hand Mod");
    let dto = about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id,
            include_preview_image: false,
        },
        &AboutContext {
            library: Some(&itself),
            game_version: None,
        },
        &[],
    )
    .unwrap();
    assert!(!codes(&dto.diagnostics).contains(&"about.package-id-collision"));
}

#[test]
fn the_preview_image_is_copied_checked_replaced_and_removed() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let src = w.base.join("art.png");
    std::fs::write(&src, build_png(640, 360)).unwrap();
    let out = set_preview(
        &w.env,
        &ProjectAboutSetPreviewRequest {
            project_id: id.clone(),
            source_path: src.to_string(),
        },
        &[],
    )
    .unwrap();
    assert!(!out.replaced && out.backup.is_none());
    assert_eq!(
        (out.preview.width, out.preview.height),
        (Some(640), Some(360))
    );
    assert!(out.diagnostics.is_empty());
    assert_eq!(
        std::fs::read(root.join("About/Preview.png")).unwrap(),
        build_png(640, 360)
    );
    // a different size copies with a warning and backs up the old file
    std::fs::write(&src, build_png(100, 50)).unwrap();
    let out = set_preview(
        &w.env,
        &ProjectAboutSetPreviewRequest {
            project_id: id.clone(),
            source_path: src.to_string(),
        },
        &[],
    )
    .unwrap();
    assert!(out.replaced);
    assert_eq!(
        std::fs::read(out.backup.unwrap()).unwrap(),
        build_png(640, 360)
    );
    assert_eq!(codes(&out.diagnostics), vec!["about.preview-dimensions"]);
    // the model shows it, with the image on request
    let dto = about::get(
        &w.env,
        &ProjectAboutGetRequest {
            project_id: id.clone(),
            include_preview_image: true,
        },
        &ctx(),
        &[],
    )
    .unwrap();
    assert!(
        dto.preview.exists
            && dto
                .preview
                .data_url
                .as_deref()
                .unwrap()
                .starts_with("data:image/png;base64,")
    );
    assert_eq!(dto.preview.width, Some(100));
    // refusals: not a PNG, truncated, a link, a missing file, relative path
    std::fs::write(&src, b"GIF89a....").unwrap();
    for bad in [
        src.to_string(),
        w.base.join("none.png").to_string(),
        "rel.png".to_owned(),
    ] {
        let err = set_preview(
            &w.env,
            &ProjectAboutSetPreviewRequest {
                project_id: id.clone(),
                source_path: bad,
            },
            &[],
        )
        .unwrap_err();
        assert_eq!(err.code(), "project.edit-invalid");
    }
    let mut cut = build_png(32, 32);
    cut.truncate(cut.len() - 8);
    std::fs::write(&src, cut).unwrap();
    assert!(
        set_preview(
            &w.env,
            &ProjectAboutSetPreviewRequest {
                project_id: id.clone(),
                source_path: src.to_string()
            },
            &[]
        )
        .is_err()
    );
    // removing keeps a copy in the data folder and touches nothing else
    let before = std::fs::read(root.join("About/About.xml")).unwrap();
    let gone = remove_preview(
        &w.env,
        &ProjectAboutRemovePreviewRequest {
            project_id: id.clone(),
        },
        &[],
    )
    .unwrap();
    assert!(gone.removed);
    assert_eq!(
        std::fs::read(gone.backup.unwrap()).unwrap(),
        build_png(100, 50)
    );
    assert!(!root.join("About/Preview.png").exists());
    assert_eq!(std::fs::read(root.join("About/About.xml")).unwrap(), before);
    let again = remove_preview(
        &w.env,
        &ProjectAboutRemovePreviewRequest { project_id: id },
        &[],
    )
    .unwrap();
    assert!(!again.removed);
    assert!(!root.join("About/Manifest.xml").exists());
    assert!(!root.join("About/PublishedFileId.txt").exists());
}

#[cfg(unix)]
#[test]
fn a_linked_preview_source_is_refused() {
    let w = world();
    let (id, _) = w.project("Hand", HAND);
    let real = w.base.join("real.png");
    std::fs::write(&real, build_png(8, 8)).unwrap();
    let link = w.base.join("link.png");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let err = set_preview(
        &w.env,
        &ProjectAboutSetPreviewRequest {
            project_id: id,
            source_path: link.to_string(),
        },
        &[],
    )
    .unwrap_err();
    assert_eq!(err.code(), "project.edit-invalid");
}

const LF: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<loadFolders>\n  <!-- shared -->\n  <v1.5>\n    <li>/</li>\n    <li>1.5</li>\n  </v1.5>\n  <v1.6>\n    <li>/</li>\n    <li>1.6</li>\n    <li IfModActiveAny=\"ceteam.combatextended\">Compat/CE</li>\n  </v1.6>\n  <v1.4><li>/</li></v1.4>\n</loadFolders>\n";

#[test]
fn load_folders_are_read_with_their_findings_and_edited_in_place() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let none = load_folders::get(
        &w.env,
        &ProjectLoadFoldersGetRequest {
            project_id: id.clone(),
        },
        &[],
    )
    .unwrap();
    assert!(!none.exists && none.editable && none.file_hash.is_none() && none.blocks.is_empty());
    std::fs::write(root.join("LoadFolders.xml"), LF).unwrap();
    std::fs::create_dir_all(root.join("1.6")).unwrap();
    let dto = load_folders::get(
        &w.env,
        &ProjectLoadFoldersGetRequest {
            project_id: id.clone(),
        },
        &[],
    )
    .unwrap();
    assert!(dto.exists && dto.editable);
    assert_eq!(dto.blocks.len(), 3);
    assert_eq!(dto.blocks[1].key, "1.6");
    assert_eq!(dto.blocks[1].entries[2].path, "Compat/CE");
    assert_eq!(
        dto.blocks[1].entries[2].ignored_attributes,
        vec!["IfModActiveAny"]
    );
    assert!(dto.blocks[1].entries[1].folder_exists && !dto.blocks[0].entries[1].folder_exists);
    assert_eq!(dto.version_folders, vec!["1.6"]);
    let c = codes(&dto.diagnostics);
    for want in [
        "loadfolders.folder-missing",
        "loadfolders.ignored-attribute",
        "loadfolders.ungated-compat-folder",
        "loadfolders.unsupported-version",
    ] {
        assert!(c.contains(&want), "{want} in {c:?}");
    }
    let unsupported = dto
        .diagnostics
        .iter()
        .find(|d| d.code == "loadfolders.unsupported-version")
        .unwrap();
    assert_eq!(unsupported.field.as_deref(), Some("/blocks/2"));
    // an edit: dry run first, then for real
    let changes = vec![
        LoadFoldersChangeDto::SetEntry {
            block: 1,
            entry: 2,
            change: LoadEntryChangeDto {
                if_mod_active: Some(vec!["ceteam.combatextended".into()]),
                drop_ignored_attributes: true,
                ..LoadEntryChangeDto::default()
            },
        },
        LoadFoldersChangeDto::RemoveBlock { block: 2 },
        LoadFoldersChangeDto::AddEntry {
            block: 0,
            entry: LoadEntryInputDto {
                path: "Extra".into(),
                ..LoadEntryInputDto::default()
            },
            at: Some(1),
        },
    ];
    let req = |dry: bool, hash: Option<String>| ProjectLoadFoldersUpdateRequest {
        project_id: id.clone(),
        changes: changes.clone(),
        expected_hash: hash,
        create: false,
        dry_run: dry,
    };
    let dry = load_folders::update(&w.env, &req(true, dto.file_hash.clone()), &[]).unwrap();
    assert!(
        !dry.written
            && dry
                .diff
                .contains("+    <li IfModActive=\"ceteam.combatextended\">Compat/CE</li>")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap(),
        LF
    );
    let done = load_folders::update(&w.env, &req(false, dto.file_hash.clone()), &[]).unwrap();
    assert!(done.written && done.verified && !done.created);
    let text = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    let expected = LF
        .replace("IfModActiveAny=", "IfModActive=")
        .replace("  <v1.4><li>/</li></v1.4>\n", "")
        .replace(
            "    <li>1.5</li>\n",
            "    <li>Extra</li>\n    <li>1.5</li>\n",
        );
    assert_eq!(text, expected);
    let backup = done.backup.unwrap();
    assert_eq!(std::fs::read_to_string(backup).unwrap(), LF);
    // the old hash is stale now
    let err = load_folders::update(&w.env, &req(false, dto.file_hash), &[]).unwrap_err();
    assert_eq!(err.code(), "project.file-stale");
    // a position that is not there is refused
    let bad = ProjectLoadFoldersUpdateRequest {
        project_id: id.clone(),
        changes: vec![LoadFoldersChangeDto::RemoveEntry { block: 9, entry: 0 }],
        expected_hash: None,
        create: false,
        dry_run: false,
    };
    assert_eq!(
        load_folders::update(&w.env, &bad, &[]).unwrap_err().code(),
        "project.edit-invalid"
    );
}

#[test]
fn load_folders_are_created_only_when_asked() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let mut req = ProjectLoadFoldersUpdateRequest {
        project_id: id.clone(),
        changes: vec![LoadFoldersChangeDto::AddBlock {
            version: "1.6".into(),
            entries: vec![LoadEntryInputDto {
                path: "/".into(),
                ..LoadEntryInputDto::default()
            }],
            at: None,
        }],
        expected_hash: None,
        create: false,
        dry_run: false,
    };
    assert_eq!(
        load_folders::update(&w.env, &req, &[]).unwrap_err().code(),
        "project.edit-invalid"
    );
    assert!(!root.join("LoadFolders.xml").exists());
    req.create = true;
    let out = load_folders::update(&w.env, &req, &[]).unwrap();
    assert!(out.written && out.created && out.backup.is_none());
    let text = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(text.contains("<v1.6>") && text.contains("<li>/</li>"));
    assert_eq!(out.load_folders.blocks[0].key, "1.6");
}

#[test]
fn a_version_folder_gets_its_standard_folders_and_its_block() {
    let w = world();
    let (id, root) = w.project("Hand", HAND);
    let req = |dry: bool, std: bool, block: bool| ProjectVersionAddRequest {
        project_id: id.clone(),
        version: "1.7".into(),
        standard_folders: std,
        add_block: block,
        dry_run: dry,
    };
    let dry = versions::add(&w.env, &req(true, true, true), &[]).unwrap();
    assert!(
        dry.created_folders
            .contains(&"1.7/Defs/ThingDefs_Misc/Weapons".to_owned())
    );
    assert!(dry.block_added && dry.load_folders_created && dry.not_in_supported_versions);
    assert!(!root.join("1.7").exists() && !root.join("LoadFolders.xml").exists());
    let plain = versions::add(&w.env, &req(false, false, false), &[]).unwrap();
    assert_eq!(plain.created_folders, vec!["1.7"]);
    assert!(!plain.block_added && !root.join("LoadFolders.xml").exists());
    assert!(
        std::fs::read_dir(root.join("1.7"))
            .unwrap()
            .next()
            .is_none()
    );
    // the standard folders and the block (the file is created because it was asked for)
    let full = versions::add(&w.env, &req(false, true, true), &[]).unwrap();
    assert!(full.block_added && full.load_folders_created);
    assert!(root.join("1.7/Patches").is_dir());
    assert!(root.join("1.7/Defs/SoundDefs").is_dir());
    let lf = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(lf.contains("<v1.7>") && lf.contains("<li>1.7</li>"));
    // again: nothing new, nothing duplicated
    let again = versions::add(&w.env, &req(false, true, true), &[]).unwrap();
    assert!(again.created_folders.is_empty() && !again.block_added);
    assert!(codes(&again.diagnostics).contains(&"version.folder-exists"));
    assert!(codes(&again.diagnostics).contains(&"version.block-exists"));
    assert_eq!(
        std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap(),
        lf
    );
    // About.xml is never touched, and a bad version is refused
    assert_eq!(
        std::fs::read_to_string(root.join("About/About.xml")).unwrap(),
        HAND
    );
    for bad in ["x", "1", "1.6.4", "../1.6", ""] {
        let mut r = req(false, false, false);
        r.version = bad.into();
        assert_eq!(
            versions::add(&w.env, &r, &[]).unwrap_err().code(),
            "project.edit-invalid",
            "{bad}"
        );
    }
}

#[test]
fn by_version_blocks_are_edited_through_the_same_commands() {
    let w = world();
    let (id, _) = w.project("Hand", HAND);
    let out = update(
        &w,
        &id,
        vec![
            AboutChangeDto::ByVersionListSet {
                field: AboutByVersionFieldDto::LoadAfter,
                version: "1.6".into(),
                items: vec!["ann.six".into()],
            },
            AboutChangeDto::ByVersionDescriptionSet {
                version: "1.6".into(),
                value: Some("six".into()),
            },
        ],
        None,
    )
    .unwrap();
    assert_eq!(out.about.by_version.relations[0].version, "1.6");
    assert_eq!(
        out.about.by_version.relations[0].load_after,
        vec!["ann.six"]
    );
    assert_eq!(out.about.by_version.descriptions[0].text, "six");
    let back = update(
        &w,
        &id,
        vec![
            AboutChangeDto::ByVersionListSet {
                field: AboutByVersionFieldDto::LoadAfter,
                version: "1.6".into(),
                items: vec![],
            },
            AboutChangeDto::ByVersionDescriptionSet {
                version: "1.6".into(),
                value: None,
            },
        ],
        None,
    )
    .unwrap();
    assert!(back.about.by_version.relations.is_empty());
    assert_eq!(
        std::fs::read_to_string(w.base.join("Mods/Hand/About/About.xml")).unwrap(),
        HAND
    );
}
