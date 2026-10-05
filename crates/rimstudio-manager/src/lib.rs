//! Mod manager use cases: settings, detection, sources and library scan.
//!
//! Layer `l3-feature`. See docs/architecture/crate-catalog.md for the contract of this crate. In the
//! 0.1.0 slice only the use cases that the item designer needs exist (settings, detection, custom mod
//! folders, scanning); the list session, sorting, profiles and launch come with the manager milestones.
//!
//! Every use case is a plain function `name(ctx: &Ctx, request) -> Result<Response, ManagerError>`
//! over the narrow [`Ctx`]. Requests and responses are small local structs and engine types;
//! `rimstudio-app` converts them to DTOs. The crate spawns no process and writes only inside the data
//! roots of the context (`settings.jsonc` and `workspace.jsonc` in the config root, the detection
//! report in the data root, the scan manifest in the cache root).

pub mod ctx;
pub mod detect;
pub mod error;
pub mod scan;
pub mod settings;
pub mod sources;

pub use ctx::Ctx;
pub use error::{ManagerError, ManagerResult};
