//! The mod basics commands of the CLI: `project about show|set|preview`, `project load-folders show|set`,
//! `project version add` and `library search`, against fictional mod folders in a temporary HOME.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::Env;

fn create(env: &Env, name: &str) -> String {
    let dir = env.path("projects").join(name);
    let p = dir.to_str().unwrap().to_owned();
    env.run(&[
        "project",
        "create",
        p.as_str(),
        "--name",
        name,
        "--package-id",
        "rs.basics",
        "--author",
        "RS Author",
    ])
    .expect(0);
    p
}

/// A 2 by 2 RGBA PNG.
const PNG_HEX: &str = "89504e470d0a1a0a0000000d494844520000000200000002080600000072b60d240000000b49444154789c63604007000012000177f1fa000000000049454e44ae426082";

fn png_bytes() -> Vec<u8> {
    (0..PNG_HEX.len() / 2)
        .map(|i| u8::from_str_radix(&PNG_HEX[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn about_text(p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(p).join("About/About.xml")).unwrap()
}

#[test]
fn show_prints_every_basic_and_the_findings() {
    let env = Env::new(false);
    let p = create(&env, "RS_Show");
    let json = env.run(&["--json", "project", "about", "show", &p]);
    assert!(matches!(json.code(), 0 | 3));
    let v = json.json();
    assert_eq!(v["name"]["value"], "RS_Show");
    assert_eq!(v["packageId"]["value"], "rs.basics");
    assert_eq!(v["author"]["value"], "RS Author");
    assert_eq!(v["editable"], true);
    assert!(v["fileHash"].as_str().unwrap().len() == 64);
    let text = env.run(&["project", "about", "show", &p]);
    assert!(matches!(text.code(), 0 | 3));
    let out = text.stdout();
    assert!(
        out.contains("package id") && out.contains("rs.basics"),
        "{out}"
    );
    assert!(out.contains("about.preview-missing"), "{out}");
    let raw = env.run(&["project", "about", "show", &p, "--raw"]);
    assert!(raw.stdout().contains("<packageId>rs.basics</packageId>"));
}

#[test]
fn set_prints_the_diff_and_writes_only_with_yes() {
    let env = Env::new(false);
    let p = create(&env, "RS_Set");
    let before = about_text(&p);
    let dry = env.run(&[
        "project",
        "about",
        "set",
        &p,
        "--name",
        "Renamed",
        "--url",
        "https://example.invalid/x",
    ]);
    assert!(matches!(dry.code(), 0 | 3), "{}", dry.stderr());
    assert!(
        dry.stdout().contains("+  <name>Renamed</name>"),
        "{}",
        dry.stdout()
    );
    assert!(dry.stdout().contains("add --yes"));
    assert_eq!(about_text(&p), before);
    let done = env.run(&[
        "project",
        "about",
        "set",
        &p,
        "--name",
        "Renamed",
        "--url",
        "https://example.invalid/x",
        "--add-load-after",
        "ludeon.rimworld",
        "--add-dependency",
        "rs.other",
        "--dependency-name",
        "Other",
        "--workshop-url",
        "https://steamcommunity.com/sharedfiles/filedetails/?id=1",
        "--yes",
    ]);
    assert!(matches!(done.code(), 0 | 3), "{}", done.stderr());
    let after = about_text(&p);
    assert!(
        after.contains("<name>Renamed</name>")
            && after.contains("<url>https://example.invalid/x</url>")
    );
    assert!(
        after.contains("<li>ludeon.rimworld</li>")
            && after.contains("<packageId>rs.other</packageId>")
    );
    let v = env.run(&["--json", "project", "about", "show", &p]).json();
    assert_eq!(v["modDependencies"][0]["displayName"], "Other");
    assert_eq!(v["loadAfter"]["items"][0], "ludeon.rimworld");
    // a no-op writes nothing, a bad request is a usage error
    let noop = env.run(&["project", "about", "set", &p, "--name", "Renamed", "--yes"]);
    assert!(noop.stdout().contains("no byte"), "{}", noop.stdout());
    env.run(&["project", "about", "set", &p]).expect(2);
    let bad = env.run(&[
        "project",
        "about",
        "set",
        &p,
        "--changes",
        "{not json",
        "--yes",
    ]);
    bad.expect(2);
    let invalid = env.run(&[
        "project",
        "about",
        "set",
        &p,
        "--changes",
        r#"[{"op":"dependency-move","packageId":"no.such","to":0}]"#,
        "--yes",
    ]);
    invalid.expect(1);
    assert_eq!(about_text(&p), after);
}

#[test]
fn the_preview_image_is_added_shown_and_removed() {
    let env = Env::new(false);
    let p = create(&env, "RS_Preview");
    let png = env.path("art.png");
    std::fs::write(&png, png_bytes()).unwrap();
    let added = env.run(&[
        "project",
        "about",
        "preview",
        &p,
        "--image",
        png.to_str().unwrap(),
    ]);
    // 2 by 2 pixels is not the size the Workshop page shows: copied with a warning (exit 3)
    assert_eq!(added.code(), 3, "{}", added.stderr());
    assert!(
        added.stdout().contains("about.preview-dimensions"),
        "{}",
        added.stdout()
    );
    assert!(std::path::Path::new(&p).join("About/Preview.png").is_file());
    let shown = env
        .run(&["--json", "project", "about", "preview", &p])
        .expect(0)
        .json();
    assert_eq!(shown["width"], 2);
    env.run(&["project", "about", "preview", &p, "--remove"])
        .expect(0);
    assert!(!std::path::Path::new(&p).join("About/Preview.png").exists());
    let none = env.run(&["project", "about", "preview", &p]).expect(0);
    assert!(none.stdout().contains("no preview image"));
}

#[test]
fn load_folders_and_versions() {
    let env = Env::new(false);
    let p = create(&env, "RS_Versions");
    let root = std::path::Path::new(&p);
    let none = env.run(&["project", "load-folders", "show", &p]).expect(0);
    assert!(none.stdout().contains("no LoadFolders.xml"));
    // listing first, then doing it
    let dry = env
        .run(&[
            "project",
            "version",
            "add",
            &p,
            "1.7",
            "--standard-folders",
            "--add-block",
        ])
        .expect(0);
    assert!(
        dry.stdout().contains("would create 1.7/Patches"),
        "{}",
        dry.stdout()
    );
    assert!(!root.join("1.7").exists());
    env.run(&[
        "project",
        "version",
        "add",
        &p,
        "1.7",
        "--standard-folders",
        "--add-block",
        "--yes",
    ])
    .expect(0);
    assert!(root.join("1.7/Defs/SoundDefs").is_dir());
    let lf = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(lf.contains("<v1.7>"), "{lf}");
    let shown = env
        .run(&["--json", "project", "load-folders", "show", &p])
        .json();
    assert_eq!(shown["blocks"][0]["key"], "1.7");
    // a gated entry added by position
    let set = env.run(&[
        "project",
        "load-folders",
        "set",
        &p,
        "--add-entry",
        "0=1.7/Compat/CE",
        "--gate",
        "ceteam.combatextended",
    ]);
    assert!(matches!(set.code(), 0 | 3));
    assert!(set.stdout().contains("add --yes"));
    assert_eq!(
        std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap(),
        lf
    );
    env.run(&[
        "project",
        "load-folders",
        "set",
        &p,
        "--add-entry",
        "0=1.7/Compat/CE",
        "--gate",
        "ceteam.combatextended",
        "--yes",
    ]);
    assert!(
        std::fs::read_to_string(root.join("LoadFolders.xml"))
            .unwrap()
            .contains("IfModActive=\"ceteam.combatextended\"")
    );
    env.run(&["project", "version", "add", &p, "bad"]).expect(1);
}

#[test]
fn library_search_scans_first_and_finds_mods_by_name() {
    let env = Env::new(true);
    env.select_install();
    let found = env
        .run(&["--json", "library", "search", "combat"])
        .expect(0)
        .json();
    assert_eq!(found["scanned"], true);
    let hits = found["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{found}");
    assert_eq!(hits[0]["name"], "Combat Extended");
    assert_eq!(
        hits[0]["packageId"].as_str().unwrap().to_lowercase(),
        "ceteam.combatextended"
    );
    let text = env.run(&["library", "search", "combat"]).expect(0);
    assert!(
        text.stdout().contains("Combat Extended"),
        "{}",
        text.stdout()
    );
    let none = env
        .run(&["--json", "library", "search", "zzzz-nothing"])
        .expect(0)
        .json();
    assert!(none["hits"].as_array().unwrap().is_empty());
    assert!(none["hint"].as_str().unwrap().contains("matches"));
}
