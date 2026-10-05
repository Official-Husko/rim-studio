//! The preview: exact readouts, suggestions with source labels and bands, and static validation.
//!
//! [`preview`] is the body of `designer_preview` and is pure and fast: it reads the cached engine (pools and
//! a fitted baseline model of the current snapshot) and the cached calibration bands, does arithmetic and
//! returns. Three kinds of answer come out of it, kept strictly apart:
//!
//! - **Readouts** are exact results of the game formulas on the numbers of the spec (cycle time, DPS, hit
//!   adjusted DPS, implied armor penetration, the strength index, melee stat panel and in fight DPS, the
//!   market value). They carry no band. When an input of a formula is missing the readout has no value.
//! - **Suggestions** are estimates for the form fields, each with the name of its source (typed, answer,
//!   anchor, class median, quantile, derived), the fallback level and pool size behind it, and the band.
//!   A field whose value the user typed is marked `locked` and its suggestion repeats the typed value.
//! - **Diagnostics** are the static validation results with field pointers.
//!
//! The Combat Extended readouts appear only when the spec carries a `ce` block (the user opted in) and
//! Combat Extended data is part of the reference set. With `ce` absent no CE readout exists, whatever is
//! installed (owner rule: vanilla by default).
//!
//! [`apply_estimate`] and [`suggest_fill`] are the write side of suggestions: both go through
//! `DesignSpec::offer`, so a typed value is never overwritten (IT-003).

use std::collections::BTreeMap;

use rimstudio_core::diag::Diagnostic;
use rimstudio_defs::DefDatabases;
use rimstudio_design::baseline::{
    BandTable, BaselineInput, Estimate, Model, StatConstraint, StrengthChoice, StrengthInput,
};
use rimstudio_design::ce::patchgen::values::predict_for;
use rimstudio_design::ce::reader::CeModel;
use rimstudio_design::classes::ClassKey;
use rimstudio_design::loo::CalibrationMode as LooMode;
use rimstudio_design::melee::{
    MeleeAttack, MeleeModifiers, enumerate_attacks, in_fight_dps, stat_panel_dps,
    strength as melee_strength,
};
use rimstudio_design::model::{
    CalibrationMode, DesignSpec, Draft, ItemKind, ScalarField, ValueSource,
};
use rimstudio_design::price::{IngredientLine, PriceInput, StuffCost, market_value};
use rimstudio_design::ranged::{
    ReferenceArmor, SustainedInput, cycle_time, hit_adjusted_dps, nominal_dps, strength_p4,
    sustained_dps,
};
use rimstudio_design::reader::access::number_map;
use rimstudio_design::validation::codes::{CALIBRATION_STALE, CE_ABSENT, POOL_THIN};
use rimstudio_design::validation::{validate_refs, validate_spec};
use rimstudio_ipc_types::designer::{
    DesignerPreviewRequest, DraftDto, EstimateSummaryDto, PreviewDto, ReadoutDto, ReadoutGroupDto,
    ReadoutStepDto, ReadoutUnitDto, SuggestionBandDto, SuggestionDto,
};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;

use super::calibrate::{CalibrationState, calibration_state};
use super::ctx::{Ctx, Engine};
use super::dto::{
    band_to_dto, chain_level_to_dto, count, draft_from_dto, draft_to_dto, estimate_summary,
    pool_kind, predictor_to_dto, source_to_dto, source_to_value_source,
};
use super::quiz::{STRENGTH_KEY, quiz_from_draft};
use crate::error::ToolkitResult;

/// Distances in tiles at which the hit adjusted DPS is read (short and long engagement).
pub const HIT_DISTANCES: [f64; 2] = [12.0, 25.0];

/// Fewer reference weapons than this behind the bands is reported as `design.pool-thin`.
pub const THIN_POOL: usize = 15;

// ---------------------------------------------------------------------------------------------------
// Stats and fields
// ---------------------------------------------------------------------------------------------------

/// The stats of the reference pool the designer suggests, in display order, for a kind.
#[must_use]
pub fn suggested_stats(kind: ItemKind) -> &'static [&'static str] {
    match kind {
        ItemKind::Melee => &["swing_damage", "cooldown", "mass", "work"],
        _ => &[
            "damage",
            "warmup",
            "cooldown",
            "range",
            "burst",
            "ticks_between",
            "touch",
            "short",
            "medium",
            "long",
            "mass",
            "work",
        ],
    }
}

