//! Static checks of the xpath of a patch operation (rule CEP022).
//!
//! The checks are structural and need no document: the root must be `Defs`, brackets, parentheses and quotes
//! must balance, and a predicate must not be a bare quoted literal (`[ "x" ]` selects every node, which is
//! never what the author meant). A leading slash is accepted, and a descendant search (`//`) is exempt from
//! the root rule.

/// Why an xpath is malformed, or `None` when it passes the static checks.
#[must_use]
pub fn malformed_reason(xpath: &str) -> Option<String> {
    let x = xpath.trim();
    if x.is_empty() {
        return Some("the xpath is empty".to_owned());
    }
    if let Some(reason) = unbalanced(x) {
        return Some(reason);
    }
    if let Some(reason) = bare_literal(x) {
        return Some(reason);
    }
    if x.starts_with("//") {
        return None;
    }
    let rest = x.strip_prefix('/').unwrap_or(x);
    let first = rest.split(['/', '[']).next().unwrap_or_default().trim();
    (first != "Defs").then(|| format!("the root step is {first:?}, not Defs"))
}

/// Checks that quotes, square brackets and parentheses are closed in the right order.
fn unbalanced(x: &str) -> Option<String> {
    let mut stack: Vec<char> = Vec::new();
    let mut quote: Option<char> = None;
    for c in x.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '[' | '(' => stack.push(c),
            ']' => {
                if stack.pop() != Some('[') {
                    return Some("a closing bracket has no opening bracket".to_owned());
                }
            }
            ')' if stack.pop() != Some('(') => {
                return Some("a closing parenthesis has no opening one".to_owned());
            }
            _ => {}
        }
    }
    if quote.is_some() {
        return Some("a quoted text is not closed".to_owned());
    }
    (!stack.is_empty()).then(|| "a bracket or parenthesis is not closed".to_owned())
}

/// Finds a predicate that is only a quoted literal.
fn bare_literal(x: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for (i, c) in x.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '[' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let inner = x.get(start..i).unwrap_or_default().trim();
                    if is_single_literal(inner) {
                        return Some(format!("the predicate [{inner}] is a bare literal"));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_single_literal(text: &str) -> bool {
    let mut chars = text.chars();
    let (Some(first), Some(last)) = (chars.next(), text.chars().last()) else {
        return false;
    };
    (first == '"' || first == '\'')
        && text.len() >= 2
        && first == last
        && !text
            .get(1..text.len().saturating_sub(1))
            .unwrap_or_default()
            .contains(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Defs/ThingDef[defName=\"RS_X\"]/statBases", false)]
    #[case("/Defs/ThingDef[defName=\"RS_X\"]", false)]
    #[case("//ThingDef[defName=\"RS_X\"]", false)]
    #[case("Defs/ThingDef[defName=\"RS_X\" or defName=\"RS_Y\"]/tools", false)]
    #[case("Defs/ThingDef[defName=\"RS_X\"]/weaponTags/li[.=\"RS_T\"]", false)]
    #[case("ThingDef[defName=\"RS_X\"]", true)]
    #[case("Defs/ThingDef[defName=\"RS_X\"", true)]
    #[case("Defs/ThingDef[defName=\"RS_X]", true)]
    #[case("Defs/ThingDef[\"RS_X\"]", true)]
    #[case("Defs/ThingDef]defName[", true)]
    #[case("", true)]
    fn xpath_shapes(#[case] xpath: &str, #[case] malformed: bool) {
        assert_eq!(malformed_reason(xpath).is_some(), malformed, "{xpath}");
    }
}
