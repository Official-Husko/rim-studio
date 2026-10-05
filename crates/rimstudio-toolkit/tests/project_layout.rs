//! The layout commands of the project tool: the annotated tree, the layout check, scaffolding missing folders
//! and reading a file. Fixtures are fictional mods in temporary folders; the hostile ones hold links, odd
//! XML and long paths.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project::{
    LayoutFixKindDto, LayoutIssueDto, LayoutProfileDto, NodeRoleDto, ProjectLayoutCheckRequest,
    ProjectReadFileRequest, ProjectScaffoldMissingRequest, ProjectTreeDto, ProjectTreeRequest,
    TreeNodeDto,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::project::{
    create, layout_check, open_project, read::read_file, scaffold_missing::scaffold_missing, tree,
};
use rimstudio_toolkit::shared::env::ProjectEnv;
use rimstudio_workspace::scaffold::ScaffoldSpec;

struct World {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    env: ProjectEnv,
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
    }
}

const ABOUT: &str = "<ModMetaData><name>RS Fixture</name><packageId>rs.fixture</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";

fn put(root: &Utf8PathBuf, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(path.as_std_path(), text).unwrap();
}

fn rifle(name: &str, tex: &str) -> String {
    format!(
        "<Defs><ThingDef ParentName=\"BaseBullet\"><defName>B_{name}</defName><projectile/></ThingDef>\
         <ThingDef ParentName=\"BaseGun\"><defName>{name}</defName><techLevel>Industrial</techLevel>\
         <graphicData><texPath>{tex}</texPath></graphicData>\
         <verbs><li><verbClass>Verb_Shoot</verbClass></li></verbs></ThingDef></Defs>"
    )
}

fn blade(name: &str) -> String {
    format!(
        "<Defs><ThingDef ParentName=\"BaseMeleeWeapon\"><defName>{name}</defName>\
         <techLevel>Medieval</techLevel><tools><li/></tools></ThingDef></Defs>"
    )
}

fn mod_at(w: &World, name: &str, files: &[(&str, &str)]) -> (Utf8PathBuf, String) {
    let root = w.base.join(name);
    put(&root, "About/About.xml", ABOUT);
    for (rel, text) in files {
        put(&root, rel, text);
    }
    let id = open_project(&w.env, &root)
        .unwrap()
        .record
        .id
        .as_str()
        .to_owned();
    (root, id)
}

fn tree_of(w: &World, id: &str) -> ProjectTreeDto {
    tree(
        &w.env,
        &ProjectTreeRequest {
            project_id: id.to_owned(),
            max_nodes: None,
        },
    )
    .unwrap()
}

fn issues_of(w: &World, id: &str) -> Vec<LayoutIssueDto> {
    layout_check(
        &w.env,
        &ProjectLayoutCheckRequest {
            project_id: id.to_owned(),
        },
    )
    .unwrap()
    .issues
}

fn codes(issues: &[LayoutIssueDto]) -> Vec<&str> {
    issues.iter().map(|i| i.code.as_str()).collect()
}

fn find<'a>(node: &'a TreeNodeDto, path: &str) -> Option<&'a TreeNodeDto> {
    if node.path == path {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, path))
}

#[test]
fn a_fresh_scaffold_follows_the_layout_with_no_issue() {
    let w = world();
    let target = w.base.join("RS_Fresh");
    let spec = ScaffoldSpec::new(target.clone(), "RS Fresh", "rs.fresh");
    let id = create(&w.env, &spec, &[])
        .unwrap()
        .project
        .record
        .id
        .as_str()
        .to_owned();
    let t = tree_of(&w, &id);
    assert_eq!(t.profile, LayoutProfileDto::Rimstudio);
    assert_eq!(t.weapons_folder, "Defs/ThingDefs_Misc/Weapons");
    assert!(t.issues.is_empty(), "{:?}", t.issues);
    assert!(!t.truncated);
    let weapons = find(&t.root, "Defs/ThingDefs_Misc/Weapons").unwrap();
    assert_eq!(weapons.role, NodeRoleDto::DefsWeapons);
    assert_eq!(
        find(&t.root, "Defs/SoundDefs").unwrap().role,
        NodeRoleDto::DefsSounds
    );
    assert_eq!(
        find(&t.root, "About/About.xml").unwrap().role,
        NodeRoleDto::About
    );
    assert_eq!(t.counts.files, 1);
    assert!(t.counts.folders >= 10);
}

