//! Market value identity: price as an output of the cost list and the work to make.
//!
//! When a definition sets no `MarketValue`, the game computes the base value as the sum over the cost list of
//! `count * ingredient value`, plus (for stuffed items) `stuff count / volume * stuff value`, plus
//! `work * 0.0036` when the work exceeds 2. The quality factor and the hit point curve are applied afterwards
//! by the stat pipeline ([`crate::stats`]); the displayed number rounds to the nearest 5 above 200.
//!
//! Units: silver for values, work units for `WorkToMake`, counts as whole or fractional item counts. Nothing
//! here holds a recipe or a price table: every price is a parameter read from the user's install.

use crate::error::{DesignError, DesignResult, non_negative, positive};
use serde::{Deserialize, Serialize};

/// Silver per unit of work in the computed market value.
pub const VALUE_PER_WORK: f64 = 0.0036;

/// Work at or below this value adds nothing to the price.
pub const WORK_FREE_THRESHOLD: f64 = 2.0;

/// Price per stuff unit the game assumes when the stuff is not known yet.
pub const UNKNOWN_STUFF_PRICE: f64 = 2.0;

/// Prices above this are displayed rounded to the nearest 5.
pub const ROUND_TO_FIVE_OVER: f64 = 200.0;

/// One line of the cost list.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IngredientLine {
    /// Number of items.
    pub count: f64,
    /// Base market value of one item in silver.
    pub unit_value: f64,
}

/// The stuff part of a stuffed item's cost.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StuffCost {
    /// A chosen material.
    Known {
        /// Stuff count of the item (the `costStuffCount`).
        count: f64,
        /// Stuff units per item: 1, or 0.1 for small-volume stuffs such as silver and gold.
        volume_per_unit: f64,
        /// Market value of one stuff item in silver.
        unit_value: f64,
    },
    /// No material chosen yet: the count is priced at [`UNKNOWN_STUFF_PRICE`] per unit.
    Unknown {
        /// Stuff count of the item.
        count: f64,
    },
}

/// Inputs of [`market_value`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceInput {
    /// The fixed ingredients of the cost list.
    pub ingredients: Vec<IngredientLine>,
    /// The stuff cost, for stuffed items.
    pub stuff: Option<StuffCost>,
    /// `WorkToMake` of the item (the base stat).
    pub work_to_make: f64,
    /// The stuff's `WorkToMake` stat factor (1 without stuff or when the stuff has none).
    pub work_factor: f64,
}

/// Parts of a computed market value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PriceBreakdown {
    /// Sum of the fixed ingredients.
    pub ingredients: f64,
    /// Value of the stuff.
    pub stuff: f64,
    /// Value of the work (0 when the work is at or below the free threshold).
    pub work: f64,
    /// The sum of the three parts, before quality and hit points.
    pub base_value: f64,
    /// The base value rounded to the nearest 5 when it exceeds 200, as the game displays it.
    pub displayed: f64,
}

/// Rounds a price the way the game displays it: to the nearest 5 (ties to even) when above 200.
///
/// Hand check: 292.0 gives `292 / 5 = 58.4`, rounded 58, shown as 290; 411.52 shown as 410; 150.0 stays 150.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite value.
pub fn display_round(value: f64) -> DesignResult<f64> {
    crate::error::finite("value", value)?;
    if value.abs() > ROUND_TO_FIVE_OVER {
        Ok((value / 5.0).round_ties_even() * 5.0)
    } else {
        Ok(value)
    }
}

