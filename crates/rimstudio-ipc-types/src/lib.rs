//! Process boundary DTOs.
//!
//! Layer `contract`. Every serde type that crosses a process boundary lives here: the error envelope, job
//! and progress records, settings, detection and source DTOs, the def explorer pages and the designer
//! requests and responses of the weapons slice. See docs/architecture/crate-catalog.md for the contract.
//!
//! Conventions that hold for every type:
//!
//! - field names are camelCase and enum strings are kebab-case;
//! - 64 bit ids travel as strings and sizes and times are plain numbers below 2^53;
//! - an `Option` member means "absent when unknown or not applicable", documented on each member, and is
//!   omitted from the JSON when absent;
//! - collections that come from maps are ordered by key, so equal data gives byte identical JSON;
//! - every type derives `ts_rs::TS` behind the off by default `ts` feature; the webview types are generated from
//!   these derives (docs/architecture/ipc-and-state.md, "Bindings as built").
//!
//! Conversions from core types live here only where they need nothing beyond `rimstudio-core`; the
//! conversion of engine types lives in the feature crates.

pub mod defs;
pub mod designer;
pub mod diagnostic;
pub mod error;
pub mod jobs;
pub mod library;
pub mod mods;
pub mod project;
pub mod project_fix;
pub mod settings;
pub mod tools;
