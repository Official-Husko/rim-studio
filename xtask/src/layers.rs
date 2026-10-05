//! `check-layers`: layer tags, the allowed edge matrix, dev-dependency rules, manifest
//! hygiene and the named third party bans.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::jsonc;
use crate::model::{DepKind, Member, Workspace};
use crate::report::Finding;

/// One named ban from `layers.jsonc`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ban {
    id: String,
    reason: String,
    crates: Vec<String>,
    prefixes: Vec<String>,
    allow_in: Vec<String>,
    allow_dev_in: Vec<String>,
    allow_layers: Vec<String>,
    require_optional_feature: Option<String>,
}

/// The parsed `layers.jsonc`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Config {
    layers: BTreeSet<String>,
    allowed: BTreeMap<String, BTreeSet<String>>,
    same_layer: BTreeSet<(String, String)>,
    dev_allowed: BTreeSet<String>,
    bans: Vec<Ban>,
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

impl Config {
    /// Parses the JSONC text of `layers.jsonc`.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let v = jsonc::parse(text).context("layers.jsonc")?;
        let mut cfg = Config {
            layers: strings(v.get("layers")).into_iter().collect(),
            ..Self::default()
        };
        if let Some(obj) = v.get("allowed").and_then(Value::as_object) {
            for (k, list) in obj {
                cfg.allowed
                    .insert(k.clone(), strings(Some(list)).into_iter().collect());
            }
        }
        for pair in v
            .get("allowedSameLayer")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let p = strings(Some(pair));
            if let [a, b] = p.as_slice() {
                cfg.same_layer.insert((a.clone(), b.clone()));
            }
        }
        cfg.dev_allowed = strings(v.get("devAllowedInternal")).into_iter().collect();
        for b in v
            .get("bans")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            cfg.bans.push(Ban {
                id: b
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("ban")
                    .to_string(),
                reason: b
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                crates: strings(b.get("crates")),
                prefixes: strings(b.get("prefixes")),
                allow_in: strings(b.get("allowIn")),
                allow_dev_in: strings(b.get("allowDevIn")),
                allow_layers: strings(b.get("allowLayers")),
                require_optional_feature: b
                    .get("requireOptionalFeature")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        Ok(cfg)
    }
}

/// Runs every layer check.
pub(crate) fn check(ws: &Workspace, cfg: &Config) -> Vec<Finding> {
    let mut out = Vec::new();
    for m in &ws.members {
        check_tag(m, cfg, &mut out);
        check_edges(m, ws, cfg, &mut out);
        check_bans(m, cfg, &mut out);
        for (line, msg) in manifest_issues(&m.manifest) {
            out.push(Finding::new(
                "layers.manifest",
                format!("{}:{line}", m.manifest_path),
                msg,
            ));
        }
    }
    out
}

fn check_tag(m: &Member, cfg: &Config, out: &mut Vec<Finding>) {
    match &m.layer {
        None => out.push(Finding::new(
            "layers.untagged",
            &m.manifest_path,
            "missing [package.metadata.rimstudio] layer",
        )),
        Some(tag) if !cfg.layers.contains(tag) => out.push(Finding::new(
            "layers.unknown-tag",
            &m.manifest_path,
            format!("unknown layer tag `{tag}`"),
        )),
        Some(_) => {}
    }
}

fn check_edges(m: &Member, ws: &Workspace, cfg: &Config, out: &mut Vec<Finding>) {
    for dep in &m.deps {
        let Some(target) = ws.member(&dep.name) else {
            continue;
        };
        if dep.kind == DepKind::Dev {
            if !cfg.dev_allowed.contains(&target.name) {
                out.push(Finding::new(
                    "layers.dev-edge",
                    &m.manifest_path,
                    format!(
                        "{} has dev-dependency {}; only {:?} may be one",
                        m.name, target.name, cfg.dev_allowed
                    ),
                ));
            }
            continue;
        }
        let (Some(from), Some(to)) = (&m.layer, &target.layer) else {
            continue;
        };
        if from == to {
            if !cfg
                .same_layer
                .contains(&(m.name.clone(), target.name.clone()))
            {
                out.push(Finding::new(
                    "layers.same-layer",
                    &m.manifest_path,
                    format!(
                        "{} -> {} stays inside layer {from} and is not in allowedSameLayer",
                        m.name, target.name
                    ),
                ));
            }
        } else if !cfg.allowed.get(from).is_some_and(|set| set.contains(to)) {
            out.push(Finding::new(
                "layers.edge",
                &m.manifest_path,
                format!(
                    "{} ({from}) must not depend on {} ({to})",
                    m.name, target.name
                ),
            ));
        }
    }
}

