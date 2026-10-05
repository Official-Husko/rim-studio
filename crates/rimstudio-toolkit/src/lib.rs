//! Toolkit tool modules (designer first).
//!
//! Layer `l3-feature`. See docs/architecture/crate-catalog.md for the contract of this crate.
//!
//! One module per tool, each behind a Cargo feature (all on by default):
//!
//! - [`designer`] (`tool-designer`): the weapon designer. Reference lists, exact readouts, suggestions with
//!   bands, the fit meter, the quiz, calibration, drafts, and (second half) write plans, apply and the
//!   conversion of existing weapons. The designer always writes vanilla definitions; the Combat Extended
//!   patch is an opt-in block of the spec.
//! - [`project`] (`tool-project`): opening and creating mod projects.
//! - [`defs`] (`tool-defs`): the def explorer.
//!
//! A tool module never uses another tool module (enforced by `xtask` and by a test); shared pieces live in
//! [`error`], in [`shared`] (the line diff, the guarded project writer, the project view and the byte span
//! merging that the designer and the project tool both need) and in the engine crates. [`registry`] is the static table the app composes its tool list from.
//! Every function is plain orchestration: it takes a context and a request and returns a response or a
//! [`error::ToolkitError`]; rules live in the engine crates (`rimstudio-design`, `rimstudio-defs`).

#![warn(missing_docs)]

#[cfg(feature = "tool-defs")]
pub mod defs;
#[cfg(feature = "tool-designer")]
pub mod designer;
pub mod error;
#[cfg(feature = "tool-project")]
pub mod project;
pub mod registry;
pub mod shared;
