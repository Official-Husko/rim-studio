//! Own VDF reader, ACF views and the game locator.
//!
//! Layer `l2-service`. See docs/architecture/crate-catalog.md for the contract of this crate.
//!
//! The crate reads the Steam client the user already has installed; nothing is bundled and nothing
//! is written. All file system, registry and environment access goes through
//! `rimstudio_core::ports::DetectEnv`, so every layout can be tested with fakes.
//!
//! - [`vdf`]: a KeyValues1 reader and printer.
//! - [`acf`]: typed views of `libraryfolders.vdf`, app manifests and workshop manifests.
//! - [`locator`]: `detect`, the options and the [`locator::GameLocator`] trait.
//! - [`probe`]: candidate folders per operating system and a deadline aware prober.
//! - [`report`]: the serde friendly `DetectionReport` and the stable warning codes.
//! - [`workshop`]: Workshop item status.
//! - [`custom`]: custom mod folders as extra sources.
//! - [`install`]: version file parsing and executable names.
//! - [`launch`]: how the game is started, as data.

pub mod acf;
pub mod custom;
pub mod error;
pub mod install;
pub mod launch;
pub mod locator;
pub mod probe;
pub mod report;
pub mod vdf;
pub mod workshop;
