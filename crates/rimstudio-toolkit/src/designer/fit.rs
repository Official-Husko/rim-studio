//! The fit meter: how typical each number of a design is among the reference weapons.
//!
//! [`fit`] scores the numbers of a draft against the class statistics and the calibrated error bands
//! (`designer_fit`). The prediction behind each stat is the estimate from the dialogue alone (tier, role,
//! strength choice, quiz answers, anchors) and not the typed values: the meter asks whether the typed
//! numbers fit what the answers say. Typed values are compared with the bands and never changed, and the
//! meter has no state, so the caller asks again after every edit. There is no red level: the levels are
//! typical, plausible and unusual.
//!
//! Without a fresh calibration the bands are the documented defaults and each stat says so
//! (`defaultBand`); the notices then carry no calibration date.

use std::collections::BTreeMap;

use rimstudio_design::fit::{Proposal, score_with};
use rimstudio_design::loo::{CalibrationMetrics, HARNESS_VERSION};
use rimstudio_ipc_types::designer::{DesignerFitRequest, FitReportDto};

use super::calibrate::calibration_state;
use super::ctx::Ctx;
use super::dto::{draft_from_dto, fit_report_to_dto, pool_kind};
use super::preview::{baseline_input, estimate_for, loo_mode, spec_stats};
use crate::error::{ToolkitError, ToolkitResult};

/// Stats of the spec that are not scored by the meter (a count of attacks says nothing about strength).
const UNSCORED: [&str; 1] = ["tools"];

/// Scores a draft (`designer_fit`).
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable draft, [`ToolkitError::ReferenceUnavailable`] without
/// reference weapons of the kind, [`ToolkitError::Design`] when the estimation rejects an input.
pub fn fit(ctx: &Ctx, req: DesignerFitRequest) -> ToolkitResult<FitReportDto> {
    let draft = draft_from_dto(&req.draft)?;
    let kind = pool_kind(draft.kind)?;
    let engine = ctx.require_engine()?;
    let model = engine
        .model(kind)?
        .ok_or_else(|| ToolkitError::ReferenceUnavailable {
            reason: "the loaded defs hold no reference weapons of this kind".to_owned(),
        })?;
    let estimate = estimate_for(ctx, &engine, &model, &draft, false)?;
    let input = baseline_input(&model, &draft, false)?;
    let class = model.class_stats(&input.key);
    let known = model.pool.stat_names();
    let values: BTreeMap<String, f64> = spec_stats(&draft.spec)
        .into_iter()
        .filter(|(k, _)| known.iter().any(|n| n == k) && !UNSCORED.contains(&k.as_str()))
        .collect();
    let proposal = Proposal {
        values,
        predicted: BTreeMap::new(),
    }
    .with_estimate(&estimate);
    let metrics = match calibration_state(ctx, &engine, kind)? {
        super::calibrate::CalibrationState::Fresh(m) => (*m).clone(),
        _ => CalibrationMetrics {
            harness_version: HARNESS_VERSION,
            kind,
            pool_size: model.pool.len(),
            calibrated_at_ms: None,
            variants: BTreeMap::new(),
            twin: None,
        },
    };
    let report = score_with(&proposal, &class, &metrics, loo_mode(&draft));
    Ok(fit_report_to_dto(&report))
}
