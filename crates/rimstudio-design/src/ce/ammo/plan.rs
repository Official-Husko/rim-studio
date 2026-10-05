//! The ammunition part of the Combat Extended plan: the definition file of a custom caliber and the effective
//! Combat Extended block of the weapon that fires it.
//!
//! [`prepare`] is called by the Combat Extended export when the block carries a custom ammo spec. It checks
//! the spec ([`super::validate`]), generates the definitions ([`super::generate`]), lints them
//! ([`super::lint`]) and loads them through the def engine ([`super::dryload`]). When all of that is clean
//! it returns the definition file (kind [`FileKind::CeDefs`], one file per caliber in
//! `[<version>/]<ce folder>/Defs/Ammo/<prefix>_<Name>.xml`, one marked section per def) and a copy of the
//! spec and the model in which the weapon uses the custom set and the projectile of the default type. The
//! file exists only when the switch is on (D-085): without a Combat Extended block nothing is planned.

use std::collections::BTreeSet;

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::codes::SET_OVERRIDDEN;
use super::dryload::dry_load;
use super::generate::generate;
use super::lint::lint_defs;
use super::validate::validate_custom_ammo;
use crate::ce::patchgen::CeProjectState;
use crate::ce::reader::{AmmoSetInfo, AmmoType, CeModel, ProjectileInfo};
use crate::model::{CePatchSpec, CustomAmmoSpec, DesignSpec};
use crate::plan::{
    FileKind, PlanBuilder, PlannedFile, ProjectLayout, WritePlan, is_safe_relative_path,
};
use crate::validation::has_errors;

/// The path of the definition file of a custom caliber: `[<version>/]<ce folder>/Defs/Ammo/<stem>.xml`.
#[must_use]
pub fn ammo_file_path(layout: &ProjectLayout, stem: &str) -> String {
    layout.in_version(&format!(
        "{}/{}/Ammo/{stem}.xml",
        layout.ce_folder,
        rimstudio_core::paths::DEFS_DIR
    ))
}

/// What the Combat Extended export needs after the ammunition was prepared.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// The spec with the custom ammo spec removed and the ammo set and default projectile of the weapon
    /// set to the custom ones.
    pub spec: DesignSpec,
    /// The model with the custom set added, so the conversion finds it.
    pub model: CeModel,
    /// The definition file; empty when preparing failed.
    pub files: Vec<PlannedFile>,
    /// Everything found: checks, hints, lint and dry load.
    pub diagnostics: Vec<Diagnostic>,
    /// True when an error stops the plan.
    pub blocked: bool,
}

fn model_with_set(
    model: &CeModel,
    custom: &CustomAmmoSpec,
    set: &str,
    ammo: &[String],
    projectiles: &[String],
) -> CeModel {
    let mut m = model.clone();
    let types = custom
        .types
        .iter()
        .zip(ammo.iter().zip(projectiles.iter()))
        .map(|(t, (a, p))| {
            let n = |v: Option<crate::model::Sourced<f64>>| v.map(|s| s.value);
            AmmoType {
                ammo: a.clone(),
                projectile: p.clone(),
                info: ProjectileInfo {
                    damage: n(t.projectile.damage),
                    ap_sharp: n(t.projectile.armor_penetration_sharp),
                    ap_blunt: n(t.projectile.armor_penetration_blunt),
                    speed: n(t.projectile.speed),
                    pellets: n(t.projectile.pellet_count),
                    explosion_radius: n(t.projectile.explosion_radius),
                },
            }
        })
        .collect();
    m.ammo_sets.push(AmmoSetInfo {
        def_name: set.to_owned(),
        similar_to: custom.similar_to.clone(),
        ammo_types: types,
    });
    m
}

/// The effective Combat Extended block of a weapon with custom ammunition: the custom ammo set as the ammo
/// set and the projectile of the default type as the default projectile. `None` when the block has no custom
/// ammo spec. A typed ammo set that differs gets a warning.
#[must_use]
pub fn effective_ce_block(
    ce: &CePatchSpec,
    set: &str,
    default_projectile: Option<&str>,
) -> (CePatchSpec, Vec<Diagnostic>) {
    let mut block = ce.clone();
    let mut diagnostics = Vec::new();
    if let Some(typed) = block
        .ammo_set
        .as_deref()
        .filter(|t| !t.is_empty() && *t != set)
    {
        diagnostics
            .push(SET_OVERRIDDEN.diagnostic("/ce/ammoSet", &[("typed", typed), ("custom", set)]));
    }
    block.ammo_set = Some(set.to_owned());
    block.default_projectile = default_projectile.map(str::to_owned);
    block.custom_ammo = None;
    (block, diagnostics)
}

