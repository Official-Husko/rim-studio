//! The Combat Extended reader: what the user's reference set says about Combat Extended.
//!
//! [`read_conversions`] looks at the resolved defs of a load and returns a [`CeModel`]: whether Combat Extended
//! is part of the set, its class names, the ammo sets with their projectile numbers, the weapon tags and AI
//! class tags found on converted defs, the auto-patcher presets when the install carries them as data, the
//! converted guns and melee weapons with their numbers and, when a second vanilla load is given, their vanilla
//! twins. Everything is read at run time from the user's install; the repository holds no Combat Extended value
//! (R11). Without Combat Extended the result is [`CeModel::absent`] with the reason (IT-005, IT-060) and the
//! optional patch cannot be generated.
//!
//! Loading a set that includes Combat Extended needs two things from the caller, because the shared engine
//! does not run assemblies (decision D-020):
//!
//! * the gun conversion operation, as [`custom_registry`] (it applies the typed merge of [`makegun`] to the
//!   unified document, so converted guns resolve as the game sees them);
//! * the def classes of the Combat Extended assembly in the type table, as [`with_ce_types`] (spike S-07).
//!
//! Modules: [`names`] (class names and the conversion detector), [`makegun`] (the typed merge and the
//! custom operation), [`presets`], [`ammo`], [`conversions`] (guns, melee weapons, twins, the update mode
//! block).

pub mod ammo;
pub mod conversions;
pub mod makegun;
pub mod names;
pub mod presets;

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_defs::{DefDatabases, DefsResult, ModOrder, TypeInfo, TypeTable};
use serde::{Deserialize, Serialize};

pub use ammo::{AmmoSetInfo, AmmoType, ProjectileInfo, read_ammo_sets};
pub use conversions::{
    CeGun, CeMelee, CeToolRow, GUN_STATS, MELEE_STATS, ce_block_from_def, read_gun, read_melee,
};
pub use makegun::{
    FIND_MOD_OP, FindModOp, MakeGunOp, MakeGunRecord, MakeGunSpec, MergeReport, custom_registry,
    makegun_records, merge_into_def,
};
pub use names::{CE_PACKAGE_ID, CeClassNames, CeMarkers, detect_markers, is_conversion};
pub use presets::{read_apparel_presets, read_gun_presets};

use crate::ce::formulas::{ApparelPreset, GunPreset};
use crate::reader::options::ReaderOptions;
use crate::reader::{ReadContext, StatTable};
use crate::stats::StatRules;

/// Diagnostic: Combat Extended is not part of the reference set.
pub const CE_ABSENT: &str = "ce.absent";
/// Diagnostic: a converted def has no twin in the vanilla load.
pub const CE_TWIN_MISSING: &str = "ce.twin-missing";
/// Diagnostic: a converted gun names an ammo set that is not loaded.
pub const CE_AMMO_SET_UNRESOLVED: &str = "ce.ammo-set-unresolved";

/// Stat definitions the Combat Extended model keeps rules for, besides the stats of the converted defs.
pub const CE_STAT_NAMES: [&str; 14] = [
    "Bulk",
    "WornBulk",
    "SwayFactor",
    "ShotSpread",
    "SightsEfficiency",
    "RangedWeapon_Cooldown",
    "Mass",
    "MeleeCounterParryBonus",
    "MeleeCritChance",
    "MeleeParryChance",
    "MeleeDodgeChance",
    "ArmorRating_Sharp",
    "ArmorRating_Blunt",
    "StuffEffectMultiplierArmor",
];

/// The Combat Extended mod as the load order names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeNames {
    /// The package id as declared (the gate uses its lower case form, [`CE_PACKAGE_ID`]).
    pub package_id: String,
    /// The mod's display name from its About file (`FindMod` compares it exactly).
    pub name: String,
}

