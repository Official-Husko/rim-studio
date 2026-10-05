//! ModsConfig.xml, save headers and patch files.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Cursor;

use rimstudio_xml::mods_config::{ModsConfigData, create, read, write_active};
use rimstudio_xml::patches::{parse_patch_file, parse_patch_file_with};
use rimstudio_xml::render::RenderOpts;
use rimstudio_xml::save_meta::read_meta;
use rimstudio_xml::{ParseMode, XmlError};

const CONFIG: &str = "\u{FEFF}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<ModsConfigData>\r\n  <version>1.6.4633 rev1270</version>\r\n  <!-- keep me -->\r\n  <activeMods>\r\n    <li>ludeon.rimworld</li>\r\n    <li>rs.one_steam</li>\r\n  </activeMods>\r\n  <knownExpansions>\r\n    <li>ludeon.rimworld.royalty</li>\r\n  </knownExpansions>\r\n  <futureThing>1</futureThing>\r\n</ModsConfigData>\r\n";

#[test]
fn reads_version_active_and_known() {
    let r = read(CONFIG.as_bytes()).unwrap();
    assert_eq!(r.data.version.as_deref(), Some("1.6.4633 rev1270"));
    assert_eq!(r.data.active_mods, ["ludeon.rimworld", "rs.one_steam"]);
    assert_eq!(
        r.data.known_expansions.as_deref(),
        Some(&["ludeon.rimworld.royalty".to_owned()][..])
    );
}

#[test]
fn build_number_is_an_alias_of_version() {
    let xml =
        "<ModsConfigData><buildNumber>1.5.4104 rev120</buildNumber><activeMods/></ModsConfigData>";
    let r = read(xml.as_bytes()).unwrap();
    assert_eq!(r.data.version.as_deref(), Some("1.5.4104 rev120"));
    assert!(r.data.active_mods.is_empty());
    assert_eq!(r.data.known_expansions, None);
}

#[test]
fn write_active_replaces_only_the_list_and_keeps_unknown_parts() {
    let data = ModsConfigData {
        version: None,
        active_mods: vec![
            "ludeon.rimworld".into(),
            "rs.two".into(),
            "rs.one_steam".into(),
        ],
        known_expansions: None,
    };
    let out = write_active(CONFIG, &data).unwrap();
    assert!(out.starts_with('\u{FEFF}'));
    assert!(out.contains("<!-- keep me -->"));
    assert!(out.contains("<futureThing>1</futureThing>"));
    assert!(out.contains("    <li>rs.two</li>\r\n    <li>rs.one_steam</li>\r\n  </activeMods>"));
    let back = read(out.as_bytes()).unwrap().data;
    assert_eq!(back.active_mods, data.active_mods);
    assert_eq!(back.version.as_deref(), Some("1.6.4633 rev1270"));
    // Everything before activeMods and after it is byte identical.
    let head = &CONFIG[..CONFIG.find("<li>ludeon.rimworld</li>").unwrap()];
    assert!(out.starts_with(head));
    let tail = &CONFIG[CONFIG.find("</activeMods>").unwrap()..];
    assert!(out.ends_with(tail));
}

#[test]
fn write_active_sets_version_and_known_expansions_when_given() {
    let data = ModsConfigData {
        version: Some("1.6.4871 rev598".into()),
        active_mods: vec!["ludeon.rimworld".into()],
        known_expansions: Some(vec![
            "ludeon.rimworld.royalty".into(),
            "ludeon.rimworld.ideology".into(),
        ]),
    };
    let out = write_active(CONFIG, &data).unwrap();
    assert_eq!(read(out.as_bytes()).unwrap().data, data);
}

#[test]
fn write_active_adds_missing_elements() {
    let data = ModsConfigData {
        version: Some("1.6.4871 rev598".into()),
        active_mods: vec!["ludeon.rimworld".into()],
        known_expansions: Some(vec![]),
    };
    let out = write_active("<ModsConfigData>\n</ModsConfigData>\n", &data).unwrap();
    assert_eq!(read(out.as_bytes()).unwrap().data, data);
}

