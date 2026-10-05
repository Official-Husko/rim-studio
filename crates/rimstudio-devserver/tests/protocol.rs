//! Protocol tests over a real loopback socket against an application booted like the app tests do:
//! fake ports, a temporary data folder and no network.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rimstudio_app::context::Platform;
use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootInput, boot};
use rimstudio_devserver::http::Limits;
use rimstudio_devserver::{ServerHandle, ServerOptions, start};
use rimstudio_testing::fakes::{
    FakeClock, FakeCredentialStore, FakeEnv, FakeInstallSource, FakeLauncher, FakeLinkBackend,
    FakeProcessProbe, FakeRegistry, FakeSandbox,
};
use serde_json::{Value, json};

const TOKEN: &str = "test-token-0123456789";
const ORIGIN: &str = "http://localhost:5173";

fn platform(base: &Path) -> Platform {
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    Platform {
        clock: Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
        env: Arc::new(
            FakeEnv::new()
                .with_home(home.to_str().unwrap().to_owned())
                .with_exe_dir(base.join("exe").to_str().unwrap().to_owned()),
        ),
        registry: Arc::new(FakeRegistry::new()),
        process: Arc::new(FakeProcessProbe::new()),
        links: Arc::new(FakeLinkBackend::new()),
        launcher: Arc::new(FakeLauncher::new()),
        credentials: Arc::new(FakeCredentialStore::new()),
        sandbox: Arc::new(FakeSandbox::none()),
        install_source: Arc::new(FakeInstallSource::default()),
        ..Platform::system()
    }
}

struct Fixture {
    tmp: tempfile::TempDir,
    server: ServerHandle,
    addr: SocketAddr,
}

impl Fixture {
    fn new() -> Self {
        Self::with_limits(Limits {
            read_timeout: Duration::from_secs(5),
            ..Limits::default()
        })
    }

    fn with_limits(limits: Limits) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let data_dir = base.join("dev-data");
        let input = BootInput::new(platform(base))
            .with_data_base(data_dir.to_str().unwrap())
            .with_log(LogConfig::off());
        let app = boot(input).unwrap();
        let options = ServerOptions {
            port: 0,
            token: Some(TOKEN.to_owned()),
            limits,
            token_file: Some(base.join("cache/bridge.json")),
            data_dir: data_dir.to_str().unwrap().to_owned(),
            ..ServerOptions::default()
        };
        let server = start(app, options).unwrap();
        let addr = server.addr();
        Self { tmp, server, addr }
    }

    fn host(&self) -> String {
        format!("127.0.0.1:{}", self.addr.port())
    }
}

#[derive(Debug)]
struct Reply {
    status: u16,
    head: String,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {:?}", self.body))
    }

    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|l| {
            let (n, v) = l.split_once(':')?;
            n.eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
        })
    }

    fn code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    }
}

fn parse_reply(raw: &[u8]) -> Reply {
    let text = String::from_utf8_lossy(raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("no status line in {text:?}"));
    Reply {
        status,
        head: head.to_owned(),
        body: body.to_owned(),
    }
}

/// Sends raw bytes and reads the answer until the server closes the connection.
fn raw(addr: SocketAddr, bytes: &[u8]) -> Reply {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(bytes).unwrap();
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    parse_reply(&out)
}

struct Req<'a> {
    fixture: &'a Fixture,
    method: &'a str,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

impl<'a> Req<'a> {
    fn new(fixture: &'a Fixture, method: &'a str, path: &str) -> Self {
        Self {
            fixture,
            method,
            path: path.to_owned(),
            headers: vec![
                ("Host".into(), fixture.host()),
                ("x-rimstudio-token".into(), TOKEN.into()),
            ],
            body: None,
        }
    }

    fn get(fixture: &'a Fixture, path: &str) -> Self {
        Self::new(fixture, "GET", path)
    }

    fn post(fixture: &'a Fixture, path: &str, body: &Value) -> Self {
        let mut r = Self::new(fixture, "POST", path);
        r.headers
            .push(("Content-Type".into(), "application/json".into()));
        r.body = Some(body.to_string().into_bytes());
        r
    }

    fn without(mut self, name: &str) -> Self {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self
    }

