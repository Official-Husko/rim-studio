//! Def merge, patch, inherit, provenance and def databases on the node tree.
//!
//! Layer `l2-engine`. A faithful port of the game's def pipeline (research note
//! `def-engine-semantics`) onto [`rimstudio_core::tree`], with no XML dependency and no IO: the
//! caller hands over files that the boundary crate has already read and parsed.
//!
//! The stages, each usable alone, and [`build::load`] chaining them:
//!
//! 1. [`merge::merge`]: every `Defs` file under one `Defs` root in load order, with an origin
//!    (mod and file) on every node ([`merge::UnifiedDoc`]);
//! 2. [`patch_ops::parse_patch_file`] and [`apply::apply_patches`]: the 13 vanilla patch classes,
//!    unknown classes (kept with their raw parameters) and custom handlers
//!    ([`custom_ops::CustomPatchOp`]), XPath through `rimstudio-xpath` with a `defName` index
//!    ([`index::DefIndex`]), per operation isolation and results ([`apply::PatchReport`]);
//! 3. [`inherit::resolve_inheritance`]: parent choice by load order, abstract bases, `Inherit`,
//!    list rules, cycle reports ([`inherit::ResolvedDoc`]);
//! 4. [`build::build_defs_with`] and [`build::DefDatabases`]: defs typed through a
//!    [`type_table::TypeTable`], per type databases with the game's override rules.
//!
//! Content problems are diagnostics with the stable codes of [`diag_codes`]; they never abort a
//! load. Output is deterministic and identical for any thread count.
//!
//! ```
//! use std::sync::Arc;
//! use rimstudio_core::ids::{FileId, ModIdx};
//! use rimstudio_core::tree::NodeBuilder;
//! use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, TypeInfo, TypeTable, load};
//!
//! let types = TypeTable::new(vec![
//!     ("Verse.Def".into(), TypeInfo::with_base("Verse.Editable")),
//!     ("Verse.RS_ThingDef".into(), TypeInfo::with_base("Verse.Def")),
//! ])?;
//! let mut input = LoadInput::new(
//!     vec![ModEntry::new(ModIdx(0), "rs.example", "RS Example")],
//!     Arc::new(types),
//! );
//! input.def_files.push(DefFile {
//!     mod_idx: ModIdx(0),
//!     file: FileId(0),
//!     rel_path: "Defs/RS_Items.xml".into(),
//!     content: FileContent::parsed(
//!         NodeBuilder::new("Defs")
//!             .child(NodeBuilder::new("RS_ThingDef").text_elem("defName", "RS_Thing"))
//!             .build(),
//!     ),
//! });
//! let out = load(input);
//! assert_eq!(out.stats.defs, 1);
//! assert!(out.get("RS_ThingDef", "RS_Thing").is_some());
//! # Ok::<(), rimstudio_defs::DefsError>(())
//! ```

pub mod apply;
pub mod assembly;
pub mod build;
pub mod custom_ops;
pub mod diag_codes;
pub mod error;
pub mod index;
pub mod inherit;
pub mod merge;
pub mod patch_ops;
pub mod provenance;
pub mod type_table;
pub mod types;

pub use apply::{PatchContext, PatchCtx, PatchEvent, PatchReport, apply_patches};
pub use build::{
    BuildOutput, DatabaseView, DefDatabases, Duplicate, LoadInput, LoadOutput, LoadTimings,
    Override, build_defs, build_defs_with, load,
};
pub use custom_ops::{CustomPatchOp, CustomRegistry, SettingsConditional};
pub use error::{DefsError, DefsResult, PatchError};
pub use inherit::{
    InheritConfig, ResolvedDoc, ResolvedNode, prune_li_may_require, resolve_inheritance,
};
pub use merge::{UnifiedDoc, merge};
pub use patch_ops::{
    Order, ParseEnv, PatchKind, PatchOp, Success, parse_operation, parse_patch_file, parse_patches,
};
pub use provenance::{ModEntry, ModOrder, OriginKind, OriginTable};
pub use type_table::{TypeInfo, TypeTable};
pub use types::{
    DefFile, DefRecord, FileContent, LoadStats, Origin, ParentRef, PatchFile, PatchRef,
};