/// What the user's reference set says about Combat Extended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeModel {
    /// The reason Combat Extended is absent; `None` when it is present.
    pub absent: Option<String>,
    /// The mod entry, when the load order was given and names Combat Extended.
    pub names: Option<CeNames>,
    /// Class names, with the shoot verb class observed on the user's converted guns.
    pub classes: CeClassNames,
    /// Ammo sets with their members.
    pub ammo_sets: Vec<AmmoSetInfo>,
    /// Union of the weapon tags of converted defs, sorted.
    pub weapon_tags: Vec<String>,
    /// The tags of [`CeModel::weapon_tags`] that name an AI class, sorted.
    pub ai_class_tags: Vec<String>,
    /// The gun presets of the install, in load order.
    pub gun_presets: Vec<GunPreset>,
    /// The apparel presets of the install, in load order.
    pub apparel_presets: Vec<ApparelPreset>,
    /// Rules of the stat definitions the model uses, by stat name.
    pub stat_rules: BTreeMap<String, StatRules>,
    /// The converted guns, in database order.
    pub guns: Vec<CeGun>,
    /// The converted melee weapons, in database order.
    pub melee: Vec<CeMelee>,
    /// A def that exists only with Combat Extended, for `Conditional` probes (the first ammo set).
    pub probe_def: Option<String>,
    /// Problems found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

impl CeModel {
    /// The model of a reference set without Combat Extended.
    #[must_use]
    pub fn absent(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            absent: Some(reason.clone()),
            names: None,
            classes: CeClassNames::default(),
            ammo_sets: Vec::new(),
            weapon_tags: Vec::new(),
            ai_class_tags: Vec::new(),
            gun_presets: Vec::new(),
            apparel_presets: Vec::new(),
            stat_rules: BTreeMap::new(),
            guns: Vec::new(),
            melee: Vec::new(),
            probe_def: None,
            diagnostics: vec![Diagnostic::new(
                DiagCode::new(CE_ABSENT),
                Severity::Info,
                reason,
            )],
        }
    }

    /// True when Combat Extended is part of the reference set.
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.absent.is_none()
    }

    /// The ammo set with the given def name.
    #[must_use]
    pub fn ammo_set(&self, def_name: &str) -> Option<&AmmoSetInfo> {
        self.ammo_sets.iter().find(|a| a.def_name == def_name)
    }

    /// True when the projectile is a member of the ammo set (false when the set is unknown).
    #[must_use]
    pub fn projectile_in_set(&self, set: &str, projectile: &str) -> bool {
        self.ammo_set(set)
            .is_some_and(|s| s.has_projectile(projectile))
    }

    /// True when the tag was found on a converted def of the user's install (an unknown tag is lint rule
    /// CEP016).
    #[must_use]
    pub fn knows_tag(&self, tag: &str) -> bool {
        self.weapon_tags.iter().any(|t| t == tag)
    }

    /// The converted gun with the given def name.
    #[must_use]
    pub fn gun(&self, def_name: &str) -> Option<&CeGun> {
        self.guns.iter().find(|g| g.def_name == def_name)
    }

    /// The converted melee weapon with the given def name.
    #[must_use]
    pub fn melee_weapon(&self, def_name: &str) -> Option<&CeMelee> {
        self.melee.iter().find(|g| g.def_name == def_name)
    }

    /// Number of converted guns that have a vanilla twin.
    #[must_use]
    pub fn twin_count(&self) -> usize {
        self.guns.iter().filter(|g| g.twin.is_some()).count()
    }
}

/// What [`read_conversions_with`] can be told.
#[derive(Debug, Clone)]
pub struct CeReadOptions<'a> {
    /// The database type that holds weapons and projectiles.
    pub db_type: String,
    /// The mods of the load, to find the Combat Extended mod by package id.
    pub order: Option<&'a ModOrder>,
    /// The databases of a second load without Combat Extended, to pair converted defs with their vanilla
    /// twins.
    pub vanilla: Option<&'a DefDatabases>,
    /// Class names.
    pub classes: CeClassNames,
    /// Options of the generic reader, used for the vanilla twins and the stat definitions.
    pub reader: ReaderOptions,
}

impl Default for CeReadOptions<'_> {
    fn default() -> Self {
        Self {
            db_type: "ThingDef".into(),
            order: None,
            vanilla: None,
            classes: CeClassNames::default(),
            reader: ReaderOptions::default(),
        }
    }
}

/// Reads Combat Extended from the databases of a load, without a load order and without vanilla twins.
#[must_use]
pub fn read_conversions(dbs: &DefDatabases) -> CeModel {
    read_conversions_with(dbs, &CeReadOptions::default())
}

