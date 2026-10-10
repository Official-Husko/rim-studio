//! The weapons a project already has (`designer_project_weapons`) and opening one of them in the designer
//! (`designer_open_weapon`).
//!
//! Both read the open project through a workspace session over the reference set without Combat Extended
//! ([`Ctx::vanilla_project_session`]), so the numbers are the ones the base game loads and a conversion
//! patch of the project does not change what the editor shows. The raw definition nodes (the way the
//! project's files write them) come from the files themselves, like the clone flow reads its source.
//!
//! Opening stores a draft that carries the project file of the definition (`edits_file`); planning that
//! draft replaces the definition where it stands (see `designer::plan`), so the weapon is never written
//! twice. The definition the draft reads is the one the project's file holds: what the designer does not
//! model stays in the draft as raw fields and is written back unchanged.

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_design::model::{CalibrationMode, Draft, ItemKind, ProjectileChoice, ValueSource};
use rimstudio_design::reader::access::child_text;
use rimstudio_design::reader::{
    OwnSource, is_weapon_def, projectile_spec_from_def, shooting_verb, spec_from_def_own,
};
use rimstudio_ipc_types::designer::{
    DesignerDraftListRequest, DesignerDraftSaveRequest, DesignerOpenWeaponRequest,
    DesignerOpenWeaponResponse, DesignerProjectWeaponsRequest, DesignerProjectWeaponsResponse,
    DraftEntryDto, ProjectWeaponDto,
};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;

use super::clone::{missing_in_source, nearest};
use super::convert::{ProjectDefs, project_defs};
use super::convert_facts::facts_of;
use super::ctx::Ctx;
use super::drafts::{check_part, draft_list, draft_save};
use super::dto::{draft_to_dto, kind_to_dto, pool_kind};
use super::patch_remove::generated_sections;
use crate::error::{ToolkitError, ToolkitResult};

/// The def type every weapon and projectile belongs to.
const THING_TYPE: &str = "ThingDef";

/// The kind of weapon a resolved definition is, when the designer can model it: a gun has a shooting verb
/// that names a projectile; a melee weapon has tools and is a weapon def.
fn weapon_kind(node: &Node) -> Option<ItemKind> {
    if shooting_verb(node).is_some() {
        Some(ItemKind::Ranged)
    } else if node.child("tools").is_some() && is_weapon_def(node) {
        Some(ItemKind::Melee)
    } else {
        None
    }
}

fn is_abstract(raw: &Node) -> bool {
    raw.attr("Abstract")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

/// The raw node of the project's definition `name`.
fn raw_def<'a>(defs: &'a ProjectDefs, name: &str) -> Option<&'a Node> {
    defs.nodes
        .iter()
        .find(|n| n.tag == THING_TYPE && n.child_text("defName") == Some(name))
}

/// The stored draft that edits the weapon `def_name` of `file`, newest first.
fn draft_of(
    ctx: &Ctx,
    project_id: &str,
    def_name: &str,
    file: &str,
) -> ToolkitResult<Option<DraftEntryDto>> {
    let listed = draft_list(
        ctx,
        DesignerDraftListRequest {
            project_id: project_id.to_owned(),
        },
    )?;
    // a draft that edits this weapon in the file it is in now, else one of the same name that edits it in
    // a file it has been moved from (a layout fix), which "read the file again" brings up to date
    let mut same_name: Option<DraftEntryDto> = None;
    for d in listed.drafts {
        if d.def_name != def_name || d.draft.edits_file.is_none() {
            continue;
        }
        if d.draft.edits_file.as_deref() == Some(file) {
            return Ok(Some(d));
        }
        same_name.get_or_insert(d);
    }
    Ok(same_name)
}

