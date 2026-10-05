//! The workspace model read from `cargo metadata --format-version 1 --no-deps`,
//! plus the small file system helpers shared by the checks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Kind of a dependency edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepKind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
    /// `[build-dependencies]`.
    Build,
}

/// A declared dependency of a workspace member.
#[derive(Debug, Clone)]
pub(crate) struct Dep {
    /// Package name of the dependency.
    pub(crate) name: String,
    /// Edge kind.
    pub(crate) kind: DepKind,
    /// Whether the dependency is optional.
    pub(crate) optional: bool,
}

/// A workspace member.
#[derive(Debug, Clone)]
pub(crate) struct Member {
    /// Package name.
    pub(crate) name: String,
    /// Layer tag from `[package.metadata.rimstudio]`, if present.
    pub(crate) layer: Option<String>,
    /// Manifest path relative to the repository root, with forward slashes.
    pub(crate) manifest_path: String,
    /// Raw manifest text.
    pub(crate) manifest: String,
    /// Declared dependencies.
    pub(crate) deps: Vec<Dep>,
    /// Declared features and what they enable.
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

/// All workspace members.
#[derive(Debug, Clone, Default)]
pub(crate) struct Workspace {
    /// Members sorted by name.
    pub(crate) members: Vec<Member>,
}

impl Workspace {
    /// Finds a member by package name.
    pub(crate) fn member(&self, name: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.name == name)
    }

    /// Runs cargo metadata at `root` and reads the manifests.
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let out = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .current_dir(root)
            .output()
            .context("failed to run cargo metadata")?;
        if !out.status.success() {
            bail!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let json: Value =
            serde_json::from_slice(&out.stdout).context("cargo metadata output is not JSON")?;
        Self::from_metadata(&json, root)
    }

    /// Builds the model from parsed metadata JSON; manifests are read from disk.
    pub(crate) fn from_metadata(json: &Value, root: &Path) -> Result<Self> {
        let mut members = Vec::new();
        let packages = json.get("packages").and_then(Value::as_array);
        for p in packages.map(Vec::as_slice).unwrap_or(&[]) {
            let name = str_field(p, "name").unwrap_or_default().to_string();
            let abs = str_field(p, "manifest_path").unwrap_or_default();
            let rel = Path::new(abs)
                .strip_prefix(root)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| abs.replace('\\', "/"));
            let manifest = std::fs::read_to_string(abs).unwrap_or_default();
            let layer = p
                .pointer("/metadata/rimstudio/layer")
                .and_then(Value::as_str)
                .map(str::to_string);
            let deps = p
                .get("dependencies")
                .and_then(Value::as_array)
                .map(|ds| ds.iter().filter_map(parse_dep).collect())
                .unwrap_or_default();
            let mut features = BTreeMap::new();
            if let Some(fs) = p.get("features").and_then(Value::as_object) {
                for (k, v) in fs {
                    let list = v
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    features.insert(k.clone(), list);
                }
            }
            members.push(Member {
                name,
                layer,
                manifest_path: rel,
                manifest,
                deps,
                features,
            });
        }
        members.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { members })
    }
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn parse_dep(d: &Value) -> Option<Dep> {
    let name = str_field(d, "name")?.to_string();
    let kind = match str_field(d, "kind") {
        Some("dev") => DepKind::Dev,
        Some("build") => DepKind::Build,
        _ => DepKind::Normal,
    };
    let optional = d.get("optional").and_then(Value::as_bool).unwrap_or(false);
    Some(Dep {
        name,
        kind,
        optional,
    })
}

/// Walks up from `start` to the directory whose `Cargo.toml` declares `[workspace]`.
pub(crate) fn find_root(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if let Ok(text) = std::fs::read_to_string(d.join("Cargo.toml"))
            && text.lines().any(|l| l.trim() == "[workspace]")
        {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Recursively lists files under `dir` whose names pass `keep`, sorted, skipping
/// build output and VCS folders. Unreadable folders are skipped.
pub(crate) fn walk(dir: &Path, keep: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(dir, keep, &mut out);
    out.sort();
    out
}

fn walk_into(dir: &Path, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "target" | ".git" | "node_modules") {
                continue;
            }
            walk_into(&path, keep, out);
        } else if keep(&path) {
            out.push(path);
        }
    }
}

/// Repository relative path with forward slashes.
pub(crate) fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_is_parsed_into_members_and_deps() {
        let json: Value = serde_json::from_str(
            r#"{"packages":[{"name":"b","manifest_path":"/nowhere/b/Cargo.toml",
            "metadata":{"rimstudio":{"layer":"l0-domain"}},
            "features":{"x":["dep:y"]},
            "dependencies":[{"name":"a","kind":"dev","optional":false},
                            {"name":"y","kind":null,"optional":true}]},
            {"name":"a","manifest_path":"/nowhere/a/Cargo.toml","metadata":null,"dependencies":[]}]}"#,
        )
        .unwrap();
        let ws = Workspace::from_metadata(&json, Path::new("/nowhere")).unwrap();
        assert_eq!(ws.members[0].name, "a");
        assert_eq!(ws.members[0].layer, None);
        let b = ws.member("b").unwrap();
        assert_eq!(b.layer.as_deref(), Some("l0-domain"));
        assert_eq!(b.manifest_path, "b/Cargo.toml");
        assert_eq!(b.deps[0].kind, DepKind::Dev);
        assert!(b.deps[1].optional);
        assert_eq!(b.features["x"], vec!["dep:y".to_string()]);
    }

    #[test]
    fn empty_metadata_gives_no_members() {
        let ws = Workspace::from_metadata(&Value::Null, Path::new("/")).unwrap();
        assert!(ws.members.is_empty());
    }
}
