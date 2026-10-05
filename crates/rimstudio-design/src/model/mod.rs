//! The designer's data model: the design spec, source tags and the draft envelope.
//!
//! - [`source`]: [`Sourced`] values, [`ValueSource`] and the rule that typed values are never overwritten.
//! - [`spec`]: [`DesignSpec`] and its parts for ranged and melee weapons, plus the optional [`CePatchSpec`].
//! - [`draft`]: the [`Draft`] envelope with answers and anchors.
//! - [`bridge`]: conversions into the inputs of the math modules.
//!
//! Everything here is plain data with `serde` support (camelCase fields, kebab-case enum strings) and no I/O.

pub mod bridge;
pub mod draft;
pub mod format;
pub mod source;
pub mod spec;

pub use bridge::{DEFAULT_BURST_COUNT, DEFAULT_TICKS_BETWEEN_SHOTS};
pub use draft::{Anchor, CalibrationMode, DRAFT_KIND, DRAFT_SCHEMA_VERSION, Draft};
pub use format::format_number;
pub use source::{OfferOutcome, Sourced, ValueSource, offer};
pub use spec::{
    AccuracyInputs, CePatchSpec, CeToolPenetration, CostEntry, DEFAULT_DAMAGE_DEF,
    DEFAULT_GRAPHIC_CLASS, DEFAULT_PROJECTILE_PARENT, DEFAULT_VERB_CLASS, DesignSpec, Identity,
    ItemKind, ParentRef, ProjectileChoice, ProjectileSpec, Quality, RangedInputs, ScalarField,
    StuffSpec, TechLevel, ToolSpec,
};
