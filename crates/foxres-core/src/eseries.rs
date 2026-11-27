//! IEC 60063 preferred number series.

use crate::Error;

const E3: [u16; 3] = [10, 22, 47];
const E6: [u16; 6] = [10, 15, 22, 33, 47, 68];
const E12: [u16; 12] = [10, 12, 15, 18, 22, 27, 33, 39, 47, 56, 68, 82];
const E24: [u16; 24] = [
    10, 11, 12, 13, 15, 16, 18, 20, 22, 24, 27, 30, 33, 36, 39, 43, 47, 51, 56, 62, 68, 75, 82, 91,
];
#[rustfmt::skip]
const E192: [u16; 192] = [
    100, 101, 102, 104, 105, 106, 107, 109, 110, 111, 113, 114, 115, 117, 118, 120,
    121, 123, 124, 126, 127, 129, 130, 132, 133, 135, 137, 138, 140, 142, 143, 145,
    147, 149, 150, 152, 154, 156, 158, 160, 162, 164, 165, 167, 169, 172, 174, 176,
    178, 180, 182, 184, 187, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213,
    215, 218, 221, 223, 226, 229, 232, 234, 237, 240, 243, 246, 249, 252, 255, 258,
    261, 264, 267, 271, 274, 277, 280, 284, 287, 291, 294, 298, 301, 305, 309, 312,
    316, 320, 324, 328, 332, 336, 340, 344, 348, 352, 357, 361, 365, 370, 374, 379,
    383, 388, 392, 397, 402, 407, 412, 417, 422, 427, 432, 437, 442, 448, 453, 459,
    464, 470, 475, 481, 487, 493, 499, 505, 511, 517, 523, 530, 536, 542, 549, 556,
    562, 569, 576, 583, 590, 597, 604, 612, 619, 626, 634, 642, 649, 657, 665, 673,
    681, 690, 698, 706, 715, 723, 732, 741, 750, 759, 768, 777, 787, 796, 806, 816,
    825, 835, 845, 856, 866, 876, 887, 898, 909, 920, 931, 942, 953, 965, 976, 988,
];

const fn every_nth<const N: usize>(step: usize) -> [u16; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = E192[i * step];
        i += 1;
    }
    out
}

const E96: [u16; 96] = every_nth(2);
const E48: [u16; 48] = every_nth(4);

/// A preferred-value series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ESeries {
    E3,
    E6,
    E12,
    E24,
    E48,
    E96,
    E192,
}

impl ESeries {
    /// All series, coarsest first.
    pub const ALL: [ESeries; 7] = [
        ESeries::E3,
        ESeries::E6,
        ESeries::E12,
        ESeries::E24,
        ESeries::E48,
        ESeries::E96,
        ESeries::E192,
    ];

