//! Command-line interface for the resistor calculator.
//!
//! The GUI binary calls [`run`] whenever it is started with arguments, so
//! `foxres smd 472` prints a result and `foxres` alone opens the window.

use std::{ffi::OsString, process::ExitCode};

use clap::{Parser, Subcommand};
use foxres_core::{
    BandCount, Color, ESeries, Error,
    bands::{self, Role},
    combo::{self, Connection, Pair},
    eseries::error_percent,
    smd,
    value::{format_ohms, format_sig, parse_ohms},
};
use serde_json::{Value, json};

mod tools;

#[derive(Debug, Parser)]
#[command(
    name = "foxres",
    version,
    about = "Resistor calculator and design toolkit: color codes, SMD markings, E-series, dividers, power, tolerance, RTDs, thermistors, 4-20 mA loops and wiring",
    after_help = "Run with no arguments to open the window.\n\n\
        Values accept engineering notation: 4700, 4.7k, 4k7, 100R, R47, 2M2, 10m (milliohms).\n\n\
        Examples:\n  \
        foxres bands yellow violet red gold\n  \
        foxres colors 4k7 --bands 5\n  \
        foxres smd 01C\n  \
        foxres marking 4.99k\n  \
        foxres nearest 5k --series E96\n  \
        foxres combine 5.7k --series E12 --json\n  \
        foxres ohm -V 12 -R 470\n  \
        foxres power 470 -V 12 --ambient 60\n  \
        foxres divider --vin 12 --vout 3.3\n  \
        foxres led --supply 5 --vf 2 -I 20m\n  \
        foxres tolerance divider 10k:1 4k7:1\n  \
        foxres rtd --temp 100 --r0 1000\n  \
        foxres ntc --beta 3950 --ohms 3.6k\n  \
        foxres loop --ma 12 --range 0:150 --supply 24 --tx-min 12\n  \
        foxres wire --awg 18 --length 150 --round-trip"
)]
pub struct Cli {
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Decode color bands, read left to right (1, 3, 4, 5 or 6 colors).
    Bands {
        #[arg(required = true, num_args = 1..=6, value_name = "COLOR")]
        colors: Vec<Color>,
    },
    /// Give the color bands for a resistance.
    Colors {
        #[arg(value_name = "VALUE")]
        value: String,
        /// Number of bands.
        #[arg(short = 'n', long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(3..=6))]
        bands: u8,
        /// Tolerance band color [default: gold for 4 bands, brown for 5 and 6].
        #[arg(short, long)]
        tolerance: Option<Color>,
        /// Temperature coefficient band color, for 6 bands.
        #[arg(long, default_value = "brown")]
        tempco: Color,
    },
    /// Decode an SMD marking such as 472, 4701, 4R7 or 01C.
    Smd {
        #[arg(value_name = "CODE")]
        code: String,
    },
    /// Give the SMD markings for a resistance.
    Marking {
        #[arg(value_name = "VALUE")]
        value: String,
    },
    /// Find the nearest standard value.
    Nearest {
        #[arg(value_name = "VALUE")]
        value: String,
        /// E-series to use (E3, E6, E12, E24, E48, E96, E192).
        #[arg(short, long, default_value = "E24")]
        series: ESeries,
    },
    /// Find the best two- and three-resistor combinations of standard values.
    Combine {
        #[arg(value_name = "VALUE")]
        value: String,
        /// E-series to use (E3, E6, E12, E24, E48, E96, E192).
        #[arg(short, long, default_value = "E24")]
        series: ESeries,
    },
    /// Ohm's law: give any two of -V, -I, -R and -P.
    Ohm(tools::OhmArgs),
    /// Check power and voltage against package ratings, with derating.
    Power(tools::PowerArgs),
    /// Design or analyse a voltage divider.
    Divider(tools::DividerArgs),
    /// Size an LED series resistor.
    Led(tools::LedArgs),
    /// Resistors in series.
    Series(tools::NetArgs),
    /// Resistors in parallel.
    Parallel(tools::NetArgs),
    /// Worst-case and RSS tolerance of a series, parallel or divider circuit.
    Tolerance(tools::ToleranceArgs),
    /// Resistance change with temperature from a tempco.
    Drift(tools::DriftArgs),
    /// Johnson–Nyquist thermal noise.
    Noise(tools::NoiseArgs),
    /// Pt100/Pt1000 RTD to IEC 60751, with tolerance classes.
    Rtd(tools::RtdArgs),
    /// NTC thermistor: Beta, Steinhart–Hart, or fit from three points.
    Ntc(tools::NtcArgs),
    /// 4–20 mA loop scaling, NAMUR NE43 status, loop budget and HART.
    Loop(tools::LoopArgs),
    /// Wheatstone bridge output, or the balancing resistor.
    Bridge(tools::BridgeArgs),
    /// Wire resistance by AWG, mm² or diameter.
    Wire(tools::WireArgs),
    /// PCB trace resistance and IPC-2221 current capacity.
    Trace(tools::TraceArgs),
}

