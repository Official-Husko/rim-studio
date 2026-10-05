//! In house statistics used by the calibration code.
//!
//! Everything here is deterministic and total: functions never panic, ignore non finite inputs where that
//! makes sense and return `None` when a statistic is not defined for the data (too few points, no spread).
//! The routines are small on purpose: pools hold tens to a few hundred items, so clarity wins over speed.

use serde::{Deserialize, Serialize};

/// Factor that turns a median absolute deviation into a standard deviation estimate for normal data.
pub const MAD_SCALE: f64 = 1.4826;

/// Returns the finite values of `values` sorted ascending.
#[must_use]
pub fn finite_sorted(values: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    out.sort_by(f64::total_cmp);
    out
}

/// Arithmetic mean of the finite values, or `None` when there are none.
#[must_use]
pub fn mean(values: &[f64]) -> Option<f64> {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return None;
    }
    Some(finite.iter().sum::<f64>() / finite.len() as f64)
}

/// Population standard deviation of the finite values, or `None` when there are fewer than two.
#[must_use]
pub fn std_dev(values: &[f64]) -> Option<f64> {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 2 {
        return None;
    }
    let m = finite.iter().sum::<f64>() / finite.len() as f64;
    let var = finite.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / finite.len() as f64;
    Some(var.sqrt())
}

/// Quantile of an ascending sorted slice by linear interpolation between order statistics (position
/// `q * (n - 1)`). `q` is clamped to `[0, 1]`. Returns `None` for an empty slice.
#[must_use]
pub fn quantile_sorted(sorted: &[f64], q: f64) -> Option<f64> {
    let first = *sorted.first()?;
    let last = *sorted.last()?;
    if !q.is_finite() {
        return None;
    }
    let q = q.clamp(0.0, 1.0);
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor();
    let frac = pos - lo;
    let lo_idx = lo as usize;
    let a = sorted.get(lo_idx).copied().unwrap_or(first);
    let b = sorted.get(lo_idx + 1).copied().unwrap_or(last);
    if frac <= 0.0 {
        Some(a)
    } else {
        Some(a + (b - a) * frac)
    }
}

/// Quantile of the finite values (see [`quantile_sorted`]).
#[must_use]
pub fn quantile(values: &[f64], q: f64) -> Option<f64> {
    quantile_sorted(&finite_sorted(values), q)
}

/// Median of the finite values, or `None` when there are none.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    quantile(values, 0.5)
}

/// Median absolute deviation (unscaled) of the finite values.
#[must_use]
pub fn mad(values: &[f64]) -> Option<f64> {
    let med = median(values)?;
    let devs: Vec<f64> = values
        .iter()
        .filter(|v| v.is_finite())
        .map(|v| (v - med).abs())
        .collect();
    median(&devs)
}

/// Spread of a distribution used for the "flat stat" and "ask" rules: `ln(p90 / p10)` when every value is
/// strictly positive, otherwise `p90 - p10`. Returns 0 for fewer than two values.
#[must_use]
pub fn ln_spread(values: &[f64]) -> f64 {
    let sorted = finite_sorted(values);
    if sorted.len() < 2 {
        return 0.0;
    }
    let (Some(p10), Some(p90)) = (quantile_sorted(&sorted, 0.1), quantile_sorted(&sorted, 0.9))
    else {
        return 0.0;
    };
    if sorted.iter().all(|v| *v > 0.0) && p10 > 0.0 {
        (p90 / p10).ln()
    } else {
        p90 - p10
    }
}

