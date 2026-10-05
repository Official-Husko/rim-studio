//! Vanilla melee weapon math: the per (tool, capacity) attack list, the two DPS numbers the game uses, and the
//! strength index.
//!
//! Units: time in seconds, power and damage in hit points per swing, armor penetration as a fraction.
//!
//! The game shows two different "melee DPS" numbers and this module computes both:
//!
//! * [`stat_panel_dps`]: the number on the item stat panel, weighting each attack by `damage^2 * chance factor`.
//! * [`in_fight_dps`]: the number a fight produces, where a pawn prefers attacks whose score
//!   `damage * (1 + AP) / cooldown * chance factor` is near the best (Best 75 percent of swings, Mid 25
//!   percent, Worst none).
//!
//! The additional hediff term of the game's selection score is not modelled. Explicit tool penetration
//! scales with the weapon's damage multiplier; a tool without one gets `0.015 * adjusted damage`.

use crate::error::{DesignError, DesignResult, finite, non_negative, positive};
use serde::{Deserialize, Serialize};

/// Armor penetration the game assumes for a tool that sets none: this fraction of the adjusted damage.
pub const DEFAULT_AP_PER_DAMAGE: f64 = 0.015;

/// Attacks within this fraction of the best score count as Best.
pub const BEST_THRESHOLD: f64 = 0.95;

/// Attacks below this fraction of the best score count as Worst and are never chosen.
pub const WORST_THRESHOLD: f64 = 0.25;

/// Share of swings the Best attacks split between them.
pub const BEST_SHARE: f64 = 0.75;

/// Share of swings the Mid attacks split between them.
pub const MID_SHARE: f64 = 0.25;

/// Which armor stat and which stuff damage multiplier an attack uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DamageKind {
    /// Sharp damage (Cut, Stab): sharp stuff multiplier.
    Sharp,
    /// Blunt damage (Blunt, Poke, Demolish): blunt stuff multiplier.
    Blunt,
}

impl DamageKind {
    /// Damage kind of a vanilla tool capacity name: `Cut` and `Stab` are sharp, everything else is blunt.
    ///
    /// The install's maneuver definitions are the authority; this covers the capacities named in the design
    /// documents and falls back to blunt for unknown names, as the research tooling does.
    #[must_use]
    pub fn for_capacity(name: &str) -> Self {
        match name {
            "Cut" | "Stab" => Self::Sharp,
            _ => Self::Blunt,
        }
    }
}

/// One capacity of a tool (the game builds one attack per tool and capacity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeleeCapacity {
    /// Capacity name (for example the name of a tool capacity definition).
    pub name: String,
    /// Damage kind of this capacity.
    pub kind: DamageKind,
}

impl MeleeCapacity {
    /// A capacity whose kind is derived from its name with [`DamageKind::for_capacity`].
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: DamageKind::for_capacity(name),
        }
    }
}

/// A melee tool as written in the definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeleeTool {
    /// Label of the tool (handle, blade, head).
    pub label: String,
    /// Capacities of the tool; each yields one attack.
    pub capacities: Vec<MeleeCapacity>,
    /// Tool power (base damage).
    pub power: f64,
    /// Cooldown in seconds.
    pub cooldown: f64,
    /// Explicit armor penetration; `None` (or a negative value) means the 0.015 per damage default.
    pub armor_penetration: Option<f64>,
    /// Chance factor (1 when the tool sets none).
    pub chance_factor: f64,
}

/// Multipliers that turn tool values into attack values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeleeModifiers {
    /// The weapon's damage multiplier stat (quality).
    pub damage_multiplier: f64,
    /// Stuff multiplier for sharp attacks.
    pub sharp_multiplier: f64,
    /// Stuff multiplier for blunt attacks.
    pub blunt_multiplier: f64,
    /// Cooldown factor (the weapon's cooldown multiplier times the stuff's).
    pub cooldown_multiplier: f64,
}

