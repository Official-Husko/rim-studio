//! A small HTTP/1.1 reader and writer for the development bridge.
//!
//! Only what the bridge needs: a request line, headers with hard limits, a `Content-Length` body and
//! one response per connection. There is no chunked upload, no keep alive, no pipelining and no
//! absolute-form target. Every refusal is a [`Reject`] that carries the HTTP status and the error code
//! of the envelope, so the router turns it into a response without further thought.
//!
//! The parser works on bytes and is pure ([`parse_head`]); the socket helpers ([`read_head`],
//! [`read_body`]) enforce one deadline for the whole request, so a client that sends one byte at a
//! time cannot hold a connection open past the read timeout.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use rimstudio_ipc_types::error::ApiError;
use serde_json::{Value, json};

use crate::codes;

/// The longest request line and header block the bridge reads.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;
/// The largest request body the bridge accepts.
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// The time a client has to deliver a complete request.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// The number of connections served at the same time.
pub const MAX_CONNECTIONS: usize = 32;
/// The longest request target.
pub const MAX_TARGET_BYTES: usize = 2048;
/// The largest number of header fields.
pub const MAX_HEADER_FIELDS: usize = 100;

/// The limits of one server, so tests can run with small ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Request line plus headers, in bytes.
    pub max_head: usize,
    /// Request body, in bytes.
    pub max_body: usize,
    /// Total time to read one request.
    pub read_timeout: Duration,
    /// Connections served at the same time.
    pub max_connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_head: MAX_HEAD_BYTES,
            max_body: MAX_BODY_BYTES,
            read_timeout: READ_TIMEOUT,
            max_connections: MAX_CONNECTIONS,
        }
    }
}

/// A refused request: the status line and the error envelope to answer with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reject {
    /// HTTP status.
    pub status: u16,
    /// Error code of the envelope.
    pub code: &'static str,
    /// Short machine readable reason (`details.reason`).
    pub reason: &'static str,
    /// Developer message.
    pub message: String,
}

impl Reject {
    /// A refusal with a status, a code, a reason slug and a message.
    #[must_use]
    pub fn new(
        status: u16,
        code: &'static str,
        reason: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            code,
            reason,
            message: message.into(),
        }
    }

    /// A 400 with the code `ipc.invalid-request`.
    #[must_use]
    pub fn bad(reason: &'static str, message: impl Into<String>) -> Self {
        Self::new(400, codes::INVALID_REQUEST, reason, message)
    }

    /// The response of this refusal.
    #[must_use]
    pub fn into_response(self) -> Response {
        let error = ApiError::new(self.code, self.message).detail("reason", self.reason);
        Response::error(self.status, &error)
    }
}

/// Why reading a request stopped.
#[derive(Debug)]
pub enum ReadError {
    /// The client closed the connection before sending a byte; nothing to answer.
    Closed,
    /// The request was not complete in time.
    Timeout,
    /// The socket failed; nothing to answer.
    Io(std::io::Error),
    /// The request is refused with this answer.
    Reject(Reject),
}

impl From<Reject> for ReadError {
    fn from(value: Reject) -> Self {
        Self::Reject(value)
    }
}

/// A parsed request head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The method, upper case as sent.
    pub method: String,
    /// The path of the target, without the query, not decoded.
    pub path: String,
    /// The decoded query parameters in order.
    pub query: Vec<(String, String)>,
    /// Header fields with lower case names, in order.
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// The value of a header (the name is lower case), `None` when absent.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The first query parameter of that name.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Headers that may appear once only; a repeated one is refused.
const SINGLETONS: [&str; 7] = [
    "host",
    "origin",
    "content-length",
    "content-type",
    "x-rimstudio-token",
    "transfer-encoding",
    "access-control-request-private-network",
];

fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn is_value_byte(b: u8) -> bool {
    b == b'\t' || (0x20..0x7f).contains(&b) || b >= 0x80
}

/// Decodes percent escapes and `+` as a space (query strings).
///
/// # Errors
/// A malformed escape or a result that is not UTF-8.
pub fn percent_decode(text: &str) -> Result<String, Reject> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'%' => {
                let hex = bytes
                    .get(i + 1..i + 3)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                    .ok_or_else(|| Reject::bad("bad-escape", "a percent escape is malformed"))?;
                out.push(hex);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| Reject::bad("bad-escape", "the query is not valid UTF-8"))
}

fn parse_query(raw: &str) -> Result<Vec<(String, String)>, Reject> {
    let mut out = Vec::new();
    for pair in raw.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.push((percent_decode(k)?, percent_decode(v)?));
    }
    Ok(out)
}

