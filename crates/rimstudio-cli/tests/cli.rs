//! Runs the built binary against a fictional install with a temporary HOME and XDG folders.
//!
//! Nothing here reads a real game install; every name starts with `RS_` and every number is invented.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::path::Path;

use common::{Env, Run, files_under};
use serde_json::{Value, json};

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// A project folder created through the CLI.
fn create_project(env: &Env, name: &str) -> String {
    let dir = env.path("projects").join(name);
    let p = path_str(&dir).to_owned();
    env.run(&[
        "project",
        "create",
        &p,
        "--name",
        name,
        "--package-id",
        "rs.mine",
    ])
    .expect(0);
    p
}

const RIFLE_SETS: [&str; 14] = [
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
];

/// Creates the draft of a complete ranged design and returns its id.
fn new_rifle(env: &Env, project: &str, name: &str) -> String {
    let mut args = vec![
        "--json",
        "new",
        "ranged",
        "--name",
        name,
        "--project",
        project,
        "--strength",
        "typical",
    ];
    args.extend(RIFLE_SETS);
    let out = env.run(&args).expect(0).json();
    out["id"].as_str().unwrap().to_owned()
}

const CE_SETS: [&str; 14] = [
    "--set",
    "ce.ammoSet=RS_AmmoSetA",
    "--set",
    "ce.weaponTagClass=RS_CE_Class",
    "--set",
    "ce.defaultProjectile=RS_CeBullet1",
    "--set",
    "ce.magazineSize=30",
    "--set",
    "ce.reloadTime=4",
    "--set",
    "ce.bulk=6.5",
    "--set",
    "ce.swayFactor=1.2",
];

fn game_files(env: &Env) -> Vec<String> {
    files_under(Path::new(env.install.game_dir.as_str()))
}

