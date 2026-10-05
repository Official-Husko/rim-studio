//! The documented merge of the Combat Extended gun conversion operation, typed.
//!
//! The shared def engine keeps this operation as an unknown class (decision D-020). The designer needs its
//! effect: to read converted guns as the game sees them, and to dry run a generated patch. This module holds
//! three things:
//!
//! * [`MakeGunSpec`]: the parameters of one operation, parsed from its raw `Operation` element (also the
//!   input of the lint rules about incomplete conversions);
//! * [`merge_into_def`]: the merge itself on an owned `ThingDef` tree, written from the operation's observed
//!   behaviour (statBases: vanilla accuracy stats deleted, same named stats replaced, new ones appended;
//!   costList: cleared then written; Properties: vanilla shoot verbs removed, one converted verb appended;
//!   AmmoUser and FireModes: components appended; weaponTags and weaponClasses: children appended;
//!   researchPrerequisite: written into the recipe maker; texPath: set with the single graphic class). The
//!   merge is not idempotent: a second run appends a second verb, comps and tags;
//! * [`MakeGunOp`]: a [`CustomPatchOp`] that applies the merge to the unified document, so a load that
//!   registers it ([`custom_registry`]) resolves converted guns.
//!
//! The merge is applied to a copy of the target and written back in place, so the def keeps its position,
//! its mod and its file; the rewritten children carry the operation as their origin.

use std::collections::BTreeMap;

use rimstudio_core::diag::{DiagCode, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::{Child, Node};
use rimstudio_defs::{
    CustomPatchOp, CustomRegistry, PatchCtx, PatchError, PatchReport, SettingsConditional,
    UnifiedDoc,
};

use super::names::CeClassNames;
use crate::reader::access::{class_attr, text_of};

/// Diagnostic: the operation names no def or a def that does not exist.
pub const MAKEGUN_MISSING_DEF: &str = "ce.makegun-missing-def";

/// Vanilla stats the merge deletes before it writes the converted stats.
pub const VANILLA_ACCURACY_STATS: [&str; 4] = [
    "AccuracyTouch",
    "AccuracyShort",
    "AccuracyMedium",
    "AccuracyLong",
];

/// Shoot verb classes the merge removes from `verbs`.
pub const VANILLA_SHOOT_VERBS: [&str; 3] =
    ["Verb_Shoot", "Verb_ShootOneUse", "Verb_LaunchProjectile"];

/// Parameters the merge does not support (weapon platforms); they are listed, never applied.
const UNSUPPORTED_FIELDS: [&str; 3] =
    ["isWeaponPlatform", "attachmentLinks", "defaultGraphicParts"];

/// The parameters of one gun conversion operation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MakeGunSpec {
    /// The `defName` of the target (trimmed; empty when missing).
    pub def_name: String,
    /// Children of `statBases`.
    pub stat_bases: Vec<Node>,
    /// Children of `costList`; `None` when the element is absent.
    pub cost_list: Option<Vec<Node>>,
    /// Children of `Properties` (the converted verb's fields).
    pub properties: Vec<Node>,
    /// Children of `AmmoUser`; `None` when absent (no ammo component is written).
    pub ammo_user: Option<Vec<Node>>,
    /// Children of `FireModes`; `None` when absent. An empty element still writes a component.
    pub fire_modes: Option<Vec<Node>>,
    /// Children of `weaponTags`.
    pub weapon_tags: Vec<Node>,
    /// Children of `weaponClasses`.
    pub weapon_classes: Vec<Node>,
    /// The `researchPrerequisite` element.
    pub research_prerequisite: Option<Node>,
    /// The `texPath` text.
    pub tex_path: Option<String>,
    /// The `AllowWithRunAndGun` value, when written.
    pub allow_with_run_and_gun: Option<bool>,
    /// Names of parameters that are present but not supported by the merge.
    pub unsupported: Vec<String>,
}

fn element_children(node: &Node) -> Vec<Node> {
    node.elements().cloned().collect()
}

impl MakeGunSpec {
    /// Parses the `Operation` element. Unknown parameters are ignored; missing ones stay at their empty value.
    #[must_use]
    pub fn parse(op: &Node) -> Self {
        let mut spec = Self::default();
        for child in op.elements() {
            match child.tag.as_str() {
                "defName" => spec.def_name = text_of(child).unwrap_or_default(),
                "statBases" => spec.stat_bases = element_children(child),
                "costList" => spec.cost_list = Some(element_children(child)),
                "Properties" => spec.properties = element_children(child),
                "AmmoUser" => spec.ammo_user = Some(element_children(child)),
                "FireModes" => spec.fire_modes = Some(element_children(child)),
                "weaponTags" => spec.weapon_tags = element_children(child),
                "weaponClasses" => spec.weapon_classes = element_children(child),
                "researchPrerequisite" => spec.research_prerequisite = Some(child.clone()),
                "texPath" => spec.tex_path = text_of(child),
                "AllowWithRunAndGun" => {
                    spec.allow_with_run_and_gun =
                        text_of(child).map(|t| t.eq_ignore_ascii_case("true"));
                }
                other if UNSUPPORTED_FIELDS.contains(&other) => {
                    spec.unsupported.push(other.to_owned());
                }
                _ => {}
            }
        }
        spec
    }
}

