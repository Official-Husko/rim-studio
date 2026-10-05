//! The single command registry (ADR 0003).
//!
//! Every command is declared once, as one row of [`for_each_command!`]:
//!
//! ```text
//! kind name (ctx, req: RequestType) -> ResponseType = path::to::handler;
//! ```
//!
//! `kind` is `query`, `action` or `job` (`stream` rows join with the mod list in M2). A query or action
//! handler is `fn(&AppContext, Req) -> Result<Resp, ApiError>`; a job handler is
//! `fn(&AppContext, Req, &JobCtx) -> Result<JobOutcome<Resp>, ApiError>` and `Resp` is the type of the
//! terminal result. Request and response types are written bare and resolve through [`wire`].
//!
//! The macro takes a callback and hands it the whole table, so a shell can generate its own wrappers
//! from the same rows (`rimstudio_app::registry::for_each_command!(my_callback)`). This crate's own
//! callback generates:
//!
//! - [`handlers`]: one typed function per command (`handlers::settings_get(ctx, req)`), the item the
//!   shell wrapper calls, so the shell never names a feature crate;
//! - [`ROUTES`]: the dispatch table, one [`Route`] per command with the JSON runner that decodes the
//!   request, calls the typed handler and encodes the response;
//! - [`describe`] and [`contract_hash`]: a JSON description of the table and its hash.
//!
//! Areas in use in 0.1.0: `app`, `job` (through `cancel_job` and `job_status`), `settings`, `detect`,
//! `sources`, `library`, `defs`, `project` and `designer`.

use rimstudio_ipc_types::error::ApiError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::context::AppContext;
use crate::error::AppError;
use crate::jobs::JobWork;

/// Every request and response type of the rows, by bare name.
pub mod wire {
    pub use crate::dto::{
        AppGetInfoRequest, AppGetInfoResponse, AppListToolsRequest, AppPingRequest,
        AppPingResponse, JobStatusRequest, JobStatusResponse, ProjectCreateRequest,
    };
    pub use rimstudio_ipc_types::defs::{
        DefPage, DefSearchRequest, DefsGetResolvedRequest, ResolvedDefDto,
    };
    pub use rimstudio_ipc_types::designer::{
        ApplyReportDto, CalibrateResultDto, CeAmmoCatalogDto, CeAmmoSuggestionDto, CeSuggestionDto,
        ConvertScanDto, DesignerApplyPlanRequest, DesignerAssetInfoRequest,
        DesignerAssetInfoResponse, DesignerCalibrateRequest, DesignerCeAmmoCatalogRequest,
        DesignerCeAmmoSuggestRequest, DesignerCeSuggestRequest, DesignerCloneDiffRequest,
        DesignerCloneDiffResponse, DesignerCloneRequest, DesignerCloneResponse,
        DesignerConvertScanRequest, DesignerDraftDeleteRequest, DesignerDraftDeleteResponse,
        DesignerDraftListRequest, DesignerDraftListResponse, DesignerDraftSaveRequest,
        DesignerDraftSaveResponse, DesignerExportPlanRequest, DesignerFitRequest,
        DesignerPreviewRequest, DesignerProjectileOwnRequest, DesignerProjectileOwnResponse,
        DesignerQuizAnswerRequest, DesignerQuizAnswerResponse, DesignerQuizBackRequest,
        DesignerQuizNextRequest, DesignerReferenceListRequest, DesignerStructureDefaultsRequest,
        DesignerStructureDefaultsResponse, FitReportDto, PreviewDto, QuizStepDto, ReferenceListDto,
        WritePlanDto,
    };
    pub use rimstudio_ipc_types::designer::{
        ArchetypeCatalogDto, ArchetypeProposalDto, DesignerArchetypeApplyRequest,
        DesignerArchetypeApplyResponse, DesignerArchetypeCatalogRequest,
        DesignerArchetypeProposeRequest,
    };
    pub use rimstudio_ipc_types::designer::{DesignerLintFilesRequest, DesignerLintFilesResult};
    pub use rimstudio_ipc_types::jobs::{CancelJobRequest, CancelJobResponse};
    pub use rimstudio_ipc_types::library::{
        DetectGetReportRequest, DetectGetReportResponse, DetectRunRequest,
        DetectSetOverrideRequest, DetectionReportDto, LibraryScanRequest, LibraryScanResult,
        SourceDto, SourcesAddFolderRequest, SourcesListRequest, SourcesListResponse,
        SourcesProbeFolderRequest, SourcesProbeFolderResponse, SourcesRemoveRequest,
        SourcesRemoveResponse, SourcesUpdateRequest,
    };
    pub use rimstudio_ipc_types::project::{
        ProjectCloseRequest, ProjectCloseResponse, ProjectFileDto, ProjectLayoutCheckDto,
        ProjectLayoutCheckRequest, ProjectOpenRequest, ProjectReadFileRequest,
        ProjectScaffoldMissingDto, ProjectScaffoldMissingRequest, ProjectSummaryDto,
        ProjectTreeDto, ProjectTreeRequest,
    };
    pub use rimstudio_ipc_types::project_fix::{
        ProjectLayoutFixApplyDto, ProjectLayoutFixApplyRequest, ProjectLayoutFixHistoryDto,
        ProjectLayoutFixHistoryRequest, ProjectLayoutFixPlanDto, ProjectLayoutFixPlanRequest,
        ProjectLayoutFixUndoDto, ProjectLayoutFixUndoRequest,
    };
    pub use rimstudio_ipc_types::project_link::{
        ProjectLinkCreateRequest, ProjectLinkRemoveRequest, ProjectLinkResultDto,
        ProjectLinkStatusDto, ProjectLinkStatusRequest,
    };
    pub use rimstudio_ipc_types::settings::{SettingsDto, SettingsGetRequest, SettingsUpdate};
    pub use rimstudio_ipc_types::tools::AppListToolsResponse;
}

