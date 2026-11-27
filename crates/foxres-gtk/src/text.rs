//! Turns core results into the strings the window shows.

use std::fmt::Write;

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
                let _ = write!(detail, "  ·  {tc} ppm/K");
            }
            if let Some(s) = smallest_series(r.ohms) {
                let _ = write!(detail, "  ·  {} value", s.name());
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

/// SMD markings for a typed value: 3-digit, 4-digit and EIA-96.
pub fn smd_for_value(text: &str) -> Result<[String; 3], String> {
    if text.trim().is_empty() {
        return Ok(Default::default());
    }
    let ohms = parse_ohms(text).map_err(|e| e.to_string())?;
    let show = |c: Option<String>| c.unwrap_or_else(|| "not available".into());
    Ok([
        show(smd::encode_three(ohms)),
        show(smd::encode_four(ohms)),
        show(smd::encode_eia96(ohms)),
    ])
}

/// Signed percentage with a true minus sign.
pub fn pct(x: f64) -> String {
    let s = format!("{x:+.2} %");
    s.replacen('-', "\u{2212}", 1)
}

/// Nearest value in a series.
pub struct Nearest {
    pub value: String,
    pub detail: String,
    pub below: String,
    pub above: String,
}

/// Nearest value in a series, plus the values either side.
pub fn nearest(text: &str, series: ESeries) -> Nearest {
    let tolerance = format!(
        "{} parts are typically ±{} %",
        series.name(),
        format_sig(series.tolerance(), 2)
    );
    let empty = |detail: String| Nearest {
        value: String::new(),
        detail,
        below: String::new(),
        above: String::new(),
    };
    if text.trim().is_empty() {
        return empty(tolerance);
    }
    let result = parse_ohms(text).and_then(|t| Ok((t, series.nearest(t)?, series.neighbors(t)?)));
    match result {
        Ok((t, near, (lo, hi))) => Nearest {
            value: format_ohms(near),
            detail: format!(
                "{} from {}  ·  {tolerance}",
                pct(error_percent(near, t)),
                format_ohms(t)
            ),
            below: format!("{}  ({})", format_ohms(lo), pct(error_percent(lo, t))),
            above: format!("{}  ({})", format_ohms(hi), pct(error_percent(hi, t))),
        },
        Err(e) => empty(e.to_string()),
    }
}

/// Best series pair, parallel pair and three-resistor combination.
pub fn pairs(text: &str, series: ESeries) -> (String, String, String) {
    if text.trim().is_empty() {
        return (String::new(), String::new(), String::new());
    }
    let target = match parse_ohms(text) {
        Ok(t) => t,
        Err(e) => return (e.to_string(), String::new(), String::new()),
    };
    let pair = |c: Connection| combo::best_pair(target, series, c);
    let line = |p: &combo::Pair, sym: &str| {
        format!(
            "{} {sym} {} = {}  ({})",
            format_ohms(p.a),
            format_ohms(p.b),
            format_ohms(p.total),
            pct(p.error)
        )
    };
    let (s, p) = match (pair(Connection::Series), pair(Connection::Parallel)) {
        (Ok(s), Ok(p)) => (s, p),
        (Err(e), _) | (_, Err(e)) => return (e.to_string(), String::new(), String::new()),
    };
    let best_pair = s.error.abs().min(p.error.abs());
    // Three resistors are only worth suggesting when they beat the best pair.
    let triple = match foxres_core::network::best_triple(target, series) {
        Ok(t) if t.error.abs() < best_pair - 1e-9 => format!(
            "{} = {}  ({})",
            t.topology
                .describe(&format_ohms(t.a), &format_ohms(t.b), &format_ohms(t.c)),
            format_ohms(t.total),
            pct(t.error)
        ),
        Ok(_) => "Not needed: a pair is already as close".to_string(),
        Err(e) => e.to_string(),
    };
    (line(&s, "+"), line(&p, "∥"), triple)
}
