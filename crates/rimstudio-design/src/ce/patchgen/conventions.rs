//! Conventions that the user's own Combat Extended conversions follow, learned at run time.
//!
//! Some parts of a conversion are not numbers the designer holds but habits of how the Combat Extended team
//! converts: the body part group of a converted tool, the pick weight of the first tool of a gun, which
//! capacity a handle strikes with, which aim mode a weapon class uses, which tags travel with the weapon.
//! None of these is a constant of this crate (R11): each is read from the converted weapons in the model,
//! used only when enough of them agree, and reported as a derived value so the user can overrule it.
//!
//! A habit is trusted when at least [`MIN_EXAMPLES`] converted tools share the label and at least
//! [`MIN_AGREEMENT`] of them agree (capacities need [`MIN_CAPACITY_EXAMPLES`] and
//! [`MIN_CAPACITY_AGREEMENT`], because they change what the tool does). A habit never overrules a value the
//! design carries: the group and the pick weight are filled in only when the tool has none.

use std::collections::BTreeMap;

use rimstudio_core::diag::Diagnostic;

use super::values::ToolValues;
use crate::ce::lint::codes::{DERIVED_VALUE, TOOL_RESTRUCTURE};
use crate::ce::reader::{CeGun, CeModel, CeToolRow};
use crate::model::{ItemKind, format_number};

/// Examples with the same label needed before a habit of that label is used.
pub const MIN_EXAMPLES: usize = 2;
/// The share of the examples that must agree on a group or a pick weight.
pub const MIN_AGREEMENT: f64 = 0.6;
/// Examples with the same label needed before the capacities of that label are changed.
pub const MIN_CAPACITY_EXAMPLES: usize = 3;
/// The share of the examples that must agree on the capacities.
pub const MIN_CAPACITY_AGREEMENT: f64 = 0.75;

/// The most frequent value of a list and how many times it occurs. A tie has no winner.
pub(super) fn majority<T: Ord + Clone>(items: &[T]) -> Option<(T, usize)> {
    let mut counts: BTreeMap<&T, usize> = BTreeMap::new();
    for i in items {
        *counts.entry(i).or_default() += 1;
    }
    let best = counts.values().copied().max()?;
    let mut winners = counts.iter().filter(|(_, n)| **n == best);
    let first = winners.next()?;
    if winners.next().is_some() {
        return None;
    }
    Some(((*first.0).clone(), best))
}

/// The converted tools of the weapons of one kind: the gun bash of the converted guns, or the tools of the
/// converted melee weapons.
fn examples(model: &CeModel, kind: ItemKind) -> Vec<&CeToolRow> {
    match kind {
        ItemKind::Ranged => model
            .guns
            .iter()
            .filter(|g| g.excluded.is_none())
            .flat_map(|g| g.tools.iter())
            .collect(),
        _ => model
            .melee
            .iter()
            .filter(|m| m.excluded.is_none())
            .flat_map(|m| m.tools.iter())
            .collect(),
    }
}

fn same_label(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// What the converted tools with one label have in common.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelHabit {
    /// How many converted tools carry the label.
    pub examples: usize,
    /// The body part group, when most of them agree on one.
    pub group: Option<String>,
    /// The pick weight, when most of them agree on one.
    pub chance_factor: Option<f64>,
    /// The capacities, when nearly all of them agree.
    pub capacities: Option<Vec<String>>,
}