#[test]
fn the_games_own_style_is_recognised_and_its_ce_patch_is_flagged() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_CoreStyle",
        &[
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml",
                &rifle("RS_Gun", "Things/Item/Equipment/WeaponRanged/rs_gun"),
            ),
            (
                "Patches/ce_patch.xml",
                "<Patch><Operation Class=\"CombatExtended.PatchOperationFindMod\"/></Patch>",
            ),
            (
                "Textures/Things/Item/Equipment/WeaponRanged/rs_gun.dds",
                "x",
            ),
        ],
    );
    let t = tree_of(&w, &id);
    assert_eq!(t.profile, LayoutProfileDto::CoreStyle);
    assert_eq!(t.counts.weapon_defs, 1);
    assert_eq!(t.counts.projectile_defs, 1);
    assert_eq!(t.counts.textures, 1);
    let found = codes(&t.issues);
    assert!(found.contains(&"layout.ce-outside-gate"), "{found:?}");
    // the texture exists, the standard sound folder does not
    assert!(!found.contains(&"layout.texture-missing"));
    assert!(found.contains(&"layout.missing-folder"));
    let gate = t
        .issues
        .iter()
        .find(|i| i.code == "layout.ce-outside-gate")
        .unwrap();
    assert!(!gate.fix.automatic);
    assert_eq!(gate.fix.kind, LayoutFixKindDto::MoveFile);
    assert_eq!(
        gate.fix.targets,
        ["Compat/CombatExtended/Patches/ce_patch.xml"]
    );
    assert_eq!(find(&t.root, "Patches").unwrap().issues, 1);
}

#[test]
fn a_common_folder_with_loosely_named_files_is_a_flat_project() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_Loose",
        &[
            (
                "Common/Defs/ThingsDef_Misc/Weapons/Weapons_Ranged.xml",
                &rifle("RS_Gun", "Things/Weapon/Ranged/Rifle/RS_Gun"),
            ),
            ("Common/Defs/SoundDef/Weapon_Sounds.xml", "<Defs/>"),
            ("Patches/Patches_For_CE_And_Others/a.xml", "<Patch/>"),
        ],
    );
    let t = tree_of(&w, &id);
    assert_eq!(t.profile, LayoutProfileDto::Flat);
    assert_eq!(t.content_folder.as_deref(), Some("Common"));
    assert_eq!(t.weapons_folder, "Common/Defs/ThingsDef_Misc/Weapons");
    assert_eq!(
        find(&t.root, "Common").unwrap().role,
        NodeRoleDto::ContentRoot
    );
    assert_eq!(
        find(
            &t.root,
            "Common/Defs/ThingsDef_Misc/Weapons/Weapons_Ranged.xml"
        )
        .unwrap()
        .role,
        NodeRoleDto::DefsWeapons
    );
}

#[test]
fn an_older_ce_folder_is_kept_and_reported_as_a_note() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_Legacy",
        &[
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Gun.xml",
                &rifle("RS_Gun", "T/x"),
            ),
            (
                "CE/Patches/RS_Weapons_Ranged.xml",
                "<Patch><Operation Class=\"CombatExtended.X\"/></Patch>",
            ),
            (
                "LoadFolders.xml",
                "<loadFolders><v1.6><li>/</li><li IfModActive=\"ceteam.combatextended\">CE</li></v1.6></loadFolders>",
            ),
        ],
    );
    let t = tree_of(&w, &id);
    assert_eq!(t.profile, LayoutProfileDto::Rimstudio);
    assert_eq!(t.ce_folder, "CE");
    assert!(t.ce_folder_exists && t.ce_folder_legacy && t.ce_gated);
    assert_eq!(
        find(&t.root, "CE/Patches").unwrap().role,
        NodeRoleDto::CeCompat
    );
    let found = codes(&t.issues);
    assert!(found.contains(&"layout.ce-legacy-folder"));
    assert!(!found.contains(&"layout.ce-outside-gate"));
    assert!(!found.contains(&"layout.ce-folder-ungated"));
}