    fn with(mut self, name: &str, value: &str) -> Self {
        self = self.without(name);
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut text = format!("{} {} HTTP/1.1\r\n", self.method, self.path);
        for (n, v) in &self.headers {
            text.push_str(&format!("{n}: {v}\r\n"));
        }
        if let Some(body) = &self.body
            && !self
                .headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        {
            text.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        text.push_str("\r\n");
        let mut bytes = text.into_bytes();
        if let Some(body) = &self.body {
            bytes.extend_from_slice(body);
        }
        bytes
    }

    fn send(&self) -> Reply {
        raw(self.fixture.addr, &self.bytes())
    }
}

fn rpc(f: &Fixture, command: &str, body: &Value) -> Value {
    let reply = Req::post(f, &format!("/rpc/{command}"), body).send();
    assert_eq!(reply.status, 200, "{reply:?}");
    reply.json()
}

fn make_tree(root: &Path) {
    for d in ["Beta/About", "alpha", ".hidden", "gamma/about"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("Beta/About/About.xml"), "x").unwrap();
    std::fs::write(root.join("gamma/about/about.xml"), "x").unwrap();
    std::fs::write(root.join("readme.txt"), "x").unwrap();
}

// ---------------------------------------------------------------------------------------------
// The five endpoint families
// ---------------------------------------------------------------------------------------------

#[test]
fn health_needs_no_token() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/health")
        .without("x-rimstudio-token")
        .send();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json()["ok"], true);
    assert_eq!(reply.header("connection").as_deref(), Some("close"));
}

#[test]
fn info_describes_the_bridge() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/info").send();
    assert_eq!(reply.status, 200);
    let v = reply.json();
    assert_eq!(v["bridgeVersion"], "0.1.0");
    assert_eq!(v["platform"], std::env::consts::OS);
    assert!(v["home"].as_str().unwrap().ends_with("/home"));
    assert!(v["commandCount"].as_u64().unwrap() >= 30);
    let data_dir = f.tmp.path().join("dev-data");
    assert_eq!(v["dataDir"], data_dir.to_str().unwrap());
    assert!(data_dir.join("config").is_dir(), "portable style roots");
    assert!(data_dir.join("data").is_dir());
    assert_eq!(v["roots"]["data"], data_dir.join("data").to_str().unwrap());
}

#[test]
fn the_command_list_has_names_kinds_and_types() {
    let f = Fixture::new();
    let v = Req::get(&f, "/dev/commands").send().json();
    let rows = v.as_array().unwrap();
    let find = |n: &str| {
        rows.iter()
            .find(|r| r["name"] == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    assert_eq!(find("app_ping")["kind"], "query");
    assert_eq!(find("sources_add_folder")["kind"], "action");
    assert_eq!(find("detect_run")["kind"], "job");
    assert_eq!(find("app_ping")["request"], "AppPingRequest");
    assert!(find("detect_run")["response"].is_string());
    let info = Req::get(&f, "/dev/info").send().json();
    assert_eq!(info["commandCount"].as_u64().unwrap(), rows.len() as u64);
}

#[test]
fn a_query_returns_its_response() {
    let f = Fixture::new();
    let v = rpc(&f, "app_ping", &json!({"echo": "hello"}));
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(v["data"]["echo"], "hello");
}

#[test]
fn an_empty_body_is_an_empty_request() {
    let f = Fixture::new();
    let mut req = Req::post(&f, "/rpc/app_ping", &Value::Null);
    req.body = Some(Vec::new());
    let v = req.send().json();
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(v["data"]["echo"], "");
}

#[test]
fn an_action_changes_state_that_a_query_then_reads() {
    let f = Fixture::new();
    let mods = f.tmp.path().join("my-mods");
    make_tree(&mods);
    let added = rpc(
        &f,
        "sources_add_folder",
        &json!({"path": mods.to_str().unwrap()}),
    );
    assert_eq!(added["ok"], true, "{added}");
    let list = rpc(&f, "sources_list", &json!({}));
    let sources = list["data"]["sources"].as_array().unwrap();
    assert!(
        sources
            .iter()
            .any(|s| s["kind"] == "custom" && s["path"] == mods.to_str().unwrap()),
        "{list}"
    );
}

#[test]
fn an_application_error_is_ok_false_with_http_200() {
    let f = Fixture::new();
    let v = rpc(
        &f,
        "sources_add_folder",
        &json!({"path": "/definitely/not/here"}),
    );
    assert_eq!(v["ok"], false);
    assert!(
        v["error"]["code"].as_str().unwrap().starts_with("sources."),
        "{v}"
    );
    assert!(v["error"]["errorId"].as_str().unwrap().starts_with("e-"));
    assert!(v["error"]["message"].is_string());
}

#[test]
fn a_job_returns_its_final_result() {
    let f = Fixture::new();
    let v = rpc(&f, "detect_run", &json!({"force": true}));
    assert_eq!(v["ok"], true, "{v}");
    assert!(v["data"]["schema"].is_number(), "{v}");
    assert!(v["data"]["installs"].is_array());
}

#[test]
fn a_job_with_a_bad_request_fails_before_it_starts() {
    let f = Fixture::new();
    let v = rpc(&f, "detect_run", &json!({"force": "yes"}));
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "ipc.invalid-request");
}

#[test]
fn an_unknown_command_is_an_error_envelope() {
    let f = Fixture::new();
    let v = rpc(&f, "no_such_command", &json!({}));
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "ipc.unknown-command");
}

