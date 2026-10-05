//! Request and response types that only the app layer owns.
//!
//! These are the rows of the registry that have no DTO in `rimstudio-ipc-types` yet: the
//! infrastructure rows (`app_ping`, `app_get_info`, `app_list_tools`, `job_status`) and
//! `project_create`. They follow the contract conventions (camelCase fields, kebab-case enums, absent
//! options omitted) so they can move into the contract crate unchanged.

use rimstudio_ipc_types::jobs::{JobEvent, ProgressDto};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jobs::JobState;

/// Request of `app_ping`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct AppPingRequest {
    /// Text echoed back unchanged.
    pub echo: String,
}

/// Response of `app_ping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppPingResponse {
    /// The echoed text.
    pub echo: String,
    /// Unix time in milliseconds by the app clock.
    pub at_ms: u64,
}

/// Request of `app_get_info`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppGetInfoRequest {}

/// Response of `app_get_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppGetInfoResponse {
    /// The application version.
    pub version: String,
    /// The operating system.
    pub os: String,
    /// True when the data folders live beside the executable.
    pub portable: bool,
    /// A hash of the command registry (names, kinds and type names); a stale webview can compare it.
    pub contract_hash: String,
    /// The random id of this run.
    pub session_id: String,
    /// How many commands the registry has.
    pub command_count: u32,
    /// True when the previous run did not end cleanly.
    pub previous_run_ended_abnormally: bool,
}

/// Request of `app_list_tools`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppListToolsRequest {}

/// Request of `job_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct JobStatusRequest {
    /// The job to look up.
    pub job_id: String,
}

/// Response of `job_status`. Finished jobs stay known for a minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct JobStatusResponse {
    /// The job id.
    pub job_id: String,
    /// Where the job is. `unknown` when the runner has no such id.
    pub state: JobStateDto,
    /// Catalog key of the job label. Absent for an unknown id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label_key: Option<String>,
    /// Unix time in milliseconds when the job was registered. Absent for an unknown id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub started_at_ms: Option<u64>,
    /// The latest progress record that was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub progress: Option<ProgressDto>,
    /// The terminal event, once the job has ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub terminal: Option<JobEvent<Value>>,
}

/// The state `job_status` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum JobStateDto {
    /// The job is running.
    Running,
    /// A cancel was requested and the job is winding down.
    Cancelling,
    /// The job finished and sent its result.
    Finished,
    /// The job failed.
    Failed,
    /// The job was cancelled.
    Cancelled,
    /// The runner does not know the id.
    Unknown,
}

impl From<JobState> for JobStateDto {
    fn from(value: JobState) -> Self {
        match value {
            JobState::Running => Self::Running,
            JobState::Cancelling => Self::Cancelling,
            JobState::Finished => Self::Finished,
            JobState::Failed => Self::Failed,
            JobState::Cancelled => Self::Cancelled,
        }
    }
}

/// Request of `project_create`: a new mod folder with the usual layout.
///
/// Mirrors the scaffold options of the workspace crate. Nothing is written when the target exists or
/// lies inside the game install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectCreateRequest {
    /// The folder to create; the mod root. Must be absolute.
    pub path: String,
    /// Display name of the mod.
    pub name: String,
    /// Package id, letters, digits and dots with at least one dot.
    pub package_id: String,
    /// Author shown in `About.xml`.
    pub author: String,
    /// Description shown in `About.xml`.
    pub description: String,
    /// Supported game versions, `major.minor` each. Empty means `1.6`.
    pub supported_versions: Vec<String>,
    /// One folder per game version instead of a flat layout.
    pub versioned_folders: bool,
    /// Create the `Patches` folder.
    pub patches_folder: bool,
    /// Create the `Languages` folder.
    pub languages_folder: bool,
    /// Create the `Assemblies` folder.
    pub assemblies_folder: bool,
    /// Create the optional `Compat/CombatExtended` folder gated in `LoadFolders.xml`. Off unless asked.
    pub ce_patch_folder: bool,
    /// Put a placeholder file in folders that would stay empty.
    pub placeholder_files: bool,
    /// Create the texture folders of weapons and projectiles.
    pub textures_folder: bool,
    /// Create `Sounds/Weapons`.
    pub sounds_folder: bool,
    /// Create `Source/Art` (art sources and code, never shipped).
    pub source_folder: bool,
    /// Write a `.gitignore` for a mod repository. Off unless asked.
    pub gitignore: bool,
    /// With `gitignore`: keep `Source/Art` and `Raw Assets` out of the repository.
    pub ignore_source_art: bool,
    /// Write a `README.md`. Off unless asked.
    pub readme: bool,
    /// Write a `Credits.txt`. Off unless asked.
    pub credits: bool,
}

impl Default for ProjectCreateRequest {
    fn default() -> Self {
        Self {
            path: String::new(),
            name: String::new(),
            package_id: String::new(),
            author: String::new(),
            description: String::new(),
            supported_versions: Vec::new(),
            versioned_folders: false,
            patches_folder: true,
            languages_folder: false,
            assemblies_folder: false,
            ce_patch_folder: false,
            placeholder_files: false,
            textures_folder: true,
            sounds_folder: true,
            source_folder: false,
            gitignore: false,
            ignore_source_art: false,
            readme: false,
            credits: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_create_defaults_match_the_scaffold_defaults() {
        let req: ProjectCreateRequest =
            serde_json::from_str(r#"{"path":"/x/y","name":"RS_Mod","packageId":"rs.mod"}"#)
                .unwrap_or_default();
        assert!(req.patches_folder);
        assert!(!req.ce_patch_folder);
        assert_eq!(req.package_id, "rs.mod");
    }

    #[test]
    fn job_state_dto_is_kebab_case() {
        let text = serde_json::to_string(&JobStateDto::Cancelling).unwrap_or_default();
        assert_eq!(text, "\"cancelling\"");
    }

    #[test]
    fn unknown_status_omits_absent_members() {
        let status = JobStatusResponse {
            job_id: "j1".into(),
            state: JobStateDto::Unknown,
            label_key: None,
            started_at_ms: None,
            progress: None,
            terminal: None,
        };
        let text = serde_json::to_string(&status).unwrap_or_default();
        assert_eq!(text, r#"{"jobId":"j1","state":"unknown"}"#);
    }
}
