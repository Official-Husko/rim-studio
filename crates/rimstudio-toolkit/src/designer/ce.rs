//! Suggestions for the optional Combat Extended block of a draft: the body of `designer_ce_suggest` and the
//! `acceptSuggestions` option of plan and apply.
//!
//! The vanilla design is the twin: [`ce_suggest`] asks [`rimstudio_design::ce::suggest::suggest_block`] what
//! the user's own conversions say about every field of the block and returns the report. It works for a
//! draft whose Combat Extended toggle is off (`spec.ce` absent) and never turns the toggle on. Without
//! Combat Extended data the answer is the plain reason.
//!
//! [`accept_for_plan`] is the plan side. When a plan or apply request opts in and the draft already carries
//! the block, the empty fields (and the fields that hold an earlier suggestion) are filled from the
//! suggestion before the patch is planned. A value the user typed, answered or took from an anchor is never
//! overwritten (IT-003). Each filled field is reported as an info diagnostic `ce.derived-value`. The ammo set
//! and the weapon tag class are never chosen for the user: while they are unanswered the plan carries the
//! error `designer.ce-needs-answer` and cannot be applied. A suggestion never creates a Combat Extended
//! block: a draft without it plans exactly as before, with an info diagnostic saying the option was
//! ignored.

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_design::ce::suggest::{
    Accept, CeSuggestion, FieldStatus, accept_suggestions, suggest_block,
};
use rimstudio_design::model::DesignSpec;
use rimstudio_ipc_types::designer::{
    AcceptSuggestionsDto, CeSuggestionDto, DesignerCeSuggestRequest,
};

use super::ctx::Ctx;
use super::dto::draft_from_dto;
use crate::error::{ToolkitError, ToolkitResult};

/// The diagnostic code (error) of a choice the plan needs and the user has not answered.
pub const CE_NEEDS_ANSWER: DiagCode = DiagCode::new("designer.ce-needs-answer");
/// The diagnostic code (info) of an estimate that was not written and why.
pub const CE_SUGGESTION_REJECTED: DiagCode = DiagCode::new("designer.ce-suggestion-rejected");
/// The diagnostic code (warning) of a named suggestion that was not accepted.
pub const CE_SUGGESTION_SKIPPED: DiagCode = DiagCode::new("designer.ce-suggestion-skipped");
/// The diagnostic code (info) of the option `acceptSuggestions` on a draft without a Combat Extended block.
pub const CE_SUGGESTION_IGNORED: DiagCode = DiagCode::new("designer.ce-suggestion-ignored");

/// The reason shown when no Combat Extended data is loaded.
const NO_CE: &str = "no Combat Extended data is loaded";

/// The suggestion for a spec: from the loaded Combat Extended model, or the plain reason when there is none.
#[must_use]
pub fn suggestion_for(ctx: &Ctx, spec: &DesignSpec) -> CeSuggestion {
    match ctx.engine() {
        Some(engine) if engine.ce_available() => suggest_block(spec, engine.ce()),
        Some(_) => suggest_block(spec, &rimstudio_design::ce::reader::CeModel::absent(NO_CE)),
        None => suggest_block(
            spec,
            &rimstudio_design::ce::reader::CeModel::absent("no game install is loaded"),
        ),
    }
}

fn through_json<A, B>(a: &A) -> ToolkitResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(a).map_err(|e| ToolkitError::Internal {
        what: format!("a suggestion cannot be encoded: {e}"),
    })?;
    serde_json::from_value(value).map_err(|e| ToolkitError::Internal {
        what: format!("a suggestion has the wrong shape: {e}"),
    })
}

/// The suggestion as the DTO.
///
/// # Errors
///
/// [`ToolkitError::Internal`] when the shapes disagree (the tests keep that from happening).
pub fn suggestion_to_dto(suggestion: &CeSuggestion) -> ToolkitResult<CeSuggestionDto> {
    through_json(suggestion)
}

/// `designer_ce_suggest`: the suggestion for the Combat Extended block of a draft. Pure and read only; the
/// draft's toggle is never changed, and a draft without the block is allowed.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a draft that cannot be read.
pub fn ce_suggest(ctx: &Ctx, req: DesignerCeSuggestRequest) -> ToolkitResult<CeSuggestionDto> {
    let draft = draft_from_dto(&req.draft)?;
    suggestion_to_dto(&suggestion_for(ctx, &draft.spec))
}

fn info(code: DiagCode, severity: Severity, message: String, field: &str) -> Diagnostic {
    Diagnostic::new(code, severity, message).with_arg("field", field)
}

