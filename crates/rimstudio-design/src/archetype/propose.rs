//! The entry point: an archetype, descriptors and a balance target in, a complete proposal out.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::baseline::StrengthInput;
use crate::classes::ClassKey;
use crate::classes::numeric::{median, mid_rank_of};
use crate::error::{DesignError, DesignResult};
use crate::loo::CalibrationMetrics;
use crate::model::{ArchetypeMode, BalanceTarget, Descriptors, ItemKind};
use crate::ranged::nominal_dps;

use super::basis::{
    Basis, MeleeMedians, RangedMedians, match_role, melee_bias, range_fraction, strength_target,
};
use super::ce::{CeInfo, ai_class_for};
use super::data::{ArchetypeRef, Taxonomy};
use super::parts::{MaterialInputs, bash_tools, materials, price, weapon_classes, weapon_tags};
use super::proposal::{CeProposal, Proposal, ResolvedChoice, SOURCE_CHIP, StrengthReport};
use super::resolve::{Resolved, resolve};
use super::verify::fit_of;
use super::{blade, gun};

/// Everything a proposal reads.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The taxonomy.
    pub taxonomy: &'a Taxonomy,
    /// What the install says.
    pub basis: Basis<'a>,
    /// The Combat Extended calibres, when Combat Extended is loaded.
    pub ce: Option<&'a CeInfo>,
}

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeRequest {
    /// The archetype id (`rifle/assault`).
    pub archetype: String,
    /// The descriptors.
    #[serde(default)]
    pub descriptors: Descriptors,
    /// The balance target.
    #[serde(default)]
    pub balance: BalanceTarget,
    /// Vanilla or Combat Extended mode.
    #[serde(default)]
    pub mode: ArchetypeMode,
    /// A strength index to aim at exactly. It replaces the balance target when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
}

fn pool_median(basis: &Basis<'_>) -> Option<f64> {
    let strengths: Vec<f64> = basis.pool().items.iter().map(|i| i.strength).collect();
    median(&strengths)
}

fn strength_report(
    ctx: &Context<'_>,
    key: &ClassKey,
    target: &super::basis::StrengthTarget,
    achieved: f64,
    scale: f64,
    clamped: bool,
) -> StrengthReport {
    let class = ctx.basis.model.class_stats(key);
    let achieved_percentile = class
        .strength
        .as_ref()
        .map_or(0.5, |s| mid_rank_of(&s.values, achieved));
    StrengthReport {
        percentile: target.percentile,
        target: target.strength,
        achieved,
        error: if target.strength > 0.0 {
            (achieved - target.strength) / target.strength
        } else {
            0.0
        },
        scale,
        clamped,
        class_label: target.class_label.clone(),
        class_n: target.class_n,
        achieved_percentile,
    }
}

fn choice_of(resolved: &Resolved<'_>) -> ResolvedChoice {
    ResolvedChoice {
        action: resolved.action.map(|a| a.id.clone()),
        rof: resolved.rof_id.clone(),
        rate: resolved.rate,
        calibre: resolved.calibre_id.clone(),
        ammo_set: resolved.ammo.map(|a| a.set.clone()),
        handling: resolved.handling.id.clone(),
        tier: resolved.tier,
    }
}

