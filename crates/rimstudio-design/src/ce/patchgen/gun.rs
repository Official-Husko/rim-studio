//! The patch of a ranged weapon: one gun conversion operation and the replacement of the tools.
//!
//! Emitted shape (the research template `ce.ranged-gun`):
//!
//! ```text
//! Operation[make gun] > defName, statBases, Properties, AmmoUser, FireModes, weaponTags
//! Operation[Replace or Add] tools     (the gun bash, as converted tools)
//! ```
//!
//! Numbers: the block of the spec first, then the vanilla twin and the predictors (see [`super::values`]).
//! Costs and research stay in the vanilla definition and are not repeated.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::{Node, NodeBuilder};

use super::container::Container;
use super::melee::tool_list;
use super::ops::DefOps;
use super::update::gun_update;
use super::values::{GunValues, predict_for, predict_tool_ratios, resolve_gun, resolve_tools};
use super::{GeneratedPatch, PatchCategory, PatchMode, PatchgenResult, guard};
use crate::ce::lint::codes::{CEP016, TAG_NOT_FOUND};
use crate::ce::reader::{CeClassNames, CeModel};
use crate::model::{CePatchSpec, DesignSpec, ItemKind, format_number};
use crate::validation::codes::{REF_UNRESOLVED, REQUIRED_MISSING};

/// The aim mode of a belt fed or suppressing weapon, a value of the fire modes enumeration.
const AIM_SUPPRESS: &str = "SuppressFire";
/// The aim mode of every other gun.
const AIM_AIMED: &str = "AimedShot";

/// Checks the references of a gun block against the user's Combat Extended: the ammo set, the projectile
/// as a member of the set (both errors, CP-009) and the class tag (CEP016, a warning).
#[must_use]
pub(crate) fn check_gun_refs(ce: &CePatchSpec, model: &CeModel) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(set) = ce.ammo_set.as_deref().filter(|s| !s.is_empty()) {
        match model.ammo_set(set) {
            None => out.push(
                REF_UNRESOLVED
                    .diagnostic("/ce/ammoSet", &[("label", "CE ammo set"), ("value", set)]),
            ),
            Some(info) => {
                if let Some(p) = ce.default_projectile.as_deref().filter(|s| !s.is_empty())
                    && !info.has_projectile(p)
                {
                    out.push(REF_UNRESOLVED.diagnostic(
                        "/ce/defaultProjectile",
                        &[
                            (
                                "label",
                                "CE default projectile (not a member of the ammo set)",
                            ),
                            ("value", p),
                        ],
                    ));
                }
            }
        }
    }
    if let Some(tag) = ce.weapon_tag_class.as_deref().filter(|s| !s.is_empty())
        && !model.knows_tag(tag)
    {
        out.push(CEP016.diagnostic("/ce/weaponTagClass", &[("value", tag)]));
    }
    out
}

/// The weapon tag of the installed data that marks a one handed weapon: the first known tag that names it.
fn one_handed_tag(model: &CeModel) -> Option<String> {
    model
        .weapon_tags
        .iter()
        .find(|t| t.to_lowercase().contains("onehanded"))
        .cloned()
}

fn push_num(b: NodeBuilder, tag: &str, v: Option<f64>) -> NodeBuilder {
    b.text_elem_opt(tag, v.map(format_number))
}

fn make_gun(
    classes: &CeClassNames,
    spec: &DesignSpec,
    ce: &CePatchSpec,
    v: &GunValues,
    tags: &[String],
) -> Node {
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
        let p = p.text_elem_opt("burstShotCount", v.burst.map(|b| b.to_string()));
        let p = push_num(p, "ticksBetweenBurstShots", v.ticks_between);
        let p = p.text_elem_opt("soundCast", ranged.and_then(|r| r.sound_cast.clone()));
        let p = p.text_elem_opt(
            "soundCastTail",
            ranged.and_then(|r| r.sound_cast_tail.clone()),
        );
        push_num(
            p,
            "muzzleFlashScale",
            ranged.and_then(|r| r.muzzle_flash_scale),
        )
    });
    b = b.elem("AmmoUser", |a| {
        a.text_elem("magazineSize", v.magazine.to_string())
            .text_elem("reloadTime", format_number(v.reload))
            .text_elem("ammoSet", &v.ammo_set)
    });
    b = b.elem("FireModes", |f| {
        let f = f.text_elem(
            "aiAimMode",
            if ce.belt_fed { AIM_SUPPRESS } else { AIM_AIMED },
        );
        f.text_elem_opt("aiUseBurstMode", v.burst.map(|_| "true".to_owned()))
    });
    if !tags.is_empty() {
        b = b.elem("weaponTags", |t| t.li_each(tags.iter().cloned()));
    }
    b.build()
}

/// Generates the patch of a ranged weapon.
///
/// Returns an empty patch (mode [`PatchMode::Off`]) when the spec has no Combat Extended block, when the
/// model is absent, or with error diagnostics when a required field is missing or a reference does not
/// resolve. A target that already carries a conversion gets update mode ([`PatchMode::Update`]).
///
/// # Errors
///
/// None at present; the result type is shared with [`super::apparel_patch`].
pub fn gun_patch(
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
    let prediction = predict_for(spec, model);
    let Some(values) = resolve_gun(spec, ce, prediction.as_ref()) else {
        diagnostics
            .push(REQUIRED_MISSING.diagnostic("/ce", &[("label", "the Combat Extended block")]));
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
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
    let mut derived = values.derived.clone();
    let mut ops = DefOps::new(&spec.identity.def_name, container);
    ops.push(make_gun(&model.classes, spec, ce, &values, &tags));
    if !spec.tools.is_empty() {
        let ratios = predict_tool_ratios(spec, model);
        let (tools, missing, tool_derived) = resolve_tools(spec, ce, ratios.as_ref(), false);
        derived.extend(tool_derived);
        if !missing.is_empty() {
            for label in missing {
                diagnostics.push(REQUIRED_MISSING.diagnostic(
                    "/ce/toolPenetration",
                    &[("label", &format!("CE blunt penetration of tool {label}"))],
                ));
            }
            return Ok(GeneratedPatch::off(spec, diagnostics, None));
        }
        ops.replace_list(tool_list(&model.classes, &tools));
    }
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
