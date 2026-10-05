//! The draft envelope: a design spec plus the dialogue state around it.
//!
//! A draft is the document the designer stores in the `designer-drafts` collection. The versioned envelope
//! (`{kind, v, data}`) belongs to `rimstudio-io`; this type is the `data` part and carries its own
//! `schemaVersion` so a reader can tell a newer draft from a supported one. [`Draft::KIND`] and
//! [`Draft::VERSION`] have the shape of the constants of the `Versioned` trait of `rimstudio-io`, which this
//! crate does not depend on; the toolkit implements the trait for [`Draft`] with them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::spec::{DesignSpec, ItemKind};

/// How the baseline values of a draft were produced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CalibrationMode {
    /// Tier, role and a strength choice only.
    #[default]
    Simple,
    /// The quiz stepper.
    Quiz,
    /// Starting from reference items.
    Anchored,
}

/// A reference item the draft is anchored on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Anchor {
    /// The def name of the reference item.
    pub def_name: String,
    /// A display label (OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Anchor {
    /// An anchor naming a def.
    #[must_use]
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            label: None,
        }
    }
}

/// The schema version of drafts this build writes.
///
/// Version 2 added the carried fields of a clone (recipe, comps, raw fields, own projectile data and the
/// accepted missing fields). The change is additive: a version 1 document reads unchanged and
/// [`migrate_value`] only stamps the new version. A document of a newer version is read as far as this build
/// understands it and is never written back ([`Draft::is_newer_than_supported`]).
pub const DRAFT_SCHEMA_VERSION: u32 = 2;

/// The oldest schema version this build reads and migrates.
pub const DRAFT_OLDEST_SCHEMA_VERSION: u32 = 1;

/// The migration step of a draft document from version 1 to version 2, on the JSON value of the draft
/// itself (the object with `schemaVersion`, `kind` and `spec`). A value of version 2 or newer is returned
/// unchanged; a value that is not an object is returned unchanged.
#[must_use]
pub fn migrate_value(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(map) = value.as_object_mut() {
        let found = map
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::from(DRAFT_OLDEST_SCHEMA_VERSION));
        if found < u64::from(DRAFT_SCHEMA_VERSION) {
            map.insert(
                "schemaVersion".to_owned(),
                serde_json::Value::from(DRAFT_SCHEMA_VERSION),
            );
        }
    }
    value
}

/// The stable kind name of drafts in the document store.
pub const DRAFT_KIND: &str = "designer-draft";

fn default_schema_version() -> u32 {
    DRAFT_SCHEMA_VERSION
}

/// A stored draft: the spec, the calibration mode, the quiz answers and the anchors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    /// The schema version of this document.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// The item kind; always equal to `spec.kind` (see [`Draft::is_consistent`]).
    pub kind: ItemKind,
    /// How baseline values are produced.
    #[serde(default)]
    pub calibration: CalibrationMode,
    /// The design itself. The optional CE patch toggle is `spec.ce` (absent means off).
    pub spec: DesignSpec,
    /// Quiz answers by question id, as the quiz module wrote them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub answers: BTreeMap<String, serde_json::Value>,
    /// The reference items the draft is anchored on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    /// The def name of the item this draft was cloned from (flow C); absent for a draft that started empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloned_from: Option<String>,
    /// What the user chose in the archetype dialog (additive: absent on older drafts and on drafts that
    /// never used it). It is intent, not numbers: the numbers live in the spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archetype: Option<super::archetype::ArchetypeChoice>,
}

impl Draft {
    /// The stable kind name, same as the `Versioned::KIND` of the store.
    pub const KIND: &'static str = DRAFT_KIND;
    /// The schema version written by this build, same as the `Versioned::VERSION` of the store.
    pub const VERSION: u32 = DRAFT_SCHEMA_VERSION;

    /// A new draft around a spec: simple calibration, no answers, no anchors.
    #[must_use]
    pub fn new(spec: DesignSpec) -> Self {
        Self {
            schema_version: Self::VERSION,
            kind: spec.kind,
            calibration: CalibrationMode::Simple,
            spec,
            answers: BTreeMap::new(),
            anchors: Vec::new(),
            cloned_from: None,
            archetype: None,
        }
    }

    /// Brings a draft read from an older schema version up to the current one. The shape is compatible, so
    /// only the version changes. A draft of a newer version is left as it is.
    pub fn migrate(&mut self) {
        if self.schema_version < Self::VERSION {
            self.schema_version = Self::VERSION;
        }
    }

