//! Whole field edits of `About.xml` (`AboutEditor`) and positional edits of `LoadFolders.xml`
//! (`LoadFoldersEditor`): every edit keeps the other bytes, and random edit sequences keep the file
//! readable and in step with a plain model of what the edits mean.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::load_plan::LoadEntry;
use rimstudio_core::mods::ModDependency;
use rimstudio_xml::about::read_lenient;
use rimstudio_xml::about_edit::{AboutEditor, DependencyPatch};
use rimstudio_xml::load_folders;
use rimstudio_xml::load_folders_edit::{EntryPatch, LoadFoldersEditor};

const HAND: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- hand written -->\n<ModMetaData>\n\t<name>Hand Mod</name>\n\t<!-- who -->\n\t<author>Ann</author>\n\t<packageId>ann.hand</packageId>\n\t<customThing>keep me</customThing>\n\t<supportedVersions>\n\t\t<li>1.5</li>\n\t\t<!-- current -->\n\t\t<li>1.6</li>\n\t</supportedVersions>\n\t<loadAfter>\n\t\t<li>ludeon.rimworld</li>\n\t\t<li>ann.base</li>\n\t</loadAfter>\n\t<modDependencies>\n\t\t<li>\n\t\t\t<packageId>ann.base</packageId>\n\t\t\t<displayName>Base</displayName>\n\t\t\t<downloadUrl>https://example.invalid/base</downloadUrl>\n\t\t</li>\n\t\t<li>\n\t\t\t<packageId>ann.core</packageId>\n\t\t\t<displayName>Core</displayName>\n\t\t</li>\n\t</modDependencies>\n\t<description>Hello</description>\n</ModMetaData>\n";

/// The part of `new` that lies between the common prefix and the common suffix with `old`.
fn changed_span<'a>(old: &str, new: &'a str) -> (usize, &'a str) {
    let prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let max_suffix = old.len().min(new.len()) - prefix;
    let suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    (prefix, &new[prefix..new.len() - suffix])
}

fn editor(text: &str) -> AboutEditor {
    AboutEditor::new(text).unwrap()
}

#[test]
fn changing_a_field_touches_only_its_text() {
    let mut e = editor(HAND);
    e.set_text("name", "Renamed").unwrap();
    assert_eq!(e.text(), HAND.replace("Hand Mod", "Renamed"));
    let (at, _) = changed_span(HAND, e.text());
    assert!(HAND[..at].ends_with("<name>") || HAND[..at].contains("<name>"));
    assert!(e.text().contains("<!-- who -->"));
    assert!(e.text().contains("<customThing>keep me</customThing>"));
}

#[test]
fn setting_the_same_value_changes_no_byte() {
    let mut e = editor(HAND);
    e.set_text("name", "Hand Mod").unwrap();
    e.set_list("supportedVersions", &["1.5".into(), "1.6".into()])
        .unwrap();
    e.list_add("loadAfter", "ANN.BASE", None).unwrap();
    assert_eq!(e.text(), HAND);
}

#[test]
fn a_missing_field_is_inserted_at_its_conventional_place() {
    let mut e = editor(HAND);
    e.set_text("url", "https://example.invalid/x").unwrap();
    e.set_text("modVersion", "0.2").unwrap();
    let t = e.text();
    let pos = |needle: &str| t.find(needle).unwrap();
    assert!(pos("<packageId>") < pos("<url>"));
    assert!(pos("<url>") < pos("<modVersion>"));
    assert!(pos("<modVersion>") < pos("<supportedVersions>"));
    // the new lines use the file's tabs
    assert!(t.contains("\n\t<url>https://example.invalid/x</url>\n"));
    e.set_text("modIconPath", "Icon/Mod").unwrap();
    let t = e.text();
    assert!(t.find("<modIconPath>").unwrap() < t.find("<description>").unwrap());
    assert!(t.find("<modDependencies>").unwrap() < t.find("<modIconPath>").unwrap());
}

