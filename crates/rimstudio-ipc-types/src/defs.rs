//! Def explorer DTOs: paged search rows and one resolved definition.
//!
//! Search returns small rows in pages; the full resolved node tree travels only for one definition at a time
//! as plain JSON (the tree shape of `rimstudio-core`: `tag`, `attrs`, `children`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request of `defs_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DefSearchRequest {
    /// The def session to search.
    pub session_id: String,
    /// Free text matched against def name, label and type; empty matches everything.
    pub query: String,
    /// Restrict to these def types (for example `ThingDef`); empty means every type.
    pub def_types: Vec<String>,
    /// Restrict to these mods (stable mod ids); empty means every mod.
    pub mod_ids: Vec<String>,
    /// Hide abstract definitions.
    pub hide_abstract: bool,
    /// Zero based offset of the first row.
    pub offset: u32,
    /// Maximum rows in the page; the backend caps it at 500.
    pub limit: u32,
    /// Id the caller minted for this query so late answers can be dropped; echoed in the page.
    pub query_id: String,
}

impl Default for DefSearchRequest {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            query: String::new(),
            def_types: Vec::new(),
            mod_ids: Vec::new(),
            hide_abstract: false,
            offset: 0,
            limit: 100,
            query_id: String::new(),
        }
    }
}

/// One search row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefRowDto {
    /// Def type, for example `ThingDef`.
    pub def_type: String,
    /// Def name. Empty for abstract definitions that only carry a `Name` attribute.
    pub def_name: String,
    /// Display label. Absent when the definition has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// True for an abstract definition.
    pub is_abstract: bool,
    /// Stable id of the mod that defines it.
    pub mod_id: String,
    /// Path of the file relative to the mod folder.
    pub file: String,
    /// Parent name. Absent when the definition has no parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Response of `defs_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefPage {
    /// Echo of the request query id.
    pub query_id: String,
    /// Total matching rows.
    pub total: u64,
    /// Offset of the first row of this page.
    pub offset: u32,
    /// The rows.
    pub items: Vec<DefRowDto>,
}

/// Request of `defs_get_resolved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefsGetResolvedRequest {
    /// The def session.
    pub session_id: String,
    /// Def type.
    pub def_type: String,
    /// Def name.
    pub def_name: String,
}

/// What happened when a patch operation met the definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatchOutcomeDto {
    /// The operation changed the definition.
    Applied,
    /// The operation matched nothing.
    NoMatch,
    /// The operation failed.
    Failed,
    /// The operation was not simulated (a custom operation class).
    NotSimulated,
}

/// One patch event that touched a definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchEventDto {
    /// Mod that owns the patch.
    pub mod_id: String,
    /// Patch file relative to the mod folder.
    pub file: String,
    /// Operation class name.
    pub operation: String,
    /// Outcome.
    pub outcome: PatchOutcomeDto,
    /// English fallback text. Absent when there is nothing to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// One definition after inheritance and patches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDefDto {
    /// Def type.
    pub def_type: String,
    /// Def name.
    pub def_name: String,
    /// Stable id of the defining mod.
    pub mod_id: String,
    /// Path of the defining file relative to the mod folder.
    pub file: String,
    /// The resolved node tree as JSON.
    pub tree: Value,
    /// Patch events in application order.
    pub patch_events: Vec<PatchEventDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_defaults_apply_to_missing_members() {
        let request: Result<DefSearchRequest, _> =
            serde_json::from_str(r#"{"sessionId":"s1","query":"RS_"}"#);
        let request = request.unwrap_or_default();
        assert_eq!(request.limit, 100);
        assert_eq!(request.session_id, "s1");
        assert!(request.def_types.is_empty());
    }

    #[test]
    fn page_and_resolved_def_round_trip() {
        let page = DefPage {
            query_id: "q1".into(),
            total: 1,
            offset: 0,
            items: vec![DefRowDto {
                def_type: "ThingDef".into(),
                def_name: "RS_TestRifle".into(),
                label: None,
                is_abstract: false,
                mod_id: "w1".into(),
                file: "Defs/Weapons.xml".into(),
                parent: Some("RS_BaseGun".into()),
            }],
        };
        let text = serde_json::to_string(&page).unwrap_or_default();
        assert!(text.contains("\"isAbstract\":false"));
        assert!(!text.contains("label"));
        let back: Result<DefPage, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(page));

        let resolved = ResolvedDefDto {
            def_type: "ThingDef".into(),
            def_name: "RS_TestRifle".into(),
            mod_id: "w1".into(),
            file: "Defs/Weapons.xml".into(),
            tree: serde_json::json!({"tag":"ThingDef","attrs":[],"children":[]}),
            patch_events: vec![PatchEventDto {
                mod_id: "w2".into(),
                file: "Patches/P.xml".into(),
                operation: "PatchOperationReplace".into(),
                outcome: PatchOutcomeDto::NotSimulated,
                message: None,
            }],
        };
        let text = serde_json::to_string(&resolved).unwrap_or_default();
        let back: Result<ResolvedDefDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(resolved));
    }
}
