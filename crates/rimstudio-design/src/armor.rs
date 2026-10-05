//! Armor mitigation: vanilla layer survival and stacks, the armor power index, and the Combat Extended
//! meets-armor preview.
//!
//! Units: vanilla ratings and penetration are fractions (0.5 means 50 percent; ratings are clamped to
//! [`MAX_RATING`], which is 200 percent). Combat Extended sharp ratings and penetration are millimetres of
//! rolled homogeneous armor, blunt ones are MPa. Nothing here reads a data table: every rating, coverage and
//! penetration is a parameter, so the caller reads them from the user's install.
//!
//! # Vanilla rule
//!
//! For one layer with rating `r` against penetration `p`, the effective rating is `e = max(r - p, 0)`.
//! A uniform draw below `e / 2` removes the hit, a draw between `e / 2` and `e` halves it, anything else leaves
//! it. The expected surviving damage fraction is therefore
//!
//! * `1 - 0.75 e` for `e <= 1`,
//! * `0.5 - e / 4` for `1 <= e <= 2`,
//! * `0` at `e = 2`.
//!
//! Layers are independent in expectation, so a stack multiplies the per layer fractions.

use crate::error::{DesignError, DesignResult, finite, non_negative};
use serde::{Deserialize, Serialize};

/// Highest armor rating the game accepts (200 percent); larger ratings are clamped to it.
pub const MAX_RATING: f64 = 2.0;

/// Expected surviving damage fraction after one armor layer.
///
/// `rating` is clamped to `[0, MAX_RATING]`; `penetration` must not be negative. The result is in `[0, 1]`
/// and never increases when `penetration` grows.
///
/// Hand check: rating 0.5, penetration 0 gives `e = 0.5` and `1 - 0.75 * 0.5 = 0.625`; rating 1.5 gives
/// `e = 1.5` and `0.5 - 1.5 / 4 = 0.125`.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when an input is not finite or is negative.
pub fn layer_survival(rating: f64, penetration: f64) -> DesignResult<f64> {
    let rating = non_negative("rating", rating)?.min(MAX_RATING);
    let penetration = non_negative("penetration", penetration)?;
    Ok(survival_for_effective((rating - penetration).max(0.0)))
}

/// Closed form of the survival fraction for an effective rating already known to be in `[0, 2]`.
fn survival_for_effective(e: f64) -> f64 {
    let value = if e <= 1.0 {
        1.0 - 0.75 * e
    } else {
        0.5 - e / 4.0
    };
    value.clamp(0.0, 1.0)
}

/// One armor layer worn over a body region, with the share of hits that reach it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorLayer {
    /// Armor rating as a fraction (0.5 is 50 percent).
    pub rating: f64,
    /// Probability in `[0, 1]` that a random hit is checked against this layer (the body coverage of the
    /// item). A layer with coverage 1 always takes part.
    pub coverage: f64,
}

impl ArmorLayer {
    /// A layer that takes part in every hit.
    #[must_use]
    pub fn full(rating: f64) -> Self {
        Self {
            rating,
            coverage: 1.0,
        }
    }
}

/// Expected surviving damage fraction through a stack of layers for a hit with the given penetration.
///
/// Each layer takes part with probability `coverage` independently of the others, so the factor of a layer is
/// `1 - c + c * layer_survival`. This is exact when all layers cover the same body parts (coverage 1) and an
/// independence approximation otherwise; use [`survival_over_parts`] when per part ratings are known.
/// An empty stack returns 1.
///
/// Hand check (vector AR-3): layers 0.4 and 0.9 at penetration 0.10: `e = 0.3` gives `0.775`, `e = 0.8`
/// gives `0.4`, product `0.31`.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when a rating, coverage or the penetration is not finite, is negative, or
/// when a coverage is above 1.
pub fn stack_survival(layers: &[ArmorLayer], penetration: f64) -> DesignResult<f64> {
    let mut product = 1.0;
    for layer in layers {
        let coverage = non_negative("coverage", layer.coverage)?;
        if coverage > 1.0 {
            return Err(DesignError::invalid("coverage", "must not be above 1"));
        }
        let survival = layer_survival(layer.rating, penetration)?;
        product *= 1.0 - coverage + coverage * survival;
    }
    Ok(product)
}

