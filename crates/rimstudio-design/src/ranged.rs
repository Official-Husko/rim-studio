//! Vanilla ranged weapon math: cycle time, nominal and hit adjusted DPS, implied armor penetration and the
//! strength index P4.
//!
//! Units: time in seconds, `ticks_between_shots` in ticks (60 per second), range and distance in cells (the
//! game's tiles), accuracy values as fractions in `[0, 1]`, damage in hit points per shot.
//! Shooter skill, weather and cover multiply every weapon alike and are excluded.
//!
//! The same functions serve any weapon whose verb has warmup, cooldown and a burst; beam, explosive,
//! mechanoid and turret weapons are outside the model and the caller must not feed them in.

use crate::armor::layer_survival;
use crate::error::{DesignError, DesignResult, finite, non_negative, positive};
use serde::{Deserialize, Serialize};

/// Game ticks per second.
pub const TICKS_PER_SECOND: f64 = 60.0;

/// Armor penetration the game assumes when nothing sets one: this fraction of the damage.
pub const DEFAULT_AP_PER_DAMAGE: f64 = 0.015;

/// Smallest hit factor the model returns.
pub const MIN_HIT_FACTOR: f64 = 0.01;

/// Inputs of [`cycle_time`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CycleInput {
    /// Warmup in seconds, paid once per burst.
    pub warmup: f64,
    /// Cooldown in seconds (the weapon stat), paid after the burst.
    pub cooldown: f64,
    /// Shots per burst, at least 1.
    pub burst_count: u32,
    /// Ticks between two shots of a burst (the game default is 15).
    pub ticks_between_shots: f64,
}

/// Full cycle time in seconds: `warmup + cooldown + (burst - 1) * ticks_between / 60`.
///
/// Hand check (vector RG-1): warmup 0.8, cooldown 1.5, burst 3, 10 ticks between shots gives
/// `0.8 + 1.5 + 2 * 10 / 60 = 2.6333` seconds.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite or negative value or a burst of 0, and
/// [`DesignError::Degenerate`] when the cycle takes no time at all.
pub fn cycle_time(input: &CycleInput) -> DesignResult<f64> {
    let warmup = non_negative("warmup", input.warmup)?;
    let cooldown = non_negative("cooldown", input.cooldown)?;
    let ticks = non_negative("ticks_between_shots", input.ticks_between_shots)?;
    if input.burst_count == 0 {
        return Err(DesignError::invalid("burst_count", "must be at least 1"));
    }
    let gaps = f64::from(input.burst_count - 1);
    let cycle = warmup + cooldown + gaps * ticks / TICKS_PER_SECOND;
    if !cycle.is_finite() {
        return Err(DesignError::invalid("cycle", "overflowed"));
    }
    if cycle <= 0.0 {
        return Err(DesignError::Degenerate {
            what: "the full cycle takes no time",
        });
    }
    Ok(cycle)
}

/// Weapon accuracy at the four range bands, as fractions in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccuracyBands {
    /// Accuracy up to 3 cells.
    pub touch: f64,
    /// Accuracy at 12 cells.
    pub short: f64,
    /// Accuracy at 25 cells.
    pub medium: f64,
    /// Accuracy at 40 cells and beyond.
    pub long: f64,
}

/// Hit factor of the weapon at `distance` cells.
///
/// Piecewise linear through (3, touch), (12, short), (25, medium), (40, long), constant outside, clamped to
/// `[0.01, 1]`.
///
/// Hand check (vector RG-2): touch 0.6, short 0.7, medium 0.65, long 0.55 at 18 cells lies on the segment
/// from (12, 0.7) to (25, 0.65): `0.7 + (0.65 - 0.7) * 6 / 13 = 0.676923`.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite accuracy or a non finite or negative distance.
pub fn hit_factor(bands: &AccuracyBands, distance: f64) -> DesignResult<f64> {
    let touch = finite("accuracy.touch", bands.touch)?;
    let short = finite("accuracy.short", bands.short)?;
    let medium = finite("accuracy.medium", bands.medium)?;
    let long = finite("accuracy.long", bands.long)?;
    let d = non_negative("distance", distance)?;
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t.clamp(0.0, 1.0);
    let value = if d <= 3.0 {
        touch
    } else if d <= 12.0 {
        lerp(touch, short, (d - 3.0) / 9.0)
    } else if d <= 25.0 {
        lerp(short, medium, (d - 12.0) / 13.0)
    } else if d <= 40.0 {
        lerp(medium, long, (d - 25.0) / 15.0)
    } else {
        long
    };
    Ok(value.clamp(MIN_HIT_FACTOR, 1.0))
}

