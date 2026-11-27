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

#[derive(Debug, Parser)]
#[command(
    name = "foxres",
    version,
    about = "Resistor calculator: color bands, SMD codes, E-series values and combinations",
    after_help = "Run with no arguments to open the window.\n\n\
        Values accept engineering notation: 4700, 4.7k, 4k7, 100R, R47, 2M2, 10m (milliohms).\n\n\
        Examples:\n  \
        foxres bands yellow violet red gold\n  \
        foxres colors 4k7 --bands 5\n  \
        foxres smd 01C\n  \
        foxres marking 4.99k\n  \
        foxres nearest 5k --series E96\n  \
        foxres combine 5.7k --series E12 --json"
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
    /// Find the best two-resistor series and parallel combinations.
    Combine {
        #[arg(value_name = "VALUE")]
        value: String,
        /// E-series to use (E3, E6, E12, E24, E48, E96, E192).
        #[arg(short, long, default_value = "E24")]
        series: ESeries,
    },
}

/// Result of a command: what to print as text and as JSON.
struct Output {
    text: String,
    json: Value,
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
    Ok(Output {
        text: format!(
            "Target {} from {}\nSeries:   {}\nParallel: {}",
            format_ohms(target),
            series.name(),
            pair_text(&s, "+"),
            pair_text(&p, "∥")
        ),
        json: json!({
            "target_ohms": target,
            "series": series.name(),
            "series_pair": pair_json(&s),
            "parallel_pair": pair_json(&p),
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
}