/// The spec fields a reference stat is written to. A melee stat maps to the same field of every tool
/// (the pool describes the stat panel of the whole weapon); a melee spec without tools still lists the
/// first tool's field so the caller knows where the value goes.
#[must_use]
pub fn fields_of_stat(spec: &DesignSpec, stat: &str) -> Vec<ScalarField> {
    let tools = spec.tools.len().max(1);
    match (spec.kind, stat) {
        (_, "mass") => vec![ScalarField::Mass],
        (_, "work") => vec![ScalarField::WorkToMake],
        (ItemKind::Melee, "swing_damage") => (0..tools).map(ScalarField::ToolPower).collect(),
        (ItemKind::Melee, "cooldown") => (0..tools).map(ScalarField::ToolCooldown).collect(),
        (ItemKind::Melee, _) => Vec::new(),
        (_, "damage") => vec![ScalarField::Damage],
        (_, "warmup") => vec![ScalarField::Warmup],
        (_, "cooldown") => vec![ScalarField::Cooldown],
        (_, "range") => vec![ScalarField::Range],
        (_, "burst") => vec![ScalarField::BurstCount],
        (_, "ticks_between") => vec![ScalarField::TicksBetweenBurstShots],
        (_, "touch") => vec![ScalarField::AccuracyTouch],
        (_, "short") => vec![ScalarField::AccuracyShort],
        (_, "medium") => vec![ScalarField::AccuracyMedium],
        (_, "long") => vec![ScalarField::AccuracyLong],
        _ => Vec::new(),
    }
}

/// The values of the spec as reference stats: what the fit meter compares with the pool. Computed values
/// (nominal DPS, implied armor penetration, melee stat panel numbers) are included.
#[must_use]
pub fn spec_stats(spec: &DesignSpec) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let mut put = |name: &str, v: Option<f64>| {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            out.insert(name.to_owned(), v);
        }
    };
    put("mass", spec.scalar(ScalarField::Mass).map(|s| s.value));
    put(
        "work",
        spec.scalar(ScalarField::WorkToMake).map(|s| s.value),
    );
    match spec.kind {
        ItemKind::Melee => {
            let attacks = melee_attacks(spec);
            if let Some(attacks) = attacks.as_deref() {
                put("tools", Some(attacks.len() as f64));
                if let Ok(panel) = stat_panel_dps(attacks) {
                    put("swing_damage", Some(panel.damage));
                    put("cooldown", Some(panel.cooldown));
                    put("dps", Some(panel.dps));
                    put("ap", Some(panel.armor_penetration));
                }
                put("fight_dps", in_fight_dps(attacks).ok().map(|f| f.dps));
            }
        }
        _ => {
            for (stat, field) in [
                ("damage", ScalarField::Damage),
                ("range", ScalarField::Range),
                ("warmup", ScalarField::Warmup),
                ("cooldown", ScalarField::Cooldown),
                ("burst", ScalarField::BurstCount),
                ("ticks_between", ScalarField::TicksBetweenBurstShots),
                ("touch", ScalarField::AccuracyTouch),
                ("short", ScalarField::AccuracyShort),
                ("medium", ScalarField::AccuracyMedium),
                ("long", ScalarField::AccuracyLong),
            ] {
                put(stat, spec.scalar(field).map(|s| s.value));
            }
            if let Some(profile) = spec.ranged_profile() {
                put("dps", nominal_dps(&profile).ok());
                put("ap", Some(profile.armor_penetration));
            }
        }
    }
    out
}

/// The numbers the user typed, as constraints on the reference stats the pool has.
fn typed_constraints(spec: &DesignSpec, model: &Model) -> BTreeMap<String, StatConstraint> {
    let known = model.pool.stat_names();
    let mut out = BTreeMap::new();
    for stat in suggested_stats(spec.kind) {
        if !known.iter().any(|k| k == stat) {
            continue;
        }
        let fields = fields_of_stat(spec, stat);
        // A stat that maps to several tool fields is constrained only when exactly one tool exists.
        let [field] = fields.as_slice() else { continue };
        if let Some(s) = spec.scalar(*field)
            && s.is_typed()
        {
            out.insert((*stat).to_owned(), StatConstraint::Typed { value: s.value });
        }
    }
    out
}

fn melee_attacks(spec: &DesignSpec) -> Option<Vec<MeleeAttack>> {
    let tools = spec.melee_tools();
    if tools.is_empty() {
        return None;
    }
    enumerate_attacks(&tools, &MeleeModifiers::default()).ok()
}