impl Default for MeleeModifiers {
    /// All multipliers 1: a Normal quality weapon with no stuff.
    fn default() -> Self {
        Self {
            damage_multiplier: 1.0,
            sharp_multiplier: 1.0,
            blunt_multiplier: 1.0,
            cooldown_multiplier: 1.0,
        }
    }
}

/// One attack: a (tool, capacity) pair with final damage, cooldown and penetration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeleeAttack {
    /// Label of the tool.
    pub tool: String,
    /// Capacity name.
    pub capacity: String,
    /// Damage kind.
    pub kind: DamageKind,
    /// Damage per swing.
    pub damage: f64,
    /// Cooldown in seconds.
    pub cooldown: f64,
    /// Armor penetration as a fraction.
    pub armor_penetration: f64,
    /// Chance factor.
    pub chance_factor: f64,
}

/// Enumerates the attacks of a tool list, one per (tool, capacity) pair, in input order.
///
/// `damage = power * damage_multiplier * kind multiplier`; `cooldown = tool cooldown * cooldown_multiplier`;
/// `AP = explicit * damage_multiplier`, or `0.015 * damage` when the tool sets no non negative value.
///
/// Hand check (vector ML-1): a handle with power 9 and cooldown 2.0 and a blade with power 18, cooldown 2.4 and
/// two sharp capacities give attacks (9, 2.0, AP 0.135), (18, 2.4, AP 0.27) and (18, 2.4, AP 0.27).
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite or negative number or a non positive cooldown or
/// multiplier, and [`DesignError::EmptyInput`] when no attack results.
pub fn enumerate_attacks(
    tools: &[MeleeTool],
    modifiers: &MeleeModifiers,
) -> DesignResult<Vec<MeleeAttack>> {
    let damage_multiplier = non_negative("damage_multiplier", modifiers.damage_multiplier)?;
    let sharp = non_negative("sharp_multiplier", modifiers.sharp_multiplier)?;
    let blunt = non_negative("blunt_multiplier", modifiers.blunt_multiplier)?;
    let cooldown_multiplier = positive("cooldown_multiplier", modifiers.cooldown_multiplier)?;
    let mut attacks = Vec::new();
    for tool in tools {
        let power = non_negative("power", tool.power)?;
        let cooldown = positive("cooldown", tool.cooldown)?;
        let chance_factor = non_negative("chance_factor", tool.chance_factor)?;
        let explicit = match tool.armor_penetration {
            Some(v) => Some(finite("armor_penetration", v)?).filter(|v| *v >= 0.0),
            None => None,
        };
        for capacity in &tool.capacities {
            let kind_multiplier = match capacity.kind {
                DamageKind::Sharp => sharp,
                DamageKind::Blunt => blunt,
            };
            let damage = power * damage_multiplier * kind_multiplier;
            let armor_penetration = match explicit {
                Some(v) => v * damage_multiplier,
                None => damage * DEFAULT_AP_PER_DAMAGE,
            };
            attacks.push(MeleeAttack {
                tool: tool.label.clone(),
                capacity: capacity.name.clone(),
                kind: capacity.kind,
                damage,
                cooldown: cooldown * cooldown_multiplier,
                armor_penetration,
                chance_factor,
            });
        }
    }
    if attacks.is_empty() {
        return Err(DesignError::EmptyInput { what: "attacks" });
    }
    Ok(attacks)
}

/// Validates an attack list and returns it unchanged.
fn checked(attacks: &[MeleeAttack]) -> DesignResult<&[MeleeAttack]> {
    if attacks.is_empty() {
        return Err(DesignError::EmptyInput { what: "attacks" });
    }
    for attack in attacks {
        non_negative("damage", attack.damage)?;
        positive("cooldown", attack.cooldown)?;
        non_negative("armor_penetration", attack.armor_penetration)?;
        non_negative("chance_factor", attack.chance_factor)?;
    }
    Ok(attacks)
}

