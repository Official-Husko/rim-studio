//! `check-licences`: the RimWorld and Combat Extended data that the repository includes carries
//! its attribution (R11, ADR 0059).
//!
//! The facts live in `xtask/third-party.jsonc`: the sources (holder, official link, texts that must
//! be printed), the sets of included data by path, the reference trees that must stay out of the
//! index, and the binary assets that are the project's own. The check reads the tracked files and
//! verifies that
//!
//! 1. the manifest is complete (every source has a holder and an https link, every set names known
//!    sources and at least one path),
//! 2. the notice file names every source with its holder, link and required texts, and documents
//!    every set by id and path,
//! 3. every set still matches tracked files (an entry that matches nothing is stale),
//! 4. data files that name game or Combat Extended definitions belong to a set,
//! 5. no file of a reference project is tracked, and
//! 6. no binary asset (art, audio, code) is tracked unless it is listed as the project's own.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::jsonc;
use crate::model::{rel, walk};
use crate::report::Finding;

/// The manifest itself names the markers it scans for, so the scan skips it.
const MANIFEST_PATH: &str = "xtask/third-party.jsonc";

const RULE_MANIFEST: &str = "licences.manifest";
const RULE_NOTICE: &str = "licences.notice";
const RULE_ATTRIBUTION: &str = "licences.attribution";
const RULE_SET: &str = "licences.set";
const RULE_DATA: &str = "licences.undeclared-data";
const RULE_TREE: &str = "licences.reference-tree";
const RULE_ASSET: &str = "licences.asset";

/// File extensions of binary assets: game art, audio and code stay out of the repository.
const ASSET_EXTENSIONS: [&str; 20] = [
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "icns", "psd", "ogg", "wav", "mp3", "flac",
    "dll", "so", "dylib", "exe", "bundle", "assets", "unity3d",
];

/// One rights holder whose data the repository includes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Source {
    id: String,
    name: String,
    holder: String,
    link: String,
    /// Texts that must appear word for word in the notice (a disclaimer, a licence link).
    required: Vec<String>,
}

/// Included data, by where it lives.
#[derive(Debug, Clone, Default)]
pub(crate) struct DataSet {
    id: String,
    from: Vec<String>,
    paths: Vec<String>,
    what: String,
}

/// The parsed `third-party.jsonc`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Manifest {
    notice: String,
    sources: Vec<Source>,
    sets: Vec<DataSet>,
    reference_trees: Vec<String>,
    extensions: Vec<String>,
    prefixes: Vec<String>,
    contains: Vec<String>,
    own_assets: Vec<String>,
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

fn text_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

impl Manifest {
    /// Parses the JSONC text of `third-party.jsonc`. Missing fields stay empty and are reported
    /// by [`check`] as findings, so one run lists every problem.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let v = jsonc::parse(text).context("third-party.jsonc")?;
        let mut m = Manifest {
            notice: text_of(&v, "notice"),
            reference_trees: strings(v.get("referenceTrees")),
            own_assets: strings(v.get("ownAssets")),
            ..Manifest::default()
        };
        for s in v
            .get("sources")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            m.sources.push(Source {
                id: text_of(s, "id"),
                name: text_of(s, "name"),
                holder: text_of(s, "holder"),
                link: text_of(s, "link"),
                required: strings(s.get("required")),
            });
        }
        for s in v
            .get("sets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            m.sets.push(DataSet {
                id: text_of(s, "id"),
                from: strings(s.get("from")),
                paths: strings(s.get("paths")),
                what: text_of(s, "what"),
            });
        }
        if let Some(scan) = v.get("scan") {
            m.extensions = strings(scan.get("extensions"));
            m.prefixes = strings(scan.get("prefixes"));
            m.contains = strings(scan.get("contains"));
        }
        Ok(m)
    }
}

// ---------- path patterns ----------

/// Matches one path segment against a pattern segment with `*` (any run) and `?` (one character).
fn segment_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        match p.get(pi) {
            Some('*') => {
                star = Some(pi);
                mark = ti;
                pi += 1;
            }
            Some(&c) if c == '?' || Some(&c) == t.get(ti) => {
                pi += 1;
                ti += 1;
            }
            _ => match star {
                Some(s) => {
                    pi = s + 1;
                    mark += 1;
                    ti = mark;
                }
                None => return false,
            },
        }
    }
    while p.get(pi) == Some(&'*') {
        pi += 1;
    }
    pi == p.len()
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| match_segments(rest, &path[skip..])),
        Some((first, rest)) => match path.split_first() {
            Some((seg, tail)) => segment_match(first, seg) && match_segments(rest, tail),
            None => false,
        },
    }
}

