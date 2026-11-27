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

/// SI prefixes used by [`format_si`], largest first.
const SI_PREFIXES: [(i32, &str); 9] = [
    (9, "G"),
    (6, "M"),
    (3, "k"),
    (0, ""),
    (-3, "m"),
    (-6, "µ"),
    (-9, "n"),
    (-12, "p"),
    (-15, "f"),
];

/// Format a quantity with an SI prefix and unit, e.g. `0.0123, "A"` → `"12.3 mA"`.
pub fn format_si(value: f64, unit: &str) -> String {
    if value == 0.0 {
        return format!("0 {unit}");
    }
    if !value.is_finite() {
        return format!("{value} {unit}");
    }
    let (exp, prefix) = SI_PREFIXES
        .iter()
        .find(|(e, _)| value.abs() >= scale10(1.0, *e) * (1.0 - 1e-12))
        .copied()
        .unwrap_or(SI_PREFIXES[SI_PREFIXES.len() - 1]);
    format!("{} {prefix}{unit}", format_sig(scale10(value, -exp), 4))
}

/// Parse a quantity with an optional SI prefix and unit: `12`, `12V`, `20m`,
/// `20 mA`, `4u7`, `1.5k`, `-40`. Any of `units` may follow the number.
/// Both `u` and `µ` mean micro; `m` is milli and `M` is mega.
///
/// # Errors
/// [`Error::Parse`] if the text is not a number with an optional prefix.
pub fn parse_si(input: &str, units: &[&str]) -> Result<f64, Error> {
    let err = || Error::Parse(input.trim().to_string());
    let mut s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    for unit in units {
        if let Some(stripped) = s.strip_suffix(unit) {
            s = stripped.to_string();
            break;
        }
    }
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (false, s.clone()),
    };
    if body.is_empty() {
        return Err(err());
    }
    let prefix = body.char_indices().find(|(_, c)| "pnuµmkKMG".contains(*c));
    let value = match prefix {
        None => number(&body).ok_or_else(err)?,
        Some((idx, letter)) => {
            let exp = match letter {
                'p' => -12,
                'n' => -9,
                'u' | 'µ' => -6,
                'm' => -3,
                'k' | 'K' => 3,
                'M' => 6,
                _ => 9,
            };
            let (head, tail) = (&body[..idx], &body[idx + letter.len_utf8()..]);
            let digits = if tail.is_empty() {
                if head.is_empty() {
                    return Err(err());
                }
                head.to_string()
            } else {
                if head.contains('.') || !tail.chars().all(|c| c.is_ascii_digit()) {
                    return Err(err());
                }
                format!("{}.{tail}", if head.is_empty() { "0" } else { head })
            };
            number(&digits).ok_or_else(err)?;
            number_with_exp(&digits, exp).ok_or_else(err)?
        }
    };
    Ok(if negative { -value } else { value })
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

    #[test]
    fn si_quantities() {
        let cases = [
            ("12", 12.0),
            ("12V", 12.0),
            ("20m", 0.02),
            ("20 mA", 0.02),
            ("4u7", 4.7e-6),
            ("4.7µ", 4.7e-6),
            ("1.5k", 1500.0),
            ("2M2", 2.2e6),
            ("-40", -40.0),
            ("100n", 1e-7),
        ];
        for (text, want) in cases {
            let got = parse_si(text, &["V", "A"]).unwrap();
            assert!(approx_eq(got, want), "{text}: {got}");
        }
        assert!(parse_si("abc", &[]).is_err());
        assert_eq!(format_si(0.0123, "A"), "12.3 mA");
        assert_eq!(format_si(4.06e-9, "V"), "4.06 nV");
        assert_eq!(format_si(1500.0, "W"), "1.5 kW");
    }
}
