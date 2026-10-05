//! Options of the reference reader: which defs are left out, how roles are decided, where the reference armor
//! comes from. All of it is data (serde, camelCase) that a project or the owner can change; the defaults are
//! structural rules, not tables of game values.

use serde::{Deserialize, Serialize};

use crate::classes::ItemKind;
use crate::melee::DamageKind;

/// Rules that keep items out of the direct fire and melee pools.
///
/// A def that matches is not read; it is listed in the reader's `skipped` list with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Exclusions {
    /// Skip defs with `menuHidden` true (turret guns and other items the player cannot build).
    pub menu_hidden: bool,
    /// Skip defs with `destroyOnDrop` true (guns that only exist inside a turret or a creature).
    pub destroy_on_drop: bool,
    /// Skip a ranged weapon whose verb class contains one of these texts (one use and launcher verbs).
    pub verb_class_parts: Vec<String>,
    /// Skip a weapon with a weapon tag containing one of these texts (case insensitive).
    pub tag_parts: Vec<String>,
    /// Skip a def whose name ends with one of these texts (duplicates of the same gun for one expansion).
    pub def_name_suffixes: Vec<String>,
    /// Skip a ranged weapon whose projectile has an explosion radius above zero.
    pub explosive: bool,
    /// Skip a def whose verb or tool entries carry a `Class` attribute (a modded verb or tool class means the
    /// vanilla numbers do not apply, as with converted weapons).
    pub custom_classes: bool,
}

impl Default for Exclusions {
    fn default() -> Self {
        Self {
            menu_hidden: true,
            destroy_on_drop: true,
            verb_class_parts: vec!["OneUse".into(), "Launch".into()],
            tag_parts: vec!["turret".into(), "mechanoid".into()],
            def_name_suffixes: vec!["_Unique".into()],
            explosive: true,
            custom_classes: true,
        }
    }
}

/// One condition of a role rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "when")]
pub enum RoleCondition {
    /// Some weapon tag contains the text (case insensitive).
    TagContains {
        /// The text.
        text: String,
    },
    /// Some weapon class contains the text (case insensitive).
    ClassContains {
        /// The text.
        text: String,
    },
    /// Some tool has one of the capacities.
    CapacityAny {
        /// Capacity def names.
        names: Vec<String>,
    },
    /// No tool has any of the capacities.
    CapacityNone {
        /// Capacity def names.
        names: Vec<String>,
    },
    /// Every attack of the weapon is blunt.
    AllBlunt,
    /// Some attack of the weapon is sharp.
    AnySharp,
    /// A named stat is at least the value.
    StatAtLeast {
        /// Stat name in the reference item's stat map.
        stat: String,
        /// The lower bound.
        min: f64,
    },
    /// A named stat is below the value.
    StatBelow {
        /// Stat name in the reference item's stat map.
        stat: String,
        /// The upper bound.
        max: f64,
    },
}

/// A role and the conditions that all have to hold for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRule {
    /// The item kind the rule applies to.
    pub kind: ItemKind,
    /// The role it assigns.
    pub role: String,
    /// All conditions must hold; an empty list always matches.
    #[serde(default)]
    pub all: Vec<RoleCondition>,
}

/// An ordered decision list for roles: the first matching rule wins. A weapon no rule claims gets no role, and
/// the pool derives one from the most common shared tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRules {
    /// The rules in order.
    pub rules: Vec<RoleRule>,
}

impl Default for RoleRules {
    /// Melee roles from tool capacities (piercing, blunt, blade) and ranged roles from the weapon classes and
    /// tags the game uses to group guns (bow, sniper, shotgun, smg, heavy, pistol, rifle).
    ///
    /// The ranged list is a decision list authored by RimStudio, not a table of game values: it names the
    /// structural tags and classes of the weapon groups and uses two plain stat thresholds (a short range
    /// separates a shotgun from a submachine gun, a high mass separates a heavy gun from a rifle). A weapon that
    /// no rule claims gets no role, and the pool derives one from its most common shared tag. Every rule can be
    /// replaced through the reader options.
    fn default() -> Self {
        let class = |text: &str| RoleCondition::ClassContains { text: text.into() };
        let ranged = |role: &str, all: Vec<RoleCondition>| RoleRule {
            kind: ItemKind::Ranged,
            role: role.into(),
            all,
        };
        Self {
            rules: vec![
                RoleRule {
                    kind: ItemKind::Melee,
                    role: "piercing".into(),
                    all: vec![
                        RoleCondition::CapacityAny {
                            names: vec!["Stab".into()],
                        },
                        RoleCondition::CapacityNone {
                            names: vec!["Cut".into()],
                        },
                    ],
                },
                RoleRule {
                    kind: ItemKind::Melee,
                    role: "blunt".into(),
                    all: vec![RoleCondition::AllBlunt],
                },
                RoleRule {
                    kind: ItemKind::Melee,
                    role: "blade".into(),
                    all: vec![RoleCondition::AnySharp],
                },
                ranged(
                    "bow",
                    vec![RoleCondition::TagContains {
                        text: "NeolithicRanged".into(),
                    }],
                ),
                ranged("sniper", vec![class("LongShots")]),
                ranged(
                    "shotgun",
                    vec![
                        class("ShortShots"),
                        RoleCondition::StatBelow {
                            stat: "range".into(),
                            max: 16.0,
                        },
                    ],
                ),
                ranged("smg", vec![class("ShortShots"), class("RangedHeavy")]),
                ranged(
                    "heavy",
                    vec![
                        class("RangedHeavy"),
                        RoleCondition::StatAtLeast {
                            stat: "mass".into(),
                            min: 8.0,
                        },
                    ],
                ),
                ranged(
                    "pistol",
                    vec![
                        class("RangedLight"),
                        RoleCondition::StatBelow {
                            stat: "mass".into(),
                            max: 3.0,
                        },
                    ],
                ),
                ranged(
                    "rifle",
                    vec![RoleCondition::TagContains { text: "Gun".into() }],
                ),
            ],
        }
    }
}