/// Everything the ranged formulas need to know about one weapon.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RangedProfile {
    /// Damage per shot (after the quality multiplier).
    pub damage: f64,
    /// Armor penetration of a shot as a fraction (see [`implied_ap`]).
    pub armor_penetration: f64,
    /// Warmup in seconds.
    pub warmup: f64,
    /// Cooldown in seconds.
    pub cooldown: f64,
    /// Shots per burst.
    pub burst_count: u32,
    /// Ticks between burst shots.
    pub ticks_between_shots: f64,
    /// Maximum range in cells.
    pub range: f64,
    /// Accuracy bands; `None` means the weapon has no accuracy stats and always hits (factor 1).
    pub accuracy: Option<AccuracyBands>,
}

impl RangedProfile {
    /// The cycle inputs of this profile.
    #[must_use]
    pub fn cycle_input(&self) -> CycleInput {
        CycleInput {
            warmup: self.warmup,
            cooldown: self.cooldown,
            burst_count: self.burst_count,
            ticks_between_shots: self.ticks_between_shots,
        }
    }
}

/// Nominal damage per second: `damage * burst / cycle`.
///
/// Hand check (vector RG-1): damage 10, burst 3 over a cycle of 2.6333 s gives `30 / 2.6333 = 11.3924`.
/// Vector RG-3: damage 20, burst 1, cycle `2.0 + 1.2 = 3.2` gives `6.25`.
///
/// # Errors
///
/// See [`cycle_time`]; also [`DesignError::InvalidInput`] for a bad damage.
pub fn nominal_dps(profile: &RangedProfile) -> DesignResult<f64> {
    let damage = non_negative("damage", profile.damage)?;
    let cycle = cycle_time(&profile.cycle_input())?;
    Ok(damage * f64::from(profile.burst_count) / cycle)
}

/// Hit factor of a profile at a distance: [`hit_factor`] of its bands, or 1 without accuracy stats.
///
/// # Errors
///
/// See [`hit_factor`].
pub fn profile_hit_factor(profile: &RangedProfile, distance: f64) -> DesignResult<f64> {
    match &profile.accuracy {
        Some(bands) => hit_factor(bands, distance),
        None => {
            non_negative("distance", distance)?;
            Ok(1.0)
        }
    }
}

/// Nominal DPS multiplied by the hit factor at `distance` cells.
///
/// Hand check: the RG-1 gun (nominal 11.3924) with the RG-2 accuracy at 18 cells gives
/// `11.3924 * 0.676923 = 7.7116`.
///
/// # Errors
///
/// See [`nominal_dps`] and [`hit_factor`].
pub fn hit_adjusted_dps(profile: &RangedProfile, distance: f64) -> DesignResult<f64> {
    Ok(nominal_dps(profile)? * profile_hit_factor(profile, distance)?)
}

/// What a projectile definition sets, for [`implied_ap`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImpliedApInput {
    /// Damage per shot.
    pub damage: f64,
    /// The projectile's explicit armor penetration base, when it sets one. A negative value counts as unset.
    pub explicit_ap: Option<f64>,
    /// Whether the projectile sets its own damage amount base.
    pub sets_damage: bool,
    /// The damage definition's default penetration, used only when the projectile sets neither damage nor
    /// penetration. A negative value counts as unset.
    pub damage_def_default_ap: Option<f64>,
    /// The weapon's penetration multiplier stat (1 at normal quality).
    pub ap_multiplier: f64,
}

