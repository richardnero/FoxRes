//! Surface-mount resistor markings.

use crate::{
    ESeries, Error,
    value::{approx_eq, scale10},
};

/// How an SMD code was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notation {
    /// `0`, `00`, `000` or `0000`: zero-ohm link.
    Zero,
    /// Two digits and a multiplier digit, e.g. `472`.
    ThreeDigit,
    /// Three digits and a multiplier digit, e.g. `4701`.
    FourDigit,
    /// `R` marks the decimal point, e.g. `4R7` or `R47`.
    RNotation,
    /// EIA-96: E96 index and a multiplier letter, e.g. `01C`.
    Eia96,
}

impl Notation {
    /// Short description for display.
    pub fn label(self) -> &'static str {
        match self {
            Notation::Zero => "zero-ohm link",
            Notation::ThreeDigit => "3-digit code (±5 %)",
            Notation::FourDigit => "4-digit code (±1 %)",
            Notation::RNotation => "R decimal-point notation",
            Notation::Eia96 => "EIA-96 code (±1 %)",
        }
    }
}

/// A decoded SMD marking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smd {
    /// Resistance in ohms.
    pub ohms: f64,
    /// Notation the code used.
    pub notation: Notation,
}

/// EIA-96 multiplier letters and their powers of ten. `R` (= 10^-2) is used
/// by some makers but clashes with R notation, so codes like `10R` are read as
/// R notation. `S` and `H` are older alternatives for `X` and `B`.
const EIA_LETTERS: [(char, i32); 11] = [
    ('Z', -3),
    ('Y', -2),
    ('X', -1),
    ('S', -1),
    ('A', 0),
    ('B', 1),
    ('H', 1),
    ('C', 2),
    ('D', 3),
    ('E', 4),
    ('F', 5),
];

fn eia_exponent(letter: char) -> Option<i32> {
    EIA_LETTERS
        .iter()
        .find(|(c, _)| *c == letter)
        .map(|&(_, e)| e)
}

/// Decode an SMD resistor marking.
///
/// # Errors
/// Returns [`Error::Parse`] if the code is not a recognised marking.
pub fn decode(code: &str) -> Result<Smd, Error> {
    let code = code.trim().to_ascii_uppercase();
    let err = || Error::Parse(code.clone());
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());

    if !code.is_empty() && code.len() <= 4 && code.bytes().all(|b| b == b'0') {
        return Ok(Smd {
            ohms: 0.0,
            notation: Notation::Zero,
        });
    }

    if let Some(idx) = code.find('R') {
        let (head, tail) = (&code[..idx], &code[idx + 1..]);
        let ok = (head.is_empty() || digits(head)) && (tail.is_empty() || digits(tail));
        if !ok || (head.is_empty() && tail.is_empty()) || code.len() > 5 {
            return Err(err());
        }
        let head = if head.is_empty() { "0" } else { head };
        let ohms: f64 = format!("{head}.{tail}").parse().map_err(|_| err())?;
        let notation = if ohms == 0.0 {
            Notation::Zero
        } else {
            Notation::RNotation
        };
        return Ok(Smd { ohms, notation });
    }

    match code.len() {
        3 if digits(&code[..2]) && !digits(&code[2..]) => {
            let index: usize = code[..2].parse().map_err(|_| err())?;
            let letter = code[2..].chars().next().ok_or_else(err)?;
            let exp = eia_exponent(letter).ok_or_else(err)?;
            let mantissa = ESeries::E96
                .values()
                .get(index.wrapping_sub(1))
                .ok_or_else(err)?;
            Ok(Smd {
                ohms: scale10(f64::from(*mantissa), exp),
                notation: Notation::Eia96,
            })
        }
        3 | 4 if digits(&code) => {
            let (m, e) = code.split_at(code.len() - 1);
            let mantissa: f64 = m.parse().map_err(|_| err())?;
            let exp: i32 = e.parse().map_err(|_| err())?;
            let notation = if code.len() == 3 {
                Notation::ThreeDigit
            } else {
                Notation::FourDigit
            };
            Ok(Smd {
                ohms: scale10(mantissa, exp),
                notation,
            })
        }
        _ => Err(err()),
    }
}

