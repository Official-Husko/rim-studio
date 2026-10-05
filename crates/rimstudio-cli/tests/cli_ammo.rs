//! The ammunition commands of the CLI: `designer ammo catalog`, `designer ammo suggest`, and the custom ammo
//! of a draft set with `--set ce.customAmmo.*`, against a fictional install in a temporary HOME. The
//! fictional Combat Extended of the CLI fixture has one ammo set and no ammo items, so the suggestions say
//! plainly that there is nothing to suggest from; the numbers of the real install are in the toolkit tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::{Env, files_under};
use serde_json::json;
use std::path::Path;

fn create_project(env: &Env, name: &str) -> String {
    let dir = env.path("projects").join(name);
    let p = dir.to_str().unwrap().to_owned();
    env.run(&[
        "project",
        "create",
        &p,
        "--name",
        name,
        "--package-id",
        "rs.ammo",
    ])
    .expect(0);
    p
}

fn plan_json(env: &Env, args: &[&str]) -> serde_json::Value {
    let run = env.run(args);
    assert!(
        matches!(run.code(), 0 | 3),
        "stdout:\n{}\nstderr:\n{}",
        run.stdout(),
        run.stderr()
    );
    run.json()
}

fn new_rifle(env: &Env, project: &str, name: &str) -> String {
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            name,
            "--project",
            project,
            "--strength",
            "typical",
            "--set",
            "ranged.projectile=RS_Shot00",
            "--set",
            "costList=RS_Steel:30",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
            "--set",
            "parent=RS_BaseGun",
            "--set",
            "workToMake=9000",
            "--set",
            "ranged.damage=12",
            "--set",
            "prefix=RS",
        ])
        .expect(0)
        .json();
    out["id"].as_str().unwrap().to_owned()
}

#[test]
fn the_catalogue_lists_the_sets_of_the_install_as_text_and_json() {
    let env = Env::new(true);
    env.select_install();
    let text = env.run(&["designer", "ammo", "catalog"]).expect(0).stdout();
    assert!(text.contains("1 ammo sets in the install"), "{text}");
    assert!(text.contains("RS_AmmoSetA"), "{text}");
    let doc = env
        .run(&["--json", "designer", "ammo", "catalog", "--page-size", "5"])
        .expect(0)
        .json();
    assert_eq!(doc["available"], json!(true));
    assert_eq!(doc["total"], json!(1));
    assert_eq!(doc["entries"][0]["defName"], json!("RS_AmmoSetA"));
    assert_eq!(
        doc["entries"][0]["types"][0]["projectileDef"],
        json!("RS_CeBullet1")
    );
    assert_eq!(doc["entries"][0]["types"][0]["damage"], json!(9.0));
    let none = env
        .run(&[
            "--json", "designer", "ammo", "catalog", "nothing", "like", "it",
        ])
        .expect(0)
        .json();
    assert_eq!(none["matching"], json!(0));
}

#[test]
fn the_catalogue_without_combat_extended_says_so_with_exit_3() {
    let env = Env::new(false);
    env.select_install();
    let run = env.run(&["designer", "ammo", "catalog"]).expect(3);
    assert!(
        run.stdout().contains("No ammunition to list"),
        "{}",
        run.stdout()
    );
}

#[test]
fn a_set_override_without_a_draft_is_a_usage_error() {
    let env = Env::new(true);
    env.select_install();
    env.run(&["designer", "ammo", "catalog", "--set", "ce.bulk=1"])
        .expect(2);
}

#[test]
fn the_catalogue_ranks_the_sets_for_a_draft_and_writes_nothing() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let doc = env
        .run(&[
            "--json",
            "designer",
            "ammo",
            "catalog",
            "--draft",
            &id,
            "--project",
            &project,
        ])
        .expect(0)
        .json();
    assert_eq!(doc["total"], json!(1));
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()]
    );
}

#[test]
fn a_suggestion_without_ammunition_data_says_so() {
    let env = Env::new(true);
    env.select_install();
    let run = env
        .run(&["designer", "ammo", "suggest", "RS_FMJ", "--damage", "12"])
        .expect(3);
    assert!(
        run.stdout().contains("Nothing to suggest"),
        "{}",
        run.stdout()
    );
    let doc = env
        .run(&["--json", "designer", "ammo", "suggest", "RS_FMJ"])
        .expect(3)
        .json();
    assert_eq!(doc["available"], json!(false));
}

#[test]
fn the_plan_of_a_custom_caliber_lists_the_ammunition_file_only_with_ce() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let sets = [
        "--set",
        "ce.weaponTagClass=RS_CE_Class",
        "--set",
        "ce.magazineSize=30",
        "--set",
        "ce.reloadTime=4",
        "--set",
        "ce.bulk=6.5",
        "--set",
        "ce.swayFactor=1.2",
        "--set",
        "ce.shotSpread=0.08",
        "--set",
        "ce.customAmmo.name=Mine",
        "--set",
        "ce.customAmmo.caliber=my test caliber",
        "--set",
        "ce.customAmmo.types.0.ammoClass=RS_FMJ",
        "--set",
        "ce.customAmmo.types.0.projectile.damage=10",
        "--set",
        "ce.customAmmo.types.0.projectile.armorPenetrationSharp=3",
        "--set",
        "ce.customAmmo.types.0.projectile.armorPenetrationBlunt=12",
        "--set",
        "ce.customAmmo.types.0.projectile.speed=120",
        "--set",
        "ce.customAmmo.types.0.recipe.ingredients.0.thing=RS_Steel",
        "--set",
        "ce.customAmmo.types.0.recipe.ingredients.0.count=9",
    ];
    let mut args = vec!["--json", "plan", &id, "--project", &project, "--ce"];
    args.extend(sets);
    let doc = plan_json(&env, &args);
    let paths: Vec<&str> = doc["plan"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(
        paths.contains(&"Compat/CombatExtended/Defs/Ammo/RS_Mine.xml"),
        "{paths:?}"
    );
    // without --ce the plan holds the vanilla file only
    let mut args = vec!["--json", "plan", &id, "--project", &project];
    args.extend(sets);
    let doc = plan_json(&env, &args);
    let text = doc.to_string();
    assert!(!text.contains("Defs/Ammo"), "{text}");
    assert!(!text.contains("RS_AmmoSet_Mine"), "{text}");
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()]
    );
}
