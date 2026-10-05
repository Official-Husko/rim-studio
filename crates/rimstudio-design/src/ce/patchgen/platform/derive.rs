//! The convert flow of platforms and under barrel units, and what the user's own conversions teach.
//!
//! - [`apply`] runs after the automatic derivation of a gun: it carries the platform choices the user gave,
//!   and, for a weapon whose vanilla definition has a unit of its own (an equippable ability component),
//!   asks for the under barrel unit's ammo set and numbers. Nothing about the unit is invented: the numbers
//!   are asked, with a reference value shown when the user's library or the vanilla ability has one.
//! - [`unit_fire_habit`] learns the fire modes of an under barrel unit from the units of the user's
//!   converted weapons, used only when enough of them agree and only for a unit that gives none.

use rimstudio_core::tree::Node;
use rimstudio_defs::DefDatabases;

use super::{EQUIPPABLE_ABILITY_PREFIX, vanilla_unit_comp};
use crate::ce::patchgen::conventions::{MIN_AGREEMENT, MIN_EXAMPLES, majority};
use crate::ce::patchgen::convert::{AskItem, AskKind, AskList, ConvertAnswers};
use crate::ce::reader::CeModel;
use crate::classes::numeric::median;
use crate::model::{CePatchSpec, CeUnderBarrel, CeUnderBarrelFireModes};
use crate::reader::access::{child_number, child_text, class_attr, list_items};

/// The fire modes that enough of the user's under barrel units agree on, `None` when there are too few
/// units or they disagree. The aim mode, the burst flag and the single shot flag are each learned on their
/// own; a part without a clear majority stays unset.
#[must_use]
pub fn unit_fire_habit(model: &CeModel) -> Option<CeUnderBarrelFireModes> {
    let units: Vec<&CeUnderBarrelFireModes> = model
        .platform
        .under_barrels
        .iter()
        .map(|e| &e.unit.fire_modes)
        .collect();
    if units.len() < MIN_EXAMPLES {
        return None;
    }
    let share = |n: usize| n as f64 / units.len() as f64;
    let mut habit = CeUnderBarrelFireModes::default();
    let aims: Vec<Option<String>> = units.iter().map(|f| f.ai_aim_mode.clone()).collect();
    if let Some((Some(a), n)) = majority(&aims)
        && share(n) >= MIN_AGREEMENT
    {
        habit.ai_aim_mode = Some(a);
    }
    let bursts: Vec<Option<bool>> = units.iter().map(|f| f.ai_use_burst_mode).collect();
    if let Some((Some(b), n)) = majority(&bursts)
        && share(n) >= MIN_AGREEMENT
    {
        habit.ai_use_burst_mode = Some(b);
    }
    let singles: Vec<bool> = units.iter().map(|f| f.no_single_shot).collect();
    if let Some((s, n)) = majority(&singles)
        && share(n) >= MIN_AGREEMENT
    {
        habit.no_single_shot = s;
    }
    (!habit.is_empty()).then_some(habit)
}

/// The median of a number over the user's units that use the given ammo set, when at least
/// [`MIN_EXAMPLES`] of them do (the reference shown with a question).
fn library_median(model: &CeModel, pick: impl Fn(&CeUnderBarrel) -> Option<f64>) -> Option<f64> {
    let values: Vec<f64> = model
        .platform
        .under_barrels
        .iter()
        .filter_map(|e| pick(&e.unit))
        .collect();
    if values.len() < MIN_EXAMPLES {
        return None;
    }
    median(&values)
}

/// A reference number of the vanilla ability the weapon's own unit uses: the ability def named by the
/// component, and a field of its verb.
fn ability_reference(node: &Node, dbs: &DefDatabases, field: &str) -> Option<f64> {
    let comp = list_items(node, "comps")
        .into_iter()
        .find(|li| class_attr(li).is_some_and(|c| c.starts_with(EQUIPPABLE_ABILITY_PREFIX)))?;
    let ability = child_text(comp, "abilityDef")?;
    let def = dbs.get("AbilityDef", &ability)?;
    child_number(def.node.child("verbProperties")?, field)
}

