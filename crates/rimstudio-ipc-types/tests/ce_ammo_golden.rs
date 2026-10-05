//! Golden JSON of the custom ammunition DTOs: the custom ammo spec of the Combat Extended block, the
//! catalogue page and the suggestion for a new ammo type. Run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite the
//! files, then review the diff.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rimstudio_ipc_types::designer::{
    CeAmmoCatalogDto, CeAmmoEntryDto, CeAmmoFacetDto, CeAmmoSecondaryDto, CeAmmoSourceDto,
    CeAmmoSuggestedFieldDto, CeAmmoSuggestionDto, CeAmmoTypeDto, CeNearestAmmoDto, CeRatingDto,
    CookOffKindDto, CustomAmmoDto, CustomAmmoItemDto, CustomAmmoRecipeDto, CustomAmmoTypeDto,
    CustomIngredientDto, CustomProjectileDto, CustomSecondaryDamageDto,
    DesignerCeAmmoCatalogRequest, DesignerCeAmmoSuggestRequest, SourcedDto, ValueSourceDto,
};
use serde::Serialize;

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

fn n(v: f64, source: ValueSourceDto) -> Option<SourcedDto<f64>> {
    Some(SourcedDto { value: v, source })
}

fn custom() -> CustomAmmoDto {
    CustomAmmoDto {
        name: "Mine".into(),
        caliber: "my test caliber".into(),
        similar_to: Some("RS_AmmoSet_Rifle".into()),
        category_parent: Some("RS_AmmoRifles".into()),
        default_type: Some("FMJ".into()),
        types: vec![CustomAmmoTypeDto {
            key: "FMJ".into(),
            ammo_class: "RS_FMJ".into(),
            label: Some("my test caliber (FMJ)".into()),
            projectile: CustomProjectileDto {
                parent: Some("RS_BulletBase".into()),
                damage_def: Some("RS_Bullet".into()),
                damage: n(16.0, ValueSourceDto::Answered),
                armor_penetration_sharp: n(7.0, ValueSourceDto::Suggested),
                armor_penetration_blunt: n(21.0, ValueSourceDto::Suggested),
                speed: n(155.0, ValueSourceDto::Typed),
                secondary_damage: vec![CustomSecondaryDamageDto {
                    def: "RS_BombSecondary".into(),
                    amount: n(4.0, ValueSourceDto::Typed),
                }],
                ..CustomProjectileDto::default()
            },
            item: CustomAmmoItemDto {
                parent: Some("RS_AmmoBase".into()),
                mass: n(0.014, ValueSourceDto::Suggested),
                bulk: n(0.028, ValueSourceDto::Suggested),
                cook_off: Some(CookOffKindDto::Projectile),
                ..CustomAmmoItemDto::default()
            },
            recipe: CustomAmmoRecipeDto {
                ingredients: vec![CustomIngredientDto {
                    thing: "Steel".into(),
                    count: n(12.0, ValueSourceDto::Typed),
                    ..CustomIngredientDto::default()
                }],
                work_amount: n(1500.0, ValueSourceDto::Suggested),
                ..CustomAmmoRecipeDto::default()
            },
            ..CustomAmmoTypeDto::default()
        }],
        ..CustomAmmoDto::default()
    }
}

#[test]
fn golden_custom_ammo() {
    check("ce-ammo-custom", &custom());
}

#[test]
fn golden_ammo_catalog() {
    let page = CeAmmoCatalogDto {
        available: true,
        total: 307,
        matching: 1,
        page: 0,
        page_size: 25,
        entries: vec![CeAmmoEntryDto {
            def_name: "AmmoSet_Test".into(),
            label: "test caliber".into(),
            caliber: "test caliber".into(),
            family: Some("rifle".into()),
            similar_to: Some("AmmoSet_Rifle".into()),
            generic: false,
            similar_sets: 0,
            types: vec![CeAmmoTypeDto {
                ammo_def: "Ammo_Test_FMJ".into(),
                ammo_label: "test caliber (FMJ)".into(),
                ammo_class: "FullMetalJacket".into(),
                ammo_class_label: "FMJ".into(),
                projectile_def: "Bullet_Test_FMJ".into(),
                damage_def: Some("Bullet".into()),
                damage: Some(16.0),
                armor_penetration_sharp: Some(7.0),
                armor_penetration_blunt: Some(21.0),
                speed: Some(155.0),
                pellets: None,
                explosion_radius: None,
                secondary_damage: vec![CeAmmoSecondaryDto {
                    def: "Bomb_Secondary".into(),
                    amount: 4.0,
                }],
            }],
            weapon_count: 3,
            example_weapons: vec!["assault rifle".into()],
            suggested: true,
            score: Some(2.5),
        }],
        calibers: vec![CeAmmoFacetDto {
            name: "rifle".into(),
            label: "rifle".into(),
            count: 1,
        }],
        classes: vec![CeAmmoFacetDto {
            name: "FullMetalJacket".into(),
            label: "FMJ".into(),
            count: 1,
        }],
        reason: None,
    };
    check("ce-ammo-catalog", &page);
    let request = DesignerCeAmmoCatalogRequest {
        query: Some("rifle".into()),
        class: Some("FullMetalJacket".into()),
        page: Some(1),
        ..DesignerCeAmmoCatalogRequest::default()
    };
    check("ce-ammo-catalog-request", &request);
}

#[test]
fn golden_ammo_suggestion() {
    let s = CeAmmoSuggestionDto {
        available: true,
        reason: None,
        class: "FullMetalJacket".into(),
        class_label: "FMJ".into(),
        copied_from: None,
        nearest: vec![CeNearestAmmoDto {
            ammo_def: "Ammo_Test_FMJ".into(),
            ammo_label: "test caliber (FMJ)".into(),
            set: Some("AmmoSet_Test".into()),
            projectile_def: "Bullet_Test_FMJ".into(),
            damage: Some(16.0),
            speed: Some(155.0),
            energy: Some(2480.0),
            distance: 0.12,
        }],
        fields: vec![CeAmmoSuggestedFieldDto {
            field: "/projectile/armorPenetrationSharp".into(),
            label: "sharp penetration".into(),
            value: Some(7.0),
            text: None,
            source: CeAmmoSourceDto::Nearest,
            rating: CeRatingDto::Rough,
            n: 5,
            from: vec!["Ammo_Test_FMJ".into()],
            range: Some((6.0, 9.0)),
        }],
        ammo_type: custom().types.remove(0),
        notes: Vec::new(),
    };
    check("ce-ammo-suggestion", &s);
    let request = DesignerCeAmmoSuggestRequest {
        class: "FullMetalJacket".into(),
        copy_from: Some("Ammo_Test_FMJ".into()),
        ..DesignerCeAmmoSuggestRequest::default()
    };
    check("ce-ammo-suggest-request", &request);
}

#[test]
fn a_minimal_request_and_a_hand_written_spec_read_with_defaults() {
    let r: DesignerCeAmmoCatalogRequest = serde_json::from_str("{}").unwrap();
    assert!(r.draft.is_none() && r.page.is_none());
    let c: CustomAmmoDto = serde_json::from_str(r#"{"name":"A","caliber":"b","types":[{"ammoClass":"X","projectile":{"damage":{"value":3}}}]}"#).unwrap();
    assert_eq!(
        c.types[0].projectile.damage.unwrap().source,
        ValueSourceDto::Typed
    );
    assert!(c.types[0].item.mass.is_none());
}
