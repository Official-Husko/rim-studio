//! Number formatting for generated text: deterministic, no exponent, no trailing zeros.

/// Formats a number the way a hand written def would: whole numbers without a decimal point, other values
/// with the shortest text that parses back to the same number. Negative zero prints as `0`.
#[must_use]
pub fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        // The check above makes the conversion exact.
        let whole = value as i64;
        return whole.to_string();
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(12.0, "12")]
    #[case(-0.0, "0")]
    #[case(0.15, "0.15")]
    #[case(1.6, "1.6")]
    #[case(-2.5, "-2.5")]
    #[case(26.9, "26.9")]
    #[case(1e-7, "0.0000001")]
    #[case(120000.0, "120000")]
    fn numbers(#[case] value: f64, #[case] want: &str) {
        assert_eq!(format_number(value), want);
    }

    #[test]
    fn output_parses_back() {
        for v in [0.1 + 0.2, 1.0 / 3.0, 123.456, 1e10 + 0.5] {
            let parsed: f64 = format_number(v).parse().unwrap();
            assert_eq!(parsed, v);
        }
    }
}
