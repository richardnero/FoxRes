//! Parse and format resistance values.

use crate::Error;

const PREFIXES: [(i32, &str); 5] = [(9, "G"), (6, "M"), (3, "k"), (0, ""), (-3, "m")];

/// `mantissa × 10^exp`, correctly rounded.
///
/// Powers of ten up to 10^22 are exact in `f64`, and IEEE 754 multiplication
/// and division round correctly, so for an exact `mantissa` (any integer below
/// 2^53) the result is the `f64` nearest the true decimal value. Multiplying by
/// an inexact constant such as `0.001` would not be.
pub fn scale10(mantissa: f64, exp: i32) -> f64 {
    debug_assert!(exp.abs() <= 22, "10^{exp} is not exact in f64");
    let p = 10f64.powi(exp.abs());
    if exp < 0 { mantissa / p } else { mantissa * p }
}

/// Format a resistance with an SI prefix, e.g. `4700.0` → `"4.7 kΩ"`.
///
/// Shows up to four significant digits and drops trailing zeros.
pub fn format_ohms(ohms: f64) -> String {
    if ohms == 0.0 {
        return "0 Ω".to_string();
    }
    if !ohms.is_finite() {
        return "∞ Ω".to_string();
    }
    let (exp, prefix) = PREFIXES
        .iter()
        .find(|(e, _)| ohms.abs() >= scale10(1.0, *e) * (1.0 - 1e-12))
        .copied()
        .unwrap_or(PREFIXES[PREFIXES.len() - 1]);
    let scaled = scale10(ohms, -exp);
    format!("{} {prefix}Ω", format_sig(scaled, 4))
}

/// Format `x` with at most `sig` significant digits, trimming trailing zeros.
pub fn format_sig(x: f64, sig: i32) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    let magnitude = x.abs().log10().floor() as i32;
    let decimals = (sig - 1 - magnitude).clamp(0, 12) as usize;
    let s = format!("{x:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Parse a resistance written the way engineers write it.
///
/// Accepts `4700`, `4.7k`, `4k7`, `4.7 kΩ`, `100R`, `R47`, `2M2`, `2.2meg`,
/// `10m` (milliohms), `1G`, and an optional trailing `Ω` or `ohm(s)`.
/// `M` is mega and `m` is milli.
///
/// # Errors
/// Returns [`Error::Parse`] if the text is not a valid non-negative resistance.
pub fn parse_ohms(input: &str) -> Result<f64, Error> {
    let err = || Error::Parse(input.trim().to_string());
    let mut s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    for suffix in [
        "ohms", "ohm", "Ohms", "Ohm", "OHMS", "OHM", "\u{3a9}", "\u{2126}",
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
            break;
        }
    }
    let s = s
        .replace("meg", "M")
        .replace("MEG", "M")
        .replace("Meg", "M");
    if s.is_empty() {
        return Err(err());
    }

    let Some((idx, letter)) = s
        .char_indices()
        .find(|(_, c)| matches!(c, 'R' | 'r' | 'k' | 'K' | 'M' | 'm' | 'G' | 'g'))
    else {
        return number(&s).ok_or_else(err);
    };

    let exp = match letter {
        'R' | 'r' => 0,
        'k' | 'K' => 3,
        'M' => 6,
        'm' => -3,
        _ => 9,
    };
    let (head, tail) = (&s[..idx], &s[idx + letter.len_utf8()..]);
    let digits = if tail.is_empty() {
        if head.is_empty() {
            return Err(err());
        }
        head.to_string()
    } else {
        if head.contains('.') || !tail.chars().all(|c| c.is_ascii_digit()) {
            return Err(err());
        }
        let head = if head.is_empty() { "0" } else { head };
        format!("{head}.{tail}")
    };
    number(&digits).ok_or_else(err)?;
    // Let the decimal parser apply the exponent so the result is correctly
    // rounded: "10m" is exactly the f64 nearest 0.01, not 10 × 0.001.
    number_with_exp(&digits, exp).ok_or_else(err)
}

fn number_with_exp(digits: &str, exp: i32) -> Option<f64> {
    format!("{digits}e{exp}")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn number(s: &str) -> Option<f64> {
    if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// True when two resistances are equal to within one part per billion.
pub fn approx_eq(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_ohms(0.0), "0 Ω");
        assert_eq!(format_ohms(4700.0), "4.7 kΩ");
        assert_eq!(format_ohms(1_000_000.0), "1 MΩ");
        assert_eq!(format_ohms(0.47), "470 mΩ");
        assert_eq!(format_ohms(10.0), "10 Ω");
        assert_eq!(format_ohms(12_345.0), "12.35 kΩ");
        assert_eq!(format_ohms(2.2e9), "2.2 GΩ");
        assert_eq!(format_ohms(999.0), "999 Ω");
    }

    #[test]
    fn parses() {
        let cases = [
            ("4700", 4700.0),
            ("4.7k", 4700.0),
            ("4k7", 4700.0),
            ("4.7 kΩ", 4700.0),
            ("4K7", 4700.0),
            ("100R", 100.0),
            ("R47", 0.47),
            ("0R", 0.0),
            ("2M2", 2.2e6),
            ("2.2meg", 2.2e6),
            ("10m", 0.01),
            ("1G", 1e9),
            ("330 ohms", 330.0),
            ("1.5", 1.5),
        ];
        for (text, want) in cases {
            let got = parse_ohms(text).unwrap();
            assert!(approx_eq(got, want), "{text}: got {got}, want {want}");
        }
    }

    #[test]
    fn rejects() {
        for bad in ["", "k", "abc", "4.7k7", "-10", "4k7x", "1..2"] {
            assert!(parse_ohms(bad).is_err(), "{bad} should fail");
        }
    }
}
