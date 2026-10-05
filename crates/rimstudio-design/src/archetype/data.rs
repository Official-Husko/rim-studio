//! The taxonomy: the families, archetypes and ladders, authored as JSON and embedded in the binary.
//!
//! The file `data/archetypes.json` is RimStudio's own data (R10, R11). It holds names, unit free counts and
//! RATIOS to medians that the solver reads from the user's install at run time, so the shapes of the
//! families live here and every number of the game comes from the install. Tool labels and capacities are
//! def names, not values. [`Taxonomy::builtin`] parses and validates the embedded file once.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::error::{DesignError, DesignResult};
use crate::model::ItemKind;

/// The embedded taxonomy file.
const BUILTIN: &str = include_str!("../../data/archetypes.json");

/// One rate of fire class of the ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RofClass {
    /// Stable id (`slow`, `medium`, `fast`).
    pub id: String,
    /// Label shown to the user.
    pub label: String,
    /// The multiplier of the cadence against the archetype's typical cadence.
    pub rate: f64,
}

/// How many shots a trigger pull fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionMode {
    /// One shot per cycle.
    Single,
    /// The short burst of the archetype.
    Burst,
    /// The long burst of the archetype.
    Auto,
}

/// An action of a gun (bolt, lever, pump, semi automatic, burst, full automatic, draw).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionProfile {
    /// Stable id.
    pub id: String,
    /// Label shown to the user.
    pub label: String,
    /// How many shots a cycle fires.
    pub mode: ActionMode,
    /// Multiplier on the warmup of the archetype.
    pub warmup: f64,
    /// Multiplier on the cooldown of the archetype.
    pub cooldown: f64,
    /// Multiplier on the ticks between burst shots.
    pub ticks: f64,
    /// One line for the UI.
    pub summary: String,
}

/// A calibre class of the vanilla damage ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalibreClass {
    /// Stable id.
    pub id: String,
    /// Label shown to the user.
    pub label: String,
    /// Typical real world rounds of the class, as a caption.
    pub examples: String,
    /// Multiplier on the damage of the archetype.
    pub damage: f64,
    /// Multiplier on the range.
    pub range: f64,
    /// Multiplier on the mass.
    pub mass: f64,
    /// Multiplier on the armor penetration.
    pub ap: f64,
    /// Multiplier on the cooldown (recoil).
    pub cooldown: f64,
}

/// A handling class (compact, standard, heavy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandlingClass {
    /// Stable id.
    pub id: String,
    /// Label shown to the user.
    pub label: String,
    /// Multiplier on the mass.
    pub mass: f64,
    /// Multiplier on the warmup.
    pub warmup: f64,
    /// Multiplier on the range.
    pub range: f64,
    /// Multiplier on the touch accuracy.
    pub touch: f64,
    /// Multiplier on the long accuracy.
    pub long: f64,
    /// Multiplier on the cooldown.
    pub cooldown: f64,
    /// Multiplier on the melee tool power.
    pub power: f64,
}

/// The tuning exponents of the solver (generic, documented in the specification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Exponents {
    /// Share of the strength scale that goes into damage (or tool power).
    pub damage_scale: f64,
    /// Share of the strength scale that goes into a shorter cooldown.
    pub cooldown_scale: f64,
    /// Share of the strength scale that goes into a shorter warmup.
    pub warmup_scale: f64,
    /// Share of the strength scale that goes into melee tool power.
    pub power_scale: f64,
    /// Share of the strength scale that goes into melee tempo.
    pub tempo_scale: f64,
    /// Exponent of the rate of fire on the cooldown.
    pub rof_cooldown: f64,
    /// Exponent of the rate of fire on the burst count.
    pub rof_burst: f64,
    /// Exponent of the rate of fire on the ticks between shots.
    pub rof_ticks: f64,
    /// Exponent of the strength scale on the mass.
    pub mass_coupling: f64,
    /// Exponent of the strength ratio on the work to make.
    pub work_coupling: f64,
    /// Exponent of the strength ratio on the material counts.
    pub cost_coupling: f64,
}