fn kinds(plan: &Value) -> Vec<String> {
    plan["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn error_code(run: &Run) -> String {
    let line = run.stderr().lines().last().unwrap_or("").to_owned();
    serde_json::from_str::<Value>(&line)
        .ok()
        .and_then(|v| v["error"]["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------
// generic commands, usage and exit codes
// ---------------------------------------------------------------------------------------------------

#[test]
fn commands_lists_the_registry_as_text_and_as_json() {
    let env = Env::new(false);
    let text = env.run(&["commands"]).expect(0).stdout();
    assert!(text.contains("designer_export_plan"), "{text}");
    assert!(text.contains("settings_get"));
    let json = env.run(&["commands", "--json"]).expect(0).json();
    let names: Vec<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"designer_apply_plan") && names.contains(&"library_scan"));
    assert!(names.len() >= 33, "{}", names.len());
}

#[test]
fn version_prints_the_version_and_the_contract_hash() {
    let env = Env::new(false);
    let text = env.run(&["version"]).expect(0).stdout();
    assert!(text.starts_with("rimstudio-cli "), "{text}");
    let json = env.run(&["--json", "version"]).expect(0).json();
    assert_eq!(json["name"], json!("rimstudio-cli"));
    assert_eq!(json["contractHash"].as_str().unwrap().len(), 16);
}

#[test]
fn usage_errors_exit_with_code_2() {
    let env = Env::new(false);
    env.run(&["no-such-command"]).expect(2);
    env.run(&["designer", "refs", "sideways"]).expect(2);
    env.run(&["sources", "add"]).expect(2);
    env.run(&["call", "no_such_command"]).expect(2);
    env.run(&["call", "app_ping", "--data", "{not json"])
        .expect(2);
    env.run(&["call", "app_ping", "{}", "--data", "{}"])
        .expect(2);
    let bad = env
        .run(&[
            "call",
            "settings_update",
            "--data",
            r#"{"appearance": {"density": "huge"}}"#,
        ])
        .expect(2);
    assert!(
        bad.stderr().contains("appearance.density"),
        "{}",
        bad.stderr()
    );
}

#[test]
fn help_exits_zero_and_names_the_exit_codes() {
    let env = Env::new(false);
    let help = env.run(&["--help"]).expect(0).stdout();
    assert!(help.contains("Exit codes"), "{help}");
    env.run(&["designer", "apply", "--help"]).expect(0);
}

#[test]
fn errors_are_json_envelopes_on_stderr_in_json_mode() {
    let env = Env::new(false);
    let run = env.run(&["--json", "scan"]).expect(1);
    assert!(
        run.stdout().trim().is_empty(),
        "nothing on stdout: {}",
        run.stdout()
    );
    assert_eq!(error_code(&run), "library.scan-failed", "{}", run.stderr());
    let text = env.run(&["scan"]).expect(1);
    assert!(text.stderr().contains("hint:"), "{}", text.stderr());
}

#[test]
fn call_runs_any_command_and_a_job() {
    let env = Env::new(false);
    let pong = env
        .run(&["call", "app_ping", "--json", r#"{"echo":"hi"}"#])
        .expect(0)
        .json();
    assert_eq!(pong["echo"], json!("hi"));
    let info = env.run(&["call", "app_get_info"]).expect(0).json();
    assert_eq!(info["commandCount"], json!(42));
    env.select_install();
    let scan = env
        .run(&["call", "library_scan", r#"{"full":true}"#])
        .expect(0)
        .json();
    assert!(scan["stats"]["modsFound"].as_u64().unwrap() >= 1);
}

#[test]
fn call_can_read_the_request_from_a_file() {
    let env = Env::new(false);
    let file = env.path("request.json");
    std::fs::write(&file, r#"{"echo": "from file"}"#).unwrap();
    let out = env
        .run(&["call", "app_ping", "--json-file", path_str(&file)])
        .expect(0)
        .json();
    assert_eq!(out["echo"], json!("from file"));
    env.run(&["call", "app_ping", "--json-file", "missing.json"])
        .expect(2);
}

#[test]
fn call_out_writes_the_response_to_a_file_and_refuses_the_app_folders() {
    let env = Env::new(false);
    let run = env
        .run(&[
            "--json",
            "call",
            "app_ping",
            r#"{"echo":"x"}"#,
            "--out",
            "result.json",
        ])
        .expect(0);
    let report = run.json();
    assert_eq!(report["path"], json!(path_str(&env.path("result.json"))));
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(env.path("result.json")).unwrap()).unwrap();
    assert_eq!(written["echo"], json!("x"));
    let inside = env.home.join(".local/share/rimstudio/x.json");
    env.run(&["call", "app_ping", "--out", path_str(&inside)])
        .expect(2);
    assert!(!inside.exists());
}

// ---------------------------------------------------------------------------------------------------
// detect, sources, scan, settings
// ---------------------------------------------------------------------------------------------------

#[test]
fn detect_reports_the_install_version_and_how_it_was_found() {
    let env = Env::new(true);
    let game = env.game_dir();
    let text = env.run(&["detect", "--install", &game]).expect(0).stdout();
    assert!(text.contains(&game), "{text}");
    assert!(text.contains("version: 1.6.1000 rev100"), "{text}");
    assert!(text.contains("found by: override"), "{text}");
    let json = env.run(&["--json", "detect"]).expect(0).json();
    assert_eq!(
        json["installs"][0]["version"]["raw"],
        json!("1.6.1000 rev100")
    );
    assert!(json["customFolders"].as_array().unwrap().is_empty());
    let cleared = env
        .run(&["--json", "detect", "--clear", "install"])
        .expect(0)
        .json();
    assert!(cleared["installs"].as_array().unwrap().is_empty());
    env.run(&["detect", "--clear", "nonsense"]).expect(2);
}

#[test]
fn detect_without_an_install_says_how_to_set_one() {
    let env = Env::new(false);
    let text = env.run(&["detect"]).expect(0).stdout();
    assert!(text.contains("none found"), "{text}");
    assert!(text.contains("--install"));
}

#[test]
fn sources_can_be_added_listed_and_removed() {
    let env = Env::new(false);
    let custom = env.path("custom_mods");
    std::fs::create_dir_all(custom.join("RS_ModA/About")).unwrap();
    std::fs::write(
        custom.join("RS_ModA/About/About.xml"),
        common::about("rs.moda"),
    )
    .unwrap();
    let added = env
        .run(&[
            "--json",
            "sources",
            "add",
            path_str(&custom),
            "--label",
            "RS Custom",
        ])
        .expect(0)
        .json();
    let id = added["source"]["id"].as_str().unwrap().to_owned();
    assert_eq!(added["source"]["kind"], json!("custom"));
    let listed = env.run(&["--json", "sources", "list"]).expect(0).json();
    assert!(
        listed["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == json!(id))
    );
    let text = env.run(&["sources", "list"]).expect(0).stdout();
    assert!(text.contains(&id) && text.contains("custom"), "{text}");
    let detect = env.run(&["--json", "detect"]).expect(0).json();
    assert_eq!(detect["customFolders"].as_array().unwrap().len(), 1);
    let removed = env
        .run(&["--json", "sources", "remove", &id])
        .expect(0)
        .json();
    assert_eq!(removed["removed"], json!(true));
    let again = env
        .run(&["--json", "sources", "remove", &id])
        .expect(0)
        .json();
    assert_eq!(again["removed"], json!(false));
    assert!(
        custom.join("RS_ModA/About/About.xml").exists(),
        "the folder itself is never touched"
    );
}

#[test]
fn scan_reports_counts_and_timings() {
    let env = Env::new(true);
    env.select_install();
    let text = env.run(&["scan"]).expect(0).stdout();
    assert!(text.contains("mods found: 2"), "{text}");
    assert!(text.contains("definitions:"));
    assert!(text.contains("time:"));
    let json = env.run(&["--json", "scan", "--full"]).expect(0).json();
    assert_eq!(json["stats"]["modsFound"], json!(2));
    assert_eq!(json["cancelled"], json!(false));
    assert!(json["timings"]["totalMs"].is_u64());
}

#[test]
fn settings_can_be_read_changed_and_reset() {
    let env = Env::new(false);
    let all = env.run(&["--json", "settings", "get"]).expect(0).json();
    assert_eq!(all["appearance"]["density"], json!("standard"));
    let text = env
        .run(&["settings", "get", "appearance.density"])
        .expect(0)
        .stdout();
    assert_eq!(text.trim(), "appearance.density = standard");
    env.run(&["settings", "set", "appearance.density", "compact"])
        .expect(0);
    let value = env
        .run(&["--json", "settings", "get", "appearance.density"])
        .expect(0)
        .json();
    assert_eq!(value, json!("compact"));
    let file = std::fs::read_to_string(env.home.join(".config/rimstudio/settings.jsonc")).unwrap();
    assert!(file.contains("compact"), "{file}");
    env.run(&["settings", "reset", "appearance.density"])
        .expect(0);
    let back = env
        .run(&["--json", "settings", "get", "appearance.density"])
        .expect(0)
        .json();
    assert_eq!(back, json!("standard"));
    env.run(&["settings", "set", "appearance.density", "huge"])
        .expect(2);
    env.run(&["settings", "get", "no.such.key"]).expect(2);
}

// ---------------------------------------------------------------------------------------------------
// the designer: vanilla by default
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_designer_needs_an_install_and_says_so() {
    let env = Env::new(false);
    let run = env.run(&["--json", "refs", "ranged"]).expect(1);
    assert_eq!(error_code(&run), "designer.reference-unavailable");
}

#[test]
fn refs_lists_reference_weapons() {
    let env = Env::new(false);
    env.select_install();
    let text = env
        .run(&["designer", "refs", "ranged", "--limit", "5"])
        .expect(0)
        .stdout();
    assert!(text.contains("RS_Gun00"), "{text}");
    let json = env.run(&["--json", "refs", "melee"]).expect(0).json();
    assert_eq!(json["kind"], json!("melee"));
    assert_eq!(json["total"], json!(8));
    let rifles = env
        .run(&["--json", "refs", "ranged", "--role", "RS_Rifle"])
        .expect(0)
        .json();
    assert!(
        rifles["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["role"] == json!("RS_Rifle"))
    );
}

#[test]
fn calibrate_prints_the_pool_and_variants() {
    let env = Env::new(false);
    env.select_install();
    let json = env.run(&["--json", "calibrate", "ranged"]).expect(0).json();
    assert_eq!(json["kind"], json!("ranged"));
    assert_eq!(json["poolSize"], json!(14));
    let text = env.run(&["calibrate", "ranged"]).expect(0).stdout();
    assert!(text.contains("pool of 14"), "{text}");
}

#[test]
fn new_creates_a_draft_that_lists_and_shows() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let list = env
        .run(&["--json", "drafts", "list", "--project", &project])
        .expect(0)
        .json();
    assert_eq!(list["drafts"][0]["id"], json!(id));
    assert_eq!(list["drafts"][0]["defName"], json!("RS_NewRifle"));
    let draft = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .json();
    assert!(
        draft["spec"].get("ce").is_none(),
        "a new draft is vanilla: {draft}"
    );
    assert_eq!(draft["spec"]["ranged"]["damage"]["source"], json!("typed"));
    assert_eq!(
        draft["spec"]["ranged"]["range"]["source"],
        json!("suggested")
    );
    let text = env
        .run(&["drafts", "list", "--project", &project])
        .expect(0)
        .stdout();
    assert!(text.contains(&id) && text.contains("RS_NewRifle"), "{text}");
    env.run(&[
        "drafts",
        "set",
        &id,
        "--project",
        &project,
        "--set",
        "mass=3.25",
    ])
    .expect(0);
    let draft = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(draft["spec"]["mass"]["value"], json!(3.25));
    let gone = env
        .run(&["--json", "drafts", "delete", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(gone["deleted"], json!(true));
}

#[test]
fn new_from_a_reference_copies_its_numbers_as_anchor_values() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_Copy",
            "--project",
            &project,
            "--from",
            "RS_Gun03",
        ])
        .expect(0)
        .json();
    let spec = &out["draft"]["spec"];
    assert_eq!(spec["ranged"]["range"]["source"], json!("anchor"));
    assert_eq!(spec["techLevel"], json!("industrial"));
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_Copy2",
        "--project",
        &project,
        "--from",
        "RS_Nothing",
    ])
    .expect(1);
}

#[test]
fn new_requires_an_existing_project_and_valid_settings() {
    let env = Env::new(false);
    env.select_install();
    let missing = env.path("projects/RS_NotThere");
    let run = env
        .run(&[
            "new",
            "ranged",
            "--name",
            "RS_X",
            "--project",
            path_str(&missing),
        ])
        .expect(1);
    assert!(run.stderr().contains("io.not-found"), "{}", run.stderr());
    let project = create_project(&env, "RS_Mine");
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_X",
        "--project",
        &project,
        "--set",
        "novalue",
    ])
    .expect(2);
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_X",
        "--project",
        &project,
        "--set",
        "ranged.damage=lots",
    ])
    .expect(1);
}

