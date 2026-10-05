//! Properties of the toolkit path: typed values survive any dialogue and any fill (IT-003), the preview is
//! deterministic and a draft stored and loaded is the same draft.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::sync::LazyLock;

use proptest::prelude::*;
use rimstudio_design::model::{CalibrationMode, Draft, ScalarField, Sourced, ValueSource};
use rimstudio_ipc_types::designer::{
    BucketDto, DesignerDraftSaveRequest, DesignerPreviewRequest, DesignerQuizAnswerRequest,
    DesignerQuizNextRequest, DraftDto, PromptDto, QuestionDto, QuizAnswerDto,
};
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{
    draft_load, draft_save, preview, quiz_answer, quiz_next, suggest_fill,
};

static FIXTURE: LazyLock<common::Fixture> = LazyLock::new(|| common::fixture(false));

const TYPED: [ScalarField; 5] = [
    ScalarField::Damage,
    ScalarField::Mass,
    ScalarField::Range,
    ScalarField::Warmup,
    ScalarField::AccuracyShort,
];

fn typed_spec(values: &[f64; 5]) -> rimstudio_design::model::DesignSpec {
    let mut spec = common::ranged_spec();
    for (field, v) in TYPED.iter().zip(values) {
        spec.offer(*field, *v, ValueSource::Typed);
    }
    spec
}

fn typed_unchanged(draft: &DraftDto, values: &[f64; 5]) -> bool {
    let Ok(d) = draft_from_dto(draft) else {
        return false;
    };
    TYPED
        .iter()
        .zip(values)
        .all(|(f, v)| d.spec.scalar(*f) == Some(Sourced::typed(*v)))
}

/// Picks a valid answer for the open question from a choice number and a number.
fn pick(prompt: &PromptDto, choice: u8, number: f64) -> QuizAnswerDto {
    let c = usize::from(choice);
    match &prompt.question {
        QuestionDto::Tier { options } => QuizAnswerDto::Tier {
            tier: options[c % options.len()].tier,
        },
        QuestionDto::Role { options } => QuizAnswerDto::Role {
            role: options[c % options.len()].name.clone(),
        },
        QuestionDto::Group { options } => QuizAnswerDto::Group {
            group: options[c % options.len()].name.clone(),
        },
        QuestionDto::Compare { .. } => [
            QuizAnswerDto::Weaker,
            QuizAnswerDto::Same,
            QuizAnswerDto::Stronger,
            QuizAnswerDto::NotSure,
            QuizAnswerDto::Skip,
            QuizAnswerDto::UseWhatIHave,
        ][c % 6]
            .clone(),
        QuestionDto::CloserTo { .. } => [
            QuizAnswerDto::CloserToLower,
            QuizAnswerDto::CloserToUpper,
            QuizAnswerDto::Skip,
        ][c % 3]
            .clone(),
        QuestionDto::Interval { bins, .. } => match c % 3 {
            0 => QuizAnswerDto::Bin {
                index: u32::try_from(c % bins.len()).unwrap_or(0),
            },
            1 => QuizAnswerDto::Typed { value: number },
            _ => QuizAnswerDto::NotSure,
        },
        QuestionDto::VsAnchor { .. } => match c % 4 {
            0 => QuizAnswerDto::Bucket {
                bucket: BucketDto::Lower,
            },
            1 => QuizAnswerDto::Bucket {
                bucket: BucketDto::Higher,
            },
            2 => QuizAnswerDto::Typed { value: number },
            _ => QuizAnswerDto::Skip,
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn typed_values_survive_any_sequence_of_answers(
        values in prop::array::uniform5(1.0f64..200.0),
        steps in prop::collection::vec((any::<u8>(), 0.5f64..120.0), 1..14),
    ) {
        let f = &*FIXTURE;
        let values = values.map(|v| (v * 100.0).round() / 100.0);
        let mut draft = Draft::new(typed_spec(&values));
        draft.calibration = CalibrationMode::Quiz;
        let mut dto = draft_to_dto(&draft).unwrap();
        for (choice, number) in steps {
            let step = quiz_next(&f.ctx, DesignerQuizNextRequest { draft: dto.clone() }).unwrap();
            let Some(prompt) = step.prompt else { break };
            let answer = pick(&prompt, choice, number);
            // a typed number may be refused for a question it does not fit; every other answer is valid
            let resp = quiz_answer(&f.ctx, DesignerQuizAnswerRequest {
                draft: dto.clone(),
                question_id: prompt.id,
                answer,
            });
            if let Ok(r) = resp {
                dto = r.draft;
                prop_assert!(typed_unchanged(&dto, &values));
            }
        }
    }

    #[test]
    fn suggest_fill_keeps_typed_values_and_is_idempotent(
        values in prop::array::uniform5(1.0f64..200.0),
    ) {
        let f = &*FIXTURE;
        let values = values.map(|v| (v * 100.0).round() / 100.0);
        let dto = draft_to_dto(&Draft::new(typed_spec(&values))).unwrap();
        let once = suggest_fill(&f.ctx, dto).unwrap();
        prop_assert!(typed_unchanged(&once, &values));
        let twice = suggest_fill(&f.ctx, once.clone()).unwrap();
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn the_preview_is_deterministic_and_leaves_the_draft_alone(
        values in prop::array::uniform5(1.0f64..200.0),
    ) {
        let f = &*FIXTURE;
        let values = values.map(|v| (v * 100.0).round() / 100.0);
        let dto = draft_to_dto(&Draft::new(typed_spec(&values))).unwrap();
        let before = dto.clone();
        let a = preview(&f.ctx, DesignerPreviewRequest { draft: dto.clone() }).unwrap();
        let b = preview(&f.ctx, DesignerPreviewRequest { draft: dto.clone() }).unwrap();
        prop_assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
        prop_assert_eq!(dto, before);
    }

    #[test]
    fn a_stored_draft_is_the_same_draft(
        values in prop::array::uniform5(1.0f64..200.0),
        anchor in 0u8..15,
    ) {
        let f = &*FIXTURE;
        let values = values.map(|v| (v * 100.0).round() / 100.0);
        let mut draft = Draft::new(typed_spec(&values));
        draft.anchors.push(rimstudio_design::model::Anchor::new(format!("RS_Gun{anchor:02}")));
        draft.answers.insert("cmp:RS_Gun01".into(), serde_json::json!({"seq": 0, "answer": {"kind": "weaker"}}));
        let dto = draft_to_dto(&draft).unwrap();
        let id = draft_save(&f.ctx, DesignerDraftSaveRequest {
            project_id: "p-prop".into(),
            id: None,
            draft: dto.clone(),
        }).unwrap().id;
        let loaded = draft_load(&f.ctx, "p-prop", &id).unwrap();
        prop_assert_eq!(loaded.draft, dto);
    }
}
