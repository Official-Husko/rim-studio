//! Tests of the own-node reader (what a clone carries) over fictional defs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::fixtures_tests::{base_defs, load_defs, types};
use super::*;
use crate::model::{ProjectileChoice, TechLevel, ValueSource};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::LoadOutput;

/// The abstract family the children inherit from: tech level, tags, comps, a recipe, stats and a tool.
fn family() -> Vec<Node> {
    let base = NodeBuilder::new("ThingDef")
        .attr("Name", "RS_BaseFamily")
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .text_elem("techLevel", "Industrial")
        .text_elem("description", "the base description")
        .elem("statBases", |s| {
            s.text_elem("Mass", "2")
                .text_elem("RS_Durability", "80")
                .text_elem("AccuracyTouch", "0.5")
        })
        .elem("weaponTags", |w| w.li("RS_BaseTag"))
        .elem("comps", |c| {
            c.elem("li", |li| {
                li.attr("Class", "RS_CompBase").text_elem("a", "1")
            })
        })
        .elem("recipeMaker", |r| {
            r.text_elem("workSkill", "RS_Crafting")
                .elem("recipeUsers", |u| u.li("RS_Bench"))
        })
        .build();
    let bullet = NodeBuilder::new("ThingDef")
        .attr("Name", "RS_BaseBullet")
        .attr("Abstract", "True")
        .text_elem("category", "Projectile")
        .elem("projectile", |p| p.text_elem("damageDef", "RS_Damage"))
        .build();
    vec![base, bullet]
}

fn shot() -> Node {
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseBullet")
        .text_elem("defName", "RS_OwnShot")
        .text_elem("label", "own shot")
        .elem("graphicData", |g| {
            g.text_elem("texPath", "RS/Shot")
                .text_elem("drawSize", "(0.5,0.5)")
        })
        .elem("projectile", |p| {
            p.text_elem("damageAmountBase", "11")
                .text_elem("speed", "60")
                .text_elem("stoppingPower", "1.5")
                .text_elem("explosionRadius", "1.9")
        })
        .text_elem("thingClass", "RS_Bullet")
        .build()
}

/// A child gun that sets its own tags, comps, recipe fields and extras and inherits the rest.
fn child_gun() -> Node {
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseFamily")
        .text_elem("defName", "RS_ChildGun")
        .text_elem("label", "child gun")
        .elem("graphicData", |g| {
            g.text_elem("texPath", "RS/Gun")
                .text_elem("graphicClass", "Graphic_Single")
                .text_elem("drawSize", "(1.5,1.5)")
                .text_elem("color", "(0.8,0.8,0.8)")
                .text_elem("shaderType", "CutoutComplex")
        })
        .elem("statBases", |s| {
            s.text_elem("WorkToMake", "9000")
                .text_elem("RangedWeapon_Cooldown", "1.5")
                .text_elem("AccuracyShort", "0.7")
                .text_elem("AccuracyMedium", "0.6")
                .text_elem("AccuracyLong", "0.5")
                .text_elem("Mass", "2")
        })
        .elem("costList", |c| c.text_elem("RS_Steel", "30"))
        .elem("recipeMaker", |r| {
            r.elem("skillRequirements", |s| s.text_elem("RS_Crafting", "5"))
                .text_elem("displayPriority", "415")
                .text_elem("soundWorking", "RS_Working")
                .text_elem("researchPrerequisite", "RS_Research")
        })
        .text_elem("soundInteract", "RS_Click")
        .text_elem("uiIconPath", "RS/Icon")
        .text_elem("uiIconScale", "1.2")
        .elem("equippedStatOffsets", |o| o.text_elem("MoveSpeed", "-0.25"))
        .text_elem("relicChance", "2")
        .elem("thingSetMakerTags", |t| t.li("RS_Reward"))
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", "RS_OwnShot")
                    .text_elem("warmupTime", "1.2")
                    .text_elem("range", "26")
                    .text_elem("forcedMissRadius", "0.5")
                    .text_elem("aimingChargeMote", "RS_Mote")
            })
        })
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "grip")
                    .elem("capacities", |c| c.li("Blunt"))
                    .text_elem("power", "8")
                    .text_elem("cooldownTime", "2")
                    .text_elem("labelUsedInLogging", "false")
                    .elem("surpriseAttack", |s| {
                        s.elem("extraMeleeDamages", |d| {
                            d.elem("li", |li| {
                                li.text_elem("def", "Stun").text_elem("amount", "14")
                            })
                        })
                    })
            })
        })
        .elem("weaponClasses", |w| w.li("RS_Long"))
        .build()
}

