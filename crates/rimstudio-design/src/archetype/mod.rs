//! Weapon archetypes: describe a gun or a melee weapon by its family, action, rate of fire and calibre and
//! get numbers that fit the game.
//!
//! The model has three parts.
//!
//! - The taxonomy ([`data`]) is RimStudio's own JSON: families such as pistol, submachine gun, shotgun,
//!   rifle, machine gun, bow, knife, sword, axe, mace, spear and club, each with archetypes (assault rifle,
//!   sniper rifle, ...), the allowed actions and fire modes, and the shape of the weapon as ratios to the
//!   medians of the user's install. It holds no game values.
//! - The solver ([`propose`], with [`gun`] and [`blade`]) turns an archetype, descriptors and a balance target
//!   into every number of the weapon: the shape comes from the ratios times the install's medians, then one
//!   scale is found by bisection so that the strength index lands on the percentile the balance target names
//!   among the install's weapons of the class. Nothing is stored: the pools are read from the install at run
//!   time and a thin pool falls back along the calibration chain and says so.
//! - The explanations: every number carries a plain reason and the multipliers behind it, and the source chip
//!   `archetype`.
//!
//! [`apply`] writes a proposal into a spec through the offer rule, so typed values are never overwritten.
//! [`derive`] names the archetype of an existing weapon from its numbers, which the validation harness uses to
//! compare proposals with the real weapons. Combat Extended is optional: [`ce`] reads the calibres from the
//! user's ammo sets and only in Combat Extended mode.

pub mod apply;
pub mod basis;
pub mod blade;
pub mod ce;
pub mod costs;
pub mod data;
pub mod derive;
pub mod gun;
pub mod parts;
pub mod proposal;
pub mod propose;
pub mod resolve;
pub mod verify;

pub use apply::{ApplyOptions, ApplyReport, apply, choice_of};
pub use basis::{Basis, Med};
pub use ce::{CeCalibre, CeInfo, ce_info};
pub use costs::{CostBasis, CostRow};
pub use data::{Archetype, ArchetypeRef, Family, Taxonomy};
pub use derive::{Derived, derive_melee, derive_ranged};
pub use proposal::{
    CeProposal, FactorTerm, Proposal, ProposedCost, ProposedStuff, ProposedTool, ProposedValue,
    ResolvedChoice, SOURCE_CHIP, StrengthReport,
};
pub use propose::{Context, ProposeRequest, propose, propose_with};
pub use verify::{Verdict, fit_of, verdict};