/// Armor penetration the game applies to a shot, as a fraction.
///
/// Rule: an explicit non negative `explicit_ap` wins. If the projectile only sets its damage, the damage
/// definition's default is not inherited. If it sets neither, the damage definition's default applies. Any
/// result that is still unset or negative becomes `0.015 * damage`. The weapon's multiplier is applied last.
///
/// Hand checks: vector RG-1 (damage 10, nothing explicit, damage set) gives `0.015 * 10 = 0.15`. An explicit
/// value is kept as written: damage 18 with explicit 0.14 gives 0.14, not `0.015 * 18 = 0.27`. (Vector RG-3 uses
/// damage 20 with explicit 0.30, which happens to equal `0.015 * 20`; the explicit-kept path is the one it
/// names.)
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite or negative damage or multiplier, or any non finite
/// penetration.
pub fn implied_ap(input: &ImpliedApInput) -> DesignResult<f64> {
    let damage = non_negative("damage", input.damage)?;
    let multiplier = non_negative("ap_multiplier", input.ap_multiplier)?;
    let explicit = match input.explicit_ap {
        Some(v) => Some(finite("explicit_ap", v)?).filter(|v| *v >= 0.0),
        None => None,
    };
    let default = match input.damage_def_default_ap {
        Some(v) => Some(finite("damage_def_default_ap", v)?).filter(|v| *v >= 0.0),
        None => None,
    };
    let base = if let Some(v) = explicit {
        v
    } else if input.sets_damage {
        damage * DEFAULT_AP_PER_DAMAGE
    } else {
        default.unwrap_or(damage * DEFAULT_AP_PER_DAMAGE)
    };
    Ok(base * multiplier)
}

/// Reference armor for the strength index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceArmor<'a> {
    /// Sharp ratings (fractions) of the reference layers, read from the user's apparel: typically an
    /// unarmored layer (0), a flak like layer and a heavy layer.
    pub sharp_ratings: &'a [f64],
}

/// Strength index P4 of a ranged weapon.
///
/// `P4 = nominal DPS * mean hit factor at 12 and 25 cells * mean armor multiplier over the reference layers *
/// sqrt(range / 25)`.
///
/// Hand check (vector PI-1): the RG-1 gun with range 28, the RG-2 accuracy and AP 0.15. Hit factors are 0.7 and
/// 0.65, mean 0.675. Reference ratings 0, 0.55, 1.0 give survival 1, `1 - 0.75 * 0.4 = 0.7` and
/// `1 - 0.75 * 0.85 = 0.3625`, mean 0.6875. `P4 = 11.3924 * 0.675 * 0.6875 * sqrt(1.12) = 5.5950`.
///
/// # Errors
///
/// [`DesignError::EmptyInput`] for no reference layers, [`DesignError::InvalidInput`] for a non positive range
/// or any bad value, and the errors of [`nominal_dps`].
pub fn strength_p4(profile: &RangedProfile, reference: &ReferenceArmor<'_>) -> DesignResult<f64> {
    if reference.sharp_ratings.is_empty() {
        return Err(DesignError::EmptyInput {
            what: "reference armor layers",
        });
    }
    let range = positive("range", profile.range)?;
    let ap = non_negative("armor_penetration", profile.armor_penetration)?;
    let nominal = nominal_dps(profile)?;
    let mid_hit = (profile_hit_factor(profile, 12.0)? + profile_hit_factor(profile, 25.0)?) / 2.0;
    let mut armor_sum = 0.0;
    for rating in reference.sharp_ratings {
        armor_sum += layer_survival(*rating, ap)?;
    }
    // Lossless for any realistic list length.
    let armor_mean = armor_sum / reference.sharp_ratings.len() as f64;
    Ok(nominal * mid_hit * armor_mean * (range / 25.0).sqrt())
}