/// What an opt in to suggestions does to a spec before planning.
///
/// Returns the spec to plan and the diagnostics of the step. A spec without the block is returned as it is.
/// When no Combat Extended data is loaded nothing is changed and the plan itself reports the missing data.
#[must_use]
pub fn accept_for_plan(
    ctx: &Ctx,
    spec: &DesignSpec,
    accept: &AcceptSuggestionsDto,
) -> (DesignSpec, Vec<Diagnostic>) {
    if spec.ce.is_none() {
        return (
            spec.clone(),
            vec![info(
                CE_SUGGESTION_IGNORED,
                Severity::Info,
                "suggestions were not accepted: the Combat Extended patch is off for this design, and suggestions never turn it on".to_owned(),
                "/ce",
            )],
        );
    }
    let Some(engine) = ctx.engine().filter(|e| e.ce_available()) else {
        return (spec.clone(), Vec::new());
    };
    let which = if accept.fields.is_empty() {
        Accept::All
    } else {
        Accept::Fields(accept.fields.clone())
    };
    let suggestion = suggest_block(spec, engine.ce());
    let outcome = accept_suggestions(spec, &suggestion, &which);
    let mut diagnostics = outcome.diagnostics.clone();
    for skipped in &outcome.skipped {
        diagnostics.push(info(
            CE_SUGGESTION_SKIPPED,
            Severity::Warning,
            format!("{} was not accepted: {}", skipped.field, skipped.reason),
            &skipped.field,
        ));
    }
    // What is still open after the fill: the choices the plan needs, and the rejected estimates.
    let after = suggest_block(&outcome.spec, engine.ce());
    let has_custom_ammo = spec.ce.as_ref().is_some_and(|c| c.custom_ammo.is_some());
    for choice in &after.choices {
        let dependent = choice.field == "/ce/defaultProjectile" && choice.candidates.is_empty();
        // a custom caliber brings its own ammo set and default projectile: nothing is asked for them
        let custom = has_custom_ammo
            && matches!(
                choice.field.as_str(),
                "/ce/ammoSet" | "/ce/defaultProjectile"
            );
        if choice.required && choice.status == FieldStatus::Ask && !dependent && !custom {
            diagnostics.push(
                info(
                    CE_NEEDS_ANSWER,
                    Severity::Error,
                    format!(
                        "{} needs your answer before this plan can be written: {}",
                        choice.label,
                        choice
                            .reason
                            .as_deref()
                            .unwrap_or("it is never chosen for you")
                    ),
                    &choice.field,
                )
                .with_arg(
                    "candidates",
                    choice
                        .candidates
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
        }
    }
    for field in after.fields.iter().filter(|f| f.status == FieldStatus::Ask) {
        let Some(reason) = &field.reason else {
            continue;
        };
        let mut d = info(
            CE_SUGGESTION_REJECTED,
            Severity::Info,
            format!("{}: {reason}", field.label),
            &field.field,
        );
        if let Some(r) = field.reference {
            d = d.with_arg("reference", r.to_string());
        }
        diagnostics.push(d);
    }
    (outcome.spec, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::ce::classes::{ConversionPredictor, Reliability};
    use rimstudio_design::ce::patchgen::{AskItem, AskKind, AskList};
    use rimstudio_design::ce::suggest::{
        Candidate, ChoiceField, Interval, SuggestedField, SuggestionBand, SuggestionSource,
    };
    use rimstudio_design::model::ItemKind;

    fn field(status: FieldStatus, source: SuggestionSource, rating: Reliability) -> SuggestedField {
        SuggestedField {
            field: "/ce/bulk".into(),
            label: "CE bulk".into(),
            required: true,
            status,
            held: Some(1.0),
            held_source: Some(SuggestionSource::Typed),
            value: Some(2.0),
            source: Some(source),
            band: Some(SuggestionBand {
                p50: Interval {
                    low: 1.5,
                    high: 2.5,
                },
                p80: Interval {
                    low: 1.0,
                    high: 3.0,
                },
            }),
            rating: Some(rating),
            error: Some(0.1),
            reference: Some(2.2),
            reason: Some("because".into()),
        }
    }

    #[test]
    fn every_variant_of_the_engine_types_has_the_same_json_in_the_dto() {
        let sources = [
            SuggestionSource::Typed,
            SuggestionSource::Anchor,
            SuggestionSource::Answered,
            SuggestionSource::Identity { n: 3 },
            SuggestionSource::Predicted {
                predictor: ConversionPredictor::Elastic,
                n: 4,
            },
            SuggestionSource::Vanilla,
            SuggestionSource::FirstOfSet,
        ];
        let ratings = [
            Reliability::Reliable,
            Reliability::Rough,
            Reliability::Unreliable,
            Reliability::Unmeasured,
        ];
        let statuses = [FieldStatus::Held, FieldStatus::Derived, FieldStatus::Ask];
        let fields: Vec<SuggestedField> = sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                field(
                    statuses[i % statuses.len()],
                    s.clone(),
                    ratings[i % ratings.len()],
                )
            })
            .collect();
        let suggestion = CeSuggestion {
            kind: ItemKind::Melee,
            def_name: "RS_X".into(),
            available: true,
            reason: None,
            toggle_on: false,
            class_label: Some("class".into()),
            pool: 9,
            fields,
            choices: vec![ChoiceField {
                field: "/ce/ammoSet".into(),
                label: "ammo".into(),
                kind: AskKind::Flag,
                required: false,
                status: FieldStatus::Derived,
                held: Some("true".into()),
                value: Some("RS_P".into()),
                source: Some(SuggestionSource::FirstOfSet),
                candidates: vec![Candidate {
                    name: "RS_Set".into(),
                    used_by: 2,
                    score: 1.5,
                    first_damage: Some(9.0),
                }],
                reason: None,
            }],
            patch_numbers: Vec::new(),
            asks: AskList {
                items: vec![AskItem {
                    field: "/ce/bulk".into(),
                    label: "bulk".into(),
                    kind: AskKind::Number,
                    options: Vec::new(),
                    reason: Some("r".into()),
                    suggestion: Some(1.0),
                }],
            },
            missing: vec!["/ce/bulk".into()],
            still_missing_after_accept: Vec::new(),
            notes: vec!["n".into()],
            options: Vec::new(),
        };
        let dto = suggestion_to_dto(&suggestion).unwrap();
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            serde_json::to_value(&suggestion).unwrap()
        );
    }
}
