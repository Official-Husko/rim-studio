//! The source of a clone as its files write it, and the weapon's own projectile.
//!
//! A clone must carry what its source defines itself and nothing its parent supplies, so the designer reads
//! the def node of the source from its file (before inheritance) next to the resolved def. A def that a
//! patch changed has no faithful file form; it is read the plain way and a note says so.
//!
//! The same module gives a gun a projectile of its own (`designer_projectile_own`): the projectile def the
//! shooting verb names is copied into an inline projectile spec with a new name derived from the weapon,
//! so that editing the damage changes the written file. Switching it off points the weapon back at the
//! projectile it was copied from.

use rimstudio_core::tree::Node;
use rimstudio_defs::DefRecord;
use rimstudio_design::model::{
    DesignSpec, ItemKind, ProjectileChoice, ProjectileSpec, ValueSource,
};
use rimstudio_design::reader::{
    OwnSource, SpecReading, projectile_spec_from_def, shooting_verb, spec_from_def,
    spec_from_def_own,
};
use rimstudio_design::validation::DefLookup;
use rimstudio_design::validation::RefKind;
use rimstudio_ipc_types::designer::{DesignerProjectileOwnRequest, DesignerProjectileOwnResponse};
use rimstudio_workspace::snapshot::{DefRef, Snapshot};
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_top_level;

use super::ctx::{Ctx, Engine};
use super::dto::{draft_from_dto, draft_to_dto};
use crate::error::{ToolkitError, ToolkitResult};

/// The def type every weapon and projectile belongs to.
const THING_TYPE: &str = "ThingDef";

/// The def node as its file writes it. `None` when the def was changed by a patch (the file does not show
/// the final state), or its file cannot be read or does not hold the def.
pub(super) fn own_node(snapshot: &Snapshot, record: &DefRecord) -> Option<Node> {
    if !record.patched_by.is_empty() {
        return None;
    }
    let file = snapshot.file(record.origin?.file)?;
    let bytes = std::fs::read(file.path.as_std_path()).ok()?;
    let parsed = parse_top_level(&bytes, ParseMode::Tolerant, "Defs").ok()?;
    parsed
        .nodes
        .into_iter()
        .find(|n| n.tag == THING_TYPE && n.child_text("defName") == Some(record.def_name.as_str()))
}

/// Reads a weapon of the loaded defs the way a clone copies it: with its own node when the file shows the
/// def as it is, otherwise the plain way (with a note).
pub(super) fn read_with_own(
    engine: &Engine,
    record: &DefRecord,
    source: ValueSource,
) -> rimstudio_design::error::DesignResult<SpecReading> {
    let snapshot = engine.snapshot();
    let dbs = engine.databases();
    let opts = engine.reader_options();
    match own_node(snapshot, record) {
        Some(def) => {
            let projectile = shooting_verb(&def)
                .and_then(|v| v.child_text("defaultProjectile"))
                .map(str::trim)
                .and_then(|name| snapshot.record(&DefRef::new(THING_TYPE, name)))
                .and_then(|r| own_node(snapshot, r));
            spec_from_def_own(
                record,
                dbs,
                opts,
                source,
                &OwnSource {
                    def: &def,
                    projectile: projectile.as_ref(),
                },
            )
        }
        None => {
            let mut reading = spec_from_def(record, dbs, opts, source)?;
            reading.notes.push(
                "the definition is changed by a patch or its file cannot be read, so the copy holds \
                 the values the designer models and no raw fields"
                    .to_owned(),
            );
            Ok(reading)
        }
    }
}

/// The name of the own projectile of a weapon: `<prefix>_Bullet_<rest>` for a weapon named
/// `<prefix>_<rest>`, `Bullet_<name>` otherwise (the layout example is `OH_G41m` and `OH_Bullet_G41m`).
#[must_use]
pub fn derived_projectile_name(def_name: &str, mod_prefix: &str) -> String {
    let wanted = format!("{mod_prefix}_");
    match def_name.strip_prefix(&wanted) {
        Some(rest) if !mod_prefix.is_empty() && !rest.is_empty() => {
            format!("{mod_prefix}_Bullet_{rest}")
        }
        _ => format!("Bullet_{def_name}"),
    }
}

/// The first name derived from the weapon that no loaded def uses.
pub(super) fn free_name(engine: &Engine, def_name: &str, mod_prefix: &str) -> String {
    let base = derived_projectile_name(def_name, mod_prefix);
    let lookup = engine.lookup();
    let taken = |name: &str| lookup.contains(RefKind::Thing, name) == Some(true);
    if !taken(&base) {
        return base;
    }
    (2..1000)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !taken(name))
        .unwrap_or(base)
}

