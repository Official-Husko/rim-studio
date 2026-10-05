//! The patch of a melee weapon: plain operations, no Combat Extended operation class.
//!
//! Emitted shape (the research template `ce.melee`): the stat entries `Bulk` and `MeleeCounterParryBonus`
//! into `statBases`, the three offsets into `equippedStatOffsets` (containers ensured first, entries the
//! target already has replaced one by one), then the replacement of `tools` with converted tools. Every tool
//! carries the Combat Extended class and a blunt penetration; cutting and piercing tools carry a sharp one.

use rimstudio_core::tree::{Node, NodeBuilder};

use super::container::Container;
use super::extras::{extras_ops, finish_tools, push_extra_tags};
use super::gun::one_handed_tag;
use super::ops::DefOps;
use super::update::melee_update;
use super::values::{ToolValues, predict_for, resolve_tools};
use super::{GeneratedPatch, PatchCategory, PatchMode, PatchgenResult, guard};
use crate::ce::lint::codes::{CEP016, TAG_NOT_FOUND};
use crate::ce::reader::{CeClassNames, CeModel};
use crate::model::{CePatchSpec, DesignSpec, ExtraMeleeDamage, ItemKind, format_number};
use crate::validation::codes::REQUIRED_MISSING;
use rimstudio_core::diag::Diagnostic;

/// One converted tool entry, in the field order of the conventions.
#[must_use]
pub(crate) fn tool_node(classes: &CeClassNames, tool: &ToolValues) -> Node {
    let mut node = tool_fields(classes, tool);
    if !tool.extra_melee_damages.is_empty() {
        node.push_child(damages_node("extraMeleeDamages", &tool.extra_melee_damages));
    }
    if let Some(surprise) = &tool.surprise_attack {
        let mut s = Node::new("surpriseAttack");
        if !surprise.extra_melee_damages.is_empty() {
            s.push_child(damages_node(
                "extraMeleeDamages",
                &surprise.extra_melee_damages,
            ));
        }
        node.push_child(s);
    }
    for extra in &tool.extra {
        node.push_child(extra.clone());
    }
    node
}

/// A list of extra melee damages as the game reads it.
fn damages_node(tag: &str, damages: &[ExtraMeleeDamage]) -> Node {
    let mut list = Node::new(tag);
    for d in damages {
        let li = NodeBuilder::new("li")
            .text_elem("def", &d.def)
            .text_elem_opt("amount", d.amount.map(format_number))
            .text_elem_opt("chance", d.chance.map(format_number))
            .build();
        list.push_child(li);
    }
    list
}

fn tool_fields(classes: &CeClassNames, tool: &ToolValues) -> Node {
    NodeBuilder::new("li")
        .attr("Class", &classes.tool)
        .text_elem_opt("label", Some(&tool.label).filter(|l| !l.is_empty()))
        .elem("capacities", |c| c.li_each(tool.capacities.iter().cloned()))
        .text_elem_opt("power", tool.power.map(format_number))
        .text_elem_opt("chanceFactor", tool.chance_factor.map(format_number))
        .text_elem_opt("cooldownTime", tool.cooldown.map(format_number))
        .text_elem_opt("armorPenetrationBlunt", tool.ap_blunt.map(format_number))
        .text_elem_opt("armorPenetrationSharp", tool.ap_sharp.map(format_number))
        .text_elem_opt("linkedBodyPartsGroup", tool.linked_body_parts_group.clone())
        .build()
}

/// The `tools` list element of converted tools.
#[must_use]
pub(crate) fn tool_list(classes: &CeClassNames, tools: &[ToolValues]) -> Node {
    let mut list = Node::new("tools");
    for t in tools {
        list.push_child(tool_node(classes, t));
    }
    list
}

