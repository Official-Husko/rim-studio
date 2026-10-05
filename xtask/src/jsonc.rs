//! A tiny JSONC reader: strips line comments, block comments and trailing commas,
//! then hands the text to `serde_json`.

use anyhow::{Context, Result};
use serde_json::Value;

/// Removes `//` and `/* */` comments outside string literals. Newlines inside block
/// comments are kept so that error positions stay meaningful.
pub(crate) fn strip_comments(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_string = false;
    while let Some(&c) = chars.get(i) {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&n) = chars.get(i + 1) {
                    out.push(n);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match (c, chars.get(i + 1)) {
            ('"', _) => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            ('/', Some('/')) => {
                while let Some(&n) = chars.get(i) {
                    if n == '\n' {
                        break;
                    }
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while let Some(&n) = chars.get(i) {
                    if n == '*' && chars.get(i + 1) == Some(&'/') {
                        i += 2;
                        break;
                    }
                    if n == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Removes commas that directly precede a closing bracket or brace (outside strings).
pub(crate) fn strip_trailing_commas(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut in_string = false;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&n) = chars.get(i + 1) {
                    out.push(n);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let next = chars.iter().skip(i + 1).find(|n| !n.is_whitespace());
            if !matches!(next, Some(']') | Some('}')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Parses JSONC text into a JSON value.
pub(crate) fn parse(src: &str) -> Result<Value> {
    let clean = strip_trailing_commas(&strip_comments(src));
    serde_json::from_str(&clean).context("invalid JSONC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_comments_are_removed() {
        assert_eq!(
            strip_comments("{\"a\": 1} // note\n").trim_end(),
            "{\"a\": 1}"
        );
    }

    #[test]
    fn block_comments_are_removed_and_keep_newlines() {
        let out = strip_comments("[1, /* a\nb */ 2]");
        assert_eq!(out, "[1, \n 2]");
    }

    #[test]
    fn comment_markers_inside_strings_survive() {
        let v = parse(r#"{"url": "http://x/*y*/", "b": "\" // still"}"#).unwrap();
        assert_eq!(v["url"], "http://x/*y*/");
        assert_eq!(v["b"], "\" // still");
    }

    #[test]
    fn trailing_commas_are_accepted() {
        let v = parse("{\"a\": [1, 2,], // c\n \"b\": {\"c\": 3,},}").unwrap();
        assert_eq!(v["a"][1], 2);
        assert_eq!(v["b"]["c"], 3);
    }

    #[test]
    fn commas_inside_strings_are_kept() {
        let v = parse(r#"["a,]", "b"]"#).unwrap();
        assert_eq!(v[0], "a,]");
    }

    #[test]
    fn unterminated_block_comment_does_not_panic() {
        let _ = strip_comments("{ /* never closed");
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(parse("{ nope").is_err());
    }
}
