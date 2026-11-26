//! Turns core results into the strings the window shows.

use foxres_core::{
    Color, ESeries,
    bands::{self, BandCount},
    combo::{self, Connection},
    eseries::error_percent,
    smd,
    value::{approx_eq, format_ohms, format_sig, parse_ohms},
};

/// Headline value and a detail line for a set of bands.
pub fn bands(colors: &[Color]) -> (String, String) {
    match bands::decode(colors) {
        Ok(r) if r.ohms == 0.0 => ("0 Ω".into(), "Zero-ohm link".into()),
        Ok(r) => {
            let mut detail = format!(
                "±{} %  ·  {} to {}",
                format_sig(r.tolerance, 3),
                format_ohms(r.min()),
                format_ohms(r.max())
            );
            if let Some(tc) = r.tempco {
                detail.push_str(&format!("  ·  {tc} ppm/K"));
            }
            if let Some(s) = smallest_series(r.ohms) {
                detail.push_str(&format!("  ·  {} value", s.name()));
            }
            (format_ohms(r.ohms), detail)
        }
        Err(e) => ("—".into(), e.to_string()),
    }
}

/// The coarsest E-series that contains `ohms` exactly.
pub fn smallest_series(ohms: f64) -> Option<ESeries> {
    ESeries::ALL
        .into_iter()
        .find(|s| s.nearest(ohms).is_ok_and(|v| approx_eq(v, ohms)))
}

/// Band colors for typed text, or an error message.
pub fn value_to_bands(
    text: &str,
    count: BandCount,
    tolerance: Color,
    tempco: Color,
) -> Result<Vec<Color>, String> {
    let ohms = parse_ohms(text).map_err(|e| e.to_string())?;
    bands::encode(ohms, count, tolerance, tempco).map_err(|e| match ESeries::E24.nearest(ohms) {
        Ok(near) if count.digits() == 2 => format!("{e}. Nearest E24 value: {}", format_ohms(near)),
        _ => e.to_string(),
    })
}

/// Decoded SMD code: (value, notation).
pub fn smd_code(code: &str) -> (String, String) {
    if code.trim().is_empty() {
        return (String::new(), "Try 472, 4701, 4R7 or 01C".into());
    }
    match smd::decode(code) {
        Ok(d) => (format_ohms(d.ohms), d.notation.label().into()),
        Err(e) => ("—".into(), e.to_string()),
    }
}

/// SMD markings for a typed value.
pub fn smd_for_value(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    match parse_ohms(text) {
        Ok(ohms) => {
            let show = |c: Option<String>| c.unwrap_or_else(|| "—".into());
            format!(
                "3-digit: {}    4-digit: {}    EIA-96: {}",
                show(smd::encode_three(ohms)),
                show(smd::encode_four(ohms)),
                show(smd::encode_eia96(ohms))
            )
        }
        Err(e) => e.to_string(),
    }
}

fn pct(x: f64) -> String {
    format!("{x:+.2} %")
}

/// Nearest value in a series: (headline, detail).
pub fn nearest(text: &str, series: ESeries) -> (String, String) {
    if text.trim().is_empty() {
        return (
            String::new(),
            format!("Typical tolerance ±{} %", format_sig(series.tolerance(), 2)),
        );
    }
    let result = parse_ohms(text).and_then(|t| Ok((t, series.nearest(t)?, series.neighbors(t)?)));
    match result {
        Ok((t, near, (lo, hi))) => (
            format!("{}  ({})", format_ohms(near), pct(error_percent(near, t))),
            format!(
                "Below: {} ({})    Above: {} ({})",
                format_ohms(lo),
                pct(error_percent(lo, t)),
                format_ohms(hi),
                pct(error_percent(hi, t))
            ),
        ),
        Err(e) => ("—".into(), e.to_string()),
    }
}

/// Best series and parallel pairs: (series line, parallel line).
pub fn pairs(text: &str, series: ESeries) -> (String, String) {
    if text.trim().is_empty() {
        return (String::new(), String::new());
    }
    let target = match parse_ohms(text) {
        Ok(t) => t,
        Err(e) => return (e.to_string(), String::new()),
    };
    let line = |c: Connection, sym: &str| match combo::best_pair(target, series, c) {
        Ok(p) => format!(
            "{} {sym} {} = {}  ({})",
            format_ohms(p.a),
            format_ohms(p.b),
            format_ohms(p.total),
            pct(p.error)
        ),
        Err(e) => e.to_string(),
    };
    (
        line(Connection::Series, "+"),
        line(Connection::Parallel, "∥"),
    )
}