/// Like [`stack_survival`] for layers that all take part (coverage 1), given only their ratings.
///
/// Hand check (vector AR-2): ratings 1.0 and 1.0 at penetration 0: `0.25 * 0.25 = 0.0625`.
///
/// # Errors
///
/// See [`layer_survival`].
pub fn stack_survival_ratings(ratings: &[f64], penetration: f64) -> DesignResult<f64> {
    let mut product = 1.0;
    for rating in ratings {
        product *= layer_survival(*rating, penetration)?;
    }
    Ok(product)
}

/// The layers that protect one body part, with the chance weight of that part being hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartStack {
    /// Relative probability that this part is hit (for example the part's absolute coverage).
    pub weight: f64,
    /// Ratings of the layers covering the part; empty for an unprotected part.
    pub ratings: Vec<f64>,
}

/// Expected surviving damage fraction for a hit on a random body part.
///
/// The result is the weighted mean over `parts` of the per part stack survival (weights are normalised, so
/// they need not sum to 1). This is the exact version of [`stack_survival`] for a worn outfit.
///
/// # Errors
///
/// [`DesignError::EmptyInput`] for no parts, [`DesignError::Degenerate`] when the weights sum to zero, and
/// [`DesignError::InvalidInput`] for a bad weight, rating or penetration.
pub fn survival_over_parts(parts: &[PartStack], penetration: f64) -> DesignResult<f64> {
    if parts.is_empty() {
        return Err(DesignError::EmptyInput { what: "body parts" });
    }
    let mut total_weight = 0.0;
    let mut total = 0.0;
    for part in parts {
        let weight = non_negative("weight", part.weight)?;
        total_weight += weight;
        total += weight * stack_survival_ratings(&part.ratings, penetration)?;
    }
    if total_weight <= 0.0 {
        return Err(DesignError::Degenerate {
            what: "body part weights sum to zero",
        });
    }
    Ok(total / total_weight)
}

/// Armor ratings of one apparel item for the power index (fractions).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorRatings {
    /// Sharp rating.
    pub sharp: f64,
    /// Blunt rating.
    pub blunt: f64,
    /// Heat rating.
    pub heat: f64,
    /// Body coverage in `[0, 1]`.
    pub coverage: f64,
}

/// Weights of the armor power index. They are an assumption of the design documents and replaceable data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiWeights {
    /// Weight of the sharp component.
    pub sharp: f64,
    /// Weight of the blunt component.
    pub blunt: f64,
    /// Weight of the heat component.
    pub heat: f64,
    /// Penetration values over which the sharp component is averaged.
    pub sharp_penetrations: Vec<f64>,
}

/// Armor power index: `100 * coverage * (w_s * S + w_b * B + w_h * H)`.
///
/// `S` is the mean prevented fraction (`1 - layer_survival`) of the sharp rating over
/// `weights.sharp_penetrations`; `B` and `H` are the prevented fractions of the blunt and heat ratings at
/// penetration 0.
///
/// Hand check (vector AP-3): sharp 0.6, blunt 0.3, heat 0.4, coverage 0.442, weights 0.6 / 0.25 / 0.15 and
/// penetrations 0, 0.15, 0.30. `S = (0.45 + 0.3375 + 0.225) / 3 = 0.3375`, `B = 0.225`, `H = 0.30`,
/// `P = 0.2025 + 0.05625 + 0.045 = 0.30375`, index `100 * 0.442 * 0.30375 = 13.4257`.
///
/// # Errors
///
/// [`DesignError::EmptyInput`] when `sharp_penetrations` is empty, [`DesignError::InvalidInput`] for bad
/// numbers or a coverage above 1.
pub fn armor_power_index(ratings: &ArmorRatings, weights: &ApiWeights) -> DesignResult<f64> {
    if weights.sharp_penetrations.is_empty() {
        return Err(DesignError::EmptyInput {
            what: "sharp penetrations",
        });
    }
    let coverage = non_negative("coverage", ratings.coverage)?;
    if coverage > 1.0 {
        return Err(DesignError::invalid("coverage", "must not be above 1"));
    }
    let mut prevented_sharp = 0.0;
    for penetration in &weights.sharp_penetrations {
        prevented_sharp += 1.0 - layer_survival(ratings.sharp, *penetration)?;
    }
    // Lossless for any realistic list length.
    let count = weights.sharp_penetrations.len() as f64;
    let s = prevented_sharp / count;
    let b = 1.0 - layer_survival(ratings.blunt, 0.0)?;
    let h = 1.0 - layer_survival(ratings.heat, 0.0)?;
    let p = finite("weights.sharp", weights.sharp)? * s
        + finite("weights.blunt", weights.blunt)? * b
        + finite("weights.heat", weights.heat)? * h;
    Ok(100.0 * coverage * p)
}

