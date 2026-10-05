//! The tool table of the application and the capabilities of this machine.
//!
//! [`TOOLS`] is the static table of `rimstudio-toolkit` (one row per tool module compiled in); the
//! manager contributes no tool of its own in 0.1.0. [`list`] resolves the capabilities every tool
//! needs against what the machine has: a game install, a Workshop folder, Combat Extended among the
//! scanned mods, an open project, the Steam client. The webview shows a tool disabled with the missing
//! capability named, so it never probes anything itself.

use rimstudio_core::paths::CE_PACKAGE_ID;
use rimstudio_ipc_types::tools::{AppListToolsResponse, Capability};
use rimstudio_manager::detect::{self, DetectGetReportRequest};

use crate::context::AppContext;

pub use rimstudio_toolkit::registry::{TOOLS, ToolDef};

/// The capabilities that are present now.
///
/// Combat Extended counts as present when the last library scan found it (or the toolkit sessions
/// hold it); before the first scan it is absent.
#[must_use]
pub fn present_capabilities(ctx: &AppContext) -> Vec<Capability> {
    let mut present = Vec::new();
    let workspace = ctx.workspace_settings();
    let report = ctx
        .with_manager(|m| detect::get_report(m, DetectGetReportRequest::default()))
        .ok()
        .and_then(|r| r.report);
    if let Some(report) = &report {
        if let Some(install) = detect::select_install(report, &workspace) {
            present.push(Capability::GameInstall);
            if !install.workshop.is_empty() || !workspace.paths.extra_workshop_dirs.is_empty() {
                present.push(Capability::WorkshopFolder);
            }
        }
        if !report.steam_roots.is_empty() {
            present.push(Capability::SteamClient);
        }
    }
    let ce_in_library = ctx
        .library
        .current()
        .is_some_and(|s| s.index.core().first_by_package_id(CE_PACKAGE_ID).is_some());
    if ce_in_library || ctx.workspace.has_combat_extended() {
        present.push(Capability::CombatExtended);
    }
    if ctx.workspace.opened_count() > 0 {
        present.push(Capability::OpenProject);
    }
    present.sort();
    present.dedup();
    present
}

/// The tool table with availability resolved for this machine (`app_list_tools`).
#[must_use]
pub fn list(ctx: &AppContext) -> AppListToolsResponse {
    AppListToolsResponse {
        tools: rimstudio_toolkit::registry::descriptors(&present_capabilities(ctx)),
    }
}