use self::wire::*;

/// How a command is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandKind {
    /// Read only, answers inline.
    Query,
    /// Changes state, answers inline.
    Action,
    /// A long lived channel owned by the caller (not used in 0.1.0).
    Stream,
    /// Registered under a job id, runs on the job runner, ends with exactly one terminal event.
    Job,
}

impl CommandKind {
    /// The kind word as the catalog writes it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Action => "action",
            Self::Stream => "stream",
            Self::Job => "job",
        }
    }
}

/// The JSON runner of a route.
#[derive(Clone, Copy)]
pub enum Runner {
    /// A query or action: decode, call, encode.
    Sync(fn(&AppContext, Value) -> Result<Value, ApiError>),
    /// A job: decode now (a bad request fails before a job is registered), return the work to run.
    Job(fn(&AppContext, Value) -> Result<JobWork, ApiError>),
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sync(_) => f.write_str("Runner::Sync"),
            Self::Job(_) => f.write_str("Runner::Job"),
        }
    }
}

/// One row of the table at run time.
#[derive(Debug, Clone, Copy)]
pub struct Route {
    /// The wire name.
    pub name: &'static str,
    /// The kind.
    pub kind: CommandKind,
    /// The request type name as written in the row.
    pub request: &'static str,
    /// The response type name (the result type for a job) as written in the row.
    pub response: &'static str,
    /// The handler path as written in the row.
    pub handler: &'static str,
    /// The JSON runner.
    pub runner: Runner,
}

/// Decodes a request, naming the offending field in the error.
///
/// `null` reads as an empty object, so a command without parameters accepts both.
///
/// # Errors
/// `ipc.invalid-request` with `details.field` holding the path of the first field that does not fit.
pub fn decode_request<T: DeserializeOwned>(command: &str, value: Value) -> Result<T, ApiError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_path_to_error::deserialize(value).map_err(|e| {
        let path = e.path().to_string();
        let field = if path == "." { ".".to_owned() } else { path };
        AppError::invalid(command, &field, e.inner().to_string()).to_api()
    })
}

