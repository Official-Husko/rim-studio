//! Generation fidelity over a fictional install: a clone carries every field its source defines, has a
//! projectile of its own by default, and writes what the user changes. Every name starts with `RS_`.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::model::{Draft, ProjectileChoice, ScalarField, ValueSource};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerCloneDiffRequest, DesignerCloneRequest, DesignerProjectileOwnRequest, DraftDto,
    WritePlanDto,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::InstallBuilder;
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{Ctx, clone_diff, clone_draft, export_plan, projectile_own};
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_top_level;

const PROJECT: &str = "p-own";

fn family() -> Vec<Node> {
    let gun_base = NodeBuilder::new("ThingDef")
        .attr("Name", "RS_BaseGun")
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .text_elem("techLevel", "Industrial")
        .elem("statBases", |s| s.text_elem("RS_Flammability", "0.5"))
        .elem("weaponTags", |w| w.li("RS_BaseTag"))
        .build();
    let bullet_base = NodeBuilder::new("ThingDef")
        .attr("Name", "RS_BaseBullet")
        .attr("Abstract", "True")
        .text_elem("category", "Projectile")
        .elem("projectile", |p| p.text_elem("damageDef", "RS_Damage"))
        .build();
    let shot = NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseBullet")
        .text_elem("defName", "RS_RichShot")
        .text_elem("label", "rich shot")
        .elem("graphicData", |g| {
            g.text_elem("texPath", "RS/Shot")
                .text_elem("graphicClass", "Graphic_Single")
        })
        .elem("projectile", |p| {
            p.text_elem("damageAmountBase", "14")
                .text_elem("speed", "60")
                .text_elem("explosionRadius", "1.9")
        })
        .build();
    // a gun that sets a recipe, comps, a sound, extras and its own tags
    let mut rich = common::gun("RS_RichGun", "RS_RichShot", 5, "Industrial", "RS_Rifle");
    rich.set_attr("ParentName", "RS_BaseGun");
    rich.remove_child("techLevel");
    for extra in [
        NodeBuilder::new("recipeMaker")
            .elem("skillRequirements", |s| s.text_elem("RS_Crafting", "5"))
            .text_elem("displayPriority", "415")
            .build(),
        Node::with_text("soundInteract", "RS_Click"),
        Node::with_text("relicChance", "2"),
        NodeBuilder::new("comps")
            .elem("li", |li| {
                li.attr("Class", "RS_CompArt")
                    .text_elem("nameMaker", "RS_Namer")
            })
            .build(),
    ] {
        rich.push_child(extra);
    }
    // a gun that is not craftable (no cost list, no work)
    let mut relic = common::gun("RS_RelicGun", "RS_RichShot", 6, "Industrial", "RS_Rifle");
    relic.remove_child("costList");
    if let Some(stats) = relic.child_mut("statBases") {
        stats.remove_child("WorkToMake");
    }
    let taken = NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseBullet")
        .text_elem("defName", "RS_Bullet_Taken")
        .build();
    vec![gun_base, bullet_base, shot, rich, relic, taken]
}

fn fixture() -> common::Fixture {
    let mut defs = common::core_defs_n(14, 8);
    defs.extend(family());
    let install = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .mod_folder(common::extra_mod())
        .mod_folder(common::ce_mod())
        .build_temp()
        .unwrap();
    let session = common::open_session(&install, false);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let ctx = Ctx::new(session.clone(), &roots, Arc::new(clock.clone())).unwrap();
    common::Fixture {
        install,
        session,
        tmp,
        roots,
        clock,
        ctx,
    }
}

fn request(source: &str, name: &str, own: Option<bool>) -> DesignerCloneRequest {
    DesignerCloneRequest {
        project_id: PROJECT.into(),
        source: source.into(),
        def_name: name.into(),
        label: None,
        mod_prefix: Some("RS".into()),
        own_projectile: own,
    }
}

fn cloned(f: &common::Fixture, source: &str, name: &str, own: Option<bool>) -> Draft {
    let response = clone_draft(&f.ctx, request(source, name, own)).unwrap();
    draft_from_dto(&response.entry.draft).unwrap()
}

