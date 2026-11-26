//! Draws a through-hole resistor with its bands.

use foxres_core::Color;
use relm4::gtk::cairo::Context;

fn rgb(cr: &Context, (r, g, b): (u8, u8, u8)) {
    cr.set_source_rgb(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
}

fn rounded_rect(cr: &Context, left: f64, top: f64, width: f64, height: f64, radius: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let (right, bottom) = (left + width, top + height);
    cr.new_sub_path();
    cr.arc(right - radius, top + radius, radius, -FRAC_PI_2, 0.0);
    cr.arc(right - radius, bottom - radius, radius, 0.0, FRAC_PI_2);
    cr.arc(left + radius, bottom - radius, radius, FRAC_PI_2, PI);
    cr.arc(left + radius, top + radius, radius, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

/// Draw `colors` on a resistor body filling `width` × `height`.
pub fn resistor(cr: &Context, width: i32, height: i32, colors: &[Color]) {
    let (w, h) = (f64::from(width), f64::from(height));
    let body_w = (w * 0.62).min(420.0);
    let body_h = (h * 0.62).min(90.0);
    let x0 = (w - body_w) / 2.0;
    let y0 = (h - body_h) / 2.0;

    // Leads.
    cr.set_source_rgb(0.62, 0.64, 0.66);
    cr.set_line_width((body_h * 0.08).max(2.0));
    cr.move_to(w * 0.04, h / 2.0);
    cr.line_to(w * 0.96, h / 2.0);
    let _ = cr.stroke();

    // Body.
    rgb(cr, Color::None.rgb());
    rounded_rect(cr, x0, y0, body_w, body_h, body_h * 0.35);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    // Bands, evenly spaced, with an extra gap before the tolerance band.
    let n = colors.len();
    let tolerance_at = match n {
        4 => Some(3),
        5 | 6 => Some(4),
        _ => None,
    };
    let slots = n + usize::from(tolerance_at.is_some());
    if slots == 0 {
        return;
    }
    let band_w = body_w * 0.07;
    let step = body_w * 0.76 / slots as f64;
    for (i, &c) in colors.iter().enumerate() {
        if c == Color::None {
            continue;
        }
        let slot = if tolerance_at.is_some_and(|t| i >= t) {
            i + 1
        } else {
            i
        };
        let x = x0 + body_w * 0.12 + step * (slot as f64 + 0.5) - band_w / 2.0;
        rgb(cr, c.rgb());
        cr.rectangle(x, y0, band_w, body_h);
        let _ = cr.fill();
    }
}
