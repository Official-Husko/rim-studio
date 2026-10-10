//! The weapons a project already has: the list of the Weapons tab, opening one as a draft that edits it where
//! it stands, and the plan that replaces the definition in its own file. Names are fictional and every
//! number is invented.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use common_project::{Proj, fixture, project, put};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_ipc_types::designer::{
    DesignerApplyPlanRequest, DesignerExportPlanRequest, DesignerOpenWeaponRequest,
    DesignerProjectWeaponsRequest, DraftDto, FileActionDto, FileKindDto, ItemKindDto,
};
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{apply_plan, draft_load, open_weapon, project_weapons};
use rimstudio_toolkit::error::ToolkitError;

const FILE: &str = "Defs/RS_ProjWeapons.xml";

const DEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Defs>
  <!-- the first gun, written by hand -->
  <ThingDef ParentName="RS_BaseGun">
    <defName>RS_HandGun</defName>
    <label>hand gun</label>
    <description>A fictional pistol.</description>
    <techLevel>Industrial</techLevel>
    <statBases>
      <Mass>1.5</Mass>
      <RangedWeapon_Cooldown>1.2</RangedWeapon_Cooldown>
      <MarketValue>250</MarketValue>
    </statBases>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_HandBullet</defaultProjectile>
        <warmupTime>0.8</warmupTime>
        <range>24</range>
        <burstShotCount>1</burstShotCount>
      </li>
    </verbs>
    <weaponTags><li>RS_Pistol</li></weaponTags>
    <comps>
      <li Class="CompProperties_Forbiddable"/>
    </comps>
  </ThingDef>
  <ThingDef ParentName="RS_BaseBullet">
    <defName>RS_HandBullet</defName>
    <label>hand bullet</label>
    <graphicData>
      <texPath>Things/Projectile/RS_HandBullet</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
    <projectile>
      <damageDef>RS_Damage</damageDef>
      <damageAmountBase>9</damageAmountBase>
      <speed>70</speed>
    </projectile>
  </ThingDef>
  <ThingDef ParentName="RS_BaseMelee">
    <defName>RS_HandBlade</defName>
    <label>hand blade</label>
    <techLevel>Industrial</techLevel>
    <statBases><Mass>1.1</Mass></statBases>
    <weaponTags><li>RS_Melee</li></weaponTags>
    <tools>
      <li>
        <label>edge</label>
        <capacities><li>Cut</li></capacities>
        <power>11</power>
        <cooldownTime>1.9</cooldownTime>
      </li>
    </tools>
  </ThingDef>
  <ThingDef>
    <defName>RS_HandRock</defName>
    <label>hand rock</label>
    <category>Item</category>
  </ThingDef>
  <ThingDef Name="RS_ProjBase" Abstract="True">
    <category>Item</category>
    <weaponTags><li>RS_Pistol</li></weaponTags>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_HandBullet</defaultProjectile>
      </li>
    </verbs>
  </ThingDef>
</Defs>
"#;

fn setup() -> (common::Fixture, Proj) {
    let f = fixture(false);
    let p = project(&f, &[(FILE, DEFS)]);
    (f, p)
}

fn read(p: &Proj, rel: &str) -> String {
    std::fs::read_to_string(p.root.join(rel).as_std_path()).unwrap()
}

fn open(
    f: &common::Fixture,
    p: &Proj,
    name: &str,
    reload: bool,
) -> Result<rimstudio_ipc_types::designer::DesignerOpenWeaponResponse, ToolkitError> {
    open_weapon(
        &f.ctx,
        DesignerOpenWeaponRequest {
            project_id: p.id.clone(),
            def_name: name.into(),
            reload: reload.then_some(true),
        },
    )
}

fn request(p: &Proj, draft: &DraftDto) -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        project_id: p.id.clone(),
        draft: draft.clone(),
        convert: None,
        accept_suggestions: None,
    }
}

