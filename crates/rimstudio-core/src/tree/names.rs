//! Character and name rules shared by the owned tree and the arena.

/// Returns a reason when `name` is not a usable XML element or attribute name.
///
/// The rule is a practical subset of the XML 1.0 `Name` production: the first character is a
/// letter, `_` or `:` (or any non ASCII letter), later characters may also be digits, `-`, `.`,
/// the middle dot and combining marks. Whitespace and markup characters are never allowed.
pub(crate) fn check_name(name: &str) -> Result<(), &'static str> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("name is empty");
    };
    let first_ok = first.is_ascii_alphabetic()
        || first == '_'
        || first == ':'
        || (!first.is_ascii() && first.is_alphabetic());
    if !first_ok {
        return Err("name does not start with a letter, underscore or colon");
    }
    for c in chars {
        let ok = c.is_ascii_alphanumeric()
            || matches!(c, '_' | ':' | '-' | '.')
            || (!c.is_ascii()
                && (c.is_alphanumeric()
                    || c == '\u{B7}'
                    || ('\u{300}'..='\u{36F}').contains(&c)
                    || ('\u{203F}'..='\u{2040}').contains(&c)));
        if !ok {
            return Err("name contains a character that is not allowed in XML names");
        }
    }
    Ok(())
}

/// Returns the first character of `text` that XML 1.0 cannot represent, if any.
///
/// Allowed: tab, line feed, carriage return and every scalar value from U+0020 except the
/// non characters U+FFFE and U+FFFF.
pub(crate) fn first_invalid_char(text: &str) -> Option<char> {
    text.chars().find(|&c| {
        (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{FFFE}' || c == '\u{FFFF}'
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("li", true)]
    #[case("RS_Thing.Sub", true)]
    #[case("xml:space", true)]
    #[case("_x", true)]
    #[case("a-b1", true)]
    #[case("", false)]
    #[case("1a", false)]
    #[case("-a", false)]
    #[case("a b", false)]
    #[case("a<b", false)]
    #[case("a/b", false)]
    #[case("a=b", false)]
    #[case("a\"b", false)]
    fn name_rules(#[case] name: &str, #[case] ok: bool) {
        assert_eq!(check_name(name).is_ok(), ok, "{name}");
    }

    #[rstest]
    #[case("plain text\n\t\r", None)]
    #[case("a\u{0}b", Some('\u{0}'))]
    #[case("a\u{1F}b", Some('\u{1F}'))]
    #[case("a\u{FFFF}", Some('\u{FFFF}'))]
    #[case("\u{10FFFF}\u{E000}", None)]
    fn char_rules(#[case] text: &str, #[case] bad: Option<char>) {
        assert_eq!(first_invalid_char(text), bad);
    }
}
