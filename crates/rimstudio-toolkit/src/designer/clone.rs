//! Flow C of the designer: clone and adjust an existing weapon (`designer_clone`), the diff of a clone
//! against its source (`designer_clone_diff`) and the structure defaults of a new weapon
//! (`designer_structure_defaults`).
//!
//! A clone is a vanilla draft. Every field the design spec models is copied from the source def as the
//! snapshot resolved it, each number marked `anchor`; stats the def only inherits from its parent stay
//! inherited (the clone keeps the same parent, so it inherits them again); the source is recorded as the
//! first anchor and in the draft (`clonedFrom`), and the calibration mode is `anchored`, so the fit meter
//! compares the clone with its source. The Combat Extended block is never created: a source that carries a
//! Combat Extended conversion in the loaded defs is refused with a plain reason, because the designer writes
//! vanilla definitions and a converted def is not their twin. The source def is never edited.
//!
//! The diff re-reads the source from the same snapshot, so it is a pure function of the draft and the loaded
//! defs.

use std::collections::BTreeMap;

use rimstudio_design::ce::reader::is_conversion;
use rimstudio_design::classes::{ItemKind as PoolKind, Pool};
use rimstudio_design::model::{
    Anchor, CalibrationMode, CostEntry, DesignSpec, Draft, ItemKind, ProjectileChoice, Sourced,
    StuffSpec, ValueSource,
};
use rimstudio_design::reader::SpecReading;
use rimstudio_design::validation::{diagnostic_field, validate_refs, validate_vanilla};
use rimstudio_ipc_types::designer::{
    CloneChangeDto, DesignerCloneDiffRequest, DesignerCloneDiffResponse, DesignerCloneRequest,
    DesignerCloneResponse, DesignerDraftSaveRequest, DesignerStructureDefaultsRequest,
    DesignerStructureDefaultsResponse, ReadoutDeltaDto, StructureReferenceDto,
};
use rimstudio_workspace::snapshot::DefRef;
use serde_json::Value;

use super::ctx::{Ctx, Engine};
use super::drafts::{check_part, draft_load, draft_save};
use super::dto::{draft_from_dto, draft_to_dto, pool_kind};
use super::own::{free_name, give_own_projectile, read_with_own};
use super::preview::{ReadoutEnv, readouts, spec_stats};
use crate::error::{ToolkitError, ToolkitResult};

/// The def type every weapon belongs to.
const WEAPON_TYPE: &str = "ThingDef";

/// Pointers of the spec that are expected to differ between a clone and its source and are therefore not
/// reported as changes: the names of the new item.
const NAME_POINTERS: [&str; 3] = [
    "/identity/defName",
    "/identity/label",
    "/identity/modPrefix",
];

// ---------------------------------------------------------------------------------------------------
// Reading the source
// ---------------------------------------------------------------------------------------------------

/// Reads a weapon of the loaded defs into a spec marked `anchor`, the way a clone copies it.
///
/// # Errors
///
/// [`ToolkitError::ReferenceUnavailable`] when the loaded defs have no such weapon,
/// [`ToolkitError::InvalidDraft`] when the def carries a Combat Extended conversion,
/// [`ToolkitError::Design`] when the def is not a weapon the designer can read.
fn read_source(engine: &Engine, name: &str) -> ToolkitResult<SpecReading> {
    let snapshot = engine.snapshot();
    let record = snapshot
        .record(&DefRef::new(WEAPON_TYPE, name))
        .ok_or_else(|| ToolkitError::ReferenceUnavailable {
            reason: format!("the loaded defs have no weapon called {name}"),
        })?;
    if is_conversion(&record.node) {
        return Err(ToolkitError::invalid_draft(format!(
            "{name} carries a Combat Extended conversion in the loaded defs; the designer writes vanilla \
             definitions, so clone it from a game setup without Combat Extended loaded"
        )));
    }
    let reading = read_with_own(engine, record, ValueSource::Anchor)?;
    Ok(with_pool_role(engine, name, reading))
}

