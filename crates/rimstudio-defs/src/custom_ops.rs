//! Custom patch operations: the extension point for classes that live in mod assemblies.
//!
//! The game resolves a `Class` that no vanilla type matches against the loaded mod assemblies. The
//! engine cannot run assemblies, so a class either stays [`crate::patch_ops::PatchKind::Unknown`] (it
//! fails like the base operation and keeps its raw parameters) or is handled by a
//! [`CustomPatchOp`] registered in a [`CustomRegistry`].
//!
//! The first implementation is [`SettingsConditional`]: a conditional whose test is a boolean mod
//! setting that the caller supplies as data (the engine embeds no setting value and no class name of
//! a mod; the Combat Extended module of the design crate registers it under that mod's class).
//! The gun conversion operation of that mod is deliberately not handled here: it stays an unknown
//! operation with its raw parameters, and the item designer simulates it later through this trait.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use rimstudio_core::diag::Severity;

use crate::apply::PatchCtx;
use crate::diag_codes;
use crate::error::PatchError;
use crate::merge::UnifiedDoc;
use crate::patch_ops::PatchOp;

/// A patch operation class implemented outside the vanilla set.
///
/// The handler sees the whole `Operation` element as [`PatchCtx::raw`]; the engine applies the
/// common `success` wrapper to the returned result.
pub trait CustomPatchOp: Send + Sync {
    /// The full class name as it appears in `Class` (matched ignoring case).
    fn class(&self) -> &str;

    /// Applies the operation to the unified document.
    ///
    /// # Errors
    /// A [`PatchError`] is the equivalent of an exception: the engine reports it as
    /// `defs.patch-exception` and aborts the enclosing sequence, conditional or mod search.
    fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError>;
}

/// The registered custom classes of a load.
#[derive(Clone, Default)]
pub struct CustomRegistry {
    ops: BTreeMap<String, Arc<dyn CustomPatchOp>>,
}

impl fmt::Debug for CustomRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.ops.keys()).finish()
    }
}

impl CustomRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a handler under its class name (a later registration of the same class wins).
    pub fn register(&mut self, op: impl CustomPatchOp + 'static) {
        self.register_arc(Arc::new(op));
    }

    /// Registers a shared handler.
    pub fn register_arc(&mut self, op: Arc<dyn CustomPatchOp>) {
        self.ops.insert(op.class().to_lowercase(), op);
    }

    /// The handler for a class name (case insensitive).
    #[must_use]
    pub fn get(&self, class: &str) -> Option<Arc<dyn CustomPatchOp>> {
        self.ops.get(&class.to_lowercase()).cloned()
    }

    /// True when a handler is registered for the class.
    #[must_use]
    pub fn contains(&self, class: &str) -> bool {
        self.ops.contains_key(&class.to_lowercase())
    }

    /// Number of registered classes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// True when nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// A conditional on a boolean setting: the `settingName` field names the setting, `match` runs when it
/// is true and `nomatch` when it is false.
///
/// The truth table is the one of `PatchOperationConditional`. An unknown setting is reported
/// (`defs.patch-setting-missing`) and the operation fails. The setting values are data given by the
/// caller (read from the user's mod settings at run time), never constants of the engine.
#[derive(Debug, Clone)]
pub struct SettingsConditional {
    class: String,
    settings: BTreeMap<String, bool>,
}

impl SettingsConditional {
    /// A handler for `class` over the given settings.
    pub fn new(class: impl Into<String>, settings: BTreeMap<String, bool>) -> Self {
        Self {
            class: class.into(),
            settings,
        }
    }
}

impl CustomPatchOp for SettingsConditional {
    fn class(&self) -> &str {
        &self.class
    }

    fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        let raw = ctx.raw.clone();
        let mut name: Option<String> = None;
        let mut on_match: Option<PatchOp> = None;
        let mut on_nomatch: Option<PatchOp> = None;
        for child in raw.elements() {
            match child.tag.as_str() {
                "settingName" => name = Some(child.text_content()),
                "match" => on_match = Some(ctx.parse_nested(child)),
                "nomatch" => on_nomatch = Some(ctx.parse_nested(child)),
                _ => {}
            }
        }
        let value = name.as_deref().and_then(|n| self.settings.get(n)).copied();
        let Some(value) = value else {
            ctx.emit(
                diag_codes::PATCH_SETTING_MISSING,
                Severity::Error,
                format!(
                    "Cannot find the bool setting {}",
                    name.as_deref().unwrap_or("(missing settingName)")
                ),
            );
            return Ok(false);
        };
        let branch = if value { &on_match } else { &on_nomatch };
        if let Some(op) = branch {
            return ctx.apply_nested(doc, op);
        }
        if on_match.is_none() {
            return Ok(on_nomatch.is_some());
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Never;
    impl CustomPatchOp for Never {
        fn class(&self) -> &str {
            "RS_Mod.Never"
        }
        fn apply(&self, _: &mut UnifiedDoc, _: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
            Ok(false)
        }
    }

    #[test]
    fn registry_lookup_ignores_case() {
        let mut r = CustomRegistry::new();
        assert!(r.is_empty());
        r.register(Never);
        assert!(r.contains("rs_mod.NEVER"));
        assert!(r.get("RS_Mod.Never").is_some());
        assert!(!r.contains("RS_Mod.Other"));
        assert_eq!(r.len(), 1);
        assert_eq!(format!("{r:?}"), "[\"rs_mod.never\"]");
    }
}
