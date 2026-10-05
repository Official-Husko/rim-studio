//! The only XML boundary: parse, render, byte-span edit and RimWorld file codecs.
//!
//! Layer `l1-infra`. See docs/architecture/crate-catalog.md for the contract of this crate.
//!
//! Every other crate exchanges [`rimstudio_core::tree::Node`] trees and typed results; XML text
//! appears only here (invariants I-02 and I-03).

pub mod about;
pub mod about_edit;
pub mod defs_scan;
pub mod edit;
pub mod error;
pub mod load_folders;
pub mod load_folders_edit;
pub mod modes;
pub mod mods_config;
pub mod patches;
pub mod reader;
pub mod render;
pub mod save_meta;

mod text;
mod walk;

pub use error::{XmlError, XmlResult};
pub use modes::ParseMode;
pub use reader::{ParsedDoc, parse_document};
pub use render::{RenderOpts, render, render_defs_file};
