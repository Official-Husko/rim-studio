//! The structure of the generated Combat Extended patch: what the conversions of the user's own Combat
//! Extended do and the generator must do too (conventions of converted tools, fire modes, carried verb
//! fields, tags), and that a patch applied twice changes nothing the second time. Everything is fictional;
//! the real install is compared by `real_ce_fidelity`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::patchgen::{
    Container, GeneratedPatch, PatchMode, dry_apply, gun_patch, melee_patch,
};
use rimstudio_design::ce::reader::{CeClassNames, CeModel, CeToolRow};
use rimstudio_design::model::{
    CePatchSpec, DesignSpec, ExtraMeleeDamage, Sourced, ToolSpec, ValueSource,
};

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use common_ce::{CLASS_TAG, ce_model};

const GROUP_STOCK: &str = "RS_Stock";
const GROUP_MUZZLE: &str = "RS_Muzzle";

fn ranged() -> DesignSpec {
    let mut ce = ranged_ce();
    ce.weapon_tag_class = Some(CLASS_TAG.into());
    let mut spec = ranged_spec();
    spec.ce = Some(ce);
    spec
}

fn melee() -> DesignSpec {
    let mut spec = melee_spec();
    spec.ce = Some(melee_ce());
    spec
}

fn row(label: &str, capacities: &[&str], group: Option<&str>, chance: Option<f64>) -> CeToolRow {
    CeToolRow {
        label: label.into(),
        capacities: capacities.iter().map(|c| (*c).to_owned()).collect(),
        power: Some(8.0),
        cooldown: Some(1.5),
        ap_sharp: None,
        ap_blunt: Some(2.0),
        chance_factor: chance,
        linked_body_parts_group: group.map(str::to_owned),
    }
}

/// A model whose converted guns all carry the gun bash `stock`, `barrel`, `muzzle` the way a conversion of
/// the fictional install does.
fn model_with_gun_tools() -> CeModel {
    let mut model = ce_model();
    for g in &mut model.guns {
        g.tools = vec![
            row("stock", &["Blunt"], Some(GROUP_STOCK), Some(1.5)),
            row("barrel", &["Blunt"], Some("RS_Barrel"), None),
            row("muzzle", &["Poke"], Some(GROUP_MUZZLE), None),
        ];
    }
    model
}

fn gun_tools(patch: &GeneratedPatch) -> Vec<Node> {
    let tools = patch
        .operations
        .iter()
        .find_map(|op| {
            op.child("value")
                .and_then(|v| v.child("tools"))
                .or_else(|| {
                    op.child("nomatch")
                        .and_then(|n| n.child("value"))
                        .and_then(|v| v.child("tools"))
                })
        })
        .expect("a tools operation");
    tools.children_named("li").cloned().collect()
}

fn capacities(tool: &Node) -> Vec<String> {
    tool.child("capacities")
        .unwrap()
        .children_named("li")
        .map(|c| c.text_content())
        .collect()
}

fn defs(node: Node) -> Vec<Node> {
    vec![node]
}

fn raw_gun(name: &str, parent: Option<&str>, with_verbs: bool) -> Node {
    let mut b = NodeBuilder::new("ThingDef");
    if let Some(p) = parent {
        b = b.attr("ParentName", p);
    }
    b = b.text_elem("defName", name);
    b = b.elem("statBases", |s| s.text_elem("AccuracyTouch", "0.6"));
    if with_verbs {
        b = b.elem("verbs", |v| {
            v.child(
                NodeBuilder::new("li")
                    .text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", "RS_Bullet")
                    .build(),
            )
        });
    }
    b.elem("tools", |t| {
        t.child(
            NodeBuilder::new("li")
                .text_elem("label", "grip")
                .elem("capacities", |c| c.li("Blunt"))
                .build(),
        )
    })
    .build()
}

fn ce_classes() -> CeClassNames {
    CeClassNames::default()
}

