//! Reading the descriptors: the defaults of the archetype, the checks, the multipliers.

use crate::error::{DesignError, DesignResult};
use crate::model::{ArchetypeMode, Descriptors, ItemKind, RateOfFire, TechLevel};

use super::ce::{CeCalibre, CeInfo, factors_of_ratio};
use super::data::{ActionProfile, ArchetypeRef, HandlingClass, Taxonomy};

/// The limits of a numeric rate of fire against the typical one of the archetype.
const RATE_MIN: f64 = 0.4;
const RATE_MAX: f64 = 2.5;

/// The multipliers of the calibre, from a class of the ladder or a real ammo set.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibreFactors {
    /// On the damage.
    pub damage: f64,
    /// On the range.
    pub range: f64,
    /// On the mass.
    pub mass: f64,
    /// On the armor penetration.
    pub ap: f64,
    /// On the cooldown.
    pub cooldown: f64,
    /// What to call it in a reason ("large calibre", "the 5.56 set").
    pub label: String,
}

/// The descriptors with every default filled in and every id checked.
#[derive(Debug, Clone)]
pub struct Resolved<'a> {
    /// The action (guns only).
    pub action: Option<&'a ActionProfile>,
    /// The rate of fire class nearest to the choice.
    pub rof_id: String,
    /// The cadence multiplier.
    pub rate: f64,
    /// The calibre class id (guns only).
    pub calibre_id: Option<String>,
    /// The calibre multipliers (neutral for melee).
    pub calibre: CalibreFactors,
    /// The Combat Extended set the calibre came from.
    pub ammo: Option<&'a CeCalibre>,
    /// The handling class.
    pub handling: &'a HandlingClass,
    /// The tier.
    pub tier: TechLevel,
    /// Descriptors that do not apply to the archetype and were ignored.
    pub notes: Vec<String>,
}

fn unknown(what: &str, id: &str) -> DesignError {
    DesignError::invalid(
        "descriptors",
        format!("{what} `{id}` is not offered by this archetype"),
    )
}

fn nearest_rof(tax: &Taxonomy, allowed: &[String], rate: f64) -> String {
    allowed
        .iter()
        .filter_map(|id| tax.rof_class(id).map(|c| (id, c.rate)))
        .min_by(|a, b| {
            (rate.ln() - a.1.ln())
                .abs()
                .total_cmp(&(rate.ln() - b.1.ln()).abs())
        })
        .map(|(id, _)| id.clone())
        .unwrap_or_default()
}

/// The tech level a name stands for (`Industrial`), compared ignoring case.
#[must_use]
pub fn tech_level_of(name: &str) -> Option<TechLevel> {
    TechLevel::ALL
        .into_iter()
        .find(|t| t.xml_name().eq_ignore_ascii_case(name))
}

