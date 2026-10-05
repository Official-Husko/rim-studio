//! `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove` and
//! `sources_probe_folder`.

use rimstudio_core::settings::FolderLayout;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::library::{
    SourceDto, SourcesAddFolderRequest, SourcesListRequest, SourcesListResponse,
    SourcesProbeFolderRequest, SourcesProbeFolderResponse, SourcesRemoveRequest,
    SourcesRemoveResponse, SourcesUpdateRequest,
};
use rimstudio_manager::ManagerError;
use rimstudio_manager::sources::{self, SourceRow};

use crate::context::{AppContext, AppEvent};
use crate::convert;

fn scanned_count(ctx: &AppContext, row: &SourceRow) -> Option<u32> {
    ctx.library.current().and_then(|s| s.mods_in(&row.id))
}

fn dto_of(ctx: &AppContext, row: &SourceRow) -> SourceDto {
    convert::source_dto(row, scanned_count(ctx, row))
}

/// `sources_list`: the install, workshop and custom folders in scan order, with the mod counts of the
/// last scan.
///
/// # Errors
/// The mapped manager error.
pub fn sources_list(
    ctx: &AppContext,
    _req: SourcesListRequest,
) -> Result<SourcesListResponse, ApiError> {
    let list = ctx
        .with_manager(|m| sources::list(m, sources::SourcesListRequest::default()))
        .map_err(|e| ctx.manager_error(&e))?;
    Ok(SourcesListResponse {
        sources: list.rows.iter().map(|r| dto_of(ctx, r)).collect(),
    })
}

/// `sources_add_folder`: registers a custom mod folder. Overlaps and unreachable folders are refused.
///
/// # Errors
/// `sources.overlap`, `sources.invalid-folder` and the other mapped manager errors.
pub fn sources_add_folder(
    ctx: &AppContext,
    req: SourcesAddFolderRequest,
) -> Result<SourceDto, ApiError> {
    let added = ctx
        .with_manager(|m| {
            sources::add_folder(
                m,
                sources::SourcesAddFolderRequest {
                    path: req.path,
                    label: req.label,
                    layout: req.layout.map(FolderLayout::from),
                    scan_depth: req.scan_depth,
                    read_only: false,
                },
            )
        })
        .map_err(|e| ctx.manager_error(&e))?;
    ctx.workspace.bump();
    ctx.events.emit(&AppEvent::SourcesChanged {
        source_ids: vec![added.row.id.to_string()],
    });
    Ok(dto_of(ctx, &added.row))
}

/// `sources_update`: changes a label, the enabled flag, the order, the layout or the scan depth.
///
/// # Errors
/// `sources.not-found` and the other mapped manager errors.
pub fn sources_update(ctx: &AppContext, req: SourcesUpdateRequest) -> Result<SourceDto, ApiError> {
    let id = req.id.clone();
    let row = ctx
        .with_manager(|m| {
            sources::update(
                m,
                sources::SourcesUpdateRequest {
                    id: req.id,
                    label: req.label,
                    enabled: req.enabled,
                    order: req.order.map(|o| usize::try_from(o).unwrap_or(usize::MAX)),
                    layout: req.layout.map(FolderLayout::from),
                    scan_depth: req.scan_depth,
                    read_only: None,
                },
            )
        })
        .map_err(|e| ctx.manager_error(&e))?;
    ctx.workspace.bump();
    ctx.events.emit(&AppEvent::SourcesChanged {
        source_ids: vec![id],
    });
    Ok(dto_of(ctx, &row))
}

/// `sources_remove`: forgets a custom folder; the folder itself is never touched. An unknown id
/// answers `removed: false`.
///
/// # Errors
/// The mapped manager errors other than an unknown id.
pub fn sources_remove(
    ctx: &AppContext,
    req: SourcesRemoveRequest,
) -> Result<SourcesRemoveResponse, ApiError> {
    let id = req.id.clone();
    let removed =
        ctx.with_manager(|m| sources::remove(m, sources::SourcesRemoveRequest { id: req.id }));
    match removed {
        Ok(_) => {
            ctx.workspace.bump();
            ctx.events.emit(&AppEvent::SourcesChanged {
                source_ids: vec![id],
            });
            Ok(SourcesRemoveResponse { removed: true })
        }
        Err(ManagerError::SourceNotFound { .. }) => Ok(SourcesRemoveResponse { removed: false }),
        Err(e) => Err(ctx.manager_error(&e)),
    }
}

/// `sources_probe_folder`: what a folder looks like before it is added.
///
/// # Errors
/// The mapped manager error.
pub fn sources_probe_folder(
    ctx: &AppContext,
    req: SourcesProbeFolderRequest,
) -> Result<SourcesProbeFolderResponse, ApiError> {
    let probe = ctx
        .with_manager(|m| {
            sources::probe_folder(m, sources::SourcesProbeFolderRequest { path: req.path })
        })
        .map_err(|e| ctx.manager_error(&e))?;
    Ok(convert::probe_dto(&probe))
}
