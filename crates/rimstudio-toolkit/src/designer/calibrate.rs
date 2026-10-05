//! Calibration: the leave one out harness over the reference pool of a kind, cached on disk.
//!
//! [`calibrate`] is the body of the `designer_calibrate` job. It builds nothing itself: the pool comes from
//! the engine of the current snapshot, [`rimstudio_design::loo::validate`] runs the harness, and the result
//! is stored in the `designer-calibration` collection of the cache root. The cache key is a hash of
//! everything that decides the numbers: the pool (every reference item with its tier, role, group, strength
//! and stats, so a different game version, DLC set or reference mod gives a different key), the baseline
//! configuration, the answer profiles, the quiz configuration, the twin check settings and the harness
//! version. Threads and the date are not part of the key. The cache holds one document per key, so switching
//! between reference sets does not thrash; at most [`MAX_KEPT`] documents per kind are kept.
//!
//! The harness is deterministic: the result is identical for any thread count (IT-071). To make a cached
//! result identical to a fresh one, the metrics always pass through their JSON text before they are stored
//! and returned, so both paths hand out the same numbers bit for bit.

use std::sync::Arc;

use rimstudio_core::jobs::{CancelToken, Progress, ProgressSink};
use rimstudio_design::baseline::{BandTable, BaselineConfig};
use rimstudio_design::classes::{ItemKind as PoolKind, Pool};
use rimstudio_design::loo::{
    CalibrationMetrics, HARNESS_VERSION, MIN_ITEMS, ValidateOptions, validate,
};
use rimstudio_io::schema::Versioned;
use rimstudio_ipc_types::designer::{CalibrateResultDto, DesignerCalibrateRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ctx::{Ctx, Engine, baseline_config};
use super::dto::{calibration_to_dto, kind_from_dto, pool_kind};
use crate::error::{ToolkitError, ToolkitResult};

/// How many calibration documents are kept per kind.
pub const MAX_KEPT: usize = 6;

/// The most worker threads a calibration uses when the request does not say.
const MAX_AUTO_THREADS: usize = 8;

/// One cached calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationRecord {
    /// The pool kind, `ranged` or `melee`.
    pub kind: String,
    /// The cache key the metrics were computed for.
    pub key: String,
    /// The harness output.
    pub metrics: CalibrationMetrics,
}

impl Versioned for CalibrationRecord {
    const KIND: &'static str = "designer-calibration";
    const VERSION: u32 = 1;
}

impl CalibrationRecord {
    /// The listing summary kept in the collection index.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({
            "kind": self.kind,
            "key": self.key,
            "calibratedAtMs": self.metrics.calibrated_at_ms,
        })
    }
}

/// What the cache knows about the calibration of the current reference set.
#[derive(Debug, Clone)]
pub enum CalibrationState {
    /// A calibration for exactly this reference set.
    Fresh(Arc<CalibrationMetrics>),
    /// Calibrations exist, but for a different reference set (the game, a DLC or a mod changed).
    Stale,
    /// Nothing was ever calibrated for this kind.
    Missing,
}

impl CalibrationState {
    /// The metrics when the calibration is fresh.
    #[must_use]
    pub fn metrics(&self) -> Option<&Arc<CalibrationMetrics>> {
        match self {
            Self::Fresh(m) => Some(m),
            Self::Stale | Self::Missing => None,
        }
    }
}

fn kind_name(kind: PoolKind) -> &'static str {
    kind.as_str()
}

/// The options the harness runs with for the cache key and for the run: standard profiles, default quiz
/// configuration of the kind, twin check on the mass stat. The date and the thread count are filled in by
/// [`calibrate`] and do not enter the key.
#[must_use]
pub fn validate_options(now_unix_ms: Option<u64>, threads: usize) -> ValidateOptions {
    ValidateOptions {
        now_unix_ms,
        threads,
        ..ValidateOptions::default()
    }
}

/// The cache key of a calibration: sixteen hex characters of a hash over everything that decides the
/// numbers (see the module documentation).
#[must_use]
pub fn calibration_key(pool: &Pool, config: &BaselineConfig, options: &ValidateOptions) -> String {
    let quiz = options
        .quiz
        .clone()
        .unwrap_or_else(|| rimstudio_design::quiz::QuizConfig::for_kind(pool.kind));
    let material = json!({
        "harness": HARNESS_VERSION,
        "kind": kind_name(pool.kind),
        "pool": pool,
        "config": config,
        "profiles": options.profiles,
        "quiz": quiz,
        "twin": [options.twin_check, options.twin_stat, options.twin_tolerance],
    });
    let bytes = serde_json::to_vec(&material).unwrap_or_default();
    blake3::hash(&bytes)
        .to_hex()
        .as_str()
        .chars()
        .take(16)
        .collect()
}

