//! The archetype choice of a draft: what the user said about the weapon, kept so the numbers can be
//! proposed again when the balance target changes.
//!
//! The choice is data about intent, never a number of the weapon. A draft without it loads and behaves
//! exactly as before (the field is optional and omitted when absent). The solver lives in
//! [`crate::archetype`].

use serde::{Deserialize, Serialize};

use super::spec::TechLevel;

/// How fast the weapon fires: a class of the ladder or a number of rounds per minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RateOfFire {
    /// A class id of the rate of fire ladder (`slow`, `medium`, `fast`).
    Class(String),
    /// Rounds per minute, read against the typical rounds per minute of the archetype.
    Rpm(f64),
}

/// The descriptors of a weapon beyond its archetype. Every one is optional; the archetype supplies the
/// default of the missing ones.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Descriptors {
    /// The action (`bolt`, `lever`, `pump`, `semi`, `burst`, `full-auto`, `draw`). Guns and bows only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The rate of fire (a melee weapon reads it as swing speed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rof: Option<RateOfFire>,
    /// The calibre class of the vanilla ladder (`tiny` to `huge`). Guns and bows only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibre: Option<String>,
    /// A Combat Extended ammo set def name: the real calibre, used in Combat Extended mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// The handling class (`compact`, `standard`, `heavy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handling: Option<String>,
    /// The tier. Absent means the tech level hint of the archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<TechLevel>,
}

/// Where the strength of the weapon should land among the install's weapons of its class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BalanceTarget {
    /// Weaker than most of its class (percentile 1/6).
    Weaker,
    /// In the middle of its class.
    #[default]
    Typical,
    /// Stronger than most of its class (percentile 5/6).
    Stronger,
    /// An explicit percentile of the class, from 0 to 1.
    Percentile(f64),
}

impl BalanceTarget {
    /// The percentile of the class strength distribution the target stands for.
    #[must_use]
    pub fn percentile(self) -> f64 {
        match self {
            Self::Weaker => 1.0 / 6.0,
            Self::Typical => 0.5,
            Self::Stronger => 5.0 / 6.0,
            Self::Percentile(p) => p,
        }
    }

    /// True when the percentile is a finite number inside the unit interval.
    #[must_use]
    pub fn is_valid(self) -> bool {
        let p = self.percentile();
        p.is_finite() && (0.0..=1.0).contains(&p)
    }
}

/// Whether the proposal also prepares the optional Combat Extended choices (the calibre as an ammo set).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchetypeMode {
    /// Vanilla numbers only. Needs no Combat Extended.
    #[default]
    Vanilla,
    /// Vanilla numbers plus a proposal for the Combat Extended block, taken from the user's ammo sets.
    CombatExtended,
}

/// What the user chose in the archetype dialog, stored in the draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeChoice {
    /// The archetype id, `family/archetype` (`rifle/assault`).
    pub archetype: String,
    /// The descriptors.
    #[serde(default)]
    pub descriptors: Descriptors,
    /// The balance target.
    #[serde(default)]
    pub balance: BalanceTarget,
    /// Vanilla or Combat Extended mode.
    #[serde(default)]
    pub mode: ArchetypeMode,
    /// A strength index to aim at exactly (for example the strength of a reference weapon). It replaces the
    /// balance target when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_round_trips_with_names_and_numbers() {
        let choice = ArchetypeChoice {
            archetype: "rifle/assault".into(),
            descriptors: Descriptors {
                action: Some("burst".into()),
                rof: Some(RateOfFire::Rpm(650.0)),
                calibre: Some("medium".into()),
                tier: Some(TechLevel::Industrial),
                ..Descriptors::default()
            },
            balance: BalanceTarget::Percentile(0.7),
            mode: ArchetypeMode::Vanilla,
            strength: None,
        };
        let text = serde_json::to_string(&choice).unwrap_or_default();
        assert!(text.contains("\"rpm\":650"), "{text}");
        assert!(text.contains("\"percentile\":0.7"), "{text}");
        let back: ArchetypeChoice = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, choice);
    }

    #[test]
    fn named_targets_map_to_the_simple_mode_percentiles() {
        assert!((BalanceTarget::Weaker.percentile() - 1.0 / 6.0).abs() < 1e-12);
        assert_eq!(BalanceTarget::Typical.percentile(), 0.5);
        assert!((BalanceTarget::Stronger.percentile() - 5.0 / 6.0).abs() < 1e-12);
        assert!(!BalanceTarget::Percentile(1.5).is_valid());
        assert!(!BalanceTarget::Percentile(f64::NAN).is_valid());
        let b: BalanceTarget = serde_json::from_str("\"stronger\"").unwrap_or_default();
        assert_eq!(b, BalanceTarget::Stronger);
    }
}
