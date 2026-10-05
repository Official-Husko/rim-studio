//! Weapon archetypes: `designer_archetype_catalog`, `designer_archetype_propose` and
//! `designer_archetype_apply`.
//!
//! The taxonomy is RimStudio's own JSON ([`rimstudio_design::archetype`]); the numbers come from the user's
//! own install at run time: the reference pools by tier and role, the cost lists of the reference weapons and
//! (in Combat Extended mode only) the ammo sets. Nothing is stored. The designer always writes vanilla
//! definitions: Combat Extended mode proposes the calibre as an ammo set and, only when the request asks for
//! it, fills the optional Combat Extended block of the draft with the existing predictors.
//!
//! A proposal never overwrites a number the user decided (IT-003): applying goes through the offer rule of the
//! spec. The choice is recorded in the draft so the numbers can be proposed again when the target changes.

use rimstudio_design::archetype::{
    ApplyOptions, Basis, CeInfo, Context, CostBasis, Proposal, ProposeRequest, Taxonomy, apply,
    ce_info, propose_with, verdict,
};
use rimstudio_design::baseline::{StrengthChoice, StrengthInput};
use rimstudio_design::ce::suggest::{Accept, accept_suggestions, suggest_block};
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_design::model::{
    ArchetypeChoice, ArchetypeMode, CePatchSpec, DesignSpec, ItemKind, TechLevel,
};
use rimstudio_ipc_types::designer::{
    ArchetypeCatalogDto, ArchetypeCeCatalogDto, ArchetypeChoiceDto, ArchetypeChoiceOptionDto,
    ArchetypeDto, ArchetypeFamilyDto, ArchetypeProposalDto, ArchetypeTierDto, BalanceOptionDto,
    BalanceTargetDto, CeCalibreDto, CeProposalDto, DesignerArchetypeApplyRequest,
    DesignerArchetypeApplyResponse, DesignerArchetypeCatalogRequest,
    DesignerArchetypeProposeRequest, DesignerProjectileOwnRequest,
    DesignerStructureDefaultsRequest, FactorTermDto, ProposedCostDto, ProposedStuffDto,
    ProposedToolDto, ProposedValueDto, ResolvedChoiceDto, StrengthReportDto,
};

use super::calibrate::{CalibrationState, calibration_state};
use super::ctx::{Ctx, Engine};
use super::dto::{
    draft_from_dto, draft_to_dto, fit_report_to_dto, kind_from_dto, kind_to_dto, tier_to_dto,
};
use super::quiz::STRENGTH_KEY;
use crate::error::{ToolkitError, ToolkitResult};

/// What the archetype solver reads besides the pools: the cost lists of the reference weapons and the
/// Combat Extended calibres. Built once per def snapshot.
#[derive(Debug)]
pub struct ArchetypeData {
    /// The cost lists and unit prices of the reference weapons.
    pub costs: CostBasis,
    /// The Combat Extended ammo sets as calibres (empty without Combat Extended).
    pub ce: CeInfo,
}

impl Engine {
    /// The cost lists and calibres of the snapshot, read on first use.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ReferenceUnavailable`] when the loaded defs have no weapon database at all.
    pub fn archetype_data(&self) -> ToolkitResult<&ArchetypeData> {
        if let Some(data) = self.archetype.get() {
            return Ok(data);
        }
        let pools = self.pools()?;
        let data = ArchetypeData {
            costs: CostBasis::read(self.databases(), self.reader_options(), &pools.set),
            ce: ce_info(self.ce()),
        };
        Ok(self.archetype.get_or_init(|| data))
    }
}

fn through_json<A, B>(a: &A, what: &str) -> ToolkitResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(a).map_err(|e| ToolkitError::Internal {
        what: format!("{what} cannot be encoded: {e}"),
    })?;
    serde_json::from_value(value).map_err(|e| ToolkitError::Internal {
        what: format!("{what} has the wrong shape: {e}"),
    })
}

fn taxonomy() -> ToolkitResult<&'static Taxonomy> {
    Taxonomy::builtin().map_err(ToolkitError::from)
}

fn pool_kind(kind: ItemKind) -> PoolKind {
    match kind {
        ItemKind::Melee => PoolKind::Melee,
        _ => PoolKind::Ranged,
    }
}

