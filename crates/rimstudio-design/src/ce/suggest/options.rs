//! The optional additions of a conversion, offered as suggestions the user accepts.
//!
//! Next to the numbers of the block, the user's own conversions show habits that are choices, not numbers:
//! the tags that converted guns of a class carry besides the class tag, a recoil pattern, a reload that
//! takes one round at a time, a restructured tool list. [`suggest_options`] reports each habit with the
//! number of converted weapons behind it, and [`accept_options`] writes the ones the user accepts into the
//! block. Nothing here is written on its own (D-085): without accepting, the generator keeps to the design.
//!
//! Rules: the habits come only from the model and need agreeing examples (see
//! [`crate::ce::patchgen::conventions`]); a field the block already holds is never overwritten; a spec
//! without a Combat Extended block is returned unchanged.

use serde::{Deserialize, Serialize};

use crate::ce::patchgen::bow::is_bow_spec;
use crate::ce::patchgen::conventions::{
    companion_shares, recoil_pattern_habit, reload_one_at_a_time_habit,
};
use crate::ce::patchgen::extras::suggest_tool_plan;
use crate::ce::reader::CeModel;
use crate::model::{CePatchSpec, CeToolPlan, DesignSpec, ItemKind};

/// The id of the companion tags option (`/ce/extraTags`).
pub const EXTRA_TAGS: &str = "extra-tags";
/// The id of the tool plan option (`/ce/toolPlan`).
pub const TOOL_PLAN: &str = "tool-plan";
/// The id of the reload option (`/ce/reloadOneAtATime`).
pub const RELOAD_ONE_AT_A_TIME: &str = "reload-one-at-a-time";
/// The id of the recoil pattern option (`/ce/recoilPattern`).
pub const RECOIL_PATTERN: &str = "recoil-pattern";

/// What accepting an option writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum CeOptionValue {
    /// Weapon tags to add.
    Tags(Vec<String>),
    /// A flag to set.
    Flag(bool),
    /// A text value to set.
    Text(String),
    /// An explicit tool list.
    ToolPlan(Vec<CeToolPlan>),
}

/// One optional addition that the user's conversions suggest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeOption {
    /// The stable id (see the constants of this module).
    pub id: String,
    /// The JSON pointer of the block field the option fills.
    pub field: String,
    /// A short English label.
    pub label: String,
    /// What accepting writes.
    pub value: CeOptionValue,
    /// Converted weapons that show the habit.
    pub examples: usize,
    /// Converted weapons of the same group in all (the share that agrees is `examples` over this).
    pub of: usize,
    /// Why the option is offered, in plain words.
    pub why: String,
}

/// The optional additions the conversions of the model suggest for a design. `held` is the block the user
/// has (the default block when the toggle is off); a field it already holds gets no option.
#[must_use]
pub fn suggest_options(spec: &DesignSpec, model: &CeModel, held: &CePatchSpec) -> Vec<CeOption> {
    let mut out = Vec::new();
    if !model.is_present() {
        return out;
    }
    let gun = spec.kind == ItemKind::Ranged && !is_bow_spec(spec);
    if gun && let Some(class) = held.weapon_tag_class.as_deref().filter(|c| !c.is_empty()) {
        let tags: Vec<_> = companion_shares(model, class)
            .into_iter()
            .filter(|t| !held.extra_tags.contains(&t.tag) && !spec.weapon_tags.contains(&t.tag))
            .collect();
        if let Some(first) = tags.first() {
            out.push(CeOption {
                id: EXTRA_TAGS.to_owned(),
                field: "/ce/extraTags".to_owned(),
                label: "Companion weapon tags".to_owned(),
                value: CeOptionValue::Tags(tags.iter().map(|t| t.tag.clone()).collect()),
                examples: first.carried,
                of: first.total,
                why: format!(
                    "converted guns of the class {class} also carry {}",
                    tags.iter()
                        .map(|t| format!("{} ({} of {})", t.tag, t.carried, t.total))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        if held.reload_one_at_a_time.is_none()
            && let Some((n, of)) = reload_one_at_a_time_habit(model, class)
        {
            out.push(CeOption {
                id: RELOAD_ONE_AT_A_TIME.to_owned(),
                field: "/ce/reloadOneAtATime".to_owned(),
                label: "Reload one round at a time".to_owned(),
                value: CeOptionValue::Flag(true),
                examples: n,
                of,
                why: format!(
                    "{n} of the {of} converted guns of the class {class} load one round at a time"
                ),
            });
        }
        if held.recoil_pattern.is_none()
            && let Some((pattern, n, of)) = recoil_pattern_habit(model, class)
        {
            out.push(CeOption {
                id: RECOIL_PATTERN.to_owned(),
                field: "/ce/recoilPattern".to_owned(),
                label: "Recoil pattern".to_owned(),
                value: CeOptionValue::Text(pattern.clone()),
                examples: n,
                of,
                why: format!("{n} of the {of} converted guns of the class {class} use the recoil pattern {pattern}"),
            });
        }
    }
    if held.tool_plan.is_empty()
        && let Some(plan) = suggest_tool_plan(spec, held, model)
    {
        let changed = plan.len();
        out.push(CeOption {
            id: TOOL_PLAN.to_owned(),
            field: "/ce/toolPlan".to_owned(),
            label: "Tool list of converted weapons".to_owned(),
            value: CeOptionValue::ToolPlan(plan),
            examples: changed,
            of: changed,
            why: "converted weapons restructure the tools of a design like this one (capacities or a separate tool for one of them)".to_owned(),
        });
    }
    out
}

/// The result of [`accept_options`].
#[derive(Debug, Clone, PartialEq)]
pub struct OptionsAccepted {
    /// The spec with the accepted options written into its block.
    pub spec: DesignSpec,
    /// The ids written.
    pub accepted: Vec<String>,
    /// Ids that were named but left alone, with the reason.
    pub skipped: Vec<(String, String)>,
}

/// Writes the options of `ids` (all of them when `ids` is `None`) into a copy of the spec's block. A spec
/// without a block is returned unchanged, and a field the block already holds is never overwritten.
#[must_use]
pub fn accept_options(
    spec: &DesignSpec,
    model: &CeModel,
    ids: Option<&[String]>,
) -> OptionsAccepted {
    let mut out = OptionsAccepted {
        spec: spec.clone(),
        accepted: Vec::new(),
        skipped: Vec::new(),
    };
    let Some(held) = spec.ce.as_ref() else {
        for id in ids.unwrap_or_default() {
            out.skipped
                .push((id.clone(), "the Combat Extended patch is off".to_owned()));
        }
        return out;
    };
    let options = suggest_options(spec, model, held);
    if let Some(ids) = ids {
        for id in ids {
            if !options.iter().any(|o| &o.id == id) {
                out.skipped.push((
                    id.clone(),
                    "no such suggestion, or the block already holds it".to_owned(),
                ));
            }
        }
    }
    let Some(ce) = out.spec.ce.as_mut() else {
        return out;
    };
    for o in options {
        if ids.is_some_and(|ids| !ids.contains(&o.id)) {
            continue;
        }
        match o.value {
            CeOptionValue::Tags(tags) => {
                for t in tags {
                    if !ce.extra_tags.contains(&t) {
                        ce.extra_tags.push(t);
                    }
                }
            }
            CeOptionValue::Flag(f) => ce.reload_one_at_a_time = Some(f),
            CeOptionValue::Text(t) => ce.recoil_pattern = Some(t),
            CeOptionValue::ToolPlan(plan) => ce.tool_plan = plan,
        }
        out.accepted.push(o.id);
    }
    out
}