/// The habit of a tool label in the converted tools of `kind`.
#[must_use]
pub fn label_habit(model: &CeModel, kind: ItemKind, label: &str) -> LabelHabit {
    let rows: Vec<&CeToolRow> = examples(model, kind)
        .into_iter()
        .filter(|r| same_label(&r.label, label))
        .collect();
    let mut habit = LabelHabit {
        examples: rows.len(),
        ..LabelHabit::default()
    };
    if rows.len() < MIN_EXAMPLES {
        return habit;
    }
    let share = |n: usize| n as f64 / rows.len() as f64;
    let groups: Vec<Option<String>> = rows
        .iter()
        .map(|r| r.linked_body_parts_group.clone())
        .collect();
    if let Some((Some(g), n)) = majority(&groups)
        && share(n) >= MIN_AGREEMENT
    {
        habit.group = Some(g);
    }
    // A pick weight is compared as written so that 1.5 and 1.50 are one value.
    let weights: Vec<Option<String>> = rows
        .iter()
        .map(|r| r.chance_factor.map(format_number))
        .collect();
    if let Some((Some(w), n)) = majority(&weights)
        && share(n) >= MIN_AGREEMENT
    {
        habit.chance_factor = w.parse().ok();
    }
    if rows.len() >= MIN_CAPACITY_EXAMPLES {
        let caps: Vec<Vec<String>> = rows.iter().map(|r| r.capacities.clone()).collect();
        if let Some((c, n)) = majority(&caps)
            && !c.is_empty()
            && share(n) >= MIN_CAPACITY_AGREEMENT
        {
            habit.capacities = Some(c);
        }
    }
    habit
}

/// The tool label that converted weapons give to a tool with exactly these capacities, other than the
/// labels in `taken`: the most frequent such label.
fn label_for_capacities(
    model: &CeModel,
    kind: ItemKind,
    capacities: &[String],
    taken: &[String],
) -> Option<String> {
    let labels: Vec<String> = examples(model, kind)
        .into_iter()
        .filter(|r| r.capacities == capacities)
        .filter(|r| !taken.iter().any(|t| same_label(t, &r.label)))
        .map(|r| r.label.clone())
        .collect();
    let (label, n) = majority(&labels)?;
    (n >= MIN_EXAMPLES).then_some(label)
}

/// A tool change that converted weapons make and the plain conversion does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restructure {
    /// The label of the tool the change concerns.
    pub tool: String,
    /// What converted weapons do, in plain words.
    pub what: String,
}

/// A derived value of the habits: the field, the value written and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HabitNote {
    /// The field, for example `tool 1 linkedBodyPartsGroup`.
    pub field: String,
    /// The value written.
    pub value: String,
    /// How it was derived.
    pub how: String,
}

/// The result of [`tool_habits`].
#[derive(Debug, Clone, PartialEq)]
pub struct HabitOutcome {
    /// The tools with the habits applied.
    pub tools: Vec<ToolValues>,
    /// What was filled in or changed, as derived values.
    pub notes: Vec<HabitNote>,
    /// The restructurings that converted weapons make. With `restructure` off they are only listed; with it
    /// on they were applied.
    pub restructures: Vec<Restructure>,
    /// For each tool a restructuring added, the label of the tool it was copied from.
    pub copies: Vec<(String, String)>,
}

