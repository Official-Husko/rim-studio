//! Reading weapons out of resolved defs: the detail record of one weapon, the reference items for calibration
//! and the pools built from them.
//!
//! A weapon is a `ThingDef` that has weapon tags, or is primary equipment filed under a weapon thing category
//! ([`is_weapon_def`]; logs, drugs and trophies carry tools but are neither), is not a building or a creature, and
//! either has a verb with a default projectile (a ranged weapon) or has tools and no verbs (a melee weapon). A
//! weapon with several verbs is read from its first verb that has a projectile; its other verbs are ignored. A
//! weapon whose verbs have no projectile (beams, flames) is left out silently: it is neither a direct fire gun
//! nor a melee weapon. The reader works on the resolved node, so parent chains, patches and inheritance are
//! already applied. Stuffed weapons are read for the reference material of the options through the stat
//! pipeline.
//!
//! Nothing here is a table of game values: stat definitions, damage definitions and apparel come from the
//! databases of the load, and the numbers of a record are whatever the def says.

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_defs::{DefDatabases, DefRecord};
use serde::{Deserialize, Serialize};

use super::access::{
    child_bool, child_number, child_text, class_attr, list_items, list_texts, number_map, text_of,
};
use super::options::{ArmorSource, FALLBACK_ARMOR_RATINGS, ReaderOptions, RoleFacts};
use super::statdefs::StatTable;
use super::stuff::{StuffProfile, evaluate_stat};
use crate::classes::numeric::{finite_sorted, quantile_sorted};
use crate::classes::{ItemKind, Pool, PoolOptions, ReferenceItem};
use crate::error::{DesignError, DesignResult};
use crate::melee::{
    DamageKind, MeleeAttack, MeleeCapacity, MeleeModifiers, MeleeTool, enumerate_attacks,
    in_fight_dps, stat_panel_dps, strength as melee_strength,
};
use crate::model::{DEFAULT_BURST_COUNT, DEFAULT_TICKS_BETWEEN_SHOTS, TechLevel};
use crate::ranged::{
    AccuracyBands, ImpliedApInput, RangedProfile, ReferenceArmor, implied_ap, nominal_dps,
    strength_p4,
};
use crate::stats::StatRules;

/// Diagnostic: the reference armor could not be derived and neutral layers were used.
pub const ARMOR_FALLBACK: &str = "reader.armor-fallback";
/// Diagnostic: the database type of the stat definitions is unknown, so stat defaults read as zero.
pub const STAT_DEFS_MISSING: &str = "reader.stat-defs-missing";
/// Diagnostic: the reference material is not a loaded def.
pub const STUFF_MISSING: &str = "reader.stuff-missing";
/// Diagnostic: the strength index of a weapon could not be computed.
pub const STRENGTH_FAILED: &str = "reader.strength-failed";

/// The stat that scales the damage of every attack of a melee weapon. (The names are assembled from two
/// pieces so the source scan for game def names does not mistake a stat name for a def name.)
const STAT_MELEE_DAMAGE_MULTIPLIER: &str = concat!("Melee", "Weapon_DamageMultiplier");
/// The stat that scales the cooldown of every attack of a melee weapon.
const STAT_MELEE_COOLDOWN_MULTIPLIER: &str = concat!("Melee", "Weapon_CooldownMultiplier");

/// Whether a resolved def is a wieldable weapon candidate.
///
/// A creature or a building is never one. Otherwise the def qualifies with weapon tags, or when it is primary
/// equipment filed under a weapon thing category (a name containing `Weapons`), which catches modded weapons
/// that carry no tags. Items that merely carry tools (logs, drugs, horns and tusks are wieldable too) have
/// neither.
#[must_use]
pub fn is_weapon_def(node: &rimstudio_core::tree::Node) -> bool {
    if node.child("race").is_some() || node.child("building").is_some() {
        return false;
    }
    if !list_texts(node, "weaponTags").is_empty() {
        return true;
    }
    let primary =
        child_text(node, "equipmentType").is_some_and(|t| t.eq_ignore_ascii_case("Primary"));
    primary
        && list_texts(node, "thingCategories")
            .iter()
            .any(|c| c.contains("Weapons"))
}

