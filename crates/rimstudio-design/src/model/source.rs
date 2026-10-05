//! Source tags: where each number of a draft came from, and the rule that protects typed values.
//!
//! Every numeric input of a [`DesignSpec`](super::DesignSpec) is a [`Sourced`] value. The source decides what
//! may replace it later: a value the user typed is never replaced by a suggestion, an answer or an anchor
//! (requirement IT-003); the other sources replace each other by rank.

use serde::{Deserialize, Serialize};

/// Where a value came from, ordered by how firmly the user stands behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValueSource {
    /// Predicted by the baseline or a role template. Replaced by anything.
    Suggested,
    /// Copied from a reference item chosen as an anchor.
    Anchor,
    /// Derived from an answer of the quiz.
    Answered,
    /// Typed or explicitly chosen by the user. Never replaced by another source.
    Typed,
}

impl ValueSource {
    /// All sources, weakest first.
    pub const ALL: [ValueSource; 4] = [
        ValueSource::Suggested,
        ValueSource::Anchor,
        ValueSource::Answered,
        ValueSource::Typed,
    ];

    /// The kebab-case name used on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Anchor => "anchor",
            Self::Answered => "answered",
            Self::Typed => "typed",
        }
    }

    /// True when a value from `self` may replace a value that has source `existing`.
    ///
    /// Typed values replace anything. Every other source replaces a value of the same or a lower rank and
    /// never a typed value.
    #[must_use]
    pub fn may_replace(self, existing: ValueSource) -> bool {
        match (self, existing) {
            (Self::Typed, _) => true,
            (_, Self::Typed) => false,
            (new, old) => new >= old,
        }
    }
}

fn default_source() -> ValueSource {
    ValueSource::Typed
}

/// A value together with the source it came from. Hand written JSON may omit the source, which means typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sourced<T> {
    /// The value.
    pub value: T,
    /// Where it came from.
    #[serde(default = "default_source")]
    pub source: ValueSource,
}

impl<T> Sourced<T> {
    /// A value with an explicit source.
    #[must_use]
    pub fn new(value: T, source: ValueSource) -> Self {
        Self { value, source }
    }

    /// A value the user typed.
    #[must_use]
    pub fn typed(value: T) -> Self {
        Self::new(value, ValueSource::Typed)
    }

    /// A value predicted by the baseline.
    #[must_use]
    pub fn suggested(value: T) -> Self {
        Self::new(value, ValueSource::Suggested)
    }

    /// A value derived from a quiz answer.
    #[must_use]
    pub fn answered(value: T) -> Self {
        Self::new(value, ValueSource::Answered)
    }

    /// A value copied from an anchor item.
    #[must_use]
    pub fn anchor(value: T) -> Self {
        Self::new(value, ValueSource::Anchor)
    }

    /// True when the user typed the value.
    #[must_use]
    pub fn is_typed(&self) -> bool {
        self.source == ValueSource::Typed
    }
}

/// What [`offer`] did with a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OfferOutcome {
    /// The slot now holds the offered value.
    Applied,
    /// The slot kept its value because it is typed or ranks higher than the offer.
    Kept,
    /// The offer does not fit this draft (a tool index that does not exist, a value that is not a count).
    NotApplicable,
}

impl OfferOutcome {
    /// True for [`OfferOutcome::Applied`].
    #[must_use]
    pub fn applied(self) -> bool {
        self == Self::Applied
    }
}

/// Offers `value` with `source` to a slot, applying the replacement rule of [`ValueSource::may_replace`].
///
/// An empty slot always takes the value. This is the only function that suggestions, answers and anchors use
/// to write into a draft, which is what keeps typed values safe.
pub fn offer<T>(slot: &mut Option<Sourced<T>>, value: T, source: ValueSource) -> OfferOutcome {
    match slot {
        Some(existing) if !source.may_replace(existing.source) => OfferOutcome::Kept,
        _ => {
            *slot = Some(Sourced::new(value, source));
            OfferOutcome::Applied
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(ValueSource::Suggested, ValueSource::Typed, false)]
    #[case(ValueSource::Anchor, ValueSource::Typed, false)]
    #[case(ValueSource::Answered, ValueSource::Typed, false)]
    #[case(ValueSource::Typed, ValueSource::Typed, true)]
    #[case(ValueSource::Typed, ValueSource::Suggested, true)]
    #[case(ValueSource::Suggested, ValueSource::Suggested, true)]
    #[case(ValueSource::Suggested, ValueSource::Answered, false)]
    #[case(ValueSource::Answered, ValueSource::Anchor, true)]
    #[case(ValueSource::Anchor, ValueSource::Answered, false)]
    fn replacement_rule(#[case] new: ValueSource, #[case] old: ValueSource, #[case] allowed: bool) {
        assert_eq!(new.may_replace(old), allowed);
    }

    #[test]
    fn offer_fills_an_empty_slot_and_keeps_typed_values() {
        let mut slot: Option<Sourced<f64>> = None;
        assert_eq!(
            offer(&mut slot, 1.0, ValueSource::Suggested),
            OfferOutcome::Applied
        );
        assert_eq!(slot, Some(Sourced::suggested(1.0)));
        assert_eq!(
            offer(&mut slot, 2.0, ValueSource::Typed),
            OfferOutcome::Applied
        );
        assert_eq!(
            offer(&mut slot, 3.0, ValueSource::Answered),
            OfferOutcome::Kept
        );
        assert_eq!(slot, Some(Sourced::typed(2.0)));
    }

    #[test]
    fn missing_source_in_json_means_typed() {
        let v: Sourced<f64> =
            serde_json::from_str(r#"{"value": 4.5}"#).unwrap_or_else(|_| Sourced::suggested(0.0));
        assert_eq!(v, Sourced::typed(4.5));
        let text = serde_json::to_string(&Sourced::anchor(2u32)).unwrap_or_default();
        assert_eq!(text, r#"{"value":2,"source":"anchor"}"#);
    }
}