/// The text of one `<ThingDef>` element that holds `defName`, from its start tag to its end tag.
fn element_of(text: &str, def_name: &str) -> String {
    let at = text
        .find(&format!("<defName>{def_name}</defName>"))
        .unwrap_or_else(|| panic!("{def_name} is not in the file:\n{text}"));
    let start = text[..at].rfind("<ThingDef").unwrap();
    let end = at + text[at..].find("</ThingDef>").unwrap() + "</ThingDef>".len();
    text[start..end].to_owned()
}

#[test]
fn the_list_has_the_weapons_the_project_defines_with_their_numbers() {
    let (f, p) = setup();
    let listed = project_weapons(
        &f.ctx,
        DesignerProjectWeaponsRequest {
            project_id: p.id.clone(),
        },
    )
    .unwrap();
    let names: Vec<&str> = listed.weapons.iter().map(|w| w.def_name.as_str()).collect();
    assert_eq!(names, vec!["RS_HandBlade", "RS_HandGun"], "{names:?}");
    let gun = listed
        .weapons
        .iter()
        .find(|w| w.def_name == "RS_HandGun")
        .unwrap();
    assert_eq!(gun.kind, ItemKindDto::Ranged);
    assert_eq!(gun.label, "hand gun");
    assert_eq!(gun.file, FILE);
    assert_eq!(gun.parent.as_deref(), Some("RS_BaseGun"));
    assert_eq!(gun.projectile.as_deref(), Some("RS_HandBullet"));
    assert_eq!(gun.tags, vec!["RS_Pistol".to_owned()]);
    assert_eq!(gun.numbers.range, Some(24.0));
    assert_eq!(gun.numbers.damage, Some(9.0));
    assert_eq!(gun.numbers.mass, Some(1.5));
    assert_eq!(gun.numbers.market_value, Some(250.0));
    assert!(!gun.generated_patch && gun.draft_id.is_none());
    let blade = listed
        .weapons
        .iter()
        .find(|w| w.def_name == "RS_HandBlade")
        .unwrap();
    assert_eq!(blade.kind, ItemKindDto::Melee);
    assert_eq!(blade.numbers.damage, Some(11.0));
    assert_eq!(blade.numbers.cooldown, Some(1.9));
    assert!(blade.projectile.is_none());
}

#[test]
fn opening_a_weapon_stores_a_draft_with_its_numbers_and_its_file() {
    let (f, p) = setup();
    let opened = open(&f, &p, "RS_HandGun", false).unwrap();
    assert!(!opened.reused);
    assert_eq!(opened.file, FILE);
    let draft = &opened.entry.draft;
    assert_eq!(draft.edits_file.as_deref(), Some(FILE));
    assert!(draft.cloned_from.is_none());
    assert_eq!(draft.spec.identity.def_name, "RS_HandGun");
    assert_eq!(draft.spec.identity.label, "hand gun");
    assert!(draft.spec.ce.is_none(), "the block stays off");
    // the role is the one of the nearest reference weapon, and the note says where it comes from
    assert!(draft.spec.role.is_some(), "{:?}", draft.spec.role);
    assert!(
        opened
            .notes
            .iter()
            .any(|n| n.contains("nearest reference weapon")),
        "{:?}",
        opened.notes
    );
    let spec = serde_json::to_value(&draft.spec).unwrap();
    assert_eq!(spec["ranged"]["range"]["value"], 24.0);
    assert_eq!(spec["ranged"]["range"]["source"], "typed");
    assert_eq!(spec["ranged"]["damage"]["value"], 9.0);
    // the projectile is defined in the same file, so it is the weapon's own and its damage is editable
    assert_eq!(spec["ranged"]["projectile"]["mode"], "inline");
    assert_eq!(
        spec["ranged"]["projectile"]["def"]["defName"], "RS_HandBullet",
        "{}",
        spec["ranged"]["projectile"]
    );
    assert!(
        spec["ranged"]["projectile"]["def"]
            .get("copiedFrom")
            .is_none()
    );
    assert!(
        opened.notes.iter().any(|n| n.contains("RS_HandBullet")),
        "{:?}",
        opened.notes
    );
    // the draft is stored and found again
    let loaded = draft_load(&f.ctx, &p.id, &opened.entry.id).unwrap();
    assert_eq!(loaded.draft.edits_file.as_deref(), Some(FILE));
}

