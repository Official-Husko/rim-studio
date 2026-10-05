//! The quiz through the toolkit: the dialogue lives in the draft, typed values survive it (IT-003) and the
//! stored answers round trip (IT-026).
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::model::{CalibrationMode, DesignSpec, ScalarField, Sourced, ValueSource};
use rimstudio_ipc_types::designer::{
    BucketDto, DesignerQuizAnswerRequest, DesignerQuizBackRequest, DesignerQuizNextRequest,
    DraftDto, PromptDto, QuestionDto, QuizAnswerDto, QuizStepDto,
};
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{quiz_answer, quiz_back, quiz_next};

fn quiz_draft(spec: DesignSpec) -> DraftDto {
    let mut draft = rimstudio_design::model::Draft::new(spec);
    draft.calibration = CalibrationMode::Quiz;
    draft_to_dto(&draft).unwrap()
}

fn next(f: &common::Fixture, draft: &DraftDto) -> QuizStepDto {
    quiz_next(
        &f.ctx,
        DesignerQuizNextRequest {
            draft: draft.clone(),
        },
    )
    .unwrap()
}

/// A modder with no particular weapon in mind: a fixed answer for every kind of question.
fn answer_for(prompt: &PromptDto) -> QuizAnswerDto {
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

fn play(f: &common::Fixture, mut draft: DraftDto) -> (DraftDto, usize) {
    let mut asked = 0;
    while asked < 40 {
        let step = next(f, &draft);
        let Some(prompt) = step.prompt else {
            assert!(step.finished);
            return (draft, asked);
        };
        let resp = quiz_answer(
            &f.ctx,
            DesignerQuizAnswerRequest {
                draft,
                question_id: prompt.id.clone(),
                answer: answer_for(&prompt),
            },
        )
        .unwrap();
        draft = resp.draft;
        asked += 1;
    }
    panic!("the quiz did not finish");
}

#[test]
fn the_setup_questions_are_answered_from_the_spec() {
    let f = common::fixture(false);
    let step = next(&f, &quiz_draft(common::ranged_spec()));
    let prompt = step.prompt.unwrap();
    assert!(
        !matches!(
            prompt.question,
            QuestionDto::Tier { .. } | QuestionDto::Role { .. }
        ),
        "tier and role are known: {:?}",
        prompt.question
    );
    assert_eq!(prompt.number, 1);
    assert_eq!(step.answered, 0);
    assert!(!step.finished);
    assert_eq!(
        step.implied.get("tier").map(String::as_str),
        Some("Industrial")
    );
    assert_eq!(
        step.implied.get("role").map(String::as_str),
        Some("RS_Rifle")
    );
    assert!(step.estimate.is_some());
}

#[test]
fn a_draft_without_tier_and_role_is_asked_for_them_first() {
    let f = common::fixture(false);
    let mut spec = common::ranged_spec();
    spec.tech_level = None;
    spec.role = None;
    let step = next(&f, &quiz_draft(spec));
    assert!(matches!(
        step.prompt.unwrap().question,
        QuestionDto::Tier { .. }
    ));
}

#[test]
fn answering_stores_the_answer_in_the_draft_and_the_dialogue_reopens_identically() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let step = next(&f, &draft);
    let prompt = step.prompt.unwrap();
    let resp = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft: draft.clone(),
            question_id: prompt.id.clone(),
            answer: answer_for(&prompt),
        },
    )
    .unwrap();
    assert_eq!(resp.step.answered, 1);
    assert!(resp.draft.answers.contains_key(&prompt.id));
    let entry = &resp.draft.answers[&prompt.id];
    assert_eq!(entry["seq"], 2, "two setup answers precede it");
    // the same draft asks the same next question, whatever process reads it
    let again = next(&f, &resp.draft);
    assert_eq!(
        serde_json::to_string(&again).unwrap(),
        serde_json::to_string(&resp.step).unwrap()
    );
}

#[test]
fn the_stepper_numbers_do_not_count_the_setup_answers() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let first = next(&f, &draft).prompt.unwrap();
    let resp = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft,
            question_id: first.id.clone(),
            answer: answer_for(&first),
        },
    )
    .unwrap();
    if let Some(p) = resp.step.prompt {
        assert_eq!(p.number, 2);
        assert!(p.about_total >= p.number);
    }
}

#[test]
fn a_full_dialogue_finishes_and_fills_the_spec_without_touching_typed_values() {
    let f = common::fixture(false);
    let mut spec = common::ranged_spec();
    spec.offer(ScalarField::Damage, 99.0, ValueSource::Typed);
    spec.offer(ScalarField::Mass, 9.9, ValueSource::Typed);
    let (draft_dto, asked) = play(&f, quiz_draft(spec));
    assert!(asked >= 2);
    let draft = draft_from_dto(&draft_dto).unwrap();
    assert_eq!(draft.calibration, CalibrationMode::Quiz);
    assert_eq!(
        draft.spec.scalar(ScalarField::Damage).unwrap(),
        Sourced::typed(99.0)
    );
    assert_eq!(
        draft.spec.scalar(ScalarField::Mass).unwrap(),
        Sourced::typed(9.9)
    );
    // the answers filled the empty fields with answered or suggested values
    let warmup = draft.spec.scalar(ScalarField::Warmup).unwrap();
    assert!(matches!(
        warmup.source,
        ValueSource::Answered | ValueSource::Suggested | ValueSource::Anchor
    ));
    assert!(draft.spec.ce.is_none());
}

