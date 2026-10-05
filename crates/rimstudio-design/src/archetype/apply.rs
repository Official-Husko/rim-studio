//! Writing a proposal into a spec: the empty and derived fields are filled, typed values are kept (IT-003).
//!
//! Every number goes through [`DesignSpec::offer`] with the source `suggested`, so a value the user typed,
//! answered or took from an anchor is never replaced, and a value an earlier proposal wrote is replaced by
//! the new one. The structure (cost list, stuff, weapon tags and classes, role, tech level) is filled only
//! where it is empty, unless the caller asks to refresh it.

use crate::error::{DesignError, DesignResult};
use crate::model::{
    ArchetypeChoice, CostEntry, DesignSpec, ItemKind, ScalarField, StuffSpec, ToolSpec, ValueSource,
};

use super::proposal::{Proposal, ProposedTool};

/// What an application did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// The pointers of the fields that now hold a proposed number.
    pub filled: Vec<String>,
    /// The pointers of the fields that kept their value because the user decided it.
    pub kept: Vec<String>,
}

/// Options of an application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Replace the structure (cost list, stuff, tags, classes, role, tech level) even when it is not empty.
    pub refresh_structure: bool,
}

fn put(spec: &mut DesignSpec, field: ScalarField, value: f64, report: &mut ApplyReport) {
    let pointer = field.pointer();
    if spec.offer(field, value, ValueSource::Suggested).applied() {
        report.filled.push(pointer);
    } else if spec.scalar(field).is_some() {
        report.kept.push(pointer);
    }
}

fn clear_suggested(spec: &mut DesignSpec, field: ScalarField) {
    let Some(r) = spec.ranged.as_mut() else {
        return;
    };
    fn weak<T: Copy>(v: Option<crate::model::Sourced<T>>) -> bool {
        v.is_some_and(|v| !v.is_typed() && v.source <= ValueSource::Suggested)
    }
    match field {
        ScalarField::ArmorPenetration if weak(r.armor_penetration) => r.armor_penetration = None,
        ScalarField::BurstCount if weak(r.burst_count) => r.burst_count = None,
        ScalarField::TicksBetweenBurstShots if weak(r.ticks_between_burst_shots) => {
            r.ticks_between_burst_shots = None;
        }
        _ => {}
    }
}

fn tool_spec(t: &ProposedTool) -> ToolSpec {
    let caps: Vec<&str> = t.capacities.iter().map(String::as_str).collect();
    ToolSpec::new(t.label.clone(), &caps)
}

fn tools_untouched(spec: &DesignSpec) -> bool {
    spec.tools.iter().all(|t| {
        [
            t.power,
            t.cooldown_time,
            t.armor_penetration,
            t.chance_factor,
        ]
        .into_iter()
        .flatten()
        .all(|v| v.source == ValueSource::Suggested)
    })
}

fn apply_tools(
    spec: &mut DesignSpec,
    tools: &[ProposedTool],
    melee: bool,
    report: &mut ApplyReport,
) {
    if tools.is_empty() {
        return;
    }
    // A gun's bash tools are only added to a gun that has none; a melee weapon's attacks follow the proposal.
    if spec.tools.is_empty() || (melee && tools_untouched(spec) && !same_labels(spec, tools)) {
        spec.tools = tools.iter().map(tool_spec).collect();
    } else if !melee {
        return;
    }
    for (i, wanted) in tools.iter().enumerate() {
        let index = spec
            .tools
            .iter()
            .position(|t| t.label == wanted.label)
            .unwrap_or(i);
        if spec.tools.get(index).is_none() {
            continue;
        }
        put(
            spec,
            ScalarField::ToolPower(index),
            wanted.power.value,
            report,
        );
        put(
            spec,
            ScalarField::ToolCooldown(index),
            wanted.cooldown.value,
            report,
        );
        if let Some(ap) = &wanted.armor_penetration {
            put(
                spec,
                ScalarField::ToolArmorPenetration(index),
                ap.value,
                report,
            );
        }
    }
}

fn same_labels(spec: &DesignSpec, tools: &[ProposedTool]) -> bool {
    spec.tools.len() == tools.len()
        && spec
            .tools
            .iter()
            .zip(tools)
            .all(|(a, b)| a.label == b.label)
}

/// Writes the proposal into the spec.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] when the proposal is for the other kind of weapon.
pub fn apply(
    spec: &mut DesignSpec,
    proposal: &Proposal,
    options: ApplyOptions,
) -> DesignResult<ApplyReport> {
    if spec.kind != proposal.kind {
        return Err(DesignError::invalid(
            "proposal",
            "the proposal is for the other kind of weapon",
        ));
    }
    let mut report = ApplyReport::default();
    for v in &proposal.values {
        if v.write {
            put(spec, v.field, v.value, &mut report);
        } else {
            clear_suggested(spec, v.field);
        }
    }
    if proposal.kind == ItemKind::Melee {
        apply_tools(spec, &proposal.tools, true, &mut report);
    } else {
        apply_tools(spec, &proposal.tools, false, &mut report);
    }
    let refresh = options.refresh_structure;
    if (refresh || spec.role.is_none()) && proposal.role.is_some() {
        spec.role.clone_from(&proposal.role);
    }
    if refresh || spec.tech_level.is_none() {
        spec.tech_level = Some(proposal.choice.tier);
    }
    if (refresh || spec.cost_list.is_empty()) && !proposal.cost_list.is_empty() {
        spec.cost_list = proposal
            .cost_list
            .iter()
            .map(|c| CostEntry::new(c.def_name.clone(), c.count))
            .collect();
    }
    if let Some(stuff) = &proposal.stuff {
        let slot = spec.stuff.get_or_insert_with(StuffSpec::default);
        if refresh || slot.categories.is_empty() {
            slot.categories.clone_from(&stuff.categories);
        }
        put(spec, ScalarField::StuffCount, stuff.count, &mut report);
    }
    if (refresh || spec.weapon_tags.is_empty()) && !proposal.weapon_tags.is_empty() {
        spec.weapon_tags.clone_from(&proposal.weapon_tags);
    }
    if (refresh || spec.weapon_classes.is_empty()) && !proposal.weapon_classes.is_empty() {
        spec.weapon_classes.clone_from(&proposal.weapon_classes);
    }
    Ok(report)
}

/// The choice a proposal was made for, as a draft stores it.
#[must_use]
pub fn choice_of(request: &super::propose::ProposeRequest) -> ArchetypeChoice {
    ArchetypeChoice {
        archetype: request.archetype.clone(),
        descriptors: request.descriptors.clone(),
        balance: request.balance,
        mode: request.mode,
        strength: request.strength,
    }
}