/// Encodes a response.
///
/// # Errors
/// `app.internal-error` when the value cannot be encoded.
pub fn encode_response<T: Serialize>(command: &str, value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|e| {
        AppError::internal(format!("the response of {command} cannot be encoded: {e}")).to_api()
    })
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rimstudio_kind {
    (query) => {
        $crate::registry::CommandKind::Query
    };
    (action) => {
        $crate::registry::CommandKind::Action
    };
    (stream) => {
        $crate::registry::CommandKind::Stream
    };
    (job) => {
        $crate::registry::CommandKind::Job
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rimstudio_handler {
    (query $name:ident, $req_ty:ty, $resp:ty, $path:path) => {
        #[doc = concat!("Typed handler of `", stringify!($name), "`.")]
        pub fn $name(
            ctx: &$crate::context::AppContext,
            req: $req_ty,
        ) -> Result<$resp, rimstudio_ipc_types::error::ApiError> {
            $path(ctx, req)
        }
    };
    (action $name:ident, $req_ty:ty, $resp:ty, $path:path) => {
        #[doc = concat!("Typed handler of `", stringify!($name), "`.")]
        pub fn $name(
            ctx: &$crate::context::AppContext,
            req: $req_ty,
        ) -> Result<$resp, rimstudio_ipc_types::error::ApiError> {
            $path(ctx, req)
        }
    };
    (job $name:ident, $req_ty:ty, $resp:ty, $path:path) => {
        #[doc = concat!("Typed job body of `", stringify!($name), "`.")]
        pub fn $name(
            ctx: &$crate::context::AppContext,
            req: $req_ty,
            job: &$crate::jobs::JobCtx,
        ) -> Result<$crate::jobs::JobOutcome<$resp>, rimstudio_ipc_types::error::ApiError> {
            $path(ctx, req, job)
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rimstudio_runner {
    (query $name:ident, $req_ty:ty) => {
        $crate::registry::Runner::Sync(|ctx, json| {
            let req: $req_ty = $crate::registry::decode_request(stringify!($name), json)?;
            let resp = handlers::$name(ctx, req)?;
            $crate::registry::encode_response(stringify!($name), &resp)
        })
    };
    (action $name:ident, $req_ty:ty) => {
        $crate::registry::Runner::Sync(|ctx, json| {
            let req: $req_ty = $crate::registry::decode_request(stringify!($name), json)?;
            let resp = handlers::$name(ctx, req)?;
            $crate::registry::encode_response(stringify!($name), &resp)
        })
    };
    (job $name:ident, $req_ty:ty) => {
        $crate::registry::Runner::Job(|ctx, json| {
            let req: $req_ty = $crate::registry::decode_request(stringify!($name), json)?;
            let app = ctx.clone();
            Ok(Box::new(move |job: &$crate::jobs::JobCtx| {
                handlers::$name(&app, req, job)?.into_output()
            }))
        })
    };
}

/// Hands the whole command table to a callback macro as `{ rows }`.
///
/// See the module documentation for the row syntax.
#[macro_export]
macro_rules! for_each_command {
    ($callback:ident) => {
        $callback! {
            query  app_ping (ctx, req: AppPingRequest) -> AppPingResponse = $crate::api::app::app_ping;
            query  app_get_info (ctx, req: AppGetInfoRequest) -> AppGetInfoResponse = $crate::api::app::app_get_info;
            query  app_list_tools (ctx, req: AppListToolsRequest) -> AppListToolsResponse = $crate::api::app::app_list_tools;
            action cancel_job (ctx, req: CancelJobRequest) -> CancelJobResponse = $crate::api::jobs::cancel_job;
            query  job_status (ctx, req: JobStatusRequest) -> JobStatusResponse = $crate::api::jobs::job_status;
            query  settings_get (ctx, req: SettingsGetRequest) -> SettingsDto = $crate::api::settings::settings_get;
            action settings_update (ctx, req: SettingsUpdate) -> SettingsDto = $crate::api::settings::settings_update;
            job    detect_run (ctx, req: DetectRunRequest) -> DetectionReportDto = $crate::api::detect::detect_run;
            query  detect_get_report (ctx, req: DetectGetReportRequest) -> DetectGetReportResponse = $crate::api::detect::detect_get_report;
            action detect_set_override (ctx, req: DetectSetOverrideRequest) -> DetectGetReportResponse = $crate::api::detect::detect_set_override;
            query  sources_list (ctx, req: SourcesListRequest) -> SourcesListResponse = $crate::api::sources::sources_list;
            action sources_add_folder (ctx, req: SourcesAddFolderRequest) -> SourceDto = $crate::api::sources::sources_add_folder;
            action sources_update (ctx, req: SourcesUpdateRequest) -> SourceDto = $crate::api::sources::sources_update;
            action sources_remove (ctx, req: SourcesRemoveRequest) -> SourcesRemoveResponse = $crate::api::sources::sources_remove;
            query  sources_probe_folder (ctx, req: SourcesProbeFolderRequest) -> SourcesProbeFolderResponse = $crate::api::sources::sources_probe_folder;
            job    library_scan (ctx, req: LibraryScanRequest) -> LibraryScanResult = $crate::api::library::library_scan;
            query  defs_search (ctx, req: DefSearchRequest) -> DefPage = $crate::api::defs::defs_search;
            query  defs_get_resolved (ctx, req: DefsGetResolvedRequest) -> ResolvedDefDto = $crate::api::defs::defs_get_resolved;
            action project_open (ctx, req: ProjectOpenRequest) -> ProjectSummaryDto = $crate::api::project::project_open;
            action project_create (ctx, req: ProjectCreateRequest) -> ProjectSummaryDto = $crate::api::project::project_create;
            action project_close (ctx, req: ProjectCloseRequest) -> ProjectCloseResponse = $crate::api::project::project_close;
            query  project_tree (ctx, req: ProjectTreeRequest) -> ProjectTreeDto = $crate::api::project::project_tree;
            query  project_layout_check (ctx, req: ProjectLayoutCheckRequest) -> ProjectLayoutCheckDto = $crate::api::project::project_layout_check;
            action project_scaffold_missing (ctx, req: ProjectScaffoldMissingRequest) -> ProjectScaffoldMissingDto = $crate::api::project::project_scaffold_missing;
            query  project_read_file (ctx, req: ProjectReadFileRequest) -> ProjectFileDto = $crate::api::project::project_read_file;
            query  project_layout_fix_plan (ctx, req: ProjectLayoutFixPlanRequest) -> ProjectLayoutFixPlanDto = $crate::api::project_fix::project_layout_fix_plan;
            job    project_layout_fix_apply (ctx, req: ProjectLayoutFixApplyRequest) -> ProjectLayoutFixApplyDto = $crate::api::project_fix::project_layout_fix_apply;
            action project_layout_fix_undo (ctx, req: ProjectLayoutFixUndoRequest) -> ProjectLayoutFixUndoDto = $crate::api::project_fix::project_layout_fix_undo;
            query  project_layout_fix_history (ctx, req: ProjectLayoutFixHistoryRequest) -> ProjectLayoutFixHistoryDto = $crate::api::project_fix::project_layout_fix_history;
            query  project_link_status (ctx, req: ProjectLinkStatusRequest) -> ProjectLinkStatusDto = $crate::api::project_link::project_link_status;
            action project_link_create (ctx, req: ProjectLinkCreateRequest) -> ProjectLinkResultDto = $crate::api::project_link::project_link_create;
            action project_link_remove (ctx, req: ProjectLinkRemoveRequest) -> ProjectLinkResultDto = $crate::api::project_link::project_link_remove;
            query  designer_reference_list (ctx, req: DesignerReferenceListRequest) -> ReferenceListDto = $crate::api::designer::designer_reference_list;
            query  designer_preview (ctx, req: DesignerPreviewRequest) -> PreviewDto = $crate::api::designer::designer_preview;
            query  designer_fit (ctx, req: DesignerFitRequest) -> FitReportDto = $crate::api::designer::designer_fit;
            query  designer_ce_suggest (ctx, req: DesignerCeSuggestRequest) -> CeSuggestionDto = $crate::api::designer::designer_ce_suggest;
            query  designer_ce_ammo_catalog (ctx, req: DesignerCeAmmoCatalogRequest) -> CeAmmoCatalogDto = $crate::api::designer::designer_ce_ammo_catalog;
            query  designer_ce_ammo_suggest (ctx, req: DesignerCeAmmoSuggestRequest) -> CeAmmoSuggestionDto = $crate::api::designer::designer_ce_ammo_suggest;
            query  designer_quiz_next (ctx, req: DesignerQuizNextRequest) -> QuizStepDto = $crate::api::designer::designer_quiz_next;
            action designer_quiz_answer (ctx, req: DesignerQuizAnswerRequest) -> DesignerQuizAnswerResponse = $crate::api::designer::designer_quiz_answer;
            action designer_quiz_back (ctx, req: DesignerQuizBackRequest) -> DesignerQuizAnswerResponse = $crate::api::designer::designer_quiz_back;
            job    designer_calibrate (ctx, req: DesignerCalibrateRequest) -> CalibrateResultDto = $crate::api::designer::designer_calibrate;
            job    designer_convert_scan (ctx, req: DesignerConvertScanRequest) -> ConvertScanDto = $crate::api::designer::designer_convert_scan;
            query  designer_export_plan (ctx, req: DesignerExportPlanRequest) -> WritePlanDto = $crate::api::designer::designer_export_plan;
            job    designer_apply_plan (ctx, req: DesignerApplyPlanRequest) -> ApplyReportDto = $crate::api::designer::designer_apply_plan;
            action designer_draft_save (ctx, req: DesignerDraftSaveRequest) -> DesignerDraftSaveResponse = $crate::api::designer::designer_draft_save;
            query  designer_draft_list (ctx, req: DesignerDraftListRequest) -> DesignerDraftListResponse = $crate::api::designer::designer_draft_list;
            action designer_draft_delete (ctx, req: DesignerDraftDeleteRequest) -> DesignerDraftDeleteResponse = $crate::api::designer::designer_draft_delete;
            action designer_clone (ctx, req: DesignerCloneRequest) -> DesignerCloneResponse = $crate::api::designer::designer_clone;
            query  designer_clone_diff (ctx, req: DesignerCloneDiffRequest) -> DesignerCloneDiffResponse = $crate::api::designer::designer_clone_diff;
            query  designer_structure_defaults (ctx, req: DesignerStructureDefaultsRequest) -> DesignerStructureDefaultsResponse = $crate::api::designer::designer_structure_defaults;
            query  designer_projectile_own (ctx, req: DesignerProjectileOwnRequest) -> DesignerProjectileOwnResponse = $crate::api::designer::designer_projectile_own;
            query  designer_asset_info (ctx, req: DesignerAssetInfoRequest) -> DesignerAssetInfoResponse = $crate::api::designer::designer_asset_info;
            query  designer_lint_files (ctx, req: DesignerLintFilesRequest) -> DesignerLintFilesResult = $crate::api::designer_lint::designer_lint_files;
            query  designer_archetype_catalog (ctx, req: DesignerArchetypeCatalogRequest) -> ArchetypeCatalogDto = $crate::api::designer_archetype::designer_archetype_catalog;
            query  designer_archetype_propose (ctx, req: DesignerArchetypeProposeRequest) -> ArchetypeProposalDto = $crate::api::designer_archetype::designer_archetype_propose;
            action designer_archetype_apply (ctx, req: DesignerArchetypeApplyRequest) -> DesignerArchetypeApplyResponse = $crate::api::designer_archetype::designer_archetype_apply;
        }
    };
}

pub use crate::for_each_command;

macro_rules! generate {
    ($( $kind:ident $name:ident ( $_ctx:ident , $_req:ident : $req_ty:ty ) -> $resp:ty = $path:path ; )*) => {
        /// One typed function per command: what a shell wrapper calls.
        pub mod handlers {
            #[allow(unused_imports)]
            use super::wire::*;
            $( $crate::__rimstudio_handler! { $kind $name, $req_ty, $resp, $path } )*
        }

        /// The dispatch table, in the order of the rows.
        pub static ROUTES: &[Route] = &[
            $( Route {
                name: stringify!($name),
                kind: $crate::__rimstudio_kind!($kind),
                request: stringify!($req_ty),
                response: stringify!($resp),
                handler: stringify!($path),
                runner: $crate::__rimstudio_runner! { $kind $name, $req_ty },
            }, )*
        ];
    };
}

for_each_command!(generate);

/// The route of a command, `None` for an unknown name.
#[must_use]
pub fn find(name: &str) -> Option<&'static Route> {
    ROUTES.iter().find(|r| r.name == name)
}

/// The names of every command, in table order.
#[must_use]
pub fn names() -> Vec<&'static str> {
    ROUTES.iter().map(|r| r.name).collect()
}

/// A JSON description of the table: name, kind, request and response type names and handler path.
#[must_use]
pub fn describe() -> Value {
    Value::Array(
        ROUTES
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "kind": r.kind.as_str(),
                    "request": r.request,
                    "response": r.response,
                    "handler": r.handler.replace(' ', ""),
                })
            })
            .collect(),
    )
}