#[test]
fn preview_shows_readouts_suggestions_and_diagnostics() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let text = env
        .run(&["preview", &id, "--project", &project])
        .expect(0)
        .stdout();
    assert!(text.contains("Exact readouts"), "{text}");
    assert!(text.contains("dps"));
    assert!(text.contains("Suggestions"));
    let json = env
        .run(&["--json", "preview", &id, "--project", &project])
        .expect(0)
        .json();
    assert!(
        json["readouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["key"] == json!("dps"))
    );
    // an incomplete draft previews with diagnostics and completes with exit code 3
    let bare = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_Bare",
            "--project",
            &project,
        ])
        .expect(0)
        .json();
    let bare_id = bare["id"].as_str().unwrap();
    env.run(&["preview", bare_id, "--project", &project])
        .expect(3);
}

#[test]
fn a_draft_file_can_be_previewed_without_a_project() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let draft = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .stdout();
    let file = env.path("draft.json");
    std::fs::write(&file, draft).unwrap();
    let text = env
        .run(&["preview", path_str(&file), "--set", "ranged.damage=20"])
        .expect(0)
        .stdout();
    assert!(text.contains("Preview of RS_NewRifle"), "{text}");
    env.run(&["preview", &id]).expect(2);
}

#[test]
fn plan_lists_the_vanilla_file_only_and_writes_nothing() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let before = files_under(Path::new(&project));
    let out = env
        .run(&["--json", "plan", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(kinds(&out["plan"]), vec!["vanilla-defs"]);
    assert_eq!(out["ce"], json!(false));
    let text = env
        .run(&["plan", &id, "--project", &project])
        .expect(0)
        .stdout();
    assert!(
        text.contains(&format!(
            "{project}/Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"
        )),
        "{text}"
    );
    assert!(text.contains("Nothing was written"));
    assert_eq!(files_under(Path::new(&project)), before);
}

#[test]
fn plan_and_apply_need_an_explicit_project_folder() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let draft = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .stdout();
    let file = env.path("draft.json");
    std::fs::write(&file, draft).unwrap();
    env.run(&["plan", path_str(&file)]).expect(2);
    env.run(&["apply", path_str(&file), "--yes"]).expect(2);
}

#[test]
fn apply_without_yes_prints_the_files_and_writes_nothing() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let before = files_under(Path::new(&project));
    let text = env
        .run(&["apply", &id, "--project", &project])
        .expect(0)
        .stdout();
    assert!(text.contains("Nothing was written"), "{text}");
    assert!(text.contains("--yes"));
    assert!(text.contains("RS_NewRifle.xml"));
    let json = env
        .run(&["--json", "apply", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(json["applied"], json!(false));
    assert_eq!(files_under(Path::new(&project)), before);
}

#[test]
fn apply_with_yes_writes_the_vanilla_definition_and_no_combat_extended_file() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let game_before = game_files(&env);
    let text = env
        .run(&["apply", &id, "--project", &project, "--yes"])
        .expect(0)
        .stdout();
    assert!(text.contains("wrote "), "{text}");
    assert!(text.contains("verified"));
    let files = files_under(Path::new(&project));
    assert!(
        files.contains(&"Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml".to_owned()),
        "{files:?}"
    );
    assert!(
        files
            .iter()
            .all(|f| !f.starts_with("Compat/CombatExtended/") && f != "LoadFolders.xml"),
        "no Combat Extended file without --ce: {files:?}"
    );
    let def = std::fs::read_to_string(
        Path::new(&project).join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"),
    )
    .unwrap();
    assert!(def.contains("RS_NewRifle"));
    assert!(!def.contains("Combat"), "{def}");
    assert_eq!(
        game_files(&env),
        game_before,
        "the install is never written"
    );
    // a second apply changes nothing
    let again = env
        .run(&["--json", "apply", &id, "--project", &project, "--yes"])
        .expect(0)
        .json();
    assert!(
        again["report"]["written"].as_array().unwrap().is_empty(),
        "{again}"
    );
}

#[test]
fn combat_extended_settings_in_a_draft_are_ignored_without_ce() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let mut args = vec!["--json", "apply", &id, "--project", &project, "--yes"];
    args.extend(CE_SETS);
    let out = env.run(&args).expect(0).json();
    assert_eq!(kinds(&out["plan"]), vec!["vanilla-defs"]);
    let files = files_under(Path::new(&project));
    assert!(
        files
            .iter()
            .all(|f| !f.starts_with("Compat/CombatExtended/")),
        "{files:?}"
    );
}

#[test]
fn ce_adds_a_gated_patch_file_and_the_load_folders_edit() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let mut args = vec![
        "--json",
        "plan",
        &id,
        "--project",
        &project,
        "--ce",
        "--set",
        "ce.shotSpread=0.08",
    ];
    args.extend(CE_SETS);
    let plan = env.run(&args).expect(3).json();
    let k = kinds(&plan["plan"]);
    assert!(
        k.contains(&"vanilla-defs".to_owned())
            && k.contains(&"ce-patch".to_owned())
            && k.contains(&"load-folders".to_owned()),
        "{k:?}"
    );
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()]
    );

    let mut args = vec![
        "apply",
        &id,
        "--project",
        &project,
        "--ce",
        "--yes",
        "--set",
        "ce.shotSpread=0.08",
    ];
    args.extend(CE_SETS);
    // the unknown tag of the fictional Combat Extended is a warning: exit code 3
    let text = env.run(&args).expect(3).stdout();
    assert!(text.contains("dry run of the patch: ok"), "{text}");
    let files = files_under(Path::new(&project));
    let patch = files
        .iter()
        .find(|f| f.starts_with("Compat/CombatExtended/"))
        .unwrap_or_else(|| panic!("{files:?}"));
    assert!(files.contains(&"LoadFolders.xml".to_owned()));
    assert!(
        files.contains(&"Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml".to_owned())
    );
    let load = std::fs::read_to_string(Path::new(&project).join("LoadFolders.xml")).unwrap();
    assert!(load.contains("ceteam.combatextended"), "{load}");
    let def = std::fs::read_to_string(
        Path::new(&project).join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"),
    )
    .unwrap();
    assert!(
        !def.contains("Combat"),
        "the vanilla definition is never mixed with Combat Extended: {def}"
    );
    let patch_text = std::fs::read_to_string(Path::new(&project).join(patch)).unwrap();
    assert!(patch_text.contains("RS_NewRifle"), "{patch_text}");
}

