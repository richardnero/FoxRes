//! Any number of resistors in series or parallel, and the best three-resistor
//! combination for a target.

use crate::{ESeries, Error, eseries::error_percent};

/// Sum of `ohms`.
pub fn series(ohms: &[f64]) -> f64 {
    ohms.iter().sum()
}

/// Parallel combination of `ohms`. Any zero gives zero; an empty list gives infinity.
pub fn parallel(ohms: &[f64]) -> f64 {
    if ohms.contains(&0.0) {
        return 0.0;
    }
    1.0 / ohms.iter().map(|r| 1.0 / r).sum::<f64>()
}

/// How three resistors are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topology {
    /// a + b + c
    Series,
    /// a ∥ b ∥ c
    Parallel,
    /// a + (b ∥ c)
    SeriesParallel,
    /// a ∥ (b + c)
    ParallelSeries,
}

impl Topology {
    pub const ALL: [Topology; 4] = [
        Topology::Series,
        Topology::Parallel,
        Topology::SeriesParallel,
        Topology::ParallelSeries,
    ];

    /// Combined resistance.
    pub fn combine(self, a: f64, b: f64, c: f64) -> f64 {
        match self {
            Topology::Series => a + b + c,
            Topology::Parallel => parallel(&[a, b, c]),
            Topology::SeriesParallel => a + parallel(&[b, c]),
            Topology::ParallelSeries => parallel(&[a, b + c]),
        }
    }

    /// `c` that makes the combination exactly `target`, if one exists.
    fn solve_c(self, target: f64, a: f64, b: f64) -> Option<f64> {
        let c = match self {
            Topology::Series => target - a - b,
            Topology::Parallel => 1.0 / (1.0 / target - 1.0 / a - 1.0 / b),
            Topology::SeriesParallel => 1.0 / (1.0 / (target - a) - 1.0 / b),
            Topology::ParallelSeries => 1.0 / (1.0 / target - 1.0 / a) - b,
        };
        (c > 0.0 && c.is_finite()).then_some(c)
    }

    /// Formula with the values filled in, e.g. `a + (b ∥ c)`.
    pub fn describe(self, a: &str, b: &str, c: &str) -> String {
        match self {
            Topology::Series => format!("{a} + {b} + {c}"),
            Topology::Parallel => format!("{a} ∥ {b} ∥ {c}"),
            Topology::SeriesParallel => format!("{a} + ({b} ∥ {c})"),
            Topology::ParallelSeries => format!("{a} ∥ ({b} + {c})"),
        }
    }
}

/// A three-resistor solution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triple {
    pub topology: Topology,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub total: f64,
    pub error: f64,
}

/// Best three standard values from `series` for `target`, over all four
/// topologies. `a` and `b` range over three decades either side of the
/// target; `c` is solved exactly and rounded to its two neighbouring values.
///
/// # Errors
/// [`Error::OutOfRange`] unless `target` is positive and finite.
pub fn best_triple(target: f64, series: ESeries) -> Result<Triple, Error> {
    if !(target > 0.0 && target.is_finite()) {
        return Err(Error::OutOfRange(target));
    }
    let pool = series.range(target / 1000.0, target * 1000.0);
    let mut best: Option<Triple> = None;
    for topology in Topology::ALL {
        for (i, &a) in pool.iter().enumerate() {
            // Order a ≥ b for symmetric topologies to halve the work.
            let start = match topology {
                Topology::Series | Topology::Parallel => i,
                _ => 0,
            };
            for &b in &pool[start..] {
                let Some(c_ideal) = topology.solve_c(target, a, b) else {
                    continue;
                };
                let Ok((lo, hi)) = series.neighbors(c_ideal) else {
                    continue;
                };
                for c in [lo, hi] {
                    let total = topology.combine(a, b, c);
                    let error = error_percent(total, target);
                    if best.is_none_or(|t| error.abs() < t.error.abs()) {
                        best = Some(Triple {
                            topology,
                            a,
                            b,
                            c,
                            total,
                            error,
                        });
                    }
                }
            }
        }
    }
    best.ok_or(Error::OutOfRange(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn series_and_parallel() {
        assert!(approx_eq(series(&[100.0, 220.0, 330.0]), 650.0));
        assert!(approx_eq(
            parallel(&[1000.0, 1000.0, 1000.0, 1000.0]),
            250.0
        ));
        assert_eq!(parallel(&[1000.0, 0.0]), 0.0);
    }

    #[test]
    fn solve_c_is_exact() {
        for t in Topology::ALL {
            let c = t.solve_c(1234.0, 4700.0, 2200.0);
            if let Some(c) = c {
                assert!(approx_eq(t.combine(4700.0, 2200.0, c), 1234.0), "{t:?}");
            }
        }
    }

    #[test]
    fn triple_beats_pair() {
        let target = 1234.0;
        let pair = crate::combo::best_pair(target, ESeries::E12, crate::combo::Connection::Series)
            .unwrap();
        let triple = best_triple(target, ESeries::E12).unwrap();
        assert!(triple.error.abs() <= pair.error.abs());
    }
}
