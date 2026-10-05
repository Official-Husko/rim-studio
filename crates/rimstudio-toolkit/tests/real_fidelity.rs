//! Structural fidelity of a clone on a real install (flow C).
//!
//! For every ranged and melee weapon of the game and its expansions the test clones the weapon through the
//! designer, plans the clone, takes the definition as the plan rendered it through the XML boundary,
//! parses it back and compares it with the source:
//!
//! 1. the OWN comparison: every own child element, attribute and list entry of the source definition (the
//!    node as its file writes it, before inheritance) must be present with the same value in the written
//!    clone, and the clone must hold nothing the source does not;
//! 2. the RESOLVED comparison: both definitions after inheritance (the clone keeps the parent of its
//!    source), read from a session that holds the game and the project with all clones;
//! 3. the PROJECTILE comparisons of both kinds, for a clone that has its own projectile.
//!
//! The explained differences are the new names (`defName`, `label`, the `Name` attribute) and, for a clone
//! with its own projectile, the projectile name in the shooting verb. Everything else is reported, grouped
//! by element path with counts. The test fails when an unexplained difference remains.
//!
//! The test is `#[ignore]`: it only reads the install and writes only into temporary folders.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! cargo test -p rimstudio-toolkit --test real_fidelity -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::tree::Node;
use rimstudio_core::version::GameVersion;
use rimstudio_design::reader::is_weapon_def;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{DesignerCloneRequest, DesignerExportPlanRequest, FileKindDto};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::{Ctx, clone_draft, export_plan};
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_top_level;

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(std::env::var(name).ok()?);
    path.is_dir().then_some(path)
}

fn open_input(game_dir: &Utf8PathBuf) -> OpenInput {
    let text = std::fs::read_to_string(game_dir.join("Version.txt")).unwrap();
    let game = GameVersion::parse(text.trim()).unwrap();
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        game_dir.join("Data"),
    ));
    let outcome = Scanner::scan(
        &set,
        &ScanOptions::default().with_game_version(game.clone()),
        &NoopProgress,
        &CancelToken::new(),
    );
    let options = DesignerReferenceOptions::new(game);
    let mut builder = ModIndex::builder();
    for (_, meta) in outcome.index.iter() {
        builder.push(meta.clone());
    }
    let reference = ReferenceSet::reference_for_designer(&builder.build(), game_dir, &options);
    OpenInput::new(reference).with_game_dir(game_dir.clone())
}

// ---------------------------------------------------------------------------------------------------
// Flattening and comparing
// ---------------------------------------------------------------------------------------------------

fn normal(text: &str) -> String {
    let t = text.trim();
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => format!("{v}"),
        _ if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") => {
            t.to_ascii_lowercase()
        }
        _ => t.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// `(path with list indexes, value)` entries of an element: attributes, leaves and list entries.
fn flatten(node: &Node, path: &str, out: &mut Vec<(String, String)>) {
    for (name, value) in &node.attrs {
        out.push((format!("{path}@{name}"), normal(value)));
    }
    let elements: Vec<&Node> = node.elements().collect();
    if elements.is_empty() {
        out.push((path.to_owned(), normal(&node.text_content())));
        return;
    }
    let mut li_index = 0usize;
    for child in elements {
        if child.tag == "li" {
            if child.elements().next().is_some() || !child.attrs.is_empty() {
                flatten(child, &format!("{path}/li[{li_index}]"), out);
                li_index += 1;
            } else {
                out.push((format!("{path}/li"), normal(&child.text_content())));
            }
        } else {
            flatten(child, &format!("{path}/{}", child.tag), out);
        }
    }
}

fn strip_indexes(path: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for ch in path.chars() {
        match ch {
            '[' => skipping = true,
            ']' => skipping = false,
            c if !skipping => out.push(c),
            _ => {}
        }
    }
    out
}

