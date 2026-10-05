//! The facts of a weapon definition that the convert scan shows next to its status: the weapon tags, the
//! weapon classes and a few numbers.
//!
//! Everything is read from the resolved definition of the load (parents merged, patches applied), through the
//! small readers of `rimstudio-design`. No number is looked up in a table and none is computed: a number the
//! definition does not give is absent. For a definition that already carries a Combat Extended conversion the
//! numbers are the converted ones, because the resolved definition includes the project's own patches.
//!
//! Where each number comes from:
//!
//! | field | gun | melee weapon |
//! | --- | --- | --- |
//! | `damage` | `damageAmountBase` of the default projectile of the shooting verb | `power` of the strongest tool |
//! | `range` | `range` of the shooting verb | absent |
//! | `cooldown` | stat `RangedWeapon_Cooldown` | `cooldownTime` of the strongest tool |
//! | `warmup` | `warmupTime` of the shooting verb | absent |
//! | `mass` | stat `Mass` | stat `Mass` |
//! | `burst` | `burstShotCount` of the shooting verb | absent |
//! | `marketValue` | stat `MarketValue` | stat `MarketValue` |

use rimstudio_core::tree::Node;
use rimstudio_defs::DefDatabases;
use rimstudio_design::reader::access::{
    child_number, child_text, list_items, list_texts, number_map,
};
use rimstudio_design::reader::shooting_verb;
use rimstudio_ipc_types::designer::ConvertVanillaDto;

/// The thing def database name.
const THING_TYPE: &str = "ThingDef";

/// The tags, the classes and the numbers of one weapon definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct WeaponFacts {
    /// The `weaponTags` entries in definition order.
    pub tags: Vec<String>,
    /// The `weaponClasses` entries in definition order.
    pub weapon_classes: Vec<String>,
    /// The numbers; `None` when the definition is not resolved.
    pub numbers: Option<ConvertVanillaDto>,
}

/// Reads the facts of the resolved definition `def_name`. A definition that is not among the resolved
/// definitions gives empty facts.
pub(crate) fn facts_of(dbs: &DefDatabases, def_name: &str) -> WeaponFacts {
    match dbs.get(THING_TYPE, def_name) {
        Some(record) => facts_of_node(dbs, &record.node),
        None => WeaponFacts::default(),
    }
}

/// Reads the facts of a resolved definition node.
pub(crate) fn facts_of_node(dbs: &DefDatabases, node: &Node) -> WeaponFacts {
    let stats = number_map(node, "statBases");
    let mut numbers = ConvertVanillaDto {
        mass: stats.get("Mass").copied(),
        market_value: stats.get("MarketValue").copied(),
        ..ConvertVanillaDto::default()
    };
    if let Some(verb) = shooting_verb(node) {
        numbers.range = child_number(verb, "range");
        numbers.warmup = child_number(verb, "warmupTime");
        numbers.burst = child_number(verb, "burstShotCount").and_then(whole);
        numbers.cooldown = stats.get("RangedWeapon_Cooldown").copied();
        numbers.damage = child_text(verb, "defaultProjectile")
            .and_then(|p| dbs.get(THING_TYPE, &p))
            .and_then(|d| d.node.child("projectile"))
            .and_then(|p| child_number(p, "damageAmountBase"));
    } else if let Some(tool) = strongest_tool(node) {
        numbers.damage = child_number(tool, "power");
        numbers.cooldown = child_number(tool, "cooldownTime");
    }
    WeaponFacts {
        tags: list_texts(node, "weaponTags"),
        weapon_classes: list_texts(node, "weaponClasses"),
        numbers: Some(numbers),
    }
}

/// The tool with the highest power; the first one wins a tie, so the result is the same on every run.
fn strongest_tool(node: &Node) -> Option<&Node> {
    let mut best: Option<(&Node, f64)> = None;
    for tool in list_items(node, "tools") {
        let Some(power) = child_number(tool, "power") else {
            continue;
        };
        if best.is_none_or(|(_, p)| power > p) {
            best = Some((tool, power));
        }
    }
    best.map(|(t, _)| t)
}

/// A non negative whole number as `u32`; `None` for a fraction, a negative number or a number that is too big.
fn whole(value: f64) -> Option<u32> {
    let rounded = value.round();
    if (value - rounded).abs() > f64::EPSILON || !(0.0..=f64::from(u32::MAX)).contains(&rounded) {
        return None;
    }
    // The range check above makes the cast exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(rounded as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_accepts_integers_only() {
        assert_eq!(whole(3.0), Some(3));
        assert_eq!(whole(0.0), Some(0));
        assert_eq!(whole(2.5), None);
        assert_eq!(whole(-1.0), None);
        assert_eq!(whole(1e12), None);
    }
}
