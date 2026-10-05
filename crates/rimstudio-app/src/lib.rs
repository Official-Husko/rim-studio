//! Composition root, command registry and dispatch.
//!
//! Layer `l4-app`. See docs/architecture/crate-catalog.md for the contract of this crate. The crate is
//! free of Tauri: the desktop shell and the headless CLI both call [`boot::boot`] to get an
//! [`context::AppContext`] and [`dispatch::dispatch`] to run commands, so they execute the same
//! handlers (ADR 0003).
//!
//! - [`context`]: [`context::AppContext`] with explicit fields and no globals, the platform ports.
//! - [`mod@boot`]: roots, settings, logging, crash marker, job runner.
//! - [`logging`]: `tracing` with a rolling JSON lines file, redaction, no telemetry.
//! - [`jobs`]: the job runner (caller minted ids, registry, rate limited progress, one terminal event).
//! - [`registry`]: the one command table, `for_each_command!`, the typed handlers and the routes.
//! - [`api`]: the handlers, grouped by area.
//! - [`mod@dispatch`]: name and JSON in, JSON or an `ApiError` out; jobs; writing a result to a file.
//! - [`tools`]: the tool table with the capabilities of this machine.
//! - [`workspace`]: the hub that builds the toolkit's def sessions lazily.
//! - [`convert`]: engine and manager types to the DTOs of the contract crate.
//! - [`dto`]: the few request and response types only this layer owns.
//! - [`error`]: [`error::AppError`], [`error::BootError`] and the mapping of crate errors to `ApiError`.

#![warn(missing_docs)]

pub mod api;
pub mod boot;
pub mod context;
pub mod convert;
pub mod dispatch;
pub mod dto;
pub mod error;
pub mod jobs;
pub mod logging;
pub mod registry;
pub mod tools;
pub mod workspace;

pub use boot::{BootInput, BootReport, boot};
pub use context::{AppContext, AppEvent, Platform};
pub use dispatch::{
    dispatch, dispatch_blocking, dispatch_blocking_with, dispatch_job, dispatch_to_file,
};
pub use error::{AppError, BootError};
