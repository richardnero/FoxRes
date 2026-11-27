//! Make a value you don't stock out of two values you do.

use crate::{ESeries, Error, eseries::error_percent};

/// How two resistors are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Series,
    Parallel,
}

impl Connection {
    /// Combined resistance of `a` and `b`.
    pub fn combine(self, a: f64, b: f64) -> f64 {
        match self {
            Connection::Series => a + b,
            Connection::Parallel => a * b / (a + b),
        }
    }
}

/// A two-resistor solution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    pub connection: Connection,
    /// The larger of the two.
    pub a: f64,
    pub b: f64,
    /// Combined resistance.
    pub total: f64,
    /// Percentage error of `total` from the target.
    pub error: f64,
}

/// Best pair from `series` for `target` ohms with the given connection.
///
/// Searches three decades on the useful side of the target.
///
/// # Errors
/// Returns [`Error::OutOfRange`] unless `target` is positive and finite.
pub fn best_pair(target: f64, series: ESeries, connection: Connection) -> Result<Pair, Error> {
    if !(target > 0.0 && target.is_finite()) {
        return Err(Error::OutOfRange(target));
    }
    let candidates = match connection {
        Connection::Series => series.range(target / 1000.0, target),
        Connection::Parallel => series.range(target, target * 1000.0),
    };

    candidates
        .into_iter()
        .filter_map(|a| {
            let needed = match connection {
                Connection::Series => target - a,
                Connection::Parallel => 1.0 / (1.0 / target - 1.0 / a),
            };
            // Try the standard values either side of the ideal partner. The
            // total is monotonic in `b`, so the best partner is one of them.
            let (lo, hi) = series.neighbors(needed).ok()?;
            [lo, hi]
                .into_iter()
                .map(|b| {
                    let total = connection.combine(a, b);
                    let (a, b) = if b > a { (b, a) } else { (a, b) };
                    Pair {
                        connection,
                        a,
                        b,
                        total,
                        error: error_percent(total, target),
                    }
                })
                .min_by(|x, y| x.error.abs().total_cmp(&y.error.abs()))
        })
        .min_by(|x, y| x.error.abs().total_cmp(&y.error.abs()))
        .ok_or(Error::OutOfRange(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn combine() {
        assert!(approx_eq(Connection::Series.combine(100.0, 220.0), 320.0));
        assert!(approx_eq(
            Connection::Parallel.combine(1000.0, 1000.0),
            500.0
        ));
    }

    #[test]
    fn finds_exact_series_pair() {
        // 5.7 kΩ = 4.7 kΩ + 1 kΩ in E12.
        let p = best_pair(5700.0, ESeries::E12, Connection::Series).unwrap();
        assert!(p.error.abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn finds_exact_parallel_pair() {
        // 500 Ω = 1 kΩ ∥ 1 kΩ in E3.
        let p = best_pair(500.0, ESeries::E3, Connection::Parallel).unwrap();
        assert!(p.error.abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn pair_beats_single_value() {
        let target = 3000.0;
        let single = ESeries::E6.nearest(target).unwrap();
        let p = best_pair(target, ESeries::E6, Connection::Series).unwrap();
        assert!(p.error.abs() < error_percent(single, target).abs());
    }
}
