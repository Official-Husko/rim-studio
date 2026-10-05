//! The suggestion of the Combat Extended block of a bow: what differs from a gun.
//!
//! A bow is asked for an arrow or bolt set (the sets the converted bows use first), gets its default
//! projectile from the set like a gun, and gets its bow tag from the converted bows when enough of them
//! agree. It has no magazine, no reload time, no recoil, no one handed flag and no belt feed.

use super::{
    AskKind, Candidate, CeModel, ChoiceField, DesignSpec, FieldStatus, NumField, SuggestionSource,
    choice, nonempty,
};
use crate::ce::patchgen::bow::{bow_ammo_sets, bow_tag_candidates};
use crate::model::CePatchSpec;

/// The numeric fields of the block of a bow.
pub(super) const BOW_FIELDS: [NumField; 4] = [
    NumField {
        pointer: "/ce/bulk",
        label: "CE bulk",
        stat: "bulk",
        required: true,
    },
    NumField {
        pointer: "/ce/swayFactor",
        label: "CE sway factor",
        stat: "sway",
        required: true,
    },
    NumField {
        pointer: "/ce/shotSpread",
        label: "CE shot spread",
        stat: "spread",
        required: true,
    },
    NumField {
        pointer: "/ce/sightsEfficiency",
        label: "CE sights efficiency",
        stat: "sights",
        required: false,
    },
];

/// The choices of a bow: the arrow set, the default projectile and the bow tag. `derived_tag` is the tag the
/// conversion flow derived from the converted bows, when it did.
pub(super) fn choices(
    _spec: &DesignSpec,
    model: &CeModel,
    held: &CePatchSpec,
    derived_tag: Option<&str>,
) -> Vec<ChoiceField> {
    let mut out = Vec::new();
    let mut ammo = choice(
        "/ce/ammoSet",
        "Which arrow or bolt set (ammo set) does the bow use?",
        AskKind::Choice,
        true,
    );
    ammo.candidates = bow_ammo_sets(model)
        .into_iter()
        .map(|name| Candidate {
            used_by: model
                .bows()
                .filter(|b| b.ammo_set.as_deref() == Some(name.as_str()))
                .count(),
            score: 0.0,
            first_damage: model
                .ammo_set(&name)
                .and_then(|s| s.first())
                .and_then(|a| a.info.damage),
            name,
        })
        .collect();
    match nonempty(&held.ammo_set) {
        Some(set) => {
            ammo.status = FieldStatus::Held;
            ammo.held = Some(set.to_owned());
        }
        None => {
            ammo.reason = Some(
                "the arrow set is never chosen for you: pick one of the candidates, the sets the converted bows use first"
                    .into(),
            );
        }
    }
    let set_info = nonempty(&held.ammo_set).and_then(|s| model.ammo_set(s));
    let mut projectile = choice(
        "/ce/defaultProjectile",
        "Which projectile of the ammo set is the default?",
        AskKind::Choice,
        true,
    );
    if let Some(set) = set_info {
        projectile.candidates = set
            .ammo_types
            .iter()
            .map(|a| Candidate {
                name: a.projectile.clone(),
                used_by: 0,
                score: 0.0,
                first_damage: None,
            })
            .collect();
    }
    match (nonempty(&held.default_projectile), set_info) {
        (Some(p), _) => {
            projectile.status = FieldStatus::Held;
            projectile.held = Some(p.to_owned());
        }
        (None, Some(set)) => match set.first() {
            Some(first) => {
                projectile.status = FieldStatus::Derived;
                projectile.value = Some(first.projectile.clone());
                projectile.source = Some(SuggestionSource::FirstOfSet);
            }
            None => {
                projectile.reason = Some("the ammo set has no members in the loaded data".into());
            }
        },
        (None, None) => {
            projectile.reason = Some("it follows the ammo set: choose the arrow set first".into());
        }
    }
    out.push(ammo);
    out.push(projectile);
    let mut tag = choice(
        "/ce/weaponTagClass",
        "Which bow tag does the weapon get?",
        AskKind::Choice,
        true,
    );
    tag.candidates = bow_tag_candidates(model)
        .into_iter()
        .map(|(name, used_by)| Candidate {
            name,
            used_by,
            score: 0.0,
            first_damage: None,
        })
        .collect();
    match (nonempty(&held.weapon_tag_class), derived_tag) {
        (Some(t), _) => {
            tag.status = FieldStatus::Held;
            tag.held = Some(t.to_owned());
        }
        (None, Some(t)) => {
            tag.status = FieldStatus::Derived;
            tag.value = Some(t.to_owned());
            tag.source = Some(SuggestionSource::Vanilla);
            tag.reason = Some("the converted bows of the library carry this tag".into());
        }
        (None, None) => {
            tag.reason = Some(
                "no tag is shared by enough converted bows to be chosen for you; pick one".into(),
            );
        }
    }
    out.push(tag);
    out
}
