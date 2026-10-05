//! The RimStudio desktop shell (layer `l4-shell`).
//!
//! The shell is thin: it boots the Tauri free composition root of `rimstudio-app` once, keeps the
//! [`commands::Shell`] in managed state and offers the webview four commands (`rs_call`, `rs_cancel`,
//! `rs_info`, `rs_commands`) plus one event ([`events::JOB_EVENT`]). Every business rule lives in the
//! crates below `rimstudio-app` (ADR 0003, ADR 0043).
//!
//! - [`commands`]: the command layer over an [`rimstudio_app::AppContext`]; it needs no window, so the
//!   tests and the smoke run call it directly.
//! - [`events`]: the JSON of a job event, the same shape as the development bridge's `/dev/events`.
//! - [`nav`]: the navigation policy of the webview (first party pages only).
//! - [`smoke`]: the headless self test behind `rimstudio --smoke`.
//! - [`boot`]: how the shell builds its boot input (platform roots, `RIMSTUDIO_DATA_BASE`, logging).
//! - [`app`]: the Tauri builder, the command wrappers and the run loop.

#![warn(missing_docs)]

pub mod app;
pub mod boot;
pub mod commands;
pub mod events;
pub mod nav;
pub mod smoke;
