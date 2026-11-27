//! Wheatstone bridge.
//!
//! Left arm: `R1` (top) and `R2` (bottom). Right arm: `R3` (top) and `R4`
//! (bottom). The output is the right midpoint minus the left midpoint.

/// Output voltage for excitation `vex`.
pub fn output(vex: f64, r1: f64, r2: f64, r3: f64, r4: f64) -> f64 {
    vex * (r4 / (r3 + r4) - r2 / (r1 + r2))
}

/// `R4` that balances the bridge (zero output): `R4 = R3 · R2 / R1`.
pub fn balance_r4(r1: f64, r2: f64, r3: f64) -> f64 {
    r3 * r2 / r1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_bridge_reads_zero() {
        let r4 = balance_r4(1000.0, 2000.0, 330.0);
        assert!(output(10.0, 1000.0, 2000.0, 330.0, r4).abs() < 1e-12);
    }

    #[test]
    fn quarter_bridge_strain() {
        // 350 Ω bridge, one arm up 0.1 %: about Vex × ΔR / (4R).
        let v = output(10.0, 350.0, 350.0, 350.0, 350.35);
        assert!((v - 10.0 * 0.35 / (4.0 * 350.0)).abs() < 2e-5);
    }
}
