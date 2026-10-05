//! The patch of a bow or crossbow: Combat Extended converts these in a style of their own.
//!
//! A bow has no magazine, no recoil and no fire modes to speak of. Its conversion is the gun operation with
//! these differences (the research template `ce.bow`, observed on the converted bows of the user's install):
//!
//! ```text
//! Operation[make gun] > defName, statBases (Bulk, SwayFactor, ShotSpread, ...),
//!                       Properties (no recoil, no burst), AmmoUser (ammoSet and a spawn count),
//!                       FireModes (an empty element), weaponTags (the bow tag), AllowWithRunAndGun false
//! Operation[Replace or Add] tools     (one blunt tool, converted)
//! ```
//!
//! What is learned at run time and what is not:
//!
//! - The numbers are estimated from the converted bows of the library only ([`bow_prediction`]); guns are
//!   a different class and never serve a bow. With fewer than the estimator's minimum of bows a number is
//!   asked, like the numbers of a gun.
//! - The mass of a bow stays what the vanilla design says: it is written only when the block gives a
//!   different one.
//! - The bow tag, the ammo spawn count and the sets that count as arrow sets are read from the converted
//!   bows, and used only when enough of them agree ([`bow_tag_habit`], [`ammo_gen_habit`]). Nothing here is
//!   a table of Combat Extended values (R11).
//! - Combat Extended has no draw time and no draw strength: the pull of a bow is carried by the range, the
//!   warmup and the projectile of the ammo set, which are the numbers of the verb.
//!
//! A bow that already carries a conversion goes through update mode of [`super::update`], exactly like a
//! gun; applied twice the patch changes nothing, because the conversion is guarded by the ammo component.

use std::collections::BTreeMap;

use rimstudio_core::tree::{Node, NodeBuilder};

use super::container::Container;
use super::conventions::{MIN_AGREEMENT, MIN_EXAMPLES, majority};
use super::extras::{extras_ops, finish_tools, push_extra_tags};
use super::gun::{WRITTEN_VERB_FIELDS, check_gun_refs, one_handed_tag, push_num};
use super::melee::tool_list;
use super::ops::{DefOps, INHERIT_ATTR, add, class_li_xpath, unless_present};
use super::update::gun_update;
use super::values::{DerivedValue, Pick, ValueOrigin, predict_tool_ratios, resolve, resolve_tools};
use super::{GeneratedPatch, PatchCategory, PatchMode, PatchgenResult, guard};
use crate::ce::classes::{
    ConversionPrediction, ConversionPredictor, EstimateOptions, ExampleSet, Profile,
    estimate_conversion,
};
use crate::ce::lint::codes::TAG_NOT_FOUND;
use crate::ce::reader::bows::is_bow_shape;
use crate::ce::reader::{AmmoSetInfo, CeClassNames, CeModel};
use crate::classes::ItemKind as PoolKind;
use crate::model::{CePatchSpec, DesignSpec, ItemKind, TechLevel, format_number};
use crate::validation::codes::REQUIRED_MISSING;

/// The role the vanilla pools give a bow (a pool selector, not a game value).
const BOW_ROLE: &str = "bow";

/// True when the spec is converted in the bow style: the block says so (`ce.bow`), else the weapon's role,
/// its tags or weapon classes name a bow, or its projectile is an arrow or a bolt.
#[must_use]
pub fn is_bow_spec(spec: &DesignSpec) -> bool {
    if spec.kind != ItemKind::Ranged {
        return false;
    }
    if let Some(choice) = spec.ce.as_ref().and_then(|c| c.bow) {
        return choice;
    }
    if spec
        .role
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case(BOW_ROLE))
    {
        return true;
    }
    let projectile = spec
        .ranged
        .as_ref()
        .and_then(|r| r.projectile.as_ref())
        .map(|p| p.def_name());
    is_bow_shape(&spec.weapon_tags, &spec.weapon_classes, projectile)
}

