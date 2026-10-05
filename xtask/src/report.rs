//! Findings shared by every check, and the one-line output format.

use std::fmt;

/// One rule violation: the rule id, where it happened and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    /// Stable rule id such as `layers.edge`.
    pub(crate) rule: String,
    /// File (and optionally line) or crate name.
    pub(crate) location: String,
    /// Human readable explanation.
    pub(crate) message: String,
}

impl Finding {
    /// Builds a finding.
    pub(crate) fn new(rule: &str, location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.to_string(),
            location: location.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.rule, self.location, self.message)
    }
}

/// Sorts findings for deterministic output and prints them, one per line.
pub(crate) fn print_findings(findings: &mut [Finding]) {
    findings.sort();
    for f in findings.iter() {
        println!("{f}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_one_line_with_rule_location_and_message() {
        let f = Finding::new("docs.dash", "docs/a.md:3", "bad");
        assert_eq!(f.to_string(), "docs.dash docs/a.md:3: bad");
    }
}