#[test]
fn a_plan_with_errors_exits_1_and_apply_writes_nothing() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let bare = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_Bare",
            "--project",
            &project,
        ])
        .expect(0)
        .json();
    let id = bare["id"].as_str().unwrap();
    let plan = env.run(&["plan", id, "--project", &project]).expect(1);
    assert!(
        plan.stdout().contains("design.required-missing"),
        "{}",
        plan.stdout()
    );
    let applied = env
        .run(&["apply", id, "--project", &project, "--yes"])
        .expect(1);
    assert!(
        applied.stdout().contains("nothing was written"),
        "{}",
        applied.stdout()
    );
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()]
    );
    // the same draft with --ce and incomplete Combat Extended settings also fails without writing
    let full = new_rifle(&env, &project, "RS_Full");
    env.run(&["apply", &full, "--project", &project, "--ce", "--yes"])
        .expect(1);
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()]
    );
}

#[test]
fn a_melee_weapon_can_be_designed_and_written() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = env
        .run(&[
            "--json",
            "new",
            "melee",
            "--name",
            "RS_NewBlade",
            "--project",
            &project,
            "--strength",
            "typical",
            "--set",
            "tier=medieval",
            "--set",
            "role=RS_Melee",
            "--set",
            "parent=RS_BaseMelee",
            "--set",
            "workToMake=4000",
            "--set",
            "mass=1.1",
            "--set",
            "costList=RS_Steel:20",
            "--set",
            "tools.0.capacities=Cut",
            "--set",
            "tools.0.power=9",
            "--set",
            "tools.0.cooldownTime=2",
        ])
        .expect(0)
        .json();
    let id = out["id"].as_str().unwrap();
    let plan = env
        .run(&["--json", "plan", id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(kinds(&plan["plan"]), vec!["vanilla-defs"]);
    env.run(&["apply", id, "--project", &project, "--yes"])
        .expect(0);
    let files = files_under(Path::new(&project));
    assert!(files.iter().any(|f| f.contains("RS_NewBlade")), "{files:?}");
}

#[test]
fn quiz_answers_from_a_file_are_stored_in_the_draft() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let answers = env.path("answers.json");
    std::fs::write(&answers, r#"[{"kind": "use-what-i-have"}]"#).unwrap();
    let out = env
        .run(&[
            "--json",
            "quiz",
            &id,
            "--project",
            &project,
            "--answers",
            path_str(&answers),
        ])
        .expect(0)
        .json();
    assert_eq!(out["answered"], json!(1));
    assert_eq!(out["id"], json!(id));
    let draft = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(draft["calibration"], json!("quiz"));
    assert!(
        draft["answers"].as_object().is_some_and(|a| !a.is_empty()),
        "{draft}"
    );
}

#[test]
fn quiz_without_a_terminal_or_answers_is_a_usage_error() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    env.run(&["quiz", &id, "--project", &project]).expect(2);
}

#[test]
fn quiz_with_too_few_answers_stops_with_exit_code_3() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let answers = env.path("none.json");
    std::fs::write(&answers, "[]").unwrap();
    let run = env
        .run(&[
            "quiz",
            &id,
            "--project",
            &project,
            "--answers",
            path_str(&answers),
        ])
        .expect(3);
    assert!(run.stdout().contains("stopped early"), "{}", run.stdout());
}

// ---------------------------------------------------------------------------------------------------
// the automatic conversion of an existing mod
// ---------------------------------------------------------------------------------------------------

#[test]
fn convert_scan_lists_the_weapons_of_a_mod_with_their_questions() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let json = env
        .run(&["--json", "convert", "scan", "--project", p])
        .expect(0)
        .json();
    assert_eq!(json["counts"]["notConverted"], json!(1));
    assert_eq!(json["candidates"][0]["defName"], json!("RS_ProjGun"));
    assert!(!json["candidates"][0]["asks"].as_array().unwrap().is_empty());
    let text = env
        .run(&["designer", "convert", "scan", "--project", p])
        .expect(0)
        .stdout();
    assert!(
        text.contains("RS_ProjGun") && text.contains("Questions for RS_ProjGun"),
        "{text}"
    );
}

#[test]
fn convert_scan_without_combat_extended_fails_with_the_reason() {
    let env = Env::new(false);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let run = env
        .run(&["--json", "convert", "scan", "--project", path_str(&proj)])
        .expect(1);
    assert_eq!(error_code(&run), "designer.reference-unavailable");
}

const CONVERT_ANSWERS: [&str; 18] = [
    "--set",
    "ammoSet=RS_AmmoSetA",
    "--set",
    "weaponTagClass=RS_CE_Class",
    "--set",
    "oneHanded=false",
    "--set",
    "beltFed=false",
    "--set",
    "overrides.bulk=6.5",
    "--set",
    "overrides.swayFactor=1.2",
    "--set",
    "overrides.shotSpread=0.08",
    "--set",
    "overrides.magazineSize=30",
    "--set",
    "overrides.reloadTime=4",
];

#[test]
fn convert_plan_reports_open_questions_and_writes_nothing() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let before = files_under(&proj);
    let run = env
        .run(&["convert", "plan", "--project", p, "--def", "RS_ProjGun"])
        .expect(1);
    assert!(
        run.stdout().contains("designer.convert-needs-answer"),
        "{}",
        run.stdout()
    );
    assert!(run.stdout().contains("--set"));
    assert!(
        run.stdout().contains("asked because"),
        "a number question says why it is asked: {}",
        run.stdout()
    );
    let mut args = vec!["--json", "convert", "plan", "--project", p, "--all"];
    args.extend(CONVERT_ANSWERS);
    let plan = env.run(&args).expect(3).json();
    let k = kinds(&plan["results"][0]["plan"]);
    assert!(
        k.contains(&"ce-patch".to_owned()) && k.contains(&"load-folders".to_owned()),
        "{k:?}"
    );
    assert_eq!(files_under(&proj), before);
}

#[test]
fn convert_apply_writes_a_gated_patch_and_leaves_the_definitions_alone() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let defs_before = std::fs::read_to_string(proj.join("Defs/Guns.xml")).unwrap();
    let mut dry = vec!["convert", "apply", "--project", p, "--all"];
    dry.extend(CONVERT_ANSWERS);
    let preview = env.run(&dry).expect(3).stdout();
    assert!(preview.contains("Nothing was written"), "{preview}");
    assert_eq!(
        files_under(&proj),
        vec!["About/About.xml".to_owned(), "Defs/Guns.xml".to_owned()]
    );

    let mut yes = dry.clone();
    yes.push("--yes");
    let text = env.run(&yes).expect(3).stdout();
    assert!(text.contains("wrote "), "{text}");
    let files = files_under(&proj);
    assert!(
        files
            .iter()
            .any(|f| f.starts_with("Compat/CombatExtended/")),
        "{files:?}"
    );
    assert!(files.contains(&"LoadFolders.xml".to_owned()));
    assert_eq!(
        std::fs::read_to_string(proj.join("Defs/Guns.xml")).unwrap(),
        defs_before
    );
    let scan = env
        .run(&["--json", "convert", "scan", "--project", p])
        .expect(0)
        .json();
    assert_eq!(scan["counts"]["alreadyCe"], json!(1));
    // converting again finds nothing to do
    env.run(&["convert", "plan", "--project", p, "--all"])
        .expect(1);
}

