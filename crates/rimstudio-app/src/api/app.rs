//! The infrastructure rows: `app_ping`, `app_get_info` and `app_list_tools`.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::tools::AppListToolsResponse;

use crate::context::AppContext;
use crate::dto::{
    AppGetInfoRequest, AppGetInfoResponse, AppListToolsRequest, AppPingRequest, AppPingResponse,
};
use crate::registry;

/// `app_ping`: echoes the text and the clock, for the performance lab and the tests.
///
/// # Errors
/// Never fails.
pub fn app_ping(ctx: &AppContext, req: AppPingRequest) -> Result<AppPingResponse, ApiError> {
    Ok(AppPingResponse {
        echo: req.echo,
        at_ms: ctx.now_ms(),
    })
}

/// `app_get_info`: version, operating system, portable flag and the contract hash.
///
/// # Errors
/// Never fails.
pub fn app_get_info(
    ctx: &AppContext,
    _req: AppGetInfoRequest,
) -> Result<AppGetInfoResponse, ApiError> {
    Ok(AppGetInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        os: format!("{:?}", ctx.platform.os).to_lowercase(),
        portable: ctx.roots.portable,
        contract_hash: registry::contract_hash(),
        session_id: ctx.boot.session_id.clone(),
        command_count: u32::try_from(registry::ROUTES.len()).unwrap_or(u32::MAX),
        previous_run_ended_abnormally: ctx.boot.previous_run_ended_abnormally,
    })
}

/// `app_list_tools`: the tool table with the capabilities of this machine resolved.
///
/// # Errors
/// Never fails.
pub fn app_list_tools(
    ctx: &AppContext,
    _req: AppListToolsRequest,
) -> Result<AppListToolsResponse, ApiError> {
    Ok(crate::tools::list(ctx))
}