#[test]
fn a_request_of_the_wrong_shape_names_the_problem() {
    let f = Fixture::new();
    let v = rpc(&f, "app_ping", &json!({"echo": 5}));
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "ipc.invalid-request");
}

#[test]
fn a_body_that_is_not_json_is_a_400() {
    let f = Fixture::new();
    let mut req = Req::post(&f, "/rpc/app_ping", &Value::Null);
    req.body = Some(b"{not json".to_vec());
    let reply = req.send();
    assert_eq!(reply.status, 400);
    assert_eq!(reply.code(), "ipc.invalid-request");
}

#[test]
fn a_badly_named_command_is_a_400() {
    let f = Fixture::new();
    for name in ["App_Ping", "a-b", "..%2f..", ""] {
        let reply = Req::post(&f, &format!("/rpc/{name}"), &json!({})).send();
        assert_eq!(reply.status, 400, "{name:?}: {reply:?}");
    }
}

fn read_until(stream: &mut TcpStream, needle: &str, within: Duration) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let deadline = Instant::now() + within;
    let mut seen = Vec::new();
    let mut chunk = [0_u8; 1024];
    while Instant::now() < deadline {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => seen.extend_from_slice(&chunk[..n]),
            Err(_) => {}
        }
        if String::from_utf8_lossy(&seen).contains(needle) {
            break;
        }
    }
    String::from_utf8_lossy(&seen).into_owned()
}

#[test]
fn job_events_arrive_on_the_event_stream() {
    let f = Fixture::new();
    let mut stream = TcpStream::connect(f.addr).unwrap();
    let req = Req::get(&f, "/dev/events").with("Origin", ORIGIN);
    stream.write_all(&req.bytes()).unwrap();
    let head = read_until(&mut stream, ": connected", Duration::from_secs(5));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("text/event-stream"));
    assert!(head.contains(&format!("Access-Control-Allow-Origin: {ORIGIN}")));

    let v = rpc(
        &f,
        "detect_run",
        &json!({"force": true, "jobId": "j-test-1"}),
    );
    assert_eq!(v["ok"], true, "{v}");
    let seen = read_until(&mut stream, "job-finished", Duration::from_secs(5));
    let line = seen
        .lines()
        .find(|l| l.starts_with("data: ") && l.contains("job-finished"))
        .unwrap_or_else(|| panic!("no job-finished in {seen:?}"));
    let event: Value = serde_json::from_str(line.trim_start_matches("data: ")).unwrap();
    assert_eq!(event["type"], "job-finished");
    assert_eq!(event["jobId"], "j-test-1");
    assert_eq!(event["command"], "detect_run");
    assert_eq!(event["ok"], true);
}

#[test]
fn the_event_stream_needs_the_token() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/events")
        .without("x-rimstudio-token")
        .send();
    assert_eq!(reply.status, 401);
}

// ---------------------------------------------------------------------------------------------
// Security
// ---------------------------------------------------------------------------------------------