/// Applies the habits of the converted tools to the tools of a patch.
///
/// - A tool without a body part group gets the group that converted tools with its label share.
/// - A tool without a pick weight gets the weight that converted tools with its label share.
/// - With `restructure` set, a tool whose capacities differ from what converted tools with its label agree
///   on is set to those capacities, and a capacity that this takes away from the tool moves to a new tool
///   when converted weapons have a tool with exactly that capacity (the muzzle of a gun), which copies the
///   numbers of the tool it came from. Without it these changes are only listed in
///   [`HabitOutcome::restructures`]: a restructured tool list is a decision of the user, taken by accepting
///   the suggestion as a tool plan.
#[must_use]
pub fn tool_habits(
    model: &CeModel,
    kind: ItemKind,
    tools: Vec<ToolValues>,
    restructure: bool,
) -> HabitOutcome {
    let mut out: Vec<ToolValues> = Vec::new();
    let mut notes: Vec<HabitNote> = Vec::new();
    let mut restructures: Vec<Restructure> = Vec::new();
    let mut copies: Vec<(String, String)> = Vec::new();
    let mut moved: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, mut tool) in tools.into_iter().enumerate() {
        let number = i.saturating_add(1);
        let habit = label_habit(model, kind, &tool.label);
        let how = |what: &str| {
            format!(
                "the {what} of the {} converted tools named {} in the library",
                habit.examples, tool.label
            )
        };
        if tool.linked_body_parts_group.is_none()
            && let Some(g) = &habit.group
        {
            notes.push(HabitNote {
                field: format!("tool {number} linkedBodyPartsGroup"),
                value: g.clone(),
                how: how("group"),
            });
            tool.linked_body_parts_group = Some(g.clone());
        }
        if tool.chance_factor.is_none()
            && let Some(w) = habit.chance_factor
        {
            notes.push(HabitNote {
                field: format!("tool {number} chanceFactor"),
                value: format_number(w),
                how: how("pick weight"),
            });
            tool.chance_factor = Some(w);
        }
        if let Some(caps) = &habit.capacities
            && !same_set(caps, &tool.capacities)
        {
            let dropped: Vec<String> = tool
                .capacities
                .iter()
                .filter(|c| !caps.contains(c))
                .cloned()
                .collect();
            let what = format!(
                "the capacities are {} in {} converted tools named {}",
                caps.join(", "),
                habit.examples,
                tool.label
            );
            // Only a pure removal splits the tool (a barrel that struck with two capacities gives its
            // poke to a muzzle); a replaced capacity (a handle that now pokes) is not a second tool.
            let removal = tool.capacities.iter().any(|c| caps.contains(c));
            if restructure {
                notes.push(HabitNote {
                    field: format!("tool {number} capacities"),
                    value: caps.join(", "),
                    how: how("capacities"),
                });
                tool.capacities.clone_from(caps);
            } else {
                restructures.push(Restructure {
                    tool: tool.label.clone(),
                    what,
                });
            }
            if removal && !dropped.is_empty() {
                moved.push((out.len(), dropped));
            }
        }
        out.push(tool);
    }
    // A capacity that a tool gave up moves to its own tool when the library has a name for such a tool.
    let mut added: Vec<ToolValues> = Vec::new();
    for (index, dropped) in moved {
        let taken: Vec<String> = out
            .iter()
            .chain(added.iter())
            .map(|t| t.label.clone())
            .collect();
        let Some(label) = label_for_capacities(model, kind, &dropped, &taken) else {
            continue;
        };
        let Some(source) = out.get(index).cloned() else {
            continue;
        };
        let habit = label_habit(model, kind, &label);
        let what = format!(
            "a new tool {label} for {}, as {} converted tools do",
            dropped.join(", "),
            habit.examples
        );
        if !restructure {
            restructures.push(Restructure {
                tool: source.label.clone(),
                what,
            });
            continue;
        }
        let number = out.len().saturating_add(added.len()).saturating_add(1);
        notes.push(HabitNote {
            field: format!("tool {number} label"),
            value: label.clone(),
            how: format!(
                "a new tool for {} taken from {}, as {} converted tools do",
                dropped.join(", "),
                source.label,
                habit.examples
            ),
        });
        copies.push((label.clone(), source.label.clone()));
        added.push(ToolValues {
            label,
            capacities: dropped,
            chance_factor: habit.chance_factor,
            linked_body_parts_group: habit.group.or(source.linked_body_parts_group.clone()),
            ..source
        });
    }
    out.extend(added);
    HabitOutcome {
        tools: out,
        notes,
        restructures,
        copies,
    }
}

/// [`tool_habits`] with the notes reported as derived values and the listed restructurings as hints in
/// `diagnostics`.
pub fn apply_tool_habits(
    model: &CeModel,
    kind: ItemKind,
    tools: Vec<ToolValues>,
    restructure: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ToolValues> {
    let outcome = tool_habits(model, kind, tools, restructure);
    for n in outcome.notes {
        diagnostics.push(DERIVED_VALUE.diagnostic(
            &n.field,
            &[("field", &n.field), ("value", &n.value), ("how", &n.how)],
        ));
    }
    for r in outcome.restructures {
        diagnostics.push(TOOL_RESTRUCTURE.diagnostic(
            "/tools",
            &[("tool", r.tool.as_str()), ("what", r.what.as_str())],
        ));
    }
    outcome.tools
}

fn same_set(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.contains(x))
}

