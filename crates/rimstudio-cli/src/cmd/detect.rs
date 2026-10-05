//! `detect`: what was found (install, version, libraries, Workshop folder, custom folders) and how.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::DetectArgs;
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bool_at, str_at, u64_at};
use crate::session::{Session, absolute};

/// A field of a response as text: a string as is, anything else as compact JSON.
pub(crate) fn plain(v: &Value, pointer: &str) -> String {
    match v.pointer(pointer) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn field_of(name: &str) -> CliResult<&'static str> {
    match name {
        "install" | "game-install" => Ok("game-install"),
        "user-dir" => Ok("user-dir"),
        "steam-root" => Ok("steam-root"),
        other => Err(CliError::usage(format!(
            "unknown override `{other}`; use install, user-dir or steam-root"
        ))),
    }
}

fn set_override(s: &Session, field: &str, path: Option<String>) -> CliResult<Value> {
    let mut request = json!({"field": field});
    if let Some(path) = path {
        request["path"] = Value::String(path);
    }
    let reply = s.call("detect_set_override", request)?;
    Ok(reply.value.get("report").cloned().unwrap_or(reply.value))
}

/// Renders a detection report for people.
#[must_use]
pub(crate) fn render_report(report: &Value, custom: &[Value]) -> String {
    let mut out = String::new();
    let selected = str_at(report, "/selected/install");
    let installs = arr_at(report, "/installs");
    let _ = writeln!(out, "System: {}", plain(report, "/os"));
    if installs.is_empty() {
        let _ = writeln!(out, "Game install: none found");
        let _ = writeln!(
            out,
            "  set one with: rimstudio-cli detect --install PATH-TO-RIMWORLD"
        );
    }
    for install in installs {
        let chosen = str_at(install, "/id") == selected;
        let _ = writeln!(
            out,
            "Game install{}: {}",
            if chosen { " (selected)" } else { "" },
            str_at(install, "/gameRoot")
        );
        let _ = writeln!(
            out,
            "  found by: {} ({} confidence), kind {}, health {}",
            plain(install, "/how"),
            plain(install, "/confidence"),
            plain(install, "/kind"),
            plain(install, "/health"),
        );
        let version = str_at(install, "/version/raw");
        let _ = writeln!(
            out,
            "  version: {}",
            if version.is_empty() {
                "unknown"
            } else {
                version
            }
        );
        let _ = writeln!(
            out,
            "  mods folder: {} ({})",
            str_at(install, "/modsDir/path"),
            if bool_at(install, "/modsDir/exists") {
                "present"
            } else {
                "missing"
            }
        );
        for w in arr_at(install, "/workshop") {
            let _ = writeln!(
                out,
                "  workshop folder: {} ({} items)",
                str_at(w, "/contentDir"),
                u64_at(w, "/itemsOnDisk")
            );
        }
    }
    let libraries = arr_at(report, "/libraries");
    if !libraries.is_empty() {
        let _ = writeln!(out, "Steam libraries:");
        for l in libraries {
            let _ = writeln!(
                out,
                "  {} ({}, {})",
                str_at(l, "/path"),
                if bool_at(l, "/online") {
                    "online"
                } else {
                    "offline"
                },
                if bool_at(l, "/hasApp") {
                    "has RimWorld"
                } else {
                    "no RimWorld"
                },
            );
        }
    }
    for u in arr_at(report, "/userDirs") {
        let _ = writeln!(
            out,
            "User data: {} (ModsConfig {})",
            str_at(u, "/path"),
            if bool_at(u, "/modsConfigExists") {
                "present"
            } else {
                "missing"
            }
        );
    }
    if custom.is_empty() {
        let _ = writeln!(
            out,
            "Custom folders: none (add one with: rimstudio-cli sources add PATH)"
        );
    } else {
        let _ = writeln!(out, "Custom folders:");
        for c in custom {
            let _ = writeln!(out, "  {} ({})", str_at(c, "/path"), str_at(c, "/label"));
        }
    }
    for w in arr_at(report, "/warnings") {
        let _ = writeln!(
            out,
            "warning {}: {}",
            str_at(w, "/code"),
            str_at(w, "/message")
        );
    }
    out
}

/// `detect`: applies any override, then prints the report.
///
/// # Errors
/// A usage error for an unknown override name, the envelope of a failing command.
pub(crate) fn run(s: &Session, args: &DetectArgs) -> CliResult {
    let mut report: Option<Value> = None;
    for (field, path) in [
        ("game-install", &args.install),
        ("user-dir", &args.user_dir),
        ("steam-root", &args.steam_root),
    ] {
        if let Some(path) = path {
            report = Some(set_override(s, field, Some(absolute(path)?))?);
        }
    }
    for name in &args.clear {
        report = Some(set_override(s, field_of(name)?, None)?);
    }
    let report = match report {
        Some(r) if !args.force => r,
        _ => s.call("detect_run", json!({"force": args.force}))?.value,
    };
    let sources = s.call("sources_list", json!({}))?.value;
    let custom: Vec<Value> = arr_at(&sources, "/sources")
        .iter()
        .filter(|x| str_at(x, "/kind") == "custom")
        .cloned()
        .collect();
    let mut doc = report.clone();
    doc["customFolders"] = Value::Array(custom.clone());
    s.emit(&doc, || render_report(&report, &custom));
    if !arr_at(&report, "/warnings").is_empty() {
        s.note_warning();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_without_installs_says_how_to_set_one() {
        let text = render_report(
            &json!({"os": "linux", "installs": [], "libraries": []}),
            &[],
        );
        assert!(text.contains("Game install: none found"));
        assert!(text.contains("--install"));
    }

    #[test]
    fn an_install_lists_version_how_and_workshop() {
        let report = json!({
            "os": "linux",
            "selected": {"install": "i1"},
            "installs": [{
                "id": "i1", "gameRoot": "/g", "how": "override", "confidence": "high",
                "kind": "override", "health": "ok",
                "version": {"raw": "1.2.3 rev4"},
                "modsDir": {"path": "/g/Mods", "exists": true},
                "workshop": [{"contentDir": "/w", "itemsOnDisk": 3}],
            }],
            "warnings": [{"code": "detect.x", "message": "m"}],
        });
        let text = render_report(&report, &[json!({"path": "/c", "label": "Mine"})]);
        assert!(text.contains("Game install (selected): /g"), "{text}");
        assert!(text.contains("found by: override (high confidence)"));
        assert!(text.contains("version: 1.2.3 rev4"));
        assert!(text.contains("workshop folder: /w (3 items)"));
        assert!(text.contains("/c (Mine)"));
        assert!(text.contains("warning detect.x: m"));
    }

    #[test]
    fn override_names_are_checked() {
        assert!(field_of("install").is_ok());
        assert!(field_of("steam-root").is_ok());
        assert!(field_of("bogus").is_err());
    }
}
