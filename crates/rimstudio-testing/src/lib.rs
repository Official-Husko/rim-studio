//! Fixture builders, fake ports and golden helpers (dev only).
//!
//! Layer `support`. Use this crate from `[dev-dependencies]` only. It provides:
//!
//! - [`fakes`]: fakes for every port of `rimstudio_core::ports` (clock, environment, registry,
//!   processes, links, launcher, credentials, sandbox, install source);
//! - [`fake_fs`]: an in memory `FsProbe` and a recording file system for fence tests;
//! - [`detect_env`]: a `DetectEnv` made of fakes, with presets for the per operating system Steam
//!   layouts of the detection research;
//! - [`fixtures`]: small fictional builders (mods, ids, sources, settings) that need no XML;
//! - [`golden`]: JSON golden file helpers over `goldenfile`;
//! - [`prelude`]: everything above in one import.
//!
//! All names and numbers are fictional (prefix `RS_`); no game data is embedded (R11).

pub mod detect_env;
pub mod fake_fs;
pub mod fakes;
pub mod fixtures;
pub mod golden;
pub mod install_tree;
pub mod loaders;
pub mod prelude;