fn doc_id(kind: PoolKind, key: &str) -> String {
    format!("{}-{key}", kind_name(kind))
}

/// Passes the metrics through their JSON text so that fresh and cached results are identical.
fn normalize(metrics: &CalibrationMetrics) -> ToolkitResult<CalibrationMetrics> {
    let text = serde_json::to_string(metrics).map_err(|e| {
        ToolkitError::internal(format!("calibration metrics cannot be encoded: {e}"))
    })?;
    serde_json::from_str(&text)
        .map_err(|e| ToolkitError::internal(format!("calibration metrics cannot be decoded: {e}")))
}

/// Looks up the calibration of the engine's current reference set for `kind` without computing anything.
///
/// The first lookup of a key may read one document; later ones are served from memory.
///
/// # Errors
///
/// [`ToolkitError::ReferenceUnavailable`] when the engine has no pools, [`ToolkitError::Store`] when the
/// cache cannot be read.
pub fn calibration_state(
    ctx: &Ctx,
    engine: &Engine,
    kind: PoolKind,
) -> ToolkitResult<CalibrationState> {
    let key = engine.calibration_key(kind)?;
    if let Some(state) = ctx.remembered_calibration(key) {
        return Ok(state);
    }
    let state = read_state(ctx, kind, key)?;
    ctx.remember_calibration(key, state.clone());
    Ok(state)
}

fn read_state(ctx: &Ctx, kind: PoolKind, key: &str) -> ToolkitResult<CalibrationState> {
    if let Some(loaded) = ctx.calibrations().get(&doc_id(kind, key))?
        && loaded.value.key == key
    {
        return Ok(CalibrationState::Fresh(Arc::new(loaded.value.metrics)));
    }
    let kind_text = kind_name(kind);
    let any = ctx
        .calibrations()
        .list_summaries()?
        .iter()
        .any(|e| e.summary.get("kind").and_then(Value::as_str) == Some(kind_text));
    Ok(if any {
        CalibrationState::Stale
    } else {
        CalibrationState::Missing
    })
}

/// The error bands of the fresh calibration for `kind`, `None` when there is none (the meter and the
/// suggestions then use the default bands and say so).
///
/// # Errors
///
/// See [`calibration_state`].
pub fn bands_for(
    ctx: &Ctx,
    engine: &Engine,
    kind: PoolKind,
    mode: rimstudio_design::loo::CalibrationMode,
) -> ToolkitResult<Option<BandTable>> {
    Ok(calibration_state(ctx, engine, kind)?
        .metrics()
        .map(|m| m.bands_for(mode)))
}

fn prune(ctx: &Ctx, kind: PoolKind) -> ToolkitResult<()> {
    let kind_text = kind_name(kind);
    let mut mine: Vec<(u64, String)> = ctx
        .calibrations()
        .list_summaries()?
        .into_iter()
        .filter(|e| e.summary.get("kind").and_then(Value::as_str) == Some(kind_text))
        .map(|e| {
            let at = e
                .summary
                .get("calibratedAtMs")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            (at, e.id)
        })
        .collect();
    if mine.len() <= MAX_KEPT {
        return Ok(());
    }
    // Newest first; the tail goes.
    mine.sort_by(|a, b| b.cmp(a));
    for (_, id) in mine.into_iter().skip(MAX_KEPT) {
        ctx.calibrations().delete(&id)?;
    }
    Ok(())
}

fn check_cancel(cancel: &CancelToken) -> ToolkitResult<()> {
    cancel.check().map_err(ToolkitError::from)
}