#[test]
fn an_ungated_ce_folder_and_a_gate_without_a_folder_are_warnings() {
    let w = world();
    let (_, ungated) = mod_at(
        &w,
        "RS_Ungated",
        &[("Compat/CombatExtended/Patches/a.xml", "<Patch/>")],
    );
    assert!(codes(&issues_of(&w, &ungated)).contains(&"layout.ce-folder-ungated"));
    let (_, dangling) = mod_at(
        &w,
        "RS_Dangling",
        &[(
            "LoadFolders.xml",
            "<loadFolders><v1.6><li>/</li><li IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li></v1.6></loadFolders>",
        )],
    );
    let found = issues_of(&w, &dangling);
    assert!(codes(&found).contains(&"layout.load-folders-missing-folder"));
}

#[test]
fn a_required_ce_mod_makes_ce_classes_everywhere_fine() {
    let w = world();
    let root = w.base.join("RS_NeedsCe");
    put(
        &root,
        "About/About.xml",
        "<ModMetaData><name>N</name><packageId>rs.needsce</packageId><supportedVersions><li>1.6</li></supportedVersions><modDependencies><li><packageId>CETeam.CombatExtended</packageId><displayName>CE</displayName></li></modDependencies></ModMetaData>",
    );
    put(
        &root,
        "Patches/a.xml",
        "<Patch><Operation Class=\"CombatExtended.X\"/></Patch>",
    );
    let id = open_project(&w.env, &root)
        .unwrap()
        .record
        .id
        .as_str()
        .to_owned();
    assert!(!codes(&issues_of(&w, &id)).contains(&"layout.ce-outside-gate"));
}

#[test]
fn misplaced_files_and_wrong_categories_are_found_and_never_moved() {
    let w = world();
    let (root, id) = mod_at(
        &w,
        "RS_Misplaced",
        &[
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Gun.xml",
                &rifle("RS_Gun", "T/a"),
            ),
            (
                "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Blade.xml",
                &blade("RS_Blade"),
            ),
            ("Defs/Misc/Extra.xml", &rifle("RS_Extra", "T/b")),
            ("Defs/Misc/Patchy.xml", "<Patch/>"),
            ("Patches/Defsy.xml", "<Defs/>"),
            ("Defs/Broken.xml", "<Defs><ThingDef></Defs>"),
        ],
    );
    let found = issues_of(&w, &id);
    let by =
        |code: &str| -> Vec<&LayoutIssueDto> { found.iter().filter(|i| i.code == code).collect() };
    let mis = by("layout.weapon-misplaced");
    assert_eq!(mis.len(), 1);
    assert_eq!(mis[0].path, "Defs/Misc/Extra.xml");
    assert_eq!(
        mis[0].fix.targets,
        ["Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Extra.xml"]
    );
    let cat = by("layout.def-wrong-category");
    assert_eq!(cat.len(), 1);
    assert_eq!(
        cat[0].path,
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Blade.xml"
    );
    assert_eq!(
        cat[0].fix.targets,
        ["Defs/ThingDefs_Misc/Weapons/MeleeMedieval/RS_Blade.xml"]
    );
    assert_eq!(by("layout.wrong-root").len(), 2);
    assert_eq!(by("layout.unparsable-file").len(), 1);
    assert!(found.iter().all(|i| i.code != "layout.missing-about"));
    // read only: the files are where they were
    assert!(root.join("Defs/Misc/Extra.xml").exists());
    assert!(
        root.join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Blade.xml")
            .exists()
    );
    // most serious first
    let sev: Vec<_> = found.iter().map(|i| i.severity).collect();
    assert!(sev.windows(2).all(|p| p[0] <= p[1]), "{sev:?}");
}

#[test]
fn a_folder_with_the_wrong_letter_case_is_a_warning() {
    let w = world();
    let (_, id) = mod_at(&w, "RS_Case", &[("defs/A.xml", "<Defs/>")]);
    let found = issues_of(&w, &id);
    assert!(codes(&found).contains(&"layout.folder-case"), "{found:?}");
}