#[derive(Default)]
struct Tally {
    /// path -> (count, a sample value, a sample weapon)
    missing: BTreeMap<String, (usize, String, String)>,
    extra: BTreeMap<String, (usize, String, String)>,
}

impl Tally {
    fn total(&self) -> usize {
        self.missing.values().map(|v| v.0).sum::<usize>()
            + self.extra.values().map(|v| v.0).sum::<usize>()
    }
}

/// Differences that are explained: the new names, the projectile name of a clone with its own projectile,
/// and (extras only) a list the parent supplies that the clone restates in full so that it can be edited:
/// `restated` holds the path prefixes, and the clone's list then replaces the parent's.
fn explained(
    path: &str,
    explain_projectile_name: bool,
    extra_side: bool,
    restated: &[String],
) -> bool {
    path == "/defName"
        || path == "/label"
        || path == "@Name"
        || (explain_projectile_name
            && path.starts_with("/verbs/li[")
            && path.ends_with("/defaultProjectile"))
        || (extra_side && restated.iter().any(|p| path.starts_with(p.as_str())))
}

fn compare(
    source: &Node,
    clone: &Node,
    weapon: &str,
    explain_projectile: bool,
    restated: &[String],
    tally: &mut Tally,
) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    flatten(source, "", &mut a);
    flatten(clone, "", &mut b);
    let mut remaining = b;
    for entry in a {
        if explained(&entry.0, explain_projectile, false, restated) {
            continue;
        }
        if let Some(pos) = remaining.iter().position(|e| *e == entry) {
            remaining.swap_remove(pos);
        } else {
            let slot = tally.missing.entry(strip_indexes(&entry.0)).or_insert((
                0,
                entry.1.clone(),
                weapon.to_owned(),
            ));
            slot.0 += 1;
        }
    }
    for entry in remaining {
        if explained(&entry.0, explain_projectile, true, restated) {
            continue;
        }
        let slot = tally.extra.entry(strip_indexes(&entry.0)).or_insert((
            0,
            entry.1.clone(),
            weapon.to_owned(),
        ));
        slot.0 += 1;
    }
}

fn print_tally(title: &str, tally: &Tally) {
    println!("== {title}: {} unexplained differences", tally.total());
    for (path, (n, sample, weapon)) in &tally.missing {
        let shown: String = sample.chars().take(60).collect();
        println!("  missing {n:4}  {path}   e.g. {shown:?} ({weapon})");
    }
    for (path, (n, sample, weapon)) in &tally.extra {
        let shown: String = sample.chars().take(60).collect();
        println!("  extra   {n:4}  {path}   e.g. {shown:?} ({weapon})");
    }
}

fn raw_node(path: &camino::Utf8Path, def_name: &str) -> Option<Node> {
    let bytes = std::fs::read(path.as_std_path()).ok()?;
    let parsed = parse_top_level(&bytes, ParseMode::Tolerant, "Defs").ok()?;
    parsed
        .nodes
        .into_iter()
        .find(|n| n.tag == "ThingDef" && n.child_text("defName") == Some(def_name))
}

fn projectile_name(node: &Node) -> Option<String> {
    node.child("verbs")?
        .children_named("li")
        .find_map(|v| v.child_text("defaultProjectile").map(str::to_owned))
}

