//! `check-source`: scans `crates/**/src` for invariant violations (platform quarantine,
//! write fence, XML string building, printing, licence hygiene).
//!
//! Test code is exempt: files under a `tests` folder, `tests.rs`, and everything after a
//! `#[cfg(test)]` attribute that introduces a `mod` (test modules sit at the end of a file).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::jsonc;
use crate::model::{rel, walk};
use crate::report::Finding;

const RULE_CFG: &str = "source.cfg-target-os";
const RULE_FS: &str = "source.fs-write";
const RULE_XML: &str = "source.xml-string";
const RULE_PRINT: &str = "source.print";
const RULE_VANILLA: &str = "source.vanilla-name";
const RULE_CE: &str = "source.ce-class";

/// Allow lists from `source-allow.jsonc`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Allow {
    /// Rule id (without the `source.` prefix) to path prefixes where it does not apply.
    paths: BTreeMap<String, Vec<String>>,
    /// Single line exceptions: (rule, path prefix, text the line contains).
    lines: Vec<(String, String, String)>,
}

impl Allow {
    /// Parses the JSONC text of `source-allow.jsonc`.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let v = jsonc::parse(text).context("source-allow.jsonc")?;
        let mut allow = Allow::default();
        if let Some(rules) = v.get("rules").and_then(Value::as_object) {
            for (k, r) in rules {
                let list = r
                    .get("paths")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                allow.paths.insert(k.clone(), list);
            }
        }
        for l in v
            .get("lines")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let get = |k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            allow
                .lines
                .push((get("rule"), get("path"), get("contains")));
        }
        Ok(allow)
    }

    fn path_allowed(&self, rule: &str, path: &str) -> bool {
        let key = rule.strip_prefix("source.").unwrap_or(rule);
        self.paths
            .get(key)
            .is_some_and(|ps| ps.iter().any(|p| path.starts_with(p.as_str())))
    }

    fn line_allowed(&self, rule: &str, path: &str, line: &str) -> bool {
        let key = rule.strip_prefix("source.").unwrap_or(rule);
        self.lines
            .iter()
            .any(|(r, p, c)| r == key && path.starts_with(p.as_str()) && line.contains(c.as_str()))
    }
}

/// Scans every Rust file under `crates/*/src`.
pub(crate) fn check(root: &Path, allow: &Allow) -> Vec<Finding> {
    let files = walk(&root.join("crates"), &|p| {
        p.extension().is_some_and(|e| e == "rs") && p.components().any(|c| c.as_os_str() == "src")
    });
    let mut out = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        out.extend(scan_file(&rel(root, &f), &text, allow));
    }
    out
}

fn is_test_file(path: &str) -> bool {
    path.split('/').any(|c| c == "tests" || c == "benches")
        || path.ends_with("/tests.rs")
        || path.ends_with("_tests.rs")
}

/// Index of the first line of the trailing test module, if any.
fn test_region_start(lines: &[&str]) -> Option<usize> {
    for (i, l) in lines.iter().enumerate() {
        if l.trim() != "#[cfg(test)]" {
            continue;
        }
        let next = lines
            .iter()
            .skip(i + 1)
            .map(|x| x.trim())
            .find(|x| !x.starts_with("#["));
        if next.is_some_and(|n| n.starts_with("mod ") || n.starts_with("pub mod ")) {
            return Some(i);
        }
    }
    None
}

/// Splits a line into code and trailing `//` comment, respecting string literals.
fn split_comment(line: &str) -> (&str, &str) {
    let b = line.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'\'' && b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'\'') {
            i += 2;
        } else if c == b'"' {
            in_str = true;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            return (line.get(..i).unwrap_or(line), line.get(i..).unwrap_or(""));
        }
        i += 1;
    }
    (line, "")
}

/// Contents of the single line string literals in a code fragment.
fn string_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut chars = code.chars();
    while let Some(c) = chars.next() {
        if in_str {
            if c == '\\' {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            } else if c == '"' {
                in_str = false;
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_str = true;
        }
    }
    out
}

/// Heuristic: does a string literal build or contain angle bracket markup?
fn looks_like_xml(s: &str) -> bool {
    if s.contains("<?xml") || s.contains("<{") {
        return true;
    }
    let b = s.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c != b'<' {
            continue;
        }
        let rest = &b[i + 1..];
        if rest.first() == Some(&b'/') && rest.get(1).is_some_and(u8::is_ascii_alphabetic) {
            return true;
        }
        if rest.first().is_some_and(u8::is_ascii_alphabetic) {
            let end = rest
                .iter()
                .position(|&x| x == b'>' || x == b'<')
                .unwrap_or(rest.len());
            if end > 0 && rest.get(end) == Some(&b'>') && rest.get(end - 1) == Some(&b'/') {
                return true;
            }
        }
    }
    false
}

