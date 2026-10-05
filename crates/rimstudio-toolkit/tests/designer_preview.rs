//! The preview through the toolkit: exact readouts, suggestions with sources and bands, locked typed
//! values (IT-003), Combat Extended gating (IT-007, IT-052) and static diagnostics.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::model::{
    CePatchSpec, DesignSpec, ScalarField, Sourced, ToolSpec, ValueSource,
};
use rimstudio_design::ranged::{
    ReferenceArmor, SustainedInput, nominal_dps, strength_p4, sustained_dps,
};
use rimstudio_ipc_types::designer::{
    DesignerPreviewRequest, PreviewDto, ReadoutDto, SuggestionSourceDto,
};
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{preview, suggest_fill};

fn run(f: &common::Fixture, spec: DesignSpec) -> PreviewDto {
    preview(
        &f.ctx,
        DesignerPreviewRequest {
            draft: common::dto(spec),
        },
    )
    .unwrap()
}

fn readout<'a>(p: &'a PreviewDto, key: &str) -> Option<&'a ReadoutDto> {
    p.readouts.iter().find(|r| r.key == key)
}

fn value(p: &PreviewDto, key: &str) -> Option<f64> {
    readout(p, key).and_then(|r| r.value)
}

fn near(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-9)
}

#[test]
fn readouts_of_typed_numbers_match_the_hand_computed_values() {
    let f = common::fixture(false);
    let p = run(&f, common::typed_ranged());
    // warmup 1.0 plus cooldown 1.5 and a single shot: 2.5 s; 12 damage over 2.5 s is 4.8 per second.
    assert!(near(value(&p, "cycle-time"), 2.5));
    assert!(near(value(&p, "dps"), 4.8));
    assert!(near(value(&p, "implied-ap"), 12.0 * 0.015));
    // price: 30 steel at 2 silver, plus 9000 work at 0.0036 per work point
    assert!(near(value(&p, "market-value"), 60.0 + 9000.0 * 0.0036));
    let price = readout(&p, "market-value").unwrap();
    assert_eq!(price.steps.len(), 4);
    assert!((price.steps[1].value - 0.0).abs() < 1e-12, "no stuff cost");
}

#[test]
fn the_strength_index_uses_the_reference_armor_of_the_install() {
    let f = common::fixture(false);
    let p = run(&f, common::typed_ranged());
    let spec = common::typed_ranged();
    let profile = spec.ranged_profile().unwrap();
    let engine = f.ctx.require_engine().unwrap();
    let ratings = engine.pools().unwrap().set.armor.ratings.clone();
    let expected = strength_p4(
        &profile,
        &ReferenceArmor {
            sharp_ratings: &ratings,
        },
    )
    .unwrap();
    assert!(near(value(&p, "strength-index"), expected));
    assert!(near(value(&p, "dps"), nominal_dps(&profile).unwrap()));
    let hit = value(&p, "hit-dps-12").unwrap();
    assert!(hit > 0.0 && hit < 4.8);
}

#[test]
fn a_new_draft_gets_suggestions_with_sources_bands_and_an_estimate() {
    let f = common::fixture(false);
    let p = run(&f, common::ranged_spec());
    assert_eq!(p.suggestions.len(), 12);
    let damage = p
        .suggestions
        .iter()
        .find(|s| s.field == "/ranged/damage")
        .unwrap();
    assert!(damage.value.is_some_and(|v| v > 5.0 && v < 30.0));
    assert!(damage.source.is_some());
    assert!(damage.band.is_some());
    assert!(!damage.locked);
    assert_eq!(damage.n, 7);
    // work to make is a plain input: no estimate, the class stays visible
    let work = p
        .suggestions
        .iter()
        .find(|s| s.field == "/workToMake")
        .unwrap();
    assert_eq!(work.value, None);
    assert_eq!(work.source, None);
    assert_eq!(work.band, None);
    let est = p.estimate.as_ref().unwrap();
    assert_eq!(est.class_n, 7);
    assert_eq!(est.anchors.len(), 3);
    assert!(est.class_label.starts_with("based on 7 items"));
    // no CE field ever appears among the suggestions
    assert!(p.suggestions.iter().all(|s| !s.field.starts_with("/ce")));
}