fn ce_part(
    ctx: &Context<'_>,
    arch: &ArchetypeRef<'_>,
    resolved: &Resolved<'_>,
    mode: ArchetypeMode,
) -> Option<CeProposal> {
    if mode != ArchetypeMode::CombatExtended || arch.kind != ItemKind::Ranged {
        return None;
    }
    let Some(info) = ctx.ce.filter(|i| !i.calibres.is_empty()) else {
        return Some(CeProposal {
            ammo_set: None,
            caliber: None,
            default_projectile: None,
            weapon_tag_class: None,
            damage_ratio: None,
            notes: vec![
                "no Combat Extended ammo sets are loaded: only the vanilla numbers are proposed"
                    .to_owned(),
            ],
        });
    };
    let mut notes = Vec::new();
    let ammo = resolved.ammo;
    if ammo.is_none() {
        notes.push("no ammo set chosen yet: pick one of the calibres of your install".to_owned());
    }
    let weapon_tag_class = ai_class_for(info, &arch.archetype.ai_class_hints);
    if weapon_tag_class.is_none() {
        notes.push("no AI class tag of your install matches this archetype".to_owned());
    }
    Some(CeProposal {
        ammo_set: ammo.map(|a| a.set.clone()),
        caliber: ammo.map(|a| a.caliber.clone()),
        default_projectile: ammo.map(|a| a.projectile.clone()),
        weapon_tag_class,
        damage_ratio: ammo.map(|a| a.ratio),
        notes,
    })
}

fn ranged_stats(p: &crate::ranged::RangedProfile, mass: f64, work: f64) -> BTreeMap<String, f64> {
    let mut s = BTreeMap::new();
    s.insert("damage".to_owned(), p.damage);
    s.insert("range".to_owned(), p.range);
    s.insert("warmup".to_owned(), p.warmup);
    s.insert("cooldown".to_owned(), p.cooldown);
    s.insert("burst".to_owned(), f64::from(p.burst_count));
    s.insert("ticks_between".to_owned(), p.ticks_between_shots);
    s.insert("ap".to_owned(), p.armor_penetration);
    if let Some(a) = p.accuracy {
        s.insert("touch".to_owned(), a.touch);
        s.insert("short".to_owned(), a.short);
        s.insert("medium".to_owned(), a.medium);
        s.insert("long".to_owned(), a.long);
    }
    if let Ok(d) = nominal_dps(p) {
        s.insert("dps".to_owned(), d);
    }
    s.insert("mass".to_owned(), mass);
    s.insert("work".to_owned(), work);
    s
}

/// Makes a proposal.
///
/// The proposal is deterministic and cheap: a handful of strength index evaluations and a fit score, far
/// below a frame budget on a pool of a few hundred weapons. `metrics` are the calibration bands for the fit
/// check (the documented defaults when `None`).
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for an unknown archetype, a descriptor the archetype does not offer or an
/// invalid balance target; [`DesignError::EmptyInput`] when the install has no reference weapons of the kind
/// or lacks a stat the shape is a ratio of.
pub fn propose_with(
    ctx: &Context<'_>,
    req: &ProposeRequest,
    metrics: Option<&CalibrationMetrics>,
) -> DesignResult<Proposal> {
    let tax = ctx.taxonomy;
    let arch = tax.find(&req.archetype).ok_or_else(|| {
        DesignError::invalid(
            "archetype",
            format!("`{}` is not an archetype", req.archetype),
        )
    })?;
    if !req.balance.is_valid() {
        return Err(DesignError::invalid(
            "balance",
            "the percentile must be a number from 0 to 1",
        ));
    }
    let basis = &ctx.basis;
    let resolved = resolve(tax, &arch, &req.descriptors, req.mode, ctx.ce)?;
    let pool = basis.pool();
    let want_kind = super::basis::pool_kind(arch.kind);
    if pool.kind != want_kind {
        return Err(DesignError::invalid(
            "archetype",
            "the reference pool is of the other kind of weapon",
        ));
    }
    let role = match_role(pool, &arch.archetype.role_hints);
    let key = ClassKey {
        role: role.clone(),
        tier: Some(resolved.tier.index()),
        group: None,
    };
    let mut target = strength_target(
        basis.model,
        &key,
        req.balance.percentile(),
        arch.archetype.strength_shift,
    )?;
    let mut input = StrengthInput::Percentile {
        p: req.balance.percentile(),
    };
    if let Some(index) = req.strength {
        if !(index.is_finite() && index > 0.0) {
            return Err(DesignError::invalid(
                "strength",
                "the strength index must be a positive number",
            ));
        }
        target.strength = index;
        target.notes = vec!["aimed at the strength index you gave".to_owned()];
        input = StrengthInput::Placed {
            ln_power: index.ln(),
        };
    }
    let pool_strength = pool_median(basis)
        .filter(|m| *m > 0.0)
        .unwrap_or(target.strength);
    let strength_ratio = target.strength / pool_strength;
    let mut notes = resolved.notes.clone();
    notes.extend(target.notes.clone());

    let mut proposal = match arch.kind {
        ItemKind::Ranged => ranged_proposal(ctx, &arch, &resolved, &key, &target, strength_ratio)?,
        ItemKind::Melee => melee_proposal(ctx, &arch, &resolved, &key, &target, strength_ratio)?,
    };
    proposal.mode = req.mode;
    proposal.role = role;
    proposal.choice = choice_of(&resolved);
    proposal.notes.extend(notes);
    proposal.ce = ce_part(ctx, &arch, &resolved, req.mode);
    if let Some(ce) = &proposal.ce {
        proposal.notes.extend(ce.notes.clone());
    }
    proposal.fit = fit_of(basis.model, &key, input, &proposal.stats, metrics).ok();
    Ok(proposal)
}