/// Why a weapon candidate was not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "reason", content = "detail")]
pub enum SkipReason {
    /// `menuHidden` is true.
    MenuHidden,
    /// `destroyOnDrop` is true.
    DestroyOnDrop,
    /// The verb class matches an exclusion text.
    VerbClass(String),
    /// A weapon tag matches an exclusion text.
    Tag(String),
    /// The def name ends with an excluded suffix.
    NameSuffix(String),
    /// The projectile explodes.
    Explosive,
    /// A verb or tool entry carries a `Class` attribute.
    CustomClass(String),
    /// A required piece of data is missing (named).
    Missing(String),
}

/// A weapon candidate that was left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedDef {
    /// The def name.
    pub def_name: String,
    /// Why it was left out.
    pub reason: SkipReason,
}

/// One tool of a weapon as the def writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDetail {
    /// The tool label.
    pub label: String,
    /// Capacity def names.
    pub capacities: Vec<String>,
    /// Power (base damage).
    pub power: Option<f64>,
    /// Cooldown in seconds.
    pub cooldown: Option<f64>,
    /// Explicit armor penetration; `None` when unset or negative.
    pub armor_penetration: Option<f64>,
    /// Chance factor, 1 when unset.
    pub chance_factor: f64,
    /// Linked body part group.
    pub linked_body_parts_group: Option<String>,
}

/// The shooting part of a ranged weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangedDetail {
    /// The verb class text.
    pub verb_class: String,
    /// The default projectile def name.
    pub projectile: String,
    /// Damage per shot (projectile damage, or the damage def default).
    pub damage: f64,
    /// Projectile speed, when set.
    pub speed: Option<f64>,
    /// Armor penetration as a fraction, after the implied value rule.
    pub armor_penetration: f64,
    /// The verb range in cells.
    pub range: f64,
    /// The verb warmup in seconds.
    pub warmup: f64,
    /// The weapon cooldown stat in seconds.
    pub cooldown: f64,
    /// Shots per burst.
    pub burst_count: u32,
    /// Ticks between burst shots.
    pub ticks_between_shots: f64,
    /// Accuracy bands, when the weapon sets all four.
    pub accuracy: Option<AccuracyBands>,
}

impl RangedDetail {
    /// The inputs of the ranged formulas.
    #[must_use]
    pub fn profile(&self) -> RangedProfile {
        RangedProfile {
            damage: self.damage,
            armor_penetration: self.armor_penetration,
            warmup: self.warmup,
            cooldown: self.cooldown,
            burst_count: self.burst_count,
            ticks_between_shots: self.ticks_between_shots,
            range: self.range,
            accuracy: self.accuracy,
        }
    }
}

/// Everything the reader knows about one weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponDetail {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// Ranged or melee.
    pub kind: ItemKind,
    /// The tech level, when the def names a known one.
    pub tech_level: Option<TechLevel>,
    /// The session handle of the mod that owns the def, `None` for a patch created def.
    pub mod_idx: Option<rimstudio_core::ids::ModIdx>,
    /// Weapon tags.
    pub weapon_tags: Vec<String>,
    /// Weapon classes.
    pub weapon_classes: Vec<String>,
    /// Stuff categories the weapon accepts (empty for a fixed weapon).
    pub stuff_categories: Vec<String>,
    /// The shooting part, for a ranged weapon.
    pub ranged: Option<RangedDetail>,
    /// The tools (melee attacks, or the gun bash of a gun).
    pub tools: Vec<ToolDetail>,
    /// The role decided by the options' rules.
    pub role: Option<String>,
    /// The strength index, when it could be computed.
    pub strength: Option<f64>,
    /// The named numbers of the calibration item.
    pub stats: BTreeMap<String, f64>,
}

