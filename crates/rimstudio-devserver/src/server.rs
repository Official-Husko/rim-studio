//! The server: accept loop, connection cap, one thread per connection, routing and shutdown.
//!
//! One request per connection (`Connection: close`). A connection thread reads the head under a total
//! deadline, runs the security checks of [`crate::security`] in a fixed order (peer, `Host`, `Origin`,
//! method, token, content type, length), reads the body only after those pass, and answers. The one
//! long lived answer is the event stream of `GET /dev/events`.

use std::io::Write as _;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rimstudio_app::AppContext;
use rimstudio_ipc_types::error::ApiError;
use serde_json::json;

use crate::events::EventHub;
use crate::http::{
    Head, Limits, ReadError, Reject, Response, finish, parse_head, read_body, read_head,
};
use crate::security::{
    TOKEN_HEADER, constant_time_eq, generate_token, host_allowed, is_json_content_type,
    is_loopback, origin_allowed,
};
use crate::{codes, fsapi, rpc};

/// The origins of the Vite development server.
pub const DEFAULT_ORIGINS: [&str; 2] = ["http://localhost:5173", "http://127.0.0.1:5173"];

/// The port used when none is given.
pub const DEFAULT_PORT: u16 = 7878;

/// How long the event stream waits between checks of the stop flag.
const TICK: Duration = Duration::from_millis(250);
/// How often an idle event stream sends a comment line so that a closed client is noticed.
const KEEPALIVE: Duration = Duration::from_secs(15);
/// How long shutdown waits for connections to end.
const DRAIN: Duration = Duration::from_secs(3);

/// How a server is started.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// The port; 0 picks a free one.
    pub port: u16,
    /// The token; `None` generates one.
    pub token: Option<String>,
    /// The origins a browser page may call from.
    pub allow_origins: Vec<String>,
    /// The protocol limits.
    pub limits: Limits,
    /// Where to write `{"port":N,"token":"..."}`; `None` writes nothing.
    pub token_file: Option<PathBuf>,
    /// The data folder shown by `/dev/info`.
    pub data_dir: String,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            token: None,
            allow_origins: DEFAULT_ORIGINS.iter().map(|s| (*s).to_owned()).collect(),
            limits: Limits::default(),
            token_file: None,
            data_dir: String::new(),
        }
    }
}

/// State shared by every connection.
#[derive(Debug)]
pub struct Shared {
    /// The application every call runs against.
    pub(crate) app: Arc<AppContext>,
    pub(crate) token: String,
    pub(crate) port: u16,
    pub(crate) allow_origins: Vec<String>,
    pub(crate) limits: Limits,
    /// The event streams.
    pub(crate) hub: EventHub,
    pub(crate) stopping: AtomicBool,
    pub(crate) active: AtomicUsize,
    pub(crate) data_dir: String,
    addr: SocketAddr,
}

struct ConnGuard(Arc<Shared>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Stops a running server from another thread.
#[derive(Debug, Clone)]
pub struct Stopper {
    shared: Arc<Shared>,
}

impl Stopper {
    /// Asks the server to stop: no new connections, event streams end.
    pub fn stop(&self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.shared.addr, Duration::from_secs(1));
    }
}

/// A running server.
#[derive(Debug)]
pub struct ServerHandle {
    shared: Arc<Shared>,
    addr: SocketAddr,
    accept: Option<JoinHandle<()>>,
    token_file: Option<PathBuf>,
}

