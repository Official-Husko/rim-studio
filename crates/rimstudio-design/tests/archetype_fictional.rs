//! The archetype solver on a fictional install: shapes, balance targets, monotonicity, determinism, typed
//! values, older drafts, Combat Extended calibres and goldens.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

mod common;
mod common_archetype;

use common_archetype::{Install, install};
use proptest::prelude::*;
use rimstudio_design::archetype::{
    ApplyOptions, CeCalibre, CeInfo, Context, Proposal, ProposeRequest, Taxonomy, apply, propose,
};
use rimstudio_design::model::{
    ArchetypeChoice, ArchetypeMode, BalanceTarget, Descriptors, DesignSpec, Draft, ItemKind,
    RateOfFire, ScalarField, TechLevel, ValueSource,
};

fn tax() -> &'static Taxonomy {
    Taxonomy::builtin().unwrap()
}

fn request(id: &str) -> ProposeRequest {
    ProposeRequest {
        archetype: id.to_owned(),
        descriptors: Descriptors::default(),
        balance: BalanceTarget::Typical,
        mode: ArchetypeMode::Vanilla,
        strength: None,
    }
}

fn make(inst: &Install, req: &ProposeRequest, ce: Option<&CeInfo>) -> Proposal {
    let melee = tax().find(&req.archetype).unwrap().kind == ItemKind::Melee;
    let ctx = Context {
        taxonomy: tax(),
        basis: if melee {
            inst.basis_melee()
        } else {
            inst.basis_ranged()
        },
        ce,
    };
    propose(&ctx, req).unwrap()
}

fn stat(p: &Proposal, name: &str) -> f64 {
    p.stats[name]
}

#[test]
fn the_shape_of_each_family_holds() {
    let inst = install();
    let sniper = make(&inst, &request("rifle/sniper"), None);
    let assault = make(&inst, &request("rifle/assault"), None);
    let smg = make(&inst, &request("smg/light"), None);
    let shotgun = make(&inst, &request("shotgun/pump"), None);
    assert!(stat(&sniper, "range") > stat(&assault, "range"));
    assert!(stat(&assault, "range") > stat(&smg, "range"));
    assert!(stat(&smg, "range") > stat(&shotgun, "range"));
    assert!(stat(&sniper, "warmup") > stat(&assault, "warmup"));
    assert!(stat(&assault, "warmup") > stat(&smg, "warmup"));
    assert!(stat(&smg, "cooldown") < stat(&assault, "cooldown"));
    assert!(stat(&sniper, "damage") > stat(&assault, "damage"));
    assert!(stat(&assault, "damage") > stat(&smg, "damage"));
    assert!(stat(&assault, "burst") >= 2.0 && stat(&sniper, "burst") == 1.0);
    // the long accuracy of a sniper is its best, the touch accuracy of an SMG is its best
    assert!(stat(&sniper, "long") > stat(&sniper, "touch"));
    assert!(stat(&smg, "touch") > stat(&smg, "long"));
}

#[test]
fn the_balance_target_orders_the_strength_and_is_met() {
    let inst = install();
    let mut last = 0.0;
    for balance in [
        BalanceTarget::Weaker,
        BalanceTarget::Typical,
        BalanceTarget::Stronger,
    ] {
        let mut req = request("rifle/assault");
        req.balance = balance;
        let p = make(&inst, &req, None);
        assert!(p.strength.achieved > last, "{balance:?}");
        assert!(
            p.strength.error.abs() < 0.08,
            "{balance:?}: {}",
            p.strength.error
        );
        assert!(!p.strength.clamped);
        last = p.strength.achieved;
    }
    let mut req = request("rifle/assault");
    req.strength = Some(5.0);
    let p = make(&inst, &req, None);
    assert!((p.strength.achieved / 5.0 - 1.0).abs() < 0.08);
    assert!(
        p.notes
            .iter()
            .any(|n| n.contains("strength index you gave"))
    );
}