/// Runs (or reads from the cache) the calibration of one kind: the body of the `designer_calibrate` job.
///
/// Progress is reported in three steps (pool, harness, store). The harness of `rimstudio-design` has no
/// cancel points of its own, so cancellation is observed before it starts and before the result is stored;
/// a cancelled run stores nothing.
///
/// # Errors
///
/// [`ToolkitError::ReferenceUnavailable`] without a reference set, [`ToolkitError::CalibrationUnavailable`]
/// when the pool has fewer than four items, [`ToolkitError::Cancelled`], [`ToolkitError::Design`] for an
/// error of the harness and [`ToolkitError::Store`] for a cache failure.
pub fn calibrate(
    ctx: &Ctx,
    req: DesignerCalibrateRequest,
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> ToolkitResult<CalibrateResultDto> {
    const STEPS: u64 = 3;
    let kind = pool_kind(kind_from_dto(req.kind))?;
    check_cancel(cancel)?;
    progress.report(Progress::new("pool", 0).with_total(STEPS));
    let engine = ctx.require_engine()?;
    let pool = engine.pool(kind)?;
    if pool.len() < MIN_ITEMS {
        return Err(ToolkitError::CalibrationUnavailable {
            reason: format!(
                "the {} pool has {} reference items and calibration needs at least {MIN_ITEMS}",
                kind_name(kind),
                pool.len()
            ),
        });
    }
    let config = baseline_config(kind);
    let key = engine.calibration_key(kind)?.to_owned();
    if !req.force
        && let CalibrationState::Fresh(metrics) = calibration_state(ctx, &engine, kind)?
    {
        progress.report(Progress::new("done", STEPS).with_total(STEPS));
        return calibration_to_dto(&metrics, true);
    }
    check_cancel(cancel)?;
    progress.report(Progress::new("harness", 1).with_total(STEPS));
    let threads = req.threads.map_or_else(
        || {
            std::thread::available_parallelism()
                .map_or(1, std::num::NonZero::get)
                .min(MAX_AUTO_THREADS)
        },
        |n| usize::from(n).max(1),
    );
    let options = validate_options(Some(ctx.now_ms()), threads);
    let metrics = normalize(&validate(pool, &config, &options)?)?;
    check_cancel(cancel)?;
    progress.report(Progress::new("store", 2).with_total(STEPS));
    let record = CalibrationRecord {
        kind: kind_name(kind).to_owned(),
        key: key.clone(),
        metrics,
    };
    ctx.calibrations().put(&doc_id(kind, &key), &record)?;
    prune(ctx, kind)?;
    let metrics = Arc::new(record.metrics);
    ctx.remember_calibration(&key, CalibrationState::Fresh(metrics.clone()));
    progress.report(Progress::new("done", STEPS).with_total(STEPS));
    calibration_to_dto(&metrics, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::classes::PoolOptions;
    use rimstudio_design::classes::synthetic::{SyntheticSpec, synthetic_items};

    fn pool(n: usize, seed: u64) -> Pool {
        let items = synthetic_items(&SyntheticSpec {
            n,
            seed,
            ..SyntheticSpec::default()
        });
        Pool::build(PoolKind::Ranged, &items, &PoolOptions::default())
    }

    #[test]
    fn the_key_is_sixteen_hex_characters_and_stable() {
        let p = pool(20, 1);
        let a = calibration_key(
            &p,
            &baseline_config(PoolKind::Ranged),
            &validate_options(None, 1),
        );
        let b = calibration_key(
            &p,
            &baseline_config(PoolKind::Ranged),
            &validate_options(None, 1),
        );
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn threads_and_the_date_are_not_part_of_the_key() {
        let p = pool(20, 1);
        let config = baseline_config(PoolKind::Ranged);
        assert_eq!(
            calibration_key(&p, &config, &validate_options(None, 1)),
            calibration_key(&p, &config, &validate_options(Some(5), 8))
        );
    }

    #[test]
    fn a_different_pool_or_configuration_changes_the_key() {
        let config = baseline_config(PoolKind::Ranged);
        let options = validate_options(None, 1);
        let base = calibration_key(&pool(20, 1), &config, &options);
        assert_ne!(base, calibration_key(&pool(21, 1), &config, &options));
        assert_ne!(base, calibration_key(&pool(20, 2), &config, &options));
        assert_ne!(
            base,
            calibration_key(&pool(20, 1), &BaselineConfig::default(), &options)
        );
        let mut fewer = validate_options(None, 1);
        fewer.profiles.truncate(2);
        assert_ne!(base, calibration_key(&pool(20, 1), &config, &fewer));
    }

    #[test]
    fn the_document_id_names_the_kind_and_the_key() {
        assert_eq!(doc_id(PoolKind::Melee, "abc"), "melee-abc");
        rimstudio_io::collection::validate_id(&doc_id(PoolKind::Ranged, "0123456789abcdef"))
            .unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn normalising_metrics_is_idempotent() {
        let p = pool(12, 3);
        let metrics = validate(
            &p,
            &baseline_config(PoolKind::Ranged),
            &validate_options(Some(1), 1),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let once = normalize(&metrics).unwrap_or_else(|e| panic!("{e}"));
        let twice = normalize(&once).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(once, twice);
    }

    #[test]
    fn the_record_summary_carries_kind_key_and_date() {
        let p = pool(12, 3);
        let metrics = validate(
            &p,
            &baseline_config(PoolKind::Ranged),
            &validate_options(Some(99), 1),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let record = CalibrationRecord {
            kind: "ranged".into(),
            key: "k".into(),
            metrics,
        };
        let s = record.summary();
        assert_eq!(s["kind"], "ranged");
        assert_eq!(s["key"], "k");
        assert_eq!(s["calibratedAtMs"], 99);
    }
}