#[test]
fn opening_again_continues_the_draft_and_reload_reads_the_file_again() {
    let (f, p) = setup();
    let first = open(&f, &p, "RS_HandGun", false).unwrap();
    let again = open(&f, &p, "RS_HandGun", false).unwrap();
    assert!(again.reused);
    assert_eq!(again.entry.id, first.entry.id);
    let fresh = open(&f, &p, "RS_HandGun", true).unwrap();
    assert!(!fresh.reused);
    assert_eq!(
        fresh.entry.id, first.entry.id,
        "the stored draft is replaced"
    );
    // the list shows the draft
    let listed = project_weapons(
        &f.ctx,
        DesignerProjectWeaponsRequest {
            project_id: p.id.clone(),
        },
    )
    .unwrap();
    let gun = listed
        .weapons
        .iter()
        .find(|w| w.def_name == "RS_HandGun")
        .unwrap();
    assert_eq!(gun.draft_id.as_deref(), Some(first.entry.id.as_str()));
}

#[test]
fn a_name_the_project_does_not_define_or_a_thing_that_is_no_weapon_is_refused() {
    let (f, p) = setup();
    for name in ["RS_Nope", "RS_HandRock", "RS_ProjBase"] {
        let err = open(&f, &p, name, false).unwrap_err();
        assert!(
            matches!(err, ToolkitError::InvalidDraft { .. }),
            "{name}: {err:?}"
        );
    }
}