#[test]
fn a_missing_or_wrong_token_is_a_401_even_for_unknown_paths() {
    let f = Fixture::new();
    for path in ["/dev/info", "/dev/commands", "/dev/fs/home", "/nope"] {
        let reply = Req::get(&f, path).without("x-rimstudio-token").send();
        assert_eq!(reply.status, 401, "{path}");
        assert_eq!(reply.code(), "bridge.unauthorized");
        let reply = Req::get(&f, path).with("x-rimstudio-token", "wrong").send();
        assert_eq!(reply.status, 401, "{path}");
        let reply = Req::get(&f, path)
            .with("x-rimstudio-token", &format!("{TOKEN}x"))
            .send();
        assert_eq!(reply.status, 401, "{path}");
    }
    let reply = Req::post(&f, "/rpc/app_ping", &json!({}))
        .with("x-rimstudio-token", "wrong")
        .send();
    assert_eq!(reply.status, 401);
}

#[test]
fn a_foreign_host_is_refused_on_every_path() {
    let f = Fixture::new();
    let port = f.addr.port();
    for host in [
        "evil.example".to_owned(),
        format!("evil.example:{port}"),
        format!("127.0.0.1:{}", port.wrapping_add(1)),
        "127.0.0.1".to_owned(),
        format!("localhost.evil.example:{port}"),
    ] {
        for path in ["/dev/health", "/dev/info"] {
            let reply = Req::get(&f, path).with("Host", &host).send();
            assert_eq!(reply.status, 403, "{host} {path}");
            assert_eq!(reply.code(), "bridge.forbidden");
        }
    }
    let reply = Req::get(&f, "/dev/health").without("Host").send();
    assert_eq!(reply.status, 403);
}

#[test]
fn localhost_is_an_allowed_host() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/info")
        .with("Host", &format!("localhost:{}", f.addr.port()))
        .send();
    assert_eq!(reply.status, 200);
}

#[test]
fn a_foreign_origin_is_refused_and_an_allowed_one_gets_cors_headers() {
    let f = Fixture::new();
    for origin in [
        "http://evil.example",
        "null",
        "http://localhost:5174",
        "https://localhost:5173",
    ] {
        let reply = Req::get(&f, "/dev/health").with("Origin", origin).send();
        assert_eq!(reply.status, 403, "{origin}");
        assert!(reply.header("access-control-allow-origin").is_none());
    }
    for origin in ["http://localhost:5173", "http://127.0.0.1:5173"] {
        let reply = Req::get(&f, "/dev/info").with("Origin", origin).send();
        assert_eq!(reply.status, 200, "{origin}");
        assert_eq!(
            reply.header("access-control-allow-origin").as_deref(),
            Some(origin)
        );
        assert_eq!(reply.header("vary").as_deref(), Some("Origin"));
    }
    let reply = Req::get(&f, "/dev/info").send();
    assert!(reply.header("access-control-allow-origin").is_none());
}

#[test]
fn a_refusal_for_a_known_origin_is_readable_by_the_page() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/info")
        .with("Origin", ORIGIN)
        .without("x-rimstudio-token")
        .send();
    assert_eq!(reply.status, 401);
    assert_eq!(
        reply.header("access-control-allow-origin").as_deref(),
        Some(ORIGIN)
    );
}

#[test]
fn the_preflight_answers_without_a_token_for_allowed_origins_only() {
    let f = Fixture::new();
    let reply = Req::new(&f, "OPTIONS", "/rpc/app_ping")
        .without("x-rimstudio-token")
        .with("Origin", ORIGIN)
        .with("Access-Control-Request-Method", "POST")
        .with(
            "Access-Control-Request-Headers",
            "content-type,x-rimstudio-token",
        )
        .send();
    assert_eq!(reply.status, 204);
    assert_eq!(
        reply.header("access-control-allow-origin").as_deref(),
        Some(ORIGIN)
    );
    let headers = reply.header("access-control-allow-headers").unwrap();
    assert!(headers.contains("x-rimstudio-token") && headers.contains("content-type"));
    assert!(
        reply
            .header("access-control-allow-methods")
            .unwrap()
            .contains("POST")
    );
    assert!(reply.body.is_empty());

    let reply = Req::new(&f, "OPTIONS", "/rpc/app_ping")
        .without("x-rimstudio-token")
        .with("Origin", "http://evil.example")
        .send();
    assert_eq!(reply.status, 403);
    let reply = Req::new(&f, "OPTIONS", "/rpc/app_ping")
        .without("x-rimstudio-token")
        .with("Origin", ORIGIN)
        .with("Host", "evil.example")
        .send();
    assert_eq!(reply.status, 403);
}

