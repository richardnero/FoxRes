//! Voltage dividers.
//!
//! `R1` is the top resistor (from the input), `R2` the bottom one (to ground);
//! the output is taken across `R2`.

use crate::{ESeries, Error, eseries::error_percent};

/// Output voltage, with an optional load across `R2`.
pub fn output(vin: f64, r1: f64, r2: f64, load: Option<f64>) -> f64 {
    let bottom = load.map_or(r2, |rl| r2 * rl / (r2 + rl));
    vin * bottom / (r1 + bottom)
}

/// Ratio `Vout / Vin` for an unloaded divider.
pub fn ratio(r1: f64, r2: f64) -> f64 {
    r2 / (r1 + r2)
}

/// A divider built from standard values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    pub r1: f64,
    pub r2: f64,
    pub ratio: f64,
    /// Percentage error of the ratio from the target.
    pub error: f64,
}

/// Best `R1`, `R2` from `series` for `target_ratio` (0 < ratio < 1).
///
/// The ratio does not depend on scale, so many pairs tie; among those with the
/// smallest error, the one whose `R1 + R2` is closest to `total_hint` wins.
///
/// # Errors
/// [`Error::Invalid`] unless `0 < target_ratio < 1` and `total_hint > 0`.
pub fn best_pair(target_ratio: f64, series: ESeries, total_hint: f64) -> Result<Pair, Error> {
    if !(target_ratio > 0.0 && target_ratio < 1.0) {
        return Err(Error::Invalid("the ratio must be between 0 and 1".into()));
    }
    if !(total_hint > 0.0 && total_hint.is_finite()) {
        return Err(Error::Invalid(
            "the total resistance must be positive".into(),
        ));
    }
    // R1 / R2 = 1/ratio - 1.
    let k = 1.0 / target_ratio - 1.0;
    let mut best: Option<(Pair, f64)> = None;
    for r2 in series.range(total_hint / 1000.0, total_hint * 10.0) {
        let Ok((lo, hi)) = series.neighbors(k * r2) else {
            continue;
        };
        for r1 in [lo, hi] {
            let r = ratio(r1, r2);
            let pair = Pair {
                r1,
                r2,
                ratio: r,
                error: error_percent(r, target_ratio),
            };
            let scale = ((r1 + r2) / total_hint).ln().abs();
            let better = match &best {
                None => true,
                Some((b, bs)) => {
                    let (e, be) = (pair.error.abs(), b.error.abs());
                    e < be - 1e-12 || ((e - be).abs() <= 1e-12 && scale < *bs)
                }
            };
            if better {
                best = Some((pair, scale));
            }
        }
    }
    best.map(|(p, _)| p)
        .ok_or(Error::Invalid("no pair found".into()))
}

/// Lowest and highest possible ratio for resistor tolerances in percent.
pub fn ratio_range(r1: f64, r2: f64, tol1: f64, tol2: f64) -> (f64, f64) {
    let (t1, t2) = (tol1 / 100.0, tol2 / 100.0);
    (
        ratio(r1 * (1.0 + t1), r2 * (1.0 - t2)),
        ratio(r1 * (1.0 - t1), r2 * (1.0 + t2)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn output_with_and_without_load() {
        assert!(approx_eq(output(10.0, 10_000.0, 10_000.0, None), 5.0));
        // 10k || 10k = 5k, so 10 V × 5k / 15k.
        assert!(approx_eq(
            output(10.0, 10_000.0, 10_000.0, Some(10_000.0)),
            10.0 / 3.0
        ));
    }

    #[test]
    fn finds_exact_half() {
        let p = best_pair(0.5, ESeries::E12, 10_000.0).unwrap();
        assert!(p.error.abs() < 1e-12);
        assert!(approx_eq(p.r1, p.r2));
        // Of all exact pairs, the one closest to the 10 kΩ hint: 4.7k + 4.7k or 5.6k + 5.6k.
        assert!((p.r1 + p.r2 - 10_000.0).abs() <= 1_200.0, "{p:?}");
    }

    #[test]
    fn ratio_range_brackets_nominal() {
        let (lo, hi) = ratio_range(10_000.0, 10_000.0, 1.0, 1.0);
        assert!(lo < 0.5 && hi > 0.5);
        assert!(approx_eq(lo, 9_900.0 / 20_000.0));
    }
}