/// Average ranks (1 based, ties share the mean of their ranks). Non finite values sort last.
#[must_use]
pub fn average_ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|a, b| {
        let va = values.get(*a).copied().unwrap_or(f64::NAN);
        let vb = values.get(*b).copied().unwrap_or(f64::NAN);
        va.total_cmp(&vb).then(a.cmp(b))
    });
    let mut ranks = vec![0.0; values.len()];
    let mut i = 0;
    while i < order.len() {
        let vi = order
            .get(i)
            .and_then(|k| values.get(*k))
            .copied()
            .unwrap_or(f64::NAN);
        let mut j = i + 1;
        while j < order.len() {
            let vj = order
                .get(j)
                .and_then(|k| values.get(*k))
                .copied()
                .unwrap_or(f64::NAN);
            if vj.total_cmp(&vi) != std::cmp::Ordering::Equal {
                break;
            }
            j += 1;
        }
        let avg = (i + 1 + j) as f64 / 2.0;
        for k in order.iter().take(j).skip(i) {
            if let Some(slot) = ranks.get_mut(*k) {
                *slot = avg;
            }
        }
        i = j;
    }
    ranks
}

/// Mid rank of `x` inside `values`: `(count below + half the count equal) / n`, in `[0, 1]`. An empty
/// slice gives 0.5. Only finite values count.
#[must_use]
pub fn mid_rank_of(values: &[f64], x: f64) -> f64 {
    let mut n = 0usize;
    let mut below = 0usize;
    let mut equal = 0usize;
    for v in values.iter().filter(|v| v.is_finite()) {
        n += 1;
        if *v < x {
            below += 1;
        } else if (*v - x).abs() <= f64::EPSILON * x.abs().max(1.0) {
            equal += 1;
        }
    }
    if n == 0 || !x.is_finite() {
        return 0.5;
    }
    (below as f64 + 0.5 * equal as f64) / n as f64
}

/// Pearson correlation of two equally long series, `None` when undefined (length below two, a constant
/// series or a length mismatch).
#[must_use]
pub fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
        sxy += (a - mx) * (b - my);
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return None;
    }
    let r = sxy / (sxx * syy).sqrt();
    r.is_finite().then_some(r.clamp(-1.0, 1.0))
}

/// Spearman rank correlation (Pearson on average ranks).
#[must_use]
pub fn spearman(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() {
        return None;
    }
    pearson(&average_ranks(x), &average_ranks(y))
}

/// Solves the square system `a x = b` by Gaussian elimination with partial pivoting.
///
/// `a` holds `n` rows of `n` entries. Returns `None` for a shape mismatch or a singular matrix.
#[must_use]
pub fn solve_linear(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    if n == 0 || a.len() != n || a.iter().any(|row| row.len() != n) {
        return None;
    }
    // Augmented matrix, row major; every index below is bounded by n and n + 1.
    let w = n + 1;
    let mut m: Vec<f64> = Vec::with_capacity(n * w);
    for (row, rhs) in a.iter().zip(b) {
        m.extend_from_slice(row);
        m.push(*rhs);
    }
    if m.iter().any(|v| !v.is_finite()) {
        return None;
    }
    for col in 0..n {
        let mut pivot = col;
        let mut best = m.get(col * w + col).copied().unwrap_or(0.0).abs();
        for r in (col + 1)..n {
            let v = m.get(r * w + col).copied().unwrap_or(0.0).abs();
            if v > best {
                best = v;
                pivot = r;
            }
        }
        if best < 1e-12 {
            return None;
        }
        if pivot != col {
            for c in 0..w {
                m.swap(col * w + c, pivot * w + c);
            }
        }
        let diag = m.get(col * w + col).copied().unwrap_or(1.0);
        for r in (col + 1)..n {
            let factor = m.get(r * w + col).copied().unwrap_or(0.0) / diag;
            if factor == 0.0 {
                continue;
            }
            for c in col..w {
                let sub = m.get(col * w + c).copied().unwrap_or(0.0) * factor;
                if let Some(cell) = m.get_mut(r * w + c) {
                    *cell -= sub;
                }
            }
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let mut acc = m.get(r * w + n).copied().unwrap_or(0.0);
        for c in (r + 1)..n {
            acc -= m.get(r * w + c).copied().unwrap_or(0.0) * x.get(c).copied().unwrap_or(0.0);
        }
        let diag = m.get(r * w + r).copied().unwrap_or(1.0);
        let value = acc / diag;
        if !value.is_finite() {
            return None;
        }
        if let Some(slot) = x.get_mut(r) {
            *slot = value;
        }
    }
    Some(x)
}

/// Ridge regression without an intercept: minimises `|y - X b|^2 + lambda |b|^2`.
///
/// `rows` is the design matrix (one `Vec` per observation, all of the same width). Returns `None` for
/// empty or ragged input or a singular system (a positive `lambda` prevents that).
#[must_use]
pub fn ridge_solve(rows: &[Vec<f64>], y: &[f64], lambda: f64) -> Option<Vec<f64>> {
    let first = rows.first()?;
    let p = first.len();
    if p == 0 || rows.len() != y.len() || rows.iter().any(|r| r.len() != p) {
        return None;
    }
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for (row, target) in rows.iter().zip(y) {
        for (i, a) in row.iter().enumerate() {
            if let Some(slot) = xty.get_mut(i) {
                *slot += a * target;
            }
            for (j, b) in row.iter().enumerate() {
                if let Some(cell) = xtx.get_mut(i).and_then(|r| r.get_mut(j)) {
                    *cell += a * b;
                }
            }
        }
    }
    for (i, row) in xtx.iter_mut().enumerate() {
        if let Some(cell) = row.get_mut(i) {
            *cell += lambda.max(0.0);
        }
    }
    solve_linear(&xtx, &xty)
}

/// Result of a one variable least squares fit `y = intercept + slope * x`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinearFit {
    /// Value of the fitted line at `x = 0`.
    pub intercept: f64,
    /// Slope of the fitted line.
    pub slope: f64,
    /// Coefficient of determination in `[0, 1]` (1 when `y` is constant and fitted exactly).
    pub r2: f64,
    /// Number of observations used.
    pub n: usize,
}