impl RoleRules {
    /// An empty decision list (every role comes from the pool's tag derivation).
    #[must_use]
    pub fn none() -> Self {
        Self { rules: Vec::new() }
    }
}

/// The input facts of a role decision.
#[derive(Debug, Clone, Copy)]
pub struct RoleFacts<'a> {
    /// Weapon tags.
    pub tags: &'a [String],
    /// Weapon classes.
    pub classes: &'a [String],
    /// Capacity names of every tool.
    pub capacities: &'a [String],
    /// The named stats of the item.
    pub stats: &'a std::collections::BTreeMap<String, f64>,
}

impl RoleCondition {
    fn holds(&self, f: &RoleFacts<'_>) -> bool {
        let lower = |s: &str| s.to_lowercase();
        match self {
            Self::TagContains { text } => f.tags.iter().any(|t| lower(t).contains(&lower(text))),
            Self::ClassContains { text } => {
                f.classes.iter().any(|t| lower(t).contains(&lower(text)))
            }
            Self::CapacityAny { names } => f.capacities.iter().any(|c| names.contains(c)),
            Self::CapacityNone { names } => !f.capacities.iter().any(|c| names.contains(c)),
            Self::AllBlunt => {
                !f.capacities.is_empty()
                    && f.capacities
                        .iter()
                        .all(|c| DamageKind::for_capacity(c) == DamageKind::Blunt)
            }
            Self::AnySharp => f
                .capacities
                .iter()
                .any(|c| DamageKind::for_capacity(c) == DamageKind::Sharp),
            Self::StatAtLeast { stat, min } => f.stats.get(stat).is_some_and(|v| v >= min),
            Self::StatBelow { stat, max } => f.stats.get(stat).is_some_and(|v| v < max),
        }
    }
}

impl RoleRules {
    /// The role of an item: the first rule of its kind whose conditions all hold.
    #[must_use]
    pub fn decide(&self, kind: ItemKind, facts: &RoleFacts<'_>) -> Option<String> {
        self.rules
            .iter()
            .filter(|r| r.kind == kind)
            .find(|r| r.all.iter().all(|c| c.holds(facts)))
            .map(|r| r.role.clone())
    }
}

/// Where the reference armor layers of the ranged strength index come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub enum ArmorSource {
    /// Derived from the user's apparel: an unarmored layer, the median and the 90th percentile of the positive
    /// sharp ratings of fixed armor items. Falls back to [`ArmorSource::Given`] with the fallback ratings when
    /// the install has fewer than three such items.
    Derived,
    /// Ratings given by the caller (sharp ratings as fractions).
    Given {
        /// The ratings.
        ratings: Vec<f64>,
    },
}

/// Ratings used when the reference armor cannot be derived. These are neutral, evenly spaced layers, not
/// values taken from any item.
pub const FALLBACK_ARMOR_RATINGS: [f64; 3] = [0.0, 0.5, 1.0];

/// Everything the reference reader can be told.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReaderOptions {
    /// The database type that holds weapons (resolved like a `Class` attribute).
    pub db_type: String,
    /// The def name of the material that stuffed weapons are read with; `None` reads them unstuffed.
    pub reference_stuff: Option<String>,
    /// Which defs are left out.
    pub exclusions: Exclusions,
    /// The role decision list.
    pub roles: RoleRules,
    /// The reference armor of the strength index.
    pub armor: ArmorSource,
    /// The database type of stat definitions, read for defaults and limits.
    pub stat_db_type: String,
    /// The database type of damage definitions, read for default armor penetration.
    pub damage_db_type: String,
    /// The database type of melee maneuvers, read to learn which tool capacities do sharp damage.
    pub maneuver_db_type: String,
}