#[test]
fn clearing_a_field_removes_its_line_and_nothing_else() {
    let mut e = editor(HAND);
    e.clear("author").unwrap();
    assert_eq!(e.text(), HAND.replace("\t<author>Ann</author>\n", ""));
    e.set_text("name", "   ").unwrap();
    assert!(!e.text().contains("<name>"));
}

#[test]
fn clearing_removes_every_copy_so_a_duplicate_cannot_take_over() {
    let text = "<ModMetaData><url>a</url><url>b</url></ModMetaData>";
    let mut e = editor(text);
    e.clear("url").unwrap();
    assert_eq!(e.text(), "<ModMetaData></ModMetaData>");
}

#[test]
fn a_list_keeps_its_comments_when_an_entry_is_added_or_removed() {
    let mut e = editor(HAND);
    e.list_add("supportedVersions", "1.7", None).unwrap();
    assert!(e.text().contains("<!-- current -->"));
    assert!(
        e.text()
            .contains("\t\t<li>1.7</li>\n\t</supportedVersions>")
    );
    e.list_remove("supportedVersions", "1.5").unwrap();
    assert!(e.text().contains("<!-- current -->"));
    assert!(!e.text().contains("<li>1.5</li>"));
    let read = read_lenient(e.text().as_bytes());
    assert_eq!(read.about.supported_versions, vec!["1.6", "1.7"]);
}

#[test]
fn the_last_entry_removes_the_list_and_an_empty_list_is_left_alone() {
    let mut e = editor(HAND);
    e.list_remove("loadAfter", "ludeon.rimworld").unwrap();
    e.list_remove("loadAfter", "ann.base").unwrap();
    assert!(!e.text().contains("loadAfter"));
    let already_empty = "<ModMetaData>\n  <loadBefore />\n</ModMetaData>\n";
    let mut e = editor(already_empty);
    e.set_list("loadBefore", &[]).unwrap();
    assert_eq!(e.text(), already_empty);
    e.set_list("loadAfter", &[]).unwrap();
    assert_eq!(e.text(), already_empty);
}

#[test]
fn entries_move_without_touching_their_neighbours() {
    let mut e = editor(HAND);
    e.list_add("loadAfter", "ann.third", None).unwrap();
    e.list_move("loadAfter", "ann.third", 0).unwrap();
    let read = read_lenient(e.text().as_bytes());
    assert_eq!(
        read.about.load_after,
        vec!["ann.third", "ludeon.rimworld", "ann.base"]
    );
    e.list_move("loadAfter", "ann.third", 99).unwrap();
    let read = read_lenient(e.text().as_bytes());
    assert_eq!(
        read.about.load_after,
        vec!["ludeon.rimworld", "ann.base", "ann.third"]
    );
    assert!(e.list_move("loadAfter", "nobody", 0).is_err());
}

#[test]
fn dependencies_are_added_changed_moved_and_removed_in_place() {
    let mut e = editor(HAND);
    let dep = ModDependency {
        package_id: "ann.extra".into(),
        display_name: "Extra".into(),
        steam_workshop_url: Some("https://steamcommunity.com/sharedfiles/filedetails/?id=1".into()),
        ..ModDependency::default()
    };
    e.dependency_add(&dep, Some(0)).unwrap();
    e.dependency_add(&dep, None).unwrap();
    let read = read_lenient(e.text().as_bytes());
    let ids: Vec<&str> = read
        .about
        .mod_dependencies
        .iter()
        .map(|d| d.package_id.as_str())
        .collect();
    assert_eq!(ids, vec!["ann.extra", "ann.base", "ann.core"]);
    e.dependency_update(
        "ann.core",
        &DependencyPatch {
            display_name: Some("Core Mod".into()),
            download_url: Some("https://example.invalid/core".into()),
            ..DependencyPatch::default()
        },
    )
    .unwrap();
    e.dependency_update(
        "ann.base",
        &DependencyPatch {
            download_url: Some(String::new()),
            ..DependencyPatch::default()
        },
    )
    .unwrap();
    e.dependency_move("ann.core", 0).unwrap();
    e.dependency_remove("ann.extra").unwrap();
    let read = read_lenient(e.text().as_bytes());
    let deps = &read.about.mod_dependencies;
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].package_id, "ann.core");
    assert_eq!(deps[0].display_name, "Core Mod");
    assert_eq!(
        deps[0].download_url.as_deref(),
        Some("https://example.invalid/core")
    );
    assert_eq!(deps[1].package_id, "ann.base");
    assert_eq!(deps[1].download_url, None);
    e.dependency_remove("ann.core").unwrap();
    e.dependency_remove("ann.base").unwrap();
    assert!(!e.text().contains("modDependencies"));
}

