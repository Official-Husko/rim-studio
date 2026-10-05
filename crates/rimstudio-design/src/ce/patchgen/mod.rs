//! The Combat Extended patch generator: a pure function from a design, a model of the user's Combat
//! Extended and the state of the target def to JSON node trees of patch operations.
//!
//! Owner rule (D-085): the designer always writes vanilla definitions; Combat Extended is an optional patch
//! that the user asks for. This module never touches a vanilla definition, produces nothing unless the spec
//! carries a Combat Extended block (`spec.ce` is `Some`), and puts everything it produces into its own patch
//! files under the gated CE folder (see [`export`]).
//!
//! # Entry points
//!
//! - [`gun_patch`] and [`melee_patch`] turn a [`DesignSpec`] into a [`GeneratedPatch`]: the operations of one
//!   item in emission order (the `MakeGun` operation first for guns, then ensure containers, adds, replaces).
//!   When the target already carries a conversion ([`Container::existing`]) the patch is in update mode:
//!   per field Replace operations, never a second `MakeGun` (IT-057). [`apparel_patch`] is deferred.
//! - [`export_ce_plan`] and [`export_ce_plan_with`] build the write plan: the patch file and the
//!   `LoadFolders.xml` edit, and nothing else (the toolkit merges it with the vanilla plan when the user
//!   opted in).
//! - [`scan`] and [`convert`] are the Flow D support: list the convertible weapons of a project and convert
//!   one, asking for what cannot be derived ([`AskList`]).
//! - [`dry_apply`] and [`MakeGunCeSimulation`] run a generated patch through the def engine on a scratch copy
//!   of the defs (IT-056).
//!
//! # Where the numbers come from
//!
//! The Combat Extended block of the spec holds the values the user decided. What it leaves open is derived
//! from the user's own conversions by [`crate::ce::classes`] and listed in [`GeneratedPatch::derived`]; no
//! number is a constant of this module (R11).
//!
//! The functions are total: an invalid input gives diagnostics and no operations, never a panic, and the
//! same input gives the same output.

pub mod bow;
pub mod container;
pub mod conventions;
pub mod convert;
pub mod export;
pub mod extras;
pub mod folders;
pub mod gun;
pub mod melee;
pub mod ops;
pub mod platform;
pub mod simulate;
pub mod update;
pub mod values;

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::tree::{Node, NodeBuilder};
use thiserror::Error;

use crate::error::DesignError;
use crate::model::{DesignSpec, ItemKind};

pub use bow::{bow_patch, is_bow_spec};
pub use container::{Container, ConversionSource, ExistingConversion};
pub use convert::{
    AskItem, AskKind, AskList, ConvertAnswers, ConvertCandidate, ConvertEnv, ConvertOutcome,
    ConvertStatus, convert, derive_ce_block, family_key, scan,
};
pub use export::{CeProjectState, export_ce_plan, export_ce_plan_with, gate_violations};
pub use folders::{LoadFoldersOutcome, load_folders_plan};
pub use gun::gun_patch;
pub use melee::melee_patch;
pub use simulate::{MakeGunCeSimulation, dry_apply};
pub use values::{DerivedValue, ValueOrigin};

/// Errors of the generator. Content problems are diagnostics; an error is a feature that is not built or a
/// number the math refused.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PatchgenError {
    /// The item kind is not available in this release.
    #[error("{what} is deferred in this release")]
    Deferred {
        /// What is deferred.
        what: &'static str,
    },
    /// A number was rejected by the math layer.
    #[error(transparent)]
    Design(#[from] DesignError),
}

impl PatchgenError {
    /// The stable code: `design.deferred`, or the code of the wrapped design error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Deferred { .. } => "design.deferred",
            Self::Design(e) => e.code(),
        }
    }
}

/// Result alias of the generator.
pub type PatchgenResult<T> = Result<T, PatchgenError>;

/// How a patch relates to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchMode {
    /// The target has no conversion: a `MakeGun` for guns, plain operations for melee weapons.
    New,
    /// The target already carries a conversion: per field Replace operations.
    Update,
    /// Nothing was generated (the toggle is off, Combat Extended is absent or the input is invalid).
    Off,
}

