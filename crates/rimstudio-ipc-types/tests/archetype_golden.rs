//! Golden JSON of the archetype DTOs: the request and the choice with descriptors, a proposal, and the
//! catalogue. Run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite the files, then review the diff.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rimstudio_ipc_types::designer::{
    ArchetypeCatalogDto, ArchetypeCeCatalogDto, ArchetypeChoiceDto, ArchetypeChoiceOptionDto,
    ArchetypeDto, ArchetypeFamilyDto, ArchetypeModeDto, ArchetypeProposalDto, ArchetypeTierDto,
    BalanceOptionDto, BalanceTargetDto, CeCalibreDto, CeProposalDto, DescriptorsDto,
    DesignerArchetypeApplyRequest, DesignerArchetypeProposeRequest, DraftDto, FactorTermDto,
    ItemKindDto, ProposedCostDto, ProposedToolDto, ProposedValueDto, RateOfFireDto,
    ResolvedChoiceDto, StrengthReportDto, TechLevelDto,
};
use serde::Serialize;
use serde_json::json;

fn check<T: Serialize>(name: &str, value: &T) {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.json"));
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); run with RIMSTUDIO_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(expected, text, "golden file {name} differs");
}

fn choice() -> ArchetypeChoiceDto {
    ArchetypeChoiceDto {
        archetype: "rifle/assault".into(),
        descriptors: DescriptorsDto {
            action: Some("burst".into()),
            rof: Some(RateOfFireDto::Rpm(650.0)),
            calibre: Some("medium".into()),
            ammo_set: None,
            handling: Some("standard".into()),
            tier: Some(TechLevelDto::Industrial),
        },
        balance: BalanceTargetDto::Percentile(0.7),
        mode: ArchetypeModeDto::Vanilla,
        strength: None,
    }
}

fn value(field: &str, stat: &str, v: f64, locked: bool) -> ProposedValueDto {
    ProposedValueDto {
        field: field.into(),
        stat: stat.into(),
        value: v,
        write: true,
        source: "archetype".into(),
        reason: format!(
            "{stat} {v}: the install's median is 10 (20 reference weapons). Assault rifle x1.20."
        ),
        median: Some(10.0),
        median_n: Some(20),
        terms: vec![
            FactorTermDto {
                label: "Assault rifle".into(),
                factor: 1.2,
            },
            FactorTermDto {
                label: "strength scale".into(),
                factor: 1.0,
            },
        ],
        locked,
    }
}

fn proposal() -> ArchetypeProposalDto {
    ArchetypeProposalDto {
        choice: choice(),
        label: "Assault rifle".into(),
        kind: ItemKindDto::Ranged,
        source: "archetype".into(),
        resolved: ResolvedChoiceDto {
            action: Some("burst".into()),
            rof: "medium".into(),
            rate: 1.0,
            calibre: Some("medium".into()),
            ammo_set: None,
            handling: "standard".into(),
            tier: TechLevelDto::Industrial,
        },
        role: Some("rifle".into()),
        values: vec![
            value("/ranged/damage", "damage", 12.0, true),
            value("/ranged/range", "range", 30.9, false),
        ],
        tools: vec![ProposedToolDto {
            label: "stock".into(),
            capacities: vec!["Blunt".into()],
            power: value("/tools/0/power", "bash_power", 9.0, false),
            cooldown: value("/tools/0/cooldownTime", "bash_cooldown", 2.0, false),
            armor_penetration: None,
        }],
        cost_list: vec![ProposedCostDto {
            def_name: "RS_Steel".into(),
            count: 40.0,
            reason: "RS_Steel of 100% of the 12 reference weapons of the same tier need it.".into(),
        }],
        stuff: None,
        weapon_tags: vec!["RsGun".into()],
        weapon_classes: vec![],
        market_value: Some(420.0),
        strength: StrengthReportDto {
            percentile: 0.7,
            target: 4.2,
            achieved: 4.18,
            error: -0.005,
            scale: 0.9,
            clamped: false,
            class_label: "based on 11 items at Industrial".into(),
            class_n: 11,
            achieved_percentile: 0.68,
        },
        notes: vec![
            "the archetype is typically +28 percent in strength against the middle of its class"
                .into(),
        ],
        ce: Some(CeProposalDto {
            ammo_set: Some("RS_AmmoSetA".into()),
            caliber: Some("RS caliber".into()),
            default_projectile: Some("RS_CeBullet1".into()),
            weapon_tag_class: Some("CE_AI_Rifle".into()),
            damage_ratio: Some(1.1),
            notes: vec![],
        }),
        fit: None,
        verdict: Some("typical".into()),
    }
}