/// What the converted guns of one weapon class do in their fire modes component.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FireModeHabit {
    /// The AI aim mode, when most guns of the class agree.
    pub aim_mode: Option<String>,
    /// The `aiUseBurstMode` flag, when most guns of the class write it and agree.
    pub use_burst_mode: Option<bool>,
    /// How many converted guns carry the class tag.
    pub examples: usize,
}

fn guns_of_class<'a>(model: &'a CeModel, class_tag: &'a str) -> impl Iterator<Item = &'a CeGun> {
    model
        .guns
        .iter()
        .filter(move |g| g.excluded.is_none() && g.ai_class.as_deref() == Some(class_tag))
}

/// The fire mode habit of a weapon class tag.
#[must_use]
pub fn fire_mode_habit(model: &CeModel, class_tag: &str) -> FireModeHabit {
    let guns: Vec<&CeGun> = guns_of_class(model, class_tag).collect();
    let mut habit = FireModeHabit {
        examples: guns.len(),
        ..FireModeHabit::default()
    };
    if guns.len() < MIN_EXAMPLES {
        return habit;
    }
    let modes: Vec<Option<String>> = guns.iter().map(|g| g.ai_aim_mode.clone()).collect();
    if let Some((Some(m), n)) = majority(&modes)
        && n as f64 / guns.len() as f64 >= MIN_AGREEMENT
    {
        habit.aim_mode = Some(m);
    }
    let bursts: Vec<bool> = guns.iter().filter_map(|g| g.use_burst_mode).collect();
    if bursts.len() * 2 >= guns.len()
        && let Some((b, n)) = majority(&bursts)
        && n as f64 / bursts.len() as f64 >= MIN_AGREEMENT
    {
        habit.use_burst_mode = Some(b);
    }
    habit
}

/// The weapon tags that converted guns of the given class added to their vanilla twin, beyond the class tag
/// itself, ordered by how many guns carry them. Used for hints only: these tags (loadout groups, one handed
/// marks, sidearm marks, bipod marks) are choices the user makes, never written on their own.
#[must_use]
pub fn companion_tags(model: &CeModel, class_tag: &str) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for g in guns_of_class(model, class_tag) {
        let mut seen: Vec<&String> = Vec::new();
        for t in &g.weapon_tags {
            if t != class_tag && !g.twin_tags.contains(t) && !seen.contains(&t) {
                seen.push(t);
                *counts.entry(t.clone()).or_default() += 1;
            }
        }
    }
    let mut v: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(_, n)| *n >= MIN_EXAMPLES)
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter().map(|(t, _)| t).collect()
}

/// A tool child that the converted tools drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedField {
    /// The child name, for example `labelUsedInLogging`.
    pub field: String,
    /// Converted weapons whose vanilla twin has it and whose converted tools lack it.
    pub dropped: usize,
    /// Converted weapons whose vanilla twin has it.
    pub total: usize,
}

/// The tool children that converted weapons of `kind` consistently drop: children that the vanilla twins
/// carry, that at least [`MIN_EXAMPLES`] twins have, and that at least [`MIN_CAPACITY_AGREEMENT`] of those
/// weapons no longer carry after the conversion. A child that converted tools keep is never listed.
#[must_use]
pub fn dropped_tool_fields(model: &CeModel, kind: ItemKind) -> Vec<DroppedField> {
    let rows: Vec<&crate::ce::reader::extras::WeaponExtras> = match kind {
        ItemKind::Ranged => model
            .guns
            .iter()
            .filter(|g| g.excluded.is_none())
            .filter_map(|g| model.extras.gun(&g.def_name))
            .collect(),
        ItemKind::Melee => model
            .melee
            .iter()
            .filter(|m| m.excluded.is_none())
            .filter_map(|m| model.extras.melee_weapon(&m.def_name))
            .collect(),
    };
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in &rows {
        for field in &row.twin_tool_fields {
            let entry = counts.entry(field.as_str()).or_default();
            entry.1 += 1;
            if !row.tool_fields.contains(field) {
                entry.0 += 1;
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, (dropped, total))| {
            *total >= MIN_EXAMPLES && *dropped as f64 / *total as f64 >= MIN_CAPACITY_AGREEMENT
        })
        .map(|(field, (dropped, total))| DroppedField {
            field: field.to_owned(),
            dropped,
            total,
        })
        .collect()
}

/// A companion tag of a weapon class with the number of guns of the class that carry it and the size of
/// the class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagShare {
    /// The tag.
    pub tag: String,
    /// Guns of the class that carry it.
    pub carried: usize,
    /// Guns of the class.
    pub total: usize,
}