/// Damage class of an attack in the Combat Extended preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CeDamageKind {
    /// Sharp damage, checked against the layers' sharp rating (mm RHA).
    Sharp,
    /// Blunt damage, checked against the layers' blunt rating (MPa).
    Blunt,
}

/// A round (or melee hit) entering the Combat Extended armor walk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CeRound {
    /// Damage per hit before armor.
    pub damage: f64,
    /// Damage class at the start.
    pub kind: CeDamageKind,
    /// Sharp penetration in mm RHA (ignored for a round that starts blunt).
    pub sharp_penetration: f64,
    /// Blunt penetration in MPa, used once the attack is blunt.
    pub blunt_penetration: f64,
}

/// One worn armor layer in the Combat Extended preview, listed outermost first.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CeLayer {
    /// Sharp rating in mm RHA.
    pub sharp: f64,
    /// Blunt rating in MPa.
    pub blunt: f64,
}

/// Result of [`meets_armor`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CeOutcome {
    /// Health damage that reaches the body part after all layers.
    pub damage: f64,
    /// Whether that damage is sharp or blunt.
    pub kind: CeDamageKind,
    /// Index (outermost is 0) of the layer that deflected the sharp attack into blunt damage, if any.
    pub deflected_at: Option<usize>,
    /// Sharp penetration left in mm RHA (0 once deflected or when the round started blunt).
    pub remaining_sharp_penetration: f64,
    /// Blunt penetration left in MPa (the initial value while the attack is still sharp).
    pub remaining_blunt_penetration: f64,
}

