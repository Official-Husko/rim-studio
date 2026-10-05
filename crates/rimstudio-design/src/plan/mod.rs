//! Write plans: the files a design would produce, as pure data with JSON node trees.
//!
//! - [`types`]: [`WritePlan`], [`PlannedFile`], [`FileAction`], section comment headers and edits.
//! - [`builder`]: [`PlanBuilder`], the extension point for other generators (the CE patch task adds its
//!   files through [`PlanBuilder::add_file`]), and the path safety guard.
//! - [`layout`]: [`ProjectLayout`], the file layout rule (RimStudio mod layout v1).
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
pub use layout::{LayoutProfile, ProjectLayout, SoundFile, WeaponCategory, file_stem};
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

/// Builds the plan of the vanilla definition file of a spec: one file that holds the weapon and, when the
/// spec creates a projectile, that projectile before it. The path follows the layout
/// ([`ProjectLayout::weapon_def_path`]): by default
/// `Defs/ThingDefs_Misc/Weapons/<Category>/<defName>.xml` (inside the version folder when the layout has
/// one), where the category comes from the kind and the tech level ([`WeaponCategory::of`]). Each def has a
/// section header `====== <defName> ======`.
///
/// Rules:
///
/// - The result is pure: the same spec and layout give an identical plan.
/// - Validation runs first ([`validate_vanilla`] plus the layout check). When any diagnostic is an error the
///   plan has no files and carries the diagnostics (IT-030, IT-041); warnings and info never block.
/// - Every file has action [`FileAction::Create`]; the toolkit compares with the disk and turns a file into
///   `Unchanged` or `UpdateRegion`. In a project that keeps one file per category the path is that shared
///   file and the toolkit appends the marked sections to it.
/// - A weapon without a texture path gets the reserved one ([`ProjectLayout::weapon_texture_path`]) and an
///   info diagnostic names the file where the art goes; a typed texture path is kept.
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
    let mut spec = spec.clone();
    if spec.texture_path.is_none() {
        let reserved = layout.weapon_texture_path(spec.kind, def_name);
        diagnostics.push(codes::TEXTURE_RESERVED.diagnostic(
            "/texturePath",
            &[
                ("label", &spec.identity.label),
                ("path", &layout.texture_file(&reserved)),
            ],
        ));
        spec.texture_path = Some(reserved);
    }
    let spec = &spec;
    let category = WeaponCategory::of(spec.kind, spec.tech_level);

    let mut defs = Vec::new();
    let mut sections = Vec::new();
    if let Some(projectile) = projectile_def(spec) {
        let name = projectile
            .child_text("defName")
            .unwrap_or_default()
            .to_owned();
        sections.push(SectionHeader::banner(defs.len(), &name));
        defs.push(projectile);
    }
    sections.push(SectionHeader::banner(defs.len(), def_name));
    defs.push(weapon_def(spec));
    let files = vec![PlannedFile::new_file(
        layout.weapon_def_path(category, def_name),
        FileKind::VanillaDefs,
        defs_root(defs),
        sections,
    )];

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