/// Gives the gun in `spec` a projectile of its own, copied from the projectile it points at.
///
/// Returns the note to show, or `Err` with the plain reason when it cannot be done: the spec is no gun, it
/// has no projectile reference, the projectile def is not loaded, or it has no parent.
pub(super) fn give_own_projectile(
    engine: &Engine,
    spec: &mut DesignSpec,
    new_name: &str,
) -> Result<String, String> {
    if spec.kind != ItemKind::Ranged {
        return Err("only a ranged weapon has a projectile".to_owned());
    }
    let Some(ranged) = spec.ranged.as_mut() else {
        return Err("the weapon has no ranged inputs".to_owned());
    };
    let name = match &ranged.projectile {
        Some(ProjectileChoice::Reference(name)) => name.clone(),
        Some(ProjectileChoice::Inline(_)) => {
            return Err("the weapon already has a projectile of its own".to_owned());
        }
        None => return Err("the weapon has no projectile to copy".to_owned()),
    };
    let snapshot = engine.snapshot();
    let record = snapshot
        .record(&DefRef::new(THING_TYPE, &name))
        .ok_or_else(|| format!("the projectile {name} is not among the loaded defs"))?;
    let own = own_node(snapshot, record).ok_or_else(|| {
        format!("the projectile {name} is changed by a patch or its file cannot be read")
    })?;
    let copy: ProjectileSpec = projectile_spec_from_def(&own, new_name, &name, ValueSource::Anchor)
        .ok_or_else(|| format!("the projectile {name} has no parent base to copy"))?;
    ranged.projectile = Some(ProjectileChoice::Inline(copy));
    Ok(format!(
        "the weapon has its own projectile {new_name}, copied from {name}; its damage, speed and graphic are \
         written to your mod and the damage you set here is the one the file holds"
    ))
}

/// Switches the own projectile of a draft on or off (`designer_projectile_own`).
///
/// On: the projectile the weapon points at is copied into a projectile of its own (see
/// [`give_own_projectile`]); a weapon that has one already is returned as it is. Off: a projectile that was
/// copied from a shared one is replaced by a reference to the original again; the damage in the draft then
/// no longer changes the written files. The draft is not stored; the caller saves it.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for an unreadable draft or when the switch cannot be made (the reason is
/// plain), [`ToolkitError::ReferenceUnavailable`] without a game install.
pub fn projectile_own(
    ctx: &Ctx,
    req: DesignerProjectileOwnRequest,
) -> ToolkitResult<DesignerProjectileOwnResponse> {
    let mut draft = draft_from_dto(&req.draft)?;
    let engine = ctx.require_engine()?;
    let mut notes = Vec::new();
    let current = draft
        .spec
        .ranged
        .as_ref()
        .and_then(|r| r.projectile.clone());
    if req.own {
        if matches!(current, Some(ProjectileChoice::Inline(_))) {
            notes.push("the weapon already has a projectile of its own".to_owned());
        } else {
            let name = free_name(
                &engine,
                &draft.spec.identity.def_name,
                &draft.spec.identity.mod_prefix,
            );
            let note = give_own_projectile(&engine, &mut draft.spec, &name)
                .map_err(ToolkitError::invalid_draft)?;
            notes.push(note);
        }
    } else if let Some(ProjectileChoice::Inline(p)) = current {
        let Some(original) = p.copied_from.clone() else {
            return Err(ToolkitError::invalid_draft(
                "the projectile was not copied from another one, so there is nothing to point back at",
            ));
        };
        if let Some(r) = draft.spec.ranged.as_mut() {
            r.projectile = Some(ProjectileChoice::Reference(original.clone()));
        }
        notes.push(format!(
            "the weapon points at the shared projectile {original} again; the damage here no longer \
             changes the written files"
        ));
    }
    Ok(DesignerProjectileOwnResponse {
        draft: draft_to_dto(&draft)?,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("OH_G41m", "OH", "OH_Bullet_G41m")]
    #[case("RS_Rifle", "RS", "RS_Bullet_Rifle")]
    #[case("Rifle", "RS", "Bullet_Rifle")]
    #[case("Rifle", "", "Bullet_Rifle")]
    #[case("RS_", "RS", "Bullet_RS_")]
    #[case("RSX_Rifle", "RS", "Bullet_RSX_Rifle")]
    fn the_projectile_name_follows_the_weapon_name(
        #[case] weapon: &str,
        #[case] prefix: &str,
        #[case] want: &str,
    ) {
        assert_eq!(derived_projectile_name(weapon, prefix), want);
    }
}