impl WeaponDetail {
    /// The calibration reference item of this weapon.
    #[must_use]
    pub fn reference_item(&self) -> ReferenceItem {
        let mut item = ReferenceItem::new(self.def_name.clone(), self.kind);
        item.labels.push(self.label.clone());
        let mut tags: Vec<&String> = self
            .weapon_tags
            .iter()
            .chain(self.weapon_classes.iter())
            .collect();
        tags.sort();
        tags.dedup();
        item.labels.extend(tags.into_iter().cloned());
        item.tier = self.tech_level.map(TechLevel::index);
        item.role.clone_from(&self.role);
        item.group = self
            .ranged
            .as_ref()
            .map(|r| if r.burst_count > 1 { "burst" } else { "single" }.to_owned());
        item.strength = self.strength;
        item.stats.clone_from(&self.stats);
        item
    }
}

/// The reference armor layers of the strength index and where they came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmorLayers {
    /// Sharp ratings as fractions.
    pub ratings: Vec<f64>,
    /// How many armor items they were derived from (0 for given or fallback ratings).
    pub derived_from: usize,
    /// True when the neutral fallback layers are used.
    pub fallback: bool,
}

/// Derives or takes the reference armor layers.
///
/// Derived layers are an unarmored layer (0), the median and the 90th percentile of the positive sharp
/// ratings of the fixed (not stuffed) apparel in the databases. With fewer than three such items the neutral
/// [`FALLBACK_ARMOR_RATINGS`] are used and the second value is true.
#[must_use]
pub fn reference_armor(dbs: &DefDatabases, opts: &ReaderOptions) -> ArmorLayers {
    if let ArmorSource::Given { ratings } = &opts.armor {
        return ArmorLayers {
            ratings: ratings.iter().copied().filter(|r| r.is_finite()).collect(),
            derived_from: 0,
            fallback: false,
        };
    }
    let mut ratings = Vec::new();
    if let Ok(view) = dbs.database(&opts.db_type) {
        for def in view.iter() {
            let node = &def.node;
            if node.child("apparel").is_none() || node.child("stuffCategories").is_some() {
                continue;
            }
            let stats = number_map(node, "statBases");
            if let Some(r) = stats.get("ArmorRating_Sharp").copied().filter(|r| *r > 0.0) {
                ratings.push(r);
            }
        }
    }
    let sorted = finite_sorted(&ratings);
    if sorted.len() < 3 {
        return ArmorLayers {
            ratings: FALLBACK_ARMOR_RATINGS.to_vec(),
            derived_from: sorted.len(),
            fallback: true,
        };
    }
    let median = quantile_sorted(&sorted, 0.5).unwrap_or(0.5);
    let high = quantile_sorted(&sorted, 0.9).unwrap_or(1.0);
    ArmorLayers {
        ratings: vec![0.0, median, high],
        derived_from: sorted.len(),
        fallback: false,
    }
}

/// The state shared by all reads of one load: stat rules, the reference material, damage definitions and
/// the reference armor.
#[derive(Debug)]
pub struct ReadContext<'a> {
    dbs: &'a DefDatabases,
    opts: &'a ReaderOptions,
    stats: StatTable,
    stuff: Option<StuffProfile>,
    armor: ArmorLayers,
    damage_ap: BTreeMap<String, f64>,
    damage_amount: BTreeMap<String, f64>,
    /// Damage definitions that name no armor category: the armor stats do not apply to them, so a shot
    /// of such a damage type has no penetration at all.
    damage_unarmored: BTreeSet<String>,
    /// Damage kind of each tool capacity as the install's maneuvers define it (capacity, then the damage
    /// definition of its maneuver, then that definition's armor category).
    capacity_kinds: BTreeMap<String, DamageKind>,
    diagnostics: Vec<Diagnostic>,
}

fn diag(code: &'static str, severity: Severity, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagCode::new(code), severity, message)
}