/// The stat panel numbers of a weapon.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatPanelDps {
    /// Weighted mean damage per swing.
    pub damage: f64,
    /// Weighted mean cooldown in seconds.
    pub cooldown: f64,
    /// `damage / cooldown`.
    pub dps: f64,
    /// Weighted mean armor penetration.
    pub armor_penetration: f64,
}

/// Stat panel DPS: weights `w = damage^2 * chance factor`, DPS = (mean damage by weight) / (mean cooldown by
/// weight). When every weight is zero (no damage), all outputs are 0.
///
/// Hand check (vector ML-1): attacks (9, 2.0) and two (18, 2.4). Weights 81, 324, 324 sum to 729. Mean damage
/// `(81 * 9 + 648 * 18) / 729 = 17.0`, mean cooldown `(81 * 2.0 + 648 * 2.4) / 729 = 2.3556`, DPS `7.2170`.
///
/// # Errors
///
/// See [`enumerate_attacks`] for the input checks; an empty list is [`DesignError::EmptyInput`].
pub fn stat_panel_dps(attacks: &[MeleeAttack]) -> DesignResult<StatPanelDps> {
    let attacks = checked(attacks)?;
    let mut total = 0.0;
    let (mut damage, mut cooldown, mut ap) = (0.0, 0.0, 0.0);
    for a in attacks {
        let w = a.damage * a.damage * a.chance_factor;
        total += w;
        damage += w * a.damage;
        cooldown += w * a.cooldown;
        ap += w * a.armor_penetration;
    }
    if total <= 0.0 || !total.is_finite() {
        return Ok(StatPanelDps {
            damage: 0.0,
            cooldown: 0.0,
            dps: 0.0,
            armor_penetration: 0.0,
        });
    }
    let (damage, cooldown, ap) = (damage / total, cooldown / total, ap / total);
    if !(damage.is_finite() && cooldown.is_finite() && ap.is_finite()) || cooldown <= 0.0 {
        return Err(DesignError::invalid("attacks", "values are too large"));
    }
    Ok(StatPanelDps {
        damage,
        cooldown,
        dps: damage / cooldown,
        armor_penetration: ap,
    })
}

/// How the in-fight selection treats an attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionCategory {
    /// Score at least 95 percent of the best.
    Best,
    /// Everything between Worst and Best.
    Mid,
    /// Score below 25 percent of the best; never chosen.
    Worst,
}

/// Selection result for one attack.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttackShare {
    /// Selection score `damage * (1 + AP) / cooldown * chance factor`.
    pub score: f64,
    /// Category from the score.
    pub category: SelectionCategory,
    /// Normalised share of swings in `[0, 1]`; the shares sum to 1.
    pub share: f64,
}

/// In-fight numbers of a weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InFightDps {
    /// One entry per input attack, in input order.
    pub attacks: Vec<AttackShare>,
    /// Expected damage per swing.
    pub swing_damage: f64,
    /// Expected cooldown per swing in seconds.
    pub swing_cooldown: f64,
    /// `swing_damage / swing_cooldown`.
    pub dps: f64,
    /// Expected armor penetration per swing.
    pub armor_penetration: f64,
}