/// The shared ladders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ladders {
    /// Rate of fire classes.
    pub rof: Vec<RofClass>,
    /// Gun actions.
    pub actions: Vec<ActionProfile>,
    /// Calibre classes, lightest first.
    pub calibres: Vec<CalibreClass>,
    /// Handling classes.
    pub handlings: Vec<HandlingClass>,
    /// The solver exponents.
    pub exponents: Exponents,
}

/// A tool of a gun that bashes in melee (the stock, the barrel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashTool {
    /// The tool label.
    pub label: String,
    /// The capacity def names.
    pub capacities: Vec<String>,
}

/// The burst counts of an archetype (counts of shots, not tuned values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BurstCounts {
    /// Shots of a burst action.
    pub burst: u32,
    /// Shots of a full automatic action.
    pub auto: u32,
}

/// The shape of a ranged archetype: ratios to the install's medians.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RangedShape {
    /// Damage against the median damage.
    pub damage: f64,
    /// Range against the median range.
    pub range: f64,
    /// Warmup against the median warmup.
    pub warmup: f64,
    /// Cooldown against the median cooldown.
    pub cooldown: f64,
    /// Mass against the median mass.
    pub mass: f64,
    /// Work to make against the median work.
    pub work: f64,
    /// Armor penetration against the implied penetration of the damage (1 leaves it implied).
    pub ap: f64,
    /// Ticks between burst shots against the median of the burst weapons.
    pub ticks: f64,
    /// Accuracy at touch, short, medium and long range against the medians.
    pub accuracy: [f64; 4],
    /// The tools of the weapon when it is used as a club.
    pub bash: Vec<BashTool>,
}

/// One tool of a melee archetype.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolShape {
    /// The tool label.
    pub label: String,
    /// The capacity def names; each is one attack.
    pub capacities: Vec<String>,
    /// Power against the median swing damage of the install.
    pub power: f64,
    /// Cooldown against the median cooldown of the install.
    pub cooldown: f64,
    /// Armor penetration against the implied penetration (absent leaves it implied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap_factor: Option<f64>,
}

/// The shape of a melee archetype.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeleeShape {
    /// Mass against the median mass.
    pub mass: f64,
    /// Work against the median work.
    pub work: f64,
    /// Stuff count against the median stuff count.
    pub stuff_count: f64,
    /// Overall tempo against the median cooldown.
    pub cooldown: f64,
    /// The attack tools.
    pub tools: Vec<ToolShape>,
}

/// One archetype: a weapon type of a family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Archetype {
    /// The id inside the family (`assault`).
    pub id: String,
    /// The label shown to the user.
    pub label: String,
    /// One line for the UI.
    pub summary: String,
    /// Role names of the install's pool this archetype belongs to, best first (matched at run time).
    pub role_hints: Vec<String>,
    /// The tech level name used when the user names none.
    pub tier_hint: String,
    /// The allowed gun actions (ranged only).
    #[serde(default)]
    pub actions: Vec<String>,
    /// The default action (ranged only).
    #[serde(default)]
    pub default_action: Option<String>,
    /// The allowed rate of fire classes.
    pub rof_classes: Vec<String>,
    /// The default rate of fire class.
    pub default_rof: String,
    /// The typical rounds per minute of the archetype, the reference of a numeric rate of fire.
    #[serde(default)]
    pub ref_rpm: Option<f64>,
    /// The allowed calibre classes (ranged only).
    #[serde(default)]
    pub calibres: Vec<String>,
    /// The default calibre class (ranged only).
    #[serde(default)]
    pub default_calibre: Option<String>,
    /// The allowed handling classes.
    pub handlings: Vec<String>,
    /// The default handling class.
    pub default_handling: String,
    /// Names of the Combat Extended ammo families that fit (matched against the install's catalogue).
    #[serde(default)]
    pub ammo_families: Vec<String>,
    /// Fragments of Combat Extended AI class tag names (matched against the install's tags).
    #[serde(default)]
    pub ai_class_hints: Vec<String>,
    /// Weapon class def names the weapon should carry, when the install has them.
    #[serde(default)]
    pub weapon_class_hints: Vec<String>,
    /// Stuff category def names a melee weapon may be made of.
    #[serde(default)]
    pub stuff_categories: Vec<String>,
    /// The shift of the strength target in natural log units when no role of the pool matches.
    #[serde(default)]
    pub strength_shift: f64,
    /// The burst counts (ranged only).
    #[serde(default)]
    pub burst: Option<BurstCounts>,
    /// The ranged shape.
    #[serde(default)]
    pub ranged: Option<RangedShape>,
    /// The melee shape.
    #[serde(default)]
    pub melee: Option<MeleeShape>,
}