fn plan_of(f: &common::Fixture, draft: &Draft) -> WritePlanDto {
    let project = common_project::project(f, &[]);
    export_plan(&f.ctx, common_project::request(&project, &draft.spec)).unwrap()
}

fn written(plan: &WritePlanDto) -> Vec<Node> {
    let file = plan
        .files
        .iter()
        .find(|f| f.kind == rimstudio_ipc_types::designer::FileKindDto::VanillaDefs)
        .unwrap();
    parse_top_level(file.rendered.as_bytes(), ParseMode::Tolerant, "Defs")
        .unwrap()
        .nodes
}

fn by_name<'a>(nodes: &'a [Node], name: &str) -> &'a Node {
    nodes
        .iter()
        .find(|n| n.child_text("defName") == Some(name))
        .unwrap_or_else(|| panic!("no def {name}"))
}

#[test]
fn a_clone_carries_the_recipe_the_sound_the_comps_and_the_raw_fields_of_its_source() {
    let f = fixture();
    let draft = cloned(&f, "RS_RichGun", "RS_CloneRich", Some(false));
    let spec = &draft.spec;
    let recipe = spec.recipe.as_ref().unwrap();
    assert_eq!(recipe.skill_requirements.get("RS_Crafting"), Some(&5));
    assert_eq!(recipe.display_priority, Some(415.0));
    assert_eq!(spec.sound_interact.as_deref(), Some("RS_Click"));
    assert_eq!(spec.comps.len(), 1);
    let raw: Vec<&str> = spec.extra_fields.iter().map(|n| n.tag.as_str()).collect();
    assert!(raw.contains(&"relicChance"), "{raw:?}");
    // the parent supplies the tech level and the base tag
    assert_eq!(
        spec.parent.as_ref().unwrap().inherited_tech_level,
        spec.tech_level
    );
    assert_eq!(spec.weapon_tags, vec!["RS_Rifle"]);
    let plan = plan_of(&f, &draft);
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let nodes = written(&plan);
    let weapon = by_name(&nodes, "RS_CloneRich");
    assert!(
        weapon.child("techLevel").is_none(),
        "the parent supplies it"
    );
    assert_eq!(weapon.child_text("soundInteract"), Some("RS_Click"));
    assert_eq!(weapon.child_text("relicChance"), Some("2"));
    assert_eq!(
        weapon
            .child("recipeMaker")
            .unwrap()
            .child("skillRequirements")
            .unwrap()
            .child_text("RS_Crafting"),
        Some("5")
    );
    assert_eq!(
        weapon
            .child("comps")
            .unwrap()
            .elements()
            .next()
            .unwrap()
            .attr("Class"),
        Some("RS_CompArt")
    );
}

#[test]
fn a_clone_has_a_projectile_of_its_own_by_default_named_after_the_weapon() {
    let f = fixture();
    let response = clone_draft(&f.ctx, request("RS_RichGun", "RS_CloneRich", None)).unwrap();
    let draft = draft_from_dto(&response.entry.draft).unwrap();
    let Some(ProjectileChoice::Inline(p)) = draft.spec.ranged.as_ref().unwrap().projectile.clone()
    else {
        panic!("an own projectile is the default");
    };
    assert_eq!(p.def_name, "RS_Bullet_CloneRich");
    assert_eq!(p.copied_from.as_deref(), Some("RS_RichShot"));
    assert_eq!(p.parent.as_deref(), Some("RS_BaseBullet"));
    assert_eq!(p.texture_path.as_deref(), Some("RS/Shot"));
    assert_eq!(p.extra.len(), 1);
    let notes = response.notes.join("\n");
    assert!(
        notes.contains("RS_Bullet_CloneRich") && notes.contains("RS_RichShot"),
        "{notes}"
    );
    // the plan writes the projectile before the weapon, in the same file, with the source numbers
    let plan = plan_of(&f, &draft);
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(plan.files.len(), 1);
    let nodes = written(&plan);
    assert_eq!(nodes[0].child_text("defName"), Some("RS_Bullet_CloneRich"));
    let props = nodes[0].child("projectile").unwrap();
    assert_eq!(props.child_text("damageAmountBase"), Some("14"));
    assert_eq!(props.child_text("explosionRadius"), Some("1.9"));
    assert!(props.child("damageDef").is_none(), "the parent supplies it");
    let weapon = by_name(&nodes, "RS_CloneRich");
    let verb = weapon.child("verbs").unwrap().elements().next().unwrap();
    assert_eq!(
        verb.child_text("defaultProjectile"),
        Some("RS_Bullet_CloneRich")
    );
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "design.texture-shared")
    );
}

