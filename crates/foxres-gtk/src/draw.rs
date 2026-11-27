//! Draws a through-hole resistor with its bands.

use foxres_core::Color;
use relm4::gtk::cairo::{Context, LinearGradient};

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

/// Vertical light-to-shadow gradient that makes a flat shape read as a cylinder.
fn cylinder_shading(top: f64, height: f64, strength: f64) -> LinearGradient {
    let g = LinearGradient::new(0.0, top, 0.0, top + height);
    g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.10 * strength);
    g.add_color_stop_rgba(0.22, 1.0, 1.0, 1.0, 0.45 * strength);
    g.add_color_stop_rgba(0.45, 1.0, 1.0, 1.0, 0.0);
    g.add_color_stop_rgba(0.75, 0.0, 0.0, 0.0, 0.08 * strength);
    g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.35 * strength);
    g
}

/// Draw `colors` on a resistor body filling `width` × `height`.
pub fn resistor(cr: &Context, width: i32, height: i32, colors: &[Color]) {
    let (w, h) = (f64::from(width), f64::from(height));
    let body_w = (w * 0.62).min(420.0);
    let body_h = (h * 0.62).min(90.0);
    let x0 = (w - body_w) / 2.0;
    let y0 = (h - body_h) / 2.0;

    // Leads: tinned copper wire.
    let lead = (body_h * 0.09).max(3.0);
    let lead_top = h / 2.0 - lead / 2.0;
    for (from, to) in [(w * 0.03, x0 + 4.0), (x0 + body_w - 4.0, w * 0.97)] {
        cr.rectangle(from, lead_top, to - from, lead);
        cr.set_source_rgb(0.66, 0.68, 0.70);
        let _ = cr.fill_preserve();
        let _ = cr.set_source(cylinder_shading(lead_top, lead, 1.2));
        let _ = cr.fill();
    }

    // Body.
    let radius = body_h * 0.38;
    rounded_rect(cr, x0, y0, body_w, body_h, radius);
    rgb(cr, Color::None.rgb());
    let _ = cr.fill_preserve();
    let _ = cr.save();
    cr.clip();

    // Bands, evenly spaced, with an extra gap before the tolerance band.
    let n = colors.len();
    let tolerance_at = match n {
        4 => Some(3),
        5 | 6 => Some(4),
        _ => None,
    };
    let slots = n + usize::from(tolerance_at.is_some());
    if slots > 0 {
        let band_w = body_w * 0.072;
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

    // Shade the whole body, bands included, as one cylinder.
    let _ = cr.set_source(cylinder_shading(y0, body_h, 1.0));
    let _ = cr.paint();
    let _ = cr.restore();

    rounded_rect(cr, x0, y0, body_w, body_h, radius);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.28);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// A small round color sample for a band dropdown.
pub fn swatch(cr: &Context, width: i32, height: i32, color: Color) {
    let (w, h) = (f64::from(width), f64::from(height));
    let r = w.min(h) / 2.0 - 1.0;
    cr.arc(w / 2.0, h / 2.0, r, 0.0, std::f64::consts::TAU);
    rgb(cr, color.rgb());
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}