#[test]
fn created_config_reads_back() {
    let data = ModsConfigData {
        version: Some("1.6.4871 rev598".into()),
        active_mods: vec!["a.b".into(), "c.d".into()],
        known_expansions: Some(vec!["x.y".into()]),
    };
    let text = create(&data, &RenderOpts::default());
    assert_eq!(read(text.as_bytes()).unwrap().data, data);
}

const SAVE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<savegame>\n  <meta>\n    <gameVersion>1.6.4871 rev598</gameVersion>\n    <modIds><li>ludeon.rimworld</li><li>rs.one</li></modIds>\n    <modSteamIds><li>0</li><li>123</li></modSteamIds>\n    <modNames><li>Core</li><li>RS One &amp; Two</li></modNames>\n  </meta>\n  <game><huge>never read</huge></game>\n</savegame>\n";

#[test]
fn save_meta_reads_the_header() {
    let meta = read_meta(Cursor::new(SAVE.as_bytes())).unwrap();
    assert_eq!(meta.game_version.as_deref(), Some("1.6.4871 rev598"));
    assert_eq!(meta.mod_ids, ["ludeon.rimworld", "rs.one"]);
    assert_eq!(meta.mod_steam_ids, ["0", "123"]);
    assert_eq!(meta.mod_names, ["Core", "RS One & Two"]);
}

struct CountingReader<'a> {
    data: &'a [u8],
    pos: usize,
    taken: usize,
}

impl std::io::Read for CountingReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.data.len() - self.pos).min(64);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        self.taken += n;
        Ok(n)
    }
}

#[test]
fn save_meta_stops_reading_after_the_meta_element() {
    let mut big = SAVE.to_owned();
    big.push_str(&"<pad>0123456789</pad>".repeat(100_000));
    let mut r = CountingReader {
        data: big.as_bytes(),
        pos: 0,
        taken: 0,
    };
    let meta = read_meta(&mut r).unwrap();
    assert_eq!(meta.mod_ids.len(), 2);
    assert!(r.taken < 20_000, "read {} bytes", r.taken);
}

#[test]
fn save_meta_without_meta_is_an_error() {
    assert!(read_meta(Cursor::new(b"<savegame><game/></savegame>".as_slice())).is_err());
    assert!(read_meta(Cursor::new(b"".as_slice())).is_err());
}

#[test]
fn patch_files_return_the_operations() {
    let xml = "<?xml version=\"1.0\"?>\n<Patch>\n  <!-- c -->\n  <Operation Class=\"PatchOperationAdd\">\n    <xpath>Defs/ThingDef[defName=\"RS_X\"]</xpath>\n    <value><label>x</label></value>\n  </Operation>\n  <Operation Class=\"RS.Custom\"/>\n</Patch>";
    let ops = parse_patch_file(xml.as_bytes()).unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].attr("Class"), Some("PatchOperationAdd"));
    assert_eq!(
        ops[0].child_text("xpath"),
        Some("Defs/ThingDef[defName=\"RS_X\"]")
    );
    assert_eq!(ops[1].attr("Class"), Some("RS.Custom"));
}

#[test]
fn patch_file_with_another_root_warns_but_keeps_children() {
    let file = parse_patch_file_with(b"<Defs><Operation/></Defs>", ParseMode::Game).unwrap();
    assert_eq!(file.operations.len(), 1);
    assert_eq!(file.root_tag, "Defs");
    assert_eq!(file.diagnostics[0].code.as_str(), "xml.unexpected-root");
}

#[test]
fn patch_files_that_the_game_skips_are_errors() {
    assert!(matches!(
        parse_patch_file(b"<!-- only a comment -->"),
        Err(XmlError::NoRoot)
    ));
    assert!(parse_patch_file(b"<Patch><Operation></Patch>").is_err());
    assert!(parse_patch_file(b"").is_err());
}