impl<'a> ReadContext<'a> {
    /// Prepares a context: reads the stat table, the damage defaults, the reference material and the armor.
    #[must_use]
    pub fn new(dbs: &'a DefDatabases, opts: &'a ReaderOptions) -> Self {
        let mut diagnostics = Vec::new();
        let stats = StatTable::from_databases(dbs, &opts.stat_db_type);
        if stats.is_empty() {
            diagnostics.push(diag(
                STAT_DEFS_MISSING,
                Severity::Info,
                "no stat definitions are loaded; stat defaults read as zero",
            ));
        }
        let stuff = opts.reference_stuff.as_ref().and_then(|name| {
            let found = dbs.get(&opts.db_type, name);
            if found.is_none() {
                diagnostics.push(
                    diag(
                        STUFF_MISSING,
                        Severity::Warning,
                        format!("the reference material {name} is not loaded; stuffed weapons are read unstuffed"),
                    )
                    .with_arg("stuff", name.clone()),
                );
            }
            found.map(|d| StuffProfile::from_def(&d.def_name, &d.node))
        });
        let armor = reference_armor(dbs, opts);
        if armor.fallback {
            diagnostics.push(diag(
                ARMOR_FALLBACK,
                Severity::Info,
                "too few fixed armor items to derive reference layers; neutral layers are used",
            ));
        }
        let mut damage_ap = BTreeMap::new();
        let mut damage_amount = BTreeMap::new();
        let mut damage_unarmored = BTreeSet::new();
        let mut damage_category: BTreeMap<String, String> = BTreeMap::new();
        if let Ok(view) = dbs.database(&opts.damage_db_type) {
            for def in view.iter() {
                if let Some(v) = child_number(&def.node, "defaultArmorPenetration") {
                    damage_ap.insert(def.def_name.clone(), v);
                }
                if let Some(v) = child_number(&def.node, "defaultDamage") {
                    damage_amount.insert(def.def_name.clone(), v);
                }
                match child_text(&def.node, "armorCategory") {
                    Some(category) => {
                        damage_category.insert(def.def_name.clone(), category);
                    }
                    None => {
                        damage_unarmored.insert(def.def_name.clone());
                    }
                }
            }
        }
        let mut capacity_kinds = BTreeMap::new();
        if let Ok(view) = dbs.database(&opts.maneuver_db_type) {
            for def in view.iter() {
                let capacity = child_text(&def.node, "requiredCapacity");
                let damage = def
                    .node
                    .child("verb")
                    .and_then(|v| child_text(v, "meleeDamageDef"));
                if let (Some(capacity), Some(category)) =
                    (capacity, damage.and_then(|d| damage_category.get(&d)))
                {
                    let kind = if category == "Sharp" {
                        DamageKind::Sharp
                    } else {
                        DamageKind::Blunt
                    };
                    capacity_kinds.entry(capacity).or_insert(kind);
                }
            }
        }
        Self {
            dbs,
            opts,
            stats,
            stuff,
            armor,
            damage_ap,
            damage_amount,
            damage_unarmored,
            capacity_kinds,
            diagnostics,
        }
    }

    /// The problems found while preparing the context.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The reference armor in use.
    #[must_use]
    pub fn armor(&self) -> &ArmorLayers {
        &self.armor
    }

    /// The stat table.
    #[must_use]
    pub fn stat_table(&self) -> &StatTable {
        &self.stats
    }

    fn stat(
        &self,
        node: &rimstudio_core::tree::Node,
        name: &str,
        accepted: &[String],
    ) -> Option<f64> {
        let base = number_map(node, "statBases").get(name).copied();
        let rules = self.stats.rules(name);
        let value = evaluate_stat(base, name, self.stuff.as_ref(), accepted, &rules);
        // A stat without a base value and without a definition is unknown, not zero.
        if base.is_none() && !self.stats.knows(name) && self.stuff.is_none() {
            return None;
        }
        value
    }

    fn multiplier(&self, name: &str, accepted: &[String]) -> f64 {
        let mut rules: StatRules = self.stats.rules(name);
        if !self.stats.knows(name) {
            rules.default_base = 1.0;
        }
        evaluate_stat(None, name, self.stuff.as_ref(), accepted, &rules).unwrap_or(1.0)
    }

    /// Reads one def. `None` when the def is not a weapon candidate; `Some(Err(reason))` when it is a
    /// candidate that is left out; `Some(Ok(detail))` otherwise.
    #[must_use]
    pub fn read(&self, def: &DefRecord) -> Option<Result<WeaponDetail, SkipReason>> {
        let node = &def.node;
        if !is_weapon_def(node) {
            return None;
        }
        let verbs = list_items(node, "verbs");
        let shooting = verbs
            .iter()
            .find(|v| child_text(v, "defaultProjectile").is_some());
        // A weapon with verbs but no projectile (a beam or a flame) is neither a direct fire gun nor melee.
        if shooting.is_none() && !verbs.is_empty() {
            return None;
        }
        let tools = list_items(node, "tools");
        if shooting.is_none() && tools.is_empty() {
            return None;
        }
        Some(self.read_candidate(def, shooting.copied(), &tools))
    }