#[test]
fn a_faster_rate_of_fire_never_raises_the_cooldown_and_a_heavier_calibre_never_lowers_the_damage() {
    let inst = install();
    let mut last_cooldown = f64::INFINITY;
    let mut last_burst = 0.0;
    for rof in ["slow", "medium", "fast"] {
        let mut req = request("rifle/assault");
        req.descriptors.rof = Some(RateOfFire::Class(rof.to_owned()));
        let p = make(&inst, &req, None);
        assert!(stat(&p, "cooldown") <= last_cooldown + 1e-9, "{rof}");
        assert!(stat(&p, "burst") >= last_burst, "{rof}");
        last_cooldown = stat(&p, "cooldown");
        last_burst = stat(&p, "burst");
    }
    let mut last_rpm_cooldown = f64::INFINITY;
    for rpm in [300.0, 450.0, 650.0, 900.0, 1200.0] {
        let mut req = request("rifle/assault");
        req.descriptors.rof = Some(RateOfFire::Rpm(rpm));
        let p = make(&inst, &req, None);
        assert!(stat(&p, "cooldown") <= last_rpm_cooldown + 1e-9, "{rpm}");
        last_rpm_cooldown = stat(&p, "cooldown");
    }
    let mut last_damage = 0.0;
    for calibre in ["tiny", "small", "medium", "large", "huge"] {
        let mut req = request("pistol/revolver");
        req.descriptors.calibre = Some(calibre.to_owned());
        let id = if tax()
            .find("pistol/revolver")
            .unwrap()
            .archetype
            .calibres
            .iter()
            .any(|c| c == calibre)
        {
            "pistol/revolver"
        } else {
            continue;
        };
        req.archetype = id.to_owned();
        let p = make(&inst, &req, None);
        assert!(
            stat(&p, "damage") >= last_damage,
            "{calibre}: {} < {last_damage}",
            stat(&p, "damage")
        );
        last_damage = stat(&p, "damage");
    }
    let mut last_mass = 0.0;
    for handling in ["compact", "standard", "heavy"] {
        let mut req = request("rifle/assault");
        req.descriptors.handling = Some(handling.to_owned());
        let p = make(&inst, &req, None);
        assert!(stat(&p, "mass") >= last_mass, "{handling}");
        last_mass = stat(&p, "mass");
    }
}

