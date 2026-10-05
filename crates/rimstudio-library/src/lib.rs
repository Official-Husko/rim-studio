//! Scanner and mod index.
//!
//! Layer `l2-service`. See docs/architecture/crate-catalog.md for the contract of this crate.
//!
//! The crate turns a [`SourceSet`](rimstudio_core::mods::SourceSet) into a [`LibraryIndex`]
//! quickly and incrementally:
//!
//! - [`scan`]: the two level scanner ([`Scanner::scan`]), options, outcome and statistics;
//! - [`cache`]: the compact JSON manifest keyed by stat keys and its debounced writer;
//! - [`index`]: the library index (core `ModIndex` plus scan facts, content and a search helper);
//! - [`sources`]: classifying a folder, scan layouts and overlap detection;
//! - [`duplicates`]: deterministic duplicate package id resolution with the game's `_steam` rule;
//! - [`error`]: typed errors and diagnostic codes.
//!
//! Thumbnails, list import and export, and deploy (link farm, `ModsConfig.xml` write) are deferred
//! beyond release 0.1.0; their modules ([`thumbs`], [`lists`], [`deploy`]) only document the plan.
//! The library never writes into the game folders.

pub mod cache;
mod content;
pub mod deploy;
mod discover;
pub mod duplicates;
pub mod error;
pub mod index;
pub mod lists;
pub mod scan;
pub mod sources;
pub mod thumbs;

pub use cache::{Manifest, ManifestWriter};
pub use duplicates::{DuplicatePolicy, DuplicateReport, resolve};
pub use error::{LibraryError, LibraryResult};
pub use index::{LibraryIndex, ModContent, ModInfo};
pub use scan::{ScanLevel, ScanOptions, ScanOutcome, Scanner};
