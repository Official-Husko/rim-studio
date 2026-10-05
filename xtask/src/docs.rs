//! `check-docs`: the documentation rules (R12) plus the dash and emoji rule for Rust
//! comments and manifests. A port of the original Python docs linter.
//!
//! Markdown rules (outside fenced code): exactly one H1, no em or en dashes, no emoji, no
//! mention of AI tooling, no leaked Steam account ids, the misnamed old product word only in
//! allowed files, relative links and heading anchors resolve, fences are balanced, research notes
//! carry their closing sections.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::jsonc;
use crate::model::{rel, walk};
use crate::report::Finding;

const EM_DASH: char = '\u{2014}';
const EN_DASH: char = '\u{2013}';

/// Lists of files that may break a rule, from the `docs` object of `source-allow.jsonc`.
#[derive(Debug, Clone, Default)]
pub(crate) struct DocsAllow {
    rimforge: Vec<String>,
    ai: Vec<String>,
}

impl DocsAllow {
    /// Reads the `docs` section of `source-allow.jsonc`.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let v = jsonc::parse(text).context("source-allow.jsonc")?;
        let list = |k: &str| -> Vec<String> {
            v.pointer(&format!("/docs/{k}"))
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(Self {
            rimforge: list("rimforgeAllowedFiles"),
            ai: list("aiMentionAllowedFiles"),
        })
    }
}

/// In memory view of the markdown files plus an existence probe for other link targets.
#[derive(Debug, Default)]
pub(crate) struct DocSet {
    files: BTreeMap<String, String>,
    extra: BTreeSet<String>,
    root: Option<PathBuf>,
}

impl DocSet {
    fn exists(&self, rel_path: &str) -> bool {
        self.files.contains_key(rel_path)
            || self.extra.contains(rel_path)
            || self
                .root
                .as_ref()
                .is_some_and(|r| r.join(rel_path).exists())
    }
}