fn check_bans(m: &Member, cfg: &Config, out: &mut Vec<Finding>) {
    for dep in &m.deps {
        for ban in &cfg.bans {
            let hit = ban.crates.contains(&dep.name)
                || ban
                    .prefixes
                    .iter()
                    .any(|p| dep.name.starts_with(p.as_str()));
            if !hit {
                continue;
            }
            let layer_ok = m
                .layer
                .as_ref()
                .is_some_and(|l| ban.allow_layers.contains(l));
            let in_allow = ban.allow_in.contains(&m.name);
            let dev_ok = dep.kind == DepKind::Dev && ban.allow_dev_in.contains(&m.name);
            if !(in_allow || dev_ok || layer_ok) {
                out.push(Finding::new(
                    "layers.ban",
                    &m.manifest_path,
                    format!(
                        "{} depends on `{}` ({}: {})",
                        m.name, dep.name, ban.id, ban.reason
                    ),
                ));
            } else if let (true, Some(feature)) = (in_allow, &ban.require_optional_feature) {
                let gated = dep.optional && feature_enables(m, feature, &dep.name);
                if !gated {
                    out.push(Finding::new(
                        "layers.ban",
                        &m.manifest_path,
                        format!(
                            "{} must keep `{}` optional and enabled only by feature `{feature}` ({})",
                            m.name, dep.name, ban.id
                        ),
                    ));
                }
            }
        }
    }
}

fn feature_enables(m: &Member, feature: &str, dep: &str) -> bool {
    m.features.get(feature).is_some_and(|list| {
        list.iter()
            .any(|e| e == dep || e.strip_prefix("dep:") == Some(dep))
    })
}