/// A unique child that inherits its verbs, tools and recipe from `RS_ChildGun`.
fn unique_gun() -> Node {
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_ChildName")
        .text_elem("defName", "RS_UniqueGun")
        .text_elem("label", "unique gun")
        .elem("recipeMaker", |r| r.attr("IsNull", "True"))
        .build()
}

fn named_child() -> Node {
    let mut node = child_gun();
    node.set_attr("Name", "RS_ChildName");
    node.set_attr("ParentName", "RS_BaseFamily");
    node.set_attr("Abstract", "True");
    node.set_child_text("defName", "RS_ChildAbstract");
    node
}

fn load() -> LoadOutput {
    let mut defs = base_defs();
    defs.extend(family());
    defs.push(shot());
    defs.push(child_gun());
    defs.push(named_child());
    defs.push(unique_gun());
    load_defs(defs, &types(&[]))
}

fn read(out: &LoadOutput, name: &str, own: &Node, projectile: Option<&Node>) -> SpecReading {
    let def = out.databases.get("ThingDef", name).unwrap();
    spec_from_def_own(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Anchor,
        &OwnSource {
            def: own,
            projectile,
        },
    )
    .unwrap()
}

#[test]
fn an_own_reading_carries_what_the_def_defines_and_leaves_what_the_parent_supplies() {
    let out = load();
    let mut own = child_gun();
    own.set_attr("MayRequire", "rs.dlc");
    let spec = read(&out, "RS_ChildGun", &own, None).spec;
    // the parent supplies the tech level, the description and its tag, comps and recipe fields
    assert_eq!(spec.tech_level, Some(TechLevel::Industrial));
    assert_eq!(
        spec.parent.as_ref().unwrap().inherited_tech_level,
        Some(TechLevel::Industrial)
    );
    assert_eq!(spec.identity.description, "");
    assert!(spec.weapon_tags.is_empty(), "{:?}", spec.weapon_tags);
    assert_eq!(spec.weapon_classes, vec!["RS_Long"]);
    assert!(spec.comps.is_empty());
    let recipe = spec.recipe.as_ref().unwrap();
    assert_eq!(recipe.skill_requirements.get("RS_Crafting"), Some(&5));
    assert_eq!(recipe.display_priority, Some(415.0));
    assert!(recipe.recipe_users.is_empty() && recipe.work_skill.is_none());
    assert_eq!(recipe.extra.len(), 1);
    assert_eq!(recipe.extra[0].tag, "soundWorking");
    assert_eq!(spec.research_prerequisite.as_deref(), Some("RS_Research"));
    // modelled simple fields
    assert_eq!(spec.sound_interact.as_deref(), Some("RS_Click"));
    assert_eq!(spec.ui_icon_path.as_deref(), Some("RS/Icon"));
    assert_eq!(spec.ui_icon_scale, Some(1.2));
    assert_eq!(spec.equipped_stat_offsets.get("MoveSpeed"), Some(&-0.25));
    // graphic data
    assert_eq!(spec.texture_path.as_deref(), Some("RS/Gun"));
    assert_eq!(spec.draw_size.as_deref(), Some("(1.5,1.5)"));
    assert_eq!(spec.graphic_color.as_deref(), Some("(0.8,0.8,0.8)"));
    assert_eq!(spec.graphic_extra.len(), 1);
    assert_eq!(spec.graphic_extra[0].tag, "shaderType");
    // root attributes and raw fields
    assert_eq!(
        spec.extra_attrs.get("MayRequire").map(String::as_str),
        Some("rs.dlc")
    );
    let raw: Vec<&str> = spec.extra_fields.iter().map(|n| n.tag.as_str()).collect();
    assert_eq!(raw, vec!["relicChance", "thingSetMakerTags"]);
    // the verb and the tool
    let ranged = spec.ranged.as_ref().unwrap();
    assert_eq!(ranged.forced_miss_radius, Some(0.5));
    assert_eq!(ranged.verb_extra.len(), 1);
    assert_eq!(ranged.verb_extra[0].tag, "aimingChargeMote");
    let tool = &spec.tools[0];
    assert_eq!(tool.extra.len(), 1);
    assert_eq!(tool.extra[0].tag, "labelUsedInLogging");
    assert_eq!(
        tool.surprise_attack.as_ref().unwrap().extra_melee_damages[0].def,
        "Stun"
    );
    assert!(spec.inherit_reset.is_empty());
}

#[test]
fn the_mapped_stats_the_def_does_not_set_are_recorded_as_the_parents() {
    let out = load();
    let own = child_gun();
    let spec = read(&out, "RS_ChildGun", &own, None).spec;
    let inherited = &spec.parent.as_ref().unwrap().inherited_stats;
    // the child sets Mass itself; AccuracyTouch comes from the parent
    assert!(!inherited.contains_key("Mass"));
    assert_eq!(inherited.get("AccuracyTouch"), Some(&0.5));
    assert_eq!(inherited.get("RS_Durability"), Some(&80.0));
    assert!(!spec.extra_stats.contains_key("RS_Durability"));
}

