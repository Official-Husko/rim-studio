//! Applying patch operations to the unified document.
//!
//! [`apply_patches`] runs the operations of all mods in load order, file by file, operation by
//! operation, each top-level operation isolated: an exception (or even a panic) inside one becomes
//! a diagnostic and a failed result, and the next operation still runs. The semantics follow the
//! game's `PatchOperation*` classes (see the research note): `success` rewrites results, `Insert`
//! reverses several value children, `Sequence` has no rollback, `Replace` and `Remove` work on a
//! snapshot of the targets, attribute and text nodes are valid targets where the game allows them.
//!
//! XPath is evaluated with the document node as context through `rimstudio-xpath`; the dominant
//! shape `Defs/Type[defName="x"]/...` is answered by the `defName` index and the rest of the
//! expression is evaluated from the found definitions. Node sets are evaluated eagerly (the game
//! iterates lazily; see the research note, section 5).
//!
//! Provenance: every top-level node whose subtree a patch touched records the operation
//! ([`UnifiedDoc::patched_by`]); nodes a patch creates carry the operation as their origin and
//! therefore have no mod.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use rimstudio_core::diag::{DiagCode, DiagSink, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::{Node, NodeId, NodeKind, OriginId};
use rimstudio_xpath::{XNode, XPath, XPathError};
use serde::Serialize;

use crate::custom_ops::CustomRegistry;
use crate::diag_codes::{self, Site};
use crate::error::PatchError;
use crate::merge::UnifiedDoc;
use crate::patch_ops::{Order, ParseEnv, PatchKind, PatchOp, parse_operation};
use crate::provenance::{ModOrder, OriginKind};
use crate::types::PatchRef;

/// What patch application consults besides the document: the active mods, their display names and
/// the custom operation handlers.
#[derive(Debug, Clone, Default)]
pub struct PatchContext {
    /// The active package ids (for `MayRequire` on list entries).
    pub active: ActiveSet,
    /// The display names of the active mods (`PatchOperationFindMod` compares them exactly).
    pub mod_names: BTreeSet<String>,
    /// Handlers for classes outside the vanilla set.
    pub custom: CustomRegistry,
}

impl PatchContext {
    /// A context from an active set and the active mod names, without custom handlers.
    pub fn new(active: ActiveSet, mod_names: impl IntoIterator<Item = String>) -> Self {
        Self {
            active,
            mod_names: mod_names.into_iter().collect(),
            custom: CustomRegistry::new(),
        }
    }

    /// The same context with custom handlers.
    #[must_use]
    pub fn with_custom(mut self, custom: CustomRegistry) -> Self {
        self.custom = custom;
        self
    }

    /// The context of a load: every mod of `order` is active, plus `extra_active_ids` (packages
    /// that count as active without a mod folder).
    pub fn for_mods(order: &ModOrder, extra_active_ids: &[String], custom: CustomRegistry) -> Self {
        let ids = order
            .iter()
            .map(|e| e.package_id.as_str())
            .chain(extra_active_ids.iter().map(String::as_str));
        Self {
            active: ActiveSet::from_ids(ids),
            mod_names: order.iter().map(|e| e.name.clone()).collect(),
            custom,
        }
    }
}

/// The result of one top-level patch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchEvent {
    /// The mod whose patch list holds the operation.
    pub mod_idx: ModIdx,
    /// The patch file, when known.
    pub file: Option<FileId>,
    /// The position of the operation in the mod's patch list.
    pub index: u32,
    /// The class as written (or the canonical name when `Class` is missing).
    pub class: String,
    /// `Class(xpath)` with white space collapsed.
    pub description: String,
    /// The final result after the `success` rewrite.
    pub result: bool,
    /// The exception text when the operation raised one or panicked.
    pub error: Option<String>,
    /// The raw `Operation` element when the class is unknown to the engine.
    pub raw: Option<Node>,
    /// How many top-level nodes the operation touched.
    pub touched_defs: u32,
}