/// Writes the token file `{"port":N,"token":"..."}` through a temporary file and a rename.
///
/// # Errors
/// The I/O error of the write.
pub fn write_token_file(path: &Path, port: u16, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let text = json!({"port": port, "token": token}).to_string();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn remove_token_file(path: &Path, token: &str) {
    let ours = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .is_some_and(|v| v.get("token").and_then(|t| t.as_str()) == Some(token));
    if ours {
        let _ = std::fs::remove_file(path);
    }
}

/// Binds the loopback port and starts serving.
///
/// # Errors
/// The I/O error of binding the port, writing the token file or starting the accept thread.
pub fn start(app: AppContext, options: ServerOptions) -> std::io::Result<ServerHandle> {
    let listener = TcpListener::bind(("127.0.0.1", options.port))?;
    let addr = listener.local_addr()?;
    let token = options.token.unwrap_or_else(generate_token);
    if let Some(path) = &options.token_file {
        write_token_file(path, addr.port(), &token)?;
    }
    let shared = Arc::new(Shared {
        app: Arc::new(app),
        token,
        port: addr.port(),
        allow_origins: options.allow_origins,
        limits: options.limits,
        hub: EventHub::new(),
        stopping: AtomicBool::new(false),
        active: AtomicUsize::new(0),
        data_dir: options.data_dir,
        addr,
    });
    let for_loop = Arc::clone(&shared);
    let accept = std::thread::Builder::new()
        .name("bridge-accept".to_owned())
        .spawn(move || accept_loop(&for_loop, &listener))?;
    Ok(ServerHandle {
        shared,
        addr,
        accept: Some(accept),
        token_file: options.token_file,
    })
}

impl ServerHandle {
    /// The address the server listens on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The token every call must carry.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.shared.token
    }

    /// The origins a page may call from.
    #[must_use]
    pub fn allow_origins(&self) -> &[String] {
        &self.shared.allow_origins
    }

    /// The number of connections being served.
    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.shared.active.load(Ordering::SeqCst)
    }

    /// A handle that can stop the server from another thread.
    #[must_use]
    pub fn stopper(&self) -> Stopper {
        Stopper {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Blocks until the server has been stopped, waits briefly for the connections to end and
    /// removes the token file it wrote.
    pub fn wait(mut self) {
        self.join();
    }

    /// Stops the server and waits for it.
    pub fn stop(mut self) {
        self.stopper().stop();
        self.join();
    }

    fn join(&mut self) {
        if let Some(handle) = self.accept.take() {
            let _ = handle.join();
        }
        let deadline = Instant::now() + DRAIN;
        while self.shared.active.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(path) = self.token_file.take() {
            remove_token_file(&path, &self.shared.token);
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if self.accept.is_some() {
            self.stopper().stop();
            self.join();
        }
    }
}

fn accept_loop(shared: &Arc<Shared>, listener: &TcpListener) {
    for incoming in listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                tracing::warn!(error = %e, "the bridge could not accept a connection");
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
        };
        let Ok(peer) = stream.peer_addr() else {
            continue;
        };
        let before = shared.active.fetch_add(1, Ordering::SeqCst);
        if before >= shared.limits.max_connections {
            shared.active.fetch_sub(1, Ordering::SeqCst);
            refuse_busy(stream);
            continue;
        }
        let guard = ConnGuard(Arc::clone(shared));
        let for_thread = Arc::clone(shared);
        let spawned = std::thread::Builder::new()
            .name("bridge-conn".to_owned())
            .spawn(move || {
                let _guard = guard;
                serve(&for_thread, stream, peer);
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "the bridge could not start a connection thread");
        }
    }
}

fn refuse_busy(mut stream: TcpStream) {
    let error = ApiError::new(codes::TOO_BUSY, "too many open connections")
        .detail("reason", "too-many-connections");
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = stream.write_all(&Response::error(503, &error).to_bytes());
    let _ = stream.shutdown(Shutdown::Both);
}

/// What the router decided.
enum Routed {
    Respond(Response),
    Events,
}

fn read_error_response(error: ReadError) -> Option<Response> {
    match error {
        ReadError::Closed | ReadError::Io(_) => None,
        ReadError::Timeout => Some(
            Reject::new(
                408,
                codes::TIMEOUT,
                "read-timeout",
                "the request was not complete in time",
            )
            .into_response(),
        ),
        ReadError::Reject(r) => Some(r.into_response()),
    }
}

fn with_cors(response: Response, origin: Option<&str>) -> Response {
    match origin {
        Some(o) => response
            .with_header("Access-Control-Allow-Origin", o)
            .with_header("Vary", "Origin"),
        None => response,
    }
}