#[test]
fn a_def_that_inherits_its_verbs_tools_and_recipe_restates_them_and_resets_the_lists() {
    let out = load();
    let own = unique_gun();
    let spec = read(&out, "RS_UniqueGun", &own, None).spec;
    assert!(spec.inherit_reset.contains(&"verbs".to_owned()));
    assert!(spec.inherit_reset.contains(&"tools".to_owned()));
    assert_eq!(spec.tools.len(), 1);
    assert!(spec.ranged.as_ref().unwrap().range.is_some());
    // the nulled recipe is carried as written
    let recipe = spec.recipe.as_ref().unwrap();
    assert_eq!(recipe.attrs.get("IsNull").map(String::as_str), Some("True"));
    assert!(recipe.is_empty() || !recipe.attrs.is_empty());
}

#[test]
fn an_inherit_false_list_is_remembered() {
    let out = load();
    let mut own = child_gun();
    if let Some(tags) = own.child_mut("weaponClasses") {
        tags.set_attr("Inherit", "false");
    }
    let spec = read(&out, "RS_ChildGun", &own, None).spec;
    assert_eq!(spec.inherit_reset, vec!["weaponClasses"]);
}

#[test]
fn the_notes_of_an_own_reading_name_the_carried_fields() {
    let out = load();
    let reading = read(&out, "RS_ChildGun", &child_gun(), None);
    let all = reading.notes.join("\n");
    assert!(all.contains("carried as written"), "{all}");
    assert!(all.contains("relicChance"), "{all}");
    assert!(all.contains("verbs.aimingChargeMote"), "{all}");
    assert!(!all.contains("does not carry"), "{all}");
}

#[test]
fn an_unreadable_damage_list_stays_a_raw_field_of_the_tool() {
    let out = load();
    let mut own = child_gun();
    let tool = own
        .child_mut("tools")
        .and_then(|t| t.elements_mut().next())
        .unwrap();
    tool.push_child(
        NodeBuilder::new("extraMeleeDamages")
            .elem("li", |li| {
                li.text_elem("def", "Stun")
                    .text_elem("armorPenetration", "1")
            })
            .build(),
    );
    let spec = read(&out, "RS_ChildGun", &own, None).spec;
    let tool = &spec.tools[0];
    assert!(tool.extra_melee_damages.is_empty());
    assert!(tool.extra.iter().any(|n| n.tag == "extraMeleeDamages"));
}

#[test]
fn a_projectile_copy_keeps_the_own_fields_of_the_source_projectile() {
    let copy = projectile_spec_from_def(
        &shot(),
        "RS_Bullet_Child",
        "RS_OwnShot",
        ValueSource::Anchor,
    )
    .unwrap();
    assert_eq!(copy.def_name, "RS_Bullet_Child");
    assert_eq!(copy.copied_from.as_deref(), Some("RS_OwnShot"));
    assert_eq!(copy.parent.as_deref(), Some("RS_BaseBullet"));
    assert_eq!(copy.label, "own shot");
    assert_eq!(copy.texture_path.as_deref(), Some("RS/Shot"));
    assert_eq!(copy.speed.map(|s| s.value), Some(60.0));
    assert_eq!(copy.stopping_power.map(|s| s.value), Some(1.5));
    assert!(copy.damage_def.is_none());
    assert_eq!(copy.extra.len(), 1);
    assert_eq!(copy.extra[0].tag, "explosionRadius");
    assert_eq!(copy.graphic_extra.len(), 1);
    assert_eq!(copy.thing_extra.len(), 1);
    // the parent supplies the damage def and the graphic class; the armor penetration is not set
    assert!(copy.omit_defaults.contains(&"damageDef".to_owned()));
    assert!(copy.omit_defaults.contains(&"graphicClass".to_owned()));
    assert!(
        copy.omit_defaults
            .contains(&"armorPenetrationBase".to_owned())
    );
    assert!(!copy.omit_defaults.contains(&"damageAmountBase".to_owned()));
}

#[test]
fn a_projectile_without_a_parent_cannot_be_copied() {
    let mut orphan = shot();
    orphan.remove_attr("ParentName");
    assert!(projectile_spec_from_def(&orphan, "RS_X", "RS_OwnShot", ValueSource::Anchor).is_none());
}

#[test]
fn the_own_reading_leaves_a_reference_projectile() {
    let out = load();
    let spec = read(&out, "RS_ChildGun", &child_gun(), Some(&shot())).spec;
    assert_eq!(
        spec.ranged.unwrap().projectile,
        Some(ProjectileChoice::Reference("RS_OwnShot".into()))
    );
}