/// Lists the weapons the open project defines, with the numbers the Weapons tab shows
/// (`designer_project_weapons`). Reads the project, writes nothing.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad project id, [`ToolkitError::ProjectNotOpen`],
/// [`ToolkitError::ReferenceUnavailable`] without a game install, [`ToolkitError::Workspace`] when the
/// folder is not a mod.
pub fn project_weapons(
    ctx: &Ctx,
    req: DesignerProjectWeaponsRequest,
) -> ToolkitResult<DesignerProjectWeaponsResponse> {
    check_part("project id", &req.project_id)?;
    let (_record, view) = ctx.env().view(&req.project_id)?;
    let session = ctx.vanilla_project_session(&view.root)?;
    let snapshot = session.current();
    let defs = project_defs(&session, &view);
    let generated = generated_sections(&view.root);
    let drafts = draft_list(
        ctx,
        DesignerDraftListRequest {
            project_id: req.project_id.clone(),
        },
    )?
    .drafts;
    let mut diagnostics: Vec<Diagnostic> = view.diagnostics.clone();
    for (path, why) in &defs.unreadable {
        diagnostics.push(
            Diagnostic::new(
                DiagCode::new("designer.def-file-unreadable"),
                Severity::Warning,
                format!("{path} could not be read: {why}"),
            )
            .with_arg("path", path),
        );
    }
    let mut weapons = Vec::new();
    for raw in defs.nodes.iter().filter(|n| n.tag == THING_TYPE) {
        let Some(name) = raw.child_text("defName") else {
            continue;
        };
        if is_abstract(raw) {
            continue;
        }
        let Some(record) = snapshot.databases.get(THING_TYPE, name) else {
            continue;
        };
        let Some(kind) = weapon_kind(&record.node) else {
            continue;
        };
        let Some(file) = defs.files.get(name) else {
            continue;
        };
        let facts = facts_of(&snapshot.databases, name);
        let label = child_text(&record.node, "label")
            .or_else(|| child_text(raw, "label"))
            .unwrap_or_default();
        weapons.push(ProjectWeaponDto {
            def_name: name.to_owned(),
            label,
            kind: kind_to_dto(kind)?,
            file: file.clone(),
            parent: raw.attr("ParentName").map(str::to_owned),
            tech_level: child_text(&record.node, "techLevel"),
            projectile: shooting_verb(&record.node)
                .and_then(|v| child_text(v, "defaultProjectile")),
            tags: facts.tags,
            weapon_classes: facts.weapon_classes,
            numbers: facts.numbers.unwrap_or_default(),
            generated_patch: generated.contains_key(name),
            draft_id: drafts
                .iter()
                .find(|d| d.def_name == name && d.draft.edits_file.as_deref() == Some(file))
                .map(|d| d.id.clone()),
        });
    }
    weapons.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    Ok(DesignerProjectWeaponsResponse {
        weapons,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    })
}

