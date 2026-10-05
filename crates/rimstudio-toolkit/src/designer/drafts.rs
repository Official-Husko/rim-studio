//! Drafts: the designer's working state, stored in the `designer-drafts` collection of the JSON document
//! store under the app data root (D-083, IT-015).
//!
//! A draft is the design spec plus the dialogue state around it (calibration mode, quiz answers, anchors,
//! the optional Combat Extended block). It never lives inside the user's mod folder. Documents are keyed by
//! the project id and a draft id: the store id is `<projectId>.<draftId>`, so one project's drafts are one
//! prefix of the index. Saving and reading are exact: the draft that comes back is equal to the draft that
//! went in, quiz answers and the CE toggle included, for every number that survives a JSON round trip.
//!
//! The functions take the context and a request DTO and return a response DTO, like every designer command.

use std::sync::atomic::{AtomicU64, Ordering};

use rimstudio_design::model::{Draft, migrate_value};
use rimstudio_io::error::MigrateError;
use rimstudio_io::migrate::MigrationFn;
use rimstudio_io::schema::Versioned;
use rimstudio_ipc_types::designer::{
    DesignerDraftDeleteRequest, DesignerDraftDeleteResponse, DesignerDraftListRequest,
    DesignerDraftListResponse, DesignerDraftSaveRequest, DesignerDraftSaveResponse, DraftEntryDto,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ctx::Ctx;
use super::dto::{draft_from_dto, draft_to_dto, kind_to_dto};
use crate::error::{ToolkitError, ToolkitResult};

/// The longest project or draft id part, in characters.
const MAX_PART: usize = 60;

/// One stored draft with the keys it is stored under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRecord {
    /// The project the draft belongs to.
    pub project_id: String,
    /// The draft id, unique within the project.
    pub id: String,
    /// Unix time in milliseconds of the last save.
    pub updated_at_ms: u64,
    /// The draft itself.
    pub draft: Draft,
}

impl Versioned for DraftRecord {
    const KIND: &'static str = Draft::KIND;
    const VERSION: u32 = Draft::VERSION;

    fn migrations() -> Vec<(u32, MigrationFn)> {
        vec![(1, migrate_record_v1)]
    }
}

/// The step from version 1 to version 2 of a stored draft record: the draft inside it gets the new schema
/// version (the shape only gained optional fields).
fn migrate_record_v1(mut value: Value) -> Result<Value, MigrateError> {
    if let Some(map) = value.as_object_mut()
        && let Some(draft) = map.remove("draft")
    {
        map.insert("draft".to_owned(), migrate_value(draft));
    }
    Ok(value)
}

impl DraftRecord {
    /// The store id of this record.
    #[must_use]
    pub fn doc_id(&self) -> String {
        doc_id(&self.project_id, &self.id)
    }

    /// The small listing summary kept in the collection index.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({
            "projectId": self.project_id,
            "id": self.id,
            "defName": self.draft.spec.identity.def_name,
            "label": self.draft.spec.identity.label,
            "kind": self.draft.kind.as_str(),
            "updatedAtMs": self.updated_at_ms,
        })
    }

    fn entry(&self) -> ToolkitResult<DraftEntryDto> {
        Ok(DraftEntryDto {
            id: self.id.clone(),
            def_name: self.draft.spec.identity.def_name.clone(),
            label: self.draft.spec.identity.label.clone(),
            kind: kind_to_dto(self.draft.kind)?,
            updated_at_ms: self.updated_at_ms,
            draft: draft_to_dto(&self.draft)?,
        })
    }
}

fn doc_id(project_id: &str, id: &str) -> String {
    format!("{project_id}.{id}")
}

/// Checks one part of a store id: lowercase letters, digits, hyphen and underscore.
pub(super) fn check_part(what: &str, part: &str) -> ToolkitResult<()> {
    let ok = !part.is_empty()
        && part.chars().count() <= MAX_PART
        && part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(ToolkitError::invalid_draft(format!(
            "the {what} must be 1 to {MAX_PART} characters of a-z, 0-9, hyphen and underscore"
        )))
    }
}

static MINT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Mints a draft id that is not used yet in the project: `d-` and ten hex characters of a hash of the
/// project, the clock and a counter.
fn mint_id(ctx: &Ctx, project_id: &str) -> ToolkitResult<String> {
    for _ in 0..16 {
        let n = MINT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let seed = format!("{project_id}|{}|{n}", ctx.now_ms());
        let hash = blake3::hash(seed.as_bytes()).to_hex();
        let short: String = hash.as_str().chars().take(10).collect();
        let id = format!("d-{short}");
        if !ctx.drafts().exists(&doc_id(project_id, &id))? {
            return Ok(id);
        }
    }
    Err(ToolkitError::internal("could not mint an unused draft id"))
}

