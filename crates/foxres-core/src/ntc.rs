//! NTC thermistors: the Beta model and the Steinhart–Hart equation.

use crate::{Error, thermal::ZERO_C_IN_K};

fn kelvin(c: f64) -> f64 {
    c + ZERO_C_IN_K
}

/// Resistance at `temp_c` from the Beta model, `R = R0·e^(β(1/T − 1/T0))`.
pub fn beta_resistance(r0: f64, t0_c: f64, beta: f64, temp_c: f64) -> f64 {
    r0 * (beta * (1.0 / kelvin(temp_c) - 1.0 / kelvin(t0_c))).exp()
}

/// Temperature in °C for `ohms` from the Beta model.
pub fn beta_temperature(r0: f64, t0_c: f64, beta: f64, ohms: f64) -> f64 {
    1.0 / (1.0 / kelvin(t0_c) + (ohms / r0).ln() / beta) - ZERO_C_IN_K
}

/// Beta from two calibration points.
pub fn beta_from_points((t1_c, r1): (f64, f64), (t2_c, r2): (f64, f64)) -> f64 {
    (r1 / r2).ln() / (1.0 / kelvin(t1_c) - 1.0 / kelvin(t2_c))
}

/// Steinhart–Hart coefficients: `1/T = a + b·ln R + c·(ln R)³`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteinhartHart {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl SteinhartHart {
    /// Fit the three coefficients exactly through three `(°C, Ω)` points.
    ///
    /// # Errors
    /// [`Error::Invalid`] if the points do not determine the coefficients
    /// (repeated or non-positive resistances).
    pub fn fit(points: [(f64, f64); 3]) -> Result<Self, Error> {
        if points.iter().any(|&(_, r)| r.is_nan() || r <= 0.0) {
            return Err(Error::Invalid("resistances must be positive".into()));
        }
        // Solve the 3×3 system [1, L, L³]·[a b c]ᵀ = 1/T with Cramer's rule.
        let rows: Vec<[f64; 4]> = points
            .iter()
            .map(|&(t, r)| {
                let l = r.ln();
                [1.0, l, l * l * l, 1.0 / kelvin(t)]
            })
            .collect();
        let det3 = |m: [[f64; 3]; 3]| {
            m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
        };
        let col = |k: usize| -> [[f64; 3]; 3] {
            let mut m = [[0.0; 3]; 3];
            for (i, row) in rows.iter().enumerate() {
                for j in 0..3 {
                    m[i][j] = if j == k { row[3] } else { row[j] };
                }
            }
            m
        };
        let base = [
            [rows[0][0], rows[0][1], rows[0][2]],
            [rows[1][0], rows[1][1], rows[1][2]],
            [rows[2][0], rows[2][1], rows[2][2]],
        ];
        let d = det3(base);
        if d.abs() < 1e-30 {
            return Err(Error::Invalid(
                "the three points must have different resistances".into(),
            ));
        }
        Ok(Self {
            a: det3(col(0)) / d,
            b: det3(col(1)) / d,
            c: det3(col(2)) / d,
        })
    }

    /// Temperature in °C for `ohms`.
    pub fn temperature(&self, ohms: f64) -> f64 {
        let l = ohms.ln();
        1.0 / (self.a + self.b * l + self.c * l * l * l) - ZERO_C_IN_K
    }

    /// Resistance at `temp_c`, solving the cubic in `ln R` exactly (Cardano).
    pub fn resistance(&self, temp_c: f64) -> f64 {
        let y = 1.0 / kelvin(temp_c);
        if self.c == 0.0 {
            return ((y - self.a) / self.b).exp();
        }
        // ln R = x with x³ + p·x + q = 0.
        let p = self.b / self.c;
        let q = (self.a - y) / self.c;
        let disc = (q * q / 4.0 + p * p * p / 27.0).sqrt();
        let x = (-q / 2.0 + disc).cbrt() + (-q / 2.0 - disc).cbrt();
        x.exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_round_trip() {
        for t in [-40.0, 0.0, 25.0, 85.0, 150.0] {
            let r = beta_resistance(10_000.0, 25.0, 3950.0, t);
            assert!((beta_temperature(10_000.0, 25.0, 3950.0, r) - t).abs() < 1e-9);
        }
        assert!((beta_resistance(10_000.0, 25.0, 3950.0, 25.0) - 10_000.0).abs() < 1e-9);
        let b = beta_from_points(
            (25.0, 10_000.0),
            (85.0, beta_resistance(10_000.0, 25.0, 3950.0, 85.0)),
        );
        assert!((b - 3950.0).abs() < 1e-6);
    }

    #[test]
    fn steinhart_hart_fit_and_round_trip() {
        // Coefficients of a common 10 kΩ NTC.
        let truth = SteinhartHart {
            a: 1.009_249_522e-3,
            b: 2.378_405_444e-4,
            c: 2.019_202_697e-7,
        };
        let pts = [0.0, 25.0, 70.0].map(|t| (t, truth.resistance(t)));
        let fit = SteinhartHart::fit(pts).unwrap();
        for t in [-30.0, 0.0, 25.0, 50.0, 100.0] {
            let r = truth.resistance(t);
            assert!((fit.temperature(r) - t).abs() < 1e-6, "{t}");
            assert!((truth.temperature(r) - t).abs() < 1e-9, "{t}");
        }
    }
}