/// True when `path` (repository relative, forward slashes) matches the glob `pattern`:
/// `**` spans any number of folders, `*` and `?` stay inside one name.
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    let p: Vec<&str> = pattern.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    match_segments(&p, &s)
}

fn extension_of(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

// ---------- markers ----------

/// True when `line` holds a definition name that starts with `prefix` and continues with a capital
/// letter, not preceded by a name character (`Gun_BoltAction` yes, `Shotgun_pump` no).
fn has_prefixed_name(line: &str, prefix: &str) -> bool {
    let b = line.as_bytes();
    let mut from = 0;
    while let Some(pos) = line.get(from..).and_then(|s| s.find(prefix)) {
        let at = from + pos;
        let before_ok = at == 0
            || b.get(at - 1)
                .is_some_and(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        let after = b.get(at + prefix.len());
        if before_ok && after.is_some_and(u8::is_ascii_uppercase) {
            return true;
        }
        from = at + prefix.len();
    }
    false
}

/// The first line of `text` that names a game or Combat Extended definition, with its 1 based number.
fn first_marker(m: &Manifest, text: &str) -> Option<usize> {
    text.lines().enumerate().find_map(|(i, line)| {
        let hit = m.prefixes.iter().any(|p| has_prefixed_name(line, p))
            || m.contains.iter().any(|c| line.contains(c.as_str()));
        hit.then_some(i + 1)
    })
}

// ---------- the check ----------

/// Repository files, relative with forward slashes: tracked files and new files that git does
/// not ignore, so the check also sees a file before its first `git add`. Falls back to walking
/// the tree when git is not available.
fn tracked_files(root: &Path) -> Vec<String> {
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output();
    if let Ok(out) = listed
        && out.status.success()
    {
        return String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    walk(root, &|_| true).iter().map(|p| rel(root, p)).collect()
}

/// Runs the check over the repository at `root`.
pub(crate) fn check(root: &Path, manifest: &Manifest) -> Vec<Finding> {
    let files = tracked_files(root);
    let notice = std::fs::read_to_string(root.join(&manifest.notice)).ok();
    check_files(manifest, &files, notice.as_deref(), &|p| {
        std::fs::read_to_string(root.join(p)).ok()
    })
}

fn manifest_findings(m: &Manifest, out: &mut Vec<Finding>) {
    let mut bad = |msg: String| out.push(Finding::new(RULE_MANIFEST, MANIFEST_PATH, msg));
    if m.notice.is_empty() {
        bad("`notice` names no file".to_string());
    }
    if m.sources.is_empty() {
        bad("no `sources`".to_string());
    }
    let mut seen = Vec::new();
    for s in &m.sources {
        if s.id.is_empty() || s.name.is_empty() || s.holder.is_empty() {
            bad(format!("source `{}` needs id, name and holder", s.id));
        }
        if !s.link.starts_with("https://") {
            bad(format!(
                "source `{}` needs an https link to the official project",
                s.id
            ));
        }
        if seen.contains(&s.id) {
            bad(format!("source id `{}` is used twice", s.id));
        }
        seen.push(s.id.clone());
    }
    let mut set_ids = Vec::new();
    for set in &m.sets {
        if set.id.is_empty() || set.what.is_empty() || set.paths.is_empty() {
            bad(format!("set `{}` needs id, what and paths", set.id));
        }
        if set.from.is_empty() {
            bad(format!("set `{}` names no source in `from`", set.id));
        }
        for f in &set.from {
            if !m.sources.iter().any(|s| &s.id == f) {
                bad(format!("set `{}` names the unknown source `{f}`", set.id));
            }
        }
        if set_ids.contains(&set.id) {
            bad(format!("set id `{}` is used twice", set.id));
        }
        set_ids.push(set.id.clone());
    }
    if m.extensions.is_empty() {
        bad("`scan.extensions` is empty, so no data file would be scanned".to_string());
    }
}

fn notice_findings(m: &Manifest, files: &[String], notice: Option<&str>, out: &mut Vec<Finding>) {
    let Some(text) = notice else {
        out.push(Finding::new(
            RULE_NOTICE,
            m.notice.clone(),
            "the notice file cannot be read; it carries the attribution of every source",
        ));
        return;
    };
    if !files.iter().any(|f| f == &m.notice) {
        out.push(Finding::new(
            RULE_NOTICE,
            m.notice.clone(),
            "the notice file is not in the repository (missing, or ignored by git)",
        ));
    }
    for s in &m.sources {
        let wanted = [
            ("name", s.name.as_str()),
            ("holder", s.holder.as_str()),
            ("official link", s.link.as_str()),
        ];
        for (what, value) in wanted {
            if !value.is_empty() && !text.contains(value) {
                out.push(Finding::new(
                    RULE_ATTRIBUTION,
                    m.notice.clone(),
                    format!(
                        "source `{}`: the {what} `{value}` is not in the notice",
                        s.id
                    ),
                ));
            }
        }
        for req in &s.required {
            if !text.contains(req.as_str()) {
                out.push(Finding::new(
                    RULE_ATTRIBUTION,
                    m.notice.clone(),
                    format!(
                        "source `{}`: required text missing from the notice: {}",
                        s.id,
                        shorten(req)
                    ),
                ));
            }
        }
    }
    for set in &m.sets {
        if !set.id.is_empty() && !text.contains(set.id.as_str()) {
            out.push(Finding::new(
                RULE_ATTRIBUTION,
                m.notice.clone(),
                format!("set `{}` is not documented in the notice", set.id),
            ));
        }
        for p in &set.paths {
            if !text.contains(p.as_str()) {
                out.push(Finding::new(
                    RULE_ATTRIBUTION,
                    m.notice.clone(),
                    format!("set `{}`: the path `{p}` is not in the notice", set.id),
                ));
            }
        }
    }
}

fn shorten(text: &str) -> String {
    let mut it = text.chars();
    let head: String = it.by_ref().take(60).collect();
    if it.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

/// The pure part of the check: tracked files, the notice text and a reader of file contents.
pub(crate) fn check_files(
    m: &Manifest,
    files: &[String],
    notice: Option<&str>,
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    manifest_findings(m, &mut out);
    notice_findings(m, files, notice, &mut out);

    // stale set entries
    for set in &m.sets {
        for pattern in &set.paths {
            if !files.iter().any(|f| glob_match(pattern, f)) {
                out.push(Finding::new(
                    RULE_SET,
                    format!("{MANIFEST_PATH}: {}", set.id),
                    format!("`{pattern}` matches no tracked file"),
                ));
            }
        }
    }

    let in_set = |path: &str| {
        m.sets
            .iter()
            .any(|s| s.paths.iter().any(|p| glob_match(p, path)))
    };

    for file in files {
        // reference projects are read, never tracked
        if m.reference_trees
            .iter()
            .any(|t| file.starts_with(t.as_str()))
        {
            out.push(Finding::new(
                RULE_TREE,
                file.clone(),
                "file of a reference project is tracked; reference trees are read only and stay outside the index",
            ));
            continue;
        }
        let ext = extension_of(file);
        // game art, audio and code stay out unless they are the project's own
        if ASSET_EXTENSIONS.contains(&ext.as_str())
            && !m.own_assets.iter().any(|p| glob_match(p, file))
        {
            out.push(Finding::new(
                RULE_ASSET,
                file.clone(),
                "binary asset that is not listed in `ownAssets`; art, audio and code of RimWorld and Combat Extended are not included",
            ));
            continue;
        }
        // data that names game or Combat Extended definitions must be an attributed set
        if m.extensions.contains(&ext) && file != MANIFEST_PATH && !in_set(file) {
            let hit = read(file).and_then(|text| first_marker(m, &text));
            if let Some(line) = hit {
                out.push(Finding::new(
                    RULE_DATA,
                    format!("{file}:{line}"),
                    "names a game or Combat Extended definition but is not part of a set in xtask/third-party.jsonc",
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"{
      // test manifest
      "notice": "NOTICE.md",
      "sources": [
        {"id": "game", "name": "TestGame", "holder": "Studio Inc.", "link": "https://game.example/",
         "required": ["Not official."]},
        {"id": "mod", "name": "TestMod", "holder": "Mod Team", "link": "https://mod.example/repo",
         "required": ["https://licence.example/by-nc-sa/4.0/"]}
      ],
      "sets": [
        {"id": "fixtures", "from": ["game", "mod"], "paths": ["fixtures/**"], "what": "recorded answers"},
        {"id": "goldens", "from": ["mod"], "paths": ["crates/*/tests/**"], "what": "outputs"}
      ],
      "referenceTrees": ["RefProject-main/", "reference/"],
      "scan": {"extensions": ["json", "xml"], "prefixes": ["Gun_", "AmmoSet_"], "contains": ["CombatExtended."]},
      "ownAssets": ["icons/**"]
    }"#;

    const NOTICE: &str = "# Notices\n\
        TestGame by Studio Inc. https://game.example/ Not official.\n\
        TestMod by Mod Team https://mod.example/repo https://licence.example/by-nc-sa/4.0/\n\
        fixtures fixtures/**\n\
        goldens crates/*/tests/**\n";

    fn manifest() -> Manifest {
        Manifest::parse(TEXT).unwrap()
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn run(list: &[&str], notice: Option<&str>, read: &[(&str, &str)]) -> Vec<Finding> {
        let reads: Vec<(String, String)> = read
            .iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect();
        check_files(&manifest(), &files(list), notice, &|p| {
            reads.iter().find(|(a, _)| a == p).map(|(_, b)| b.clone())
        })
    }

    fn rules(found: &[Finding]) -> Vec<&str> {
        let mut r: Vec<&str> = found.iter().map(|f| f.rule.as_str()).collect();
        r.sort_unstable();
        r
    }

    const GOOD: [&str; 4] = [
        "NOTICE.md",
        "fixtures/a.json",
        "crates/x/tests/golden.json",
        "icons/app.png",
    ];

    #[test]
    fn a_complete_repository_passes() {
        assert!(run(&GOOD, Some(NOTICE), &[]).is_empty());
    }

    #[test]
    fn glob_patterns_match_folders_and_names() {
        assert!(glob_match("docs/**", "docs/a/b.md"));
        assert!(glob_match("docs/**", "docs/a.md"));
        assert!(glob_match("crates/*/tests/**", "crates/x/tests/g/h.json"));
        assert!(!glob_match("crates/*/tests/**", "crates/x/y/tests/g.json"));
        assert!(glob_match(
            "docs/research/data/**/*.png",
            "docs/research/data/a/b/c.png"
        ));
        assert!(glob_match("README.md", "README.md"));
        assert!(!glob_match("README.md", "docs/README.md"));
        assert!(glob_match("a/?.json", "a/b.json"));
        assert!(!glob_match("a/*.json", "a/b.xml"));
    }

    #[test]
    fn prefixed_names_need_a_boundary_and_a_capital() {
        assert!(has_prefixed_name(
            "\"def\": \"Gun_BoltActionRifle\"",
            "Gun_"
        ));
        assert!(has_prefixed_name("Gun_Revolver", "Gun_"));
        assert!(!has_prefixed_name("Shotgun_Pump", "Gun_"));
        assert!(!has_prefixed_name("Gun_lower", "Gun_"));
        assert!(!has_prefixed_name("a gun_Name", "Gun_"));
    }

    #[test]
    fn a_missing_notice_is_one_finding() {
        let found = run(&GOOD, None, &[]);
        assert_eq!(rules(&found), ["licences.notice"]);
    }

    #[test]
    fn the_notice_must_name_every_source_link_and_required_text() {
        let notice = "# Notices\nTestGame https://game.example/ fixtures fixtures/** goldens crates/*/tests/**\n";
        let found = run(&GOOD, Some(notice), &[]);
        let text: Vec<String> = found.iter().map(ToString::to_string).collect();
        assert!(
            text.iter().any(|t| t.contains("holder `Studio Inc.`")),
            "{text:?}"
        );
        assert!(text.iter().any(|t| t.contains("Not official.")), "{text:?}");
        assert!(
            text.iter().any(|t| t.contains("source `mod`: the name")),
            "{text:?}"
        );
        assert!(rules(&found).iter().all(|r| *r == "licences.attribution"));
    }

    #[test]
    fn every_set_is_documented_by_id_and_path() {
        let notice = "TestGame Studio Inc. https://game.example/ Not official. \
            TestMod Mod Team https://mod.example/repo https://licence.example/by-nc-sa/4.0/ fixtures";
        let found = run(&GOOD, Some(notice), &[]);
        let text: Vec<String> = found.iter().map(ToString::to_string).collect();
        assert!(
            text.iter().any(|t| t.contains("path `fixtures/**`")),
            "{text:?}"
        );
        assert!(
            text.iter()
                .any(|t| t.contains("set `goldens` is not documented")),
            "{text:?}"
        );
    }

    #[test]
    fn a_set_that_matches_nothing_is_stale() {
        let found = run(
            &["NOTICE.md", "fixtures/a.json", "icons/app.png"],
            Some(NOTICE),
            &[],
        );
        assert_eq!(rules(&found), ["licences.set"]);
        assert!(found[0].message.contains("crates/*/tests/**"));
    }

    #[test]
    fn data_with_game_names_outside_a_set_is_flagged_with_its_line() {
        let mut list = GOOD.to_vec();
        list.push("tables/weapons.json");
        list.push("tables/own.json");
        let found = run(
            &list,
            Some(NOTICE),
            &[
                (
                    "tables/weapons.json",
                    "{\n  \"a\": 1,\n  \"d\": \"Gun_BoltActionRifle\"\n}",
                ),
                ("tables/own.json", "{\"a\": \"RS_Rifle\"}"),
            ],
        );
        assert_eq!(rules(&found), ["licences.undeclared-data"]);
        assert_eq!(found[0].location, "tables/weapons.json:3");
    }

    #[test]
    fn combat_extended_class_strings_count_as_markers() {
        let mut list = GOOD.to_vec();
        list.push("x/patch.xml");
        let found = run(
            &list,
            Some(NOTICE),
            &[("x/patch.xml", "<value>CombatExtended.Verb_ShootCE</value>")],
        );
        assert_eq!(rules(&found), ["licences.undeclared-data"]);
    }

    #[test]
    fn data_inside_a_set_and_other_file_types_are_not_scanned() {
        let mut list = GOOD.to_vec();
        list.push("notes/readme.md");
        let found = run(
            &list,
            Some(NOTICE),
            &[
                ("fixtures/a.json", "Gun_BoltActionRifle"),
                ("notes/readme.md", "Gun_BoltActionRifle"),
            ],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn reference_trees_must_not_be_tracked() {
        let mut list = GOOD.to_vec();
        list.push("RefProject-main/src/lib.py");
        list.push("reference/notes.txt");
        let found = run(&list, Some(NOTICE), &[]);
        assert_eq!(
            rules(&found),
            ["licences.reference-tree", "licences.reference-tree"]
        );
    }

    #[test]
    fn binary_assets_must_be_the_projects_own() {
        let mut list = GOOD.to_vec();
        list.push("art/Rifle.PNG");
        list.push("sounds/shot.ogg");
        list.push("lib/Assembly-CSharp.dll");
        let found = run(&list, Some(NOTICE), &[]);
        assert_eq!(rules(&found), ["licences.asset"; 3]);
    }

    #[test]
    fn manifest_problems_are_all_listed() {
        let bad = Manifest::parse(
            r#"{"notice": "", "sources": [{"id": "a", "name": "A", "holder": "", "link": "http://x"},
                {"id": "a", "name": "B", "holder": "H", "link": "https://b"}],
               "sets": [{"id": "s", "from": ["zzz"], "paths": [], "what": ""}], "scan": {}}"#,
        )
        .unwrap();
        let found = check_files(&bad, &[], Some("x"), &|_| None);
        let text: Vec<String> = found.iter().map(ToString::to_string).collect();
        for needle in [
            "`notice` names no file",
            "needs id, name and holder",
            "needs an https link",
            "used twice",
            "unknown source `zzz`",
            "needs id, what and paths",
            "`scan.extensions` is empty",
        ] {
            assert!(
                text.iter().any(|t| t.contains(needle)),
                "{needle}: {text:?}"
            );
        }
    }

    #[test]
    fn invalid_jsonc_is_an_error() {
        assert!(Manifest::parse("{ nope").is_err());
    }

    #[test]
    fn the_repository_manifest_parses_and_names_both_sources() {
        let text = include_str!("../third-party.jsonc");
        let m = Manifest::parse(text).unwrap();
        let ids: Vec<&str> = m.sources.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["rimworld", "combat-extended"]);
        assert!(m.sets.iter().all(|s| !s.paths.is_empty()));
    }
}