// ---------------------------------------------------------------------------------------------------
// Readouts
// ---------------------------------------------------------------------------------------------------

/// What the readouts may read besides the spec.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadoutEnv<'a> {
    /// Sharp ratings of the reference armor layers of the strength index (from the user's apparel).
    pub armor: Option<&'a [f64]>,
    /// The resolved defs, for ingredient prices. Without them the market value has no value.
    pub defs: Option<&'a DefDatabases>,
    /// The Combat Extended model; with the spec's `ce` block it enables the CE readouts.
    pub ce: Option<&'a CeModel>,
}

fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite())
}

fn readout(
    key: &str,
    group: ReadoutGroupDto,
    unit: ReadoutUnitDto,
    value: Option<f64>,
) -> ReadoutDto {
    ReadoutDto {
        key: key.to_owned(),
        group,
        unit,
        value: finite(value),
        steps: Vec::new(),
    }
}

fn step(label: &str, value: f64) -> ReadoutStepDto {
    ReadoutStepDto {
        label: label.to_owned(),
        value,
    }
}

fn ranged_readouts(spec: &DesignSpec, env: &ReadoutEnv<'_>, out: &mut Vec<ReadoutDto>) {
    use ReadoutGroupDto::Ranged as G;
    use ReadoutUnitDto::{DamagePerSecond, Fraction, Number, Seconds};
    let profile = spec.ranged_profile();
    let cycle = profile
        .as_ref()
        .and_then(|p| cycle_time(&p.cycle_input()).ok());
    let mut r = readout("cycle-time", G, Seconds, cycle);
    if let (Some(p), Some(c)) = (profile.as_ref(), r.value) {
        let burst_gaps = f64::from(p.burst_count.saturating_sub(1)) * p.ticks_between_shots / 60.0;
        r.steps = vec![
            step("warmup", p.warmup),
            step("warmup plus cooldown", p.warmup + p.cooldown),
            step("burst gaps", burst_gaps),
            step("cycle time", c),
        ];
    }
    out.push(r);
    let dps = profile.as_ref().and_then(|p| nominal_dps(p).ok());
    let mut r = readout("dps", G, DamagePerSecond, dps);
    if let (Some(p), Some(c), Some(d)) = (profile.as_ref(), cycle, r.value) {
        r.steps = vec![
            step("damage per burst", p.damage * f64::from(p.burst_count)),
            step("cycle time", c),
            step("damage per second", d),
        ];
    }
    out.push(r);
    for distance in HIT_DISTANCES {
        // Without the four accuracy values there is no hit chance to apply: no value, not a guess.
        let value = profile
            .as_ref()
            .filter(|p| p.accuracy.is_some())
            .and_then(|p| hit_adjusted_dps(p, distance).ok());
        out.push(readout(
            &format!("hit-dps-{distance:.0}"),
            G,
            DamagePerSecond,
            value,
        ));
    }
    out.push(readout(
        "implied-ap",
        G,
        Fraction,
        profile.as_ref().map(|p| p.armor_penetration),
    ));
    let strength = match (profile.as_ref(), env.armor) {
        (Some(p), Some(ratings)) => strength_p4(
            p,
            &ReferenceArmor {
                sharp_ratings: ratings,
            },
        )
        .ok(),
        _ => None,
    };
    out.push(readout("strength-index", G, Number, strength));
}

fn melee_readouts(spec: &DesignSpec, out: &mut Vec<ReadoutDto>) {
    use ReadoutGroupDto::Melee as G;
    use ReadoutUnitDto::{DamagePerSecond, Fraction, Number, Seconds};
    let attacks = melee_attacks(spec);
    let panel = attacks.as_deref().and_then(|a| stat_panel_dps(a).ok());
    out.push(readout(
        "melee-swing-damage",
        G,
        Number,
        panel.as_ref().map(|p| p.damage),
    ));
    out.push(readout(
        "melee-swing-cooldown",
        G,
        Seconds,
        panel.as_ref().map(|p| p.cooldown),
    ));
    out.push(readout(
        "melee-dps",
        G,
        DamagePerSecond,
        panel.as_ref().map(|p| p.dps),
    ));
    out.push(readout(
        "melee-ap",
        G,
        Fraction,
        panel.as_ref().map(|p| p.armor_penetration),
    ));
    let fight = attacks.as_deref().and_then(|a| in_fight_dps(a).ok());
    let mut r = readout(
        "melee-fight-dps",
        G,
        DamagePerSecond,
        fight.as_ref().map(|f| f.dps),
    );
    if let (Some(f), Some(a)) = (fight.as_ref(), attacks.as_deref()) {
        r.steps = f
            .attacks
            .iter()
            .zip(a)
            .map(|(share, attack)| {
                step(
                    &format!("{} {} share", attack.tool, attack.capacity),
                    share.share,
                )
            })
            .collect();
    }
    out.push(r);
    out.push(readout(
        "strength-index",
        G,
        Number,
        attacks.as_deref().and_then(|a| melee_strength(a).ok()),
    ));
}

