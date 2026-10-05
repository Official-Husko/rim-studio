//! The Combat Extended block of a bow in the convert flow: the choices and numbers that differ from a gun.
//!
//! A bow is asked for its arrow or bolt set (the sets the converted bows use first, then every set that
//! serves arrows), takes its default projectile from the first member of the set like a gun, and takes its
//! tag and its ammo spawn count from the converted bows when enough of them agree. It has no magazine, no
//! reload time and no recoil: those exist only when the user gives them. Its numbers are estimated from the
//! converted bows alone (see [`crate::ce::patchgen::bow::bow_prediction`]).

use super::{AskItem, AskKind, Deriver, guess_of, whole};
use crate::ce::classes::ConversionPrediction;
use crate::ce::patchgen::bow::{ammo_gen_habit, bow_ammo_sets, bow_tag_candidates, bow_tag_habit};
use crate::model::{CePatchSpec, Sourced, ValueSource};

/// Asks for the arrow set, derives the default projectile, and derives or asks for the bow tag.
pub(super) fn choices(d: &mut Deriver<'_>, block: &mut CePatchSpec) {
    if block.ammo_set.is_none() {
        d.asks.push(AskItem {
            field: "/ce/ammoSet".into(),
            label: "Which arrow or bolt set (ammo set) does the bow use?".into(),
            kind: AskKind::Choice,
            options: bow_ammo_sets(d.model),
            reason: None,
            suggestion: None,
        });
    }
    if block.default_projectile.is_none() {
        let first = block
            .ammo_set
            .as_deref()
            .and_then(|s| d.model.ammo_set(s))
            .and_then(|s| s.first())
            .map(|a| a.projectile.clone());
        match first {
            Some(p) => block.default_projectile = Some(p),
            None if block.ammo_set.is_some() => d.asks.push(AskItem {
                field: "/ce/defaultProjectile".into(),
                label: "Which projectile of the ammo set is the default?".into(),
                kind: AskKind::Choice,
                options: Vec::new(),
                reason: None,
                suggestion: None,
            }),
            None => {}
        }
    }
    if block.weapon_tag_class.is_none() {
        match bow_tag_habit(d.model) {
            Some(tag) => block.weapon_tag_class = Some(tag),
            None => {
                let mut options: Vec<String> = bow_tag_candidates(d.model)
                    .into_iter()
                    .map(|(tag, _)| tag)
                    .collect();
                if options.is_empty() {
                    options.clone_from(&d.model.weapon_tags);
                }
                d.asks.push(AskItem {
                    field: "/ce/weaponTagClass".into(),
                    label: "Which bow tag does the weapon get?".into(),
                    kind: AskKind::Choice,
                    options,
                    reason: None,
                    suggestion: None,
                });
            }
        }
    }
}

/// The numbers of the block of a bow. `prediction` is the estimate over the converted bows.
pub(super) fn numbers(
    d: &mut Deriver<'_>,
    block: &mut CePatchSpec,
    prediction: Option<&ConversionPrediction>,
) {
    let over = d.answers.overrides.clone();
    let mut derived = Vec::new();
    let bulk = guess_of(prediction, "bulk", &mut derived, "ce.bulk");
    let sway = guess_of(prediction, "sway", &mut derived, "ce.swayFactor");
    let spread = guess_of(prediction, "spread", &mut derived, "ce.shotSpread");
    let sights = guess_of(prediction, "sights", &mut derived, "ce.sightsEfficiency");
    d.derived.extend(derived);
    block.bulk = d.number("/ce/bulk", "CE bulk", over.bulk, &bulk, false);
    block.sway_factor = d.number(
        "/ce/swayFactor",
        "CE sway factor",
        over.sway_factor,
        &sway,
        false,
    );
    block.shot_spread = d.number(
        "/ce/shotSpread",
        "CE shot spread",
        over.shot_spread,
        &spread,
        false,
    );
    block.sights_efficiency = d.number(
        "/ce/sightsEfficiency",
        "CE sights efficiency",
        over.sights_efficiency,
        &sights,
        true,
    );
    block.cooldown = over.cooldown;
    block.recoil_amount = over.recoil_amount;
    block.magazine_size = over.magazine_size;
    block.reload_time = over.reload_time;
    block.mass = over.mass;
    block.allow_with_run_and_gun = over.allow_with_run_and_gun;
    block.ammo_gen_per_mag = over.ammo_gen_per_mag.or_else(|| {
        whole(ammo_gen_habit(d.model).map(f64::from))
            .map(|v| Sourced::new(v, ValueSource::Suggested))
    });
}