/// Inputs of [`sustained_dps`], a RimStudio display metric for magazine weapons (not a game formula).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SustainedInput {
    /// Damage per shot.
    pub damage: f64,
    /// Rounds in a magazine, at least 1.
    pub magazine: u32,
    /// Shots per burst, at least 1.
    pub burst_count: u32,
    /// Ticks between burst shots.
    pub ticks_between_shots: f64,
    /// Warmup in seconds, paid once per burst.
    pub warmup: f64,
    /// Cooldown in seconds, paid after each burst.
    pub cooldown: f64,
    /// Reload time in seconds.
    pub reload: f64,
}

/// Sustained DPS over a whole magazine including the reload:
/// `damage * magazine / (magazine_time + reload)` with
/// `magazine_time = bursts * (warmup + cooldown) + (magazine - bursts) * ticks / 60` and
/// `bursts = ceil(magazine / burst)`.
///
/// Hand check (vector CE-4): damage 12, magazine 30, burst 6, 5 ticks, warmup 1.0, cooldown 0.4, reload 4.0:
/// bursts 5, magazine time `5 * 1.4 + 25 * 5 / 60 = 9.0833`, DPS `360 / 13.0833 = 27.52`.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a bad number or a zero magazine or burst, [`DesignError::Degenerate`]
/// when the total time is zero.
pub fn sustained_dps(input: &SustainedInput) -> DesignResult<f64> {
    let damage = non_negative("damage", input.damage)?;
    let ticks = non_negative("ticks_between_shots", input.ticks_between_shots)?;
    let warmup = non_negative("warmup", input.warmup)?;
    let cooldown = non_negative("cooldown", input.cooldown)?;
    let reload = non_negative("reload", input.reload)?;
    if input.magazine == 0 {
        return Err(DesignError::invalid("magazine", "must be at least 1"));
    }
    if input.burst_count == 0 {
        return Err(DesignError::invalid("burst_count", "must be at least 1"));
    }
    let bursts = input.magazine.div_ceil(input.burst_count);
    let in_burst_gaps = f64::from(input.magazine - bursts.min(input.magazine));
    let magazine_time =
        f64::from(bursts) * (warmup + cooldown) + in_burst_gaps * ticks / TICKS_PER_SECOND;
    let total = magazine_time + reload;
    if !total.is_finite() {
        return Err(DesignError::invalid("time", "overflowed"));
    }
    if total <= 0.0 {
        return Err(DesignError::Degenerate {
            what: "the magazine cycle takes no time",
        });
    }
    Ok(damage * f64::from(input.magazine) / total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    fn rg2() -> AccuracyBands {
        AccuracyBands {
            touch: 0.6,
            short: 0.7,
            medium: 0.65,
            long: 0.55,
        }
    }

    fn rg1(range: f64) -> RangedProfile {
        RangedProfile {
            damage: 10.0,
            armor_penetration: 0.15,
            warmup: 0.8,
            cooldown: 1.5,
            burst_count: 3,
            ticks_between_shots: 10.0,
            range,
            accuracy: Some(rg2()),
        }
    }

    fn rg3(range: f64) -> RangedProfile {
        RangedProfile {
            damage: 20.0,
            armor_penetration: 0.30,
            warmup: 2.0,
            cooldown: 1.2,
            burst_count: 1,
            ticks_between_shots: 15.0,
            range,
            accuracy: Some(rg2()),
        }
    }

    #[rstest]
    // RG-1: 0.8 + 1.5 + 2 * 10 / 60.
    #[case(0.8, 1.5, 3, 10.0, 2.633_333_333_333)]
    // RG-3: a single shot pays no gaps even with 15 ticks configured.
    #[case(2.0, 1.2, 1, 15.0, 3.2)]
    // Two shots, 30 ticks apart: half a second more.
    #[case(1.0, 1.0, 2, 30.0, 2.5)]
    fn cycle_time_cases(
        #[case] warmup: f64,
        #[case] cooldown: f64,
        #[case] burst: u32,
        #[case] ticks: f64,
        #[case] expected: f64,
    ) {
        let got = cycle_time(&CycleInput {
            warmup,
            cooldown,
            burst_count: burst,
            ticks_between_shots: ticks,
        })
        .unwrap();
        assert!(close(got, expected, 1e-9), "got {got}");
    }

    #[test]
    fn cycle_time_rejects_zero_burst_and_zero_cycle() {
        let mut input = CycleInput {
            warmup: 1.0,
            cooldown: 1.0,
            burst_count: 0,
            ticks_between_shots: 5.0,
        };
        assert_eq!(
            cycle_time(&input).unwrap_err().code(),
            "design.invalid-input"
        );
        input.burst_count = 1;
        input.warmup = 0.0;
        input.cooldown = 0.0;
        assert_eq!(cycle_time(&input).unwrap_err().code(), "design.degenerate");
    }

    #[test]
    fn nominal_dps_matches_vectors() {
        assert!(close(nominal_dps(&rg1(28.0)).unwrap(), 11.3924, 1e-4));
        assert!(close(nominal_dps(&rg3(40.0)).unwrap(), 6.25, 1e-12));
    }

    #[rstest]
    // Bands clamp outside the outer anchors.
    #[case(0.0, 0.6)]
    #[case(3.0, 0.6)]
    // Halfway from touch to short: (3 + 12) / 2 = 7.5 gives 0.65.
    #[case(7.5, 0.65)]
    #[case(12.0, 0.7)]
    // RG-2: 18 cells.
    #[case(18.0, 0.676_923_076_923)]
    #[case(25.0, 0.65)]
    // Halfway from medium to long: 32.5 gives 0.6.
    #[case(32.5, 0.6)]
    #[case(40.0, 0.55)]
    #[case(100.0, 0.55)]
    fn hit_factor_cases(#[case] distance: f64, #[case] expected: f64) {
        let got = hit_factor(&rg2(), distance).unwrap();
        assert!(close(got, expected, 1e-9), "got {got}");
    }

    #[test]
    fn hit_factor_is_clamped_to_the_documented_range() {
        let wild = AccuracyBands {
            touch: 1.7,
            short: -0.4,
            medium: 0.0,
            long: 0.0,
        };
        assert_eq!(hit_factor(&wild, 0.0).unwrap(), 1.0);
        assert_eq!(hit_factor(&wild, 12.0).unwrap(), MIN_HIT_FACTOR);
    }

    #[test]
    fn hit_adjusted_dps_multiplies_the_hit_factor() {
        let got = hit_adjusted_dps(&rg1(28.0), 18.0).unwrap();
        assert!(close(got, 11.3924 * 0.676_923, 1e-3), "got {got}");
    }

    #[test]
    fn missing_accuracy_means_a_flat_factor_of_one() {
        let mut profile = rg1(28.0);
        profile.accuracy = None;
        assert_eq!(profile_hit_factor(&profile, 30.0).unwrap(), 1.0);
        assert_eq!(
            hit_adjusted_dps(&profile, 30.0).unwrap(),
            nominal_dps(&profile).unwrap()
        );
    }

    fn ap_input(damage: f64) -> ImpliedApInput {
        ImpliedApInput {
            damage,
            explicit_ap: None,
            sets_damage: true,
            damage_def_default_ap: None,
            ap_multiplier: 1.0,
        }
    }

    #[test]
    fn implied_ap_is_one_and_a_half_percent_of_damage() {
        // RG-1: 0.015 * 10 = 0.15.
        assert!(close(implied_ap(&ap_input(10.0)).unwrap(), 0.15, 1e-12));
    }

    #[test]
    fn explicit_ap_is_kept() {
        // RG-3: explicit 0.30 on a damage of 20 stays 0.30; explicit 0.35 on the same damage stays 0.35
        // (not 0.015 * 20 = 0.30); a shotgun style 0.14 on damage 18 stays 0.14.
        let mut input = ap_input(20.0);
        input.explicit_ap = Some(0.30);
        assert!(close(implied_ap(&input).unwrap(), 0.30, 1e-12));
        input.explicit_ap = Some(0.35);
        assert!(close(implied_ap(&input).unwrap(), 0.35, 1e-12));
        let mut shotgun = ap_input(18.0);
        shotgun.explicit_ap = Some(0.14);
        assert!(close(implied_ap(&shotgun).unwrap(), 0.14, 1e-12));
    }

    #[test]
    fn damage_def_default_is_not_inherited_when_the_projectile_sets_damage() {
        let mut input = ap_input(10.0);
        input.damage_def_default_ap = Some(0.5);
        assert!(close(implied_ap(&input).unwrap(), 0.15, 1e-12));
    }

    #[test]
    fn damage_def_default_applies_when_nothing_is_set() {
        let mut input = ap_input(10.0);
        input.sets_damage = false;
        input.damage_def_default_ap = Some(0.5);
        assert!(close(implied_ap(&input).unwrap(), 0.5, 1e-12));
    }

    #[test]
    fn negative_defaults_fall_back_to_the_damage_rule() {
        let mut input = ap_input(10.0);
        input.sets_damage = false;
        input.damage_def_default_ap = Some(-1.0);
        assert!(close(implied_ap(&input).unwrap(), 0.15, 1e-12));
        input.explicit_ap = Some(-1.0);
        assert!(close(implied_ap(&input).unwrap(), 0.15, 1e-12));
    }

    #[test]
    fn ap_multiplier_scales_the_result() {
        // Quality multiplier 1.5 on 0.15 gives 0.225.
        let mut input = ap_input(10.0);
        input.ap_multiplier = 1.5;
        assert!(close(implied_ap(&input).unwrap(), 0.225, 1e-12));
    }

    #[test]
    fn strength_p4_matches_vector_pi1_and_pi2() {
        let refs = ReferenceArmor {
            sharp_ratings: &[0.0, 0.55, 1.0],
        };
        // PI-1: 11.3924 * 0.675 * 0.6875 * sqrt(1.12) = 5.5950.
        let p1 = strength_p4(&rg1(28.0), &refs).unwrap();
        assert!(close(p1, 5.5950, 1e-3), "got {p1}");
        // PI-2: RG-3 with range 40, AP 0.30: hit mid (0.7 + 0.65) / 2 = 0.675 with the RG-2 accuracy.
        // The spec vector uses its own mid hit 0.775 (a different fictional accuracy), checked below.
        let accuracy = AccuracyBands {
            touch: 0.6,
            short: 0.8,
            medium: 0.75,
            long: 0.55,
        };
        let mut gun = rg3(40.0);
        gun.accuracy = Some(accuracy);
        gun.armor_penetration = 0.30;
        // armor mean: ratings 0, 0.55, 1.0 at AP 0.3: 1, 1 - 0.75 * 0.25 = 0.8125, 1 - 0.75 * 0.7 = 0.475
        // mean 0.762500; hit mid (0.8 + 0.75) / 2 = 0.775; P4 = 6.25 * 0.775 * 0.7625 * sqrt(1.6).
        let p2 = strength_p4(&gun, &refs).unwrap();
        assert!(close(p2, 4.6718, 1e-3), "got {p2}");
    }

    #[test]
    fn strength_p4_input_checks() {
        let refs = ReferenceArmor { sharp_ratings: &[] };
        assert_eq!(
            strength_p4(&rg1(28.0), &refs).unwrap_err().code(),
            "design.empty-input"
        );
        let refs = ReferenceArmor {
            sharp_ratings: &[0.0],
        };
        assert!(strength_p4(&rg1(0.0), &refs).is_err());
    }

    #[test]
    fn sustained_dps_matches_vector_ce4() {
        let got = sustained_dps(&SustainedInput {
            damage: 12.0,
            magazine: 30,
            burst_count: 6,
            ticks_between_shots: 5.0,
            warmup: 1.0,
            cooldown: 0.4,
            reload: 4.0,
        })
        .unwrap();
        assert!(close(got, 27.52, 5e-3), "got {got}");
    }

    #[test]
    fn sustained_dps_handles_a_partial_last_burst() {
        // Magazine 7, burst 3: bursts = 3, in-burst gaps = 7 - 3 = 4 at 6 ticks = 0.1 s each.
        // Magazine time 3 * (0.5 + 0.5) + 0.4 = 3.4; reload 0.6; DPS = 10 * 7 / 4.0 = 17.5.
        let got = sustained_dps(&SustainedInput {
            damage: 10.0,
            magazine: 7,
            burst_count: 3,
            ticks_between_shots: 6.0,
            warmup: 0.5,
            cooldown: 0.5,
            reload: 0.6,
        })
        .unwrap();
        assert!(close(got, 17.5, 1e-9), "got {got}");
    }

    #[rstest]
    #[case(f64::NAN)]
    #[case(f64::INFINITY)]
    #[case(-1.0)]
    fn nominal_dps_rejects_bad_numbers(#[case] bad: f64) {
        let mut profile = rg1(28.0);
        profile.damage = bad;
        assert!(nominal_dps(&profile).is_err());
        let mut profile = rg1(28.0);
        profile.warmup = bad;
        assert!(nominal_dps(&profile).is_err());
        assert!(hit_factor(&rg2(), bad).is_err());
    }

    fn arb_profile() -> impl Strategy<Value = RangedProfile> {
        (
            0.0f64..200.0,
            0.0f64..2.0,
            0.0f64..5.0,
            0.1f64..5.0,
            1u32..10,
            0.0f64..60.0,
            1.0f64..80.0,
        )
            .prop_map(|(damage, ap, warmup, cooldown, burst, ticks, range)| {
                RangedProfile {
                    damage,
                    armor_penetration: ap,
                    warmup,
                    cooldown,
                    burst_count: burst,
                    ticks_between_shots: ticks,
                    range,
                    accuracy: Some(rg2()),
                }
            })
    }

    proptest! {
        #[test]
        fn more_damage_never_lowers_dps(profile in arb_profile(), extra in 0.0f64..50.0) {
            let mut stronger = profile;
            stronger.damage += extra;
            prop_assert!(nominal_dps(&stronger).unwrap() >= nominal_dps(&profile).unwrap());
            prop_assert!(
                hit_adjusted_dps(&stronger, 20.0).unwrap() >= hit_adjusted_dps(&profile, 20.0).unwrap()
            );
        }

        #[test]
        fn a_longer_cooldown_never_raises_dps(profile in arb_profile(), extra in 0.0f64..5.0) {
            let mut slower = profile;
            slower.cooldown += extra;
            prop_assert!(nominal_dps(&slower).unwrap() <= nominal_dps(&profile).unwrap() + 1e-12);
        }

        #[test]
        fn hit_factor_stays_in_bounds(
            touch in 0.0f64..1.0, short in 0.0f64..1.0, medium in 0.0f64..1.0, long in 0.0f64..1.0,
            distance in 0.0f64..200.0,
        ) {
            let h = hit_factor(&AccuracyBands { touch, short, medium, long }, distance).unwrap();
            prop_assert!((MIN_HIT_FACTOR..=1.0).contains(&h));
        }

        #[test]
        fn more_penetration_never_lowers_p4(profile in arb_profile(), extra in 0.0f64..1.0) {
            let refs = ReferenceArmor { sharp_ratings: &[0.0, 0.55, 1.0] };
            let mut piercing = profile;
            piercing.armor_penetration += extra;
            prop_assert!(
                strength_p4(&piercing, &refs).unwrap() >= strength_p4(&profile, &refs).unwrap() - 1e-9
            );
        }

        #[test]
        fn non_finite_inputs_return_errors(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY)]) {
            let mut profile = rg1(28.0);
            profile.cooldown = bad;
            prop_assert!(nominal_dps(&profile).is_err());
            prop_assert!(hit_adjusted_dps(&profile, 10.0).is_err());
            let refs = ReferenceArmor { sharp_ratings: &[0.0] };
            prop_assert!(strength_p4(&profile, &refs).is_err());
            let mut input = ap_input(10.0);
            input.damage = bad;
            prop_assert!(implied_ap(&input).is_err());
        }
    }
}
