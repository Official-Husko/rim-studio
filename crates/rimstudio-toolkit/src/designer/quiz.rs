//! The quiz of the designer: one question at a time, answers stored in the draft.
//!
//! The engine quiz ([`rimstudio_design::quiz::Quiz`]) is a state machine that stores only its answers; this
//! module keeps those answers inside the draft, so the draft is the whole dialogue state (IT-026) and the
//! calls are pure functions of the draft:
//!
//! - [`quiz_next`]: the open question, the estimate so far and what the answers imply;
//! - [`quiz_answer`]: stores one answer, applies the estimate to the spec (typed values untouched, IT-003)
//!   and returns the next step;
//! - [`quiz_back`]: removes the last answer and recomputes (the Back button).
//!
//! Draft layout: `answers[<question id>] = {"seq": <position>, "answer": <engine answer>}`. The position
//! restores the order, because a question depends on the answers before it. Keys that are not of this shape
//! (the `strength` choice of simple mode) are kept as they are.
//!
//! The setup questions are answered from the spec when it already says the tier and the role (they are
//! required fields of a draft): the quiz then starts at the strength dialogue. Those answers are stored in
//! the draft with the first answer the user gives, marked `"auto": true`, so the dialogue reopens
//! identically; they do not count in the question numbers the stepper shows and Back does not undo them.
//!
//! The quiz needs at least eight reference weapons of the kind; below that the designer is in simple mode
//! only and these calls report [`ToolkitError::CalibrationUnavailable`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rimstudio_design::baseline::{Model, StatConstraint};
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_design::model::{CalibrationMode, Draft};
use rimstudio_design::quiz::{Answer, AnswerEntry, Question, Quiz, QuizError, QuizState};
use rimstudio_ipc_types::designer::{
    DesignerQuizAnswerRequest, DesignerQuizAnswerResponse, DesignerQuizBackRequest,
    DesignerQuizNextRequest, QuizStepDto,
};
use serde_json::{Value, json};

use super::calibrate::bands_for;
use super::ctx::{Ctx, Engine};
use super::dto::{
    answer_from_dto, count, draft_from_dto, draft_to_dto, estimate_summary, pool_kind,
    prompt_to_dto, tier_from_index,
};
use super::preview::apply_estimate;
use crate::error::{ToolkitError, ToolkitResult};

/// The key of the strength choice of simple mode in the draft's answers.
pub const STRENGTH_KEY: &str = "strength";

/// The quiz of a draft after the replay of its stored answers.
#[derive(Debug, Clone)]
pub struct DraftQuiz {
    /// The quiz with the stored answers and the seeded setup answers.
    pub quiz: Quiz,
    /// Stored answers dropped because the reference pool changed under the draft.
    pub dropped: usize,
    /// The ids of the answers that were given from the spec and not by the user.
    pub auto: BTreeSet<String>,
}

/// Maps an engine quiz error to a toolkit error.
#[must_use]
pub fn map_quiz_error(e: QuizError) -> ToolkitError {
    match e {
        QuizError::WrongAnswer { question } => ToolkitError::QuizWrongAnswer {
            detail: format!("the answer does not fit the open question {question}"),
        },
        QuizError::Finished => ToolkitError::QuizFinished,
        QuizError::Stale { index } => ToolkitError::InvalidDraft {
            reason: format!("stored answer {index} no longer matches the reference weapons"),
        },
        QuizError::UnknownChoice { what } => ToolkitError::QuizWrongAnswer {
            detail: format!("the reference weapons have no {what}"),
        },
        QuizError::Design(e) => ToolkitError::Design(e),
    }
}

fn entry_of(key: &str, value: &Value) -> Option<(u64, AnswerEntry, bool)> {
    let seq = value.get("seq")?.as_u64()?;
    let answer: Answer = serde_json::from_value(value.get("answer")?.clone()).ok()?;
    let question = key.split('#').next().unwrap_or(key).to_owned();
    let auto = value.get("auto").and_then(Value::as_bool).unwrap_or(false);
    Some((seq, AnswerEntry { question, answer }, auto))
}

/// True when the value has the shape of a stored quiz answer.
fn is_quiz_entry(value: &Value) -> bool {
    value.get("seq").is_some() && value.get("answer").is_some()
}

