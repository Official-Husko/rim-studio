//! Update mode: the target already carries a Combat Extended conversion.
//!
//! A second gun conversion would append a second verb, a second ammo component and duplicate tags (CEP007),
//! so it is never emitted. Instead the Combat Extended block of the spec is compared with the values the
//! existing conversion holds, and each field that differs becomes one Replace operation (preceded by an Add
//! when the field is absent). Fields the block leaves open are not touched. Foreign files are never
//! rewritten: the operations always go into a RimStudio owned patch file of the project's CE folder, and a
//! hint says that they must run after the original conversion.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::container::{Container, ConversionSource, ExistingConversion};
use super::gun::one_handed_tag;
use super::ops::{DefOps, add, branch, class_li_xpath, replace, xpath_literal};
use super::{GeneratedPatch, PatchCategory, PatchMode};
use crate::ce::lint::codes::{UPDATE_LOAD_AFTER, UPDATE_NOTHING};
use crate::ce::reader::CeModel;
use crate::model::{CePatchSpec, DesignSpec, Sourced, format_number};
use crate::validation::codes::CE_ALREADY_CONVERTED;

/// True when a desired number differs from the current one (or there is no current one).
pub(super) fn changed(desired: f64, current: Option<f64>) -> bool {
    current.is_none_or(|c| (c - desired).abs() > 1e-6 * desired.abs().max(c.abs()).max(1e-9))
}