// ---------------------------------------------------------------------------------------------------
// The catalogue
// ---------------------------------------------------------------------------------------------------

fn option(
    id: &str,
    label: &str,
    summary: Option<&str>,
    rate: Option<f64>,
) -> ArchetypeChoiceOptionDto {
    ArchetypeChoiceOptionDto {
        id: id.to_owned(),
        label: label.to_owned(),
        summary: summary.map(str::to_owned),
        rate,
    }
}

fn archetype_dto(
    tax: &Taxonomy,
    engine: Option<&Engine>,
    family: &rimstudio_design::archetype::Family,
    a: &rimstudio_design::archetype::Archetype,
    kind: ItemKind,
) -> ToolkitResult<ArchetypeDto> {
    let mut applies: Vec<String> = Vec::new();
    if kind == ItemKind::Ranged {
        applies
            .extend(["action", "rof", "calibre", "handling", "tier", "balance"].map(str::to_owned));
    } else {
        applies.extend(["rof", "handling", "tier", "balance"].map(str::to_owned));
    }
    let tier = rimstudio_design::archetype::resolve::tech_level_of(&a.tier_hint)
        .unwrap_or(TechLevel::Industrial);
    let pool_role = engine
        .and_then(|e| e.pool(pool_kind(kind)).ok())
        .and_then(|pool| rimstudio_design::archetype::basis::match_role(pool, &a.role_hints));
    let _ = tax;
    Ok(ArchetypeDto {
        id: format!("{}/{}", family.id, a.id),
        label: a.label.clone(),
        summary: a.summary.clone(),
        kind: kind_to_dto(kind)?,
        applies,
        actions: a.actions.clone(),
        default_action: a.default_action.clone(),
        rof_classes: a.rof_classes.clone(),
        default_rof: a.default_rof.clone(),
        ref_rpm: a.ref_rpm,
        calibres: a.calibres.clone(),
        default_calibre: a.default_calibre.clone(),
        handlings: a.handlings.clone(),
        default_handling: a.default_handling.clone(),
        default_tier: tier_to_dto(tier),
        pool_role,
        ammo_families: a.ammo_families.clone(),
    })
}

fn ce_calibre_dto(c: &rimstudio_design::archetype::CeCalibre) -> CeCalibreDto {
    CeCalibreDto {
        set: c.set.clone(),
        label: c.label.clone(),
        caliber: c.caliber.clone(),
        family: c.family.clone(),
        projectile: c.projectile.clone(),
        damage: c.damage,
        ap_sharp: c.ap_sharp,
        ratio: c.ratio,
        weapon_count: crate::designer::dto::count(c.weapon_count),
    }
}