    /// True when the envelope kind matches the spec kind.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.kind == self.spec.kind
    }

    /// True when the document was written by a newer build and must be opened read only.
    #[must_use]
    pub fn is_newer_than_supported(&self) -> bool {
        self.schema_version > Self::VERSION
    }

    /// True when the optional Combat Extended patch is on for this draft.
    #[must_use]
    pub fn ce_patch_enabled(&self) -> bool {
        self.spec.ce_patch_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CePatchSpec, ScalarField, ValueSource};

    fn sample() -> Draft {
        let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
        spec.offer(ScalarField::Damage, 11.0, ValueSource::Typed);
        spec.offer(ScalarField::Warmup, 1.2, ValueSource::Suggested);
        let mut d = Draft::new(spec);
        d.calibration = CalibrationMode::Quiz;
        d.answers
            .insert("tier".into(), serde_json::json!({"pick": "industrial"}));
        d.anchors.push(Anchor::new("RS_Anchor"));
        d
    }

    #[test]
    fn draft_round_trips_through_json() {
        let d = sample();
        let text = serde_json::to_string_pretty(&d).unwrap();
        let back: Draft = serde_json::from_str(&text).unwrap();
        assert_eq!(back, d);
        let again = serde_json::to_string_pretty(&back).unwrap();
        assert_eq!(text, again);
    }

    #[test]
    fn draft_with_the_ce_patch_round_trips_and_the_toggle_follows_the_block() {
        let mut d = sample();
        assert!(!d.ce_patch_enabled());
        d.spec.ce = Some(CePatchSpec {
            ammo_set: Some("RS_AmmoSet".into()),
            one_handed: true,
            ..CePatchSpec::default()
        });
        assert!(d.ce_patch_enabled());
        let back: Draft = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn the_clone_source_is_stored_only_when_present() {
        let mut d = sample();
        let json = serde_json::to_value(&d).unwrap();
        assert!(json.get("clonedFrom").is_none());
        d.cloned_from = Some("RS_Source".into());
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["clonedFrom"], "RS_Source");
        let back: Draft = serde_json::from_value(json).unwrap();
        assert_eq!(back.cloned_from.as_deref(), Some("RS_Source"));
    }

    #[test]
    fn envelope_uses_camel_case_and_exposes_constants() {
        let json = serde_json::to_value(sample()).unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["kind"], "ranged");
        assert_eq!(json["calibration"], "quiz");
        assert_eq!(Draft::KIND, "designer-draft");
        assert_eq!(Draft::VERSION, 2);
    }

    #[test]
    fn minimal_documents_load_with_defaults() {
        let d: Draft = serde_json::from_str(r#"{"kind":"melee","spec":{"kind":"melee"}}"#).unwrap();
        assert_eq!(d.schema_version, Draft::VERSION);
        assert_eq!(d.calibration, CalibrationMode::Simple);
        assert!(d.is_consistent());
        assert!(!d.is_newer_than_supported());
    }

    #[test]
    fn newer_and_inconsistent_drafts_are_detected() {
        let mut d = sample();
        d.schema_version = Draft::VERSION + 1;
        assert!(d.is_newer_than_supported());
        d.kind = ItemKind::Melee;
        assert!(!d.is_consistent());
    }

    #[test]
    fn a_version_one_document_reads_and_migrates_to_the_current_version() {
        let old = serde_json::json!({
            "schemaVersion": 1,
            "kind": "ranged",
            "spec": {"kind": "ranged", "identity": {"defName": "RS_Old", "label": "old"}}
        });
        let migrated = migrate_value(old.clone());
        assert_eq!(migrated["schemaVersion"], Draft::VERSION);
        assert_eq!(migrated["spec"], old["spec"]);
        let mut draft: Draft = serde_json::from_value(old).unwrap();
        assert_eq!(draft.schema_version, 1);
        assert!(!draft.is_newer_than_supported());
        draft.migrate();
        assert_eq!(draft.schema_version, Draft::VERSION);
        assert!(draft.spec.extra_fields.is_empty());
    }

    #[test]
    fn migration_is_idempotent_and_leaves_newer_documents_alone() {
        let current = serde_json::to_value(sample()).unwrap();
        assert_eq!(migrate_value(current.clone()), current);
        let newer =
            serde_json::json!({"schemaVersion": 9, "kind": "melee", "spec": {"kind": "melee"}});
        assert_eq!(migrate_value(newer.clone()), newer);
        assert_eq!(migrate_value(serde_json::json!(5)), serde_json::json!(5));
    }

    #[test]
    fn a_newer_document_with_unknown_fields_still_reads() {
        let newer = serde_json::json!({
            "schemaVersion": 9, "kind": "melee", "futureField": [1, 2],
            "spec": {"kind": "melee", "futureSpecField": true}
        });
        let d: Draft = serde_json::from_value(newer).unwrap();
        assert!(d.is_newer_than_supported());
    }
}