/// What a merge did, for tests and for the dry run report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    /// Containers that did not exist and were created.
    pub created: Vec<String>,
    /// Vanilla accuracy stats that were deleted.
    pub accuracy_removed: usize,
    /// Vanilla shoot verbs that were removed.
    pub verbs_removed: usize,
    /// Whether a converted verb entry was appended.
    pub verb_added: bool,
    /// Components appended (ammo user, fire modes), by class.
    pub comps_added: Vec<String>,
}

fn container<'a>(def: &'a mut Node, tag: &str, report: &mut MergeReport) -> Option<&'a mut Node> {
    if def.child(tag).is_none() {
        def.push_child(Node::new(tag));
        report.created.push(tag.to_owned());
    }
    def.elements_mut().find(|e| e.tag == tag)
}

fn remove_children_named(parent: &mut Node, tag: &str) -> usize {
    let before = parent.children.len();
    parent
        .children
        .retain(|c| !matches!(c, Child::Element(e) if e.tag == tag));
    before.saturating_sub(parent.children.len())
}

/// Applies the conversion to a resolved `ThingDef` node in place.
pub fn merge_into_def(def: &mut Node, spec: &MakeGunSpec, classes: &CeClassNames) -> MergeReport {
    let mut report = MergeReport::default();
    merge_stats(def, spec, &mut report);
    if let Some(items) = spec.cost_list.as_ref().filter(|c| !c.is_empty())
        && let Some(list) = container(def, "costList", &mut report)
    {
        list.children.clear();
        for item in items {
            list.push_child(item.clone());
        }
    }
    merge_verb(def, spec, classes, &mut report);
    merge_comps(def, spec, classes, &mut report);
    for (children, tag) in [
        (&spec.weapon_classes, "weaponClasses"),
        (&spec.weapon_tags, "weaponTags"),
    ] {
        if !children.is_empty()
            && let Some(list) = container(def, tag, &mut report)
        {
            for c in children {
                list.push_child(c.clone());
            }
        }
    }
    if let Some(prerequisite) = &spec.research_prerequisite
        && let Some(maker) = container(def, "recipeMaker", &mut report)
    {
        remove_children_named(maker, &prerequisite.tag);
        maker.push_child(prerequisite.clone());
    }
    if let Some(path) = &spec.tex_path
        && let Some(graphic) = container(def, "graphicData", &mut report)
    {
        graphic.set_child_text("texPath", path.clone());
        graphic.set_child_text("graphicClass", "Graphic_Single");
    }
    report
}

fn merge_stats(def: &mut Node, spec: &MakeGunSpec, report: &mut MergeReport) {
    if spec.stat_bases.is_empty() {
        return;
    }
    if let Some(stats) = container(def, "statBases", report) {
        for name in VANILLA_ACCURACY_STATS {
            report.accuracy_removed += remove_children_named(stats, name);
        }
        for stat in &spec.stat_bases {
            remove_children_named(stats, &stat.tag);
            stats.push_child(stat.clone());
        }
    }
}

fn merge_verb(
    def: &mut Node,
    spec: &MakeGunSpec,
    classes: &CeClassNames,
    report: &mut MergeReport,
) {
    if spec.properties.is_empty() {
        return;
    }
    if let Some(verbs) = container(def, "verbs", report) {
        let before = verbs.children.len();
        verbs.children.retain(|c| match c {
            Child::Element(li) => !li
                .child("verbClass")
                .and_then(text_of)
                .is_some_and(|v| VANILLA_SHOOT_VERBS.contains(&v.as_str())),
            Child::Text(_) => true,
        });
        report.verbs_removed = before.saturating_sub(verbs.children.len());
        let mut li = Node::new("li");
        li.set_attr("Class", classes.verb_properties.clone());
        for p in &spec.properties {
            li.push_child(p.clone());
        }
        verbs.push_child(li);
        report.verb_added = true;
    }
}

fn merge_comps(
    def: &mut Node,
    spec: &MakeGunSpec,
    classes: &CeClassNames,
    report: &mut MergeReport,
) {
    if spec.ammo_user.is_none() && spec.fire_modes.is_none() {
        return;
    }
    let mut added = Vec::new();
    if let Some(comps) = container(def, "comps", report) {
        for (children, class) in [
            (&spec.ammo_user, &classes.ammo_user),
            (&spec.fire_modes, &classes.fire_modes),
        ] {
            if let Some(children) = children {
                let mut li = Node::new("li");
                li.set_attr("Class", class.clone());
                for c in children {
                    li.push_child(c.clone());
                }
                comps.push_child(li);
                added.push(class.clone());
            }
        }
    }
    report.comps_added = added;
}

