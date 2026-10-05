//! The dry run: a generated patch applied to a scratch copy of the defs by the app's own patch engine.
//!
//! The shared def engine keeps the gun conversion operation as an unknown class (decision D-020). For the
//! designer's dry run, [`MakeGunCeSimulation`] is registered as a custom operation in the scratch context
//! only: it applies the documented merge of [`crate::ce::reader::makegun`] to the target def and reports the
//! parameters it cannot apply. [`dry_apply`] runs whole patch files and turns every top level operation that
//! the game would log as failed into a `ce.dry-run-failed` error (IT-056, CP-013).

use rimstudio_core::diag::{DiagCode, DiagSink, Diagnostic, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::Node;
use rimstudio_defs::{
    CustomPatchOp, CustomRegistry, DefFile, FileContent, ModEntry, ModOrder, ParseEnv,
    PatchContext, PatchCtx, PatchError, UnifiedDoc, apply_patches, merge, parse_patch_file,
};

use crate::ce::reader::{CE_PACKAGE_ID, CeClassNames, CeModel, MakeGunOp, MakeGunSpec};
use crate::validation::codes::CE_DRY_RUN_FAILED;

/// Diagnostic: a parameter of the gun conversion is parsed but not applied by the simulation.
pub const SIMULATION_UNSUPPORTED: &str = "ce.simulation-unsupported";

/// The gun conversion operation as a custom patch operation of the def engine, for dry runs.
///
/// It behaves like [`MakeGunOp`] (the typed merge) and in addition reports the parameters that the merge
/// parses but does not apply as warnings, so a dry run never claims more than it did. The weapon platform
/// parameters are applied by the merge, so none is reported at present.
#[derive(Debug, Clone)]
pub struct MakeGunCeSimulation {
    inner: MakeGunOp,
    class: String,
}

impl MakeGunCeSimulation {
    /// A simulation for the class names given.
    #[must_use]
    pub fn new(classes: &CeClassNames) -> Self {
        Self {
            class: classes.make_gun_op.clone(),
            inner: MakeGunOp::new(classes.clone()),
        }
    }
}

impl Default for MakeGunCeSimulation {
    fn default() -> Self {
        Self::new(&CeClassNames::default())
    }
}

impl CustomPatchOp for MakeGunCeSimulation {
    fn class(&self) -> &str {
        &self.class
    }

    fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        let spec = MakeGunSpec::parse(&ctx.raw);
        for name in &spec.unsupported {
            ctx.emit(
                DiagCode::new(SIMULATION_UNSUPPORTED),
                Severity::Warning,
                format!("the parameter {name} of the gun conversion is not simulated"),
            );
        }
        self.inner.apply(doc, ctx)
    }
}

/// The result of a dry run.
#[derive(Debug, Clone, PartialEq)]
pub struct DryRun {
    /// One `ce.dry-run-failed` error per top level operation that failed or raised, in application order,
    /// plus the warnings of the simulation.
    pub diagnostics: Vec<Diagnostic>,
    /// The top level nodes of the document after patching, in document order.
    pub defs: Vec<Node>,
    /// Top level operations applied.
    pub applied: usize,
    /// Top level operations that failed.
    pub failed: usize,
}

impl DryRun {
    /// The patched def with the given element name and `defName`.
    #[must_use]
    pub fn def(&self, tag: &str, def_name: &str) -> Option<&Node> {
        self.defs
            .iter()
            .find(|n| n.tag == tag && n.child_text("defName") == Some(def_name))
    }

    /// True when every operation applied.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failed == 0
            && !self
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error)
    }
}

/// Applies patch files to a scratch copy of the defs.
///
/// - `defs` are the top level def nodes of the scratch set (the target and what it refers to), as one mod.
/// - `patch_files` are `Patch` roots, in application order, as a second mod that loads after the defs.
/// - `model` supplies the class names and the Combat Extended mod name and package id for `FindMod` gates.
#[must_use]
pub fn dry_apply(defs: &[Node], patch_files: &[Node], model: &CeModel) -> DryRun {
    let ce_id = model
        .names
        .as_ref()
        .map_or(CE_PACKAGE_ID, |n| n.package_id.as_str())
        .to_owned();
    let ce_name = model
        .names
        .as_ref()
        .map_or("Combat Extended", |n| n.name.as_str())
        .to_owned();
    let order = ModOrder::new(vec![
        ModEntry::new(ModIdx(0), "rs.scratch.defs", "Scratch defs"),
        ModEntry::new(ModIdx(1), "rs.scratch.patch", "Scratch patch"),
        ModEntry::new(ModIdx(2), ce_id.clone(), ce_name),
    ]);
    let mut root = Node::new("Defs");
    for d in defs {
        root.push_child(d.clone());
    }
    let files = vec![DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/Scratch.xml".to_owned(),
        content: FileContent::parsed(root),
    }];
    let mut doc = merge(&order, &files);
    let mut registry = CustomRegistry::new();
    registry.register(MakeGunCeSimulation::new(&model.classes));
    registry.register(crate::ce::reader::FindModOp);
    let ctx = PatchContext::for_mods(&order, &[ce_id], registry);
    let env = ParseEnv {
        registry: &ctx.custom,
        active: &ctx.active,
    };
    let mut sink = DiagSink::new();
    let mut ops = Vec::new();
    for (i, file) in patch_files.iter().enumerate() {
        let id = FileId(u32::try_from(i.saturating_add(1)).unwrap_or(u32::MAX));
        ops.extend(parse_patch_file(
            file,
            &env,
            Some(ModIdx(1)),
            Some(id),
            &mut sink,
        ));
    }
    let report = apply_patches(&mut doc, &[(ModIdx(1), ops)], &ctx);
    let mut diagnostics = Vec::new();
    let mut applied = 0usize;
    let mut failed = 0usize;
    for event in &report.events {
        if event.result {
            applied = applied.saturating_add(1);
        } else {
            failed = failed.saturating_add(1);
            let reason = event.error.clone().unwrap_or_else(|| {
                "the operation found nothing to change or reported failure".to_owned()
            });
            diagnostics.push(
                CE_DRY_RUN_FAILED
                    .diagnostic(
                        "",
                        &[("operation", &event.description), ("reason", &reason)],
                    )
                    .with_arg("index", event.index.to_string()),
            );
        }
    }
    // The simulation's own warnings (parameters it does not apply) travel with the result.
    for d in report.diagnostics.finish().samples {
        if d.code.as_str() == SIMULATION_UNSUPPORTED {
            diagnostics.push(d);
        }
    }
    let defs = doc
        .top_level()
        .into_iter()
        .filter_map(|id| doc.arena().to_node(id))
        .collect();
    DryRun {
        diagnostics,
        defs,
        applied,
        failed,
    }
}
