//! Design and instrumentation pages, declared as data: inputs with defaults
//! and a function from inputs to result rows. One builder makes every page,
//! so they all look and behave the same.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use foxres_core::{
    ESeries, bridge, current_loop, divider, led, network, ntc, ohm,
    power::{self, Mount},
    rtd, thermal, tolerance,
    value::{format_ohms, format_si, format_sig, parse_ohms, parse_si},
    wire,
};
use relm4::{
    adw::{self, prelude::*},
    gtk::{self, glib},
};

use crate::text::pct;

/// One input on a tool page.
pub enum Input {
    /// Free text, with a default.
    Text {
        key: &'static str,
        title: &'static str,
        default: &'static str,
    },
    /// A choice from a list; the default is an index.
    Choice {
        key: &'static str,
        title: &'static str,
        options: &'static [&'static str],
        default: u32,
    },
}

/// Turns the inputs into labelled result lines, or an error message.
pub type Compute = fn(&Values) -> Result<Vec<(String, String)>, String>;

/// Result rows currently shown on a page.
type ResultRows = Rc<RefCell<Vec<adw::ActionRow>>>;

/// A tool page.
pub struct Tool {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub inputs: Vec<Input>,
    pub compute: Compute,
}

/// Current input values, by key.
pub struct Values(HashMap<&'static str, String>);

impl Values {
    fn text(&self, key: &str) -> &str {
        self.0.get(key).map_or("", |s| s.trim())
    }

    fn is_empty(&self, key: &str) -> bool {
        self.text(key).is_empty()
    }

    fn num(&self, key: &str, name: &str, units: &[&str]) -> Result<f64, String> {
        parse_si(self.text(key), units).map_err(|_| format!("{name}: enter a number"))
    }

    fn opt(&self, key: &str, name: &str, units: &[&str]) -> Result<Option<f64>, String> {
        if self.is_empty(key) {
            Ok(None)
        } else {
            self.num(key, name, units).map(Some)
        }
    }

    fn ohms(&self, key: &str, name: &str) -> Result<f64, String> {
        parse_ohms(self.text(key)).map_err(|_| format!("{name}: enter a resistance such as 4k7"))
    }

    fn series(&self, key: &str) -> ESeries {
        self.text(key).parse().unwrap_or(ESeries::E24)
    }
}

const SERIES: &[&str] = &["E3", "E6", "E12", "E24", "E48", "E96", "E192"];

fn row(label: &str, value: impl Into<String>) -> (String, String) {
    (label.to_string(), value.into())
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn ohms_list(text: &str) -> Result<Vec<f64>, String> {
    text.split([',', ' ', ';'])
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_ohms(s).map_err(|_| format!("\"{}\" is not a resistance", s.trim())))
        .collect()
}

/// Every tool, in sidebar order. Long because it is a table of definitions.
#[allow(clippy::too_many_lines)]
pub fn all() -> Vec<Tool> {
    vec![
        Tool {
            id: "divider",
            title: "Voltage Divider",
            description: "Best pair of standard values for an output voltage, with tolerance and loading",
            inputs: vec![
                Input::Text {
                    key: "vin",
                    title: "Input voltage (V)",
                    default: "12",
                },
                Input::Text {
                    key: "vout",
                    title: "Wanted output voltage (V)",
                    default: "3.3",
                },
                Input::Choice {
                    key: "series",
                    title: "Series",
                    options: SERIES,
                    default: 3,
                },
                Input::Text {
                    key: "total",
                    title: "Aim for R1 + R2 around",
                    default: "10k",
                },
                Input::Text {
                    key: "tol",
                    title: "Resistor tolerance (%)",
                    default: "1",
                },
                Input::Text {
                    key: "load",
                    title: "Load on the output (optional)",
                    default: "",
                },
            ],
            compute: |v| {
                let vin = v.num("vin", "Input voltage", &["V"])?;
                let vout = v.num("vout", "Output voltage", &["V"])?;
                let tol = v.num("tol", "Tolerance", &["%"])?;
                let p =
                    divider::best_pair(vout / vin, v.series("series"), v.ohms("total", "Total")?)
                        .map_err(err)?;
                let out = divider::output(vin, p.r1, p.r2, None);
                let (lo, hi) = divider::ratio_range(p.r1, p.r2, tol, tol);
                let mut rows = vec![
                    row("R1 (top)", format_ohms(p.r1)),
                    row("R2 (bottom)", format_ohms(p.r2)),
                    row(
                        "Output",
                        format!("{}  ({})", format_si(out, "V"), pct(p.error)),
                    ),
                    row(
                        "Worst case",
                        format!(
                            "{} to {} with ±{} % parts",
                            format_si(vin * lo, "V"),
                            format_si(vin * hi, "V"),
                            format_sig(tol, 3)
                        ),
                    ),
                    row("Divider current", format_si(vin / (p.r1 + p.r2), "A")),
                    row(
                        "Power",
                        format!(
                            "R1 {},  R2 {}",
                            format_si((vin - out).powi(2) / p.r1, "W"),
                            format_si(out * out / p.r2, "W")
                        ),
                    ),
                ];
                if !v.is_empty("load") {
                    let rl = v.ohms("load", "Load")?;
                    let loaded = divider::output(vin, p.r1, p.r2, Some(rl));
                    rows.push(row(
                        "Output with load",
                        format!(
                            "{}  ({})",
                            format_si(loaded, "V"),
                            pct((loaded - out) / out * 100.0)
                        ),
                    ));
                }
                Ok(rows)
            },
        },
        Tool {
            id: "led",
            title: "LED Resistor",
            description: "Series resistor for one or more LEDs; rounds up so the current never exceeds the target",
            inputs: vec![
                Input::Text {
                    key: "supply",
                    title: "Supply voltage (V)",
                    default: "5",
                },
                Input::Text {
                    key: "vf",
                    title: "LED forward voltage (V)",
                    default: "2",
                },
                Input::Text {
                    key: "current",
                    title: "LED current (A, e.g. 20m)",
                    default: "20m",
                },
                Input::Text {
                    key: "count",
                    title: "LEDs in series",
                    default: "1",
                },
                Input::Choice {
                    key: "series",
                    title: "Series",
                    options: SERIES,
                    default: 3,
                },
            ],
            compute: |v| {
                let count: u32 = v
                    .text("count")
                    .parse()
                    .map_err(|_| "LEDs in series: enter a whole number".to_string())?;
                let r = led::size(
                    v.num("supply", "Supply", &["V"])?,
                    v.num("vf", "Forward voltage", &["V"])?,
                    count,
                    v.num("current", "Current", &["A"])?,
                    v.series("series"),
                )
                .map_err(err)?;
                Ok(vec![
                    row("Resistor", format_ohms(r.ohms)),
                    row("Exact value", format_ohms(r.ideal_ohms)),
                    row("LED current", format_si(r.amps, "A")),
                    row(
                        "Resistor power",
                        format!(
                            "{} (use a part rated {} or more)",
                            format_si(r.resistor_watts, "W"),
                            format_si(r.resistor_watts * 2.0, "W")
                        ),
                    ),
                    row("Power per LED", format_si(r.led_watts, "W")),
                ])
            },
        },
        Tool {
            id: "ohm",
            title: "Ohm's Law",
            description: "Fill in any two of voltage, current, resistance and power",
            inputs: vec![
                Input::Text {
                    key: "v",
                    title: "Voltage (V)",
                    default: "12",
                },
                Input::Text {
                    key: "i",
                    title: "Current (A, e.g. 20m)",
                    default: "",
                },
                Input::Text {
                    key: "r",
                    title: "Resistance (Ω)",
                    default: "470",
                },
                Input::Text {
                    key: "p",
                    title: "Power (W)",
                    default: "",
                },
            ],
            compute: |v| {
                let r = if v.is_empty("r") {
                    None
                } else {
                    Some(v.ohms("r", "Resistance")?)
                };
                let c = ohm::solve(
                    v.opt("v", "Voltage", &["V"])?,
                    v.opt("i", "Current", &["A"])?,
                    r,
                    v.opt("p", "Power", &["W"])?,
                )
                .map_err(err)?;
                Ok(vec![
                    row("Voltage", format_si(c.volts, "V")),
                    row("Current", format_si(c.amps, "A")),
                    row("Resistance", format_ohms(c.ohms)),
                    row("Power", format_si(c.watts, "W")),
                ])
            },
        },
        Tool {
            id: "power",
            title: "Power & Package",
            description: "Power and voltage against typical package ratings, derated above 70 °C",
            inputs: vec![
                Input::Text {
                    key: "r",
                    title: "Resistance (Ω)",
                    default: "470",
                },
                Input::Text {
                    key: "v",
                    title: "Voltage across it (V)",
                    default: "12",
                },
                Input::Text {
                    key: "i",
                    title: "…or current through it (A)",
                    default: "",
                },
                Input::Text {
                    key: "ambient",
                    title: "Ambient temperature (°C)",
                    default: "25",
                },
                Input::Choice {
                    key: "margin",
                    title: "Safety margin",
                    options: &["2× (recommended)", "1.5×", "None"],
                    default: 0,
                },
                Input::Choice {
                    key: "mount",
                    title: "Mounting",
                    options: &["Surface mount", "Through-hole"],
                    default: 0,
                },
            ],
            compute: |v| {
                let r = v.ohms("r", "Resistance")?;
                let (volts, amps) = (
                    v.opt("v", "Voltage", &["V"])?,
                    v.opt("i", "Current", &["A"])?,
                );
                let c = match (volts, amps) {
                    (Some(_), Some(_)) => {
                        return Err("Give the voltage or the current, not both".into());
                    }
                    _ => ohm::solve(volts, amps, Some(r), None)
                        .map_err(|_| "Give the voltage or the current".to_string())?,
                };
                let ambient = v.num("ambient", "Ambient", &["°C", "C"])?;
                let margin = match v.text("margin") {
                    "1.5×" => 1.5,
                    "None" => 1.0,
                    _ => 2.0,
                };
                let mount = if v.text("mount") == "Through-hole" {
                    Mount::ThroughHole
                } else {
                    Mount::Smd
                };
                let mut rows = vec![
                    row(
                        "Dissipation",
                        format!("{} at {}", format_si(c.watts, "W"), format_si(c.volts, "V")),
                    ),
                    row(
                        "Smallest package",
                        power::recommend(mount, r, c.watts, ambient, margin)
                            .map_or("None is large enough".into(), |k| {
                                k.package.name.to_string()
                            }),
                    ),
                    row(
                        "Derating at this temperature",
                        format!("{:.0} % of rated power", power::derating(ambient) * 100.0),
                    ),
                ];
                for p in power::PACKAGES.iter().filter(|p| p.mount == mount) {
                    let k = power::check(*p, r, c.watts, ambient, margin);
                    rows.push(row(
                        p.name,
                        format!(
                            "{}   {:.0} % of {} allowed, {} V max",
                            if k.ok { "✓" } else { "✗" },
                            k.utilisation * 100.0,
                            format_si(k.allowed_watts, "W"),
                            format_sig(p.max_volts, 3)
                        ),
                    ));
                }
                rows.push(row("Note", "Typical ratings; always check the datasheet"));
                Ok(rows)
            },
        },
        Tool {
            id: "network",
            title: "Series & Parallel",
            description: "Any number of resistors combined in series or in parallel",
            inputs: vec![Input::Text {
                key: "list",
                title: "Resistors (separated by commas or spaces)",
                default: "1k, 2k2, 4k7",
            }],
            compute: |v| {
                let vals = ohms_list(v.text("list"))?;
                if vals.is_empty() {
                    return Err("Enter at least one resistance".into());
                }
                Ok(vec![
                    row("In series", format_ohms(network::series(&vals))),
                    row("In parallel", format_ohms(network::parallel(&vals))),
                ])
            },
        },
        Tool {
            id: "tolerance",
            title: "Tolerance",
            description: "Worst-case and statistical (RSS) spread of series, parallel and divider circuits",
            inputs: vec![
                Input::Choice {
                    key: "circuit",
                    title: "Circuit",
                    options: &["Divider ratio (R1, R2)", "Series", "Parallel"],
                    default: 0,
                },
                Input::Text {
                    key: "parts",
                    title: "Resistors as value:tolerance %",
                    default: "10k:1, 4k7:1",
                },
            ],
            compute: |v| {
                let mut vals = Vec::new();
                let mut tols = Vec::new();
                for p in v
                    .text("parts")
                    .split([',', ' ', ';'])
                    .filter(|s| !s.trim().is_empty())
                {
                    let (r, t) = p.split_once(':').unwrap_or((p, "5"));
                    vals.push(parse_ohms(r).map_err(|_| format!("\"{r}\" is not a resistance"))?);
                    tols.push(
                        parse_si(t, &["%"]).map_err(|_| format!("\"{t}\" is not a tolerance"))?,
                    );
                }
                let divider_mode = v.text("circuit").starts_with("Divider");
                let s = if divider_mode {
                    if vals.len() != 2 {
                        return Err("A divider takes exactly two resistors: R1, R2".into());
                    }
                    tolerance::analyse(&vals, &tols, |x| divider::ratio(x[0], x[1]))
                } else if v.text("circuit") == "Parallel" {
                    tolerance::analyse(&vals, &tols, network::parallel)
                } else {
                    tolerance::analyse(&vals, &tols, network::series)
                }
                .map_err(err)?;
                let show = |x: f64| {
                    if divider_mode {
                        format_sig(x, 6)
                    } else {
                        format_ohms(x)
                    }
                };
                let rel = |x: f64| pct((x - s.nominal) / s.nominal * 100.0);
                Ok(vec![
                    row("Nominal", show(s.nominal)),
                    row(
                        "Worst-case minimum",
                        format!("{}  ({})", show(s.min), rel(s.min)),
                    ),
                    row(
                        "Worst-case maximum",
                        format!("{}  ({})", show(s.max), rel(s.max)),
                    ),
                    row(
                        "RSS (statistical)",
                        format!("± {}  (± {:.3} %)", show(s.rss), s.rss / s.nominal * 100.0),
                    ),
                ])
            },
        },
        Tool {
            id: "thermal",
            title: "Temperature & Noise",
            description: "Drift from the temperature coefficient, and Johnson–Nyquist thermal noise",
            inputs: vec![
                Input::Text {
                    key: "r",
                    title: "Resistance (Ω)",
                    default: "10k",
                },
                Input::Text {
                    key: "ppm",
                    title: "Temperature coefficient (ppm/K)",
                    default: "100",
                },
                Input::Text {
                    key: "t0",
                    title: "Reference temperature (°C)",
                    default: "25",
                },
                Input::Text {
                    key: "t",
                    title: "Operating temperature (°C)",
                    default: "85",
                },
                Input::Text {
                    key: "bw",
                    title: "Noise bandwidth (Hz)",
                    default: "20k",
                },
            ],
            compute: |v| {
                let r = v.ohms("r", "Resistance")?;
                let (t0, t) = (
                    v.num("t0", "Reference", &["°C", "C"])?,
                    v.num("t", "Temperature", &["°C", "C"])?,
                );
                let rt = thermal::at_temperature(r, v.num("ppm", "Tempco", &[])?, t0, t);
                let n = thermal::johnson(r, t, v.num("bw", "Bandwidth", &["Hz"])?);
                Ok(vec![
                    row(
                        "Resistance at temperature",
                        format!("{}  ({})", format_ohms(rt), pct((rt - r) / r * 100.0)),
                    ),
                    row(
                        "Noise density",
                        format!("{}/√Hz", format_si(n.density, "V")),
                    ),
                    row("RMS noise voltage", format_si(n.volts_rms, "V")),
                    row("RMS noise current", format_si(n.amps_rms, "A")),
                ])
            },
        },
        Tool {
            id: "rtd",
            title: "RTD (Pt100/Pt1000)",
            description: "IEC 60751 Callendar–Van Dusen, both directions, with tolerance classes",
            inputs: vec![
                Input::Choice {
                    key: "type",
                    title: "Sensor",
                    options: &["Pt100", "Pt500", "Pt1000"],
                    default: 0,
                },
                Input::Choice {
                    key: "dir",
                    title: "Convert",
                    options: &["°C → Ω", "Ω → °C"],
                    default: 0,
                },
                Input::Text {
                    key: "value",
                    title: "Value (°C or Ω)",
                    default: "100",
                },
                Input::Choice {
                    key: "element",
                    title: "Element",
                    options: &["Wire-wound", "Film"],
                    default: 0,
                },
            ],
            compute: |v| {
                let r0 = match v.text("type") {
                    "Pt500" => 500.0,
                    "Pt1000" => 1000.0,
                    _ => 100.0,
                };
                let x = v.num("value", "Value", &["°C", "C", "Ω"])?;
                let (t, r) = if v.text("dir").starts_with("°C") {
                    (
                        x,
                        rtd::resistance(r0, x)
                            .map_err(|_| "Temperature must be −200 to 850 °C".to_string())?,
                    )
                } else {
                    (
                        rtd::temperature(r0, x)
                            .map_err(|_| "Resistance is outside −200 to 850 °C".to_string())?,
                        x,
                    )
                };
                let s = rtd::sensitivity(r0, t);
                let element = if v.text("element") == "Film" {
                    rtd::Element::Film
                } else {
                    rtd::Element::WireWound
                };
                let mut rows = vec![
                    row("Temperature", format!("{t:.3} °C")),
                    row("Resistance", format!("{r:.4} Ω")),
                    row("Sensitivity", format!("{s:.4} Ω/K")),
                ];
                for c in rtd::Class::ALL {
                    let (lo, hi) = c.valid_range(element);
                    rows.push(row(
                        &format!("Class {}", c.name()),
                        if (lo..=hi).contains(&t) {
                            let k = c.tolerance_k(t);
                            format!("± {k:.3} K   (± {:.4} Ω)", k * s)
                        } else {
                            format!("not defined here (valid {lo} to {hi} °C)")
                        },
                    ));
                }
                Ok(rows)
            },
        },
        Tool {
            id: "ntc",
            title: "NTC Thermistor",
            description: "Beta model, or a Steinhart–Hart fit through three calibration points",
            inputs: vec![
                Input::Choice {
                    key: "model",
                    title: "Model",
                    options: &["Beta", "Steinhart–Hart (3 points)"],
                    default: 0,
                },
                Input::Text {
                    key: "r0",
                    title: "R at 25 °C (Beta)",
                    default: "10k",
                },
                Input::Text {
                    key: "beta",
                    title: "Beta (K)",
                    default: "3950",
                },
                Input::Text {
                    key: "points",
                    title: "Points as °C:Ω (Steinhart–Hart)",
                    default: "0:32.65k, 25:10k, 50:3.6k",
                },
                Input::Choice {
                    key: "dir",
                    title: "Convert",
                    options: &["°C → Ω", "Ω → °C"],
                    default: 0,
                },
                Input::Text {
                    key: "value",
                    title: "Value (°C or Ω)",
                    default: "37",
                },
            ],
            compute: |v| {
                let x = v.text("value");
                let to_r = v.text("dir").starts_with("°C");
                let mut rows = Vec::new();
                let (t, r) = if v.text("model") == "Beta" {
                    let (r0, b) = (v.ohms("r0", "R at 25 °C")?, v.num("beta", "Beta", &["K"])?);
                    if to_r {
                        let t = parse_si(x, &["°C", "C"]).map_err(err)?;
                        (t, ntc::beta_resistance(r0, 25.0, b, t))
                    } else {
                        let r = parse_ohms(x).map_err(err)?;
                        (ntc::beta_temperature(r0, 25.0, b, r), r)
                    }
                } else {
                    let mut p = Vec::new();
                    for s in v
                        .text("points")
                        .split([',', ';'])
                        .filter(|s| !s.trim().is_empty())
                    {
                        let (t, r) = s.split_once(':').ok_or("Points are °C:Ω, e.g. 25:10k")?;
                        p.push((
                            parse_si(t, &["°C", "C"]).map_err(err)?,
                            parse_ohms(r).map_err(err)?,
                        ));
                    }
                    let p: [(f64, f64); 3] = p
                        .try_into()
                        .map_err(|_| "Give exactly three points".to_string())?;
                    let sh = ntc::SteinhartHart::fit(p).map_err(err)?;
                    rows.push(row(
                        "A, B, C",
                        format!("{:.6e},  {:.6e},  {:.6e}", sh.a, sh.b, sh.c),
                    ));
                    rows.push(row(
                        "Equivalent Beta (0–50 °C span)",
                        format!("{:.1} K", ntc::beta_from_points(p[0], p[2])),
                    ));
                    if to_r {
                        let t = parse_si(x, &["°C", "C"]).map_err(err)?;
                        (t, sh.resistance(t))
                    } else {
                        let r = parse_ohms(x).map_err(err)?;
                        (sh.temperature(r), r)
                    }
                };
                rows.insert(0, row("Resistance", format_ohms(r)));
                rows.insert(0, row("Temperature", format!("{t:.3} °C")));
                Ok(rows)
            },
        },
        Tool {
            id: "loop",
            title: "4–20 mA Loop",
            description: "Scaling, NAMUR NE43 signal status, loop voltage budget and HART",
            inputs: vec![
                Input::Text {
                    key: "lrv",
                    title: "Range at 4 mA",
                    default: "0",
                },
                Input::Text {
                    key: "urv",
                    title: "Range at 20 mA",
                    default: "150",
                },
                Input::Text {
                    key: "ma",
                    title: "Loop current (mA)",
                    default: "12",
                },
                Input::Text {
                    key: "supply",
                    title: "Loop supply (V)",
                    default: "24",
                },
                Input::Text {
                    key: "tx",
                    title: "Transmitter minimum voltage (V)",
                    default: "12",
                },
                Input::Text {
                    key: "rl",
                    title: "Total loop resistance (Ω)",
                    default: "250",
                },
                Input::Choice {
                    key: "max",
                    title: "Budget at",
                    options: &["20 mA", "22 mA (covers NE43 failure signal)"],
                    default: 0,
                },
            ],
            compute: |v| {
                let (lrv, urv) = (
                    v.num("lrv", "Range at 4 mA", &[])?,
                    v.num("urv", "Range at 20 mA", &[])?,
                );
                let ma = v.num("ma", "Loop current", &["mA"])?;
                let max = if v.text("max").starts_with("22") {
                    22.0
                } else {
                    20.0
                };
                let b = current_loop::budget(
                    v.num("supply", "Supply", &["V"])?,
                    v.num("tx", "Transmitter voltage", &["V"])?,
                    v.ohms("rl", "Loop resistance")?,
                    max,
                );
                Ok(vec![
                    row(
                        "Percent of span",
                        format!("{:.3} %", current_loop::percent_from_ma(ma)),
                    ),
                    row(
                        "Process value",
                        format_sig(current_loop::value_from_ma(ma, lrv, urv), 6),
                    ),
                    row("NE43 status", current_loop::Status::of(ma).label()),
                    row(
                        "Across 250 Ω",
                        format!("{:.4} V", current_loop::sense_volts(250.0, ma)),
                    ),
                    row(
                        "Maximum loop resistance",
                        format!("{} at {} mA", format_ohms(b.max_loop_ohms), max),
                    ),
                    row(
                        "Transmitter terminal voltage",
                        format!(
                            "{:.2} V  {}",
                            b.transmitter_volts,
                            if b.ok { "✓" } else { "✗ too low" }
                        ),
                    ),
                    row(
                        "HART (needs 230–1100 Ω)",
                        if b.hart_ok {
                            "✓ within range"
                        } else {
                            "✗ outside range"
                        },
                    ),
                ])
            },
        },
        Tool {
            id: "bridge",
            title: "Wheatstone Bridge",
            description: "Output voltage, and the R4 that balances the bridge",
            inputs: vec![
                Input::Text {
                    key: "vex",
                    title: "Excitation (V)",
                    default: "10",
                },
                Input::Text {
                    key: "r1",
                    title: "R1 (left top)",
                    default: "350",
                },
                Input::Text {
                    key: "r2",
                    title: "R2 (left bottom)",
                    default: "350",
                },
                Input::Text {
                    key: "r3",
                    title: "R3 (right top)",
                    default: "350",
                },
                Input::Text {
                    key: "r4",
                    title: "R4 (right bottom)",
                    default: "350.7",
                },
            ],
            compute: |v| {
                let (r1, r2, r3, r4) = (
                    v.ohms("r1", "R1")?,
                    v.ohms("r2", "R2")?,
                    v.ohms("r3", "R3")?,
                    v.ohms("r4", "R4")?,
                );
                let out = bridge::output(v.num("vex", "Excitation", &["V"])?, r1, r2, r3, r4);
                Ok(vec![
                    row("Output", format_si(out, "V")),
                    row(
                        "R4 for balance",
                        format_ohms(bridge::balance_r4(r1, r2, r3)),
                    ),
                ])
            },
        },
        Tool {
            id: "wire",
            title: "Wire",
            description: "Conductor resistance by AWG, mm² or diameter, corrected for temperature",
            inputs: vec![
                Input::Choice {
                    key: "unit",
                    title: "Size given as",
                    options: &["AWG", "mm²", "Diameter (mm)"],
                    default: 0,
                },
                Input::Text {
                    key: "size",
                    title: "Size",
                    default: "18",
                },
                Input::Text {
                    key: "length",
                    title: "Cable length (m)",
                    default: "150",
                },
                Input::Choice {
                    key: "trip",
                    title: "Count",
                    options: &["Out and back (two conductors)", "One conductor"],
                    default: 0,
                },
                Input::Choice {
                    key: "material",
                    title: "Material",
                    options: &["Copper", "Aluminum"],
                    default: 0,
                },
                Input::Text {
                    key: "temp",
                    title: "Conductor temperature (°C)",
                    default: "20",
                },
            ],
            compute: |v| {
                let size = v.text("size");
                let area = match v.text("unit") {
                    "mm²" => parse_si(size, &["mm²", "mm2"]).map_err(err)?,
                    "Diameter (mm)" => wire::area_mm2(parse_si(size, &["mm"]).map_err(err)?),
                    _ => wire::area_mm2(wire::awg_diameter_mm(
                        wire::awg_from_label(size)
                            .map_err(|_| "AWG must be 4/0 to 40".to_string())?,
                    )),
                };
                let material = if v.text("material") == "Aluminum" {
                    wire::Material::Aluminium
                } else {
                    wire::Material::Copper
                };
                let mut length = v.num("length", "Length", &["m"])?;
                if v.text("trip").starts_with("Out") {
                    length *= 2.0;
                }
                let r = wire::resistance(
                    material,
                    area,
                    length,
                    v.num("temp", "Temperature", &["°C", "C"])?,
                );
                Ok(vec![
                    row("Resistance", format_ohms(r)),
                    row("Cross-section", format!("{} mm²", format_sig(area, 4))),
                    row("Conductor length", format!("{} m", format_sig(length, 6))),
                    row("Per kilometer", format_ohms(r / length * 1000.0)),
                ])
            },
        },
        Tool {
            id: "trace",
            title: "PCB Trace",
            description: "Trace resistance and IPC-2221 current capacity; leave the width empty to size it for a current",
            inputs: vec![
                Input::Text {
                    key: "width",
                    title: "Width (mm)",
                    default: "0.5",
                },
                Input::Text {
                    key: "current",
                    title: "…or current to carry (A)",
                    default: "",
                },
                Input::Text {
                    key: "length",
                    title: "Length (mm)",
                    default: "25.4",
                },
                Input::Choice {
                    key: "oz",
                    title: "Copper weight",
                    options: &["0.5 oz", "1 oz", "2 oz", "3 oz"],
                    default: 1,
                },
                Input::Text {
                    key: "rise",
                    title: "Allowed temperature rise (K)",
                    default: "10",
                },
                Input::Choice {
                    key: "layer",
                    title: "Layer",
                    options: &["External", "Internal"],
                    default: 0,
                },
            ],
            compute: |v| {
                let oz: f64 = v.text("oz").trim_end_matches(" oz").parse().unwrap_or(1.0);
                let rise = v.num("rise", "Temperature rise", &["K"])?;
                let layer = if v.text("layer") == "Internal" {
                    wire::Layer::Internal
                } else {
                    wire::Layer::External
                };
                let width = if v.is_empty("width") {
                    wire::trace_width_for(v.num("current", "Current", &["A"])?, oz, rise, layer)
                } else {
                    v.num("width", "Width", &["mm"])?
                };
                let r =
                    wire::trace_resistance(width, v.num("length", "Length", &["mm"])?, oz, 20.0);
                Ok(vec![
                    row(
                        "Width",
                        format!(
                            "{} mm  ({} mil)",
                            format_sig(width, 4),
                            format_sig(width / 0.0254, 4)
                        ),
                    ),
                    row("Resistance at 20 °C", format_ohms(r)),
                    row(
                        "IPC-2221 current",
                        format_si(wire::trace_current(width, oz, rise, layer), "A"),
                    ),
                ])
            },
        },
    ]
}

/// Build the page for `tool`. Results update as the inputs change.
pub fn page(tool: Tool) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    // The page title is already in the header bar; the group carries the description.
    let inputs = adw::PreferencesGroup::builder()
        .description(glib::markup_escape_text(tool.description).as_str())
        .build();
    let results = adw::PreferencesGroup::new();
    page.add(&inputs);
    page.add(&results);

    let mut initial = HashMap::new();
    for input in &tool.inputs {
        match input {
            Input::Text { key, default, .. } => initial.insert(*key, (*default).to_string()),
            Input::Choice {
                key,
                options,
                default,
                ..
            } => initial.insert(*key, options[*default as usize].to_string()),
        };
    }
    let values = Rc::new(RefCell::new(Values(initial)));
    let rows: ResultRows = Rc::default();
    let compute = tool.compute;

    let refresh = {
        let (values, rows, results) = (values.clone(), rows.clone(), results.clone());
        Rc::new(move || {
            for r in rows.borrow_mut().drain(..) {
                results.remove(&r);
            }
            let out = compute(&values.borrow());
            let lines = match out {
                Ok(lines) => lines,
                Err(e) => vec![("Check the inputs".to_string(), e)],
            };
            let failed = lines.len() == 1 && lines[0].0 == "Check the inputs";
            for (title, value) in lines {
                let row = adw::ActionRow::builder()
                    .use_markup(false)
                    .title(title)
                    .subtitle(value)
                    .subtitle_selectable(true)
                    .build();
                row.add_css_class("property");
                if failed {
                    row.add_css_class("error");
                }
                results.add(&row);
                rows.borrow_mut().push(row);
            }
        })
    };

    for input in tool.inputs {
        match input {
            Input::Text {
                key,
                title,
                default,
            } => {
                let entry = adw::EntryRow::builder()
                    .use_markup(false)
                    .title(title)
                    .text(default)
                    .build();
                let (values, refresh) = (values.clone(), refresh.clone());
                entry.connect_changed(move |e| {
                    values.borrow_mut().0.insert(key, e.text().to_string());
                    refresh();
                });
                inputs.add(&entry);
            }
            Input::Choice {
                key,
                title,
                options,
                default,
            } => {
                let combo = adw::ComboRow::builder()
                    .use_markup(false)
                    .title(title)
                    .model(&gtk::StringList::new(options))
                    .selected(default)
                    .build();
                let (values, refresh) = (values.clone(), refresh.clone());
                combo.connect_selected_notify(move |c| {
                    if let Some(o) = options.get(c.selected() as usize) {
                        values.borrow_mut().0.insert(key, (*o).to_string());
                        refresh();
                    }
                });
                inputs.add(&combo);
            }
        }
    }
    refresh();
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tool computes a result from its defaults, so no page opens on an error.
    #[test]
    fn defaults_compute() {
        for tool in all() {
            let mut m = HashMap::new();
            for input in &tool.inputs {
                match input {
                    Input::Text { key, default, .. } => m.insert(*key, (*default).to_string()),
                    Input::Choice {
                        key,
                        options,
                        default,
                        ..
                    } => m.insert(*key, options[*default as usize].to_string()),
                };
            }
            let out = (tool.compute)(&Values(m));
            assert!(out.is_ok(), "{}: {:?}", tool.id, out.err());
        }
    }
}