#[test]
fn scaffolding_missing_folders_creates_folders_only_and_a_second_run_does_nothing() {
    let w = world();
    let (root, id) = mod_at(
        &w,
        "RS_Partial",
        &[(
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml",
            &rifle("RS_Gun", "T/c"),
        )],
    );
    let before =
        std::fs::read(root.join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml")).unwrap();
    let req = ProjectScaffoldMissingRequest {
        project_id: id.clone(),
        dry_run: true,
    };
    let dry = scaffold_missing(&w.env, &req, &[]).unwrap();
    assert!(
        dry.folders.contains(&"Sounds".to_owned()) && dry.folders.contains(&"Textures".to_owned())
    );
    assert!(!root.join("Sounds").exists());
    let done = scaffold_missing(
        &w.env,
        &ProjectScaffoldMissingRequest {
            project_id: id.clone(),
            dry_run: false,
        },
        &[],
    )
    .unwrap();
    assert_eq!(done.folders, dry.folders);
    assert!(done.files.is_empty() && done.skipped.is_empty());
    assert!(root.join("Sounds").is_dir() && root.join("Defs/SoundDefs").is_dir());
    assert_eq!(
        std::fs::read(root.join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml")).unwrap(),
        before
    );
    let again = scaffold_missing(
        &w.env,
        &ProjectScaffoldMissingRequest {
            project_id: id.clone(),
            dry_run: false,
        },
        &[],
    )
    .unwrap();
    assert!(again.folders.is_empty());
    assert_eq!(
        issues_of(&w, &id)
            .iter()
            .filter(|i| i.fix.automatic)
            .count(),
        0
    );
}

#[test]
fn scaffolding_refuses_a_project_inside_a_protected_folder() {
    let w = world();
    let (root, id) = mod_at(&w, "RS_Fenced", &[]);
    let r = scaffold_missing(
        &w.env,
        &ProjectScaffoldMissingRequest {
            project_id: id,
            dry_run: false,
        },
        std::slice::from_ref(&w.base),
    );
    assert!(matches!(r, Err(ToolkitError::PathRefused { .. })), "{r:?}");
    assert!(!root.join("Defs").exists());
}

fn read(
    w: &World,
    id: &str,
    path: &str,
    max: Option<u32>,
) -> Result<rimstudio_ipc_types::project::ProjectFileDto, ToolkitError> {
    read_file(
        &w.env,
        &ProjectReadFileRequest {
            project_id: id.to_owned(),
            path: path.to_owned(),
            max_bytes: max,
        },
        &[],
    )
}

#[test]
fn reading_a_file_is_limited_and_marks_binary_files() {
    let w = world();
    let (root, id) = mod_at(&w, "RS_Read", &[("Defs/A.xml", "<Defs>\u{e9}</Defs>")]);
    let f = read(&w, &id, "Defs/A.xml", None).unwrap();
    assert_eq!(f.role, NodeRoleDto::Defs);
    assert_eq!(f.text, "<Defs>\u{e9}</Defs>");
    assert!(!f.truncated && !f.binary);
    let cut = read(&w, &id, "Defs/A.xml", Some(8)).unwrap();
    assert!(cut.truncated);
    assert_eq!(cut.bytes, 15);
    assert_eq!(cut.text, "<Defs>\u{e9}");
    let mid = read(&w, &id, "Defs/A.xml", Some(7)).unwrap();
    assert_eq!(
        mid.text, "<Defs>",
        "a character cut by the limit is dropped"
    );
    std::fs::write(root.join("Textures.bin").as_std_path(), [0u8, 1, 2]).unwrap();
    let bin = read(&w, &id, "Textures.bin", None).unwrap();
    assert!(bin.binary && bin.text.is_empty());
    assert!(matches!(
        read(&w, &id, "Defs/Missing.xml", None),
        Err(ToolkitError::ProjectInvalid { .. })
    ));
    assert!(matches!(
        read(&w, &id, "Defs", None),
        Err(ToolkitError::PathRefused { .. })
    ));
}

#[test]
fn path_attacks_on_the_file_reader_are_refused() {
    let w = world();
    let (_, id) = mod_at(&w, "RS_Attacks", &[("Defs/A.xml", "<Defs/>")]);
    std::fs::write(w.base.join("secret.txt").as_std_path(), "secret").unwrap();
    let long = format!("Defs/{}.xml", "a".repeat(300));
    let attacks = [
        "../secret.txt",
        "Defs/../../secret.txt",
        "/etc/passwd",
        "C:/Windows/win.ini",
        "C:\\Windows\\win.ini",
        "Defs\\..\\..\\secret.txt",
        "Defs/A.xml\0.txt",
        "",
        ".",
        "Defs//A.xml",
        "CON",
        "Defs/A.xml.",
        "Defs/A.xml::$DATA",
        long.as_str(),
    ];
    for attack in attacks {
        let r = read(&w, &id, attack, None);
        assert!(
            matches!(r, Err(ToolkitError::PathRefused { .. })),
            "{attack:?} gave {r:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn links_out_of_the_project_are_neither_followed_nor_read() {
    let w = world();
    let outside = w.base.join("outside");
    put(&outside, "secret.xml", "<Defs/>");
    put(&outside, "deep/more.txt", "x");
    let (root, id) = mod_at(&w, "RS_Links", &[("Defs/A.xml", "<Defs/>")]);
    std::os::unix::fs::symlink(outside.as_std_path(), root.join("Linked").as_std_path()).unwrap();
    std::os::unix::fs::symlink(
        outside.join("secret.xml").as_std_path(),
        root.join("Defs/Link.xml").as_std_path(),
    )
    .unwrap();
    let t = tree_of(&w, &id);
    assert_eq!(find(&t.root, "Linked").map(|n| n.children.len()), Some(0));
    assert_eq!(
        t.counts.files, 4,
        "the two links are files; nothing below them is listed"
    );
    assert!(
        t.root
            .children
            .iter()
            .all(|c| !c.path.starts_with("Linked/"))
    );
    assert!(read(&w, &id, "Linked/secret.xml", None).is_err());
    assert!(read(&w, &id, "Defs/Link.xml", None).is_err());
}

#[test]
fn hostile_xml_and_huge_trees_do_not_break_the_scan() {
    let w = world();
    let bomb = "<?xml version=\"1.0\"?><!DOCTYPE d [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\">]><Defs>&b;</Defs>";
    let big = format!("<Defs>{}</Defs>", " ".repeat(2_100_000));
    let (root, id) = mod_at(
        &w,
        "RS_Hostile",
        &[
            ("Defs/Bomb.xml", bomb),
            ("Defs/Big.xml", &big),
            ("Defs/Empty.xml", ""),
            (
                "Patches/Deep.xml",
                &format!("{}{}", "<a>".repeat(3000), "</a>".repeat(3000)),
            ),
        ],
    );
    std::fs::write(
        root.join("Defs/Bytes.xml").as_std_path(),
        [0xFFu8, 0xFE, 0x00, 0x3C],
    )
    .unwrap();
    let mut deep = String::new();
    for i in 0..40 {
        deep.push_str(&format!("d{i}/"));
    }
    put(&root, &format!("Textures/{deep}x.png"), "x");
    let t = tree(
        &w.env,
        &ProjectTreeRequest {
            project_id: id.clone(),
            max_nodes: Some(10),
        },
    )
    .unwrap();
    assert!(t.truncated);
    assert!(t.counts.files >= 6);
    let found = issues_of(&w, &id);
    assert!(codes(&found).contains(&"layout.unparsable-file"));
}

#[test]
fn the_tree_is_deterministic_and_lists_folders_before_files() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_Order",
        &[
            ("Defs/b.xml", "<Defs/>"),
            ("Defs/a.xml", "<Defs/>"),
            ("Defs/Z/z.xml", "<Defs/>"),
        ],
    );
    let a = tree_of(&w, &id);
    let b = tree_of(&w, &id);
    assert_eq!(a, b);
    let defs = find(&a.root, "Defs").unwrap();
    let names: Vec<&str> = defs.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Z", "a.xml", "b.xml"]);
    assert_eq!(defs.files, 3);
}

#[test]
fn a_standard_folder_at_the_root_counts_when_the_content_lives_in_common() {
    let w = world();
    let (_, id) = mod_at(
        &w,
        "RS_RootAreas",
        &[
            ("Common/Defs/Weapons/A.xml", "<Defs/>"),
            ("Patches/a.xml", "<Patch/>"),
            ("Textures/Things/a.png", "x"),
            ("Sounds/a.wav", "x"),
        ],
    );
    let found = issues_of(&w, &id);
    let missing: Vec<&str> = found
        .iter()
        .filter(|i| i.code == "layout.missing-folder")
        .map(|i| i.path.as_str())
        .collect();
    assert_eq!(missing, ["Common/Defs/SoundDefs"], "{found:?}");
}
