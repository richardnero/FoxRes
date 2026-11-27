//! Temperature coefficient drift and thermal (Johnson–Nyquist) noise.

/// Boltzmann constant, J/K (exact since the 2019 SI redefinition).
pub const BOLTZMANN: f64 = 1.380_649e-23;

/// Kelvin offset from Celsius.
pub const ZERO_C_IN_K: f64 = 273.15;

/// Resistance at `temp_c` for a resistor of `r_ref` at `ref_c` with a linear
/// temperature coefficient of `ppm_per_k`.
pub fn at_temperature(r_ref: f64, ppm_per_k: f64, ref_c: f64, temp_c: f64) -> f64 {
    r_ref * (1.0 + ppm_per_k * 1e-6 * (temp_c - ref_c))
}

/// Thermal noise of a resistor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noise {
    /// RMS noise voltage over the bandwidth, in volts.
    pub volts_rms: f64,
    /// Noise density, in V/√Hz.
    pub density: f64,
    /// RMS noise current into a short, in amps.
    pub amps_rms: f64,
}

/// Johnson–Nyquist noise, `√(4·k·T·R·B)`, of `ohms` at `temp_c` over
/// `bandwidth_hz`.
pub fn johnson(ohms: f64, temp_c: f64, bandwidth_hz: f64) -> Noise {
    let t = temp_c + ZERO_C_IN_K;
    let density = (4.0 * BOLTZMANN * t * ohms).sqrt();
    let volts_rms = density * bandwidth_hz.sqrt();
    Noise {
        volts_rms,
        density,
        amps_rms: if ohms > 0.0 { volts_rms / ohms } else { 0.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift() {
        // 100 ppm/K over +75 K is +0.75 %.
        assert!((at_temperature(1000.0, 100.0, 25.0, 100.0) - 1007.5).abs() < 1e-9);
    }

    #[test]
    fn noise_of_1k_at_room_temperature() {
        // 1 kΩ at 25 °C: 4.06 nV/√Hz, the textbook value.
        let n = johnson(1000.0, 25.0, 1.0);
        assert!((n.density * 1e9 - 4.0578).abs() < 1e-3, "{}", n.density);
        // Over 20 kHz: 0.574 µV.
        assert!((johnson(1000.0, 25.0, 20_000.0).volts_rms * 1e6 - 0.5739).abs() < 1e-3);
    }
}
