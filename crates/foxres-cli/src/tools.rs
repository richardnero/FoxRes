//! Design and instrumentation commands.

use std::error::Error as StdError;

use clap::{Args, ValueEnum};
use foxres_core::{
    ESeries, bridge, current_loop, divider, led, network, ntc, ohm,
    power::{self, Mount},
    rtd, thermal, tolerance,
    value::{format_ohms, format_si, format_sig, parse_ohms, parse_si},
    wire,
};
use serde_json::json;

use crate::Output;

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

fn ohms(text: &str) -> Result<f64> {
    Ok(parse_ohms(text)?)
}

fn qty(text: &str, units: &[&str]) -> Result<f64> {
    Ok(parse_si(text, units)?)
}

fn opt(text: Option<&String>, units: &[&str]) -> Result<Option<f64>> {
    text.map(|t| qty(t, units)).transpose()
}

/// Lines of `label: value`, with the values aligned.
fn table(rows: &[(&str, String)]) -> String {
    let width = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    rows.iter()
        .map(|(l, v)| format!("{l}:{} {v}", " ".repeat(width - l.chars().count())))
        .collect::<Vec<_>>()
        .join("\n")
}

fn pct(x: f64) -> String {
    format!("{x:+.3} %")
}

// ----------------------------------------------------------------------------

/// Ohm's law: give any two of voltage, current, resistance and power.
#[derive(Debug, Args)]
pub struct OhmArgs {
    /// Voltage, e.g. 12 or 3.3.
    #[arg(short = 'V', long)]
    volts: Option<String>,
    /// Current, e.g. 20m or 0.02.
    #[arg(short = 'I', long)]
    amps: Option<String>,
    /// Resistance, e.g. 470 or 4k7.
    #[arg(short = 'R', long)]
    ohms: Option<String>,
    /// Power, e.g. 250m.
    #[arg(short = 'P', long)]
    watts: Option<String>,
}