/// The patch file category of an item, which names the file (`Weapons_Ranged.xml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchCategory {
    /// Guns and bows.
    WeaponsRanged,
    /// Melee weapons.
    WeaponsMelee,
}

impl PatchCategory {
    /// The category name used in file names.
    #[must_use]
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::WeaponsRanged => "Weapons_Ranged",
            Self::WeaponsMelee => "Weapons_Melee",
        }
    }

    /// The category of an item kind.
    #[must_use]
    pub fn of(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Ranged => Self::WeaponsRanged,
            ItemKind::Melee => Self::WeaponsMelee,
        }
    }
}

/// The operations generated for one item.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPatch {
    /// How the patch relates to the target.
    pub mode: PatchMode,
    /// The file category.
    pub category: PatchCategory,
    /// The def the operations target.
    pub def_name: String,
    /// The `Operation` elements in emission order.
    pub operations: Vec<Node>,
    /// Problems and hints. An error diagnostic means `operations` is empty.
    pub diagnostics: Vec<Diagnostic>,
    /// The numbers written that the Combat Extended block did not hold, with their origin.
    pub derived: Vec<DerivedValue>,
    /// In update mode, where the existing conversion lives. A foreign or unknown source is never rewritten;
    /// for a RimStudio or project file the toolkit may splice the changed values into that file instead of
    /// writing the override file.
    pub source: Option<ConversionSource>,
}

impl GeneratedPatch {
    /// An empty patch with diagnostics.
    #[must_use]
    pub fn off(
        spec: &DesignSpec,
        diagnostics: Vec<Diagnostic>,
        mode_hint: Option<PatchMode>,
    ) -> Self {
        Self {
            mode: mode_hint.unwrap_or(PatchMode::Off),
            category: PatchCategory::of(spec.kind),
            def_name: spec.identity.def_name.clone(),
            operations: Vec::new(),
            diagnostics,
            derived: Vec::new(),
            source: None,
        }
    }

    /// True when a diagnostic is an error.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// The gun conversion operation of the patch, wherever it sits: the operation itself, or the branch of
    /// the conditional that guards it. `None` for a melee patch, an update or an empty patch.
    #[must_use]
    pub fn gun_conversion(&self, classes: &crate::ce::reader::CeClassNames) -> Option<&Node> {
        fn find<'a>(op: &'a Node, class: &str) -> Option<&'a Node> {
            if op.attr("Class").is_some_and(|c| c == class) {
                return Some(op);
            }
            ["match", "nomatch"]
                .into_iter()
                .filter_map(|t| op.child(t))
                .find_map(|b| find(b, class))
        }
        self.operations
            .iter()
            .find_map(|op| find(op, &classes.make_gun_op))
    }

    /// The `Patch` root element holding the operations.
    #[must_use]
    pub fn patch_root(&self) -> Node {
        let mut root = NodeBuilder::new("Patch").build();
        for op in &self.operations {
            root.push_child(op.clone());
        }
        root
    }
}

/// The patch of an apparel item. Apparel is deferred in this release.
///
/// # Errors
///
/// Always [`PatchgenError::Deferred`] (code `design.deferred`).
pub fn apparel_patch(
    _spec: &DesignSpec,
    _model: &crate::ce::reader::CeModel,
    _container: &Container,
) -> PatchgenResult<GeneratedPatch> {
    Err(PatchgenError::Deferred {
        what: "the Combat Extended patch of apparel",
    })
}

/// The exact `About` name of the installed Combat Extended, the string `FindMod` compares (case sensitive,
/// untrimmed). It is a different thing from [`ce_package_id`]; the two are never mixed (IT-055).
#[must_use]
pub fn ce_name(model: &crate::ce::reader::CeModel) -> &str {
    model
        .names
        .as_ref()
        .map_or(DEFAULT_CE_NAME, |n| n.name.as_str())
}