fn diag(code: &'static str, severity: Severity, message: String) -> Diagnostic {
    Diagnostic::new(DiagCode::new(code), severity, message)
}

/// Reads Combat Extended from the databases of a load.
///
/// Combat Extended counts as present when the load order has a mod with the package id
/// [`CE_PACKAGE_ID`] (compared ignoring case), or an ammo set def is loaded, or a weapon carries a conversion
/// marker. Otherwise the result is [`CeModel::absent`].
#[must_use]
pub fn read_conversions_with(dbs: &DefDatabases, options: &CeReadOptions<'_>) -> CeModel {
    let classes = options.classes.clone();
    let names = options.order.and_then(|order| {
        order
            .iter()
            .find(|m| m.package_id.eq_ignore_ascii_case(CE_PACKAGE_ID))
            .map(|m| CeNames {
                package_id: m.package_id.clone(),
                name: m.name.clone(),
            })
    });
    let ammo_sets = read_ammo_sets(dbs, &classes.ammo_set_def, &options.db_type);
    let mut converted_guns = Vec::new();
    let mut converted_melee = Vec::new();
    if let Ok(view) = dbs.database(&options.db_type) {
        for def in view.iter() {
            let markers = detect_markers(&def.node, &classes);
            if !markers.is_conversion() {
                continue;
            }
            if !conversions::is_item(&def.node) {
                continue;
            }
            if conversions::is_gun(&markers) {
                converted_guns.push(def);
            } else {
                converted_melee.push(def);
            }
        }
    }
    if names.is_none()
        && ammo_sets.is_empty()
        && converted_guns.is_empty()
        && converted_melee.is_empty()
    {
        return CeModel::absent(format!(
            "Combat Extended is not part of the reference set: no mod with the package id {CE_PACKAGE_ID}, no ammo set and no converted weapon"
        ));
    }
    let mut classes = classes;
    if let Some(observed) = most_common_shoot_verb(&converted_guns, &classes) {
        classes.shoot_verb = observed;
    }
    let mut diagnostics = Vec::new();
    let mut guns: Vec<CeGun> = converted_guns
        .iter()
        .map(|d| {
            read_gun(
                d,
                &classes,
                &ammo_sets,
                dbs,
                &options.db_type,
                &options.reader.exclusions,
            )
        })
        .collect();
    let mut melee: Vec<CeMelee> = converted_melee
        .iter()
        .filter_map(|d| read_melee(d, &options.reader.exclusions))
        .collect();
    for g in &guns {
        if let Some(set) = &g.ammo_set
            && !ammo_sets.iter().any(|a| &a.def_name == set)
        {
            diagnostics.push(
                diag(
                    CE_AMMO_SET_UNRESOLVED,
                    Severity::Warning,
                    format!(
                        "{} uses the ammo set {set}, which is not loaded",
                        g.def_name
                    ),
                )
                .with_arg("def", g.def_name.clone())
                .with_arg("ammoSet", set.clone()),
            );
        }
    }
    if let Some(vanilla) = options.vanilla {
        pair_twins(vanilla, options, &mut guns, &mut melee, &mut diagnostics);
    }
    let mut tags: BTreeSet<String> = BTreeSet::new();
    for def in converted_guns.iter().chain(converted_melee.iter()) {
        for tag in crate::reader::access::list_texts(&def.node, "weaponTags") {
            if tag.starts_with(&classes.tag_prefix) {
                tags.insert(tag);
            }
        }
    }
    let weapon_tags: Vec<String> = tags.into_iter().collect();
    let ai_class_tags = weapon_tags
        .iter()
        .filter(|t| t.starts_with(&classes.ai_tag_prefix))
        .cloned()
        .collect();
    let mut used: BTreeSet<String> = CE_STAT_NAMES.into_iter().map(str::to_owned).collect();
    for def in converted_guns.iter().chain(converted_melee.iter()) {
        for (name, _) in crate::reader::access::named_numbers(&def.node, "statBases") {
            used.insert(name);
        }
    }
    let table = StatTable::from_databases(dbs, &options.reader.stat_db_type);
    let stat_rules = used
        .into_iter()
        .filter(|n| table.knows(n))
        .map(|n| {
            let rules = table.rules(&n);
            (n, rules)
        })
        .collect();
    CeModel {
        absent: None,
        names,
        probe_def: ammo_sets.first().map(|a| a.def_name.clone()),
        gun_presets: read_gun_presets(dbs, &classes.gun_preset_def),
        apparel_presets: read_apparel_presets(dbs, &classes.apparel_preset_def),
        classes,
        ammo_sets,
        weapon_tags,
        ai_class_tags,
        stat_rules,
        guns,
        melee,
        diagnostics,
    }
}