/// The taxonomy and the choices of every descriptor (`designer_archetype_catalog`).
///
/// Works without a game install (the taxonomy is the application's own data); without one no archetype has a
/// pool role and `proposalsAvailable` is false.
///
/// # Errors
///
/// [`ToolkitError::Design`] when the embedded taxonomy is broken (the tests keep that from happening).
pub fn archetype_catalog(
    ctx: &Ctx,
    req: DesignerArchetypeCatalogRequest,
) -> ToolkitResult<ArchetypeCatalogDto> {
    let tax = taxonomy()?;
    let engine = ctx.engine();
    let want = req.kind.map(kind_from_dto);
    let mut families = Vec::new();
    for (kind, list) in [
        (ItemKind::Ranged, &tax.ranged),
        (ItemKind::Melee, &tax.melee),
    ] {
        if want.is_some_and(|w| w != kind) {
            continue;
        }
        for family in list {
            let archetypes = family
                .archetypes
                .iter()
                .map(|a| archetype_dto(tax, engine.as_deref(), family, a, kind))
                .collect::<ToolkitResult<Vec<_>>>()?;
            families.push(ArchetypeFamilyDto {
                id: family.id.clone(),
                label: family.label.clone(),
                kind: kind_to_dto(kind)?,
                archetypes,
            });
        }
    }
    let l = &tax.ladders;
    let pools = engine.as_ref().and_then(|e| e.pools().ok());
    let count_at = |pool: Option<&rimstudio_design::classes::Pool>, tier: u8| -> u32 {
        pool.map_or(0, |p| {
            crate::designer::dto::count(p.items.iter().filter(|i| i.tier == tier).count())
        })
    };
    let tiers = TechLevel::ALL
        .into_iter()
        .map(|t| ArchetypeTierDto {
            tier: tier_to_dto(t),
            label: t.xml_name().to_owned(),
            ranged_count: count_at(pools.map(|p| &p.ranged), t.index()),
            melee_count: count_at(pools.map(|p| &p.melee), t.index()),
        })
        .collect();
    let ce = match engine.as_ref() {
        Some(e) if e.ce_available() => {
            let data = e.archetype_data()?;
            ArchetypeCeCatalogDto {
                available: true,
                reason: None,
                calibres: if req.include_calibres {
                    data.ce.calibres.iter().map(ce_calibre_dto).collect()
                } else {
                    Vec::new()
                },
                ai_class_tags: data.ce.ai_class_tags.clone(),
            }
        }
        Some(_) => ArchetypeCeCatalogDto {
            available: false,
            reason: Some("no Combat Extended data is loaded".to_owned()),
            calibres: Vec::new(),
            ai_class_tags: Vec::new(),
        },
        None => ArchetypeCeCatalogDto {
            available: false,
            reason: Some("no game install is loaded".to_owned()),
            calibres: Vec::new(),
            ai_class_tags: Vec::new(),
        },
    };
    Ok(ArchetypeCatalogDto {
        families,
        rof: l
            .rof
            .iter()
            .map(|r| option(&r.id, &r.label, None, Some(r.rate)))
            .collect(),
        actions: l
            .actions
            .iter()
            .map(|a| option(&a.id, &a.label, Some(&a.summary), None))
            .collect(),
        calibres: l
            .calibres
            .iter()
            .map(|c| option(&c.id, &c.label, Some(&c.examples), None))
            .collect(),
        handlings: l
            .handlings
            .iter()
            .map(|h| option(&h.id, &h.label, None, None))
            .collect(),
        tiers,
        balance: vec![
            BalanceOptionDto {
                target: BalanceTargetDto::Weaker,
                label: "Weaker than most of its class".to_owned(),
                percentile: StrengthChoice::Weaker.percentile(),
            },
            BalanceOptionDto {
                target: BalanceTargetDto::Typical,
                label: "Typical for its class".to_owned(),
                percentile: StrengthChoice::Typical.percentile(),
            },
            BalanceOptionDto {
                target: BalanceTargetDto::Stronger,
                label: "Stronger than most of its class".to_owned(),
                percentile: StrengthChoice::Stronger.percentile(),
            },
        ],
        pool_sizes: [
            crate::designer::dto::count(pools.map_or(0, |p| p.ranged.len())),
            crate::designer::dto::count(pools.map_or(0, |p| p.melee.len())),
        ],
        proposals_available: pools.is_some_and(|p| !p.ranged.is_empty() || !p.melee.is_empty()),
        ce,
    })
}

// ---------------------------------------------------------------------------------------------------
// The proposal
// ---------------------------------------------------------------------------------------------------

fn choice_from_dto(dto: &ArchetypeChoiceDto) -> ToolkitResult<ArchetypeChoice> {
    through_json(dto, "the archetype choice")
}

fn choice_to_dto(choice: &ArchetypeChoice) -> ToolkitResult<ArchetypeChoiceDto> {
    through_json(choice, "the archetype choice")
}

fn engine_proposal(
    ctx: &Ctx,
    choice: &ArchetypeChoice,
) -> ToolkitResult<(Proposal, std::sync::Arc<Engine>)> {
    let tax = taxonomy()?;
    let arch = tax.find(&choice.archetype).ok_or_else(|| {
        ToolkitError::from(rimstudio_design::error::DesignError::invalid(
            "archetype",
            format!("`{}` is not an archetype", choice.archetype),
        ))
    })?;
    let engine = ctx.require_engine()?;
    let kind = pool_kind(arch.kind);
    let model = engine
        .model(kind)?
        .ok_or_else(|| ToolkitError::ReferenceUnavailable {
            reason: "the loaded defs hold no reference weapons of this kind".to_owned(),
        })?;
    let data = engine.archetype_data()?;
    let pools = engine.pools()?;
    let state = calibration_state(ctx, &engine, kind)?;
    let metrics = match &state {
        CalibrationState::Fresh(m) => Some(m.as_ref()),
        _ => None,
    };
    let context = Context {
        taxonomy: tax,
        basis: Basis {
            model: &model,
            weapons: &pools.set.weapons,
            armor: &pools.set.armor.ratings,
            costs: Some(&data.costs),
        },
        ce: (choice.mode == ArchetypeMode::CombatExtended).then_some(&data.ce),
    };
    let request = ProposeRequest {
        archetype: choice.archetype.clone(),
        descriptors: choice.descriptors.clone(),
        balance: choice.balance,
        mode: choice.mode,
        strength: choice.strength,
    };
    let proposal = propose_with(&context, &request, metrics)?;
    Ok((proposal, engine.clone()))
}

