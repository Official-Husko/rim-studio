//! Weapon archetypes through the toolkit: the catalogue, the proposal and its application to a draft, on a
//! fictional install. Owner rules: vanilla by default (Combat Extended only on request) and typed values are
//! never overwritten.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::model::{ScalarField, ValueSource};
use rimstudio_ipc_types::designer::{
    ArchetypeChoiceDto, ArchetypeModeDto, ArchetypeProposalDto, BalanceTargetDto, DescriptorsDto,
    DesignerArchetypeApplyRequest, DesignerArchetypeCatalogRequest,
    DesignerArchetypeProposeRequest, ItemKindDto, RateOfFireDto,
};
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{archetype_apply, archetype_catalog, archetype_propose};

fn request(kind: ItemKindDto, archetype: &str) -> DesignerArchetypeProposeRequest {
    DesignerArchetypeProposeRequest {
        kind,
        archetype: archetype.to_owned(),
        descriptors: DescriptorsDto::default(),
        balance_target: BalanceTargetDto::Typical,
        mode: ArchetypeModeDto::Vanilla,
        strength: None,
        draft: None,
    }
}

fn value(p: &ArchetypeProposalDto, field: &str) -> f64 {
    p.values.iter().find(|v| v.field == field).unwrap().value
}

#[test]
fn the_catalogue_works_without_a_game_install_and_proposals_are_unavailable() {
    let (_tmp, ctx, _clock) = common::offline();
    let c = archetype_catalog(&ctx, DesignerArchetypeCatalogRequest::default()).unwrap();
    assert!(!c.proposals_available);
    assert!(c.families.iter().any(|f| f.id == "rifle"));
    assert!(c.families.iter().any(|f| f.id == "sword"));
    let assault = c
        .families
        .iter()
        .flat_map(|f| &f.archetypes)
        .find(|a| a.id == "rifle/assault")
        .unwrap();
    assert!(assault.applies.contains(&"calibre".to_owned()));
    assert!(assault.actions.contains(&"burst".to_owned()));
    let knife = c
        .families
        .iter()
        .flat_map(|f| &f.archetypes)
        .find(|a| a.id == "knife/knife")
        .unwrap();
    assert!(!knife.applies.contains(&"action".to_owned()));
    assert!(!c.ce.available);
    let err = archetype_propose(&ctx, request(ItemKindDto::Ranged, "rifle/assault")).unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn the_catalogue_of_an_install_counts_the_tiers_and_lists_one_kind() {
    let f = common::fixture(false);
    let c = archetype_catalog(
        &f.ctx,
        DesignerArchetypeCatalogRequest {
            kind: Some(ItemKindDto::Melee),
            include_calibres: false,
        },
    )
    .unwrap();
    assert!(c.proposals_available);
    assert!(c.pool_sizes[0] > 0 && c.pool_sizes[1] > 0);
    assert!(c.families.iter().all(|f| f.kind == ItemKindDto::Melee));
    assert!(c.tiers.iter().map(|t| t.ranged_count).sum::<u32>() > 0);
    assert_eq!(c.tiers.len(), 6);
    assert_eq!(c.balance.len(), 3);
    assert!(!c.ce.available);
}

#[test]
fn a_gun_proposal_has_every_number_a_reason_and_a_verdict() {
    let f = common::fixture(false);
    let p = archetype_propose(&f.ctx, request(ItemKindDto::Ranged, "rifle/assault")).unwrap();
    assert_eq!(p.source, "archetype");
    for field in [
        "/ranged/damage",
        "/ranged/range",
        "/ranged/warmup",
        "/ranged/cooldown",
        "/ranged/accuracy/long",
        "/mass",
        "/workToMake",
    ] {
        assert!(value(&p, field) > 0.0, "{field}");
    }
    assert!(
        p.values
            .iter()
            .filter(|v| v.write)
            .all(|v| !v.reason.is_empty() && v.source == "archetype")
    );
    assert!(p.fit.is_some() && p.verdict.is_some());
    assert!(p.strength.error.abs() < 0.1, "{}", p.strength.error);
    assert!(p.ce.is_none());
    assert!(!p.cost_list.is_empty());
    // the same request twice gives the same proposal
    let again = archetype_propose(&f.ctx, request(ItemKindDto::Ranged, "rifle/assault")).unwrap();
    assert_eq!(p, again);
}

#[test]
fn a_melee_proposal_has_tools_and_stuff() {
    let f = common::fixture(false);
    let p = archetype_propose(&f.ctx, request(ItemKindDto::Melee, "sword/long")).unwrap();
    assert_eq!(p.tools.len(), 3);
    assert!(
        p.tools
            .iter()
            .all(|t| t.power.value > 0.0 && t.cooldown.value > 0.0)
    );
    assert!(p.strength.error.abs() < 0.1);
}

#[test]
fn a_wrong_kind_an_unknown_archetype_and_a_foreign_descriptor_are_refused() {
    let f = common::fixture(false);
    let wrong =
        archetype_propose(&f.ctx, request(ItemKindDto::Melee, "rifle/assault")).unwrap_err();
    assert_eq!(wrong.code(), "designer.invalid-draft");
    let unknown =
        archetype_propose(&f.ctx, request(ItemKindDto::Ranged, "rifle/laser")).unwrap_err();
    assert!(unknown.to_string().contains("rifle/laser"), "{unknown}");
    let mut req = request(ItemKindDto::Ranged, "rifle/sniper");
    req.descriptors.action = Some("full-auto".into());
    assert!(archetype_propose(&f.ctx, req).is_err());
    let mut req = request(ItemKindDto::Ranged, "rifle/assault");
    req.balance_target = BalanceTargetDto::Percentile(2.0);
    assert!(archetype_propose(&f.ctx, req).is_err());
}

#[test]
fn applying_keeps_typed_values_records_the_choice_and_the_strength_answer() {
    let f = common::fixture(false);
    let mut spec = common::ranged_spec();
    spec.offer(ScalarField::Damage, 14.0, ValueSource::Typed);
    let mut req = request(ItemKindDto::Ranged, "rifle/assault");
    req.descriptors.rof = Some(RateOfFireDto::Class("fast".into()));
    req.balance_target = BalanceTargetDto::Stronger;
    let proposal = archetype_propose(&f.ctx, req.clone()).unwrap();
    let reply = archetype_apply(
        &f.ctx,
        DesignerArchetypeApplyRequest {
            draft: common::dto(spec),
            proposal: proposal.clone(),
            include_ce: false,
            refresh_structure: false,
            complete_structure: true,
        },
    )
    .unwrap();
    let draft = draft_from_dto(&reply.draft).unwrap();
    assert_eq!(draft.spec.scalar(ScalarField::Damage).unwrap().value, 14.0);
    assert!(draft.spec.scalar(ScalarField::Damage).unwrap().is_typed());
    assert!(reply.kept.iter().any(|k| k == "/ranged/damage"));
    let range = draft.spec.scalar(ScalarField::Range).unwrap();
    assert_eq!(range.source, ValueSource::Suggested);
    assert_eq!(range.value, value(&proposal, "/ranged/range"));
    let choice: &ArchetypeChoiceDto = reply.draft.archetype.as_ref().unwrap();
    assert_eq!(choice.archetype, "rifle/assault");
    assert_eq!(choice.balance, BalanceTargetDto::Stronger);
    assert!(draft.answers.contains_key("strength"));
    assert!(
        draft.spec.ce.is_none(),
        "vanilla by default: Combat Extended stays off"
    );
    // an older draft without the choice loads and round trips
    let mut plain = draft.clone();
    plain.archetype = None;
    let back = draft_from_dto(&draft_to_dto(&plain).unwrap()).unwrap();
    assert!(back.archetype.is_none());
    // proposing again with a weaker target replaces the suggestions and keeps the typed damage
    let mut weaker = req;
    weaker.balance_target = BalanceTargetDto::Weaker;
    weaker.draft = Some(reply.draft.clone());
    let p2 = archetype_propose(&f.ctx, weaker).unwrap();
    assert!(
        p2.values
            .iter()
            .any(|v| v.field == "/ranged/damage" && v.locked)
    );
    let reply2 = archetype_apply(
        &f.ctx,
        DesignerArchetypeApplyRequest {
            draft: reply.draft,
            proposal: p2.clone(),
            include_ce: false,
            refresh_structure: false,
            complete_structure: false,
        },
    )
    .unwrap();
    let d2 = draft_from_dto(&reply2.draft).unwrap();
    assert_eq!(d2.spec.scalar(ScalarField::Damage).unwrap().value, 14.0);
    assert_eq!(
        d2.spec.scalar(ScalarField::Range).unwrap().value,
        value(&p2, "/ranged/range")
    );
}

#[test]
fn a_draft_of_the_other_kind_is_refused() {
    let f = common::fixture(false);
    let proposal = archetype_propose(&f.ctx, request(ItemKindDto::Melee, "knife/knife")).unwrap();
    let err = archetype_apply(
        &f.ctx,
        DesignerArchetypeApplyRequest {
            draft: common::dto(common::ranged_spec()),
            proposal,
            include_ce: false,
            refresh_structure: false,
            complete_structure: false,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
}

#[test]
fn combat_extended_mode_lists_the_ammo_sets_and_fills_the_block_only_on_request() {
    let f = common::fixture(true);
    let c = archetype_catalog(
        &f.ctx,
        DesignerArchetypeCatalogRequest {
            kind: Some(ItemKindDto::Ranged),
            include_calibres: true,
        },
    )
    .unwrap();
    assert!(c.ce.available);
    let set =
        c.ce.calibres
            .first()
            .expect("the fictional Combat Extended has an ammo set");
    let mut req = request(ItemKindDto::Ranged, "rifle/assault");
    req.mode = ArchetypeModeDto::CombatExtended;
    req.descriptors.ammo_set = Some(set.set.clone());
    let proposal = archetype_propose(&f.ctx, req).unwrap();
    let ce = proposal.ce.as_ref().unwrap();
    assert_eq!(ce.ammo_set.as_deref(), Some(set.set.as_str()));
    assert_eq!(
        proposal.resolved.ammo_set.as_deref(),
        Some(set.set.as_str())
    );
    let spec = common::ranged_spec();
    let without = archetype_apply(
        &f.ctx,
        DesignerArchetypeApplyRequest {
            draft: common::dto(spec.clone()),
            proposal: proposal.clone(),
            include_ce: false,
            refresh_structure: false,
            complete_structure: false,
        },
    )
    .unwrap();
    assert!(draft_from_dto(&without.draft).unwrap().spec.ce.is_none());
    let with = archetype_apply(
        &f.ctx,
        DesignerArchetypeApplyRequest {
            draft: common::dto(spec),
            proposal,
            include_ce: true,
            refresh_structure: false,
            complete_structure: false,
        },
    )
    .unwrap();
    let block = draft_from_dto(&with.draft).unwrap().spec.ce.unwrap();
    assert_eq!(block.ammo_set.as_deref(), Some(set.set.as_str()));
    // an ammo set the install does not have is refused with its name
    let mut bad = request(ItemKindDto::Ranged, "rifle/assault");
    bad.mode = ArchetypeModeDto::CombatExtended;
    bad.descriptors.ammo_set = Some("RS_NoSuchSet".into());
    assert!(
        archetype_propose(&f.ctx, bad)
            .unwrap_err()
            .to_string()
            .contains("RS_NoSuchSet")
    );
}
