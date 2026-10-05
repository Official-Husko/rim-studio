//! The navigation policy of the webview: first party pages only.
//!
//! The window loads the bundled frontend (`tauri://localhost`, or `http://tauri.localhost` on
//! Windows) and, in a debug build, the Vite dev server named in the configuration. Every other
//! address is refused, so a link or a script cannot take the window to a remote page (security and
//! privacy, section 4). Opening a link in the system browser goes through the opener permission.

use tauri::Url;

/// True when the window may navigate to `url`.
///
/// `dev_origin` is the dev server address of the configuration; it counts only when `debug` is true.
#[must_use]
pub fn navigation_allowed(url: &Url, dev_origin: Option<&Url>, debug: bool) -> bool {
    let host = url.host_str().unwrap_or_default();
    match url.scheme() {
        "tauri" => host == "localhost",
        "http" | "https" if host == "tauri.localhost" => true,
        // blank pages and in page data used by the webview itself
        "about" => url.as_str() == "about:blank",
        _ => debug && dev_origin.is_some_and(|dev| dev.origin() == url.origin()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn the_bundled_frontend_is_allowed() {
        assert!(navigation_allowed(
            &u("tauri://localhost/index.html"),
            None,
            false
        ));
        assert!(navigation_allowed(
            &u("http://tauri.localhost/"),
            None,
            false
        ));
        assert!(navigation_allowed(&u("about:blank"), None, false));
    }

    #[test]
    fn remote_pages_are_refused() {
        for url in [
            "https://example.com/",
            "http://localhost:5173/",
            "file:///etc/passwd",
            "tauri://evil/",
            "javascript:alert(1)",
            "https://tauri.localhost.evil.example/",
        ] {
            assert!(!navigation_allowed(&u(url), None, false), "{url}");
        }
    }

    #[test]
    fn the_dev_server_is_allowed_in_a_debug_build_only() {
        let dev = u("http://localhost:5173/");
        assert!(navigation_allowed(
            &u("http://localhost:5173/#/setup"),
            Some(&dev),
            true
        ));
        assert!(!navigation_allowed(
            &u("http://localhost:5173/"),
            Some(&dev),
            false
        ));
        assert!(!navigation_allowed(
            &u("http://localhost:5174/"),
            Some(&dev),
            true
        ));
        assert!(!navigation_allowed(
            &u("https://example.com/"),
            Some(&dev),
            true
        ));
    }
}
