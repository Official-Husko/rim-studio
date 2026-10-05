//! Ids, versions, node tree, diagnostics, load plan, ports, settings and job types.
//!
//! Layer `l0-domain`. See docs/architecture/crate-catalog.md for the contract of this crate.

pub mod diag;
pub mod error;
pub mod ids;
pub mod jobs;
pub mod load_plan;
pub mod mods;
pub mod os;
pub mod paths;
pub mod ports;
pub mod redact;
pub mod settings;
pub mod tree;
pub mod version;