/// Parses the bytes of a request head (everything before the blank line).
///
/// # Errors
/// A [`Reject`] for a malformed request line, an unsupported version, a bad header field, a repeated
/// security header or a target that is too long.
pub fn parse_head(bytes: &[u8]) -> Result<Head, Reject> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Reject::bad("not-text", "the request head is not valid text"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Reject::bad(
            "bad-request-line",
            "the request line must be METHOD TARGET VERSION",
        ));
    };
    if method.is_empty() || !method.bytes().all(is_token_byte) {
        return Err(Reject::bad("bad-method", "the method is not a token"));
    }
    if !(version == "HTTP/1.1" || version == "HTTP/1.0") {
        return Err(Reject::new(
            505,
            codes::INVALID_REQUEST,
            "bad-version",
            "only HTTP/1.1 and HTTP/1.0 are supported",
        ));
    }
    if target.len() > MAX_TARGET_BYTES {
        return Err(Reject::new(
            414,
            codes::INVALID_REQUEST,
            "target-too-long",
            "the request target is too long",
        ));
    }
    if !target.starts_with('/') || target.starts_with("//") {
        return Err(Reject::bad(
            "bad-target",
            "the target must be an absolute path starting with one slash",
        ));
    }
    if !target.bytes().all(|b| (0x21..0x7f).contains(&b)) {
        return Err(Reject::bad(
            "bad-target",
            "the target contains characters that must be escaped",
        ));
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, parse_query(q)?),
        None => (target, Vec::new()),
    };
    let path = path.split_once('#').map_or(path, |(p, _)| p);

    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        if headers.len() >= MAX_HEADER_FIELDS {
            return Err(Reject::new(
                431,
                codes::INVALID_REQUEST,
                "too-many-headers",
                "the request has too many header fields",
            ));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(Reject::bad("bad-header", "a header field has no colon"));
        };
        if name.is_empty() || !name.bytes().all(is_token_byte) {
            return Err(Reject::bad("bad-header", "a header name is not a token"));
        }
        if !value.bytes().all(is_value_byte) {
            return Err(Reject::bad(
                "bad-header",
                "a header value has a control character",
            ));
        }
        let name = name.to_ascii_lowercase();
        if SINGLETONS.contains(&name.as_str()) && headers.iter().any(|(n, _)| *n == name) {
            return Err(Reject::bad(
                "duplicate-header",
                format!("the header {name} must appear once"),
            ));
        }
        headers.push((name, value.trim_matches([' ', '\t']).to_owned()));
    }
    Ok(Head {
        method: method.to_owned(),
        path: path.to_owned(),
        query,
        headers,
    })
}

fn remaining(deadline: Instant) -> Option<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() { None } else { Some(left) }
}

fn read_some(
    stream: &mut TcpStream,
    buf: &mut [u8],
    deadline: Instant,
) -> Result<usize, ReadError> {
    let left = remaining(deadline).ok_or(ReadError::Timeout)?;
    stream.set_read_timeout(Some(left)).map_err(ReadError::Io)?;
    loop {
        match stream.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(ReadError::Timeout);
            }
            Err(e) => return Err(ReadError::Io(e)),
        }
    }
}

fn find_blank_line(buf: &[u8], from: usize) -> Option<usize> {
    let start = from.saturating_sub(3);
    buf.get(start..)?
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + start)
}

/// Reads the request head. Returns the head bytes (without the blank line) and the body bytes that
/// arrived in the same reads.
///
/// # Errors
/// [`ReadError::Reject`] (431) when the head exceeds the limit, [`ReadError::Timeout`] when the
/// deadline passes, [`ReadError::Closed`] when the client sent nothing.
pub fn read_head(
    stream: &mut TcpStream,
    deadline: Instant,
    max_head: usize,
) -> Result<(Vec<u8>, Vec<u8>), ReadError> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 2048];
    let mut searched = 0_usize;
    loop {
        if let Some(pos) = find_blank_line(&buf, searched) {
            return split_head(buf, pos, max_head);
        }
        if has_bare_lf(&buf, searched) {
            return Err(Reject::bad("bare-lf", "lines must end with CR LF").into());
        }
        searched = buf.len();
        if buf.len() > max_head {
            return Err(too_large_head().into());
        }
        let n = read_some(stream, &mut chunk, deadline)?;
        if n == 0 {
            return Err(if buf.is_empty() {
                ReadError::Closed
            } else {
                Reject::bad("truncated", "the connection ended inside the request head").into()
            });
        }
        buf.extend_from_slice(chunk.get(..n).unwrap_or_default());
    }
}

