//! The `designer_archetype_*` commands: the catalogue of weapon archetypes, the proposal of every number of a
//! weapon from an archetype, and the application of a proposal to a draft.

use rimstudio_ipc_types::designer::{
    ArchetypeCatalogDto, ArchetypeProposalDto, DesignerArchetypeApplyRequest,
    DesignerArchetypeApplyResponse, DesignerArchetypeCatalogRequest,
    DesignerArchetypeProposeRequest,
};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_toolkit::designer as toolkit;

use crate::context::AppContext;

/// `designer_archetype_catalog` (query): the families and archetypes with the descriptors that apply to each,
/// the choices of every descriptor, the tiers of the install and, on request, the Combat Extended calibres.
/// Works without a game install; without one nothing can be proposed (`proposalsAvailable` is false).
///
/// # Errors
/// Mapped toolkit errors.
pub fn designer_archetype_catalog(
    ctx: &AppContext,
    req: DesignerArchetypeCatalogRequest,
) -> Result<ArchetypeCatalogDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::archetype_catalog(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_archetype_propose` (query): every number of the weapon the archetype describes, each with a plain
/// reason, and the fit meter's verdict. Read only; typed values of the optional draft are marked locked.
///
/// # Errors
/// `designer.invalid-draft` (with the plain reason) for an unknown archetype or a descriptor it does not offer,
/// `designer.reference-unavailable` without reference weapons of the kind.
pub fn designer_archetype_propose(
    ctx: &AppContext,
    req: DesignerArchetypeProposeRequest,
) -> Result<ArchetypeProposalDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::archetype_propose(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_archetype_apply` (action): fills the empty and derived fields of a draft from the proposal and
/// records the choice in the draft. Typed values are kept (IT-003). Returns the new draft state; it does not
/// save the draft.
///
/// # Errors
/// As `designer_archetype_propose`, and `designer.invalid-draft` for a draft of the other kind.
pub fn designer_archetype_apply(
    ctx: &AppContext,
    req: DesignerArchetypeApplyRequest,
) -> Result<DesignerArchetypeApplyResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::archetype_apply(&d, req).map_err(|e| ctx.toolkit_error(&e))
}
