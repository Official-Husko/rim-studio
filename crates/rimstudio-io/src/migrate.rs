//! Forward only schema migrations on [`serde_json::Value`].
//!
//! A migration step is a pure function from the document of version N to the document of version
//! N + 1. A [`MigrationRegistry`] holds the steps per document kind and applies the chain in order.
//! There is no downgrade: a document newer than the app is opened read only by the callers.
//! The registry is an ordinary value, so tests and stores build their own; types usually register
//! through [`Versioned::migrations`].

use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::MigrateError;
use crate::schema::Versioned;

/// One migration step: pure, no IO, from version N to N + 1.
pub type MigrationFn = fn(Value) -> Result<Value, MigrateError>;

/// The migration steps of every known document kind.
#[derive(Debug, Clone, Default)]
pub struct MigrationRegistry {
    steps: BTreeMap<(String, u32), MigrationFn>,
}

impl MigrationRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry holding the steps declared by `T`.
    pub fn for_type<T: Versioned>() -> Self {
        let mut r = Self::new();
        for (from, f) in T::migrations() {
            r.register(T::KIND, from, f);
        }
        r
    }

    /// Registers the step that turns version `from` of `kind` into `from + 1`. A second
    /// registration for the same pair replaces the first.
    pub fn register(&mut self, kind: &str, from: u32, step: MigrationFn) -> &mut Self {
        self.steps.insert((kind.to_owned(), from), step);
        self
    }

    /// True when a step from `from` exists for `kind`.
    pub fn has_step(&self, kind: &str, from: u32) -> bool {
        self.steps.contains_key(&(kind.to_owned(), from))
    }

    /// Applies the steps from version `from` up to version `to` (the result is
    /// at version `to`). `from == to` returns the value unchanged. A missing step is
    /// [`MigrateError::NoPath`]; `from > to` is also `NoPath` because migrations only go forward.
    /// The `after_step` callback runs after each step with the new version and may adjust the value
    /// (the stores use it to stamp the version field).
    pub fn migrate_with(
        &self,
        kind: &str,
        from: u32,
        to: u32,
        mut value: Value,
        mut after_step: impl FnMut(u32, &mut Value),
    ) -> Result<Value, MigrateError> {
        if from > to {
            return Err(MigrateError::NoPath {
                kind: kind.to_owned(),
                from,
                to,
            });
        }
        let mut at = from;
        while at < to {
            let step =
                self.steps
                    .get(&(kind.to_owned(), at))
                    .ok_or_else(|| MigrateError::NoPath {
                        kind: kind.to_owned(),
                        from,
                        to,
                    })?;
            value = step(value)?;
            at = at.saturating_add(1);
            after_step(at, &mut value);
        }
        Ok(value)
    }

    /// [`migrate_with`](Self::migrate_with) without a callback.
    pub fn migrate(
        &self,
        kind: &str,
        from: u32,
        to: u32,
        value: Value,
    ) -> Result<Value, MigrateError> {
        self.migrate_with(kind, from, to, value, |_, _| {})
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn v1_to_v2(mut v: Value) -> Result<Value, MigrateError> {
        let old = v
            .get("name")
            .cloned()
            .ok_or_else(|| MigrateError::failed("RS_Doc", 1, "name missing"))?;
        if let Some(o) = v.as_object_mut() {
            o.remove("name");
            o.insert("title".into(), old);
        }
        Ok(v)
    }

    fn v2_to_v3(mut v: Value) -> Result<Value, MigrateError> {
        if let Some(o) = v.as_object_mut() {
            o.insert("extra".into(), json!(true));
        }
        Ok(v)
    }

    fn registry() -> MigrationRegistry {
        let mut r = MigrationRegistry::new();
        r.register("RS_Doc", 1, v1_to_v2)
            .register("RS_Doc", 2, v2_to_v3);
        r
    }

    #[test]
    fn chain_runs_in_order() {
        let out = registry()
            .migrate("RS_Doc", 1, 3, json!({"name": "RS_A"}))
            .unwrap();
        assert_eq!(out, json!({"title": "RS_A", "extra": true}));
    }

    #[test]
    fn same_version_is_identity() {
        let out = registry().migrate("RS_Doc", 3, 3, json!({"a": 1})).unwrap();
        assert_eq!(out, json!({"a": 1}));
    }

    #[test]
    fn missing_step_is_no_path() {
        let err = registry().migrate("RS_Doc", 3, 4, json!({})).unwrap_err();
        assert_eq!(err.code(), "migrate.no-path");
        let err = registry().migrate("RS_Other", 1, 2, json!({})).unwrap_err();
        assert_eq!(err.code(), "migrate.no-path");
    }

    #[test]
    fn downgrade_is_refused() {
        let err = registry().migrate("RS_Doc", 3, 1, json!({})).unwrap_err();
        assert_eq!(err.code(), "migrate.no-path");
    }

    #[test]
    fn step_failure_is_reported() {
        let err = registry().migrate("RS_Doc", 1, 2, json!({})).unwrap_err();
        assert_eq!(err.code(), "migrate.failed");
    }

    #[test]
    fn callback_sees_each_version() {
        let mut seen = Vec::new();
        registry()
            .migrate_with("RS_Doc", 1, 3, json!({"name": "x"}), |v, _| seen.push(v))
            .unwrap();
        assert_eq!(seen, vec![2, 3]);
    }
}
