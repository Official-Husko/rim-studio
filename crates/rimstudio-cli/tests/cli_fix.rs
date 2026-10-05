//! `project fix plan|apply|undo|history` against a fictional mod in a temporary HOME.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::{Env, files_under};

const ABOUT: &str = "<ModMetaData><name>RS Fix</name><packageId>rs.fix</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";
const CE_PATCH: &str = "<Patch><Operation Class=\"CombatExtended.PatchOperationFindMod\"><modName>Combat Extended</modName></Operation></Patch>";

fn mod_with_stray_patch(env: &Env) -> String {
    let dir = env.path("projects/RS_Fix");
    std::fs::create_dir_all(dir.join("About")).unwrap();
    std::fs::create_dir_all(dir.join("Patches")).unwrap();
    std::fs::write(dir.join("About/About.xml"), ABOUT).unwrap();
    std::fs::write(dir.join("Patches/ce.xml"), CE_PATCH).unwrap();
    dir.to_str().unwrap().to_owned()
}

#[test]
fn plan_apply_history_and_undo_round_trip() {
    let env = Env::new(false);
    let p = mod_with_stray_patch(&env);
    let before = files_under(std::path::Path::new(&p));

    let plan = env
        .run(&["--json", "project", "fix", "plan", &p])
        .expect(0)
        .json();
    let items = plan["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|i| i["kind"] == "move-file" && i["to"] == "Compat/CombatExtended/Patches/ce.xml"),
        "{plan}"
    );
    let text = env.run(&["project", "fix", "plan", &p]).expect(0).stdout();
    assert!(
        text.contains("Patches/ce.xml -> Compat/CombatExtended/Patches/ce.xml"),
        "{text}"
    );
    assert!(
        text.contains("+    <li IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>"),
        "{text}"
    );
    assert_eq!(before, files_under(std::path::Path::new(&p)));

    // without --yes nothing is written
    let dry = env
        .run(&["project", "fix", "apply", &p, "--all"])
        .expect(0)
        .stdout();
    assert!(dry.contains("Nothing was changed"), "{dry}");
    assert_eq!(before, files_under(std::path::Path::new(&p)));
    // an apply needs to be told what to do
    env.run(&["project", "fix", "apply", &p, "--yes"]).expect(2);

    let done = env
        .run(&["--json", "project", "fix", "apply", &p, "--all", "--yes"])
        .expect(0)
        .json();
    let apply_id = done["applyId"].as_str().unwrap().to_owned();
    assert!(
        std::path::Path::new(&p)
            .join("Compat/CombatExtended/Patches/ce.xml")
            .is_file()
    );
    assert!(!std::path::Path::new(&p).join("Patches/ce.xml").exists());
    assert_eq!(done["check"]["warnings"], 0);

    let history = env
        .run(&["project", "fix", "history", &p])
        .expect(0)
        .stdout();
    assert!(
        history.contains(&apply_id) && history.contains("can be undone"),
        "{history}"
    );

    let undone = env
        .run(&["project", "fix", "undo", &p, &apply_id])
        .expect(0)
        .stdout();
    assert!(undone.contains("moved back Patches/ce.xml"), "{undone}");
    assert_eq!(before, files_under(std::path::Path::new(&p)));
    // a second undo is refused with an error exit
    env.run(&["project", "fix", "undo", &p, &apply_id])
        .expect(1);
    let history = env
        .run(&["project", "fix", "history", &p])
        .expect(0)
        .stdout();
    assert!(history.contains("undone"), "{history}");
}

#[test]
fn a_plan_id_that_is_stale_refuses_the_apply() {
    let env = Env::new(false);
    let p = mod_with_stray_patch(&env);
    let before = files_under(std::path::Path::new(&p));
    env.run(&[
        "project",
        "fix",
        "apply",
        &p,
        "--all",
        "--yes",
        "--plan-id",
        "0123456789abcdef0123456789abcdef",
    ])
    .expect(1);
    assert_eq!(before, files_under(std::path::Path::new(&p)));
}