/// Computes the market value of an item without its own `MarketValue`.
///
/// `base = sum(count * unit value) + stuff + work`, where `stuff = count / volume * unit value` and
/// `work = work_to_make * work_factor * 0.0036` when `work_to_make * work_factor` exceeds 2.
///
/// Hand checks:
///
/// * MV-1: 2 of an ingredient at 30 gives 60; 80 stuff at volume 1 and price 2.0 gives 160; work 20,000 gives
///   `72`; total 292.0, displayed 290.
/// * MV-2: stuff count 40 at volume 0.1 and price 1.0 gives `40 / 0.1 * 1.0 = 400`; work 3,200 gives 11.52;
///   total 411.52, displayed 410.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for a non finite or negative number or a non positive volume.
pub fn market_value(input: &PriceInput) -> DesignResult<PriceBreakdown> {
    let mut ingredients = 0.0;
    for line in &input.ingredients {
        ingredients += non_negative("ingredient.count", line.count)?
            * non_negative("ingredient.unit_value", line.unit_value)?;
    }
    let stuff = match input.stuff {
        None => 0.0,
        Some(StuffCost::Unknown { count }) => {
            non_negative("stuff.count", count)? * UNKNOWN_STUFF_PRICE
        }
        Some(StuffCost::Known {
            count,
            volume_per_unit,
            unit_value,
        }) => {
            non_negative("stuff.count", count)?
                / positive("stuff.volume_per_unit", volume_per_unit)?
                * non_negative("stuff.unit_value", unit_value)?
        }
    };
    let work_total = non_negative("work_to_make", input.work_to_make)?
        * non_negative("work_factor", input.work_factor)?;
    let work = if work_total > WORK_FREE_THRESHOLD {
        work_total * VALUE_PER_WORK
    } else {
        0.0
    };
    let base_value = ingredients + stuff + work;
    if !base_value.is_finite() {
        return Err(DesignError::invalid("price", "overflowed"));
    }
    Ok(PriceBreakdown {
        ingredients,
        stuff,
        work,
        base_value,
        displayed: display_round(base_value)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn mv1_matches_the_vector() {
        let input = PriceInput {
            ingredients: vec![IngredientLine {
                count: 2.0,
                unit_value: 30.0,
            }],
            stuff: Some(StuffCost::Known {
                count: 80.0,
                volume_per_unit: 1.0,
                unit_value: 2.0,
            }),
            work_to_make: 20_000.0,
            work_factor: 1.0,
        };
        let b = market_value(&input).unwrap();
        assert!(close(b.ingredients, 60.0));
        assert!(close(b.stuff, 160.0));
        assert!(close(b.work, 72.0));
        assert!(close(b.base_value, 292.0));
        assert_eq!(b.displayed, 290.0);
    }

    #[test]
    fn mv2_matches_the_vector() {
        let input = PriceInput {
            ingredients: vec![],
            stuff: Some(StuffCost::Known {
                count: 40.0,
                volume_per_unit: 0.1,
                unit_value: 1.0,
            }),
            work_to_make: 3_200.0,
            work_factor: 1.0,
        };
        let b = market_value(&input).unwrap();
        assert!(close(b.stuff, 400.0));
        assert!(close(b.work, 11.52));
        assert!(close(b.base_value, 411.52));
        assert_eq!(b.displayed, 410.0);
    }

    #[test]
    fn work_factor_scales_the_work_term() {
        // Work 1000 * factor 1.5 = 1500; 1500 * 0.0036 = 5.4.
        let input = PriceInput {
            ingredients: vec![],
            stuff: None,
            work_to_make: 1_000.0,
            work_factor: 1.5,
        };
        assert!(close(market_value(&input).unwrap().work, 5.4));
    }

    #[rstest]
    #[case(0.0, 0.0)]
    #[case(1.0, 0.0)]
    #[case(2.0, 0.0)]
    // Just above the threshold the whole work counts: 2.5 * 0.0036 = 0.009.
    #[case(2.5, 0.009)]
    fn small_work_is_free(#[case] work: f64, #[case] expected: f64) {
        let input = PriceInput {
            ingredients: vec![],
            stuff: None,
            work_to_make: work,
            work_factor: 1.0,
        };
        assert!(close(market_value(&input).unwrap().work, expected));
    }

    #[test]
    fn unknown_stuff_uses_the_default_guess() {
        let input = PriceInput {
            ingredients: vec![],
            stuff: Some(StuffCost::Unknown { count: 50.0 }),
            work_to_make: 0.0,
            work_factor: 1.0,
        };
        assert!(close(market_value(&input).unwrap().stuff, 100.0));
    }

    #[rstest]
    #[case(150.0, 150.0)]
    #[case(200.0, 200.0)]
    #[case(202.0, 200.0)]
    #[case(203.0, 205.0)]
    // Ties go to even: 202.5 / 5 = 40.5 rounds to 40.
    #[case(202.5, 200.0)]
    #[case(207.5, 210.0)]
    fn display_rounding(#[case] value: f64, #[case] expected: f64) {
        assert_eq!(display_round(value).unwrap(), expected);
    }

    #[test]
    fn bad_input_is_rejected() {
        let mut input = PriceInput {
            ingredients: vec![IngredientLine {
                count: f64::NAN,
                unit_value: 1.0,
            }],
            stuff: None,
            work_to_make: 0.0,
            work_factor: 1.0,
        };
        assert_eq!(
            market_value(&input).unwrap_err().code(),
            "design.invalid-input"
        );
        input.ingredients.clear();
        input.stuff = Some(StuffCost::Known {
            count: 1.0,
            volume_per_unit: 0.0,
            unit_value: 1.0,
        });
        assert!(market_value(&input).is_err());
        input.stuff = None;
        input.work_factor = -1.0;
        assert!(market_value(&input).is_err());
        assert!(display_round(f64::NAN).is_err());
    }

    proptest! {
        #[test]
        fn parts_sum_to_the_base_value(
            count in 0.0f64..100.0, price in 0.0f64..500.0,
            stuff_count in 0.0f64..300.0, stuff_price in 0.0f64..50.0,
            work in 0.0f64..100_000.0,
        ) {
            let input = PriceInput {
                ingredients: vec![IngredientLine { count, unit_value: price }],
                stuff: Some(StuffCost::Known { count: stuff_count, volume_per_unit: 1.0, unit_value: stuff_price }),
                work_to_make: work,
                work_factor: 1.0,
            };
            let b = market_value(&input).unwrap();
            prop_assert!((b.base_value - (b.ingredients + b.stuff + b.work)).abs() < 1e-6);
            prop_assert!(b.base_value >= 0.0);
            prop_assert!((b.displayed - b.base_value).abs() <= 2.5 + 1e-9);
        }

        #[test]
        fn more_work_never_lowers_the_price(work in 0.0f64..100_000.0, extra in 0.0f64..50_000.0) {
            let base = PriceInput { ingredients: vec![], stuff: None, work_to_make: work, work_factor: 1.0 };
            let mut more = base.clone();
            more.work_to_make += extra;
            prop_assert!(market_value(&more).unwrap().base_value >= market_value(&base).unwrap().base_value);
        }

        #[test]
        fn non_finite_numbers_are_errors(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY)]) {
            let input = PriceInput { ingredients: vec![], stuff: None, work_to_make: bad, work_factor: 1.0 };
            prop_assert!(market_value(&input).is_err());
        }
    }
}
