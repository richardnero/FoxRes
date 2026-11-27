//! Tolerance analysis: worst case and root-sum-square (RSS).

use crate::Error;

/// Result of a tolerance analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spread {
    pub nominal: f64,
    /// Lowest value over all tolerance corners.
    pub min: f64,
    /// Highest value over all tolerance corners.
    pub max: f64,
    /// RSS deviation: the root sum of each component's individual effect at
    /// its full tolerance. A statistical estimate, tighter than worst case.
    pub rss: f64,
}

/// Analyse `f(values)` when each value may be off by `tol_percent[i]` %.
///
/// Worst case evaluates every one of the 2ⁿ tolerance corners, which is exact
/// for functions that are monotonic in each value (series, parallel, divider
/// ratio, bridges). Up to 16 values are accepted.
///
/// # Errors
/// [`Error::Invalid`] if the slices differ in length, are empty, or have more
/// than 16 entries.
pub fn analyse(
    values: &[f64],
    tol_percent: &[f64],
    f: impl Fn(&[f64]) -> f64,
) -> Result<Spread, Error> {
    if values.len() != tol_percent.len() || values.is_empty() || values.len() > 16 {
        return Err(Error::Invalid(
            "give 1 to 16 values, each with a tolerance".into(),
        ));
    }
    let nominal = f(values);
    let n = values.len();
    let mut x = values.to_vec();
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for corner in 0u32..(1 << n) {
        for i in 0..n {
            let sign = if (corner >> i) & 1 == 1 { 1.0 } else { -1.0 };
            x[i] = values[i] * (1.0 + sign * tol_percent[i] / 100.0);
        }
        let y = f(&x);
        min = min.min(y);
        max = max.max(y);
    }
    let mut sum_sq = 0.0;
    for i in 0..n {
        x.copy_from_slice(values);
        x[i] = values[i] * (1.0 + tol_percent[i] / 100.0);
        let up = f(&x) - nominal;
        x[i] = values[i] * (1.0 - tol_percent[i] / 100.0);
        let down = f(&x) - nominal;
        let effect = up.abs().max(down.abs());
        sum_sq += effect * effect;
    }
    Ok(Spread {
        nominal,
        min,
        max,
        rss: sum_sq.sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{divider, network};

    #[test]
    fn series_worst_case_is_sum_of_tolerances() {
        let s = analyse(&[1000.0, 2000.0], &[1.0, 1.0], network::series).unwrap();
        assert!((s.min - 2970.0).abs() < 1e-9 && (s.max - 3030.0).abs() < 1e-9);
        // RSS: sqrt(10² + 20²).
        assert!((s.rss - 500f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn divider_worst_case_matches_closed_form() {
        let s = analyse(&[10_000.0, 10_000.0], &[1.0, 1.0], |v| {
            divider::ratio(v[0], v[1])
        })
        .unwrap();
        let (lo, hi) = divider::ratio_range(10_000.0, 10_000.0, 1.0, 1.0);
        assert!((s.min - lo).abs() < 1e-15 && (s.max - hi).abs() < 1e-15);
    }
}
