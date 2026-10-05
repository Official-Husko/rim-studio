//! The reverse mapping from a resolved def to a [`DesignSpec`].
//!
//! Two flows use it: "clone and adjust" (a reference weapon becomes the starting point of a new design) and
//! "convert an existing item" (a weapon of the user's own mod is read so its Combat Extended patch can be
//! generated). The spec always describes the vanilla definition: the Combat Extended block stays off
//! (`ce` is `None`), whatever the def contains.
//!
//! Values are the def's own numbers (the resolved `statBases` and verb fields), not stuff evaluated ones, and
//! every number carries the [`ValueSource`] the caller asks for (`Anchor` for a clone, `Typed` for an item the
//! user owns).

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, DefRecord};

use super::access::{child_number, child_text, list_items, list_texts, named_numbers, number_map};
use super::options::ReaderOptions;
use super::own::{OwnSource, apply_own};
use super::weapons::{ReadContext, description_of, tech_level_named};
use crate::error::{DesignError, DesignResult};
use crate::model::{
    AccuracyInputs, CostEntry, DesignSpec, ItemKind, ParentRef, ProjectileChoice, RangedInputs,
    Sourced, StuffSpec, ToolSpec, ValueSource,
};

/// Stats that have a field of their own in the spec; the rest go to `extra_stats`.
const MAPPED_STATS: [&str; 8] = [
    "Mass",
    "WorkToMake",
    "MarketValue",
    "AccuracyTouch",
    "AccuracyShort",
    "AccuracyMedium",
    "AccuracyLong",
    "RangedWeapon_Cooldown",
];

/// The result of reading a def into a spec.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecReading {
    /// The spec. Its identity is the def's own; use [`SpecReading::clone_as`] for a new item.
    pub spec: DesignSpec,
    /// Things the reader could not map or had to assume, in plain words.
    pub notes: Vec<String>,
    /// The strength index of the def, when it is a readable weapon.
    pub strength: Option<f64>,
}

impl SpecReading {
    /// The spec renamed for a new item (the clone flow). The parent stays the def's parent.
    #[must_use]
    pub fn clone_as(mut self, def_name: &str, label: &str) -> DesignSpec {
        self.spec.identity.def_name = def_name.to_owned();
        self.spec.identity.label = label.to_owned();
        self.spec
    }
}

/// Like [`spec_from_def`], for the clone flow: the stats that the def only inherits from its parent stay
/// inherited.
///
/// `own_stats` names the stats the def declares itself (the raw `statBases` of its file). Every other extra
/// stat of the resolved def is a value of the parent: it is recorded in
/// [`ParentRef::inherited_stats`] and left out of the spec's own stats, so the clone, which keeps the same
/// parent, inherits it and the written definition does not repeat it. The market value is kept only when the
/// def sets it itself. The numbers the readouts need (mass, work, accuracy, cooldown) are always copied.
/// Without a parent nothing is inherited and the reading is the plain one.
///
/// # Errors
///
/// As [`spec_from_def`].
pub fn spec_from_def_inheriting(
    def: &DefRecord,
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    source: ValueSource,
    own_stats: &BTreeSet<String>,
) -> DesignResult<SpecReading> {
    let mut reading = spec_from_def(def, dbs, opts, source)?;
    let spec = &mut reading.spec;
    let Some(parent) = spec.parent.as_mut() else {
        return Ok(reading);
    };
    let inherited: Vec<String> = spec
        .extra_stats
        .keys()
        .filter(|name| !own_stats.contains(*name))
        .cloned()
        .collect();
    for name in inherited {
        if let Some(value) = spec.extra_stats.remove(&name) {
            parent.inherited_stats.insert(name, value.value);
        }
    }
    if !own_stats.contains("MarketValue")
        && let Some(price) = spec.market_value.take()
    {
        parent
            .inherited_stats
            .insert("MarketValue".to_owned(), price.value);
    }
    // The mapped stats the def does not set itself keep their value in the spec (the readouts need it) and
    // are recorded as the parent's, so the written definition does not restate them until they change.
    let mut mapped: Vec<(&str, Option<f64>)> = vec![
        ("Mass", spec.mass.map(|s| s.value)),
        ("WorkToMake", spec.work_to_make.map(|s| s.value)),
    ];
    if let Some(r) = &spec.ranged {
        mapped.extend([
            ("AccuracyTouch", r.accuracy.touch.map(|s| s.value)),
            ("AccuracyShort", r.accuracy.short.map(|s| s.value)),
            ("AccuracyMedium", r.accuracy.medium.map(|s| s.value)),
            ("AccuracyLong", r.accuracy.long.map(|s| s.value)),
            ("RangedWeapon_Cooldown", r.cooldown.map(|s| s.value)),
        ]);
    }
    for (name, value) in mapped {
        if let Some(v) = value
            && !own_stats.contains(name)
        {
            parent.inherited_stats.insert(name.to_owned(), v);
        }
    }
    Ok(reading)
}