#[test]
fn typed_values_are_locked_and_repeated_in_the_suggestions() {
    let f = common::fixture(false);
    let p = run(&f, common::typed_ranged());
    for (field, typed) in [
        ("/ranged/damage", 12.0),
        ("/mass", 3.5),
        ("/ranged/range", 28.0),
    ] {
        let s = p.suggestions.iter().find(|s| s.field == field).unwrap();
        assert!(s.locked, "{field}");
        assert!(near(s.value, typed), "{field}");
        assert_eq!(s.source, Some(SuggestionSourceDto::Typed), "{field}");
    }
    let burst = p
        .suggestions
        .iter()
        .find(|s| s.field == "/ranged/burstCount")
        .unwrap();
    assert!(!burst.locked);
}

#[test]
fn a_suggestion_never_overwrites_a_typed_value_through_the_toolkit_path() {
    let f = common::fixture(false);
    let mut spec = common::ranged_spec();
    spec.offer(ScalarField::Damage, 99.0, ValueSource::Typed);
    spec.offer(ScalarField::Mass, 9.9, ValueSource::Typed);
    spec.offer(ScalarField::Warmup, 0.1, ValueSource::Suggested);
    let filled = suggest_fill(&f.ctx, common::dto(spec)).unwrap();
    let draft = draft_from_dto(&filled).unwrap();
    let damage = draft.spec.scalar(ScalarField::Damage).unwrap();
    assert_eq!(damage, Sourced::typed(99.0));
    assert_eq!(
        draft.spec.scalar(ScalarField::Mass).unwrap(),
        Sourced::typed(9.9)
    );
    // a suggested value is replaced, an empty one is filled, and the CE block stays off
    let warmup = draft.spec.scalar(ScalarField::Warmup).unwrap();
    assert!(
        warmup.value > 0.5,
        "the suggestion replaced the old suggestion"
    );
    assert!(draft.spec.scalar(ScalarField::Cooldown).is_some());
    assert!(draft.spec.ce.is_none());
}

#[test]
fn suggest_fill_builds_a_complete_ranged_design_from_tier_and_role_alone() {
    let f = common::fixture(false);
    let filled = suggest_fill(&f.ctx, common::dto(common::ranged_spec())).unwrap();
    let draft = draft_from_dto(&filled).unwrap();
    for field in [
        ScalarField::Damage,
        ScalarField::Warmup,
        ScalarField::Cooldown,
        ScalarField::Range,
        ScalarField::Mass,
        ScalarField::AccuracyTouch,
        ScalarField::AccuracyLong,
    ] {
        assert!(draft.spec.scalar(field).is_some(), "{field:?}");
    }
    // work is a plain input and stays empty
    assert!(draft.spec.scalar(ScalarField::WorkToMake).is_none());
    // the preview of the filled draft now has the exact readouts
    let p = preview(&f.ctx, DesignerPreviewRequest { draft: filled }).unwrap();
    assert!(value(&p, "dps").is_some_and(|v| v > 0.0));
    assert!(value(&p, "hit-dps-25").is_some());
}

#[test]
fn the_preview_is_deterministic() {
    let f = common::fixture(false);
    let a = serde_json::to_string(&run(&f, common::typed_ranged())).unwrap();
    let b = serde_json::to_string(&run(&f, common::typed_ranged())).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_spec_without_a_ce_block_has_no_ce_readout_even_when_ce_is_installed() {
    let f = common::fixture(true);
    let p = run(&f, common::typed_ranged());
    assert!(p.readouts.iter().all(|r| !r.key.starts_with("ce-")));
    assert!(p.diagnostics.iter().all(|d| d.code != "design.ce-absent"));
    assert!(p.suggestions.iter().all(|s| !s.field.starts_with("/ce")));
}

#[test]
fn with_the_ce_block_and_ce_installed_the_ce_readouts_appear() {
    let f = common::fixture(true);
    let mut spec = common::typed_ranged();
    spec.ce = Some(CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        default_projectile: Some("RS_CeBullet1".into()),
        magazine_size: Some(Sourced::typed(30)),
        reload_time: Some(Sourced::typed(4.0)),
        bulk: Some(Sourced::typed(6.0)),
        ..CePatchSpec::default()
    });
    let p = run(&f, spec);
    assert!(near(value(&p, "ce-bulk"), 6.0));
    assert!(near(value(&p, "ce-ammo-damage"), 9.0));
    let expected = sustained_dps(&SustainedInput {
        damage: 9.0,
        magazine: 30,
        burst_count: 1,
        ticks_between_shots: 15.0,
        warmup: 1.0,
        cooldown: 1.5,
        reload: 4.0,
    })
    .unwrap();
    assert!(near(value(&p, "ce-sustained-dps"), expected));
    assert!(p.diagnostics.iter().all(|d| d.code != "design.ce-absent"));
    // the vanilla readouts are unchanged by the block
    assert!(near(value(&p, "dps"), 4.8));
}