fn count_class(def: &Node, list: &str, class: &str) -> usize {
    def.child(list)
        .map(|l| {
            l.children_named("li")
                .filter(|li| li.attr("Class") == Some(class))
                .count()
        })
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------------------
// tool conventions

#[test]
fn tool_habits_fill_the_group_and_the_pick_weight_of_a_known_label() {
    let mut spec = ranged();
    spec.tools =
        vec![ToolSpec::new("stock", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed)];
    let model = model_with_gun_tools();
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let tools = gun_tools(&patch);
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].child_text("linkedBodyPartsGroup"),
        Some(GROUP_STOCK)
    );
    assert_eq!(tools[0].child_text("chanceFactor"), Some("1.5"));
    // the user is told where the two values came from
    let fields: Vec<&str> = patch
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "ce.derived-value")
        .filter_map(|d| d.args.get("field").map(String::as_str))
        .collect();
    assert!(
        fields.contains(&"tool 1 linkedBodyPartsGroup"),
        "{fields:?}"
    );
    assert!(fields.contains(&"tool 1 chanceFactor"), "{fields:?}");
}

#[test]
fn a_value_of_the_design_is_never_overruled_by_a_habit() {
    let mut spec = ranged();
    let mut stock = ToolSpec::new("stock", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed);
    stock.linked_body_parts_group = Some("RS_Mine".into());
    stock.chance_factor = Some(Sourced::typed(0.5));
    spec.tools = vec![stock];
    let patch = gun_patch(&spec, &model_with_gun_tools(), &Container::from_spec(&spec)).unwrap();
    let tools = gun_tools(&patch);
    assert_eq!(tools[0].child_text("linkedBodyPartsGroup"), Some("RS_Mine"));
    assert_eq!(tools[0].child_text("chanceFactor"), Some("0.5"));
}

#[test]
fn a_label_without_enough_examples_gets_no_habit() {
    let mut spec = ranged();
    spec.tools =
        vec![ToolSpec::new("pommel", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed)];
    let patch = gun_patch(&spec, &model_with_gun_tools(), &Container::from_spec(&spec)).unwrap();
    let tools = gun_tools(&patch);
    assert!(tools[0].child("linkedBodyPartsGroup").is_none());
    assert!(tools[0].child("chanceFactor").is_none());
    // and a model without any gun bash examples changes nothing at all
    let bare = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert_eq!(gun_tools(&bare), tools);
}

#[test]
fn a_capacity_that_conversions_drop_moves_to_a_tool_of_its_own() {
    let mut spec = ranged();
    spec.tools = vec![
        ToolSpec::new("stock", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed),
        ToolSpec::new("barrel", &["Blunt", "Poke"]).with_numbers(7.0, 2.5, ValueSource::Typed),
    ];
    let patch = gun_patch(&spec, &model_with_gun_tools(), &Container::from_spec(&spec)).unwrap();
    let tools = gun_tools(&patch);
    assert_eq!(tools.len(), 3);
    assert_eq!(capacities(&tools[1]), ["Blunt"]);
    assert_eq!(tools[2].child_text("label"), Some("muzzle"));
    assert_eq!(capacities(&tools[2]), ["Poke"]);
    assert_eq!(
        tools[2].child_text("linkedBodyPartsGroup"),
        Some(GROUP_MUZZLE)
    );
    // the new tool copies the numbers of the barrel it came from
    assert_eq!(tools[2].child_text("power"), tools[1].child_text("power"));
    assert_eq!(
        tools[2].child_text("armorPenetrationBlunt"),
        tools[1].child_text("armorPenetrationBlunt")
    );
}