/// Lexically joins a link to the folder of `from` (both repository relative).
/// Returns `None` when the link escapes the repository.
fn resolve(from: &str, link: &str) -> Option<String> {
    let mut parts: Vec<String> = Path::new(from)
        .parent()
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    for c in Path::new(link).components() {
        match c {
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(n) => parts.push(n.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// GitHub style heading slug.
fn slugify(text: &str) -> String {
    let mut plain = String::new();
    let mut in_code = false;
    for c in text.trim().chars() {
        if c == '`' {
            in_code = !in_code;
        } else {
            plain.push(c);
        }
    }
    let _ = in_code;
    plain
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Fence tracking: yields `(line_number, line, in_fence)`; the closing fence counts as in fence.
fn fenced(text: &str) -> (Vec<(usize, &str, bool)>, bool) {
    let mut out = Vec::new();
    let mut marker: Option<char> = None;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        let fence_char = ['`', '~']
            .into_iter()
            .find(|c| t.starts_with(&c.to_string().repeat(3)));
        match (marker, fence_char) {
            (None, Some(c)) => {
                marker = Some(c);
                out.push((i + 1, line, true));
            }
            (Some(m), Some(c)) if m == c && t.trim_start_matches(c).trim().is_empty() => {
                marker = None;
                out.push((i + 1, line, true));
            }
            (m, _) => out.push((i + 1, line, m.is_some())),
        }
    }
    (out, marker.is_some())
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&level) && line.chars().nth(level) == Some(' ') {
        Some((
            level,
            line.get(level..)
                .unwrap_or("")
                .trim()
                .trim_end_matches('#')
                .trim(),
        ))
    } else {
        None
    }
}

fn anchors_of(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = BTreeSet::new();
    for (_, line, in_fence) in fenced(text).0 {
        if in_fence {
            continue;
        }
        if let Some((_, h)) = heading(line) {
            let s = slugify(h);
            let n = seen.entry(s.clone()).or_insert(0);
            out.insert(if *n == 0 { s } else { format!("{s}-{n}") });
            *n += 1;
        }
    }
    out
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B50 | 0x2B06 | 0x2B55)
}

fn word_at(haystack: &str, term: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack.get(from..).and_then(|s| s.find(term)) {
        let at = from + pos;
        let before = haystack.get(..at).and_then(|s| s.chars().last());
        let after = haystack
            .get(at + term.len()..)
            .and_then(|s| s.chars().next());
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if boundary(before) && boundary(after) {
            return true;
        }
        from = at + term.len();
    }
    false
}

const AI_TERMS: [&str; 18] = [
    "claude",
    "chatgpt",
    "openai",
    "anthropic",
    "copilot",
    "llm",
    "llms",
    "ai assistant",
    "ai assistants",
    "ai assistance",
    "ai-assisted",
    "ai-generated",
    "ai generated",
    "ai tool",
    "ai tools",
    "ai agent",
    "co-authored-by",
    "language model",
];

fn mentions_ai(line: &str) -> bool {
    let lower = line.to_lowercase();
    AI_TERMS.iter().any(|t| word_at(&lower, t))
}

/// An allow list entry matches a file exactly, or a folder when it ends with a slash.
fn listed(list: &[String], path: &str) -> bool {
    list.iter()
        .any(|a| path == a || (a.ends_with('/') && path.starts_with(a.as_str())))
}

/// Text rules shared by markdown, Rust comments and manifests. `text` is one line (or the
/// comment part of it).
fn text_rules(path: &str, ln: usize, text: &str, allow: &DocsAllow, out: &mut Vec<Finding>) {
    let loc = format!("{path}:{ln}");
    if text.contains(EM_DASH) || text.contains(EN_DASH) {
        out.push(Finding::new(
            "docs.dash",
            &loc,
            "em or en dash; use a hyphen, a colon or the word to",
        ));
    }
    if text.chars().any(is_emoji) {
        out.push(Finding::new("docs.emoji", &loc, "emoji character"));
    }
    if text.to_lowercase().contains("rimforge") && !listed(&allow.rimforge, path) {
        out.push(Finding::new(
            "docs.misnamed",
            &loc,
            "the word rimforge outside the allowed files",
        ));
    }
    if mentions_ai(text) && !listed(&allow.ai, path) {
        out.push(Finding::new(
            "docs.ai-mention",
            &loc,
            "mention of an AI tool or assistant",
        ));
    }
}

fn blank_code_spans(line: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Link targets in a line (inline links and images), code spans excluded.
fn link_targets(line: &str) -> Vec<String> {
    let clean = blank_code_spans(line);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = clean.get(from..).and_then(|s| s.find("](")) {
        let start = from + pos + 2;
        let rest = clean.get(start..).unwrap_or("");
        let end = rest
            .find(|c: char| c == ')' || c.is_whitespace())
            .unwrap_or(rest.len());
        if let Some(t) = rest.get(..end)
            && !t.is_empty()
        {
            out.push(t.to_string());
        }
        from = start + end;
    }
    out
}

fn check_link(set: &DocSet, path: &str, ln: usize, target: &str, out: &mut Vec<Finding>) {
    const EXTERNAL: [&str; 5] = ["http://", "https://", "mailto:", "tel:", "data:"];
    if EXTERNAL.iter().any(|p| target.starts_with(p)) {
        return;
    }
    let loc = format!("{path}:{ln}");
    let (path_part, frag) = target.split_once('#').unwrap_or((target, ""));
    let dest = if path_part.is_empty() {
        path.to_string()
    } else {
        match resolve(path, path_part) {
            Some(d) if set.exists(&d) => d,
            _ => {
                out.push(Finding::new(
                    "docs.broken-link",
                    &loc,
                    format!("link does not resolve: {target}"),
                ));
                return;
            }
        }
    };
    if !frag.is_empty()
        && dest.ends_with(".md")
        && let Some(text) = set.files.get(&dest)
        && !anchors_of(text).contains(&frag.to_lowercase())
    {
        out.push(Finding::new(
            "docs.broken-anchor",
            &loc,
            format!("anchor does not resolve: {target}"),
        ));
    }
}

/// Checks one markdown file. `path` is repository relative.
pub(crate) fn check_markdown(set: &DocSet, path: &str, allow: &DocsAllow) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(text) = set.files.get(path) else {
        return out;
    };
    let (lines, unclosed) = fenced(text);
    if unclosed {
        out.push(Finding::new(
            "docs.unclosed-fence",
            path,
            "unbalanced code fence",
        ));
    }
    let mut h1 = 0;
    for (ln, line, in_fence) in lines {
        if in_fence {
            continue;
        }
        if heading(line).is_some_and(|(l, _)| l == 1) {
            h1 += 1;
        }
        text_rules(path, ln, line, allow, &mut out);
        if has_steam_id(line) {
            out.push(Finding::new(
                "docs.steam-id",
                format!("{path}:{ln}"),
                "possible Steam account id",
            ));
        }
        for t in link_targets(line) {
            check_link(set, path, ln, &t, &mut out);
        }
    }
    if h1 != 1 {
        out.push(Finding::new(
            "docs.h1-count",
            path,
            format!("{h1} H1 headings, expected 1"),
        ));
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() == 3 && parts[0] == "docs" && parts[1] == "research" {
        for req in ["## Implications for RimStudio", "## Open questions"] {
            if !text.contains(req) {
                out.push(Finding::new(
                    "docs.missing-section",
                    path,
                    format!("lacks `{req}`"),
                ));
            }
        }
    }
    out
}

fn has_steam_id(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - start == 17 && line.get(start..start + 7) == Some("7656119") {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// Checks the comment part of every line of a Rust file.
pub(crate) fn check_rust_comments(path: &str, text: &str, allow: &DocsAllow) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut in_block = false;
    for (i, line) in text.lines().enumerate() {
        let comment = if in_block {
            if line.contains("*/") {
                in_block = false;
            }
            Some(line)
        } else if let Some(p) = comment_start(line) {
            if line.get(p..).is_some_and(|c| c.starts_with("/*")) && !line.contains("*/") {
                in_block = true;
            }
            line.get(p..)
        } else {
            None
        };
        if let Some(c) = comment {
            text_rules(path, i + 1, c, allow, &mut out);
        }
    }
    out
}

/// Byte index where a comment starts outside a string literal.
fn comment_start(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'/' && matches!(b.get(i + 1), Some(b'/') | Some(b'*')) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Checks a whole manifest file (every line, comments included).
pub(crate) fn check_manifest(path: &str, text: &str, allow: &DocsAllow) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        text_rules(path, i + 1, line, allow, &mut out);
    }
    out
}

/// Runs the docs checks over the repository.
pub(crate) fn check(root: &Path, allow: &DocsAllow) -> Vec<Finding> {
    let mut set = DocSet {
        root: Some(root.to_path_buf()),
        ..DocSet::default()
    };
    let mut md: Vec<PathBuf> = walk(&root.join("docs"), &|p| {
        p.extension().is_some_and(|e| e == "md")
    });
    for name in ["CLAUDE.md", "README.md"] {
        if root.join(name).exists() {
            md.push(root.join(name));
        }
    }
    for p in &md {
        if let Ok(t) = std::fs::read_to_string(p) {
            set.files.insert(rel(root, p), t);
        }
    }
    let mut out = Vec::new();
    for path in set.files.keys() {
        out.extend(check_markdown(&set, path, allow));
    }
    let rs = walk(&root.join("crates"), &|p| {
        p.extension().is_some_and(|e| e == "rs")
    });
    let xtask_rs = walk(&root.join("xtask"), &|p| {
        p.extension().is_some_and(|e| e == "rs")
    });
    for f in rs.iter().chain(&xtask_rs) {
        if let Ok(t) = std::fs::read_to_string(f) {
            out.extend(check_rust_comments(&rel(root, f), &t, allow));
        }
    }
    let mut manifests = vec![root.join("Cargo.toml"), root.join("deny.toml")];
    manifests.extend(walk(&root.join("crates"), &|p| {
        p.file_name().is_some_and(|n| n == "Cargo.toml")
    }));
    manifests.extend(walk(&root.join("xtask"), &|p| {
        p.file_name().is_some_and(|n| n == "Cargo.toml")
    }));
    for f in manifests {
        if let Ok(t) = std::fs::read_to_string(&f) {
            out.extend(check_manifest(&rel(root, &f), &t, allow));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow() -> DocsAllow {
        DocsAllow {
            rimforge: vec!["docs/old.md".to_string()],
            ai: vec![],
        }
    }

    fn set(files: &[(&str, &str)]) -> DocSet {
        DocSet {
            files: files
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
            extra: ["docs/data.json".to_string()].into_iter().collect(),
            root: None,
        }
    }

    fn rules_of(files: &[(&str, &str)], path: &str) -> Vec<String> {
        check_markdown(&set(files), path, &allow())
            .into_iter()
            .map(|f| f.rule)
            .collect()
    }

    #[test]
    fn clean_markdown_has_no_findings() {
        assert!(
            rules_of(
                &[
                    (
                        "docs/a.md",
                        "# Title\n\ntext and a [link](b.md#second-part)\n"
                    ),
                    ("docs/b.md", "# B\n## Second part\n")
                ],
                "docs/a.md"
            )
            .is_empty()
        );
    }

    #[test]
    fn dashes_are_flagged_outside_fences_only() {
        let text =
            format!("# T\nan {EM_DASH} dash and an {EN_DASH} one\n```\nin {EM_DASH} fence\n```\n");
        assert_eq!(
            rules_of(&[("docs/a.md", &text)], "docs/a.md"),
            ["docs.dash"]
        );
    }

    #[test]
    fn emoji_are_flagged() {
        let text = "# T\nok \u{1F600} bad\n";
        assert_eq!(
            rules_of(&[("docs/a.md", text)], "docs/a.md"),
            ["docs.emoji"]
        );
    }

    #[test]
    fn ai_mentions_are_flagged_on_word_boundaries() {
        assert_eq!(
            rules_of(&[("docs/a.md", "# T\nwritten by Claude\n")], "docs/a.md"),
            ["docs.ai-mention"]
        );
        assert!(rules_of(&[("docs/a.md", "# T\nthe claudette module\n")], "docs/a.md").is_empty());
    }

    #[test]
    fn misnamed_product_is_flagged_except_in_allowed_files() {
        assert_eq!(
            rules_of(&[("docs/a.md", "# T\nRimForge\n")], "docs/a.md"),
            ["docs.misnamed"]
        );
        assert!(
            rules_of(
                &[("docs/old.md", "# T\nthe rimforge folder\n")],
                "docs/old.md"
            )
            .is_empty()
        );
    }

    #[test]
    fn broken_links_and_anchors_are_flagged() {
        let files = [
            (
                "docs/a.md",
                "# T\n[x](missing.md) [y](b.md#nope) [z](#t) [w](data.json) [v](../../x.md)\n",
            ),
            ("docs/b.md", "# B\n"),
        ];
        assert_eq!(
            rules_of(&files, "docs/a.md"),
            ["docs.broken-link", "docs.broken-anchor", "docs.broken-link"]
        );
    }

    #[test]
    fn links_inside_code_spans_and_external_links_are_ignored() {
        assert!(
            rules_of(
                &[(
                    "docs/a.md",
                    "# T\n`[x](nope.md)` [e](https://example.com/a)\n"
                )],
                "docs/a.md"
            )
            .is_empty()
        );
    }

    #[test]
    fn duplicate_headings_get_numbered_anchors() {
        let files = [
            ("docs/a.md", "# T\n[x](b.md#same-1)\n"),
            ("docs/b.md", "# B\n## Same\n## Same\n"),
        ];
        assert!(rules_of(&files, "docs/a.md").is_empty());
    }

    #[test]
    fn heading_count_and_fences_are_checked() {
        assert_eq!(
            rules_of(&[("docs/a.md", "text\n```\nopen\n")], "docs/a.md"),
            ["docs.unclosed-fence", "docs.h1-count"]
        );
        assert_eq!(
            rules_of(&[("docs/a.md", "# A\n# B\n")], "docs/a.md"),
            ["docs.h1-count"]
        );
    }

    #[test]
    fn research_notes_need_their_closing_sections() {
        assert_eq!(
            rules_of(&[("docs/research/n.md", "# N\n")], "docs/research/n.md"),
            ["docs.missing-section", "docs.missing-section"]
        );
    }

    #[test]
    fn steam_ids_are_flagged() {
        assert_eq!(
            rules_of(
                &[("docs/a.md", "# T\nid 76561198000000000 here\n")],
                "docs/a.md"
            ),
            ["docs.steam-id"]
        );
    }

    #[test]
    fn rust_comments_are_checked_but_code_strings_are_not() {
        let text = format!(
            "// a {EM_DASH} b\nlet s = \"{EN_DASH}\"; // ok\nlet t = 1; /* {EN_DASH}\nmore {EN_DASH} */\n"
        );
        let f = check_rust_comments("crates/a/src/x.rs", &text, &allow());
        let lines: Vec<&str> = f.iter().map(|x| x.location.as_str()).collect();
        assert_eq!(
            lines,
            [
                "crates/a/src/x.rs:1",
                "crates/a/src/x.rs:3",
                "crates/a/src/x.rs:4"
            ]
        );
    }

    #[test]
    fn manifests_are_checked_on_every_line() {
        let f = check_manifest(
            "Cargo.toml",
            &format!("description = \"a {EM_DASH} b\"\n"),
            &allow(),
        );
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn shipped_allow_file_lists_the_docs_exceptions() {
        let a = DocsAllow::parse(include_str!("../source-allow.jsonc")).unwrap();
        assert!(listed(&a.rimforge, "CLAUDE.md"));
        assert!(!listed(&a.rimforge, "docs/features/x.md"));
    }

    #[test]
    fn allow_entries_match_files_and_folders() {
        let l = vec!["a/b.md".to_string(), "docs/research/".to_string()];
        assert!(listed(&l, "a/b.md"));
        assert!(listed(&l, "docs/research/x.md"));
        assert!(!listed(&l, "docs/researchx.md"));
        assert!(!listed(&l, "a/b.mdx"));
    }

    #[test]
    fn slugs_follow_github_rules() {
        assert_eq!(
            slugify("4. Allowed-dependency `matrix`!"),
            "4-allowed-dependency-matrix"
        );
    }

    #[test]
    fn resolve_normalises_and_rejects_escapes() {
        assert_eq!(
            resolve("docs/a/b.md", "../c.md").as_deref(),
            Some("docs/c.md")
        );
        assert_eq!(resolve("a.md", "../c.md"), None);
    }
}