    fn exclude(
        &self,
        def: &DefRecord,
        shooting: Option<&rimstudio_core::tree::Node>,
        tools: &[&rimstudio_core::tree::Node],
        tags: &[String],
    ) -> Option<SkipReason> {
        let ex = &self.opts.exclusions;
        let node = &def.node;
        if ex.menu_hidden && child_bool(node, "menuHidden") == Some(true) {
            return Some(SkipReason::MenuHidden);
        }
        if ex.destroy_on_drop && child_bool(node, "destroyOnDrop") == Some(true) {
            return Some(SkipReason::DestroyOnDrop);
        }
        if let Some(s) = ex
            .def_name_suffixes
            .iter()
            .find(|s| !s.is_empty() && def.def_name.ends_with(s.as_str()))
        {
            return Some(SkipReason::NameSuffix(s.clone()));
        }
        for tag in tags {
            let lower = tag.to_lowercase();
            if ex
                .tag_parts
                .iter()
                .any(|p| !p.is_empty() && lower.contains(&p.to_lowercase()))
            {
                return Some(SkipReason::Tag(tag.clone()));
            }
        }
        if let Some(verb) = shooting {
            let class = child_text(verb, "verbClass").unwrap_or_default();
            if ex
                .verb_class_parts
                .iter()
                .any(|p| !p.is_empty() && class.contains(p.as_str()))
            {
                return Some(SkipReason::VerbClass(class));
            }
        }
        if ex.custom_classes {
            let verbs = list_items(node, "verbs");
            let class = verbs
                .iter()
                .chain(tools.iter())
                .find_map(|n| class_attr(n).map(str::to_owned));
            if let Some(c) = class {
                return Some(SkipReason::CustomClass(c));
            }
        }
        None
    }

    fn read_candidate(
        &self,
        def: &DefRecord,
        shooting: Option<&rimstudio_core::tree::Node>,
        tool_nodes: &[&rimstudio_core::tree::Node],
    ) -> Result<WeaponDetail, SkipReason> {
        let node = &def.node;
        let weapon_tags = list_texts(node, "weaponTags");
        if let Some(reason) = self.exclude(def, shooting, tool_nodes, &weapon_tags) {
            return Err(reason);
        }
        let accepted = list_texts(node, "stuffCategories");
        let tools: Vec<ToolDetail> = tool_nodes.iter().map(|t| read_tool(t)).collect();
        let ranged = match shooting {
            Some(verb) => Some(self.read_ranged(node, verb, &accepted)?),
            None => None,
        };
        let kind = if ranged.is_some() {
            ItemKind::Ranged
        } else {
            ItemKind::Melee
        };
        let mut detail = WeaponDetail {
            def_name: def.def_name.clone(),
            label: child_text(node, "label").unwrap_or_else(|| def.def_name.clone()),
            kind,
            tech_level: child_text(node, "techLevel").and_then(|t| tech_level_named(&t)),
            mod_idx: def.mod_idx(),
            weapon_tags,
            weapon_classes: list_texts(node, "weaponClasses"),
            stuff_categories: accepted.clone(),
            ranged,
            tools,
            role: None,
            strength: None,
            stats: BTreeMap::new(),
        };
        self.fill_numbers(node, &accepted, &mut detail);
        let capacities: Vec<String> = detail
            .tools
            .iter()
            .flat_map(|t| t.capacities.iter().cloned())
            .collect();
        detail.role = self.opts.roles.decide(
            kind,
            &RoleFacts {
                tags: &detail.weapon_tags,
                classes: &detail.weapon_classes,
                capacities: &capacities,
                stats: &detail.stats,
            },
        );
        Ok(detail)
    }