/// Reads a weapon def for the clone flow, with the def as its file writes it.
///
/// Like [`spec_from_def_inheriting`], and in addition the spec carries what the source defines itself and
/// nothing the parent supplies: its own tags, cost list, recipe (skill requirements, display priority,
/// workbenches), comps, interaction sound, icon, equipped stat offsets, verb and tool extras, the verbs
/// other than the shooting verb, and every field the designer does not model as a raw node. The tech level
/// the parent supplies is recorded in [`ParentRef::inherited_tech_level`] and not written. A clone built
/// from this reading reproduces its source.
///
/// # Errors
///
/// As [`spec_from_def`].
pub fn spec_from_def_own(
    def: &DefRecord,
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    source: ValueSource,
    own: &OwnSource<'_>,
) -> DesignResult<SpecReading> {
    let reset = own.def.child("statBases").is_some_and(|s| {
        s.attr("Inherit")
            .is_some_and(|v| v.eq_ignore_ascii_case("false"))
    });
    let stats_node = if reset {
        def.node.child("statBases")
    } else {
        own.def.child("statBases")
    };
    let own_stats: BTreeSet<String> = stats_node
        .map(|s| s.elements().map(|e| e.tag.clone()).collect())
        .unwrap_or_default();
    let mut reading = spec_from_def_inheriting(def, dbs, opts, source, &own_stats)?;
    apply_own(&mut reading.spec, &def.node, own.def, source);
    reading.notes.retain(|n| {
        !n.starts_with("fields the designer does not model") && !n.contains("other verb(s)")
    });
    if let Some(note) = carried_note(&reading.spec) {
        reading.notes.push(note);
    }
    Ok(reading)
}

/// The raw fields a spec carries, named in plain words (`None` when it carries none).
#[must_use]
pub fn carried_note(spec: &DesignSpec) -> Option<String> {
    let mut names: Vec<String> = spec.extra_fields.iter().map(|n| n.tag.clone()).collect();
    if !spec.comps.is_empty() {
        names.push(format!("comps ({})", spec.comps.len()));
    }
    if !spec.other_verbs.is_empty() {
        names.push(format!("{} more verb(s)", spec.other_verbs.len()));
    }
    if let Some(r) = &spec.ranged {
        names.extend(r.verb_extra.iter().map(|n| format!("verbs.{}", n.tag)));
    }
    names.extend(
        spec.tools
            .iter()
            .flat_map(|t| t.extra.iter().map(|n| format!("tools.{}", n.tag))),
    );
    names.extend(
        spec.graphic_extra
            .iter()
            .map(|n| format!("graphicData.{}", n.tag)),
    );
    if let Some(recipe) = &spec.recipe {
        names.extend(
            recipe
                .extra
                .iter()
                .map(|n| format!("recipeMaker.{}", n.tag)),
        );
    }
    names.dedup();
    (!names.is_empty()).then(|| {
        format!(
            "fields the designer does not model are carried as written: {}",
            names.join(", ")
        )
    })
}

fn sourced(v: Option<f64>, source: ValueSource) -> Option<Sourced<f64>> {
    v.map(|v| Sourced::new(v, source))
}

