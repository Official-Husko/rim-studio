//! LoadFolders.xml reading, editing and creation.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_xml::load_folders::{
    add_entry, block_tag, create, ensure_block, gate_folder, read, remove_entry, to_node,
};
use rimstudio_xml::render::RenderOpts;

const SAMPLE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<loadFolders>\r\n\t<!-- main -->\r\n\t<v1.5>\r\n\t\t<li>/</li>\r\n\t\t<li>1.5</li>\r\n\t</v1.5>\r\n\t<V1.6>\r\n\t\t<li>\\</li>\r\n\t\t<li IfModActive=\" Ludeon.RimWorld.Royalty , ludeon.rimworld.ideology\" IfModNotActive=\"rs.x\">Royalty</li>\r\n\t\t<li IfModActiveAny=\"a\">Odd</li>\r\n\t</V1.6>\r\n</loadFolders>\r\n";

#[test]
fn reads_blocks_entries_and_conditions() {
    let r = read(SAMPLE.as_bytes()).unwrap();
    let spec = &r.spec;
    assert_eq!(spec.blocks.len(), 2);
    let b15 = spec.block("1.5").unwrap();
    assert_eq!(b15.entries[0], LoadEntry::root());
    assert_eq!(b15.entries[1].path, "1.5");
    let b16 = spec.block("1.6").unwrap();
    assert_eq!(b16.entries[0].path, "");
    assert_eq!(
        b16.entries[1].if_active,
        ["Ludeon.RimWorld.Royalty", "ludeon.rimworld.ideology"]
    );
    assert_eq!(b16.entries[1].if_not_active, ["rs.x"]);
    assert_eq!(b16.entries[2].ignored_attributes, ["IfModActiveAny"]);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "loadfolders.ignored-attribute")
    );
}

#[test]
fn repeated_blocks_merge_and_the_root_name_is_not_checked() {
    let xml = "<LoadFolders><v1.6><li>a</li></v1.6><V1.6><li>b</li></V1.6><default><li>/</li></default></LoadFolders>";
    let spec = read(xml.as_bytes()).unwrap().spec;
    assert_eq!(spec.blocks.len(), 2);
    assert_eq!(spec.block("1.6").unwrap().entries.len(), 2);
    assert!(spec.block("default").is_some());
}

#[test]
fn empty_and_whitespace_entries_mean_the_root_and_text_is_not_trimmed() {
    let xml = "<loadFolders><v1.6><li></li><li> </li><li/><li> Sub </li></v1.6></loadFolders>";
    let spec = read(xml.as_bytes()).unwrap().spec;
    let e = &spec.block("1.6").unwrap().entries;
    assert_eq!(e[0].path, "");
    assert_eq!(e[1].path, "");
    assert_eq!(e[2].path, "");
    assert_eq!(e[3].path, " Sub ");
}

#[test]
fn spec_round_trips_through_create_and_read() {
    let spec = read(SAMPLE.as_bytes()).unwrap().spec;
    let text = create(&spec, &RenderOpts::default());
    let back = read(text.as_bytes()).unwrap().spec;
    // Ignored attributes cannot be written, everything else survives.
    let mut expect = spec.clone();
    for b in &mut expect.blocks {
        for e in &mut b.entries {
            e.ignored_attributes.clear();
        }
    }
    assert_eq!(back, expect);
    assert!(text.contains("<v1.6>"));
}

#[test]
fn block_tags_get_a_letter_prefix_for_numeric_keys() {
    assert_eq!(block_tag("1.6"), "v1.6");
    assert_eq!(block_tag("default"), "default");
    let spec = LoadFoldersSpec::from_blocks([("1.6", vec![LoadEntry::root()])]);
    assert_eq!(to_node(&spec).elements().next().unwrap().tag, "v1.6");
}

#[test]
fn gate_folder_adds_a_gated_entry_in_the_existing_block_style() {
    let out = gate_folder(SAMPLE, "1.6", "CE", &["ceteam.combatextended"]).unwrap();
    assert!(out.contains("\t\t<li IfModActive=\"ceteam.combatextended\">CE</li>\r\n\t</V1.6>"));
    assert!(out.starts_with(&SAMPLE[..SAMPLE.find("</V1.6>").unwrap() - 3]));
    let spec = read(out.as_bytes()).unwrap().spec;
    let last = spec.block("1.6").unwrap().entries.last().unwrap();
    assert_eq!(last.path, "CE");
    assert_eq!(last.if_active, ["ceteam.combatextended"]);
}

#[test]
fn adding_an_equal_entry_twice_changes_nothing() {
    let once = gate_folder(SAMPLE, "v1.6", "CE", &["ceteam.combatextended"]).unwrap();
    let twice = gate_folder(&once, "1.6", "CE", &["CETeam.CombatExtended"]).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn adding_to_a_missing_block_appends_a_new_block() {
    let out = add_entry(SAMPLE, "1.7", &LoadEntry::dir("Seven")).unwrap();
    let spec = read(out.as_bytes()).unwrap().spec;
    assert_eq!(spec.block("1.7").unwrap().entries[0].path, "Seven");
    assert!(out.contains("<v1.7>"));
}

#[test]
fn ensure_block_copies_the_nearest_lower_block() {
    let src = "<loadFolders>\n  <v1.4><li>/</li><li>1.4</li></v1.4>\n  <v1.5><li>/</li><li>1.5</li></v1.5>\n</loadFolders>\n";
    let out = ensure_block(src, "1.6", &[LoadEntry::root()]).unwrap();
    let spec = read(out.as_bytes()).unwrap().spec;
    let paths: Vec<&str> = spec
        .block("1.6")
        .unwrap()
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["", "1.5"]);
    assert_eq!(ensure_block(&out, "1.6", &[]).unwrap(), out);
    let none = ensure_block("<loadFolders/>", "1.6", &[LoadEntry::root()]).unwrap();
    assert_eq!(
        read(none.as_bytes())
            .unwrap()
            .spec
            .block("1.6")
            .unwrap()
            .entries
            .len(),
        1
    );
    let from_default = ensure_block(
        "<loadFolders><default><li>Common</li></default></loadFolders>",
        "1.6",
        &[],
    )
    .unwrap();
    assert_eq!(
        read(from_default.as_bytes())
            .unwrap()
            .spec
            .block("1.6")
            .unwrap()
            .entries[0]
            .path,
        "Common"
    );
}

#[test]
fn remove_entry_removes_all_equal_entries() {
    let gated = gate_folder(SAMPLE, "1.6", "CE", &["ceteam.combatextended"]).unwrap();
    let out = remove_entry(
        &gated,
        "1.6",
        &LoadEntry::dir("CE").if_active(["ceteam.combatextended"]),
    )
    .unwrap();
    assert_eq!(out, SAMPLE);
}

#[test]
fn unparseable_files_are_an_error_the_caller_can_treat_as_empty() {
    assert!(read(b"nothing").is_err());
}