#[test]
fn a_replaced_capacity_is_not_a_split() {
    // Conversions of the fictional melee weapons strike with a poke from the handle.
    let mut model = ce_model();
    for m in &mut model.melee {
        m.tools = vec![row("handle", &["Poke"], Some("RS_Handle"), None); 1];
    }
    let mut spec = melee();
    spec.tools = vec![
        ToolSpec::new("handle", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed),
        ToolSpec::new("blade", &["Cut"]).with_numbers(18.0, 2.4, ValueSource::Typed),
    ];
    spec.ce
        .as_mut()
        .unwrap()
        .tool_penetration
        .retain(|p| p.tool != "point");
    let patch = melee_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let tools = gun_tools(&patch);
    assert_eq!(tools.len(), 2);
    assert_eq!(capacities(&tools[0]), ["Poke"]);
    assert_eq!(capacities(&tools[1]), ["Cut"]);
}

#[test]
fn extra_damages_and_other_tool_fields_survive_the_conversion() {
    let mut spec = melee();
    let blade = spec.tools.iter_mut().find(|t| t.label == "blade").unwrap();
    blade.extra_melee_damages = vec![ExtraMeleeDamage {
        def: "RS_Burn".into(),
        amount: Some(5.0),
        chance: Some(0.3),
    }];
    blade.extra = vec![Node::with_text("labelUsedInLogging", "false")];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    let tools = gun_tools(&patch);
    let blade = tools
        .iter()
        .find(|t| t.child_text("label") == Some("blade"))
        .unwrap();
    let damage = blade
        .child("extraMeleeDamages")
        .unwrap()
        .child("li")
        .unwrap();
    assert_eq!(damage.child_text("def"), Some("RS_Burn"));
    assert_eq!(damage.child_text("amount"), Some("5"));
    assert_eq!(damage.child_text("chance"), Some("0.3"));
    assert_eq!(blade.child_text("labelUsedInLogging"), Some("false"));
}

// ---------------------------------------------------------------------------------------------------------
// fire modes and the verb

fn conversion(patch: &GeneratedPatch) -> &Node {
    patch.gun_conversion(&ce_classes()).expect("a conversion")
}

#[test]
fn the_fire_modes_follow_the_class_of_the_converted_guns() {
    let mut model = ce_model();
    for g in &mut model.guns {
        g.ai_aim_mode = Some("Snapshot".into());
        g.use_burst_mode = Some(false);
    }
    let spec = ranged();
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    let modes = conversion(&patch).child("FireModes").unwrap();
    assert_eq!(modes.child_text("aiAimMode"), Some("Snapshot"));
    assert_eq!(modes.child_text("aiUseBurstMode"), Some("false"));
    // an explicit belt fed flag wins over the habit
    let mut belt = ranged();
    belt.ce.as_mut().unwrap().belt_fed = true;
    let patch = gun_patch(&belt, &model, &Container::from_spec(&belt)).unwrap();
    assert_eq!(
        conversion(&patch)
            .child("FireModes")
            .unwrap()
            .child_text("aiAimMode"),
        Some("SuppressFire")
    );
}

#[test]
fn without_examples_of_the_class_the_fire_modes_use_the_plain_defaults() {
    let mut model = ce_model();
    for g in &mut model.guns {
        g.ai_class = Some("RS_Other_Class".into());
        g.ai_aim_mode = Some("Snapshot".into());
    }
    let spec = ranged();
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    let modes = conversion(&patch).child("FireModes").unwrap();
    assert_eq!(modes.child_text("aiAimMode"), Some("AimedShot"));
    // a burst weapon uses its bursts
    assert_eq!(modes.child_text("aiUseBurstMode"), Some("true"));
}

#[test]
fn a_burst_weapon_keeps_its_vanilla_burst_as_the_aimed_burst() {
    let spec = ranged();
    // the fixture burst is 3
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    let modes = conversion(&patch).child("FireModes").unwrap();
    assert_eq!(modes.child_text("aimedBurstShotCount"), Some("3"));
    // a single shot weapon has none
    let mut single = ranged();
    single.ranged.as_mut().unwrap().burst_count = None;
    let patch = gun_patch(&single, &ce_model(), &Container::from_spec(&single)).unwrap();
    assert!(
        conversion(&patch)
            .child("FireModes")
            .unwrap()
            .child("aimedBurstShotCount")
            .is_none()
    );
}