fn serve(shared: &Arc<Shared>, mut stream: TcpStream, peer: SocketAddr) {
    if shared.stopping.load(Ordering::SeqCst) {
        return;
    }
    if !is_loopback(peer.ip()) {
        let r = Reject::new(
            403,
            codes::FORBIDDEN,
            "peer-not-loopback",
            "only loopback peers are served",
        );
        finish(&mut stream, &r.into_response());
        return;
    }
    let deadline = Instant::now() + shared.limits.read_timeout;
    let (head_bytes, leftover) = match read_head(&mut stream, deadline, shared.limits.max_head) {
        Ok(parts) => parts,
        Err(e) => {
            if let Some(response) = read_error_response(e) {
                finish(&mut stream, &response);
            }
            return;
        }
    };
    let head = match parse_head(&head_bytes) {
        Ok(h) => h,
        Err(r) => {
            finish(&mut stream, &r.into_response());
            return;
        }
    };
    let cors_origin = head
        .header("origin")
        .filter(|o| origin_allowed(o, &shared.allow_origins))
        .map(str::to_owned);
    tracing::debug!(method = %head.method, path = %head.path, "bridge request");
    match route(shared, &head, &mut stream, leftover, deadline) {
        Routed::Respond(response) => {
            finish(&mut stream, &with_cors(response, cors_origin.as_deref()));
        }
        Routed::Events => serve_events(shared, &mut stream, cors_origin.as_deref()),
    }
}

fn forbidden(reason: &'static str, message: &'static str) -> Routed {
    Routed::Respond(Reject::new(403, codes::FORBIDDEN, reason, message).into_response())
}

fn not_allowed(allow: &str) -> Routed {
    Routed::Respond(
        Reject::new(
            405,
            codes::INVALID_REQUEST,
            "method-not-allowed",
            "the method is not allowed for this path",
        )
        .into_response()
        .with_header("Allow", allow),
    )
}

fn preflight(head: &Head, origin_ok: bool) -> Response {
    let mut r = Response::empty(204)
        .with_header("Allow", "GET, POST, OPTIONS")
        .with_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .with_header(
            "Access-Control-Allow-Headers",
            format!("content-type, {TOKEN_HEADER}"),
        )
        .with_header("Access-Control-Max-Age", "600");
    if origin_ok
        && head
            .header("access-control-request-private-network")
            .is_some()
    {
        r = r.with_header("Access-Control-Allow-Private-Network", "true");
    }
    r
}

fn route(
    shared: &Shared,
    head: &Head,
    stream: &mut TcpStream,
    leftover: Vec<u8>,
    deadline: Instant,
) -> Routed {
    if !host_allowed(head.header("host"), shared.port) {
        return forbidden(
            "host-not-allowed",
            "the Host header does not name this bridge",
        );
    }
    let origin_ok = match head.header("origin") {
        None => false,
        Some(o) if origin_allowed(o, &shared.allow_origins) => true,
        Some(_) => return forbidden("origin-not-allowed", "the Origin is not allowed"),
    };
    if head.method == "OPTIONS" {
        return Routed::Respond(preflight(head, origin_ok));
    }
    if head.method != "GET" && head.method != "POST" {
        return not_allowed("GET, POST, OPTIONS");
    }
    if head.path == "/dev/health" {
        return if head.method == "GET" {
            Routed::Respond(Response::json(
                200,
                &json!({"ok": true, "bridgeVersion": crate::BRIDGE_VERSION}),
            ))
        } else {
            not_allowed("GET")
        };
    }
    let token_ok = head
        .header(TOKEN_HEADER)
        .is_some_and(|t| constant_time_eq(t.as_bytes(), shared.token.as_bytes()));
    if !token_ok {
        return Routed::Respond(
            Reject::new(
                401,
                codes::UNAUTHORIZED,
                "bad-token",
                "the x-rimstudio-token header is missing or wrong",
            )
            .into_response(),
        );
    }
    if let Some(name) = head.path.strip_prefix("/rpc/") {
        if head.method != "POST" {
            return not_allowed("POST");
        }
        return match read_post_body(shared, head, stream, leftover, deadline)
            .and_then(|body| rpc::call(shared, name, &body).map_err(ReadError::Reject))
        {
            Ok(response) => Routed::Respond(response),
            Err(e) => {
                Routed::Respond(read_error_response(e).unwrap_or_else(|| Response::empty(400)))
            }
        };
    }
    if head.method != "GET" {
        return not_allowed("GET");
    }
    match head.path.as_str() {
        "/dev/info" => Routed::Respond(Response::json(200, &rpc::info(shared))),
        "/dev/commands" => Routed::Respond(Response::json(200, &rpc::commands())),
        "/dev/events" => Routed::Events,
        "/dev/fs/home" => Routed::Respond(Response::json(200, &fsapi::home(&shared.app))),
        "/dev/fs/list" => Routed::Respond(list_response(head)),
        _ => Routed::Respond(
            Reject::new(
                404,
                codes::NOT_FOUND,
                "no-such-path",
                "there is no such path",
            )
            .into_response(),
        ),
    }
}