/// In-fight DPS from the 75/25 attack selection model.
///
/// Best attacks share 0.75 of the swings equally, Mid attacks share 0.25 equally, Worst attacks get none.
/// When there is no Mid attack the Best attacks take all swings (the shares are normalised to sum to 1), and
/// likewise for no Best attack, which cannot happen because the top scorer is always Best.
///
/// Hand check (vector ML-1): scores are handle `9 * 1.135 / 2.0 = 5.1075` and blade `18 * 1.27 / 2.4 = 9.525`
/// (twice). The blades are Best (each 0.375), the handle is Mid (0.25). Swing damage
/// `0.75 * 18 + 0.25 * 9 = 15.75`, cooldown `0.75 * 2.4 + 0.25 * 2.0 = 2.3`, DPS `6.8478`, AP
/// `0.75 * 0.27 + 0.25 * 0.135 = 0.23625`.
///
/// # Errors
///
/// See [`enumerate_attacks`] for the input checks; an empty list is [`DesignError::EmptyInput`].
pub fn in_fight_dps(attacks: &[MeleeAttack]) -> DesignResult<InFightDps> {
    let attacks = checked(attacks)?;
    let scores: Vec<f64> = attacks
        .iter()
        .map(|a| a.damage * (1.0 + a.armor_penetration) / a.cooldown * a.chance_factor)
        .collect();
    let best_score = scores.iter().copied().fold(0.0_f64, f64::max);
    if !best_score.is_finite() {
        return Err(DesignError::invalid("attacks", "values are too large"));
    }
    let categories: Vec<SelectionCategory> = scores
        .iter()
        .map(|s| {
            if *s >= BEST_THRESHOLD * best_score {
                SelectionCategory::Best
            } else if *s < WORST_THRESHOLD * best_score {
                SelectionCategory::Worst
            } else {
                SelectionCategory::Mid
            }
        })
        .collect();
    let best_count = categories
        .iter()
        .filter(|c| **c == SelectionCategory::Best)
        .count();
    let mid_count = categories
        .iter()
        .filter(|c| **c == SelectionCategory::Mid)
        .count();
    // Counts are tiny, the conversion is lossless.
    let raw: Vec<f64> = categories
        .iter()
        .map(|c| match c {
            SelectionCategory::Best => BEST_SHARE / best_count as f64,
            SelectionCategory::Mid => MID_SHARE / mid_count as f64,
            SelectionCategory::Worst => 0.0,
        })
        .collect();
    let total: f64 = raw.iter().sum();
    if total <= 0.0 {
        return Err(DesignError::Degenerate {
            what: "no attack can be selected",
        });
    }
    let mut out = Vec::with_capacity(attacks.len());
    let (mut damage, mut cooldown, mut ap) = (0.0, 0.0, 0.0);
    for ((a, r), (score, category)) in attacks
        .iter()
        .zip(raw.iter())
        .zip(scores.iter().copied().zip(categories.iter().copied()))
    {
        let share = r / total;
        damage += share * a.damage;
        cooldown += share * a.cooldown;
        ap += share * a.armor_penetration;
        out.push(AttackShare {
            score,
            category,
            share,
        });
    }
    if cooldown <= 0.0 || !(damage.is_finite() && cooldown.is_finite() && ap.is_finite()) {
        return Err(DesignError::invalid("attacks", "values are too large"));
    }
    Ok(InFightDps {
        attacks: out,
        swing_damage: damage,
        swing_cooldown: cooldown,
        dps: damage / cooldown,
        armor_penetration: ap,
    })
}