/// Result of a command: what to print as text and as JSON.
pub(crate) struct Output {
    pub(crate) text: String,
    pub(crate) json: Value,
}

/// Parse `args` (including the program name) and run the command.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match execute(&cli.command) {
        Ok(out) => {
            if cli.json {
                println!("{}", out.json);
            } else {
                println!("{}", out.text);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            if cli.json {
                println!("{}", json!({ "error": e.to_string() }));
            } else {
                eprintln!("foxres: {e}");
            }
            ExitCode::FAILURE
        }
    }
}

fn names(colors: &[Color]) -> Vec<String> {
    colors
        .iter()
        .map(|c| c.name().to_ascii_lowercase())
        .collect()
}

fn pct(x: f64) -> String {
    format!("{x:+.2} %")
}

/// The coarsest E-series that contains `ohms` exactly.
fn series_of(ohms: f64) -> Option<ESeries> {
    use foxres_core::value::approx_eq;
    ESeries::ALL
        .into_iter()
        .find(|s| s.nearest(ohms).is_ok_and(|v| approx_eq(v, ohms)))
}

fn execute(command: &Command) -> Result<Output, Box<dyn std::error::Error>> {
    match command {
        Command::Bands { colors } => decode_bands(colors),
        Command::Colors {
            value,
            bands,
            tolerance,
            tempco,
        } => encode_bands(value, *bands, *tolerance, *tempco),
        Command::Smd { code } => {
            let d = smd::decode(code)?;
            Ok(Output {
                text: format!("{}\nNotation: {}", format_ohms(d.ohms), d.notation.label()),
                json: json!({ "code": code.trim().to_ascii_uppercase(), "ohms": d.ohms, "notation": d.notation.label() }),
            })
        }
        Command::Marking { value } => {
            let ohms = parse_ohms(value)?;
            let (three, four, eia) = (
                smd::encode_three(ohms),
                smd::encode_four(ohms),
                smd::encode_eia96(ohms),
            );
            let show = |c: &Option<String>| c.clone().unwrap_or_else(|| "—".into());
            Ok(Output {
                text: format!(
                    "{}\n3-digit: {}\n4-digit: {}\nEIA-96:  {}",
                    format_ohms(ohms),
                    show(&three),
                    show(&four),
                    show(&eia)
                ),
                json: json!({ "ohms": ohms, "three_digit": three, "four_digit": four, "eia96": eia }),
            })
        }
        Command::Nearest { value, series } => nearest(value, *series),
        Command::Combine { value, series } => combine(value, *series),
        Command::Ohm(a) => tools::ohm_law(a),
        Command::Power(a) => tools::power_check(a),
        Command::Divider(a) => tools::divider(a),
        Command::Led(a) => tools::led(a),
        Command::Series(a) => tools::net(a, false),
        Command::Parallel(a) => tools::net(a, true),
        Command::Tolerance(a) => tools::tolerance(a),
        Command::Drift(a) => tools::drift(a),
        Command::Noise(a) => tools::noise(a),
        Command::Rtd(a) => tools::rtd_cmd(a),
        Command::Ntc(a) => tools::ntc_cmd(a),
        Command::Loop(a) => tools::current_loop_cmd(a),
        Command::Bridge(a) => tools::bridge_cmd(a),
        Command::Wire(a) => tools::wire_cmd(a),
        Command::Trace(a) => tools::trace_cmd(a),
    }
}

fn decode_bands(colors: &[Color]) -> Result<Output, Box<dyn std::error::Error>> {
    let r = bands::decode(colors)?;
    let mut lines = vec![format_ohms(r.ohms)];
    if r.ohms > 0.0 {
        lines.push(format!(
            "Tolerance: ±{} % ({} to {})",
            format_sig(r.tolerance, 3),
            format_ohms(r.min()),
            format_ohms(r.max())
        ));
    } else {
        lines.push("Zero-ohm link".into());
    }
    if let Some(tc) = r.tempco {
        lines.push(format!("Tempco: {tc} ppm/K"));
    }
    let series = (r.ohms > 0.0).then(|| series_of(r.ohms)).flatten();
    if let Some(s) = series {
        lines.push(format!("Series: {}", s.name()));
    }
    Ok(Output {
        text: lines.join("\n"),
        json: json!({
            "bands": names(colors),
            "ohms": r.ohms,
            "tolerance_percent": r.tolerance,
            "min_ohms": r.min(),
            "max_ohms": r.max(),
            "tempco_ppm_per_k": r.tempco,
            "series": series.map(ESeries::name),
        }),
    })
}

