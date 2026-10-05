//! Real data: the layout tree and check over copies of the owner's own weapon mods. Run with
//! `RIMSTUDIO_CUSTOM_DIR=<folder of the owner's mods> cargo test -p rimstudio-toolkit --test real_layout -- --ignored --nocapture`.
//! The mods are copied into a temporary folder first (without `.git`, `Source` and `Raw Assets`); nothing is
//! ever written into the originals.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project::{LayoutProfileDto, ProjectTreeRequest, TreeNodeDto};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::project::{open_project, tree};
use rimstudio_toolkit::shared::env::ProjectEnv;

const SKIPPED: [&str; 3] = [".git", "Source", "Raw Assets"];

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if !SKIPPED.contains(&name_text.as_ref()) {
                copy_dir(&entry.path(), &to.join(&name));
            }
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}

fn print_tree(node: &TreeNodeDto, depth: usize, max: usize) {
    if depth > max {
        return;
    }
    let mark = if node.issues > 0 {
        format!("  ({} issues)", node.issues)
    } else {
        String::new()
    };
    println!(
        "{}{}  [{:?}] {} files{mark}",
        "  ".repeat(depth),
        node.name,
        node.role,
        node.files
    );
    for child in node
        .children
        .iter()
        .filter(|c| !c.children.is_empty() || depth < 2)
    {
        print_tree(child, depth + 1, max);
    }
}

#[test]
#[ignore = "reads the owner's mod folder (RIMSTUDIO_CUSTOM_DIR); copies two mods into a temporary folder"]
fn tree_and_check_of_the_owners_weapon_mods() {
    let Some(custom) = std::env::var_os("RIMSTUDIO_CUSTOM_DIR") else {
        println!("RIMSTUDIO_CUSTOM_DIR is not set; nothing to do");
        return;
    };
    let custom = Path::new(&custom);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    for (folder, expected) in [
        ("[OH] Gewehr 41", LayoutProfileDto::CoreStyle),
        ("[OH] The Lone Wolf Weapon Package", LayoutProfileDto::Flat),
    ] {
        let source = custom.join(folder);
        if !source.is_dir() {
            println!("{folder} is not in {}; skipped", custom.display());
            continue;
        }
        let copy = base
            .join("copies")
            .join(folder.replace(['[', ']', ' '], "_"));
        copy_dir(&source, copy.as_std_path());
        let opened = open_project(&env, &copy).unwrap();
        let t = tree(
            &env,
            &ProjectTreeRequest {
                project_id: opened.record.id.as_str().to_owned(),
                max_nodes: None,
            },
        )
        .unwrap();
        println!("\n=== {folder}");
        println!(
            "profile {:?}, weapons folder {}, ce folder {} (exists {}, legacy {}, gated {})",
            t.profile,
            t.weapons_folder,
            t.ce_folder,
            t.ce_folder_exists,
            t.ce_folder_legacy,
            t.ce_gated
        );
        println!(
            "{} files, {} bytes, {} def files, {} weapons, {} projectiles, {} patch files, {} textures, {} sounds",
            t.counts.files,
            t.counts.bytes,
            t.counts.def_files,
            t.counts.weapon_defs,
            t.counts.projectile_defs,
            t.counts.patch_files,
            t.counts.textures,
            t.counts.sounds
        );
        print_tree(&t.root, 0, 3);
        for issue in &t.issues {
            println!(
                "{:?} {} {}\n    {}\n    fix ({}): {}",
                issue.severity,
                issue.code,
                issue.path,
                issue.message,
                if issue.fix.automatic {
                    "automatic"
                } else {
                    "manual"
                },
                issue.fix.summary
            );
        }
        assert_eq!(t.profile, expected, "{folder}");
        assert!(t.counts.weapon_defs > 0, "{folder} has weapons");
    }
}