/// Melee strength index `P_M = in-fight DPS * (1 + in-fight armor penetration)`.
///
/// For placing a weapon inside a band of real weapons, not for pricing.
///
/// Hand check (vector ML-1): `6.8478 * 1.23625 = 8.4655`.
///
/// # Errors
///
/// See [`in_fight_dps`].
pub fn strength(attacks: &[MeleeAttack]) -> DesignResult<f64> {
    let fight = in_fight_dps(attacks)?;
    Ok(fight.dps * (1.0 + fight.armor_penetration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    fn tool(label: &str, caps: &[&str], power: f64, cooldown: f64, ap: Option<f64>) -> MeleeTool {
        MeleeTool {
            label: label.to_owned(),
            capacities: caps.iter().map(|c| MeleeCapacity::named(c)).collect(),
            power,
            cooldown,
            armor_penetration: ap,
            chance_factor: 1.0,
        }
    }

    fn ml1() -> Vec<MeleeAttack> {
        let tools = [
            tool("handle", &["Blunt"], 9.0, 2.0, None),
            tool("blade", &["Cut", "Stab"], 18.0, 2.4, None),
        ];
        enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap()
    }

    fn ml2() -> Vec<MeleeAttack> {
        let tools = [
            tool("handle", &["Blunt"], 9.0, 2.0, None),
            tool("blade", &["Cut"], 12.0, 1.5, None),
            tool("point", &["Stab"], 13.0, 2.0, None),
        ];
        enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap()
    }

    #[rstest]
    #[case("Cut", DamageKind::Sharp)]
    #[case("Stab", DamageKind::Sharp)]
    #[case("Blunt", DamageKind::Blunt)]
    #[case("Poke", DamageKind::Blunt)]
    #[case("Demolish", DamageKind::Blunt)]
    #[case("RS_Unknown", DamageKind::Blunt)]
    fn capacity_kinds(#[case] name: &str, #[case] kind: DamageKind) {
        assert_eq!(DamageKind::for_capacity(name), kind);
    }

    #[test]
    fn enumeration_makes_one_attack_per_tool_and_capacity() {
        let attacks = ml1();
        assert_eq!(attacks.len(), 3);
        assert_eq!(attacks[0].tool, "handle");
        assert_eq!(attacks[1].capacity, "Cut");
        assert_eq!(attacks[2].capacity, "Stab");
        assert!(close(attacks[0].armor_penetration, 0.135, 1e-12));
        assert!(close(attacks[1].armor_penetration, 0.27, 1e-12));
    }

    #[test]
    fn modifiers_apply_by_damage_kind_and_scale_explicit_ap() {
        // Quality 1.5 on damage, sharp 2.0, blunt 0.5, cooldown 0.8.
        // Blade power 10 sharp: 10 * 1.5 * 2.0 = 30, cooldown 3.0 * 0.8 = 2.4, explicit AP 0.2 * 1.5 = 0.3.
        // Handle power 8 blunt: 8 * 1.5 * 0.5 = 6, implied AP 0.09.
        let tools = [
            tool("blade", &["Cut"], 10.0, 3.0, Some(0.2)),
            tool("handle", &["Blunt"], 8.0, 1.0, None),
        ];
        let mods = MeleeModifiers {
            damage_multiplier: 1.5,
            sharp_multiplier: 2.0,
            blunt_multiplier: 0.5,
            cooldown_multiplier: 0.8,
        };
        let attacks = enumerate_attacks(&tools, &mods).unwrap();
        assert!(close(attacks[0].damage, 30.0, 1e-12));
        assert!(close(attacks[0].cooldown, 2.4, 1e-12));
        assert!(close(attacks[0].armor_penetration, 0.3, 1e-12));
        assert!(close(attacks[1].damage, 6.0, 1e-12));
        assert!(close(attacks[1].armor_penetration, 0.09, 1e-12));
    }

    #[test]
    fn stat_panel_matches_ml1() {
        let s = stat_panel_dps(&ml1()).unwrap();
        assert!(close(s.damage, 17.0, 1e-9));
        assert!(close(s.cooldown, 2.3556, 1e-4));
        assert!(close(s.dps, 7.2170, 1e-4), "got {}", s.dps);
    }

    #[test]
    fn stat_panel_matches_ml2() {
        // Weights 81, 144, 169 sum 394. Damage (729 + 1728 + 2197) / 394 = 11.812;
        // cooldown (162 + 216 + 338) / 394 = 1.8173; DPS 6.5000.
        let s = stat_panel_dps(&ml2()).unwrap();
        assert!(close(s.damage, 11.812, 1e-3));
        assert!(close(s.cooldown, 1.8173, 1e-4));
        assert!(close(s.dps, 6.5, 1e-3), "got {}", s.dps);
    }

    #[test]
    fn in_fight_matches_ml1() {
        let f = in_fight_dps(&ml1()).unwrap();
        assert_eq!(f.attacks[0].category, SelectionCategory::Mid);
        assert_eq!(f.attacks[1].category, SelectionCategory::Best);
        assert_eq!(f.attacks[2].category, SelectionCategory::Best);
        assert!(close(f.attacks[0].share, 0.25, 1e-12));
        assert!(close(f.attacks[1].share, 0.375, 1e-12));
        assert!(close(f.swing_damage, 15.75, 1e-9));
        assert!(close(f.swing_cooldown, 2.3, 1e-9));
        assert!(close(f.dps, 6.8478, 1e-4));
        assert!(close(f.armor_penetration, 0.23625, 1e-9));
    }

    #[test]
    fn in_fight_matches_ml2() {
        // Scores: handle 9 * 1.135 / 2 = 5.1075, blade 12 * 1.18 / 1.5 = 9.44, point 13 * 1.195 / 2 = 7.7675.
        // Best threshold 0.95 * 9.44 = 8.968: only the blade is Best. Point and handle are Mid (0.125 each).
        // Damage 0.75 * 12 + 0.125 * 13 + 0.125 * 9 = 11.75, cooldown 0.75 * 1.5 + 0.25 * 2 = 1.625,
        // DPS 7.2308, AP 0.75 * 0.18 + 0.125 * 0.195 + 0.125 * 0.135 = 0.17625.
        let f = in_fight_dps(&ml2()).unwrap();
        assert_eq!(f.attacks[1].category, SelectionCategory::Best);
        assert_eq!(f.attacks[2].category, SelectionCategory::Mid);
        assert!(close(f.swing_damage, 11.75, 1e-9));
        assert!(close(f.swing_cooldown, 1.625, 1e-9));
        assert!(close(f.dps, 7.2308, 1e-4));
        assert!(close(f.armor_penetration, 0.17625, 1e-9));
    }

    #[test]
    fn worst_attacks_get_no_swings() {
        // Scores: blade 20 / 2 = 10 (AP 0 explicit), nibble 1 / 2 = 0.5 which is below 2.5 so Worst.
        let tools = [
            tool("blade", &["Cut"], 20.0, 2.0, Some(0.0)),
            tool("nibble", &["Blunt"], 1.0, 2.0, Some(0.0)),
        ];
        let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap();
        let f = in_fight_dps(&attacks).unwrap();
        assert_eq!(f.attacks[1].category, SelectionCategory::Worst);
        assert_eq!(f.attacks[1].share, 0.0);
        // Only Best remains, so it takes all swings: DPS 20 / 2 = 10.
        assert!(close(f.attacks[0].share, 1.0, 1e-12));
        assert!(close(f.dps, 10.0, 1e-12));
    }

    #[test]
    fn strength_is_dps_times_one_plus_ap() {
        // ML-1: 6.8478 * 1.23625 = 8.4655.
        assert!(close(strength(&ml1()).unwrap(), 8.4655, 1e-3));
    }

    #[test]
    fn a_single_attack_has_equal_stat_and_fight_dps() {
        let tools = [tool("club", &["Blunt"], 12.0, 3.0, None)];
        let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap();
        let s = stat_panel_dps(&attacks).unwrap();
        let f = in_fight_dps(&attacks).unwrap();
        assert!(close(s.dps, 4.0, 1e-12));
        assert!(close(f.dps, 4.0, 1e-12));
    }

    #[test]
    fn zero_damage_weapon_has_zero_stat_dps() {
        let tools = [tool("stick", &["Blunt"], 0.0, 2.0, None)];
        let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap();
        assert_eq!(stat_panel_dps(&attacks).unwrap().dps, 0.0);
        assert_eq!(in_fight_dps(&attacks).unwrap().dps, 0.0);
    }

    #[test]
    fn empty_and_invalid_inputs_are_errors() {
        assert_eq!(
            stat_panel_dps(&[]).unwrap_err().code(),
            "design.empty-input"
        );
        assert_eq!(in_fight_dps(&[]).unwrap_err().code(), "design.empty-input");
        assert_eq!(
            enumerate_attacks(&[], &MeleeModifiers::default())
                .unwrap_err()
                .code(),
            "design.empty-input"
        );
        let bad = [tool("x", &["Cut"], 5.0, 0.0, None)];
        assert_eq!(
            enumerate_attacks(&bad, &MeleeModifiers::default())
                .unwrap_err()
                .code(),
            "design.invalid-input"
        );
        let nan = [tool("x", &["Cut"], f64::NAN, 1.0, None)];
        assert!(enumerate_attacks(&nan, &MeleeModifiers::default()).is_err());
    }

    fn arb_tools() -> impl Strategy<Value = Vec<MeleeTool>> {
        proptest::collection::vec(
            (0.5f64..60.0, 0.3f64..6.0, 0.1f64..3.0, any::<bool>()),
            1..4,
        )
        .prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (power, cooldown, chance, sharp))| MeleeTool {
                    label: format!("t{i}"),
                    capacities: vec![MeleeCapacity::named(if sharp { "Cut" } else { "Blunt" })],
                    power,
                    cooldown,
                    armor_penetration: None,
                    chance_factor: chance,
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn shares_sum_to_one(tools in arb_tools()) {
            let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap();
            let f = in_fight_dps(&attacks).unwrap();
            let total: f64 = f.attacks.iter().map(|a| a.share).sum();
            prop_assert!((total - 1.0).abs() < 1e-9);
        }

        #[test]
        fn dps_lies_between_the_slowest_and_fastest_attack(tools in arb_tools()) {
            let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).unwrap();
            let rates: Vec<f64> = attacks.iter().map(|a| a.damage / a.cooldown).collect();
            let lo = rates.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = rates.iter().copied().fold(0.0, f64::max);
            // Both numbers are ratios of weighted means, so they are bounded by the extreme per attack rates.
            let s = stat_panel_dps(&attacks).unwrap().dps;
            let f = in_fight_dps(&attacks).unwrap().dps;
            prop_assert!(s >= lo - 1e-9 && s <= hi + 1e-9);
            prop_assert!(f >= lo - 1e-9 && f <= hi + 1e-9);
        }

        #[test]
        fn more_damage_never_lowers_either_dps_of_a_single_tool(
            power in 0.5f64..60.0, extra in 0.0f64..30.0, cooldown in 0.3f64..6.0,
        ) {
            let weak = [tool("t", &["Cut"], power, cooldown, None)];
            let strong = [tool("t", &["Cut"], power + extra, cooldown, None)];
            let m = MeleeModifiers::default();
            let w = enumerate_attacks(&weak, &m).unwrap();
            let s = enumerate_attacks(&strong, &m).unwrap();
            prop_assert!(stat_panel_dps(&s).unwrap().dps >= stat_panel_dps(&w).unwrap().dps);
            prop_assert!(in_fight_dps(&s).unwrap().dps >= in_fight_dps(&w).unwrap().dps);
            prop_assert!(strength(&s).unwrap() >= strength(&w).unwrap());
        }

        #[test]
        fn non_finite_values_are_errors(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY)]) {
            let tools = [tool("t", &["Cut"], bad, 1.0, None)];
            prop_assert!(enumerate_attacks(&tools, &MeleeModifiers::default()).is_err());
            let attack = MeleeAttack {
                tool: "t".into(), capacity: "Cut".into(), kind: DamageKind::Sharp,
                damage: 5.0, cooldown: bad, armor_penetration: 0.0, chance_factor: 1.0,
            };
            prop_assert!(stat_panel_dps(std::slice::from_ref(&attack)).is_err());
            prop_assert!(in_fight_dps(std::slice::from_ref(&attack)).is_err());
        }
    }
}