/// The gun conversion operation as a custom patch operation of the def engine.
#[derive(Debug, Clone)]
pub struct MakeGunOp {
    classes: CeClassNames,
}

impl MakeGunOp {
    /// An operation handler for the class names given.
    #[must_use]
    pub fn new(classes: CeClassNames) -> Self {
        Self { classes }
    }
}

impl Default for MakeGunOp {
    fn default() -> Self {
        Self::new(CeClassNames::default())
    }
}

impl CustomPatchOp for MakeGunOp {
    fn class(&self) -> &str {
        &self.classes.make_gun_op
    }

    fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        let spec = MakeGunSpec::parse(&ctx.raw);
        if spec.def_name.is_empty() {
            ctx.emit(
                DiagCode::new(MAKEGUN_MISSING_DEF),
                Severity::Warning,
                "the gun conversion names no def",
            );
            return Ok(false);
        }
        let targets = doc.find_defs("ThingDef", &spec.def_name);
        if targets.is_empty() {
            ctx.emit(
                DiagCode::new(MAKEGUN_MISSING_DEF),
                Severity::Warning,
                format!(
                    "the gun conversion targets {}, which does not exist",
                    spec.def_name
                ),
            );
            return Ok(false);
        }
        let origin = ctx.origin(doc);
        for id in targets {
            let Some(mut def) = doc.arena().to_node(id) else {
                continue;
            };
            merge_into_def(&mut def, &spec, &self.classes);
            let arena = doc.arena_mut();
            arena.clear_children(id)?;
            for child in &def.children {
                let node = arena.import_child(child, origin)?;
                arena.append_child(id, node)?;
            }
            ctx.touch(id);
        }
        Ok(true)
    }
}

/// The class of Combat Extended's own mod gate. Hand written conversions put it first in a sequence, so a
/// load that does not know it never applies them (and a scan would then offer to convert the weapon a second
/// time, which the non idempotent gun merge would apply twice).
pub const FIND_MOD_OP: &str = "CombatExtended.PatchOperationFindMod";

/// Combat Extended's mod gate as a custom patch operation: succeeds when an active mod has exactly the
/// display name in `modName` (no trimming, case sensitive), and fails when `modName` is missing or empty.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindModOp;

impl CustomPatchOp for FindModOp {
    fn class(&self) -> &str {
        FIND_MOD_OP
    }

    fn apply(&self, _doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        let name = ctx
            .raw
            .child("modName")
            .map(Node::text_content)
            .unwrap_or_default();
        if name.is_empty() {
            return Ok(false);
        }
        Ok(ctx.mod_names.contains(name.as_str()))
    }
}

/// A custom patch registry for loading a set that includes Combat Extended: the gun conversion operation, the
/// mod gate and the settings conditional. `settings` holds the boolean mod settings of the user's Combat Extended (read at
/// run time by the caller); a conditional on a setting that is not in the map fails with
/// `defs.patch-setting-missing`.
#[must_use]
pub fn custom_registry(classes: &CeClassNames, settings: BTreeMap<String, bool>) -> CustomRegistry {
    let mut registry = CustomRegistry::new();
    registry.register(MakeGunOp::new(classes.clone()));
    registry.register(FindModOp);
    registry.register(SettingsConditional::new(
        classes.settings_conditional_op.clone(),
        settings,
    ));
    registry
}

/// One gun conversion operation found in a patch report.
#[derive(Debug, Clone, PartialEq)]
pub struct MakeGunRecord {
    /// The mod whose patch list holds the operation.
    pub mod_idx: ModIdx,
    /// The patch file, when known.
    pub file: Option<FileId>,
    /// The position of the operation in the mod's patch list.
    pub index: u32,
    /// The result of the operation in this load.
    pub applied: bool,
    /// The parsed parameters.
    pub spec: MakeGunSpec,
}

/// The gun conversion operations of a patch report, in application order. The engine keeps the raw
/// `Operation` element only for classes it does not handle, so this lists the conversions of a load that did
/// not register [`MakeGunOp`] (the lint and the update mode read them this way).
#[must_use]
pub fn makegun_records(report: &PatchReport, classes: &CeClassNames) -> Vec<MakeGunRecord> {
    report
        .events
        .iter()
        .filter(|e| e.class.eq_ignore_ascii_case(&classes.make_gun_op))
        .filter_map(|e| {
            let raw = e.raw.as_ref()?;
            Some(MakeGunRecord {
                mod_idx: e.mod_idx,
                file: e.file,
                index: e.index,
                applied: e.result,
                spec: MakeGunSpec::parse(raw),
            })
        })
        .collect()
}

/// The class attribute of a node, for callers that check a verb or tool entry.
#[must_use]
pub fn class_of(node: &Node) -> Option<&str> {
    class_attr(node)
}