/// A family of archetypes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Family {
    /// The family id (`rifle`).
    pub id: String,
    /// The label shown to the user.
    pub label: String,
    /// The archetypes.
    pub archetypes: Vec<Archetype>,
}

/// One condition of a derive rule: a stat inside an optional closed range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condition {
    /// The stat name (see [`crate::archetype::derive`]).
    pub stat: String,
    /// The lower bound, when there is one.
    #[serde(default)]
    pub min: Option<f64>,
    /// The upper bound, when there is one.
    #[serde(default)]
    pub max: Option<f64>,
}

/// A rule that names the archetype of an existing weapon from its numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeriveRule {
    /// The archetype id (`family/id`).
    pub archetype: String,
    /// Every condition must hold.
    pub when: Vec<Condition>,
}

/// The ordered derive rules of both kinds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeriveRules {
    /// Rules for guns and bows, first match wins.
    pub ranged: Vec<DeriveRule>,
    /// Rules for melee weapons, first match wins.
    pub melee: Vec<DeriveRule>,
}

/// The whole taxonomy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Taxonomy {
    /// The format version.
    pub version: u32,
    /// A note about what the file holds.
    pub notes: String,
    /// The shared ladders.
    pub ladders: Ladders,
    /// The ranged families.
    pub ranged: Vec<Family>,
    /// The melee families.
    pub melee: Vec<Family>,
    /// The rules that name the archetype of an existing weapon.
    pub derive_rules: DeriveRules,
}

/// A reference to an archetype with its family.
#[derive(Debug, Clone, Copy)]
pub struct ArchetypeRef<'a> {
    /// The family.
    pub family: &'a Family,
    /// The archetype.
    pub archetype: &'a Archetype,
    /// The kind of the family.
    pub kind: ItemKind,
}

impl ArchetypeRef<'_> {
    /// The full id, `family/archetype`.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}/{}", self.family.id, self.archetype.id)
    }
}

