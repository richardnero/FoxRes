//! Decode and encode through-hole resistor color bands.

use crate::{Color, Error, value::approx_eq};

/// A decoded resistor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resistor {
    /// Nominal resistance in ohms.
    pub ohms: f64,
    /// Tolerance in percent.
    pub tolerance: f64,
    /// Temperature coefficient in ppm/K, for 6-band resistors.
    pub tempco: Option<u32>,
}

impl Resistor {
    /// Lowest value allowed by the tolerance.
    pub fn min(&self) -> f64 {
        self.ohms * (1.0 - self.tolerance / 100.0)
    }

    /// Highest value allowed by the tolerance.
    pub fn max(&self) -> f64 {
        self.ohms * (1.0 + self.tolerance / 100.0)
    }
}

/// What a band position means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Digit,
    Multiplier,
    Tolerance,
    Tempco,
}

impl Role {
    /// Short label for display.
    pub fn label(self) -> &'static str {
        match self {
            Role::Digit => "digit",
            Role::Multiplier => "multiplier",
            Role::Tolerance => "tolerance",
            Role::Tempco => "temperature coefficient",
        }
    }

    /// Whether `color` has a meaning in this position.
    pub fn allows(self, color: Color) -> bool {
        match self {
            Role::Digit => color.digit().is_some(),
            Role::Multiplier => color.multiplier_exp().is_some(),
            Role::Tolerance => color.tolerance().is_some(),
            Role::Tempco => color.tempco().is_some(),
        }
    }

    /// Colors allowed in this position, in display order.
    pub fn choices(self) -> Vec<Color> {
        Color::ALL.into_iter().filter(|&c| self.allows(c)).collect()
    }
}

/// Number of bands on the resistor body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandCount {
    /// Single black band: zero-ohm link.
    One,
    /// Two digits and a multiplier, ±20 %.
    Three,
    /// Two digits, multiplier, tolerance.
    Four,
    /// Three digits, multiplier, tolerance.
    Five,
    /// Three digits, multiplier, tolerance, temperature coefficient.
    Six,
}

impl BandCount {
    /// Band counts offered in the user interface.
    pub const CHOICES: [BandCount; 4] = [
        BandCount::Three,
        BandCount::Four,
        BandCount::Five,
        BandCount::Six,
    ];

    /// Band count for a given number of colors.
    ///
    /// # Errors
    /// Returns [`Error::BandCount`] for anything but 1, 3, 4, 5 or 6.
    pub fn from_len(n: usize) -> Result<Self, Error> {
        match n {
            1 => Ok(BandCount::One),
            3 => Ok(BandCount::Three),
            4 => Ok(BandCount::Four),
            5 => Ok(BandCount::Five),
            6 => Ok(BandCount::Six),
            _ => Err(Error::BandCount(n)),
        }
    }

    /// Number of bands.
    pub fn len(self) -> usize {
        self.roles().len()
    }

    /// Always false; present to satisfy the `len` convention.
    pub fn is_empty(self) -> bool {
        false
    }

    /// Number of significant digits.
    pub fn digits(self) -> usize {
        match self {
            BandCount::One => 0,
            BandCount::Three | BandCount::Four => 2,
            BandCount::Five | BandCount::Six => 3,
        }
    }

    /// Meaning of each band position, left to right.
    pub fn roles(self) -> &'static [Role] {
        use Role::{Digit as D, Multiplier as M, Tempco as C, Tolerance as T};
        match self {
            BandCount::One => &[D],
            BandCount::Three => &[D, D, M],
            BandCount::Four => &[D, D, M, T],
            BandCount::Five => &[D, D, D, M, T],
            BandCount::Six => &[D, D, D, M, T, C],
        }
    }
}

fn pow10(exp: i32) -> f64 {
    10f64.powi(exp.abs())
}

fn apply_exp(mantissa: f64, exp: i32) -> f64 {
    if exp < 0 {
        mantissa / pow10(exp)
    } else {
        mantissa * pow10(exp)
    }
}

