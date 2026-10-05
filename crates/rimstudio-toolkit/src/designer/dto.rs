//! Conversions between the engine types of `rimstudio-design` and the DTOs of `rimstudio-ipc-types`.
//!
//! Types whose JSON shape is identical on both sides (the design spec, the draft) convert through a
//! `serde_json` value round trip; the module tests prove the shapes agree for every variant and fail when a
//! DTO and its model drift apart. Types whose shapes differ on purpose (suggestions, questions, the fit
//! report, calibration metrics) convert field by field, with exhaustive matches over the engine enums, so a
//! new engine variant is a compile error here and not a silent gap.
//!
//! The functions are plain and total except the spec and draft conversions, which return a
//! [`ToolkitError::InvalidDraft`] for JSON that does not fit the target.

use std::collections::BTreeMap;

use rimstudio_design::baseline::{
    Anchor, Band, Bucket, Estimate, Source, StatEstimate, StrengthChoice, StrengthInput,
};
use rimstudio_design::classes::{ChainLevel, ItemKind as PoolKind, Predictor as ClassPredictor};
use rimstudio_design::fit::{FitLevel, FitReport};
use rimstudio_design::loo::{CalibrationMetrics, StatMetrics, TwinCheck, VariantMetrics};
use rimstudio_design::model::{
    Anchor as DraftAnchor, CalibrationMode, DesignSpec, Draft, ItemKind, TechLevel, ValueSource,
};
use rimstudio_design::quiz::{AnchorCard, Answer, Bin, NamedOption, Prompt, Question, TierOption};
use rimstudio_ipc_types::designer::{
    AnchorCardDto, AnchorDto, BinDto, BucketDto, CalibrateResultDto, CalibrationModeDto,
    ChainLevelDto, DesignSpecDto, DraftDto, EstimateSummaryDto, FitLevelDto, FitNoticesDto,
    FitReportDto, FitSummaryDto, IntervalDto, ItemKindDto, NamedOptionDto, PredictorDto, PromptDto,
    QuestionDto, QuizAnswerDto, RankDto, StatFitDto, StatMetricsDto, SuggestionBandDto,
    SuggestionSourceDto, TechLevelDto, TierOptionDto, TwinCheckDto, ValueSourceDto,
    VariantMetricsDto,
};

use crate::error::{ToolkitError, ToolkitResult};

/// A count as the wire's 32 bit integer; saturates instead of wrapping.
#[must_use]
pub fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------------------------------
// Value round trips for types with identical shapes
// ---------------------------------------------------------------------------------------------------

fn through_json<A, B>(a: &A, what: &str) -> ToolkitResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(a)
        .map_err(|e| ToolkitError::invalid_draft(format!("{what} cannot be encoded: {e}")))?;
    serde_json::from_value(value)
        .map_err(|e| ToolkitError::invalid_draft(format!("{what} has the wrong shape: {e}")))
}

/// The engine spec as the DTO.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] when the shapes disagree (the parity tests keep that from happening).
pub fn spec_to_dto(spec: &DesignSpec) -> ToolkitResult<DesignSpecDto> {
    through_json(spec, "the design spec")
}

/// The DTO spec as the engine spec.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] when the JSON does not fit the engine type.
pub fn spec_from_dto(spec: &DesignSpecDto) -> ToolkitResult<DesignSpec> {
    through_json(spec, "the design spec")
}

/// The engine draft as the DTO.
///
/// # Errors
///
/// See [`spec_to_dto`].
pub fn draft_to_dto(draft: &Draft) -> ToolkitResult<DraftDto> {
    through_json(draft, "the draft")
}

