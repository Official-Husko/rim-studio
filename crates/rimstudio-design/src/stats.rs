//! A pure stat pipeline: base value, stuff factor and offset, stat parts by priority, rounding and clamping.
//!
//! This mirrors, over plain structs, what the game does for one stat of an item that has no pawn:
//!
//! 1. Start from the item's base value, or the stat definition's default when the item sets none.
//! 2. With a stuff: multiply by the stuff's factor for the stat (only when the value is above zero or the
//!    stat applies factors to negative values), then add the stuff's offset.
//! 3. Run the stat parts sorted by priority, highest first (ties keep their input order). A stuff part adds
//!    `item multiplier * stuff power`; a quality part scales the whole running sum (the gain limited by an
//!    optional maximum) and does nothing to values at or below zero unless it applies to negatives; factor and
//!    offset parts multiply and add.
//! 4. Finalize: round to the nearest 5 (ties to even) when the absolute value exceeds a threshold, round to an
//!    integer (ties to even) when the stat asks for it, clamp to `[min, max]`.
//!
//! Every number is a parameter: there are no def types and no tables. The caller resolves the stat
//! definitions, stuff and quality from the user's install. For a market value without an explicit
//! `MarketValue`, pass `crate::price::market_value(..).base_value` as the base.

use crate::error::{DesignError, DesignResult, finite};
use serde::{Deserialize, Serialize};

/// Largest magnitude the game uses as the open end of a stat range.
pub const OPEN_LIMIT: f64 = 9_999_999.0;

/// The finalisation rules of a stat definition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatRules {
    /// Value used when the item has no base value for the stat.
    pub default_base: f64,
    /// Whether the stuff factor also applies to values at or below zero.
    pub apply_factors_if_negative: bool,
    /// Absolute values above this are rounded to the nearest 5; `None` never rounds that way.
    pub round_to_five_over: Option<f64>,
    /// Round the final value to an integer.
    pub round_value: bool,
    /// Lower clamp.
    pub min_value: f64,
    /// Upper clamp.
    pub max_value: f64,
}

impl Default for StatRules {
    /// Default 0, no negative factors, no rounding, open range.
    fn default() -> Self {
        Self {
            default_base: 0.0,
            apply_factors_if_negative: false,
            round_to_five_over: None,
            round_value: false,
            min_value: -OPEN_LIMIT,
            max_value: OPEN_LIMIT,
        }
    }
}

/// What the chosen stuff does to the stat directly (not through a stat part).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StuffEffect {
    /// Stuff factor for the stat; `None` means no factor (1).
    pub factor: Option<f64>,
    /// Stuff offset for the stat (0 when none).
    pub offset: f64,
}

/// One stat part, with its inputs already resolved by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StatPart {
    /// Adds `multiplier * stuff_power`: the item's multiplier stat times the stuff's power stat.
    Stuff {
        /// Priority; higher runs first.
        priority: i32,
        /// The item's multiplier stat value.
        multiplier: f64,
        /// The stuff's power stat value (0 without a stuff).
        stuff_power: f64,
    },
    /// Scales the running value by the quality factor.
    QualityFactor {
        /// Priority; higher runs first.
        priority: i32,
        /// Quality factor for the item's quality.
        factor: f64,
        /// Maximum gain over the unscaled value, when the stat defines one.
        max_gain: Option<f64>,
        /// Whether the part also applies to values at or below zero.
        apply_to_negative: bool,
    },
    /// Adds the quality offset.
    QualityOffset {
        /// Priority; higher runs first.
        priority: i32,
        /// Offset for the item's quality.
        offset: f64,
    },
    /// Multiplies by a plain factor.
    Factor {
        /// Priority; higher runs first.
        priority: i32,
        /// The factor.
        factor: f64,
    },
    /// Adds a plain offset.
    Offset {
        /// Priority; higher runs first.
        priority: i32,
        /// The offset.
        offset: f64,
    },
}

impl StatPart {
    /// The part's priority.
    #[must_use]
    pub fn priority(&self) -> i32 {
        match self {
            Self::Stuff { priority, .. }
            | Self::QualityFactor { priority, .. }
            | Self::QualityOffset { priority, .. }
            | Self::Factor { priority, .. }
            | Self::Offset { priority, .. } => *priority,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Stuff { .. } => "part:stuff",
            Self::QualityFactor { .. } => "part:quality-factor",
            Self::QualityOffset { .. } => "part:quality-offset",
            Self::Factor { .. } => "part:factor",
            Self::Offset { .. } => "part:offset",
        }
    }
}

