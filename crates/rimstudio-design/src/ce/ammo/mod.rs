//! Custom Combat Extended ammunition: the catalogue of the user's ammo sets, the suggestions for a new
//! ammo type, the generator of the definition files, the checks and the dry load.
//!
//! The designer always writes vanilla definitions; this module works only when the Combat Extended switch of
//! the spec is on and the spec carries a [`CustomAmmoSpec`](crate::model::CustomAmmoSpec). Everything it knows
//! about the game comes from the user's own Combat Extended at run time ([`library`]); no value is built in.
//!
//! - [`names`]: class names and the def names derived from the name of a custom caliber.
//! - [`library`]: ammo classes, items, projectiles, recipes and thing categories of the install.
//! - [`catalog`]: every ammo set of the install, searchable, filterable and paged.
//! - [`suggest`]: defaults for a new type from the nearest of the user's own ammunition, and a copy of an
//!   existing type.
//! - [`generate`]: the node trees of the definition file.
//! - [`validate`]: required fields, plausibility against the user's own ammunition, duplicates.
//! - [`lint`]: the lint rules for the generated definitions (CEP050 and following).
//! - [`dryload`]: the generated definitions loaded through the def engine.
//! - [`plan`]: the planned file and the effective Combat Extended block of a weapon.

pub mod catalog;
pub mod codes;
pub mod dryload;
pub mod generate;
pub mod library;
pub mod lint;
pub mod names;
pub mod plan;
pub mod suggest;
pub mod validate;

pub use catalog::{
    CatalogEntry, CatalogFacet, CatalogPage, CatalogQuery, CatalogSecondary, CatalogType, catalog,
};
pub use dryload::{DryLoad, dry_load};
pub use generate::{GeneratedAmmo, generate};
pub use library::{
    AmmoClassFacts, AmmoFacts, AmmoLibrary, CategoryFacts, ProjectileFacts, RecipeFacts,
    read_library,
};
pub use lint::lint_defs;
pub use names::{DerivedNames, PairForm, identifier, type_key};
pub use plan::{
    Prepared, ammo_file_path, ammo_plan, effective_ce_block, export_with_ammo, prepare,
};
pub use suggest::{
    AmmoHints, AmmoSourceKind, AmmoSuggestion, NearestAmmo, SuggestedAmmoField, set_from_existing,
    suggest_type, type_from_existing, type_from_pair,
};
pub use validate::{
    duplicate_defs, installed_names, validate_custom_ammo, validate_custom_ammo_refs,
};