/// The market value of the ingredient def (its `MarketValue` stat base), when the defs hold it.
fn unit_value(defs: &DefDatabases, name: &str) -> Option<f64> {
    let def = defs.get("ThingDef", name)?;
    number_map(&def.node, "statBases")
        .get("MarketValue")
        .copied()
}

fn economy_readouts(spec: &DesignSpec, env: &ReadoutEnv<'_>, out: &mut Vec<ReadoutDto>) {
    let group = ReadoutGroupDto::Economy;
    let unit = ReadoutUnitDto::Silver;
    if let Some(explicit) = spec.scalar(ScalarField::MarketValue) {
        out.push(readout("market-value", group, unit, Some(explicit.value)));
        return;
    }
    let computed = (|| {
        let defs = env.defs?;
        let work = spec.scalar(ScalarField::WorkToMake)?.value;
        let mut ingredients = Vec::new();
        for c in &spec.cost_list {
            ingredients.push(IngredientLine {
                count: c.count,
                unit_value: unit_value(defs, &c.def_name)?,
            });
        }
        let stuff = spec
            .stuff
            .as_ref()
            .and_then(|s| s.count)
            .map(|c| StuffCost::Unknown { count: c.value });
        market_value(&PriceInput {
            ingredients,
            stuff,
            work_to_make: work,
            work_factor: 1.0,
        })
        .ok()
    })();
    let mut r = readout(
        "market-value",
        group,
        unit,
        computed.as_ref().map(|b| b.displayed),
    );
    if let Some(b) = computed {
        r.steps = vec![
            step("ingredients", b.ingredients),
            step("stuff", b.stuff),
            step("work", b.work),
            step("base value", b.base_value),
        ];
    }
    out.push(r);
}