impl Taxonomy {
    /// The embedded taxonomy, parsed and validated once.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] when the embedded file is broken; the unit tests keep that from
    /// happening.
    pub fn builtin() -> DesignResult<&'static Taxonomy> {
        static CELL: OnceLock<Result<Taxonomy, String>> = OnceLock::new();
        match CELL.get_or_init(|| Self::parse(BUILTIN).map_err(|e| e.to_string())) {
            Ok(t) => Ok(t),
            Err(message) => Err(DesignError::invalid("archetypes", message.clone())),
        }
    }

    /// Parses and validates a taxonomy document.
    ///
    /// # Errors
    ///
    /// [`DesignError::InvalidInput`] for malformed JSON, an unknown id, a missing shape, a non positive
    /// ratio or a derive rule that names an unknown archetype.
    pub fn parse(text: &str) -> DesignResult<Taxonomy> {
        let taxonomy: Taxonomy = serde_json::from_str(text)
            .map_err(|e| DesignError::invalid("archetypes", format!("not a taxonomy: {e}")))?;
        taxonomy.validate()?;
        Ok(taxonomy)
    }

    fn validate(&self) -> DesignResult<()> {
        let bad = |message: String| Err(DesignError::invalid("archetypes", message));
        let rof: BTreeSet<&str> = self.ladders.rof.iter().map(|r| r.id.as_str()).collect();
        let actions: BTreeSet<&str> = self.ladders.actions.iter().map(|r| r.id.as_str()).collect();
        let calibres: BTreeSet<&str> = self
            .ladders
            .calibres
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        let handlings: BTreeSet<&str> = self
            .ladders
            .handlings
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !self.ladders.rof.iter().all(|r| positive(r.rate)) {
            return bad("a rate of fire class has a non positive rate".to_owned());
        }
        for (kind, families) in [
            (ItemKind::Ranged, &self.ranged),
            (ItemKind::Melee, &self.melee),
        ] {
            let mut ids = BTreeSet::new();
            for family in families {
                for a in &family.archetypes {
                    let id = format!("{}/{}", family.id, a.id);
                    if !ids.insert(id.clone()) {
                        return bad(format!("duplicate archetype {id}"));
                    }
                    if !a.rof_classes.iter().all(|r| rof.contains(r.as_str()))
                        || !rof.contains(a.default_rof.as_str())
                        || !a.rof_classes.contains(&a.default_rof)
                    {
                        return bad(format!("{id} names an unknown rate of fire class"));
                    }
                    if !a.handlings.iter().all(|h| handlings.contains(h.as_str()))
                        || !a.handlings.contains(&a.default_handling)
                    {
                        return bad(format!("{id} names an unknown handling class"));
                    }
                    match kind {
                        ItemKind::Ranged => {
                            let Some(shape) = &a.ranged else {
                                return bad(format!("{id} has no ranged shape"));
                            };
                            let numbers = [
                                shape.damage,
                                shape.range,
                                shape.warmup,
                                shape.cooldown,
                                shape.mass,
                                shape.work,
                                shape.ap,
                                shape.ticks,
                            ];
                            if !numbers
                                .iter()
                                .chain(shape.accuracy.iter())
                                .all(|v| positive(*v))
                            {
                                return bad(format!("{id} has a non positive ratio"));
                            }
                            let ok_action = a.actions.iter().all(|x| actions.contains(x.as_str()))
                                && a.default_action
                                    .as_deref()
                                    .is_some_and(|d| a.actions.iter().any(|x| x == d));
                            let ok_calibre =
                                a.calibres.iter().all(|x| calibres.contains(x.as_str()))
                                    && a.default_calibre
                                        .as_deref()
                                        .is_some_and(|d| a.calibres.iter().any(|x| x == d));
                            if !ok_action || !ok_calibre || a.burst.is_none() {
                                return bad(format!(
                                    "{id} has an unknown or missing action, calibre or burst"
                                ));
                            }
                        }
                        ItemKind::Melee => {
                            let Some(shape) = &a.melee else {
                                return bad(format!("{id} has no melee shape"));
                            };
                            let tool_ok = shape.tools.iter().all(|t| {
                                positive(t.power)
                                    && positive(t.cooldown)
                                    && !t.capacities.is_empty()
                            });
                            if shape.tools.is_empty()
                                || !tool_ok
                                || !positive(shape.mass)
                                || !positive(shape.work)
                                || !positive(shape.stuff_count)
                                || !positive(shape.cooldown)
                            {
                                return bad(format!("{id} has an unusable melee shape"));
                            }
                        }
                    }
                }
            }
            let rules = match kind {
                ItemKind::Ranged => &self.derive_rules.ranged,
                ItemKind::Melee => &self.derive_rules.melee,
            };
            for rule in rules {
                if !ids.contains(&rule.archetype) {
                    return bad(format!(
                        "a derive rule names the unknown archetype {}",
                        rule.archetype
                    ));
                }
            }
        }
        Ok(())
    }

    /// Finds an archetype by its full id `family/archetype`.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<ArchetypeRef<'_>> {
        let (family_id, local) = id.split_once('/')?;
        for (kind, families) in [
            (ItemKind::Ranged, &self.ranged),
            (ItemKind::Melee, &self.melee),
        ] {
            for family in families {
                if family.id == family_id
                    && let Some(archetype) = family.archetypes.iter().find(|a| a.id == local)
                {
                    return Some(ArchetypeRef {
                        family,
                        archetype,
                        kind,
                    });
                }
            }
        }
        None
    }

    /// Every archetype of a kind, in file order.
    #[must_use]
    pub fn archetypes(&self, kind: ItemKind) -> Vec<ArchetypeRef<'_>> {
        let families = match kind {
            ItemKind::Ranged => &self.ranged,
            ItemKind::Melee => &self.melee,
        };
        families
            .iter()
            .flat_map(|family| {
                family.archetypes.iter().map(move |archetype| ArchetypeRef {
                    family,
                    archetype,
                    kind,
                })
            })
            .collect()
    }

    /// A rate of fire class by id.
    #[must_use]
    pub fn rof_class(&self, id: &str) -> Option<&RofClass> {
        self.ladders.rof.iter().find(|r| r.id == id)
    }

    /// A gun action by id.
    #[must_use]
    pub fn action(&self, id: &str) -> Option<&ActionProfile> {
        self.ladders.actions.iter().find(|r| r.id == id)
    }

    /// A calibre class by id.
    #[must_use]
    pub fn calibre(&self, id: &str) -> Option<&CalibreClass> {
        self.ladders.calibres.iter().find(|r| r.id == id)
    }

    /// A handling class by id.
    #[must_use]
    pub fn handling(&self, id: &str) -> Option<&HandlingClass> {
        self.ladders.handlings.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_taxonomy_parses_and_validates() {
        let t = Taxonomy::builtin().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(t.version, 1);
        assert!(t.archetypes(ItemKind::Ranged).len() >= 20);
        assert!(t.archetypes(ItemKind::Melee).len() >= 8);
    }

    #[test]
    fn every_family_of_the_request_exists() {
        let t = Taxonomy::builtin().unwrap_or_else(|e| panic!("{e}"));
        for id in [
            "pistol/light",
            "pistol/revolver",
            "pistol/heavy",
            "pistol/machine",
            "smg/light",
            "smg/heavy",
            "smg/pdw",
            "shotgun/pump",
            "shotgun/semi",
            "shotgun/auto",
            "rifle/carbine",
            "rifle/assault",
            "rifle/battle",
            "rifle/bolt",
            "rifle/dmr",
            "rifle/sniper",
            "rifle/anti-materiel",
            "machine-gun/lmg",
            "machine-gun/gpmg",
            "machine-gun/heavy",
            "bow/short",
            "bow/recurve",
            "bow/great",
            "bow/crossbow",
            "knife/knife",
            "sword/short",
            "sword/long",
            "axe/axe",
            "mace-hammer/mace",
            "mace-hammer/hammer",
            "spear/spear",
            "spear/polearm",
            "club-staff/club",
            "club-staff/staff",
        ] {
            assert!(t.find(id).is_some(), "{id}");
        }
        assert!(t.find("rifle").is_none());
        assert!(t.find("rifle/nothing").is_none());
    }

    #[test]
    fn a_broken_document_is_refused_with_the_reason() {
        let text = BUILTIN.replace("\"defaultRof\": \"medium\"", "\"defaultRof\": \"warp\"");
        let err = Taxonomy::parse(&text).err().map(|e| e.to_string());
        assert!(err.is_some_and(|m| m.contains("rate of fire")));
        assert!(Taxonomy::parse("{}").is_err());
    }

    #[test]
    fn the_file_holds_no_game_values_but_ratios() {
        // Every ratio of the ranged shapes is a modest multiplier: a game value such as a range of 30 or a
        // warmup of 2 seconds would not fit the bound.
        let t = Taxonomy::builtin().unwrap_or_else(|e| panic!("{e}"));
        for a in t.archetypes(ItemKind::Ranged) {
            if let Some(s) = &a.archetype.ranged {
                for v in [
                    s.damage, s.range, s.warmup, s.cooldown, s.mass, s.work, s.ap, s.ticks,
                ] {
                    assert!(v > 0.0 && v <= 6.0, "{}: {v}", a.id());
                }
            }
        }
    }
}