fn encode_bands(
    value: &str,
    bands: u8,
    tolerance: Option<Color>,
    tempco: Color,
) -> Result<Output, Box<dyn std::error::Error>> {
    let ohms = parse_ohms(value)?;
    let count = BandCount::from_len(usize::from(bands))?;
    let roles = count.roles();
    let tolerance = tolerance.unwrap_or(if count.digits() == 3 {
        Color::Brown
    } else {
        Color::Gold
    });
    let colors = match bands::encode(ohms, count, tolerance, tempco) {
        Err(e @ Error::NotRepresentable(_)) if count.digits() == 2 => {
            let hint = if bands::encode_value(ohms, BandCount::Five).is_ok() {
                " (try --bands 5)"
            } else {
                ""
            };
            return Err(format!("{e}{hint}").into());
        }
        other => other?,
    };
    let mut text = names(&colors).join(" ");
    if colors.len() == 1 {
        text.push_str("  (zero-ohm link)");
    } else if roles.contains(&Role::Tolerance) {
        text = format!(
            "{text}\n{} ±{} %",
            format_ohms(ohms),
            format_sig(tolerance.tolerance().unwrap_or(20.0), 3)
        );
    } else {
        text = format!("{text}\n{} ±20 %", format_ohms(ohms));
    }
    Ok(Output {
        text,
        json: json!({ "ohms": ohms, "bands": names(&colors) }),
    })
}

fn nearest(value: &str, series: ESeries) -> Result<Output, Box<dyn std::error::Error>> {
    let target = parse_ohms(value)?;
    let near = series.nearest(target)?;
    let (lo, hi) = series.neighbors(target)?;
    Ok(Output {
        text: format!(
            "{} {} ({})\nBelow: {} ({})\nAbove: {} ({})",
            series.name(),
            format_ohms(near),
            pct(error_percent(near, target)),
            format_ohms(lo),
            pct(error_percent(lo, target)),
            format_ohms(hi),
            pct(error_percent(hi, target)),
        ),
        json: json!({
            "target_ohms": target,
            "series": series.name(),
            "nearest_ohms": near,
            "error_percent": error_percent(near, target),
            "below_ohms": lo,
            "above_ohms": hi,
        }),
    })
}

fn pair_json(p: &Pair) -> Value {
    json!({ "a_ohms": p.a, "b_ohms": p.b, "total_ohms": p.total, "error_percent": p.error })
}

fn pair_text(p: &Pair, sym: &str) -> String {
    format!(
        "{} {sym} {} = {} ({})",
        format_ohms(p.a),
        format_ohms(p.b),
        format_ohms(p.total),
        pct(p.error)
    )
}