#[test]
fn convert_needs_a_selection_and_reads_answers_from_a_file() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    env.run(&["convert", "plan", "--project", p]).expect(2);
    env.run(&[
        "convert",
        "plan",
        "--project",
        p,
        "--all",
        "--def",
        "RS_ProjGun",
    ])
    .expect(2);
    let answers = env.path("answers.json");
    std::fs::write(
        &answers,
        r#"{"default": {"ammoSet": "RS_AmmoSetA", "weaponTagClass": "RS_CE_Class", "oneHanded": false, "beltFed": false,
            "overrides": {"bulk": {"value": 6.5, "source": "typed"}}},
           "defs": {"RS_ProjGun": {"overrides": {"swayFactor": {"value": 1.2, "source": "typed"},
                    "shotSpread": {"value": 0.08, "source": "typed"}, "magazineSize": {"value": 30, "source": "typed"},
                    "reloadTime": {"value": 4, "source": "typed"}}}}}"#,
    )
    .unwrap();
    // the per definition overrides merge into the default ones, so every answer is present
    let out = env
        .run(&[
            "--json",
            "convert",
            "plan",
            "--project",
            p,
            "--def",
            "RS_ProjGun",
            "--answers",
            path_str(&answers),
        ])
        .expect(3)
        .json();
    assert!(
        kinds(&out["results"][0]["plan"]).contains(&"ce-patch".to_owned()),
        "{out}"
    );
}

const GROUP_ANSWERS: &str = r#"{"ammoSet": "RS_AmmoSetA", "weaponTagClass": "RS_CE_Class",
    "oneHanded": false, "beltFed": false,
    "overrides": {"bulk": {"value": 6.5}, "swayFactor": {"value": 1.2}, "shotSpread": {"value": 0.08},
                  "magazineSize": {"value": 30}, "reloadTime": {"value": 4}}}"#;

#[test]
fn convert_scan_prints_the_family_key_and_a_family_group_answers_the_weapon() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let scan = env
        .run(&["--json", "convert", "scan", "--project", p])
        .expect(0)
        .json();
    let gun = scan["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["defName"] == json!("RS_ProjGun"))
        .unwrap();
    let family = gun["family"].as_str().unwrap().to_owned();
    assert!(family.starts_with("ranged/"), "{family}");
    let text = env
        .run(&["convert", "scan", "--project", p])
        .expect(0)
        .stdout();
    assert!(text.contains(&family), "{text}");
    let answers = env.path("groups.json");
    std::fs::write(
        &answers,
        format!(r#"{{"groups": [{{"family": "{family}", "answers": {GROUP_ANSWERS}}}]}}"#),
    )
    .unwrap();
    let out = env
        .run(&[
            "--json",
            "convert",
            "plan",
            "--project",
            p,
            "--def",
            "RS_ProjGun",
            "--answers",
            path_str(&answers),
        ])
        .expect(3)
        .json();
    assert!(
        kinds(&out["results"][0]["plan"]).contains(&"ce-patch".to_owned()),
        "{out}"
    );
    // a group of another family leaves the questions open
    std::fs::write(
        &answers,
        format!(
            r#"{{"groups": [{{"family": "ranged/Nothing/Here", "answers": {GROUP_ANSWERS}}}]}}"#
        ),
    )
    .unwrap();
    let run = env
        .run(&[
            "convert",
            "plan",
            "--project",
            p,
            "--def",
            "RS_ProjGun",
            "--answers",
            path_str(&answers),
        ])
        .expect(1);
    assert!(run.stdout().contains("designer.convert-needs-answer"));
}

#[test]
fn convert_groups_can_name_definitions_and_the_default_comes_first() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let answers = env.path("named.json");
    std::fs::write(
        &answers,
        format!(
            r#"{{"default": {{"ammoSet": "RS_Wrong"}},
                "groups": [{{"defNames": ["RS_ProjGun"], "answers": {GROUP_ANSWERS}}}]}}"#
        ),
    )
    .unwrap();
    // the group wins over the default, so the ammo set is the right one and the plan is complete
    let out = env
        .run(&[
            "--json",
            "convert",
            "plan",
            "--project",
            p,
            "--def",
            "RS_ProjGun",
            "--answers",
            path_str(&answers),
        ])
        .expect(3)
        .json();
    assert!(
        kinds(&out["results"][0]["plan"]).contains(&"ce-patch".to_owned()),
        "{out}"
    );
}

#[test]
fn a_malformed_answer_group_is_a_usage_error() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let answers = env.path("bad.json");
    for text in [
        r#"{"groups": [{"answers": {"oneHanded": false}}]}"#,
        r#"{"groups": [{"family": "x"}]}"#,
        r#"{"groups": [{"family": "x", "answers": {}, "extra": 1}]}"#,
        r#"{"groups": {"family": "x"}}"#,
    ] {
        std::fs::write(&answers, text).unwrap();
        env.run(&[
            "convert",
            "plan",
            "--project",
            p,
            "--def",
            "RS_ProjGun",
            "--answers",
            path_str(&answers),
        ])
        .expect(2);
    }
}

#[test]
fn the_help_of_convert_and_designer_plan_documents_the_answer_shapes() {
    let env = Env::new(false);
    let convert = env.run(&["convert", "plan", "--help"]).expect(0).stdout();
    for needle in [
        "toolPenetration",
        r#""blunt": { "value": 2.0 }"#,
        r#""overrides": { "bulk": { "value": 6.5 }"#,
        "toolPenetration.0.tool=edge",
        "groups",
        "family",
    ] {
        assert!(convert.contains(needle), "missing {needle}: {convert}");
    }
    let designer = env.run(&["designer", "plan", "--help"]).expect(0).stdout();
    assert!(
        designer.contains("ce.toolPenetration.0.tool=edge"),
        "{designer}"
    );
    assert!(designer.contains("N in ce.toolPenetration.N.tool"));
}

#[test]
fn lint_ce_checks_converted_mods_and_reports_clean_ones() {
    let env = Env::new(true);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let p = path_str(&proj);
    let mut yes = vec!["convert", "apply", "--project", p, "--all", "--yes"];
    yes.extend(CONVERT_ANSWERS);
    env.run(&yes).expect(3);
    let json = env.run(&["--json", "lint", "ce", p]).expect(0).json();
    assert_eq!(json["errors"], json!(0));
    let text = env.run(&["lint", "ce", p]).expect(0).stdout();
    assert!(text.contains("0 errors"), "{text}");
    env.run(&["lint", "ce"]).expect(2);
}

// ---------------------------------------------------------------------------------------------------
// the def explorer
// ---------------------------------------------------------------------------------------------------

#[test]
fn defs_search_and_resolve_read_the_reference_session() {
    let env = Env::new(false);
    env.select_install();
    let page = env
        .run(&["--json", "defs", "search", "RS_Gun01"])
        .expect(0)
        .json();
    assert!(page["total"].as_u64().unwrap() >= 1, "{page}");
    let text = env
        .run(&["defs", "search", "RS_Gun01", "--type", "ThingDef"])
        .expect(0)
        .stdout();
    assert!(text.contains("RS_Gun01"), "{text}");
    let text = env
        .run(&["defs", "resolve", "ThingDef", "RS_Gun01"])
        .expect(0)
        .stdout();
    assert!(text.contains("defName: RS_Gun01"), "{text}");
    assert!(text.contains("statBases"));
    let json = env
        .run(&["--json", "defs", "resolve", "ThingDef", "RS_Gun01"])
        .expect(0)
        .json();
    assert_eq!(json["defName"], json!("RS_Gun01"));
    env.run(&["defs", "search", "x", "--session", "nope"])
        .expect(1);
}