impl LinearFit {
    /// Evaluates the fitted line.
    #[must_use]
    pub fn predict(&self, x: f64) -> f64 {
        self.intercept + self.slope * x
    }
}

/// Ordinary least squares of `y` on `x`. Returns `None` for fewer than two finite pairs or no spread in `x`.
#[must_use]
pub fn linear_fit(x: &[f64], y: &[f64]) -> Option<LinearFit> {
    if x.len() != y.len() {
        return None;
    }
    let pairs: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|(a, b)| (*a, *b))
        .collect();
    if pairs.len() < 2 {
        return None;
    }
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let syy: f64 = pairs.iter().map(|p| (p.1 - my) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let ss_res: f64 = pairs
        .iter()
        .map(|p| (p.1 - (intercept + slope * p.0)).powi(2))
        .sum();
    let r2 = if syy <= 0.0 {
        1.0
    } else {
        (1.0 - ss_res / syy).clamp(0.0, 1.0)
    };
    Some(LinearFit {
        intercept,
        slope,
        r2,
        n: pairs.len(),
    })
}

/// Log linear fit `ln y = intercept + slope * x`; pairs with `y <= 0` are dropped.
#[must_use]
pub fn log_linear_fit(x: &[f64], y: &[f64]) -> Option<LinearFit> {
    if x.len() != y.len() {
        return None;
    }
    let (xs, ys): (Vec<f64>, Vec<f64>) = x
        .iter()
        .zip(y)
        .filter(|(_, b)| **b > 0.0 && b.is_finite())
        .map(|(a, b)| (*a, b.ln()))
        .unzip();
    linear_fit(&xs, &ys)
}

/// Log log fit `ln y = intercept + slope * ln x` (a power law); pairs with a non positive value are dropped.
#[must_use]
pub fn log_log_fit(x: &[f64], y: &[f64]) -> Option<LinearFit> {
    if x.len() != y.len() {
        return None;
    }
    let (xs, ys): (Vec<f64>, Vec<f64>) = x
        .iter()
        .zip(y)
        .filter(|(a, b)| **a > 0.0 && **b > 0.0 && a.is_finite() && b.is_finite())
        .map(|(a, b)| (a.ln(), b.ln()))
        .unzip();
    linear_fit(&xs, &ys)
}