fn list_response(head: &Head) -> Response {
    let Some(path) = head.param("path") else {
        return Reject::bad("missing-path", "the path parameter is required").into_response();
    };
    let files = head.param("files").is_some_and(|v| v == "1" || v == "true");
    match fsapi::list(path, files) {
        Ok(listing) => Response::json(200, &listing.to_json()),
        Err(r) => r.into_response(),
    }
}

fn read_post_body(
    shared: &Shared,
    head: &Head,
    stream: &mut TcpStream,
    leftover: Vec<u8>,
    deadline: Instant,
) -> Result<Vec<u8>, ReadError> {
    if !is_json_content_type(head.header("content-type")) {
        return Err(Reject::new(
            415,
            codes::INVALID_REQUEST,
            "content-type",
            "a POST must use Content-Type: application/json",
        )
        .into());
    }
    if head.header("transfer-encoding").is_some() {
        return Err(Reject::new(
            411,
            codes::INVALID_REQUEST,
            "chunked",
            "chunked bodies are not supported; send Content-Length",
        )
        .into());
    }
    let length = match head.header("content-length") {
        None => {
            return Err(Reject::new(
                411,
                codes::INVALID_REQUEST,
                "length-required",
                "a POST needs Content-Length",
            )
            .into());
        }
        Some(v) => v
            .parse::<u64>()
            .ok()
            .filter(|_| v.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| Reject::bad("bad-length", "Content-Length is not a number"))?,
    };
    let max = u64::try_from(shared.limits.max_body).unwrap_or(u64::MAX);
    if length > max {
        return Err(Reject::new(
            413,
            codes::TOO_LARGE,
            "body-too-large",
            format!("the body is larger than {max} bytes"),
        )
        .into());
    }
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    read_body(stream, leftover, length, deadline)
}

fn serve_events(shared: &Shared, stream: &mut TcpStream, cors_origin: Option<&str>) {
    let rx = shared.hub.subscribe();
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n",
    );
    if let Some(origin) = cors_origin {
        head.push_str(&format!(
            "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n"
        ));
    }
    head.push_str("\r\nretry: 1000\n: connected\n\n");
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    if stream.write_all(head.as_bytes()).is_err() || stream.flush().is_err() {
        return;
    }
    let mut idle = Duration::ZERO;
    while !shared.stopping.load(Ordering::SeqCst) {
        let chunk = match rx.recv_timeout(TICK) {
            Ok(line) => {
                idle = Duration::ZERO;
                format!("data: {line}\n\n")
            }
            Err(RecvTimeoutError::Timeout) => {
                idle += TICK;
                if idle < KEEPALIVE {
                    continue;
                }
                idle = Duration::ZERO;
                ": keepalive\n\n".to_owned()
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if stream.write_all(chunk.as_bytes()).is_err() || stream.flush().is_err() {
            break;
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_file_is_written_and_removed_only_when_it_is_ours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/bridge.json");
        write_token_file(&path, 4242, "tok").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v, json!({"port": 4242, "token": "tok"}));
        remove_token_file(&path, "other");
        assert!(path.exists());
        remove_token_file(&path, "tok");
        assert!(!path.exists());
    }

    #[test]
    fn defaults_follow_the_api() {
        let o = ServerOptions::default();
        assert_eq!(o.port, 7878);
        assert_eq!(
            o.allow_origins,
            ["http://localhost:5173", "http://127.0.0.1:5173"]
        );
        assert_eq!(o.limits.max_head, 16 * 1024);
        assert_eq!(o.limits.max_body, 8 * 1024 * 1024);
        assert_eq!(o.limits.read_timeout, Duration::from_secs(30));
        assert_eq!(o.limits.max_connections, 32);
    }
}