fn ask(field: &str, label: &str, kind: AskKind, options: Vec<String>) -> AskItem {
    AskItem {
        field: field.to_owned(),
        label: label.to_owned(),
        kind,
        options,
        reason: None,
        suggestion: None,
    }
}

fn number_ask(field: &str, label: &str, library: Option<f64>, vanilla: Option<f64>) -> AskItem {
    let mut item = ask(field, label, AskKind::Number, Vec::new());
    if let Some(v) = library {
        item.suggestion = Some(v);
        item.reason = Some("the median of the under barrel units in your library".to_owned());
    } else if let Some(v) = vanilla {
        item.suggestion = Some(v);
        item.reason =
            Some("the value of the vanilla ability; the conversion usually changes it".to_owned());
    }
    item
}

/// Carries the platform and under barrel choices into the derived block, and asks for what a weapon with a
/// unit of its own still lacks. `node` is the resolved vanilla def of the weapon.
pub fn apply(
    block: &mut CePatchSpec,
    asks: &mut AskList,
    node: &Node,
    dbs: &DefDatabases,
    model: &CeModel,
    answers: &ConvertAnswers,
) {
    let over = &answers.overrides;
    block.is_weapon_platform = over.is_weapon_platform;
    block.attachment_links.clone_from(&over.attachment_links);
    block
        .default_graphic_parts
        .clone_from(&over.default_graphic_parts);
    if answers.skip_under_barrel {
        return;
    }
    let has_unit = vanilla_unit_comp(node).is_some();
    if over.under_barrel.is_none() && !has_unit {
        return;
    }
    let mut unit = over.under_barrel.clone().unwrap_or_default();
    if unit.is_bare() && over.under_barrel.is_some() {
        // The user asked for the bare slot form: nothing to ask.
        block.under_barrel = Some(unit);
        return;
    }
    let set_names: Vec<String> = {
        let mut v: Vec<String> = model.ammo_sets.iter().map(|a| a.def_name.clone()).collect();
        v.sort();
        v
    };
    let own_ammo = !unit.one_ammo_holder;
    if own_ammo && unit.ammo_set.is_none() {
        asks.items.push(ask(
            "/ce/underBarrel/ammoSet",
            "Which ammo set does the under barrel unit use?",
            AskKind::Choice,
            set_names,
        ));
    }
    if unit.default_projectile.is_none() {
        match unit
            .ammo_set
            .as_deref()
            .and_then(|s| model.ammo_set(s))
            .and_then(|s| s.first())
        {
            Some(first) => unit.default_projectile = Some(first.projectile.clone()),
            None if unit.ammo_set.is_some() || !own_ammo => asks.items.push(ask(
                "/ce/underBarrel/defaultProjectile",
                "Which projectile of the unit's ammo set is the default?",
                AskKind::Choice,
                Vec::new(),
            )),
            None => {}
        }
    }
    if own_ammo && unit.magazine_size.is_none() {
        asks.items.push(number_ask(
            "/ce/underBarrel/magazineSize",
            "Magazine size of the under barrel unit",
            library_median(model, |u| u.magazine_size.map(|m| f64::from(m.value))),
            None,
        ));
    }
    if own_ammo && unit.reload_time.is_none() {
        asks.items.push(number_ask(
            "/ce/underBarrel/reloadTime",
            "Reload time of the under barrel unit",
            library_median(model, |u| u.reload_time.map(|r| r.value)),
            None,
        ));
    }
    if unit.range.is_none() {
        asks.items.push(number_ask(
            "/ce/underBarrel/range",
            "Range of the under barrel unit",
            library_median(model, |u| u.range.map(|r| r.value)),
            ability_reference(node, dbs, "range"),
        ));
    }
    block.under_barrel = Some(unit);
}