/// Gives a reading the role the reference pool derived for the def when the def has no explicit one, so the
/// spec is complete for the planner (the role is required input and is never written into the def).
fn with_pool_role(engine: &Engine, name: &str, mut reading: SpecReading) -> SpecReading {
    if reading.spec.role.is_none()
        && let Ok(kind) = pool_kind(reading.spec.kind)
        && let Ok(pool) = engine.pool(kind)
        && let Some(item) = pool.items.iter().find(|i| i.id == name)
    {
        reading.spec.role = Some(item.role.clone());
    }
    reading
}

// ---------------------------------------------------------------------------------------------------
// Clone
// ---------------------------------------------------------------------------------------------------

/// A label from a definition name: underscores and hyphens become spaces, a capital after a lower case
/// letter starts a new word, everything lower case.
fn label_of(def_name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in def_name.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            prev_lower = false;
        } else {
            if ch.is_uppercase() && prev_lower {
                out.push(' ');
            }
            prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
            out.extend(ch.to_lowercase());
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The new definition name: trimmed, and with the mod prefix when one is given and the name lacks it.
fn applied_name(def_name: &str, prefix: &str) -> String {
    let name = def_name.trim();
    if prefix.is_empty() || name.is_empty() || name.starts_with(&format!("{prefix}_")) {
        name.to_owned()
    } else {
        format!("{prefix}_{name}")
    }
}

/// The pointers of the required fields a clone lacks because its source lacks them. They are accepted: the
/// clone is written without them, as the source is.
fn missing_in_source(spec: &DesignSpec) -> Vec<String> {
    let mut out: Vec<String> = validate_vanilla(spec)
        .iter()
        .filter(|d| d.code.as_str() == "design.required-missing")
        .filter_map(|d| diagnostic_field(d).map(str::to_owned))
        .filter(|f| !f.starts_with("/identity/"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The error diagnostics about the names of the new item, as one plain reason.
fn name_problems(engine: &Engine, spec: &DesignSpec) -> Option<String> {
    let mut all = validate_vanilla(spec);
    all.extend(validate_refs(spec, &engine.lookup()));
    let reasons: Vec<String> = all
        .iter()
        .filter(|d| d.severity == rimstudio_core::diag::Severity::Error)
        .filter(|d| diagnostic_field(d).is_some_and(|f| f.starts_with("/identity/")))
        .map(|d| d.message.clone())
        .collect();
    (!reasons.is_empty()).then(|| reasons.join("; "))
}

/// The notes of a clone: what the reader could not carry and what the copy shares with its source.
fn clone_notes(source: &str, reading: &SpecReading, spec: &DesignSpec) -> Vec<String> {
    let mut notes = reading.notes.clone();
    if spec.work_to_make.is_none() {
        notes.push(format!(
            "{source} has no work to make; the designer needs one before the clone can be written"
        ));
    }
    if spec.cost_list.is_empty() && spec.stuff.is_none() {
        notes.push(format!(
            "{source} has no cost list and no stuff (it cannot be crafted); the designer needs one of \
             them before the clone can be written"
        ));
    }
    if let Some(ProjectileChoice::Reference(projectile)) =
        spec.ranged.as_ref().and_then(|r| r.projectile.as_ref())
    {
        notes.push(format!(
            "the clone points at the projectile {projectile} of {source}; the damage and armor \
             penetration live in that projectile, so changing them here does not change the written \
             definition until the clone has a projectile of its own"
        ));
    }
    notes
}

/// Clones a weapon of the loaded defs into a new draft and stores it (`designer_clone`).
///
/// The draft copies every field of the source that the design spec models, with the numbers marked `anchor`
/// and inherited values left inherited. The source is the first anchor and `clonedFrom`; the calibration
/// mode is anchored. The Combat Extended toggle is off.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad project id, a new name that is empty, malformed or already used
/// by a loaded def, or a source with a Combat Extended conversion; [`ToolkitError::ReferenceUnavailable`]
/// without a game install or when the source is not among the loaded defs; [`ToolkitError::Design`] when
/// the source is not a readable weapon; store errors from saving the draft.
pub fn clone_draft(ctx: &Ctx, req: DesignerCloneRequest) -> ToolkitResult<DesignerCloneResponse> {
    check_part("project id", &req.project_id)?;
    let engine = ctx.require_engine()?;
    let source = req.source.trim();
    let reading = read_source(&engine, source)?;
    let source_label = reading.spec.identity.label.clone();

    let prefix = req.mod_prefix.as_deref().map(str::trim).unwrap_or_default();
    let def_name = applied_name(&req.def_name, prefix);
    let label = match req.label.as_deref().map(str::trim) {
        Some(l) if !l.is_empty() => l.to_owned(),
        _ => label_of(
            def_name
                .strip_prefix(&format!("{prefix}_"))
                .unwrap_or(&def_name),
        ),
    };
    let mut spec = reading.clone().clone_as(&def_name, &label);
    spec.identity.mod_prefix = prefix.to_owned();
    spec.ce = None;
    if let Some(reason) = name_problems(&engine, &spec) {
        return Err(ToolkitError::invalid_draft(format!(
            "the new name cannot be used: {reason}"
        )));
    }
    let mut extra_notes = Vec::new();
    if req.own_projectile.unwrap_or(true) && spec.kind == ItemKind::Ranged {
        let name = free_name(&engine, &def_name, prefix);
        match give_own_projectile(&engine, &mut spec, &name) {
            Ok(note) => extra_notes.push(note),
            Err(reason) => extra_notes.push(format!(
                "the weapon keeps pointing at the projectile of {source}: {reason}"
            )),
        }
    }
    spec.accepted_missing = missing_in_source(&spec);
    let mut notes = clone_notes(source, &reading, &spec);
    notes.extend(extra_notes);

    let mut draft = Draft::new(spec);
    draft.calibration = CalibrationMode::Anchored;
    draft.anchors = vec![Anchor {
        def_name: source.to_owned(),
        label: Some(source_label),
    }];
    draft.cloned_from = Some(source.to_owned());
    let saved = draft_save(
        ctx,
        DesignerDraftSaveRequest {
            project_id: req.project_id.clone(),
            id: None,
            draft: draft_to_dto(&draft)?,
        },
    )?;
    let entry = draft_load(ctx, &req.project_id, &saved.id)?;
    Ok(DesignerCloneResponse { entry, notes })
}

// ---------------------------------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------------------------------

/// A leaf value for comparing: the number or text behind a `{value, source}` pair, else the value itself.
fn collapse(value: &Value) -> Option<&Value> {
    let map = value.as_object()?;
    let inner = map.get("value")?;
    (map.len() == 2 && map.contains_key("source")).then_some(inner)
}

/// Flattens a JSON value into `(pointer, leaf)` pairs in traversal order. A sourced number is a leaf; an
/// empty object or list contributes nothing.
fn flatten(value: &Value, pointer: &str, out: &mut Vec<(String, Value)>) {
    if let Some(inner) = collapse(value) {
        out.push((pointer.to_owned(), inner.clone()));
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                flatten(inner, &format!("{pointer}/{escaped}"), out);
            }
        }
        Value::Array(items) => {
            for (i, inner) in items.iter().enumerate() {
                flatten(inner, &format!("{pointer}/{i}"), out);
            }
        }
        Value::Null => {}
        leaf => out.push((pointer.to_owned(), leaf.clone())),
    }
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) if a.is_number() && b.is_number() => (x - y).abs() <= 1e-12,
        _ => a == b,
    }
}