/// The Combat Extended tags a melee conversion adds: the class tag the user chose and the one handed mark.
/// A name that the installed data does not know is a warning (the class tag) or a hint (the one handed mark).
fn melee_tags(ce: &CePatchSpec, model: &CeModel, diagnostics: &mut Vec<Diagnostic>) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    if let Some(tag) = ce.weapon_tag_class.as_deref().filter(|t| !t.is_empty()) {
        if !model.knows_tag(tag) {
            diagnostics.push(CEP016.diagnostic("/ce/weaponTagClass", &[("value", tag)]));
        }
        tags.push(tag.to_owned());
    }
    if ce.one_handed {
        match one_handed_tag(model) {
            Some(t) if !tags.contains(&t) => tags.push(t),
            Some(_) => {}
            None => diagnostics
                .push(TAG_NOT_FOUND.diagnostic("/ce/oneHanded", &[("what", "one handed")])),
        }
    }
    tags
}

/// Generates the patch of a melee weapon.
///
/// Returns an empty patch (mode [`PatchMode::Off`]) when the spec has no Combat Extended block, when the
/// model is absent, or with error diagnostics when a required field (bulk, the three offsets, the blunt
/// penetration of a tool) is missing. A target that already carries converted tools gets update mode.
///
/// # Errors
///
/// None at present; the result type is shared with [`super::apparel_patch`].
pub fn melee_patch(
    spec: &DesignSpec,
    model: &CeModel,
    container: &Container,
) -> PatchgenResult<GeneratedPatch> {
    let mut diagnostics = match guard(spec, model, ItemKind::Melee, container.is_converted()) {
        Ok(d) => d,
        Err(finished) => return Ok(*finished),
    };
    let Some(ce) = spec.ce.as_ref() else {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
    if let Some(existing) = &container.existing {
        return Ok(melee_update(
            spec,
            ce,
            model,
            container,
            existing,
            diagnostics,
        ));
    }
    let (Some(bulk), Some(crit), Some(parry), Some(dodge)) = (
        ce.bulk,
        ce.melee_crit_chance,
        ce.melee_parry_chance,
        ce.melee_dodge_chance,
    ) else {
        diagnostics.push(
            REQUIRED_MISSING.diagnostic("/ce", &[("label", "the Combat Extended melee block")]),
        );
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
    let prediction = predict_for(spec, model);
    let (tools, missing, derived) = resolve_tools(spec, ce, prediction.as_ref(), true);
    diagnostics.extend(super::extras::validate(ce, model));
    if crate::validation::has_errors(&diagnostics) {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    }
    let Some(tools) = finish_tools(
        model,
        ItemKind::Melee,
        ce,
        tools,
        &missing,
        &mut diagnostics,
    ) else {
        return Ok(GeneratedPatch::off(spec, diagnostics, None));
    };
    let mut ops = DefOps::new(&spec.identity.def_name, container);
    let mut stats = vec![("Bulk", format_number(bulk.value))];
    if let Some(p) = ce.parry_bonus {
        stats.push(("MeleeCounterParryBonus", format_number(p.value)));
    }
    ops.set_entries("statBases", &stats);
    ops.set_entries(
        "equippedStatOffsets",
        &[
            ("MeleeCritChance", format_number(crit.value)),
            ("MeleeParryChance", format_number(parry.value)),
            ("MeleeDodgeChance", format_number(dodge.value)),
        ],
    );
    let mut tags = melee_tags(ce, model, &mut diagnostics);
    push_extra_tags(&mut tags, ce);
    ops.add_unique_li("weaponTags", &tags);
    if !tools.is_empty() {
        ops.replace_list(tool_list(&model.classes, &tools));
    }
    extras_ops(&mut ops, ce);
    diagnostics.push(
        crate::ce::lint::codes::ECONOMY_BY_DESIGN
            .diagnostic("", &[("def", &spec.identity.def_name)]),
    );
    Ok(GeneratedPatch {
        mode: PatchMode::New,
        category: PatchCategory::WeaponsMelee,
        def_name: spec.identity.def_name.clone(),
        operations: ops.finish(),
        diagnostics,
        derived,
        source: None,
    })
}
