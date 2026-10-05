//! Bridges from the spec to the numeric inputs of the design math modules.
//!
//! The conversions return `None` (or skip a tool) when a required number is missing; they never guess a
//! value except for the documented game defaults of burst count, burst gap, chance factor and implied
//! armor penetration.

use crate::melee::{MeleeCapacity, MeleeTool};
use crate::ranged::{AccuracyBands, DEFAULT_AP_PER_DAMAGE, RangedProfile};

use super::spec::{DesignSpec, ItemKind};

/// Shots per burst when the verb sets none.
pub const DEFAULT_BURST_COUNT: u32 = 1;
/// Ticks between burst shots when the verb sets none.
pub const DEFAULT_TICKS_BETWEEN_SHOTS: f64 = 15.0;

impl DesignSpec {
    /// The inputs of the ranged formulas, when damage, warmup, cooldown and range are set.
    ///
    /// Armor penetration is the explicit value, else the implied `0.015 x damage`. The accuracy bands are
    /// present only when all four are set.
    #[must_use]
    pub fn ranged_profile(&self) -> Option<RangedProfile> {
        if self.kind != ItemKind::Ranged {
            return None;
        }
        let r = self.ranged.as_ref()?;
        let damage = r.damage?.value;
        let accuracy = match (
            r.accuracy.touch,
            r.accuracy.short,
            r.accuracy.medium,
            r.accuracy.long,
        ) {
            (Some(touch), Some(short), Some(medium), Some(long)) => Some(AccuracyBands {
                touch: touch.value,
                short: short.value,
                medium: medium.value,
                long: long.value,
            }),
            _ => None,
        };
        Some(RangedProfile {
            damage,
            armor_penetration: r
                .armor_penetration
                .map_or(damage * DEFAULT_AP_PER_DAMAGE, |a| a.value),
            warmup: r.warmup?.value,
            cooldown: r.cooldown?.value,
            burst_count: r.burst_count.map_or(DEFAULT_BURST_COUNT, |b| b.value),
            ticks_between_shots: r
                .ticks_between_burst_shots
                .map_or(DEFAULT_TICKS_BETWEEN_SHOTS, |t| t.value),
            range: r.range?.value,
            accuracy,
        })
    }

    /// The tools that have a power and a cooldown, as inputs of the melee formulas. Incomplete tools are
    /// skipped.
    #[must_use]
    pub fn melee_tools(&self) -> Vec<MeleeTool> {
        self.tools
            .iter()
            .filter_map(|t| {
                Some(MeleeTool {
                    label: t.label.clone(),
                    capacities: t
                        .capacities
                        .iter()
                        .map(|c| MeleeCapacity::named(c))
                        .collect(),
                    power: t.power?.value,
                    cooldown: t.cooldown_time?.value,
                    armor_penetration: t.armor_penetration.map(|a| a.value),
                    chance_factor: t.chance_factor.map_or(1.0, |c| c.value),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ScalarField, ToolSpec, ValueSource};

    #[test]
    fn ranged_profile_uses_defaults_for_optional_inputs() {
        let mut spec = DesignSpec::new_ranged("RS_X", "x");
        assert!(spec.ranged_profile().is_none());
        spec.offer(ScalarField::Damage, 10.0, ValueSource::Typed);
        spec.offer(ScalarField::Warmup, 1.0, ValueSource::Typed);
        spec.offer(ScalarField::Cooldown, 2.0, ValueSource::Typed);
        spec.offer(ScalarField::Range, 20.0, ValueSource::Typed);
        let p = spec.ranged_profile().unwrap();
        assert_eq!(p.burst_count, 1);
        assert!((p.armor_penetration - 0.15).abs() < 1e-12);
        assert!((p.ticks_between_shots - 15.0).abs() < 1e-12);
        assert!(p.accuracy.is_none());
    }

    #[test]
    fn melee_tools_skip_incomplete_entries() {
        let mut spec = DesignSpec::new_melee("RS_X", "x");
        spec.tools
            .push(ToolSpec::new("blade", &["Cut"]).with_numbers(9.0, 2.0, ValueSource::Typed));
        spec.tools.push(ToolSpec::new("handle", &["Blunt"]));
        let tools = spec.melee_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].label, "blade");
        assert!((tools[0].chance_factor - 1.0).abs() < 1e-12);
    }
}