#[test]
fn a_damage_edit_of_a_clone_with_its_own_projectile_changes_the_written_file() {
    let f = fixture();
    let mut draft = cloned(&f, "RS_RichGun", "RS_CloneRich", None);
    draft
        .spec
        .offer(ScalarField::Damage, 20.0, ValueSource::Typed);
    let nodes = written(&plan_of(&f, &draft));
    assert_eq!(
        nodes[0]
            .child("projectile")
            .unwrap()
            .child_text("damageAmountBase"),
        Some("20")
    );
}

#[test]
fn the_shared_projectile_stays_when_the_clone_asks_for_it() {
    let f = fixture();
    let draft = cloned(&f, "RS_RichGun", "RS_CloneRich", Some(false));
    assert_eq!(
        draft.spec.ranged.as_ref().unwrap().projectile,
        Some(ProjectileChoice::Reference("RS_RichShot".into()))
    );
    let nodes = written(&plan_of(&f, &draft));
    assert_eq!(nodes.len(), 1, "no projectile is written");
}

#[test]
fn an_untouched_clone_with_its_own_projectile_has_an_empty_diff_and_an_edit_shows_alone() {
    let f = fixture();
    let draft = cloned(&f, "RS_RichGun", "RS_CloneRich", None);
    let diff = clone_diff(
        &f.ctx,
        DesignerCloneDiffRequest {
            draft: draft_to_dto(&draft).unwrap(),
        },
    )
    .unwrap();
    assert!(diff.changes.is_empty(), "{:?}", diff.changes);
    let mut edited = draft.clone();
    edited
        .spec
        .offer(ScalarField::Damage, 20.0, ValueSource::Typed);
    let diff = clone_diff(
        &f.ctx,
        DesignerCloneDiffRequest {
            draft: draft_to_dto(&edited).unwrap(),
        },
    )
    .unwrap();
    let fields: Vec<&str> = diff.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, ["/ranged/damage"]);
    assert!(
        diff.notes.is_empty(),
        "the damage is written now: {:?}",
        diff.notes
    );
}

fn switch(f: &common::Fixture, draft: &Draft, own: bool) -> (Draft, Vec<String>) {
    let response = projectile_own(
        &f.ctx,
        DesignerProjectileOwnRequest {
            draft: draft_to_dto(draft).unwrap(),
            own,
        },
    )
    .unwrap();
    (draft_from_dto(&response.draft).unwrap(), response.notes)
}

#[test]
fn the_own_projectile_can_be_switched_on_and_off_and_back() {
    let f = fixture();
    let shared = cloned(&f, "RS_RichGun", "RS_CloneRich", Some(false));
    let (own, notes) = switch(&f, &shared, true);
    let Some(ProjectileChoice::Inline(p)) = own.spec.ranged.as_ref().unwrap().projectile.clone()
    else {
        panic!("switched on");
    };
    assert_eq!(p.def_name, "RS_Bullet_CloneRich");
    assert!(notes.join(" ").contains("RS_RichShot"));
    // on again changes nothing
    let (again, notes) = switch(&f, &own, true);
    assert_eq!(again, own);
    assert!(notes.join(" ").contains("already"));
    // off points back at the shared projectile
    let (back, _) = switch(&f, &own, false);
    assert_eq!(
        back.spec.ranged.as_ref().unwrap().projectile,
        Some(ProjectileChoice::Reference("RS_RichShot".into()))
    );
    assert_eq!(
        back.spec.ranged.as_ref().unwrap().damage,
        shared.spec.ranged.as_ref().unwrap().damage
    );
}