#[test]
fn crlf_bom_and_odd_white_space_survive_an_edit() {
    let crlf = format!(
        "\u{feff}{}",
        "<ModMetaData>\r\n    <name>A</name>\r\n    <packageId>a.b</packageId>\r\n</ModMetaData>\r\n"
    );
    let mut e = editor(&crlf);
    e.set_text("url", "https://example.invalid").unwrap();
    e.list_add("supportedVersions", "1.6", None).unwrap();
    let t = e.text();
    assert!(t.starts_with('\u{feff}'));
    assert!(
        !t.replace("\r\n", "").contains('\n'),
        "a bare newline was written"
    );
    assert!(t.contains("\r\n    <url>https://example.invalid</url>"));
    let one_line = "<ModMetaData><name>A</name><packageId>a.b</packageId></ModMetaData>";
    let mut e = editor(one_line);
    e.set_text("author", "Zed").unwrap();
    assert_eq!(
        e.text(),
        "<ModMetaData><name>A</name><author>Zed</author><packageId>a.b</packageId></ModMetaData>"
    );
}

#[test]
fn special_characters_are_escaped_and_read_back() {
    let mut e = editor(HAND);
    e.set_text("description", "a < b & c\nsecond line").unwrap();
    let read = read_lenient(e.text().as_bytes());
    assert_eq!(read.about.description, "a < b & c\nsecond line");
}

#[test]
fn by_version_blocks_are_set_and_removed() {
    let mut e = editor(HAND);
    e.by_version_set_list("loadAfterByVersion", "1.6", &["ann.six".into()])
        .unwrap();
    e.by_version_set_description("1.6", Some("six text"))
        .unwrap();
    let read = read_lenient(e.text().as_bytes());
    assert_eq!(read.about.by_version["1.6"].load_after, vec!["ann.six"]);
    assert_eq!(read.about.descriptions_by_version["1.6"], "six text");
    e.by_version_set_list("loadAfterByVersion", "v1.6", &[])
        .unwrap();
    e.by_version_set_description("1.6", None).unwrap();
    assert_eq!(e.text(), HAND);
}

#[test]
fn a_broken_document_is_refused() {
    assert!(AboutEditor::new("<ModMetaData><name>x</ModMetaData>").is_err());
    assert!(AboutEditor::new("").is_err());
}

// ------------------------------------------------------------------------------ properties

