//! The checks that keep a web page from using the bridge without permission.
//!
//! The bridge listens on the loopback address only, but any page in the person's browser can send
//! requests to loopback. Four checks stop that: the peer must be loopback, the `Host` header must name
//! the bridge itself (against DNS rebinding), a present `Origin` header must be on the allow list
//! (against cross site requests), and every call except the health probe must carry the secret token.
//! A `POST` must use the content type `application/json`, which a simple cross site request cannot.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// The header that carries the token.
pub const TOKEN_HEADER: &str = "x-rimstudio-token";

/// Compares two byte strings without stopping at the first difference. The time depends on the longer
/// of the two lengths, not on where they differ.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let longest = a.len().max(b.len());
    let mut diff = a.len() ^ b.len();
    for i in 0..longest {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= usize::from(x ^ y);
    }
    std::hint::black_box(diff) == 0
}

/// A fresh token: 32 random bytes as 64 hex digits.
///
/// The bytes come from the operating system's random device when it can be read and otherwise from the
/// randomly keyed hasher of the standard library mixed with the clock.
#[must_use]
pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    let from_os = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if !from_os || bytes.iter().all(|b| *b == 0) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        for (i, chunk) in bytes.chunks_mut(8).enumerate() {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u128(nanos);
            hasher.write_usize(i);
            hasher.write_u32(std::process::id());
            let word = hasher.finish().to_le_bytes();
            chunk.copy_from_slice(word.get(..chunk.len()).unwrap_or(&[0; 8][..chunk.len()]));
        }
    }
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// True when the `Host` header names this bridge: `127.0.0.1:PORT` or `localhost:PORT`.
#[must_use]
pub fn host_allowed(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else { return false };
    let host = host.to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

/// True when the origin is on the list (compared without case and without a trailing slash).
#[must_use]
pub fn origin_allowed(origin: &str, allowed: &[String]) -> bool {
    let norm = |s: &str| s.trim_end_matches('/').to_ascii_lowercase();
    let origin = norm(origin);
    allowed.iter().any(|a| norm(a) == origin)
}

/// True when the content type is `application/json`, with or without parameters.
#[must_use]
pub fn is_json_content_type(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        v.split(';')
            .next()
            .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
    })
}

/// True when the peer address is a loopback address.
#[must_use]
pub fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings_compare_equal_and_others_do_not() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(!constant_time_eq(b"a\0", b"a"));
    }

    #[test]
    fn tokens_are_64_hex_digits_and_differ() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert_ne!(a, "0".repeat(64));
    }

    #[test]
    fn only_the_bridge_itself_is_an_allowed_host() {
        assert!(host_allowed(Some("127.0.0.1:7878"), 7878));
        assert!(host_allowed(Some("localhost:7878"), 7878));
        assert!(host_allowed(Some("LocalHost:7878"), 7878));
        assert!(!host_allowed(Some("127.0.0.1:7879"), 7878));
        assert!(!host_allowed(Some("127.0.0.1"), 7878));
        assert!(!host_allowed(Some("evil.example:7878"), 7878));
        assert!(!host_allowed(Some("localhost.evil.example:7878"), 7878));
        assert!(!host_allowed(None, 7878));
    }

    #[test]
    fn origins_match_the_list_exactly() {
        let list = vec!["http://localhost:5173".to_owned()];
        assert!(origin_allowed("http://localhost:5173", &list));
        assert!(origin_allowed("HTTP://LOCALHOST:5173/", &list));
        assert!(!origin_allowed("http://localhost:5174", &list));
        assert!(!origin_allowed("https://localhost:5173", &list));
        assert!(!origin_allowed("null", &list));
        assert!(!origin_allowed("http://localhost:5173.evil.example", &list));
        assert!(!origin_allowed("anything", &[]));
    }

    #[test]
    fn content_types() {
        assert!(is_json_content_type(Some("application/json")));
        assert!(is_json_content_type(Some(
            "Application/JSON; charset=utf-8"
        )));
        assert!(!is_json_content_type(Some("text/plain")));
        assert!(!is_json_content_type(Some(
            "application/x-www-form-urlencoded"
        )));
        assert!(!is_json_content_type(Some("application/jsonp")));
        assert!(!is_json_content_type(None));
    }

    #[test]
    fn loopback_peers() {
        assert!(is_loopback("127.0.0.1".parse().unwrap()));
        assert!(is_loopback("::1".parse().unwrap()));
        assert!(is_loopback("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_loopback("192.168.1.5".parse().unwrap()));
        assert!(!is_loopback("::ffff:10.0.0.1".parse().unwrap()));
    }
}
