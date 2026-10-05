//! About.xml reading, editing and creation.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::ids::SourceId;
use rimstudio_core::version::GameVersion;
use rimstudio_xml::about::{
    AboutOptions, AboutSpec, add_load_after, add_supported_version, create_about, read_lenient,
    read_lenient_with, render_about, set_description, set_name, set_package_id,
    set_supported_versions,
};
use rimstudio_xml::render::RenderOpts;

const FULL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>RS Test Mod</name>
  <author>Ann and Bob, Cy</author>
  <packageId> RS.TestMod </packageId>
  <url>https://example.invalid/rs</url>
  <modVersion>0.1</modVersion>
  <steamAppId>12</steamAppId>
  <supportedVersions>
    <li>1.5</li>
    <li>v1.6</li>
    <li>1.6.4871</li>
  </supportedVersions>
  <loadAfter><li>Ludeon.RimWorld</li><li>rs.other</li></loadAfter>
  <loadBefore><li>rs.late</li></loadBefore>
  <incompatibleWith><li>rs.bad</li></incompatibleWith>
  <forceLoadAfter><li>ludeon.rimworld.royalty</li></forceLoadAfter>
  <modDependencies>
    <li>
      <packageId>rs.dep</packageId>
      <displayName>RS Dep</displayName>
      <downloadUrl>https://example.invalid/dep</downloadUrl>
      <alternativePackageIds><li>rs.dep.alt</li></alternativePackageIds>
    </li>
    <li><displayName>no id</displayName></li>
  </modDependencies>
  <modDependenciesByVersion>
    <v1.5><li><packageId>rs.old.dep</packageId><displayName>Old</displayName></li></v1.5>
    <V1.6><li><packageId>rs.new.dep</packageId><displayName>New</displayName></li></V1.6>
  </modDependenciesByVersion>
  <loadAfterByVersion>
    <v1.6><li>rs.six</li></v1.6>
    <1.6><li>rs.ignored</li></1.6>
  </loadAfterByVersion>
  <descriptionsByVersion><v1.6>six</v1.6></descriptionsByVersion>
  <description>Hello &amp; welcome</description>
  <extraThing>x</extraThing>
</ModMetaData>
"#;

