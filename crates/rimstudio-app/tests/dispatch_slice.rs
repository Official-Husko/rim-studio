//! Dispatch of every command of the 0.1.0 slice against an application booted over a fictional install
//! (real temporary folders, fake ports, no network, no real home folder).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::collections::BTreeSet;

use common::{Fixture, draft_json, fixture, project_folder, ranged_spec, select_install};
use rimstudio_app::registry::{self, ROUTES};
use rimstudio_app::{dispatch, dispatch_blocking};
use rimstudio_design::model::{CalibrationMode, CePatchSpec, Draft, Sourced};
use rimstudio_ipc_types::designer::{
    BucketDto, PromptDto, QuestionDto, QuizAnswerDto, QuizStepDto,
};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_toolkit::designer::dto::draft_to_dto;
use serde_json::{Value, json};

struct Run<'a> {
    f: &'a Fixture,
    seen: BTreeSet<String>,
}

impl<'a> Run<'a> {
    fn new(f: &'a Fixture) -> Self {
        Self {
            f,
            seen: BTreeSet::new(),
        }
    }

    fn call(&mut self, name: &str, req: Value) -> Result<Value, ApiError> {
        self.seen.insert(name.to_owned());
        dispatch_blocking(&self.f.app, name, req)
    }

    fn ok(&mut self, name: &str, req: Value) -> Value {
        self.call(name, req)
            .unwrap_or_else(|e| panic!("{name} failed: {e}"))
    }

    fn err(&mut self, name: &str, req: Value) -> ApiError {
        match self.call(name, req) {
            Ok(v) => panic!("{name} should have failed, answered {v}"),
            Err(e) => e,
        }
    }
}

fn s<'v>(v: &'v Value, key: &str) -> &'v str {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no string {key} in {v}"))
}

fn ce_block() -> CePatchSpec {
    CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        default_projectile: Some("RS_CeBullet1".into()),
        weapon_tag_class: Some("RS_CE_Class".into()),
        magazine_size: Some(Sourced::typed(30)),
        reload_time: Some(Sourced::typed(4.0)),
        bulk: Some(Sourced::typed(6.5)),
        sway_factor: Some(Sourced::typed(1.2)),
        shot_spread: Some(Sourced::typed(0.08)),
        ..CePatchSpec::default()
    }
}

fn quiz_draft() -> Value {
    let mut draft = Draft::new(ranged_spec());
    draft.calibration = CalibrationMode::Quiz;
    serde_json::to_value(draft_to_dto(&draft).unwrap()).unwrap()
}

fn quiz_answer_for(prompt: &PromptDto) -> QuizAnswerDto {
    match &prompt.question {
        QuestionDto::Tier { options } => QuizAnswerDto::Tier {
            tier: options[0].tier,
        },
        QuestionDto::Role { options } => QuizAnswerDto::Role {
            role: options[0].name.clone(),
        },
        QuestionDto::Compare { .. } => QuizAnswerDto::Stronger,
        QuestionDto::CloserTo { .. } => QuizAnswerDto::CloserToLower,
        QuestionDto::Group { options } => QuizAnswerDto::Group {
            group: options[0].name.clone(),
        },
        QuestionDto::Interval { .. } => QuizAnswerDto::Bin { index: 1 },
        QuestionDto::VsAnchor { .. } => QuizAnswerDto::Bucket {
            bucket: BucketDto::Similar,
        },
    }
}