/// Reads a weapon def into a spec.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when the def has neither a shooting verb nor tools.
pub fn spec_from_def(
    def: &DefRecord,
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    source: ValueSource,
) -> DesignResult<SpecReading> {
    let node = &def.node;
    let verbs = list_items(node, "verbs");
    let shooting = verbs
        .iter()
        .find(|v| child_text(v, "defaultProjectile").is_some())
        .copied();
    let tool_nodes = list_items(node, "tools");
    if shooting.is_none() && tool_nodes.is_empty() {
        return Err(DesignError::invalid(
            "def",
            "the def has no shooting verb and no tools",
        ));
    }
    let kind = if shooting.is_some() {
        ItemKind::Ranged
    } else {
        ItemKind::Melee
    };
    let label = child_text(node, "label").unwrap_or_else(|| def.def_name.clone());
    let mut spec = DesignSpec::new(kind, def.def_name.clone(), label);
    let mut notes = Vec::new();
    spec.identity.description = description_of(node);
    if let Some(parent) = def.parents.first() {
        spec.parent = Some(ParentRef::named(parent.name.clone()));
    }
    spec.tech_level = child_text(node, "techLevel").and_then(|t| tech_level_named(&t));
    let stats = number_map(node, "statBases");
    spec.mass = sourced(stats.get("Mass").copied(), source);
    spec.work_to_make = sourced(stats.get("WorkToMake").copied(), source);
    spec.market_value = sourced(stats.get("MarketValue").copied(), source);
    spec.cost_list = named_numbers(node, "costList")
        .into_iter()
        .map(|(name, count)| CostEntry::new(name, count))
        .collect();
    let categories = list_texts(node, "stuffCategories");
    if !categories.is_empty() {
        spec.stuff = Some(StuffSpec {
            categories,
            count: sourced(child_number(node, "costStuffCount"), source),
        });
    }
    spec.research_prerequisite = node
        .child("recipeMaker")
        .and_then(|m| child_text(m, "researchPrerequisite"));
    spec.weapon_tags = list_texts(node, "weaponTags");
    spec.trade_tags = list_texts(node, "tradeTags");
    spec.weapon_classes = list_texts(node, "weaponClasses");
    if let Some(graphic) = node.child("graphicData") {
        spec.texture_path = child_text(graphic, "texPath");
        spec.graphic_class = child_text(graphic, "graphicClass");
    }
    spec.sound_interact = child_text(node, "soundInteract");
    spec.ui_icon_path = child_text(node, "uiIconPath");
    spec.ui_icon_scale = child_number(node, "uiIconScale");
    spec.extra_stats = extra_stats(&stats, source, kind);
    spec.tools = tool_nodes.iter().map(|t| tool_spec(t, source)).collect();
    if let Some(verb) = shooting {
        spec.ranged = Some(ranged_inputs(verb, &stats, dbs, opts, source, &mut notes));
    }
    for class in verbs
        .iter()
        .chain(tool_nodes.iter())
        .filter_map(|n| n.attr("Class"))
    {
        notes.push(format!(
            "the def uses the class {class} for a verb or tool; its fields are read as plain values"
        ));
    }
    notes.extend(unmodelled_notes(node, &verbs));
    let ctx = ReadContext::new(dbs, opts);
    let read = ctx.read(def);
    let strength = read
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|d| d.strength);
    if let Some(Ok(detail)) = &read {
        spec.role.clone_from(&detail.role);
    }
    Ok(SpecReading {
        spec,
        notes,
        strength,
    })
}

/// Children of the def node the spec has a field for (or that the written definition always inherits).
const MODELLED_TOP: [&str; 18] = [
    "soundInteract",
    "uiIconPath",
    "uiIconScale",
    "defName",
    "label",
    "description",
    "techLevel",
    "statBases",
    "costList",
    "stuffCategories",
    "costStuffCount",
    "recipeMaker",
    "weaponTags",
    "tradeTags",
    "weaponClasses",
    "graphicData",
    "verbs",
    "tools",
];
/// Children of the shooting verb the spec has a field for.
const MODELLED_VERB: [&str; 11] = [
    "forcedMissRadius",
    "verbClass",
    "defaultProjectile",
    "range",
    "burstShotCount",
    "ticksBetweenBurstShots",
    "warmupTime",
    "soundCast",
    "soundCastTail",
    "muzzleFlashScale",
    "hasStandardCommand",
];
/// Children of a tool the spec has a field for.
const MODELLED_TOOL: [&str; 7] = [
    "label",
    "capacities",
    "power",
    "cooldownTime",
    "armorPenetration",
    "chanceFactor",
    "linkedBodyPartsGroup",
];

fn unseen(
    node: &Node,
    known: &[&str],
    prefix: &str,
    out: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    for child in node.elements() {
        if known.contains(&child.tag.as_str()) {
            continue;
        }
        let name = if prefix.is_empty() {
            child.tag.clone()
        } else {
            format!("{prefix}.{}", child.tag)
        };
        if seen.insert(name.clone()) {
            out.push(name);
        }
    }
}

