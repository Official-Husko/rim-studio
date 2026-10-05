//! Pure redaction of paths, Steam ids and secrets for logs, error envelopes and support bundles.
//!
//! The [`Redactor`] works on text only: it is built from the home folder and user name the shell
//! knows, plus any secrets that are known to be in play, and rewrites a string so that it is safe to
//! write to a log or share. Output is deterministic: the same input gives the same output, and a
//! Steam id always maps to the same short tag so two log lines about one account stay correlated.

use serde::{Deserialize, Serialize};

/// The replacement for a secret value.
pub const SECRET_PLACEHOLDER: &str = "<secret>";
/// The replacement for a user name.
pub const USER_PLACEHOLDER: &str = "<user>";
/// The replacement for a home folder prefix.
pub const HOME_PLACEHOLDER: &str = "~";

/// The first digits of every 64 bit Steam id of an individual account.
const STEAM_ID_PREFIX: &str = "7656119";
/// The length of a 64 bit Steam id in decimal digits.
const STEAM_ID_DIGITS: usize = 17;
/// Alphanumeric runs at least this long that mix letters and digits are treated as tokens.
const TOKEN_MIN_LEN: usize = 40;
/// The shortest user name that is replaced when it appears on its own (shorter names collide with
/// ordinary words).
const MIN_USER_LEN: usize = 3;

const SENSITIVE_KEYS: [&str; 8] = [
    "token",
    "secret",
    "password",
    "passwd",
    "apikey",
    "api_key",
    "api-key",
    "authorization",
];

/// Rewrites text so it is safe to log or share.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redactor {
    home: Vec<String>,
    user: Option<String>,
    secrets: Vec<String>,
}

impl Redactor {
    /// A redactor that only applies the pattern based rules (Steam ids, `key=value` secrets, bearer
    /// tokens, token like runs and the `/home/<name>`, `/Users/<name>` and `C:\Users\<name>` shapes).
    pub fn new() -> Self {
        Redactor::default()
    }

    /// Sets the home folder; both separator styles are replaced by `~`.
    #[must_use]
    pub fn with_home(mut self, home: &str) -> Self {
        let trimmed = home.trim().trim_end_matches(['/', '\\']);
        if !trimmed.is_empty() {
            self.home.push(trimmed.to_owned());
            let other = if trimmed.contains('\\') {
                trimmed.replace('\\', "/")
            } else {
                trimmed.replace('/', "\\")
            };
            if other != trimmed {
                self.home.push(other);
            }
            // Longest first so a nested home wins over its parent.
            self.home.sort_by_key(|h| std::cmp::Reverse(h.len()));
        }
        self
    }

    /// Sets the OS user name.
    #[must_use]
    pub fn with_user(mut self, user: &str) -> Self {
        let t = user.trim();
        if !t.is_empty() {
            self.user = Some(t.to_owned());
        }
        self
    }

    /// Registers a secret that must never appear in output, whatever its shape.
    #[must_use]
    pub fn with_secret(mut self, secret: &str) -> Self {
        self.add_secret(secret);
        self
    }

    /// Registers a secret that must never appear in output. Empty text is ignored.
    pub fn add_secret(&mut self, secret: &str) {
        if !secret.is_empty() && !self.secrets.iter().any(|s| s == secret) {
            self.secrets.push(secret.to_owned());
            self.secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        }
    }

    /// Redacts a text. Rules run in this order: known secrets, home folder, user directory shapes,
    /// the user name, `key=value` secrets and bearer tokens, Steam ids, token like runs.
    pub fn redact(&self, input: &str) -> String {
        let mut text = input.to_owned();
        for secret in &self.secrets {
            text = text.replace(secret.as_str(), SECRET_PLACEHOLDER);
        }
        for home in &self.home {
            text = replace_prefix_at_boundary(&text, home, HOME_PLACEHOLDER);
        }
        text = redact_user_dirs(&text);
        if let Some(user) = self
            .user
            .as_ref()
            .filter(|u| u.chars().count() >= MIN_USER_LEN)
        {
            text = replace_whole_word(&text, user, USER_PLACEHOLDER);
        }
        text = redact_key_values(&text);
        text = redact_bearer(&text);
        text = map_runs(&text, |c| c.is_ascii_digit(), steam_id_tag);
        map_runs(
            &text,
            |c| c.is_ascii_alphanumeric() || c == '-' || c == '_',
            token_tag,
        )
    }

    /// The stable short tag for a 64 bit Steam id, `steam-<8 hex>`.
    pub fn steam_id_tag(id: u64) -> String {
        let hash = blake3::hash(id.to_string().as_bytes()).to_hex();
        format!("steam-{}", hash.as_str().get(..8).unwrap_or("00000000"))
    }
}