#[test]
fn the_ce_block_without_ce_data_gives_a_diagnostic_and_no_ce_readout() {
    let f = common::fixture(false);
    let mut spec = common::typed_ranged();
    spec.ce = Some(CePatchSpec::default());
    let p = run(&f, spec);
    assert!(p.readouts.iter().all(|r| !r.key.starts_with("ce-")));
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.code == "design.ce-absent")
        .unwrap();
    assert_eq!(d.field.as_deref(), Some("/ce"));
}

#[test]
fn without_a_game_install_the_exact_readouts_still_work() {
    let (_tmp, ctx, _clock) = common::offline();
    let p = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: common::dto(common::typed_ranged()),
        },
    )
    .unwrap();
    assert!(near(value(&p, "dps"), 4.8));
    assert_eq!(value(&p, "strength-index"), None);
    assert_eq!(value(&p, "market-value"), None);
    assert!(p.suggestions.is_empty());
    assert!(p.estimate.is_none());
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == "design.required-missing")
    );
}

#[test]
fn diagnostics_check_the_def_name_and_the_references_against_the_install() {
    let f = common::fixture(false);
    let mut spec = common::typed_ranged();
    spec.identity.def_name = "RS_Gun00".into();
    spec.cost_list.push(rimstudio_design::model::CostEntry::new(
        "RS_NoSuchItem",
        2.0,
    ));
    let p = run(&f, spec);
    let conflict = p
        .diagnostics
        .iter()
        .find(|d| d.code == "design.defname-conflict")
        .unwrap();
    assert_eq!(
        conflict.severity,
        rimstudio_ipc_types::diagnostic::SeverityDto::Error
    );
    let unresolved: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == "design.ref-unresolved")
        .collect();
    assert!(
        unresolved
            .iter()
            .any(|d| d.args.get("value").map(String::as_str) == Some("RS_NoSuchItem"))
    );
    // the thin pool is reported as information
    assert!(p.diagnostics.iter().any(|d| d.code == "design.pool-thin"));
}

#[test]
fn melee_previews_use_the_stat_panel_and_suggest_tool_fields() {
    let f = common::fixture(false);
    let mut spec = common::melee_spec();
    spec.tools = vec![
        ToolSpec::new("head", &["Cut"]).with_numbers(10.0, 2.0, ValueSource::Typed),
        ToolSpec::new("point", &["Stab"]).with_numbers(20.0, 2.0, ValueSource::Typed),
    ];
    let p = run(&f, spec);
    assert!(near(value(&p, "melee-swing-damage"), 18.0));
    assert!(near(value(&p, "melee-dps"), 9.0));
    assert!(value(&p, "melee-fight-dps").is_some());
    assert!(readout(&p, "cycle-time").is_none());
    let fields: Vec<&str> = p.suggestions.iter().map(|s| s.field.as_str()).collect();
    assert!(fields.contains(&"/tools/0/power"));
    assert!(fields.contains(&"/tools/1/cooldownTime"));
    let power = p
        .suggestions
        .iter()
        .find(|s| s.field == "/tools/0/power")
        .unwrap();
    assert!(power.locked, "the typed tool power is locked");
}

#[test]
fn a_draft_round_trips_through_the_preview_without_change() {
    let f = common::fixture(false);
    let dto = common::dto(common::typed_ranged());
    let before = serde_json::to_string(&dto).unwrap();
    let _ = preview(&f.ctx, DesignerPreviewRequest { draft: dto.clone() }).unwrap();
    assert_eq!(serde_json::to_string(&dto).unwrap(), before);
    let again = draft_to_dto(&draft_from_dto(&dto).unwrap()).unwrap();
    assert_eq!(again, dto);
}