#[test]
fn proposals_are_deterministic() {
    let inst = install();
    let a = serde_json::to_string(&make(&inst, &request("rifle/sniper"), None)).unwrap();
    let b = serde_json::to_string(&make(&inst, &request("rifle/sniper"), None)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn every_number_has_a_reason_and_the_archetype_chip() {
    let inst = install();
    let p = make(&inst, &request("rifle/bolt"), None);
    assert_eq!(p.source, "archetype");
    let range = p
        .values
        .iter()
        .find(|v| v.field == ScalarField::Range)
        .unwrap();
    assert!(
        range.reason.contains("the install's median is"),
        "{}",
        range.reason
    );
    assert!(
        range.reason.contains("Bolt action hunting rifle"),
        "{}",
        range.reason
    );
    assert!(range.median.is_some() && range.median_n.unwrap_or(0) > 10);
    assert!(range.terms.iter().any(|t| t.label.contains("calibre")));
    for v in p.values.iter().filter(|v| v.write) {
        assert!(
            !v.reason.is_empty() && v.value.is_finite() && v.value > 0.0,
            "{:?}",
            v.field
        );
    }
    assert!(p.strength.class_label.starts_with("based on"));
}

#[test]
fn the_proposal_is_cheap() {
    let inst = install();
    let start = std::time::Instant::now();
    for _ in 0..50 {
        make(&inst, &request("rifle/assault"), None);
    }
    // 50 proposals well inside a second even in a debug build: a single one is far below a frame
    assert!(start.elapsed().as_millis() < 2000, "{:?}", start.elapsed());
}

#[test]
fn melee_proposals_follow_the_same_machinery() {
    let inst = install();
    let long = make(&inst, &request("sword/long"), None);
    let knife = make(&inst, &request("knife/knife"), None);
    assert_eq!(long.tools.len(), 3);
    assert!(stat(&long, "swing_damage") > stat(&knife, "swing_damage"));
    assert!(stat(&long, "cooldown") >= stat(&knife, "cooldown"));
    assert!(long.strength.error.abs() < 0.1, "{}", long.strength.error);
    assert!(long.stuff.is_some());
    assert!(
        long.tools
            .iter()
            .all(|t| t.power.value > 0.0 && t.cooldown.value > 0.0)
    );
}

#[test]
fn a_thin_pool_falls_back_and_says_so() {
    let inst = install();
    let mut thin_items = inst.ranged.items.clone();
    thin_items.truncate(2);
    let mut thin = inst.ranged.clone();
    thin.items = thin_items;
    let model = rimstudio_design::baseline::Model::fit(
        thin,
        rimstudio_design::baseline::BaselineConfig::default(),
    );
    let basis = rimstudio_design::archetype::Basis {
        model: &model,
        weapons: &inst.weapons,
        armor: &common_archetype::ARMOR,
        costs: Some(&inst.costs),
    };
    let ctx = Context {
        taxonomy: tax(),
        basis,
        ce: None,
    };
    let p = propose(&ctx, &request("rifle/assault")).unwrap();
    assert!(
        p.strength.class_label.contains("widened") || p.strength.class_n <= 2,
        "{}",
        p.strength.class_label
    );
}

fn calibres() -> CeInfo {
    let c = |set: &str, damage: f64, ratio: f64| CeCalibre {
        set: set.to_owned(),
        label: set.to_owned(),
        caliber: format!("{set} caliber"),
        family: Some("Rifle".into()),
        projectile: format!("{set}_Bullet"),
        damage,
        ap_sharp: Some(4.0),
        ratio,
        weapon_count: 1,
    };
    CeInfo {
        calibres: vec![
            c("RS_Light", 6.0, 0.6),
            c("RS_Mid", 10.0, 1.0),
            c("RS_Heavy", 18.0, 1.8),
        ],
        ai_class_tags: vec!["CE_AI_AssaultRifle".into(), "CE_AI_Sniper".into()],
    }
}

#[test]
fn combat_extended_calibres_set_the_ammo_and_never_lower_the_damage_when_heavier() {
    let inst = install();
    let info = calibres();
    let mut last = 0.0;
    for set in ["RS_Light", "RS_Mid", "RS_Heavy"] {
        let mut req = request("rifle/assault");
        req.mode = ArchetypeMode::CombatExtended;
        req.descriptors.ammo_set = Some(set.to_owned());
        let p = make(&inst, &req, Some(&info));
        let ce = p.ce.as_ref().unwrap();
        assert_eq!(ce.ammo_set.as_deref(), Some(set));
        assert_eq!(ce.default_projectile, Some(format!("{set}_Bullet")));
        assert_eq!(ce.weapon_tag_class.as_deref(), Some("CE_AI_AssaultRifle"));
        assert!(stat(&p, "damage") >= last, "{set}");
        last = stat(&p, "damage");
    }
    // vanilla mode needs no Combat Extended and proposes no block
    let p = make(&inst, &request("rifle/assault"), None);
    assert!(p.ce.is_none());
    // Combat Extended mode without the data gives the vanilla numbers and a plain note
    let mut req = request("rifle/assault");
    req.mode = ArchetypeMode::CombatExtended;
    let p = make(&inst, &req, None);
    assert!(
        p.ce.as_ref()
            .is_some_and(|c| c.ammo_set.is_none() && !c.notes.is_empty())
    );
}

#[test]
fn applying_fills_the_empty_fields_and_keeps_what_was_typed() {
    let inst = install();
    let mut spec = DesignSpec::new_ranged("RS_Mine", "my rifle");
    spec.offer(ScalarField::Damage, 14.0, ValueSource::Typed);
    spec.offer(ScalarField::Range, 33.0, ValueSource::Answered);
    let p = make(&inst, &request("rifle/assault"), None);
    let report = apply(&mut spec, &p, ApplyOptions::default()).unwrap();
    assert_eq!(spec.scalar(ScalarField::Damage).unwrap().value, 14.0);
    assert!(spec.scalar(ScalarField::Damage).unwrap().is_typed());
    assert!(report.kept.iter().any(|k| k == "/ranged/damage"));
    // an answered value is replaced only by the same or a stronger source: the proposal is a suggestion
    assert_eq!(spec.scalar(ScalarField::Range).unwrap().value, 33.0);
    let cooldown = spec.scalar(ScalarField::Cooldown).unwrap();
    assert_eq!(cooldown.source, ValueSource::Suggested);
    assert_eq!(cooldown.value, stat(&p, "cooldown"));
    assert!(
        spec.scalar(ScalarField::Mass).is_some() && spec.scalar(ScalarField::WorkToMake).is_some()
    );
    assert_eq!(spec.tech_level, Some(TechLevel::Industrial));
    assert_eq!(spec.role.as_deref(), Some("rifle"));
    assert!(!spec.cost_list.is_empty() && !spec.weapon_tags.is_empty());
    assert_eq!(spec.tools.len(), 2);
    // proposing again with a stronger target replaces the suggestions and keeps the typed damage
    let mut stronger = request("rifle/assault");
    stronger.balance = BalanceTarget::Stronger;
    let p2 = make(&inst, &stronger, None);
    apply(&mut spec, &p2, ApplyOptions::default()).unwrap();
    assert_eq!(spec.scalar(ScalarField::Damage).unwrap().value, 14.0);
    assert_eq!(
        spec.scalar(ScalarField::Cooldown).unwrap().value,
        stat(&p2, "cooldown")
    );
    // a proposal for the other kind is refused
    let melee = make(&inst, &request("knife/knife"), None);
    assert!(apply(&mut spec, &melee, ApplyOptions::default()).is_err());
}

#[test]
fn a_single_shot_proposal_clears_the_burst_a_burst_proposal_wrote() {
    let inst = install();
    let mut spec = DesignSpec::new_ranged("RS_Mine", "my rifle");
    apply(
        &mut spec,
        &make(&inst, &request("rifle/assault"), None),
        ApplyOptions::default(),
    )
    .unwrap();
    assert!(spec.scalar(ScalarField::BurstCount).is_some());
    apply(
        &mut spec,
        &make(&inst, &request("rifle/sniper"), None),
        ApplyOptions::default(),
    )
    .unwrap();
    assert!(spec.scalar(ScalarField::BurstCount).is_none());
    assert!(spec.scalar(ScalarField::TicksBetweenBurstShots).is_none());
}

#[test]
fn an_older_draft_loads_without_the_choice_and_a_new_one_round_trips() {
    let old = serde_json::json!({"kind": "ranged", "spec": {"kind": "ranged"}});
    let d: Draft = serde_json::from_value(old).unwrap();
    assert!(d.archetype.is_none());
    let json = serde_json::to_value(&d).unwrap();
    assert!(json.get("archetype").is_none());
    let mut d2 = d;
    d2.archetype = Some(ArchetypeChoice {
        archetype: "rifle/assault".into(),
        descriptors: Descriptors {
            rof: Some(RateOfFire::Rpm(700.0)),
            ..Descriptors::default()
        },
        balance: BalanceTarget::Stronger,
        mode: ArchetypeMode::Vanilla,
        strength: None,
    });
    let back: Draft = serde_json::from_value(serde_json::to_value(&d2).unwrap()).unwrap();
    assert_eq!(back, d2);
}

#[test]
fn unknown_ids_and_bad_targets_are_refused_with_the_reason() {
    let inst = install();
    let ctx = Context {
        taxonomy: tax(),
        basis: inst.basis_ranged(),
        ce: None,
    };
    assert!(propose(&ctx, &request("rifle/nothing")).is_err());
    let mut bad = request("rifle/assault");
    bad.balance = BalanceTarget::Percentile(1.5);
    assert!(propose(&ctx, &bad).is_err());
    let mut bad = request("rifle/assault");
    bad.descriptors.calibre = Some("giant".into());
    assert!(
        propose(&ctx, &bad)
            .unwrap_err()
            .to_string()
            .contains("giant")
    );
    let mut bad = request("rifle/assault");
    bad.strength = Some(-1.0);
    assert!(propose(&ctx, &bad).is_err());
    // a melee archetype against the ranged pool is refused
    assert!(propose(&ctx, &request("knife/knife")).is_err());
}

#[test]
fn goldens_of_the_proposals_on_the_fictional_install() {
    let inst = install();
    let mut out = serde_json::Map::new();
    for id in [
        "rifle/assault",
        "rifle/sniper",
        "smg/light",
        "pistol/revolver",
        "bow/short",
        "shotgun/pump",
        "sword/long",
    ] {
        let p = make(&inst, &request(id), None);
        // the golden holds the numbers and reasons, not the fit report (its bands are covered elsewhere)
        let mut value = serde_json::to_value(&p).unwrap();
        value.as_object_mut().unwrap().remove("fit");
        out.insert(id.to_owned(), value);
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap();
    common::assert_golden("archetype_proposals.json", &format!("{text}\n"));
}

fn arch_ids(kind: ItemKind) -> Vec<String> {
    tax().archetypes(kind).iter().map(|a| a.id()).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn every_ranged_proposal_is_finite_positive_and_lands_near_its_target(
        idx in 0usize..24,
        p in 0.05f64..0.95,
        rpm_scale in 0.5f64..2.0,
        use_rpm in any::<bool>(),
    ) {
        let inst = install();
        let ids = arch_ids(ItemKind::Ranged);
        let id = &ids[idx % ids.len()];
        let arch = tax().find(id).unwrap();
        let mut req = request(id);
        req.balance = BalanceTarget::Percentile(p);
        if use_rpm && let Some(r) = arch.archetype.ref_rpm {
            req.descriptors.rof = Some(RateOfFire::Rpm(r * rpm_scale));
        }
        let proposal = make(&inst, &req, None);
        for v in proposal.values.iter().filter(|v| v.write) {
            prop_assert!(v.value.is_finite() && v.value > 0.0, "{id} {:?} {}", v.field, v.value);
        }
        prop_assert!(proposal.strength.clamped || proposal.strength.error.abs() < 0.15, "{id}: {}", proposal.strength.error);
        let accuracy = ["touch", "short", "medium", "long"].map(|k| proposal.stats[k]);
        prop_assert!(accuracy.iter().all(|a| (0.0..=1.0).contains(a)));
    }

    #[test]
    fn a_higher_percentile_never_lowers_the_strength(idx in 0usize..24, a in 0.05f64..0.5, b in 0.5f64..0.95) {
        let inst = install();
        let ids = arch_ids(ItemKind::Ranged);
        let id = &ids[idx % ids.len()];
        let mut low = request(id);
        low.balance = BalanceTarget::Percentile(a);
        let mut high = request(id);
        high.balance = BalanceTarget::Percentile(b);
        let (pl, ph) = (make(&inst, &low, None), make(&inst, &high, None));
        prop_assert!(ph.strength.target >= pl.strength.target);
        prop_assert!(ph.strength.achieved >= pl.strength.achieved * 0.9);
    }

    #[test]
    fn melee_proposals_are_finite(idx in 0usize..10, p in 0.05f64..0.95) {
        let inst = install();
        let ids = arch_ids(ItemKind::Melee);
        let id = &ids[idx % ids.len()];
        let mut req = request(id);
        req.balance = BalanceTarget::Percentile(p);
        let proposal = make(&inst, &req, None);
        for t in &proposal.tools {
            prop_assert!(t.power.value.is_finite() && t.power.value > 0.0 && t.cooldown.value > 0.0);
        }
        prop_assert!(proposal.strength.clamped || proposal.strength.error.abs() < 0.2, "{id}: {}", proposal.strength.error);
    }
}

#[test]
fn existing_weapons_are_named_by_the_rules_and_proposed_close_to_themselves() {
    use rimstudio_design::archetype::basis::{MeleeMedians, RangedMedians};
    use rimstudio_design::archetype::{derive_melee, derive_ranged};
    let inst = install();
    let ranged = RangedMedians::read(&inst.ranged).unwrap();
    let melee = MeleeMedians::read(&inst.melee).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    for w in &inst.weapons {
        let derived = if w.kind == rimstudio_design::classes::ItemKind::Melee {
            derive_melee(tax(), w, &melee)
        } else {
            derive_ranged(tax(), w, &ranged)
        };
        let derived = derived.unwrap_or_else(|| panic!("{} has no archetype", w.def_name));
        let family = derived.archetype.split('/').next().unwrap().to_owned();
        seen.insert(w.role.clone().unwrap(), family);
        // aimed at its own strength, the proposal for the derived archetype is near the real weapon
        let mut req = request(&derived.archetype);
        req.descriptors = derived.descriptors.clone();
        req.strength = w.strength;
        let p = make(&inst, &req, None);
        let real = w
            .stats
            .get(if w.kind == rimstudio_design::classes::ItemKind::Melee {
                "swing_damage"
            } else {
                "range"
            })
            .unwrap();
        let proposed = p.stats[if w.kind == rimstudio_design::classes::ItemKind::Melee {
            "swing_damage"
        } else {
            "range"
        }];
        assert!(
            (proposed / real - 1.0).abs() < 0.45,
            "{}: {proposed} against {real}",
            w.def_name
        );
    }
    assert_eq!(seen["sniper"], "rifle");
    assert_eq!(seen["rifle"], "rifle");
    assert_eq!(seen["shotgun"], "shotgun");
    assert_eq!(seen["heavy"], "machine-gun");
    assert_eq!(seen["bow"], "bow");
    assert_eq!(seen["blade"], "sword");
}

#[test]
fn the_scale_search_reaches_a_monotone_target_and_flags_an_unreachable_one() {
    use rimstudio_design::archetype::gun::solve_scale;
    let (s, clamped) = solve_scale(|s| Ok(2.0 * s), 3.0).unwrap();
    assert!((s - 1.5).abs() < 1e-6 && !clamped);
    let (_, clamped) = solve_scale(|s| Ok(2.0 * s), 1.0e9).unwrap();
    assert!(clamped);
    let (_, clamped) = solve_scale(|s| Ok(2.0 * s), 1.0e-9).unwrap();
    assert!(clamped);
}
