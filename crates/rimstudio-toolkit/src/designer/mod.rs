//! The weapon designer: reference lists, exact readouts, suggestions, the fit meter, the quiz,
//! calibration and drafts, as thin orchestration over the engines of `rimstudio-design` and the def
//! snapshots of `rimstudio-workspace`.
//!
//! The designer always writes vanilla definitions. Combat Extended is an opt-in block of the spec (`ce`,
//! absent by default); every function here treats a spec without it as vanilla only and never adds it.
//!
//! Every command is `pub fn name(ctx: &Ctx, req: Req) -> Result<Resp, ToolkitError>`; the calibration job
//! also takes a progress sink and a cancel token.
//!
//! - `ctx`: the narrow context ([`Ctx`]) and the lazily built per snapshot [`Engine`].
//! - `refs`: `designer_reference_list`.
//! - `preview`: `designer_preview`, and the helpers that write suggestions into specs.
//! - `fit`: `designer_fit`.
//! - `ce`: `designer_ce_suggest` and the `acceptSuggestions` option of plan and apply.
//! - `ammo`: `designer_ce_ammo_catalog` and `designer_ce_ammo_suggest`, the catalogue of the ammo sets of the
//!   install and the suggestions for a new ammo type.
//! - `clone`: flow C: `designer_clone`, `designer_clone_diff` and `designer_structure_defaults`.
//! - `own`: the source of a clone as its file writes it, and `designer_projectile_own`.
//! - `quiz`: `designer_quiz_next`, `designer_quiz_answer` and Back.
//! - `calibrate`: `designer_calibrate`, the calibration cache.
//! - `drafts`: `designer_draft_save`, `_list` and `_delete`.
//! - `dto`: conversions between engine types and DTOs.
//! - `asset_info`: `designer_asset_info`, the facts of a texture or clip file the page offers to import.
//! - `plan`, `apply`, `convert`: write plans, applying them and converting existing weapons.

pub mod ammo;
pub mod apply;
pub mod asset_info;
pub mod calibrate;
pub mod ce;
pub mod clone;
pub mod convert;
mod convert_facts;
pub mod ctx;
pub mod drafts;
pub mod dto;
pub mod fit;
pub mod lint_explain;
pub mod lint_files;
pub mod own;
pub mod plan;
pub mod preview;
pub mod quiz;
pub mod refs;

pub use ammo::{ce_ammo_catalog, ce_ammo_suggest};
pub use apply::{ApplyOptions, apply_built, apply_plan};
pub use asset_info::asset_info;
pub use calibrate::{CalibrationState, calibrate};
pub use ce::{accept_for_plan, ce_suggest, suggestion_for};
pub use clone::{clone_diff, clone_draft, structure_defaults};
pub use convert::{answers_for_def, convert_plan, convert_scan, group_applies};
pub use ctx::{Ctx, Engine, SessionLookup};
pub use drafts::{draft_delete, draft_list, draft_load, draft_save};
pub use fit::fit;
pub use lint_files::lint_files;
pub use own::{derived_projectile_name, projectile_own};
pub use plan::{BuiltPlan, export_plan};
pub use preview::{preview, suggest_fill};
pub use quiz::{quiz_answer, quiz_back, quiz_next};
pub use refs::{reference_list, reference_list_ce};
