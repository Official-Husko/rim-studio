//! Real data: the layout fix plan, apply and undo over copies of the owner's own mods. Run with
//! `RIMSTUDIO_CUSTOM_DIR=<folder of the owner's mods> cargo test -p rimstudio-toolkit --test real_fix -- --ignored --nocapture`.
//! Every mod is copied into a temporary folder first (without `.git`, `Source` and `Raw Assets`, and only mods
//! below 40 MB); nothing is ever written into the originals.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common_fix;

use std::path::Path;

use common_fix::*;
use rimstudio_ipc_types::project_fix::LayoutFixRiskDto;
use rimstudio_toolkit::project::open_project;

const SKIPPED: [&str; 3] = [".git", "Source", "Raw Assets"];
const MAX_BYTES: u64 = 40 * 1024 * 1024;

fn copy_filtered(from: &Path, to: &Path, total: &mut u64) -> bool {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if SKIPPED.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            if !copy_filtered(&entry.path(), &to.join(&name), total) {
                return false;
            }
        } else if kind.is_file() {
            *total += entry.metadata().unwrap().len();
            if *total > MAX_BYTES {
                return false;
            }
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
    true
}

#[test]
#[ignore = "reads the owner's mod folder (RIMSTUDIO_CUSTOM_DIR); copies mods into a temporary folder"]
fn plan_apply_and_undo_over_the_owners_mods() {
    let Some(custom) = std::env::var_os("RIMSTUDIO_CUSTOM_DIR") else {
        println!("RIMSTUDIO_CUSTOM_DIR is not set; nothing to do");
        return;
    };
    let custom = Path::new(&custom);
    let w = world();
    let mut names: Vec<_> = std::fs::read_dir(custom)
        .unwrap()
        .flatten()
        .filter(|e| e.path().join("About/About.xml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let (mut mods, mut with_items, mut applied, mut moved) = (0, 0, 0, 0);
    for name in names {
        let copy = w
            .base
            .join("copies")
            .join(name.replace(['[', ']', ' ', '&'], "_"));
        let mut total = 0;
        if !copy_filtered(&custom.join(&name), copy.as_std_path(), &mut total) {
            println!("{name}: skipped (larger than 40 MB)");
            continue;
        }
        let Ok(opened) = open_project(&w.env, &copy) else {
            println!("{name}: cannot be opened");
            continue;
        };
        mods += 1;
        let id = opened.record.id.as_str().to_owned();
        let before = snapshot(&copy);
        let checked_before = issues_of(&w, &id).len();
        let p = plan_of(&w, &id);
        println!(
            "{name}: {} items ({} safe, {} for review, {} conflicts), {} findings",
            p.items.len(),
            p.safe,
            p.needs_review,
            p.conflicts,
            checked_before
        );
        for i in &p.items {
            println!(
                "    {:?} {} {} -> {} [{}]{}",
                i.kind,
                i.issue_code,
                i.from,
                i.to,
                if i.risk == LayoutFixRiskDto::Safe {
                    "safe"
                } else {
                    "review"
                },
                i.review_reason
                    .as_deref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            );
        }
        for i in p
            .items
            .iter()
            .filter(|i| i.diff.is_some() && i.to.ends_with("LoadFolders.xml"))
        {
            println!("{}", i.diff.as_deref().unwrap_or_default());
        }
        // the plan is read only
        assert_eq!(
            before,
            snapshot(&copy),
            "{name}: the plan changed the folder"
        );
        if p.items.is_empty() {
            continue;
        }
        with_items += 1;
        let report = apply_all(&w, &id);
        applied += 1;
        moved += report.done.len();
        println!(
            "    applied {} items, {} skipped, findings {} -> {}",
            report.done.len(),
            report.skipped.len(),
            checked_before,
            report.check.issues.len()
        );
        for s in &report.skipped {
            println!("    skipped {}: {}", s.id, s.reason);
        }
        assert!(
            report.check.issues.len() <= checked_before,
            "{name}: the apply added findings: {:?}",
            report.check.issues
        );
        let undone = undo_of(&w, &id, &report.apply_id).unwrap();
        assert_eq!(
            before,
            snapshot(&copy),
            "{name}: the undo did not restore every byte"
        );
        println!(
            "    undone: {} moved back, {} restored",
            undone.moved_back.len(),
            undone.restored.len()
        );
    }
    println!(
        "{mods} mods, {with_items} with fix items, {applied} applied and undone, {moved} items done"
    );
}