#[test]
fn a_custom_origin_list_replaces_the_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let input = BootInput::new(platform(tmp.path()))
        .with_data_base(tmp.path().join("d").to_str().unwrap())
        .with_log(LogConfig::off());
    let server = start(
        boot(input).unwrap(),
        ServerOptions {
            port: 0,
            token: Some(TOKEN.into()),
            allow_origins: vec!["http://localhost:3000".into()],
            ..ServerOptions::default()
        },
    )
    .unwrap();
    let host = format!("127.0.0.1:{}", server.port());
    let ask = |origin: &str| {
        let text = format!("GET /dev/health HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\n\r\n");
        raw(server.addr(), text.as_bytes()).status
    };
    assert_eq!(ask("http://localhost:3000"), 200);
    assert_eq!(ask(ORIGIN), 403);
}

#[test]
fn a_post_needs_the_json_content_type() {
    let f = Fixture::new();
    for ct in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data; boundary=x",
    ] {
        let reply = Req::post(&f, "/rpc/app_ping", &json!({}))
            .with("Content-Type", ct)
            .send();
        assert_eq!(reply.status, 415, "{ct}");
    }
    let reply = Req::post(&f, "/rpc/app_ping", &json!({}))
        .without("Content-Type")
        .send();
    assert_eq!(reply.status, 415);
    let reply = Req::post(&f, "/rpc/app_ping", &json!({"echo": "x"}))
        .with("Content-Type", "application/json; charset=utf-8")
        .send();
    assert_eq!(reply.status, 200);
}

#[test]
fn a_post_needs_a_plain_content_length() {
    let f = Fixture::new();
    let mut req = Req::post(&f, "/rpc/app_ping", &json!({}));
    req.headers.push(("Content-Length".into(), "abc".into()));
    assert_eq!(req.send().status, 400);

    let text = format!(
        "POST /rpc/app_ping HTTP/1.1\r\nHost: {}\r\nx-rimstudio-token: {TOKEN}\r\nContent-Type: application/json\r\n\r\n{{}}",
        f.host()
    );
    assert_eq!(raw(f.addr, text.as_bytes()).status, 411);

    let text = format!(
        "POST /rpc/app_ping HTTP/1.1\r\nHost: {}\r\nx-rimstudio-token: {TOKEN}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{{}}\r\n0\r\n\r\n",
        f.host()
    );
    assert_eq!(raw(f.addr, text.as_bytes()).status, 411);
}

#[test]
fn duplicate_security_headers_are_refused() {
    let f = Fixture::new();
    let text = format!(
        "GET /dev/info HTTP/1.1\r\nHost: {}\r\nHost: evil.example\r\nx-rimstudio-token: {TOKEN}\r\n\r\n",
        f.host()
    );
    assert_eq!(raw(f.addr, text.as_bytes()).status, 400);
    let text = format!(
        "GET /dev/info HTTP/1.1\r\nHost: {}\r\nx-rimstudio-token: wrong\r\nx-rimstudio-token: {TOKEN}\r\n\r\n",
        f.host()
    );
    assert_eq!(raw(f.addr, text.as_bytes()).status, 400);
}

#[test]
fn methods_and_paths_that_do_not_exist() {
    let f = Fixture::new();
    for method in ["DELETE", "PUT", "PATCH", "HEAD", "TRACE", "CONNECT"] {
        let reply = Req::new(&f, method, "/dev/info").send();
        assert_eq!(reply.status, 405, "{method}");
        assert!(reply.header("allow").is_some());
    }
    assert_eq!(Req::post(&f, "/dev/info", &json!({})).send().status, 405);
    assert_eq!(Req::get(&f, "/rpc/app_ping").send().status, 405);
    assert_eq!(Req::post(&f, "/dev/health", &json!({})).send().status, 405);
    let reply = Req::get(&f, "/nope").send();
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (404, "bridge.not-found")
    );
}

// ---------------------------------------------------------------------------------------------
// Limits and malformed input
// ---------------------------------------------------------------------------------------------

#[test]
fn an_oversized_header_block_is_a_431() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/info")
        .with("X-Big", &"a".repeat(20 * 1024))
        .send();
    assert_eq!(reply.status, 431, "{reply:?}");
    assert_eq!(reply.code(), "ipc.invalid-request");
    // A head with no end at all stops at the limit too.
    let mut text = b"GET /dev/info HTTP/1.1\r\n".to_vec();
    text.extend_from_slice(&vec![b'a'; 40 * 1024]);
    assert_eq!(raw(f.addr, &text).status, 431);
}