/// Preview of the damage that survives a stack of Combat Extended armor layers.
///
/// While the attack is sharp, a layer whose sharp rating `rs` exceeds the remaining sharp penetration `p`
/// deflects it: the attack turns blunt at that layer. Otherwise the damage is multiplied by `(p - rs) / p` and
/// `p` becomes `p - rs`. On deflection the blunt penetration becomes `pb * (p / ps)` (the share of sharp
/// penetration that was left) and the blunt damage becomes
/// `cbrt(blunt_penetration * 10000) / 10 * (current damage / original damage)`; the blunt rule then applies at
/// the same layer. Blunt rule: a layer with `rb >= pb` stops the attack (damage 0), otherwise damage is
/// multiplied by `(pb - rb) / pb` and `pb` becomes `pb - rb`. Body part density, armor damage and partial
/// armor are not modelled.
///
/// Hand checks with a round of damage 10, 5 mm and 20 MPa:
///
/// * against one layer 4 mm / 6 MPa: `10 * (5 - 4) / 5 = 2.0` sharp;
/// * against one layer 6 mm / 6 MPa: deflected, blunt damage `cbrt(200000) / 10 = 5.848`, then
///   `5.848 * (20 - 6) / 20 = 4.094` blunt.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite or negative number, or a round with no damage share to
/// compute (original damage is allowed to be zero and yields zero damage).
pub fn meets_armor(round: &CeRound, layers: &[CeLayer]) -> DesignResult<CeOutcome> {
    let original = non_negative("damage", round.damage)?;
    let start_sharp = non_negative("sharp_penetration", round.sharp_penetration)?;
    let start_blunt = non_negative("blunt_penetration", round.blunt_penetration)?;
    for layer in layers {
        non_negative("layer.sharp", layer.sharp)?;
        non_negative("layer.blunt", layer.blunt)?;
    }

    let mut damage = original;
    let mut kind = round.kind;
    let mut p = if kind == CeDamageKind::Sharp {
        start_sharp
    } else {
        0.0
    };
    let mut pb = start_blunt;
    let mut deflected_at = None;

    for (index, layer) in layers.iter().enumerate() {
        if kind == CeDamageKind::Sharp {
            if layer.sharp > p || p <= 0.0 {
                // Deflected: the attack continues as blunt damage.
                let kept = if start_sharp > 0.0 {
                    p / start_sharp
                } else {
                    0.0
                };
                pb = start_blunt * kept.clamp(0.0, 1.0);
                let share = if original > 0.0 {
                    damage / original
                } else {
                    0.0
                };
                damage = (pb * 10_000.0).cbrt() / 10.0 * share;
                kind = CeDamageKind::Blunt;
                deflected_at = Some(index);
                p = 0.0;
            } else {
                damage *= (p - layer.sharp) / p;
                p -= layer.sharp;
                continue;
            }
        }
        // Blunt rule (also applied at the layer that deflected).
        if layer.blunt >= pb {
            damage = 0.0;
            pb = 0.0;
        } else {
            damage *= (pb - layer.blunt) / pb;
            pb -= layer.blunt;
        }
    }

    Ok(CeOutcome {
        damage: damage.max(0.0),
        kind,
        deflected_at,
        remaining_sharp_penetration: p,
        remaining_blunt_penetration: pb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[rstest]
    // AR-1: e = 0.5, 1 - 0.375.
    #[case(0.5, 0.0, 0.625)]
    // e = 1 exactly: both branches give 0.25.
    #[case(1.0, 0.0, 0.25)]
    // AR-4: e = 1.5, 0.5 - 0.375.
    #[case(1.5, 0.0, 0.125)]
    // A rating of 2 deflects everything.
    #[case(2.0, 0.0, 0.0)]
    // Ratings above 2 are clamped to 2.
    #[case(3.5, 0.0, 0.0)]
    // Penetration above the rating: nothing is absorbed.
    #[case(0.3, 0.5, 1.0)]
    // Rating 1.2 at penetration 0.3: e = 0.9 stays on the cheap slope, 1 - 0.675.
    #[case(1.2, 0.3, 0.325)]
    fn layer_survival_cases(#[case] rating: f64, #[case] ap: f64, #[case] expected: f64) {
        let got = layer_survival(rating, ap).unwrap();
        assert!(
            close(got, expected, 1e-12),
            "got {got}, expected {expected}"
        );
    }

    #[rstest]
    // AR-2: 0.25 * 0.25.
    #[case(&[1.0, 1.0], 0.0, 0.0625)]
    // AR-3: 0.775 * 0.4.
    #[case(&[0.4, 0.9], 0.10, 0.31)]
    // AR-5: 1.0 * 0.3625 * 0.8335 = 0.30214375.
    #[case(&[0.072, 1.0, 0.372], 0.15, 0.30214375)]
    // Two thin layers are weaker than one thick one: 0.625 * 0.625 = 0.390625.
    #[case(&[0.5, 0.5], 0.0, 0.390625)]
    // No layers: everything survives.
    #[case(&[], 0.2, 1.0)]
    fn stack_ratings_cases(#[case] ratings: &[f64], #[case] ap: f64, #[case] expected: f64) {
        let got = stack_survival_ratings(ratings, ap).unwrap();
        assert!(close(got, expected, 1e-9), "got {got}, expected {expected}");
    }

    #[test]
    fn stack_with_full_coverage_equals_plain_product() {
        let layers = [ArmorLayer::full(0.4), ArmorLayer::full(0.9)];
        let got = stack_survival(&layers, 0.10).unwrap();
        assert!(close(got, 0.31, 1e-12));
    }

    #[test]
    fn partial_coverage_mixes_in_the_unprotected_share() {
        // One layer 0.5 (survival 0.625) over half of the hits: 0.5 + 0.5 * 0.625 = 0.8125.
        let layers = [ArmorLayer {
            rating: 0.5,
            coverage: 0.5,
        }];
        let got = stack_survival(&layers, 0.0).unwrap();
        assert!(close(got, 0.8125, 1e-12));
    }

    #[test]
    fn coverage_above_one_is_rejected() {
        let layers = [ArmorLayer {
            rating: 0.5,
            coverage: 1.5,
        }];
        assert_eq!(
            stack_survival(&layers, 0.0).unwrap_err().code(),
            "design.invalid-input"
        );
    }

    #[test]
    fn survival_over_parts_is_a_weighted_mean() {
        // Part A weight 3 with a layer of 1.0 (0.25); part B weight 1 unprotected (1.0):
        // (3 * 0.25 + 1 * 1.0) / 4 = 0.4375.
        let parts = [
            PartStack {
                weight: 3.0,
                ratings: vec![1.0],
            },
            PartStack {
                weight: 1.0,
                ratings: vec![],
            },
        ];
        let got = survival_over_parts(&parts, 0.0).unwrap();
        assert!(close(got, 0.4375, 1e-12));
    }

    #[test]
    fn survival_over_parts_error_cases() {
        assert_eq!(
            survival_over_parts(&[], 0.0).unwrap_err().code(),
            "design.empty-input"
        );
        let zero = [PartStack {
            weight: 0.0,
            ratings: vec![],
        }];
        assert_eq!(
            survival_over_parts(&zero, 0.0).unwrap_err().code(),
            "design.degenerate"
        );
    }

    #[test]
    fn power_index_matches_hand_value() {
        let ratings = ArmorRatings {
            sharp: 0.6,
            blunt: 0.3,
            heat: 0.4,
            coverage: 0.442,
        };
        let weights = ApiWeights {
            sharp: 0.6,
            blunt: 0.25,
            heat: 0.15,
            sharp_penetrations: vec![0.0, 0.15, 0.30],
        };
        let got = armor_power_index(&ratings, &weights).unwrap();
        assert!(close(got, 13.4257, 1e-3), "got {got}");
    }

    #[test]
    fn power_index_needs_penetrations() {
        let ratings = ArmorRatings {
            sharp: 0.6,
            blunt: 0.3,
            heat: 0.4,
            coverage: 0.4,
        };
        let weights = ApiWeights {
            sharp: 1.0,
            blunt: 0.0,
            heat: 0.0,
            sharp_penetrations: vec![],
        };
        assert!(armor_power_index(&ratings, &weights).is_err());
    }

    fn round() -> CeRound {
        CeRound {
            damage: 10.0,
            kind: CeDamageKind::Sharp,
            sharp_penetration: 5.0,
            blunt_penetration: 20.0,
        }
    }

    #[test]
    fn ce_stays_sharp_against_a_thin_layer() {
        // CE-1: 10 * (5 - 4) / 5 = 2.0.
        let layers = [CeLayer {
            sharp: 4.0,
            blunt: 6.0,
        }];
        let out = meets_armor(&round(), &layers).unwrap();
        assert_eq!(out.kind, CeDamageKind::Sharp);
        assert!(close(out.damage, 2.0, 1e-12));
        assert_eq!(out.deflected_at, None);
        assert!(close(out.remaining_sharp_penetration, 1.0, 1e-12));
    }

    #[test]
    fn ce_deflection_turns_blunt_and_applies_the_blunt_rule() {
        // CE-2: cbrt(200000) / 10 = 5.8480; times (20 - 6) / 20 = 0.7 gives 4.0936.
        let layers = [CeLayer {
            sharp: 6.0,
            blunt: 6.0,
        }];
        let out = meets_armor(&round(), &layers).unwrap();
        assert_eq!(out.kind, CeDamageKind::Blunt);
        assert_eq!(out.deflected_at, Some(0));
        assert!(close(out.damage, 4.0936, 1e-3), "got {}", out.damage);
        assert!(close(out.remaining_blunt_penetration, 14.0, 1e-12));
    }

    #[test]
    fn ce_second_layer_deflects_and_blunt_penetration_is_scaled() {
        // CE-3: after layer 1 the damage is 2.0 and p = 1; layer 2 (8 mm) deflects; blunt penetration
        // 20 * (1 / 5) = 4 MPa is below 12 MPa, so the attack stops.
        let layers = [
            CeLayer {
                sharp: 4.0,
                blunt: 6.0,
            },
            CeLayer {
                sharp: 8.0,
                blunt: 12.0,
            },
        ];
        let out = meets_armor(&round(), &layers).unwrap();
        assert_eq!(out.kind, CeDamageKind::Blunt);
        assert_eq!(out.deflected_at, Some(1));
        assert_eq!(out.damage, 0.0);
    }

    #[test]
    fn ce_blunt_share_uses_current_over_original_damage() {
        // Layer 1 passes with damage 2.0 (share 0.2) and p = 1. Layer 2 (sharp 2, blunt 1) deflects:
        // blunt penetration 20 * 1 / 5 = 4, blunt damage cbrt(40000) / 10 * 0.2 = 3.4200 * 0.2 = 0.6840,
        // then * (4 - 1) / 4 = 0.5130.
        let layers = [
            CeLayer {
                sharp: 4.0,
                blunt: 0.0,
            },
            CeLayer {
                sharp: 2.0,
                blunt: 1.0,
            },
        ];
        let out = meets_armor(&round(), &layers).unwrap();
        assert!(close(out.damage, 0.5130, 1e-3), "got {}", out.damage);
    }

    #[test]
    fn ce_no_layers_returns_the_round_unchanged() {
        let out = meets_armor(&round(), &[]).unwrap();
        assert_eq!(out.kind, CeDamageKind::Sharp);
        assert_eq!(out.damage, 10.0);
    }

    #[test]
    fn ce_blunt_round_skips_the_sharp_stage() {
        // Blunt round 8 damage, 10 MPa against a layer 3 MPa: 8 * 7 / 10 = 5.6.
        let blunt = CeRound {
            damage: 8.0,
            kind: CeDamageKind::Blunt,
            sharp_penetration: 0.0,
            blunt_penetration: 10.0,
        };
        let layers = [CeLayer {
            sharp: 50.0,
            blunt: 3.0,
        }];
        let out = meets_armor(&blunt, &layers).unwrap();
        assert_eq!(out.kind, CeDamageKind::Blunt);
        assert_eq!(out.deflected_at, None);
        assert!(close(out.damage, 5.6, 1e-12));
    }

    #[test]
    fn ce_zero_sharp_penetration_never_divides_by_zero() {
        let weak = CeRound {
            damage: 5.0,
            kind: CeDamageKind::Sharp,
            sharp_penetration: 0.0,
            blunt_penetration: 4.0,
        };
        let layers = [CeLayer {
            sharp: 0.0,
            blunt: 1.0,
        }];
        let out = meets_armor(&weak, &layers).unwrap();
        assert!(out.damage.is_finite());
        assert_eq!(out.damage, 0.0);
    }

    #[test]
    fn ce_rejects_non_finite_input() {
        let mut bad = round();
        bad.damage = f64::NAN;
        assert!(meets_armor(&bad, &[]).is_err());
        let layers = [CeLayer {
            sharp: f64::INFINITY,
            blunt: 1.0,
        }];
        assert!(meets_armor(&round(), &layers).is_err());
    }

    #[rstest]
    #[case(f64::NAN, 0.1)]
    #[case(0.5, f64::NAN)]
    #[case(f64::INFINITY, 0.1)]
    #[case(0.5, f64::INFINITY)]
    #[case(-0.5, 0.1)]
    #[case(0.5, -0.1)]
    fn layer_survival_rejects_bad_input(#[case] rating: f64, #[case] ap: f64) {
        assert_eq!(
            layer_survival(rating, ap).unwrap_err().code(),
            "design.invalid-input"
        );
    }

    proptest! {
        #[test]
        fn survival_is_within_bounds(rating in 0.0f64..4.0, ap in 0.0f64..3.0) {
            let s = layer_survival(rating, ap).unwrap();
            prop_assert!((0.0..=1.0).contains(&s));
        }

        #[test]
        fn more_penetration_never_increases_survival(
            ratings in proptest::collection::vec(0.0f64..2.5, 0..5),
            ap in 0.0f64..2.0,
            extra in 0.0f64..2.0,
        ) {
            let low = stack_survival_ratings(&ratings, ap).unwrap();
            let high = stack_survival_ratings(&ratings, ap + extra).unwrap();
            prop_assert!(high >= low - 1e-12);
        }

        #[test]
        fn higher_rating_never_decreases_survival_loss(
            rating in 0.0f64..2.0,
            extra in 0.0f64..1.0,
            ap in 0.0f64..1.0,
        ) {
            let weak = layer_survival(rating, ap).unwrap();
            let strong = layer_survival(rating + extra, ap).unwrap();
            prop_assert!(strong <= weak + 1e-12);
        }

        #[test]
        fn adding_a_layer_never_increases_survival(
            ratings in proptest::collection::vec(0.0f64..2.0, 0..4),
            extra in 0.0f64..2.0,
            ap in 0.0f64..1.0,
        ) {
            let base = stack_survival_ratings(&ratings, ap).unwrap();
            let mut more = ratings.clone();
            more.push(extra);
            let stacked = stack_survival_ratings(&more, ap).unwrap();
            prop_assert!(stacked <= base + 1e-12);
        }

        #[test]
        fn non_finite_inputs_are_errors(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)]) {
            prop_assert!(layer_survival(bad, 0.0).is_err());
            prop_assert!(layer_survival(0.5, bad).is_err());
            prop_assert!(stack_survival_ratings(&[0.5, bad], 0.0).is_err());
        }

        #[test]
        fn ce_damage_is_finite_and_not_negative(
            damage in 0.0f64..100.0,
            ps in 0.0f64..50.0,
            pb in 0.0f64..100.0,
            layers in proptest::collection::vec((0.0f64..30.0, 0.0f64..60.0), 0..4),
        ) {
            let round = CeRound { damage, kind: CeDamageKind::Sharp, sharp_penetration: ps, blunt_penetration: pb };
            let layers: Vec<CeLayer> = layers.into_iter().map(|(sharp, blunt)| CeLayer { sharp, blunt }).collect();
            let out = meets_armor(&round, &layers).unwrap();
            prop_assert!(out.damage.is_finite());
            prop_assert!(out.damage >= 0.0);
        }

        #[test]
        fn ce_sharp_only_walk_never_gains_damage_with_more_layers(
            damage in 1.0f64..100.0,
            ps in 1.0f64..50.0,
            thin in proptest::collection::vec(0.0f64..0.9, 0..4),
        ) {
            // Layers thinner than the remaining penetration never deflect here (each below 0.9 and the
            // total is checked), so the result stays sharp and shrinks with each layer.
            let total: f64 = thin.iter().sum();
            prop_assume!(total < ps);
            let round = CeRound { damage, kind: CeDamageKind::Sharp, sharp_penetration: ps, blunt_penetration: 10.0 };
            let layers: Vec<CeLayer> = thin.iter().map(|s| CeLayer { sharp: *s, blunt: 1.0 }).collect();
            let out = meets_armor(&round, &layers).unwrap();
            prop_assert_eq!(out.kind, CeDamageKind::Sharp);
            prop_assert!(out.damage <= damage + 1e-9);
        }
    }
}