impl Default for ReaderOptions {
    fn default() -> Self {
        Self {
            db_type: "ThingDef".into(),
            reference_stuff: None,
            exclusions: Exclusions::default(),
            roles: RoleRules::default(),
            armor: ArmorSource::Derived,
            stat_db_type: "StatDef".into(),
            damage_db_type: "DamageDef".into(),
            maneuver_db_type: "ManeuverDef".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn facts<'a>(
        caps: &'a [String],
        stats: &'a BTreeMap<String, f64>,
        tags: &'a [String],
    ) -> RoleFacts<'a> {
        RoleFacts {
            tags,
            classes: &[],
            capacities: caps,
            stats,
        }
    }

    #[test]
    fn default_melee_roles_follow_the_capacities() {
        let rules = RoleRules::default();
        let stats = BTreeMap::new();
        let decide = |caps: &[&str]| {
            let caps: Vec<String> = caps.iter().map(|c| (*c).to_owned()).collect();
            rules.decide(ItemKind::Melee, &facts(&caps, &stats, &[]))
        };
        assert_eq!(decide(&["Stab"]).as_deref(), Some("piercing"));
        assert_eq!(decide(&["Blunt"]).as_deref(), Some("blunt"));
        assert_eq!(decide(&["Cut", "Blunt"]).as_deref(), Some("blade"));
        assert_eq!(decide(&["Cut", "Stab"]).as_deref(), Some("blade"));
        assert!(
            rules
                .decide(ItemKind::Ranged, &facts(&[], &stats, &[]))
                .is_none()
        );
    }

    #[test]
    fn default_ranged_roles_follow_classes_tags_and_two_thresholds() {
        let rules = RoleRules::default();
        let decide = |tags: &[&str], classes: &[&str], mass: f64, range: f64| {
            let tags: Vec<String> = tags.iter().map(|t| (*t).to_owned()).collect();
            let classes: Vec<String> = classes.iter().map(|t| (*t).to_owned()).collect();
            let stats: BTreeMap<String, f64> =
                [("mass".to_owned(), mass), ("range".to_owned(), range)]
                    .into_iter()
                    .collect();
            rules.decide(
                ItemKind::Ranged,
                &RoleFacts {
                    tags: &tags,
                    classes: &classes,
                    capacities: &[],
                    stats: &stats,
                },
            )
        };
        type Case<'a> = (&'a [&'a str], &'a [&'a str], f64, f64, Option<&'a str>);
        let cases: [Case<'_>; 9] = [
            (
                &["NeolithicRangedBasic"],
                &["Ranged"],
                1.0,
                20.0,
                Some("bow"),
            ),
            (
                &["Gun"],
                &["Ranged", "LongShots"],
                4.0,
                40.0,
                Some("sniper"),
            ),
            (&["Gun"], &["ShortShots"], 4.0, 12.0, Some("shotgun")),
            (
                &["Gun"],
                &["ShortShots", "RangedHeavy"],
                3.5,
                22.0,
                Some("smg"),
            ),
            (&["Gun"], &["RangedHeavy"], 9.0, 25.0, Some("heavy")),
            (&["Gun"], &["RangedLight"], 1.5, 25.0, Some("pistol")),
            (&["Gun"], &["RangedLight"], 4.5, 25.0, Some("rifle")),
            (&["Gun"], &["Ranged"], 3.5, 30.0, Some("rifle")),
            (&["RS_Odd"], &["Ranged"], 3.5, 30.0, None),
        ];
        for (tags, classes, mass, range, expected) in cases {
            assert_eq!(
                decide(tags, classes, mass, range).as_deref(),
                expected,
                "{tags:?} {classes:?} {mass} {range}"
            );
        }
    }

    #[test]
    fn stat_and_tag_conditions_decide_ranged_roles() {
        let rules = RoleRules {
            rules: vec![RoleRule {
                kind: ItemKind::Ranged,
                role: "RS_long".into(),
                all: vec![
                    RoleCondition::StatAtLeast {
                        stat: "range".into(),
                        min: 40.0,
                    },
                    RoleCondition::TagContains {
                        text: "rs_gun".into(),
                    },
                ],
            }],
        };
        let tags = vec!["RS_GunHeavy".to_owned()];
        let mut stats = BTreeMap::new();
        stats.insert("range".to_owned(), 41.0);
        assert_eq!(
            rules
                .decide(ItemKind::Ranged, &facts(&[], &stats, &tags))
                .as_deref(),
            Some("RS_long")
        );
        stats.insert("range".to_owned(), 39.0);
        assert!(
            rules
                .decide(ItemKind::Ranged, &facts(&[], &stats, &tags))
                .is_none()
        );
    }

    #[test]
    fn options_round_trip_through_json() {
        let o = ReaderOptions::default();
        let text = serde_json::to_string(&o).unwrap_or_default();
        let back: Result<ReaderOptions, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(o));
    }
}
