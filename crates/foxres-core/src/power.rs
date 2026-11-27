//! Resistor package ratings, temperature derating and voltage limits.
//!
//! Ratings are **typical** values for general-purpose thick-film chip
//! resistors and metal-film through-hole resistors. Individual manufacturers
//! differ, so the datasheet always wins; these are for first-pass sizing.

/// Mounting style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mount {
    Smd,
    ThroughHole,
}

/// A resistor package with its typical ratings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Package {
    pub name: &'static str,
    pub mount: Mount,
    /// Rated power at or below 70 °C ambient, in watts.
    pub watts: f64,
    /// Maximum working voltage, in volts.
    pub max_volts: f64,
}

/// Typical packages, smallest first within each mounting style.
pub const PACKAGES: [Package; 13] = [
    Package {
        name: "0201",
        mount: Mount::Smd,
        watts: 0.05,
        max_volts: 25.0,
    },
    Package {
        name: "0402",
        mount: Mount::Smd,
        watts: 0.0625,
        max_volts: 50.0,
    },
    Package {
        name: "0603",
        mount: Mount::Smd,
        watts: 0.1,
        max_volts: 75.0,
    },
    Package {
        name: "0805",
        mount: Mount::Smd,
        watts: 0.125,
        max_volts: 150.0,
    },
    Package {
        name: "1206",
        mount: Mount::Smd,
        watts: 0.25,
        max_volts: 200.0,
    },
    Package {
        name: "1210",
        mount: Mount::Smd,
        watts: 0.5,
        max_volts: 200.0,
    },
    Package {
        name: "2010",
        mount: Mount::Smd,
        watts: 0.75,
        max_volts: 200.0,
    },
    Package {
        name: "2512",
        mount: Mount::Smd,
        watts: 1.0,
        max_volts: 200.0,
    },
    Package {
        name: "⅛ W axial",
        mount: Mount::ThroughHole,
        watts: 0.125,
        max_volts: 150.0,
    },
    Package {
        name: "¼ W axial",
        mount: Mount::ThroughHole,
        watts: 0.25,
        max_volts: 250.0,
    },
    Package {
        name: "½ W axial",
        mount: Mount::ThroughHole,
        watts: 0.5,
        max_volts: 350.0,
    },
    Package {
        name: "1 W axial",
        mount: Mount::ThroughHole,
        watts: 1.0,
        max_volts: 500.0,
    },
    Package {
        name: "2 W axial",
        mount: Mount::ThroughHole,
        watts: 2.0,
        max_volts: 500.0,
    },
];

/// Full rating up to this ambient temperature, in °C.
pub const DERATE_START_C: f64 = 70.0;
/// Zero allowed power at this ambient temperature, in °C.
pub const DERATE_END_C: f64 = 155.0;

/// Fraction of rated power allowed at `ambient_c`: 1 up to 70 °C, falling
/// linearly to 0 at 155 °C (the common IEC 60115 derating curve).
pub fn derating(ambient_c: f64) -> f64 {
    ((DERATE_END_C - ambient_c) / (DERATE_END_C - DERATE_START_C)).clamp(0.0, 1.0)
}

/// How a resistor is loaded in a given package.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Check {
    pub package: Package,
    /// Allowed power at the ambient temperature, in watts.
    pub allowed_watts: f64,
    /// Dissipated power as a fraction of the allowed power.
    pub utilisation: f64,
    /// Voltage across the resistor, in volts.
    pub volts: f64,
    /// Resistance above which the voltage limit, not power, is the constraint.
    pub critical_ohms: f64,
    /// True when both power (with `margin`) and voltage are within limits.
    pub ok: bool,
}

/// Check `watts` dissipated in `ohms` in `package` at `ambient_c`.
///
/// `margin` is the required headroom: 2.0 means use at most half the
/// allowed power, the usual design rule.
pub fn check(package: Package, ohms: f64, watts: f64, ambient_c: f64, margin: f64) -> Check {
    let allowed = package.watts * derating(ambient_c);
    let volts = (watts * ohms).sqrt();
    let utilisation = if allowed > 0.0 {
        watts / allowed
    } else {
        f64::INFINITY
    };
    Check {
        package,
        allowed_watts: allowed,
        utilisation,
        volts,
        critical_ohms: package.max_volts * package.max_volts / package.watts,
        ok: utilisation * margin <= 1.0 && volts <= package.max_volts,
    }
}

/// The smallest package of `mount` that passes [`check`].
pub fn recommend(
    mount: Mount,
    ohms: f64,
    watts: f64,
    ambient_c: f64,
    margin: f64,
) -> Option<Check> {
    PACKAGES
        .iter()
        .filter(|p| p.mount == mount)
        .map(|&p| check(p, ohms, watts, ambient_c, margin))
        .find(|c| c.ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derating_curve() {
        assert_eq!(derating(25.0), 1.0);
        assert_eq!(derating(70.0), 1.0);
        assert!((derating(112.5) - 0.5).abs() < 1e-12);
        assert_eq!(derating(155.0), 0.0);
        assert_eq!(derating(200.0), 0.0);
    }

    #[test]
    fn recommends_smallest_safe_package() {
        // 100 mW at 25 °C with 2× margin needs at least 200 mW: 1206 (250 mW).
        let c = recommend(Mount::Smd, 1000.0, 0.1, 25.0, 2.0).unwrap();
        assert_eq!(c.package.name, "1206");
        // 1 MΩ at 0.1 W is 316 V: past every SMD voltage rating.
        assert!(recommend(Mount::Smd, 1e6, 0.1, 25.0, 1.0).is_none());
        assert_eq!(
            recommend(Mount::ThroughHole, 1e6, 0.1, 25.0, 1.0)
                .unwrap()
                .package
                .name,
            "½ W axial"
        );
    }

    #[test]
    fn critical_resistance() {
        // 0805: 150 V, 0.125 W -> 180 kΩ.
        let c = check(PACKAGES[3], 1000.0, 0.01, 25.0, 1.0);
        assert!((c.critical_ohms - 180_000.0).abs() < 1e-6);
    }
}