/// Deterministic pseudo random generator (SplitMix64) used by the simulated answer harness.
///
/// It is implemented here so that results do not depend on an external crate version.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator from a seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal value (Box-Muller).
    pub fn next_normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Uniform integer in `0..bound` (0 when `bound` is 0).
    pub fn next_below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[rstest]
    #[case(&[3.0, 1.0, 2.0], Some(2.0))]
    #[case(&[4.0, 1.0, 2.0, 3.0], Some(2.5))]
    #[case(&[5.0], Some(5.0))]
    #[case(&[], None)]
    #[case(&[f64::NAN, 2.0], Some(2.0))]
    fn median_table(#[case] values: &[f64], #[case] expected: Option<f64>) {
        assert_eq!(median(values), expected);
    }

    #[test]
    fn quantile_interpolates_and_clamps() {
        let v = [10.0, 20.0, 30.0, 40.0, 50.0];
        assert!(close(quantile(&v, 0.0).unwrap(), 10.0));
        assert!(close(quantile(&v, 1.0).unwrap(), 50.0));
        assert!(close(quantile(&v, 0.1).unwrap(), 14.0));
        assert!(close(quantile(&v, 0.9).unwrap(), 46.0));
        assert!(close(quantile(&v, 7.0).unwrap(), 50.0));
        assert!(close(quantile(&v, -1.0).unwrap(), 10.0));
        assert_eq!(quantile(&v, f64::NAN), None);
    }

    #[test]
    fn mad_matches_hand_value() {
        // deviations from 3 are 2,1,0,1,6 with median 1
        assert!(close(mad(&[1.0, 2.0, 3.0, 4.0, 9.0]).unwrap(), 1.0));
        assert_eq!(mad(&[]), None);
    }

    #[test]
    fn ranks_average_ties() {
        assert_eq!(
            average_ranks(&[10.0, 20.0, 20.0, 30.0]),
            vec![1.0, 2.5, 2.5, 4.0]
        );
        assert!(average_ranks(&[]).is_empty());
    }

    #[test]
    fn mid_rank_counts_half_of_ties() {
        let v = [1.0, 2.0, 2.0, 3.0];
        assert!(close(mid_rank_of(&v, 2.0), 0.5));
        assert!(close(mid_rank_of(&v, 0.0), 0.0));
        assert!(close(mid_rank_of(&v, 9.0), 1.0));
        assert!(close(mid_rank_of(&[], 1.0), 0.5));
    }

    #[test]
    fn spearman_is_one_for_monotone_and_minus_one_for_reversed() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [1.0, 4.0, 9.0, 16.0, 25.0];
        let z = [5.0, 4.0, 3.0, 2.0, 1.0];
        assert!(close(spearman(&x, &y).unwrap(), 1.0));
        assert!(close(spearman(&x, &z).unwrap(), -1.0));
        assert_eq!(spearman(&x, &[1.0; 5]), None);
        assert_eq!(spearman(&x, &[1.0]), None);
    }

    #[test]
    fn linear_fit_recovers_a_line() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [1.0, 3.0, 5.0, 7.0];
        let f = linear_fit(&x, &y).unwrap();
        assert!(close(f.slope, 2.0) && close(f.intercept, 1.0) && close(f.r2, 1.0));
        assert!(close(f.predict(10.0), 21.0));
        assert!(linear_fit(&[1.0, 1.0], &[1.0, 2.0]).is_none());
    }

    #[test]
    fn log_fits_recover_exponent_and_rate() {
        let x: Vec<f64> = (1..8).map(f64::from).collect();
        let power: Vec<f64> = x.iter().map(|v| 3.0 * v.powf(0.6)).collect();
        let f = log_log_fit(&x, &power).unwrap();
        assert!(close(f.slope, 0.6) && close(f.intercept, 3.0f64.ln()));
        let expo: Vec<f64> = x.iter().map(|v| 2.0 * (0.3 * v).exp()).collect();
        let g = log_linear_fit(&x, &expo).unwrap();
        assert!(close(g.slope, 0.3) && close(g.intercept, 2.0f64.ln()));
    }

    #[test]
    fn ridge_shrinks_towards_zero() {
        let rows: Vec<Vec<f64>> = (0..10).map(|i| vec![f64::from(i) - 4.5]).collect();
        let y: Vec<f64> = rows.iter().map(|r| 2.0 * r[0]).collect();
        let plain = ridge_solve(&rows, &y, 0.0).unwrap()[0];
        let shrunk = ridge_solve(&rows, &y, 50.0).unwrap()[0];
        assert!(close(plain, 2.0));
        assert!(shrunk > 0.0 && shrunk < plain);
        assert!(ridge_solve(&[], &[], 0.2).is_none());
        assert!(ridge_solve(&[vec![1.0], vec![1.0, 2.0]], &[1.0, 2.0], 0.2).is_none());
    }

    #[test]
    fn solve_linear_handles_pivoting_and_singular() {
        let a = vec![vec![0.0, 2.0], vec![1.0, 1.0]];
        let x = solve_linear(&a, &[4.0, 3.0]).unwrap();
        assert!(close(x[0], 1.0) && close(x[1], 2.0));
        assert!(solve_linear(&[vec![1.0, 2.0], vec![2.0, 4.0]], &[1.0, 2.0]).is_none());
    }

    #[test]
    fn ln_spread_switches_for_non_positive_values() {
        assert!(close(ln_spread(&[1.0, 1.0, 1.0]), 0.0));
        assert!(ln_spread(&[1.0, 2.0, 4.0, 8.0]) > 1.0);
        assert!(close(ln_spread(&[0.0, 1.0]), 0.9 - 0.1));
    }

    #[test]
    fn generator_is_reproducible_and_in_range() {
        let mut a = SplitMix64::new(1000);
        let mut b = SplitMix64::new(1000);
        for _ in 0..100 {
            let (x, y) = (a.next_f64(), b.next_f64());
            assert_eq!(x.to_bits(), y.to_bits());
            assert!((0.0..1.0).contains(&x));
        }
        let mut g = SplitMix64::new(7);
        let normals: Vec<f64> = (0..4000).map(|_| g.next_normal()).collect();
        assert!(mean(&normals).unwrap().abs() < 0.1);
        assert!((std_dev(&normals).unwrap() - 1.0).abs() < 0.1);
        assert_eq!(g.next_below(0), 0);
    }

    proptest! {
        #[test]
        fn quantile_is_monotone_and_bounded(values in proptest::collection::vec(-1.0e6f64..1.0e6, 1..40), q1 in 0.0f64..1.0, q2 in 0.0f64..1.0) {
            let (lo, hi) = if q1 <= q2 { (q1, q2) } else { (q2, q1) };
            let a = quantile(&values, lo).unwrap();
            let b = quantile(&values, hi).unwrap();
            prop_assert!(a <= b + 1e-9);
            let sorted = finite_sorted(&values);
            prop_assert!(a >= sorted[0] - 1e-9 && b <= sorted[sorted.len() - 1] + 1e-9);
        }

        #[test]
        fn spearman_is_bounded(x in proptest::collection::vec(-100.0f64..100.0, 3..20), y in proptest::collection::vec(-100.0f64..100.0, 3..20)) {
            let n = x.len().min(y.len());
            if let Some(r) = spearman(&x[..n], &y[..n]) {
                prop_assert!((-1.0..=1.0).contains(&r));
            }
        }

        #[test]
        fn ranks_sum_to_triangle_number(values in proptest::collection::vec(-50.0f64..50.0, 1..30)) {
            let total: f64 = average_ranks(&values).iter().sum();
            let n = values.len() as f64;
            prop_assert!((total - n * (n + 1.0) / 2.0).abs() < 1e-6);
        }
    }
}
