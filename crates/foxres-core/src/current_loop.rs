//! 4–20 mA current loops: scaling, NAMUR NE43 signal status, loop budget,
//! sense resistors and HART.

use crate::Error;

pub const LOW_MA: f64 = 4.0;
pub const HIGH_MA: f64 = 20.0;
/// HART needs this much loop resistance, minimum and maximum, in ohms.
pub const HART_MIN_OHMS: f64 = 230.0;
pub const HART_MAX_OHMS: f64 = 1100.0;
/// Sense resistor that turns 4–20 mA into 1–5 V.
pub const HART_STANDARD_OHMS: f64 = 250.0;

/// Loop current for `percent` of span (0 % = 4 mA, 100 % = 20 mA).
pub fn ma_from_percent(percent: f64) -> f64 {
    LOW_MA + (HIGH_MA - LOW_MA) * percent / 100.0
}

/// Percent of span for a loop current.
pub fn percent_from_ma(ma: f64) -> f64 {
    (ma - LOW_MA) / (HIGH_MA - LOW_MA) * 100.0
}

/// Engineering value for `ma` on a range `lrv` (4 mA) to `urv` (20 mA).
pub fn value_from_ma(ma: f64, lrv: f64, urv: f64) -> f64 {
    lrv + (urv - lrv) * percent_from_ma(ma) / 100.0
}

/// Loop current for an engineering value on `lrv` … `urv`.
///
/// # Errors
/// [`Error::Invalid`] if the range is empty.
pub fn ma_from_value(value: f64, lrv: f64, urv: f64) -> Result<f64, Error> {
    if (urv - lrv).abs() < f64::EPSILON * lrv.abs().max(urv.abs()).max(1.0) {
        return Err(Error::Invalid("the range must not be empty".into()));
    }
    Ok(ma_from_percent((value - lrv) / (urv - lrv) * 100.0))
}

/// Signal status to NAMUR NE43.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// ≤ 3.6 mA: device failure (low).
    FailureLow,
    /// 3.6 – 3.8 mA: outside the measuring range, not a valid failure signal.
    BelowRange,
    /// 3.8 – 20.5 mA: valid measurement (including the 3.8–4 and 20–20.5 mA saturation zones).
    Measurement,
    /// 20.5 – 21 mA: outside the measuring range, not a valid failure signal.
    AboveRange,
    /// ≥ 21 mA: device failure (high).
    FailureHigh,
}

impl Status {
    pub fn of(ma: f64) -> Self {
        if ma <= 3.6 {
            Status::FailureLow
        } else if ma < 3.8 {
            Status::BelowRange
        } else if ma <= 20.5 {
            Status::Measurement
        } else if ma < 21.0 {
            Status::AboveRange
        } else {
            Status::FailureHigh
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::FailureLow => "NE43 failure (≤ 3.6 mA)",
            Status::BelowRange => "Below measuring range (3.6–3.8 mA)",
            Status::Measurement => "Valid measurement (3.8–20.5 mA)",
            Status::AboveRange => "Above measuring range (20.5–21 mA)",
            Status::FailureHigh => "NE43 failure (≥ 21 mA)",
        }
    }
}

/// Loop voltage budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    /// Largest total loop resistance (sense resistor, barriers, wiring) that
    /// still leaves the transmitter its minimum voltage at `max_ma`.
    pub max_loop_ohms: f64,
    /// Voltage left for the loop at `max_ma` with the given loop resistance.
    pub transmitter_volts: f64,
    /// True when the transmitter gets at least its minimum voltage.
    pub ok: bool,
    /// True when the loop resistance is within the HART window (230–1100 Ω).
    pub hart_ok: bool,
}

/// Budget for a loop powered from `supply_volts` with total resistance
/// `loop_ohms`, a transmitter needing `min_transmitter_volts`, at `max_ma`
/// (use 20 mA, or 21–22 mA to cover NE43 failure signalling).
pub fn budget(
    supply_volts: f64,
    min_transmitter_volts: f64,
    loop_ohms: f64,
    max_ma: f64,
) -> Budget {
    let i = max_ma / 1000.0;
    let transmitter_volts = supply_volts - i * loop_ohms;
    Budget {
        max_loop_ohms: ((supply_volts - min_transmitter_volts) / i).max(0.0),
        transmitter_volts,
        ok: transmitter_volts >= min_transmitter_volts,
        hart_ok: (HART_MIN_OHMS..=HART_MAX_OHMS).contains(&loop_ohms),
    }
}

/// Voltage across a sense resistor at `ma`.
pub fn sense_volts(ohms: f64, ma: f64) -> f64 {
    ohms * ma / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling() {
        assert_eq!(ma_from_percent(0.0), 4.0);
        assert_eq!(ma_from_percent(50.0), 12.0);
        assert_eq!(ma_from_percent(100.0), 20.0);
        assert!((value_from_ma(12.0, 0.0, 150.0) - 75.0).abs() < 1e-12);
        assert!((ma_from_value(75.0, 0.0, 150.0).unwrap() - 12.0).abs() < 1e-12);
        assert!((value_from_ma(8.0, -50.0, 150.0) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn ne43() {
        assert_eq!(Status::of(3.5), Status::FailureLow);
        assert_eq!(Status::of(3.6), Status::FailureLow);
        assert_eq!(Status::of(3.7), Status::BelowRange);
        assert_eq!(Status::of(3.8), Status::Measurement);
        assert_eq!(Status::of(20.5), Status::Measurement);
        assert_eq!(Status::of(20.8), Status::AboveRange);
        assert_eq!(Status::of(21.0), Status::FailureHigh);
    }

    #[test]
    fn loop_budget() {
        // 24 V, 12 V transmitter, 20 mA: 600 Ω maximum.
        let b = budget(24.0, 12.0, 250.0, 20.0);
        assert!((b.max_loop_ohms - 600.0).abs() < 1e-9);
        assert!(b.ok && b.hart_ok);
        assert!((b.transmitter_volts - 19.0).abs() < 1e-12);
        assert!((sense_volts(250.0, 20.0) - 5.0).abs() < 1e-12);
    }
}
