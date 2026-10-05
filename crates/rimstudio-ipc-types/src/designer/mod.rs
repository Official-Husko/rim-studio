//! Designer DTOs: the requests and responses of the `designer_*` commands of the weapons slice.
//!
//! The designer always writes vanilla definitions. The Combat Extended patch is an optional block of the
//! spec (`ce`, absent by default) and is never enabled automatically.
//!
//! - [`spec`]: the design spec and its building blocks.
//! - [`carried`]: the recipe, tool extras and the raw fields a clone carries.
//! - [`draft`]: drafts and the draft store requests.
//! - [`reference`]: the reference weapon list.
//! - [`preview`]: readouts, suggestions and the material matrix.
//! - [`fit`]: the fit report.
//! - [`quiz`]: the quiz prompts and answers.
//! - [`calibrate`]: the calibration job.
//! - [`convert`]: the scan and ask list of the automatic conversion.
//! - [`clone`]: flow C, clone and adjust, the diff against the source and the structure defaults.
//! - [`ce_suggest`]: the suggestions for the optional Combat Extended block.
//! - [`assets`]: texture imports, custom sounds and `designer_asset_info`.
//! - [`plan`]: the write plan and the apply report.

pub mod assets;
pub mod calibrate;
pub mod carried;
pub mod ce_suggest;
pub mod clone;
pub mod convert;
pub mod draft;
pub mod fit;
pub mod plan;
pub mod preview;
pub mod quiz;
pub mod reference;
pub mod spec;

pub use crate::diagnostic::DiagnosticDto;
pub use assets::*;
pub use calibrate::*;
pub use carried::*;
pub use ce_suggest::*;
pub use clone::*;
pub use convert::*;
pub use draft::*;
pub use fit::*;
pub use plan::*;
pub use preview::*;
pub use quiz::*;
pub use reference::*;
pub use spec::*;
