//! The asset commands of the CLI: `designer asset DRAFT` (texture and shot sound imports) and
//! `designer asset info PATH`, against a fictional install in a temporary HOME. The image and the clip are
//! tiny files built here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::path::Path;

use common::{Env, files_under};
use serde_json::Value;

/// A 2 by 2 RGBA PNG.
const PNG_HEX: &str = "89504e470d0a1a0a0000000d494844520000000200000002080600000072b60d240000000b49444154789c63604007000012000177f1fa000000000049454e44ae426082";

fn png_bytes() -> Vec<u8> {
    (0..PNG_HEX.len() / 2)
        .map(|i| u8::from_str_radix(&PNG_HEX[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// A mono 16 bit WAV of silence.
fn wav_bytes(frames: u32) -> Vec<u8> {
    let data = frames * 2;
    let mut v = b"RIFF".to_vec();
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&8000u32.to_le_bytes());
    v.extend_from_slice(&16_000u32.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    v.extend(std::iter::repeat_n(0u8, data as usize));
    v
}

fn create_project(env: &Env, name: &str) -> String {
    let dir = env.path("projects").join(name);
    let p = dir.to_str().unwrap().to_owned();
    env.run(&[
        "project",
        "create",
        &p,
        "--name",
        name,
        "--package-id",
        "rs.assets",
    ])
    .expect(0);
    p
}

fn new_rifle(env: &Env, project: &str, name: &str) -> String {
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            name,
            "--project",
            project,
            "--strength",
            "typical",
            "--set",
            "ranged.projectile=RS_Shot00",
            "--set",
            "costList=RS_Steel:30",
            "--set",
            "tier=industrial",
            "--set",
            "role=RS_Rifle",
            "--set",
            "parent=RS_BaseGun",
            "--set",
            "workToMake=9000",
            "--set",
            "ranged.damage=12",
        ])
        .expect(0)
        .json();
    out["id"].as_str().unwrap().to_owned()
}

#[test]
fn asset_info_describes_a_png_and_a_wav_and_flags_a_missing_file() {
    let env = Env::new(false);
    let dir = env.path("art");
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("rifle.png");
    std::fs::write(&png, png_bytes()).unwrap();
    let wav = dir.join("shot.wav");
    std::fs::write(&wav, wav_bytes(800)).unwrap();

    let doc = env
        .run(&["--json", "designer", "asset", "info", png.to_str().unwrap()])
        .expect(0)
        .json();
    assert_eq!(doc["status"], "found", "{doc}");
    assert_eq!(doc["kind"], "png");
    assert_eq!(
        (doc["width"].as_u64(), doc["height"].as_u64()),
        (Some(2), Some(2))
    );
    assert_eq!(doc["sha256"].as_str().unwrap().len(), 64);

    let text = env
        .run(&["designer", "asset", "info", wav.to_str().unwrap()])
        .expect(0)
        .stdout();
    assert!(
        text.contains("wav") && text.contains("8000 Hz") && text.contains("100 ms"),
        "{text}"
    );

    // a missing file is reported and the run ends with the warning code
    let gone = dir.join("gone.png");
    env.run(&["designer", "asset", "info", gone.to_str().unwrap()])
        .expect(3);
}

#[test]
fn asset_stores_the_imports_in_the_draft_and_apply_copies_them() {
    let env = Env::new(false);
    env.select_install();
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    let dir = env.path("art");
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("rifle.png");
    std::fs::write(&png, png_bytes()).unwrap();
    let wav = dir.join("bang.wav");
    std::fs::write(&wav, wav_bytes(800)).unwrap();

    let doc = env
        .run(&[
            "--json",
            "designer",
            "asset",
            &id,
            "--project",
            &project,
            "--texture",
            png.to_str().unwrap(),
            "--shot-clip",
            wav.to_str().unwrap(),
            "--shot-volume",
            "30~34",
            "--shot-pitch",
            "0.95~1.05",
            "--shot-max",
            "1",
        ])
        .expect(0)
        .json();
    assert_eq!(doc["saved"], true, "{doc}");
    assert_eq!(doc["files"].as_array().unwrap().len(), 2, "{doc}");
    assert_eq!(doc["sounds"]["shot"]["volume"]["max"], 34.0);

    // the plan lists the copies; nothing is copied until apply --yes
    let plan = env
        .run(&["--json", "plan", &id, "--project", &project])
        .expect(0)
        .json();
    let kinds: Vec<&str> = plan["plan"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == "copy").count(), 2, "{plan}");
    assert!(
        !files_under(Path::new(&project))
            .iter()
            .any(|f| f.ends_with(".png") || f.ends_with(".wav")),
        "plan writes nothing"
    );

    env.run(&["apply", &id, "--project", &project, "--yes"])
        .expect(0);
    let files = files_under(Path::new(&project));
    for want in [
        "Textures/Things/Item/Equipment/WeaponRanged/RS_NewRifle.png",
        "Sounds/Weapons/RS_NewRifle_Shot/bang.wav",
        "Defs/SoundDefs/World_Oneshots_Weapons.xml",
    ] {
        assert!(files.contains(&want.to_owned()), "{want} in {files:?}");
    }
    assert_eq!(
        std::fs::read(
            Path::new(&project).join("Textures/Things/Item/Equipment/WeaponRanged/RS_NewRifle.png")
        )
        .unwrap(),
        png_bytes()
    );

    // clearing the sound takes it out of the next plan
    env.run(&[
        "designer",
        "asset",
        &id,
        "--project",
        &project,
        "--clear-shot-sound",
        "--clear-texture",
    ])
    .expect(0);
    let after: Value = env
        .run(&["--json", "plan", &id, "--project", &project])
        .expect(0)
        .json();
    assert!(
        after["plan"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["kind"] != "copy"),
        "{after}"
    );
}

#[test]
fn asset_needs_a_draft_and_a_project_and_checks_ranges() {
    let env = Env::new(false);
    env.select_install();
    env.run(&["designer", "asset"]).expect(2);
    let project = create_project(&env, "RS_Mine");
    let id = new_rifle(&env, &project, "RS_NewRifle");
    env.run(&["designer", "asset", &id, "--texture", "x.png"])
        .expect(2);
    env.run(&[
        "designer",
        "asset",
        &id,
        "--project",
        &project,
        "--shot-volume",
        "loud",
    ])
    .expect(2);
    // the help names the flags
    let help = env.run(&["designer", "asset", "--help"]).expect(0).stdout();
    for flag in ["--texture", "--projectile-texture", "--shot-clip", "info"] {
        assert!(help.contains(flag), "{flag} in {help}");
    }
}