/// [`propose_with`] with the default bands.
///
/// # Errors
///
/// See [`propose_with`].
pub fn propose(ctx: &Context<'_>, req: &ProposeRequest) -> DesignResult<Proposal> {
    propose_with(ctx, req, None)
}

fn skeleton(arch: &ArchetypeRef<'_>) -> Proposal {
    Proposal {
        archetype: arch.id(),
        label: arch.archetype.label.clone(),
        kind: arch.kind,
        mode: ArchetypeMode::Vanilla,
        source: SOURCE_CHIP.to_owned(),
        choice: ResolvedChoice {
            action: None,
            rof: String::new(),
            rate: 1.0,
            calibre: None,
            ammo_set: None,
            handling: String::new(),
            tier: crate::model::TechLevel::Industrial,
        },
        role: None,
        values: Vec::new(),
        stats: BTreeMap::new(),
        tools: Vec::new(),
        cost_list: Vec::new(),
        stuff: None,
        weapon_tags: Vec::new(),
        weapon_classes: Vec::new(),
        market_value: None,
        forced_miss_radius: None,
        strength: StrengthReport {
            percentile: 0.5,
            target: 0.0,
            achieved: 0.0,
            error: 0.0,
            scale: 1.0,
            clamped: false,
            class_label: String::new(),
            class_n: 0,
            achieved_percentile: 0.5,
        },
        notes: Vec::new(),
        ce: None,
        fit: None,
    }
}

fn ranged_proposal(
    ctx: &Context<'_>,
    arch: &ArchetypeRef<'_>,
    resolved: &Resolved<'_>,
    key: &ClassKey,
    target: &super::basis::StrengthTarget,
    strength_ratio: f64,
) -> DesignResult<Proposal> {
    let basis = &ctx.basis;
    let tax = ctx.taxonomy;
    let exps = &tax.ladders.exponents;
    let medians = RangedMedians::read(basis.pool())?;
    let (shape, terms) = gun::shape(arch, resolved, &medians, tax)?;
    let ratios = arch
        .archetype
        .ranged
        .as_ref()
        .ok_or_else(|| DesignError::invalid("archetype", "not a ranged archetype"))?;
    let numbers = gun::solve(
        &shape,
        exps,
        basis,
        target.strength,
        &medians,
        range_fraction(basis.pool()),
        strength_ratio,
    )?;
    let mut p = skeleton(arch);
    p.values = gun::values(&numbers, ratios, &terms, &medians, exps);
    p.stats = ranged_stats(&numbers.profile, numbers.mass, numbers.work);
    p.tools = bash_tools(arch, basis);
    let tier = resolved.tier.index();
    p.weapon_tags = weapon_tags(basis, arch.kind, tier, key.role.as_deref());
    p.weapon_classes = weapon_classes(basis, &arch.archetype.weapon_class_hints);
    if let Some(costs) = basis.costs {
        let m = materials(
            costs,
            &MaterialInputs {
                kind: arch.kind,
                tier,
                target: target.strength,
                size: numbers.mass / medians.mass.value,
                stuff_factor: 1.0,
                stuff_categories: &[],
            },
            exps,
        );
        p.market_value = price(costs, &m, numbers.work);
        if let Some(v) = p.market_value {
            p.stats.insert("market_value".to_owned(), v);
        }
        p.cost_list = m.cost_list;
        p.notes.extend(m.notes);
    } else {
        p.notes.push(
            "no cost lists were read from the install: the cost list is left empty".to_owned(),
        );
    }
    p.strength = strength_report(
        ctx,
        key,
        target,
        numbers.achieved,
        numbers.scale,
        numbers.clamped,
    );
    if numbers.clamped {
        p.notes.push(
            "the strength target is outside what the shape can reach: the number is as close as the scale allows".to_owned(),
        );
    }
    Ok(p)
}

