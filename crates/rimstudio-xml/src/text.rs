//! Small text helpers shared by the reader, the renderer and the span editor.

use std::borrow::Cow;

/// True when the text is empty or holds only XML white space.
#[inline]
pub(crate) fn is_ws_only(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

/// One based line and column of a byte offset (column counts characters).
pub(crate) fn line_col(text: &str, offset: usize) -> (u32, u32) {
    let cut = offset.min(text.len());
    let end = (0..=cut)
        .rev()
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0);
    let head = &text[..end];
    let mut line: u32 = 1;
    let mut line_start = 0usize;
    for (i, b) in head.bytes().enumerate() {
        if b == b'\n' {
            line = line.saturating_add(1);
            line_start = i + 1;
        }
    }
    let col = head[line_start..].chars().count();
    (
        line,
        u32::try_from(col).unwrap_or(u32::MAX).saturating_add(1),
    )
}

/// A practical subset of the XML 1.0 `Name` production (the same rule the node tree validates with).
pub(crate) fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let first_ok = first.is_ascii_alphabetic()
        || first == '_'
        || first == ':'
        || (!first.is_ascii() && first.is_alphabetic());
    if !first_ok {
        return false;
    }
    chars.all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | ':' | '-' | '.')
            || (!c.is_ascii()
                && (c.is_alphanumeric()
                    || c == '\u{B7}'
                    || ('\u{300}'..='\u{36F}').contains(&c)
                    || ('\u{203F}'..='\u{2040}').contains(&c)))
    })
}

/// Characters that XML 1.0 cannot carry even as a character reference by the letter of the standard.
#[inline]
pub(crate) fn is_illegal_xml_char(c: char) -> bool {
    (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{FFFE}' || c == '\u{FFFF}'
}

/// Escapes character data: `&`, `<`, `>` and carriage return.
pub(crate) fn escape_text(s: &str) -> Cow<'_, str> {
    escape_with(s, false)
}

/// Escapes an attribute value written between double quotes.
pub(crate) fn escape_attr(s: &str) -> Cow<'_, str> {
    escape_with(s, true)
}

fn needs_escape(c: char, attr: bool) -> bool {
    matches!(c, '&' | '<' | '>' | '\r')
        || is_illegal_xml_char(c)
        || (attr && matches!(c, '"' | '\t' | '\n'))
}

fn escape_with(s: &str, attr: bool) -> Cow<'_, str> {
    if !s.chars().any(|c| needs_escape(c, attr)) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '"' if attr => out.push_str("&quot;"),
            '\t' if attr => out.push_str("&#9;"),
            '\n' if attr => out.push_str("&#10;"),
            '\0' => {}
            c if is_illegal_xml_char(c) => {
                out.push_str(&format!("&#x{:X};", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Resolves one of the five predefined entities by name.
pub(crate) fn predefined_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}

/// Replaces the predefined entities and character references of a raw text run. Unknown
/// references stay as written. Used by readers that must not fail on odd input.
pub(crate) fn unescape_lenient(raw: &str) -> Cow<'_, str> {
    if !raw.contains('&') {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        match after.find(';') {
            Some(end) if end <= 12 => {
                let name = &after[..end];
                match resolve_reference(name) {
                    Some(c) => out.push(c),
                    None => {
                        out.push('&');
                        out.push_str(name);
                        out.push(';');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Resolves the text between `&` and `;`: a predefined entity or a character reference.
pub(crate) fn resolve_reference(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        if code == 0 {
            return None;
        }
        return char::from_u32(code);
    }
    predefined_entity(name)
}

/// Normalises line endings of text content the XML way: `\r\n` and lone `\r` become `\n`.
pub(crate) fn normalize_eols(s: &str) -> Cow<'_, str> {
    if !s.contains('\r') {
        return Cow::Borrowed(s);
    }
    Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("a<b&c", "a&lt;b&amp;c")]
    #[case("x\r\ny", "x&#13;\ny")]
    #[case("plain", "plain")]
    fn text_escaping(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(escape_text(input), expected);
    }

    #[test]
    fn attribute_escaping_covers_quotes_and_whitespace_controls() {
        assert_eq!(escape_attr("a\"b\tc\nd"), "a&quot;b&#9;c&#10;d");
    }

    #[test]
    fn line_col_counts_characters_and_lines() {
        assert_eq!(line_col("ab\ncd", 4), (2, 2));
        assert_eq!(line_col("\u{e9}x", 2), (1, 2));
    }

    #[rstest]
    #[case("&amp;&lt;&#65;&#x42;", "&<AB")]
    #[case("&nbsp; & x", "&nbsp; & x")]
    #[case("&#0;", "&#0;")]
    fn lenient_unescape(#[case] raw: &str, #[case] expected: &str) {
        assert_eq!(unescape_lenient(raw), expected);
    }
}