#[test]
fn a_typed_value_given_as_a_quiz_answer_is_stored_as_typed() {
    let f = common::fixture(false);
    let mut draft = quiz_draft(common::ranged_spec());
    let mut typed_stat = None;
    for _ in 0..40 {
        let step = next(&f, &draft);
        let Some(prompt) = step.prompt else { break };
        let answer = match &prompt.question {
            QuestionDto::Interval { stat, .. } | QuestionDto::VsAnchor { stat, .. }
                if typed_stat.is_none() =>
            {
                typed_stat = Some(stat.clone());
                QuizAnswerDto::Typed { value: 4.25 }
            }
            _ => answer_for(&prompt),
        };
        draft = quiz_answer(
            &f.ctx,
            DesignerQuizAnswerRequest {
                draft,
                question_id: prompt.id,
                answer,
            },
        )
        .unwrap()
        .draft;
    }
    let stat = typed_stat.expect("the dialogue asks a stat question");
    let field = match stat.as_str() {
        "mass" => ScalarField::Mass,
        "range" => ScalarField::Range,
        "warmup" => ScalarField::Warmup,
        "cooldown" => ScalarField::Cooldown,
        other => panic!("unexpected stat {other}"),
    };
    let spec = draft_from_dto(&draft).unwrap().spec;
    assert_eq!(spec.scalar(field).unwrap(), Sourced::typed(4.25));
}

#[test]
fn back_removes_the_last_user_answer_but_never_the_setup() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let first = next(&f, &draft).prompt.unwrap();
    let resp = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft: draft.clone(),
            question_id: first.id.clone(),
            answer: answer_for(&first),
        },
    )
    .unwrap();
    let back = quiz_back(&f.ctx, DesignerQuizBackRequest { draft: resp.draft }).unwrap();
    assert_eq!(back.step.answered, 0);
    assert_eq!(back.step.prompt.unwrap().id, first.id);
    assert!(!back.draft.answers.contains_key(&first.id));
    // nothing but the setup answers is left: Back has nothing to undo
    let err = quiz_back(&f.ctx, DesignerQuizBackRequest { draft: back.draft }).unwrap_err();
    assert_eq!(err.code(), "designer.quiz-wrong-answer");
}

#[test]
fn an_answer_for_another_question_is_refused() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let err = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft,
            question_id: "cmp:RS_NoSuchGun".into(),
            answer: QuizAnswerDto::Weaker,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.quiz-wrong-answer");
}

#[test]
fn an_answer_that_does_not_fit_the_open_question_is_refused_and_changes_nothing() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let prompt = next(&f, &draft).prompt.unwrap();
    let wrong = match prompt.question {
        QuestionDto::Compare { .. } => QuizAnswerDto::Bin { index: 0 },
        _ => QuizAnswerDto::Weaker,
    };
    let err = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft: draft.clone(),
            question_id: prompt.id,
            answer: wrong,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.quiz-wrong-answer");
    assert!(draft.answers.is_empty());
}

#[test]
fn answering_a_finished_quiz_reports_it() {
    let f = common::fixture(false);
    let (finished, _) = play(&f, quiz_draft(common::ranged_spec()));
    let err = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft: finished,
            question_id: "cmp:x".into(),
            answer: QuizAnswerDto::Skip,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.quiz-finished");
}

#[test]
fn use_what_i_have_finishes_the_quiz_at_any_point() {
    let f = common::fixture(false);
    let draft = quiz_draft(common::ranged_spec());
    let prompt = next(&f, &draft).prompt.unwrap();
    let resp = quiz_answer(
        &f.ctx,
        DesignerQuizAnswerRequest {
            draft,
            question_id: prompt.id,
            answer: QuizAnswerDto::UseWhatIHave,
        },
    )
    .unwrap();
    assert!(resp.step.finished);
    assert!(resp.step.prompt.is_none());
}

#[test]
fn an_unreadable_stored_answer_is_an_invalid_draft() {
    let f = common::fixture(false);
    let mut draft = quiz_draft(common::ranged_spec());
    draft.answers.insert(
        "cmp:RS_Gun01".into(),
        serde_json::json!({"seq": 0, "answer": {"kind": "no-such-kind"}}),
    );
    let err = quiz_next(&f.ctx, DesignerQuizNextRequest { draft }).unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
}

#[test]
fn a_stale_answer_is_dropped_instead_of_misapplied() {
    let f = common::fixture(false);
    let mut draft = quiz_draft(common::ranged_spec());
    draft.answers.insert(
        "cmp:RS_Vanished".into(),
        serde_json::json!({"seq": 5, "answer": {"kind": "weaker"}}),
    );
    let step = next(&f, &draft);
    assert_eq!(step.answered, 0, "the stale answer does not count");
    assert!(step.prompt.is_some());
}

#[test]
fn a_pool_with_fewer_than_eight_weapons_has_no_quiz() {
    let install = common::install_custom(3, 8, false, false);
    let session = common::open_session_with(&install, &["ludeon.rimworld"]);
    let tmp = tempfile::tempdir().unwrap();
    let base = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = rimstudio_io::roots::DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = rimstudio_testing::fakes::FakeClock::new(1_700_000_000_000);
    let ctx =
        rimstudio_toolkit::designer::Ctx::new(session, &roots, std::sync::Arc::new(clock)).unwrap();
    let err = quiz_next(
        &ctx,
        DesignerQuizNextRequest {
            draft: quiz_draft(common::ranged_spec()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.calibration-unavailable");
}

#[test]
fn the_quiz_needs_a_game_install() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = quiz_next(
        &ctx,
        DesignerQuizNextRequest {
            draft: quiz_draft(common::ranged_spec()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}