/// The outcome of applying all patches.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchReport {
    /// One event per top-level operation, in application order.
    pub events: Vec<PatchEvent>,
    /// Microseconds spent per event (parallel to `events`; not part of the deterministic output).
    pub op_micros: Vec<u64>,
    /// Microseconds spent in total.
    pub total_micros: u64,
    /// Diagnostics found while applying (`defs.patch-failed` and friends).
    #[serde(skip_serializing)]
    pub diagnostics: DiagSink,
}

impl PatchReport {
    /// The results of the top-level operations in order.
    #[must_use]
    pub fn results(&self) -> Vec<bool> {
        self.events.iter().map(|e| e.result).collect()
    }

    /// Number of operations whose result was true.
    #[must_use]
    pub fn applied(&self) -> usize {
        self.events.iter().filter(|e| e.result).count()
    }

    /// Number of operations whose result was false.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.events.len().saturating_sub(self.applied())
    }

    /// Number of operations that raised an exception or panicked.
    #[must_use]
    pub fn exceptions(&self) -> usize {
        self.events.iter().filter(|e| e.error.is_some()).count()
    }

    /// The events without timing, for byte comparisons of two runs.
    #[must_use]
    pub fn deterministic_events(&self) -> &[PatchEvent] {
        &self.events
    }
}

/// The working state of one top-level operation, handed to custom handlers.
pub struct PatchCtx<'a> {
    /// The mod whose patch list holds the operation.
    pub mod_idx: ModIdx,
    /// The patch file, when known.
    pub file: Option<FileId>,
    /// The position of the top-level operation in the mod's patch list.
    pub op_index: u32,
    /// The `Operation` element of the custom operation being applied.
    pub raw: Node,
    /// The active package ids.
    pub active: &'a ActiveSet,
    /// The display names of the active mods.
    pub mod_names: &'a BTreeSet<String>,
    /// The registered custom handlers.
    pub registry: &'a CustomRegistry,
    diag: &'a mut DiagSink,
    touched: Vec<NodeId>,
    changed: Vec<NodeId>,
    top_changed: Vec<NodeId>,
    origin: Option<OriginId>,
}

impl PatchCtx<'_> {
    /// Where diagnostics of this operation point.
    #[must_use]
    pub fn site(&self) -> Site {
        Site::new(Some(self.mod_idx), self.file)
    }

    /// Records a diagnostic located at this operation's mod and file.
    pub fn emit(&mut self, code: DiagCode, severity: Severity, message: impl Into<String>) {
        let d = self.site().diag(code, severity, message);
        self.diag.push(d);
    }

    /// Records that the subtree around `id` was changed (provenance: `patched_by` of its
    /// top-level node).
    pub fn touch(&mut self, id: NodeId) {
        self.touched.push(id);
    }

    /// Records that the children of `container` changed, so the `defName` index can follow.
    /// Handlers that edit through [`UnifiedDoc::arena_mut`] do not need this.
    pub fn changed(&mut self, container: NodeId) {
        self.changed.push(container);
    }

    /// Records that a top-level node (a child of `Defs`) was added, removed or replaced, so the
    /// `defName` index can follow without a rebuild.
    pub fn top_level_changed(&mut self, id: NodeId) {
        self.top_changed.push(id);
    }

    /// Hands the recorded structural changes to the document's index now instead of at the end of the
    /// top-level operation, so that a later step of the same operation finds replaced or added defs.
    pub fn flush_index_notes(&mut self, doc: &mut UnifiedDoc) {
        for c in std::mem::take(&mut self.changed) {
            doc.note_changed(c);
        }
        for t in std::mem::take(&mut self.top_changed) {
            doc.note_top_level(t);
        }
    }

    /// The origin tag for nodes this operation creates (created on first use).
    pub fn origin(&mut self, doc: &mut UnifiedDoc) -> OriginId {
        if let Some(o) = self.origin {
            return o;
        }
        let o = doc.origins_mut().push(OriginKind::Patch {
            mod_idx: self.mod_idx,
            file: self.file,
            op_index: self.op_index,
        });
        self.origin = Some(o);
        o
    }

    /// Parses a nested `Operation`-like element (a `match`, `nomatch` or `li` of a custom class).
    pub fn parse_nested(&mut self, el: &Node) -> PatchOp {
        let env = ParseEnv {
            registry: self.registry,
            active: self.active,
        };
        let site = self.site();
        parse_operation(el, &env, site, self.file, self.diag)
    }

    /// Applies a nested operation, with its `success` rewrite.
    ///
    /// # Errors
    /// The nested operation's exception, which should usually be propagated.
    pub fn apply_nested(&mut self, doc: &mut UnifiedDoc, op: &PatchOp) -> Result<bool, PatchError> {
        apply_op(op, doc, self)
    }

    /// Evaluates an XPath with the document node as context.
    ///
    /// # Errors
    /// A missing expression, a syntax error or a non node-set result (all exceptions in the game).
    pub fn select(
        &mut self,
        doc: &mut UnifiedDoc,
        xpath: Option<&str>,
    ) -> Result<Vec<XNode>, PatchError> {
        select(doc, xpath)
    }
}