/// Prepares the ammunition of a spec; `None` when the spec has no custom ammo spec (the switch is off or
/// the block holds none).
#[must_use]
pub fn prepare(spec: &DesignSpec, model: &CeModel, layout: &ProjectLayout) -> Option<Prepared> {
    let ce = spec.ce.as_ref()?;
    let custom = ce.custom_ammo.as_ref()?;
    let prefix = spec.identity.mod_prefix.as_str();
    let mut diagnostics = validate_custom_ammo(custom, prefix, model);
    let mut prepared = Prepared {
        spec: spec.clone(),
        model: model.clone(),
        files: Vec::new(),
        diagnostics: Vec::new(),
        blocked: false,
    };
    if has_errors(&diagnostics) {
        prepared.blocked = true;
        prepared.diagnostics = diagnostics;
        return Some(prepared);
    }
    let generated = generate(custom, prefix, model, layout);
    diagnostics.extend(generated.diagnostics.iter().cloned());
    let mut root = Node::new("Defs");
    for d in &generated.defs {
        root.push_child(d.clone());
    }
    let path = ammo_file_path(layout, &generated.stem);
    diagnostics.extend(lint_defs(
        &[(Some(path.clone()), root.clone())],
        model,
        &BTreeSet::new(),
    ));
    diagnostics.extend(dry_load(&generated.defs, model).diagnostics);
    if has_errors(&diagnostics) || !is_safe_relative_path(&path) || root.validate().is_err() {
        prepared.blocked = true;
        prepared.diagnostics = diagnostics;
        return Some(prepared);
    }
    let (block, more) = effective_ce_block(
        ce,
        &generated.set_name,
        generated.default_projectile.as_deref(),
    );
    diagnostics.extend(more);
    prepared.spec.ce = Some(block);
    prepared.model = model_with_set(
        model,
        custom,
        &generated.set_name,
        &generated.ammo,
        &generated.projectiles,
    );
    prepared.files = vec![PlannedFile::new_file(
        path,
        FileKind::CeDefs,
        root,
        generated.sections,
    )];
    prepared.diagnostics = diagnostics;
    Some(prepared)
}

/// Plans the weapon conversion of a prepared spec with `export` (the Combat Extended export) and adds the
/// definition file. A plan with errors carries no ammunition file.
#[must_use]
pub fn export_with_ammo(
    prepared: Prepared,
    layout: &ProjectLayout,
    state: &CeProjectState,
    export: fn(&DesignSpec, &CeModel, &ProjectLayout, &CeProjectState) -> WritePlan,
) -> WritePlan {
    let mut builder = PlanBuilder::new();
    if prepared.blocked {
        builder.extend_diagnostics(prepared.diagnostics);
        return builder.build();
    }
    let inner = export(&prepared.spec, &prepared.model, layout, state);
    let failed = inner.has_errors();
    builder.extend_diagnostics(inner.diagnostics);
    builder.extend_diagnostics(prepared.diagnostics);
    for f in inner.files {
        builder.add_file(f);
    }
    if !failed {
        for f in prepared.files {
            builder.add_file(f);
        }
    }
    let plan = builder.build();
    let violations = crate::ce::patchgen::gate_violations(&plan, layout);
    if violations.is_empty() {
        return plan;
    }
    let mut out = PlanBuilder::new();
    out.extend_diagnostics(violations.iter().map(|path| {
        crate::ce::lint::codes::CEP004
            .diagnostic("", &[("class", "CombatExtended.")])
            .with_arg("path", path)
    }));
    out.build()
}

/// The ammunition plan of a spec on its own: the definition file and the diagnostics, for callers that want
/// the ammunition without the weapon conversion (the CLI, tests).
#[must_use]
pub fn ammo_plan(spec: &DesignSpec, model: &CeModel, layout: &ProjectLayout) -> WritePlan {
    let mut builder = PlanBuilder::new();
    if !model.is_present() {
        builder.extend_diagnostics(vec![
            crate::validation::codes::CE_ABSENT.diagnostic("", &[]),
        ]);
        return builder.build();
    }
    if let Some(p) = prepare(spec, model, layout) {
        builder.extend_diagnostics(p.diagnostics);
        if !p.blocked {
            for f in p.files {
                builder.add_file(f);
            }
        }
    }
    builder.build()
}