#[test]
fn defs_can_be_searched_in_a_project_session() {
    let env = Env::new(false);
    env.select_install();
    let proj = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let page = env
        .run(&[
            "--json",
            "defs",
            "search",
            "RS_ProjGun",
            "--project",
            path_str(&proj),
        ])
        .expect(0)
        .json();
    assert!(page["total"].as_u64().unwrap() >= 1, "{page}");
}

// ---------------------------------------------------------------------------------------------------
// invariants over whole flows
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_whole_session_never_writes_into_the_install_or_next_to_the_project() {
    let env = Env::new(true);
    let game_before = game_files(&env);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    env.run(&["preview", &id, "--project", &project]).expect(0);
    env.run(&["plan", &id, "--project", &project]).expect(0);
    env.run(&["apply", &id, "--project", &project, "--yes"])
        .expect(0);
    env.run(&["calibrate", "ranged"]).expect(0);
    assert_eq!(game_files(&env), game_before);
    let siblings = files_under(&env.path("projects"));
    assert!(
        siblings.iter().all(|f| f.starts_with("RS_Mine/")),
        "{siblings:?}"
    );
}

#[test]
fn json_output_parses_for_every_friendly_command() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let conv = env.mod_folder("RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let calls: Vec<Vec<&str>> = vec![
        vec!["detect"],
        vec!["sources", "list"],
        vec!["scan"],
        vec!["settings", "get"],
        vec!["refs", "ranged"],
        vec!["calibrate", "melee"],
        vec!["drafts", "list", "--project", &project],
        vec!["preview", &id, "--project", &project],
        vec!["plan", &id, "--project", &project],
        vec!["apply", &id, "--project", &project],
        vec!["convert", "scan", "--project", path_str(&conv)],
        vec!["defs", "search", "RS_"],
        vec!["project", "open", &project],
        vec!["commands"],
        vec!["version"],
    ];
    for call in calls {
        let mut args = vec!["--json"];
        args.extend(call.iter().copied());
        let run = env.run(&args);
        assert!(
            matches!(run.code(), 0 | 3),
            "{call:?}: {}\n{}",
            run.stdout(),
            run.stderr()
        );
        let _ = run.json();
    }
}

// ---------------------------------------------------------------------------------------------------
// a real install (ignored by default; read only)
// ---------------------------------------------------------------------------------------------------

/// Runs against the folders named by RIMSTUDIO_GAME_DIR (and RIMSTUDIO_CE_DIR when set). Nothing is
/// written outside the temporary home. Run with `cargo test -p rimstudio-cli -- --ignored`.
#[test]
#[ignore = "reads a real RimWorld install named by RIMSTUDIO_GAME_DIR"]
fn a_real_install_detects_scans_and_lists_references() {
    let Some(game) = std::env::var_os("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; nothing to do");
        return;
    };
    let env = Env::new(false);
    let game = game.to_string_lossy().into_owned();
    let run = |args: &[&str]| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_rimstudio-cli"));
        cmd.args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &env.home)
            .env("XDG_CONFIG_HOME", env.home.join(".config"))
            .env("XDG_DATA_HOME", env.home.join(".local/share"))
            .env("XDG_STATE_HOME", env.home.join(".local/state"))
            .env("XDG_CACHE_HOME", env.home.join(".cache"));
        Run {
            output: cmd.output().unwrap(),
        }
    };
    run(&["detect", "--install", &game]).expect(0);
    let scan = run(&["--json", "scan"]);
    assert!(matches!(scan.code(), 0 | 3), "{}", scan.stderr());
    let refs = run(&["--json", "refs", "ranged", "--limit", "3"])
        .expect(0)
        .json();
    assert!(refs["total"].as_u64().unwrap() > 0);
}

// ---------------------------------------------------------------------------------------------------
// Combat Extended suggestions
// ---------------------------------------------------------------------------------------------------

/// The Combat Extended settings of a user who typed every number and the choices except the default
/// projectile, which follows the ammo set.
const CE_ANSWERS: [&str; 14] = [
    "--set",
    "ce.ammoSet=RS_AmmoSetA",
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
];

#[test]
fn ce_suggest_reports_without_turning_the_patch_on_or_writing_anything() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let text = env
        .run(&["designer", "ce-suggest", &id, "--project", &project])
        .expect(0)
        .stdout();
    assert!(
        text.contains("patch toggle off; nothing here turns it on"),
        "{text}"
    );
    assert!(text.contains("/ce/ammoSet"), "{text}");
    assert!(text.contains("Nothing was changed"), "{text}");
    let doc = env
        .run(&["--json", "ce-suggest", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(doc["available"], json!(true));
    assert_eq!(doc["toggleOn"], json!(false));
    assert_eq!(doc["kind"], json!("ranged"));
    let choices = doc["choices"].as_array().unwrap();
    let ammo = choices
        .iter()
        .find(|c| c["field"] == "/ce/ammoSet")
        .unwrap();
    assert_eq!(ammo["status"], json!("ask"));
    assert_eq!(ammo["candidates"][0]["name"], json!("RS_AmmoSetA"));
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()],
        "a suggestion writes nothing"
    );
    // the draft is unchanged: it still has no Combat Extended block
    let shown = env
        .run(&["--json", "drafts", "show", &id, "--project", &project])
        .expect(0)
        .json();
    assert!(shown.to_string().contains("RS_NewRifle"));
    assert!(!shown.to_string().contains("\"ce\""), "{shown}");
}

#[test]
fn ce_suggest_without_combat_extended_prints_the_plain_reason_with_exit_3() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let run = env
        .run(&["designer", "ce-suggest", &id, "--project", &project])
        .expect(3);
    assert!(
        run.stdout()
            .contains("No Combat Extended suggestions for RS_NewRifle"),
        "{}",
        run.stdout()
    );
    let doc = env
        .run(&["--json", "ce-suggest", &id, "--project", &project])
        .expect(3)
        .json();
    assert_eq!(doc["available"], json!(false));
    assert!(doc["reason"].as_str().unwrap().contains("Combat Extended"));
}

#[test]
fn accepting_suggestions_fills_the_projectile_and_marks_it_while_typed_values_stay() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    // without the option the projectile is missing and the plan fails
    let mut args = vec!["--json", "plan", &id, "--project", &project, "--ce"];
    args.extend(CE_ANSWERS);
    env.run(&args).expect(1);
    // with it the plan is complete
    let mut args = vec![
        "--json",
        "plan",
        &id,
        "--project",
        &project,
        "--ce",
        "--accept-suggestions",
    ];
    args.extend(CE_ANSWERS);
    let run = env.run(&args);
    assert!([0, 3].contains(&run.code()), "{}", run.stdout());
    let out = run.json();
    assert_eq!(out["plan"]["hasErrors"], json!(false), "{out}");
    assert!(kinds(&out["plan"]).contains(&"ce-patch".to_owned()));
    let diags = out["plan"]["diagnostics"].as_array().unwrap();
    assert!(
        diags.iter().any(|d| d["code"] == "ce.derived-value"
            && d["field"] == "/ce/defaultProjectile"
            && d["message"]
                .as_str()
                .unwrap()
                .contains("Combat Extended suggestion")),
        "{diags:?}"
    );
    let patch = out["plan"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "ce-patch")
        .unwrap();
    let text = patch["rendered"].as_str().unwrap();
    assert!(text.contains("RS_CeBullet1"), "{text}");
    assert!(
        text.contains("<Bulk>6.5</Bulk>"),
        "the typed bulk is kept: {text}"
    );
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()],
        "plan writes nothing"
    );
}