#[test]
fn a_head_just_under_the_limit_is_served() {
    let f = Fixture::new();
    let reply = Req::get(&f, "/dev/info")
        .with("X-Pad", &"a".repeat(15 * 1024))
        .send();
    assert_eq!(reply.status, 200, "{reply:?}");
}

#[test]
fn an_oversized_body_is_a_413_before_it_is_read() {
    let f = Fixture::new();
    let mut req = Req::post(&f, "/rpc/app_ping", &json!({}));
    req.body = Some(Vec::new());
    req.headers
        .push(("Content-Length".into(), (8 * 1024 * 1024 + 1).to_string()));
    let reply = req.send();
    assert_eq!(reply.status, 413, "{reply:?}");
    assert_eq!(reply.code(), "bridge.too-large");
}

#[test]
fn a_body_at_the_limit_is_read() {
    let f = Fixture::with_limits(Limits {
        max_body: 1024,
        read_timeout: Duration::from_secs(5),
        ..Limits::default()
    });
    let ok = Req::post(&f, "/rpc/app_ping", &json!({"echo": "a".repeat(900)})).send();
    assert_eq!(ok.status, 200, "{ok:?}");
    let big = Req::post(&f, "/rpc/app_ping", &json!({"echo": "a".repeat(1100)})).send();
    assert_eq!(big.status, 413);
}