/// A hash of [`describe`]: it changes when a command, a kind or a type name changes, so a webview
/// built against another table can notice.
#[must_use]
pub fn contract_hash() -> String {
    let text = serde_json::to_string(&describe()).unwrap_or_default();
    let hash = blake3::hash(text.as_bytes()).to_hex();
    hash.as_str().chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn names_are_unique_and_well_formed() {
        let mut seen = BTreeSet::new();
        for route in ROUTES {
            assert!(seen.insert(route.name), "duplicate {}", route.name);
            assert!(
                route
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                "{}",
                route.name
            );
        }
    }

    #[test]
    fn every_route_has_a_runner_that_matches_its_kind() {
        for route in ROUTES {
            match (route.kind, route.runner) {
                (CommandKind::Job, Runner::Job(_))
                | (CommandKind::Query | CommandKind::Action, Runner::Sync(_)) => {}
                (kind, runner) => panic!("{}: {kind:?} with {runner:?}", route.name),
            }
        }
    }

    #[test]
    fn the_hash_is_stable_and_sixteen_hex_digits() {
        let a = contract_hash();
        assert_eq!(a, contract_hash());
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_bad_field_is_named_in_the_error() {
        let err = decode_request::<DefSearchRequest>("defs_search", json!({"limit": "many"}))
            .expect_err("a string is not a limit");
        assert_eq!(err.code, "ipc.invalid-request");
        let field = err
            .details
            .as_ref()
            .and_then(|d| d.get("field"))
            .and_then(Value::as_str);
        assert_eq!(field, Some("limit"));
    }

    #[test]
    fn null_reads_as_an_empty_request() {
        assert!(decode_request::<AppGetInfoRequest>("app_get_info", Value::Null).is_ok());
        assert!(decode_request::<DetectGetReportRequest>("detect_get_report", json!({})).is_ok());
    }

    #[test]
    fn find_answers_known_and_unknown_names() {
        assert!(find("settings_get").is_some());
        assert!(find("settings_gett").is_none());
    }
}