fn ce_readouts(spec: &DesignSpec, model: &CeModel, out: &mut Vec<ReadoutDto>) {
    use ReadoutGroupDto::CombatExtended as G;
    use ReadoutUnitDto::{DamagePerSecond, Kilograms, Number, Seconds, Tiles};
    let Some(ce) = spec.ce.as_ref() else { return };
    if !model.is_present() {
        return;
    }
    let prediction = predict_for(spec, model);
    let predicted = |stat: &str| {
        prediction
            .as_ref()
            .and_then(|p| p.stats.get(stat))
            .map(|s| s.value)
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let bulk = ce.bulk.map(|b| b.value).or_else(|| predicted("bulk"));
    out.push(readout("ce-bulk", G, Number, bulk));
    if spec.kind != ItemKind::Ranged {
        return;
    }
    let ranged = spec.ranged.as_ref();
    let mass = predicted("mass").or_else(|| spec.mass.map(|m| m.value));
    let range = predicted("range").or_else(|| ranged.and_then(|r| r.range).map(|v| v.value));
    let warmup = predicted("warmup").or_else(|| ranged.and_then(|r| r.warmup).map(|v| v.value));
    let cooldown = ce
        .cooldown
        .map(|c| c.value)
        .or_else(|| predicted("cooldown"))
        .or_else(|| ranged.and_then(|r| r.cooldown).map(|v| v.value));
    out.push(readout("ce-mass", G, Kilograms, mass));
    out.push(readout("ce-range", G, Tiles, range));
    out.push(readout("ce-warmup", G, Seconds, warmup));
    out.push(readout("ce-cooldown", G, Seconds, cooldown));
    out.push(readout(
        "ce-magazine",
        G,
        Number,
        ce.magazine_size.map(|m| f64::from(m.value)),
    ));
    out.push(readout(
        "ce-reload",
        G,
        Seconds,
        ce.reload_time.map(|r| r.value),
    ));
    // Damage comes from the ammo, not from the gun: the first member of the chosen default projectile.
    let shot_damage = (|| {
        let set = model.ammo_set(ce.ammo_set.as_deref()?)?;
        let wanted = ce.default_projectile.as_deref()?;
        let ammo = set.ammo_types.iter().find(|a| a.projectile == wanted)?;
        let pellets = ammo.info.pellets.unwrap_or(1.0);
        Some(ammo.info.damage? * pellets)
    })();
    out.push(readout("ce-ammo-damage", G, Number, shot_damage));
    let sustained = (|| {
        let burst = ranged.and_then(|r| r.burst_count).map_or(1, |b| b.value);
        let ticks = ranged
            .and_then(|r| r.ticks_between_burst_shots)
            .map_or(rimstudio_design::model::DEFAULT_TICKS_BETWEEN_SHOTS, |t| {
                t.value
            });
        sustained_dps(&SustainedInput {
            damage: shot_damage?,
            magazine: ce.magazine_size?.value,
            burst_count: burst,
            ticks_between_shots: ticks,
            warmup: warmup?,
            cooldown: cooldown?,
            reload: ce.reload_time?.value,
        })
        .ok()
    })();
    out.push(readout("ce-sustained-dps", G, DamagePerSecond, sustained));
}

/// The exact readouts of a spec, in display order. Pure: the same spec and environment give the same
/// readouts.
#[must_use]
pub fn readouts(spec: &DesignSpec, env: &ReadoutEnv<'_>) -> Vec<ReadoutDto> {
    let mut out = Vec::new();
    match spec.kind {
        ItemKind::Melee => melee_readouts(spec, &mut out),
        _ => ranged_readouts(spec, env, &mut out),
    }
    economy_readouts(spec, env, &mut out);
    if let Some(model) = env.ce {
        ce_readouts(spec, model, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------------------------------
// The estimate behind the suggestions
// ---------------------------------------------------------------------------------------------------

/// The strength choice of a simple mode draft: the `strength` answer, typical when absent or unreadable.
fn simple_strength(draft: &Draft) -> StrengthInput {
    draft
        .answers
        .get(STRENGTH_KEY)
        .and_then(|v| serde_json::from_value::<StrengthInput>(v.clone()).ok())
        .unwrap_or(StrengthInput::Choice {
            choice: StrengthChoice::Typical,
        })
}

fn class_key(spec: &DesignSpec) -> ClassKey {
    let group = spec
        .ranged
        .as_ref()
        .and_then(|r| r.burst_count)
        .and_then(|b| {
            b.is_typed()
                .then(|| if b.value > 1 { "burst" } else { "single" }.to_owned())
        });
    ClassKey {
        role: spec.role.clone(),
        tier: spec.tech_level.map(|t| t.index()),
        group,
    }
}

/// The baseline input a draft describes: its calibration mode decides where the class and strength come
/// from. With `with_typed` the numbers the user typed become constraints, so every other estimate is made
/// around them; without it the input holds only what the dialogue says (the fit meter compares typed values
/// against that).
///
/// # Errors
///
/// [`crate::error::ToolkitError::InvalidDraft`] for an unreadable stored answer.
pub fn baseline_input(
    model: &Model,
    draft: &Draft,
    with_typed: bool,
) -> ToolkitResult<BaselineInput> {
    let spec = &draft.spec;
    let mut input = match draft.calibration {
        CalibrationMode::Quiz if model.quiz_available() => {
            let quiz = quiz_from_draft(model, draft)?.quiz;
            match quiz.input(model) {
                Ok(i) => i,
                Err(e) => return Err(super::quiz::map_quiz_error(e)),
            }
        }
        CalibrationMode::Anchored => {
            let ln: Vec<f64> = draft
                .anchors
                .iter()
                .filter_map(|a| model.pool.index_of(&a.def_name))
                .filter_map(|i| model.pool.item(i))
                .map(|item| item.ln_strength)
                .collect();
            let strength = if ln.is_empty() {
                StrengthInput::Unknown
            } else {
                StrengthInput::Placed {
                    ln_power: ln.iter().sum::<f64>() / ln.len() as f64,
                }
            };
            BaselineInput {
                key: class_key(spec),
                strength: Some(strength),
                constraints: BTreeMap::new(),
            }
        }
        _ => BaselineInput {
            key: class_key(spec),
            strength: Some(simple_strength(draft)),
            constraints: BTreeMap::new(),
        },
    };
    if with_typed {
        input.constraints.extend(typed_constraints(spec, model));
    }
    Ok(input)
}

/// The estimate of a draft against its model, with the bands of the fresh calibration when there is one.
///
/// # Errors
///
/// [`crate::error::ToolkitError::Design`] when the estimation rejects an input.
pub fn estimate_for(
    ctx: &Ctx,
    engine: &Engine,
    model: &Model,
    draft: &Draft,
    with_typed: bool,
) -> ToolkitResult<Estimate> {
    let kind = pool_kind(draft.kind)?;
    let input = baseline_input(model, draft, with_typed)?;
    let bands: Option<BandTable> = super::calibrate::bands_for(ctx, engine, kind, loo_mode(draft))?;
    Ok(match bands {
        Some(b) => model.estimate_with_bands(&input, &b)?,
        None => model.estimate(&input)?,
    })
}

/// The calibration bands a draft's mode reads: the simple run for simple mode, the noisy quiz run else.
#[must_use]
pub fn loo_mode(draft: &Draft) -> LooMode {
    match draft.calibration {
        CalibrationMode::Simple => LooMode::Simple,
        _ => LooMode::Quiz,
    }
}

fn suggestions(spec: &DesignSpec, estimate: &Estimate) -> Vec<SuggestionDto> {
    let mut out = Vec::new();
    for stat in suggested_stats(spec.kind) {
        let Some(est) = estimate.stats.get(*stat) else {
            continue;
        };
        for field in fields_of_stat(spec, stat) {
            let locked = spec.scalar(field).is_some_and(|s| s.is_typed());
            out.push(SuggestionDto {
                field: field.pointer(),
                stat: (*stat).to_owned(),
                value: est.value,
                source: est.value.and(est.source).map(source_to_dto),
                band: est
                    .value
                    .and(est.band.as_ref())
                    .map(|b| -> SuggestionBandDto { band_to_dto(b, est) }),
                level: chain_level_to_dto(est.level),
                predictor: predictor_to_dto(est.predictor),
                n: count(est.n),
                locked,
            });
        }
    }
    out
}

/// Writes an estimate into a spec through the offer rule: typed values are kept, every other value is
/// replaced only by a source of at least its rank. Returns how many fields changed.
pub fn apply_estimate(spec: &mut DesignSpec, estimate: &Estimate) -> usize {
    let mut changed = 0;
    for (stat, est) in &estimate.stats {
        let (Some(value), Some(source)) = (est.value, est.source) else {
            continue;
        };
        let value = if stat == "burst" {
            value.round().max(1.0)
        } else {
            value
        };
        let value_source = source_to_value_source(source);
        for field in fields_of_stat(spec, stat) {
            // A number typed in the dialogue fills an empty or estimated field as typed, but it never
            // replaces a number typed in the form: the form value is the user's latest word (IT-003).
            if value_source == ValueSource::Typed
                && spec.scalar(field).is_some_and(|s| s.is_typed())
            {
                continue;
            }
            if spec.offer(field, value, value_source).applied() {
                changed += 1;
            }
        }
    }
    changed
}

// ---------------------------------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------------------------------

fn static_diagnostics(
    ctx: &Ctx,
    engine: Option<&Engine>,
    draft: &Draft,
    estimate: Option<&Estimate>,
) -> Vec<Diagnostic> {
    let spec = &draft.spec;
    let mut out = validate_spec(spec);
    if let Some(engine) = engine {
        out.extend(validate_refs(spec, &engine.lookup()));
    }
    if spec.ce.is_some() && !engine.is_some_and(Engine::ce_available) {
        out.push(CE_ABSENT.diagnostic("/ce", &[]));
    }
    if let Some(e) = estimate
        && e.class_n < THIN_POOL
    {
        out.push(POOL_THIN.diagnostic("", &[("count", &e.class_n.to_string())]));
    }
    if let Some(engine) = engine
        && let Ok(kind) = pool_kind(draft.kind)
        && matches!(
            calibration_state(ctx, engine, kind),
            Ok(CalibrationState::Stale)
        )
    {
        out.push(CALIBRATION_STALE.diagnostic(
            "",
            &[(
                "reason",
                "the reference weapons changed since the last calibration",
            )],
        ));
    }
    out
}

/// The preview of a draft that is already decoded. See [`preview`].
///
/// # Errors
///
/// [`crate::error::ToolkitError::Design`] when the estimation rejects an input. Missing reference data is
/// not an error: the readouts and diagnostics are still returned, without suggestions and estimate.
pub fn preview_draft(ctx: &Ctx, draft: &Draft) -> ToolkitResult<PreviewDto> {
    let spec = &draft.spec;
    let engine = ctx.engine();
    let kind = pool_kind(draft.kind)?;
    let pools = engine.as_ref().and_then(|e| e.pools().ok());
    let armor: Option<&[f64]> = pools.map(|p| p.set.armor.ratings.as_slice());
    let ce_model = match (&engine, spec.ce.as_ref()) {
        (Some(e), Some(_)) => Some(e.ce()),
        _ => None,
    };
    let env = ReadoutEnv {
        armor,
        defs: engine.as_ref().map(|e| e.databases().as_ref()),
        ce: ce_model,
    };
    let readouts = readouts(spec, &env);

    let mut estimate = None;
    let mut suggestions_out = Vec::new();
    let mut summary: Option<EstimateSummaryDto> = None;
    if let Some(e) = engine.as_ref()
        && let Some(model) = e.model(kind)?
    {
        let est = estimate_for(ctx, e, &model, draft, true)?;
        suggestions_out = suggestions(spec, &est);
        summary = Some(estimate_summary(&est));
        estimate = Some(est);
    }
    let diags = static_diagnostics(ctx, engine.as_deref(), draft, estimate.as_ref());
    Ok(PreviewDto {
        readouts,
        suggestions: suggestions_out,
        estimate: summary,
        diagnostics: diagnostics_to_dtos(&diags),
    })
}

/// Exact readouts, suggestions with bands and static diagnostics for a draft state (`designer_preview`).
///
/// Pure and fast: it reads the cached engine of the snapshot and the cached calibration bands. Without a
/// game install it still returns the readouts that need only the spec.
///
/// # Errors
///
/// [`crate::error::ToolkitError::InvalidDraft`] for an unreadable draft; see [`preview_draft`].
pub fn preview(ctx: &Ctx, req: DesignerPreviewRequest) -> ToolkitResult<PreviewDto> {
    let draft = draft_from_dto(&req.draft)?;
    preview_draft(ctx, &draft)
}

/// Fills the empty and non typed fields of a draft from its estimate: the headless way to get a complete
/// design from tier, role and a strength choice alone (IT-010). Typed values are never replaced.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ReferenceUnavailable`] without reference weapons, otherwise as
/// [`preview_draft`].
pub fn suggest_fill(ctx: &Ctx, draft: DraftDto) -> ToolkitResult<DraftDto> {
    let mut draft = draft_from_dto(&draft)?;
    let kind = pool_kind(draft.kind)?;
    let engine = ctx.require_engine()?;
    let model =
        engine
            .model(kind)?
            .ok_or_else(|| crate::error::ToolkitError::ReferenceUnavailable {
                reason: "the loaded defs hold no reference weapons of this kind".to_owned(),
            })?;
    let estimate = estimate_for(ctx, &engine, &model, &draft, true)?;
    apply_estimate(&mut draft.spec, &estimate);
    draft_to_dto(&draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::model::{CostEntry, ToolSpec};

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    fn value_of(rs: &[ReadoutDto], key: &str) -> Option<f64> {
        rs.iter().find(|r| r.key == key).and_then(|r| r.value)
    }

    fn rifle() -> DesignSpec {
        let mut s = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        for (f, v) in [
            (ScalarField::Damage, 12.0),
            (ScalarField::Warmup, 0.8),
            (ScalarField::Cooldown, 1.5),
            (ScalarField::BurstCount, 3.0),
            (ScalarField::TicksBetweenBurstShots, 10.0),
            (ScalarField::Range, 28.0),
        ] {
            s.offer(f, v, ValueSource::Typed);
        }
        s
    }

    #[test]
    fn ranged_cycle_time_and_dps_match_the_hand_computed_vector() {
        // warmup 0.8 + cooldown 1.5 + 2 gaps of 10 ticks = 2.6333 s; 3 shots of 12 = 36 damage.
        let r = readouts(&rifle(), &ReadoutEnv::default());
        assert!(close(value_of(&r, "cycle-time"), 0.8 + 1.5 + 20.0 / 60.0));
        assert!(close(value_of(&r, "dps"), 36.0 / (0.8 + 1.5 + 20.0 / 60.0)));
        assert!(close(value_of(&r, "implied-ap"), 12.0 * 0.015));
        assert_eq!(r[0].steps.len(), 4);
    }

    #[test]
    fn hit_adjusted_dps_needs_the_accuracy_profile() {
        let mut s = rifle();
        let r = readouts(&s, &ReadoutEnv::default());
        assert_eq!(value_of(&r, "hit-dps-12"), None);
        for (f, v) in [
            (ScalarField::AccuracyTouch, 0.9),
            (ScalarField::AccuracyShort, 0.8),
            (ScalarField::AccuracyMedium, 0.6),
            (ScalarField::AccuracyLong, 0.4),
        ] {
            s.offer(f, v, ValueSource::Typed);
        }
        let r = readouts(&s, &ReadoutEnv::default());
        let dps = value_of(&r, "dps").unwrap_or(0.0);
        let hit = value_of(&r, "hit-dps-12").unwrap_or(0.0);
        assert!(hit > 0.0 && hit < dps, "hit {hit} dps {dps}");
    }

    #[test]
    fn a_missing_input_gives_a_readout_without_a_value() {
        let s = DesignSpec::new_ranged("RS_Empty", "empty");
        let r = readouts(&s, &ReadoutEnv::default());
        assert!(r.iter().all(|x| x.value.is_none()));
        assert!(r.iter().any(|x| x.key == "cycle-time"));
    }

    #[test]
    fn strength_index_needs_the_reference_armor() {
        let mut s = rifle();
        for (f, v) in [
            (ScalarField::AccuracyTouch, 0.9),
            (ScalarField::AccuracyShort, 0.8),
            (ScalarField::AccuracyMedium, 0.6),
            (ScalarField::AccuracyLong, 0.4),
        ] {
            s.offer(f, v, ValueSource::Typed);
        }
        let none = readouts(&s, &ReadoutEnv::default());
        assert_eq!(value_of(&none, "strength-index"), None);
        let armor = [0.0, 0.5, 1.0];
        let with = readouts(
            &s,
            &ReadoutEnv {
                armor: Some(&armor),
                ..ReadoutEnv::default()
            },
        );
        assert!(value_of(&with, "strength-index").is_some_and(|v| v > 0.0));
    }

    #[test]
    fn melee_readouts_follow_the_stat_panel() {
        let mut s = DesignSpec::new_melee("RS_Blade", "blade");
        s.tools
            .push(ToolSpec::new("blade", &["Cut"]).with_numbers(10.0, 2.0, ValueSource::Typed));
        s.tools
            .push(ToolSpec::new("point", &["Stab"]).with_numbers(20.0, 2.0, ValueSource::Typed));
        let r = readouts(&s, &ReadoutEnv::default());
        // weights are damage squared: (100 * 10 + 400 * 20) / 500 = 18 over a cooldown of 2 gives 9
        assert!(close(value_of(&r, "melee-swing-damage"), 18.0));
        assert!(close(value_of(&r, "melee-swing-cooldown"), 2.0));
        assert!(close(value_of(&r, "melee-dps"), 9.0));
        assert!(value_of(&r, "melee-fight-dps").is_some());
        assert!(r.iter().all(|x| !x.key.starts_with("ce-")));
    }

    #[test]
    fn an_explicit_market_value_is_the_price_readout() {
        let mut s = rifle();
        s.offer(ScalarField::MarketValue, 420.0, ValueSource::Typed);
        s.cost_list.push(CostEntry::new("RS_Steel", 30.0));
        let r = readouts(&s, &ReadoutEnv::default());
        assert!(close(value_of(&r, "market-value"), 420.0));
    }

    #[test]
    fn stat_fields_follow_the_kind() {
        let melee = DesignSpec::new_melee("RS_B", "b");
        assert_eq!(
            fields_of_stat(&melee, "swing_damage"),
            vec![ScalarField::ToolPower(0)]
        );
        assert!(fields_of_stat(&melee, "damage").is_empty());
        assert_eq!(
            fields_of_stat(&rifle(), "damage"),
            vec![ScalarField::Damage]
        );
        assert_eq!(fields_of_stat(&rifle(), "mass"), vec![ScalarField::Mass]);
    }

    #[test]
    fn spec_stats_include_computed_values() {
        let stats = spec_stats(&rifle());
        assert!(stats.contains_key("dps"));
        assert!(close(stats.get("ap").copied(), 0.18));
        assert!(close(stats.get("burst").copied(), 3.0));
    }
}
