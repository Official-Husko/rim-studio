//! Drafts in the document store: save, list, load, delete, and the exact round trip (IT-015).
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::model::{
    Anchor, CalibrationMode, CePatchSpec, Draft, ScalarField, Sourced, ValueSource,
};
use rimstudio_ipc_types::designer::{
    DesignerDraftDeleteRequest, DesignerDraftListRequest, DesignerDraftSaveRequest, DraftDto,
};
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{draft_delete, draft_list, draft_load, draft_save};

fn full_draft() -> DraftDto {
    let mut spec = common::typed_ranged();
    spec.offer(ScalarField::BurstCount, 3.0, ValueSource::Answered);
    spec.offer(
        ScalarField::TicksBetweenBurstShots,
        8.0,
        ValueSource::Suggested,
    );
    spec.ce = Some(CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        magazine_size: Some(Sourced::typed(30)),
        one_handed: true,
        ..CePatchSpec::default()
    });
    let mut draft = Draft::new(spec);
    draft.calibration = CalibrationMode::Quiz;
    draft.answers.insert(
        "cmp:RS_Gun03".into(),
        serde_json::json!({"seq": 2, "answer": {"kind": "stronger"}}),
    );
    draft.answers.insert(
        "tier".into(),
        serde_json::json!({"seq": 0, "answer": {"kind": "tier", "tier": 2}, "auto": true}),
    );
    draft.anchors.push(Anchor::new("RS_Gun07"));
    draft_to_dto(&draft).unwrap()
}

fn save(f: &common::Fixture, project: &str, id: Option<&str>, draft: DraftDto) -> String {
    draft_save(
        &f.ctx,
        DesignerDraftSaveRequest {
            project_id: project.into(),
            id: id.map(str::to_owned),
            draft,
        },
    )
    .unwrap()
    .id
}

#[test]
fn a_saved_draft_comes_back_identical_with_answers_anchors_and_the_ce_toggle() {
    let f = common::fixture(false);
    let draft = full_draft();
    let id = save(&f, "p-test0001", None, draft.clone());
    let entry = draft_load(&f.ctx, "p-test0001", &id).unwrap();
    assert_eq!(entry.draft, draft);
    assert_eq!(entry.id, id);
    assert_eq!(entry.def_name, "RS_NewRifle");
    assert_eq!(entry.label, "new rifle");
    assert_eq!(
        entry.updated_at_ms,
        rimstudio_testing::fakes::FakeClock::DEFAULT_START_MS
    );
    let back = draft_from_dto(&entry.draft).unwrap();
    assert!(back.ce_patch_enabled());
    assert_eq!(back.answers.len(), 2);
    assert_eq!(back.anchors, vec![Anchor::new("RS_Gun07")]);
}

#[test]
fn a_new_draft_has_the_ce_toggle_off_and_it_stays_off() {
    let f = common::fixture(true);
    let draft = common::dto(common::ranged_spec());
    let id = save(&f, "p-test0001", None, draft);
    let entry = draft_load(&f.ctx, "p-test0001", &id).unwrap();
    assert!(
        entry.draft.spec.ce.is_none(),
        "CE is installed and the toggle is still off"
    );
}

#[test]
fn saving_with_an_id_overwrites_and_without_one_creates() {
    let f = common::fixture(false);
    let a = save(&f, "p-test0001", None, common::dto(common::ranged_spec()));
    f.clock.advance(5_000);
    let again = save(&f, "p-test0001", Some(&a), full_draft());
    assert_eq!(again, a);
    let list = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: "p-test0001".into(),
        },
    )
    .unwrap();
    assert_eq!(list.drafts.len(), 1);
    assert_eq!(list.drafts[0].draft, full_draft());
    assert_eq!(
        list.drafts[0].updated_at_ms,
        rimstudio_testing::fakes::FakeClock::DEFAULT_START_MS + 5_000
    );
    let b = save(&f, "p-test0001", None, common::dto(common::melee_spec()));
    assert_ne!(a, b);
}

#[test]
fn the_list_is_per_project_newest_first_then_by_id() {
    let f = common::fixture(false);
    let first = save(&f, "p-one", None, common::dto(common::ranged_spec()));
    f.clock.advance(1_000);
    let second = save(&f, "p-one", None, common::dto(common::melee_spec()));
    let other = save(&f, "p-two", None, common::dto(common::ranged_spec()));
    let one = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: "p-one".into(),
        },
    )
    .unwrap();
    let ids: Vec<&str> = one.drafts.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, vec![second.as_str(), first.as_str()]);
    let two = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: "p-two".into(),
        },
    )
    .unwrap();
    assert_eq!(two.drafts.len(), 1);
    assert_eq!(two.drafts[0].id, other);
    let none = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: "p-three".into(),
        },
    )
    .unwrap();
    assert!(none.drafts.is_empty());
}

