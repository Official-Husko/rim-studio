//! The archetype commands of the CLI: `designer archetypes`, `designer propose` and `designer new
//! --archetype`, against a fictional install in a temporary HOME. Owner rules: nothing is written outside the
//! project, vanilla by default (Combat Extended only with `--ce`), typed values are never overwritten.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::Env;
use serde_json::json;

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
        "rs.arch",
    ])
    .expect(0);
    p
}

#[test]
fn the_catalogue_lists_the_families_as_text_and_json() {
    let env = Env::new(false);
    env.select_install();
    let text = env.run(&["designer", "archetypes"]).expect(0).stdout();
    assert!(text.contains("rifle/assault"), "{text}");
    assert!(text.contains("sword/long"), "{text}");
    assert!(text.contains("reference weapons in your install"), "{text}");
    let doc = env
        .run(&["--json", "designer", "archetypes", "melee"])
        .expect(0)
        .json();
    assert_eq!(doc["proposalsAvailable"], json!(true));
    let ids: Vec<&str> = doc["families"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["archetypes"].as_array().unwrap())
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"knife/knife") && !ids.contains(&"rifle/assault"),
        "{ids:?}"
    );
}

#[test]
fn propose_prints_every_number_with_its_reason_and_changes_nothing() {
    let env = Env::new(false);
    env.select_install();
    let text = env
        .run(&[
            "designer",
            "propose",
            "rifle/assault",
            "--rof",
            "fast",
            "--balance",
            "stronger",
        ])
        .expect(0)
        .stdout();
    assert!(text.contains("Proposal for Assault rifle"), "{text}");
    assert!(text.contains("/ranged/damage"), "{text}");
    assert!(text.contains("the install's median is"), "{text}");
    let doc = env
        .run(&[
            "--json",
            "designer",
            "propose",
            "rifle/sniper",
            "--rof",
            "20",
            "--balance",
            "0.7",
        ])
        .expect(0)
        .json();
    assert_eq!(doc["source"], json!("archetype"));
    assert_eq!(doc["choice"]["balance"], json!({"percentile": 0.7}));
    assert_eq!(doc["choice"]["descriptors"]["rof"], json!({"rpm": 20.0}));
    assert!(doc["values"].as_array().unwrap().len() >= 10);
}

#[test]
fn a_bad_archetype_descriptor_or_balance_is_a_clear_failure() {
    let env = Env::new(false);
    env.select_install();
    env.run(&["designer", "propose", "rifle/nothing"]).expect(2);
    env.run(&["designer", "propose", "rifle/assault", "--balance", "huge"])
        .expect(2);
    let run = env.run(&[
        "designer",
        "propose",
        "rifle/sniper",
        "--action",
        "full-auto",
    ]);
    assert_ne!(run.code(), 0);
    assert!(run.stderr().contains("full-auto"), "{}", run.stderr());
}

#[test]
fn new_with_an_archetype_fills_a_draft_and_keeps_typed_values() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let doc = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_MyRifle",
            "--project",
            &project,
            "--archetype",
            "rifle/assault",
            "--rof",
            "fast",
            "--balance",
            "typical",
            "--set",
            "ranged.damage=14",
        ])
        .expect(0)
        .json();
    let draft = &doc["draft"];
    assert_eq!(draft["archetype"]["archetype"], json!("rifle/assault"));
    assert_eq!(draft["spec"]["ranged"]["damage"]["value"], json!(14.0));
    assert_eq!(draft["spec"]["ranged"]["damage"]["source"], json!("typed"));
    assert_eq!(
        draft["spec"]["ranged"]["range"]["source"],
        json!("suggested")
    );
    assert!(draft["spec"].get("ce").is_none(), "vanilla by default");
    assert!(doc["filled"].as_u64().unwrap() >= 8);
    let files = common::files_under(std::path::Path::new(&project));
    assert_eq!(
        files,
        vec!["About/About.xml".to_owned()],
        "nothing but the draft store is written"
    );
    // the descriptors need an archetype
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_Other",
        "--project",
        &project,
        "--rof",
        "fast",
    ])
    .expect(2);
}

#[test]
fn a_melee_archetype_makes_tools() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let doc = env
        .run(&[
            "--json",
            "new",
            "melee",
            "--name",
            "RS_MyBlade",
            "--project",
            &project,
            "--archetype",
            "sword/long",
        ])
        .expect(0)
        .json();
    assert_eq!(doc["draft"]["spec"]["tools"].as_array().unwrap().len(), 3);
}