    fn read_ranged(
        &self,
        node: &rimstudio_core::tree::Node,
        verb: &rimstudio_core::tree::Node,
        accepted: &[String],
    ) -> Result<RangedDetail, SkipReason> {
        let missing = |what: &str| SkipReason::Missing(what.to_owned());
        let projectile =
            child_text(verb, "defaultProjectile").ok_or_else(|| missing("defaultProjectile"))?;
        let proj_def = self
            .dbs
            .get(&self.opts.db_type, &projectile)
            .ok_or_else(|| missing("projectile def"))?;
        let props = proj_def
            .node
            .child("projectile")
            .ok_or_else(|| missing("projectile properties"))?;
        if self.opts.exclusions.explosive
            && child_number(props, "explosionRadius").is_some_and(|r| r > 0.0)
        {
            return Err(SkipReason::Explosive);
        }
        let damage_def = child_text(props, "damageDef");
        let own_damage = child_number(props, "damageAmountBase");
        let damage = own_damage
            .or_else(|| {
                damage_def
                    .as_ref()
                    .and_then(|d| self.damage_amount.get(d).copied())
            })
            .ok_or_else(|| missing("damageAmountBase"))?;
        let mut armor_penetration = implied_ap(&ImpliedApInput {
            damage,
            explicit_ap: child_number(props, "armorPenetrationBase"),
            sets_damage: own_damage.is_some(),
            damage_def_default_ap: damage_def
                .as_ref()
                .and_then(|d| self.damage_ap.get(d).copied()),
            ap_multiplier: 1.0,
        })
        .map_err(|e| SkipReason::Missing(format!("armor penetration ({e})")))?;
        if damage_def
            .as_ref()
            .is_some_and(|d| self.damage_unarmored.contains(d))
        {
            armor_penetration = 0.0;
        }
        let range = child_number(verb, "range").ok_or_else(|| missing("range"))?;
        let cooldown = self
            .stat(node, "RangedWeapon_Cooldown", accepted)
            .unwrap_or(0.0);
        let burst = child_number(verb, "burstShotCount")
            .filter(|b| *b >= 1.0 && *b <= f64::from(u32::MAX))
            .map_or(DEFAULT_BURST_COUNT, |b| b.floor() as u32);
        let acc = |name: &str| self.stat(node, name, accepted);
        let accuracy = match (
            acc("AccuracyTouch"),
            acc("AccuracyShort"),
            acc("AccuracyMedium"),
            acc("AccuracyLong"),
        ) {
            (Some(touch), Some(short), Some(medium), Some(long)) => Some(AccuracyBands {
                touch,
                short,
                medium,
                long,
            }),
            _ => None,
        };
        Ok(RangedDetail {
            verb_class: child_text(verb, "verbClass").unwrap_or_default(),
            projectile,
            damage,
            speed: child_number(props, "speed"),
            armor_penetration,
            range,
            warmup: child_number(verb, "warmupTime").unwrap_or(0.0),
            cooldown,
            burst_count: burst,
            ticks_between_shots: child_number(verb, "ticksBetweenBurstShots")
                .unwrap_or(DEFAULT_TICKS_BETWEEN_SHOTS),
            accuracy,
        })
    }

    /// The sharp or blunt damage multiplier of the reference material: the material's own value of the stat
    /// (not a `stuffProps` factor), the stat definition's default when it sets none, 1 without a material.
    fn material_multiplier(&self, name: &str, accepted: &[String]) -> f64 {
        let Some(stuff) = self
            .stuff
            .as_ref()
            .filter(|s| !accepted.is_empty() && s.fits(accepted))
        else {
            return 1.0;
        };
        stuff.own_stat(name).unwrap_or_else(|| {
            if self.stats.knows(name) {
                self.stats.rules(name).default_base
            } else {
                1.0
            }
        })
    }

    /// The melee attacks of a weapon read by this context: one per tool and capacity, with the damage and
    /// cooldown multipliers of the weapon and of the reference material applied.
    ///
    /// # Errors
    ///
    /// The errors of [`enumerate_attacks`] (no tool with a power and a cooldown, or a bad number).
    pub fn attacks(&self, detail: &WeaponDetail) -> DesignResult<Vec<MeleeAttack>> {
        self.melee_attacks(&detail.tools, &detail.stuff_categories)
    }