/// A model that holds only the converted bows of `model` as its guns: what the estimator learns a bow from.
fn bow_view(model: &CeModel) -> CeModel {
    let mut view = CeModel::absent(String::new());
    view.absent = None;
    view.classes = model.classes.clone();
    view.guns = model.bows().cloned().collect();
    view
}

/// The vanilla numbers of a design under the names the estimator uses.
fn vanilla_numbers(spec: &DesignSpec) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let mut put = |name: &str, v: Option<f64>| {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            out.insert(name.to_owned(), v);
        }
    };
    let r = spec.ranged.as_ref();
    put("mass", spec.mass.map(|m| m.value));
    put("range", r.and_then(|r| r.range).map(|v| v.value));
    put("warmup", r.and_then(|r| r.warmup).map(|v| v.value));
    put("cooldown", r.and_then(|r| r.cooldown).map(|v| v.value));
    put("burst", Some(1.0));
    out
}

/// The estimates of the Combat Extended numbers of a bow, from the converted bows of the model. `None`
/// when the model has no converted bow.
#[must_use]
pub fn bow_prediction(spec: &DesignSpec, model: &CeModel) -> Option<ConversionPrediction> {
    let view = bow_view(model);
    let set = ExampleSet::from_model(&view, PoolKind::Ranged);
    if set.is_empty() {
        return None;
    }
    let profile = Profile {
        tier: spec.tech_level.map(TechLevel::index),
        tags: spec.weapon_tags.clone(),
        role: None,
        vanilla: vanilla_numbers(spec),
    };
    Some(estimate_conversion(
        &set,
        &profile,
        &EstimateOptions::default(),
    ))
}

