//! Fictional fixtures shared by the model, validation and plan tests. Every name starts with `RS_` and
//! every number is invented; nothing here comes from a game install.

#![allow(dead_code)]

use rimstudio_design::model::{
    CePatchSpec, CeToolPenetration, CostEntry, DesignSpec, ParentRef, ProjectileChoice,
    ProjectileSpec, ScalarField, Sourced, StuffSpec, TechLevel, ToolSpec, ValueSource,
};

/// A complete, valid ranged spec with an inline projectile and a gun bash tool.
pub(crate) fn ranged_spec() -> DesignSpec {
    let mut s = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
    s.identity.description = "A fictional rifle used by the tests.".into();
    s.identity.mod_prefix = "RS".into();
    s.parent = Some(ParentRef::named("RS_BaseGun"));
    s.tech_level = Some(TechLevel::Industrial);
    s.role = Some("rifle".into());
    s.mass = Some(Sourced::typed(3.3));
    s.work_to_make = Some(Sourced::answered(9100.0));
    s.cost_list = vec![
        CostEntry::new("RS_Metal", 40.0),
        CostEntry::new("RS_Part", 3.0),
    ];
    s.research_prerequisite = Some("RS_Research".into());
    s.weapon_tags = vec!["RS_Gun".into()];
    s.texture_path = Some("Things/RS/TestRifle".into());
    for (field, value) in [
        (ScalarField::Damage, 11.0),
        (ScalarField::Range, 27.5),
        (ScalarField::BurstCount, 3.0),
        (ScalarField::TicksBetweenBurstShots, 8.0),
        (ScalarField::Warmup, 1.4),
        (ScalarField::Cooldown, 1.9),
        (ScalarField::AccuracyTouch, 0.6),
        (ScalarField::AccuracyShort, 0.7),
        (ScalarField::AccuracyMedium, 0.62),
        (ScalarField::AccuracyLong, 0.5),
    ] {
        s.offer(field, value, ValueSource::Suggested);
    }
    if let Some(r) = s.ranged.as_mut() {
        r.sound_cast = Some("RS_Shot".into());
        r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
            def_name: "RS_Bullet_TestRifle".into(),
            label: "test bullet".into(),
            texture_path: Some("Things/RS/Bullet".into()),
            speed: Some(Sourced::typed(50.0)),
            ..ProjectileSpec::default()
        }));
    }
    s.tools =
        vec![ToolSpec::new("grip", &["Blunt"]).with_numbers(8.0, 2.1, ValueSource::Suggested)];
    s
}

/// A complete, valid melee spec with three tools.
pub(crate) fn melee_spec() -> DesignSpec {
    let mut s = DesignSpec::new_melee("RS_TestBlade", "test blade");
    s.identity.mod_prefix = "RS".into();
    s.parent = Some(ParentRef::named("RS_BaseMelee"));
    s.tech_level = Some(TechLevel::Medieval);
    s.role = Some("blade".into());
    s.mass = Some(Sourced::typed(1.4));
    s.work_to_make = Some(Sourced::typed(7000.0));
    s.stuff = Some(StuffSpec {
        categories: vec!["RS_Metallic".into()],
        count: Some(Sourced::typed(60.0)),
    });
    let mut handle =
        ToolSpec::new("handle", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Suggested);
    handle.chance_factor = Some(Sourced::typed(0.3));
    handle.linked_body_parts_group = Some("RS_Handle".into());
    s.tools = vec![
        handle,
        ToolSpec::new("blade", &["Cut"]).with_numbers(18.0, 2.4, ValueSource::Suggested),
        ToolSpec::new("point", &["Stab"]).with_numbers(20.0, 2.6, ValueSource::Answered),
    ];
    s.weapon_tags = vec!["RS_Blade".into()];
    s
}

/// A complete CE patch block for [`ranged_spec`].
pub(crate) fn ranged_ce() -> CePatchSpec {
    CePatchSpec {
        ammo_set: Some("RS_AmmoSet".into()),
        default_projectile: Some("RS_Bullet_CE".into()),
        magazine_size: Some(Sourced::typed(30)),
        reload_time: Some(Sourced::typed(4.0)),
        bulk: Some(Sourced::suggested(8.5)),
        sway_factor: Some(Sourced::suggested(1.2)),
        shot_spread: Some(Sourced::suggested(0.1)),
        weapon_tag_class: Some("RS_CE_Class".into()),
        ..CePatchSpec::default()
    }
}

/// A complete CE patch block for [`melee_spec`].
pub(crate) fn melee_ce() -> CePatchSpec {
    let pen = |tool: &str, sharp: Option<f64>, blunt: f64| CeToolPenetration {
        tool: tool.into(),
        sharp: sharp.map(Sourced::typed),
        blunt: Some(Sourced::typed(blunt)),
    };
    CePatchSpec {
        bulk: Some(Sourced::suggested(6.0)),
        melee_crit_chance: Some(Sourced::typed(0.1)),
        melee_parry_chance: Some(Sourced::typed(0.2)),
        melee_dodge_chance: Some(Sourced::typed(-0.05)),
        tool_penetration: vec![
            pen("handle", None, 0.4),
            pen("blade", Some(0.6), 0.5),
            pen("point", Some(0.9), 0.5),
        ],
        ..CePatchSpec::default()
    }
}

/// Reads a golden file, or writes it when `RIMSTUDIO_UPDATE_GOLDEN` is set, and compares.
pub(crate) fn assert_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("missing golden file {name}; run with RIMSTUDIO_UPDATE_GOLDEN=1")
    });
    assert_eq!(expected, actual, "golden file {name} differs");
}