#[derive(Debug, Clone)]
enum Op {
    Set(&'static str, String),
    Clear(&'static str),
    ListAdd(&'static str, String),
    ListRemove(&'static str, String),
    DepAdd(String),
    DepRemove(String),
}

fn word() -> impl Strategy<Value = String> {
    "[a-z]{1,5}\\.[a-z]{1,5}"
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        ("[A-Za-z &<>]{1,12}").prop_map(|v| Op::Set("name", v)),
        ("[A-Za-z]{1,8}").prop_map(|v| Op::Set("author", v)),
        ("[a-z]{1,8}").prop_map(|v| Op::Set("url", v)),
        Just(Op::Clear("url")),
        Just(Op::Clear("author")),
        word().prop_map(|v| Op::ListAdd("loadAfter", v)),
        word().prop_map(|v| Op::ListRemove("loadAfter", v)),
        word().prop_map(|v| Op::ListAdd("supportedVersions", v)),
        word().prop_map(|v| Op::ListRemove("supportedVersions", v)),
        word().prop_map(Op::DepAdd),
        word().prop_map(Op::DepRemove),
    ]
}

fn styles() -> Vec<String> {
    let tabs = HAND.to_owned();
    let crlf = HAND.replace('\n', "\r\n");
    let bom = format!("\u{feff}{HAND}");
    let spaces = HAND.replace('\t', "    ");
    let tight = "<ModMetaData><name>T</name><packageId>t.t</packageId><!-- c --><zzz a=\"1\"/></ModMetaData>"
        .to_owned();
    vec![tabs, crlf, bom, spaces, tight]
}

fn comments_of(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("<!--") {
        let Some(j) = rest[i..].find("-->") else {
            break;
        };
        out.push(&rest[i..i + j + 3]);
        rest = &rest[i + j + 3..];
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn random_edit_sequences_keep_the_file_readable_and_in_step_with_a_model(
        style in 0usize..5,
        ops in proptest::collection::vec(op(), 1..12),
    ) {
        let original = styles()[style].clone();
        let before = read_lenient(original.as_bytes()).about;
        let mut model = before.clone();
        let mut e = AboutEditor::new(original.clone()).unwrap();
        for o in &ops {
            match o {
                Op::Set(field, v) => {
                    e.set_text(field, v).unwrap();
                    let v = v.trim().to_owned();
                    let slot = match *field {
                        "name" => { model.name = v; continue; }
                        "author" => &mut model.author,
                        _ => &mut model.url,
                    };
                    *slot = Some(v);
                }
                Op::Clear(field) => {
                    e.clear(field).unwrap();
                    match *field {
                        "author" => model.author = None,
                        _ => model.url = None,
                    }
                }
                Op::ListAdd(field, v) => {
                    e.list_add(field, v, None).unwrap();
                    let list = if *field == "loadAfter" { &mut model.load_after } else { &mut model.supported_versions };
                    if !list.iter().any(|x| x.eq_ignore_ascii_case(v)) { list.push(v.clone()); }
                }
                Op::ListRemove(field, v) => {
                    e.list_remove(field, v).unwrap();
                    let list = if *field == "loadAfter" { &mut model.load_after } else { &mut model.supported_versions };
                    list.retain(|x| !x.eq_ignore_ascii_case(v));
                }
                Op::DepAdd(id) => {
                    let dep = ModDependency { package_id: id.clone(), display_name: id.clone(), ..ModDependency::default() };
                    e.dependency_add(&dep, None).unwrap();
                    if !model.mod_dependencies.iter().any(|d| d.package_id.eq_ignore_ascii_case(id)) {
                        model.mod_dependencies.push(dep);
                    }
                }
                Op::DepRemove(id) => {
                    e.dependency_remove(id).unwrap();
                    model.mod_dependencies.retain(|d| !d.package_id.eq_ignore_ascii_case(id));
                }
            }
        }
        let after = read_lenient(e.text().as_bytes());
        prop_assert!(after.parsed);
        let a = after.about;
        prop_assert_eq!(&a.name, &model.name);
        prop_assert_eq!(&a.author, &model.author);
        prop_assert_eq!(&a.url, &model.url);
        prop_assert_eq!(&a.load_after, &model.load_after);
        prop_assert_eq!(&a.supported_versions, &model.supported_versions);
        prop_assert_eq!(&a.mod_dependencies, &model.mod_dependencies);
        // what no operation touches
        prop_assert_eq!(&a.package_id, &before.package_id);
        prop_assert_eq!(&a.unknown_tags, &before.unknown_tags);
        prop_assert_eq!(&a.description, &before.description);
        // comments outside the lists the sequence rewrote are never lost
        let kept = comments_of(&original);
        let now = comments_of(e.text());
        for c in kept {
            if c == "<!-- current -->" { continue; }
            prop_assert!(now.contains(&c), "comment lost: {}", c);
        }
        // the encoding frame is kept
        prop_assert_eq!(e.text().starts_with('\u{feff}'), original.starts_with('\u{feff}'));
        if original.contains("\r\n") {
            prop_assert!(!e.text().replace("\r\n", "").contains('\n'));
        }
    }

    #[test]
    fn an_edit_that_sets_what_is_already_there_is_byte_identical(style in 0usize..5) {
        let original = styles()[style].clone();
        let about = read_lenient(original.as_bytes()).about;
        let mut e = AboutEditor::new(original.clone()).unwrap();
        e.set_text("name", &about.name).unwrap();
        e.set_text("packageId", &about.package_id).unwrap();
        e.set_list("supportedVersions", &about.supported_versions).unwrap();
        e.set_list("loadAfter", &about.load_after).unwrap();
        e.set_list("incompatibleWith", &about.incompatible_with).unwrap();
        prop_assert_eq!(e.text(), original.as_str());
    }
}

// ------------------------------------------------------------------------- LoadFolders.xml

const LF: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<loadFolders>\n  <!-- shared -->\n  <v1.5>\n    <li>/</li>\n    <li>1.5</li>\n  </v1.5>\n  <v1.6>\n    <li>/</li>\n    <li>1.6</li>\n    <li IfModActiveAny=\"ceteam.combatextended\">Compat/CE</li>\n  </v1.6>\n</loadFolders>\n";

fn lf_entries(text: &str) -> Vec<(String, Vec<String>)> {
    let read = load_folders::read(text.as_bytes()).unwrap();
    read.spec
        .blocks
        .iter()
        .map(|b| {
            (
                b.key.clone(),
                b.entries.iter().map(|e| e.path.clone()).collect(),
            )
        })
        .collect()
}

#[test]
fn entries_are_added_moved_changed_and_removed_by_position() {
    let mut e = LoadFoldersEditor::new(LF).unwrap();
    e.add_entry(1, &LoadEntry::dir("Extra"), Some(1)).unwrap();
    assert_eq!(
        lf_entries(e.text())[1].1,
        vec!["", "Extra", "1.6", "Compat/CE"]
    );
    e.move_entry(1, 1, 3).unwrap();
    assert_eq!(
        lf_entries(e.text())[1].1,
        vec!["", "1.6", "Compat/CE", "Extra"]
    );
    e.set_entry(
        1,
        2,
        &EntryPatch {
            if_active: Some(vec!["ceteam.combatextended".into()]),
            drop_ignored_attributes: true,
            ..EntryPatch::default()
        },
    )
    .unwrap();
    assert!(
        e.text()
            .contains("<li IfModActive=\"ceteam.combatextended\">Compat/CE</li>")
    );
    assert!(!e.text().contains("IfModActiveAny"));
    e.remove_entry(1, 3).unwrap();
    assert_eq!(lf_entries(e.text())[1].1, vec!["", "1.6", "Compat/CE"]);
    assert!(e.text().contains("<!-- shared -->"));
    assert!(e.move_entry(1, 9, 0).is_err());
}

#[test]
fn blocks_are_added_and_removed_and_the_rest_is_untouched() {
    let mut e = LoadFoldersEditor::new(LF).unwrap();
    e.add_block("1.4", &[LoadEntry::root(), LoadEntry::dir("1.4")], Some(0))
        .unwrap();
    let blocks = lf_entries(e.text());
    assert_eq!(blocks[0].0, "1.4");
    assert_eq!(blocks[0].1, vec!["", "1.4"]);
    e.remove_block(0).unwrap();
    assert_eq!(e.text(), LF);
    e.remove_block(1).unwrap();
    assert_eq!(lf_entries(e.text()).len(), 1);
    assert!(e.remove_block(5).is_err());
}

#[test]
fn conditions_are_set_and_cleared() {
    let mut e = LoadFoldersEditor::new(LF).unwrap();
    e.set_entry(
        0,
        1,
        &EntryPatch {
            if_not_active: Some(vec!["a.b".into(), " c.d ".into()]),
            ..EntryPatch::default()
        },
    )
    .unwrap();
    assert!(e.text().contains("<li IfModNotActive=\"a.b,c.d\">1.5</li>"));
    e.set_entry(
        0,
        1,
        &EntryPatch {
            if_not_active: Some(Vec::new()),
            ..EntryPatch::default()
        },
    )
    .unwrap();
    assert_eq!(e.text(), LF);
}

#[derive(Debug, Clone)]
enum LfOp {
    AddBlock(u8),
    RemoveBlock(usize),
    AddEntry(usize, u8),
    RemoveEntry(usize, usize),
    MoveEntry(usize, usize, usize),
}

fn lf_op() -> impl Strategy<Value = LfOp> {
    prop_oneof![
        (0u8..6).prop_map(LfOp::AddBlock),
        (0usize..4).prop_map(LfOp::RemoveBlock),
        (0usize..4, 0u8..9).prop_map(|(b, e)| LfOp::AddEntry(b, e)),
        (0usize..4, 0usize..5).prop_map(|(b, e)| LfOp::RemoveEntry(b, e)),
        (0usize..4, 0usize..5, 0usize..5).prop_map(|(b, f, t)| LfOp::MoveEntry(b, f, t)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn random_load_folders_edits_match_a_model_and_keep_the_comments(
        crlf in any::<bool>(),
        ops in proptest::collection::vec(lf_op(), 1..14),
    ) {
        let mut text = LF.to_owned();
        if crlf {
            text = text.replace('\n', "\r\n");
        }
        let mut model = lf_entries(&text);
        let mut e = LoadFoldersEditor::new(text.clone()).unwrap();
        for op in &ops {
            match op {
                LfOp::AddBlock(n) => {
                    let key = format!("1.{}", 10 + n);
                    if model.iter().any(|(k, _)| *k == key) { continue; }
                    e.add_block(&key, &[LoadEntry::dir("x")], None).unwrap();
                    model.push((key, vec!["x".to_owned()]));
                }
                LfOp::RemoveBlock(b) => {
                    if *b >= model.len() { continue; }
                    e.remove_block(*b).unwrap();
                    model.remove(*b);
                }
                LfOp::AddEntry(b, n) => {
                    if *b >= model.len() { continue; }
                    let name = format!("f{n}");
                    e.add_entry(*b, &LoadEntry::dir(name.clone()), None).unwrap();
                    model[*b].1.push(name);
                }
                LfOp::RemoveEntry(b, i) => {
                    if *b >= model.len() || *i >= model[*b].1.len() { continue; }
                    e.remove_entry(*b, *i).unwrap();
                    model[*b].1.remove(*i);
                }
                LfOp::MoveEntry(b, from, to) => {
                    if *b >= model.len() || *from >= model[*b].1.len() { continue; }
                    e.move_entry(*b, *from, *to).unwrap();
                    let item = model[*b].1.remove(*from);
                    let at = (*to).min(model[*b].1.len());
                    model[*b].1.insert(at, item);
                }
            }
        }
        // block keys of the model that the game merges would hide a repeated block; the pool avoids repeats
        let after = lf_entries(e.text());
        prop_assert_eq!(after, model);
        prop_assert!(e.text().contains("<!-- shared -->"));
        if crlf {
            prop_assert!(!e.text().replace("\r\n", "").contains('\n'));
        }
    }
}

#[test]
fn a_comment_only_element_keeps_its_comment_when_the_first_child_is_added() {
    let text = "<loadFolders>\n  <!-- nothing yet -->\n</loadFolders>\n";
    let mut e = LoadFoldersEditor::new(text).unwrap();
    e.add_block("1.6", &[LoadEntry::root()], None).unwrap();
    assert_eq!(
        e.text(),
        "<loadFolders>\n  <!-- nothing yet -->\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n"
    );
    let mut a = AboutEditor::new("<ModMetaData><!-- todo --></ModMetaData>").unwrap();
    a.set_text("name", "A").unwrap();
    assert_eq!(
        a.text(),
        "<ModMetaData><!-- todo --><name>A</name></ModMetaData>"
    );
}