#[test]
fn every_command_of_the_slice_dispatches_in_one_session() {
    let f = fixture(true);
    let mut run = Run::new(&f);

    // application
    let pong = run.ok("app_ping", json!({"echo": "hello"}));
    assert_eq!(s(&pong, "echo"), "hello");
    let info = run.ok("app_get_info", Value::Null);
    assert_eq!(info["commandCount"], json!(ROUTES.len()));
    assert_eq!(s(&info, "contractHash").len(), 16);
    assert_eq!(info["portable"], json!(true));
    let tools = run.ok("app_list_tools", json!({}));
    let designer = &tools["tools"][0];
    assert_eq!(designer["id"], json!("designer"));
    assert_eq!(
        designer["available"],
        json!(false),
        "no install is selected yet"
    );

    // settings
    let settings = run.ok("settings_get", json!({}));
    let rev = settings["rev"].as_u64().unwrap();
    assert_eq!(settings["appearance"]["density"], json!("standard"));
    let events = f.app.events.subscribe();
    let updated = run.ok(
        "settings_update",
        json!({"expectedRev": rev, "appearance": {"density": "compact"}}),
    );
    assert_eq!(updated["appearance"]["density"], json!("compact"));
    assert_ne!(updated["rev"].as_u64().unwrap(), rev);
    let event = events.try_recv().unwrap();
    assert_eq!(event.name(), "settings:changed");
    let on_disk = std::fs::read_to_string(f.base.join("app/config/settings.jsonc")).unwrap();
    assert!(on_disk.contains("compact"), "{on_disk}");
    let stale = run.err(
        "settings_update",
        json!({"expectedRev": rev, "appearance": {"density": "touch"}}),
    );
    assert_eq!(stale.code, "settings.revision-conflict");

    // detection
    let before = run.ok("detect_get_report", json!({}));
    assert!(
        before.get("report").is_none(),
        "nothing was detected yet: {before}"
    );
    let detected = run.ok("detect_run", json!({"force": true}));
    assert_eq!(
        detected["installs"],
        json!([]),
        "the fake machine has no Steam"
    );
    let overridden = run.ok(
        "detect_set_override",
        json!({"field": "game-install", "path": f.install.game_dir.as_str()}),
    );
    assert_eq!(
        overridden["report"]["installs"][0]["kind"],
        json!("override")
    );
    let bad = run.err(
        "detect_set_override",
        json!({"field": "game-install", "path": f.base.join("nowhere").as_str()}),
    );
    assert_eq!(bad.code, "detect.failed");
    let tools = run.ok("app_list_tools", json!({}));
    assert_eq!(tools["tools"][0]["available"], json!(true), "{tools}");

    // sources
    let custom = f.base.join("custom_mods");
    let mod_a = custom.join("RS_ModA");
    std::fs::create_dir_all(mod_a.join("About").as_std_path()).unwrap();
    std::fs::write(
        mod_a.join("About/About.xml").as_std_path(),
        common::about("rs.moda"),
    )
    .unwrap();
    let probe = run.ok("sources_probe_folder", json!({"path": custom.as_str()}));
    assert_eq!(probe["canSave"], json!(true), "{probe}");
    assert_eq!(probe["kind"], json!("mods-root"));
    let added = run.ok(
        "sources_add_folder",
        json!({"path": custom.as_str(), "label": "RS Custom"}),
    );
    let source_id = s(&added, "id").to_owned();
    assert_eq!(s(&added, "kind"), "custom");
    let listed = run.ok("sources_list", json!({}));
    let ids: Vec<&str> = listed["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| s(v, "id"))
        .collect();
    assert!(
        ids.contains(&"game-data") && ids.contains(&"game-mods"),
        "{ids:?}"
    );
    assert!(ids.contains(&source_id.as_str()));
    let disabled = run.ok("sources_update", json!({"id": source_id, "enabled": false}));
    assert_eq!(s(&disabled, "status"), "disabled");
    let removed = run.ok("sources_remove", json!({"id": source_id}));
    assert_eq!(removed["removed"], json!(true));
    let again = run.ok("sources_remove", json!({"id": source_id}));
    assert_eq!(again["removed"], json!(false));

    // library
    let scan = run.ok("library_scan", json!({}));
    assert_eq!(scan["rev"], json!(1));
    assert!(scan["stats"]["modsFound"].as_u64().unwrap() >= 2, "{scan}");
    assert_eq!(scan["cancelled"], json!(false));
    let present = rimstudio_app::tools::present_capabilities(&f.app);
    assert!(
        present.contains(&rimstudio_ipc_types::tools::Capability::CombatExtended),
        "{present:?}"
    );
    assert!(present.contains(&rimstudio_ipc_types::tools::Capability::GameInstall));
    assert!(!present.contains(&rimstudio_ipc_types::tools::Capability::OpenProject));

    // def explorer
    let page = run.ok(
        "defs_search",
        json!({"sessionId": "", "query": "RS_Gun01", "queryId": "q1"}),
    );
    assert!(page["total"].as_u64().unwrap() >= 1, "{page}");
    assert_eq!(s(&page, "queryId"), "q1");
    let resolved = run.ok(
        "defs_get_resolved",
        json!({"sessionId": "reference", "defType": "ThingDef", "defName": "RS_Gun01"}),
    );
    assert_eq!(s(&resolved, "defName"), "RS_Gun01");
    assert!(resolved["tree"].is_object());

    // projects
    let created = run.ok(
        "project_create",
        json!({
            "path": f.base.join("projects/RS_Created").as_str(),
            "name": "RS Created",
            "packageId": "rs.created",
            "author": "RS Author",
        }),
    );
    let pid = s(&created, "projectId").to_owned();
    assert_eq!(s(&created, "packageId"), "rs.created");
    assert!(f.base.join("projects/RS_Created/About/About.xml").is_file());
    assert!(
        rimstudio_app::tools::present_capabilities(&f.app)
            .contains(&rimstudio_ipc_types::tools::Capability::OpenProject)
    );
    let conv_root = project_folder(&f, "RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let opened = run.ok("project_open", json!({"path": conv_root.as_str()}));
    let conv_id = s(&opened, "projectId").to_owned();
    assert_eq!(opened["defFiles"], json!(1));
    let closed = run.ok("project_close", json!({"projectId": conv_id.clone()}));
    assert_eq!(closed["closed"], json!(true));
    run.ok("project_open", json!({"path": conv_root.as_str()}));

    // designer: references, readouts, fit, quiz
    let refs = run.ok("designer_reference_list", json!({"kind": "ranged"}));
    assert!(refs["items"].as_array().unwrap().len() >= 10, "{refs}");
    let draft = draft_json(&ranged_spec());
    let preview = run.ok("designer_preview", json!({"draft": draft.clone()}));
    assert!(preview["readouts"].is_array());
    let fit = run.ok("designer_fit", json!({"draft": draft.clone()}));
    assert!(fit.is_object());
    // the Combat Extended suggestion of a draft whose toggle is off: a report, the toggle stays off
    let suggestion = run.ok("designer_ce_suggest", json!({"draft": draft.clone()}));
    assert_eq!(suggestion["toggleOn"], json!(false), "{suggestion}");
    assert!(suggestion["available"].is_boolean());
    let mut quiz = quiz_draft();
    let step: QuizStepDto =
        serde_json::from_value(run.ok("designer_quiz_next", json!({"draft": quiz.clone()})))
            .unwrap();
    let prompt = step.prompt.expect("the quiz opens with a question");
    let answered = run.ok(
        "designer_quiz_answer",
        json!({
            "draft": quiz.clone(),
            "questionId": prompt.id,
            "answer": serde_json::to_value(quiz_answer_for(&prompt)).unwrap(),
        }),
    );
    assert!(answered["step"]["answered"].as_u64().unwrap() >= 1);
    quiz = answered["draft"].clone();
    assert!(quiz.is_object());
    let backed = run.ok("designer_quiz_back", json!({"draft": quiz.clone()}));
    assert!(
        backed["step"]["answered"].as_u64().unwrap()
            < answered["step"]["answered"].as_u64().unwrap()
    );
    assert_eq!(backed["step"]["prompt"]["id"], json!(prompt.id));

    // designer: calibration job
    let calibrated = run.ok("designer_calibrate", json!({"kind": "ranged"}));
    assert!(calibrated.is_object());

    // designer: drafts
    let saved = run.ok(
        "designer_draft_save",
        json!({"projectId": pid.clone(), "draft": draft.clone()}),
    );
    let draft_id = s(&saved, "id").to_owned();
    let list = run.ok("designer_draft_list", json!({"projectId": pid.clone()}));
    assert_eq!(list["drafts"].as_array().unwrap().len(), 1);
    let deleted = run.ok(
        "designer_draft_delete",
        json!({"projectId": pid.clone(), "id": draft_id}),
    );
    assert_eq!(deleted["deleted"], json!(true));

    // designer: clone and adjust, the diff against the source, the structure of a new weapon
    let cloned = run.ok(
        "designer_clone",
        json!({"projectId": pid.clone(), "source": "RS_Gun03", "defName": "RS_CloneGun03"}),
    );
    assert_eq!(
        cloned["entry"]["draft"]["clonedFrom"],
        json!("RS_Gun03"),
        "{cloned}"
    );
    assert!(
        cloned["entry"]["draft"]["spec"].get("ce").is_none(),
        "a clone is vanilla: {cloned}"
    );
    let mut clone_draft = cloned["entry"]["draft"].clone();
    let diff = run.ok("designer_clone_diff", json!({"draft": clone_draft.clone()}));
    assert_eq!(diff["source"], json!("RS_Gun03"));
    assert_eq!(diff["changes"], json!([]), "{diff}");
    clone_draft["spec"]["ranged"]["damage"] = json!({"value": 20.0, "source": "typed"});
    let diff = run.ok("designer_clone_diff", json!({"draft": clone_draft}));
    assert_eq!(
        diff["changes"][0]["field"],
        json!("/ranged/damage"),
        "{diff}"
    );
    let missing_source = run.err(
        "designer_clone",
        json!({"projectId": pid.clone(), "source": "RS_NoSuchGun", "defName": "RS_CloneX"}),
    );
    assert_eq!(missing_source.code, "designer.reference-unavailable");
    let not_a_clone = run.err("designer_clone_diff", json!({"draft": draft.clone()}));
    assert_eq!(not_a_clone.code, "designer.invalid-draft");
    let structured = run.ok(
        "designer_structure_defaults",
        json!({"draft": draft.clone()}),
    );
    assert!(structured["draft"].is_object(), "{structured}");

    // designer: vanilla plan and apply
    let request = json!({"projectId": pid.clone(), "draft": draft.clone()});
    let plan = run.ok("designer_export_plan", request.clone());
    let kinds: Vec<&str> = plan["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| s(v, "kind"))
        .collect();
    assert!(kinds.contains(&"vanilla-defs"), "{kinds:?}");
    assert!(
        !kinds.contains(&"ce-patch"),
        "Combat Extended is opt in: {kinds:?}"
    );
    assert_eq!(plan["hasErrors"], json!(false), "{plan}");
    let plan_id = s(&plan, "planId").to_owned();
    let stale = run.err(
        "designer_apply_plan",
        json!({"planId": "not-the-plan", "request": request.clone()}),
    );
    assert_eq!(stale.code, "designer.plan-stale");
    let applied = run.ok(
        "designer_apply_plan",
        json!({"planId": plan_id, "request": request.clone()}),
    );
    assert!(
        !applied["written"].as_array().unwrap().is_empty(),
        "{applied}"
    );
    let first = applied["written"][0]["path"].as_str().unwrap();
    assert!(f.base.join("projects/RS_Created").join(first).is_file());

    // designer: the optional Combat Extended patch, gated and in its own file
    let mut with_ce = ranged_spec();
    with_ce.def_name_for_test();
    with_ce.ce = Some(ce_block());
    let ce_plan = run.ok(
        "designer_export_plan",
        json!({"projectId": pid.clone(), "draft": draft_json(&with_ce)}),
    );
    let ce_kinds: Vec<&str> = ce_plan["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| s(v, "kind"))
        .collect();
    assert!(
        ce_kinds.contains(&"ce-patch") && ce_kinds.contains(&"load-folders"),
        "{ce_kinds:?}"
    );

    // designer: converting the weapons of an existing mod
    let scan = run.ok("designer_convert_scan", json!({"projectId": conv_id}));
    let candidates = scan["candidates"].as_array().unwrap();
    assert!(
        candidates
            .iter()
            .any(|c| c["defName"] == json!("RS_ProjGun")),
        "{scan}"
    );

    // jobs: the registry answers for a job started without a channel
    let handle = dispatch(&f.app, "library_scan", json!({"jobId": "scan-registry"})).unwrap();
    run.seen.insert("library_scan".to_owned());
    assert_eq!(s(&handle, "jobId"), "scan-registry");
    let terminal = f
        .app
        .jobs
        .wait_terminal("scan-registry", std::time::Duration::from_secs(60))
        .expect("the job ends");
    assert!(terminal.is_terminal());
    let status = run.ok("job_status", json!({"jobId": "scan-registry"}));
    assert_eq!(s(&status, "state"), "finished");
    assert_eq!(status["terminal"]["kind"], json!("finished"));
    let late = run.ok("cancel_job", json!({"jobId": "scan-registry"}));
    assert_eq!(s(&late, "state"), "finished");
    let unknown = run.ok("cancel_job", json!({"jobId": "nobody"}));
    assert_eq!(s(&unknown, "state"), "unknown");
    assert_eq!(
        s(&run.ok("job_status", json!({"jobId": "nobody"})), "state"),
        "unknown"
    );

    let all: BTreeSet<String> = ROUTES.iter().map(|r| r.name.to_owned()).collect();
    let missing: Vec<&String> = all.difference(&run.seen).collect();
    assert!(missing.is_empty(), "commands never dispatched: {missing:?}");
    assert_eq!(all.len(), registry::names().len());
}

trait SpecExt {
    fn def_name_for_test(&mut self);
}

impl SpecExt for rimstudio_design::model::DesignSpec {
    /// The CE plan lives in the same project as the vanilla one, so it gets its own def name.
    fn def_name_for_test(&mut self) {
        self.identity.def_name = "RS_NewRifleCe".into();
    }
}

#[test]
fn designer_commands_need_a_game_install_but_drafts_and_projects_do_not() {
    let f = fixture(false);
    let draft = draft_json(&ranged_spec());
    let e = dispatch_blocking(&f.app, "designer_reference_list", json!({"kind": "ranged"}))
        .unwrap_err();
    assert_eq!(e.code, "designer.reference-unavailable");
    let root = project_folder(&f, "RS_Offline", &[]);
    let opened = dispatch_blocking(&f.app, "project_open", json!({"path": root.as_str()})).unwrap();
    let pid = s(&opened, "projectId").to_owned();
    let saved = dispatch_blocking(
        &f.app,
        "designer_draft_save",
        json!({"projectId": pid, "draft": draft}),
    );
    assert!(saved.is_ok(), "{saved:?}");
}

#[test]
fn convert_scan_without_combat_extended_says_why() {
    let f = fixture(false);
    select_install(&f);
    let root = project_folder(&f, "RS_Conv", &[("Defs/Guns.xml", common::PROJECT_DEFS)]);
    let opened = dispatch_blocking(&f.app, "project_open", json!({"path": root.as_str()})).unwrap();
    let e = dispatch_blocking(
        &f.app,
        "designer_convert_scan",
        json!({"projectId": s(&opened, "projectId")}),
    )
    .unwrap_err();
    assert_eq!(e.code, "designer.reference-unavailable");
}

#[test]
fn unknown_sessions_are_named_in_the_error() {
    let f = fixture(false);
    select_install(&f);
    for session in ["nope", "ce"] {
        let e = dispatch_blocking(
            &f.app,
            "defs_search",
            json!({"sessionId": session, "query": "x"}),
        )
        .unwrap_err();
        assert_eq!(e.code, "defs.session-not-found", "{session}");
    }
    let e = dispatch_blocking(
        &f.app,
        "defs_get_resolved",
        json!({"sessionId": "", "defType": "ThingDef", "defName": "RS_Nothing"}),
    )
    .unwrap_err();
    assert_eq!(e.code, "defs.invalid-query");
}

#[test]
fn projects_are_never_created_inside_the_game_install() {
    let f = fixture(false);
    select_install(&f);
    let inside = f.install.game_dir.join("RS_Inside");
    let e = dispatch_blocking(
        &f.app,
        "project_create",
        json!({"path": inside.as_str(), "name": "RS Inside", "packageId": "rs.inside"}),
    )
    .unwrap_err();
    assert_eq!(e.code, "project.path-outside-root", "{e}");
    assert!(!inside.exists());
}

#[test]
fn creating_over_an_existing_mod_is_refused_and_opening_a_non_mod_too() {
    let f = fixture(false);
    let target = f.base.join("projects/RS_Twice");
    let req = json!({"path": target.as_str(), "name": "RS Twice", "packageId": "rs.twice"});
    dispatch_blocking(&f.app, "project_create", req.clone()).unwrap();
    let e = dispatch_blocking(&f.app, "project_create", req).unwrap_err();
    assert_eq!(e.code, "designer.apply-failed");
    let plain = f.base.join("projects/not_a_mod");
    std::fs::create_dir_all(plain.as_std_path()).unwrap();
    let e = dispatch_blocking(&f.app, "project_open", json!({"path": plain.as_str()})).unwrap_err();
    assert_eq!(e.code, "io.not-found");
}

#[test]
fn a_folder_inside_the_game_mods_folder_cannot_become_a_source() {
    let f = fixture(false);
    select_install(&f);
    let e = dispatch_blocking(
        &f.app,
        "sources_add_folder",
        json!({"path": f.install.mods_dir.as_str()}),
    )
    .unwrap_err();
    assert!(
        matches!(
            e.code.as_str(),
            "sources.overlap" | "sources.invalid-folder"
        ),
        "{e}"
    );
}

#[test]
fn an_invalid_settings_value_is_refused_and_the_file_stays_as_it_was() {
    let f = fixture(false);
    dispatch_blocking(&f.app, "settings_get", json!({})).unwrap();
    let path = f.base.join("app/config/settings.jsonc");
    let before = std::fs::read_to_string(&path).unwrap();
    let e = dispatch_blocking(
        &f.app,
        "settings_update",
        json!({"appearance": {"fontScale": 99.0}}),
    )
    .unwrap_err();
    assert_eq!(e.code, "settings.invalid");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}

#[test]
fn a_reset_key_returns_a_value_to_its_default() {
    let f = fixture(false);
    dispatch_blocking(
        &f.app,
        "settings_update",
        json!({"appearance": {"density": "touch"}}),
    )
    .unwrap();
    let reset = dispatch_blocking(
        &f.app,
        "settings_update",
        json!({"reset": ["appearance.density"]}),
    )
    .unwrap();
    assert_eq!(reset["appearance"]["density"], json!("standard"));
}

#[test]
fn request_errors_name_the_offending_field() {
    let f = fixture(false);
    let e = dispatch(
        &f.app,
        "settings_update",
        json!({"appearance": {"density": "huge"}}),
    )
    .unwrap_err();
    assert_eq!(e.code, "ipc.invalid-request");
    let field = e
        .details
        .as_ref()
        .and_then(|d| d.get("field"))
        .and_then(Value::as_str);
    assert_eq!(field, Some("appearance.density"));
    let e = dispatch(&f.app, "no_such_command", json!({})).unwrap_err();
    assert_eq!(e.code, "ipc.unknown-command");
    let e = rimstudio_app::dispatch_job(
        &f.app,
        "app_ping",
        json!({}),
        None,
        std::sync::Arc::new(rimstudio_app::jobs::NullSink),
    )
    .unwrap_err();
    assert_eq!(e.code, "ipc.invalid-request");
}

#[test]
fn a_job_with_a_bad_request_fails_before_any_job_exists() {
    let f = fixture(false);
    let e = dispatch(
        &f.app,
        "library_scan",
        json!({"full": "yes", "jobId": "bad-1"}),
    )
    .unwrap_err();
    assert_eq!(e.code, "ipc.invalid-request");
    assert!(f.app.jobs.status("bad-1").is_none());
}

#[test]
fn a_duplicate_job_id_is_rejected() {
    let f = fixture(false);
    select_install(&f);
    dispatch(&f.app, "library_scan", json!({"jobId": "dup-1"})).unwrap();
    let e = dispatch(&f.app, "library_scan", json!({"jobId": "dup-1"})).unwrap_err();
    assert_eq!(e.code, "job.duplicate-id");
    f.app.shutdown(std::time::Duration::from_secs(30));
}

#[test]
fn a_blocking_job_reports_started_progress_and_the_terminal_event_to_the_observer() {
    let f = fixture(false);
    select_install(&f);
    let mut kinds = Vec::new();
    let result = rimstudio_app::dispatch::dispatch_blocking_with(
        &f.app,
        "library_scan",
        json!({}),
        &mut |event| {
            kinds.push(match event {
                rimstudio_ipc_types::jobs::JobEvent::Started { .. } => "started",
                rimstudio_ipc_types::jobs::JobEvent::Progress { .. } => "progress",
                rimstudio_ipc_types::jobs::JobEvent::Finished { .. } => "finished",
                rimstudio_ipc_types::jobs::JobEvent::Failed { .. } => "failed",
                rimstudio_ipc_types::jobs::JobEvent::Cancelled { .. } => "cancelled",
            });
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(kinds.first(), Some(&"started"));
    assert_eq!(kinds.last(), Some(&"finished"));
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(**k, "finished" | "failed" | "cancelled"))
            .count(),
        1
    );
}

#[test]
fn a_scan_without_any_source_fails_with_the_library_code() {
    let f = fixture(false);
    // nothing is detected on the fake machine and no folder is registered
    let e = dispatch_blocking(&f.app, "library_scan", json!({})).unwrap_err();
    assert_eq!(e.code, "library.scan-failed");
}

#[test]
fn results_can_be_written_to_a_file_outside_the_app_folders_only() {
    let f = fixture(false);
    let out = f.base.join("out/result.json");
    let report =
        rimstudio_app::dispatch_to_file(&f.app, "app_ping", json!({"echo": "x"}), &out).unwrap();
    assert_eq!(report.path, out);
    let text = std::fs::read_to_string(&out).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["echo"], json!("x"));
    assert_eq!(report.bytes, text.len());
    let relative = rimstudio_app::dispatch_to_file(
        &f.app,
        "app_ping",
        json!({}),
        camino::Utf8Path::new("x.json"),
    );
    assert_eq!(relative.unwrap_err().code, "ipc.invalid-request");
    let inside = f.base.join("app/data/result.json");
    let refused = rimstudio_app::dispatch_to_file(&f.app, "app_ping", json!({}), &inside);
    assert_eq!(refused.unwrap_err().code, "ipc.invalid-request");
    assert!(!inside.exists());
    let escaping = f.base.join("out/../app/config/x.json");
    let refused = rimstudio_app::dispatch_to_file(&f.app, "app_ping", json!({}), &escaping);
    assert_eq!(refused.unwrap_err().code, "ipc.invalid-request");
}

#[test]
fn selecting_an_install_after_an_offline_start_rebuilds_the_sessions() {
    let f = fixture(false);
    let first = dispatch_blocking(&f.app, "designer_reference_list", json!({"kind": "ranged"}))
        .unwrap_err();
    assert_eq!(first.code, "designer.reference-unavailable");
    assert!(f.app.workspace.is_built(), "the offline context is kept");
    select_install(&f);
    let refs =
        dispatch_blocking(&f.app, "designer_reference_list", json!({"kind": "ranged"})).unwrap();
    assert!(refs["items"].as_array().unwrap().len() >= 10);
    let tools = dispatch_blocking(&f.app, "app_list_tools", json!({})).unwrap();
    assert_eq!(tools["tools"][0]["available"], json!(true));
}

#[test]
fn clearing_the_install_override_goes_back_to_offline() {
    let f = fixture(false);
    select_install(&f);
    dispatch_blocking(&f.app, "designer_reference_list", json!({"kind": "ranged"})).unwrap();
    dispatch_blocking(
        &f.app,
        "detect_set_override",
        json!({"field": "game-install"}),
    )
    .unwrap();
    let e = dispatch_blocking(&f.app, "designer_reference_list", json!({"kind": "ranged"}))
        .unwrap_err();
    assert_eq!(e.code, "designer.reference-unavailable");
}

#[test]
fn path_overrides_can_be_set_and_reset_through_the_settings_update() {
    let f = fixture(false);
    let set = dispatch_blocking(
        &f.app,
        "settings_update",
        json!({"paths": {"gameInstall": {"path": f.install.game_dir.as_str(), "pinned": true}}}),
    )
    .unwrap();
    assert_eq!(
        set["paths"]["gameInstall"]["path"],
        json!(f.install.game_dir.as_str())
    );
    let cleared = dispatch_blocking(
        &f.app,
        "settings_update",
        json!({"reset": ["paths.gameInstall"]}),
    )
    .unwrap();
    assert!(cleared["paths"].get("gameInstall").is_none(), "{cleared}");
}
