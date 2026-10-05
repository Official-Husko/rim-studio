//! Reference sets, projects and def database snapshots.
//!
//! Layer `l2-service`. See docs/architecture/crate-catalog.md for the contract of this crate.
//!
//! - [`refset`]: the content packs the def engine reads, in load order (game, expansions, chosen
//!   mods, the project last), each with its resolved load plan and file lists.
//! - [`session`]: [`session::WorkspaceSession`] parses the reference set, builds the def type table
//!   from the game and mod assemblies at run time, runs the def engine and keeps an immutable
//!   [`snapshot::Snapshot`] and a [`defindex::DefIndex`].
//! - [`defindex`]: the always-on def index with paged search.
//! - [`project`]: the store of the mod folders the user works on.
//! - [`scaffold`]: plans for new mod folders as node trees.
//!
//! The crate never writes into game or mod folders; applying a [`scaffold::ScaffoldPlan`] is the
//! caller's job through `rimstudio-io`.

pub mod defindex;
pub mod error;
pub mod listing;
pub mod parse;
pub mod project;
pub mod refset;
pub mod scaffold;
pub mod session;
pub mod snapshot;
pub mod typetable;

pub use error::{WorkspaceError, WorkspaceResult};