// ---------------------------------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------------------------------

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR)"]
fn every_vanilla_weapon_clones_with_every_own_field_of_its_source() {
    let Some(game_dir) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let session = Arc::new(WorkspaceSession::open_simple(open_input(&game_dir)).unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session.clone(),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    let root = base.join("ZZ_FidelityProject");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>ZZ Fidelity</name><packageId>zz.fidelity</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    )
    .unwrap();
    let project_id = ctx
        .env()
        .projects()
        .open(&root)
        .unwrap()
        .id
        .as_str()
        .to_owned();

    let snapshot = session.snapshot();
    let weapons: Vec<(String, camino::Utf8PathBuf, bool)> = snapshot
        .database("ThingDef")
        .unwrap()
        .iter()
        .filter(|r| is_weapon_def(&r.node))
        .filter_map(|r| {
            let file = session.file(r.origin?.file)?;
            Some((
                r.def_name.clone(),
                file.path.clone(),
                !r.patched_by.is_empty(),
            ))
        })
        .collect();
    println!("weapons found: {}", weapons.len());

    let mut own = Tally::default();
    let mut own_projectile = Tally::default();
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    let mut cloned: Vec<(String, String)> = Vec::new();
    let mut with_own_projectile = 0usize;
    let mut patched = 0usize;
    let mut facts: BTreeMap<&str, usize> = BTreeMap::new();
    for (source, path, was_patched) in &weapons {
        let target = format!("ZZ_{source}");
        let response = clone_draft(
            &ctx,
            DesignerCloneRequest {
                project_id: project_id.clone(),
                source: source.clone(),
                def_name: target.clone(),
                label: None,
                mod_prefix: None,
                own_projectile: None,
            },
        );
        let response = match response {
            Ok(r) => r,
            Err(e) => {
                *refused.entry(format!("clone: {e}")).or_default() += 1;
                continue;
            }
        };
        {
            let spec = &response.entry.draft.spec;
            let mut bump = |name: &'static str, on: bool| {
                if on {
                    *facts.entry(name).or_default() += 1;
                }
            };
            bump("clones", true);
            bump("a recipe", spec.recipe.is_some());
            bump("comps", !spec.comps.is_empty());
            bump("an interaction sound", spec.sound_interact.is_some());
            bump("raw extra fields", !spec.extra_fields.is_empty());
            bump(
                "raw verb extras",
                spec.ranged
                    .as_ref()
                    .is_some_and(|r| !r.verb_extra.is_empty()),
            );
            bump(
                "tool extras",
                spec.tools.iter().any(|t| !t.extra.is_empty()),
            );
            bump(
                "a parent tech level not written",
                spec.parent
                    .as_ref()
                    .is_some_and(|p| p.inherited_tech_level.is_some()),
            );
            bump("accepted missing fields", !spec.accepted_missing.is_empty());
            bump(
                "lists that replace the parent's",
                !spec.inherit_reset.is_empty(),
            );
            bump(
                "equipped stat offsets",
                !spec.equipped_stat_offsets.is_empty(),
            );
        }
        let plan = export_plan(
            &ctx,
            DesignerExportPlanRequest {
                project_id: project_id.clone(),
                draft: response.entry.draft.clone(),
                convert: None,
                accept_suggestions: None,
            },
        )
        .unwrap();
        let Some(file) = plan
            .files
            .iter()
            .find(|f| f.kind == FileKindDto::VanillaDefs)
        else {
            let codes: Vec<String> = plan
                .diagnostics
                .iter()
                .filter(|d| d.severity == rimstudio_ipc_types::diagnostic::SeverityDto::Error)
                .map(|d| format!("{}@{}", d.code, d.field.clone().unwrap_or_default()))
                .collect();
            for c in codes {
                *refused.entry(format!("plan: {c}")).or_default() += 1;
            }
            continue;
        };
        if std::env::var("RIMSTUDIO_PRINT").is_ok_and(|n| n == *source) {
            println!("---- {} ----\n{}", file.path, file.rendered);
            for d in &plan.diagnostics {
                println!("  plan diagnostic {} {}", d.code, d.message);
            }
        }
        let target_path = root.join(&file.path);
        std::fs::create_dir_all(target_path.parent().unwrap().as_std_path()).unwrap();
        std::fs::write(target_path.as_std_path(), file.rendered.as_bytes()).unwrap();
        let parsed =
            parse_top_level(file.rendered.as_bytes(), ParseMode::Tolerant, "Defs").unwrap();
        let written = parsed
            .nodes
            .iter()
            .find(|n| n.child_text("defName") == Some(target.as_str()))
            .expect("the plan holds the clone");
        let Some(source_own) = raw_node(path, source) else {
            *refused
                .entry("source file not found".to_owned())
                .or_default() += 1;
            continue;
        };
        if *was_patched {
            patched += 1;
        }
        let source_projectile = projectile_name(&source_own).or_else(|| {
            snapshot
                .get("ThingDef", source)
                .and_then(|r| projectile_name(&r.node))
        });
        let clone_projectile = projectile_name(written);
        let own_proj = match (&source_projectile, &clone_projectile) {
            (Some(s), Some(c)) => s != c,
            _ => false,
        };
        let restated: Vec<String> = ["verbs", "tools"]
            .iter()
            .filter(|t| source_own.child(t).is_none())
            .map(|t| format!("/{t}"))
            .collect();
        compare(&source_own, written, source, own_proj, &restated, &mut own);
        if own_proj {
            with_own_projectile += 1;
            let c = clone_projectile.unwrap();
            let s = source_projectile.unwrap();
            let clone_proj_node = parsed
                .nodes
                .iter()
                .find(|n| n.child_text("defName") == Some(c.as_str()));
            let source_proj_path = snapshot
                .get("ThingDef", &s)
                .and_then(|r| r.origin)
                .and_then(|o| session.file(o.file))
                .map(|f| f.path.clone());
            if let (Some(node), Some(p)) = (clone_proj_node, source_proj_path)
                && let Some(src) = raw_node(&p, &s)
            {
                compare(
                    &src,
                    node,
                    &format!("{source} projectile"),
                    false,
                    &[],
                    &mut own_projectile,
                );
            } else {
                *refused
                    .entry("projectile def not found for the comparison".to_owned())
                    .or_default() += 1;
            }
        }
        cloned.push((source.clone(), target));
    }

    println!("cloned and planned: {} of {}", cloned.len(), weapons.len());
    println!(
        "with an own projectile: {with_own_projectile}; sources changed by a patch: {patched}"
    );
    for (name, n) in &facts {
        println!("clones with {name}: {n}");
    }
    for (reason, n) in &refused {
        println!("refused {n:3}  {reason}");
    }
    print_tally("OWN fields of the weapon", &own);
    print_tally("OWN fields of the projectiles", &own_projectile);

    // the resolved comparison, from a session that holds the project
    let with_project =
        WorkspaceSession::open_simple(open_input(&game_dir).with_project(&root).unwrap()).unwrap();
    let resolved_snapshot = with_project.snapshot();
    let mut resolved = Tally::default();
    let mut resolved_projectile = Tally::default();
    for (source, target) in &cloned {
        let (Some(a), Some(b)) = (
            resolved_snapshot.get("ThingDef", source),
            resolved_snapshot.get("ThingDef", target),
        ) else {
            *refused
                .entry("resolved def missing".to_owned())
                .or_default() += 1;
            continue;
        };
        let (ps, pc) = (projectile_name(&a.node), projectile_name(&b.node));
        let own_proj = matches!((&ps, &pc), (Some(s), Some(c)) if s != c);
        compare(&a.node, &b.node, source, own_proj, &[], &mut resolved);
        if own_proj
            && let (Some(ps), Some(pc)) = (ps, pc)
            && let (Some(x), Some(y)) = (
                resolved_snapshot.get("ThingDef", &ps),
                resolved_snapshot.get("ThingDef", &pc),
            )
        {
            compare(
                &x.node,
                &y.node,
                &format!("{source} projectile"),
                false,
                &[],
                &mut resolved_projectile,
            );
        }
    }
    print_tally("RESOLVED weapon", &resolved);
    print_tally("RESOLVED projectile", &resolved_projectile);
    let unexplained =
        own.total() + own_projectile.total() + resolved.total() + resolved_projectile.total();
    assert_eq!(unexplained, 0, "unexplained differences remain");
}