/// One step of an evaluation, for explaining a number in a user interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatStep {
    /// Stable step label: `base`, `stuff-factor`, `stuff-offset`, `part:*`, `round-five`, `round-int`, `clamp`.
    pub label: &'static str,
    /// Running value after the step.
    pub value: f64,
}

/// Result of [`StatPipeline::evaluate`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatOutput {
    /// The final value.
    pub value: f64,
    /// The value before rounding and clamping.
    pub unfinalized: f64,
    /// Every step that changed or checked the value, in order.
    pub steps: Vec<StatStep>,
}

/// A stat pipeline over plain inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatPipeline {
    /// Finalisation rules of the stat.
    pub rules: StatRules,
    /// The item's base value; `None` uses `rules.default_base`.
    pub base: Option<f64>,
    /// The chosen stuff's direct effect on the stat, when a stuff is chosen.
    pub stuff: Option<StuffEffect>,
    /// The stat's parts, in any order (they are sorted by priority on evaluation).
    pub parts: Vec<StatPart>,
}

impl StatPipeline {
    /// Evaluates the stat.
    ///
    /// Hand checks:
    ///
    /// * AP-1: stuff part multiplier 0.5 and power 0.8 add 0.4 to a base of 0; a quality part with factor 1.15
    ///   scales it to `0.46`.
    /// * AP-2: multiplier 1.5, power 1.4 gives 2.1; quality 1.8 gives 3.78; the clamp to `[0, 2]` gives 2.0.
    /// * A base of 100 with stuff factor 2.0 and offset 10 gives 210; rounding to 5 above 200 keeps 210; with
    ///   offset 12 the value 212 becomes `42.4 -> 42 -> 210`.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] for a non finite input, a negative maximum gain, or `min_value` above
    /// `max_value`.
    pub fn evaluate(&self) -> DesignResult<StatOutput> {
        let rules = &self.rules;
        finite("default_base", rules.default_base)?;
        finite("min_value", rules.min_value)?;
        finite("max_value", rules.max_value)?;
        if rules.min_value > rules.max_value {
            return Err(DesignError::invalid(
                "min_value",
                "must not be above max_value",
            ));
        }
        if let Some(threshold) = rules.round_to_five_over {
            finite("round_to_five_over", threshold)?;
        }

        let mut steps = Vec::new();
        let mut value = match self.base {
            Some(v) => finite("base", v)?,
            None => rules.default_base,
        };
        steps.push(StatStep {
            label: "base",
            value,
        });

        if let Some(stuff) = &self.stuff {
            if let Some(factor) = stuff.factor {
                finite("stuff.factor", factor)?;
                if value > 0.0 || rules.apply_factors_if_negative {
                    value *= factor;
                }
                steps.push(StatStep {
                    label: "stuff-factor",
                    value,
                });
            }
            value += finite("stuff.offset", stuff.offset)?;
            steps.push(StatStep {
                label: "stuff-offset",
                value,
            });
        }

        let mut parts: Vec<&StatPart> = self.parts.iter().collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.priority()));
        for part in parts {
            value = apply_part(part, value)?;
            steps.push(StatStep {
                label: part.label(),
                value,
            });
        }
        if !value.is_finite() {
            return Err(DesignError::invalid("value", "overflowed"));
        }
        let unfinalized = value;

        if let Some(threshold) = rules.round_to_five_over
            && value.abs() > threshold
        {
            value = (value / 5.0).round_ties_even() * 5.0;
            steps.push(StatStep {
                label: "round-five",
                value,
            });
        }
        if rules.round_value {
            value = value.round_ties_even();
            steps.push(StatStep {
                label: "round-int",
                value,
            });
        }
        let clamped = value.clamp(rules.min_value, rules.max_value);
        if clamped != value {
            value = clamped;
            steps.push(StatStep {
                label: "clamp",
                value,
            });
        }
        Ok(StatOutput {
            value,
            unfinalized,
            steps,
        })
    }
}