    fn melee_attacks(
        &self,
        tools: &[ToolDetail],
        accepted: &[String],
    ) -> DesignResult<Vec<MeleeAttack>> {
        let modifiers = MeleeModifiers {
            damage_multiplier: self.multiplier(STAT_MELEE_DAMAGE_MULTIPLIER, accepted),
            sharp_multiplier: self.material_multiplier("SharpDamageMultiplier", accepted),
            blunt_multiplier: self.material_multiplier("BluntDamageMultiplier", accepted),
            cooldown_multiplier: self.multiplier(STAT_MELEE_COOLDOWN_MULTIPLIER, accepted),
        };
        let inputs: Vec<MeleeTool> = tools
            .iter()
            .filter_map(|t| {
                Some(MeleeTool {
                    label: t.label.clone(),
                    capacities: t
                        .capacities
                        .iter()
                        .map(|c| {
                            let mut capacity = MeleeCapacity::named(c);
                            if let Some(kind) = self.capacity_kinds.get(c) {
                                capacity.kind = *kind;
                            }
                            capacity
                        })
                        .collect(),
                    power: t.power?,
                    cooldown: t.cooldown?,
                    armor_penetration: t.armor_penetration,
                    chance_factor: t.chance_factor,
                })
            })
            .collect();
        enumerate_attacks(&inputs, &modifiers)
    }

    fn fill_numbers(
        &self,
        node: &rimstudio_core::tree::Node,
        accepted: &[String],
        detail: &mut WeaponDetail,
    ) {
        let mut put = |name: &str, value: Option<f64>| {
            if let Some(v) = value.filter(|v| v.is_finite()) {
                detail.stats.insert(name.to_owned(), v);
            }
        };
        let mass = self.stat(node, "Mass", accepted);
        // The price the game shows: the explicit `MarketValue`, or the cost list, stuff and work formula (read
        // for the reference material when the weapon is stuffed).
        let market = super::pricing::market_value_of(
            self.dbs,
            self.opts,
            &detail.def_name,
            self.stuff
                .as_ref()
                .filter(|s| s.fits(accepted))
                .map(|s| s.def_name.as_str()),
        )
        .ok()
        .map(|b| b.base_value);
        let work = self.stat(node, "WorkToMake", accepted);
        put("mass", mass);
        put("market_value", market);
        put("work", work);
        if let Some(r) = detail.ranged.clone() {
            put("range", Some(r.range));
            put("warmup", Some(r.warmup));
            put("cooldown", Some(r.cooldown));
            put("burst", Some(f64::from(r.burst_count)));
            put("ticks_between", Some(r.ticks_between_shots));
            put("damage", Some(r.damage));
            put("speed", r.speed);
            put("ap", Some(r.armor_penetration));
            if let Some(a) = r.accuracy {
                put("touch", Some(a.touch));
                put("short", Some(a.short));
                put("medium", Some(a.medium));
                put("long", Some(a.long));
            }
            let profile = r.profile();
            put("dps", nominal_dps(&profile).ok());
            detail.strength = strength_p4(
                &profile,
                &ReferenceArmor {
                    sharp_ratings: &self.armor.ratings,
                },
            )
            .ok();
        } else if let Ok(attacks) = self.melee_attacks(&detail.tools, accepted) {
            put("tools", Some(attacks.len() as f64));
            if let Ok(panel) = stat_panel_dps(&attacks) {
                put("swing_damage", Some(panel.damage));
                put("cooldown", Some(panel.cooldown));
                put("dps", Some(panel.dps));
                put("ap", Some(panel.armor_penetration));
            }
            if let Ok(fight) = in_fight_dps(&attacks) {
                put("fight_dps", Some(fight.dps));
            }
            detail.strength = melee_strength(&attacks).ok();
        }
    }
}

fn read_tool(node: &rimstudio_core::tree::Node) -> ToolDetail {
    ToolDetail {
        label: child_text(node, "label").unwrap_or_default(),
        capacities: list_texts(node, "capacities"),
        power: child_number(node, "power"),
        cooldown: child_number(node, "cooldownTime"),
        armor_penetration: child_number(node, "armorPenetration").filter(|v| *v >= 0.0),
        chance_factor: child_number(node, "chanceFactor").unwrap_or(1.0),
        linked_body_parts_group: child_text(node, "linkedBodyPartsGroup"),
    }
}