fn most_common_shoot_verb(
    guns: &[&rimstudio_defs::DefRecord],
    classes: &CeClassNames,
) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for def in guns {
        let verbs = crate::reader::access::list_items(&def.node, "verbs");
        for li in verbs {
            let is_ce = crate::reader::access::class_attr(li)
                .is_some_and(|c| c.eq_ignore_ascii_case(&classes.verb_properties));
            if is_ce && let Some(v) = crate::reader::access::child_text(li, "verbClass") {
                *counts.entry(v).or_insert(0) += 1;
            }
        }
    }
    // Most common; ties go to the lexicographically smaller name (BTreeMap order, first maximum wins).
    let mut best: Option<(&String, usize)> = None;
    for (name, count) in &counts {
        if best.is_none_or(|(_, c)| *count > c) {
            best = Some((name, *count));
        }
    }
    best.map(|(n, _)| n.clone())
}

fn pair_twins(
    vanilla: &DefDatabases,
    options: &CeReadOptions<'_>,
    guns: &mut [CeGun],
    melee: &mut [CeMelee],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ctx = ReadContext::new(vanilla, &options.reader);
    let twin_of = |name: &str| {
        let def = vanilla.get(&options.db_type, name)?;
        match ctx.read(def)? {
            Ok(detail) => Some((conversions::twin_stats(&detail), detail.weapon_tags.clone())),
            Err(_) => None,
        }
    };
    for g in guns.iter_mut() {
        let found = twin_of(&g.def_name);
        if let Some((_, tags)) = &found {
            g.twin_tags.clone_from(tags);
        }
        g.twin = found.map(|(stats, _)| stats);
        if g.twin.is_none() {
            diagnostics.push(
                diag(
                    CE_TWIN_MISSING,
                    Severity::Hint,
                    format!("{} has no vanilla twin", g.def_name),
                )
                .with_arg("def", g.def_name.clone()),
            );
        }
    }
    for m in melee.iter_mut() {
        let found = twin_of(&m.def_name);
        if let Some((_, tags)) = &found {
            m.twin_tags.clone_from(tags);
        }
        m.twin = found.map(|(stats, _)| stats);
    }
}

/// The def types of the Combat Extended assembly that the reader needs, as `(full name, base)` entries for a
/// type table (spike S-07: the vanilla table lacks them, so these defs would count as unknown types).
#[must_use]
pub fn ce_type_entries(classes: &CeClassNames) -> Vec<(String, String)> {
    vec![
        (classes.ammo_set_def.clone(), "Verse.Def".to_owned()),
        (classes.gun_preset_def.clone(), "Verse.Def".to_owned()),
        (classes.apparel_preset_def.clone(), "Verse.Def".to_owned()),
    ]
}

/// A copy of the type table with the Combat Extended def types added (those already present are kept as
/// they are). Types are appended in assembly order after the existing ones; the table's root, assemblies and
/// short name rules are preserved.
///
/// # Errors
///
/// [`rimstudio_defs::DefsError::InvalidTypeTable`] when the table does not rebuild.
pub fn with_ce_types(table: &TypeTable, classes: &CeClassNames) -> DefsResult<TypeTable> {
    let mut value = table.to_json_value();
    if let Some(types) = value.get_mut("types").and_then(|t| t.as_object_mut()) {
        for (name, base) in ce_type_entries(classes) {
            if table.info(&name).is_none() {
                let info = TypeInfo::with_base(base);
                if let Ok(v) = serde_json::to_value(info) {
                    types.insert(name, v);
                }
            }
        }
    }
    TypeTable::from_json_value(value)
}

#[cfg(test)]
pub(crate) mod fixtures_tests;
#[cfg(test)]
mod tests;
