//! The layout commands of the CLI: `project create` with the layout options, `tree`, `check`,
//! `scaffold-missing` and `read`, against fictional mod folders in a temporary HOME.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::{Env, files_under};

const RANGED: &str = "<Defs><ThingDef ParentName=\"BaseGun\"><defName>RS_Gun</defName><techLevel>Industrial</techLevel><graphicData><texPath>Things/Item/Equipment/WeaponRanged/RS_Gun</texPath></graphicData><verbs><li><verbClass>Verb_Shoot</verbClass></li></verbs></ThingDef></Defs>";

fn create(env: &Env, name: &str, extra: &[&str]) -> String {
    let dir = env.path("projects").join(name);
    let p = dir.to_str().unwrap().to_owned();
    let mut args = vec![
        "project",
        "create",
        p.as_str(),
        "--name",
        name,
        "--package-id",
        "rs.layout",
    ];
    args.extend_from_slice(extra);
    env.run(&args).expect(0);
    p
}

#[test]
fn a_new_mod_has_the_standard_skeleton_and_nothing_optional() {
    let env = Env::new(false);
    let p = create(&env, "RS_Standard", &[]);
    let files = files_under(std::path::Path::new(&p));
    assert!(files.contains(&"About/About.xml".to_owned()), "{files:?}");
    for never in [
        ".gitignore",
        "README.md",
        "Credits.txt",
        "About/Preview.png",
        "About/Manifest.xml",
        "LoadFolders.xml",
    ] {
        assert!(!files.contains(&never.to_owned()), "{never}");
    }
    let dir = std::path::Path::new(&p);
    for folder in [
        "Defs/ThingDefs_Misc/Weapons",
        "Defs/SoundDefs",
        "Patches",
        "Sounds/Weapons",
        "Textures/Things/Item/Equipment/WeaponRanged",
        "Textures/Things/Item/Equipment/WeaponMelee",
        "Textures/Things/Projectile",
    ] {
        assert!(dir.join(folder).is_dir(), "{folder}");
    }
    assert!(!dir.join("Compat").exists() && !dir.join("Source").exists());
    let tree = env.run(&["--json", "project", "tree", &p]).expect(0).json();
    assert_eq!(tree["profile"], "rimstudio");
    assert_eq!(tree["issues"].as_array().unwrap().len(), 0, "{tree}");
}

#[test]
fn the_optional_folders_and_files_appear_only_when_asked() {
    let env = Env::new(false);
    let p = create(
        &env,
        "RS_Full",
        &[
            "--versioned",
            "--ce-folder",
            "--source",
            "--gitignore",
            "--ignore-source-art",
            "--readme",
            "--credits",
            "--languages",
        ],
    );
    let dir = std::path::Path::new(&p);
    for rel in [
        "LoadFolders.xml",
        "Common",
        "1.6/Compat/CombatExtended/Patches",
        "Source/Art",
        "1.6/Languages/English/Keyed",
        ".gitignore",
        "README.md",
        "Credits.txt",
    ] {
        assert!(dir.join(rel).exists(), "{rel}");
    }
    assert!(
        std::fs::read_to_string(dir.join(".gitignore"))
            .unwrap()
            .contains("Source/Art/")
    );
    let lf = std::fs::read_to_string(dir.join("LoadFolders.xml")).unwrap();
    assert!(
        lf.contains("IfModActive=\"ceteam.combatextended\">1.6/Compat/CombatExtended</li>"),
        "{lf}"
    );
    let check = env
        .run(&["--json", "project", "check", &p])
        .expect(0)
        .json();
    assert_eq!(check["errors"], 0, "{check}");
}

#[test]
fn check_lists_the_issues_and_scaffold_missing_creates_folders_only() {
    let env = Env::new(false);
    let dir = env.mod_folder(
        "RS_Core",
        &[
            ("Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml", RANGED),
            (
                "Patches/ce.xml",
                "<Patch><Operation Class=\"CombatExtended.PatchOperationFindMod\"/></Patch>",
            ),
        ],
    );
    let p = dir.to_str().unwrap();
    let text = env.run(&["project", "check", p]).expect(3);
    let out = text.stdout();
    assert!(out.contains("layout.ce-outside-gate"), "{out}");
    assert!(out.contains("core-style"), "{out}");
    assert!(out.contains("suggested fix"), "{out}");
    let dry = env
        .run(&["--json", "project", "scaffold-missing", p, "--dry-run"])
        .expect(0)
        .json();
    assert_eq!(dry["dryRun"], true);
    assert!(!dir.join("Sounds").exists());
    let before = std::fs::read(dir.join("Patches/ce.xml")).unwrap();
    let done = env
        .run(&["--json", "project", "scaffold-missing", p])
        .expect(0)
        .json();
    assert_eq!(done["folders"], dry["folders"]);
    assert!(dir.join("Sounds").is_dir() && dir.join("Textures").is_dir());
    assert_eq!(std::fs::read(dir.join("Patches/ce.xml")).unwrap(), before);
    let again = env
        .run(&["--json", "project", "scaffold-missing", p])
        .expect(0)
        .json();
    assert_eq!(again["folders"].as_array().unwrap().len(), 0);
    let text = env
        .run(&["project", "scaffold-missing", p])
        .expect(0)
        .stdout();
    assert!(text.contains("nothing is missing"), "{text}");
}

#[test]
fn tree_prints_roles_and_read_prints_a_file_and_refuses_paths_outside() {
    let env = Env::new(false);
    let dir = env.mod_folder(
        "RS_Tree",
        &[(
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Gun.xml",
            RANGED,
        )],
    );
    let p = dir.to_str().unwrap();
    let text = env
        .run(&["project", "tree", p, "--files", "--depth", "6"])
        .expect(0)
        .stdout();
    assert!(text.contains("[defs-weapons]"), "{text}");
    assert!(text.contains("RS_Gun.xml"), "{text}");
    assert!(text.contains("layout: rimstudio"), "{text}");
    let read = env
        .run(&[
            "project",
            "read",
            p,
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_Gun.xml",
        ])
        .expect(0)
        .stdout();
    assert!(read.contains("<defName>RS_Gun</defName>"), "{read}");
    let cut = env
        .run(&["project", "read", p, "About/About.xml", "--max-bytes", "10"])
        .expect(0)
        .stdout();
    assert!(cut.contains("cut at"), "{cut}");
    for attack in ["../outside.txt", "/etc/passwd", "Defs/../../x"] {
        let run = env.run(&["--json", "project", "read", p, attack]);
        assert_ne!(run.code(), 0, "{attack}");
        assert!(
            run.stderr().contains("project.path-outside-root")
                || run.stdout().contains("project.path-outside-root"),
            "{attack}: {}",
            run.stderr()
        );
    }
}