/// The Combat Extended tags that the converted bows carry (not the AI class tags and not the one handed
/// mark), with the number of bows that carry each, most used first.
#[must_use]
pub fn bow_tag_candidates(model: &CeModel) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for bow in model.bows() {
        for tag in &bow.ce_tags {
            if tag.starts_with(&model.classes.ai_tag_prefix)
                || tag.to_lowercase().contains("onehanded")
            {
                continue;
            }
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// The bow tag when enough converted bows agree on one: at least [`MIN_EXAMPLES`] bows, and a share of
/// [`MIN_AGREEMENT`] of them carry the same tag.
#[must_use]
pub fn bow_tag_habit(model: &CeModel) -> Option<String> {
    let total = model.bows().count();
    if total < MIN_EXAMPLES {
        return None;
    }
    let candidates = bow_tag_candidates(model);
    let (tag, n) = candidates.first()?;
    if let Some((_, second)) = candidates.get(1)
        && second == n
    {
        return None;
    }
    (*n >= MIN_EXAMPLES && *n as f64 / total as f64 >= MIN_AGREEMENT).then(|| tag.clone())
}

/// The ammo spawn count (`AmmoGenPerMagOverride`) when enough converted bows write the same one.
#[must_use]
pub fn ammo_gen_habit(model: &CeModel) -> Option<u32> {
    let bows: Vec<_> = model.bows().collect();
    let written: Vec<u32> = bows
        .iter()
        .filter_map(|b| b.ammo_gen_per_mag)
        .filter(|v| *v >= 1.0 && *v <= f64::from(u32::MAX))
        .map(|v| v.round() as u32)
        .collect();
    if written.len() < MIN_EXAMPLES {
        return None;
    }
    let (value, n) = majority(&written)?;
    (n as f64 / bows.len().max(1) as f64 >= MIN_AGREEMENT).then_some(value)
}

/// True when the ammo set serves arrows or bolts: a converted bow uses it, or its name or the name of one of
/// its ammo or projectiles reads as an arrow or a bolt.
pub(crate) fn serves_arrows(set: &AmmoSetInfo, used_by_bows: bool) -> bool {
    used_by_bows
        || crate::ce::reader::bows::is_bow_projectile(&set.def_name)
        || set.ammo_types.iter().any(|a| {
            crate::ce::reader::bows::is_bow_projectile(&a.ammo)
                || crate::ce::reader::bows::is_bow_projectile(&a.projectile)
        })
}

/// The ammo sets a bow can use, best first: the sets that converted bows use (the most used first), then
/// the other sets that serve arrows or bolts, by name. When the model shows no such set at all, every set
/// is listed, by name, because nothing can be told apart.
#[must_use]
pub fn bow_ammo_sets(model: &CeModel) -> Vec<String> {
    let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
    for bow in model.bows() {
        if let Some(set) = bow.ammo_set.as_deref() {
            *uses.entry(set).or_default() += 1;
        }
    }
    let mut found: Vec<(String, usize)> = model
        .ammo_sets
        .iter()
        .filter(|s| serves_arrows(s, uses.contains_key(s.def_name.as_str())))
        .map(|s| {
            (
                s.def_name.clone(),
                uses.get(s.def_name.as_str()).copied().unwrap_or(0),
            )
        })
        .collect();
    if found.is_empty() {
        let mut all: Vec<String> = model.ammo_sets.iter().map(|s| s.def_name.clone()).collect();
        all.sort();
        return all;
    }
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found.into_iter().map(|(n, _)| n).collect()
}

/// The numbers of a converted bow, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct BowValues {
    /// The `Mass` stat, only when the block gives a mass that differs from the vanilla one.
    pub mass: Option<f64>,
    /// The `Bulk` stat.
    pub bulk: f64,
    /// The `SwayFactor` stat.
    pub sway: f64,
    /// The `ShotSpread` stat.
    pub spread: f64,
    /// The `SightsEfficiency` stat.
    pub sights: Option<f64>,
    /// The `RangedWeapon_Cooldown` stat.
    pub cooldown: Option<f64>,
    /// The verb's warmup in seconds.
    pub warmup: Option<f64>,
    /// The verb's range in cells.
    pub range: Option<f64>,
    /// The verb's recoil, only when the block gives one.
    pub recoil: Option<f64>,
    /// The magazine size, only when the block gives one (a bow has none by default).
    pub magazine: Option<u32>,
    /// The reload time, only when the block gives one.
    pub reload: Option<f64>,
    /// The ammo spawn count.
    pub ammo_gen: Option<u32>,
    /// The ammo set.
    pub ammo_set: String,
    /// The default projectile.
    pub projectile: String,
    /// The bow tag.
    pub tag: Option<String>,
    /// Whether the conversion writes `AllowWithRunAndGun` as false.
    pub forbid_run_and_gun: bool,
    /// The numbers that were not in the block.
    pub derived: Vec<DerivedValue>,
}

/// Resolves the numbers of a bow patch. `None` when a field the block must hold is missing; validation
/// reports which.
#[must_use]
pub fn resolve_bow(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    prediction: Option<&ConversionPrediction>,
) -> Option<BowValues> {
    let ranged = spec.ranged.as_ref();
    let mut derived = Vec::new();
    let pick = |derived: &mut Vec<DerivedValue>,
                field: &str,
                block: Option<f64>,
                vanilla: Option<f64>,
                stat: &str|
     -> Pick { resolve(derived, field, block, vanilla, prediction, stat) };
    let range = pick(
        &mut derived,
        "range",
        None,
        ranged.and_then(|r| r.range).map(|v| v.value),
        "range",
    )
    .value;
    let warmup = pick(
        &mut derived,
        "warmup",
        None,
        ranged.and_then(|r| r.warmup).map(|v| v.value),
        "warmup",
    )
    .value;
    let cooldown = pick(
        &mut derived,
        "cooldown",
        ce.cooldown.map(|c| c.value),
        ranged.and_then(|r| r.cooldown).map(|v| v.value),
        "cooldown",
    )
    .value;
    let sights = pick(
        &mut derived,
        "sights",
        ce.sights_efficiency.map(|v| v.value),
        None,
        "sights",
    )
    .value;
    let vanilla_mass = spec.mass.map(|m| m.value);
    let mass = ce.mass.map(|m| m.value).filter(|m| {
        vanilla_mass.is_none_or(|v| (v - m).abs() > 1e-6 * v.abs().max(m.abs()).max(1e-9))
    });
    Some(BowValues {
        mass,
        bulk: ce.bulk?.value,
        sway: ce.sway_factor?.value,
        spread: ce.shot_spread?.value,
        sights,
        cooldown,
        warmup,
        range,
        recoil: ce.recoil_amount.map(|v| v.value),
        magazine: ce.magazine_size.map(|m| m.value),
        reload: ce.reload_time.map(|r| r.value),
        ammo_gen: ce.ammo_gen_per_mag.map(|a| a.value),
        ammo_set: ce.ammo_set.clone().filter(|s| !s.is_empty())?,
        projectile: ce.default_projectile.clone().filter(|s| !s.is_empty())?,
        tag: ce.weapon_tag_class.clone().filter(|s| !s.is_empty()),
        forbid_run_and_gun: ce.allow_with_run_and_gun != Some(true),
        derived,
    })
}

fn make_bow(classes: &CeClassNames, spec: &DesignSpec, v: &BowValues, tags: &[String]) -> Node {
    let ranged = spec.ranged.as_ref();
    let mut b = NodeBuilder::new("Operation")
        .attr("Class", &classes.make_gun_op)
        .text_elem("defName", &spec.identity.def_name);
    b = b.elem("statBases", |s| {
        let s = push_num(s, "Mass", v.mass);
        let s = s.text_elem("Bulk", format_number(v.bulk));
        let s = s.text_elem("SwayFactor", format_number(v.sway));
        let s = s.text_elem("ShotSpread", format_number(v.spread));
        let s = push_num(s, "SightsEfficiency", v.sights);
        push_num(s, "RangedWeapon_Cooldown", v.cooldown)
    });
    b = b.elem("Properties", |p| {
        let p = push_num(p, "recoilAmount", v.recoil);
        let p = p
            .text_elem("verbClass", &classes.shoot_verb)
            .text_elem("hasStandardCommand", "true")
            .text_elem("defaultProjectile", &v.projectile);
        let p = push_num(p, "warmupTime", v.warmup);
        let p = push_num(p, "range", v.range);
        let p = p.text_elem_opt("soundCast", ranged.and_then(|r| r.sound_cast.clone()));
        let p = p.text_elem_opt(
            "soundCastTail",
            ranged.and_then(|r| r.sound_cast_tail.clone()),
        );
        let mut p = push_num(
            p,
            "muzzleFlashScale",
            ranged.and_then(|r| r.muzzle_flash_scale),
        );
        p = push_num(
            p,
            "forcedMissRadius",
            ranged.and_then(|r| r.forced_miss_radius),
        );
        // The conversion replaces the whole shooting verb, so what the vanilla verb carries beyond the
        // modelled fields is written again.
        for extra in ranged.map(|r| r.verb_extra.as_slice()).unwrap_or_default() {
            if !WRITTEN_VERB_FIELDS.contains(&extra.tag.as_str()) {
                p = p.child(extra.clone());
            }
        }
        p
    });
    b = b.elem("AmmoUser", |a| {
        let a = a.text_elem_opt("magazineSize", v.magazine.map(|m| m.to_string()));
        let a = push_num(a, "reloadTime", v.reload);
        let a = a.text_elem("ammoSet", &v.ammo_set);
        a.text_elem_opt("AmmoGenPerMagOverride", v.ammo_gen.map(|g| g.to_string()))
    });
    // An empty element still adds the fire modes component, which a bow has without any mode of its own.
    b = b.empty_elem("FireModes");
    if !tags.is_empty() {
        b = b.elem("weaponTags", |t| t.li_each(tags.iter().cloned()));
    }
    if v.forbid_run_and_gun {
        b = b.text_elem("AllowWithRunAndGun", "false");
    }
    b.build()
}

/// Generates the patch of a bow or crossbow. Same contract as [`super::gun_patch`], which calls it for a
/// weapon that [`is_bow_spec`] recognises.
///
/// # Errors
///
/// None at present; the result type is shared with the other generators.
pub fn bow_patch(
    spec: &DesignSpec,
    model: &CeModel,
    container: &Container,
) -> PatchgenResult<GeneratedPatch> {
    let mut diagnostics = match guard(spec, model, ItemKind::Ranged, container.is_converted()) {
        Ok(d) => d,
        Err(finished) => return Ok(*finished),
    };
    let Some(ce) = spec.ce.as_ref() else {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
    diagnostics.extend(check_gun_refs(ce, model));
    if crate::validation::has_errors(&diagnostics) {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    }
    if let Some(existing) = &container.existing {
        return Ok(gun_update(
            spec,
            ce,
            model,
            container,
            existing,
            diagnostics,
        ));
    }
    let prediction = bow_prediction(spec, model);
    let Some(mut values) = resolve_bow(spec, ce, prediction.as_ref()) else {
        diagnostics
            .push(REQUIRED_MISSING.diagnostic("/ce", &[("label", "the Combat Extended block")]));
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
    // A bow without a spawn count in the block takes the one the converted bows agree on.
    if values.ammo_gen.is_none()
        && let Some(count) = ammo_gen_habit(model)
    {
        values.ammo_gen = Some(count);
        let n = model
            .bows()
            .filter(|b| {
                b.ammo_gen_per_mag
                    .is_some_and(|g| (g - f64::from(count)).abs() < 0.5)
            })
            .count();
        values.derived.push(DerivedValue {
            field: "ammoGenPerMagOverride".to_owned(),
            value: f64::from(count),
            origin: ValueOrigin::Predicted {
                predictor: ConversionPredictor::Median,
                n,
            },
            rating: None,
            error: None,
        });
    }
    let mut tags: Vec<String> = Vec::new();
    if let Some(t) = &values.tag {
        tags.push(t.clone());
    }
    if ce.one_handed {
        match one_handed_tag(model) {
            Some(t) if !tags.contains(&t) => tags.push(t),
            Some(_) => {}
            None => diagnostics
                .push(TAG_NOT_FOUND.diagnostic("/ce/oneHanded", &[("what", "one handed")])),
        }
    }
    push_extra_tags(&mut tags, ce);
    diagnostics.extend(super::extras::validate(ce, model));
    if crate::validation::has_errors(&diagnostics) {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    }
    let mut derived = values.derived.clone();
    let mut ops = DefOps::new(&spec.identity.def_name, container);
    // A def that takes its verbs from a parent starts its own list with `Inherit="False"`, so the converted
    // verb replaces the inherited ones.
    if container.has_parent && !container.has("verbs") {
        let mut verbs = Node::new("verbs");
        verbs.set_attr(INHERIT_ATTR, "False");
        let own = ops.path("verbs");
        ops.push(unless_present(&own, add(ops.def_xpath(), vec![verbs])));
    }
    let conversion = make_bow(&model.classes, spec, &values, &tags);
    let guard_path = ops.path(&format!(
        "comps/{}",
        class_li_xpath(&model.classes.ammo_user)
    ));
    ops.push(unless_present(&guard_path, conversion));
    if !spec.tools.is_empty() || !ce.tool_plan.is_empty() {
        let ratios = predict_tool_ratios(spec, model);
        let (tools, missing, tool_derived) = resolve_tools(spec, ce, ratios.as_ref(), false);
        derived.extend(tool_derived);
        let Some(tools) = finish_tools(
            model,
            ItemKind::Ranged,
            ce,
            tools,
            &missing,
            &mut diagnostics,
        ) else {
            return Ok(GeneratedPatch::off(spec, diagnostics, None));
        };
        ops.replace_list(tool_list(&model.classes, &tools));
    }
    extras_ops(&mut ops, ce);
    diagnostics.push(
        crate::ce::lint::codes::ECONOMY_BY_DESIGN
            .diagnostic("", &[("def", &spec.identity.def_name)]),
    );
    Ok(GeneratedPatch {
        mode: PatchMode::New,
        category: PatchCategory::WeaponsRanged,
        def_name: spec.identity.def_name.clone(),
        operations: ops.finish(),
        diagnostics,
        derived,
        source: None,
    })
}
