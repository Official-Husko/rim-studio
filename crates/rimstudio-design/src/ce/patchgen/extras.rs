//! The optional additions of a conversion beyond the converted numbers: an explicit tool plan, the tool
//! children that converted weapons drop, accepted companion tags and raw nodes.
//!
//! Every addition is a decision of the user (D-085) and none is written on its own:
//!
//! - **Tool plan.** With `ce.toolPlan` the converted tools are exactly the entries of the plan, in order. An
//!   entry starts from a vanilla tool (numbers, penetration, group, carried children) and applies the
//!   fields it sets. Without a plan the tools follow the design and the restructurings that converted
//!   weapons make (a muzzle tool, relabelled tools) are listed as hints with the plan that would write them
//!   ([`suggest_tool_plan`]).
//! - **Dropped tool children.** A child of the vanilla tools that the user's own conversions consistently
//!   remove (see [`super::conventions::dropped_tool_fields`]) is removed by the conversion too, reported as
//!   a derived value; `ce.keepToolFields` keeps it.
//! - **Extra tags.** `ce.extraTags` are written next to the class tag, each once. They come from accepting
//!   the companion tag suggestion.
//! - **Raw nodes.** `ce.rawExtras` are nodes appended to the converted def as written (for example
//!   `modExtensions` with a gun draw extension), checked for their shape. A list (all children are `li`) is
//!   merged entry by entry, each entry only when the def does not hold it yet; any other node replaces the
//!   def's own node of that name or is added. Applied twice the patch changes nothing.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::conventions::{dropped_tool_fields, tool_habits};
use super::ops::{DefOps, add, branch, class_li_xpath, replace, unless_present, xpath_literal};
use super::values::{ToolValues, resolve_tools};
use crate::ce::lint::codes::{
    CEP016, DERIVED_VALUE, RAW_EXTRA_INVALID, TOOL_PLAN_INVALID, TOOL_RESTRUCTURE,
};
use crate::ce::reader::CeModel;
use crate::model::{CePatchSpec, CeToolPlan, DesignSpec, ItemKind};
use crate::validation::codes::REQUIRED_MISSING;

/// The children of a def that the conversion writes itself; a raw node of that name is refused.
pub const OWNED_CHILDREN: [&str; 7] = [
    "defName",
    "statBases",
    "verbs",
    "comps",
    "tools",
    "weaponTags",
    "equippedStatOffsets",
];

/// The problems of the optional additions of a block: raw nodes with a bad shape or an owned name, extra
/// tags that the installed data does not know (a warning).
#[must_use]
pub fn validate(ce: &CePatchSpec, model: &CeModel) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for (i, node) in ce.raw_extras.iter().enumerate() {
        let pointer = format!("/ce/rawExtras/{i}");
        let mut refuse = |reason: &str| {
            out.push(
                RAW_EXTRA_INVALID.diagnostic(&pointer, &[("node", &node.tag), ("reason", reason)]),
            );
        };
        if let Err(e) = node.validate() {
            refuse(&format!("it is not a valid XML node ({e})"));
            continue;
        }
        if OWNED_CHILDREN.contains(&node.tag.as_str()) {
            refuse("the conversion writes this element itself");
            continue;
        }
        if seen.contains(&node.tag.as_str()) {
            refuse("another raw node has the same name");
            continue;
        }
        seen.push(&node.tag);
        if is_list(node) {
            for li in node.elements() {
                if entry_guard(&node.tag, li).is_none() {
                    refuse(
                        "a list entry needs a Class attribute or plain text so that it is added once",
                    );
                    break;
                }
            }
        }
    }
    for tag in &ce.extra_tags {
        if !model.knows_tag(tag) {
            out.push(CEP016.diagnostic("/ce/extraTags", &[("value", tag.as_str())]));
        }
    }
    out
}

/// True when every child element of the node is a list entry.
fn is_list(node: &Node) -> bool {
    node.elements().next().is_some() && node.elements().all(|e| e.tag == "li")
}

/// The xpath below the def that tells whether a list entry is already there: by its `Class` attribute, or
/// by its text. `None` for an entry that has neither.
fn entry_guard(list: &str, li: &Node) -> Option<String> {
    if let Some(class) = li.attr("Class") {
        return Some(format!("{list}/{}", class_li_xpath(class)));
    }
    let text = li.leaf_text()?;
    Some(format!("{list}/li[.={}]", xpath_literal(text)))
}