/// The tags beyond the class tag that at least [`MIN_AGREEMENT`] of the converted guns of the class added
/// to their vanilla twin, with at least [`MIN_EXAMPLES`] guns. Ordered by how many guns carry them.
#[must_use]
pub fn companion_shares(model: &CeModel, class_tag: &str) -> Vec<TagShare> {
    let total = guns_of_class(model, class_tag).count();
    if total < MIN_EXAMPLES {
        return Vec::new();
    }
    companion_tags(model, class_tag)
        .into_iter()
        .filter_map(|tag| {
            let carried = guns_of_class(model, class_tag)
                .filter(|g| g.weapon_tags.contains(&tag) && !g.twin_tags.contains(&tag))
                .count();
            (carried as f64 / total as f64 >= MIN_AGREEMENT).then_some(TagShare {
                tag,
                carried,
                total,
            })
        })
        .collect()
}

/// The verb recoil pattern that most converted guns of the class write, with the number of guns that write
/// it and the size of the class. Needs [`MIN_EXAMPLES`] guns that write one and [`MIN_AGREEMENT`] of the
/// class.
#[must_use]
pub fn recoil_pattern_habit(model: &CeModel, class_tag: &str) -> Option<(String, usize, usize)> {
    let guns: Vec<&CeGun> = guns_of_class(model, class_tag).collect();
    let written: Vec<String> = guns
        .iter()
        .filter_map(|g| model.extras.gun(&g.def_name)?.recoil_pattern.clone())
        .collect();
    let (pattern, n) = majority(&written)?;
    (n >= MIN_EXAMPLES && n as f64 / guns.len().max(1) as f64 >= MIN_AGREEMENT).then_some((
        pattern,
        n,
        guns.len(),
    ))
}

/// How many converted guns of the class load one round at a time, and how many guns the class has, when
/// the habit holds: at least [`MIN_EXAMPLES`] guns do and they are at least [`MIN_AGREEMENT`] of the class.
#[must_use]
pub fn reload_one_at_a_time_habit(model: &CeModel, class_tag: &str) -> Option<(usize, usize)> {
    let guns: Vec<&CeGun> = guns_of_class(model, class_tag).collect();
    let n = guns.iter().filter(|g| g.reload_one_at_a_time).count();
    (n >= MIN_EXAMPLES && n as f64 / guns.len().max(1) as f64 >= MIN_AGREEMENT)
        .then_some((n, guns.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majority_needs_a_single_winner() {
        assert_eq!(majority(&["a", "a", "b"]), Some(("a", 2)));
        assert_eq!(majority(&["a", "b"]), None);
        assert_eq!(majority::<u8>(&[]), None);
    }

    #[test]
    fn labels_compare_without_case_or_edges() {
        assert!(same_label(" Stock", "stock "));
        assert!(!same_label("stock", "barrel"));
    }

    #[test]
    fn sets_compare_without_order() {
        assert!(same_set(
            &["Blunt".into(), "Poke".into()],
            &["Poke".into(), "Blunt".into()]
        ));
        assert!(!same_set(&["Blunt".into()], &["Poke".into()]));
    }
}