/// Applies the patch lists of all mods, in the order given (load order), to the document.
///
/// `patches` holds, per mod, its operations in file order. Content problems never abort: they are
/// in [`PatchReport::diagnostics`] and the per operation results.
pub fn apply_patches(
    doc: &mut UnifiedDoc,
    patches: &[(ModIdx, Vec<PatchOp>)],
    ctx: &PatchContext,
) -> PatchReport {
    let started = Instant::now();
    let mut sink = DiagSink::new();
    let mut events = Vec::new();
    let mut op_micros = Vec::new();
    for (mod_idx, ops) in patches {
        for (i, op) in ops.iter().enumerate() {
            let t = Instant::now();
            let op_index = u32::try_from(i).unwrap_or(u32::MAX);
            let event = run_top_level(doc, *mod_idx, op_index, op, ctx, &mut sink);
            events.push(event);
            op_micros.push(u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX));
        }
    }
    PatchReport {
        events,
        op_micros,
        total_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        diagnostics: sink,
    }
}

fn run_top_level(
    doc: &mut UnifiedDoc,
    mod_idx: ModIdx,
    op_index: u32,
    op: &PatchOp,
    ctx: &PatchContext,
    sink: &mut DiagSink,
) -> PatchEvent {
    let mut pctx = PatchCtx {
        mod_idx,
        file: op.file,
        op_index,
        raw: Node::default(),
        active: &ctx.active,
        mod_names: &ctx.mod_names,
        registry: &ctx.custom,
        diag: sink,
        touched: Vec::new(),
        changed: Vec::new(),
        top_changed: Vec::new(),
        origin: None,
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| apply_op(op, doc, &mut pctx)));
    let (result, error) = match outcome {
        Ok(Ok(b)) => (b, None),
        Ok(Err(e)) => {
            let text = e.to_string();
            pctx.emit(
                diag_codes::PATCH_EXCEPTION,
                Severity::Error,
                format!("Error in patch.Apply(): {text}"),
            );
            (false, Some(text))
        }
        Err(payload) => {
            let text = format!("the operation panicked: {}", panic_text(payload.as_ref()));
            doc.mark_index_dirty();
            pctx.emit(
                diag_codes::PATCH_EXCEPTION,
                Severity::Error,
                format!("Error in patch.Apply(): {text}"),
            );
            (false, Some(text))
        }
    };
    if !result && error.is_none() {
        pctx.emit(
            diag_codes::PATCH_FAILED,
            Severity::Error,
            format!("Patch operation {} failed", op.describe()),
        );
    }
    for c in std::mem::take(&mut pctx.changed) {
        doc.note_changed(c);
    }
    for t in std::mem::take(&mut pctx.top_changed) {
        doc.note_top_level(t);
    }
    let touched = std::mem::take(&mut pctx.touched);
    let mut touched_defs = 0u32;
    if result || !touched.is_empty() {
        let pref = PatchRef {
            mod_idx,
            file: op.file,
            op_index,
            class: op.class_name().to_owned(),
            xpath: op
                .xpath()
                .map(|x| x.split_whitespace().collect::<Vec<_>>().join(" ")),
            description: op.describe(),
        };
        let mut seen: Vec<NodeId> = Vec::new();
        for t in touched {
            if let Some(top) = doc.top_level_of(t)
                && !seen.contains(&top)
            {
                seen.push(top);
                doc.record_patch(top, pref.clone());
                touched_defs = touched_defs.saturating_add(1);
            }
        }
    }
    PatchEvent {
        mod_idx,
        file: op.file,
        index: op_index,
        class: op.display_class(),
        description: op.describe(),
        result,
        error,
        raw: if op.is_unknown() {
            op.raw().cloned()
        } else {
            None
        },
        touched_defs,
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_owned()
    }
}

