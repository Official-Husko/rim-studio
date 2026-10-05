//! The handlers of the registry: plain functions over [`crate::context::AppContext`].
//!
//! Each module holds the handlers of one command area. A handler decodes nothing and encodes nothing:
//! the registry wrappers do that. A handler calls the feature crate, converts engine types to the DTOs
//! of `rimstudio-ipc-types` and maps the crate error to an `ApiError` (rule 1 of section 2.4 of the IPC
//! document). Business rules live in the feature crates; nothing here decides ordering, validation or
//! balance.
//!
//! - [`app`]: `app_ping`, `app_get_info`, `app_list_tools`.
//! - [`jobs`]: `cancel_job`, `job_status`.
//! - [`settings`]: `settings_get`, `settings_update`.
//! - [`detect`]: `detect_run`, `detect_get_report`, `detect_set_override`.
//! - [`sources`]: the `sources_*` commands.
//! - [`library`]: the `library_scan` job.
//! - [`defs`]: `defs_search`, `defs_get_resolved`.
//! - [`project`]: `project_open`, `project_create`, `project_close`.
//! - [`project_fix`]: the `project_layout_fix_*` commands (plan, apply, undo, history).
//! - [`project_link`]: the `project_link_*` commands (status, create, remove): make a project visible to the game.
//! - [`designer`]: every `designer_*` command the toolkit implements, except the lint of patch files,
//!   which is `designer_lint::designer_lint_files`.

pub mod app;
pub mod defs;
pub mod designer;
pub mod designer_lint;
pub mod detect;
pub mod jobs;
pub mod library;
pub mod project;
pub mod project_fix;
pub mod project_link;
pub mod settings;
pub mod sources;
