//! Cross-checks for the design tools: brute force, reference tables and
//! algebraic identities.

use foxres_core::{
    ESeries, divider,
    eseries::error_percent,
    led,
    network::{self, Topology},
    ohm, rtd,
    value::approx_eq,
};

#[test]
fn divider_pair_matches_brute_force() {
    for series in [ESeries::E6, ESeries::E12, ESeries::E24] {
        let pool = series.range(10.0, 1e6);
        for k in 1..60 {
            let target = f64::from(k) / 61.0;
            let found = divider::best_pair(target, series, 10_000.0).unwrap();
            let mut best = f64::INFINITY;
            for &r1 in &pool {
                for &r2 in &pool {
                    best = best.min(error_percent(divider::ratio(r1, r2), target).abs());
                }
            }
            assert!(
                found.error.abs() <= best + 1e-9,
                "{} ratio {target}: found {} %, brute force {best} %",
                series.name(),
                found.error
            );
        }
    }
}

#[test]
fn best_triple_matches_brute_force() {
    let series = ESeries::E6;
    for target in [1234.0, 3300.0, 5000.0, 7777.0, 45.6] {
        let found = network::best_triple(target, series).unwrap();
        assert!(approx_eq(
            found.topology.combine(found.a, found.b, found.c),
            found.total
        ));
        let pool = series.range(target / 1000.0, target * 1000.0);
        let mut best = f64::INFINITY;
        for t in Topology::ALL {
            for &a in &pool {
                for &b in &pool {
                    for &c in &pool {
                        best = best.min(error_percent(t.combine(a, b, c), target).abs());
                    }
                }
            }
        }
        assert!(
            found.error.abs() <= best + 1e-9,
            "{target}: found {} %, brute force {best} %",
            found.error
        );
    }
}

#[test]
fn pt100_matches_iec_60751_table() {
    // Printed IEC 60751 Pt100 table values (Ω), 0.01 Ω resolution.
    let table = [
        (-200.0, 18.52),
        (-100.0, 60.26),
        (-50.0, 80.31),
        (0.0, 100.00),
        (50.0, 119.40),
        (100.0, 138.51),
        (200.0, 175.86),
        (300.0, 212.05),
        (400.0, 247.09),
        (500.0, 280.98),
        (600.0, 313.71),
        (700.0, 345.28),
        (800.0, 375.70),
        (850.0, 390.48),
    ];
    for (t, r) in table {
        let got = rtd::resistance(100.0, t).unwrap();
        assert!((got - r).abs() <= 0.005 + 1e-9, "{t} °C: {got} vs {r}");
        // The printed 18.52 Ω at −200 °C is rounded down from 18.52008 Ω, so it
        // lies just outside the standard's range; every other entry inverts.
        if t > rtd::MIN_C {
            let back = rtd::temperature(100.0, r).unwrap();
            // 0.005 Ω of table rounding is at most about 0.02 K.
            assert!((back - t).abs() < 0.02, "{r} Ω: {back} vs {t}");
        } else {
            assert!(rtd::temperature(100.0, r).is_err());
        }
    }
    assert!(rtd::resistance(100.0, -200.1).is_err());
    assert!(rtd::resistance(100.0, 850.1).is_err());
}

#[test]
fn ohms_law_is_self_consistent_on_a_grid() {
    let values = [1e-3, 0.1, 1.0, 3.3, 12.0, 47.0, 1e3, 2.2e5];
    for &v in &values {
        for &r in &values {
            let c = ohm::solve(Some(v), None, Some(r), None).unwrap();
            for (a, b) in [
                (
                    ohm::solve(None, Some(c.amps), None, Some(c.watts)).unwrap(),
                    "I,P",
                ),
                (
                    ohm::solve(Some(c.volts), Some(c.amps), None, None).unwrap(),
                    "V,I",
                ),
                (
                    ohm::solve(None, None, Some(c.ohms), Some(c.watts)).unwrap(),
                    "R,P",
                ),
            ] {
                let close = |x: f64, y: f64| (x - y).abs() <= 1e-12 * x.abs().max(y.abs());
                assert!(
                    close(a.volts, v) && close(a.ohms, r),
                    "{b}: {a:?} for V={v} R={r}"
                );
            }
        }
    }
}

#[test]
fn led_current_never_exceeds_target() {
    for supply in [3.3, 5.0, 9.0, 12.0, 24.0] {
        for (vf, count) in [(1.8, 1), (2.0, 2), (3.1, 1), (3.1, 3)] {
            for ma in [1.0, 2.0, 5.0, 10.0, 20.0, 30.0] {
                if let Ok(r) = led::size(supply, vf, count, ma / 1000.0, ESeries::E24) {
                    assert!(
                        r.amps <= ma / 1000.0 * (1.0 + 1e-12),
                        "{supply} V {vf}×{count} {ma} mA"
                    );
                    assert!(r.ohms >= r.ideal_ohms * (1.0 - 1e-12));
                }
            }
        }
    }
}