/// Applies one operation including its `success` rewrite.
fn apply_op(
    op: &PatchOp,
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
) -> Result<bool, PatchError> {
    let worker = apply_worker(op, doc, ctx)?;
    Ok(op.success.rewrite(worker))
}

fn need<'v, T: ?Sized>(value: Option<&'v T>, what: &str) -> Result<&'v T, PatchError> {
    value.ok_or_else(|| PatchError::exception(format!("NullReferenceException: {what} is missing")))
}

fn apply_worker(
    op: &PatchOp,
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
) -> Result<bool, PatchError> {
    match &op.kind {
        PatchKind::Base => Ok(base_failure(op, ctx)),
        PatchKind::Unknown { class, raw } | PatchKind::Custom { class, raw } => {
            match ctx.registry.get(class) {
                Some(handler) => {
                    let saved = std::mem::replace(&mut ctx.raw, raw.clone());
                    let r = handler.apply(doc, ctx);
                    ctx.raw = saved;
                    r
                }
                None => Ok(base_failure(op, ctx)),
            }
        }
        PatchKind::Add {
            xpath,
            value,
            order,
        } => apply_add(doc, ctx, xpath.as_deref(), value.as_ref(), *order),
        PatchKind::Insert {
            xpath,
            value,
            order,
        } => apply_insert(doc, ctx, xpath.as_deref(), value.as_ref(), *order),
        PatchKind::Replace { xpath, value } => {
            apply_replace(doc, ctx, xpath.as_deref(), value.as_ref())
        }
        PatchKind::Remove { xpath } => apply_remove(doc, ctx, xpath.as_deref()),
        PatchKind::AddModExtension { xpath, value } => {
            apply_add_mod_extension(doc, ctx, xpath.as_deref(), value.as_ref())
        }
        PatchKind::AttributeAdd {
            xpath,
            attribute,
            value,
        } => apply_attribute(
            doc,
            ctx,
            xpath.as_deref(),
            attribute.as_deref(),
            value.as_deref(),
            AttributeMode::Add,
        ),
        PatchKind::AttributeSet {
            xpath,
            attribute,
            value,
        } => apply_attribute(
            doc,
            ctx,
            xpath.as_deref(),
            attribute.as_deref(),
            value.as_deref(),
            AttributeMode::Set,
        ),
        PatchKind::AttributeRemove { xpath, attribute } => apply_attribute(
            doc,
            ctx,
            xpath.as_deref(),
            attribute.as_deref(),
            None,
            AttributeMode::Remove,
        ),
        PatchKind::SetName { xpath, name } => {
            apply_set_name(doc, ctx, xpath.as_deref(), name.as_deref())
        }
        PatchKind::Test { xpath } => Ok(!select(doc, xpath.as_deref())?.is_empty()),
        PatchKind::Conditional {
            xpath,
            match_op,
            nomatch_op,
        } => {
            let found = !select(doc, xpath.as_deref())?.is_empty();
            if found {
                if let Some(m) = match_op {
                    return apply_op(m, doc, ctx);
                }
            } else if let Some(n) = nomatch_op {
                return apply_op(n, doc, ctx);
            }
            if match_op.is_none() {
                return Ok(nomatch_op.is_some());
            }
            Ok(true)
        }
        PatchKind::FindMod {
            mods,
            match_op,
            nomatch_op,
        } => {
            let found = mods.iter().any(|m| ctx.mod_names.contains(m));
            if found {
                if let Some(m) = match_op {
                    return apply_op(m, doc, ctx);
                }
            } else if let Some(n) = nomatch_op {
                return apply_op(n, doc, ctx);
            }
            Ok(true)
        }
        PatchKind::Sequence { operations } => {
            let ops = need(operations.as_ref(), "operations")?;
            for step in ops {
                let ok = apply_op(step, doc, ctx);
                // The next step selects through the `defName` index, so the structural changes of this
                // step must be in it already (the game's XPath sees the live document).
                ctx.flush_index_notes(doc);
                if !ok? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn base_failure(op: &PatchOp, ctx: &mut PatchCtx<'_>) -> bool {
    ctx.emit(
        diag_codes::PATCH_BASE_CLASS,
        Severity::Error,
        format!(
            "Attempted to use PatchOperation directly; patch will always fail (class {:?})",
            op.declared_class
        ),
    );
    false
}

// ------------------------------------------------------------------------- XPath

/// Evaluates an XPath with the document node as context, through the `defName` index when the
/// expression has the indexable shape.
fn select(doc: &mut UnifiedDoc, xpath: Option<&str>) -> Result<Vec<XNode>, PatchError> {
    let text = need(xpath, "xpath")?;
    let xp = XPath::parse(text)
        .map_err(|e| PatchError::exception(format!("XPath error in {text:?}: {e}")))?;
    if let Some(plan) = xp.index_plan()
        && doc.index_usable()
    {
        let defs = doc.lookup_keys(&plan.hint.keys());
        if plan.rest.is_empty() {
            return Ok(defs.into_iter().map(XNode::Node).collect());
        }
        match xp.select_with_defs(doc.arena(), &defs) {
            Ok(found) => return Ok(found.into_iter().map(XNode::Node).collect()),
            Err(XPathError::AttributeResult) => {}
            Err(e) => {
                return Err(PatchError::exception(format!(
                    "XPath error in {text:?}: {e}"
                )));
            }
        }
    }
    let root = doc.arena().root();
    xp.select_nodes(doc.arena(), root)
        .map_err(|e| PatchError::exception(format!("XPath error in {text:?}: {e}")))
}

// ------------------------------------------------------------------------- helpers

/// Copies every child of a `value` element into the arena as detached nodes.
fn import_children(
    doc: &mut UnifiedDoc,
    value: &Node,
    origin: OriginId,
) -> Result<Vec<NodeId>, PatchError> {
    let mut out: Vec<NodeId> = Vec::with_capacity(value.children.len());
    for child in &value.children {
        match doc.edit().import_child(child, origin) {
            Ok(id) => out.push(id),
            Err(e) => {
                free_detached(doc, &out);
                return Err(e.into());
            }
        }
    }
    Ok(out)
}

/// Frees nodes that are still detached (never linked, or detached again).
fn free_detached(doc: &mut UnifiedDoc, nodes: &[NodeId]) {
    for &n in nodes {
        if doc.arena().contains(n) && doc.arena().parent(n).is_none() {
            let _ = doc.edit().remove(n);
        }
    }
}

/// Reports a change of the children of `parent`: re-indexes the nodes `affected` when `parent` is
/// the `Defs` root (they are top-level nodes), otherwise the top-level node around `parent`.
fn note_children_changed(
    doc: &UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    parent: NodeId,
    affected: &[NodeId],
) {
    if parent == doc.defs_root() {
        for &a in affected {
            ctx.top_level_changed(a);
        }
    } else {
        ctx.changed(parent);
    }
}

fn is_element(doc: &UnifiedDoc, id: NodeId) -> bool {
    doc.arena().kind(id) == Some(NodeKind::Element)
}

// ------------------------------------------------------------------------- the operations

fn apply_add(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    value: Option<&Node>,
    order: Order,
) -> Result<bool, PatchError> {
    let value = need(value, "value")?;
    let mut result = false;
    for target in select(doc, xpath)? {
        result = true;
        match target {
            XNode::Attr { owner, index } => {
                if value.elements().next().is_some() {
                    return Err(PatchError::exception(
                        "InvalidOperationException: attribute cannot contain elements",
                    ));
                }
                let text: String = value
                    .children
                    .iter()
                    .filter_map(|c| c.as_text())
                    .collect::<Vec<_>>()
                    .concat();
                let Some((name, cur)) = doc
                    .arena()
                    .attr_at(owner, index as usize)
                    .map(|(n, v)| (n.to_owned(), v.to_owned()))
                else {
                    return Err(PatchError::exception("the attribute no longer exists"));
                };
                let new = match order {
                    Order::Append => format!("{cur}{text}"),
                    Order::Prepend => format!("{text}{cur}"),
                };
                doc.edit().set_attr(owner, &name, &new)?;
                ctx.touch(owner);
            }
            XNode::Node(id) => {
                if doc.arena().kind(id) == Some(NodeKind::Text) {
                    return Err(PatchError::exception(
                        "InvalidOperationException: text node cannot contain children",
                    ));
                }
                let origin = ctx.origin(doc);
                let items = import_children(doc, value, origin)?;
                let linked = match order {
                    Order::Append => doc.edit().append_children(id, &items),
                    Order::Prepend => doc.edit().prepend_children(id, &items),
                };
                if let Err(e) = linked {
                    free_detached(doc, &items);
                    return Err(e.into());
                }
                ctx.touch(id);
                note_children_changed(doc, ctx, id, &items);
            }
        }
    }
    Ok(result)
}

fn apply_insert(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    value: Option<&Node>,
    order: Order,
) -> Result<bool, PatchError> {
    let value = need(value, "value")?;
    let mut result = false;
    for target in select(doc, xpath)? {
        result = true;
        let XNode::Node(id) = target else {
            return Err(PatchError::exception(
                "NullReferenceException: attribute has no parent node",
            ));
        };
        let origin = ctx.origin(doc);
        let items = import_children(doc, value, origin)?;
        let outcome = match order {
            // Each copy lands directly after the target, so the copies end up reversed.
            Order::Append => items
                .iter()
                .try_for_each(|&it| doc.edit().insert_after(id, it)),
            // Walking the value backwards puts each copy directly before the target, so the
            // copies end up reversed as well.
            Order::Prepend => items
                .iter()
                .rev()
                .try_for_each(|&it| doc.edit().insert_before(id, it)),
        };
        if let Err(e) = outcome {
            free_detached(doc, &items);
            return Err(e.into());
        }
        let parent = doc.arena().parent(id);
        ctx.touch(if is_element(doc, id) {
            id
        } else {
            parent.unwrap_or(id)
        });
        if let Some(p) = parent {
            note_children_changed(doc, ctx, p, &items);
        }
    }
    Ok(result)
}

fn apply_replace(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    value: Option<&Node>,
) -> Result<bool, PatchError> {
    let value = need(value, "value")?;
    let targets = select(doc, xpath)?;
    let mut detached: Vec<NodeId> = Vec::new();
    let outcome = (|| -> Result<bool, PatchError> {
        let mut result = false;
        for target in targets {
            result = true;
            let XNode::Node(id) = target else {
                return Err(PatchError::exception(
                    "NullReferenceException: attribute has no parent node",
                ));
            };
            if !doc.arena().contains(id) {
                continue;
            }
            let Some(parent) = doc.arena().parent(id) else {
                return Err(PatchError::exception("cannot replace the document element"));
            };
            let origin = ctx.origin(doc);
            let items = import_children(doc, value, origin)?;
            ctx.touch(parent);
            if let Err(e) = doc.edit().replace_with(id, &items) {
                free_detached(doc, &items);
                return Err(e.into());
            }
            let mut affected = items;
            affected.push(id);
            note_children_changed(doc, ctx, parent, &affected);
            detached.push(id);
        }
        Ok(result)
    })();
    free_detached(doc, &detached);
    outcome
}

fn apply_remove(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
) -> Result<bool, PatchError> {
    let targets = select(doc, xpath)?;
    let mut detached: Vec<NodeId> = Vec::new();
    let outcome = (|| -> Result<bool, PatchError> {
        let mut result = false;
        for target in targets {
            result = true;
            let XNode::Node(id) = target else {
                return Err(PatchError::exception(
                    "NullReferenceException: attribute has no parent node",
                ));
            };
            if !doc.arena().contains(id) {
                continue;
            }
            let Some(parent) = doc.arena().parent(id) else {
                return Err(PatchError::exception("cannot remove the document element"));
            };
            ctx.touch(parent);
            doc.edit().detach(id)?;
            note_children_changed(doc, ctx, parent, &[id]);
            detached.push(id);
        }
        Ok(result)
    })();
    free_detached(doc, &detached);
    outcome
}

fn apply_add_mod_extension(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    value: Option<&Node>,
) -> Result<bool, PatchError> {
    let value = need(value, "value")?;
    let mut result = false;
    for target in select(doc, xpath)? {
        let XNode::Node(id) = target else {
            return Err(PatchError::exception(
                "NullReferenceException: target is not an element",
            ));
        };
        if !is_element(doc, id) {
            return Err(PatchError::exception(
                "NullReferenceException: target is not an element",
            ));
        }
        let origin = ctx.origin(doc);
        let ext = match doc.arena().child_named(id, "modExtensions") {
            Some(e) => e,
            None => doc.edit().append_element(id, "modExtensions", origin)?,
        };
        let items = import_children(doc, value, origin)?;
        if let Err(e) = doc.edit().append_children(ext, &items) {
            free_detached(doc, &items);
            return Err(e.into());
        }
        ctx.touch(id);
        ctx.changed(id);
        result = true;
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttributeMode {
    Add,
    Set,
    Remove,
}

fn apply_attribute(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    attribute: Option<&str>,
    value: Option<&str>,
    mode: AttributeMode,
) -> Result<bool, PatchError> {
    let mut result = false;
    for target in select(doc, xpath)? {
        let id = match target {
            XNode::Node(id) if is_element(doc, id) => id,
            _ => {
                return Err(PatchError::exception(
                    "NullReferenceException: node has no attributes",
                ));
            }
        };
        let name = need(attribute, "attribute")?;
        let exists = doc.arena().attr(id, name).is_some();
        match mode {
            AttributeMode::Add if !exists => {
                doc.edit().set_attr(id, name, value.unwrap_or(""))?;
                ctx.touch(id);
                result = true;
            }
            AttributeMode::Remove if exists => {
                doc.edit().remove_attr(id, name)?;
                ctx.touch(id);
                result = true;
            }
            AttributeMode::Set => {
                doc.edit().set_attr(id, name, value.unwrap_or(""))?;
                ctx.touch(id);
                result = true;
            }
            _ => {}
        }
    }
    Ok(result)
}

fn apply_set_name(
    doc: &mut UnifiedDoc,
    ctx: &mut PatchCtx<'_>,
    xpath: Option<&str>,
    name: Option<&str>,
) -> Result<bool, PatchError> {
    let mut result = false;
    for target in select(doc, xpath)? {
        result = true;
        let XNode::Node(id) = target else {
            return Err(PatchError::exception("node is not an element"));
        };
        if !is_element(doc, id) {
            return Err(PatchError::exception("node is not an element"));
        }
        if !doc.arena().contains(id) {
            continue;
        }
        let Some(parent) = doc.arena().parent(id) else {
            return Err(PatchError::exception("cannot rename the document element"));
        };
        let name = need(name, "name")?;
        // like the game, the new node has no asset: it is a node a patch created (its children
        // keep their own origins)
        let origin = ctx.origin(doc);
        let replacement = doc.edit().create_element(name, origin)?;
        let kids: Vec<NodeId> = doc.arena().children(id).collect();
        for k in kids {
            doc.edit().detach(k)?;
            doc.edit().append_child(replacement, k)?;
        }
        doc.edit().replace_with(id, &[replacement])?;
        doc.edit().remove(id)?;
        ctx.touch(parent);
        note_children_changed(doc, ctx, parent, &[id, replacement]);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