fn term_dto(t: &rimstudio_design::archetype::FactorTerm) -> FactorTermDto {
    FactorTermDto {
        label: t.label.clone(),
        factor: t.factor,
    }
}

fn value_dto(
    v: &rimstudio_design::archetype::ProposedValue,
    locked: bool,
    pointer: Option<String>,
) -> ProposedValueDto {
    ProposedValueDto {
        field: pointer.unwrap_or_else(|| v.field.pointer()),
        stat: v.stat.clone(),
        value: v.value,
        write: v.write,
        source: rimstudio_design::archetype::SOURCE_CHIP.to_owned(),
        reason: v.reason.clone(),
        median: v.median,
        median_n: v.median_n.map(crate::designer::dto::count),
        terms: v.terms.iter().map(term_dto).collect(),
        locked,
    }
}

fn is_locked(spec: Option<&DesignSpec>, field: rimstudio_design::model::ScalarField) -> bool {
    spec.and_then(|s| s.scalar(field))
        .is_some_and(|v| v.is_typed())
}

/// The proposal as the DTO. `spec` marks the fields the user decided.
fn proposal_to_dto(
    proposal: &Proposal,
    choice: &ArchetypeChoice,
    spec: Option<&DesignSpec>,
) -> ToolkitResult<ArchetypeProposalDto> {
    let tools = proposal
        .tools
        .iter()
        .enumerate()
        .map(|(i, t)| {
            // A tool is locked when the draft has a tool of that label whose numbers the user typed.
            let existing = spec.and_then(|s| s.tools.iter().position(|x| x.label == t.label));
            let lock = |field: rimstudio_design::model::ScalarField| {
                existing.is_some_and(|_| is_locked(spec, field))
            };
            let at = existing.unwrap_or(i);
            let power_field = rimstudio_design::model::ScalarField::ToolPower(at);
            let cool_field = rimstudio_design::model::ScalarField::ToolCooldown(at);
            ProposedToolDto {
                label: t.label.clone(),
                capacities: t.capacities.clone(),
                power: value_dto(&t.power, lock(power_field), Some(power_field.pointer())),
                cooldown: value_dto(&t.cooldown, lock(cool_field), Some(cool_field.pointer())),
                armor_penetration: t.armor_penetration.as_ref().map(|ap| {
                    let f = rimstudio_design::model::ScalarField::ToolArmorPenetration(at);
                    value_dto(ap, lock(f), Some(f.pointer()))
                }),
            }
        })
        .collect();
    let s = &proposal.strength;
    Ok(ArchetypeProposalDto {
        choice: choice_to_dto(choice)?,
        label: proposal.label.clone(),
        kind: kind_to_dto(proposal.kind)?,
        source: proposal.source.clone(),
        resolved: ResolvedChoiceDto {
            action: proposal.choice.action.clone(),
            rof: proposal.choice.rof.clone(),
            rate: proposal.choice.rate,
            calibre: proposal.choice.calibre.clone(),
            ammo_set: proposal.choice.ammo_set.clone(),
            handling: proposal.choice.handling.clone(),
            tier: tier_to_dto(proposal.choice.tier),
        },
        role: proposal.role.clone(),
        values: proposal
            .values
            .iter()
            .map(|v| value_dto(v, is_locked(spec, v.field), None))
            .collect(),
        tools,
        cost_list: proposal
            .cost_list
            .iter()
            .map(|c| ProposedCostDto {
                def_name: c.def_name.clone(),
                count: c.count,
                reason: c.reason.clone(),
            })
            .collect(),
        stuff: proposal.stuff.as_ref().map(|s| ProposedStuffDto {
            categories: s.categories.clone(),
            count: s.count,
            reason: s.reason.clone(),
        }),
        weapon_tags: proposal.weapon_tags.clone(),
        weapon_classes: proposal.weapon_classes.clone(),
        market_value: proposal.market_value,
        strength: StrengthReportDto {
            percentile: s.percentile,
            target: s.target,
            achieved: s.achieved,
            error: s.error,
            scale: s.scale,
            clamped: s.clamped,
            class_label: s.class_label.clone(),
            class_n: crate::designer::dto::count(s.class_n),
            achieved_percentile: s.achieved_percentile,
        },
        notes: proposal.notes.clone(),
        ce: proposal.ce.as_ref().map(|c| CeProposalDto {
            ammo_set: c.ammo_set.clone(),
            caliber: c.caliber.clone(),
            default_projectile: c.default_projectile.clone(),
            weapon_tag_class: c.weapon_tag_class.clone(),
            damage_ratio: c.damage_ratio,
            notes: c.notes.clone(),
        }),
        fit: proposal.fit.as_ref().map(fit_report_to_dto),
        verdict: proposal
            .fit
            .as_ref()
            .map(|f| format!("{:?}", verdict(f)).to_lowercase()),
    })
}