fn decode(draft: &Draft) -> ToolkitResult<(Vec<AnswerEntry>, BTreeSet<String>)> {
    let mut found = Vec::new();
    for (key, value) in &draft.answers {
        if !is_quiz_entry(value) {
            continue;
        }
        match entry_of(key, value) {
            Some(e) => found.push(e),
            None => {
                return Err(ToolkitError::invalid_draft(format!(
                    "the stored quiz answer {key} cannot be read"
                )));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.question.cmp(&b.1.question)));
    let auto = found
        .iter()
        .filter(|(_, _, a)| *a)
        .map(|(_, e, _)| e.question.clone())
        .collect();
    Ok((found.into_iter().map(|(_, e, _)| e).collect(), auto))
}

/// Reads the stored answers of a draft in answer order.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] when an entry has the quiz shape but its answer cannot be read.
pub fn decode_answers(draft: &Draft) -> ToolkitResult<Vec<AnswerEntry>> {
    Ok(decode(draft)?.0)
}

/// The answers map of a draft after the quiz answers were replaced by those of `quiz`. Other keys stay.
/// Answers whose question id is in `auto` are marked as given from the spec.
#[must_use]
pub fn encode_answers(
    old: &BTreeMap<String, Value>,
    quiz: &Quiz,
    auto: &BTreeSet<String>,
) -> BTreeMap<String, Value> {
    let mut out: BTreeMap<String, Value> = old
        .iter()
        .filter(|(_, v)| !is_quiz_entry(v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (i, entry) in quiz.answers.iter().enumerate() {
        let mut key = entry.question.clone();
        let mut n = 1usize;
        while out.contains_key(&key) {
            n += 1;
            key = format!("{}#{n}", entry.question);
        }
        let mut value = json!({
            "seq": i,
            "answer": serde_json::to_value(&entry.answer).unwrap_or(Value::Null),
        });
        if auto.contains(&entry.question)
            && let Some(map) = value.as_object_mut()
        {
            map.insert("auto".to_owned(), Value::Bool(true));
        }
        out.insert(key, value);
    }
    out
}

/// Answers the open setup questions from the spec: the tier and the role the draft already names. The
/// seeded answers are appended to the quiz so that they persist with the next stored answer.
fn seed_setup(quiz: &mut Quiz, model: &Model, draft: &Draft, auto: &mut BTreeSet<String>) {
    for _ in 0..2 {
        let Ok(Some(prompt)) = quiz.next(model) else {
            return;
        };
        let answer = match (&prompt.question, &draft.spec.tech_level, &draft.spec.role) {
            (Question::Tier { options }, Some(t), _)
                if options.iter().any(|o| o.tier == t.index()) =>
            {
                Answer::Tier { tier: t.index() }
            }
            (Question::Role { options }, _, Some(r)) if options.iter().any(|o| &o.name == r) => {
                Answer::Role { role: r.clone() }
            }
            _ => return,
        };
        if quiz.answer(model, answer).is_err() {
            return;
        }
        auto.insert(prompt.id);
    }
}

/// Rebuilds the quiz of a draft: its stored answers replayed against the model, the stale tail dropped,
/// the setup questions answered from the spec.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable stored answer or an unsupported kind.
pub fn quiz_from_draft(model: &Model, draft: &Draft) -> ToolkitResult<DraftQuiz> {
    let kind = pool_kind(draft.kind)?;
    let mut quiz = Quiz::for_kind(kind);
    let (answers, mut auto) = decode(draft)?;
    quiz.answers = answers;
    let dropped = quiz.truncate_stale(model);
    if dropped > 0 {
        tracing::info!(
            dropped,
            "dropped quiz answers that no longer fit the reference weapons"
        );
    }
    auto.retain(|q| quiz.answers.iter().any(|a| &a.question == q));
    seed_setup(&mut quiz, model, draft, &mut auto);
    Ok(DraftQuiz {
        quiz,
        dropped,
        auto,
    })
}

struct QuizEnv {
    engine: Arc<Engine>,
    model: Arc<Model>,
    kind: PoolKind,
}

fn env(ctx: &Ctx, draft: &Draft) -> ToolkitResult<QuizEnv> {
    let kind = pool_kind(draft.kind)?;
    let engine = ctx.require_engine()?;
    let model = engine
        .model(kind)?
        .ok_or_else(|| ToolkitError::ReferenceUnavailable {
            reason: "the loaded defs hold no reference weapons of this kind".to_owned(),
        })?;
    if !model.quiz_available() {
        return Err(ToolkitError::CalibrationUnavailable {
            reason: format!(
                "the quiz needs at least {} reference weapons and this pool has {}",
                rimstudio_design::baseline::MIN_QUIZ_ITEMS,
                model.pool.len()
            ),
        });
    }
    Ok(QuizEnv {
        engine,
        model,
        kind,
    })
}

fn describe_constraint(c: &StatConstraint) -> String {
    match c {
        StatConstraint::Typed { value } => format!("typed {value}"),
        StatConstraint::Interval {
            lo: Some(lo),
            hi: Some(hi),
        } => format!("from {lo} to {hi}"),
        StatConstraint::Interval {
            lo: Some(lo),
            hi: None,
        } => format!("at least {lo}"),
        StatConstraint::Interval {
            lo: None,
            hi: Some(hi),
        } => format!("below {hi}"),
        StatConstraint::Interval { lo: None, hi: None } => "any".to_owned(),
        StatConstraint::VsAnchor { anchor, bucket } => {
            format!("{} than {anchor}", format!("{bucket:?}").to_lowercase())
        }
    }
}

/// What the answers imply, as display text by name: the class selectors and one line per constrained stat.
fn implied(state: &QuizState) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(t) = state.key.tier {
        out.insert(
            "tier".to_owned(),
            tier_from_index(t).map_or_else(|| t.to_string(), |l| l.xml_name().to_owned()),
        );
    }
    if let Some(r) = &state.key.role {
        out.insert("role".to_owned(), r.clone());
    }
    if let Some(g) = &state.key.group {
        out.insert("group".to_owned(), g.clone());
    }
    for (stat, c) in &state.constraints {
        out.insert(stat.clone(), describe_constraint(c));
    }
    out
}

fn build_step(
    ctx: &Ctx,
    env: &QuizEnv,
    quiz: &Quiz,
    auto: &BTreeSet<String>,
) -> ToolkitResult<QuizStepDto> {
    let model = &env.model;
    let prompt = quiz.next(model).map_err(map_quiz_error)?;
    let bands = bands_for(
        ctx,
        &env.engine,
        env.kind,
        rimstudio_design::loo::CalibrationMode::Quiz,
    )?;
    let estimate = quiz
        .estimate(model, bands.as_ref())
        .map_err(map_quiz_error)?;
    let state = quiz.state(model).map_err(map_quiz_error)?;
    // The setup answers given from the spec are not the user's questions: leave them out of the numbers.
    let hidden = quiz
        .answers
        .iter()
        .filter(|a| auto.contains(&a.question))
        .count();
    let mut prompt_dto = prompt.as_ref().map(prompt_to_dto);
    if let Some(p) = prompt_dto.as_mut() {
        p.number = p.number.saturating_sub(count(hidden)).max(1);
        p.about_total = p.about_total.saturating_sub(count(hidden)).max(p.number);
    }
    Ok(QuizStepDto {
        finished: prompt.is_none(),
        prompt: prompt_dto,
        answered: count(quiz.answers.len().saturating_sub(hidden)),
        estimate: Some(estimate_summary(&estimate)),
        implied: implied(&state),
    })
}

/// The next question of a draft with the estimate so far (`designer_quiz_next`).
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable draft, [`ToolkitError::ReferenceUnavailable`] without
/// reference weapons, [`ToolkitError::CalibrationUnavailable`] when the pool is too small for a quiz.
pub fn quiz_next(ctx: &Ctx, req: DesignerQuizNextRequest) -> ToolkitResult<QuizStepDto> {
    let draft = draft_from_dto(&req.draft)?;
    let env = env(ctx, &draft)?;
    let dq = quiz_from_draft(&env.model, &draft)?;
    build_step(ctx, &env, &dq.quiz, &dq.auto)
}

fn finish_draft(
    ctx: &Ctx,
    env: &QuizEnv,
    mut draft: Draft,
    quiz: &Quiz,
    auto: &BTreeSet<String>,
    last: Option<&Answer>,
) -> ToolkitResult<DesignerQuizAnswerResponse> {
    draft.calibration = CalibrationMode::Quiz;
    draft.answers = encode_answers(&draft.answers, quiz, auto);
    if let Some(Answer::Tier { tier }) = last
        && draft.spec.tech_level.is_none()
    {
        draft.spec.tech_level = tier_from_index(*tier);
    }
    if let Some(Answer::Role { role }) = last
        && draft.spec.role.is_none()
    {
        draft.spec.role = Some(role.clone());
    }
    let bands = bands_for(
        ctx,
        &env.engine,
        env.kind,
        rimstudio_design::loo::CalibrationMode::Quiz,
    )?;
    let estimate = quiz
        .estimate(&env.model, bands.as_ref())
        .map_err(map_quiz_error)?;
    apply_estimate(&mut draft.spec, &estimate);
    let step = build_step(ctx, env, quiz, auto)?;
    Ok(DesignerQuizAnswerResponse {
        draft: draft_to_dto(&draft)?,
        step,
    })
}

/// Stores one answer and returns the draft with the implied values applied and the next step
/// (`designer_quiz_answer`). Values the user typed are never replaced.
///
/// # Errors
///
/// [`ToolkitError::QuizFinished`] when no question is open, [`ToolkitError::QuizWrongAnswer`] when the
/// question id is not the open question or the answer does not fit it; the other errors of
/// [`quiz_next`].
pub fn quiz_answer(
    ctx: &Ctx,
    req: DesignerQuizAnswerRequest,
) -> ToolkitResult<DesignerQuizAnswerResponse> {
    let draft = draft_from_dto(&req.draft)?;
    let env = env(ctx, &draft)?;
    let DraftQuiz { mut quiz, auto, .. } = quiz_from_draft(&env.model, &draft)?;
    let open = quiz
        .next(&env.model)
        .map_err(map_quiz_error)?
        .ok_or(ToolkitError::QuizFinished)?;
    if open.id != req.question_id {
        return Err(ToolkitError::QuizWrongAnswer {
            detail: format!("the open question is {}, not {}", open.id, req.question_id),
        });
    }
    let answer = answer_from_dto(&req.answer);
    quiz.answer(&env.model, answer.clone())
        .map_err(map_quiz_error)?;
    finish_draft(ctx, &env, draft, &quiz, &auto, Some(&answer))
}

/// Removes the last answer and recomputes (the Back button of the stepper; `designer_quiz_back`).
///
/// Values the earlier answers had applied to the spec stay (a replacement never lowers a source rank), so
/// the caller treats the returned estimate and step as the truth and the form values as a starting point.
///
/// # Errors
///
/// [`ToolkitError::QuizWrongAnswer`] when the draft has no answer to remove; the other errors of
/// [`quiz_next`].
pub fn quiz_back(
    ctx: &Ctx,
    req: DesignerQuizBackRequest,
) -> ToolkitResult<DesignerQuizAnswerResponse> {
    let draft = draft_from_dto(&req.draft)?;
    let env = env(ctx, &draft)?;
    let DraftQuiz { mut quiz, auto, .. } = quiz_from_draft(&env.model, &draft)?;
    // The answers given from the spec belong to the spec and cannot be undone here.
    let last_is_the_users = quiz
        .answers
        .last()
        .is_some_and(|a| !auto.contains(&a.question));
    if !last_is_the_users || quiz.back().is_none() {
        return Err(ToolkitError::QuizWrongAnswer {
            detail: "there is no answer to take back".to_owned(),
        });
    }
    finish_draft(ctx, &env, draft, &quiz, &auto, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::baseline::Bucket;
    use rimstudio_design::classes::ItemKind as PoolKind;
    use rimstudio_design::model::DesignSpec;

    fn quiz_of(entries: &[(&str, Answer)]) -> Quiz {
        let mut quiz = Quiz::for_kind(PoolKind::Ranged);
        quiz.answers = entries
            .iter()
            .map(|(q, a)| AnswerEntry {
                question: (*q).to_owned(),
                answer: a.clone(),
            })
            .collect();
        quiz
    }

    fn draft_with(answers: BTreeMap<String, Value>) -> Draft {
        let mut draft = Draft::new(DesignSpec::new_ranged("RS_X", "x"));
        draft.answers = answers;
        draft
    }

    #[test]
    fn encoding_then_decoding_restores_the_order_and_the_auto_flags() {
        let quiz = quiz_of(&[
            ("tier", Answer::Tier { tier: 2 }),
            ("cmp:RS_A", Answer::Stronger),
            ("cmp:RS_B", Answer::Same),
        ]);
        let auto: BTreeSet<String> = ["tier".to_owned()].into();
        let map = encode_answers(&BTreeMap::new(), &quiz, &auto);
        assert_eq!(map["tier"]["auto"], true);
        assert!(map["cmp:RS_A"].get("auto").is_none());
        let draft = draft_with(map);
        let (answers, found_auto) = decode(&draft).unwrap();
        assert_eq!(answers, quiz.answers);
        assert_eq!(found_auto, auto);
    }

    #[test]
    fn keys_that_are_not_quiz_answers_survive_an_encode() {
        let mut old = BTreeMap::new();
        old.insert(
            STRENGTH_KEY.to_owned(),
            serde_json::json!({"kind": "choice", "choice": "weaker"}),
        );
        old.insert(
            "cmp:RS_Old".to_owned(),
            serde_json::json!({"seq": 0, "answer": {"kind": "weaker"}}),
        );
        let quiz = quiz_of(&[("cmp:RS_New", Answer::Weaker)]);
        let map = encode_answers(&old, &quiz, &BTreeSet::new());
        assert!(map.contains_key(STRENGTH_KEY));
        assert!(
            !map.contains_key("cmp:RS_Old"),
            "old quiz answers are replaced"
        );
        assert!(map.contains_key("cmp:RS_New"));
        let decoded = decode_answers(&draft_with(map)).unwrap();
        assert_eq!(decoded.len(), 1, "the strength choice is not an answer");
    }

    #[test]
    fn a_repeated_question_id_gets_a_suffix_and_decodes_in_order() {
        let quiz = quiz_of(&[
            ("bin:range:20", Answer::Bin { index: 1 }),
            ("bin:range:20", Answer::Bin { index: 2 }),
        ]);
        let map = encode_answers(&BTreeMap::new(), &quiz, &BTreeSet::new());
        assert!(map.contains_key("bin:range:20#2"));
        let decoded = decode_answers(&draft_with(map)).unwrap();
        assert_eq!(decoded, quiz.answers);
    }

    #[test]
    fn answers_of_every_kind_round_trip_through_the_draft() {
        let quiz = quiz_of(&[
            ("tier", Answer::Tier { tier: 3 }),
            (
                "role",
                Answer::Role {
                    role: "rifle".into(),
                },
            ),
            (
                "group",
                Answer::Group {
                    group: "burst".into(),
                },
            ),
            ("cmp:a", Answer::Weaker),
            ("closer:a:b", Answer::CloserToUpper),
            ("bin:mass:1", Answer::Bin { index: 2 }),
            (
                "vs:mass:a",
                Answer::Bucket {
                    bucket: Bucket::Higher,
                },
            ),
            ("vs:range:a", Answer::Typed { value: 31.5 }),
            ("vs:warmup:a", Answer::NotSure),
            ("vs:cooldown:a", Answer::Skip),
            ("cmp:z", Answer::UseWhatIHave),
        ]);
        let map = encode_answers(&BTreeMap::new(), &quiz, &BTreeSet::new());
        assert_eq!(decode_answers(&draft_with(map)).unwrap(), quiz.answers);
    }

    #[test]
    fn quiz_errors_map_to_the_registered_codes() {
        assert_eq!(
            map_quiz_error(QuizError::Finished).code(),
            "designer.quiz-finished"
        );
        assert_eq!(
            map_quiz_error(QuizError::WrongAnswer {
                question: "q".into()
            })
            .code(),
            "designer.quiz-wrong-answer"
        );
        assert_eq!(
            map_quiz_error(QuizError::UnknownChoice { what: "x".into() }).code(),
            "designer.quiz-wrong-answer"
        );
        assert_eq!(
            map_quiz_error(QuizError::Stale { index: 1 }).code(),
            "designer.invalid-draft"
        );
        assert_eq!(
            map_quiz_error(QuizError::Design(
                rimstudio_design::error::DesignError::invalid("x", "y")
            ))
            .code(),
            "design.invalid-input"
        );
    }

    #[test]
    fn describing_constraints_is_readable() {
        assert_eq!(
            describe_constraint(&StatConstraint::Interval {
                lo: Some(1.0),
                hi: Some(2.0)
            }),
            "from 1 to 2"
        );
        assert_eq!(
            describe_constraint(&StatConstraint::Interval {
                lo: None,
                hi: Some(2.0)
            }),
            "below 2"
        );
        assert_eq!(
            describe_constraint(&StatConstraint::Typed { value: 4.5 }),
            "typed 4.5"
        );
        assert_eq!(
            describe_constraint(&StatConstraint::VsAnchor {
                anchor: "RS_A".into(),
                bucket: Bucket::Lower
            }),
            "lower than RS_A"
        );
    }
}