    /// Name such as `"E24"`.
    pub fn name(self) -> &'static str {
        match self {
            ESeries::E3 => "E3",
            ESeries::E6 => "E6",
            ESeries::E12 => "E12",
            ESeries::E24 => "E24",
            ESeries::E48 => "E48",
            ESeries::E96 => "E96",
            ESeries::E192 => "E192",
        }
    }

    /// Typical tolerance in percent for parts sold in this series.
    pub fn tolerance(self) -> f64 {
        match self {
            ESeries::E3 => 40.0,
            ESeries::E6 => 20.0,
            ESeries::E12 => 10.0,
            ESeries::E24 => 5.0,
            ESeries::E48 => 2.0,
            ESeries::E96 => 1.0,
            ESeries::E192 => 0.5,
        }
    }

    /// Mantissas for one decade, e.g. `[10, 22, 47]` for E3.
    pub fn values(self) -> &'static [u16] {
        match self {
            ESeries::E3 => &E3,
            ESeries::E6 => &E6,
            ESeries::E12 => &E12,
            ESeries::E24 => &E24,
            ESeries::E48 => &E48,
            ESeries::E96 => &E96,
            ESeries::E192 => &E192,
        }
    }

    /// Significant digits per value (2 or 3).
    pub fn digits(self) -> i32 {
        if self.values()[0] >= 100 { 3 } else { 2 }
    }

    /// Every value of the series from `min` to `max` ohms inclusive, ascending.
    pub fn range(self, min: f64, max: f64) -> Vec<f64> {
        if !(min > 0.0 && max >= min && max.is_finite()) {
            return Vec::new();
        }
        let first = min.log10().floor() as i32 - 1;
        let last = max.log10().ceil() as i32;
        (first..=last)
            .flat_map(|decade| self.values().iter().map(move |&m| scaled(self, m, decade)))
            .filter(|&v| v >= min * (1.0 - 1e-12) && v <= max * (1.0 + 1e-12))
            .collect()
    }

    /// The series values just below and just above `ohms` (equal values count as both).
    ///
    /// # Errors
    /// Returns [`Error::OutOfRange`] unless `ohms` is positive and finite.
    pub fn neighbors(self, ohms: f64) -> Result<(f64, f64), Error> {
        if !(ohms > 0.0 && ohms.is_finite()) {
            return Err(Error::OutOfRange(ohms));
        }
        // Candidates, ascending: the last value of the previous decade, every
        // value of the decade containing `ohms`, and the first of the next.
        // Binary search over them needs no allocation, so this is cheap enough
        // to call millions of times from the combination searches.
        let values = self.values();
        let n = values.len();
        let exp = ohms.log10().floor() as i32 - (self.digits() - 1);
        let at = |i: usize| -> f64 {
            if i == 0 {
                crate::value::scale10(f64::from(values[n - 1]), exp - 1)
            } else if i <= n {
                crate::value::scale10(f64::from(values[i - 1]), exp)
            } else {
                crate::value::scale10(f64::from(values[0]), exp + 1)
            }
        };
        // First index in 0..n + 2 where `pred` holds (pred is monotonic).
        let first = |pred: &dyn Fn(f64) -> bool| -> usize {
            let (mut lo, mut hi) = (0, n + 2);
            while lo < hi {
                let mid = usize::midpoint(lo, hi);
                if pred(at(mid)) {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lo
        };
        let above_i = first(&|v| v >= ohms * (1.0 - 1e-12));
        let below_end = first(&|v| v > ohms * (1.0 + 1e-12));
        if above_i > n + 1 || below_end == 0 {
            return Err(Error::OutOfRange(ohms));
        }
        Ok((at(below_end - 1), at(above_i)))
    }

    /// Closest series value to `ohms`, compared on a logarithmic scale.
    ///
    /// # Errors
    /// Returns [`Error::OutOfRange`] unless `ohms` is positive and finite.
    pub fn nearest(self, ohms: f64) -> Result<f64, Error> {
        let (below, above) = self.neighbors(ohms)?;
        Ok(if (ohms / below) <= (above / ohms) {
            below
        } else {
            above
        })
    }
}

fn scaled(series: ESeries, mantissa: u16, decade: i32) -> f64 {
    let exp = decade - (series.digits() - 1);
    crate::value::scale10(f64::from(mantissa), exp)
}

/// Percentage difference of `actual` from `target`.
pub fn error_percent(actual: f64, target: f64) -> f64 {
    (actual - target) / target * 100.0
}

impl std::str::FromStr for ESeries {
    type Err = Error;

    /// Parse `E24`, `e24` or `24`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        let digits = t.strip_prefix(['E', 'e']).unwrap_or(t);
        ESeries::ALL
            .into_iter()
            .find(|e| &e.name()[1..] == digits)
            .ok_or_else(|| Error::Parse(t.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn table_sizes_and_order() {
        for s in ESeries::ALL {
            let v = s.values();
            assert_eq!(
                v.len(),
                s.name()[1..].parse::<usize>().unwrap(),
                "{}",
                s.name()
            );
            assert!(
                v.windows(2).all(|w| w[0] < w[1]),
                "{} not ascending",
                s.name()
            );
        }
    }

    #[test]
    fn known_e96_values() {
        assert_eq!(&E96[..6], &[100, 102, 105, 107, 110, 113]);
        assert_eq!(E96[95], 976);
        assert_eq!(&E48[..4], &[100, 105, 110, 115]);
    }

    #[test]
    fn nearest_values() {
        assert!(approx_eq(ESeries::E12.nearest(5000.0).unwrap(), 4700.0));
        assert!(approx_eq(ESeries::E24.nearest(5000.0).unwrap(), 5100.0));
        assert!(approx_eq(ESeries::E96.nearest(5000.0).unwrap(), 4990.0));
        assert!(approx_eq(ESeries::E12.nearest(95.0).unwrap(), 100.0));
        assert!(approx_eq(ESeries::E24.nearest(4700.0).unwrap(), 4700.0));
        assert!(approx_eq(ESeries::E6.nearest(0.5).unwrap(), 0.47));
        assert!(ESeries::E6.nearest(0.0).is_err());
    }

    #[test]
    fn range_spans_decades() {
        let r = ESeries::E3.range(1.0, 1000.0);
        let want = [1.0, 2.2, 4.7, 10.0, 22.0, 47.0, 100.0, 220.0, 470.0, 1000.0];
        assert_eq!(r.len(), want.len());
        assert!(r.iter().zip(want).all(|(a, b)| approx_eq(*a, b)));
    }

    #[test]
    fn parses_names() {
        assert_eq!("E24".parse::<ESeries>().unwrap(), ESeries::E24);
        assert_eq!("e96".parse::<ESeries>().unwrap(), ESeries::E96);
        assert_eq!("12".parse::<ESeries>().unwrap(), ESeries::E12);
        assert!("E7".parse::<ESeries>().is_err());
    }

    /// The original allocation-based definition, kept as the reference.
    fn neighbors_reference(series: ESeries, ohms: f64) -> (f64, f64) {
        let c = series.range(ohms / 10.0, ohms * 10.0);
        let below = c
            .iter()
            .copied()
            .rfind(|&v| v <= ohms * (1.0 + 1e-12))
            .unwrap();
        let above = c
            .iter()
            .copied()
            .find(|&v| v >= ohms * (1.0 - 1e-12))
            .unwrap();
        (below, above)
    }

    #[test]
    fn fast_neighbors_match_reference() {
        for series in ESeries::ALL {
            // Every series value itself, across decades, and points between them.
            for decade in -2..=8 {
                for &m in series.values() {
                    let v = scaled(series, m, decade);
                    for x in [v, v * (1.0 + 1e-9), v * (1.0 - 1e-9), v * 1.004, v * 0.996] {
                        assert_eq!(
                            series.neighbors(x).unwrap(),
                            neighbors_reference(series, x),
                            "{} {x}",
                            series.name()
                        );
                    }
                }
            }
            for k in 0..20_000 {
                let x = 10f64.powf(-1.5 + f64::from(k) * 0.0005);
                assert_eq!(
                    series.neighbors(x).unwrap(),
                    neighbors_reference(series, x),
                    "{} {x}",
                    series.name()
                );
            }
        }
    }
}