#[test]
fn delete_removes_the_draft_and_reports_unknown_ids() {
    let f = common::fixture(false);
    let id = save(&f, "p-one", None, common::dto(common::ranged_spec()));
    let gone = draft_delete(
        &f.ctx,
        DesignerDraftDeleteRequest {
            project_id: "p-one".into(),
            id: id.clone(),
        },
    )
    .unwrap();
    assert!(gone.deleted);
    let again = draft_delete(
        &f.ctx,
        DesignerDraftDeleteRequest {
            project_id: "p-one".into(),
            id: id.clone(),
        },
    )
    .unwrap();
    assert!(!again.deleted);
    let err = draft_load(&f.ctx, "p-one", &id).unwrap_err();
    assert_eq!(err.code(), "designer.draft-not-found");
}

#[test]
fn drafts_live_in_the_app_data_root_and_never_in_a_mod_folder() {
    let f = common::fixture(false);
    let id = save(&f, "p-one", None, common::dto(common::ranged_spec()));
    let doc = f
        .roots
        .data
        .join("designer-drafts")
        .join(format!("p-one.{id}.json"));
    assert!(doc.is_file(), "{doc}");
    // nothing was written below the fictional install
    let mut found = Vec::new();
    for entry in std::fs::read_dir(f.install.root.as_std_path())
        .unwrap()
        .flatten()
    {
        found.push(entry.file_name());
    }
    assert!(!f.install.root.join("designer-drafts").exists());
    assert!(!found.is_empty());
}

#[test]
fn bad_ids_and_inconsistent_drafts_are_invalid_drafts() {
    let f = common::fixture(false);
    for project in ["", "Has.Dot", "UPPER", "a b"] {
        let err = draft_save(
            &f.ctx,
            DesignerDraftSaveRequest {
                project_id: project.into(),
                id: None,
                draft: common::dto(common::ranged_spec()),
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "designer.invalid-draft", "{project:?}");
    }
    let mut draft = common::dto(common::ranged_spec());
    draft.kind = rimstudio_ipc_types::designer::ItemKindDto::Melee;
    let err = draft_save(
        &f.ctx,
        DesignerDraftSaveRequest {
            project_id: "p-one".into(),
            id: None,
            draft,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
}

#[test]
fn a_draft_from_a_newer_build_is_refused_on_save() {
    let f = common::fixture(false);
    let mut draft = common::dto(common::ranged_spec());
    draft.schema_version = 99;
    let err = draft_save(
        &f.ctx,
        DesignerDraftSaveRequest {
            project_id: "p-one".into(),
            id: None,
            draft,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.draft-newer-schema");
}

#[test]
fn drafts_work_without_a_game_install() {
    let (_tmp, ctx, _clock) = common::offline();
    let resp = draft_save(
        &ctx,
        DesignerDraftSaveRequest {
            project_id: "p-one".into(),
            id: None,
            draft: full_draft(),
        },
    )
    .unwrap();
    let entry = draft_load(&ctx, "p-one", &resp.id).unwrap();
    assert_eq!(entry.draft, full_draft());
}

#[test]
fn minted_ids_are_unique_and_well_formed() {
    let f = common::fixture(false);
    let mut ids = Vec::new();
    for _ in 0..20 {
        ids.push(save(&f, "p-one", None, common::dto(common::ranged_spec())));
    }
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 20);
    assert!(ids.iter().all(|i| i.starts_with("d-") && i.len() == 12));
}

#[test]
fn a_stored_draft_of_a_newer_build_is_never_overwritten() {
    use rimstudio_io::collection::Collection;
    use rimstudio_io::roots::RootKind;
    use rimstudio_toolkit::designer::drafts::DraftRecord;

    let f = common::fixture(false);
    let store = Collection::<DraftRecord>::open(
        &f.roots,
        RootKind::Data,
        "designer-drafts",
        std::sync::Arc::new(f.clock.clone()),
    )
    .unwrap();
    let mut draft = Draft::new(common::ranged_spec());
    draft.schema_version = 99;
    let record = DraftRecord {
        project_id: "p-one".into(),
        id: "d-newer".into(),
        updated_at_ms: 1,
        draft,
    };
    store.put(&record.doc_id(), &record).unwrap();
    let err = draft_save(
        &f.ctx,
        DesignerDraftSaveRequest {
            project_id: "p-one".into(),
            id: Some("d-newer".into()),
            draft: common::dto(common::ranged_spec()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.draft-newer-schema");
    let still = store.get(&record.doc_id()).unwrap().unwrap();
    assert_eq!(still.value.draft.schema_version, 99);
}