fn steam_id_tag(run: &str) -> Option<String> {
    if run.len() == STEAM_ID_DIGITS && run.starts_with(STEAM_ID_PREFIX) {
        run.parse::<u64>().ok().map(Redactor::steam_id_tag)
    } else {
        None
    }
}

fn token_tag(run: &str) -> Option<String> {
    let long = run.chars().count() >= TOKEN_MIN_LEN;
    let has_digit = run.bytes().any(|b| b.is_ascii_digit());
    let has_alpha = run.bytes().any(|b| b.is_ascii_alphabetic());
    (long && has_digit && has_alpha).then(|| SECRET_PLACEHOLDER.to_owned())
}

/// Splits `text` into maximal runs of characters accepted by `in_run` and other text, and lets `f`
/// replace a run.
fn map_runs(
    text: &str,
    in_run: impl Fn(char) -> bool,
    f: impl Fn(&str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run_start: Option<usize> = None;
    let flush = |out: &mut String, start: usize, end: usize| {
        let run = text.get(start..end).unwrap_or("");
        match f(run) {
            Some(r) => out.push_str(&r),
            None => out.push_str(run),
        }
    };
    for (i, c) in text.char_indices() {
        if in_run(c) {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else {
            if let Some(s) = run_start.take() {
                flush(&mut out, s, i);
            }
            out.push(c);
        }
    }
    if let Some(s) = run_start {
        flush(&mut out, s, text.len());
    }
    out
}

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Replaces every occurrence of `prefix` that is followed by a separator or the end of the text.
fn replace_prefix_at_boundary(text: &str, prefix: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find(prefix) {
        let (before, after_start) = rest.split_at(pos);
        let after = after_start.get(prefix.len()..).unwrap_or("");
        out.push_str(before);
        if after.chars().next().is_none_or(is_sep) {
            out.push_str(with);
        } else {
            out.push_str(prefix);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Replaces whole word occurrences (not touching letters, digits, `_` or `-` on either side).
fn replace_whole_word(text: &str, word: &str, with: &str) -> String {
    let word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut prev: Option<char> = None;
    while let Some(pos) = rest.find(word) {
        let (before, tail) = rest.split_at(pos);
        let after = tail.get(word.len()..).unwrap_or("");
        let left = before.chars().next_back().or(prev);
        let ok = !left.is_some_and(word_char) && !after.chars().next().is_some_and(word_char);
        out.push_str(before);
        out.push_str(if ok { with } else { word });
        prev = word.chars().next_back();
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Rewrites `/home/<name>`, `/Users/<name>` and `X:\Users\<name>` (either separator) to use
/// the user placeholder.
fn redact_user_dirs(text: &str) -> String {
    let mut out = text.to_owned();
    for marker in ["/home/", "/Users/", "\\Users\\", "/users/"] {
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(pos) = rest.find(marker) {
            let (before, tail) = rest.split_at(pos);
            let after = tail.get(marker.len()..).unwrap_or("");
            result.push_str(before);
            result.push_str(marker);
            let name_end = after
                .find(|c: char| is_sep(c) || c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(after.len());
            let name = after.get(..name_end).unwrap_or("");
            let keep =
                name.is_empty() || name == USER_PLACEHOLDER || name == "Public" || name == "Shared";
            result.push_str(if keep { name } else { USER_PLACEHOLDER });
            rest = after.get(name_end..).unwrap_or("");
        }
        result.push_str(rest);
        out = result;
    }
    out
}

fn redact_key_values(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while cursor < text.len() {
        let next = SENSITIVE_KEYS
            .iter()
            .filter_map(|k| {
                lower
                    .get(cursor..)
                    .and_then(|l| l.find(k))
                    .map(|p| (p + cursor, k.len()))
            })
            .min();
        let Some((pos, len)) = next else { break };
        let key_end = pos + len;
        let tail = text.get(key_end..).unwrap_or("");
        // Skip closing quote and spaces, then require ':' or '='.
        let skipped = tail.len() - tail.trim_start_matches(['"', '\'', ' ']).len();
        let after_key = tail.get(skipped..).unwrap_or("");
        let Some(sep) = after_key.chars().next().filter(|c| *c == ':' || *c == '=') else {
            out.push_str(text.get(cursor..key_end).unwrap_or(""));
            cursor = key_end;
            continue;
        };
        let value_area = after_key.get(sep.len_utf8()..).unwrap_or("");
        let ws = value_area.len() - value_area.trim_start_matches([' ', '"', '\'']).len();
        let value = value_area.get(ws..).unwrap_or("");
        let value_len = value
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | '&' | '}' | ')')
            })
            .unwrap_or(value.len());
        let value_start = key_end + skipped + sep.len_utf8() + ws;
        out.push_str(text.get(cursor..value_start).unwrap_or(""));
        if value_len > 0 {
            out.push_str(SECRET_PLACEHOLDER);
        }
        cursor = value_start + value_len;
    }
    out.push_str(text.get(cursor..).unwrap_or(""));
    out
}

fn redact_bearer(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(rel) = lower.get(cursor..).and_then(|l| l.find("bearer ")) {
        let value_start = cursor + rel + "bearer ".len();
        let value = text.get(value_start..).unwrap_or("");
        let len = value.find(char::is_whitespace).unwrap_or(value.len());
        out.push_str(text.get(cursor..value_start).unwrap_or(""));
        if len > 0 {
            out.push_str(SECRET_PLACEHOLDER);
        }
        cursor = value_start + len;
    }
    out.push_str(text.get(cursor..).unwrap_or(""));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn r() -> Redactor {
        Redactor::new()
            .with_home("/home/rsuser")
            .with_user("rsuser")
    }

    #[test]
    fn home_prefix_becomes_tilde() {
        assert_eq!(
            r().redact("open /home/rsuser/mods/a.xml"),
            "open ~/mods/a.xml"
        );
        assert_eq!(r().redact("at /home/rsuser"), "at ~");
    }

    #[test]
    fn home_prefix_needs_a_boundary() {
        assert_eq!(r().redact("/home/rsuser2/x"), "/home/<user>/x");
    }

    #[test]
    fn other_user_directories_are_rewritten_by_shape() {
        let plain = Redactor::new();
        assert_eq!(
            plain.redact("C:\\Users\\Alice\\AppData"),
            "C:\\Users\\<user>\\AppData"
        );
        assert_eq!(plain.redact("/Users/bob/Library"), "/Users/<user>/Library");
        assert_eq!(plain.redact("/home/carol"), "/home/<user>");
    }

    #[test]
    fn user_name_is_replaced_as_a_whole_word_only() {
        let red = Redactor::new().with_user("rsuser");
        assert_eq!(red.redact("hello rsuser, rsuserx"), "hello <user>, rsuserx");
    }

    #[test]
    fn steam_ids_become_stable_tags() {
        let out = Redactor::new().redact("user 76561198000000001 and 76561198000000001");
        let tag = Redactor::steam_id_tag(76_561_198_000_000_001);
        assert_eq!(out, format!("user {tag} and {tag}"));
        assert!(tag.starts_with("steam-") && tag.len() == 14);
        assert_eq!(
            Redactor::new().redact("build 4871 id 12345678901234567"),
            "build 4871 id 12345678901234567"
        );
    }

    #[test]
    fn key_value_secrets_are_hidden() {
        let red = Redactor::new();
        assert_eq!(
            red.redact("password=hunter2 next"),
            "password=<secret> next"
        );
        assert_eq!(
            red.redact(r#"{"apiKey": "abc", "x": 1}"#),
            r#"{"apiKey": "<secret>", "x": 1}"#
        );
        assert_eq!(red.redact("token: abc&other=1"), "token: <secret>&other=1");
        assert_eq!(
            red.redact("the token was refreshed"),
            "the token was refreshed"
        );
    }

    #[test]
    fn bearer_tokens_and_long_mixed_runs_are_hidden() {
        let red = Redactor::new();
        assert_eq!(
            red.redact("Authorization header Bearer abc.def"),
            "Authorization header Bearer <secret>"
        );
        let long = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3";
        assert_eq!(red.redact(&format!("key {long} end")), "key <secret> end");
        assert_eq!(
            red.redact("RS_VeryLongNameWithoutDigitsAtAllSoItStaysReadable"),
            "RS_VeryLongNameWithoutDigitsAtAllSoItStaysReadable"
        );
    }

    #[test]
    fn registered_secrets_are_removed_everywhere() {
        let red = Redactor::new().with_secret("s3cr3t");
        assert_eq!(red.redact("a s3cr3t b s3cr3t"), "a <secret> b <secret>");
    }

    #[test]
    fn windows_home_in_either_separator_style() {
        let red = Redactor::new().with_home("C:\\Users\\rsuser");
        assert_eq!(
            red.redact("C:\\Users\\rsuser\\x and C:/Users/rsuser/y"),
            "~\\x and ~/y"
        );
    }

    proptest! {
        #[test]
        fn never_panics_and_is_deterministic(s in "\\PC{0,200}") {
            let red = r().with_secret("zz");
            let a = red.redact(&s);
            prop_assert_eq!(a, red.redact(&s));
        }

        #[test]
        fn text_without_triggers_is_unchanged(s in "[a-z ]{0,60}") {
            prop_assume!(!SENSITIVE_KEYS.iter().any(|k| s.contains(k)) && !s.contains("bearer"));
            prop_assert_eq!(Redactor::new().redact(&s), s);
        }
    }
}
