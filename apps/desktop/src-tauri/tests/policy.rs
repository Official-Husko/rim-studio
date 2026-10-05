//! Static checks of the security policy: the capability file, the content security policy and the
//! window configuration. They read the files the Tauri build reads, so a careless edit fails here.

#![allow(clippy::unwrap_used)]

use serde_json::Value;

fn read_json(name: &str) -> Value {
    let path = format!("{}/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn read_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// The identifiers of a capability, objects (scoped permissions) by their `identifier`.
fn permissions(capability: &Value) -> Vec<String> {
    capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| match p {
            Value::String(s) => s.clone(),
            other => other["identifier"].as_str().unwrap().to_owned(),
        })
        .collect()
}

#[test]
fn the_capability_grants_only_the_permissions_the_app_uses() {
    let capability = read_json("capabilities/main.json");
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    let mut granted = permissions(&capability);
    granted.sort();
    assert_eq!(
        granted,
        [
            "allow-rs-call",
            "allow-rs-cancel",
            "allow-rs-commands",
            "allow-rs-info",
            "core:default",
            "dialog:allow-open",
            "opener:allow-open-url",
            "opener:allow-reveal-item-in-dir",
        ]
    );
}

#[test]
fn no_file_system_shell_or_http_permission_is_granted() {
    for permission in permissions(&read_json("capabilities/main.json")) {
        for banned in [
            "fs:", "shell:", "http:", "store:", "sql:", "log:", "process:",
        ] {
            assert!(!permission.starts_with(banned), "{permission}");
        }
        assert!(!permission.contains("create-webview"), "{permission}");
    }
}

#[test]
fn opening_a_url_is_limited_to_https() {
    let capability = read_json("capabilities/main.json");
    let scoped = capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["identifier"] == "opener:allow-open-url")
        .unwrap();
    assert_eq!(scoped["allow"], serde_json::json!([{ "url": "https://*" }]));
}

#[test]
fn the_app_command_list_matches_the_handlers_and_the_permissions() {
    let build = read_text("build.rs");
    let app = read_text("src/app.rs");
    let handler = app
        .split("generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap();
    let mut registered: Vec<String> = handler.split(',').map(|s| s.trim().to_owned()).collect();
    registered.retain(|s| !s.is_empty());
    registered.sort();
    let manifest = build
        .split("&[")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap();
    let mut listed: Vec<String> = manifest
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    listed.sort();
    assert_eq!(registered, listed);
    let mut permissions = permissions(&read_json("capabilities/main.json"));
    permissions.retain(|p| p.starts_with("allow-rs-"));
    permissions.sort();
    let expected: Vec<String> = listed
        .iter()
        .map(|c| format!("allow-{}", c.replace('_', "-")))
        .collect();
    assert_eq!(permissions, expected);
}

#[test]
fn the_content_security_policy_loads_nothing_remote() {
    let config = read_json("tauri.conf.json");
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert!(csp.contains("script-src 'self';"));
    assert!(csp.contains("object-src 'none'") && csp.contains("frame-src 'none'"));
    assert!(csp.contains("connect-src ipc: http://ipc.localhost;"));
    for word in csp.split_whitespace() {
        assert!(
            !word.starts_with("http://")
                || word.starts_with("http://ipc.localhost")
                || word.starts_with("http://rsimg.localhost"),
            "remote source in the CSP: {word}"
        );
        assert!(!word.starts_with("https:") && word != "*");
    }
    assert!(!csp.contains("'unsafe-eval'"));
}

#[test]
fn the_development_policy_adds_only_the_vite_origin() {
    let config = read_json("tauri.conf.json");
    let dev = config["app"]["security"]["devCsp"].as_str().unwrap();
    for word in dev.split_whitespace() {
        let remote = word.starts_with("http://") || word.starts_with("ws://");
        if remote {
            assert!(
                word.starts_with("http://ipc.localhost")
                    || word.starts_with("http://rsimg.localhost")
                    || word.trim_end_matches(';') == "http://localhost:5173"
                    || word.trim_end_matches(';') == "ws://localhost:5173",
                "unexpected origin in the development CSP: {word}"
            );
        }
    }
    assert!(!dev.contains("'unsafe-eval'"));
}

#[test]
fn the_window_and_identity_follow_the_decision() {
    let config = read_json("tauri.conf.json");
    assert_eq!(config["identifier"], "app.rimstudio.desktop");
    assert_eq!(config["productName"], "RimStudio");
    assert_eq!(config["bundle"]["active"], false);
    let window = &config["app"]["windows"][0];
    assert_eq!(window["label"], "main");
    assert_eq!(window["title"], "RimStudio");
    assert_eq!(window["width"], 1440);
    assert_eq!(window["height"], 900);
    assert_eq!(window["minWidth"], 1024);
    assert_eq!(window["minHeight"], 700);
    assert!(window["backgroundColor"].is_string());
    assert_eq!(config["build"]["devUrl"], "http://localhost:5173");
    assert_eq!(config["build"]["frontendDist"], "../dist");
}

#[test]
fn the_icon_set_exists() {
    for file in [
        "32x32.png",
        "128x128.png",
        "128x128@2x.png",
        "icon.icns",
        "icon.ico",
        "icon.png",
    ] {
        assert!(
            std::path::Path::new(&format!("{}/icons/{file}", env!("CARGO_MANIFEST_DIR"))).is_file(),
            "{file}"
        );
    }
}