/// Writes the operations of the raw nodes of the block.
pub fn extras_ops(ops: &mut DefOps<'_>, ce: &CePatchSpec) {
    for node in &ce.raw_extras {
        if is_list(node) {
            ops.ensure(&node.tag);
            for li in node.elements() {
                let Some(guard) = entry_guard(&node.tag, li) else {
                    continue;
                };
                let guard = ops.path(&guard);
                let into = ops.path(&node.tag);
                ops.push(unless_present(&guard, add(&into, vec![li.clone()])));
            }
        } else {
            let here = ops.path(&node.tag);
            let def = ops.def_xpath().to_owned();
            ops.push(branch(
                &here,
                replace(&here, vec![node.clone()]),
                add(&def, vec![node.clone()]),
            ));
        }
    }
}

/// The extra tags of the block that are not in `tags` yet, appended in order, each once.
pub fn push_extra_tags(tags: &mut Vec<String>, ce: &CePatchSpec) {
    for t in &ce.extra_tags {
        if !t.is_empty() && !tags.contains(t) {
            tags.push(t.clone());
        }
    }
}

/// Builds the tools of an explicit plan from the tools the design resolves to. `None` when an entry cannot
/// be written (the reasons are in `diagnostics`).
fn apply_plan(
    resolved: &[ToolValues],
    plan: &[CeToolPlan],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<ToolValues>> {
    let mut out: Vec<ToolValues> = Vec::new();
    let mut ok = true;
    for (i, entry) in plan.iter().enumerate() {
        let pointer = format!("/ce/toolPlan/{i}");
        let mut refuse = |reason: &str| {
            diagnostics.push(
                TOOL_PLAN_INVALID
                    .diagnostic(&pointer, &[("tool", &entry.label), ("reason", reason)]),
            );
        };
        // Combat Extended's own bow tool has no label: an empty label is allowed for an entry that starts
        // from a vanilla tool, and then the converted tool is written without one.
        if entry.label.trim().is_empty() && entry.from.is_none() {
            refuse("the label is empty and the entry names no vanilla tool to start from");
            ok = false;
            continue;
        }
        let base = resolved.iter().find(|t| t.label == entry.source_label());
        let mut tool = base.cloned().unwrap_or_else(|| ToolValues {
            label: entry.label.clone(),
            capacities: Vec::new(),
            power: None,
            cooldown: None,
            ap_sharp: None,
            ap_blunt: None,
            chance_factor: None,
            linked_body_parts_group: None,
            extra_melee_damages: Vec::new(),
            surprise_attack: None,
            extra: Vec::new(),
        });
        tool.label.clone_from(&entry.label);
        if let Some(c) = &entry.capacities {
            tool.capacities.clone_from(c);
        }
        tool.power = entry.power.or(tool.power);
        tool.cooldown = entry.cooldown.or(tool.cooldown);
        tool.chance_factor = entry.chance_factor.or(tool.chance_factor);
        tool.ap_sharp = entry.armor_penetration_sharp.or(tool.ap_sharp);
        tool.ap_blunt = entry.armor_penetration_blunt.or(tool.ap_blunt);
        if entry.linked_body_parts_group.is_some() {
            tool.linked_body_parts_group
                .clone_from(&entry.linked_body_parts_group);
        }
        let from = base.map_or_else(
            || "no vanilla tool to start from".to_owned(),
            |b| format!("the vanilla tool {} has none", b.label),
        );
        let mut need = |what: &str, present: bool| {
            if !present {
                refuse(&format!("{what} is missing and {from}"));
                ok = false;
            }
        };
        need("a capacity", !tool.capacities.is_empty());
        need("power", tool.power.is_some());
        need("cooldown", tool.cooldown.is_some());
        need("a blunt penetration", tool.ap_blunt.is_some());
        out.push(tool);
    }
    ok.then_some(out)
}

/// The tools a conversion writes: the explicit plan when the block has one, else the tools the design
/// resolves to; then the habits of the converted tools (body part group, pick weight), the removal of the
/// children that converted weapons drop, and the hints about restructurings that are not written.
///
/// `None` when a tool cannot be written: a tool without a blunt penetration (a required field), or a plan
/// entry that is not usable. The diagnostics say which.
pub fn finish_tools(
    model: &CeModel,
    kind: ItemKind,
    ce: &CePatchSpec,
    resolved: Vec<ToolValues>,
    missing: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<ToolValues>> {
    let planned = !ce.tool_plan.is_empty();
    let tools = if planned {
        apply_plan(&resolved, &ce.tool_plan, diagnostics)?
    } else {
        if !missing.is_empty() {
            for label in missing {
                diagnostics.push(REQUIRED_MISSING.diagnostic(
                    "/ce/toolPenetration",
                    &[("label", &format!("CE blunt penetration of tool {label}"))],
                ));
            }
            return None;
        }
        resolved
    };
    // A plan is a decision: the habits only fill the group and pick weight it leaves open.
    let outcome = tool_habits(model, kind, tools, false);
    for n in &outcome.notes {
        diagnostics.push(DERIVED_VALUE.diagnostic(
            &n.field,
            &[("field", &n.field), ("value", &n.value), ("how", &n.how)],
        ));
    }
    if !planned {
        for r in &outcome.restructures {
            diagnostics.push(TOOL_RESTRUCTURE.diagnostic(
                "/tools",
                &[("tool", r.tool.as_str()), ("what", r.what.as_str())],
            ));
        }
    }
    let mut tools = outcome.tools;
    drop_fields(model, kind, ce, &mut tools, diagnostics);
    Some(tools)
}

/// Removes from the carried children of the tools those that converted weapons consistently drop.
fn drop_fields(
    model: &CeModel,
    kind: ItemKind,
    ce: &CePatchSpec,
    tools: &mut [ToolValues],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for d in dropped_tool_fields(model, kind) {
        if ce.keep_tool_fields.contains(&d.field) {
            continue;
        }
        let mut removed = false;
        for tool in tools.iter_mut() {
            let before = tool.extra.len();
            tool.extra.retain(|n| n.tag != d.field);
            removed |= tool.extra.len() != before;
        }
        if removed {
            let field = format!("tools {}", d.field);
            let how = format!(
                "removed, as {} of the {} converted weapons whose vanilla tools carry it removed it",
                d.dropped, d.total
            );
            diagnostics.push(DERIVED_VALUE.diagnostic(
                &field,
                &[("field", &field), ("value", "removed"), ("how", &how)],
            ));
        }
    }
}

/// The explicit tool plan that writes the restructurings which converted weapons make for this design, or
/// `None` when they would change nothing. Accepting it (setting `ce.toolPlan`) is the user's decision; the
/// plain conversion keeps the tools of the design.
#[must_use]
pub fn suggest_tool_plan(
    spec: &DesignSpec,
    ce: &CePatchSpec,
    model: &CeModel,
) -> Option<Vec<CeToolPlan>> {
    if spec.tools.is_empty() {
        return None;
    }
    let kind = spec.kind;
    let prediction = match kind {
        ItemKind::Ranged => super::values::predict_tool_ratios(spec, model),
        ItemKind::Melee => super::values::predict_for(spec, model),
    };
    let own = CePatchSpec {
        tool_plan: Vec::new(),
        ..ce.clone()
    };
    let (resolved, _missing, _derived) =
        resolve_tools(spec, &own, prediction.as_ref(), kind == ItemKind::Melee);
    let plain = tool_habits(model, kind, resolved.clone(), false);
    if plain.restructures.is_empty() {
        return None;
    }
    let restructured = tool_habits(model, kind, resolved.clone(), true);
    let plan = restructured
        .tools
        .iter()
        .map(|t| {
            let copy = restructured
                .copies
                .iter()
                .find(|(new, _)| *new == t.label)
                .map(|(_, source)| source.clone());
            CeToolPlan {
                label: t.label.clone(),
                from: copy,
                capacities: Some(t.capacities.clone()),
                power: t.power,
                cooldown: t.cooldown,
                chance_factor: t.chance_factor,
                armor_penetration_sharp: t.ap_sharp,
                armor_penetration_blunt: t.ap_blunt,
                linked_body_parts_group: t.linked_body_parts_group.clone(),
            }
        })
        .collect();
    Some(plan)
}

#[cfg(test)]
mod tests;
