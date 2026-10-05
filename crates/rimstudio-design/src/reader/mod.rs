//! The reference reader: resolved defs in, design inputs out.
//!
//! The def engine hands over resolved `ThingDef` records (parents merged, patches applied). This module maps
//! them to the things the designer works with:
//!
//! * [`weapons`]: one [`weapons::WeaponDetail`] per ranged or melee weapon, the calibration
//!   [`crate::classes::ReferenceItem`]s derived from them, and the ranged and melee pools
//!   ([`weapons::build_pools`]), with tier, role and group derived at run time;
//! * [`spec`]: the reverse mapping from a def to a [`crate::model::DesignSpec`], for the clone and adjust
//!   flow and for the convert an existing item flow;
//! * [`options`]: what is left out, how roles are decided, where the reference armor comes from (all data);
//! * [`stuff`] and [`statdefs`]: the reference material and the stat definitions of the install, evaluated
//!   through the stat pipeline;
//! * [`pricing`]: the market value breakdown of a def from its cost list, stuff and work;
//! * [`access`]: small readers over nodes.
//!
//! The reader reads only the vanilla shape. A weapon whose verbs or tools carry a `Class` attribute (a modded
//! verb or tool class) is left out of the vanilla pools; the Combat Extended reader in [`crate::ce::reader`]
//! picks those up. No value table of the game or of any mod lives in this module: every number comes from the
//! databases of the load.

pub mod access;
pub mod options;
pub mod pricing;
pub mod spec;
pub mod statdefs;
pub mod stuff;
pub mod weapons;

pub use options::{
    ArmorSource, Exclusions, FALLBACK_ARMOR_RATINGS, ReaderOptions, RoleCondition, RoleFacts,
    RoleRule, RoleRules,
};
pub use pricing::market_value_of;
pub use spec::{SpecReading, spec_from_def, spec_from_def_inheriting};
pub use statdefs::StatTable;
pub use stuff::{StuffProfile, evaluate_stat};
pub use weapons::{
    ArmorLayers, RangedDetail, ReadContext, ReferencePools, ReferenceSet, SkipReason, SkippedDef,
    ToolDetail, WeaponDetail, build_pools, is_weapon_def, read_reference_set, reference_armor,
    tech_level_named,
};

#[cfg(test)]
pub(crate) mod fixtures_tests;
#[cfg(test)]
mod tests;