fn codes(r: &rimstudio_xml::about::AboutRead) -> Vec<&str> {
    r.warnings.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn reads_every_field_of_a_full_file() {
    let r = read_lenient(FULL.as_bytes());
    assert!(r.parsed);
    let a = &r.about;
    assert_eq!(a.name, "RS Test Mod");
    assert_eq!(a.package_id, "RS.TestMod");
    assert_eq!(a.author_list(), ["Ann", "Bob", "Cy"]);
    assert_eq!(a.url.as_deref(), Some("https://example.invalid/rs"));
    assert_eq!(a.steam_app_id, Some(12));
    assert_eq!(a.supported_versions, ["1.5", "v1.6", "1.6.4871"]);
    assert!(a.has_supported_versions);
    assert_eq!(a.load_after, ["Ludeon.RimWorld", "rs.other"]);
    assert_eq!(a.load_before, ["rs.late"]);
    assert_eq!(a.incompatible_with, ["rs.bad"]);
    assert_eq!(a.force_load_after, ["ludeon.rimworld.royalty"]);
    assert_eq!(a.description, "Hello & welcome");
    assert_eq!(a.mod_dependencies.len(), 1);
    assert_eq!(a.mod_dependencies[0].package_id, "rs.dep");
    assert_eq!(a.mod_dependencies[0].alternatives, ["rs.dep.alt"]);
    assert_eq!(a.unknown_tags, ["extraThing"]);
    assert!(codes(&r).contains(&"about.dependency-no-id"));
    assert!(codes(&r).contains(&"about.unknown-tag"));
}

#[test]
fn by_version_keys_are_normalised_and_the_first_one_wins() {
    let a = read_lenient(FULL.as_bytes()).about;
    assert_eq!(a.by_version.len(), 2);
    assert_eq!(
        a.by_version["1.5"].mod_dependencies[0].package_id,
        "rs.old.dep"
    );
    assert_eq!(
        a.by_version["1.6"].mod_dependencies[0].package_id,
        "rs.new.dep"
    );
    assert_eq!(a.by_version["1.6"].load_after, ["rs.six"]);
    assert_eq!(a.descriptions_by_version["1.6"], "six");
    let r = read_lenient(FULL.as_bytes());
    assert!(codes(&r).contains(&"about.duplicate-version-key"));
}

#[test]
fn into_meta_feeds_the_core_version_rules() {
    let a = read_lenient(FULL.as_bytes()).about;
    let meta = a
        .into_meta(SourceId::game_mods(), "/mods/RSTest".into())
        .unwrap();
    let v16 = GameVersion::parse("1.6.4871 rev598").unwrap();
    assert!(meta.supports(&v16));
    assert_eq!(
        meta.effective_load_after(&v16),
        ["Ludeon.RimWorld", "rs.other", "rs.six"]
    );
    assert_eq!(meta.effective_dependencies(&v16).len(), 2);
    assert_eq!(meta.authors, ["Ann", "Bob", "Cy"]);
    assert_eq!(meta.package_id.lower(), "rs.testmod");
}

#[test]
fn into_meta_uses_the_folder_name_when_the_name_is_empty_and_rejects_missing_ids() {
    let a = read_lenient(b"<ModMetaData><packageId>rs.a</packageId></ModMetaData>").about;
    let meta = a
        .into_meta(SourceId::game_mods(), "/mods/FolderName".into())
        .unwrap();
    assert_eq!(meta.name, "FolderName");
    let none = read_lenient(b"<ModMetaData/>").about;
    assert!(none.into_meta(SourceId::game_mods(), "/m".into()).is_err());
}

#[test]
fn missing_supported_versions_is_reported_and_an_empty_list_is_not_missing() {
    let r = read_lenient(b"<ModMetaData><packageId>rs.a</packageId></ModMetaData>");
    assert!(!r.about.has_supported_versions);
    assert!(codes(&r).contains(&"about.no-supported-versions"));
    let r = read_lenient(b"<ModMetaData><supportedVersions/></ModMetaData>");
    assert!(r.about.has_supported_versions);
    assert!(!codes(&r).contains(&"about.no-supported-versions"));
}

#[test]
fn a_comment_inside_a_text_field_drops_the_value_like_the_game() {
    let xml =
        "<ModMetaData><description>before<!-- c -->after</description><name>N</name></ModMetaData>";
    let r = read_lenient(xml.as_bytes());
    assert_eq!(r.about.description, "");
    assert_eq!(r.about.name, "N");
    assert!(codes(&r).contains(&"about.field-dropped"));
    let html = "<ModMetaData><description>a <b>bold</b></description></ModMetaData>";
    assert_eq!(read_lenient(html.as_bytes()).about.description, "");
}

#[test]
fn field_names_are_case_sensitive_unless_asked() {
    let xml = "<ModMetaData><PackageId>rs.a</PackageId><NAME>N</NAME></ModMetaData>";
    let strict = read_lenient(xml.as_bytes());
    assert_eq!(strict.about.package_id, "");
    assert!(codes(&strict).contains(&"about.tag-case"));
    let lenient = read_lenient_with(
        xml.as_bytes(),
        &AboutOptions {
            case_insensitive_fields: true,
        },
    );
    assert_eq!(lenient.about.package_id, "rs.a");
    assert_eq!(lenient.about.name, "N");
}

#[test]
fn duplicate_fields_keep_the_first_and_warn() {
    let xml = "<ModMetaData><name>One</name><name>Two</name></ModMetaData>";
    let r = read_lenient(xml.as_bytes());
    assert_eq!(r.about.name, "One");
    assert!(codes(&r).contains(&"about.duplicate-field"));
}

#[test]
fn broken_files_give_defaults_or_partial_data_without_failing() {
    let none = read_lenient(b"not xml at all");
    assert!(!none.parsed);
    assert_eq!(none.about.package_id, "");
    assert!(!none.warnings.is_empty());
    let partial = read_lenient(b"<ModMetaData><name>N</name><packageId>rs.a</packageId><descr");
    assert!(partial.parsed);
    assert_eq!(partial.about.package_id, "rs.a");
    let amp =
        read_lenient(b"<ModMetaData><description>R&D & more &nbsp;</description></ModMetaData>");
    assert!(amp.about.description.contains("R&D"));
}

#[test]
fn utf16_and_utf8_byte_order_marks_are_honoured() {
    let xml = "<ModMetaData><packageId>rs.a</packageId></ModMetaData>";
    let mut utf8 = vec![0xEF, 0xBB, 0xBF];
    utf8.extend_from_slice(xml.as_bytes());
    assert_eq!(read_lenient(&utf8).about.package_id, "rs.a");
    let mut utf16 = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        utf16.extend_from_slice(&u.to_le_bytes());
    }
    assert_eq!(read_lenient(&utf16).about.package_id, "rs.a");
    let mut be = vec![0xFE, 0xFF];
    for u in xml.encode_utf16() {
        be.extend_from_slice(&u.to_be_bytes());
    }
    assert_eq!(read_lenient(&be).about.package_id, "rs.a");
}