/// Marking with `sig` significant digits (2 → 3-digit code, 3 → 4-digit code).
fn encode_digits(ohms: f64, sig: usize) -> Option<String> {
    if ohms == 0.0 {
        return Some("0".repeat(sig + 1));
    }
    let floor = 10f64.powi(sig as i32 - 1);
    let code = if ohms >= floor {
        (0..=9).find_map(|exp| {
            let m = scale10(ohms, -exp);
            let r = m.round();
            (approx_eq(m, r) && r >= floor && r < floor * 10.0)
                .then(|| format!("{}{exp}", r as u64))
        })?
    } else {
        let int_digits = if ohms >= 1.0 {
            ohms.log10().floor() as usize + 1
        } else {
            0
        };
        let decimals = sig - int_digits;
        let text = format!("{ohms:.decimals$}");
        let text = text.strip_prefix('0').unwrap_or(&text).replace('.', "R");
        if text.len() != sig + 1 {
            return None;
        }
        text
    };
    decode(&code)
        .ok()
        .filter(|d| approx_eq(d.ohms, ohms))
        .map(|_| code)
}

/// 3-digit marking (`472`, `4R7`), if `ohms` fits in two significant digits.
pub fn encode_three(ohms: f64) -> Option<String> {
    encode_digits(ohms, 2)
}

/// 4-digit marking (`4701`, `47R0`), if `ohms` fits in three significant digits.
pub fn encode_four(ohms: f64) -> Option<String> {
    encode_digits(ohms, 3)
}

/// EIA-96 marking (`01C`), if `ohms` is an E96 value in range.
pub fn encode_eia96(ohms: f64) -> Option<String> {
    let table = ESeries::E96.values();
    EIA_LETTERS
        .iter()
        .filter(|(c, _)| *c != 'S' && *c != 'H')
        .find_map(|&(letter, exp)| {
            let m = scale10(ohms, -exp);
            let r = m.round();
            if !approx_eq(m, r) {
                return None;
            }
            table
                .iter()
                .position(|&v| u64::from(v) == r as u64)
                .map(|i| format!("{:02}{letter}", i + 1))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ohms(code: &str) -> f64 {
        decode(code).unwrap().ohms
    }

    #[test]
    fn decodes() {
        assert!(approx_eq(ohms("472"), 4700.0));
        assert!(approx_eq(ohms("103"), 10_000.0));
        assert!(approx_eq(ohms("4701"), 4700.0));
        assert!(approx_eq(ohms("1002"), 10_000.0));
        assert!(approx_eq(ohms("4R7"), 4.7));
        assert!(approx_eq(ohms("R47"), 0.47));
        assert!(approx_eq(ohms("r010"), 0.01));
        assert!(approx_eq(ohms("10R0"), 10.0));
        assert!(approx_eq(ohms("01C"), 10_000.0));
        assert!(approx_eq(ohms("68X"), 49.9));
        assert!(approx_eq(ohms("96F"), 97_600_000.0));
        assert_eq!(decode("000").unwrap().notation, Notation::Zero);
        assert_eq!(decode("0R").unwrap().notation, Notation::Zero);
    }

    #[test]
    fn rejects() {
        for bad in ["", "R", "4R7R", "97C", "00C", "12Q", "47", "12345", "4.7"] {
            assert!(decode(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn encodes() {
        assert_eq!(encode_three(4700.0).as_deref(), Some("472"));
        assert_eq!(encode_three(10.0).as_deref(), Some("100"));
        assert_eq!(encode_three(4.7).as_deref(), Some("4R7"));
        assert_eq!(encode_three(0.47).as_deref(), Some("R47"));
        assert_eq!(encode_three(4750.0), None);
        assert_eq!(encode_four(4750.0).as_deref(), Some("4751"));
        assert_eq!(encode_four(47.0).as_deref(), Some("47R0"));
        assert_eq!(encode_eia96(10_000.0).as_deref(), Some("01C"));
        assert_eq!(encode_eia96(49.9).as_deref(), Some("68X"));
        assert_eq!(encode_eia96(4700.0), None);
    }
}