#[test]
fn the_choice_and_the_requests_keep_their_shape() {
    check("archetype-choice", &choice());
    check(
        "archetype-propose-request",
        &DesignerArchetypeProposeRequest {
            kind: ItemKindDto::Ranged,
            archetype: "rifle/assault".into(),
            descriptors: choice().descriptors,
            balance_target: BalanceTargetDto::Stronger,
            mode: ArchetypeModeDto::CombatExtended,
            strength: Some(4.5),
            draft: None,
        },
    );
}

#[test]
fn the_proposal_keeps_its_shape_and_round_trips() {
    let p = proposal();
    check("archetype-proposal", &p);
    let back: ArchetypeProposalDto =
        serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(back, p);
}

#[test]
fn the_catalogue_keeps_its_shape() {
    let c = ArchetypeCatalogDto {
        families: vec![ArchetypeFamilyDto {
            id: "rifle".into(),
            label: "Rifle".into(),
            kind: ItemKindDto::Ranged,
            archetypes: vec![ArchetypeDto {
                id: "rifle/assault".into(),
                label: "Assault rifle".into(),
                summary: "A balanced automatic rifle.".into(),
                kind: ItemKindDto::Ranged,
                applies: vec!["action".into(), "rof".into(), "calibre".into()],
                actions: vec!["burst".into(), "full-auto".into()],
                default_action: Some("burst".into()),
                rof_classes: vec!["slow".into(), "medium".into(), "fast".into()],
                default_rof: "medium".into(),
                ref_rpm: Some(650.0),
                calibres: vec!["small".into(), "medium".into()],
                default_calibre: Some("medium".into()),
                handlings: vec!["standard".into()],
                default_handling: "standard".into(),
                default_tier: TechLevelDto::Industrial,
                pool_role: Some("rifle".into()),
                ammo_families: vec!["rifle".into()],
            }],
        }],
        rof: vec![ArchetypeChoiceOptionDto {
            id: "fast".into(),
            label: "Fast".into(),
            summary: None,
            rate: Some(1.35),
        }],
        actions: vec![],
        calibres: vec![],
        handlings: vec![],
        tiers: vec![ArchetypeTierDto {
            tier: TechLevelDto::Industrial,
            label: "Industrial".into(),
            ranged_count: 12,
            melee_count: 0,
        }],
        balance: vec![BalanceOptionDto {
            target: BalanceTargetDto::Typical,
            label: "Typical for its class".into(),
            percentile: 0.5,
        }],
        pool_sizes: [20, 17],
        proposals_available: true,
        ce: ArchetypeCeCatalogDto {
            available: true,
            reason: None,
            calibres: vec![CeCalibreDto {
                set: "RS_AmmoSetA".into(),
                label: "RS ammo".into(),
                caliber: "RS caliber".into(),
                family: Some("rifle".into()),
                projectile: "RS_CeBullet1".into(),
                damage: 9.0,
                ap_sharp: Some(2.5),
                ratio: 1.0,
                weapon_count: 3,
            }],
            ai_class_tags: vec!["CE_AI_Rifle".into()],
        },
    };
    check("archetype-catalog", &c);
}

#[test]
fn the_wire_forms_of_the_descriptors_are_the_ones_the_design_engine_uses() {
    let rof: RateOfFireDto = serde_json::from_value(json!({"class": "fast"})).unwrap();
    assert_eq!(rof, RateOfFireDto::Class("fast".into()));
    let rpm: RateOfFireDto = serde_json::from_value(json!({"rpm": 600})).unwrap();
    assert_eq!(rpm, RateOfFireDto::Rpm(600.0));
    let named: BalanceTargetDto = serde_json::from_value(json!("weaker")).unwrap();
    assert_eq!(named, BalanceTargetDto::Weaker);
    let pct: BalanceTargetDto = serde_json::from_value(json!({"percentile": 0.3})).unwrap();
    assert_eq!(pct, BalanceTargetDto::Percentile(0.3));
    // a request that omits the optional parts uses the defaults
    let apply: Result<DesignerArchetypeApplyRequest, _> = serde_json::from_value(json!({
        "draft": serde_json::to_value(DraftDto::new(rimstudio_ipc_types::designer::DesignSpecDto::new(ItemKindDto::Ranged))).unwrap(),
        "proposal": serde_json::to_value(proposal()).unwrap(),
    }));
    let apply = apply.unwrap();
    assert!(!apply.include_ce && !apply.refresh_structure && apply.complete_structure);
    // an older draft has no archetype member
    let draft = serde_json::to_value(DraftDto::new(
        rimstudio_ipc_types::designer::DesignSpecDto::new(ItemKindDto::Melee),
    ))
    .unwrap();
    assert!(draft.get("archetype").is_none());
}
