//! The development bridge: a loopback HTTP server that lets the browser UI call the real backend.
//!
//! Layer `l4-shell`. During frontend development the UI runs in a normal browser (Vite) and not in the
//! desktop shell, so it has no `invoke`. This crate boots the same [`rimstudio_app::AppContext`] as the
//! shell and the CLI and serves the registry over HTTP on 127.0.0.1, so a browser page works with the
//! real numbers of the owner's install. It is a development tool only: it is never packaged, it is not
//! part of any release artefact and the desktop shell does not depend on it (ADR 0040).
//!
//! The wire contract is "bridge API v1" in `docs/architecture/ipc-and-state.md` section 14.
//!
//! - [`http`]: the small HTTP/1.1 reader and writer with hard limits.
//! - [`security`]: token, `Host`, `Origin`, content type and loopback checks.
//! - [`server`]: accept loop, connection cap, routing, shutdown and the token file.
//! - [`rpc`]: `POST /rpc/COMMAND`, `/dev/info` and `/dev/commands`.
//! - [`events`]: the event hub behind `GET /dev/events`.
//! - [`fsapi`]: the folder listing for the development folder picker.
//! - [`cli`]: command line flags.

#![warn(missing_docs)]

pub mod cli;
pub mod events;
pub mod fsapi;
pub mod http;
pub mod rpc;
pub mod security;
pub mod server;

pub use server::{ServerHandle, ServerOptions, Stopper, start};

/// The version of the bridge API (the crate version).
pub const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error codes the bridge adds for protocol problems. They are developer facing and are not part of
/// the contract registry of `rimstudio-ipc-types`.
pub mod codes {
    /// A malformed or unsupported request (the registered `ipc.invalid-request`).
    pub const INVALID_REQUEST: &str = "ipc.invalid-request";
    /// The token is missing or wrong.
    pub const UNAUTHORIZED: &str = "bridge.unauthorized";
    /// The peer, `Host` or `Origin` is not allowed.
    pub const FORBIDDEN: &str = "bridge.forbidden";
    /// The request was not complete in time.
    pub const TIMEOUT: &str = "bridge.timeout";
    /// The body is larger than the limit.
    pub const TOO_LARGE: &str = "bridge.too-large";
    /// Too many open connections.
    pub const TOO_BUSY: &str = "bridge.too-busy";
    /// No such path.
    pub const NOT_FOUND: &str = "bridge.not-found";
    /// The registered `io.not-found`.
    pub const IO_NOT_FOUND: &str = "io.not-found";
    /// The registered `io.not-a-directory`.
    pub const IO_NOT_A_DIRECTORY: &str = "io.not-a-directory";
    /// The registered `io.permission-denied`.
    pub const IO_PERMISSION_DENIED: &str = "io.permission-denied";
    /// The registered `app.internal-error`.
    pub const APP_INTERNAL_ERROR: &str = "app.internal-error";
}
