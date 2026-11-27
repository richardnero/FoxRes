//! Platinum resistance thermometers (Pt100, Pt1000, …) to IEC 60751.
//!
//! The Callendar–Van Dusen equation with the standard coefficients:
//!
//! - −200 °C ≤ t < 0 °C: `R = R0 · (1 + A·t + B·t² + C·(t − 100)·t³)`
//! - 0 °C ≤ t ≤ 850 °C: `R = R0 · (1 + A·t + B·t²)`

use crate::Error;

pub const A: f64 = 3.9083e-3;
pub const B: f64 = -5.775e-7;
pub const C: f64 = -4.183e-12;
pub const MIN_C: f64 = -200.0;
pub const MAX_C: f64 = 850.0;

/// Mean temperature coefficient 0 to 100 °C, `(R100 − R0) / (100 · R0)`.
pub const ALPHA: f64 = 0.003_850_55;

/// Resistance at `temp_c` for a sensor with `r0` ohms at 0 °C.
///
/// # Errors
/// [`Error::OutOfRange`] outside −200 … 850 °C.
pub fn resistance(r0: f64, temp_c: f64) -> Result<f64, Error> {
    if !(MIN_C..=MAX_C).contains(&temp_c) {
        return Err(Error::OutOfRange(temp_c));
    }
    Ok(r0 * ratio(temp_c))
}

fn ratio(t: f64) -> f64 {
    let base = 1.0 + A * t + B * t * t;
    if t < 0.0 {
        base + C * (t - 100.0) * t * t * t
    } else {
        base
    }
}

/// Temperature for a measured `ohms` on a sensor with `r0` ohms at 0 °C.
///
/// Above 0 °C the quadratic is solved exactly; below, Newton's method on the
/// full equation converges to machine precision in a few steps.
///
/// # Errors
/// [`Error::OutOfRange`] if the resistance is outside the −200 … 850 °C span.
pub fn temperature(r0: f64, ohms: f64) -> Result<f64, Error> {
    let q = ohms / r0;
    if !(ratio(MIN_C)..=ratio(MAX_C)).contains(&q) {
        return Err(Error::OutOfRange(ohms));
    }
    // Quadratic solution, exact for t ≥ 0 and a good start below.
    let mut t = (-A + (A * A - 4.0 * B * (1.0 - q)).sqrt()) / (2.0 * B);
    if q < 1.0 {
        for _ in 0..50 {
            let f = ratio(t) - q;
            let df = A + 2.0 * B * t + C * (4.0 * t * t * t - 300.0 * t * t);
            let step = f / df;
            t -= step;
            if step.abs() < 1e-12 {
                break;
            }
        }
    }
    Ok(t)
}

/// IEC 60751 tolerance class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    AA,
    A,
    B,
    C,
}

/// Sensing element construction; it sets the range over which a class applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    WireWound,
    Film,
}

impl Class {
    pub const ALL: [Class; 4] = [Class::AA, Class::A, Class::B, Class::C];

    pub fn name(self) -> &'static str {
        match self {
            Class::AA => "AA",
            Class::A => "A",
            Class::B => "B",
            Class::C => "C",
        }
    }

    /// Temperature tolerance `±(a + b·|t|)` in kelvin.
    pub fn tolerance_k(self, temp_c: f64) -> f64 {
        let (a, b) = match self {
            Class::AA => (0.10, 0.0017),
            Class::A => (0.15, 0.002),
            Class::B => (0.30, 0.005),
            Class::C => (0.60, 0.010),
        };
        a + b * temp_c.abs()
    }

    /// Temperature range in °C over which the class is defined.
    pub fn valid_range(self, element: Element) -> (f64, f64) {
        match (self, element) {
            (Class::AA, Element::WireWound) => (-50.0, 250.0),
            (Class::AA, Element::Film) => (0.0, 150.0),
            (Class::A, Element::WireWound) => (-100.0, 450.0),
            (Class::A, Element::Film) => (-30.0, 300.0),
            (Class::B | Class::C, Element::WireWound) => (-196.0, 600.0),
            (Class::B, Element::Film) => (-50.0, 500.0),
            (Class::C, Element::Film) => (-50.0, 600.0),
        }
    }
}

/// Sensitivity dR/dt in Ω/K at `temp_c`.
pub fn sensitivity(r0: f64, temp_c: f64) -> f64 {
    let t = temp_c;
    let d = A
        + 2.0 * B * t
        + if t < 0.0 {
            C * (4.0 * t * t * t - 300.0 * t * t)
        } else {
            0.0
        };
    r0 * d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_matches_coefficients() {
        let r100 = resistance(100.0, 100.0).unwrap();
        assert!(((r100 - 100.0) / 10_000.0 - ALPHA).abs() < 1e-9);
    }

    #[test]
    fn round_trip_every_tenth_degree() {
        for k in -2000..=8500 {
            let t = f64::from(k) / 10.0;
            for r0 in [100.0, 500.0, 1000.0] {
                let r = resistance(r0, t).unwrap();
                let back = temperature(r0, r).unwrap();
                assert!((back - t).abs() < 1e-9, "{r0} {t}: {back}");
            }
        }
    }

    #[test]
    fn class_tolerances() {
        assert!((Class::B.tolerance_k(100.0) - 0.8).abs() < 1e-12);
        assert!((Class::A.tolerance_k(-100.0) - 0.35).abs() < 1e-12);
    }
}