fn choice_of_request(req: &DesignerArchetypeProposeRequest) -> ToolkitResult<ArchetypeChoice> {
    Ok(ArchetypeChoice {
        archetype: req.archetype.clone(),
        descriptors: through_json(&req.descriptors, "the descriptors")?,
        balance: through_json(&req.balance_target, "the balance target")?,
        mode: through_json(&req.mode, "the mode")?,
        strength: req.strength,
    })
}

/// The proposal for an archetype, descriptors and balance target (`designer_archetype_propose`).
///
/// Pure and cheap: it reads the cached pools and models of the snapshot and does a handful of strength
/// index evaluations. Typed values of the optional draft are marked `locked` and never touched.
///
/// # Errors
///
/// [`ToolkitError::Design`] for an unknown archetype or a descriptor it does not offer;
/// [`ToolkitError::ReferenceUnavailable`] without reference weapons of the kind; [`ToolkitError::InvalidDraft`]
/// when the kind does not match the archetype or the draft cannot be read.
pub fn archetype_propose(
    ctx: &Ctx,
    req: DesignerArchetypeProposeRequest,
) -> ToolkitResult<ArchetypeProposalDto> {
    let choice = choice_of_request(&req)?;
    let tax = taxonomy()?;
    if let Some(arch) = tax.find(&choice.archetype)
        && arch.kind != kind_from_dto(req.kind)
    {
        return Err(ToolkitError::invalid_draft(format!(
            "`{}` is a {} archetype, not a {} one",
            choice.archetype,
            arch.kind.as_str(),
            kind_from_dto(req.kind).as_str()
        )));
    }
    let draft = req.draft.as_ref().map(draft_from_dto).transpose()?;
    let (proposal, _engine) = engine_proposal(ctx, &choice)?;
    proposal_to_dto(&proposal, &choice, draft.as_ref().map(|d| &d.spec))
}

// ---------------------------------------------------------------------------------------------------
// Applying
// ---------------------------------------------------------------------------------------------------

fn fill_ce_block(spec: &mut DesignSpec, proposal: &Proposal, engine: &Engine, refresh: bool) {
    let Some(ce) = proposal.ce.as_ref() else {
        return;
    };
    if !engine.ce_available() || ce.ammo_set.is_none() {
        return;
    }
    let block = spec.ce.get_or_insert_with(CePatchSpec::default);
    let put = |slot: &mut Option<String>, value: &Option<String>| {
        if value.is_some() && (refresh || slot.is_none()) {
            slot.clone_from(value);
        }
    };
    put(&mut block.caliber, &ce.caliber);
    put(&mut block.ammo_set, &ce.ammo_set);
    put(&mut block.default_projectile, &ce.default_projectile);
    put(&mut block.weapon_tag_class, &ce.weapon_tag_class);
    // The numbers of the block come from the existing predictors, with the vanilla numbers as the driver.
    let suggestion = suggest_block(spec, engine.ce());
    let outcome = accept_suggestions(spec, &suggestion, &Accept::All);
    *spec = outcome.spec;
}