/// Plain notes about what the reader cannot carry: fields of the def, its verbs and its tools that the spec
/// has no place for, and verbs beyond the shooting verb. The names come from the def itself, in document
/// order, so a clone says exactly what it leaves behind.
fn unmodelled_notes(node: &Node, verbs: &[&Node]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    unseen(node, &MODELLED_TOP, "", &mut names, &mut seen);
    if let Some(recipe) = node.child("recipeMaker") {
        unseen(
            recipe,
            &["researchPrerequisite"],
            "recipeMaker",
            &mut names,
            &mut seen,
        );
    }
    if let Some(graphic) = node.child("graphicData") {
        unseen(
            graphic,
            &["texPath", "graphicClass"],
            "graphicData",
            &mut names,
            &mut seen,
        );
    }
    let shooting = verbs
        .iter()
        .find(|v| child_text(v, "defaultProjectile").is_some());
    if let Some(verb) = shooting {
        unseen(verb, &MODELLED_VERB, "verbs", &mut names, &mut seen);
    }
    for tool in list_items(node, "tools") {
        unseen(tool, &MODELLED_TOOL, "tools", &mut names, &mut seen);
    }
    let mut notes = Vec::new();
    if !names.is_empty() {
        notes.push(format!(
            "fields the designer does not model and a clone does not carry (the parent still supplies what the source inherited from it): {}",
            names.join(", ")
        ));
    }
    let others = verbs.len().saturating_sub(usize::from(shooting.is_some()));
    if others > 0 {
        notes.push(format!(
            "{others} other verb(s) of the def are not carried; only the shooting verb is read"
        ));
    }
    notes
}

/// The stats without a field of their own. The ranged stats (accuracy, cooldown) have fields only on a
/// shooting weapon; on any other weapon (a flamethrower verb, a thrown item) they are plain extra stats.
fn extra_stats(
    stats: &BTreeMap<String, f64>,
    source: ValueSource,
    kind: ItemKind,
) -> BTreeMap<String, Sourced<f64>> {
    const RANGED_ONLY: [&str; 5] = [
        "AccuracyTouch",
        "AccuracyShort",
        "AccuracyMedium",
        "AccuracyLong",
        "RangedWeapon_Cooldown",
    ];
    stats
        .iter()
        .filter(|(name, _)| {
            let ranged_only = RANGED_ONLY.contains(&name.as_str());
            !MAPPED_STATS.contains(&name.as_str()) || (ranged_only && kind != ItemKind::Ranged)
        })
        .map(|(name, v)| (name.clone(), Sourced::new(*v, source)))
        .collect()
}

fn tool_spec(node: &Node, source: ValueSource) -> ToolSpec {
    ToolSpec {
        label: child_text(node, "label").unwrap_or_default(),
        capacities: list_texts(node, "capacities"),
        power: sourced(child_number(node, "power"), source),
        cooldown_time: sourced(child_number(node, "cooldownTime"), source),
        armor_penetration: sourced(child_number(node, "armorPenetration"), source),
        chance_factor: sourced(child_number(node, "chanceFactor"), source),
        linked_body_parts_group: child_text(node, "linkedBodyPartsGroup"),
        ..ToolSpec::default()
    }
}

fn ranged_inputs(
    verb: &Node,
    stats: &BTreeMap<String, f64>,
    dbs: &DefDatabases,
    opts: &ReaderOptions,
    source: ValueSource,
    notes: &mut Vec<String>,
) -> RangedInputs {
    let projectile = child_text(verb, "defaultProjectile");
    let props = projectile
        .as_deref()
        .and_then(|p| dbs.get(&opts.db_type, p))
        .and_then(|d| d.node.child("projectile").cloned());
    if projectile.is_some() && props.is_none() {
        notes.push("the projectile def is not loaded; damage is unknown".to_owned());
    }
    let projectile_number = |tag: &str| props.as_ref().and_then(|p| child_number(p, tag));
    let stat = |name: &str| stats.get(name).copied();
    RangedInputs {
        verb_class: child_text(verb, "verbClass"),
        damage: sourced(projectile_number("damageAmountBase"), source),
        armor_penetration: sourced(
            projectile_number("armorPenetrationBase").filter(|v| *v >= 0.0),
            source,
        ),
        range: sourced(child_number(verb, "range"), source),
        burst_count: child_number(verb, "burstShotCount")
            .filter(|b| *b >= 1.0 && *b <= f64::from(u32::MAX))
            .map(|b| Sourced::new(b.floor() as u32, source)),
        ticks_between_burst_shots: sourced(child_number(verb, "ticksBetweenBurstShots"), source),
        warmup: sourced(child_number(verb, "warmupTime"), source),
        cooldown: sourced(stat("RangedWeapon_Cooldown"), source),
        accuracy: AccuracyInputs {
            touch: sourced(stat("AccuracyTouch"), source),
            short: sourced(stat("AccuracyShort"), source),
            medium: sourced(stat("AccuracyMedium"), source),
            long: sourced(stat("AccuracyLong"), source),
        },
        projectile: projectile.map(ProjectileChoice::Reference),
        sound_cast: child_text(verb, "soundCast"),
        sound_cast_tail: child_text(verb, "soundCastTail"),
        muzzle_flash_scale: child_number(verb, "muzzleFlashScale"),
        forced_miss_radius: child_number(verb, "forcedMissRadius"),
        verb_extra: Vec::new(),
    }
}
