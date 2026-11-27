//! Ohm's law and power: give any two of voltage, current, resistance and
//! power, get the other two.

use crate::Error;

/// A fully solved resistor operating point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circuit {
    pub volts: f64,
    pub amps: f64,
    pub ohms: f64,
    pub watts: f64,
}

fn check(name: &str, v: Option<f64>) -> Result<(), Error> {
    match v {
        Some(x) if !x.is_finite() || x < 0.0 => Err(Error::Invalid(format!(
            "{name} must be a finite, non-negative number"
        ))),
        _ => Ok(()),
    }
}

/// Solve from exactly two known quantities.
///
/// # Errors
/// [`Error::Invalid`] unless exactly two values are given, all are finite and
/// non-negative, and the answer does not need a division by zero.
pub fn solve(
    volts: Option<f64>,
    amps: Option<f64>,
    ohms: Option<f64>,
    watts: Option<f64>,
) -> Result<Circuit, Error> {
    check("Voltage", volts)?;
    check("Current", amps)?;
    check("Resistance", ohms)?;
    check("Power", watts)?;
    let zero = |what: &str| Error::Invalid(format!("{what} cannot be zero here"));
    let c = match (volts, amps, ohms, watts) {
        (Some(v), Some(i), None, None) => {
            if i == 0.0 {
                return Err(zero("Current"));
            }
            Circuit {
                volts: v,
                amps: i,
                ohms: v / i,
                watts: v * i,
            }
        }
        (Some(v), None, Some(r), None) => {
            if r == 0.0 {
                return Err(zero("Resistance"));
            }
            Circuit {
                volts: v,
                amps: v / r,
                ohms: r,
                watts: v * v / r,
            }
        }
        (Some(v), None, None, Some(p)) => {
            if v == 0.0 {
                return Err(zero("Voltage"));
            }
            Circuit {
                volts: v,
                amps: p / v,
                ohms: v * v / p,
                watts: p,
            }
        }
        (None, Some(i), Some(r), None) => Circuit {
            volts: i * r,
            amps: i,
            ohms: r,
            watts: i * i * r,
        },
        (None, Some(i), None, Some(p)) => {
            if i == 0.0 {
                return Err(zero("Current"));
            }
            Circuit {
                volts: p / i,
                amps: i,
                ohms: p / (i * i),
                watts: p,
            }
        }
        (None, None, Some(r), Some(p)) => {
            if r == 0.0 {
                return Err(zero("Resistance"));
            }
            Circuit {
                volts: (p * r).sqrt(),
                amps: (p / r).sqrt(),
                ohms: r,
                watts: p,
            }
        }
        _ => {
            return Err(Error::Invalid(
                "give exactly two of voltage, current, resistance and power".into(),
            ));
        }
    };
    if [c.volts, c.amps, c.ohms, c.watts]
        .iter()
        .all(|x| x.is_finite())
    {
        Ok(c)
    } else {
        Err(Error::Invalid(
            "those values have no finite solution".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::approx_eq;

    #[test]
    fn every_pair_agrees() {
        let truth = Circuit {
            volts: 12.0,
            amps: 0.025,
            ohms: 480.0,
            watts: 0.3,
        };
        let all = [
            Some(truth.volts),
            Some(truth.amps),
            Some(truth.ohms),
            Some(truth.watts),
        ];
        for a in 0..4 {
            for b in a + 1..4 {
                let mut given = [None; 4];
                given[a] = all[a];
                given[b] = all[b];
                let c = solve(given[0], given[1], given[2], given[3]).unwrap();
                for (got, want) in [c.volts, c.amps, c.ohms, c.watts]
                    .iter()
                    .zip([12.0, 0.025, 480.0, 0.3])
                {
                    assert!(approx_eq(*got, want), "pair {a},{b}: {got} vs {want}");
                }
            }
        }
    }

    #[test]
    fn rejects() {
        assert!(solve(Some(1.0), None, None, None).is_err());
        assert!(solve(Some(1.0), Some(1.0), Some(1.0), None).is_err());
        assert!(solve(Some(1.0), Some(0.0), None, None).is_err());
        assert!(solve(Some(-1.0), Some(1.0), None, None).is_err());
    }
}