fn apply_part(part: &StatPart, value: f64) -> DesignResult<f64> {
    Ok(match *part {
        StatPart::Stuff {
            multiplier,
            stuff_power,
            ..
        } => value + finite("multiplier", multiplier)? * finite("stuff_power", stuff_power)?,
        StatPart::QualityFactor {
            factor,
            max_gain,
            apply_to_negative,
            ..
        } => {
            finite("quality.factor", factor)?;
            if let Some(cap) = max_gain {
                finite("quality.max_gain", cap)?;
            }
            if value <= 0.0 && !apply_to_negative {
                value
            } else {
                let gain = value * factor - value;
                value + max_gain.map_or(gain, |cap| gain.min(cap))
            }
        }
        StatPart::QualityOffset { offset, .. } | StatPart::Offset { offset, .. } => {
            value + finite("offset", offset)?
        }
        StatPart::Factor { factor, .. } => value * finite("factor", factor)?,
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

    fn armor_rules() -> StatRules {
        StatRules {
            min_value: 0.0,
            max_value: 2.0,
            ..StatRules::default()
        }
    }

    fn armor(multiplier: f64, power: f64, quality: f64) -> StatPipeline {
        StatPipeline {
            rules: armor_rules(),
            base: None,
            stuff: Some(StuffEffect {
                factor: None,
                offset: 0.0,
            }),
            parts: vec![
                // Listed in the "wrong" order on purpose: priority decides, not position.
                StatPart::QualityFactor {
                    priority: 0,
                    factor: quality,
                    max_gain: None,
                    apply_to_negative: false,
                },
                StatPart::Stuff {
                    priority: 100,
                    multiplier,
                    stuff_power: power,
                },
            ],
        }
    }

    #[test]
    fn ap1_quality_scales_the_stuff_contribution() {
        // (0.5 * 0.8) * 1.15 = 0.46.
        let out = armor(0.5, 0.8, 1.15).evaluate().unwrap();
        assert!(close(out.value, 0.46));
        assert!(close(out.unfinalized, 0.46));
    }

    #[test]
    fn ap2_clamps_to_the_stat_maximum() {
        // 1.5 * 1.4 = 2.1; 2.1 * 1.8 = 3.78; clamped to 2.0.
        let out = armor(1.5, 1.4, 1.8).evaluate().unwrap();
        assert!(close(out.unfinalized, 3.78));
        assert_eq!(out.value, 2.0);
        assert_eq!(out.steps.last().map(|s| s.label), Some("clamp"));
    }

    #[test]
    fn priority_decides_the_order_not_position() {
        // Quality (priority 0) after stuff (priority 100): (0 + 0.5) * 2 = 1.0.
        // If the order were reversed the result would be 0 * 2 + 0.5 = 0.5.
        let out = armor(1.0, 0.5, 2.0).evaluate().unwrap();
        assert!(close(out.value, 1.0));
    }

    #[test]
    fn equal_priorities_keep_input_order() {
        // Offset +1 then factor x3: (0 + 1) * 3 = 3. Reversed it would be 1.
        let pipeline = StatPipeline {
            rules: StatRules::default(),
            base: Some(0.0),
            stuff: None,
            parts: vec![
                StatPart::Offset {
                    priority: 5,
                    offset: 1.0,
                },
                StatPart::Factor {
                    priority: 5,
                    factor: 3.0,
                },
            ],
        };
        assert!(close(pipeline.evaluate().unwrap().value, 3.0));
    }

    #[test]
    fn base_defaults_when_the_item_sets_none() {
        let pipeline = StatPipeline {
            rules: StatRules {
                default_base: 100.0,
                ..StatRules::default()
            },
            base: None,
            stuff: None,
            parts: vec![],
        };
        assert_eq!(pipeline.evaluate().unwrap().value, 100.0);
    }

    #[rstest]
    // Factor applies to positive values.
    #[case(10.0, false, 2.0, 5.0, 25.0)]
    // Factor skipped for a zero base, offset still added.
    #[case(0.0, false, 2.0, 5.0, 5.0)]
    // Negative base keeps its value unless the stat applies factors to negatives.
    #[case(-4.0, false, 2.0, 1.0, -3.0)]
    #[case(-4.0, true, 2.0, 1.0, -7.0)]
    fn stuff_factor_then_offset(
        #[case] base: f64,
        #[case] negatives: bool,
        #[case] factor: f64,
        #[case] offset: f64,
        #[case] expected: f64,
    ) {
        let pipeline = StatPipeline {
            rules: StatRules {
                apply_factors_if_negative: negatives,
                ..StatRules::default()
            },
            base: Some(base),
            stuff: Some(StuffEffect {
                factor: Some(factor),
                offset,
            }),
            parts: vec![],
        };
        assert!(close(pipeline.evaluate().unwrap().value, expected));
    }

    #[test]
    fn quality_gain_is_capped_and_negatives_are_skipped() {
        let part = |max_gain| StatPart::QualityFactor {
            priority: 0,
            factor: 5.0,
            max_gain,
            apply_to_negative: false,
        };
        let build = |base: f64, cap: Option<f64>| StatPipeline {
            rules: StatRules::default(),
            base: Some(base),
            stuff: None,
            parts: vec![part(cap)],
        };
        // 100 * 5 = 500, gain 400 capped at 300: 400.
        assert!(close(
            build(100.0, Some(300.0)).evaluate().unwrap().value,
            400.0
        ));
        // No cap: 500.
        assert!(close(build(100.0, None).evaluate().unwrap().value, 500.0));
        // A negative value is left alone.
        assert!(close(build(-10.0, None).evaluate().unwrap().value, -10.0));
    }

    #[rstest]
    // Not above the threshold: untouched.
    #[case(199.0, 199.0)]
    #[case(200.0, 200.0)]
    // 212 / 5 = 42.4 rounds to 42: 210.
    #[case(212.0, 210.0)]
    // 202.5 / 5 = 40.5 ties to even 40: 200.
    #[case(202.5, 200.0)]
    // 207.5 / 5 = 41.5 ties to even 42: 210.
    #[case(207.5, 210.0)]
    // Negative magnitudes round the same way.
    #[case(-212.0, -210.0)]
    fn rounding_to_five(#[case] value: f64, #[case] expected: f64) {
        let pipeline = StatPipeline {
            rules: StatRules {
                round_to_five_over: Some(200.0),
                ..StatRules::default()
            },
            base: Some(value),
            stuff: None,
            parts: vec![],
        };
        assert_eq!(pipeline.evaluate().unwrap().value, expected);
    }

    #[rstest]
    #[case(2.4, 2.0)]
    #[case(2.5, 2.0)]
    #[case(3.5, 4.0)]
    #[case(2.6, 3.0)]
    fn integer_rounding_ties_to_even(#[case] value: f64, #[case] expected: f64) {
        let pipeline = StatPipeline {
            rules: StatRules {
                round_value: true,
                ..StatRules::default()
            },
            base: Some(value),
            stuff: None,
            parts: vec![],
        };
        assert_eq!(pipeline.evaluate().unwrap().value, expected);
    }

    #[test]
    fn steps_explain_the_value() {
        let out = armor(0.5, 0.8, 1.15).evaluate().unwrap();
        let labels: Vec<&str> = out.steps.iter().map(|s| s.label).collect();
        assert_eq!(
            labels,
            ["base", "stuff-offset", "part:stuff", "part:quality-factor"]
        );
    }

    #[test]
    fn invalid_inputs_are_errors() {
        let mut p = armor(0.5, 0.8, 1.15);
        p.base = Some(f64::NAN);
        assert_eq!(p.evaluate().unwrap_err().code(), "design.invalid-input");
        let mut p = armor(0.5, f64::INFINITY, 1.15);
        assert!(p.evaluate().is_err());
        p = armor(0.5, 0.8, 1.15);
        p.rules.min_value = 3.0;
        assert!(p.evaluate().is_err());
    }

    #[test]
    fn quality_offset_part_adds() {
        let pipeline = StatPipeline {
            rules: StatRules::default(),
            base: Some(1.0),
            stuff: None,
            parts: vec![StatPart::QualityOffset {
                priority: 0,
                offset: 0.25,
            }],
        };
        assert!(close(pipeline.evaluate().unwrap().value, 1.25));
    }

    proptest! {
        #[test]
        fn clamped_output_stays_in_range(
            multiplier in 0.0f64..5.0, power in 0.0f64..5.0, quality in 0.0f64..3.0,
        ) {
            let out = armor(multiplier, power, quality).evaluate().unwrap();
            prop_assert!((0.0..=2.0).contains(&out.value));
        }

        #[test]
        fn higher_quality_never_lowers_a_positive_stat(
            multiplier in 0.0f64..5.0, power in 0.0f64..5.0, quality in 0.1f64..2.0, extra in 0.0f64..1.0,
        ) {
            let low = armor(multiplier, power, quality).evaluate().unwrap().value;
            let high = armor(multiplier, power, quality + extra).evaluate().unwrap().value;
            prop_assert!(high >= low - 1e-12);
        }

        #[test]
        fn non_finite_inputs_are_errors(bad in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)]) {
            prop_assert!(armor(bad, 1.0, 1.0).evaluate().is_err());
            prop_assert!(armor(1.0, bad, 1.0).evaluate().is_err());
            prop_assert!(armor(1.0, 1.0, bad).evaluate().is_err());
        }
    }
}
