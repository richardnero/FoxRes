//! Series resistor for LEDs.

use crate::{ESeries, Error};

/// A sized LED resistor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedResistor {
    /// Exact resistance for the target current.
    pub ideal_ohms: f64,
    /// Next standard value at or above the ideal, so the current never exceeds the target.
    pub ohms: f64,
    /// Current with the standard value, in amps.
    pub amps: f64,
    /// Power in the resistor, in watts.
    pub resistor_watts: f64,
    /// Power in each LED, in watts.
    pub led_watts: f64,
}

/// Size the resistor for `count` LEDs in series with forward voltage
/// `forward_volts` each, at `amps`, from `supply_volts`.
///
/// # Errors
/// [`Error::Invalid`] if the LEDs need more voltage than the supply gives, or
/// an input is not positive.
pub fn size(
    supply_volts: f64,
    forward_volts: f64,
    count: u32,
    amps: f64,
    series: ESeries,
) -> Result<LedResistor, Error> {
    if !(supply_volts > 0.0 && forward_volts > 0.0 && amps > 0.0 && count > 0) {
        return Err(Error::Invalid(
            "supply, forward voltage, current and count must be positive".into(),
        ));
    }
    let drop = supply_volts - f64::from(count) * forward_volts;
    if drop <= 0.0 {
        return Err(Error::Invalid(format!(
            "{count} LED(s) need {:.3} V, more than the {supply_volts} V supply",
            f64::from(count) * forward_volts
        )));
    }
    let ideal = drop / amps;
    let (lo, hi) = series.neighbors(ideal)?;
    let ohms = if crate::value::approx_eq(lo, ideal) {
        lo
    } else {
        hi
    };
    let actual = drop / ohms;
    Ok(LedResistor {
        ideal_ohms: ideal,
        ohms,
        amps: actual,
        resistor_watts: actual * actual * ohms,
        led_watts: actual * forward_volts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn classic_red_led() {
        // 5 V, 2 V LED, 20 mA: 150 Ω exactly (an E6 value).
        let r = size(5.0, 2.0, 1, 0.020, ESeries::E24).unwrap();
        assert!(approx_eq(r.ohms, 150.0));
        assert!(approx_eq(r.amps, 0.020));
        assert!(approx_eq(r.resistor_watts, 0.06));
    }

    #[test]
    fn rounds_up_so_current_never_exceeds_target() {
        // 12 V, 3 × 3.1 V, 15 mA: ideal 180 Ω... 2.7 V / 0.015 A = 180 Ω exactly in E12.
        let r = size(12.0, 3.1, 3, 0.015, ESeries::E12).unwrap();
        assert!(approx_eq(r.ohms, 180.0));
        let r = size(12.0, 3.0, 3, 0.015, ESeries::E12).unwrap();
        // ideal 200 Ω -> 220 Ω in E12.
        assert!(approx_eq(r.ohms, 220.0));
        assert!(r.amps < 0.015);
    }

    #[test]
    fn rejects_too_many_leds() {
        assert!(size(5.0, 3.0, 2, 0.02, ESeries::E12).is_err());
    }
}