#[test]
fn accepting_named_suggestions_reports_names_that_match_no_field() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let mut args = vec![
        "--json",
        "plan",
        &id,
        "--project",
        &project,
        "--ce",
        "--accept-suggestions",
        "defaultProjectile",
        "nonsense",
    ];
    args.extend(CE_ANSWERS);
    let run = env.run(&args);
    let out = run.json();
    assert_eq!(out["plan"]["hasErrors"], json!(false), "{out}");
    let diags = out["plan"]["diagnostics"].as_array().unwrap();
    assert!(
        diags
            .iter()
            .any(|d| d["code"] == "designer.ce-suggestion-skipped"
                && d["message"].as_str().unwrap().contains("nonsense")),
        "{diags:?}"
    );
    assert_eq!(run.code(), 3, "a skipped suggestion is a warning");
}

#[test]
fn the_ammo_set_is_never_chosen_for_the_user() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let args = [
        "--json",
        "apply",
        &id,
        "--project",
        &project,
        "--ce",
        "--accept-suggestions",
        "--yes",
        "--set",
        "ce.bulk=6.5",
    ];
    let run = env.run(&args).expect(1);
    let out = run.json();
    let diags = out["plan"]["diagnostics"].as_array().unwrap();
    assert!(
        diags
            .iter()
            .any(|d| d["code"] == "designer.ce-needs-answer" && d["field"] == "/ce/ammoSet"),
        "{diags:?}"
    );
    assert_eq!(
        files_under(Path::new(&project)),
        vec!["About/About.xml".to_owned()],
        "nothing is written while an answer is open"
    );
}

#[test]
fn apply_with_accepted_suggestions_writes_the_gated_patch() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let mut args = vec![
        "apply",
        &id,
        "--project",
        &project,
        "--ce",
        "--accept-suggestions",
        "--yes",
    ];
    args.extend(CE_ANSWERS);
    let run = env.run(&args);
    assert!([0, 3].contains(&run.code()), "{}", run.stdout());
    let files = files_under(Path::new(&project));
    let patch = files
        .iter()
        .find(|f| f.starts_with("Compat/CombatExtended/"))
        .unwrap();
    let text = std::fs::read_to_string(Path::new(&project).join(patch)).unwrap();
    assert!(text.contains("RS_CeBullet1"), "{text}");
    let def = std::fs::read_to_string(
        Path::new(&project).join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"),
    )
    .unwrap();
    assert!(!def.contains("Combat"), "{def}");
}

#[test]
fn accept_suggestions_needs_ce() {
    let env = Env::new(true);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    env.run(&["plan", &id, "--project", &project, "--accept-suggestions"])
        .expect(2);
}

// ---------------------------------------------------------------------------------------------------
// flow C: clone and adjust, diff, structure defaults
// ---------------------------------------------------------------------------------------------------

fn clone_of(env: &Env, project: &str, kind: &str, from: &str, name: &str) -> Value {
    env.run(&[
        "--json",
        "new",
        kind,
        "--name",
        name,
        "--project",
        project,
        "--from",
        from,
    ])
    .expect(0)
    .json()
}

#[test]
fn a_clone_copies_every_field_and_records_the_source() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let before_game = game_files(&env);
    let before_project = files_under(Path::new(&project));
    let out = clone_of(&env, &project, "ranged", "RS_Gun03", "RS_CopyGun");
    let draft = &out["draft"];
    let spec = &draft["spec"];
    assert_eq!(out["clonedFrom"], json!("RS_Gun03"));
    assert_eq!(draft["clonedFrom"], json!("RS_Gun03"));
    assert_eq!(draft["anchors"][0]["defName"], json!("RS_Gun03"));
    assert_eq!(draft["calibration"], json!("anchored"));
    assert_eq!(spec["identity"]["defName"], json!("RS_CopyGun"));
    assert_eq!(spec["parent"]["defName"], json!("RS_BaseGun"));
    assert_eq!(
        spec["ranged"]["projectile"],
        json!({"mode": "reference", "def": "RS_Shot03"})
    );
    assert_eq!(spec["costList"][0]["defName"], json!("RS_Steel"));
    assert_eq!(spec["weaponTags"], json!(["RS_Rifle"]));
    assert_eq!(spec["ranged"]["damage"]["source"], json!("anchor"));
    assert_eq!(spec["ranged"]["range"]["source"], json!("anchor"));
    assert_eq!(spec["mass"]["source"], json!("anchor"));
    assert_eq!(spec["techLevel"], json!("industrial"));
    assert!(spec.get("ce").is_none(), "a clone is vanilla: {spec}");
    assert_eq!(
        game_files(&env),
        before_game,
        "the game folders are never written"
    );
    assert_eq!(
        files_under(Path::new(&project)),
        before_project,
        "a clone only stores a draft"
    );
}

#[test]
fn the_text_after_a_clone_tells_the_next_step() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let run = env
        .run(&[
            "new",
            "ranged",
            "--name",
            "RS_CopyGun",
            "--project",
            &project,
            "--from",
            "RS_Gun03",
        ])
        .expect(0);
    let text = run.stdout();
    assert!(text.contains("cloned from RS_Gun03"), "{text}");
    assert!(text.contains("designer diff"), "{text}");
    assert!(text.contains("designer plan"), "{text}");
    assert!(text.contains("Combat Extended stays off"), "{text}");
}

