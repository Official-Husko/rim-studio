//! Validation of designer input: the diagnostic code registry and the pure checks.
//!
//! - [`codes`]: the registry of codes with default severities and message templates, and the field pointer
//!   convention ([`FIELD_ARG`]).
//! - [`checks`]: the pure functions. [`validate_vanilla`] and [`validate_ce_patch`] are the two scopes,
//!   [`validate_spec`] and [`validate_draft`] the convenient wholes, [`validate_refs`] the seam for checks
//!   against loaded defs.
//!
//! Diagnostics are `rimstudio_core::diag::Diagnostic` values; the JSON pointer of the field each concerns
//! is in the `field` argument (see [`diagnostic_field`]) and is relative to the spec.

pub mod asset_codes;
pub mod assets;
pub mod checks;
pub mod codes;

pub use assets::validate_assets;
pub use checks::{
    DefLookup, MAX_PLAUSIBLE_MASS, MAX_PLAUSIBLE_RANGE, MAX_PLAUSIBLE_SECONDS, RefKind,
    validate_ce_patch, validate_draft, validate_refs, validate_spec, validate_vanilla,
};
pub use codes::{
    CodeInfo, FIELD_ARG, REGISTRY, diagnostic_field, has_errors, lookup, render_template,
};