/// Splits a camel case word into lower case words.
fn words_of(segment: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in segment.chars() {
        if ch.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.extend(ch.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// A short label for a spec pointer: `/tools/0/power` reads `tool 1 power`, `/weaponTags/1` reads
/// `weapon tag 2`.
fn label_of_pointer(pointer: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for raw in pointer.split('/').skip(1) {
        let segment = raw.replace("~1", "/").replace("~0", "~");
        if let Ok(index) = segment.parse::<usize>() {
            if let Some(last) = words.last_mut()
                && last.ends_with('s')
                && last.len() > 1
            {
                last.pop();
            }
            words.push((index + 1).to_string());
        } else {
            words.extend(words_of(&segment));
        }
    }
    words.join(" ")
}

fn is_ignored(pointer: &str) -> bool {
    NAME_POINTERS.contains(&pointer)
        || pointer == "/ce"
        || pointer.starts_with("/ce/")
        || pointer.starts_with("/acceptedMissing")
}

/// The changed fields between two specs, as old and new leaf values.
fn changes_between(source: &DesignSpec, clone: &DesignSpec) -> ToolkitResult<Vec<CloneChangeDto>> {
    let encode = |spec: &DesignSpec| {
        serde_json::to_value(spec)
            .map_err(|e| ToolkitError::internal(format!("a spec cannot be encoded: {e}")))
    };
    let (old_value, new_value) = (encode(source)?, encode(clone)?);
    let (mut old_leaves, mut new_leaves) = (Vec::new(), Vec::new());
    flatten(&old_value, "", &mut old_leaves);
    flatten(&new_value, "", &mut new_leaves);
    let old_map: BTreeMap<&str, &Value> = old_leaves.iter().map(|(p, v)| (p.as_str(), v)).collect();
    let new_map: BTreeMap<&str, &Value> = new_leaves.iter().map(|(p, v)| (p.as_str(), v)).collect();
    let mut order: Vec<&str> = old_leaves.iter().map(|(p, _)| p.as_str()).collect();
    order.extend(
        new_leaves
            .iter()
            .map(|(p, _)| p.as_str())
            .filter(|p| !old_map.contains_key(p)),
    );
    let mut out = Vec::new();
    for pointer in order {
        if is_ignored(pointer) {
            continue;
        }
        let (old, new) = (old_map.get(pointer), new_map.get(pointer));
        let unchanged = matches!((old, new), (Some(a), Some(b)) if same(a, b));
        if unchanged {
            continue;
        }
        out.push(CloneChangeDto {
            field: pointer.to_owned(),
            label: label_of_pointer(pointer),
            old: old.map(|v| (*v).clone()),
            new: new.map(|v| (*v).clone()),
        });
    }
    Ok(out)
}

/// The exact readouts of both specs, paired by key in the display order of the clone's readouts.
fn readout_deltas(
    engine: &Engine,
    source: &DesignSpec,
    clone: &DesignSpec,
) -> Vec<ReadoutDeltaDto> {
    let armor: Option<&[f64]> = engine.pools().ok().map(|p| p.set.armor.ratings.as_slice());
    let env = ReadoutEnv {
        armor,
        defs: Some(engine.databases().as_ref()),
        ce: None,
    };
    let old = readouts(source, &env);
    readouts(clone, &env)
        .into_iter()
        .map(|new| {
            let before = old.iter().find(|r| r.key == new.key).and_then(|r| r.value);
            ReadoutDeltaDto {
                delta: match (before, new.value) {
                    (Some(a), Some(b)) => Some(b - a).filter(|d| d.is_finite()),
                    _ => None,
                },
                key: new.key,
                group: new.group,
                unit: new.unit,
                old: before,
                new: new.value,
            }
        })
        .collect()
}

/// The changed fields of a clone against its source with the effect on the exact readouts
/// (`designer_clone_diff`).
///
/// The source is the weapon named by the draft's `clonedFrom`, read again from the loaded defs the way the
/// clone copied it. The new name and label are not changes. The readouts are the vanilla ones, whatever the
/// draft's Combat Extended block says.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable draft or one that is not a clone,
/// [`ToolkitError::ReferenceUnavailable`] without a game install or when the source is no longer loaded.
pub fn clone_diff(
    ctx: &Ctx,
    req: DesignerCloneDiffRequest,
) -> ToolkitResult<DesignerCloneDiffResponse> {
    let draft = draft_from_dto(&req.draft)?;
    let Some(source_name) = draft.cloned_from.clone() else {
        return Err(ToolkitError::invalid_draft(
            "the draft was not cloned from a weapon, so there is nothing to compare it with",
        ));
    };
    let engine = ctx.require_engine()?;
    let mut reading = read_source(&engine, &source_name)?;
    // A clone with a projectile of its own is compared with the source read the same way, so that only
    // real edits show up.
    if let Some(ProjectileChoice::Inline(p)) = draft
        .spec
        .ranged
        .as_ref()
        .and_then(|r| r.projectile.as_ref())
        && p.copied_from.is_some()
    {
        // A source whose projectile cannot be copied stays as read; the difference then shows.
        let _ = give_own_projectile(&engine, &mut reading.spec, &p.def_name);
    }
    let source = &reading.spec;
    let clone = &draft.spec;
    let changes = changes_between(source, clone)?;
    let mut notes = Vec::new();
    let damage_changed = changes
        .iter()
        .any(|c| c.field == "/ranged/damage" || c.field == "/ranged/armorPenetration");
    if damage_changed
        && let Some(ProjectileChoice::Reference(projectile)) =
            clone.ranged.as_ref().and_then(|r| r.projectile.as_ref())
    {
        notes.push(format!(
            "the damage is read from the projectile {projectile}, which the clone shares with \
             {source_name}; the written definition only points at it, so the new damage is not written \
             until the clone has a projectile of its own"
        ));
    }
    Ok(DesignerCloneDiffResponse {
        source: source_name,
        source_label: source.identity.label.clone(),
        readouts: readout_deltas(&engine, source, clone),
        changes,
        notes,
    })
}

// ---------------------------------------------------------------------------------------------------
// Structure defaults
// ---------------------------------------------------------------------------------------------------

/// The reference stats compared to find the nearest weapon, per pool kind.
fn comparison_stats(kind: PoolKind) -> &'static [&'static str] {
    match kind {
        PoolKind::Melee => &["swing_damage", "cooldown", "mass"],
        _ => &["damage", "warmup", "cooldown", "range", "mass"],
    }
}

/// The pool item nearest to the numbers the draft holds, preferring weapons of its role and tier.
///
/// The distance is the root of the summed squared differences, each stat scaled by its spread in the pool;
/// stats the draft or the item lacks are left out. Ties go to the smaller def name. `None` when the draft
/// has no number to compare.
fn nearest<'a>(
    pool: &'a Pool,
    spec: &DesignSpec,
) -> Option<&'a rimstudio_design::classes::PoolItem> {
    let held = spec_stats(spec);
    let stats = comparison_stats(pool.kind);
    let spread = |stat: &str| {
        let values: Vec<f64> = pool.items.iter().filter_map(|i| i.stat(stat)).collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let span = max - min;
        if span.is_finite() && span > 0.0 {
            span
        } else {
            1.0
        }
    };
    let same_role: Vec<&_> = match spec.role.as_deref() {
        Some(role) => pool
            .items
            .iter()
            .filter(|i| i.role.eq_ignore_ascii_case(role))
            .collect(),
        None => Vec::new(),
    };
    let same_class: Vec<&_> = match spec.tech_level {
        Some(tier) => same_role
            .iter()
            .copied()
            .filter(|i| i.tier == tier.index())
            .collect(),
        None => Vec::new(),
    };
    let candidates: Vec<&_> = if !same_class.is_empty() {
        same_class
    } else if !same_role.is_empty() {
        same_role
    } else {
        pool.items.iter().collect()
    };
    candidates
        .into_iter()
        .filter_map(|item| {
            let mut sum = 0.0;
            let mut used = 0_u32;
            for stat in stats {
                if let (Some(a), Some(b)) = (held.get(*stat).copied(), item.stat(stat)) {
                    let d = (a - b) / spread(stat);
                    sum += d * d;
                    used += 1;
                }
            }
            (used > 0).then(|| (sum.sqrt(), item))
        })
        .min_by(|(da, a), (db, b)| da.total_cmp(db).then_with(|| a.id.cmp(&b.id)))
        .map(|(_, item)| item)
}