#[test]
fn the_projectile_switch_refuses_in_plain_words() {
    let f = fixture();
    // a melee weapon has no projectile
    let melee = {
        let response = clone_draft(&f.ctx, request("RS_Blade01", "RS_CloneBlade", None)).unwrap();
        response.entry.draft
    };
    let err = projectile_own(
        &f.ctx,
        DesignerProjectileOwnRequest {
            draft: melee,
            own: true,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
    assert!(err.to_string().contains("ranged"), "{err}");
    // a draft whose projectile was never copied cannot point back
    let mut draft = cloned(&f, "RS_RichGun", "RS_CloneRich", None);
    if let Some(ProjectileChoice::Inline(p)) =
        draft.spec.ranged.as_mut().unwrap().projectile.as_mut()
    {
        p.copied_from = None;
    }
    let err = projectile_own(
        &f.ctx,
        DesignerProjectileOwnRequest {
            draft: draft_to_dto(&draft).unwrap(),
            own: false,
        },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("nothing to point back at"),
        "{err}"
    );
}

#[test]
fn the_name_of_the_own_projectile_avoids_a_name_a_loaded_def_uses() {
    let f = fixture();
    // a loaded def already has the derived name, so the clone gets a suffix
    let first = cloned(&f, "RS_RichGun", "RS_Taken", None);
    let name = match first
        .spec
        .ranged
        .as_ref()
        .unwrap()
        .projectile
        .as_ref()
        .unwrap()
    {
        ProjectileChoice::Inline(p) => p.def_name.clone(),
        ProjectileChoice::Reference(_) => panic!("own"),
    };
    assert_eq!(name, "RS_Bullet_Taken_2");
}

#[test]
fn a_clone_of_a_weapon_that_cannot_be_crafted_plans_without_errors() {
    let f = fixture();
    let draft = cloned(&f, "RS_RelicGun", "RS_CloneRelic", None);
    assert!(
        draft
            .spec
            .accepted_missing
            .contains(&"/costList".to_owned())
    );
    assert!(
        draft
            .spec
            .accepted_missing
            .contains(&"/workToMake".to_owned())
    );
    let plan = plan_of(&f, &draft);
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "design.accepted-missing")
    );
    let weapon = by_name(&written(&plan), "RS_CloneRelic").clone();
    assert!(weapon.child("costList").is_none());
}

#[test]
fn the_draft_record_of_schema_one_is_migrated_when_it_is_loaded() {
    let f = fixture();
    let draft = cloned(&f, "RS_RichGun", "RS_CloneRich", Some(false));
    // rewrite the stored document the way an older build wrote it: schema 1 and no new fields
    let dir = f.roots.data.join("designer-drafts");
    let entry = std::fs::read_dir(dir.as_std_path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.extension().is_some_and(|e| e == "json") && !p.to_string_lossy().contains("index")
        })
        .expect("a stored draft");
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&entry).unwrap()).unwrap();
    let text = doc.to_string();
    assert!(text.contains("\"schemaVersion\":2"), "{text}");
    doc["schemaVersion"] = serde_json::json!(1);
    doc["draft"]["schemaVersion"] = serde_json::json!(1);
    std::fs::write(&entry, serde_json::to_vec(&doc).unwrap()).unwrap();
    let ctx2 = Ctx::new(
        f.session.clone(),
        &f.roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    let list = rimstudio_toolkit::designer::draft_list(
        &ctx2,
        rimstudio_ipc_types::designer::DesignerDraftListRequest {
            project_id: PROJECT.into(),
        },
    )
    .unwrap();
    assert_eq!(list.drafts.len(), 1);
    let loaded: DraftDto = list.drafts[0].draft.clone();
    assert_eq!(loaded.schema_version, Draft::VERSION);
    assert_eq!(draft_from_dto(&loaded).unwrap().spec, draft.spec);
}