/// Opens a weapon of the project as a draft that edits it where it stands (`designer_open_weapon`).
///
/// A stored draft of the weapon is continued unless `reload` is set. Otherwise the definition is read from
/// the project's file: every field the designer models becomes a typed value, everything else stays as a raw
/// field, a projectile defined in the same file becomes the weapon's own projectile (the damage edits change
/// that definition), and the Combat Extended block is absent. Nothing is written to the project.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad id, a name the project does not define or a definition that is
/// not a weapon the designer models, [`ToolkitError::ReferenceUnavailable`] without a game install,
/// [`ToolkitError::Design`] when the definition cannot be read, and store errors from saving the draft.
pub fn open_weapon(
    ctx: &Ctx,
    req: DesignerOpenWeaponRequest,
) -> ToolkitResult<DesignerOpenWeaponResponse> {
    check_part("project id", &req.project_id)?;
    let name = req.def_name.trim();
    let (_record, view) = ctx.env().view(&req.project_id)?;
    let session = ctx.vanilla_project_session(&view.root)?;
    let snapshot = session.current();
    let defs = project_defs(&session, &view);
    let (Some(raw), Some(file)) = (raw_def(&defs, name), defs.files.get(name)) else {
        return Err(ToolkitError::invalid_draft(format!(
            "{name} is not a weapon this mod defines in its own files"
        )));
    };
    let reload = req.reload.unwrap_or(false);
    let known = draft_of(ctx, &req.project_id, name, file)?;
    if let Some(entry) = known.clone().filter(|_| !reload) {
        return Ok(DesignerOpenWeaponResponse {
            entry,
            file: file.clone(),
            reused: true,
            notes: Vec::new(),
        });
    }
    let record = snapshot.databases.get(THING_TYPE, name).ok_or_else(|| {
        ToolkitError::invalid_draft(format!(
            "{name} is among the mod's files but not among the loaded definitions; it may be \
                 abstract or have a parent the game cannot find"
        ))
    })?;
    if is_abstract(raw) {
        return Err(ToolkitError::invalid_draft(format!(
            "{name} is an abstract base, not a weapon"
        )));
    }
    if weapon_kind(&record.node).is_none() {
        return Err(ToolkitError::invalid_draft(format!(
            "{name} is not a gun with a projectile or a melee weapon with tools, which are the weapons \
             the designer models"
        )));
    }
    let reading = spec_from_def_own(
        record,
        &snapshot.databases,
        ctx.reader_options(),
        ValueSource::Typed,
        &OwnSource {
            def: raw,
            projectile: None,
        },
    )?;
    let mut spec = reading.spec;
    let mut notes = reading.notes;
    spec.ce = None;
    if spec.kind == ItemKind::Ranged
        && let Some(ranged) = spec.ranged.as_mut()
        && let Some(ProjectileChoice::Reference(projectile)) = ranged.projectile.clone()
    {
        match raw_def(&defs, &projectile).filter(|_| defs.files.get(&projectile) == Some(file)) {
            Some(own) => match projectile_spec_from_def(own, &projectile, &projectile, ValueSource::Typed)
            {
                Some(mut spec_of_projectile) => {
                    // the definition is edited where it stands, so it is not a copy of another one
                    spec_of_projectile.copied_from = None;
                    ranged.projectile = Some(ProjectileChoice::Inline(spec_of_projectile));
                    notes.push(format!(
                        "the projectile {projectile} is defined next to the weapon, so its damage and \
                         armor penetration are edited here and written back to it"
                    ));
                }
                None => notes.push(format!(
                    "the projectile {projectile} has no parent base, so it stays as it is and its \
                     damage is not written"
                )),
            },
            None if defs.files.contains_key(&projectile) => notes.push(format!(
                "the projectile {projectile} is defined in another file of the mod; its damage and armor \
                 penetration are not written from here"
            )),
            None => notes.push(format!(
                "the projectile {projectile} comes from the game or another mod; its damage and armor \
                 penetration are read from it and not written"
            )),
        }
    }
    // the role is an input of the designer that no definition holds: the nearest reference weapon by the
    // numbers of the weapon tells it, so the fit meter has a class to compare with
    if spec.role.is_none()
        && let Some(engine) = ctx.engine()
        && let Ok(kind) = pool_kind(spec.kind)
        && let Ok(pool) = engine.pool(kind)
        && let Some(item) = nearest(pool, &spec)
    {
        notes.push(format!(
            "the role {} comes from the nearest reference weapon, {} ({}); change it when it does not fit",
            item.role, item.label, item.id
        ));
        spec.role = Some(item.role.clone());
    }
    spec.accepted_missing = missing_in_source(&spec);
    let mut draft = Draft::new(spec);
    draft.calibration = CalibrationMode::Simple;
    draft.edits_file = Some(file.clone());
    let saved = draft_save(
        ctx,
        DesignerDraftSaveRequest {
            project_id: req.project_id.clone(),
            id: known.map(|k| k.id),
            draft: draft_to_dto(&draft)?,
        },
    )?;
    let entry = super::drafts::draft_load(ctx, &req.project_id, &saved.id)?;
    Ok(DesignerOpenWeaponResponse {
        entry,
        file: file.clone(),
        reused: false,
        notes,
    })
}