fn combine(value: &str, series: ESeries) -> Result<Output, Box<dyn std::error::Error>> {
    let target = parse_ohms(value)?;
    let s = combo::best_pair(target, series, Connection::Series)?;
    let p = combo::best_pair(target, series, Connection::Parallel)?;
    let t = foxres_core::network::best_triple(target, series)?;
    // Three resistors are only worth suggesting when they beat the best pair.
    let improves = t.error.abs() < s.error.abs().min(p.error.abs()) - 1e-9;
    let three = if improves {
        format!(
            "{} = {} ({})",
            t.topology
                .describe(&format_ohms(t.a), &format_ohms(t.b), &format_ohms(t.c)),
            format_ohms(t.total),
            pct(t.error)
        )
    } else {
        "not needed, a pair is already as close".to_string()
    };
    Ok(Output {
        text: format!(
            "Target {} from {}\nSeries:   {}\nParallel: {}\nThree:    {}",
            format_ohms(target),
            series.name(),
            pair_text(&s, "+"),
            pair_text(&p, "∥"),
            three
        ),
        json: json!({
            "target_ohms": target,
            "series": series.name(),
            "series_pair": pair_json(&s),
            "parallel_pair": pair_json(&p),
            "triple": if improves { json!({ "topology": format!("{:?}", t.topology), "a_ohms": t.a, "b_ohms": t.b, "c_ohms": t.c, "total_ohms": t.total, "error_percent": t.error }) } else { Value::Null },
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(args: &[&str]) -> Output {
        let cli =
            Cli::try_parse_from(std::iter::once("foxres").chain(args.iter().copied())).unwrap();
        execute(&cli.command).unwrap()
    }

    #[test]
    fn bands_text() {
        let o = out(&["bands", "yellow", "violet", "red", "gold"]);
        assert!(o.text.starts_with("4.7 kΩ\n"), "{}", o.text);
        assert_eq!(o.json["series"], "E3");
    }

    #[test]
    fn colors_round_trip() {
        let o = out(&["colors", "4k7", "-n", "5"]);
        assert_eq!(
            o.json["bands"],
            json!(["yellow", "violet", "black", "brown", "brown"])
        );
    }

    #[test]
    fn smd_and_marking() {
        assert_eq!(out(&["smd", "01c"]).json["ohms"], 10_000.0);
        assert_eq!(out(&["marking", "4.99k"]).json["eia96"], "68B");
    }

    #[test]
    fn nearest_and_combine() {
        assert_eq!(
            out(&["nearest", "5k", "-s", "E96"]).json["nearest_ohms"],
            4990.0
        );
        let c = out(&["combine", "5.7k", "--series", "e12"]);
        assert!(
            c.json["series_pair"]["error_percent"]
                .as_f64()
                .unwrap()
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Cli::try_parse_from(["foxres", "bands", "teal", "red", "red"]).is_err());
        assert!(Cli::try_parse_from(["foxres", "colors", "1k", "-n", "7"]).is_err());
        let cli = Cli::try_parse_from(["foxres", "colors", "4750", "-n", "4"]).unwrap();
        assert!(execute(&cli.command).is_err());
    }

    #[test]
    fn design_commands() {
        let o = out(&["ohm", "-V", "12", "-R", "480"]);
        assert!((o.json["amps"].as_f64().unwrap() - 0.025).abs() < 1e-15);
        assert_eq!(
            out(&["power", "1k", "-P", "100m"]).json["smallest"]["SMD"],
            "1206"
        );
        let d = out(&["divider", "--ratio", "0.5"]);
        assert!(d.json["error_percent"].as_f64().unwrap().abs() < 1e-12);
        assert_eq!(
            out(&["led", "--supply", "5", "--vf", "2"]).json["ohms"],
            150.0
        );
        assert_eq!(out(&["parallel", "1k", "1k"]).json["ohms"], 500.0);
        assert_eq!(out(&["series", "1k", "2k2"]).json["ohms"], 3200.0);
        let r = out(&["rtd", "--temp", "100"]);
        assert!((r.json["ohms"].as_f64().unwrap() - 138.5055).abs() < 1e-9);
        let r = out(&["rtd", "--ohms", "1385.055", "--r0", "1k"]);
        assert!((r.json["temp_c"].as_f64().unwrap() - 100.0).abs() < 1e-9);
        let n = out(&["ntc", "--beta", "3950", "--temp", "25"]);
        assert!((n.json["ohms"].as_f64().unwrap() - 10_000.0).abs() < 1e-9);
        let l = out(&[
            "loop", "--ma", "12", "--range", "0:150", "--supply", "24", "--tx-min", "12",
        ]);
        assert_eq!(l.json["value"], 75.0);
        assert_eq!(l.json["max_loop_ohms"], 600.0);
        let t = out(&["tolerance", "series", "1k:1", "2k:1"]);
        assert!((t.json["max"].as_f64().unwrap() - 3030.0).abs() < 1e-9);
        assert!(
            out(&["bridge", "1k", "1k", "1k", "1k"]).json["volts"]
                .as_f64()
                .unwrap()
                .abs()
                < 1e-15
        );
        let w = out(&["wire", "--awg", "24", "--length", "1000"]);
        assert!((w.json["ohms"].as_f64().unwrap() - 84.2).abs() < 0.3);
        let tr = out(&["trace", "-I", "1", "--oz", "1"]);
        assert!((tr.json["max_amps"].as_f64().unwrap() - 1.0).abs() < 1e-9);
        let c = out(&["combine", "1234", "-s", "E12"]);
        assert!(
            c.json["triple"]["error_percent"].as_f64().unwrap().abs()
                <= c.json["series_pair"]["error_percent"]
                    .as_f64()
                    .unwrap()
                    .abs()
        );
    }

    #[test]
    fn triple_only_when_better() {
        // 5.7 kΩ is exact as 5.6 kΩ + 100 Ω, so no triple is offered.
        assert!(out(&["combine", "5.7k", "-s", "E24"]).json["triple"].is_null());
        // 1234 Ω has no exact E12 pair, and three resistors do better.
        let c = out(&["combine", "1234", "-s", "E12"]);
        let t = c.json["triple"]["error_percent"].as_f64().unwrap().abs();
        assert!(
            t < c.json["series_pair"]["error_percent"]
                .as_f64()
                .unwrap()
                .abs()
        );
    }
}