fn suggested(value: Option<Sourced<f64>>) -> Option<Sourced<f64>> {
    value.map(|v| Sourced::new(v.value, ValueSource::Suggested))
}

/// Fills the empty structure of a new weapon from the nearest reference weapon
/// (`designer_structure_defaults`): the parent base, the projectile of a gun, the cost list and the stuff of
/// a melee weapon.
///
/// The nearest weapon is found by the numbers the draft already holds (see the pool of its kind). The
/// values are suggestions: a number that has a source is marked `suggested`, never `typed`, and a field that
/// already holds a value (typed or not) is never replaced. The response says so in plain words.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable draft, [`ToolkitError::ReferenceUnavailable`] without a
/// game install or without reference weapons of the kind.
pub fn structure_defaults(
    ctx: &Ctx,
    req: DesignerStructureDefaultsRequest,
) -> ToolkitResult<DesignerStructureDefaultsResponse> {
    let mut draft = draft_from_dto(&req.draft)?;
    let engine = ctx.require_engine()?;
    let kind = pool_kind(draft.kind)?;
    let pool = engine.pool(kind)?;
    let Some(item) = nearest(pool, &draft.spec) else {
        return Ok(DesignerStructureDefaultsResponse {
            draft: draft_to_dto(&draft)?,
            reference: None,
            filled: Vec::new(),
            notes: vec![
                "no structure was suggested: the draft holds no number to compare with the reference \
                 weapons yet"
                    .to_owned(),
            ],
        });
    };
    let (id, label) = (item.id.clone(), item.label.clone());
    let reading = read_source(&engine, &id)?;
    let reference = reading.spec;
    let spec = &mut draft.spec;
    let mut filled = Vec::new();
    if spec.parent.is_none()
        && let Some(parent) = reference.parent.clone()
    {
        spec.parent = Some(parent);
        filled.push("/parent".to_owned());
    }
    if spec.kind == ItemKind::Ranged
        && let (Some(ours), Some(theirs)) = (spec.ranged.as_mut(), reference.ranged.as_ref())
        && ours.projectile.is_none()
        && let Some(projectile) = theirs.projectile.clone()
    {
        ours.projectile = Some(projectile);
        filled.push("/ranged/projectile".to_owned());
    }
    if spec.cost_list.is_empty() && !reference.cost_list.is_empty() {
        spec.cost_list = reference
            .cost_list
            .iter()
            .map(|c| CostEntry::new(c.def_name.clone(), c.count))
            .collect();
        filled.push("/costList".to_owned());
    }
    if spec.kind == ItemKind::Melee
        && spec.stuff.is_none()
        && let Some(stuff) = &reference.stuff
    {
        spec.stuff = Some(StuffSpec {
            categories: stuff.categories.clone(),
            count: suggested(stuff.count),
        });
        filled.push("/stuff".to_owned());
    }
    let notes = if filled.is_empty() {
        vec![format!(
            "nothing to fill: the draft already holds the structure fields that {label} ({id}) would \
             have suggested"
        )]
    } else {
        vec![format!(
            "the parent, projectile, cost list and stuff were copied from {label} ({id}), the nearest \
             reference weapon by the numbers of the draft; they are suggestions, not typed values, so \
             check them before writing"
        )]
    };
    Ok(DesignerStructureDefaultsResponse {
        draft: draft_to_dto(&draft)?,
        reference: Some(StructureReferenceDto {
            def_name: id,
            label,
        }),
        filled,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    #[case("/ranged/damage", "ranged damage")]
    #[case("/tools/0/power", "tool 1 power")]
    #[case("/weaponTags/1", "weapon tag 2")]
    #[case("/ranged/accuracy/touch", "ranged accuracy touch")]
    #[case("/costList/2/count", "cost list 3 count")]
    #[case("/mass", "mass")]
    fn pointers_read_as_short_labels(#[case] pointer: &str, #[case] label: &str) {
        assert_eq!(label_of_pointer(pointer), label);
    }

    #[rstest]
    #[case("RS_TestRifle", "", "RS_TestRifle")]
    #[case("TestRifle", "RS", "RS_TestRifle")]
    #[case("RS_TestRifle", "RS", "RS_TestRifle")]
    #[case("  RS_X ", "", "RS_X")]
    #[case("", "RS", "")]
    fn the_prefix_is_applied_once(#[case] name: &str, #[case] prefix: &str, #[case] want: &str) {
        assert_eq!(applied_name(name, prefix), want);
    }

    #[test]
    fn labels_come_from_names() {
        assert_eq!(label_of("TestRifle"), "test rifle");
        assert_eq!(label_of("rs_test-rifle"), "rs test rifle");
        assert_eq!(label_of(""), "");
    }

    #[test]
    fn sourced_numbers_collapse_to_their_value_and_empty_lists_vanish() {
        let value = json!({"ranged": {"damage": {"value": 10.0, "source": "anchor"}},
            "weaponTags": ["a", "b"], "tools": [], "role": null});
        let mut out = Vec::new();
        flatten(&value, "", &mut out);
        let pointers: Vec<&str> = out.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            pointers,
            vec!["/ranged/damage", "/weaponTags/0", "/weaponTags/1"]
        );
        assert_eq!(out[0].1, json!(10.0));
    }

    #[test]
    fn numbers_compare_by_value_not_by_json_spelling() {
        assert!(same(&json!(10), &json!(10.0)));
        assert!(!same(&json!(10), &json!(10.5)));
        assert!(same(&json!("a"), &json!("a")));
        assert!(!same(&json!("1"), &json!(1)));
    }

    mod properties {
        use proptest::prelude::*;
        use rimstudio_design::model::{DesignSpec, ScalarField, ValueSource};

        use super::super::{changes_between, label_of_pointer};

        fn spec_with(damage: f64, mass: f64) -> DesignSpec {
            let mut spec = DesignSpec::new_ranged("RS_P", "p");
            spec.offer(ScalarField::Damage, damage, ValueSource::Anchor);
            spec.offer(ScalarField::Mass, mass, ValueSource::Anchor);
            spec
        }

        proptest! {
            #[test]
            fn a_spec_never_differs_from_itself(damage in 0.5_f64..200.0, mass in 0.1_f64..50.0) {
                let spec = spec_with(damage, mass);
                prop_assert!(changes_between(&spec, &spec).unwrap().is_empty());
            }

            #[test]
            fn changing_one_number_lists_exactly_that_field(
                a in 0.5_f64..200.0,
                b in 0.5_f64..200.0,
                mass in 0.1_f64..50.0,
            ) {
                let changes = changes_between(&spec_with(a, mass), &spec_with(b, mass)).unwrap();
                if (a - b).abs() <= 1e-12 {
                    prop_assert!(changes.is_empty());
                } else {
                    prop_assert_eq!(changes.len(), 1);
                    prop_assert_eq!(changes[0].field.as_str(), "/ranged/damage");
                }
            }

            #[test]
            fn a_difference_is_symmetric_in_its_fields(a in 0.5_f64..200.0, b in 0.5_f64..200.0) {
                let (x, y) = (spec_with(a, 2.0), spec_with(b, 2.0));
                let forward: Vec<String> = changes_between(&x, &y).unwrap().into_iter().map(|c| c.field).collect();
                let backward: Vec<String> = changes_between(&y, &x).unwrap().into_iter().map(|c| c.field).collect();
                prop_assert_eq!(forward, backward);
            }

            #[test]
            fn pointer_labels_never_panic(pointer in ".{0,40}") {
                let _ = label_of_pointer(&pointer);
            }
        }
    }
}
