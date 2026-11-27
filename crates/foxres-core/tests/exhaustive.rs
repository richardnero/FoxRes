//! Exhaustive correctness checks.
//!
//! These tests do not sample: they walk every color-band combination, every
//! 3- and 4-digit SMD code and every EIA-96 code. Expected values come from
//! Rust's decimal parser (`"470e1".parse::<f64>()`), which is correctly rounded,
//! so a pass means each decoded value is the `f64` nearest the exact value.

use foxres_core::{
    BandCount, Color, ESeries,
    bands::{self, Role},
    combo::{self, Connection},
    eseries::error_percent,
    smd,
};

/// The `f64` nearest `mantissa × 10^exp`, from the decimal parser.
fn exact(mantissa: u64, exp: i32) -> f64 {
    format!("{mantissa}e{exp}").parse().unwrap()
}

fn for_each_combination(roles: &[Role], f: &mut impl FnMut(&[Color])) {
    fn walk(roles: &[Role], acc: &mut Vec<Color>, f: &mut impl FnMut(&[Color])) {
        match roles.split_first() {
            None => f(acc),
            Some((role, rest)) => {
                for c in role.choices() {
                    acc.push(c);
                    walk(rest, acc, f);
                    acc.pop();
                }
            }
        }
    }
    walk(roles, &mut Vec::new(), f);
}

#[test]
fn every_color_code_decodes_exactly_and_round_trips() {
    let mut checked = 0u64;
    for count in BandCount::CHOICES {
        let roles = count.roles();
        let digits = count.digits();
        for_each_combination(roles, &mut |colors| {
            let r = bands::decode(colors).unwrap();

            let mantissa = colors[..digits]
                .iter()
                .fold(0u64, |m, c| m * 10 + u64::from(c.digit().unwrap()));
            let exp = colors[digits].multiplier_exp().unwrap();
            assert_eq!(
                r.ohms.to_bits(),
                exact(mantissa, exp).to_bits(),
                "{colors:?}"
            );

            let tolerance = colors.get(digits + 1).copied().unwrap_or(Color::None);
            assert_eq!(r.tolerance, tolerance.tolerance().unwrap(), "{colors:?}");
            assert_eq!(
                r.tempco,
                colors.get(digits + 2).and_then(|c| c.tempco()),
                "{colors:?}"
            );

            let tempco = colors.get(digits + 2).copied().unwrap_or(Color::Brown);
            let encoded = bands::encode(r.ohms, count, tolerance, tempco).unwrap();
            if mantissa == 0 {
                assert_eq!(encoded, vec![Color::Black]);
            } else {
                assert_eq!(
                    bands::decode(&encoded).unwrap().ohms.to_bits(),
                    r.ohms.to_bits()
                );
                // With a non-zero first digit the code is canonical, so encoding
                // must give back exactly the bands we started with.
                if colors[0] != Color::Black {
                    assert_eq!(encoded, colors, "{colors:?}");
                }
            }
            checked += 1;
        });
    }
    // 3, 4, 5 and 6-band combinations: 1,300 + 14,300 + 143,000 + 1,287,000.
    assert_eq!(checked, 1_445_600);
}

#[test]
fn every_three_and_four_digit_smd_code() {
    for (len, count) in [(3usize, 1_000u64), (4, 10_000)] {
        for n in 0..count {
            let code = format!("{n:0len$}");
            let d = smd::decode(&code).unwrap();
            let (mantissa, exp) = (n / 10, (n % 10) as i32);
            let want = if mantissa == 0 {
                0.0
            } else {
                exact(mantissa, exp)
            };
            assert_eq!(d.ohms.to_bits(), want.to_bits(), "{code}");

            let encode = if len == 3 {
                smd::encode_three
            } else {
                smd::encode_four
            };
            let canonical = mantissa >= 10u64.pow(len as u32 - 2);
            match encode(d.ohms) {
                Some(again) if canonical => assert_eq!(again, code, "{code}"),
                Some(again) => assert_eq!(
                    smd::decode(&again).unwrap().ohms.to_bits(),
                    d.ohms.to_bits(),
                    "{code} -> {again}"
                ),
                None => panic!("{code} ({}) has no {len}-character marking", d.ohms),
            }
        }
    }
}

