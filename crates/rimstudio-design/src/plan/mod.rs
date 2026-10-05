//! Write plans: the files a design would produce, as pure data with JSON node trees.
//!
//! - [`types`]: [`WritePlan`], [`PlannedFile`], [`FileAction`], section comment headers and edits.
//! - [`builder`]: [`PlanBuilder`], the extension point for other generators (the CE patch task adds its
//!   files through [`PlanBuilder::add_file`]), and the path safety guard.
//! - [`layout`]: [`ProjectLayout`], the file layout rule.
//! - [`vanilla`]: the node trees of vanilla weapons and projectiles.
//!
//! [`export_ce_plan`] (re-exported from `ce::patchgen`) is the entry point of the optional Combat Extended
//! patch: only its files and the `LoadFolders.xml` edit, never a vanilla file.
//!
//! [`export_vanilla_plan`] is the entry point for the vanilla definition. It never emits a Combat
//! Extended file or class, even when the spec carries a CE patch: the CE plan is a separate function of the
//! CE patch generator that adds its files to the same [`PlanBuilder`].

pub mod builder;
pub mod layout;
pub mod types;
pub mod vanilla;

use rimstudio_core::tree::Node;

use crate::model::DesignSpec;
use crate::validation::{codes, has_errors, validate_vanilla};

pub use crate::ce::patchgen::{
    CeProjectState, export_ce_plan, export_ce_plan_with, gate_violations,
};
pub use builder::{PlanBuilder, is_safe_relative_path};
pub use layout::{ProjectLayout, file_stem};
pub use types::{
    FileAction, FileKind, PlannedFile, SectionGroup, SectionHeader, TextEdit, WritePlan,
};
pub use vanilla::{
    TEMPLATE_VANILLA_MELEE, TEMPLATE_VANILLA_RANGED, melee_def, projectile_def, ranged_def,
    weapon_def,
};

/// The text that marks a Combat Extended class. No file of a vanilla plan contains it.
const CE_CLASS_MARK: &str = "CombatExtended";

/// A `Defs` root holding the given definitions.
fn defs_root(defs: Vec<Node>) -> Node {
    let mut root = Node::new("Defs");
    for d in defs {
        root.push_child(d);
    }
    root
}

/// Builds the plan of the vanilla definition files of a spec: the weapon at
/// `Defs/Weapons/<defName>.xml` (inside the version folder when the layout has one) and, when the spec
/// creates a projectile, that projectile at `Defs/Projectiles/<defName>.xml`. Each file is a `Defs` tree
/// with one section header `====== <defName> ======`.
///
/// Rules:
///
/// - The result is pure: the same spec and layout give an identical plan.
/// - Validation runs first ([`validate_vanilla`] plus the layout check). When any diagnostic is an error the
///   plan has no files and carries the diagnostics (IT-030, IT-041); warnings and info never block.
/// - Every file has action [`FileAction::Create`]; the toolkit compares with the disk and turns a file into
///   `Unchanged` or `UpdateRegion`.
/// - Nothing about Combat Extended is written, whatever `spec.ce` holds. As a second guard, a tree that
///   contains `CombatExtended` anywhere turns the plan into an error plan.
#[must_use]
pub fn export_vanilla_plan(spec: &DesignSpec, layout: &ProjectLayout) -> WritePlan {
    let mut builder = PlanBuilder::new();
    let mut diagnostics = validate_vanilla(spec);
    diagnostics.extend(layout.validate());
    if has_errors(&diagnostics) {
        builder.extend_diagnostics(diagnostics);
        return builder.build();
    }

    let def_name = spec.identity.def_name.as_str();
    let weapon = PlannedFile::new_file(
        layout.weapon_def_path(def_name),
        FileKind::VanillaDefs,
        defs_root(vec![weapon_def(spec)]),
        vec![SectionHeader::banner(0, def_name)],
    );
    let mut files = vec![weapon];
    if let Some(projectile) = projectile_def(spec) {
        let name = projectile
            .child_text("defName")
            .unwrap_or_default()
            .to_owned();
        files.push(PlannedFile::new_file(
            layout.projectile_def_path(&name),
            FileKind::VanillaDefs,
            defs_root(vec![projectile]),
            vec![SectionHeader::banner(0, &name)],
        ));
    }

    for file in &files {
        if let Some(tree) = &file.tree
            && tree.validate().is_err()
        {
            diagnostics.push(codes::VALUE_INVALID.diagnostic(
                "",
                &[
                    ("label", &file.path),
                    ("value", ""),
                    ("reason", "the generated tree is not valid XML structure"),
                ],
            ));
        }
        if file.contains_text(CE_CLASS_MARK) {
            diagnostics.push(
                codes::CE_IN_VANILLA
                    .diagnostic("", &[("label", &file.path), ("value", CE_CLASS_MARK)]),
            );
        }
    }
    if has_errors(&diagnostics) {
        builder.extend_diagnostics(diagnostics);
        return builder.build();
    }
    for file in files {
        builder.add_file(file);
    }
    builder.extend_diagnostics(diagnostics);
    builder.build()
}