/// The DTO draft as the engine draft. The kind of the envelope must equal the kind of the spec.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a shape problem or an envelope kind that differs from the spec's,
/// [`ToolkitError::DraftNewerSchema`] for a draft written by a newer build.
pub fn draft_from_dto(draft: &DraftDto) -> ToolkitResult<Draft> {
    let parsed: Draft = through_json(draft, "the draft")?;
    if parsed.is_newer_than_supported() {
        return Err(ToolkitError::DraftNewerSchema {
            found: parsed.schema_version,
            supported: Draft::VERSION,
        });
    }
    if !parsed.is_consistent() {
        return Err(ToolkitError::invalid_draft(format!(
            "the draft says {} but its spec is {}",
            parsed.kind.as_str(),
            parsed.spec.kind.as_str()
        )));
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------------------------------
// Small enums
// ---------------------------------------------------------------------------------------------------

/// The wire kind of an engine kind.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a kind this release does not support.
pub fn kind_to_dto(kind: ItemKind) -> ToolkitResult<ItemKindDto> {
    match kind {
        ItemKind::Ranged => Ok(ItemKindDto::Ranged),
        ItemKind::Melee => Ok(ItemKindDto::Melee),
        _ => Err(ToolkitError::invalid_draft(
            "this item kind is not supported in this release",
        )),
    }
}

/// The engine kind of a wire kind.
#[must_use]
pub fn kind_from_dto(kind: ItemKindDto) -> ItemKind {
    match kind {
        ItemKindDto::Ranged => ItemKind::Ranged,
        ItemKindDto::Melee => ItemKind::Melee,
    }
}

/// The pool kind of a spec kind.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a kind this release does not support.
pub fn pool_kind(kind: ItemKind) -> ToolkitResult<PoolKind> {
    match kind {
        ItemKind::Ranged => Ok(PoolKind::Ranged),
        ItemKind::Melee => Ok(PoolKind::Melee),
        _ => Err(ToolkitError::invalid_draft(
            "this item kind is not supported in this release",
        )),
    }
}

/// The wire kind of a pool kind.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for apparel, which this release does not design.
pub fn pool_kind_to_dto(kind: PoolKind) -> ToolkitResult<ItemKindDto> {
    match kind {
        PoolKind::Ranged => Ok(ItemKindDto::Ranged),
        PoolKind::Melee => Ok(ItemKindDto::Melee),
        PoolKind::Apparel => Err(ToolkitError::invalid_draft(
            "apparel is not designed in this release",
        )),
    }
}

/// The wire tech level of an engine tech level.
#[must_use]
pub fn tier_to_dto(tier: TechLevel) -> TechLevelDto {
    match tier {
        TechLevel::Neolithic => TechLevelDto::Neolithic,
        TechLevel::Medieval => TechLevelDto::Medieval,
        TechLevel::Industrial => TechLevelDto::Industrial,
        TechLevel::Spacer => TechLevelDto::Spacer,
        TechLevel::Ultra => TechLevelDto::Ultra,
        TechLevel::Archotech => TechLevelDto::Archotech,
    }
}

/// The engine tech level of a wire tech level.
#[must_use]
pub fn tier_from_dto(tier: TechLevelDto) -> TechLevel {
    match tier {
        TechLevelDto::Neolithic => TechLevel::Neolithic,
        TechLevelDto::Medieval => TechLevel::Medieval,
        TechLevelDto::Industrial => TechLevel::Industrial,
        TechLevelDto::Spacer => TechLevel::Spacer,
        TechLevelDto::Ultra => TechLevel::Ultra,
        TechLevelDto::Archotech => TechLevel::Archotech,
    }
}

/// The tech level with this tier index (zero is neolithic).
#[must_use]
pub fn tier_from_index(index: u8) -> Option<TechLevel> {
    TechLevel::ALL.get(usize::from(index)).copied()
}

/// The wire calibration mode.
#[must_use]
pub fn mode_to_dto(mode: CalibrationMode) -> CalibrationModeDto {
    match mode {
        CalibrationMode::Simple => CalibrationModeDto::Simple,
        CalibrationMode::Quiz => CalibrationModeDto::Quiz,
        CalibrationMode::Anchored => CalibrationModeDto::Anchored,
    }
}

/// The wire source tag of a value.
#[must_use]
pub fn value_source_to_dto(source: ValueSource) -> ValueSourceDto {
    match source {
        ValueSource::Suggested => ValueSourceDto::Suggested,
        ValueSource::Anchor => ValueSourceDto::Anchor,
        ValueSource::Answered => ValueSourceDto::Answered,
        ValueSource::Typed => ValueSourceDto::Typed,
    }
}

/// The wire fallback level.
#[must_use]
pub fn chain_level_to_dto(level: ChainLevel) -> ChainLevelDto {
    match level {
        ChainLevel::RoleTier => ChainLevelDto::RoleTier,
        ChainLevel::Role => ChainLevelDto::Role,
        ChainLevel::GroupTier => ChainLevelDto::GroupTier,
        ChainLevel::Tier => ChainLevelDto::Tier,
        ChainLevel::Group => ChainLevelDto::Group,
        ChainLevel::All => ChainLevelDto::All,
    }
}

/// The wire predictor.
#[must_use]
pub fn predictor_to_dto(predictor: ClassPredictor) -> PredictorDto {
    match predictor {
        ClassPredictor::Median => PredictorDto::Median,
        ClassPredictor::Quantile => PredictorDto::Quantile,
        ClassPredictor::Anchor => PredictorDto::Anchor,
    }
}

/// The wire suggestion source of an estimate source.
#[must_use]
pub fn source_to_dto(source: Source) -> SuggestionSourceDto {
    match source {
        Source::Typed => SuggestionSourceDto::Typed,
        Source::Answer => SuggestionSourceDto::Answer,
        Source::Anchor => SuggestionSourceDto::Anchor,
        Source::ClassMedian => SuggestionSourceDto::ClassMedian,
        Source::Quantile => SuggestionSourceDto::Quantile,
        Source::Derived => SuggestionSourceDto::Derived,
    }
}

/// The value source an estimate source is stored under in a spec: answers and anchors keep their rank, a
/// typed number stays typed, every predicted value is a suggestion.
#[must_use]
pub fn source_to_value_source(source: Source) -> ValueSource {
    match source {
        Source::Typed => ValueSource::Typed,
        Source::Answer => ValueSource::Answered,
        Source::Anchor => ValueSource::Anchor,
        Source::ClassMedian | Source::Quantile | Source::Derived => ValueSource::Suggested,
    }
}

/// The wire fit level.
#[must_use]
pub fn fit_level_to_dto(level: FitLevel) -> FitLevelDto {
    match level {
        FitLevel::Typical => FitLevelDto::Typical,
        FitLevel::Plausible => FitLevelDto::Plausible,
        FitLevel::Unusual => FitLevelDto::Unusual,
    }
}

/// The wire bucket.
#[must_use]
pub fn bucket_to_dto(bucket: Bucket) -> BucketDto {
    match bucket {
        Bucket::Lower => BucketDto::Lower,
        Bucket::Similar => BucketDto::Similar,
        Bucket::Higher => BucketDto::Higher,
    }
}

/// The engine bucket.
#[must_use]
pub fn bucket_from_dto(bucket: BucketDto) -> Bucket {
    match bucket {
        BucketDto::Lower => Bucket::Lower,
        BucketDto::Similar => Bucket::Similar,
        BucketDto::Higher => Bucket::Higher,
    }
}

// ---------------------------------------------------------------------------------------------------
// Drafts
// ---------------------------------------------------------------------------------------------------

/// The wire anchor of a draft anchor.
#[must_use]
pub fn anchor_to_dto(anchor: &DraftAnchor) -> AnchorDto {
    AnchorDto {
        def_name: anchor.def_name.clone(),
        label: anchor.label.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------
// Estimates and suggestions
// ---------------------------------------------------------------------------------------------------

/// The wire band of an estimate band, with the pool quantiles of the stat beside it.
#[must_use]
pub fn band_to_dto(band: &Band, stat: &StatEstimate) -> SuggestionBandDto {
    SuggestionBandDto {
        p10: stat.p10,
        median: stat.median,
        p90: stat.p90,
        p50: band.p50,
        p80: band.p80,
        shift: band.shift,
    }
}

/// The card of an anchor weapon.
#[must_use]
pub fn anchor_card(anchor: &Anchor) -> AnchorCardDto {
    AnchorCardDto {
        id: anchor.id.clone(),
        label: anchor.label.clone(),
        strength: anchor.strength,
        tier: anchor.tier,
        role: anchor.role.clone(),
        stats: anchor.stats.clone(),
    }
}

/// The card of a quiz anchor.
#[must_use]
pub fn quiz_card(card: &AnchorCard) -> AnchorCardDto {
    AnchorCardDto {
        id: card.id.clone(),
        label: card.label.clone(),
        strength: card.strength,
        tier: card.tier,
        role: card.role.clone(),
        stats: card.stats.clone(),
    }
}

/// The summary of an estimate.
#[must_use]
pub fn estimate_summary(estimate: &Estimate) -> EstimateSummaryDto {
    EstimateSummaryDto {
        class_label: estimate.class_label.clone(),
        level: chain_level_to_dto(estimate.level),
        class_n: count(estimate.class_n),
        strength: estimate.strength,
        strength_percentile: estimate.strength_percentile,
        anchors: estimate.anchors.iter().map(anchor_card).collect(),
        notes: estimate.notes.clone(),
    }
}

/// Text for the strength choice of a simple mode draft (used in notes and the CLI).
#[must_use]
pub fn strength_text(input: &StrengthInput) -> String {
    match input {
        StrengthInput::Unknown => "unknown".to_owned(),
        StrengthInput::Choice { choice } => match choice {
            StrengthChoice::Weaker => "weaker than most".to_owned(),
            StrengthChoice::Typical => "typical".to_owned(),
            StrengthChoice::Stronger => "stronger than most".to_owned(),
        },
        StrengthInput::Percentile { p } => format!("percentile {p:.2}"),
        StrengthInput::Placed { ln_power } => format!("strength {:.3}", ln_power.exp()),
    }
}

// ---------------------------------------------------------------------------------------------------
// The quiz
// ---------------------------------------------------------------------------------------------------

fn tier_option(o: &TierOption) -> TierOptionDto {
    TierOptionDto {
        tier: o.tier,
        label: o.label.clone(),
        count: count(o.count),
    }
}

fn named_option(o: &NamedOption) -> NamedOptionDto {
    NamedOptionDto {
        name: o.name.clone(),
        count: count(o.count),
    }
}

fn bin(b: &Bin) -> BinDto {
    BinDto {
        lo: b.lo,
        hi: b.hi,
        label: b.label.clone(),
    }
}

/// The wire question.
#[must_use]
pub fn question_to_dto(question: &Question) -> QuestionDto {
    match question {
        Question::Tier { options } => QuestionDto::Tier {
            options: options.iter().map(tier_option).collect(),
        },
        Question::Role { options } => QuestionDto::Role {
            options: options.iter().map(named_option).collect(),
        },
        Question::Compare {
            anchor,
            remaining,
            asked,
            budget,
            information,
        } => QuestionDto::Compare {
            anchor: quiz_card(anchor),
            remaining: count(*remaining),
            asked: count(*asked),
            budget: count(*budget),
            information: *information,
        },
        Question::CloserTo { lower, upper } => QuestionDto::CloserTo {
            lower: quiz_card(lower),
            upper: quiz_card(upper),
        },
        Question::Group { options } => QuestionDto::Group {
            options: options.iter().map(named_option).collect(),
        },
        Question::Interval { stat, bins, spread } => QuestionDto::Interval {
            stat: stat.clone(),
            bins: bins.iter().map(bin).collect(),
            spread: *spread,
        },
        Question::VsAnchor {
            stat,
            anchor,
            value,
        } => QuestionDto::VsAnchor {
            stat: stat.clone(),
            anchor: quiz_card(anchor),
            value: *value,
        },
    }
}

/// The wire prompt.
#[must_use]
pub fn prompt_to_dto(prompt: &Prompt) -> PromptDto {
    PromptDto {
        id: prompt.id.clone(),
        question: question_to_dto(&prompt.question),
        number: count(prompt.number),
        about_total: count(prompt.about_total),
    }
}

/// The engine answer of a wire answer.
#[must_use]
pub fn answer_from_dto(answer: &QuizAnswerDto) -> Answer {
    match answer {
        QuizAnswerDto::Tier { tier } => Answer::Tier { tier: *tier },
        QuizAnswerDto::Role { role } => Answer::Role { role: role.clone() },
        QuizAnswerDto::Group { group } => Answer::Group {
            group: group.clone(),
        },
        QuizAnswerDto::Weaker => Answer::Weaker,
        QuizAnswerDto::Same => Answer::Same,
        QuizAnswerDto::Stronger => Answer::Stronger,
        QuizAnswerDto::CloserToLower => Answer::CloserToLower,
        QuizAnswerDto::CloserToUpper => Answer::CloserToUpper,
        QuizAnswerDto::Bin { index } => Answer::Bin {
            index: usize::try_from(*index).unwrap_or(usize::MAX),
        },
        QuizAnswerDto::Bucket { bucket } => Answer::Bucket {
            bucket: bucket_from_dto(*bucket),
        },
        QuizAnswerDto::Typed { value } => Answer::Typed { value: *value },
        QuizAnswerDto::NotSure => Answer::NotSure,
        QuizAnswerDto::Skip => Answer::Skip,
        QuizAnswerDto::UseWhatIHave => Answer::UseWhatIHave,
    }
}

/// The wire answer of an engine answer.
#[must_use]
pub fn answer_to_dto(answer: &Answer) -> QuizAnswerDto {
    match answer {
        Answer::Tier { tier } => QuizAnswerDto::Tier { tier: *tier },
        Answer::Role { role } => QuizAnswerDto::Role { role: role.clone() },
        Answer::Group { group } => QuizAnswerDto::Group {
            group: group.clone(),
        },
        Answer::Weaker => QuizAnswerDto::Weaker,
        Answer::Same => QuizAnswerDto::Same,
        Answer::Stronger => QuizAnswerDto::Stronger,
        Answer::CloserToLower => QuizAnswerDto::CloserToLower,
        Answer::CloserToUpper => QuizAnswerDto::CloserToUpper,
        Answer::Bin { index } => QuizAnswerDto::Bin {
            index: count(*index),
        },
        Answer::Bucket { bucket } => QuizAnswerDto::Bucket {
            bucket: bucket_to_dto(*bucket),
        },
        Answer::Typed { value } => QuizAnswerDto::Typed { value: *value },
        Answer::NotSure => QuizAnswerDto::NotSure,
        Answer::Skip => QuizAnswerDto::Skip,
        Answer::UseWhatIHave => QuizAnswerDto::UseWhatIHave,
    }
}

// ---------------------------------------------------------------------------------------------------
// The fit report
// ---------------------------------------------------------------------------------------------------

/// The wire fit report.
#[must_use]
pub fn fit_report_to_dto(report: &FitReport) -> FitReportDto {
    FitReportDto {
        per_stat: report
            .per_stat
            .iter()
            .map(|s| StatFitDto {
                stat: s.stat.clone(),
                value: s.value,
                predicted: s.predicted,
                p50: IntervalDto {
                    low: s.p50.low,
                    high: s.p50.high,
                },
                p80: IntervalDto {
                    low: s.p80.low,
                    high: s.p80.high,
                },
                rank: RankDto {
                    below: count(s.rank.below),
                    equal: count(s.rank.equal),
                    total: count(s.rank.total),
                },
                level: fit_level_to_dto(s.level),
                nearest_reference: s.nearest_reference,
                default_band: s.default_band,
            })
            .collect(),
        typicality: report.typicality,
        summary: FitSummaryDto {
            scored: count(report.summary.scored),
            typical: count(report.summary.typical),
            plausible: count(report.summary.plausible),
            unusual: count(report.summary.unusual),
            share_in_p80: report.summary.share_in_p80,
        },
        notices: FitNoticesDto {
            calibrated_at_ms: report.notices.calibrated_at_ms,
            pool_size: count(report.notices.pool_size),
            class_label: report.notices.class_label.clone(),
            class_n: count(report.notices.class_n),
            rough: report.notices.rough,
            optimistic: report.notices.optimistic,
        },
        unscored: report.unscored.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------------------------------

fn stat_metrics(m: &StatMetrics) -> StatMetricsDto {
    StatMetricsDto {
        n: count(m.n),
        median_error: m.median_error,
        p80_error: m.p80_error,
        mean_error: m.mean_error,
        factor_p50: m.factor_p50,
        factor_p80: m.factor_p80,
        shift: m.shift,
        coverage_p50: m.coverage_p50,
        coverage_p80: m.coverage_p80,
        rank_correlation: m.rank_correlation,
        baseline_median_error: m.baseline_median_error,
    }
}

fn variant_metrics(v: &VariantMetrics) -> VariantMetricsDto {
    VariantMetricsDto {
        name: v.name.clone(),
        replicates: count(v.replicates),
        per_stat: v
            .per_stat
            .iter()
            .map(|(k, m)| (k.clone(), stat_metrics(m)))
            .collect::<BTreeMap<_, _>>(),
        macro_median_error: v.macro_median_error,
        macro_mean_error: v.macro_mean_error,
        mean_questions: v.mean_questions,
        max_questions: count(v.max_questions),
    }
}

fn twin_check(t: &TwinCheck) -> TwinCheckDto {
    TwinCheckDto {
        plain_error: t.plain_error,
        twin_free_error: t.twin_free_error,
        gap: t.gap,
        optimistic: t.optimistic,
    }
}

/// The wire result of a calibration.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] when the metrics are of the apparel kind, which has no wire kind.
pub fn calibration_to_dto(
    metrics: &CalibrationMetrics,
    from_cache: bool,
) -> ToolkitResult<CalibrateResultDto> {
    Ok(CalibrateResultDto {
        kind: pool_kind_to_dto(metrics.kind)?,
        harness_version: metrics.harness_version,
        pool_size: count(metrics.pool_size),
        calibrated_at_ms: metrics.calibrated_at_ms,
        variants: metrics
            .variants
            .iter()
            .map(|(k, v)| (k.clone(), variant_metrics(v)))
            .collect(),
        twin: metrics.twin.as_ref().map(twin_check),
        from_cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::model::{CePatchSpec, ScalarField, ToolSpec};
    use serde_json::Value;

    fn json<T: serde::Serialize>(v: &T) -> Value {
        serde_json::to_value(v).unwrap_or(Value::Null)
    }

    fn sample_ranged() -> DesignSpec {
        let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        spec.tech_level = Some(TechLevel::Industrial);
        spec.role = Some("rifle".into());
        spec.offer(ScalarField::Damage, 11.0, ValueSource::Typed);
        spec.offer(ScalarField::Warmup, 1.2, ValueSource::Suggested);
        spec.offer(ScalarField::BurstCount, 3.0, ValueSource::Answered);
        spec.offer(ScalarField::Mass, 3.4, ValueSource::Anchor);
        spec
    }

    fn sample_melee() -> DesignSpec {
        let mut spec = DesignSpec::new_melee("RS_TestBlade", "test blade");
        spec.tools
            .push(ToolSpec::new("blade", &["Cut"]).with_numbers(18.0, 2.4, ValueSource::Typed));
        spec.ce = Some(CePatchSpec {
            ammo_set: Some("RS_AmmoSet".into()),
            one_handed: true,
            ..CePatchSpec::default()
        });
        spec
    }

    #[test]
    fn spec_json_is_identical_on_both_sides_for_ranged_and_melee_with_ce() {
        for spec in [sample_ranged(), sample_melee()] {
            let dto = spec_to_dto(&spec).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(json(&dto), json(&spec));
            let back = spec_from_dto(&dto).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(back, spec);
        }
    }

    #[test]
    fn draft_json_is_identical_on_both_sides_including_answers_and_anchors() {
        let mut draft = Draft::new(sample_ranged());
        draft.calibration = CalibrationMode::Quiz;
        draft.answers.insert(
            "tier".into(),
            serde_json::json!({"seq": 0, "answer": {"kind": "tier", "tier": 2}}),
        );
        draft.anchors.push(DraftAnchor::new("RS_Anchor"));
        draft.cloned_from = Some("RS_Anchor".into());
        let dto = draft_to_dto(&draft).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(json(&dto), json(&draft));
        assert_eq!(dto.cloned_from.as_deref(), Some("RS_Anchor"));
        assert_eq!(dto.anchors, vec![anchor_to_dto(&draft.anchors[0])]);
        assert_eq!(draft_from_dto(&dto).ok(), Some(draft));
    }

    #[test]
    fn a_draft_whose_envelope_kind_differs_from_its_spec_is_refused() {
        let mut dto = draft_to_dto(&Draft::new(sample_ranged())).unwrap_or_else(|e| panic!("{e}"));
        dto.kind = ItemKindDto::Melee;
        let err = draft_from_dto(&dto).err();
        assert_eq!(
            err.map(|e| e.code().to_owned()).as_deref(),
            Some("designer.invalid-draft")
        );
    }

    #[test]
    fn a_draft_from_a_newer_build_is_refused_with_its_own_code() {
        let mut dto = draft_to_dto(&Draft::new(sample_ranged())).unwrap_or_else(|e| panic!("{e}"));
        dto.schema_version = Draft::VERSION + 1;
        let err = draft_from_dto(&dto).err();
        assert_eq!(
            err.map(|e| e.code().to_owned()).as_deref(),
            Some("designer.draft-newer-schema")
        );
    }

    #[test]
    fn enum_strings_agree_for_every_variant() {
        for t in TechLevel::ALL {
            assert_eq!(json(&t), json(&tier_to_dto(t)));
            assert_eq!(tier_from_dto(tier_to_dto(t)), t);
            assert_eq!(tier_from_index(t.index()), Some(t));
        }
        for k in [ItemKind::Ranged, ItemKind::Melee] {
            let dto = kind_to_dto(k).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(json(&k), json(&dto));
            assert_eq!(kind_from_dto(dto), k);
        }
        for m in [
            CalibrationMode::Simple,
            CalibrationMode::Quiz,
            CalibrationMode::Anchored,
        ] {
            assert_eq!(json(&m), json(&mode_to_dto(m)));
        }
        for s in ValueSource::ALL {
            assert_eq!(json(&s), json(&value_source_to_dto(s)));
        }
        for l in ChainLevel::CHAIN {
            assert_eq!(json(&l), json(&chain_level_to_dto(l)));
        }
        for p in [
            ClassPredictor::Median,
            ClassPredictor::Quantile,
            ClassPredictor::Anchor,
        ] {
            assert_eq!(json(&p), json(&predictor_to_dto(p)));
        }
        for s in [
            Source::Typed,
            Source::Answer,
            Source::Anchor,
            Source::ClassMedian,
            Source::Quantile,
            Source::Derived,
        ] {
            assert_eq!(json(&s), json(&source_to_dto(s)));
        }
        for l in [FitLevel::Typical, FitLevel::Plausible, FitLevel::Unusual] {
            assert_eq!(json(&l), json(&fit_level_to_dto(l)));
        }
        for b in [Bucket::Lower, Bucket::Similar, Bucket::Higher] {
            assert_eq!(json(&b), json(&bucket_to_dto(b)));
            assert_eq!(bucket_from_dto(bucket_to_dto(b)), b);
        }
    }

    #[test]
    fn every_answer_converts_both_ways_with_identical_json() {
        let answers = [
            Answer::Tier { tier: 2 },
            Answer::Role {
                role: "rifle".into(),
            },
            Answer::Group {
                group: "burst".into(),
            },
            Answer::Weaker,
            Answer::Same,
            Answer::Stronger,
            Answer::CloserToLower,
            Answer::CloserToUpper,
            Answer::Bin { index: 3 },
            Answer::Bucket {
                bucket: Bucket::Higher,
            },
            Answer::Typed { value: 1.5 },
            Answer::NotSure,
            Answer::Skip,
            Answer::UseWhatIHave,
        ];
        for a in answers {
            let dto = answer_to_dto(&a);
            assert_eq!(json(&dto), json(&a), "{a:?}");
            assert_eq!(answer_from_dto(&dto), a);
        }
    }

    #[test]
    fn every_question_converts_with_identical_kind_tags() {
        let card = AnchorCard {
            id: "RS_A".into(),
            label: "a".into(),
            strength: 2.0,
            tier: 2,
            role: "rifle".into(),
            stats: BTreeMap::from([("mass".to_owned(), 3.0)]),
        };
        let questions = vec![
            Question::Tier {
                options: vec![TierOption {
                    tier: 2,
                    label: "Industrial".into(),
                    count: 4,
                }],
            },
            Question::Role {
                options: vec![NamedOption {
                    name: "rifle".into(),
                    count: 4,
                }],
            },
            Question::Compare {
                anchor: card.clone(),
                remaining: 2,
                asked: 1,
                budget: 3,
                information: 1.1,
            },
            Question::CloserTo {
                lower: card.clone(),
                upper: card.clone(),
            },
            Question::Group {
                options: vec![NamedOption {
                    name: "burst".into(),
                    count: 2,
                }],
            },
            Question::Interval {
                stat: "range".into(),
                bins: vec![
                    Bin {
                        lo: None,
                        hi: Some(20.0),
                        label: "under 20".into(),
                    },
                    Bin {
                        lo: Some(20.0),
                        hi: None,
                        label: "over 20".into(),
                    },
                ],
                spread: 0.3,
            },
            Question::VsAnchor {
                stat: "mass".into(),
                anchor: card,
                value: 3.0,
            },
        ];
        for q in questions {
            let dto = question_to_dto(&q);
            let (a, b) = (json(&q), json(&dto));
            assert_eq!(a["kind"], b["kind"], "{q:?}");
            // The wire uses camelCase field names, the engine snake case: compare after normalising.
            assert_eq!(snake_keys(&b), snake_keys(&a), "{q:?}");
        }
    }

    fn snake_keys(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, v)| (snake(k), snake_keys(v)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(snake_keys).collect()),
            other => other.clone(),
        }
    }

    fn snake(key: &str) -> String {
        let mut out = String::new();
        for c in key.chars() {
            if c.is_ascii_uppercase() {
                out.push('_');
                out.push(c.to_ascii_lowercase());
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn counts_saturate() {
        assert_eq!(count(7), 7);
        assert_eq!(count(usize::MAX), u32::MAX);
    }

    #[test]
    fn source_to_value_source_keeps_typed_and_ranks_answers() {
        assert_eq!(source_to_value_source(Source::Typed), ValueSource::Typed);
        assert_eq!(
            source_to_value_source(Source::Answer),
            ValueSource::Answered
        );
        assert_eq!(source_to_value_source(Source::Anchor), ValueSource::Anchor);
        assert_eq!(
            source_to_value_source(Source::Quantile),
            ValueSource::Suggested
        );
    }

    #[test]
    fn pool_kinds_map_and_apparel_is_refused() {
        assert_eq!(pool_kind(ItemKind::Melee).ok(), Some(PoolKind::Melee));
        assert!(pool_kind_to_dto(PoolKind::Apparel).is_err());
        assert_eq!(
            pool_kind_to_dto(PoolKind::Ranged).ok(),
            Some(ItemKindDto::Ranged)
        );
    }
}
