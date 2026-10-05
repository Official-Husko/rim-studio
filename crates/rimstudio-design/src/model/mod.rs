//! The designer's data model: the design spec, source tags and the draft envelope.
//!
//! - [`source`]: [`Sourced`] values, [`ValueSource`] and the rule that typed values are never overwritten.
//! - [`spec`]: [`DesignSpec`] and its parts for ranged and melee weapons, plus the optional [`CePatchSpec`].
//! - [`assets`]: texture imports and custom sounds, the inputs of the asset copy step.
//! - [`carried`]: the recipe, tool extras and the raw fields a clone carries from its source.
//! - [`draft`]: the [`Draft`] envelope with answers and anchors.
//! - [`bridge`]: conversions into the inputs of the math modules.
//!
//! Everything here is plain data with `serde` support (camelCase fields, kebab-case enum strings) and no I/O.

pub mod archetype;
pub mod assets;
pub mod bridge;
pub mod carried;
pub mod ce_ammo;
pub mod ce_extras;
pub mod draft;
pub mod format;
pub mod platform;
pub mod source;
pub mod spec;

pub use archetype::{ArchetypeChoice, ArchetypeMode, BalanceTarget, Descriptors, RateOfFire};
pub use assets::{AssetImports, CustomSound, FloatRange, SoundImports, shot_sound_def_name};
pub use bridge::{DEFAULT_BURST_COUNT, DEFAULT_TICKS_BETWEEN_SHOTS};
pub use carried::{
    ExtraMeleeDamage, INHERIT_RESETTABLE, MODELLED_THING_FIELDS, RecipeSpec, SurpriseAttackSpec,
    extra_damage_from_node,
};
pub use ce_ammo::{
    CookOffKind, CustomAmmoItem, CustomAmmoRecipe, CustomAmmoSpec, CustomAmmoType, CustomFragment,
    CustomIngredient, CustomProjectile, CustomSecondaryDamage,
};
pub use ce_extras::CeToolPlan;
pub use draft::{
    Anchor, CalibrationMode, DRAFT_KIND, DRAFT_OLDEST_SCHEMA_VERSION, DRAFT_SCHEMA_VERSION, Draft,
    migrate_value,
};
pub use format::format_number;
pub use platform::{
    CeAttachmentLink, CeGraphicPart, CeStatEntry, CeUnderBarrel, CeUnderBarrelFireModes,
};
pub use source::{OfferOutcome, Sourced, ValueSource, offer};
pub use spec::{
    AccuracyInputs, CePatchSpec, CeToolPenetration, CostEntry, DEFAULT_DAMAGE_DEF,
    DEFAULT_GRAPHIC_CLASS, DEFAULT_PROJECTILE_PARENT, DEFAULT_VERB_CLASS, DesignSpec, Identity,
    ItemKind, ParentRef, ProjectileChoice, ProjectileSpec, Quality, RangedInputs, ScalarField,
    StuffSpec, TechLevel, ToolSpec,
};