/// The package id of the installed Combat Extended in the form gates use: lower case (the game lower cases
/// both sides). Falls back to the surveyed id when the load order did not name the mod.
#[must_use]
pub fn ce_package_id(model: &crate::ce::reader::CeModel) -> String {
    model
        .names
        .as_ref()
        .map_or(crate::ce::reader::CE_PACKAGE_ID, |n| n.package_id.as_str())
        .to_ascii_lowercase()
}

/// The mod name used when the load order did not name the installed Combat Extended.
pub const DEFAULT_CE_NAME: &str = "Combat Extended";

/// The namespace prefix of every Combat Extended class.
const CE_NAMESPACE: &str = "CombatExtended.";

/// True when an operation or any operation nested in it uses a Combat Extended class.
fn uses_ce_operation_class(op: &Node) -> bool {
    if op
        .attr("Class")
        .is_some_and(|c| c.starts_with(CE_NAMESPACE))
    {
        return true;
    }
    let nested = ["match", "nomatch"]
        .into_iter()
        .filter_map(|t| op.child(t))
        .chain(
            op.child("operations")
                .into_iter()
                .flat_map(|o| o.children_named("li")),
        );
    nested.into_iter().any(uses_ce_operation_class)
}

/// The fallback for a mod that must not use `LoadFolders.xml`: every operation wrapped in a `FindMod` on the
/// exact mod name ([`ce_name`]), which is safe only for operations of vanilla classes. A patch that holds a
/// Combat Extended operation class (the gun conversion) is refused with `ce.cep004`, because such a class
/// fails to load in a file that loads without Combat Extended. The wrapped operations still contain
/// Combat Extended class names inside their values (converted tools), which only take effect when the
/// operation runs.
#[must_use]
pub fn find_mod_fallback(
    patch: &GeneratedPatch,
    model: &crate::ce::reader::CeModel,
) -> GeneratedPatch {
    let mut out = patch.clone();
    if patch.operations.iter().any(uses_ce_operation_class) {
        out.operations.clear();
        out.diagnostics.push(
            crate::ce::lint::codes::CEP004
                .diagnostic("", &[("class", model.classes.make_gun_op.as_str())]),
        );
        return out;
    }
    out.operations = patch
        .operations
        .iter()
        .map(|op| ops::find_mod(ce_name(model), op.clone()))
        .collect();
    out
}

/// The common guard of the item functions: the toggle, the model and the spec's own checks. `Err` is the
/// finished empty patch; `Ok` carries the warnings of the checks. With `update` set (the target already
/// carries a conversion) the required field checks are skipped.
pub(crate) fn guard(
    spec: &DesignSpec,
    model: &crate::ce::reader::CeModel,
    kind: ItemKind,
    update: bool,
) -> Result<Vec<Diagnostic>, Box<GeneratedPatch>> {
    if spec.ce.is_none() {
        return Err(Box::new(GeneratedPatch::off(spec, Vec::new(), None)));
    }
    if !model.is_present() {
        return Err(Box::new(GeneratedPatch::off(
            spec,
            vec![crate::validation::codes::CE_ABSENT.diagnostic("", &[])],
            None,
        )));
    }
    if spec.kind != kind {
        return Err(Box::new(GeneratedPatch::off(
            spec,
            vec![crate::validation::codes::VALUE_INVALID.diagnostic(
                "/kind",
                &[
                    ("label", "the item kind"),
                    ("value", spec.kind.as_str()),
                    ("reason", "this generator handles another kind"),
                ],
            )],
            None,
        )));
    }
    let mut diagnostics = crate::validation::validate_ce_patch(spec);
    if update {
        // An existing conversion supplies what the block leaves open; only the fields it names are changed.
        diagnostics.retain(|d| d.code.as_str() != crate::validation::codes::REQUIRED_MISSING.code);
    }
    if crate::validation::has_errors(&diagnostics) {
        return Err(Box::new(GeneratedPatch::off(spec, diagnostics, None)));
    }
    Ok(diagnostics)
}

#[cfg(test)]
mod tests;