#[test]
fn every_eia96_code() {
    let letters = [
        ('Z', -3),
        ('Y', -2),
        ('X', -1),
        ('A', 0),
        ('B', 1),
        ('C', 2),
        ('D', 3),
        ('E', 4),
        ('F', 5),
    ];
    let mut checked = 0;
    for (index, &mantissa) in ESeries::E96.values().iter().enumerate() {
        for (letter, exp) in letters {
            let code = format!("{:02}{letter}", index + 1);
            let d = smd::decode(&code).unwrap();
            assert_eq!(
                d.ohms.to_bits(),
                exact(u64::from(mantissa), exp).to_bits(),
                "{code}"
            );
            assert_eq!(smd::encode_eia96(d.ohms).as_deref(), Some(code.as_str()));
            checked += 1;
        }
        // Older alternative letters decode to the same values.
        for (alt, main) in [('S', 'X'), ('H', 'B')] {
            let a = smd::decode(&format!("{:02}{alt}", index + 1)).unwrap();
            let m = smd::decode(&format!("{:02}{main}", index + 1)).unwrap();
            assert_eq!(a.ohms.to_bits(), m.ohms.to_bits());
        }
    }
    assert_eq!(checked, 96 * 9);
}

/// IEC 60063 values are `10^(i/n)` rounded to 2 or 3 significant digits,
/// except where the standard keeps historical values. Every departure from the
/// formula must be on this list, so a typo anywhere in a table fails the test.
#[test]
fn eseries_tables_match_iec_60063() {
    let departures: [(ESeries, &[u16]); 7] = [
        (ESeries::E3, &[47]),
        (ESeries::E6, &[33, 47]),
        (ESeries::E12, &[27, 33, 39, 47, 82]),
        (ESeries::E24, &[27, 30, 33, 36, 39, 43, 47, 82]),
        (ESeries::E48, &[]),
        (ESeries::E96, &[]),
        (ESeries::E192, &[920]),
    ];
    for (series, expected) in departures {
        let n = series.values().len();
        let scale = 10f64.powi(series.digits() - 1);
        let mut found = Vec::new();
        for (i, &v) in series.values().iter().enumerate() {
            let formula = (10f64.powf(i as f64 / n as f64) * scale).round() as u16;
            if formula != v {
                found.push(v);
                // Departures are never more than one step in the last digit.
                assert_eq!(
                    formula.abs_diff(v),
                    1,
                    "{} value {v} vs formula {formula}",
                    series.name()
                );
            }
        }
        assert_eq!(found, expected, "{}", series.name());
    }
}

#[test]
fn eseries_nesting() {
    // Each coarser series is every second value of the next finer one.
    for chain in [
        &[ESeries::E3, ESeries::E6, ESeries::E12, ESeries::E24][..],
        &[ESeries::E48, ESeries::E96, ESeries::E192][..],
    ] {
        for w in chain.windows(2) {
            let every_other: Vec<u16> = w[1].values().iter().step_by(2).copied().collect();
            assert_eq!(
                w[0].values(),
                every_other.as_slice(),
                "{} in {}",
                w[0].name(),
                w[1].name()
            );
        }
    }
}

#[test]
fn nearest_is_never_beaten_by_another_series_value() {
    for series in ESeries::ALL {
        for k in 0..600 {
            let target = 10f64.powf(-1.0 + f64::from(k) * 0.0137);
            let near = series.nearest(target).unwrap();
            let log_err = (near / target).ln().abs();
            for v in series.range(target / 10.0, target * 10.0) {
                assert!(
                    log_err <= (v / target).ln().abs() + 1e-12,
                    "{} {target}: {near} beaten by {v}",
                    series.name()
                );
            }
        }
    }
}

#[test]
fn best_pair_matches_brute_force() {
    for series in [ESeries::E6, ESeries::E12, ESeries::E24] {
        for k in 0..40 {
            let target = 10f64.powf(0.5 + f64::from(k) * 0.123);
            for connection in [Connection::Series, Connection::Parallel] {
                let found = combo::best_pair(target, series, connection).unwrap();
                let pool = match connection {
                    Connection::Series => series.range(target / 1000.0, target),
                    Connection::Parallel => series.range(target, target * 1000.0),
                };
                let mut best = f64::INFINITY;
                for &a in &pool {
                    for &b in &pool {
                        best = best.min(error_percent(connection.combine(a, b), target).abs());
                    }
                }
                assert!(
                    found.error.abs() <= best + 1e-9,
                    "{} {connection:?} {target}: found {:.6} %, brute force {best:.6} %",
                    series.name(),
                    found.error
                );
            }
        }
    }
}