/// Stores a draft (`designer_draft_save`). An absent id creates a new draft.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad id or an inconsistent draft, [`ToolkitError::DraftNewerSchema`]
/// for a draft written by a newer build or one that would overwrite a stored draft of a newer build,
/// [`ToolkitError::Store`] when the write fails.
pub fn draft_save(
    ctx: &Ctx,
    req: DesignerDraftSaveRequest,
) -> ToolkitResult<DesignerDraftSaveResponse> {
    check_part("project id", &req.project_id)?;
    let draft = draft_from_dto(&req.draft)?;
    let id = match req.id {
        Some(id) => {
            check_part("draft id", &id)?;
            if let Some(existing) = ctx.drafts().get(&doc_id(&req.project_id, &id))?
                && (existing.read_only || existing.value.draft.is_newer_than_supported())
            {
                return Err(ToolkitError::DraftNewerSchema {
                    found: existing.value.draft.schema_version,
                    supported: Draft::VERSION,
                });
            }
            id
        }
        None => mint_id(ctx, &req.project_id)?,
    };
    let saved_at_ms = ctx.now_ms();
    let record = DraftRecord {
        project_id: req.project_id,
        id: id.clone(),
        updated_at_ms: saved_at_ms,
        draft,
    };
    ctx.drafts().put(&record.doc_id(), &record)?;
    Ok(DesignerDraftSaveResponse { id, saved_at_ms })
}

/// Reads one draft.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad id, [`ToolkitError::DraftNotFound`] when the draft does not
/// exist, [`ToolkitError::Store`] when the document cannot be read.
pub fn draft_load(ctx: &Ctx, project_id: &str, id: &str) -> ToolkitResult<DraftEntryDto> {
    check_part("project id", project_id)?;
    check_part("draft id", id)?;
    match ctx.drafts().get(&doc_id(project_id, id))? {
        Some(loaded) => loaded.value.entry(),
        None => Err(ToolkitError::DraftNotFound {
            project_id: project_id.to_owned(),
            id: id.to_owned(),
        }),
    }
}

/// Lists the drafts of a project, newest first and then by id (`designer_draft_list`).
///
/// A document that cannot be read is skipped here; the store quarantines it and keeps the file.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad project id, [`ToolkitError::Store`] when the index cannot be
/// read.
pub fn draft_list(
    ctx: &Ctx,
    req: DesignerDraftListRequest,
) -> ToolkitResult<DesignerDraftListResponse> {
    check_part("project id", &req.project_id)?;
    let prefix = format!("{}.", req.project_id);
    let mut drafts = Vec::new();
    for entry in ctx.drafts().list_summaries()? {
        if !entry.id.starts_with(&prefix) {
            continue;
        }
        match ctx.drafts().get(&entry.id) {
            Ok(Some(loaded)) => drafts.push(loaded.value.entry()?),
            Ok(None) => {}
            Err(e) => tracing::warn!(id = %entry.id, error = %e, "skipping an unreadable draft"),
        }
    }
    drafts.sort_by(|a, b| {
        b.updated_at_ms
            .cmp(&a.updated_at_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(DesignerDraftListResponse { drafts })
}

/// Deletes a draft (`designer_draft_delete`). Deleting an unknown id answers `deleted: false`.
///
/// # Errors
///
/// [`ToolkitError::InvalidDraft`] for a bad id, [`ToolkitError::Store`] when the delete fails.
pub fn draft_delete(
    ctx: &Ctx,
    req: DesignerDraftDeleteRequest,
) -> ToolkitResult<DesignerDraftDeleteResponse> {
    check_part("project id", &req.project_id)?;
    check_part("draft id", &req.id)?;
    let deleted = ctx.drafts().delete(&doc_id(&req.project_id, &req.id))?;
    Ok(DesignerDraftDeleteResponse { deleted })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::model::DesignSpec;
    use rstest::rstest;

    #[rstest]
    #[case("p-0001", true)]
    #[case("p_test-9", true)]
    #[case("", false)]
    #[case("Upper", false)]
    #[case("has.dot", false)]
    #[case("with space", false)]
    #[case("slash/x", false)]
    fn id_parts_are_checked(#[case] part: &str, #[case] ok: bool) {
        assert_eq!(check_part("id", part).is_ok(), ok, "{part:?}");
    }

    #[test]
    fn an_overlong_id_part_is_refused() {
        assert!(check_part("id", &"a".repeat(MAX_PART)).is_ok());
        assert!(check_part("id", &"a".repeat(MAX_PART + 1)).is_err());
    }

    #[test]
    fn the_store_id_joins_project_and_draft() {
        let record = DraftRecord {
            project_id: "p-one".into(),
            id: "d-abc".into(),
            updated_at_ms: 7,
            draft: Draft::new(DesignSpec::new_ranged("RS_X", "x")),
        };
        assert_eq!(record.doc_id(), "p-one.d-abc");
        rimstudio_io::collection::validate_id(&record.doc_id()).unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn the_summary_names_the_listing_fields() {
        let record = DraftRecord {
            project_id: "p-one".into(),
            id: "d-abc".into(),
            updated_at_ms: 7,
            draft: Draft::new(DesignSpec::new_melee("RS_Blade", "blade")),
        };
        let s = record.summary();
        assert_eq!(s["projectId"], "p-one");
        assert_eq!(s["defName"], "RS_Blade");
        assert_eq!(s["kind"], "melee");
        assert_eq!(s["updatedAtMs"], 7);
    }
}