#[test]
fn diff_lists_the_changed_fields_and_what_they_do_to_the_readouts() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = clone_of(&env, &project, "ranged", "RS_Gun03", "RS_CopyGun")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // untouched: nothing differs
    let none = env
        .run(&["--json", "diff", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(none["changes"], json!([]), "{none}");
    // a trial change is shown but not saved
    let trial = env
        .run(&[
            "--json",
            "diff",
            &id,
            "--project",
            &project,
            "--set",
            "ranged.damage=15",
            "--set",
            "ranged.range=30",
        ])
        .expect(0)
        .json();
    let fields: Vec<&str> = trial["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["field"].as_str().unwrap())
        .collect();
    assert_eq!(fields, vec!["/ranged/damage", "/ranged/range"], "{trial}");
    let dps = trial["readouts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"] == json!("dps"))
        .unwrap();
    assert!(dps["delta"].as_f64().unwrap() > 0.0, "{dps}");
    let again = env
        .run(&["--json", "diff", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(again["changes"], json!([]), "the trial was not saved");
    // the text view
    let text = env
        .run(&[
            "diff",
            &id,
            "--project",
            &project,
            "--set",
            "ranged.damage=15",
        ])
        .expect(0)
        .stdout();
    assert!(text.contains("Changed fields:"), "{text}");
    assert!(text.contains("/ranged/damage"), "{text}");
    assert!(text.contains("Effect on the exact readouts:"), "{text}");
    assert!(text.contains("dps"), "{text}");
    assert!(
        text.contains("RS_Shot03"),
        "the shared projectile is noted: {text}"
    );
}

#[test]
fn diff_of_a_draft_that_is_not_a_clone_fails_plainly() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let run = env
        .run(&["--json", "diff", &id, "--project", &project])
        .expect(1);
    assert_eq!(error_code(&run), "designer.invalid-draft");
}

#[test]
fn a_clone_of_the_wrong_kind_is_refused_and_leaves_no_draft_behind() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let run = env
        .run(&[
            "new",
            "ranged",
            "--name",
            "RS_CopyBlade",
            "--project",
            &project,
            "--from",
            "RS_Blade01",
        ])
        .expect(2);
    assert!(run.stderr().contains("melee"), "{}", run.stderr());
    let listed = env
        .run(&["--json", "drafts", "list", "--project", &project])
        .expect(0)
        .json();
    assert_eq!(listed["drafts"], json!([]));
}

#[test]
fn a_clone_needs_a_new_unused_name_and_a_loaded_source() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let used = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_Gun01",
            "--project",
            &project,
            "--from",
            "RS_Gun03",
        ])
        .expect(1);
    assert_eq!(error_code(&used), "designer.invalid-draft");
    let bad = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS bad name",
            "--project",
            &project,
            "--from",
            "RS_Gun03",
        ])
        .expect(1);
    assert_eq!(error_code(&bad), "designer.invalid-draft");
    let gone = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_Fresh",
            "--project",
            &project,
            "--from",
            "RS_Nothing",
        ])
        .expect(1);
    assert_eq!(error_code(&gone), "designer.reference-unavailable");
}

#[test]
fn from_and_strength_do_not_go_together_and_prefix_needs_from() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_X",
        "--project",
        &project,
        "--from",
        "RS_Gun03",
        "--strength",
        "typical",
    ])
    .expect(2);
    env.run(&[
        "new",
        "ranged",
        "--name",
        "RS_X",
        "--project",
        &project,
        "--prefix",
        "RS",
    ])
    .expect(2);
}

#[test]
fn the_prefix_and_settings_apply_to_a_clone() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "CopyGun",
            "--project",
            &project,
            "--from",
            "RS_Gun03",
            "--prefix",
            "RSM",
            "--set",
            "ranged.damage=21",
        ])
        .expect(0)
        .json();
    let spec = &out["draft"]["spec"];
    assert_eq!(spec["identity"]["defName"], json!("RSM_CopyGun"));
    assert_eq!(spec["identity"]["modPrefix"], json!("RSM"));
    assert_eq!(
        spec["ranged"]["damage"],
        json!({"value": 21, "source": "typed"})
    );
    // the stored draft has the setting too
    let id = out["id"].as_str().unwrap();
    let shown = env
        .run(&["--json", "drafts", "show", id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(shown["spec"]["ranged"]["damage"]["value"], json!(21.0));
}

#[test]
fn a_clone_can_be_planned_and_written_into_the_project_only() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = clone_of(&env, &project, "ranged", "RS_Gun03", "RS_CopyGun")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let plan = env
        .run(&["--json", "plan", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(kinds(&plan["plan"]), vec!["vanilla-defs"]);
    let text = plan["plan"]["files"][0]["rendered"].as_str().unwrap();
    assert!(text.contains("ParentName=\"RS_BaseGun\""), "{text}");
    assert!(
        text.contains("<defaultProjectile>RS_Shot03</defaultProjectile>"),
        "{text}"
    );
    let before_game = game_files(&env);
    env.run(&["apply", &id, "--project", &project, "--yes"])
        .expect(0);
    let written = std::fs::read_to_string(
        Path::new(&project).join("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_CopyGun.xml"),
    )
    .unwrap();
    assert!(!written.contains("Combat"), "{written}");
    assert_eq!(game_files(&env), before_game);
}

#[test]
fn a_melee_clone_copies_its_tools() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = clone_of(&env, &project, "melee", "RS_Blade02", "RS_CopyBlade");
    let tools = out["draft"]["spec"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["capacities"], json!(["Cut"]));
    assert_eq!(tools[0]["power"]["source"], json!("anchor"));
    assert_eq!(
        out["draft"]["spec"]["parent"]["defName"],
        json!("RS_BaseMelee")
    );
}

#[test]
fn clones_of_one_source_are_identical_apart_from_the_draft_id() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let a = clone_of(&env, &project, "ranged", "RS_Gun03", "RS_CopyGun");
    let b = clone_of(&env, &project, "ranged", "RS_Gun03", "RS_CopyGun");
    assert_ne!(a["id"], b["id"]);
    assert_eq!(a["draft"], b["draft"]);
}

#[test]
fn a_new_weapon_with_a_strength_gets_a_suggested_structure_with_a_plain_note() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_NewRifle",
            "--project",
            &project,
            "--strength",
            "typical",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
        ])
        .expect(0)
        .json();
    let spec = &out["draft"]["spec"];
    assert_eq!(spec["parent"]["defName"], json!("RS_BaseGun"));
    assert!(
        spec["ranged"]["projectile"]["def"]
            .as_str()
            .unwrap()
            .starts_with("RS_Shot")
    );
    assert_eq!(spec["costList"][0]["defName"], json!("RS_Steel"));
    let notes = out["notes"].to_string();
    assert!(
        notes.contains("suggestions") && notes.contains("not typed values"),
        "{notes}"
    );
    let text = env
        .run(&[
            "new",
            "ranged",
            "--name",
            "RS_NewRifle2",
            "--project",
            &project,
            "--strength",
            "typical",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
        ])
        .expect(0)
        .stdout();
    assert!(
        text.contains("note:") && text.contains("suggestions"),
        "{text}"
    );
}

#[test]
fn typed_structure_is_never_replaced_by_the_defaults() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_NewRifle",
            "--project",
            &project,
            "--strength",
            "typical",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
            "--set",
            "parent=RS_BaseBullet",
            "--set",
            "costList=RS_Part:2",
            "--set",
            "ranged.projectile=RS_Shot00",
        ])
        .expect(0)
        .json();
    let spec = &out["draft"]["spec"];
    assert_eq!(spec["parent"]["defName"], json!("RS_BaseBullet"));
    assert_eq!(spec["costList"][0]["defName"], json!("RS_Part"));
    assert_eq!(spec["ranged"]["projectile"]["def"], json!("RS_Shot00"));
}

#[test]
fn a_new_weapon_with_a_strength_can_now_be_planned_with_only_the_work_left_to_type() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_NewRifle",
            "--project",
            &project,
            "--strength",
            "typical",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
            "--set",
            "workToMake=9000",
        ])
        .expect(0)
        .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let plan = env
        .run(&["--json", "plan", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(plan["plan"]["hasErrors"], json!(false), "{plan}");
}

#[test]
fn the_clone_command_help_names_the_clone_and_diff_commands() {
    let env = Env::new(false);
    let help = env.run(&["new", "--help"]).expect(0).stdout();
    assert!(help.contains("--from") && help.contains("Clone"), "{help}");
    let top = env.run(&["diff", "--help"]).expect(0).stdout();
    assert!(top.contains("source"), "{top}");
}