const CFG_PATTERNS: [&str; 7] = [
    "target_os",
    "target_family",
    "cfg(windows)",
    "cfg(unix)",
    "cfg!(windows)",
    "cfg!(unix)",
    "not(windows)",
];

const FS_PATTERNS: [&str; 12] = [
    "fs::write(",
    "fs::rename(",
    "fs::remove_file(",
    "fs::remove_dir_all(",
    "fs::remove_dir(",
    "fs::create_dir(",
    "fs::create_dir_all(",
    "fs::copy(",
    "fs::hard_link(",
    "fs::set_permissions(",
    "File::create(",
    "OpenOptions::new(",
];

const PRINT_PATTERNS: [&str; 4] = ["println!", "eprintln!", "print!(", "eprint!("];

const VANILLA_PREFIXES: [&str; 5] = ["Gun_", "Bullet_", "Apparel_", "MeleeWeapon_", "Ammo_"];

fn has_vanilla_name(line: &str) -> bool {
    let b = line.as_bytes();
    for p in VANILLA_PREFIXES {
        let mut from = 0;
        while let Some(pos) = line.get(from..).and_then(|s| s.find(p)) {
            let at = from + pos;
            let before_ok = at == 0
                || b.get(at - 1)
                    .is_some_and(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
            let after = b.get(at + p.len());
            if before_ok && after.is_some_and(u8::is_ascii_uppercase) {
                return true;
            }
            from = at + p.len();
        }
    }
    false
}

/// Scans one file. `path` is repository relative with forward slashes.
pub(crate) fn scan_file(path: &str, text: &str, allow: &Allow) -> Vec<Finding> {
    let mut out = Vec::new();
    if is_test_file(path) {
        return out;
    }
    let lines: Vec<&str> = text.lines().collect();
    let end = test_region_start(&lines).unwrap_or(lines.len());
    for (idx, line) in lines.iter().take(end).enumerate() {
        let (code, _comment) = split_comment(line);
        let mut hit = |rule: &str, msg: &str| {
            if !allow.path_allowed(rule, path) && !allow.line_allowed(rule, path, line) {
                out.push(Finding::new(rule, format!("{path}:{}", idx + 1), msg));
            }
        };
        if CFG_PATTERNS.iter().any(|p| code.contains(p)) {
            hit(
                RULE_CFG,
                "operating system cfg outside rimstudio-platform (I-11)",
            );
        }
        if FS_PATTERNS.iter().any(|p| code.contains(p)) {
            hit(RULE_FS, "file system write outside rimstudio-io (I-05)");
        }
        if PRINT_PATTERNS.iter().any(|p| code.contains(p)) {
            hit(RULE_PRINT, "printing outside rimstudio-cli");
        }
        if string_literals(code).iter().any(|s| looks_like_xml(s)) {
            hit(
                RULE_XML,
                "angle bracket markup in a string outside rimstudio-xml (I-02)",
            );
        }
        if has_vanilla_name(line) {
            hit(
                RULE_VANILLA,
                "vanilla style def name; tests and docs use RS_ names (IT-061)",
            );
        }
        if line.contains("CombatExtended.") {
            hit(
                RULE_CE,
                "CombatExtended class string outside rimstudio-design::ce (IT-061)",
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOW: &str = r#"{
      "rules": {
        "cfg-target-os": {"paths": ["crates/rimstudio-platform/"]},
        "fs-write": {"paths": ["crates/rimstudio-io/"]},
        "xml-string": {"paths": ["crates/rimstudio-xml/"]},
        "print": {"paths": ["crates/rimstudio-cli/"]},
        "vanilla-name": {"paths": ["crates/rimstudio-design/src/ce"]},
        "ce-class": {"paths": ["crates/rimstudio-design/src/ce"]}
      },
      "lines": [{"rule": "xml-string", "path": "crates/a/", "contains": "ALLOWED"}]
    }"#;

    fn scan(path: &str, text: &str) -> Vec<String> {
        let allow = Allow::parse(ALLOW).unwrap();
        scan_file(path, text, &allow)
            .into_iter()
            .map(|f| f.rule)
            .collect()
    }

    #[test]
    fn cfg_target_os_is_flagged_outside_platform() {
        assert_eq!(
            scan("crates/a/src/x.rs", "#[cfg(target_os = \"linux\")]\n"),
            [RULE_CFG]
        );
        assert!(
            scan(
                "crates/rimstudio-platform/src/x.rs",
                "#[cfg(unix)]\n#[cfg(target_os = \"linux\")]"
            )
            .is_empty()
        );
    }

    #[test]
    fn fs_writes_are_flagged_outside_io() {
        assert_eq!(
            scan("crates/a/src/x.rs", "std::fs::write(p, b)?;"),
            [RULE_FS]
        );
        assert_eq!(
            scan("crates/a/src/x.rs", "let f = File::create(p)?;"),
            [RULE_FS]
        );
        assert!(scan("crates/rimstudio-io/src/x.rs", "std::fs::write(p, b)?;").is_empty());
        assert!(scan("crates/a/src/x.rs", "std::fs::read_to_string(p)?;").is_empty());
    }

    #[test]
    fn comments_are_ignored_for_code_rules() {
        assert!(
            scan(
                "crates/a/src/x.rs",
                "// std::fs::write(p, b) and println!(\"x\")"
            )
            .is_empty()
        );
        assert!(scan("crates/a/src/x.rs", "/// use `target_os` here").is_empty());
    }

    #[test]
    fn comment_marker_inside_a_string_does_not_hide_code() {
        assert_eq!(
            scan(
                "crates/a/src/x.rs",
                "let u = \"http://x\"; println!(\"a\");"
            ),
            [RULE_PRINT]
        );
    }

    #[test]
    fn printing_is_flagged_outside_the_cli() {
        assert_eq!(scan("crates/a/src/x.rs", "eprintln!(\"x\");"), [RULE_PRINT]);
        assert!(scan("crates/rimstudio-cli/src/x.rs", "println!(\"x\");").is_empty());
    }

    #[test]
    fn xml_string_construction_is_flagged() {
        assert_eq!(
            scan("crates/a/src/x.rs", "let s = format!(\"<{tag}>\");"),
            [RULE_XML]
        );
        assert_eq!(
            scan("crates/a/src/x.rs", "let s = \"</defName>\";"),
            [RULE_XML]
        );
        assert_eq!(
            scan("crates/a/src/x.rs", "let s = \"<li Class=\\\"X\\\"/>\";"),
            [RULE_XML]
        );
        assert!(
            scan(
                "crates/rimstudio-xml/src/x.rs",
                "let s = \"<?xml version\";"
            )
            .is_empty()
        );
    }

    #[test]
    fn generic_types_in_strings_are_not_xml() {
        assert!(
            scan(
                "crates/a/src/x.rs",
                "let s = \"expected Vec<String> or Option<u8>\";"
            )
            .is_empty()
        );
        assert!(scan("crates/a/src/x.rs", "fn f() -> Vec<String> { vec![] }").is_empty());
    }

    #[test]
    fn line_allow_list_suppresses_one_line() {
        assert!(scan("crates/a/src/x.rs", "let s = \"</x>\"; // ALLOWED").is_empty());
        assert_eq!(
            scan("crates/b/src/x.rs", "let s = \"</x>\"; // ALLOWED"),
            [RULE_XML]
        );
    }

    #[test]
    fn vanilla_like_names_are_flagged_but_fictional_names_pass() {
        assert_eq!(
            scan("crates/a/src/x.rs", "let n = \"Gun_Foo\";"),
            [RULE_VANILLA]
        );
        assert_eq!(
            scan("crates/a/src/x.rs", "// MeleeWeapon_Bar"),
            [RULE_VANILLA]
        );
        assert!(
            scan(
                "crates/a/src/x.rs",
                "let n = \"RS_Gun_Foo\"; let m = \"Gun_lower\";"
            )
            .is_empty()
        );
        assert!(scan("crates/rimstudio-design/src/ce/x.rs", "\"Bullet_Foo\"").is_empty());
    }

    #[test]
    fn ce_class_strings_are_flagged_outside_the_ce_module() {
        assert_eq!(
            scan("crates/a/src/x.rs", "\"CombatExtended.Foo\""),
            [RULE_CE]
        );
        assert!(
            scan(
                "crates/rimstudio-design/src/ce/mod.rs",
                "\"CombatExtended.Foo\""
            )
            .is_empty()
        );
    }

    #[test]
    fn test_files_and_trailing_test_modules_are_exempt() {
        assert!(scan("crates/a/src/tests.rs", "println!(\"x\");").is_empty());
        assert!(scan("crates/a/tests/t.rs", "println!(\"x\");").is_empty());
        let text = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() { println!(\"x\"); }\n}\n";
        assert!(scan("crates/a/src/x.rs", text).is_empty());
        let before = "fn a() { println!(\"x\"); }\n#[cfg(test)]\nmod tests {}\n";
        assert_eq!(scan("crates/a/src/x.rs", before), [RULE_PRINT]);
    }

    #[test]
    fn cfg_test_on_a_non_module_does_not_open_a_test_region() {
        let text = "#[cfg(test)]\nuse foo::bar;\nfn a() { println!(\"x\"); }\n";
        assert_eq!(scan("crates/a/src/x.rs", text), [RULE_PRINT]);
    }

    #[test]
    fn shipped_allow_file_parses() {
        let a = Allow::parse(include_str!("../source-allow.jsonc")).unwrap();
        assert!(a.path_allowed(
            "source.cfg-target-os",
            "crates/rimstudio-platform/src/lib.rs"
        ));
    }
}