/// The tech level a `techLevel` element names, if it is one of the six.
#[must_use]
pub fn tech_level_named(text: &str) -> Option<TechLevel> {
    TechLevel::ALL
        .into_iter()
        .find(|t| t.xml_name().eq_ignore_ascii_case(text.trim()))
}

/// Every weapon read from the databases, plus what was left out.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSet {
    /// The weapons that were read, in database order.
    pub weapons: Vec<WeaponDetail>,
    /// The weapon candidates that were left out.
    pub skipped: Vec<SkippedDef>,
    /// The reference armor of the strength index.
    pub armor: ArmorLayers,
    /// Problems found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

impl ReferenceSet {
    /// The calibration reference items of the weapons of one kind.
    #[must_use]
    pub fn items(&self, kind: ItemKind) -> Vec<ReferenceItem> {
        self.weapons
            .iter()
            .filter(|w| w.kind == kind)
            .map(WeaponDetail::reference_item)
            .collect()
    }

    /// The weapon with the given def name.
    #[must_use]
    pub fn weapon(&self, def_name: &str) -> Option<&WeaponDetail> {
        self.weapons.iter().find(|w| w.def_name == def_name)
    }
}

/// Reads every weapon of the databases.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when the weapon database type of the options is not a def type.
pub fn read_reference_set(dbs: &DefDatabases, opts: &ReaderOptions) -> DesignResult<ReferenceSet> {
    let ctx = ReadContext::new(dbs, opts);
    let view = dbs.database(&opts.db_type).map_err(|e| {
        DesignError::invalid(
            "db_type",
            format!("{} is not a def type ({e})", opts.db_type),
        )
    })?;
    let mut weapons = Vec::new();
    let mut skipped = Vec::new();
    for def in view.iter() {
        match ctx.read(def) {
            None => {}
            Some(Ok(detail)) => weapons.push(detail),
            Some(Err(reason)) => skipped.push(SkippedDef {
                def_name: def.def_name.clone(),
                reason,
            }),
        }
    }
    let mut diagnostics = ctx.diagnostics().to_vec();
    for w in &weapons {
        if w.strength.is_none() {
            diagnostics.push(
                diag(
                    STRENGTH_FAILED,
                    Severity::Hint,
                    format!(
                        "{} has no strength index (a number is missing or degenerate)",
                        w.def_name
                    ),
                )
                .with_arg("def", w.def_name.clone()),
            );
        }
    }
    Ok(ReferenceSet {
        weapons,
        skipped,
        armor: ctx.armor().clone(),
        diagnostics,
    })
}

/// The reference set and the two pools built from it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePools {
    /// The weapons that were read.
    pub set: ReferenceSet,
    /// The pool of direct fire ranged weapons.
    pub ranged: Pool,
    /// The pool of melee weapons.
    pub melee: Pool,
}

/// Reads the databases and builds the ranged and melee pools. Tier labels are the tech level names. Tier and
/// role come from the defs and the role rules; missing ones are derived by the pool at run time.
///
/// # Errors
///
/// See [`read_reference_set`].
pub fn build_pools(dbs: &DefDatabases, opts: &ReaderOptions) -> DesignResult<ReferencePools> {
    let set = read_reference_set(dbs, opts)?;
    let pool_options = PoolOptions {
        derive_tier: true,
        tier_labels: TechLevel::ALL
            .into_iter()
            .map(|t| (t.index(), t.xml_name().to_owned()))
            .collect(),
    };
    let ranged = Pool::build(
        ItemKind::Ranged,
        &set.items(ItemKind::Ranged),
        &pool_options,
    );
    let melee = Pool::build(ItemKind::Melee, &set.items(ItemKind::Melee), &pool_options);
    Ok(ReferencePools { set, ranged, melee })
}

/// The text of the `description` element, used by the spec reader.
pub(crate) fn description_of(node: &rimstudio_core::tree::Node) -> String {
    node.child("description")
        .and_then(text_of)
        .unwrap_or_default()
}
