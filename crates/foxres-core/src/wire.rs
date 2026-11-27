//! Resistance of wire and PCB traces, and IPC-2221 trace current capacity.

use crate::Error;

/// Conductor material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// Annealed copper to IACS: 1/58 Ω·mm²/m at 20 °C.
    Copper,
    /// Aluminium, EC grade.
    Aluminium,
}

impl Material {
    pub const ALL: [Material; 2] = [Material::Copper, Material::Aluminium];

    pub fn name(self) -> &'static str {
        match self {
            Material::Copper => "Copper",
            Material::Aluminium => "Aluminium",
        }
    }

    /// Resistivity at 20 °C, Ω·m.
    pub fn resistivity_20c(self) -> f64 {
        match self {
            Material::Copper => 1.0 / 58e6,
            Material::Aluminium => 2.826e-8,
        }
    }

    /// Temperature coefficient of resistance at 20 °C, 1/K.
    pub fn alpha_20c(self) -> f64 {
        match self {
            Material::Copper => 0.003_93,
            Material::Aluminium => 0.004_03,
        }
    }

    /// Resistivity at `temp_c`, Ω·m.
    pub fn resistivity(self, temp_c: f64) -> f64 {
        self.resistivity_20c() * (1.0 + self.alpha_20c() * (temp_c - 20.0))
    }
}

/// Diameter in mm of solid AWG `gauge`, from `0.127 mm × 92^((36 − n)/39)`.
/// The aught sizes are 0 (1/0), −1 (2/0), −2 (3/0) and −3 (4/0); see
/// [`awg_from_label`].
pub fn awg_diameter_mm(gauge: i32) -> f64 {
    0.127 * 92f64.powf(f64::from(36 - gauge) / 39.0)
}

/// Gauge number for a label such as `"24"`, `"1/0"`, `"0000"` or `"4/0"`.
///
/// # Errors
/// [`Error::Parse`] for anything outside 4/0 … 40.
pub fn awg_from_label(label: &str) -> Result<i32, Error> {
    let t = label.trim().to_ascii_lowercase();
    let t = t.strip_prefix("awg").unwrap_or(&t).trim();
    let g = match t {
        "0000" | "4/0" => -3,
        "000" | "3/0" => -2,
        "00" | "2/0" => -1,
        "0" | "1/0" => 0,
        n => n
            .parse::<i32>()
            .map_err(|_| Error::Parse(label.to_string()))?,
    };
    if (-3..=40).contains(&g) {
        Ok(g)
    } else {
        Err(Error::Parse(label.to_string()))
    }
}

/// Cross-section in mm² of a round conductor.
pub fn area_mm2(diameter_mm: f64) -> f64 {
    std::f64::consts::PI * diameter_mm * diameter_mm / 4.0
}

/// Resistance of `length_m` of conductor with `area_mm2` cross-section.
pub fn resistance(material: Material, area_mm2: f64, length_m: f64, temp_c: f64) -> f64 {
    material.resistivity(temp_c) * length_m / (area_mm2 * 1e-6)
}

/// Copper thickness of 1 oz/ft² foil, by industry convention 1.37 mil.
pub const OZ_COPPER_MM: f64 = 1.37 * 0.0254;

/// Resistance of a copper PCB trace.
pub fn trace_resistance(width_mm: f64, length_mm: f64, copper_oz: f64, temp_c: f64) -> f64 {
    let thickness_m = copper_oz * OZ_COPPER_MM * 1e-3;
    Material::Copper.resistivity(temp_c) * (length_mm * 1e-3) / (width_mm * 1e-3 * thickness_m)
}

/// Trace layer for IPC-2221.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    External,
    Internal,
}

impl Layer {
    fn k(self) -> f64 {
        match self {
            Layer::External => 0.048,
            Layer::Internal => 0.024,
        }
    }
}

/// IPC-2221 current capacity: `I = k·ΔT^0.44·A^0.725`, A in mil².
pub fn trace_current(width_mm: f64, copper_oz: f64, temp_rise_k: f64, layer: Layer) -> f64 {
    let area_mil2 = (width_mm / 0.0254) * (copper_oz * 1.37);
    layer.k() * temp_rise_k.powf(0.44) * area_mil2.powf(0.725)
}

/// Trace width in mm needed for `amps` (inverse of [`trace_current`]).
pub fn trace_width_for(amps: f64, copper_oz: f64, temp_rise_k: f64, layer: Layer) -> f64 {
    let area_mil2 = (amps / (layer.k() * temp_rise_k.powf(0.44))).powf(1.0 / 0.725);
    area_mil2 / (copper_oz * 1.37) * 0.0254
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awg_table_values() {
        // Diameter (mm) and copper resistance (Ω/km at 20 °C) from standard AWG tables.
        for (label, d, ohm_km) in [
            ("4/0", 11.684, 0.1608),
            ("10", 2.588, 3.277),
            ("14", 1.628, 8.286),
            ("18", 1.024, 20.95),
            ("24", 0.511, 84.22),
            ("30", 0.2546, 338.6),
        ] {
            let g = awg_from_label(label).unwrap();
            let dia = awg_diameter_mm(g);
            assert!((dia - d).abs() / d < 1e-3, "{label}: {dia}");
            let r = resistance(Material::Copper, area_mm2(dia), 1000.0, 20.0);
            assert!((r - ohm_km).abs() / ohm_km < 3e-3, "{label}: {r}");
        }
    }

    #[test]
    fn ipc_2221_round_trip() {
        let w = trace_width_for(1.0, 1.0, 10.0, Layer::External);
        assert!((trace_current(w, 1.0, 10.0, Layer::External) - 1.0).abs() < 1e-12);
        // 10 mil, 1 oz, 10 °C rise, external: about 0.88 A.
        let i = trace_current(0.254, 1.0, 10.0, Layer::External);
        assert!((i - 0.88).abs() < 0.02, "{i}");
    }

    #[test]
    fn trace_resistance_sanity() {
        // 1 oz, 0.254 mm wide, 25.4 mm long: about 49 mΩ at 20 °C.
        let r = trace_resistance(0.254, 25.4, 1.0, 20.0);
        assert!((r - 0.0495).abs() < 0.002, "{r}");
    }
}