fn diff_stat(
    out: &mut Vec<(&'static str, String)>,
    name: &'static str,
    desired: Option<Sourced<f64>>,
    current: Option<Sourced<f64>>,
) {
    if let Some(d) = desired
        && changed(d.value, current.map(|c| c.value))
    {
        out.push((name, format_number(d.value)));
    }
}

/// Sets one field of a list entry that the xpath `below` selects: Replace when the field is there, and a
/// conditional (replace when it is there by then, add when it is not) otherwise, so that applying the update
/// twice leaves one field.
pub(super) fn set_field(
    ops: &mut DefOps<'_>,
    below: &str,
    field: &str,
    value: String,
    present: bool,
) {
    let node = Node::with_text(field, value);
    let field_path = ops.path(&format!("{below}/{field}"));
    if present {
        ops.push(replace(&field_path, vec![node]));
    } else {
        let entry = ops.path(below);
        ops.push(branch(
            &field_path,
            replace(&field_path, vec![node.clone()]),
            add(&entry, vec![node]),
        ));
    }
}

fn finish(
    spec: &DesignSpec,
    category: PatchCategory,
    ops: DefOps<'_>,
    mut diagnostics: Vec<Diagnostic>,
    existing: &ExistingConversion,
) -> GeneratedPatch {
    let operations = ops.finish();
    diagnostics.push(CE_ALREADY_CONVERTED.diagnostic("", &[("target", &spec.identity.def_name)]));
    if operations.is_empty() {
        diagnostics.push(UPDATE_NOTHING.diagnostic("", &[("target", &spec.identity.def_name)]));
    } else if !existing.source.is_rimstudio() {
        diagnostics.push(UPDATE_LOAD_AFTER.diagnostic(
            "",
            &[
                ("target", &spec.identity.def_name),
                ("source", existing.source.describe()),
            ],
        ));
    }
    GeneratedPatch {
        mode: PatchMode::Update,
        category,
        def_name: spec.identity.def_name.clone(),
        operations,
        diagnostics,
        derived: Vec::new(),
        source: Some(existing.source.clone()),
    }
}

/// The weapon tags of an update: the class tag is replaced (or added), the one handed mark is added when the
/// design says one handed and the conversion does not carry it yet.
fn update_tags(ops: &mut DefOps<'_>, ce: &CePatchSpec, cur: &CePatchSpec, model: &CeModel) {
    if let Some(tag) = ce.weapon_tag_class.as_deref().filter(|t| !t.is_empty())
        && cur.weapon_tag_class.as_deref() != Some(tag)
    {
        match cur.weapon_tag_class.as_deref() {
            Some(old) => ops.push(replace(
                &ops.path(&format!("weaponTags/li[.={}]", xpath_literal(old))),
                vec![Node::with_text("li", tag)],
            )),
            None => ops.add_unique_li("weaponTags", &[tag.to_owned()]),
        }
    }
    if ce.one_handed
        && !cur.one_handed
        && let Some(tag) = one_handed_tag(model)
    {
        ops.add_unique_li("weaponTags", &[tag]);
    }
    // The accepted companion tags: each is added only when the def does not carry it.
    let mut extra: Vec<String> = Vec::new();
    super::extras::push_extra_tags(&mut extra, ce);
    ops.add_unique_li("weaponTags", &extra);
}

/// Update mode for a gun.
pub(crate) fn gun_update(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    model: &CeModel,
    container: &Container,
    existing: &ExistingConversion,
    diagnostics: Vec<Diagnostic>,
) -> GeneratedPatch {
    let cur = &existing.block;
    let mut ops = DefOps::new(&spec.identity.def_name, container);
    let mut stats = Vec::new();
    diff_stat(&mut stats, "Mass", ce.mass, cur.mass);
    diff_stat(&mut stats, "Bulk", ce.bulk, cur.bulk);
    diff_stat(&mut stats, "SwayFactor", ce.sway_factor, cur.sway_factor);
    diff_stat(&mut stats, "ShotSpread", ce.shot_spread, cur.shot_spread);
    diff_stat(
        &mut stats,
        "SightsEfficiency",
        ce.sights_efficiency,
        cur.sights_efficiency,
    );
    diff_stat(
        &mut stats,
        "RangedWeapon_Cooldown",
        ce.cooldown,
        cur.cooldown,
    );
    if !stats.is_empty() {
        ops.set_entries("statBases", &stats);
    }

    let verb = format!("verbs/{}", class_li_xpath(&model.classes.verb_properties));
    if let Some(r) = ce.recoil_amount
        && changed(r.value, cur.recoil_amount.map(|c| c.value))
    {
        let present = existing.verb_fields.contains("recoilAmount");
        set_field(
            &mut ops,
            &verb,
            "recoilAmount",
            format_number(r.value),
            present,
        );
    }
    if let Some(p) = ce.default_projectile.as_deref().filter(|p| !p.is_empty())
        && cur.default_projectile.as_deref() != Some(p)
    {
        let present = existing.verb_fields.contains("defaultProjectile");
        set_field(&mut ops, &verb, "defaultProjectile", p.to_owned(), present);
    }

    let ammo = format!("comps/{}", class_li_xpath(&model.classes.ammo_user));
    if let Some(m) = ce.magazine_size
        && changed(
            f64::from(m.value),
            cur.magazine_size.map(|c| f64::from(c.value)),
        )
    {
        let present = existing.ammo_fields.contains("magazineSize");
        set_field(
            &mut ops,
            &ammo,
            "magazineSize",
            m.value.to_string(),
            present,
        );
    }
    if let Some(r) = ce.reload_time
        && changed(r.value, cur.reload_time.map(|c| c.value))
    {
        let present = existing.ammo_fields.contains("reloadTime");
        set_field(
            &mut ops,
            &ammo,
            "reloadTime",
            format_number(r.value),
            present,
        );
    }
    if let Some(s) = ce.ammo_set.as_deref().filter(|s| !s.is_empty())
        && cur.ammo_set.as_deref() != Some(s)
    {
        let present = existing.ammo_fields.contains("ammoSet");
        set_field(&mut ops, &ammo, "ammoSet", s.to_owned(), present);
    }
    if let Some(g) = ce.ammo_gen_per_mag
        && changed(
            f64::from(g.value),
            cur.ammo_gen_per_mag.map(|c| f64::from(c.value)),
        )
    {
        let present = existing.ammo_fields.contains("AmmoGenPerMagOverride");
        set_field(
            &mut ops,
            &ammo,
            "AmmoGenPerMagOverride",
            g.value.to_string(),
            present,
        );
    }

    if ce.reload_one_at_a_time == Some(true) && cur.reload_one_at_a_time != Some(true) {
        let present = existing.ammo_fields.contains("reloadOneAtATime");
        set_field(
            &mut ops,
            &ammo,
            "reloadOneAtATime",
            "true".to_owned(),
            present,
        );
    }
    if let Some(p) = ce.recoil_pattern.as_deref().filter(|p| !p.is_empty())
        && cur.recoil_pattern.as_deref() != Some(p)
    {
        let present = existing.verb_fields.contains("recoilPattern");
        set_field(&mut ops, &verb, "recoilPattern", p.to_owned(), present);
    }

    update_tags(&mut ops, ce, cur, model);
    super::extras::extras_ops(&mut ops, ce);
    super::platform::update_ops(
        &mut ops,
        &spec.identity.def_name,
        ce,
        existing,
        &model.classes,
        container,
    );
    finish(
        spec,
        PatchCategory::WeaponsRanged,
        ops,
        diagnostics,
        existing,
    )
}

/// Update mode for a melee weapon.
pub(crate) fn melee_update(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    model: &CeModel,
    container: &Container,
    existing: &ExistingConversion,
    diagnostics: Vec<Diagnostic>,
) -> GeneratedPatch {
    let cur = &existing.block;
    let mut ops = DefOps::new(&spec.identity.def_name, container);
    let mut stats = Vec::new();
    diff_stat(&mut stats, "Bulk", ce.bulk, cur.bulk);
    diff_stat(
        &mut stats,
        "MeleeCounterParryBonus",
        ce.parry_bonus,
        cur.parry_bonus,
    );
    if !stats.is_empty() {
        ops.set_entries("statBases", &stats);
    }
    let mut offsets = Vec::new();
    diff_stat(
        &mut offsets,
        "MeleeCritChance",
        ce.melee_crit_chance,
        cur.melee_crit_chance,
    );
    diff_stat(
        &mut offsets,
        "MeleeParryChance",
        ce.melee_parry_chance,
        cur.melee_parry_chance,
    );
    diff_stat(
        &mut offsets,
        "MeleeDodgeChance",
        ce.melee_dodge_chance,
        cur.melee_dodge_chance,
    );
    if !offsets.is_empty() {
        ops.set_entries("equippedStatOffsets", &offsets);
    }
    for want in &ce.tool_penetration {
        let have = cur.tool_penetration.iter().find(|p| p.tool == want.tool);
        let entry = format!("tools/li[label={}]", xpath_literal(&want.tool));
        for (field, desired, current) in [
            (
                "armorPenetrationBlunt",
                want.blunt,
                have.and_then(|h| h.blunt),
            ),
            (
                "armorPenetrationSharp",
                want.sharp,
                have.and_then(|h| h.sharp),
            ),
        ] {
            if let Some(d) = desired
                && changed(d.value, current.map(|c| c.value))
            {
                set_field(
                    &mut ops,
                    &entry,
                    field,
                    format_number(d.value),
                    current.is_some(),
                );
            }
        }
    }
    update_tags(&mut ops, ce, cur, model);
    super::extras::extras_ops(&mut ops, ce);
    finish(
        spec,
        PatchCategory::WeaponsMelee,
        ops,
        diagnostics,
        existing,
    )
}

/// True when the source of an existing conversion may be edited in place by the toolkit (a RimStudio or
/// project file). Foreign and unknown sources never are.
#[must_use]
pub fn may_edit_in_place(source: &ConversionSource) -> bool {
    matches!(
        source,
        ConversionSource::RimStudio(_) | ConversionSource::Project(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_compares_with_a_relative_tolerance() {
        assert!(changed(1.0, None));
        assert!(!changed(1.0, Some(1.0)));
        assert!(!changed(1000.0, Some(1000.0000001)));
        assert!(changed(1.0, Some(1.01)));
    }

    #[test]
    fn only_rimstudio_and_project_files_may_be_edited_in_place() {
        assert!(may_edit_in_place(&ConversionSource::RimStudio(None)));
        assert!(may_edit_in_place(&ConversionSource::Project(None)));
        assert!(!may_edit_in_place(&ConversionSource::Foreign(None)));
        assert!(!may_edit_in_place(&ConversionSource::Unknown));
    }
}
