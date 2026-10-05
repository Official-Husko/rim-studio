//! The datasets used by the spike. Paths can be overridden with SPIKE_ROOT_<NAME> environment variables.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Root {
    pub name: &'static str,
    pub path: PathBuf,
    /// true: the root itself is one mod; false: its child directories are mods.
    pub single_mod: bool,
}

fn env_or(name: &str, default: String) -> PathBuf {
    std::env::var_os(format!("SPIKE_ROOT_{}", name.to_uppercase())).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(default))
}

pub fn all_roots() -> Vec<Root> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
    let steam = format!("{home}/.steam/steam/steamapps");
    vec![
        Root { name: "workshop", path: env_or("workshop", format!("{steam}/workshop/content/294100")), single_mod: false },
        Root { name: "data", path: env_or("data", format!("{steam}/common/RimWorld/Data")), single_mod: false },
        Root { name: "localmods", path: env_or("localmods", format!("{steam}/common/RimWorld/Mods")), single_mod: false },
        Root {
            name: "owner",
            path: env_or("owner", "/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods".into()),
            single_mod: false,
        },
        Root {
            name: "ce",
            path: env_or("ce", "/run/media/pawbeans/project_drive/pawbeans/Projects/Rust/rimforge-studio/CombatExtended-Development".into()),
            single_mod: true,
        },
    ]
}

/// Select roots by comma separated names. A spec of the form `path:/some/dir` adds an ad-hoc parent-of-mods root.
pub fn select(names: &[String]) -> Vec<Root> {
    let all = all_roots();
    let mut out = vec![];
    for n in names {
        if let Some(p) = n.strip_prefix("path:") {
            out.push(Root { name: "adhoc", path: PathBuf::from(p), single_mod: false });
        } else if let Some(p) = n.strip_prefix("modpath:") {
            out.push(Root { name: "adhoc-mod", path: PathBuf::from(p), single_mod: true });
        } else if let Some(r) = all.iter().find(|r| r.name == n) {
            out.push(r.clone());
        } else {
            eprintln!("unknown root {n}");
            std::process::exit(2);
        }
    }
    out
}

/// Candidate mod directories of a root (sorted). Loose files are ignored.
pub fn mod_dirs(root: &Root) -> Vec<PathBuf> {
    if root.single_mod {
        return vec![root.path.clone()];
    }
    let mut v: Vec<PathBuf> = match std::fs::read_dir(&root.path) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect(),
        Err(_) => vec![],
    };
    v.sort();
    v
}