/// Scans manifest text for members that opt out of the workspace tables.
/// Returns `(line, message)` pairs.
pub(crate) fn manifest_issues(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut lints_ok = false;
    let mut has_lints = false;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line
                .trim_matches(|c| c == '[' || c == ']')
                .trim()
                .to_string();
            has_lints |= section == "lints";
            continue;
        }
        if section == "lints" && line.replace(' ', "") == "workspace=true" {
            lints_ok = true;
        }
        let is_dep_section = section.ends_with("dependencies");
        if is_dep_section {
            let workspace_inherited =
                line.contains("workspace = true") || line.contains("workspace=true");
            if !workspace_inherited && line.contains('=') {
                let key = line.split('=').next().unwrap_or("").trim();
                out.push((
                    i + 1,
                    format!("dependency `{key}` is not declared with `workspace = true`"),
                ));
            }
        }
    }
    if !(has_lints && lints_ok) {
        out.push((
            1,
            "member does not opt into [lints] workspace = true".to_string(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Dep;

    const CFG: &str = r#"{
      "layers": ["l0-domain","l1-infra","l2-engine","l4-shell","support"],
      "allowed": {"l0-domain": [], "l1-infra": ["l0-domain"], "l2-engine": ["l0-domain"],
                  "l4-shell": ["l0-domain"], "support": ["l0-domain"]},
      "allowedSameLayer": [["e1", "e2"]],
      "devAllowedInternal": ["rimstudio-testing"],
      "bans": [
        {"id": "I-02", "reason": "xml", "crates": ["quick-xml"], "allowIn": ["rimstudio-xml"]},
        {"id": "I-01", "reason": "tauri", "crates": ["tauri"], "allowLayers": ["l4-shell"]},
        {"id": "D-031", "reason": "db", "crates": ["rusqlite"], "allowIn": ["rimstudio-datasets"],
         "requireOptionalFeature": "aux-db"},
        {"id": "D-037", "reason": "dev only", "crates": ["steamlocate"], "allowDevIn": ["rimstudio-steam"]}
      ]
    }"#;

    const GOOD_MANIFEST: &str = "[package]\nname = \"x\"\n[lints]\nworkspace = true\n[dependencies]\nserde.workspace = true\n";

    fn member(name: &str, layer: Option<&str>, deps: &[(&str, DepKind)]) -> Member {
        Member {
            name: name.to_string(),
            layer: layer.map(str::to_string),
            manifest_path: format!("crates/{name}/Cargo.toml"),
            manifest: GOOD_MANIFEST.to_string(),
            deps: deps
                .iter()
                .map(|(n, k)| Dep {
                    name: n.to_string(),
                    kind: *k,
                    optional: false,
                })
                .collect(),
            features: BTreeMap::new(),
        }
    }

    fn run(members: Vec<Member>) -> Vec<Finding> {
        let cfg = Config::parse(CFG).unwrap();
        check(&Workspace { members }, &cfg)
    }

    fn rules(f: &[Finding]) -> Vec<&str> {
        f.iter().map(|x| x.rule.as_str()).collect()
    }

    #[test]
    fn a_clean_workspace_has_no_findings() {
        let f = run(vec![
            member("core", Some("l0-domain"), &[]),
            member("io", Some("l1-infra"), &[("core", DepKind::Normal)]),
        ]);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn upward_edges_are_rejected() {
        let f = run(vec![
            member("core", Some("l0-domain"), &[("io", DepKind::Normal)]),
            member("io", Some("l1-infra"), &[]),
        ]);
        assert_eq!(rules(&f), ["layers.edge"]);
    }

    #[test]
    fn build_dependencies_follow_the_matrix() {
        let f = run(vec![
            member("eng", Some("l2-engine"), &[("io", DepKind::Build)]),
            member("io", Some("l1-infra"), &[]),
        ]);
        assert_eq!(rules(&f), ["layers.edge"]);
    }

    #[test]
    fn same_layer_edges_need_an_exception() {
        let f = run(vec![
            member("e1", Some("l2-engine"), &[("e2", DepKind::Normal)]),
            member("e2", Some("l2-engine"), &[("e1", DepKind::Normal)]),
        ]);
        assert_eq!(rules(&f), ["layers.same-layer"]);
        assert!(f[0].message.contains("e2 -> e1"));
    }

    #[test]
    fn dev_dependencies_may_only_be_the_testing_crate() {
        let f = run(vec![
            member(
                "e1",
                Some("l2-engine"),
                &[("rimstudio-testing", DepKind::Dev), ("core", DepKind::Dev)],
            ),
            member("core", Some("l0-domain"), &[]),
            member(
                "rimstudio-testing",
                Some("support"),
                &[("core", DepKind::Normal)],
            ),
        ]);
        assert_eq!(rules(&f), ["layers.dev-edge"]);
    }

    #[test]
    fn testing_crate_as_normal_dependency_is_rejected() {
        let f = run(vec![
            member(
                "e1",
                Some("l2-engine"),
                &[("rimstudio-testing", DepKind::Normal)],
            ),
            member("rimstudio-testing", Some("support"), &[]),
        ]);
        assert_eq!(rules(&f), ["layers.edge"]);
    }

    #[test]
    fn missing_and_unknown_tags_are_reported() {
        let f = run(vec![member("a", None, &[]), member("b", Some("l9"), &[])]);
        assert_eq!(rules(&f), ["layers.untagged", "layers.unknown-tag"]);
    }

    #[test]
    fn xml_crates_are_banned_outside_the_xml_crate() {
        let f = run(vec![
            member(
                "rimstudio-xml",
                Some("l1-infra"),
                &[("quick-xml", DepKind::Normal)],
            ),
            member("other", Some("l2-engine"), &[("quick-xml", DepKind::Dev)]),
        ]);
        assert_eq!(rules(&f), ["layers.ban"]);
        assert!(f[0].location.contains("other"));
    }

    #[test]
    fn tauri_is_allowed_only_in_the_shell_layer() {
        let f = run(vec![
            member("shell", Some("l4-shell"), &[("tauri", DepKind::Normal)]),
            member("cli", Some("l2-engine"), &[("tauri", DepKind::Normal)]),
        ]);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("cli depends on `tauri`"));
    }

    #[test]
    fn dev_only_crates_pass_as_dev_dependency_of_their_owner_only() {
        let dev = run(vec![member(
            "rimstudio-steam",
            Some("l2-engine"),
            &[("steamlocate", DepKind::Dev)],
        )]);
        assert!(dev.is_empty());
        let normal = run(vec![member(
            "rimstudio-steam",
            Some("l2-engine"),
            &[("steamlocate", DepKind::Normal)],
        )]);
        assert_eq!(rules(&normal), ["layers.ban"]);
    }

    #[test]
    fn rusqlite_must_stay_optional_behind_its_feature() {
        let mut m = member(
            "rimstudio-datasets",
            Some("l2-engine"),
            &[("rusqlite", DepKind::Normal)],
        );
        assert_eq!(rules(&run(vec![m.clone()])), ["layers.ban"]);
        m.deps[0].optional = true;
        m.features
            .insert("aux-db".to_string(), vec!["dep:rusqlite".to_string()]);
        assert!(run(vec![m]).is_empty());
    }

    #[test]
    fn unknown_dependencies_are_tolerated() {
        let f = run(vec![member(
            "a",
            Some("l2-engine"),
            &[("rimstudio-future", DepKind::Normal)],
        )]);
        assert!(f.is_empty());
    }

    #[test]
    fn manifest_opt_outs_are_reported_with_lines() {
        let text = "[package]\nname = \"x\"\n[dependencies]\n# c\nserde = \"1\"\nserde_json.workspace = true\n";
        let issues = manifest_issues(text);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].0, 5);
        assert!(issues[1].1.contains("[lints]"));
    }

    #[test]
    fn table_style_lints_opt_in_is_recognised() {
        assert!(manifest_issues("[lints]\nworkspace = true\n").is_empty());
    }

    #[test]
    fn shipped_config_parses_and_covers_every_layer() {
        let cfg = Config::parse(include_str!("../layers.jsonc")).unwrap();
        assert_eq!(cfg.layers.len(), 11);
        for l in &cfg.layers {
            assert!(cfg.allowed.contains_key(l), "no row for {l}");
        }
        assert!(cfg.bans.len() >= 8);
    }
}