/// True when a line feed that is not preceded by a carriage return appears at or after `from`.
fn has_bare_lf(buf: &[u8], from: usize) -> bool {
    let start = from.saturating_sub(1);
    buf.iter()
        .enumerate()
        .skip(start)
        .any(|(i, b)| *b == b'\n' && i.checked_sub(1).and_then(|p| buf.get(p)) != Some(&b'\r'))
}

fn too_large_head() -> Reject {
    Reject::new(
        431,
        codes::INVALID_REQUEST,
        "head-too-large",
        "the request line and headers are larger than the limit",
    )
}

fn split_head(
    mut buf: Vec<u8>,
    pos: usize,
    max_head: usize,
) -> Result<(Vec<u8>, Vec<u8>), ReadError> {
    if pos + 4 > max_head {
        return Err(too_large_head().into());
    }
    let rest = buf.split_off(pos + 4);
    buf.truncate(pos);
    Ok((buf, rest))
}

/// Reads exactly `length` body bytes, starting with what arrived with the head.
///
/// # Errors
/// [`ReadError::Timeout`] when the deadline passes, a 400 when the client closes early.
pub fn read_body(
    stream: &mut TcpStream,
    mut have: Vec<u8>,
    length: usize,
    deadline: Instant,
) -> Result<Vec<u8>, ReadError> {
    have.truncate(length);
    let mut chunk = [0_u8; 8192];
    while have.len() < length {
        let n = read_some(stream, &mut chunk, deadline)?;
        if n == 0 {
            return Err(Reject::bad("truncated", "the connection ended inside the body").into());
        }
        let want = (length - have.len()).min(n);
        have.extend_from_slice(chunk.get(..want).unwrap_or_default());
    }
    Ok(have)
}

/// The reason phrase of a status code.
#[must_use]
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Status",
    }
}

/// One response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// HTTP status.
    pub status: u16,
    /// Extra header fields (name, value), written after the standard ones.
    pub headers: Vec<(String, String)>,
    /// The body (JSON text), empty for 204.
    pub body: Vec<u8>,
}