/// Decode a list of band colors, read left to right.
///
/// # Errors
/// Returns an error if the number of bands is invalid or a color has no
/// meaning in its position.
pub fn decode(colors: &[Color]) -> Result<Resistor, Error> {
    let count = BandCount::from_len(colors.len())?;
    if count == BandCount::One {
        return if colors[0] == Color::Black {
            Ok(Resistor {
                ohms: 0.0,
                tolerance: 0.0,
                tempco: None,
            })
        } else {
            Err(Error::InvalidBand {
                position: 0,
                color: colors[0],
                role: "zero-ohm",
            })
        };
    }

    let mut mantissa: u32 = 0;
    let mut exp = 0;
    let mut tolerance = 20.0;
    let mut tempco = None;
    for (position, (&color, &role)) in colors.iter().zip(count.roles()).enumerate() {
        let invalid = || Error::InvalidBand {
            position,
            color,
            role: role.label(),
        };
        match role {
            Role::Digit => mantissa = mantissa * 10 + u32::from(color.digit().ok_or_else(invalid)?),
            Role::Multiplier => exp = color.multiplier_exp().ok_or_else(invalid)?,
            Role::Tolerance => tolerance = color.tolerance().ok_or_else(invalid)?,
            Role::Tempco => tempco = Some(color.tempco().ok_or_else(invalid)?),
        }
    }
    Ok(Resistor {
        ohms: apply_exp(f64::from(mantissa), exp),
        tolerance,
        tempco,
    })
}

/// Digit and multiplier colors for `ohms`, without tolerance or tempco bands.
///
/// A value of zero returns a single black band for any band count.
///
/// # Errors
/// Returns [`Error::NotRepresentable`] if `ohms` needs more significant digits
/// than the band count has, or [`Error::OutOfRange`] if it is negative, not
/// finite, or too large.
///
/// # Panics
/// Never: digits and exponents are range-checked before colors are looked up.
pub fn encode_value(ohms: f64, count: BandCount) -> Result<Vec<Color>, Error> {
    if !ohms.is_finite() || ohms < 0.0 {
        return Err(Error::OutOfRange(ohms));
    }
    if ohms == 0.0 {
        return Ok(vec![Color::Black]);
    }
    let n = count.digits();
    if n == 0 {
        return Err(Error::NotRepresentable(ohms));
    }
    let limit = 10f64.powi(n as i32);
    if ohms >= limit * 1e9 {
        return Err(Error::OutOfRange(ohms));
    }

    let exact = |exp: i32| -> Option<u32> {
        let m = apply_exp(ohms, -exp);
        let r = m.round();
        (approx_eq(m, r) && r >= 1.0 && r < limit).then_some(r as u32)
    };
    // Prefer a non-zero leading digit, then allow a leading zero (e.g. 0.001 Ω).
    let found = (-3..=9)
        .find_map(|e| {
            exact(e)
                .filter(|&m| f64::from(m) >= limit / 10.0)
                .map(|m| (m, e))
        })
        .or_else(|| (-3..=9).find_map(|e| exact(e).map(|m| (m, e))));
    let (mantissa, exp) = found.ok_or(Error::NotRepresentable(ohms))?;

    let mut bands: Vec<Color> = format!("{mantissa:0n$}")
        .bytes()
        .map(|b| Color::from_digit(b - b'0').expect("ascii digit"))
        .collect();
    bands.push(Color::from_multiplier_exp(exp).expect("exponent in range"));
    Ok(bands)
}