fn melee_proposal(
    ctx: &Context<'_>,
    arch: &ArchetypeRef<'_>,
    resolved: &Resolved<'_>,
    key: &ClassKey,
    target: &super::basis::StrengthTarget,
    strength_ratio: f64,
) -> DesignResult<Proposal> {
    let basis = &ctx.basis;
    let tax = ctx.taxonomy;
    let exps = &tax.ladders.exponents;
    let medians = MeleeMedians::read(basis.pool())?;
    let (shape, melee_shape) = blade::shape(arch, resolved, &medians, exps)?;
    let bias = melee_bias(basis.weapons);
    let numbers = blade::solve(&shape, exps, target.strength / bias, strength_ratio)?;
    let (tools, scalars) = blade::values(
        &numbers,
        &melee_shape,
        resolved,
        &arch.archetype.label,
        &medians,
        exps,
    );
    let mut p = skeleton(arch);
    p.values = scalars;
    p.tools = tools;
    if let Ok([damage, cooldown, dps, ap, fight]) = blade::panel(&numbers.tools) {
        p.stats.insert("swing_damage".to_owned(), damage);
        p.stats.insert("cooldown".to_owned(), cooldown);
        p.stats.insert("dps".to_owned(), dps);
        p.stats.insert("ap".to_owned(), ap);
        p.stats.insert("fight_dps".to_owned(), fight);
    }
    p.stats
        .insert("tools".to_owned(), numbers.tools.len() as f64);
    p.stats.insert("mass".to_owned(), numbers.mass);
    p.stats.insert("work".to_owned(), numbers.work);
    let tier = resolved.tier.index();
    p.weapon_tags = weapon_tags(basis, arch.kind, tier, key.role.as_deref());
    p.weapon_classes = weapon_classes(basis, &arch.archetype.weapon_class_hints);
    if let Some(costs) = basis.costs {
        let m = materials(
            costs,
            &MaterialInputs {
                kind: arch.kind,
                tier,
                target: target.strength,
                size: numbers.mass / medians.mass.value,
                stuff_factor: melee_shape.stuff_count,
                stuff_categories: &arch.archetype.stuff_categories,
            },
            exps,
        );
        p.market_value = price(costs, &m, numbers.work);
        if let Some(v) = p.market_value {
            p.stats.insert("market_value".to_owned(), v);
        }
        p.cost_list = m.cost_list;
        p.stuff = m.stuff;
        p.notes.extend(m.notes);
    }
    p.strength = strength_report(
        ctx,
        key,
        target,
        numbers.achieved_plain * bias,
        numbers.scale,
        numbers.clamped,
    );
    if numbers.clamped {
        p.notes.push(
            "the strength target is outside what the shape can reach: the number is as close as the scale allows".to_owned(),
        );
    }
    Ok(p)
}
