//! The interactive quiz driven through a pseudo terminal. The terminal comes from `script` of util-linux,
//! so the test runs on Linux only and returns early when the tool is missing.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::io::Write;
use std::process::Stdio;

use common::Env;
use serde_json::Value;

fn script_available() -> bool {
    std::process::Command::new("script")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn draft_id(env: &Env, project: &str) -> String {
    let out = env
        .run(&[
            "--json",
            "new",
            "ranged",
            "--name",
            "RS_PtyRifle",
            "--project",
            project,
        ])
        .expect(0)
        .json();
    out["id"].as_str().unwrap().to_owned()
}

/// Runs the quiz under a terminal, feeds the lines and returns (exit code, terminal text).
fn drive(env: &Env, project: &str, id: &str, lines: &str) -> (i32, String) {
    let exe = env!("CARGO_BIN_EXE_rimstudio-cli");
    let inner = format!("{exe} quiz {id} --project {project}");
    let base = env.command();
    let mut script = std::process::Command::new("script");
    script
        .env_clear()
        .envs(base.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .current_dir(base.get_current_dir().unwrap())
        .args(["-qec", &inner, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = script.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(lines.as_bytes()).unwrap();
    // The pipe stays open until the program has read every line and ended.
    let out = child.wait_with_output().unwrap();
    drop(stdin);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn the_terminal_quiz_handles_skip_back_bad_input_and_done() {
    if !script_available() {
        return;
    }
    let env = Env::new(false);
    env.select_install();
    let dir = env.path("projects/RS_Pty");
    let project = dir.to_str().unwrap().to_owned();
    env.run(&[
        "project",
        "create",
        &project,
        "--name",
        "RS_Pty",
        "--package-id",
        "rs.pty",
    ])
    .expect(0);
    let id = draft_id(&env, &project);
    // skip, take it back, take back again (nothing left), a typed value on a choice question, nonsense, done.
    let (code, text) = drive(&env, &project, &id, "s\nb\nb\n=3\nzzz\nd\n");
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("Question 1"), "{text}");
    assert!(text.contains("there is no answer to take back"), "{text}");
    assert!(text.contains("matches no choice"), "{text}");
    assert!(text.contains("saved draft"), "{text}");
    let shown = env
        .run(&["drafts", "show", &id, "--project", &project])
        .expect(0)
        .json();
    assert_eq!(shown["calibration"], Value::String("quiz".into()));
}