#[test]
fn the_vanilla_verb_fields_beyond_the_modelled_ones_are_carried_into_the_properties() {
    let mut spec = ranged();
    {
        let r = spec.ranged.as_mut().unwrap();
        r.forced_miss_radius = Some(1.9);
        r.verb_extra = vec![
            Node::with_text("minRange", "4"),
            NodeBuilder::new("targetParams")
                .text_elem("canTargetLocations", "true")
                .build(),
            // a field the properties write themselves is not repeated
            Node::with_text("range", "99"),
        ];
    }
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    let props = conversion(&patch).child("Properties").unwrap();
    assert_eq!(props.child_text("forcedMissRadius"), Some("1.9"));
    assert_eq!(props.child_text("minRange"), Some("4"));
    assert_eq!(
        props
            .child("targetParams")
            .and_then(|t| t.child_text("canTargetLocations")),
        Some("true")
    );
    assert_eq!(props.children_named("range").count(), 1);
    assert_ne!(props.child_text("range"), Some("99"));
}

#[test]
fn companion_tags_of_the_class_are_offered_as_a_hint_and_never_written() {
    let mut model = ce_model();
    for g in &mut model.guns {
        g.weapon_tags.push("RS_Sidearm".into());
    }
    let spec = ranged();
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    let hint = patch
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "ce.companion-tags")
        .expect("a hint");
    assert!(hint.args["tags"].contains("RS_Sidearm"));
    let tags = conversion(&patch).child("weaponTags").unwrap();
    assert!(
        tags.children_named("li")
            .all(|li| li.text_content() != "RS_Sidearm")
    );
}

// ---------------------------------------------------------------------------------------------------------
// a def that inherits its verbs

