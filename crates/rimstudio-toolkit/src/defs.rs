//! The def explorer (`tool-defs`, minimal): search and resolve over a workspace session, for the CLI.
//!
//! Both functions read the session only. Search uses the always on def index (abstract nodes included, no
//! patches applied); resolve returns one definition after inheritance and patches as a JSON node tree with
//! the patch events that touched it.

use rimstudio_ipc_types::defs::{
    DefPage, DefRowDto, DefSearchRequest, DefsGetResolvedRequest, PatchEventDto, PatchOutcomeDto,
    ResolvedDefDto,
};
use rimstudio_workspace::WorkspaceError;
use rimstudio_workspace::defindex::{DefHit, Page, Query};
use rimstudio_workspace::session::WorkspaceSession;
use rimstudio_workspace::snapshot::DefRef;

use crate::error::{ToolkitError, ToolkitResult};

/// The largest page a search returns.
pub const MAX_PAGE: u32 = 500;

fn row_of(session: &WorkspaceSession, hit: &DefHit) -> DefRowDto {
    let root = session
        .reference()
        .packs
        .get(hit.pack.index)
        .map(|p| p.root.clone());
    let file = root
        .and_then(|r| {
            hit.path
                .strip_prefix(&r)
                .ok()
                .map(|p| p.as_str().replace('\\', "/"))
        })
        .unwrap_or_else(|| hit.relative.clone());
    DefRowDto {
        def_type: hit.def_type.clone(),
        def_name: hit.def_name.clone(),
        label: hit.label.clone(),
        is_abstract: hit.is_abstract,
        mod_id: hit.pack.package_id.clone(),
        file,
        parent: hit.parent.clone(),
    }
}

fn matches(hit: &DefHit, req: &DefSearchRequest) -> bool {
    (req.def_types.is_empty()
        || req
            .def_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&hit.def_type)))
        && (req.mod_ids.is_empty()
            || req
                .mod_ids
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&hit.pack.package_id)))
        && !(req.hide_abstract && hit.is_abstract)
}

/// `defs_search`: a page of the definitions that match the text, def types and mods of the request.
///
/// The request's `session_id` is not read here: the caller resolves it to the session it passes in.
///
/// # Errors
///
/// None at present; the signature keeps room for a failing index.
pub fn search(session: &WorkspaceSession, req: &DefSearchRequest) -> ToolkitResult<DefPage> {
    let limit = req.limit.clamp(1, MAX_PAGE) as usize;
    let offset = req.offset as usize;
    let mut query = Query::text(req.query.clone());
    query.include_abstract = !req.hide_abstract;
    let simple = req.def_types.len() <= 1 && req.mod_ids.len() <= 1;
    if simple {
        query.def_type = req.def_types.first().cloned();
        query.pack = req.mod_ids.first().cloned();
        let result = session.search(&query, Page::new(offset, limit));
        return Ok(DefPage {
            query_id: req.query_id.clone(),
            total: result.total as u64,
            offset: req.offset,
            items: result.hits.iter().map(|h| row_of(session, h)).collect(),
        });
    }
    let all = session.search(&query, Page::new(0, usize::MAX / 2));
    let filtered: Vec<&DefHit> = all.hits.iter().filter(|h| matches(h, req)).collect();
    Ok(DefPage {
        query_id: req.query_id.clone(),
        total: filtered.len() as u64,
        offset: req.offset,
        items: filtered
            .iter()
            .skip(offset)
            .take(limit)
            .map(|h| row_of(session, h))
            .collect(),
    })
}

/// `defs_get_resolved`: one definition after inheritance and patches, as JSON, with its patch events.
///
/// # Errors
///
/// [`ToolkitError::Workspace`] with `DefNotFound` when the snapshot has no such definition (an abstract
/// node is not a definition), [`ToolkitError::Internal`] when the tree cannot be encoded.
pub fn resolve(
    session: &WorkspaceSession,
    req: &DefsGetResolvedRequest,
) -> ToolkitResult<ResolvedDefDto> {
    let view = session
        .resolve_def(&DefRef::new(req.def_type.clone(), req.def_name.clone()))
        .ok_or_else(|| {
            ToolkitError::Workspace(WorkspaceError::DefNotFound {
                def_type: req.def_type.clone(),
                def_name: req.def_name.clone(),
            })
        })?;
    let tree = serde_json::to_value(&view.node)
        .map_err(|e| ToolkitError::internal(format!("the def tree cannot be encoded: {e}")))?;
    let provenance = &view.provenance;
    let file = provenance
        .file
        .as_ref()
        .map(|f| {
            session
                .reference()
                .packs
                .get(f.pack)
                .and_then(|p| f.path.strip_prefix(&p.root).ok())
                .map_or_else(|| f.relative.clone(), |p| p.as_str().replace('\\', "/"))
        })
        .unwrap_or_default();
    Ok(ResolvedDefDto {
        def_type: provenance.type_name.clone(),
        def_name: provenance.def_name.clone(),
        mod_id: provenance
            .pack
            .as_ref()
            .map(|p| p.package_id.clone())
            .unwrap_or_default(),
        file,
        tree,
        patch_events: provenance
            .patched_by
            .iter()
            .map(|p| PatchEventDto {
                mod_id: p
                    .pack
                    .as_ref()
                    .map(|r| r.package_id.clone())
                    .unwrap_or_default(),
                file: p
                    .file
                    .as_ref()
                    .map(|f| f.relative.clone())
                    .unwrap_or_default(),
                operation: p.class.clone(),
                outcome: PatchOutcomeDto::Applied,
                message: Some(p.description.clone()).filter(|d| !d.is_empty()),
            })
            .collect(),
    })
}