#[test]
fn a_body_shorter_than_declared_times_out() {
    let f = Fixture::with_limits(Limits {
        read_timeout: Duration::from_millis(400),
        ..Limits::default()
    });
    let mut req = Req::post(&f, "/rpc/app_ping", &json!({}));
    req.body = Some(b"{}".to_vec());
    req.headers.push(("Content-Length".into(), "50".into()));
    let started = Instant::now();
    let reply = req.send();
    assert_eq!(reply.status, 408, "{reply:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_slow_client_is_timed_out() {
    let f = Fixture::with_limits(Limits {
        read_timeout: Duration::from_millis(400),
        ..Limits::default()
    });
    let mut stream = TcpStream::connect(f.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let started = Instant::now();
    stream.write_all(b"GET /dev/hea").unwrap();
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    let reply = parse_reply(&out);
    assert_eq!(reply.status, 408, "{reply:?}");
    assert_eq!(reply.code(), "bridge.timeout");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_client_that_trickles_bytes_cannot_outlast_the_deadline() {
    let f = Fixture::with_limits(Limits {
        read_timeout: Duration::from_millis(500),
        ..Limits::default()
    });
    let mut stream = TcpStream::connect(f.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let started = Instant::now();
    for byte in b"GET /dev/health HTTP/1.1\r\nHost: x" {
        if stream.write_all(&[*byte]).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(60));
        if started.elapsed() > Duration::from_secs(3) {
            break;
        }
    }
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    assert!(started.elapsed() < Duration::from_secs(4));
    assert_eq!(parse_reply(&out).status, 408);
}

#[test]
fn too_many_connections_are_refused_with_a_503() {
    let f = Fixture::with_limits(Limits {
        max_connections: 2,
        read_timeout: Duration::from_secs(5),
        ..Limits::default()
    });
    let _a = TcpStream::connect(f.addr).unwrap();
    let _b = TcpStream::connect(f.addr).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while f.server.active_connections() < 2 {
        assert!(Instant::now() < deadline, "connections were not counted");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut c = TcpStream::connect(f.addr).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut out = Vec::new();
    let _ = c.read_to_end(&mut out);
    let reply = parse_reply(&out);
    assert_eq!(reply.status, 503, "{reply:?}");
    assert_eq!(reply.code(), "bridge.too-busy");
    drop(_a);
    drop(_b);
    let deadline = Instant::now() + Duration::from_secs(5);
    while f.server.active_connections() > 0 {
        assert!(Instant::now() < deadline, "connections were not released");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(Req::get(&f, "/dev/info").send().status, 200);
}

#[test]
fn malformed_requests_get_a_4xx_envelope() {
    let f = Fixture::new();
    let host = f.host();
    let cases: Vec<(String, u16)> = vec![
        ("GARBAGE\r\n\r\n".to_owned(), 400),
        ("GET\r\n\r\n".to_owned(), 400),
        ("GET /dev/info\r\n\r\n".to_owned(), 400),
        (
            format!("GET /dev/info HTTP/1.1 junk\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET  /dev/info HTTP/1.1\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET dev/info HTTP/1.1\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET //dev/info HTTP/1.1\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET http://{host}/dev/info HTTP/1.1\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET /dev/info HTTP/2.0\r\nHost: {host}\r\n\r\n"),
            505,
        ),
        (
            format!("GET /dev/info HTTP/1.1\r\nHost {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET /dev/info HTTP/1.1\r\n: nothing\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (
            format!("GET /dev/info HTTP/1.1\r\nBad Name: x\r\nHost: {host}\r\n\r\n"),
            400,
        ),
        (format!("GET /dev/info HTTP/1.1\nHost: {host}\n\n"), 400),
        (
            format!("GET /{} HTTP/1.1\r\nHost: {host}\r\n\r\n", "a".repeat(3000)),
            414,
        ),
        (
            format!(
                "GET /dev/fs/list?path=%zz HTTP/1.1\r\nHost: {host}\r\nx-rimstudio-token: {TOKEN}\r\n\r\n"
            ),
            400,
        ),
        ("\r\nGET /dev/info HTTP/1.1\r\n\r\n".to_owned(), 400),
    ];
    for (text, status) in cases {
        let reply = raw(f.addr, text.as_bytes());
        assert_eq!(reply.status, status, "{text:?}: {reply:?}");
        let v = reply.json();
        assert_eq!(v["ok"], false);
        assert!(v["error"]["errorId"].is_string());
    }
}

#[test]
fn a_binary_head_is_refused_and_the_server_keeps_serving() {
    let f = Fixture::new();
    let mut junk = vec![0xff, 0xfe, 0x00, 0x01];
    junk.extend_from_slice(b"\r\n\r\n");
    assert_eq!(raw(f.addr, &junk).status, 400);
    assert_eq!(Req::get(&f, "/dev/info").send().status, 200);
}

#[test]
fn a_client_that_connects_and_leaves_does_no_harm() {
    let f = Fixture::new();
    drop(TcpStream::connect(f.addr).unwrap());
    let mut s = TcpStream::connect(f.addr).unwrap();
    s.write_all(b"GET /dev/inf").unwrap();
    drop(s);
    assert_eq!(Req::get(&f, "/dev/info").send().status, 200);
}

// ---------------------------------------------------------------------------------------------
// Folder listing
// ---------------------------------------------------------------------------------------------

fn encode(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn list(f: &Fixture, path: &str, files: bool) -> Reply {
    let suffix = if files { "&files=1" } else { "" };
    Req::get(f, &format!("/dev/fs/list?path={}{suffix}", encode(path))).send()
}

#[test]
fn a_fixture_tree_is_listed_directories_first_with_mod_flags() {
    let f = Fixture::new();
    let root = f.tmp.path().join("tree");
    make_tree(&root);
    let reply = list(&f, root.to_str().unwrap(), false);
    assert_eq!(reply.status, 200, "{reply:?}");
    let v = reply.json();
    assert_eq!(v["path"], root.to_str().unwrap());
    assert_eq!(v["parent"], f.tmp.path().to_str().unwrap());
    let entries = v["entries"].as_array().unwrap();
    let names: Vec<&str> = entries
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["alpha", "Beta", "gamma"],
        "dotfiles and files are hidden"
    );
    let by = |n: &str| entries.iter().find(|e| e["name"] == n).unwrap();
    assert_eq!(by("Beta")["isModFolder"], true);
    assert_eq!(by("Beta")["hasAbout"], true);
    assert_eq!(by("gamma")["isModFolder"], true, "case insensitive");
    assert_eq!(by("alpha")["isModFolder"], false);
    assert_eq!(by("alpha")["hasAbout"], false);
    assert_eq!(by("alpha")["kind"], "dir");

    let v = list(&f, root.to_str().unwrap(), true).json();
    let names: Vec<&str> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["alpha", "Beta", "gamma", "readme.txt"]);
    assert_eq!(v["entries"][3]["kind"], "file");
}

#[test]
fn relative_and_traversing_paths_are_refused() {
    let f = Fixture::new();
    for bad in ["relative/dir", ".", "..", "../etc", "/tmp/../etc", "~", ""] {
        let reply = list(&f, bad, false);
        assert_eq!(reply.status, 400, "{bad:?}: {reply:?}");
        assert_eq!(reply.code(), "ipc.invalid-request");
    }
    let reply = Req::get(&f, "/dev/fs/list").send();
    assert_eq!(reply.status, 400, "the path parameter is required");
}

#[test]
fn missing_folders_and_files_have_their_own_errors() {
    let f = Fixture::new();
    let missing = f.tmp.path().join("nope");
    let reply = list(&f, missing.to_str().unwrap(), false);
    assert_eq!((reply.status, reply.code().as_str()), (404, "io.not-found"));
    let root = f.tmp.path().join("tree");
    make_tree(&root);
    let file = root.join("readme.txt");
    let reply = list(&f, file.to_str().unwrap(), false);
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (400, "io.not-a-directory")
    );
}

#[test]
fn home_lists_the_home_folder_and_the_known_mod_folders() {
    let f = Fixture::new();
    let mods = f.tmp.path().join("my-mods");
    make_tree(&mods);
    rpc(
        &f,
        "sources_add_folder",
        &json!({"path": mods.to_str().unwrap(), "label": "Mine"}),
    );
    let v = Req::get(&f, "/dev/fs/home").send().json();
    let home = f.tmp.path().join("home");
    assert_eq!(v["home"], home.to_str().unwrap());
    let places = v["places"].as_array().unwrap();
    assert_eq!(places[0]["label"], "Home");
    assert_eq!(places[0]["path"], home.to_str().unwrap());
    assert!(
        places
            .iter()
            .any(|p| p["path"] == mods.to_str().unwrap() && p["label"] == "Mod folder: Mine"),
        "{v}"
    );
}

// ---------------------------------------------------------------------------------------------
// Start up and shutdown
// ---------------------------------------------------------------------------------------------

#[test]
fn the_token_file_names_the_port_and_the_token_and_goes_away_on_stop() {
    let f = Fixture::new();
    let path = f.tmp.path().join("cache/bridge.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v, json!({"port": f.addr.port(), "token": TOKEN}));
    assert_eq!(f.server.token(), TOKEN);
    let addr = f.addr;
    let tmp_path = path.clone();
    let Fixture { server, tmp, .. } = f;
    server.stop();
    assert!(!tmp_path.exists());
    assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err());
    drop(tmp);
}

#[test]
fn stopping_ends_open_event_streams() {
    let f = Fixture::new();
    let mut stream = TcpStream::connect(f.addr).unwrap();
    stream
        .write_all(&Req::get(&f, "/dev/events").bytes())
        .unwrap();
    let head = read_until(&mut stream, ": connected", Duration::from_secs(5));
    assert!(head.contains("200 OK"));
    f.server.stopper().stop();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut rest = Vec::new();
    let _ = stream.read_to_end(&mut rest);
    let Fixture { server, tmp, .. } = f;
    server.stop();
    drop(tmp);
}

#[test]
fn a_generated_token_is_used_when_none_is_given() {
    let tmp = tempfile::tempdir().unwrap();
    let input = BootInput::new(platform(tmp.path()))
        .with_data_base(tmp.path().join("d").to_str().unwrap())
        .with_log(LogConfig::off());
    let server = start(
        boot(input).unwrap(),
        ServerOptions {
            port: 0,
            ..ServerOptions::default()
        },
    )
    .unwrap();
    assert_eq!(server.token().len(), 64);
    let host = format!("127.0.0.1:{}", server.port());
    let text = format!(
        "GET /dev/info HTTP/1.1\r\nHost: {host}\r\nx-rimstudio-token: {}\r\n\r\n",
        server.token()
    );
    assert_eq!(raw(server.addr(), text.as_bytes()).status, 200);
}

#[test]
fn many_parallel_calls_all_succeed() {
    let f = Fixture::new();
    let addr = f.addr;
    let host = f.host();
    let handles: Vec<_> = (0..16)
        .map(|i| {
            let host = host.clone();
            std::thread::spawn(move || {
                let body = json!({"echo": format!("n{i}")}).to_string();
                let text = format!(
                    "POST /rpc/app_ping HTTP/1.1\r\nHost: {host}\r\nx-rimstudio-token: {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let reply = raw(addr, text.as_bytes());
                (i, reply.json())
            })
        })
        .collect();
    for h in handles {
        let (i, v) = h.join().unwrap();
        assert_eq!(v["data"]["echo"], format!("n{i}"));
    }
}