/// Fills what is still empty of the structure from the nearest reference weapon (the parent base and, for a
/// gun, a projectile of its own) so the draft can be planned. A failure is a note, never an error: the numbers
/// of the proposal are already in the draft.
fn complete_structure(
    ctx: &Ctx,
    draft: &mut rimstudio_design::model::Draft,
    notes: &mut Vec<String>,
) {
    let Ok(dto) = draft_to_dto(draft) else {
        return;
    };
    match super::clone::structure_defaults(ctx, DesignerStructureDefaultsRequest { draft: dto }) {
        Ok(reply) => {
            if let Ok(next) = draft_from_dto(&reply.draft) {
                *draft = next;
            }
            notes.extend(reply.notes);
        }
        Err(e) => notes.push(format!("the structure could not be completed: {e}")),
    }
    let shared = draft
        .spec
        .ranged
        .as_ref()
        .and_then(|r| r.projectile.as_ref())
        .is_some_and(|p| matches!(p, rimstudio_design::model::ProjectileChoice::Reference(_)));
    if shared && let Ok(dto) = draft_to_dto(draft) {
        match super::own::projectile_own(
            ctx,
            DesignerProjectileOwnRequest {
                draft: dto,
                own: true,
            },
        ) {
            Ok(reply) => {
                if let Ok(next) = draft_from_dto(&reply.draft) {
                    *draft = next;
                }
                notes.extend(reply.notes);
            }
            Err(e) => notes.push(format!("the weapon keeps a shared projectile: {e}")),
        }
    }
}

/// Fills the empty and derived fields of a draft from the proposal and records the choice
/// (`designer_archetype_apply`).
///
/// Typed values are kept. The numbers are derived again from the choice the proposal carries. In a draft in
/// simple mode the balance target is also recorded as the strength choice the fit meter compares with. The
/// optional Combat Extended block is touched only when the request says `includeCe` and the choice is in
/// Combat Extended mode.
///
/// # Errors
///
/// As [`archetype_propose`]; [`ToolkitError::InvalidDraft`] when the draft is of the other kind.
pub fn archetype_apply(
    ctx: &Ctx,
    req: DesignerArchetypeApplyRequest,
) -> ToolkitResult<DesignerArchetypeApplyResponse> {
    let mut draft = draft_from_dto(&req.draft)?;
    let choice = choice_from_dto(&req.proposal.choice)?;
    let (proposal, engine) = engine_proposal(ctx, &choice)?;
    if proposal.kind != draft.spec.kind {
        return Err(ToolkitError::invalid_draft(format!(
            "the draft is a {} design and `{}` is a {} archetype",
            draft.spec.kind.as_str(),
            choice.archetype,
            proposal.kind.as_str()
        )));
    }
    let report = apply(
        &mut draft.spec,
        &proposal,
        ApplyOptions {
            refresh_structure: req.refresh_structure,
        },
    )?;
    if req.include_ce && choice.mode == ArchetypeMode::CombatExtended {
        fill_ce_block(&mut draft.spec, &proposal, &engine, req.refresh_structure);
    }
    // The fit meter compares the design with the class at the strength the choice asked for.
    if draft.calibration == rimstudio_design::model::CalibrationMode::Simple {
        let input = match choice.strength {
            Some(index) if index.is_finite() && index > 0.0 => StrengthInput::Placed {
                ln_power: index.ln(),
            },
            _ => StrengthInput::Percentile {
                p: choice.balance.percentile(),
            },
        };
        let value = serde_json::to_value(input).map_err(|e| ToolkitError::Internal {
            what: format!("the strength choice cannot be encoded: {e}"),
        })?;
        draft.answers.insert(STRENGTH_KEY.to_owned(), value);
    }
    draft.archetype = Some(choice.clone());
    let mut notes = Vec::new();
    if req.complete_structure {
        complete_structure(ctx, &mut draft, &mut notes);
    }
    let dto = proposal_to_dto(&proposal, &choice, Some(&draft.spec))?;
    Ok(DesignerArchetypeApplyResponse {
        draft: draft_to_dto(&draft)?,
        filled: report.filled,
        kept: report.kept,
        proposal: dto,
        notes,
    })
}