#[test]
fn a_def_that_takes_its_verbs_from_a_parent_starts_a_list_of_its_own() {
    let model = ce_model();
    let spec = ranged();
    let node = raw_gun("RS_Child", Some("RS_Parent"), false);
    let patch = gun_patch(
        &spec_for(&spec, "RS_Child"),
        &model,
        &Container::from_node(&node),
    )
    .unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    // the list is started with Inherit="False" before the conversion
    let first = &patch.operations[0];
    assert_eq!(first.attr("Class"), Some("PatchOperationConditional"));
    assert!(first.child_text("xpath").unwrap().ends_with("/verbs"));
    let add = first.child("nomatch").unwrap();
    assert_eq!(
        add.child("value")
            .and_then(|v| v.child("verbs"))
            .and_then(|v| v.attr("Inherit")),
        Some("False")
    );
    let run = dry_apply(&defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_Child").unwrap();
    assert_eq!(def.child("verbs").unwrap().attr("Inherit"), Some("False"));
    assert_eq!(count_class(def, "verbs", &ce_classes().verb_properties), 1);
    // a def with a verbs list of its own needs nothing of the sort
    let own = raw_gun("RS_Own", Some("RS_Parent"), true);
    let patch = gun_patch(
        &spec_for(&spec, "RS_Own"),
        &model,
        &Container::from_node(&own),
    )
    .unwrap();
    assert!(
        patch.operations[0]
            .child_text("xpath")
            .unwrap()
            .contains("comps/li")
    );
}

fn spec_for(spec: &DesignSpec, name: &str) -> DesignSpec {
    let mut s = spec.clone();
    s.identity.def_name = name.into();
    s
}

// ---------------------------------------------------------------------------------------------------------
// applying twice

#[test]
fn a_gun_patch_applied_twice_converts_once() {
    let model = ce_model();
    let spec = spec_for(&ranged(), "RS_Twice");
    let node = raw_gun("RS_Twice", Some("RS_Parent"), false);
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let once = dry_apply(&defs(node.clone()), &[patch.patch_root()], &model);
    let twice = dry_apply(
        &defs(node),
        &[patch.patch_root(), patch.patch_root()],
        &model,
    );
    assert!(twice.is_clean(), "{:?}", twice.diagnostics);
    assert_eq!(
        once.def("ThingDef", "RS_Twice"),
        twice.def("ThingDef", "RS_Twice")
    );
    let def = twice.def("ThingDef", "RS_Twice").unwrap();
    let classes = ce_classes();
    assert_eq!(count_class(def, "verbs", &classes.verb_properties), 1);
    assert_eq!(count_class(def, "comps", &classes.ammo_user), 1);
    assert_eq!(count_class(def, "comps", &classes.fire_modes), 1);
    assert_eq!(def.child("tools").unwrap().children_named("li").count(), 1);
    assert_eq!(def.children_named("tools").count(), 1);
}

#[test]
fn a_melee_patch_applied_twice_changes_nothing_the_second_time() {
    let model = ce_model();
    let mut spec = melee();
    spec.ce.as_mut().unwrap().one_handed = true;
    let mut node = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestBlade")
        .build();
    node.push_child(Node::new("statBases"));
    let patch = melee_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let once = dry_apply(&defs(node.clone()), &[patch.patch_root()], &model);
    let twice = dry_apply(
        &defs(node),
        &[patch.patch_root(), patch.patch_root()],
        &model,
    );
    assert!(
        once.is_clean() && twice.is_clean(),
        "{:?}",
        twice.diagnostics
    );
    let a = once.def("ThingDef", "RS_TestBlade").unwrap();
    let b = twice.def("ThingDef", "RS_TestBlade").unwrap();
    assert_eq!(a, b);
    assert_eq!(
        b.child("statBases").unwrap().children_named("Bulk").count(),
        1
    );
    assert_eq!(b.children_named("equippedStatOffsets").count(), 1);
}

#[test]
fn a_second_application_over_the_changed_value_of_another_patch_writes_the_design_value() {
    let model = ce_model();
    let spec = melee();
    // the def has no Bulk when the patch is generated, but another patch adds a different one first
    let mut node = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestBlade")
        .build();
    node.push_child(Node::new("statBases"));
    let patch = melee_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let mut other = NodeBuilder::new("Patch").build();
    other.push_child(
        NodeBuilder::new("Operation")
            .attr("Class", "PatchOperationAdd")
            .text_elem("xpath", "Defs/ThingDef[defName=\"RS_TestBlade\"]/statBases")
            .elem("value", |v| v.text_elem("Bulk", "99"))
            .build(),
    );
    let run = dry_apply(&defs(node), &[other, patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let stats = run
        .def("ThingDef", "RS_TestBlade")
        .unwrap()
        .child("statBases")
        .unwrap();
    assert_eq!(stats.children_named("Bulk").count(), 1);
    assert_eq!(stats.child_text("Bulk"), Some("6"));
}

// ---------------------------------------------------------------------------------------------------------
// melee tags and update mode

fn tag_texts(def: &Node) -> Vec<String> {
    def.child("weaponTags")
        .map(|t| t.children_named("li").map(Node::text_content).collect())
        .unwrap_or_default()
}

#[test]
fn a_melee_conversion_adds_the_class_tag_and_the_one_handed_mark() {
    let mut model = ce_model();
    model.weapon_tags = vec!["RS_CE_Sidearm".into(), "RS_CE_OneHandedWeapon".into()];
    let mut spec = melee();
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.weapon_tag_class = Some("RS_CE_Sidearm".into());
        ce.one_handed = true;
    }
    let node = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestBlade")
        .build();
    let patch = melee_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let run = dry_apply(&defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_TestBlade").unwrap();
    assert_eq!(tag_texts(def), ["RS_CE_Sidearm", "RS_CE_OneHandedWeapon"]);
    // a class tag the install does not know is a warning, and still written
    spec.ce.as_mut().unwrap().weapon_tag_class = Some("RS_Unknown".into());
    let patch = melee_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    assert!(
        patch
            .diagnostics
            .iter()
            .any(|d| d.code.as_str().starts_with("ce.cep016"))
    );
}

#[test]
fn the_class_tag_of_a_melee_conversion_is_read_back_for_update_mode() {
    use rimstudio_design::ce::reader::ce_block_from_def;
    let classes = ce_classes();
    let node = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Conv")
        .elem("tools", |t| {
            t.child(
                NodeBuilder::new("li")
                    .attr("Class", &classes.tool)
                    .text_elem("label", "blade")
                    .build(),
            )
        })
        .elem("weaponTags", |t| {
            t.li("RS_Vanilla")
                .li("CE_Sidearm_Melee")
                .li("CE_OneHandedWeapon")
        })
        .build();
    let record = rimstudio_defs::DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: "RS_Conv".into(),
        origin: None,
        node,
        parents: Vec::new(),
        patched_by: Vec::new(),
    };
    let block: CePatchSpec = ce_block_from_def(&record, &classes, ValueSource::Anchor).unwrap();
    assert_eq!(block.weapon_tag_class.as_deref(), Some("CE_Sidearm_Melee"));
    assert!(block.one_handed);
}

// ---------------------------------------------------------------------------------------------------------
// update mode

fn existing_of(block: CePatchSpec) -> rimstudio_design::ce::patchgen::ExistingConversion {
    use rimstudio_design::ce::patchgen::{ConversionSource, ExistingConversion};
    use rimstudio_design::ce::reader::CeMarkers;
    ExistingConversion {
        markers: CeMarkers {
            verb: true,
            tool: true,
            ammo_comp: true,
            fire_modes: true,
        },
        block,
        source: ConversionSource::RimStudio(None),
        verb_fields: std::collections::BTreeSet::new(),
        ammo_fields: std::collections::BTreeSet::new(),
        weapon_tags: Vec::new(),
    }
}

#[test]
fn an_update_adds_a_missing_field_once_even_when_applied_twice() {
    let model = ce_model();
    let spec = ranged();
    let current = existing_of(ranged_ce());
    let mut changed = spec.clone();
    changed.ce.as_mut().unwrap().recoil_amount = Some(Sourced::typed(1.7));
    // the converted verb has no recoilAmount yet, so the update adds it
    let classes = ce_classes();
    let node = NodeBuilder::new("ThingDef")
        .text_elem("defName", &spec.identity.def_name)
        .elem("verbs", |v| {
            v.child(
                NodeBuilder::new("li")
                    .attr("Class", &classes.verb_properties)
                    .text_elem("range", "30")
                    .build(),
            )
        })
        .build();
    let container = Container::from_node(&node).with_existing(current);
    let patch = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    assert!(!patch.operations.is_empty());
    let once = dry_apply(&defs(node.clone()), &[patch.patch_root()], &model);
    let twice = dry_apply(
        &defs(node),
        &[patch.patch_root(), patch.patch_root()],
        &model,
    );
    assert!(
        once.is_clean() && twice.is_clean(),
        "{:?}",
        twice.diagnostics
    );
    let verb = |run: &rimstudio_design::ce::patchgen::simulate::DryRun| {
        run.def("ThingDef", &spec.identity.def_name)
            .unwrap()
            .child("verbs")
            .unwrap()
            .child("li")
            .unwrap()
            .clone()
    };
    assert_eq!(verb(&once).children_named("recoilAmount").count(), 1);
    assert_eq!(verb(&twice), verb(&once));
}

#[test]
fn an_update_adds_the_one_handed_mark_the_conversion_lacks() {
    let mut model = ce_model();
    model.weapon_tags = vec!["RS_CE_OneHandedWeapon".into()];
    let mut spec = melee();
    spec.ce.as_mut().unwrap().one_handed = true;
    let mut current = melee_ce();
    current.one_handed = false;
    let container = Container::from_spec(&spec).with_existing(existing_of(current));
    let patch = melee_patch(&spec, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    let text = serde_json::to_string(&patch.patch_root()).unwrap();
    assert!(text.contains("RS_CE_OneHandedWeapon"), "{text}");
}