/// Resolves the descriptors against an archetype.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for an id the archetype does not offer, a numeric rate of fire that is not
/// a positive number or that the archetype has no reference for, or an ammo set the install does not have.
pub fn resolve<'a>(
    tax: &'a Taxonomy,
    arch: &ArchetypeRef<'a>,
    d: &Descriptors,
    mode: ArchetypeMode,
    ce: Option<&'a CeInfo>,
) -> DesignResult<Resolved<'a>> {
    let a = arch.archetype;
    let mut notes = Vec::new();
    let ranged = arch.kind == ItemKind::Ranged;

    let rate = match &d.rof {
        None => {
            let id = a.default_rof.clone();
            tax.rof_class(&id).map_or(1.0, |c| c.rate)
        }
        Some(RateOfFire::Class(id)) => {
            if !a.rof_classes.contains(id) {
                return Err(unknown("the rate of fire", id));
            }
            tax.rof_class(id).map_or(1.0, |c| c.rate)
        }
        Some(RateOfFire::Rpm(rpm)) => {
            let reference = a.ref_rpm.filter(|r| *r > 0.0).ok_or_else(|| {
                DesignError::invalid(
                    "descriptors",
                    "this archetype has no reference rate in rounds per minute",
                )
            })?;
            if !rpm.is_finite() || *rpm <= 0.0 {
                return Err(DesignError::invalid(
                    "descriptors",
                    "the rate of fire must be a positive number of rounds per minute",
                ));
            }
            (rpm / reference).clamp(RATE_MIN, RATE_MAX)
        }
    };
    let rof_id = nearest_rof(tax, &a.rof_classes, rate);

    let handling_id = d
        .handling
        .clone()
        .unwrap_or_else(|| a.default_handling.clone());
    if !a.handlings.contains(&handling_id) {
        return Err(unknown("the handling", &handling_id));
    }
    let handling = tax
        .handling(&handling_id)
        .ok_or_else(|| unknown("the handling", &handling_id))?;

    let tier = match d.tier {
        Some(t) => t,
        None => tech_level_of(&a.tier_hint).unwrap_or(TechLevel::Industrial),
    };

    let (action, calibre_id, calibre, ammo) = if ranged {
        let action_id = d
            .action
            .clone()
            .or_else(|| a.default_action.clone())
            .unwrap_or_default();
        if !a.actions.contains(&action_id) {
            return Err(unknown("the action", &action_id));
        }
        let action = tax.action(&action_id);
        let class_id = d
            .calibre
            .clone()
            .or_else(|| a.default_calibre.clone())
            .unwrap_or_default();
        if !a.calibres.contains(&class_id) {
            return Err(unknown("the calibre", &class_id));
        }
        let class = tax
            .calibre(&class_id)
            .ok_or_else(|| unknown("the calibre", &class_id))?;
        // The shape of an archetype is its shape at its default calibre, so the factors of a class are
        // relative to the default class.
        let base = a
            .default_calibre
            .as_deref()
            .and_then(|d| tax.calibre(d))
            .unwrap_or(class);
        let mut factors = CalibreFactors {
            damage: class.damage / base.damage,
            range: class.range / base.range,
            mass: class.mass / base.mass,
            ap: class.ap / base.ap,
            cooldown: class.cooldown / base.cooldown,
            label: format!("{} calibre", class.label.to_lowercase()),
        };
        let mut ammo = None;
        if mode == ArchetypeMode::CombatExtended
            && let Some(set) = d.ammo_set.as_deref()
        {
            let found = ce.and_then(|info| info.calibre(set)).ok_or_else(|| {
                DesignError::invalid(
                    "descriptors",
                    format!("the ammo set `{set}` is not in the Combat Extended of this install"),
                )
            })?;
            let (damage, range, mass, ap, cooldown) = factors_of_ratio(found.ratio);
            factors = CalibreFactors {
                damage,
                range,
                mass,
                ap,
                cooldown,
                label: format!("the {} ammo set", found.label),
            };
            ammo = Some(found);
        }
        (action, Some(class_id), factors, ammo)
    } else {
        if d.action.is_some() {
            notes.push("a melee weapon has no action: the action was ignored".to_owned());
        }
        if d.calibre.is_some() || d.ammo_set.is_some() {
            notes.push("a melee weapon has no calibre: the calibre was ignored".to_owned());
        }
        (
            None,
            None,
            CalibreFactors {
                damage: 1.0,
                range: 1.0,
                mass: 1.0,
                ap: 1.0,
                cooldown: 1.0,
                label: String::new(),
            },
            None,
        )
    };

    Ok(Resolved {
        action,
        rof_id,
        rate,
        calibre_id,
        calibre,
        ammo,
        handling,
        tier,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tax() -> &'static Taxonomy {
        Taxonomy::builtin().unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn defaults_come_from_the_archetype() {
        let t = tax();
        let a = t.find("rifle/assault").unwrap_or_else(|| panic!("missing"));
        let r = resolve(t, &a, &Descriptors::default(), ArchetypeMode::Vanilla, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(r.action.map(|a| a.id.as_str()), Some("burst"));
        assert_eq!(r.rof_id, "medium");
        assert_eq!(r.calibre_id.as_deref(), Some("medium"));
        assert_eq!(r.tier, TechLevel::Industrial);
        assert_eq!(r.rate, 1.0);
    }

    #[test]
    fn rounds_per_minute_map_to_a_rate_and_a_class() {
        let t = tax();
        let a = t.find("rifle/assault").unwrap_or_else(|| panic!("missing"));
        let d = Descriptors {
            rof: Some(RateOfFire::Rpm(900.0)),
            ..Descriptors::default()
        };
        let r = resolve(t, &a, &d, ArchetypeMode::Vanilla, None).unwrap_or_else(|e| panic!("{e}"));
        assert!(r.rate > 1.3 && r.rate < 1.4, "{}", r.rate);
        assert_eq!(r.rof_id, "fast");
        let bad = Descriptors {
            rof: Some(RateOfFire::Rpm(-1.0)),
            ..Descriptors::default()
        };
        assert!(resolve(t, &a, &bad, ArchetypeMode::Vanilla, None).is_err());
    }

    #[test]
    fn an_id_the_archetype_does_not_offer_is_refused() {
        let t = tax();
        let a = t.find("rifle/sniper").unwrap_or_else(|| panic!("missing"));
        let d = Descriptors {
            action: Some("full-auto".into()),
            ..Descriptors::default()
        };
        let err = resolve(t, &a, &d, ArchetypeMode::Vanilla, None).err();
        assert!(err.is_some_and(|e| e.to_string().contains("full-auto")));
    }

    #[test]
    fn a_real_ammo_set_needs_combat_extended_mode_and_the_install() {
        let t = tax();
        let a = t.find("rifle/assault").unwrap_or_else(|| panic!("missing"));
        let d = Descriptors {
            ammo_set: Some("RS_Missing".into()),
            ..Descriptors::default()
        };
        // vanilla mode ignores it, Combat Extended mode refuses a set the install does not have
        assert!(resolve(t, &a, &d, ArchetypeMode::Vanilla, None).is_ok());
        assert!(resolve(t, &a, &d, ArchetypeMode::CombatExtended, None).is_err());
    }

    #[test]
    fn melee_ignores_gun_descriptors_with_a_note() {
        let t = tax();
        let a = t.find("sword/long").unwrap_or_else(|| panic!("missing"));
        let d = Descriptors {
            calibre: Some("large".into()),
            ..Descriptors::default()
        };
        let r = resolve(t, &a, &d, ArchetypeMode::Vanilla, None).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(r.notes.len(), 1);
        assert!(r.calibre_id.is_none());
    }
}
