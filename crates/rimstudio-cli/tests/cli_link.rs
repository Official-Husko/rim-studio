//! `project link status|create|remove` against a fictional install in a temporary folder with real links.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::Env;

fn project(env: &Env) -> String {
    env.mod_folder("RS_Linked", &[])
        .to_str()
        .unwrap()
        .to_owned()
}

fn mods(env: &Env) -> std::path::PathBuf {
    std::path::PathBuf::from(env.game_dir()).join("Mods")
}

#[test]
fn status_create_and_remove_round_trip() {
    let env = Env::new(false);
    env.select_install();
    let p = project(&env);
    std::fs::create_dir_all(mods(&env)).unwrap();

    let st = env
        .run(&["--json", "project", "link", "status", &p])
        .expect(0)
        .json();
    assert_eq!(st["state"], "not-linked", "{st}");
    assert_eq!(st["canCreate"], true);
    assert!(st["manualCommand"].as_str().unwrap().starts_with("ln -s "));

    // without --yes nothing is written
    let dry = env
        .run(&["project", "link", "create", &p])
        .expect(0)
        .stdout();
    assert!(dry.contains("Nothing was changed"), "{dry}");
    assert!(!mods(&env).join("RS_Linked").exists());

    let made = env
        .run(&["--json", "project", "link", "create", &p, "--yes"])
        .expect(0)
        .json();
    assert_eq!(made["done"], true, "{made}");
    assert_eq!(made["status"]["state"], "linked");
    let link = mods(&env).join("RS_Linked");
    assert!(link.is_symlink());
    assert!(link.join("About/About.xml").is_file());

    let text = env
        .run(&["project", "link", "status", &p])
        .expect(0)
        .stdout();
    assert!(text.contains("linked: the game sees this mod"), "{text}");

    // a second create is refused with the failure code and the reason
    let again = env
        .run(&["project", "link", "create", &p, "--yes"])
        .expect(1);
    assert!(
        again.stdout().contains("nothing was changed"),
        "{}",
        again.stdout()
    );

    let gone = env
        .run(&["--json", "project", "link", "remove", &p])
        .expect(0)
        .json();
    assert_eq!(gone["done"], true);
    assert!(!link.exists());
    assert!(std::path::Path::new(&p).join("About/About.xml").is_file());
}

#[test]
fn an_existing_folder_is_refused_and_left_alone() {
    let env = Env::new(false);
    env.select_install();
    let p = project(&env);
    let taken = mods(&env).join("RS_Linked");
    std::fs::create_dir_all(&taken).unwrap();
    std::fs::write(taken.join("keep.txt"), "keep").unwrap();
    let out = env
        .run(&["--json", "project", "link", "create", &p, "--yes"])
        .expect(1)
        .json();
    assert_eq!(out["done"], false, "{out}");
    assert_eq!(out["refusal"]["code"], "deploy.name-taken");
    assert_eq!(
        std::fs::read_to_string(taken.join("keep.txt")).unwrap(),
        "keep"
    );
    env.run(&["project", "link", "remove", &p]).expect(1);
    assert!(taken.join("keep.txt").is_file());
}

#[test]
fn a_missing_mods_folder_says_so_and_gives_the_manual_command() {
    let env = Env::new(false);
    env.select_install();
    let p = project(&env);
    let m = mods(&env);
    if m.exists() {
        std::fs::remove_dir_all(&m).unwrap();
    }
    let st = env
        .run(&["--json", "project", "link", "status", &p])
        .expect(0)
        .json();
    assert_eq!(st["state"], "unavailable");
    assert_eq!(st["modsExists"], false);
    assert!(st["manualCommand"].as_str().unwrap().contains("ln -s"));
    let out = env
        .run(&["--json", "project", "link", "create", &p, "--yes"])
        .expect(1)
        .json();
    assert_eq!(out["refusal"]["code"], "deploy.mods-missing");
    assert!(!m.exists());
}

#[test]
fn without_an_install_nothing_can_be_linked() {
    let env = Env::new(false);
    let p = project(&env);
    let out = env
        .run(&["--json", "project", "link", "create", &p, "--yes"])
        .expect(1)
        .json();
    // Detection may find nothing, or the fictional install is not selected: either way nothing is written.
    let code = out["refusal"]["code"].as_str().unwrap_or_default();
    assert!(code.starts_with("deploy."), "{out}");
    assert!(!mods(&env).join("RS_Linked").exists());
}