#[test]
fn edit_helpers_change_one_field_and_keep_the_rest() {
    let out = set_package_id(FULL, "rs.renamed").unwrap();
    assert_eq!(out.replace("rs.renamed", " RS.TestMod "), FULL);
    assert_eq!(read_lenient(out.as_bytes()).about.package_id, "rs.renamed");

    let out = set_name(FULL, "A & B").unwrap();
    assert_eq!(read_lenient(out.as_bytes()).about.name, "A & B");

    let out = set_description(FULL, "New <text>").unwrap();
    assert_eq!(read_lenient(out.as_bytes()).about.description, "New <text>");
}

#[test]
fn edit_helpers_create_missing_elements() {
    let src = "<ModMetaData>\n  <name>N</name>\n</ModMetaData>\n";
    let out = set_package_id(src, "rs.new").unwrap();
    assert_eq!(
        out,
        "<ModMetaData>\n  <name>N</name>\n  <packageId>rs.new</packageId>\n</ModMetaData>\n"
    );
    let out = set_supported_versions(src, &["1.5".to_owned(), "1.6".to_owned()]).unwrap();
    assert_eq!(
        read_lenient(out.as_bytes()).about.supported_versions,
        ["1.5", "1.6"]
    );
    assert!(out.contains("    <li>1.5</li>"));
}

#[test]
fn list_edits_replace_and_add_without_duplicates() {
    let out = add_load_after(FULL, "RS.Other").unwrap();
    assert_eq!(out, FULL, "an equal entry ignoring case is not added twice");
    let out = add_load_after(FULL, "ceteam.combatextended").unwrap();
    assert_eq!(
        read_lenient(out.as_bytes()).about.load_after,
        ["Ludeon.RimWorld", "rs.other", "ceteam.combatextended"]
    );
    let out = add_supported_version(FULL, "1.7").unwrap();
    assert!(
        read_lenient(out.as_bytes())
            .about
            .supported_versions
            .contains(&"1.7".to_owned())
    );
    let out = add_load_after("<ModMetaData/>", "rs.a").unwrap();
    assert_eq!(read_lenient(out.as_bytes()).about.load_after, ["rs.a"]);
}

#[test]
fn created_about_files_read_back() {
    let spec = AboutSpec {
        name: "RS New Mod".into(),
        author: "Ann".into(),
        package_id: "rs.newmod".into(),
        description: "Line 1\nLine 2 & more".into(),
        supported_versions: vec!["1.6".into()],
        load_after: vec!["ludeon.rimworld".into()],
        mod_dependencies: vec![rimstudio_core::mods::ModDependency {
            package_id: "rs.dep".into(),
            display_name: "Dep".into(),
            download_url: Some("https://example.invalid".into()),
            ..Default::default()
        }],
        ..AboutSpec::default()
    };
    let node = create_about(&spec);
    assert_eq!(node.tag, "ModMetaData");
    let text = render_about(&spec, &RenderOpts::default());
    let back = read_lenient(text.as_bytes());
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    assert_eq!(back.about.name, "RS New Mod");
    assert_eq!(back.about.description, "Line 1\nLine 2 & more");
    assert_eq!(back.about.mod_dependencies[0].package_id, "rs.dep");
    assert_eq!(back.about.supported_versions, ["1.6"]);
}