#[test]
fn the_plan_of_an_opened_weapon_replaces_the_definition_where_it_stands() {
    let (f, p) = setup();
    let before = read(&p, FILE);
    let opened = open(&f, &p, "RS_HandGun", false).unwrap();
    let plan = export_plan(&f.ctx, request(&p, &opened.entry.draft)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    // one file, the file of the definition, and no second definition anywhere
    let defs: Vec<_> = plan
        .files
        .iter()
        .filter(|x| x.kind == FileKindDto::VanillaDefs)
        .collect();
    assert_eq!(
        defs.len(),
        1,
        "{:?}",
        plan.files.iter().map(|x| &x.path).collect::<Vec<_>>()
    );
    assert_eq!(defs[0].path, FILE);
    // nothing was edited, so nothing is rewritten: the file keeps its bytes, comments and order
    assert_eq!(
        defs[0].action,
        FileActionDto::Unchanged,
        "{:?}",
        defs[0].diff
    );
    assert_eq!(defs[0].rendered, before);
    assert_eq!(
        defs[0]
            .rendered
            .matches("<defName>RS_HandGun</defName>")
            .count(),
        1
    );
    assert_eq!(
        defs[0]
            .rendered
            .matches("<defName>RS_HandBullet</defName>")
            .count(),
        1
    );
    // the other definitions of the file are byte identical
    for other in ["RS_HandBlade", "RS_HandRock"] {
        assert_eq!(
            element_of(&defs[0].rendered, other),
            element_of(&before, other)
        );
    }
    assert!(
        defs[0]
            .rendered
            .contains("<!-- the first gun, written by hand -->")
    );
}

#[test]
fn an_edited_number_is_written_into_the_same_file_and_nothing_else_changes() {
    let (f, p) = setup();
    let before = read(&p, FILE);
    let opened = open(&f, &p, "RS_HandGun", false).unwrap();
    let mut value = serde_json::to_value(&opened.entry.draft).unwrap();
    value["spec"]["ranged"]["range"] = serde_json::json!({"value": 31.0, "source": "typed"});
    value["spec"]["ranged"]["damage"] = serde_json::json!({"value": 14.0, "source": "typed"});
    let draft: DraftDto = serde_json::from_value(value).unwrap();
    let req = request(&p, &draft);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    apply_plan(
        &f.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id,
            request: req,
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let after = read(&p, FILE);
    // exactly the two edited values changed: no member moved and no line was added or lost
    assert_eq!(before.lines().count(), after.lines().count());
    let changed = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(changed, 2, "{after}");
    let gun = element_of(&after, "RS_HandGun");
    assert!(gun.contains("<range>31</range>"), "{gun}");
    let bullet = element_of(&after, "RS_HandBullet");
    assert!(
        bullet.contains("<damageAmountBase>14</damageAmountBase>"),
        "{bullet}"
    );
    for other in ["RS_HandBlade", "RS_HandRock"] {
        assert_eq!(element_of(&after, other), element_of(&before, other));
    }
    assert_eq!(after.matches("<defName>RS_HandGun</defName>").count(), 1);
    // no second definition file appears
    assert!(!p.root.join("Defs/ThingDefs_Weapons").as_std_path().exists());
    // the comment of the person is kept
    assert!(after.contains("<!-- the first gun, written by hand -->"));
    // the project lists the new numbers
    let listed = project_weapons(
        &f.ctx,
        DesignerProjectWeaponsRequest {
            project_id: p.id.clone(),
        },
    )
    .unwrap();
    let gun = listed
        .weapons
        .iter()
        .find(|w| w.def_name == "RS_HandGun")
        .unwrap();
    assert_eq!(gun.numbers.range, Some(31.0));
    assert_eq!(gun.numbers.damage, Some(14.0));
}

#[test]
fn a_draft_whose_file_is_gone_cannot_be_applied_and_reading_again_finds_the_new_place() {
    let (f, p) = setup();
    let opened = open(&f, &p, "RS_HandGun", false).unwrap();
    // a layout fix moved the definition file
    std::fs::create_dir_all(p.root.join("Defs/Moved").as_std_path()).unwrap();
    std::fs::rename(
        p.root.join(FILE).as_std_path(),
        p.root.join("Defs/Moved/Guns.xml").as_std_path(),
    )
    .unwrap();
    let plan = export_plan(&f.ctx, request(&p, &opened.entry.draft)).unwrap();
    assert!(plan.has_errors);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "designer.edits-file-missing"),
        "{:?}",
        plan.diagnostics
    );
    // reading the weapon again brings the same draft to the new file
    let fresh = open(&f, &p, "RS_HandGun", true).unwrap();
    assert_eq!(fresh.file, "Defs/Moved/Guns.xml");
    assert_eq!(fresh.entry.id, opened.entry.id);
    assert_eq!(
        fresh.entry.draft.edits_file.as_deref(),
        Some("Defs/Moved/Guns.xml")
    );
    let plan = export_plan(&f.ctx, request(&p, &fresh.entry.draft)).unwrap();
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
}

#[test]
fn renaming_a_weapon_that_is_edited_where_it_stands_is_refused() {
    let (f, p) = setup();
    let opened = open(&f, &p, "RS_HandGun", false).unwrap();
    let mut value = serde_json::to_value(&opened.entry.draft).unwrap();
    value["spec"]["identity"]["defName"] = serde_json::json!("RS_HandGunCopy");
    let draft: DraftDto = serde_json::from_value(value).unwrap();
    let plan = export_plan(&f.ctx, request(&p, &draft)).unwrap();
    assert!(plan.has_errors);
    let diag = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "designer.edits-name-changed")
        .unwrap_or_else(|| panic!("{:?}", plan.diagnostics));
    assert_eq!(diag.field.as_deref(), Some("/identity/defName"));
}

#[test]
fn a_weapon_of_a_file_that_changed_is_read_again_on_reload() {
    let (f, p) = setup();
    open(&f, &p, "RS_HandBlade", false).unwrap();
    put(
        &p.root,
        FILE,
        &DEFS.replace("<power>11</power>", "<power>13</power>"),
    );
    let fresh = open(&f, &p, "RS_HandBlade", true).unwrap();
    let spec = serde_json::to_value(&fresh.entry.draft.spec).unwrap();
    assert_eq!(spec["tools"][0]["power"]["value"], 13.0, "{spec}");
}