/// Full band list for `ohms`.
///
/// `tolerance` is used for 4, 5 and 6 bands; `tempco` only for 6 bands.
///
/// # Errors
/// As [`encode_value`], plus [`Error::InvalidBand`] if `tolerance` or
/// `tempco` is not a valid color for that position.
pub fn encode(
    ohms: f64,
    count: BandCount,
    tolerance: Color,
    tempco: Color,
) -> Result<Vec<Color>, Error> {
    let mut bands = encode_value(ohms, count)?;
    if bands.len() == 1 {
        return Ok(bands);
    }
    let roles = count.roles();
    for (role, color) in [(Role::Tolerance, tolerance), (Role::Tempco, tempco)] {
        if roles.contains(&role) {
            if !role.allows(color) {
                return Err(Error::InvalidBand {
                    position: bands.len(),
                    color,
                    role: role.label(),
                });
            }
            bands.push(color);
        }
    }
    Ok(bands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Color::*;

    fn ohms(colors: &[Color]) -> f64 {
        decode(colors).unwrap().ohms
    }

    #[test]
    fn decodes_common_values() {
        assert!(approx_eq(ohms(&[Yellow, Violet, Red, Gold]), 4700.0));
        assert!(approx_eq(ohms(&[Brown, Black, Orange]), 10_000.0));
        assert!(approx_eq(
            ohms(&[Brown, Black, Black, Brown, Brown]),
            1000.0
        ));
        assert!(approx_eq(ohms(&[Yellow, Violet, Silver, Gold]), 0.47));
        assert!(approx_eq(ohms(&[Brown, Black, Pink, Gold]), 0.01));
        assert!(approx_eq(ohms(&[Black]), 0.0));
    }

    #[test]
    fn decodes_tolerance_and_tempco() {
        let r = decode(&[Red, Red, Black, Brown, Brown, Red]).unwrap();
        assert!(approx_eq(r.ohms, 2200.0));
        assert_eq!(r.tolerance, 1.0);
        assert_eq!(r.tempco, Some(50));
        assert_eq!(decode(&[Red, Red, Red]).unwrap().tolerance, 20.0);
        assert!(approx_eq(
            decode(&[Brown, Black, Red, Gold]).unwrap().max(),
            1050.0
        ));
    }

    #[test]
    fn rejects_bad_bands() {
        assert!(matches!(
            decode(&[Gold, Black, Red, Gold]),
            Err(Error::InvalidBand { position: 0, .. })
        ));
        assert!(matches!(
            decode(&[Red, Red, Red, Black]),
            Err(Error::InvalidBand { position: 3, .. })
        ));
        assert!(matches!(decode(&[Red, Red]), Err(Error::BandCount(2))));
        assert!(decode(&[Red]).is_err());
    }

    #[test]
    fn encodes() {
        assert_eq!(
            encode_value(4700.0, BandCount::Four).unwrap(),
            vec![Yellow, Violet, Red]
        );
        assert_eq!(
            encode_value(0.47, BandCount::Four).unwrap(),
            vec![Yellow, Violet, Silver]
        );
        assert_eq!(
            encode_value(1.0, BandCount::Four).unwrap(),
            vec![Brown, Black, Gold]
        );
        assert_eq!(
            encode_value(1000.0, BandCount::Five).unwrap(),
            vec![Brown, Black, Black, Brown]
        );
        assert_eq!(
            encode_value(0.001, BandCount::Four).unwrap(),
            vec![Black, Brown, Pink]
        );
        assert_eq!(encode_value(0.0, BandCount::Four).unwrap(), vec![Black]);
        assert!(matches!(
            encode_value(4750.0, BandCount::Four),
            Err(Error::NotRepresentable(_))
        ));
        assert_eq!(
            encode_value(4750.0, BandCount::Five).unwrap(),
            vec![Yellow, Violet, Green, Brown]
        );
        assert_eq!(
            encode(2200.0, BandCount::Six, Brown, Red).unwrap(),
            vec![Red, Red, Black, Brown, Brown, Red]
        );
        assert!(encode(2200.0, BandCount::Four, Black, Red).is_err());
    }

    #[test]
    fn round_trips_every_e24_value() {
        for decade in -2..=8 {
            for &base in crate::ESeries::E24.values() {
                let v = f64::from(base) * 10f64.powi(decade) / 10.0;
                let bands = encode(v, BandCount::Four, Gold, Black).unwrap();
                assert!(approx_eq(decode(&bands).unwrap().ohms, v), "{v}");
            }
        }
    }
}