pub fn ohm_law(a: &OhmArgs) -> Result<Output> {
    let c = ohm::solve(
        opt(a.volts.as_ref(), &["V"])?,
        opt(a.amps.as_ref(), &["A"])?,
        a.ohms.as_deref().map(ohms).transpose()?,
        opt(a.watts.as_ref(), &["W"])?,
    )?;
    Ok(Output {
        text: table(&[
            ("Voltage", format_si(c.volts, "V")),
            ("Current", format_si(c.amps, "A")),
            ("Resistance", format_ohms(c.ohms)),
            ("Power", format_si(c.watts, "W")),
        ]),
        json: json!({ "volts": c.volts, "amps": c.amps, "ohms": c.ohms, "watts": c.watts }),
    })
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MountArg {
    Smd,
    Tht,
    All,
}

/// Power and voltage check against typical package ratings.
#[derive(Debug, Args)]
pub struct PowerArgs {
    /// Resistance.
    ohms: String,
    /// Power dissipated.
    #[arg(short = 'P', long, conflicts_with_all = ["volts", "amps"])]
    watts: Option<String>,
    /// Voltage across the resistor.
    #[arg(short = 'V', long, conflicts_with = "amps")]
    volts: Option<String>,
    /// Current through the resistor.
    #[arg(short = 'I', long)]
    amps: Option<String>,
    /// Ambient temperature in °C.
    #[arg(long, default_value = "25", allow_hyphen_values = true)]
    ambient: String,
    /// Required power headroom (2 = use at most half the allowed power).
    #[arg(long, default_value_t = 2.0)]
    margin: f64,
    /// Which packages to list.
    #[arg(long, value_enum, default_value = "all")]
    mount: MountArg,
}

pub fn power_check(a: &PowerArgs) -> Result<Output> {
    let r = ohms(&a.ohms)?;
    let c = ohm::solve(
        opt(a.volts.as_ref(), &["V"])?,
        opt(a.amps.as_ref(), &["A"])?,
        Some(r),
        opt(a.watts.as_ref(), &["W"])?,
    )
    .map_err(|_| "give one of --watts, --volts or --amps")?;
    let ambient = qty(&a.ambient, &["°C", "C"])?;
    let mounts: &[Mount] = match a.mount {
        MountArg::Smd => &[Mount::Smd],
        MountArg::Tht => &[Mount::ThroughHole],
        MountArg::All => &[Mount::Smd, Mount::ThroughHole],
    };
    let mut lines = vec![format!(
        "{} dissipating {} ({} across it), {} °C ambient, {}× margin\n",
        format_ohms(r),
        format_si(c.watts, "W"),
        format_si(c.volts, "V"),
        format_sig(ambient, 4),
        format_sig(a.margin, 3)
    )];
    lines.push(format!(
        "{:<11} {:>9} {:>9} {:>6} {:>7}",
        "Package", "Allowed", "Load", "Volts", ""
    ));
    let mut rows = Vec::new();
    for p in power::PACKAGES.iter().filter(|p| mounts.contains(&p.mount)) {
        let k = power::check(*p, r, c.watts, ambient, a.margin);
        lines.push(format!(
            "{:<11} {:>9} {:>8.0}% {:>5}V {:>7}",
            p.name,
            format_si(k.allowed_watts, "W"),
            k.utilisation * 100.0,
            format_sig(p.max_volts, 3),
            if k.ok { "ok" } else { "too small" }
        ));
        rows.push(json!({ "package": p.name, "allowed_watts": k.allowed_watts, "utilisation": k.utilisation, "max_volts": p.max_volts, "ok": k.ok }));
    }
    let mut best = serde_json::Map::new();
    lines.push(String::new());
    for &m in mounts {
        let label = if m == Mount::Smd {
            "SMD"
        } else {
            "through-hole"
        };
        match power::recommend(m, r, c.watts, ambient, a.margin) {
            Some(k) => {
                lines.push(format!("Smallest {label}: {}", k.package.name));
                best.insert(label.into(), json!(k.package.name));
            }
            None => lines.push(format!("No {label} package is large enough")),
        }
    }
    lines.push(
        "Typical ratings, derated linearly from 70 °C to zero at 155 °C. Check the datasheet."
            .into(),
    );
    Ok(Output {
        text: lines.join("\n"),
        json: json!({ "ohms": r, "watts": c.watts, "volts": c.volts, "ambient_c": ambient, "packages": rows, "smallest": best }),
    })
}

// ----------------------------------------------------------------------------

/// Voltage divider: design one for a ratio, or analyse R1 and R2.
#[derive(Debug, Args)]
pub struct DividerArgs {
    /// Input voltage.
    #[arg(long)]
    vin: Option<String>,
    /// Wanted output voltage (with --vin), to design a divider.
    #[arg(long)]
    vout: Option<String>,
    /// Wanted Vout/Vin ratio, to design a divider.
    #[arg(long, conflicts_with = "vout")]
    ratio: Option<f64>,
    /// Top resistor, to analyse a divider.
    #[arg(long, requires = "r2")]
    r1: Option<String>,
    /// Bottom resistor, to analyse a divider.
    #[arg(long, requires = "r1")]
    r2: Option<String>,
    /// Load resistance across the output.
    #[arg(long)]
    load: Option<String>,
    /// E-series to design from.
    #[arg(short, long, default_value = "E24")]
    series: ESeries,
    /// Rough total R1 + R2 to aim for when designing.
    #[arg(long, default_value = "10k")]
    total: String,
    /// Resistor tolerance in percent, for the worst-case range.
    #[arg(short, long, default_value_t = 1.0)]
    tolerance: f64,
}

pub fn divider(a: &DividerArgs) -> Result<Output> {
    let vin = opt(a.vin.as_ref(), &["V"])?;
    let load = a.load.as_deref().map(ohms).transpose()?;
    let (r1, r2, mut rows, mut j) = if let (Some(r1), Some(r2)) = (&a.r1, &a.r2) {
        let (r1, r2) = (ohms(r1)?, ohms(r2)?);
        (
            r1,
            r2,
            vec![("R1", format_ohms(r1)), ("R2", format_ohms(r2))],
            json!({ "r1": r1, "r2": r2 }),
        )
    } else {
        let target = match (a.ratio, vin, opt(a.vout.as_ref(), &["V"])?) {
            (Some(r), _, _) => r,
            (None, Some(vi), Some(vo)) => vo / vi,
            _ => return Err("give --ratio, or --vin and --vout, or --r1 and --r2".into()),
        };
        let p = divider::best_pair(target, a.series, ohms(&a.total)?)?;
        (
            p.r1,
            p.r2,
            vec![
                ("Target ratio", format_sig(target, 6)),
                (
                    "Best pair",
                    format!(
                        "R1 = {}, R2 = {} ({})",
                        format_ohms(p.r1),
                        format_ohms(p.r2),
                        a.series.name()
                    ),
                ),
                ("Ratio error", pct(p.error)),
            ],
            json!({ "target_ratio": target, "series": a.series.name(), "r1": p.r1, "r2": p.r2, "error_percent": p.error }),
        )
    };
    let ratio = divider::ratio(r1, r2);
    let (lo, hi) = divider::ratio_range(r1, r2, a.tolerance, a.tolerance);
    rows.push(("Ratio", format_sig(ratio, 6)));
    rows.push((
        "Worst case",
        format!(
            "{} to {} (±{} % parts)",
            format_sig(lo, 6),
            format_sig(hi, 6),
            format_sig(a.tolerance, 3)
        ),
    ));
    j["ratio"] = json!(ratio);
    j["ratio_min"] = json!(lo);
    j["ratio_max"] = json!(hi);
    if let Some(v) = vin {
        let out = divider::output(v, r1, r2, None);
        rows.push((
            "Vout",
            format!(
                "{} (worst case {} to {})",
                format_si(out, "V"),
                format_si(v * lo, "V"),
                format_si(v * hi, "V")
            ),
        ));
        rows.push(("Divider current", format_si(v / (r1 + r2), "A")));
        rows.push((
            "Power",
            format!(
                "R1 {}, R2 {}",
                format_si((v - out).powi(2) / r1, "W"),
                format_si(out * out / r2, "W")
            ),
        ));
        j["vout"] = json!(out);
        if let Some(rl) = load {
            let loaded = divider::output(v, r1, r2, Some(rl));
            rows.push((
                "Vout loaded",
                format!(
                    "{} with {} load ({})",
                    format_si(loaded, "V"),
                    format_ohms(rl),
                    pct((loaded - out) / out * 100.0)
                ),
            ));
            j["vout_loaded"] = json!(loaded);
        }
    }
    Ok(Output {
        text: table(&rows),
        json: j,
    })
}

// ----------------------------------------------------------------------------

/// LED series resistor.
#[derive(Debug, Args)]
pub struct LedArgs {
    /// Supply voltage.
    #[arg(long)]
    supply: String,
    /// LED forward voltage.
    #[arg(long)]
    vf: String,
    /// LED current.
    #[arg(short = 'I', long, default_value = "20m")]
    current: String,
    /// LEDs in series.
    #[arg(short = 'n', long, default_value_t = 1)]
    count: u32,
    #[arg(short, long, default_value = "E24")]
    series: ESeries,
}

pub fn led(a: &LedArgs) -> Result<Output> {
    let r = led::size(
        qty(&a.supply, &["V"])?,
        qty(&a.vf, &["V"])?,
        a.count,
        qty(&a.current, &["A"])?,
        a.series,
    )?;
    Ok(Output {
        text: table(&[
            ("Exact resistance", format_ohms(r.ideal_ohms)),
            (
                "Standard value",
                format!(
                    "{} {} (next value up, so the current never exceeds the target)",
                    a.series.name(),
                    format_ohms(r.ohms)
                ),
            ),
            ("LED current", format_si(r.amps, "A")),
            (
                "Resistor power",
                format!(
                    "{} (use a part rated at least {})",
                    format_si(r.resistor_watts, "W"),
                    format_si(r.resistor_watts * 2.0, "W")
                ),
            ),
            ("Power per LED", format_si(r.led_watts, "W")),
        ]),
        json: json!({ "ideal_ohms": r.ideal_ohms, "ohms": r.ohms, "amps": r.amps, "resistor_watts": r.resistor_watts, "led_watts": r.led_watts }),
    })
}

// ----------------------------------------------------------------------------

/// Resistors to combine.
#[derive(Debug, Args)]
pub struct NetArgs {
    #[arg(required = true, num_args = 2.., value_name = "OHMS")]
    values: Vec<String>,
}

pub fn net(a: &NetArgs, parallel: bool) -> Result<Output> {
    let vals = a
        .values
        .iter()
        .map(|v| ohms(v))
        .collect::<Result<Vec<_>>>()?;
    let total = if parallel {
        network::parallel(&vals)
    } else {
        network::series(&vals)
    };
    let sym = if parallel { " ∥ " } else { " + " };
    Ok(Output {
        text: format!(
            "{} = {}",
            vals.iter()
                .map(|&v| format_ohms(v))
                .collect::<Vec<_>>()
                .join(sym),
            format_ohms(total)
        ),
        json: json!({ "values": vals, "ohms": total }),
    })
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CircuitArg {
    Series,
    Parallel,
    Divider,
}

/// Worst-case and RSS tolerance analysis.
#[derive(Debug, Args)]
pub struct ToleranceArgs {
    /// How the resistors are connected (divider takes R1 then R2).
    #[arg(value_enum)]
    circuit: CircuitArg,
    /// Resistors as VALUE:PERCENT, e.g. 10k:1 4k7:5 (default 5 %).
    #[arg(required = true, num_args = 1.., value_name = "OHMS[:PCT]")]
    parts: Vec<String>,
}

pub fn tolerance(a: &ToleranceArgs) -> Result<Output> {
    let mut vals = Vec::new();
    let mut tols = Vec::new();
    for p in &a.parts {
        let (v, t) = p.split_once(':').unwrap_or((p, "5"));
        vals.push(ohms(v)?);
        tols.push(qty(t, &["%"])?);
    }
    let s = match a.circuit {
        CircuitArg::Series => tolerance::analyse(&vals, &tols, network::series)?,
        CircuitArg::Parallel => tolerance::analyse(&vals, &tols, network::parallel)?,
        CircuitArg::Divider => {
            if vals.len() != 2 {
                return Err("a divider takes exactly R1 and R2".into());
            }
            tolerance::analyse(&vals, &tols, |v| divider::ratio(v[0], v[1]))?
        }
    };
    let show = |x: f64| match a.circuit {
        CircuitArg::Divider => format_sig(x, 6),
        _ => format_ohms(x),
    };
    let rel = |x: f64| pct((x - s.nominal) / s.nominal * 100.0);
    Ok(Output {
        text: table(&[
            ("Nominal", show(s.nominal)),
            (
                "Worst case",
                format!(
                    "{} ({}) to {} ({})",
                    show(s.min),
                    rel(s.min),
                    show(s.max),
                    rel(s.max)
                ),
            ),
            (
                "RSS",
                format!("± {} (± {:.3} %)", show(s.rss), s.rss / s.nominal * 100.0),
            ),
        ]),
        json: json!({ "nominal": s.nominal, "min": s.min, "max": s.max, "rss": s.rss }),
    })
}

// ----------------------------------------------------------------------------

/// Resistance change with temperature.
#[derive(Debug, Args)]
pub struct DriftArgs {
    /// Resistance at the reference temperature.
    ohms: String,
    /// Temperature coefficient in ppm/K.
    #[arg(long)]
    ppm: f64,
    /// Reference temperature in °C.
    #[arg(long, default_value = "25", allow_hyphen_values = true)]
    from: String,
    /// Temperature of interest in °C.
    #[arg(long, allow_hyphen_values = true)]
    to: String,
}

pub fn drift(a: &DriftArgs) -> Result<Output> {
    let r = ohms(&a.ohms)?;
    let (t0, t) = (qty(&a.from, &["°C", "C"])?, qty(&a.to, &["°C", "C"])?);
    let rt = thermal::at_temperature(r, a.ppm, t0, t);
    Ok(Output {
        text: format!(
            "{} at {} °C → {} at {} °C ({})",
            format_ohms(r),
            format_sig(t0, 4),
            format_ohms(rt),
            format_sig(t, 4),
            pct((rt - r) / r * 100.0)
        ),
        json: json!({ "ohms": rt, "change_percent": (rt - r) / r * 100.0 }),
    })
}

/// Johnson–Nyquist thermal noise.
#[derive(Debug, Args)]
pub struct NoiseArgs {
    ohms: String,
    /// Temperature in °C.
    #[arg(long, default_value = "25", allow_hyphen_values = true)]
    temp: String,
    /// Bandwidth in Hz, e.g. 20k.
    #[arg(short, long, default_value = "1")]
    bandwidth: String,
}

pub fn noise(a: &NoiseArgs) -> Result<Output> {
    let r = ohms(&a.ohms)?;
    let n = thermal::johnson(r, qty(&a.temp, &["°C", "C"])?, qty(&a.bandwidth, &["Hz"])?);
    Ok(Output {
        text: table(&[
            (
                "Noise density",
                format!("{}/√Hz", format_si(n.density, "V")),
            ),
            ("RMS voltage", format_si(n.volts_rms, "V")),
            ("RMS current", format_si(n.amps_rms, "A")),
        ]),
        json: json!({ "density_v_per_rthz": n.density, "volts_rms": n.volts_rms, "amps_rms": n.amps_rms }),
    })
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ElementArg {
    Wire,
    Film,
}

/// Platinum RTD (Pt100, Pt1000) to IEC 60751.
#[derive(Debug, Args)]
pub struct RtdArgs {
    /// Temperature in °C, to get resistance.
    #[arg(short, long, allow_hyphen_values = true, conflicts_with = "ohms")]
    temp: Option<String>,
    /// Measured resistance, to get temperature.
    #[arg(short = 'R', long)]
    ohms: Option<String>,
    /// Resistance at 0 °C (100 for Pt100, 1000 for Pt1000).
    #[arg(long, default_value = "100")]
    r0: String,
    /// Sensing element, for tolerance-class ranges.
    #[arg(long, value_enum, default_value = "wire")]
    element: ElementArg,
}

pub fn rtd_cmd(a: &RtdArgs) -> Result<Output> {
    let r0 = ohms(&a.r0)?;
    let (t, r) = match (&a.temp, &a.ohms) {
        (Some(t), _) => {
            let t = qty(t, &["°C", "C"])?;
            (t, rtd::resistance(r0, t)?)
        }
        (None, Some(r)) => {
            let r = ohms(r)?;
            (rtd::temperature(r0, r)?, r)
        }
        _ => return Err("give --temp or --ohms".into()),
    };
    let s = rtd::sensitivity(r0, t);
    let element = match a.element {
        ElementArg::Wire => rtd::Element::WireWound,
        ElementArg::Film => rtd::Element::Film,
    };
    let mut lines = vec![format!(
        "Pt{}: {:.3} °C ⇔ {:.4} Ω\nSensitivity: {:.4} Ω/K\n\nIEC 60751 tolerance at {:.1} °C ({}):",
        format_sig(r0, 6),
        t,
        r,
        s,
        t,
        if element == rtd::Element::Film {
            "film"
        } else {
            "wire-wound"
        }
    )];
    let mut classes = Vec::new();
    for c in rtd::Class::ALL {
        let (lo, hi) = c.valid_range(element);
        if (lo..=hi).contains(&t) {
            let k = c.tolerance_k(t);
            lines.push(format!(
                "  Class {:<2}  ±{:.3} K  (±{:.4} Ω)",
                c.name(),
                k,
                k * s
            ));
            classes.push(json!({ "class": c.name(), "tolerance_k": k, "tolerance_ohms": k * s }));
        } else {
            lines.push(format!(
                "  Class {:<2}  not defined at this temperature ({lo} to {hi} °C)",
                c.name()
            ));
        }
    }
    Ok(Output {
        text: lines.join("\n"),
        json: json!({ "r0": r0, "temp_c": t, "ohms": r, "ohms_per_k": s, "classes": classes }),
    })
}

/// NTC thermistor: Beta model, Steinhart–Hart, or fit from three points.
#[derive(Debug, Args)]
pub struct NtcArgs {
    /// Temperature in °C, to get resistance.
    #[arg(short, long, allow_hyphen_values = true, conflicts_with = "ohms")]
    temp: Option<String>,
    /// Measured resistance, to get temperature.
    #[arg(short = 'R', long)]
    ohms: Option<String>,
    /// Resistance at the reference temperature (Beta model).
    #[arg(long, default_value = "10k")]
    r0: String,
    /// Reference temperature in °C (Beta model).
    #[arg(long, default_value = "25")]
    t0: String,
    /// Beta constant in kelvin.
    #[arg(long, conflicts_with_all = ["sh", "fit"])]
    beta: Option<f64>,
    /// Steinhart–Hart coefficients A,B,C.
    #[arg(
        long,
        value_delimiter = ',',
        allow_hyphen_values = true,
        conflicts_with = "fit"
    )]
    sh: Option<Vec<f64>>,
    /// Fit Steinhart–Hart through three points, e.g. 0:32.65k,25:10k,50:3.6k.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    fit: Option<Vec<String>>,
}

pub fn ntc_cmd(a: &NtcArgs) -> Result<Output> {
    let model = if let Some(b) = a.beta {
        let (r0, t0) = (ohms(&a.r0)?, qty(&a.t0, &["°C", "C"])?);
        Model::Beta(r0, t0, b)
    } else if let Some(c) = &a.sh {
        if c.len() != 3 {
            return Err("--sh takes three coefficients: A,B,C".into());
        }
        Model::Sh(ntc::SteinhartHart {
            a: c[0],
            b: c[1],
            c: c[2],
        })
    } else if let Some(points) = &a.fit {
        if points.len() != 3 {
            return Err("--fit takes three TEMP:OHMS points".into());
        }
        let mut p = [(0.0, 0.0); 3];
        for (i, s) in points.iter().enumerate() {
            let (t, r) = s.split_once(':').ok_or("points are TEMP:OHMS")?;
            p[i] = (qty(t, &["°C", "C"])?, ohms(r)?);
        }
        Model::Sh(ntc::SteinhartHart::fit(p)?)
    } else {
        return Err("give --beta, --sh A,B,C or --fit with three points".into());
    };
    let mut lines = Vec::new();
    let mut j = json!({});
    if let Model::Sh(c) = model {
        lines.push(format!(
            "Steinhart–Hart: A = {:.10e}, B = {:.10e}, C = {:.10e}",
            c.a, c.b, c.c
        ));
        j["sh"] = json!([c.a, c.b, c.c]);
    }
    match (&a.temp, &a.ohms) {
        (Some(t), _) => {
            let t = qty(t, &["°C", "C"])?;
            let r = model.resistance(t);
            lines.push(format!("{:.3} °C → {}", t, format_ohms(r)));
            j["temp_c"] = json!(t);
            j["ohms"] = json!(r);
        }
        (None, Some(r)) => {
            let r = ohms(r)?;
            let t = model.temperature(r);
            lines.push(format!("{} → {:.3} °C", format_ohms(r), t));
            j["temp_c"] = json!(t);
            j["ohms"] = json!(r);
        }
        _ if lines.is_empty() => return Err("give --temp or --ohms".into()),
        _ => {}
    }
    Ok(Output {
        text: lines.join("\n"),
        json: j,
    })
}

#[derive(Clone, Copy)]
enum Model {
    Beta(f64, f64, f64),
    Sh(ntc::SteinhartHart),
}

impl Model {
    fn resistance(self, t: f64) -> f64 {
        match self {
            Model::Beta(r0, t0, b) => ntc::beta_resistance(r0, t0, b, t),
            Model::Sh(c) => c.resistance(t),
        }
    }
    fn temperature(self, r: f64) -> f64 {
        match self {
            Model::Beta(r0, t0, b) => ntc::beta_temperature(r0, t0, b, r),
            Model::Sh(c) => c.temperature(r),
        }
    }
}

// ----------------------------------------------------------------------------

/// 4–20 mA loop: scaling, NAMUR NE43 status, loop budget and HART.
#[derive(Debug, Args)]
pub struct LoopArgs {
    /// Loop current in mA.
    #[arg(long, conflicts_with_all = ["percent", "value"])]
    ma: Option<f64>,
    /// Percent of span.
    #[arg(long, allow_hyphen_values = true, conflicts_with = "value")]
    percent: Option<f64>,
    /// Engineering value (needs --range).
    #[arg(long, allow_hyphen_values = true)]
    value: Option<f64>,
    /// Range LRV:URV, e.g. 0:150 or -50:150.
    #[arg(long, default_value = "0:100", allow_hyphen_values = true)]
    range: String,
    /// Loop supply voltage, for the budget.
    #[arg(long, requires = "tx_min")]
    supply: Option<String>,
    /// Minimum transmitter voltage, for the budget.
    #[arg(long)]
    tx_min: Option<String>,
    /// Total loop resistance (sense resistor, barriers, wiring).
    #[arg(long, default_value = "250")]
    loop_ohms: String,
    /// Current to budget for in mA (20, or 22 to cover NE43 failure signalling).
    #[arg(long, default_value_t = 20.0)]
    max_ma: f64,
}

pub fn current_loop_cmd(a: &LoopArgs) -> Result<Output> {
    let (lrv, urv) = a.range.split_once(':').ok_or("range is LRV:URV")?;
    let (lrv, urv) = (qty(lrv, &[])?, qty(urv, &[])?);
    let mut lines = Vec::new();
    let mut j = json!({ "lrv": lrv, "urv": urv });
    let ma = match (a.ma, a.percent, a.value) {
        (Some(m), _, _) => Some(m),
        (None, Some(p), _) => Some(current_loop::ma_from_percent(p)),
        (None, None, Some(v)) => Some(current_loop::ma_from_value(v, lrv, urv)?),
        _ => None,
    };
    if let Some(m) = ma {
        let status = current_loop::Status::of(m);
        lines.push(table(&[
            ("Current", format!("{m:.4} mA")),
            (
                "Percent",
                format!("{:.3} %", current_loop::percent_from_ma(m)),
            ),
            (
                "Value",
                format_sig(current_loop::value_from_ma(m, lrv, urv), 6),
            ),
            ("NE43", status.label().to_string()),
            (
                "On 250 Ω",
                format!("{:.4} V", current_loop::sense_volts(250.0, m)),
            ),
        ]));
        j["ma"] = json!(m);
        j["percent"] = json!(current_loop::percent_from_ma(m));
        j["value"] = json!(current_loop::value_from_ma(m, lrv, urv));
        j["ne43"] = json!(status.label());
    }
    if let (Some(s), Some(tx)) = (&a.supply, &a.tx_min) {
        let (s, tx, rl) = (qty(s, &["V"])?, qty(tx, &["V"])?, ohms(&a.loop_ohms)?);
        let b = current_loop::budget(s, tx, rl, a.max_ma);
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(table(&[
            (
                "Max loop resistance",
                format!(
                    "{} at {} mA",
                    format_ohms(b.max_loop_ohms),
                    format_sig(a.max_ma, 3)
                ),
            ),
            (
                "Transmitter gets",
                format!(
                    "{:.2} V with {} in the loop ({})",
                    b.transmitter_volts,
                    format_ohms(rl),
                    if b.ok { "ok" } else { "TOO LOW" }
                ),
            ),
            (
                "HART",
                if b.hart_ok {
                    "ok (230–1100 Ω)".into()
                } else {
                    format!("needs 230–1100 Ω in the loop, has {}", format_ohms(rl))
                },
            ),
        ]));
        j["max_loop_ohms"] = json!(b.max_loop_ohms);
        j["transmitter_volts"] = json!(b.transmitter_volts);
        j["budget_ok"] = json!(b.ok);
        j["hart_ok"] = json!(b.hart_ok);
    }
    if lines.is_empty() {
        return Err("give --ma, --percent or --value, and/or --supply with --tx-min".into());
    }
    Ok(Output {
        text: lines.join("\n"),
        json: j,
    })
}

// ----------------------------------------------------------------------------

/// Wheatstone bridge: four arms give the output, three give the balancing R4.
#[derive(Debug, Args)]
pub struct BridgeArgs {
    /// Excitation voltage.
    #[arg(long, default_value = "10")]
    vex: String,
    /// R1 R2 R3 [R4]: left arm top and bottom, right arm top and bottom.
    #[arg(required = true, num_args = 3..=4, value_name = "OHMS")]
    arms: Vec<String>,
}

pub fn bridge_cmd(a: &BridgeArgs) -> Result<Output> {
    let r = a.arms.iter().map(|v| ohms(v)).collect::<Result<Vec<_>>>()?;
    if r.len() == 3 {
        let r4 = bridge::balance_r4(r[0], r[1], r[2]);
        return Ok(Output {
            text: format!("Balancing R4 = {}", format_ohms(r4)),
            json: json!({ "r4": r4 }),
        });
    }
    let v = bridge::output(qty(&a.vex, &["V"])?, r[0], r[1], r[2], r[3]);
    Ok(Output {
        text: format!("Output = {}", format_si(v, "V")),
        json: json!({ "volts": v }),
    })
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MaterialArg {
    Copper,
    #[value(alias = "aluminum")]
    Aluminium,
}

/// Wire resistance by AWG, mm² or diameter.
#[derive(Debug, Args)]
pub struct WireArgs {
    /// AWG size, e.g. 24, 1/0, 4/0.
    #[arg(long, conflicts_with_all = ["mm2", "diameter"])]
    awg: Option<String>,
    /// Cross-section in mm².
    #[arg(long, conflicts_with = "diameter")]
    mm2: Option<f64>,
    /// Diameter in mm.
    #[arg(long)]
    diameter: Option<f64>,
    /// Length in meters (one way).
    #[arg(short, long)]
    length: f64,
    /// Count both conductors of a two-wire run (out and back).
    #[arg(long)]
    round_trip: bool,
    #[arg(long, value_enum, default_value = "copper")]
    material: MaterialArg,
    /// Conductor temperature in °C.
    #[arg(long, default_value = "20", allow_hyphen_values = true)]
    temp: String,
}

pub fn wire_cmd(a: &WireArgs) -> Result<Output> {
    let area = match (&a.awg, a.mm2, a.diameter) {
        (Some(g), _, _) => wire::area_mm2(wire::awg_diameter_mm(wire::awg_from_label(g)?)),
        (None, Some(m), _) => m,
        (None, None, Some(d)) => wire::area_mm2(d),
        _ => return Err("give --awg, --mm2 or --diameter".into()),
    };
    let material = match a.material {
        MaterialArg::Copper => wire::Material::Copper,
        MaterialArg::Aluminium => wire::Material::Aluminium,
    };
    let t = qty(&a.temp, &["°C", "C"])?;
    let length = if a.round_trip {
        a.length * 2.0
    } else {
        a.length
    };
    let r = wire::resistance(material, area, length, t);
    Ok(Output {
        text: table(&[
            ("Cross-section", format!("{} mm²", format_sig(area, 4))),
            ("Conductor length", format!("{} m", format_sig(length, 6))),
            (
                "Resistance",
                format!(
                    "{} ({} at {} °C)",
                    format_ohms(r),
                    material.name(),
                    format_sig(t, 4)
                ),
            ),
            ("Per km", format_ohms(r / length * 1000.0)),
        ]),
        json: json!({ "area_mm2": area, "length_m": length, "ohms": r }),
    })
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LayerArg {
    External,
    Internal,
}

/// PCB trace resistance and IPC-2221 current capacity.
#[derive(Debug, Args)]
pub struct TraceArgs {
    /// Trace width in mm (or give --current to find the width).
    #[arg(short, long)]
    width: Option<f64>,
    /// Trace length in mm.
    #[arg(short, long, default_value_t = 25.4)]
    length: f64,
    /// Copper weight in oz/ft².
    #[arg(long, default_value_t = 1.0)]
    oz: f64,
    /// Allowed temperature rise in K.
    #[arg(long, default_value_t = 10.0)]
    rise: f64,
    #[arg(long, value_enum, default_value = "external")]
    layer: LayerArg,
    /// Current to carry, to find the minimum width.
    #[arg(short = 'I', long)]
    current: Option<String>,
    /// Copper temperature in °C for the resistance.
    #[arg(long, default_value = "20", allow_hyphen_values = true)]
    temp: String,
}

pub fn trace_cmd(a: &TraceArgs) -> Result<Output> {
    let layer = match a.layer {
        LayerArg::External => wire::Layer::External,
        LayerArg::Internal => wire::Layer::Internal,
    };
    let width = match (a.width, &a.current) {
        (Some(w), _) => w,
        (None, Some(i)) => wire::trace_width_for(qty(i, &["A"])?, a.oz, a.rise, layer),
        _ => return Err("give --width or --current".into()),
    };
    let r = wire::trace_resistance(width, a.length, a.oz, qty(&a.temp, &["°C", "C"])?);
    let imax = wire::trace_current(width, a.oz, a.rise, layer);
    Ok(Output {
        text: table(&[
            (
                "Width",
                format!(
                    "{} mm ({} mil)",
                    format_sig(width, 4),
                    format_sig(width / 0.0254, 4)
                ),
            ),
            (
                "Resistance",
                format!("{} over {} mm", format_ohms(r), format_sig(a.length, 5)),
            ),
            (
                "IPC-2221 current",
                format!(
                    "{} for a {} K rise ({} layer)",
                    format_si(imax, "A"),
                    format_sig(a.rise, 3),
                    if matches!(a.layer, LayerArg::External) {
                        "external"
                    } else {
                        "internal"
                    }
                ),
            ),
        ]),
        json: json!({ "width_mm": width, "ohms": r, "max_amps": imax }),
    })
}