impl Response {
    /// A JSON response.
    #[must_use]
    pub fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec()),
        }
    }

    /// An error envelope `{"ok":false,"error":{...}}`.
    #[must_use]
    pub fn error(status: u16, error: &ApiError) -> Self {
        Self::json(status, &json!({"ok": false, "error": error}))
    }

    /// An empty response.
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Adds one header field.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    /// The bytes of the response as they go on the wire.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n",
            self.status,
            reason_phrase(self.status),
            self.body.len()
        );
        if !self.body.is_empty() {
            head.push_str("Content-Type: application/json; charset=utf-8\r\n");
        }
        for (name, value) in &self.headers {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// Writes a response and closes the connection politely: the write side is shut down and what the
/// client still sends is read for a moment, so a refused oversized request does not turn into a
/// connection reset that hides the answer.
pub fn finish(stream: &mut TcpStream, response: &Response) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    if stream.write_all(&response.to_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut sink = [0_u8; 4096];
    let mut total = 0_usize;
    while total < 1024 * 1024 {
        match read_some(stream, &mut sink, deadline) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(text: &str) -> Result<Head, Reject> {
        parse_head(text.as_bytes())
    }

    #[test]
    fn a_plain_request_parses() {
        let h = head("GET /dev/info HTTP/1.1\r\nHost: 127.0.0.1:1\r\nX-RimStudio-Token:  abc ")
            .unwrap();
        assert_eq!(h.method, "GET");
        assert_eq!(h.path, "/dev/info");
        assert_eq!(h.header("host"), Some("127.0.0.1:1"));
        assert_eq!(h.header("x-rimstudio-token"), Some("abc"));
    }

    #[test]
    fn the_query_is_decoded() {
        let h = head("GET /dev/fs/list?path=%2Fhome%2Fa+b&files=1 HTTP/1.1\r\nHost: x").unwrap();
        assert_eq!(h.path, "/dev/fs/list");
        assert_eq!(h.param("path"), Some("/home/a b"));
        assert_eq!(h.param("files"), Some("1"));
        assert_eq!(h.param("nope"), None);
    }

    #[test]
    fn malformed_request_lines_are_refused() {
        for line in [
            "",
            "GET",
            "GET /",
            "GET / HTTP/1.1 extra",
            "GET  / HTTP/1.1",
            "G E T / HTTP/1.1",
            "GET / HTTP/2",
            "GET x HTTP/1.1",
            "GET http://example.com/ HTTP/1.1",
            "GET //evil HTTP/1.1",
            "GET /a b HTTP/1.1",
            "GET /?q=%zz HTTP/1.1",
        ] {
            let result = head(&format!("{line}\r\nHost: x"));
            assert!(result.is_err(), "{line:?} was accepted: {result:?}");
        }
    }

    #[test]
    fn an_unsupported_version_is_a_505() {
        let e = head("GET / HTTP/2\r\nHost: x").unwrap_err();
        assert_eq!(e.status, 505);
    }

    #[test]
    fn a_long_target_is_a_414() {
        let e = head(&format!("GET /{} HTTP/1.1\r\nHost: x", "a".repeat(3000))).unwrap_err();
        assert_eq!(e.status, 414);
    }

    #[test]
    fn bad_header_fields_are_refused() {
        assert!(head("GET / HTTP/1.1\r\nno colon here").is_err());
        assert!(head("GET / HTTP/1.1\r\n: empty").is_err());
        assert!(head("GET / HTTP/1.1\r\nbad name: v").is_err());
        assert!(head("GET / HTTP/1.1\r\nA: v\u{1}").is_err());
        assert!(head("GET / HTTP/1.1\r\nA: line\nbreak").is_err());
    }

    #[test]
    fn repeated_security_headers_are_refused_and_others_are_kept() {
        let e = head("GET / HTTP/1.1\r\nHost: a\r\nHost: b").unwrap_err();
        assert_eq!(e.reason, "duplicate-header");
        let h = head("GET / HTTP/1.1\r\nAccept: a\r\nAccept: b").unwrap();
        assert_eq!(h.headers.len(), 2);
    }

    #[test]
    fn too_many_header_fields_are_a_431() {
        let mut text = String::from("GET / HTTP/1.1");
        for i in 0..=MAX_HEADER_FIELDS {
            text.push_str(&format!("\r\nX-{i}: v"));
        }
        assert_eq!(head(&text).unwrap_err().status, 431);
    }

    #[test]
    fn a_fragment_is_dropped_from_the_path() {
        assert_eq!(head("GET /a#frag HTTP/1.1\r\nHost: x").unwrap().path, "/a");
    }

    #[test]
    fn percent_decode_handles_escapes_and_rejects_bad_ones() {
        assert_eq!(percent_decode("a%20b%2Fc").unwrap(), "a b/c");
        assert_eq!(percent_decode("%C3%A9").unwrap(), "\u{e9}");
        assert!(percent_decode("%C3").is_err());
        assert!(percent_decode("%4").is_err());
    }

    #[test]
    fn a_bare_line_feed_is_found() {
        assert!(has_bare_lf(b"GET / HTTP/1.1\nHost: x", 0));
        assert!(!has_bare_lf(b"GET / HTTP/1.1\r\nHost: x\r\n", 0));
        assert!(has_bare_lf(b"a\r\nb\n", 3));
        assert!(has_bare_lf(b"\n", 0));
    }

    #[test]
    fn blank_line_search_finds_a_split_terminator() {
        let buf = b"GET / HTTP/1.1\r\nA: b\r\n\r\nbody";
        assert_eq!(find_blank_line(buf, 0), Some(20));
        assert_eq!(find_blank_line(b"abc\r\n\r", 0), None);
        let full = b"abc\r\n\r\n";
        assert_eq!(find_blank_line(full, 5), Some(3));
    }

    #[test]
    fn a_response_has_length_close_and_json_type() {
        let bytes = Response::json(200, &json!({"a": 1})).to_bytes();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 7\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains("application/json"));
        assert!(text.ends_with("\r\n\r\n{\"a\":1}"));
    }

    #[test]
    fn an_empty_response_has_no_content_type() {
        let text = String::from_utf8(Response::empty(204).to_bytes()).unwrap();
        assert!(!text.contains("Content-Type:"));
        assert!(text.contains("Content-Length: 0"));
    }

    #[test]
    fn a_rejection_becomes_an_error_envelope() {
        let response = Reject::bad("x-reason", "nope").into_response();
        assert_eq!(response.status, 400);
        let v: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "ipc.invalid-request");
        assert_eq!(v["error"]["details"]["reason"], "x-reason");
        assert!(v["error"]["errorId"].as_str().unwrap().starts_with("e-"));
    }
}
